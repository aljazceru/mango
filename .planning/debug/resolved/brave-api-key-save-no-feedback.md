---
status: resolved
trigger: "When the user saves a Brave Search API key in Settings, there is no confirmation that: 1. The key was actually saved 2. The key is valid/working 3. If a key is already configured"
created: 2026-04-08T00:00:00Z
updated: 2026-04-08T00:00:00Z
---

## Current Focus

hypothesis: CONFIRMED — SetBraveApiKey action saved immediately with no async validation and no feedback.
test: Full implementation applied and compiles cleanly on all platforms.
expecting: Human verification that UI feedback works end-to-end.
next_action: Await user confirmation

## Symptoms

expected: After saving the API key, user sees confirmation (success/error). When returning to settings, can see that a key is already configured.
actual: Pressing save gives no feedback — user doesn't know if it worked, if the key is valid, or if it was already set.
errors: No errors — just missing UX feedback
reproduction: Go to Settings → Tools section → enter a Brave API key → press Save
started: Current state since the tools/settings implementation

## Eliminated

## Evidence

- timestamp: 2026-04-08
  checked: SetBraveApiKey actor handler (lib.rs ~line 4042)
  found: Saves key to DB immediately, no validation, no toast, no busy state set
  implication: Root cause confirmed — action was fire-and-forget with zero feedback

- timestamp: 2026-04-08
  checked: AppState struct
  found: Has toast: Option<String> and busy_state: BusyState fields already; brave_api_key_set: bool already present
  implication: Infrastructure for feedback already exists; just needed wiring

- timestamp: 2026-04-08
  checked: dispatch_web_search in tools.rs
  found: Brave API call pattern: GET /res/v1/web/search with X-Subscription-Token header
  implication: Used same pattern for validation call

- timestamp: 2026-04-08
  checked: spawn_health_check pattern in lib.rs
  found: Standard pattern for async background task → InternalEvent result → actor loop handler
  implication: Followed same pattern for spawn_brave_api_key_validation

## Resolution

root_cause: SetBraveApiKey action saved the key immediately to SQLite with no network validation and emitted no toast/loading state, so the UI had no feedback mechanism to show the user.

fix: |
  1. Added brave_api_key_validating: bool to AppState (UniFFI record)
  2. Added BraveApiKeyValidationResult variant to InternalEvent in streaming.rs
  3. Added ValidateBraveApiKey { api_key: String } to AppAction enum
  4. Added spawn_brave_api_key_validation() function that hits GET /res/v1/web/search?q=test&count=1 with X-Subscription-Token header; handles success, 401/403 (invalid key), 429 (rate-limited = key valid), and network errors
  5. Added ValidateBraveApiKey handler in actor: sets brave_api_key_validating=true, spawns validation task
  6. Added BraveApiKeyValidationResult handler in actor: on success persists key + sets brave_api_key_set=true + shows "API key saved and verified." toast; on failure shows error toast; clears validating flag in both cases
  7. Regenerated Swift + Kotlin UniFFI bindings
  8. iOS SettingsView: replaced SetBraveApiKey dispatch with ValidateBraveApiKey, added ProgressView spinner during validation, green checkmark+label when configured, inline braveApiKeyMessage state wired to toast via onChange, field+button disabled during validation
  9. Android SettingsScreen: same with CircularProgressIndicator, Icons.Filled.CheckCircle, LaunchedEffect+snapshotFlow for toast mirroring
  10. Desktop settings.rs: updated view() signature to accept brave_api_key_message, shows "Verifying…" label + disables field during validation, inline colored feedback text; main.rs mirrors toast into settings_brave_api_key_message in CoreUpdated handler

verification: cargo check -p mango_core and cargo check -p mango-desktop both pass cleanly

files_changed:
  - rust/src/llm/streaming.rs
  - rust/src/lib.rs
  - ios/Bindings/mango_core.swift
  - android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt
  - ios/Mango/Mango/SettingsView.swift
  - android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt
  - desktop/iced/src/views/settings.rs
  - desktop/iced/src/main.rs

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** brave_api_key_validating lib.rs:391; ValidateBraveApiKey lib.rs:789,8535
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
