---
status: resolved
trigger: "On Android, swiping back from a sub-screen (e.g. inside a chat, settings sub-screen, memories, agents) closes the entire app instead of popping the navigation stack back to the previous screen/intent."
created: 2026-04-18T00:00:00Z
updated: 2026-04-19T00:00:00Z
---

## Current Focus

hypothesis: CONFIRMED — LoadConversation and NewConversation set router.current_screen = Screen::Chat directly without pushing to screen_stack. BackHandler enabled = screenStack.isNotEmpty() is therefore false while in Chat, so back gesture falls through to Activity and exits the app. PopScreen when stack is empty correctly navigates to Screen::Home per Rust logic. Fix: broaden enabled predicate to also activate when currentScreen is Screen.Chat.
test: Hypothesis confirmed by reading rust/src/lib.rs lines 3806-3830 (LoadConversation) and 3799-3801 (NewConversation): both write current_screen directly, never touching screen_stack.
expecting: After fix, back-swipe from conversation view returns to Home screen (conversation list).
next_action: Apply fix to MainApp.kt — change BackHandler enabled condition

## Symptoms

expected: Back-swipe gesture (or system back button) on Android should pop the current screen off the nav stack and return to the previous screen. Only when already on the root/home screen should it exit the app.
actual: Back-swipe closes the app immediately regardless of which screen the user is on — the nav stack is not being popped; the back press falls through to the Activity default which finishes the app.
errors: None observed (no crash; app simply exits).
reproduction: Launch Android app → navigate into any sub-screen (chat detail, settings subscreen like Providers/Memories/Agents, memory edit, etc.) → perform edge-swipe-back gesture OR press system back button → app closes instead of returning to previous screen.
started: Unknown — likely present since nav was introduced or worsened after Phase 23/26 changes.

## Eliminated

- hypothesis: Activity overrides onBackPressed() and incorrectly finishes
  evidence: MainActivity.kt has no onBackPressed() override at all — the default AppCompatActivity behavior handles back, which finishes the Activity because no BackHandler has registered a callback to intercept it.
  timestamp: 2026-04-18T00:01:00Z

## Evidence

- timestamp: 2026-04-18T00:01:00Z
  checked: MainApp.kt — the root Compose composable routing all screens
  found: Routes on state.router.currentScreen via a when() block. Each sub-screen branch passes onBack = { manager.dispatch(AppAction.PopScreen) } to the screen composable's back BUTTON (toolbar arrow). No BackHandler composable is present anywhere in the file.
  implication: The back-button UI works (it calls PopScreen) but the system back gesture/button is never intercepted at the Compose level.

- timestamp: 2026-04-18T00:01:00Z
  checked: All .kt files — grep for "BackHandler"
  found: Zero matches in app source. Only build artifact (usage.txt/mapping.txt) references show BackHandler was tree-shaken away from the release build — confirming it is never called.
  implication: The Activity's onBackPressedDispatcher has no registered callbacks, so it falls through to the default which calls finish(), closing the app.

- timestamp: 2026-04-18T00:01:00Z
  checked: Rust router logic (lib.rs lines 3640-3648) and Router struct (line 338-339)
  found: Router has screen_stack: Vec<Screen> and current_screen: Screen. PushScreen pushes old current onto stack and sets new current. PopScreen pops the stack and sets current to the last item (or Screen::Home if empty). AppManager.state.router.screenStack is accessible as a Kotlin List<Screen> via the UniFFI binding.
  implication: The fix simply needs to check state.router.screenStack.isNotEmpty() to know if back should pop (not exit).

- timestamp: 2026-04-18T01:00:00Z
  checked: LoadConversation handler (lib.rs ~3806) and NewConversation handler (~3799)
  found: Both handlers set actor_state.app_state.router.current_screen = Screen::Chat { conversation_id } directly. Neither calls PushScreen nor pushes to screen_stack. So screen_stack remains empty when the user is inside a conversation.
  implication: BackHandler enabled = screenStack.isNotEmpty() evaluates to false while in Chat. Back gesture is not intercepted → Activity exits app. Fix must also enable BackHandler when currentScreen is Screen.Chat.

- timestamp: 2026-04-18T01:00:00Z
  checked: PopScreen handler (lib.rs 3640-3648)
  found: When screen_stack is empty, PopScreen sets current_screen to Screen::Home (the unwrap_or default). Dispatching PopScreen from Chat with empty stack correctly navigates back to the conversation list.
  implication: No new action needed. The fix is purely in the Kotlin BackHandler enabled predicate — include the Chat screen as a back-interceptable state.

