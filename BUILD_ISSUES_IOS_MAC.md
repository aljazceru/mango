# iOS / macOS Build Issues

Issues identified by comparing with readstr reference project. Address these when working on iOS/macOS builds.

## Missing: `tools/` directory

Readstr has three build helper scripts that bridge Nix environment with Xcode toolchain:

### `tools/cargo-with-xcode`
Wraps `cargo` on macOS to unset conflicting Nix env vars (NIX_LDFLAGS, NIX_CFLAGS_COMPILE, LIBRARY_PATH, SDKROOT) and set Xcode clang/ar/ranlib. Without this, desktop builds on macOS break when Nix and Xcode fight over compiler paths.

### `tools/xcode-dev-dir`
Locates Xcode Developer directory with fallback chain:
1. DEVELOPER_DIR env var (flake pins this)
2. `xcode-select -p` output
3. Scan `/Applications/Xcode*.app/Contents/Developer`
Validates the directory has simctl + clang before returning.

### `tools/xcode-run`
Wraps `xcodebuild` calls with clean Xcode environment, stripping Nix vars. Used for `ios-build` and `ios-xcframework` targets.

## Missing: iOS CI release publishing

Current iOS CI builds for simulator but doesn't:
- Archive for distribution (IPA)
- Attach to GitHub releases on tag push
- Handle code signing for TestFlight/App Store

Readstr's Android CI publishes APKs to GitHub releases; iOS should follow the same pattern once signing is set up.

## Missing: Desktop CI Windows target

Readstr desktop CI builds on Linux + macOS + Windows (matrix strategy). Current desktop CI only covers Linux and macOS.

## justfile iOS targets use raw `xcode-select`

The `build-ios` and `ios-xcframework` targets call `xcode-select -p` directly instead of using a `tools/xcode-dev-dir` helper. This works but lacks the fallback chain and validation that readstr's helper provides.

## XCFramework name mismatch risk

Verify that `ios/Mango/project.yml` framework reference matches the `XCF_NAME` in justfile (`MangoCore`). Readstr had a similar mismatch between project.yml and justfile.
