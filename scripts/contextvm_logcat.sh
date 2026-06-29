#!/usr/bin/env bash
# Level-4 contextvm tool invocation trace for a connected Android device.
#
# What it shows:
#   - Secret key loading / creation
#   - dispatch_tools routing decisions (local vs remote arm)
#   - invoke_tool: relay connect, send, receive, timeout
#   - Full tool call result returned to the LLM
#
# Usage:
#   ./scripts/contextvm_logcat.sh              # all contextvm traces
#   ./scripts/contextvm_logcat.sh --errors     # only errors + timeouts
#   ./scripts/contextvm_logcat.sh --full       # all app logs (noisy)
#
# Requires: adb in PATH, device connected and app installed.

set -euo pipefail

MODE="contextvm"
if [[ "${1:-}" == "--errors" ]]; then
  MODE="errors"
elif [[ "${1:-}" == "--full" ]]; then
  MODE="full"
fi

APP_PKG="dev.disobey.mango.dev"

if ! adb get-state &>/dev/null; then
  echo "ERROR: no device found. Connect a device or start an emulator." >&2
  exit 1
fi

echo "=== contextvm live trace ==="
echo "Package : $APP_PKG"
echo "Mode    : $MODE"
echo "Device  : $(adb get-serialno 2>/dev/null || echo unknown)"
echo ""
echo "Tip: enable a contextvm tool in Settings → Tools, then send a message"
echo "     that should trigger it. Watch for 'invoke_tool' and 'dispatch_tools'."
echo ""
echo "Press Ctrl+C to stop."
echo "============================================"

# Clear stale logs first so we only see fresh output.
adb logcat -c

case "$MODE" in
  errors)
    adb logcat -v time \
      | grep --line-buffered -iE \
          "contextvm|invoke_tool|dispatch_tools|secret.key|relay.*unreachable|timed out|tool.*error|Error.*tool"
    ;;
  full)
    adb logcat -v time "$APP_PKG":V "*:S"
    ;;
  *)
    # Default: contextvm-relevant tags + Rust log lines that mention
    # the relevant functions / error strings.
    adb logcat -v time \
      | grep --line-buffered -iE \
          "contextvm_sdk|invoke_tool|dispatch_tools|secret.key|relay.*unreachable|tools/call|timed out \(15s\)|NostrMCPProxy|contextvm_secret_key|hydrate_from_db|finalise_for_turn|remote tool|reserved.name|provider_pubkey"
    ;;
esac
