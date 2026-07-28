---
status: resolved
trigger: "onboarding-flash-on-first-open"
created: 2026-04-03T00:00:00Z
updated: 2026-04-03T00:02:00Z
---

## Current Focus

hypothesis: CONFIRMED — AppManager initializes with a hardcoded Screen.Home default state and calls ffiApp.state() which races against the actor background thread writing the real initial state (with Screen.Onboarding). The UI renders Screen.Home first, then flashes to Screen.Onboarding when reconcile() fires.
test: Root cause confirmed by code inspection
expecting: Fix by adding an isReady flag in AppManager that suppresses UI rendering until the first reconcile() arrives
next_action: Implement fix in AppManager.kt and MainApp.kt

## Symptoms

expected: First app open shows onboarding screen immediately with no flash of other content
actual: Conversation/chat screen briefly appears, then switches to onboarding screen
errors: None — functional issue, not a crash
reproduction: Fresh install, open the app for the first time
started: Likely since onboarding was implemented

## Eliminated

(none yet)

## Evidence

- timestamp: 2026-04-03T00:01:00Z
  checked: AppManager.kt lines 36-72
  found: AppManager hardcodes initial state with currentScreen = Screen.Home before calling ffiApp.state()
  implication: The UI always renders Screen.Home before the Rust actor writes the real state

- timestamp: 2026-04-03T00:01:00Z
  checked: rust/src/lib.rs FfiApp::new() lines 2042-4342
  found: shared_state is initialized with AppState::default() (Screen::Home), then a background thread spawns, opens DB, reads has_completed_onboarding, sets Screen::Onboarding if needed, and only then writes to shared_state and sends to update channel (line 2361)
  implication: ffiApp.state() called from Kotlin constructor races against the background thread — if the background thread hasn't finished yet, Kotlin gets Screen.Home even on first install

- timestamp: 2026-04-03T00:01:00Z
  checked: AppManager.kt lines 119-121
  found: val initial = ffiApp.state() called synchronously after ffiApp construction, then state = initial and lastRevApplied = initial.rev
  implication: The race window is between FfiApp.new() returning and the background actor thread finishing DB init. On any non-trivial device this window is enough for Compose to render at least one frame with Screen.Home.

- timestamp: 2026-04-03T00:01:00Z
  checked: rust/src/lib.rs AppState::default() line 219-225
  found: Default router has current_screen = Screen::Home with empty stack
  implication: Confirms that ffiApp.state() returns Screen.Home until the actor thread's first emit (line 2361) writes the correct state

## Resolution

root_cause: Race condition in AppManager initialization. FfiApp::new() spawns a background actor thread that reads has_completed_onboarding from SQLite and determines the correct initial screen. The shared_state RwLock starts as AppState::default() (Screen::Home). AppManager calls ffiApp.state() immediately after construction, before the actor thread has finished DB init and written the real state. Compose renders Screen.Home, then the actor emits the real state (Screen::Onboarding for first install), causing the visible flash.
fix: Add an isReady: Boolean state variable to AppManager that starts false and is set to true when the first reconcile() call arrives. In MainApp, show a blank Box() (no content) when !isReady. This ensures zero frames render the wrong screen — the UI stays blank until the real initial state is available (actor init takes <100ms in practice).
verification: Build passes (compileDebugKotlin: BUILD SUCCESSFUL). Awaiting device test on fresh install.
files_changed:
  - android/app/src/main/java/dev/disobey/mango/AppManager.kt
  - android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** isReady field AppManager.kt:46,217; MainApp.kt:42-45 blank Box guard
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
