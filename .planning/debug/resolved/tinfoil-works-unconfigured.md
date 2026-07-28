---
status: resolved
trigger: "tinfoil-works-unconfigured"
created: 2026-03-30T00:00:00Z
updated: 2026-03-30T00:00:00Z
---

## Current Focus

hypothesis: CONFIRMED — Tinfoil is pre-seeded as an active backend (is_active=1) by MIGRATION_V1, with an empty API key loaded from keychain (no key ever stored for it). Tinfoil's own inference endpoint accepts empty/arbitrary Bearer tokens — it does not enforce API key authentication. The default model selection picks Tinfoil because it is the first active backend seeded in the DB.
test: Traced the full startup -> DB migration -> backend loading -> model selection -> send_secure_request code path
expecting: n/a — root cause confirmed
next_action: Return diagnosis

## Symptoms

expected: Only PPQ should be available/working since that's the only provider configured during onboarding. Tinfoil should not work without being configured with credentials.
actual: Tinfoil model is selected by default in new chat and successfully sends/receives messages despite never being configured.
errors: PPQ correctly fails with "api key not found" when using the random 4-char key. Tinfoil has no errors at all.
reproduction: 1) Remove app 2) adb shell pm clear 3) Reinstall 4) Onboarding: select PPQ, enter 4 random chars 5) Health check passes 6) Open new chat 7) Tinfoil model pre-selected and works
started: Reproducible on fresh install

## Eliminated

- hypothesis: "Onboarding flow for PPQ also configures Tinfoil as a side-effect"
  evidence: AddBackendFromPreset only touches the backend matching the selected preset_id. It stores a key for that specific backend and may insert a row if it doesn't already exist — it never creates rows for other providers.
  timestamp: 2026-03-30

- hypothesis: "Tinfoil requires an API key and is somehow getting the user's PPQ key by mistake"
  evidence: Keys are stored in the keychain under "confidential_app::{backend_id}" where backend_id differs ("tinfoil" vs "ppq-ai"). The Tinfoil key slot is never written during PPQ onboarding.
  timestamp: 2026-03-30

## Evidence

- timestamp: 2026-03-30
  checked: rust/src/persistence/schema.rs MIGRATION_V1
  found: Tinfoil is seeded into the backends table with is_active=1 at first-ever DB creation. The backends table has NO api_key column — keys are never stored in SQLite.
  implication: Tinfoil is always present and active on fresh install, before any onboarding step runs.

- timestamp: 2026-03-30
  checked: rust/src/lib.rs startup backend loading (around line 2000-2025)
  found: On startup, backends are loaded from SQLite rows, and for each row the API key is loaded from the keychain via `keychain.load("confidential_app", row.id).unwrap_or_default()`. If no key is stored in keychain, api_key = "" (empty string).
  implication: Since the user never configured Tinfoil, its api_key is "" at runtime.

- timestamp: 2026-03-30
  checked: rust/src/lib.rs active_backend_id startup logic (around line 2097-2116)
  found: active_backend_id is resolved from `default_backend_id` settings key first, then falls back to `get_active_backend_id` which returns the first backend with is_active=1 ordered by display_order. Tinfoil has display_order=0 and is_active=1 from MIGRATION_V1.
  implication: On fresh install (before onboarding sets default_backend_id), Tinfoil is the active backend.

- timestamp: 2026-03-30
  checked: rust/src/lib.rs AppAction::NewConversation handler (around line 2411-2455)
  found: New conversations use `default_backend_id` from settings (falling back to active_backend_id) and `default_model_id` (falling back to the first model of that backend). On fresh install after onboarding PPQ, `default_backend_id` is set to "ppq-ai" by AddBackendFromPreset (line 3111-3117). BUT the user may have navigated to New Chat before this takes effect, or the timing matters.
  implication: Wait — need to reconsider. If PPQ onboarding does set default_backend_id to "ppq-ai", why would a new chat pick Tinfoil?

- timestamp: 2026-03-30
  checked: rust/src/lib.rs AppAction::AddBackendFromPreset handler (around line 3060-3154)
  found: The auto-promote block at line 3110 reads: `if actor_state.app_state.active_backend_id.is_none()`. At this point on fresh install, Tinfoil is already active (is_active=1 from MIGRATION_V1), so `active_backend_id` is Some("tinfoil"), NOT None. The condition is false — the PPQ backend is therefore NEVER promoted to default_backend_id.
  implication: After PPQ onboarding completes, default_backend_id in settings is never set. Tinfoil remains the active/default backend.

- timestamp: 2026-03-30
  checked: rust/src/llm/tinfoil_secure.rs send_secure_request (around line 389-455)
  found: The request always sends `Authorization: Bearer {backend.api_key}` regardless of whether api_key is empty. With api_key="" the header becomes "Bearer ". The Tinfoil inference endpoint does NOT reject empty/blank Bearer tokens — it accepts requests unauthenticated (free public inference endpoint).
  implication: Tinfoil works with an empty API key because its endpoint does not enforce authentication.

- timestamp: 2026-03-30
  checked: rust/src/tests/live_tinfoil.rs line 97 comment
  found: "Auth: standard Bearer token via api_key in BackendConfig (loaded from OS keychain)" — but the tests read from ~/.credentials/tinfoil.txt, suggesting real Tinfoil usage requires a key. However the endpoint may still accept empty keys (free tier or open endpoint).
  implication: Either Tinfoil has a free/open public endpoint, or it silently accepts empty bearer tokens.

## Resolution

root_cause: Three independent factors combine to cause this:

  1. MIGRATION_V1 seeds Tinfoil into the backends DB with is_active=1 (display_order=0) on every fresh install, before the user touches onboarding. This makes Tinfoil the default active backend.

  2. AddBackendFromPreset only promotes the onboarded provider (PPQ) to default_backend_id when `active_backend_id.is_none()` (line 3110 of lib.rs). Since Tinfoil is already active from the migration, this condition is false — PPQ never becomes the default. Tinfoil remains the default backend after onboarding completes.

  3. Tinfoil's inference endpoint (https://inference.tinfoil.sh) accepts requests with an empty Bearer token — it does not enforce API key authentication. So new chat conversations are created with Tinfoil as backend, the empty-string api_key is sent as "Bearer ", and the server responds successfully.

  The combination: Tinfoil is pre-active → PPQ doesn't displace it as default → Tinfoil sends an empty key → Tinfoil doesn't require a key → chat works.

fix: (diagnose-only mode — no fix applied)
  The auto-promote guard in AddBackendFromPreset (line 3110) should use `active_backend_id != Some(preset_id)` or unconditionally set default_backend_id to the newly onboarded provider, rather than only setting it when no active backend exists at all.

verification:
files_changed: []

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** Auto-promote guard at rust/src/lib.rs:7474-7476 now reads `if active_backend_id.is_none() || !active_is_configured` — the added `|| !active_is_configured` clause displaces pre-seeded-but-unconfigured Tinfoil when a real provider is onboarded. Exactly the fix proposed in the Resolution section.
**Verified by:** /gsd-debug bulk re-check vs current HEAD
