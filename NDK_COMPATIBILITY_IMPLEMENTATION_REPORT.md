# NDK / NumKong x86_64 compatibility — implementation report

Date: 2026-09-06
Executor: Devin CLI SWE-1.7 (YOLO `--permission-mode dangerous`)
Reviewer: Codex
Scope: Make the Android native pipeline build `mango_core` for both `arm64-v8a` and `x86_64` with NDK 28.2.13676358, preserving vector-search (usearch) behavior and the existing arm64 app build.

## 1. Root cause

`Cargo.lock` resolves `usearch 2.26.1`, which depends on `numkong 7.8.1` for its SIMD kernels. NumKong's `include/numkong/capabilities.h` declares a forward `syscall()` for Linux x86_64/RISC-V targets:

```cpp
#if defined(NK_DEFINED_LINUX_) && (NK_TARGET_X8664_ || NK_TARGET_RISCV64_)
#include <sys/syscall.h>
#ifdef __cplusplus
extern "C" long syscall(long, ...) noexcept;
#else
extern long syscall(long, ...);
#endif
```

Line 138 (`extern "C" long syscall(long, ...) noexcept;`) is compatible with glibc because glibc's `<unistd.h>` declares `syscall` with `__THROW` (which is `noexcept` in C++). Android's Bionic libc, however, declares:

```c
long syscall(long __number, ...);
```

without an exception specification. `<unistd.h>` is already included by NumKong at line 121 under `NK_DEFINED_LINUX_`, so `capabilities.h` then redeclares `syscall` with a mismatched exception spec, producing:

```text
error: exception specification in declaration does not match previous declaration
```

The failing translation unit is `usearch/rust/lib.cpp` (compiled as C++17 by the `usearch` build script through `cxx_build`), which includes `numkong/numkong.h` → `numkong/capabilities.h` with `NK_DYNAMIC_DISPATCH=1`.

## 2. Bounded reproduction

A minimal compile-only reproducer reproduces the failure with the real NDK 28.2.13676358 x86_64 sysroot:

```bash
NDK=/home/lio/Android/Sdk/ndk/28.2.13676358
NUMKONG=~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/numkong-7.8.1
cat > /tmp/repro.cpp <<'EOF'
#include <unistd.h>
#include "numkong/capabilities.h"
int main() { return (int)nk_capabilities(); }
EOF
$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android28-clang++ \
  -std=c++17 -I"$NUMKONG/include" -c /tmp/repro.cpp
```

Result:

```text
.../numkong/capabilities.h:138:17: error: exception specification in declaration does not match previous declaration
extern "C" long syscall(long, ...) noexcept;
                ^
.../sysroot/usr/include/unistd.h:404:6: note: previous declaration is here
long syscall(long __number, ...);
     ^
```

The same reproducer compiled with `aarch64-linux-android28-clang++` and the host `g++`/`clang++` passes, confirming the defect is isolated to Android Bionic + x86_64.

The actual `mango_core` x86_64 build failed identically:

```bash
cargo ndk -o /tmp/jniLibs-repro -P 28 -t x86_64 build -p mango_core --release
# exit 101
# usearch build.rs: numkong/capabilities.h:138 — exception specification mismatch
```

## 3. Repair selected

### 3.1 No suitable released fix

- `numkong 7.8.2` (latest on crates.io at the time of investigation) still contains the same unguarded `noexcept` forward-declaration.
- Upstream `ashvardanian/NumKong:main` (checked 2026-09-06) still contains the same code.
- Bumping `usearch` to 2.26.2 would not help because it still depends on `numkong` in the `>=7.5.0, <8` range and the latest `numkong` release does not address Android/Bionic.

### 3.2 Repository-owned vendored patch

A local copy of `numkong 7.8.1` was placed in `vendor/numkong/`, sourced from the original crates.io package (sha256 `32abf4fad33e67bc129973bf953f076803bd33f393ac17589cb05afb5b3b8420`), published from upstream git commit `928a2143ac3370547f833eeb38dbd736f7d54d5e` (see `.cargo_vcs_info.json` and `Cargo.toml`). The crate is Apache-2.0 licensed. A single minimal platform guard was added to `vendor/numkong/include/numkong/capabilities.h`:

