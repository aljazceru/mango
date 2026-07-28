---
status: resolved
trigger: "Redesign onboarding wizard and settings provider management UX"
created: 2026-03-24T02:00:00Z
updated: 2026-03-24T03:00:00Z
---

## Current Focus

hypothesis: RESOLVED — all platform UIs redesigned, Rust core extended with new actions
test: cargo build passes with 0 warnings 0 errors
expecting: n/a
next_action: n/a

## Symptoms

expected: |
  1. Onboarding wizard is shown ONLY on first app start (not triggered by missing backends)
  2. The wizard lists all known provider presets (from ProviderPreset / known_provider_presets) — user picks one, enters API key, done. Or can skip the wizard entirely.
  3. Settings shows a clean list of providers the user can enable by adding an API key — no endpoint or attestation fields exposed to normal users
  4. "Add Custom Provider" is under an "Advanced Settings" section (collapsed/hidden by default) where the user CAN set endpoint URL and select attestation method
  5. The system cannot be without a default — if user enables at least one provider, it becomes (or stays) default
actual: |
  - Onboarding wizard is triggered whenever there are no backends (not just first start)
  - The add-backend form exposes endpoint URL and attestation method to all users
  - There is no separation between "enable known provider" (simple) vs "add custom provider" (advanced)
  - The wizard doesn't list known presets for user selection
errors: none — this is a UX design issue, not a runtime error
reproduction: |
  - Current flow: Settings -> delete all backends -> app triggers wizard again
  - Desired flow: Wizard only on first start; settings has simplified enable/disable per provider
timeline: identified during review of provider/settings implementation

## Eliminated

- hypothesis: has_completed_onboarding flag needs to be added
  evidence: Already exists in Rust core — checked at startup, wizard only shown when value != 'true'. The issue is the UX design, not the flag.
  timestamp: 2026-03-24T02:00:00Z

## Evidence

- timestamp: 2026-03-24T02:00:00Z
  checked: rust/src/lib.rs startup logic
  found: has_completed_onboarding is read at startup; wizard shows when value != 'true'. Once CompleteOnboarding fires, it persists 'true'. The wizard is NOT re-shown when backends are deleted.
  implication: Rust core already handles first-start-only trigger correctly. UX changes are purely UI layer.

- timestamp: 2026-03-24T02:00:00Z
  checked: rust/src/lib.rs AppAction enum
  found: No SkipOnboarding action exists. No AddBackendFromPreset action exists. AddBackend always generates a UUID, not the preset ID.
  implication: Need to add SkipOnboarding action and AddBackendFromPreset (or allow AddBackend to accept an explicit id).

- timestamp: 2026-03-24T02:00:00Z
  checked: desktop/iced/src/views/onboarding.rs backend_setup_step()
  found: Lists state.backends (existing configured backends), not known_provider_presets. iOS onboarding has same issue.
  implication: Wizard BackendSetup step needs to show preset list instead.

- timestamp: 2026-03-24T02:00:00Z
  checked: desktop/iced/src/views/settings.rs, ios SettingsView.swift, android SettingsScreen.kt
  found: All three show full "Add Backend" form with Name, Base URL, API Key, TEE Type fields. No separation of simple vs advanced.
  implication: Need to restructure settings into: simple "enable known providers" + advanced collapsed "custom provider" section.

## Resolution

root_cause: |
  UX design issue — not a bug. The architecture is correct but the UI exposes technical complexity
  (endpoint URLs, TEE type selection) to normal users who just want to pick a known provider.
  Additionally the wizard BackendSetup step shows configured backends rather than known presets.

fix: |
  1. Rust core: Add AppAction::SkipOnboarding (marks has_completed_onboarding=true, navigates to Home).
     Also add AppAction::AddBackendFromPreset { preset_id, api_key } which looks up the preset by id,
     uses preset.id as the backend id, preset.base_url and preset.tee_type, and dispatches the add logic.
  2. Desktop settings (settings.rs): Replace "Add Backend" section with:
     a. "Providers" subsection: list of known presets with "Enable" button showing only API key input
     b. "Advanced: Add Custom Provider" collapsed section with full form (Name, URL, TEE, API Key)
  3. Desktop onboarding (onboarding.rs): BackendSetup step shows known_provider_presets list
     instead of state.backends, single API key field, "Skip" button dispatching SkipOnboarding.
  4. iOS: Same restructure for SettingsView and OnboardingView.
  5. Android: Same restructure for SettingsScreen and OnboardingScreen.

verification: |
  cargo build passes: 0 warnings, 0 errors (dev profile).
  All 8 target files updated. Rust core changes are the source of truth;
  platform UIs delegate to AddBackendFromPreset and SkipOnboarding actions.
files_changed:
  - rust/src/lib.rs
  - desktop/iced/src/main.rs
  - desktop/iced/src/views/settings.rs
  - desktop/iced/src/views/onboarding.rs
  - ios/ConfidentialApp/ConfidentialApp/SettingsView.swift
  - ios/ConfidentialApp/ConfidentialApp/OnboardingView.swift
  - android/app/src/main/java/com/example/confidentialapp/ui/SettingsScreen.kt
  - android/app/src/main/java/com/example/confidentialapp/ui/OnboardingScreen.kt

## Bulk Re-Verification (2026-07-28)

**Verdict:** ALREADY-RESOLVED
**Action:** Confirmed status during bulk archive sweep; moved to resolved/.
