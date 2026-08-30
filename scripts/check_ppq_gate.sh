#!/usr/bin/env bash
# Wave 0 release gate for managed PPQ orchestration.
#
# A distributable build with MANAGED_PPQ_ENABLED=true may only be produced
# when docs/integrations/ppq-orchestration-approval.md declares
# "status: approved" and every expected sanitized fixture is present under
# docs/integrations/ppq-fixtures/ (plan Section 2, hard dependency rule).
#
# Usage:
#   scripts/check_ppq_gate.sh            # enforce gate (exit 0 = passed)
#   scripts/check_ppq_gate.sh --self-test # prove the gate FAILS on
#                                         # missing/pending/unsanitized input
set -euo pipefail

ROOT="${MANGO_PPQ_GATE_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
ARTIFACT_REL="docs/integrations/ppq-orchestration-approval.md"
FIXTURE_DIR_REL="docs/integrations/ppq-fixtures"

# Single source of truth for the required fixture set; mirrors Section 4 of
# the approval artifact.
EXPECTED_FIXTURES=(
  accounts_create.json
  topup_payment_methods.json
  topup_create_btc_lightning.json
  topup_status_pending.json
  topup_status_paid.json
  topup_status_expired.json
  topup_status_error.json
  credits_balance.json
  keys_list.json
  keys_create.json
)

fail() { echo "PPQ GATE FAIL: $*" >&2; exit 1; }
pass() { echo "PPQ gate passed: approval artifact approved, all ${#EXPECTED_FIXTURES[@]} sanitized fixtures present."; }

check_gate() {
  local artifact="$ROOT/$ARTIFACT_REL"
  local fixture_dir="$ROOT/$FIXTURE_DIR_REL"

  [[ -f "$artifact" ]] || fail "missing approval artifact $ARTIFACT_REL"
  grep -Eq '^status:[[:space:]]*approved[[:space:]]*$' "$artifact" \
    || fail "approval artifact does not declare 'status: approved' (Wave 0 not complete; MANAGED_PPQ_ENABLED must stay false)"

  for name in "${EXPECTED_FIXTURES[@]}"; do
    local f="$fixture_dir/$name"
    [[ -f "$f" ]] || fail "missing fixture $FIXTURE_DIR_REL/$name"
    [[ -s "$f" ]] || fail "empty fixture $FIXTURE_DIR_REL/$name"
    python3 -c "
import json, sys
with open('$f') as fh:
    doc = json.load(fh)
if doc.get('sanitized') is not True:
    sys.exit(1)
" || fail "fixture $FIXTURE_DIR_REL/$name is not valid sanitized JSON (must be an object with \"sanitized\": true; see ppq-fixtures/README.md)"
  done
}

self_test() {
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  local gate_root="$tmp/root"
  local artifact="$gate_root/$ARTIFACT_REL"
  local fixtures="$gate_root/$FIXTURE_DIR_REL"
  mkdir -p "$fixtures"

  expect_fail() { # name, command...
    local name="$1"; shift
    if "$@" >/dev/null 2>&1; then
      echo "self-test FAIL: '$name' should have failed" >&2
      exit 1
    fi
  }

  # 1. missing artifact
  expect_fail "missing artifact" env MANGO_PPQ_GATE_ROOT="$gate_root" "$0"

  # 2. pending status
  printf 'status: pending\n' > "$artifact"
  expect_fail "status pending" env MANGO_PPQ_GATE_ROOT="$gate_root" "$0"

  # 3. approved but fixtures missing
  printf '# approval\nstatus: approved\n' > "$artifact"
  expect_fail "fixtures missing" env MANGO_PPQ_GATE_ROOT="$gate_root" "$0"

  # 4. unsanitized fixture
  printf '{"response": {}}' > "$fixtures/accounts_create.json"
  expect_fail "unsanitized fixture" env MANGO_PPQ_GATE_ROOT="$gate_root" "$0"

  # 5. empty fixture
  printf '{"sanitized": true}' > "$fixtures/accounts_create.json"
  : > "$fixtures/topup_payment_methods.json"
  expect_fail "empty fixture" env MANGO_PPQ_GATE_ROOT="$gate_root" "$0"

  # 6. all good -> must pass
  for name in "${EXPECTED_FIXTURES[@]}"; do
    printf '{"sanitized": true, "endpoint": "/x", "captured_at": "1970-01-01T00:00:00Z"}' > "$fixtures/$name"
  done
  if ! env MANGO_PPQ_GATE_ROOT="$gate_root" "$0" >/dev/null; then
    echo "self-test FAIL: complete fixture set should pass" >&2
    exit 1
  fi

  trap - EXIT
  rm -rf "$tmp"
  echo "PPQ gate self-test passed (fails on missing/pending/unsanitized, passes on complete set)."
}

case "${1:-}" in
  --self-test) self_test ;;
  "") check_gate && pass ;;
  *) echo "Unknown argument: $1" >&2; exit 2 ;;
esac