```cpp
#if defined(NK_DEFINED_LINUX_) && (NK_TARGET_X8664_ || NK_TARGET_RISCV64_)
#include <sys/syscall.h> // `SYS_arch_prctl`, `SYS_riscv_hwprobe`
#if defined(__ANDROID__)
// Android's Bionic libc declares `syscall()` in <unistd.h> — already included
// above under `NK_DEFINED_LINUX_` — without an exception specification, so
// redeclaring it `noexcept` here would be a hard error. Reuse the platform
// declaration; the include below only guards against future reordering.
#include <unistd.h> // `syscall`
#elif defined(__cplusplus)
extern "C" long syscall(long, ...) noexcept;
#else
extern long syscall(long, ...);
#endif
```

This preserves the glibc/noexcept path on Linux and leaves non-Linux platforms unchanged, while reusing Bionic's existing declaration on Android. No SIMD dispatch is disabled, no `noexcept` macro is redefined globally, and no optimization or NDK downgrade is required.

The vendored crate is activated through the workspace root:

```toml
[patch.crates-io]
numkong = { path = "vendor/numkong" }
```

`usearch` requires `numkong >=7.5.0, <8`; the vendored `7.8.1` satisfies this, so no other dependency versions change.

### 3.3 Removal conditions

The patch can be removed once a released `numkong` version (≥ current) guards its `syscall` forward-declaration for `__ANDROID__` / Bionic, or removes the manual forward-declaration and relies on `<unistd.h>` (as the patch now does on Android). At that point, delete the `[patch.crates-io]` block and the `vendor/numkong/` directory.

## 4. Dependency and lockfile impact

- `Cargo.toml`: adds the `[patch.crates-io]` block (workspace root only; no `rust/Cargo.toml` edits).
- `Cargo.lock`: only the `numkong` entry changes — `source = "registry+..."` and `checksum` are removed and the version remains `7.8.1`.
- `usearch`, `mango_core`, and all other crates retain their resolved versions (no lockfile churn elsewhere).

Diff between vendored `numkong 7.8.1` and the registry copy is a single hunk in `capabilities.h` plus the added `LICENSE` and `VENDOR.md`:

```text
diff -r .../numkong-7.8.1/include/numkong/capabilities.h vendor/numkong/include/numkong/capabilities.h
137c137,143
< #ifdef __cplusplus
---
> #if defined(__ANDROID__)
> #include <unistd.h> // `syscall`
> #elif defined(__cplusplus)
...
```

## 5. Regression check

`scripts/check_numkong_android_headers.sh` is a compile-only regression check that uses the real NDK 28.2.13676358 x86_64 and arm64 toolchains plus the host C++ compiler. It compiles a TU including `<unistd.h>` before `numkong/capabilities.h` (the same order as `usearch/rust/lib.cpp`):

```bash
just check-numkong-headers
# or
scripts/check_numkong_android_headers.sh
```

All three compiler checks are **mandatory**: a missing NDK, missing headers directory, or missing host C++ compiler fails the script with a nonzero exit — it can never print `RESULT: PASS` after silently skipping a required target. An explicit opt-out flag `--allow-missing` turns missing tools into `SKIP` lines and ends with `RESULT: INCOMPLETE … this is NOT full verification`.

Verified scenarios (exit codes observed 2026-09-06):

| Scenario | Command variation | Result | Exit |
|---|---|---|---|
| Normal success (patched headers) | `./scripts/check_numkong_android_headers.sh` | `RESULT: PASS` — all 3 targets | `0` |
| Original-header failure | `NUMKONG_INCLUDE_DIR=<registry>/numkong-7.8.1/include …` | `FAIL android-x86_64` (exception-spec mismatch), arm64 + host PASS | `1` |
| Missing NDK | `NDK_HOME=/nonexistent-ndk …` | `FAIL android-* — required NDK not found` | `1` |
| Missing headers dir | `NUMKONG_INCLUDE_DIR=/nonexistent …` | `FAIL headers — not found` | `1` |
| Explicit partial mode | `NDK_HOME=/nonexistent-ndk … --allow-missing` | `SKIP android-*` + `PASS host-linux` → `RESULT: INCOMPLETE` | `0` |

Header-compatibility results:

