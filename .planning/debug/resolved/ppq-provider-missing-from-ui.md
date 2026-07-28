---
status: resolved
trigger: "ppq-provider-missing-from-ui"
created: 2026-03-26T00:00:00Z
updated: 2026-03-26T00:00:10Z
---

## Current Focus
<!-- OVERWRITE on each update - reflects NOW -->

hypothesis: CONFIRMED AND FIXED — PPQ.AI was seeded into the backends table by MIGRATION_V10 (no api_key). UI treated any backend in appState.backends as "enabled", hiding the API key input form. Fix: added has_api_key: bool to BackendSummary; UI now only shows the "enabled" card when has_api_key=true.
test: cargo check passes, all 170 tests pass, desktop compiles clean
expecting: Human confirms PPQ.AI now shows as a provider with an API key input form in Settings
next_action: await human verification

## Symptoms
<!-- Written during gathering, then IMMUTABLE -->

expected: User should be able to add PPQ.AI as a provider, either by creating a new account on PPQ.AI or by inputting an API key (similar to how Tinfoil provider works)
actual: No UI option exists to add PPQ.AI provider — neither account creation flow nor API key input
errors: None reported (feature simply absent)
reproduction: Open the app, navigate to provider/backend settings — PPQ.AI option is absent or incomplete
started: Recent — PPQ.AI backend was added in commits 53aea4c and 3b08eb3

## Eliminated
<!-- APPEND only - prevents re-investigating -->

- hypothesis: PPQ.AI preset is missing from known_provider_presets() in Rust core
  evidence: Checked rust/src/llm/backend.rs — PPQ.AI is present in known_provider_presets() at line 114
  timestamp: 2026-03-26T00:00:02Z

- hypothesis: Generated Kotlin bindings are missing PPQ.AI or the TeeType::AmdSevSnp variant
  evidence: Checked confidential_app_core.kt — TeeType.AMD_SEV_SNP is present at ordinal position 3; knownProviderPresets function is exported
  timestamp: 2026-03-26T00:00:03Z

- hypothesis: Android/iOS UI doesn't call knownProviderPresets() for the Settings screen
  evidence: Both SettingsScreen.kt and SettingsView.swift call knownProviderPresets() and iterate all presets
  timestamp: 2026-03-26T00:00:04Z

## Evidence
<!-- APPEND only - facts discovered -->

- timestamp: 2026-03-26T00:00:01Z
  checked: rust/src/llm/backend.rs — known_provider_presets()
  found: PPQ.AI is present at line 114 with id="ppq-ai", base_url="https://api.ppq.ai/v1/", tee_type=AmdSevSnp
  implication: Rust core is correct

- timestamp: 2026-03-26T00:00:02Z
  checked: android SettingsScreen.kt — isEnabled logic
  found: original check was `val isEnabled = enabledIds.contains(preset.id)` where enabledIds = appState.backends.map { it.id }
  implication: Any backend in appState.backends was treated as "enabled"

- timestamp: 2026-03-26T00:00:03Z
  checked: rust/src/persistence/schema.rs — MIGRATION_V10
  found: PPQ.AI is seeded into the backends table with is_active=0 and no API key on first launch
  implication: PPQ.AI row exists in backends table before user has ever interacted with it

- timestamp: 2026-03-26T00:00:04Z
  checked: rust/src/lib.rs — reload_backends()
  found: Loads ALL rows from backends table into actor_state.backends, using keychain.load() which returns "" for unset keys
  implication: PPQ.AI appears in appState.backends with api_key="" from the moment the app is installed

- timestamp: 2026-03-26T00:00:05Z
  checked: BackendSummary struct (rust/src/llm/backend.rs)
  found: BackendSummary had no field to indicate whether an API key has been configured
  implication: UI could not distinguish between "backend seeded with no key" and "backend properly configured by user"

- timestamp: 2026-03-26T00:00:08Z
  checked: cargo check and cargo test output
  found: All 170 tests pass; both core and desktop compile cleanly after adding has_api_key field
  implication: Fix is structurally sound

## Resolution
<!-- OVERWRITE as understanding evolves -->

root_cause: MIGRATION_V10 seeds PPQ.AI into the backends SQLite table (is_active=0, no api_key). reload_backends() loads all DB rows into actor_state.backends including this seeded row with api_key="". The Settings/Onboarding UI checked `appState.backends.map { it.id }.contains(preset.id)` to decide if a preset is "enabled" — so PPQ.AI was always shown as "enabled" (because it's always in the backends list), and the API key input row was never rendered for the user.

fix: Added `has_api_key: bool` to BackendSummary (set to `!self.api_key.is_empty()` in to_summary()). Updated all platform UI "isEnabled" checks to require has_api_key=true. Updated Android Kotlin bindings to include the new field in FfiConverterTypeBackendSummary (read/write/allocationSize).

verification: cargo check passes, all 170 unit tests pass, desktop compiles cleanly

files_changed:
  - rust/src/llm/backend.rs
  - android/app/src/main/java/com/example/confidentialapp/rust/confidential_app_core.kt
  - android/app/src/main/java/com/example/confidentialapp/ui/SettingsScreen.kt
  - ios/ConfidentialApp/ConfidentialApp/SettingsView.swift
  - desktop/iced/src/views/settings.rs

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** has_api_key backend.rs:75,89; MIGRATION_V10 schema.rs:198-208
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
