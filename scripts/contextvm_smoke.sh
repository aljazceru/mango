#!/usr/bin/env bash
# contextvm end-to-end smoke test against a real Android device / emulator.
#
# What it does:
#   1. Builds the contextvm-echo-server (if needed).
#   2. Starts the echo server on public Nostr relays in the background.
#   3. Waits for the server to announce its pubkey (up to 30 s).
#   4. Runs the mobile-smoke contextvm_echo scenario with that pubkey.
#   5. Tears down the echo server on exit.
#
# Usage:
#   ./scripts/contextvm_smoke.sh [extra runner flags]
#
# Examples:
#   ./scripts/contextvm_smoke.sh --record
#   ./scripts/contextvm_smoke.sh --install --serial emulator-5554
#
# Optional env:
#   CONTEXTVM_SECRET_KEY   — 64-char hex, reuse identity across runs
#   MANGO_SMOKE_PIN        — app PIN (default: 1234)
#   MANGO_SMOKE_MESSAGE    — message to send (default triggers echo tool)
#   MANGO_SMOKE_LLM_BASE_URL — OpenAI-compatible base URL; default starts a local mock
#   ANDROID_SERIAL         — passed straight through to adb

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export MANGO_SMOKE_PIN="${MANGO_SMOKE_PIN:-1234}"
export MANGO_SMOKE_MESSAGE="${MANGO_SMOKE_MESSAGE:-use the echo tool with the message: hello from smoke test}"
export MANGO_SMOKE_LLM_MODEL="${MANGO_SMOKE_LLM_MODEL:-mango-smoke-model}"
export MANGO_SMOKE_LLM_API_KEY="${MANGO_SMOKE_LLM_API_KEY:-mango-smoke-key}"
export MANGO_SMOKE_ECHO_EXPECTED="${MANGO_SMOKE_ECHO_EXPECTED:-Echo: hello from smoke test}"

SERVER_LOG="$(mktemp /tmp/contextvm-echo-server.XXXXXX.log)"
SERVER_PID=""
MOCK_LLM_LOG="$(mktemp /tmp/mango-smoke-openai.XXXXXX.log)"
MOCK_LLM_PID=""

cleanup() {
    if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "[smoke] stopping echo server (pid $SERVER_PID)"
        kill "$SERVER_PID" 2>/dev/null || true
    fi
    if [[ -n "$MOCK_LLM_PID" ]] && kill -0 "$MOCK_LLM_PID" 2>/dev/null; then
        echo "[smoke] stopping mock OpenAI provider (pid $MOCK_LLM_PID)"
        kill "$MOCK_LLM_PID" 2>/dev/null || true
    fi
    rm -f "$SERVER_LOG"
    rm -f "$MOCK_LLM_LOG"
}
trap cleanup EXIT

# ── 0. Start local OpenAI-compatible mock LLM ─────────────────────────
if [[ -z "${MANGO_SMOKE_LLM_BASE_URL:-}" ]]; then
    echo "[smoke] starting mock OpenAI-compatible provider..."
    python3 "$ROOT/tools/mobile-smoke/mock_openai_provider.py" \
        --host 127.0.0.1 \
        --port 0 \
        --model "$MANGO_SMOKE_LLM_MODEL" >"$MOCK_LLM_LOG" 2>&1 &
    MOCK_LLM_PID=$!

    MOCK_LLM_HOST_URL=""
    for i in $(seq 1 30); do
        if ! kill -0 "$MOCK_LLM_PID" 2>/dev/null; then
            echo "[smoke] ERROR: mock OpenAI provider exited early. Log:" >&2
            cat "$MOCK_LLM_LOG" >&2
            exit 1
        fi
        MOCK_LINE=$(grep "^Mock OpenAI provider:" "$MOCK_LLM_LOG" 2>/dev/null || true)
        if [[ -n "$MOCK_LINE" ]]; then
            MOCK_LLM_HOST_URL="${MOCK_LINE#Mock OpenAI provider: }"
            MOCK_LLM_HOST_URL="${MOCK_LLM_HOST_URL%%[[:space:]]*}"
            break
        fi
        sleep 1
    done
    if [[ -z "$MOCK_LLM_HOST_URL" ]]; then
        echo "[smoke] ERROR: mock OpenAI provider did not print a URL. Log:" >&2
        cat "$MOCK_LLM_LOG" >&2
        exit 1
    fi
    MOCK_PORT="${MOCK_LLM_HOST_URL##*:}"
    MOCK_PORT="${MOCK_PORT%/v1}"
    export MANGO_SMOKE_LLM_BASE_URL="http://10.0.2.2:${MOCK_PORT}/v1"
    echo "[smoke] mock provider base URL for emulator: $MANGO_SMOKE_LLM_BASE_URL"
