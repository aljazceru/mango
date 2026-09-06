#!/usr/bin/env bash
# Compile-only regression check for the NumKong / Android NDK `syscall`
# declaration incompatibility fixed by the vendored crate in vendor/numkong
# (see vendor/numkong/VENDOR.md).
#
# It compiles a C++ TU that includes <unistd.h> before <numkong/capabilities.h>
# — the same order produced by usearch's cxx bridge — with three toolchains.
# ALL THREE checks are mandatory on the supported host (Linux with the
# Android NDK installed); a missing compiler, missing headers, or an
# unsupported host configuration is a hard failure, never a silent pass:
#
#   1. NDK clang++ for x86_64-linux-android   (failed before the patch)
#   2. NDK clang++ for aarch64-linux-android  (unaffected path, must stay green)
#   3. Host C++ compiler on glibc Linux       (unaffected path, must stay green)
#
# On the unpatched header, case 1 fails with:
#   error: exception specification in declaration does not match previous
#   declaration ... `extern "C" long syscall(long, ...) noexcept;`
# (numkong 7.8.1 and 7.8.2 are both affected.)
#
# Usage:
#   scripts/check_numkong_android_headers.sh
#   scripts/check_numkong_android_headers.sh --allow-missing   # partial check
#
# --allow-missing turns missing tools into explicit skips and reports
# "RESULT: INCOMPLETE"; it never claims full verification.
#
# Optional env vars:
#   NDK_HOME             Android NDK root (default: $ANDROID_NDK_HOME,
#                        $ANDROID_HOME/ndk/28.2.13676358, or ~/Android/Sdk/ndk/28.2.13676358)
#   NUMKONG_INCLUDE_DIR  NumKong include dir to test
#                        (default: <repo>/vendor/numkong/include)
#   ANDROID_API          NDK API level suffix (default: 28)

set -euo pipefail

ALLOW_MISSING=0
for arg in "$@"; do
    case "$arg" in
        --allow-missing) ALLOW_MISSING=1 ;;
        -h|--help) sed -n '2,40p' "$0"; exit 0 ;;
        *) echo "Unknown argument: $arg" >&2; exit 2 ;;
    esac
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NDK_HOME="${NDK_HOME:-${ANDROID_NDK_HOME:-${ANDROID_HOME:-$HOME/Android/Sdk}/ndk/28.2.13676358}}"
NUMKONG_INCLUDE_DIR="${NUMKONG_INCLUDE_DIR:-$REPO_ROOT/vendor/numkong/include}"
ANDROID_API="${ANDROID_API:-28}"

HOST_TAG="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
PREBUILT="$NDK_HOME/toolchains/llvm/prebuilt/$HOST_TAG/bin"

failed=0
skipped=0

check() { # <name> <compiler>
    local name="$1" compiler="$2"
    if [[ ! -x "$compiler" ]]; then
        if [[ "$ALLOW_MISSING" -eq 1 ]]; then
            echo "SKIP  $name — compiler not found: $compiler"
            skipped=$((skipped + 1))
        else
            echo "FAIL  $name — required compiler not found: $compiler"
            failed=1
        fi
        return
    fi
    if "$compiler" -std=c++17 -fsyntax-only -I"$NUMKONG_INCLUDE_DIR" "$TU"; then
        echo "PASS  $name"
    else
        echo "FAIL  $name"
        failed=1
    fi
}

# Headers under test must exist — a wrong/missing include dir is a failure.
if [[ ! -f "$NUMKONG_INCLUDE_DIR/numkong/capabilities.h" ]]; then
    echo "FAIL  headers — not found: $NUMKONG_INCLUDE_DIR/numkong/capabilities.h" >&2
    exit 1
fi
if [[ ! -d "$NDK_HOME" ]]; then
    if [[ "$ALLOW_MISSING" -eq 1 ]]; then
        echo "SKIP  android-* — NDK not found: $NDK_HOME"
        skipped=$((skipped + 2))
    else
        echo "FAIL  android-* — required NDK not found: $NDK_HOME" >&2
        exit 1
    fi
fi

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
TU="$TMPDIR/numkong_syscall_check.cpp"
cat > "$TU" <<'EOF'
// Bionic's <unistd.h> declares `syscall` without an exception specification;
// numkong 7.8.1 and 7.8.2 redeclare it `noexcept` for Linux x86_64/RISC-V
// targets. Including <unistd.h> first reproduces the conflicting-declaration
// order seen in usearch's rust/lib.cpp translation unit.
#include <unistd.h>
#include "numkong/capabilities.h"
int main() { return (int)nk_capabilities(); }
EOF

echo "NumKong headers under test: $NUMKONG_INCLUDE_DIR"
echo "NDK: $NDK_HOME"

if [[ -d "$NDK_HOME" ]]; then
    check "android-x86_64" "$PREBUILT/x86_64-linux-android${ANDROID_API}-clang++"
    check "android-arm64"  "$PREBUILT/aarch64-linux-android${ANDROID_API}-clang++"
fi

host_cxx=""
for cxx in clang++ g++ c++; do
    if command -v "$cxx" >/dev/null 2>&1; then
        host_cxx="$(command -v "$cxx")"
        break
    fi
done
if [[ -n "$host_cxx" ]]; then
    check "host-linux" "$host_cxx"
elif [[ "$ALLOW_MISSING" -eq 1 ]]; then
    echo "SKIP  host-linux — no C++ compiler found"
    skipped=$((skipped + 1))
else
    echo "FAIL  host-linux — no C++ compiler found (need clang++, g++, or c++)"
    failed=1
fi

if [[ "$failed" -ne 0 ]]; then
    echo "RESULT: FAIL — NumKong headers are not compatible with all checked targets" >&2
    exit 1
fi
if [[ "$skipped" -gt 0 ]]; then
    echo "RESULT: INCOMPLETE — $skipped required check(s) skipped (--allow-missing); this is NOT full verification" >&2
    exit 0
fi
echo "RESULT: PASS — NumKong headers compile on all checked targets"
