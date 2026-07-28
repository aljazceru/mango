---
status: resolved
trigger: "Implement periodic-attestation-refresh"
created: 2026-03-25T00:00:00Z
updated: 2026-03-25T00:00:00Z
symptoms_prefilled: true
---

## Current Focus

hypothesis: Feature not yet implemented — new feature addition
test: Code inspection complete, implementing now
expecting: After implementation: background timer re-runs attestation every N minutes; interval persisted; UI exposed in Advanced Settings
next_action: Implement all changes

## Symptoms

expected: |
  1. A background timer re-runs spawn_attestation_task for the active backend every N minutes
  2. Default interval is 15 minutes
  3. The interval is stored in app settings/preferences (persisted across restarts)
  4. The setting is exposed in the "Advanced Settings" section of the settings UI on all platforms
  5. The timer resets when the active backend changes
  6. The timer is cancelled/reset when the interval setting changes
actual: Attestation only runs on specific events (startup, add backend, change default, onboarding) — no periodic refresh
errors: none — this is a new feature
reproduction: N/A — feature request
timeline: N/A — new feature

## Eliminated

(none)

## Evidence

- timestamp: 2026-03-25T00:00:00Z
  checked: rust/src/lib.rs actor loop, InternalEvent enum, CoreMsg enum
  found: |
    - CoreMsg has two variants: Action(AppAction) and InternalEvent(Box<llm::InternalEvent>)
    - The actor loop is `while let Ok(msg) = core_rx.recv()` — synchronous blocking recv
    - InternalEvent variants: StreamChunk, StreamDone, StreamError, StreamCancelled,
      AttestationResult, HealthCheckResult, EmbeddingComplete, AgentStepComplete
    - spawn_attestation_task is called at startup, SetActiveBackend, AddBackend,
      AddBackendFromPreset, SetDefaultBackend
    - settings are stored as key/value pairs in the `settings` SQLite table via get_setting/set_setting
    - default interval should be stored as "attestation_interval_minutes" = "15"
  implication: Need to add AttestationTick to InternalEvent, add timer task, add SetAttestationInterval action, expose interval in AppState

- timestamp: 2026-03-25T00:00:00Z
  checked: desktop/iced/src/views/settings.rs, ios SettingsView.swift, android SettingsScreen.kt
  found: |
    - Desktop: Advanced section toggle controlled by show_advanced bool param; form rendered inside `if show_advanced` block
    - iOS: advancedSection uses DisclosureGroup("Add Custom Provider", isExpanded: $showAdvanced)
    - Android: showAdvanced bool state, content shown in if (showAdvanced) block
    - All three platforms already have the advanced section pattern
    - Desktop view fn signature includes show_advanced param; Message enum has SettingsToggleAdvanced
    - AppState does NOT yet have attestation_interval_minutes field
  implication: Need to add attestation_interval_minutes to AppState, add SetAttestationInterval action, add UI controls in all three platforms

## Resolution

root_cause: Feature not yet implemented
fix: |
  1. rust/src/lib.rs:
     - Add `attestation_interval_minutes: u32` to AppState (default 15)
     - Add `AppAction::SetAttestationInterval { minutes: u32 }` to AppAction enum
     - Add `InternalEvent::AttestationTick` to llm/streaming.rs InternalEvent enum
     - Add timer field to ActorState: `attestation_timer_token: Option<tokio_util::sync::CancellationToken>`
     - Load interval from settings on startup
     - Spawn timer task after startup (helper fn spawn_attestation_timer)
     - Handle InternalEvent::AttestationTick: run attestation for active backend
     - Handle AppAction::SetAttestationInterval: persist, update AppState, reset timer
     - Reset timer in SetActiveBackend and SetDefaultBackend handlers
  2. Desktop: add SettingsAttestationIntervalChanged(String) message + UI stepper/input in advanced section
  3. iOS: add @State attestationIntervalInput, Stepper in advancedSection
  4. Android: add attestationIntervalInput state, OutlinedTextField in advanced section
verification: |
  cargo build: 0 warnings, 0 errors
  cargo test -- --test-threads=1: 163 passed, 0 failed, 12 ignored
files_changed:
  - rust/src/lib.rs
  - rust/src/llm/streaming.rs
  - desktop/iced/src/views/settings.rs
  - desktop/iced/src/main.rs
  - ios/ConfidentialApp/ConfidentialApp/SettingsView.swift
  - android/app/src/main/java/com/example/confidentialapp/ui/SettingsScreen.kt

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** SetAttestationInterval lib.rs:789,8420; AttestationTick streaming.rs:73
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
