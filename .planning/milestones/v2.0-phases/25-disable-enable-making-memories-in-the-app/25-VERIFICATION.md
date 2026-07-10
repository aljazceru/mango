---
phase: 25-disable-enable-making-memories-in-the-app
verified: 2026-04-05T14:00:00Z
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 25: Disable/Enable Making Memories Verification Report

**Phase Goal:** Add a user-facing toggle in the Settings MEMORY section to disable/enable automatic memory extraction. When disabled, no new memories are extracted after conversations complete. When re-enabled, extraction resumes. The toggle state persists across app restarts. Default is enabled (true).
**Verified:** 2026-04-05T14:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #  | Truth                                                                 | Status     | Evidence                                                                                                    |
|----|-----------------------------------------------------------------------|------------|-------------------------------------------------------------------------------------------------------------|
| 1  | memories_enabled defaults to true on fresh install and after upgrade  | VERIFIED  | `AppState::default()` sets `memories_enabled: true`; startup load uses `.unwrap_or(true)` (lib.rs:294, 2509) |
| 2  | SetMemoriesEnabled persists '0'/'1' to settings table and updates AppState | VERIFIED  | Handler at lib.rs:3812–3818 calls `set_setting("memories_enabled", "1"/"0")` and sets `actor_state.app_state.memories_enabled` |
| 3  | Memory extraction is skipped when memories_enabled is false           | VERIFIED  | lib.rs:3946–3947: `actor_state.app_state.memories_enabled && memory::extract::should_extract(...)` — outermost guard |
| 4  | iOS Settings MEMORY section shows an Auto-extract Memories toggle     | VERIFIED  | SettingsView.swift:211–216: `Toggle(isOn:)` with `appState.memoriesEnabled` dispatching `.setMemoriesEnabled` |
| 5  | Android Settings MEMORY section shows an Auto-extract Memories switch | VERIFIED  | SettingsScreen.kt:403–418: `Switch` row with `appState.memoriesEnabled` dispatching `AppAction.SetMemoriesEnabled` |
| 6  | Desktop Settings MEMORY section shows an Auto-extract Memories toggler | VERIFIED  | settings.rs:518–524: `toggler(state.memories_enabled).on_toggle(Message::SettingsMemoriesEnabledToggled)` |
| 7  | Toggling on any platform dispatches SetMemoriesEnabled to Rust core   | VERIFIED  | iOS: `dispatch(.setMemoriesEnabled(enabled:))`; Android: `onDispatch(AppAction.SetMemoriesEnabled(enabled=))`; Desktop: main.rs:765–766 handler dispatches `AppAction::SetMemoriesEnabled` |
| 8  | Toggle state reflects appState.memoriesEnabled from Rust              | VERIFIED  | All three platforms bind toggle state directly to `appState.memoriesEnabled` / `state.memories_enabled` |
| 9  | Unit test confirms round-trip toggle persistence                      | VERIFIED  | `cargo test -p mango_core test_memories_enabled_toggle` passes: default true -> disable -> false -> re-enable -> true |

**Score:** 9/9 truths verified

### Required Artifacts

| Artifact                                                                           | Expected                                             | Status   | Details                                                                  |
|------------------------------------------------------------------------------------|------------------------------------------------------|----------|--------------------------------------------------------------------------|
| `rust/src/lib.rs`                                                                  | memories_enabled field, action, handler, gate        | VERIFIED | Contains `pub memories_enabled: bool`, `SetMemoriesEnabled { enabled: bool }`, handler, startup load, extraction gate |
| `rust/src/tests/settings.rs`                                                       | Unit test for toggle persistence                     | VERIFIED | `fn test_memories_enabled_toggle()` at line 172, passes                  |
| `ios/Mango/Mango/SettingsView.swift`                                               | Toggle row in memorySection                          | VERIFIED | Line 211–216: `Label("Auto-extract Memories")` with live binding         |
| `android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt`                | Switch row in MEMORY card                            | VERIFIED | Lines 403–418: Switch dispatching `AppAction.SetMemoriesEnabled`         |
| `desktop/iced/src/views/settings.rs`                                               | toggler widget in memory section                     | VERIFIED | Lines 518–524: `toggler(state.memories_enabled).on_toggle(...)`          |
| `ios/Bindings/mango_core.swift`                                                    | Updated UniFFI bindings with memoriesEnabled         | VERIFIED | 7 occurrences of `memoriesEnabled`; `.setMemoriesEnabled` case present   |
| `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt`                  | Updated UniFFI bindings with memoriesEnabled         | VERIFIED | `var memoriesEnabled: kotlin.Boolean` in AppState; `SetMemoriesEnabled` in AppAction |
| `android/app/src/main/java/dev/disobey/mango/AppManager.kt`                       | Default AppState includes memoriesEnabled = true     | VERIFIED | Line 81: `memoriesEnabled = true` in hardcoded default constructor       |
| `desktop/iced/src/main.rs`                                                         | SettingsMemoriesEnabledToggled message and handler   | VERIFIED | Line 309: `SettingsMemoriesEnabledToggled(bool)`; line 765: handler dispatches `AppAction::SetMemoriesEnabled` |

### Key Link Verification

