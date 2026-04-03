set shell := ["bash", "-c"]

CORE_CRATE := "mango_core"
LIB_NAME := "mango_core"
XCF_NAME := "MangoCore"
DYLIB_EXT := if os() == "macos" { "dylib" } else { "so" }

default:
  @just --list

# ── Core build ────────────────────────────────────────────────────────────────

# Build Rust core for the host (debug)
build:
  cargo build

# Build Rust core for the host (release)
build-release:
  cargo build --release

# Check Rust core without producing binaries
check:
  cargo check

# Run all tests
test:
  cargo test

# Clean build artifacts
clean:
  cargo clean

# ── UniFFI bindings ───────────────────────────────────────────────────────────

# Build host release binary (required for bindings generation)
_host-build:
  cargo build -p {{CORE_CRATE}} --release

# Generate Swift bindings from the compiled library
bindings-swift: _host-build
  cargo run --bin uniffi-bindgen -- generate \
    --library target/release/lib{{LIB_NAME}}.{{DYLIB_EXT}} \
    --language swift \
    --out-dir ios/Bindings \
    --config rust/uniffi.toml

# Generate Kotlin bindings from the compiled library
bindings-kotlin: _host-build
  cargo run --bin uniffi-bindgen -- generate \
    --library target/release/lib{{LIB_NAME}}.{{DYLIB_EXT}} \
    --language kotlin \
    --out-dir android/app/src/main/java \
    --no-format \
    --config rust/uniffi.toml

# ── iOS ───────────────────────────────────────────────────────────────────────

# Cross-compile Rust for iOS device and simulator (arm64)
build-ios:
  #!/usr/bin/env bash
  set -e
  DEV_DIR="$(xcode-select -p 2>/dev/null)"
  TOOLCHAIN_BIN="$DEV_DIR/Toolchains/XcodeDefault.xctoolchain/usr/bin"
  IOS_SDK="$DEV_DIR/Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS.sdk"
  SIM_SDK="$DEV_DIR/Platforms/iPhoneSimulator.platform/Developer/SDKs/iPhoneSimulator.sdk"
  for pair in "aarch64-apple-ios $IOS_SDK -miphoneos-version-min=17.0" \
              "aarch64-apple-ios-sim $SIM_SDK -mios-simulator-version-min=17.0"; do
    set -- $pair; TARGET=$1; SDK=$2; VFLAG=$3
    RUSTFLAGS="-C linker=$TOOLCHAIN_BIN/clang -C link-arg=$VFLAG -C link-arg=-isysroot -C link-arg=$SDK" \
      cargo build -p {{CORE_CRATE}} --lib --target "$TARGET" --release
  done

# Package static libs into an xcframework
ios-xcframework:
  #!/usr/bin/env bash
  set -e
  DEV_DIR="$(xcode-select -p 2>/dev/null)"
  rm -rf ios/Frameworks/{{XCF_NAME}}.xcframework staging
  mkdir -p staging/headers
  cp ios/Bindings/{{LIB_NAME}}FFI.h staging/headers/
  cp ios/Bindings/{{LIB_NAME}}FFI.modulemap staging/headers/module.modulemap
  xcodebuild -create-xcframework \
    -library target/aarch64-apple-ios/release/lib{{LIB_NAME}}.a -headers staging/headers \
    -library target/aarch64-apple-ios-sim/release/lib{{LIB_NAME}}.a -headers staging/headers \
    -output ios/Frameworks/{{XCF_NAME}}.xcframework
  rm -rf staging

# Full iOS pipeline: bindings -> cross-compile -> xcframework
ios-full: bindings-swift build-ios ios-xcframework

# ── Android ───────────────────────────────────────────────────────────────────

# Cross-compile Rust for Android ABIs via cargo-ndk
# Note: armv7/x86_64 dropped — ort (ONNX Runtime) has no prebuilt binaries for those targets
build-android:
  #!/usr/bin/env bash
  set -e
  cargo ndk -o android/app/src/main/jniLibs -P 28 \
    -t arm64-v8a \
    build -p {{CORE_CRATE}} --release
  # Bundle libc++_shared.so required by ONNX Runtime
  NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_HOME:-${HOME}/Android/Sdk}/ndk/28.2.13676358}"
  cp "$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" \
    android/app/src/main/jniLibs/arm64-v8a/

# Full Android pipeline: bindings -> cross-compile -> assemble
android-full: bindings-kotlin build-android
  cd android && ./gradlew :app:assembleDebug

# ── Desktop ───────────────────────────────────────────────────────────────────

# Run the iced desktop app (Phase 1 placeholder -- desktop shell added in Phase 4+)
run-desktop:
  cargo run -p mango-desktop
