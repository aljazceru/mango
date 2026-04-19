#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export MANGO_SMOKE_MESSAGE="${MANGO_SMOKE_MESSAGE:-adb_smoke_ping}"
export MANGO_SMOKE_PIN="${MANGO_SMOKE_PIN:-1234}"

exec python3 "$ROOT_DIR/tools/mobile-smoke/runner.py" \
  --profile "$ROOT_DIR/tools/mobile-smoke/profiles/mango.yaml" \
  --scenario "$ROOT_DIR/tools/mobile-smoke/scenarios/mango_smoke.yaml" \
  "$@"