| From                                        | To                                  | Via                                              | Status   | Details                                                              |
|---------------------------------------------|-------------------------------------|--------------------------------------------------|----------|----------------------------------------------------------------------|
| AppAction::SetMemoriesEnabled handler       | persistence::queries::set_setting   | `set_setting(conn, "memories_enabled", "1"/"0")` | WIRED   | lib.rs:3813–3815 — confirmed pattern                                 |
| StreamDone handler                          | actor_state.app_state.memories_enabled | guard check before should_extract             | WIRED   | lib.rs:3946: `actor_state.app_state.memories_enabled && memory::extract::should_extract(...)` |
| ios/SettingsView.swift Toggle               | AppAction.setMemoriesEnabled        | `appManager.dispatch`                            | WIRED   | SettingsView.swift:213: `appManager.dispatch(.setMemoriesEnabled(enabled: $0))` |
| android/SettingsScreen.kt Switch            | AppAction.SetMemoriesEnabled        | onDispatch                                       | WIRED   | SettingsScreen.kt:416: `onDispatch(AppAction.SetMemoriesEnabled(enabled = checked))` |
| desktop/settings.rs toggler                 | Message::SettingsMemoriesEnabledToggled | on_toggle                                   | WIRED   | settings.rs:523: `.on_toggle(Message::SettingsMemoriesEnabledToggled)`; main.rs:765 handler confirmed |

### Data-Flow Trace (Level 4)

| Artifact                              | Data Variable      | Source                                       | Produces Real Data | Status    |
|---------------------------------------|--------------------|----------------------------------------------|--------------------|-----------|
| `ios/SettingsView.swift` Toggle       | `appState.memoriesEnabled` | UniFFI-bridged AppState from Rust actor | Yes — from settings table DB query on startup | FLOWING |
| `android/SettingsScreen.kt` Switch    | `appState.memoriesEnabled` | AppManager Kotlin state from UniFFI         | Yes — from settings table DB query on startup | FLOWING |
| `desktop/views/settings.rs` toggler   | `state.memories_enabled`   | Rust AppState passed into view function      | Yes — from settings table DB query on startup | FLOWING |

### Behavioral Spot-Checks

| Behavior                                        | Command                                                       | Result             | Status |
|-------------------------------------------------|---------------------------------------------------------------|--------------------|--------|
| Unit test: toggle round-trip persistence        | `cargo test -p mango_core test_memories_enabled_toggle`       | 1 passed; 0 failed | PASS  |

### Requirements Coverage

| Requirement   | Source Plan | Description                                                      | Status    | Evidence                                                                        |
|---------------|-------------|------------------------------------------------------------------|-----------|---------------------------------------------------------------------------------|
| MEM-TOGGLE-01 | 25-01       | memories_enabled bool in AppState, default true, persist/load   | SATISFIED | AppState field at lib.rs:258; default at 294; startup load at 2507–2510; handler at 3812–3818 |
| MEM-TOGGLE-02 | 25-01       | Memory extraction gated behind memories_enabled in StreamDone   | SATISFIED | Extraction guard at lib.rs:3946–3947                                            |
| MEM-TOGGLE-03 | 25-01       | Unit test confirms toggle persistence round-trip                 | SATISFIED | `test_memories_enabled_toggle` passes in cargo test                             |
| MEM-TOGGLE-04 | 25-02       | Toggle visible in Settings MEMORY section on all three platforms; dispatches SetMemoriesEnabled | SATISFIED | iOS SettingsView.swift:211–216; Android SettingsScreen.kt:403–418; Desktop settings.rs:518–524; all dispatch confirmed |

No orphaned requirements: all four MEM-TOGGLE IDs are claimed by plans and verified in the codebase.

### Anti-Patterns Found

None. No TODO/FIXME/PLACEHOLDER markers found in any modified file. No empty or stub implementations detected in the toggle wiring.

### Human Verification Required

#### 1. Toggle persistence across app restart

**Test:** Open the app, go to Settings > Memory, disable "Auto-extract Memories". Force-quit and relaunch the app. Open Settings > Memory again.
**Expected:** The toggle remains in the disabled (off) state.
**Why human:** Cannot verify cross-restart persistence without running the full app on a device or simulator.

#### 2. Extraction actually stops when disabled

**Test:** Disable the toggle. Have a multi-turn conversation. Check the Memories screen after the conversation completes.
**Expected:** No new memories appear after the conversation.
**Why human:** Requires a live LLM inference run and end-to-end memory pipeline execution.

#### 3. Toggle visual placement in Settings MEMORY section

**Test:** Open Settings on each platform (iOS, Android, Desktop) and navigate to the Memory section.
**Expected:** "Auto-extract Memories" toggle appears as the first row in the Memory section, above the Memories navigation link.
**Why human:** Visual layout cannot be verified from code alone.

### Gaps Summary

No gaps. All must-haves verified across both plans. The Rust core (plan 25-01) implements the full toggle plumbing — AppState field, default, action variant, startup load, persistence handler, and extraction gate — and the unit test passes. The platform UI layer (plan 25-02) correctly regenerates UniFFI bindings on both iOS and Android, updates the Android AppManager default constructor, and adds the visible toggle to all three settings screens with proper dispatch wiring to the Rust core.

---

_Verified: 2026-04-05T14:00:00Z_
_Verifier: Claude (gsd-verifier)_
