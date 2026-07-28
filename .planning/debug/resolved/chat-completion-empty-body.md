---
status: resolved
trigger: "chat-completion-empty-body"
created: 2026-03-24T04:00:00Z
updated: 2026-03-24T04:00:00Z
---

## Current Focus

hypothesis: CONFIRMED - AddBackendFromPreset keychain.store() is inside the `if !already` block.
  Tinfoil and Redpill are seeded by MIGRATION_V1, so already=true on first launch. The user's
  API key entered in onboarding or settings is silently discarded. Backend api_key remains "".
  spawn_streaming_task sends "Authorization: Bearer " (empty) and the provider returns a
  plain-text error that async-openai fails to JSON-parse, producing the observed error string.
test: Root cause confirmed by code inspection of lines 2390-2443 in lib.rs and schema.rs MIGRATION_V1.
expecting: Fix moves keychain.store() outside if !already; adds reload_backends in the already path.
next_action: Apply fix to rust/src/lib.rs AddBackendFromPreset handler, run cargo build + tests

## Symptoms

expected: Chat messages are sent to the backend provider and a streaming response is returned
actual: Error: "connection failed: failed to deserialize api response: error: expected value at line 1 column 1 content::chat completion request body is empty"
errors: "connection failed: failed to deserialize api response: error: expected value at line 1 column 1 content::chat completion request body is empty"
reproduction: |
  Enable a provider in settings -> open a chat -> type a message -> send -> error appears
started: |
  Likely introduced by recent changes to the provider/onboarding UX redesign session
  (AddBackendFromPreset was introduced, api key flow was changed)

## Eliminated

- hypothesis: messages Vec is empty when spawn_streaming_task is called
  evidence: do_send_message pushes user message before building chat_messages; actor is single-threaded
  so no race can clear messages between the push and the loop at line 953.
  timestamp: 2026-03-24T04:10:00Z

- hypothesis: model string is empty causing select_backend to return None
  evidence: select_backend with empty model_id returns None and sets "No healthy backend available"
  not "chat completion request body is empty". That error is distinct and server-generated.
  timestamp: 2026-03-24T04:10:00Z

- hypothesis: select_backend routing logic is wrong
  evidence: routing logic correctly requires model in backend.models. The bug is upstream of routing.
  timestamp: 2026-03-24T04:10:00Z

## Evidence

- timestamp: 2026-03-24T04:05:00Z
  checked: rust/src/persistence/schema.rs MIGRATION_V1
  found: Tinfoil and Redpill are seeded by the migration with model lists. is_active=1 for Tinfoil.
  implication: Both backends exist in DB from first launch; AddBackendFromPreset sees them as already present.

- timestamp: 2026-03-24T04:07:00Z
  checked: rust/src/lib.rs AddBackendFromPreset handler (lines 2390-2443)
  found: Line 2395 checks `already = actor_state.backends.iter().any(|b| b.id == preset_id)`.
  For Tinfoil and Redpill (always present from migration), already = true on first launch.
  The keychain store (lines 2411-2415) is INSIDE the `if !already` block.
  When already = true, the entire block is skipped — the user's API key is silently discarded.
  implication: Any API key entered by the user in onboarding wizard or settings for Tinfoil/Redpill
  is never written to the keychain. The backend api_key remains "".

- timestamp: 2026-03-24T04:09:00Z
  checked: rust/src/lib.rs reload_backends (line 706)
  found: api_key loaded from keychain.load(...).unwrap_or_default(). If keychain has no entry, api_key = "".
  implication: spawn_streaming_task is called with api_key = "". Provider gets "Authorization: Bearer "
  (empty token). If Tinfoil's /models endpoint does not require auth, validation passes with empty key.
  The streaming /chat/completions endpoint does require auth — returns plain-text error.
  The plain-text response "chat completion request body is empty" fails JSON parse in async-openai,
  producing the exact error observed.

## Resolution

root_cause: |
  AddBackendFromPreset keychain store is inside the `if !already` guard. Tinfoil and Redpill are
  seeded by MIGRATION_V1 so they always exist on first launch. The `already = true` path skips
  the entire block including the keychain.store() call. Users who enter their API key in the
  onboarding wizard or settings provider list have their key silently discarded. The backend
  api_key stays "" and streaming requests are sent with "Authorization: Bearer " (empty token).
  Providers return a plain-text error that async-openai cannot JSON-parse, producing:
  "connection failed: failed to deserialize api response: error: expected value at line 1 column 1
  content::chat completion request body is empty"

fix: |
  In AddBackendFromPreset handler: move the keychain.store() call OUTSIDE the `if !already` block,
  so it always updates the stored API key regardless of whether the backend already existed.
  Also call reload_backends after the keychain store in the `already = true` path, so the
  in-memory BackendConfig.api_key is updated immediately.

verification: |
  cargo build: 0 warnings, 0 errors
  cargo test -- --test-threads=1: 163 passed, 0 failed, 12 ignored
files_changed:
  - rust/src/lib.rs

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** lib.rs:8010-8022 keychain.store() before guard, comment cites this bug
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
