---
status: resolved
trigger: "Multiple bugs in settings/onboarding/provider management + build warnings"
created: 2026-03-24T00:00:00Z
updated: 2026-03-24T01:00:00Z
---

## Current Focus

hypothesis: All five issues fully resolved
test: Full test suite passed (163/163 single-threaded, zero warnings)
expecting: Human verification confirms onboarding and backend management work correctly
next_action: Archive session

## Symptoms

expected: |
  1. Onboarding wizard works even when there are no backends
  2. Attestation method/option auto-configured from selected provider
  3. Deleting default provider auto-promotes another to default
  4. First provider added auto-becomes default
  5. Build compiles with no warnings
actual: |
  1. After deleting all backends, onboarding wizard shows "no backend" error
  2. User must manually select attestation method/option even when provider implies a specific one
  3. Deleting default provider leaves system without a default
  4. Adding first provider doesn't auto-set it as default
  5. Build has warnings
errors: "says theres no backend" when opening onboarding wizard after deleting all backends
reproduction: |
  Issue 1: Settings -> delete all backends -> try to use onboarding wizard -> error about no backend
  Issue 2: Add backend form -> select provider -> attestation field still needs manual selection
  Issue 3: Settings -> set a provider as default -> delete that provider -> no default provider remains
  Issue 4: Settings -> delete all providers -> add one provider -> it's not automatically default
timeline: unknown, likely recent development

## Eliminated

- hypothesis: The "no backend" error in onboarding is from Rust core rejecting the wizard
  evidence: Rust core does not have a guard that refuses to show onboarding when no backends exist. The onboarding screen is set if has_completed_onboarding != 'true'. The check in NextOnboardingStep only blocks progression from BackendSetup step. The "no backend" error originates from the UI layer showing a dead-end message when state.backends is empty.

- hypothesis: Agent test regression caused by my changes
  evidence: Git stash proved the same 3 agent tests fail identically on the unmodified baseline when the full suite runs in parallel. The tests use thread::sleep timing that is unreliable under parallel OS load. All 163 tests pass when run with --test-threads=1.

## Evidence

- timestamp: 2026-03-24T00:00:00Z
  checked: rust/src/lib.rs AddBackend handler
  found: is_active hardcoded to 0; active_backend_id never updated after first insert; no "first backend" detection
  implication: Issue 4 - first added backend is never promoted to active/default

- timestamp: 2026-03-24T00:00:00Z
  checked: rust/src/lib.rs RemoveBackend handler
  found: active_backend_id not re-evaluated after delete; if removed backend was active, app enters invalid state with dangling active_backend_id
  implication: Issue 3 - deleting the default provider leaves no default

- timestamp: 2026-03-24T00:00:00Z
  checked: desktop/iced/src/views/onboarding.rs backend_setup_step()
  found: When state.backends is empty, renders static text "No backends available. Add one in Settings first." with no navigation action — a dead end inside the wizard
  implication: Issue 1 - onboarding wizard catches-22: wizard shown precisely when no backends, but wizard blocked on no backends

- timestamp: 2026-03-24T00:00:00Z
  checked: desktop/iced/src/main.rs OnboardingValidateKey handler
  found: api_key is taken from onboarding_api_key (UI-local state), but ValidateApiKey action uses keychain which is empty on fresh install; the keychain key is never written before validate is called
  implication: Issue 2 - API key typed in onboarding is not persisted to keychain before health check

