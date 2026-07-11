# Mobile Smoke Runner

Reusable adb-driven smoke test runner with:

- YAML app profiles
- YAML scenarios
- optional APK install
- optional emulator/device screen recording
- UI XML dumps and logcat artifacts

## Runner

```bash
python3 tools/mobile-smoke/runner.py \
  --profile tools/mobile-smoke/profiles/mango.yaml \
  --scenario tools/mobile-smoke/scenarios/mango_smoke.yaml
```

Optional flags:

```bash
--install
--record
--serial emulator-5554
--artifacts-dir artifacts/mobile-runs/custom
```

## Profile

Profile files declare app-specific values:

```yaml
name: my-app
package: com.example.app.debug
release_package: com.example.app
activity: com.example.app.MainActivity
apk: app/build/outputs/apk/debug/app-debug.apk
recording_remote: /sdcard/my-app-smoke.mp4
```

## Scenario

Scenario files declare reusable steps:

```yaml
name: smoke
steps:
  - launch: {}
  - clear_logcat: {}
  - wait_for_text: Home
  - tap_text: Settings
  - wait_for_text: About
```

Supported step types:

- `launch`
- `clear_logcat`
- `wait_for_text`
- `wait_for_desc`
- `wait_for_any_package`
- `assert_text`
- `assert_no_text`
- `assert_desc`
- `assert_any_text`
- `assert_package_any`
- `tap_text`
- `tap_desc`
- `tap_percent`
- `type_into_text`
- `press_enter`
- `back`
- `sleep`
- `wait_for_foreground`
- `maybe`
- `screenshot`
- `wait_logcat_contains`
- `assert_logcat_contains`

## Mango Compatibility Wrapper

The existing entrypoint still works:

```bash
./scripts/android_smoke_test.sh
```

It now delegates to the generic runner with the Mango profile and scenario.
