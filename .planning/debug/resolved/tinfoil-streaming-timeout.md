---
status: resolved
trigger: "live_e2e_tinfoil test times out at Streaming timed out — streaming request never completes"
created: 2026-03-25T00:00:00Z
updated: 2026-03-25T12:35:00Z
---

## Current Focus

hypothesis: CONFIRMED — base_url seeded as "https://inference.tinfoil.sh/v1/" (trailing slash). async-openai 0.33 constructs URL as format!("{}{}", api_base, "/chat/completions") → double slash "https://inference.tinfoil.sh/v1//chat/completions". Tinfoil rejects this with plain text "chat completions request body is empty". async-openai cannot parse this as JSON → StreamError { NetworkError } → busy_state=Idle with no assistant → test loops to 30s deadline.
test: RUST_LOG=debug cargo test confirmed: streaming logs show error=Network error: failed to deserialize api response: error:expected value at line 1 column 1 content:chat completions request body is empty
expecting: n/a — confirmed via logs
next_action: trim trailing slash from base_url in spawn_streaming_task when building OpenAIConfig

## Symptoms

expected: live_e2e_tinfoil completes within 30s with a streaming response from Tinfoil
actual: test runs for full 31s then panics with "Streaming timed out" at live_tinfoil.rs:227
errors: |
  thread 'tests::live_tinfoil::live_e2e_tinfoil' panicked at rust/src/tests/live_tinfoil.rs:227:9:
  Streaming timed out
reproduction: cargo test -p confidential_app_core --lib -- live_tinfoil::live_e2e_tinfoil --ignored --nocapture 2>&1
started: After two quick tasks on 2026-03-25: quick-260325-gmx (added logging) and quick-260325-hb4 (removed Redpill provider)

## Eliminated

- hypothesis: MIGRATION_V8 wiped Tinfoil data
  evidence: MIGRATION_V8 only deletes rows WHERE id = 'redpill' — Tinfoil row is untouched
  timestamp: 2026-03-25

- hypothesis: API key not loaded from keychain
  evidence: keychain service/key match exactly ("confidential_app"/"tinfoil") between seed_keychain() and reload_backends(); keychain path is correct
  timestamp: 2026-03-25

- hypothesis: SetActiveBackend fails to find tinfoil backend
  evidence: MIGRATION_V1 seeds tinfoil with id='tinfoil', MIGRATION_V8 does not delete it; SetActiveBackend check backends.iter().any(|b| b.id == backend_id) will succeed
  timestamp: 2026-03-25

- hypothesis: TINFOIL_MODEL constant mismatch (stale "meta-llama/Llama-3.3-70B-Instruct")
  evidence: Model ID fix was applied (now "llama3-3-70b") but test still failed — RUST_LOG=debug shows streaming task WAS spawned and the model ID routed correctly; error is in the API response, not routing
  timestamp: 2026-03-25

- hypothesis: API key wrong / auth failure
  evidence: RUST_LOG=debug shows StreamError content is "chat completions request body is empty" (not 401), and keychain round-trip assertion passed
  timestamp: 2026-03-25

## Evidence

- timestamp: 2026-03-25
  checked: rust/src/persistence/schema.rs MIGRATION_V1 and MIGRATION_V7
  found: MIGRATION_V1 seeds Tinfoil model_list as '["llama3-3-70b","deepseek-r1-0528","kimi-k2-5"]'; MIGRATION_V7 patches existing DBs with the same list
  implication: The in-memory backend.models list for "tinfoil" contains ["llama3-3-70b", ...] not "meta-llama/Llama-3.3-70B-Instruct"

- timestamp: 2026-03-25
  checked: rust/src/llm/router.rs FailoverRouter::select_backend() line 74
  found: "if !backend.models.iter().any(|m| m == model_id) { return false; }" — model must be in the list
  implication: Passing model_id="meta-llama/Llama-3.3-70B-Instruct" causes select_backend to return None for all backends

- timestamp: 2026-03-25
  checked: rust/src/lib.rs do_send_message() lines 892-904
  found: if select_backend returns None → sets last_error = "No healthy backend available" and returns early; busy_state is never set to Streaming; no streaming task is spawned
  implication: Test loop sees busy_state=Idle but has_assistant=false → times out at 30s (exact observed behavior)

- timestamp: 2026-03-25
  checked: git log for rust/src/persistence/schema.rs
  found: commit 8c5abd5 (2026-03-24) changed TINFOIL_MODEL from "meta-llama/Llama-3.3-70B-Instruct" to "llama3-3-70b" in the seed data, but live_tinfoil.rs const TINFOIL_MODEL was not updated
  implication: This is the exact change that introduced the mismatch (model ID fix was necessary but not sufficient)

- timestamp: 2026-03-25
  checked: RUST_LOG=debug test output
  found: streaming task IS spawned (log: "[streaming] connection setup base_url=https://inference.tinfoil.sh/v1/ model=llama3-3-70b"); then "[WARN streaming] stream error ... error=Network error: failed to deserialize api response: error:expected value at line 1 column 1 content:chat completions request body is empty"
  implication: The request reaches Tinfoil but the server rejects it with plain text (not JSON). async-openai fails to parse the response → StreamError → busy_state=Idle → no assistant message → test loops to deadline.

- timestamp: 2026-03-25
  checked: async-openai 0.33.1 src/config.rs line 220
  found: fn url(&self, path: &str) -> String { format!("{}{}", self.api_base, path) } — no trailing slash stripping. path="/chat/completions" (leading slash). base_url="https://inference.tinfoil.sh/v1/" (trailing slash from DB seed MIGRATION_V1).
  implication: URL constructed = "https://inference.tinfoil.sh/v1//chat/completions" (double slash). Tinfoil's server receives malformed URL and responds with plain-text "chat completions request body is empty" instead of JSON error.

## Resolution

root_cause: base_url seeded in MIGRATION_V1 as 'https://inference.tinfoil.sh/v1/' (with trailing slash). async-openai 0.33.1 constructs chat completions URL as format!("{}{}", api_base, "/chat/completions") → double slash "https://inference.tinfoil.sh/v1//chat/completions". Tinfoil's server responds with plain text "chat completions request body is empty" (not valid JSON). async-openai fails to deserialize this → StreamError { NetworkError } → busy_state=Idle with no assistant message → test loops to 30s deadline and panics "Streaming timed out".
fix: In spawn_streaming_task (rust/src/llm/streaming.rs), strip trailing slash from base_url before passing to OpenAIConfig::with_api_base. One-line change: let base_url = backend.base_url.trim_end_matches('/').to_string(). (spawn_health_check in lib.rs already used trim_end_matches('/') when constructing its /models URL.)
verification: RUST_LOG=streaming=debug cargo test live_tinfoil::live_e2e_tinfoil --ignored passes in 2.25s. Streaming log shows "base_url=https://inference.tinfoil.sh/v1" (no trailing slash), stream completes, "Step 2/3: Streaming OK — \"it works\"", "Step 3/3: Attestation terminal status = ProviderVerified", "OK: live_e2e_tinfoil PASSED".
files_changed: [rust/src/llm/streaming.rs]