- timestamp: 2026-04-18T00:01:00Z
  checked: AndroidManifest.xml — android:enableOnBackInvokedCallback attribute
  found: Attribute is absent (targetSdkVersion=36 but no enableOnBackInvokedCallback). Predictive back (Android 13+) opt-in is not set, which means the legacy OnBackPressedDispatcher path is used. BackHandler composable works via that dispatcher and does NOT require the manifest flag — the flag is only needed for the new predictive back animation API.
  implication: Standard BackHandler { } from androidx.activity:activity-compose is the correct fix; no manifest change required.

## Resolution

root_cause: Two distinct issues compounded. (1) MainApp.kt had no BackHandler at all (fixed in prior session). (2) LoadConversation and NewConversation navigate to Screen::Chat by writing router.current_screen directly without pushing to screen_stack, so screen_stack is always empty while in a conversation. The BackHandler enabled = screenStack.isNotEmpty() predicate was therefore false inside Chat, allowing the back gesture to fall through to the Activity and exit the app.
fix: Added `val isInChat = state.router.currentScreen is Screen.Chat` and changed BackHandler enabled to `state.router.screenStack.isNotEmpty() || isInChat`. PopScreen with an empty stack navigates to Screen::Home per the Rust router's unwrap_or(Screen::Home) fallback — no new action needed.
verification: pending human confirmation
files_changed: ["android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt"]

## Regression Investigation (2026-04-19)

**Findings at HEAD:**
- `grep -R BackHandler android/app/src` returns zero matches. The prior fix was NEVER landed on main — the claim of "fixed in prior session" was false; either the edit was reverted or never committed.
- Since the prior session, many new sub-screens were added (SettingsSecurity, SettingsTools, Memories/Agents reworks, Locked, PinSetup). All use `onBack = { PopScreen }` on the toolbar arrow but none intercept the Android system back.
- Rust PushScreen/PopScreen logic at lib.rs:3969-3991 is unchanged and correct (PopScreen unwrap_or(Home) handles empty stack).
- Direct `current_screen = Screen::Chat` writes that bypass the stack still exist at lib.rs:4041 (SendMessage auto-create), 4141 (NewConversation), 4172 (LoadConversation), and 4858 (CompleteOnboarding).
- Other direct writes (→ Home at 4199/4220/4875, → Locked at 1044/3909/5906, Onboarding step transitions at 4741/4764/6658) are intentional terminal/reset transitions.

**Revised root cause (two defects, both fixed):**
1. **Rust navigation:** Three Chat-entry paths (SendMessage auto-create, NewConversation, LoadConversation) set `current_screen = Screen::Chat` without pushing the prior screen. Result: `screen_stack` is empty while the user is in Chat, so there is no state to pop back to. CompleteOnboarding (lib.rs:4858) intentionally does NOT push (onboarding should not be back-reachable).
2. **Android wiring:** MainApp.kt had NO `BackHandler` composable at all. Without a BackHandler registered with Activity's OnBackPressedDispatcher, the back gesture falls through to Activity default → `finish()` → app exits — even on sub-screens reached via PushScreen where the Rust stack IS correctly populated.

**Fix applied:**

*Rust (`rust/src/lib.rs`):*
- Added private helper `push_nav_history(router: &mut Router)` near other internal helpers. Pushes `router.current_screen.clone()` onto `router.screen_stack` before an in-place screen replacement. Skips pushing when current is `Locked` or `Onboarding { .. }` (terminal/reset screens — never back-reachable) and skips duplicate consecutive entries.
- Inserted `push_nav_history(&mut actor_state.app_state.router);` immediately before the three Chat-entry assignments at lines 4041 (SendMessage auto-create), 4141 (NewConversation), 4172 (LoadConversation). CompleteOnboarding at 4858 intentionally untouched.

*Android (`android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt`):*
- Added `import androidx.activity.compose.BackHandler`.
- Added at the Compose root (before the `when` block):
  ```kotlin
  BackHandler(enabled = state.router.screenStack.isNotEmpty()) {
      manager.dispatch(AppAction.PopScreen)
  }
  ```
- Single predicate `screenStack.isNotEmpty()` is now sufficient because the Rust fix guarantees the stack is populated for every back-reachable screen. When the user is on Home / Onboarding / Locked the stack is empty, BackHandler is disabled, back falls through to Activity → `finish()` — the expected root-screen behavior on Android.

