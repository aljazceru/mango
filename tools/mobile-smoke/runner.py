#!/usr/bin/env python3
import argparse
import json
import os
import re
import shlex
import signal
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml


class SmokeError(RuntimeError):
    pass


def log(message: str) -> None:
    print(f"[smoke] {message}", flush=True)


def sanitize_input_text(text: str) -> str:
    return text.replace(" ", "%s")


def ensure_hierarchy(xml_text: str) -> str:
    end = xml_text.find("</hierarchy>")
    if end == -1:
        raise SmokeError("missing </hierarchy> in UI dump")
    return xml_text[: end + len("</hierarchy>")]


@dataclass
class Recorder:
    adb: "Adb"
    remote_path: str
    local_path: Path
    process: subprocess.Popen[str] | None = None

    def start(self) -> None:
        self.adb.run(["shell", "rm", "-f", self.remote_path], check=False)
        self.process = subprocess.Popen(
            self.adb.base_cmd + ["shell", "screenrecord", self.remote_path],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        time.sleep(1)

    def stop(self) -> None:
        if self.process is not None and self.process.poll() is None:
            self.process.send_signal(signal.SIGINT)
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.terminate()
                self.process.wait(timeout=10)
        self.local_path.parent.mkdir(parents=True, exist_ok=True)
        self.adb.run(["pull", self.remote_path, str(self.local_path)])


class Adb:
    def __init__(self, adb_bin: str, serial: str | None) -> None:
        self.adb_bin = adb_bin
        self.serial = serial

    @property
    def base_cmd(self) -> list[str]:
        cmd = [self.adb_bin]
        if self.serial:
            cmd += ["-s", self.serial]
        return cmd

    def run(
        self,
        args: list[str],
        *,
        check: bool = True,
        capture: bool = True,
        timeout: int | None = None,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            self.base_cmd + args,
            check=check,
            capture_output=capture,
            text=True,
            timeout=timeout,
        )

    def shell(self, command: str, *, check: bool = True) -> str:
        result = self.run(["shell", command], check=check)
        return result.stdout.strip()

    def exec_out(self, args: list[str], *, check: bool = True) -> bytes:
        result = subprocess.run(
            self.base_cmd + ["exec-out"] + args,
            check=check,
            capture_output=True,
        )
        return result.stdout


class UiState:
    def __init__(self, xml_path: Path) -> None:
        self.xml_path = xml_path
        raw = ensure_hierarchy(xml_path.read_text(encoding="utf-8"))
        self.root = ET.fromstring(raw)
        self.parents: dict[ET.Element, ET.Element] = {}
        self._index_parents(self.root)

    def _index_parents(self, node: ET.Element) -> None:
        for child in list(node):
            self.parents[child] = node
            self._index_parents(child)

    def nodes(self) -> list[ET.Element]:
        return list(self.root.iter("node"))

    def has_text(self, text: str) -> bool:
        return any(node.attrib.get("text", "") == text for node in self.nodes())

    def has_desc(self, desc: str) -> bool:
        return any(node.attrib.get("content-desc", "") == desc for node in self.nodes())

    def package(self) -> str | None:
        for node in self.nodes():
            pkg = node.attrib.get("package")
            if pkg:
                return pkg
        return None

    def bounds_for(self, node: ET.Element) -> tuple[int, int, int, int]:
        bounds = node.attrib.get("bounds", "")
        m = re.match(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", bounds)
        if not m:
            raise SmokeError(f"node has no bounds: {node.attrib}")
        return tuple(map(int, m.groups()))

    def center_for(self, node: ET.Element) -> tuple[int, int]:
        x1, y1, x2, y2 = self.bounds_for(node)
        return ((x1 + x2) // 2, (y1 + y2) // 2)

    def find_center(self, kind: str, value: str) -> tuple[int, int]:
        return self.center_for(self.find_node(kind, value))

    def find_input_center_for_label(self, label: str) -> tuple[int, int]:
        label_node = self.find_node("text", label)
        node = label_node
        while node in self.parents:
            node = self.parents[node]
            if node.attrib.get("class") == "android.widget.EditText":
                return self.center_for(node)

        label_x, label_y = self.center_for(label_node)
        candidates: list[tuple[int, int, int, ET.Element]] = []
        for input_node in self.nodes():
            if input_node.attrib.get("class") != "android.widget.EditText":
                continue
            x1, y1, x2, y2 = self.bounds_for(input_node)
            if x1 <= label_x <= x2 and y1 <= label_y <= y2:
                return self.center_for(input_node)
            vertical_gap = max(y1 - label_y, label_y - y2, 0)
            horizontal_gap = max(x1 - label_x, label_x - x2, 0)
            center_y = (y1 + y2) // 2
            candidates.append((vertical_gap, horizontal_gap, abs(center_y - label_y), input_node))

        if candidates:
            vertical_gap, _, _, input_node = min(candidates, key=lambda item: item[:3])
            if vertical_gap <= 120:
                return self.center_for(input_node)
        raise SmokeError(f"could not find input for label: {label}")

    def is_checked(self, kind: str, value: str) -> bool:
        node = self.find_node(kind, value)
        return node.attrib.get("checked", "false") == "true"

    def find_node(self, kind: str, value: str) -> ET.Element:
        def match(node: ET.Element) -> bool:
            text = node.attrib.get("text", "")
            desc = node.attrib.get("content-desc", "")
            if kind == "text":
                return text == value
            if kind == "desc":
                return desc == value
            if kind == "text_contains":
                return value in text
            if kind == "desc_contains":
                return value in desc
            if kind == "either_contains":
                return value in text or value in desc
            raise SmokeError(f"unsupported selector kind: {kind}")

        for node in self.nodes():
            if not match(node):
                continue
            return node
        raise SmokeError(f"could not find node ({kind}): {value}")


class Runner:
    def __init__(
        self,
        adb: Adb,
        profile: dict[str, Any],
        scenario: dict[str, Any],
        artifact_dir: Path,
        *,
        record: bool,
        install_apk: bool,
        reset_app_data: bool,
        env: dict[str, str],
    ) -> None:
        self.adb = adb
        self.profile = profile
        self.scenario = scenario
        self.artifact_dir = artifact_dir
        self.record = record
        self.install_apk = install_apk
        self.reset_app_data = reset_app_data
        self.env = env
        self.xml_counter = 0
        self.recorder: Recorder | None = None
        self.screen_size: tuple[int, int] | None = None

    @property
    def package(self) -> str:
        return self.profile["package"]

    @property
    def activity(self) -> str:
        return self.profile["activity"]

    @property
    def release_package(self) -> str | None:
        return self.profile.get("release_package")

    def resolve_value(self, value: str) -> str:
        return re.sub(
            r"\{\{([A-Z0-9_]+)\}\}",
            lambda m: self.env.get(m.group(1), m.group(0)),
            value,
        )

    def dump_ui(self, label: str) -> UiState:
        self.xml_counter += 1
        safe = re.sub(r"[^a-zA-Z0-9_.-]+", "-", label).strip("-") or "step"
        path = self.artifact_dir / f"{self.xml_counter:03d}-{safe}.xml"
        output = b""
        for attempt in range(15):
            output = self.adb.exec_out(["uiautomator", "dump", "/dev/tty"])
            if b"</hierarchy>" in output:
                break
            if attempt < 14:
                time.sleep(1)
        path.write_bytes(output)
        return UiState(path)

    def screenshot(self, label: str) -> Path:
        safe = re.sub(r"[^a-zA-Z0-9_.-]+", "-", label).strip("-") or "shot"
        path = self.artifact_dir / f"{safe}.png"
        path.write_bytes(self.adb.exec_out(["screencap", "-p"]))
        return path

    def get_screen_size(self) -> tuple[int, int]:
        if self.screen_size is None:
            size = self.adb.shell("wm size")
            m = re.search(r"Physical size:\s*(\d+)x(\d+)", size)
            if not m:
                raise SmokeError("could not read device screen size")
            self.screen_size = (int(m.group(1)), int(m.group(2)))
        return self.screen_size

    def tap_center(self, x: int, y: int) -> None:
        self.adb.shell(f"input tap {x} {y}")

    def tap_percent(self, px: int, py: int) -> None:
        width, height = self.get_screen_size()
        self.tap_center(width * px // 100, height * py // 100)

    def swipe_percent(self, x1: int, y1: int, x2: int, y2: int, duration_ms: int = 500) -> None:
        width, height = self.get_screen_size()
        self.adb.shell(
            f"input swipe {width * x1 // 100} {height * y1 // 100} "
            f"{width * x2 // 100} {height * y2 // 100} {duration_ms}"
        )

    def tap_selector(self, state: UiState, kind: str, value: str) -> None:
        x, y = state.find_center(kind, value)
        self.tap_center(x, y)

    def input_text(self, text: str) -> None:
        self.adb.run(["shell", "input", "text", sanitize_input_text(text)])

    def focus_labeled_input(self, state: UiState, label: str) -> UiState:
        last_error: SmokeError | None = None
        for attempt in range(8):
            try:
                x, y = state.find_input_center_for_label(label)
                _, height = self.get_screen_size()
                if 40 <= y <= height - 40:
                    self.tap_center(x, y)
                    return state
                if y > height - 160:
                    self.swipe_percent(50, 74, 50, 40)
                else:
                    self.swipe_percent(50, 36, 50, 70)
            except SmokeError as exc:
                last_error = exc
                self.swipe_percent(50, 74, 50, 40)
            time.sleep(0.4)
            state = self.dump_ui(f"focus-{label}-{attempt + 1}")
        if last_error is not None:
            raise last_error
        raise SmokeError(f"could not bring input into view: {label}")

    def wait_for_foreground(self, package: str, tries: int = 20) -> None:
        for _ in range(tries):
            out = self.adb.shell("dumpsys activity activities")
            packages = [
                match.group(1)
                for pattern in [
                    r"topResumedActivity=.* u0 ([^/]+)/",
                    r"mResumedActivity: .* u0 ([^/]+)/",
                    r"ResumedActivity: .* u0 ([^/]+)/",
                ]
                for match in re.finditer(pattern, out)
            ]
            if package in packages:
                return
            time.sleep(1)
        raise SmokeError(f"foreground package did not become {package}")

    def wait_for_ui(
        self,
        *,
        text: str | None = None,
        desc: str | None = None,
        package_any: list[str] | None = None,
        tries: int = 20,
        label: str = "wait",
    ) -> UiState:
        for _ in range(tries):
            state = self.dump_ui(label)
            if text is not None and state.has_text(text):
                return state
            if desc is not None and state.has_desc(desc):
                return state
            if package_any is not None and state.package() in package_any:
                return state
            time.sleep(1)
        target = text or desc or "|".join(package_any or [])
        raise SmokeError(f"timed out waiting for '{target}'")

    def scroll_to_ui(
        self,
        *,
        text: str | None = None,
        desc: str | None = None,
        tries: int = 6,
        label: str = "scroll",
    ) -> UiState:
        for attempt in range(tries):
            state = self.dump_ui(f"{label}-{attempt + 1}")
            if text is not None and state.has_text(text):
                return state
            if desc is not None and state.has_desc(desc):
                return state
            self.swipe_percent(50, 82, 50, 28)
            time.sleep(0.5)
        target = text or desc or ""
        raise SmokeError(f"timed out scrolling to '{target}'")

    def clear_logcat(self) -> None:
        self.adb.run(["logcat", "-c"])

    def save_logcat(self) -> Path:
        path = self.artifact_dir / "logcat.txt"
        path.write_text(self.adb.run(["logcat", "-d", "-v", "brief"]).stdout, encoding="utf-8")
        return path

    def maybe_install(self) -> None:
        apk = self.profile.get("apk")
        if not self.install_apk or not apk:
            return
        apk_path = Path(self.resolve_value(apk))
        if not apk_path.exists():
            raise SmokeError(f"APK not found: {apk_path}")
        log(f"Installing {apk_path}")
        self.adb.run(["install", "-r", str(apk_path)])

    def maybe_reset_app_data(self) -> None:
        if not self.reset_app_data:
            return
        log(f"Clearing app data for {self.package}")
        self.adb.shell(f"pm clear {shlex.quote(self.package)}", check=False)

    def maybe_force_stop(self) -> None:
        if self.release_package:
            self.adb.shell(f"am force-stop {shlex.quote(self.release_package)}")
        self.adb.shell(f"am force-stop {shlex.quote(self.package)}", check=False)

    def launch(self) -> None:
        self.adb.shell(f"am start -S -n {shlex.quote(self.package)}/{shlex.quote(self.activity)}")
        self.wait_for_foreground(self.package, tries=30)

    def start_recording(self) -> None:
        remote = self.profile.get("recording_remote", "/sdcard/mobile-smoke.mp4")
        local = self.artifact_dir / "run.mp4"
        self.recorder = Recorder(self.adb, remote, local)
        self.recorder.start()

    def stop_recording(self) -> None:
        if self.recorder is not None:
            self.recorder.stop()

    def assert_logcat(self) -> None:
        logcat_file = self.save_logcat()
        content = logcat_file.read_text(encoding="utf-8")
        allow = tuple(self.profile.get("allow_fatal_patterns", []))
        if "FATAL EXCEPTION" not in content:
            return
        lines = [line for line in content.splitlines() if "FATAL EXCEPTION" in line or self.package in line]
        if allow and any(pattern in content for pattern in allow):
            filtered = [line for line in lines if not any(pattern in line for pattern in allow)]
            if not filtered:
                return
        raise SmokeError(f"fatal exception detected; see {logcat_file}")

    def assert_ui(self, state: UiState, text: str | None = None, desc: str | None = None) -> None:
        if text is not None and not state.has_text(text):
            raise SmokeError(f"expected text '{text}'")
        if desc is not None and not state.has_desc(desc):
            raise SmokeError(f"expected desc '{desc}'")

    def execute_step(self, step: dict[str, Any], current: UiState | None) -> UiState | None:
        if "launch" in step:
            self.launch()
            return current
        if "wait_for_text" in step:
            spec = step["wait_for_text"]
            text = spec["text"] if isinstance(spec, dict) else spec
            text = self.resolve_value(text)
            tries = spec.get("tries", 20) if isinstance(spec, dict) else 20
            label = spec.get("label", f"wait-{text}") if isinstance(spec, dict) else f"wait-{text}"
            return self.wait_for_ui(text=text, tries=tries, label=label)
        if "wait_for_desc" in step:
            spec = step["wait_for_desc"]
            desc = spec["desc"] if isinstance(spec, dict) else spec
            desc = self.resolve_value(desc)
            tries = spec.get("tries", 20) if isinstance(spec, dict) else 20
            label = spec.get("label", f"wait-{desc}") if isinstance(spec, dict) else f"wait-{desc}"
            return self.wait_for_ui(desc=desc, tries=tries, label=label)
        if "wait_for_any_package" in step:
            spec = step["wait_for_any_package"]
            packages = list(spec["packages"] if isinstance(spec, dict) else spec)
            tries = spec.get("tries", 20) if isinstance(spec, dict) else 20
            label = spec.get("label", "wait-package") if isinstance(spec, dict) else "wait-package"
            return self.wait_for_ui(package_any=packages, tries=tries, label=label)
        if "assert_text" in step:
            if current is None:
                raise SmokeError("assert_text requires current UI state")
            self.assert_ui(current, text=self.resolve_value(step["assert_text"]))
            return current
        if "assert_no_text" in step:
            if current is None:
                raise SmokeError("assert_no_text requires current UI state")
            text = self.resolve_value(step["assert_no_text"])
            if current.has_text(text):
                raise SmokeError(f"unexpected text '{text}'")
            return current
        if "assert_desc" in step:
            if current is None:
                raise SmokeError("assert_desc requires current UI state")
            self.assert_ui(current, desc=self.resolve_value(step["assert_desc"]))
            return current
        if "assert_any_text" in step:
            if current is None:
                raise SmokeError("assert_any_text requires current UI state")
            expected = [self.resolve_value(text) for text in step["assert_any_text"]]
            for text in expected:
                if current.has_text(text):
                    return current
            raise SmokeError(f"expected one of texts {expected}")
        if "assert_package_any" in step:
            if current is None:
                raise SmokeError("assert_package_any requires current UI state")
            packages = step["assert_package_any"]
            if current.package() not in packages:
                raise SmokeError(f"expected package in {packages}, got {current.package()}")
            return current
        if "tap_text" in step:
            if current is None:
                raise SmokeError("tap_text requires current UI state")
            self.tap_selector(current, "text", self.resolve_value(step["tap_text"]))
            return current
        if "tap_desc" in step:
            if current is None:
                raise SmokeError("tap_desc requires current UI state")
            self.tap_selector(current, "desc", self.resolve_value(step["tap_desc"]))
            return current
        if "ensure_checked_desc" in step:
            if current is None:
                raise SmokeError("ensure_checked_desc requires current UI state")
            desc = self.resolve_value(step["ensure_checked_desc"])
            if not current.is_checked("desc", desc):
                self.tap_selector(current, "desc", desc)
                return self.dump_ui(f"checked-{desc}")
            return current
        if "tap_percent" in step:
            spec = step["tap_percent"]
            self.tap_percent(int(spec["x"]), int(spec["y"]))
            return current
        if "swipe_percent" in step:
            spec = step["swipe_percent"]
            self.swipe_percent(
                int(spec["x1"]),
                int(spec["y1"]),
                int(spec["x2"]),
                int(spec["y2"]),
                int(spec.get("duration_ms", 500)),
            )
            return self.dump_ui(str(spec.get("label", "swipe")))
        if "scroll_to_text" in step:
            spec = step["scroll_to_text"]
            text = spec["text"] if isinstance(spec, dict) else spec
            tries = spec.get("tries", 6) if isinstance(spec, dict) else 6
            label = spec.get("label", f"scroll-{text}") if isinstance(spec, dict) else f"scroll-{text}"
            return self.scroll_to_ui(text=self.resolve_value(text), tries=tries, label=label)
        if "scroll_to_desc" in step:
            spec = step["scroll_to_desc"]
            desc = spec["desc"] if isinstance(spec, dict) else spec
            tries = spec.get("tries", 6) if isinstance(spec, dict) else 6
            label = spec.get("label", f"scroll-{desc}") if isinstance(spec, dict) else f"scroll-{desc}"
            return self.scroll_to_ui(desc=self.resolve_value(desc), tries=tries, label=label)
        if "back" in step:
            if current is not None and current.has_desc("Back"):
                self.tap_selector(current, "desc", "Back")
            else:
                self.adb.shell("input keyevent KEYCODE_BACK")
            return current
        if "type_into_text" in step:
            if current is None:
                raise SmokeError("type_into_text requires current UI state")
            spec = step["type_into_text"]
            field = self.resolve_value(spec["field"])
            current = self.focus_labeled_input(current, field)
            self.input_text(self.resolve_value(spec["text"]))
            return self.dump_ui(f"type-{spec['field']}")
        if "press_enter" in step:
            self.adb.shell("input keyevent KEYCODE_ENTER")
            return current
        if "hide_keyboard" in step:
            self.adb.shell("input keyevent KEYCODE_BACK")
            time.sleep(0.5)
            return self.dump_ui("hide-keyboard")
        if "sleep" in step:
            time.sleep(float(step["sleep"]))
            return current
        if "wait_for_foreground" in step:
            self.wait_for_foreground(step["wait_for_foreground"], tries=step.get("tries", 20))
            return current
        if "maybe" in step:
            if current is None:
                current = self.dump_ui("maybe")
            spec = step["maybe"]
            when_text = spec.get("when_text")
            when_desc = spec.get("when_desc")
            active = (when_text is not None and current.has_text(self.resolve_value(when_text))) or (
                when_desc is not None and current.has_desc(self.resolve_value(when_desc))
            )
            if active:
                for inner in spec.get("steps", []):
                    current = self.execute_step(inner, current)
            return current
        if "clear_logcat" in step:
            self.clear_logcat()
            return current
        if "screenshot" in step:
            self.screenshot(str(step["screenshot"]))
            return current
        if "assert_logcat_contains" in step:
            spec = step["assert_logcat_contains"]
            pattern = self.resolve_value(spec["pattern"] if isinstance(spec, dict) else spec)
            label = spec.get("label", "logcat-check") if isinstance(spec, dict) else "logcat-check"
            logcat_file = self.save_logcat()
            content = logcat_file.read_text(encoding="utf-8")
            if pattern not in content:
                raise SmokeError(f"logcat did not contain '{pattern}' (label={label}); see {logcat_file}")
            log(f"logcat contains '{pattern}' ✓")
            return current
        if "wait_logcat_contains" in step:
            spec = step["wait_logcat_contains"]
            pattern = self.resolve_value(spec["pattern"] if isinstance(spec, dict) else spec)
            label = spec.get("label", "logcat-wait") if isinstance(spec, dict) else "logcat-wait"
            timeout = float(spec.get("timeout", 60)) if isinstance(spec, dict) else 60.0
            deadline = time.monotonic() + timeout
            while True:
                logcat_file = self.save_logcat()
                content = logcat_file.read_text(encoding="utf-8")
                if pattern in content:
                    log(f"logcat contains '{pattern}' ✓")
                    return current
                if time.monotonic() >= deadline:
                    raise SmokeError(
                        f"timed out waiting for logcat pattern '{pattern}' "
                        f"(label={label}); see {logcat_file}"
                    )
                time.sleep(2)
        raise SmokeError(f"unsupported step: {step}")

    def run(self) -> Path | None:
        self.artifact_dir.mkdir(parents=True, exist_ok=True)
        self.maybe_force_stop()
        self.maybe_reset_app_data()
        self.maybe_install()
        if self.record:
            self.start_recording()
        try:
            current: UiState | None = None
            for step in self.scenario["steps"]:
                current = self.execute_step(step, current)
            self.assert_logcat()
            self.wait_for_foreground(self.package, tries=20)
            summary = {
                "profile": self.profile["name"],
                "scenario": self.scenario["name"],
                "serial": self.adb.serial,
                "package": self.package,
                "artifacts": {
                    "logcat": str(self.artifact_dir / "logcat.txt"),
                    "video": str(self.artifact_dir / "run.mp4") if self.record else None,
                },
            }
            (self.artifact_dir / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
            log("Smoke test passed")
        finally:
            if self.record:
                self.stop_recording()
        return self.artifact_dir / "run.mp4" if self.record else None


def load_yaml(path: Path) -> dict[str, Any]:
    return yaml.safe_load(path.read_text(encoding="utf-8"))


def discover_serial(adb_bin: str, requested: str | None) -> str:
    if requested:
        return requested
    result = subprocess.run([adb_bin, "devices"], capture_output=True, text=True, check=True)
    devices = []
    for line in result.stdout.splitlines()[1:]:
        parts = line.split()
        if len(parts) >= 2 and parts[1] == "device":
            devices.append(parts[0])
    if not devices:
        raise SmokeError("no connected adb devices")
    if len(devices) > 1:
        raise SmokeError("multiple adb devices connected; set ANDROID_SERIAL")
    return devices[0]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", required=True, help="Profile YAML path")
    parser.add_argument("--scenario", required=True, help="Scenario YAML path")
    parser.add_argument("--adb", default=os.environ.get("ADB", "adb"))
    parser.add_argument("--serial", default=os.environ.get("ANDROID_SERIAL"))
    parser.add_argument("--record", action="store_true")
    parser.add_argument("--install", action="store_true")
    parser.add_argument("--reset-app-data", action="store_true")
    parser.add_argument("--artifacts-dir")
    args = parser.parse_args()

    profile_path = Path(args.profile)
    scenario_path = Path(args.scenario)
    profile = load_yaml(profile_path)
    scenario = load_yaml(scenario_path)
    serial = discover_serial(args.adb, args.serial)

    artifacts_dir = (
        Path(args.artifacts_dir)
        if args.artifacts_dir
        else Path("artifacts/mobile-runs") / profile["name"] / scenario["name"] / time.strftime("%Y%m%d-%H%M%S")
    )
    env = dict(os.environ)

    adb = Adb(args.adb, serial)
    runner = Runner(
        adb,
        profile,
        scenario,
        artifacts_dir,
        record=args.record,
        install_apk=args.install,
        reset_app_data=args.reset_app_data,
        env=env,
    )
    try:
        video = runner.run()
        if video:
            log(f"Video saved to {video}")
        return 0
    except SmokeError as exc:
        log(f"ERROR: {exc}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
