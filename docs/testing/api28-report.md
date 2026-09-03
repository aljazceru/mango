# Mango API 28 x86_64 Emulator Verification Report

**Date:** 2026-09-03  
**Host:** Linux x86_64  
**Repo:** `/run/media/lio/data/g/confidential-app`  
**Emulator:** AVD `mango28` (Pixel 5, API 28, `default/x86_64` system image)  
**Android SDK:** `/home/lio/Android/Sdk` (NDK `28.2.13676358`)  
**Report file:** `/tmp/api28-report.md`  

---

## 1. Emulator Setup

### 1.1 System Image
- Initial `system-images/` only contained `android-36`.
- Installed: `system-images;android-28;default;x86_64` via `sdkmanager`.
- Install completed successfully.

### 1.2 AVD
- Created/Overwrote AVD `mango28` with `avdmanager`:
  ```
  avdmanager create avd -n mango28 -k "system-images;android-28;default;x86_64" -d pixel_5 --force
  ```
- Auto-selected single ABI `x86_64`.

### 1.3 Boot
- Launched headless:
  ```
  $ANDROID_HOME/emulator/emulator -avd mango28 -no-window -no-audio -gpu swiftshader_indirect -no-boot-anim
  ```
- Waited for `sys.boot_completed`.
- `adb -s emulator-5554 shell getprop ro.build.version.sdk` returned:
  ```
  28
  ```
- **Emulator booted to API 28 successfully.**

---

## 2. x86_64 JNI Library Build

### 2.1 Full Android Rust build (`just build-android`)
- Command run from repo root:
  ```
  export ANDROID_HOME=/home/lio/Android/Sdk
  export ANDROID_NDK_HOME=/home/lio/Android/Sdk/ndk/28.2.13676358
  just build-android
  ```
- `arm64-v8a` target compiled successfully:
  ```
  Building arm64-v8a (aarch64-linux-android)
  Compiling mango_core v0.3.0 (/run/media/lio/data/g/confidential-app/rust)
  Finished `release` profile [optimized] target(s) in 27.03s
  ```
- `x86_64` target failed during `usearch` C++ build with the known `numkong` syscall declaration conflict:
  ```
  error: exception specification in declaration does not match previous declaration
    extern "C" long syscall(long, ...) noexcept;
                    ^
  /.../sysroot/usr/include/unistd.h:404:6: note: previous declaration is here
    long syscall(long __number, ...);
  ```
  Full crate: `usearch v2.26.1` → `numkong-7.8.1/include/numkong/capabilities.h:138`.

### 2.2 Fallback single-target x86_64 build
- Per instructions, retried with only x86_64:
  ```
  cargo ndk -o android/app/src/main/jniLibs -P 28 -t x86_64 build -p mango_core --release
  ```
- Same `usearch`/`numkong` error reproduced.
- Result: **No `libmango_core.so` is produced for `x86_64`.**

### 2.3 Llama x86_64 libraries
- `llama.cpp` build directory on disk is only `build-android-arm64`.
- No `build-android-x86_64` directory exists.
- `build-llama-android` in `justfile` is hardcoded for `ANDROID_ABI=arm64-v8a`.
- Result: **No x86_64 llama shared libraries exist.**

### 2.4 Additional packaging blocker
- `android/app/build.gradle.kts` (debug build) restricts native ABIs:
  ```kotlin
  ndk {
      abiFilters += listOf("arm64-v8a")
  }
  ```
- Even if the x86_64 Rust and llama libraries built, the debug APK would not package `x86_64` native libraries without changing the filter.

---

## 3. APK Install on x86_64 API 28 Emulator

- The debug APK currently in `android/app/build/outputs/apk/debug/app-debug.apk` was examined and contains only `lib/arm64-v8a/` native libraries.
- Attempted install on `emulator-5554`:
  ```
  adb -s emulator-5554 install -r android/app/build/outputs/apk/debug/app-debug.apk
  ```
- Result:
  ```
  Performing Streamed Install
  adb: failed to install .../app-debug.apk: Failure [INSTALL_FAILED_NO_MATCHING_ABIS:
  Failed to extract native libraries, res=-113]
  ```
- **App did not install on the x86_64 emulator.**

---

## 4. Smoke Test

- Because the APK cannot be installed, the mobile smoke runner was not executed.
- Required environment variables would have been:
  ```
  MANGO_SMOKE_PIN=1234
  MANGO_SMOKE_MESSAGE=hello
  ```
- Runner supports `--serial`, which would have been set to `emulator-5554`.

---

## 5. Logcat Excerpts

The full logcat capture is saved at `/tmp/api28-logcat.txt`.

Relevant filtered excerpts (`AndroidRuntime|FATAL|mango|INSTALL`):

```
09-03 22:04:26.352  1729  1729 D AndroidRuntime: >>>>>> START com.android.internal.os.ZygoteInit uid 0 <<<<<<
09-03 22:04:26.378  1730  1730 D AndroidRuntime: >>>>>> START com.android.internal.os.ZygoteInit uid 0 <<<<<<
09-03 22:04:27.820  2062  2062 I AndroidRuntime: VM exiting with result code 0, cleanup skipped.
09-03 22:04:27.821  2069  2069 I AndroidRuntime: VM exiting with result code 0, cleanup skipped.
```

The `PackageManager` logcat does not contain a stack trace for `INSTALL_FAILED_NO_MATCHING_ABIS`; the install failure is returned directly by `pm` to `adb`.

---

## 6. Summary of Blockers

| # | Blocker | Effect | Location |
|---|---|---|---|
| 1 | `usearch`/`numkong` C++ `syscall` declaration conflict on `x86_64-linux-android` | `mango_core` cannot compile for `x86_64` | `numkong-7.8.1/include/numkong/capabilities.h:138` |
| 2 | No `llama.cpp` x86_64 build | Missing `libggml*.so` / `libllama*.so` for `x86_64` | `llama.cpp/build-android-arm64` only |
| 3 | Debug APK `abiFilters` is `arm64-v8a` only | x86_64 `.so` files would be excluded even if they built | `android/app/build.gradle.kts:61-63` |
| 4 | Existing debug APK is `arm64-v8a` only | `INSTALL_FAILED_NO_MATCHING_ABIS` on x86_64 emulator | `android/app/build/outputs/apk/debug/app-debug.apk` |

---

## 7. Conclusion

- **Emulator verification passed:** `mango28` AVD boots, `ro.build.version.sdk == 28`.
- **App testing blocked:** The debug APK cannot run on an x86_64 API 28 emulator because no x86_64 native libraries are built or packaged.
- **Root cause:** The `usearch` dependency (via `numkong`) fails to compile for `x86_64-linux-android` with NDK 28.2.13676358.
- **Secondary cause:** The project does not build `llama.cpp` for `x86_64` and the debug build type does not include `x86_64` in its ABI filter.
- No source files were modified during this verification.

## Release disposition (2026-09-03)

API-28 emulator verification is **blocked by a toolchain issue**, not app
behavior: the x86_64-linux-android build of `usearch` (numkong headers)
fails under NDK 28 clang, so no x86_64 APK exists to install on the API-28
x86_64 emulator (INSTALL_FAILED_NO_MATCHING_ABIS). Options for the release
team: (a) accept API 37 (Pixel 9a, physical, fully verified) as the only
tested platform for v1 and state min-SDK behavior is best-effort; (b) fix
the usearch/numkong x86_64 build (upstream header issue) and run the
emulator matrix before public rollout; (c) obtain an API-28 arm64 physical
device. Managed-PPQ UI requires API 24+ features only (SAF, BiometricPrompt
via androidx); no API-28-specific code paths are used.
