{
  description = "Mango -- multi-platform personal AI platform with TEE attestation";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    android-nixpkgs = {
      url = "github:tadfisher/android-nixpkgs";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, flake-utils, rust-overlay, android-nixpkgs, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
          config.allowUnfree = true;
          config.android_sdk.accept_license = true;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [
            # iOS targets
            "aarch64-apple-ios"
            "aarch64-apple-ios-sim"
            "x86_64-apple-ios"
            # macOS targets
            "aarch64-apple-darwin"
            "x86_64-apple-darwin"
            # Android targets
            "aarch64-linux-android"
            "armv7-linux-androideabi"
            "x86_64-linux-android"
          ];
        };

        androidSdk = android-nixpkgs.sdk.${system} (sdkPkgs: with sdkPkgs; [
          cmdline-tools-latest
          platform-tools
          build-tools-34-0-0
          build-tools-35-0-0
          platforms-android-34
          platforms-android-35
          ndk-28-2-13676358
          emulator
          (if pkgs.stdenv.isDarwin
           then system-images-android-35-google-apis-arm64-v8a
           else system-images-android-35-google-apis-x86-64)
        ]);

        shell = pkgs.mkShell {
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];

          packages = [
            rustToolchain
            androidSdk
            pkgs.just
            pkgs.cargo-ndk
            pkgs.nodejs_22
            pkgs.python3
            pkgs.curl
            pkgs.git
            pkgs.gradle
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.xcodegen
          ];

          shellHook = ''
            export IN_NIX_SHELL=1
            export ANDROID_HOME=${androidSdk}/share/android-sdk
            export ANDROID_SDK_ROOT=${androidSdk}/share/android-sdk
            export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/28.2.13676358"
            export PATH=$ANDROID_HOME/emulator:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$PATH
            export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"

            if [ "$(uname -s)" = "Darwin" ]; then
              if [ -n "''${DEVELOPER_DIR:-}" ] && [ -x "''${DEVELOPER_DIR}/usr/bin/simctl" ]; then
                DEV_DIR="$DEVELOPER_DIR"
              else
                DEV_DIR="$(xcode-select -p 2>/dev/null || true)"
              fi
              if [ -n "$DEV_DIR" ] && [ -d "$DEV_DIR/Toolchains/XcodeDefault.xctoolchain/usr/bin" ]; then
                export DEVELOPER_DIR="$DEV_DIR"
                TOOLCHAIN_BIN="$DEV_DIR/Toolchains/XcodeDefault.xctoolchain/usr/bin"
                export CC="$TOOLCHAIN_BIN/clang"
                export CXX="$TOOLCHAIN_BIN/clang++"
                export AR="$TOOLCHAIN_BIN/ar"
                export RANLIB="$TOOLCHAIN_BIN/ranlib"
              fi
            fi

            mkdir -p android
            cat > android/local.properties <<EOF
            sdk.dir=$ANDROID_HOME
EOF

            echo ""
            echo "Mango dev environment ready"
            echo "  Rust: $(rustc --version)"
            echo ""
          '';
        };
      in {
        devShells.default = shell;
      }
    );
}
