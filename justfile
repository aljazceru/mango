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

# Cross-compile Rust for iOS device and simulator.
build-ios:
  #!/usr/bin/env bash
  set -e
  DEV_DIR="$(xcode-select -p 2>/dev/null)"
  TOOLCHAIN_BIN="$DEV_DIR/Toolchains/XcodeDefault.xctoolchain/usr/bin"
  IOS_SDK="$DEV_DIR/Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS.sdk"
  SIM_SDK="$DEV_DIR/Platforms/iPhoneSimulator.platform/Developer/SDKs/iPhoneSimulator.sdk"
  DEPLOYMENT_TARGET="18.0"
  for pair in "aarch64-apple-ios $IOS_SDK -miphoneos-version-min=$DEPLOYMENT_TARGET" \
              "aarch64-apple-ios-sim $SIM_SDK -mios-simulator-version-min=$DEPLOYMENT_TARGET" \
              "x86_64-apple-ios $SIM_SDK -mios-simulator-version-min=$DEPLOYMENT_TARGET"; do
    set -- $pair; TARGET=$1; SDK=$2; VFLAG=$3
    case "$TARGET" in
      aarch64-apple-ios)
        CLANG_TARGET="arm64-apple-ios$DEPLOYMENT_TARGET"
        ;;
      aarch64-apple-ios-sim)
        CLANG_TARGET="arm64-apple-ios$DEPLOYMENT_TARGET-simulator"
        ;;
      x86_64-apple-ios)
        CLANG_TARGET="x86_64-apple-ios$DEPLOYMENT_TARGET-simulator"
        ;;
      *)
        echo "unsupported iOS target: $TARGET" >&2
        exit 1
        ;;
    esac
    IPHONEOS_DEPLOYMENT_TARGET="$DEPLOYMENT_TARGET" \
      IPHONESIMULATOR_DEPLOYMENT_TARGET="$DEPLOYMENT_TARGET" \
      RUSTFLAGS="-C linker=$TOOLCHAIN_BIN/clang -C link-arg=-target -C link-arg=$CLANG_TARGET -C link-arg=$VFLAG -C link-arg=-isysroot -C link-arg=$SDK" \
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
  lipo -create \
    target/aarch64-apple-ios-sim/release/lib{{LIB_NAME}}.a \
    target/x86_64-apple-ios/release/lib{{LIB_NAME}}.a \
    -output staging/lib{{LIB_NAME}}-ios-simulator.a
  xcodebuild -create-xcframework \
    -library target/aarch64-apple-ios/release/lib{{LIB_NAME}}.a -headers staging/headers \
    -library staging/lib{{LIB_NAME}}-ios-simulator.a -headers staging/headers \
    -output ios/Frameworks/{{XCF_NAME}}.xcframework
  rm -rf staging

# Full iOS pipeline: bindings -> cross-compile -> xcframework
ios-full: bindings-swift build-ios ios-xcframework

# ── Android ───────────────────────────────────────────────────────────────────

# Fetch or update the external llama.cpp checkout to the pinned release used by iOS LlamaSwift.
fetch-llama-cpp:
  #!/usr/bin/env bash
  set -e
  source android/llama.cpp.version
  LLAMA_CPP_DIR="${LLAMA_CPP_DIR:-$(cd .. && pwd)/llama.cpp}"
  if [[ -d "$LLAMA_CPP_DIR/.git" ]]; then
    git -C "$LLAMA_CPP_DIR" fetch --tags "$LLAMA_CPP_REPO" "$LLAMA_CPP_TAG"
    git -C "$LLAMA_CPP_DIR" checkout "$LLAMA_CPP_COMMIT"
  elif [[ -e "$LLAMA_CPP_DIR" ]]; then
    echo "LLAMA_CPP_DIR exists but is not a git checkout: $LLAMA_CPP_DIR" >&2
    exit 1
  else
    git clone --branch "$LLAMA_CPP_TAG" "$LLAMA_CPP_REPO" "$LLAMA_CPP_DIR"
    git -C "$LLAMA_CPP_DIR" checkout "$LLAMA_CPP_COMMIT"
  fi
  scripts/check_llama_versions.sh --skip-libs

# Verify Android and iOS llama.cpp pins match.
check-llama-versions:
  scripts/check_llama_versions.sh