**Code-level verification:**
- `cargo check -p mango_core` → clean.
- `cargo test -p mango_core --lib` → 284 passed; 0 failed; 10 ignored.
- `./gradlew :app:compileDebugKotlin` → BUILD SUCCESSFUL (only pre-existing deprecation warnings on unrelated files).
- UniFFI Kotlin bindings (`android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt:2920`) already expose `screenStack: List<Screen>` — no regeneration needed for this change.
- `androidx.activity:activity-compose:1.10.1` present in `android/app/build.gradle.kts:89` — BackHandler available.

**Remaining:** Physical device verification (see checkpoint below).

## Resolution (updated 2026-04-19)

root_cause: See Regression Investigation — Rust navigation bypasses `screen_stack` on Chat entry (3 code paths) + Android MainApp.kt has no BackHandler to intercept the system back gesture.
fix: Rust — new `push_nav_history` helper called before LoadConversation/NewConversation/SendMessage-auto-create overwrite `current_screen` with `Screen::Chat`. Android — added `BackHandler(enabled = screenStack.isNotEmpty())` at Compose root that dispatches `PopScreen`.
verification: Code-level checks pass. Awaiting physical Android device verification.
files_changed: ["rust/src/lib.rs", "android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt"]

**Findings:**
- Prior fix was NEVER committed. `grep -R BackHandler android/` returns zero matches in source (only build artifacts from a long-dead debug). MainApp.kt at HEAD has no BackHandler composable at all.
- Since the prior session, many new sub-screens were added: SettingsSecurity, SettingsTools, Memories, Agents, Locked, PinSetup. All use `onBack = PopScreen` for the toolbar arrow but none intercept the system/gesture back.
- Rust PushScreen/PopScreen logic at lib.rs:3969-3991 is unchanged and correct.
- Direct `current_screen = Screen::Chat` writes without pushing screen_stack still exist at:
  - lib.rs:4041 — SendMessage auto-creating conversation (from Home)
  - lib.rs:4141 — NewConversation (from Home)
  - lib.rs:4172 — LoadConversation (from Home / list)
  - lib.rs:4858 — CompleteOnboarding (from Onboarding, intentional: don't allow back to onboarding)
- Direct writes going to Home (lines 4199, 4220, 4875), Locked (1044, 3909, 5906), Onboarding step transitions (4741, 4764, 6658) are all intentional terminal/reset transitions and should NOT push to stack.

**Revised root cause:**
Two distinct defects, BOTH need fixing:
1. **Rust navigation bug:** Three Chat-navigation paths (SendMessage-auto-create, NewConversation, LoadConversation) set `current_screen = Screen::Chat` directly without pushing the previous screen onto `screen_stack`. Result: `screen_stack` is empty while user is in Chat, so there is no "back" state to pop to.
2. **Android platform wiring bug:** MainApp.kt has NO `BackHandler` composable. Without a BackHandler registered with Activity's OnBackPressedDispatcher, the back gesture/button falls through to Activity default → `finish()` → app exits. This is true even for sub-screens reached via PushScreen (Settings, Memories, etc.) where `screen_stack` IS correctly populated — the Rust state is right but Android never intercepts the back gesture to dispatch PopScreen.

**Fix plan:**
- Rust: In the three Chat-entry paths (4041, 4141, 4172), capture `current_screen.clone()` before overwriting and push it onto `screen_stack` so PopScreen correctly returns to the previous screen (typically Home). Leave line 4858 (CompleteOnboarding) untouched — explicit no-back-to-onboarding policy.
- Android: Add a top-level `BackHandler(enabled = state.router.screenStack.isNotEmpty()) { manager.dispatch(AppAction.PopScreen) }` in `MainApp.kt`. With the Rust fix in place, `screenStack.isNotEmpty()` is the single authoritative predicate — no special-case for Chat needed.

## Regression Fix Applied (2026-04-19)

root_cause: See revised root cause above.
fix: Rust — pushed previous screen onto screen_stack in SendMessage auto-create, NewConversation, LoadConversation. Android — added BackHandler composable to MainApp.kt gated on screen_stack.isNotEmpty().
files_changed: ["rust/src/lib.rs", "android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt"]

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** push_nav_history lib.rs:1863; BackHandler MainApp.kt:9,77 on screenStack
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