| Target | Original 7.8.1 (registry) | Vendored 7.8.1 (patched) |
|---|---|---|
| Android x86_64 | **FAIL** (exception spec mismatch) | PASS |
| Android arm64-v8a | PASS | PASS |
| Host Linux glibc | PASS | PASS |

## 6. Full integration verification

### 6.1 `cargo ndk` per-ABI builds

```bash
# x86_64 (temp output dir, does not erase existing jniLibs)
cargo ndk -o /tmp/jniLibs-repro -P 28 -t x86_64 build -p mango_core --release
# exit 0
file /tmp/jniLibs-repro/x86_64/libmango_core.so
# ELF 64-bit LSB shared object, x86-64, version 1 (SYSV), for Android 28, NDK r28c

# arm64-v8a
cargo ndk -o /tmp/jniLibs-repro -P 28 -t arm64-v8a build -p mango_core --release
# exit 0
```

### 6.2 `just build-android` pipeline

```bash
just build-android
```

- exit `0`
- `numkong` probe warnings (`NK_TARGET_DIAMOND`, ARM SME probes not supported by the NDK clang) are expected behavior — the build script uses `try_compile` and falls back to lower ISA levels.
- `jniLibs/` contains both ABIs:
  - `arm64-v8a/`: `libmango_core.so`, `libc++_shared.so`, 5 llama.cpp companion libs (`libggml-base.so`, `libggml-cpu.so`, `libggml.so`, `libllama-common.so`, `libllama.so`), plus `libdcap_qvl-*.so` and `librtf_parser-*.so` cdylib copies emitted by `cargo-ndk`.
  - `x86_64/`: `libmango_core.so`, `libc++_shared.so`, plus the same `libdcap_qvl-*.so` and `librtf_parser-*.so` cdylib copies.

### 6.3 Host test suite — observed results (not a conclusive diagnosis)

```bash
# Serial run
cargo test -p mango_core -- --test-threads=1
# exit 0 — 632 passed; 0 failed; 21 ignored; finished in 153.90s
```

Observed across runs:

- `cargo test -p mango_core` (default parallel harness) with the patched
  resolution failed **three times** on the WIP test
  `tests::enrollment_recovery::test_setup_pin_migrates_plaintext_preserving_conversations`:
  a `db unlocked` panic in the actor thread at `rust/src/lib.rs:12713`
  (`AttestationCache::new(actor_state.db.as_ref().expect("db unlocked").conn())`,
  attestation-result handler) followed by a 60 s actor-channel timeout
  (`rust/src/lib.rs:13643`).
- The same test **passed** when `tests::enrollment_recovery` was run alone
  (`cargo test -p mango_core --lib tests::enrollment_recovery -- --test-threads=1`,
  22/22, 51 s) on the patched build.
- The full suite **passed** on an exact-version control run with the
  unpatched registry `numkong 7.8.1` (`git checkout -- Cargo.toml Cargo.lock`,
  `cargo test -p mango_core` → exit 0, 632 passed, 21.58 s), and also once
  earlier with registry `numkong 7.8.2`.
- The full suite **passed** on the patched resolution under
  `--test-threads=1` (632 passed, 153.90 s).

Assessment: the only source difference the patch introduces is inside
`#if defined(__ANDROID__)`, which is dead code on host Linux, so a direct
Linux semantic regression from the vendored header is unlikely. However, the
parallel-harness failure was observed repeatedly under the patched
resolution and only once — passing — under the exact-version unpatched
control, so these observations **do not prove** a pre-existing failure of
this exact test and do **not** conclusively diagnose the timing/race. The
`expect("db unlocked")` panic class is documented in
`PRE_RELEASE_FINDINGS.md` as a known WIP race (C3/M-1), and the failing test
file is untracked WIP from the prior app-remediation work. Root cause of the
residual parallel-harness failure is **not conclusively diagnosed** in this
remediation and is out of scope for the NDK fix; it is reported here as an
open observation for the app/actor workstream.

No unrelated app/actor code was edited. Host tests must not be reported as
unconditionally green: under the default parallel harness the suite failed
on this test in all three patched-resolution runs, while passing in the
single exact-version unpatched control and in serial/isolated runs.

### 6.4 Desktop check

```bash
cargo check -p mango-desktop
```

- exit `0`
- Vendored `numkong` compiles cleanly on the host x86_64 Linux toolchain.