# Build pinned llama.cpp Android shared libraries consumed by the app CMake wrapper.
build-llama-android: fetch-llama-cpp
  #!/usr/bin/env bash
  set -e
  LLAMA_CPP_DIR="${LLAMA_CPP_DIR:-$(cd .. && pwd)/llama.cpp}"
  NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_HOME:-${HOME}/Android/Sdk}/ndk/28.2.13676358}"
  cmake -S "$LLAMA_CPP_DIR" -B "$LLAMA_CPP_DIR/build-android-arm64" \
    -DCMAKE_TOOLCHAIN_FILE="$NDK_HOME/build/cmake/android.toolchain.cmake" \
    -DANDROID_ABI=arm64-v8a \
    -DANDROID_PLATFORM=android-28 \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=ON \
    -DLLAMA_BUILD_EXAMPLES=OFF \
    -DLLAMA_BUILD_SERVER=OFF \
    -DLLAMA_BUILD_TESTS=OFF
  cmake --build "$LLAMA_CPP_DIR/build-android-arm64" --config Release --parallel \
    --target ggml-base ggml-cpu ggml llama llama-common
  scripts/check_llama_versions.sh

# Cross-compile Rust for Android ABIs via cargo-ndk (debug: arm64 + x86_64)
build-android:
  #!/usr/bin/env bash
  set -e
  # Clean stale artifacts before rebuild
  rm -rf android/app/src/main/jniLibs
  cargo ndk -o android/app/src/main/jniLibs -P 28 \
    -t arm64-v8a -t x86_64 \
    build -p {{CORE_CRATE}} --release
  # Bundle libc++_shared.so required by ONNX Runtime
  NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_HOME:-${HOME}/Android/Sdk}/ndk/28.2.13676358}"
  PREBUILT="$NDK_HOME/toolchains/llvm/prebuilt"
  HOST_TAG="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
  SYSROOT="$PREBUILT/$HOST_TAG/sysroot/usr/lib"
  cp "$SYSROOT/aarch64-linux-android/libc++_shared.so" \
    android/app/src/main/jniLibs/arm64-v8a/
  cp "$SYSROOT/x86_64-linux-android/libc++_shared.so" \
    android/app/src/main/jniLibs/x86_64/
  scripts/check_llama_versions.sh
  LLAMA_CPP_DIR="${LLAMA_CPP_DIR:-$(cd .. && pwd)/llama.cpp}"
  LLAMA_ANDROID_BIN="${LLAMA_ANDROID_BIN:-$LLAMA_CPP_DIR/build-android-arm64/bin}"
  for lib in libggml-base.so libggml-cpu.so libggml.so libllama.so libllama-common.so; do
    cp "$LLAMA_ANDROID_BIN/$lib" android/app/src/main/jniLibs/arm64-v8a/
  done

# Cross-compile Rust for Android release (arm64-v8a only, matches CI)
build-android-release:
  #!/usr/bin/env bash
  set -e
  rm -rf android/app/src/main/jniLibs
  cargo ndk -o android/app/src/main/jniLibs -P 28 \
    -t arm64-v8a \
    build -p {{CORE_CRATE}} --release
  NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_HOME:-${HOME}/Android/Sdk}/ndk/28.2.13676358}"
  PREBUILT="$NDK_HOME/toolchains/llvm/prebuilt"
  HOST_TAG="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
  SYSROOT="$PREBUILT/$HOST_TAG/sysroot/usr/lib"
  cp "$SYSROOT/aarch64-linux-android/libc++_shared.so" \
    android/app/src/main/jniLibs/arm64-v8a/
  scripts/check_llama_versions.sh
  LLAMA_CPP_DIR="${LLAMA_CPP_DIR:-$(cd .. && pwd)/llama.cpp}"
  LLAMA_ANDROID_BIN="${LLAMA_ANDROID_BIN:-$LLAMA_CPP_DIR/build-android-arm64/bin}"
  for lib in libggml-base.so libggml-cpu.so libggml.so libllama.so libllama-common.so; do
    cp "$LLAMA_ANDROID_BIN/$lib" android/app/src/main/jniLibs/arm64-v8a/
  done

# Full Android debug pipeline: bindings -> cross-compile -> assembleDebug
android-full: bindings-kotlin build-android
  cd android && ./gradlew :app:assembleDebug

# Full Android release pipeline (matches CI): bindings -> arm64-only -> assembleRelease
android-release: bindings-kotlin build-android-release
  cd android && ./gradlew :app:assembleRelease

# Run the Android adb smoke test against an attached device
android-smoke:
  ./scripts/android_smoke_test.sh

# Run a generic mobile smoke scenario
mobile-smoke profile scenario:
  python3 tools/mobile-smoke/runner.py --profile {{profile}} --scenario {{scenario}}

# ── Desktop ───────────────────────────────────────────────────────────────────

# Run the iced desktop app (Phase 1 placeholder -- desktop shell added in Phase 4+)
run-desktop:
  cargo run -p mango-desktop
