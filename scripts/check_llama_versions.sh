#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION_FILE="$ROOT/android/llama.cpp.version"

if [[ ! -f "$VERSION_FILE" ]]; then
  echo "Missing llama.cpp version manifest: $VERSION_FILE" >&2
  exit 1
fi

# shellcheck source=/dev/null
source "$VERSION_FILE"

CHECK_LIBS=1
for arg in "$@"; do
  case "$arg" in
    --skip-libs) CHECK_LIBS=0 ;;
    *)
      echo "Unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

LLAMA_CPP_DIR="${LLAMA_CPP_DIR:-$(cd "$ROOT/.." && pwd)/llama.cpp}"
LLAMA_ANDROID_BIN="${LLAMA_ANDROID_BIN:-$LLAMA_CPP_DIR/build-android-arm64/bin}"

if [[ ! -d "$LLAMA_CPP_DIR/.git" ]]; then
  echo "Missing llama.cpp checkout at $LLAMA_CPP_DIR; run 'just fetch-llama-cpp' or set LLAMA_CPP_DIR" >&2
  exit 1
fi

actual_commit="$(git -C "$LLAMA_CPP_DIR" rev-parse HEAD)"
if [[ "$actual_commit" != "$LLAMA_CPP_COMMIT" ]]; then
  echo "llama.cpp checkout is not pinned to $LLAMA_CPP_TAG ($LLAMA_CPP_COMMIT)" >&2
  echo "  actual: $actual_commit" >&2
  echo "Run 'just fetch-llama-cpp' or set LLAMA_CPP_DIR to the pinned checkout." >&2
  exit 1
fi

if [[ -n "$(git -C "$LLAMA_CPP_DIR" status --short --untracked-files=no)" ]]; then
  echo "llama.cpp checkout has tracked local modifications; rebuild from a clean $LLAMA_CPP_TAG checkout" >&2
  exit 1
fi

for header in include/llama.h ggml/include/ggml.h; do
  if [[ ! -f "$LLAMA_CPP_DIR/$header" ]]; then
    echo "Missing llama.cpp header: $LLAMA_CPP_DIR/$header" >&2
    exit 1
  fi
done

if ! grep -q "exactVersion: \"$LLAMA_SWIFT_VERSION\"" "$ROOT/ios/Mango/project.yml"; then
  echo "iOS LlamaSwift version does not match android/llama.cpp.version ($LLAMA_SWIFT_VERSION)" >&2
  exit 1
fi

if [[ "$CHECK_LIBS" == "1" ]]; then
  for lib in libggml-base.so libggml-cpu.so libggml.so libllama.so; do
    if [[ ! -f "$LLAMA_ANDROID_BIN/$lib" ]]; then
      echo "Missing pinned llama.cpp Android library: $LLAMA_ANDROID_BIN/$lib" >&2
      echo "Run 'just build-llama-android' or set LLAMA_ANDROID_BIN." >&2
      exit 1
    fi
  done
fi

echo "llama.cpp pinned: $LLAMA_CPP_TAG ($LLAMA_CPP_COMMIT); LlamaSwift $LLAMA_SWIFT_VERSION"