### 6.5 Gradle APK assembly

```bash
cd android && ./gradlew :app:assembleDebug
```

- exit `0`
- `BUILD SUCCESSFUL in 8s`

The project's `android/app/build.gradle.kts` currently sets `ndk.abiFilters += listOf("arm64-v8a")` for both `debug` and `release`. This existing configuration was not changed, so the packaged APK contains only `arm64-v8a` native libraries (as before). The `x86_64` `libmango_core.so` is built and staged in `jniLibs/x86_64/` by `just build-android`; including it in the APK requires changing that Gradle `abiFilters` setting.

```text
lib/arm64-v8a/libmango_core.so
lib/arm64-v8a/libc++_shared.so
lib/arm64-v8a/libllama.so
lib/arm64-v8a/libggml*.so
lib/arm64-v8a/libdcap_qvl-*.so
lib/arm64-v8a/librtf_parser-*.so
```

All packaged `arm64-v8a` libraries are `ELF 64-bit LSB shared object, ARM aarch64`.

### 6.6 `libmango_core.so` runtime linkage

`llvm-readelf -d libmango_core.so` for both ABIs shows only Android system libraries, `libc++_shared.so`, and the internally-linked rlib symbols; the `libdcap_qvl` / `librtf_parser` cdylib artifacts emitted by `cargo-ndk` are not `NEEDED` by `libmango_core.so`.

## 7. Alternatives considered

| Alternative | Why not selected |
|---|---|
| Bump `numkong` to 7.8.2 / `usearch` to 2.26.2 | 7.8.2 still contains the same `noexcept` declaration; 2.26.2 still accepts any `numkong` in `>=7.5.0, <8`. No upstream fix is released or on `main` at the time of writing. |
| Use a `[patch]` git dependency to a personal fork | Prohibited by scope (no new upstream issues/PRs/forks) and less self-contained than a vendored directory. |
| Patch the global `~/.cargo/registry/src/.../numkong-7.8.1/` in place | Explicitly prohibited by the task instructions ("Do not edit the global Cargo registry/cache as the permanent fix"). |
| Compile with `-fno-exceptions` or globally suppress the error | Would silently change C++ semantics for the whole `usearch`/`numkong` TU and could mask real errors. |
| Define `NK_TARGET_X8664_=0` or otherwise disable x86 dispatch | Would turn off the x86 SIMD kernels, degrading vector-search performance. |
| Downgrade the NDK | No NDK version changes the Bionic `syscall` declaration; the conflict is in the source, not the toolchain. |

## 8. Known unresolved items

- The WIP `test_setup_pin_migrates_plaintext_preserving_conversations` test failed repeatedly under the default parallel test harness with a `db unlocked` actor panic; it passed under `--test-threads=1`, in module isolation, and in a single exact-version unpatched control run. A semantic regression from this patch is unlikely (the vendored diff is dead code on host Linux), but a pre-existing failure of this exact test is **not proven** and the timing/race is **not conclusively diagnosed** — see §6.3 for the full observation record.
- The APK still ships only `arm64-v8a` because `android/app/build.gradle.kts` `abiFilters` is unchanged. The `x86_64` native library now builds successfully and is available in `jniLibs/x86_64/`; packaging it is a Gradle configuration change outside the NDK-remediation scope.

## 9. Prior verification documents superseded

The x86_64 Android NDK blocker documented in `APP_REVIEW_IMPLEMENTATION_REPORT.md` and `APP_REVIEW_VERIFICATION.md` is now resolved at the native-build level. The relevant earlier paragraphs have been updated with a superseding note, while iOS build/runtime limitations are retained unchanged.

## 10. Reproducibility commands

```bash
# 1. Reproduce the original x86_64 header failure (against registry copy)
NUMKONG_REG=~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/numkong-7.8.1
NUMKONG_INCLUDE_DIR="$NUMKONG_REG/include" ./scripts/check_numkong_android_headers.sh

# 2. Verify the patched vendored headers
./scripts/check_numkong_android_headers.sh

# 3. Build both Android ABIs
just build-android

# 4. Host tests and checks
cargo test -p mango_core -- --test-threads=1
cargo check -p mango-desktop

# 5. APK assembly
cd android && ./gradlew :app:assembleDebug
```
