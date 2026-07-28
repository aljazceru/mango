---
status: resolved
trigger: "Settings provider list missing attestation verification results and default provider indicator"
created: 2026-03-24T00:00:00Z
updated: 2026-03-25T14:00:00Z
---

## Current Focus

hypothesis: Four issues identified and fixed:
  1. (Prior session) AddBackendFromPreset / AddBackend / SetDefaultBackend did not spawn
     attestation task — fixed in commit a3a5564.
  2. (Prior session) iOS enabledPresetRow missing "Default" badge — fixed in a3a5564.
  3. (This session) Regression: attestation_statuses starts empty on every launch and is only
     populated after the async attestation task completes (~5-30s). If user opens Settings
     before the task finishes, no badge is shown. Fix: load cached attestation results from
     SQLite (attestation_cache table) into initial_state.attestation_statuses at startup.
  4. (This session) AddBackend, AddBackendFromPreset, SetActiveBackend, SetDefaultBackend all
     update in-memory state (active_backend_id, backends summaries) but do NOT emit a rev
     increment, so the UI never sees the change until an async task (health check, attestation)
     completes. Fixed by adding rev += 1; emit() at the end of each handler.
test: cargo build + cargo test -- 162 passed 0 failed
expecting: attestation badges appear immediately on Settings open (from cache); UI refreshes
  immediately when user clicks Enable/Set Default (no waiting for health check).
next_action: human verification

## Symptoms

expected: |
  1. Each enabled provider in the settings list shows its attestation verification status
     (e.g. "Attested", "Unverified", or the attestation type like "SEV-SNP verified")
  2. The provider that is currently set as the default has a clear visual indicator
     (e.g. a star, "Default" badge, or "Set as Default" button replaced by indicator)
  3. When the only enabled provider is auto-promoted to default (via AddBackendFromPreset
     or RemoveBackend promotion logic), the UI reflects this immediately
actual: |
  - Provider rows in settings show no attestation status
  - No visual distinction between the default provider and others on iOS
  - The auto-default logic works in Rust core but is invisible to the user in the UI
errors: none — purely a display/UI issue
reproduction: |
  Settings -> enable a provider -> provider row shows no attestation status and no default badge
timeline: introduced during the onboarding-provider-ux-redesign session

## Eliminated

- hypothesis: All three platform UIs are missing attestation/default display code
  evidence: Desktop (settings.rs) and Android (SettingsScreen.kt) already have both attestation
    display and default badge rendering. Only iOS is missing the Default badge.
  timestamp: 2026-03-24

- hypothesis: attestation_statuses is never populated in AppState
  evidence: lib.rs AttestationResult handler correctly upserts into attestation_statuses and
    increments rev. The issue is startup race (task completes after user opens Settings) and
    missing emit in action handlers.
  timestamp: 2026-03-25

- hypothesis: tee_type key mismatch prevents cache lookup
  evidence: BackendConfig.tee_type="IntelTdx" but cache stores tee_type="AmdSevSnp" (from
    attestation result). Fixed by adding get_latest_for_backend() that queries by backend_id
    only (ORDER BY verified_at DESC LIMIT 1), bypassing the tee_type mismatch.
  timestamp: 2026-03-25

## Evidence

- timestamp: 2026-03-24
  checked: rust/src/lib.rs AddBackendFromPreset handler
  found: spawns health check but NOT attestation task and NOT emit
  implication: newly-added preset providers never get attestation status populated, and
    the UI doesn't refresh until a health check completes

- timestamp: 2026-03-24
  checked: rust/src/lib.rs AddBackend handler
  found: same — only spawns health check, no attestation, no emit
  implication: custom backend additions also miss attestation and immediate UI refresh

- timestamp: 2026-03-24
  checked: rust/src/lib.rs SetDefaultBackend handler
  found: sets active_backend_id and calls refresh_backend_summaries but does NOT emit
  implication: changing the default backend doesn't update the UI until next async event

- timestamp: 2026-03-24
  checked: ios/ConfidentialApp/ConfidentialApp/SettingsView.swift enabledPresetRow
  found: block only hides "Set Default" button when isActive==true, no "Default" badge shown
  implication: when a provider IS the default, there's no visual indicator on iOS

- timestamp: 2026-03-25
  checked: rust/src/lib.rs startup (line ~1808)
  found: initial_state.attestation_statuses = vec![] (empty). Startup spawns attestation task
    but it runs asynchronously. If user opens Settings before it completes, no badge shown.
  implication: root cause of regression — badges appear only after async task, not immediately

- timestamp: 2026-03-25
  checked: rust/src/attestation/cache.rs AttestationCache::get()
  found: queries by (backend_id, tee_type). BackendConfig.tee_type="IntelTdx" but cached
    entry has tee_type="AmdSevSnp" (from AttestationEvent::ProviderVerified). Key mismatch
    means cache.get("tinfoil", "IntelTdx") returns None even if "AmdSevSnp" entry exists.
  implication: startup cache load needs to use backend_id-only query

## Resolution

root_cause: |
  Primary: attestation_statuses is initialized empty on every app launch. The async attestation
  task (which runs over HTTP) may take 5-30 seconds. If the user opens Settings during that
  window, they see no attestation badge. The SQLite attestation_cache table already stores
  verified attestation results from previous sessions, but was never loaded at startup.

  Secondary: AddBackend, AddBackendFromPreset, SetActiveBackend, SetDefaultBackend all update
  in-memory state (active_backend_id, backend summaries, toast) but never call rev += 1 / emit.
  The UI only refreshes when an async event (HealthCheckResult, AttestationResult) arrives.
  This means "Set Default" button presses appear to do nothing until the health check completes.

  Tertiary (pre-existing, fixed in a3a5564): attestation was only spawned on SetActiveBackend
  and app startup. AddBackend/AddBackendFromPreset/SetDefaultBackend never triggered attestation.

fix: |
  1. rust/src/attestation/cache.rs:
     Added AttestationCache::get_latest_for_backend(backend_id) — queries attestation_cache
     by backend_id only (ignoring tee_type), returns the most recent non-expired entry.
     This handles the tee_type mismatch between BackendConfig and cached AttestationRecord.

  2. rust/src/lib.rs (startup):
     After loading documents and before constructing ActorState, iterate all backends and
     call cache.get_latest_for_backend() for each, pushing non-expired entries into
     initial_state.attestation_statuses. Badges appear immediately on first Settings open.

  3. rust/src/lib.rs (action handlers):
     Added rev += 1; emit() at the end of:
       - AddBackend handler
       - AddBackendFromPreset handler (inside the if-let preset block)
       - SetActiveBackend handler (inside the if-any backend_id block)
       - SetDefaultBackend handler
     This ensures the UI refreshes immediately when user clicks Enable/Set Default/Switch.

verification: |
  cargo build -p confidential_app_core: 0 errors (4 pre-existing dead_code warnings)
  cargo test -p confidential_app_core --lib -- --test-threads=1: 162 passed, 0 failed, 7 ignored
files_changed:
  - rust/src/attestation/cache.rs
  - rust/src/lib.rs

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** lib.rs:6176-6198 cache load; iOS SettingsProvidersView.swift:215,315
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