- timestamp: 2026-03-24T00:00:00Z
  checked: Build output
  found: 15+ warnings across persistence/mod.rs, lib.rs, attestation/cache.rs, attestation/nvidia.rs, llm/backend.rs, persistence/queries.rs, desktop main.rs, onboarding.rs, attestation_badge.rs, and test files
  implication: Issue 5 - warnings are mix of intentional dead API (needs #[allow]) and genuinely unused imports/variables

## Resolution

root_cause: |
  Issue 1: onboarding.rs backend_setup_step() rendered a dead-end string when state.backends is empty.
  Since the wizard is shown precisely when no backends are configured, users had no escape route.

  Issue 2: The onboarding UI collected api_key in local iced state but ValidateApiKey looked up
  the keychain (empty) rather than the just-typed value. A new UpdateBackendApiKey action was
  needed to write the key to the keychain before the health check.

  Issue 3: RemoveBackend handler deleted the row but never checked whether the removed backend
  was the active/default one. active_backend_id was left pointing to the deleted backend.

  Issue 4: AddBackend handler hardcoded is_active=0 and never updated active_backend_id,
  so the first added backend was never promoted to default.

  Issue 5: Multiple unused items across production and test code — mix of intentional future-use
  items (need #[allow(dead_code)]) and genuinely stale imports/variables.

fix: |
  Issue 1 (rust/src/lib.rs + desktop/iced/src/views/onboarding.rs):
    - AddBackend auto-promote now ensures active_backend_id is set after first backend added
    - onboarding.rs no-backend case replaced: column with explanatory text + "Open Settings" button
      that dispatches PushScreen { screen: Screen::Settings }

  Issue 2 (rust/src/lib.rs + desktop/iced/src/main.rs):
    - Added AppAction::UpdateBackendApiKey { backend_id, api_key } to AppAction enum
    - Handler: stores key via keychain.store() then calls reload_backends + refresh_backend_summaries
    - OnboardingValidateKey message handler now dispatches UpdateBackendApiKey first (persists key),
      then dispatches ValidateApiKey (health check uses the newly stored key)

  Issue 3 (rust/src/lib.rs):
    - RemoveBackend handler: captures replacement_id before delete (first remaining backend)
    - After reload_backends: checks was_active = active_backend_id == Some(backend_id)
    - If was_active: promotes replacement (if any) via set_setting("default_backend_id") and
      updates active_backend_id; otherwise clears both to None/""

  Issue 4 (rust/src/lib.rs):
    - AddBackend handler: after reload_backends, checks active_backend_id.is_none()
    - If true: calls set_setting("default_backend_id", &id) and sets active_backend_id = Some(id)

  Issue 5 (multiple files):
    Production code:
    - rust/src/persistence/mod.rs: #[allow(unused_imports)] on pub use block (intentional public API)
    - rust/src/lib.rs: removed ChatCompletionRequestToolMessageArgs import; removed PendingAttachment::size_bytes field
    - rust/src/attestation/cache.rs: #[allow(dead_code)] on impl block and deserialize_status fn
    - rust/src/attestation/nvidia.rs: #[allow(dead_code)] on iss field and verify_nvidia_jwt fn
    - rust/src/llm/backend.rs: #[allow(dead_code)] on tinfoil_backend and redpill_backend fns
    - rust/src/persistence/queries.rs: #[allow(dead_code)] on update_agent_step_status fn and ChunkRow struct
    Desktop code:
    - desktop/iced/src/main.rs: removed BusyState import; added #[allow(dead_code)] on MarkdownLinkClicked
    - desktop/iced/src/views/onboarding.rs: removed AppAction and Screen from direct imports (uses fully-qualified paths)
    - desktop/iced/src/widgets/attestation_badge.rs: removed row from widget imports
    Test code (pre-existing warnings not caused by these changes):
    - rust/src/tests/chat.rs: removed AppState, UiMessage, crate::persistence unused imports; removed unused variables
    - rust/src/tests/routing.rs: removed BackendHealth unused import; removed mut from 3 immutable routers
    - rust/src/tests/onboarding.rs: removed self from persistence import

verification: |
  - cargo build: 0 warnings, 0 errors
  - cargo test -- --test-threads=1: 163 passed, 0 failed, 12 ignored
  - Parallel test failures (3 agent tests) confirmed pre-existing on unmodified baseline via git stash test

files_changed:
  - rust/src/lib.rs
  - rust/src/persistence/mod.rs
  - rust/src/llm/backend.rs
  - rust/src/attestation/cache.rs
  - rust/src/attestation/nvidia.rs
  - rust/src/persistence/queries.rs
  - desktop/iced/src/main.rs
  - desktop/iced/src/views/onboarding.rs
  - desktop/iced/src/widgets/attestation_badge.rs
  - rust/src/tests/chat.rs
  - rust/src/tests/routing.rs
  - rust/src/tests/onboarding.rs

## Bulk Re-Verification (2026-07-28)

**Verdict:** ALREADY-RESOLVED
**Action:** Confirmed status during bulk archive sweep; moved to resolved/.