fi

# ── 1. Build echo server ──────────────────────────────────────────────
echo "[smoke] building contextvm-echo-server..."
cargo build -p contextvm-echo-server --quiet 2>&1

# ── 2. Start echo server ──────────────────────────────────────────────
echo "[smoke] starting echo server on public relays..."
if [[ -n "${CONTEXTVM_SECRET_KEY:-}" ]]; then
    CONTEXTVM_SECRET_KEY="$CONTEXTVM_SECRET_KEY" \
        "$ROOT/target/debug/contextvm-echo-server" >"$SERVER_LOG" 2>&1 &
else
    "$ROOT/target/debug/contextvm-echo-server" >"$SERVER_LOG" 2>&1 &
fi
SERVER_PID=$!

# ── 3. Wait for pubkey + serving transport ───────────────────────────
echo "[smoke] waiting for echo server pubkey (up to 30 s)..."
ECHO_SERVER_PUBKEY=""
for i in $(seq 1 30); do
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "[smoke] ERROR: echo server exited early. Log:" >&2
        cat "$SERVER_LOG" >&2
        exit 1
    fi
    PUBKEY_LINE=$(grep "^Server pubkey:" "$SERVER_LOG" 2>/dev/null || true)
    if [[ -n "$PUBKEY_LINE" ]]; then
        ECHO_SERVER_PUBKEY="${PUBKEY_LINE#Server pubkey: }"
        ECHO_SERVER_PUBKEY="${ECHO_SERVER_PUBKEY%%[[:space:]]*}"
        break
    fi
    sleep 1
done

if [[ -z "$ECHO_SERVER_PUBKEY" ]]; then
    echo "[smoke] ERROR: echo server did not print a pubkey within 30 s. Log:" >&2
    cat "$SERVER_LOG" >&2
    exit 1
fi

echo "[smoke] echo server pubkey: $ECHO_SERVER_PUBKEY"

echo "[smoke] waiting for echo server readiness (up to 45 s)..."
for i in $(seq 1 45); do
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "[smoke] ERROR: echo server exited before readiness. Log:" >&2
        cat "$SERVER_LOG" >&2
        exit 1
    fi
    if grep -q "^Server ready$" "$SERVER_LOG" 2>/dev/null; then
        break
    fi
    if [[ "$i" == "45" ]]; then
        echo "[smoke] ERROR: echo server did not become ready. Log:" >&2
        cat "$SERVER_LOG" >&2
        exit 1
    fi
    sleep 1
done

# ── 4. Run mobile smoke scenario ─────────────────────────────────────
export ECHO_SERVER_PUBKEY
set +e
python3 "$ROOT/tools/mobile-smoke/runner.py" \
    --profile   "$ROOT/tools/mobile-smoke/profiles/mango.yaml" \
    --scenario  "$ROOT/tools/mobile-smoke/scenarios/contextvm_echo.yaml" \
    --reset-app-data \
    "$@"
STATUS=$?
set -e
if [[ "$STATUS" -ne 0 ]]; then
    echo "[smoke] mock OpenAI provider log:" >&2
    cat "$MOCK_LLM_LOG" >&2 || true
    echo "[smoke] echo server log:" >&2
    cat "$SERVER_LOG" >&2 || true
    exit "$STATUS"
fi
