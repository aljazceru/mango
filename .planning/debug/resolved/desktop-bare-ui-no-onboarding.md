---
status: resolved
trigger: "On desktop, the app opens to an empty chat list with a 'New Conversation' button that does nothing. No settings, onboarding, or lock screen UI."
created: 2026-04-21T00:00:00Z
updated: 2026-04-21T00:00:00Z
resolved_commit: c728f91
---

## Current Focus

hypothesis: CONFIRMED — the desktop iced `Message::CoreUpdated` handler was silently dropped the `*state = latest;` assignment in commit a7c204b (Apr 19, IMG-07 thumbnail rendering). After that commit, the UI's `App::Loaded { state, .. }` is set once at startup from `manager.state()` (which returns the AppState::default() before the actor thread has finished its initial emit) and is never updated thereafter. `view()` branches on `state.router.current_screen`, which is frozen at Screen::Home, so the lock screen, onboarding, settings etc. are never rendered regardless of what the actor does.
test: Read desktop/iced/src/main.rs around the CoreUpdated handler. `git log -S "*state = latest" -- desktop/iced/src/main.rs` → identified commit a7c204b as the regression point. `git show a7c204b -- desktop/iced/src/main.rs` confirms the line `*state = latest;` was removed when IMG-07 thumbnail logic was added.
expecting: Fix is to restore the assignment unconditionally (the downstream logic was already idempotent), not merely guarded by `rev > state.rev` (which would still never fire on initial emit since both sides start at rev=0).
next_action: Apply fix, compile-check, hand to user for desktop smoke test.

## Symptoms

expected: Onboarding flow on first launch, lock screen if PIN configured, or home with functional sidebar + settings.
actual: Empty chat list + single non-functional "New Conversation" button. No settings, no onboarding, no lock.
errors: No panic — the `NewConversation` guard from the previous session silently drops the action when db is None.
reproduction: cargo run -p mango-desktop -> app opens, click button, nothing happens.
started: Regressed in commit a7c204b (Apr 19, 2026) — desktop-only, platform had not been smoke-tested since.

## Eliminated

- hypothesis: Desktop iced view() does not branch on Screen::Locked / Screen::Onboarding / Screen::PinSetup.
  evidence: main.rs:1769-1858 explicitly branches on every screen before falling through to sidebar + chat area. Lock screen, PIN setup, onboarding, settings, memories, agents, documents, directory sources are all wired up. Routing is correct.
  timestamp: 2026-04-21

- hypothesis: Desktop is missing an onboarding flow entirely.
  evidence: desktop/iced/src/views/onboarding.rs exists (647 lines). rust/src/lib.rs:3990 sets Screen::Onboarding { step: Welcome } on first launch when !has_completed_onboarding. Onboarding is fully wired — it just wasn't being rendered because `state` was frozen.
  timestamp: 2026-04-21

- hypothesis: The startup order is racing — manager.state() returns before the actor emits, so UI sees a stale default.
  evidence: PARTIALLY TRUE as a contributing factor. manager.state() at main.rs:512 runs synchronously after AppManager::new returns, while the actor is still initializing on a background thread. So yes, the initial `state` field captured in App::Loaded is AppState::default() (Screen::Home, rev=0, conversations=[]). BUT this is fine by design — the CoreUpdated subscription is supposed to sync the UI state once the actor emits. The real bug is that the CoreUpdated handler never assigns `latest` back to `state`. Ruled out as a root cause in isolation; confirmed as the trigger the real bug depends on.
  timestamp: 2026-04-21

## Evidence

- timestamp: 2026-04-21
  checked: rust/src/lib.rs:4216-4306 — startup screen selection for Case D (returning user with auth_params).
  found: `if has_auth && !bypass_succeeded { initial_state.router.current_screen = Screen::Locked; }`. Core correctly selects Screen::Locked for this machine (auth_params present, cold_launch_bypass=0).
  implication: The core is doing the right thing. Problem is on the UI side.

- timestamp: 2026-04-21
  checked: Local state at ~/.local/share/mango/ — auth_params row exists, cold_launch_bypass=0, mango.db is SQLCipher-encrypted (`file mango.db` → `data`).
  found: This machine is unambiguously Case D (returning user, locked).
  implication: The app MUST route to Screen::Locked on startup. Therefore the lock screen MUST be the first thing rendered. Any other outcome is a UI bug.

- timestamp: 2026-04-21
  checked: desktop/iced/src/main.rs:1716-1894 — the top-level view() function.
  found: The screen routing is correct. Branches on Screen::Locked / PinSetup / Onboarding / Settings / SettingsProviders / SettingsDefaults / Documents / DirectorySources / Memories / Agents. Fallback is `row![sidebar, chat_area]`. `sidebar_view` in views/home.rs:41-99 with conversations.is_empty() renders: "New Conversation" button + "No conversations yet / Start a new conversation to chat with a private AI." — EXACTLY matching the reported UI.
  implication: The screen rendered is the fallback (Screen::Home with empty conversations), which means `state.router.current_screen == Screen::Home` at render time. The question is: why Home when the actor set Locked?

- timestamp: 2026-04-21
  checked: desktop/iced/src/main.rs Message::CoreUpdated handler (pre-fix at line 702-787).
  found: The handler reads `let latest = manager.state();`, does a bunch of delta work inside `if latest.rev > state.rev { ... }`, but NEVER assigns `*state = latest`. So the `state` field in App::Loaded is frozen at whatever it was at startup.
  implication: Smoking gun. view() consumes `state`, not `latest`. The state mirror never updates.

- timestamp: 2026-04-21
  checked: `git log -S "*state = latest" -- desktop/iced/src/main.rs` + `git show a7c204b`.
  found: Commit a7c204b "feat(260419-ece): desktop iced thumbnail rendering for encrypted images" removed the line `*state = latest;` in the same diff that added IMG-07 thumbnail Task spawning. The removal looks accidental — the commit's stated purpose is thumbnail rendering, and there is no comment explaining why UI state sync should be dropped.
  implication: Regression, not intentional. Fix: restore the assignment.

- timestamp: 2026-04-21
  checked: rust/src/lib.rs — when is rev bumped relative to the initial `emit()` at line 4360?
  found: rev=0 in AppState::default() (line 306). The initial `emit()` runs without any `rev += 1`. load_post_unlock (called in Cases A/B/C) also does not bump rev. So the initial emit ALWAYS has rev=0. UI's App::Loaded.state also has rev=0. Therefore `latest.rev > state.rev` is ALWAYS false on the first emit, which means even if `*state = latest` were restored inside that guard, the initial lock/onboarding/PinSetup screen would still never render — only later, once the user performed an action that bumped rev, would the screen update.
  implication: The minimal fix must both (a) restore the assignment AND (b) run unconditionally (or relax the guard). I chose to run unconditionally — all downstream logic in that handler is already idempotent (parsed_messages uses `!contains_key`, streaming uses prev_streaming_len, settings_default_instructions uses its own init flag, toast dispatches ClearToast, thumbnail tasks use `!image_cache.contains_key`).

- timestamp: 2026-04-21
  checked: `cargo check -p mango-desktop` after the fix.
  found: Compiles cleanly. Only pre-existing warning (unrelated `tee_type_to_str` dead_code).
  implication: Fix is syntactically and type-wise sound. Smoke test on real desktop is the remaining verification.

## Resolution

root_cause: |
  Commit a7c204b (Apr 19, 2026) "feat(260419-ece): desktop iced thumbnail rendering for encrypted images" accidentally removed the `*state = latest;` assignment at the end of the `Message::CoreUpdated` handler in desktop/iced/src/main.rs. From that commit onward, the desktop UI's `App::Loaded { state, .. }` field was set exactly once — during App::new, via `manager.state()` — and then never updated. Because `manager.state()` at that point returns AppState::default() (the actor thread is still initializing on a background thread and has not yet emitted its first snapshot), the UI was permanently frozen at:
    - `router.current_screen = Screen::Home`
    - `conversations = []`
    - `auth_initialized = false`, etc.
  The top-level `view()` function correctly branches on `state.router.current_screen`, but because `state` is frozen at Screen::Home, it falls through to the sidebar + welcome chat area. With an empty conversations list, `sidebar_view` renders a "New Conversation" button and the text "No conversations yet / Start a new conversation to chat with a private AI." The chat area shows "Select or create a conversation to begin." This exactly matches the reported bare UI.

  Clicking "New Conversation" dispatches `AppAction::NewConversation`, which the actor receives and — because `actor_state.db` is None in Case D (locked) — silently drops via the guard added in the previous debug session. No UI feedback, no state change.

  Secondary observation: the pre-regression code guarded the assignment with `if latest.rev > state.rev`, which is also wrong: the actor's initial emit happens at rev=0, matching the UI's rev=0, so `>` is false. Had the assignment been restored inside that guard, the bug would still manifest on first launch (just resolve after the user's first action). The correct fix is to run the handler body unconditionally — the downstream logic is already idempotent.

fix: |
  Modified `Message::CoreUpdated` handler in desktop/iced/src/main.rs (~line 702):
    1. Removed the `if latest.rev > state.rev { ... }` guard, running all delta logic unconditionally (every block is already idempotent).
    2. Restored `*state = latest;` before the Task::batch/Task::none return. A comment explains why (ties back to commit a7c204b).
    3. Changed `for msg in &state.messages` (thumbnail loop) to `for msg in &latest.messages` so thumbnails for messages in the most recent snapshot are loaded (was subtly wrong against the stale `state` too).

verification: |
  - `cargo check -p mango-desktop` passes with no new warnings (only pre-existing `tee_type_to_str` dead_code warning, unrelated).
  - Core-side logic for initial screen selection was verified read-only (rust/src/lib.rs:4216-4306) — Case D correctly routes to Screen::Locked, Case B/C routes via load_post_unlock to Screen::Onboarding { Welcome } on first launch.
  - UI-side verification requires a real display server — cannot run `cargo run -p mango-desktop` in this headless sandbox. User must run the binary on their desktop to confirm:
      (a) cold launch on this machine shows the lock screen (Case D: auth_params present);
      (b) entering the PIN unlocks and shows home with conversations;
      (c) deleting `~/.local/share/mango/` and restarting shows the onboarding welcome (Case B: first launch);
      (d) the sidebar "New Conversation" button produces a new chat once unlocked.

files_changed:
  - desktop/iced/src/main.rs (CoreUpdated handler: restore `*state = latest;`, drop stale-rev guard, fix thumbnail loop to iterate `latest.messages`)

## Summary for orchestrator

The UI routing on desktop is correct. Onboarding, lock screen, PIN setup, settings are all wired up and branch-selected in view(). The bug is a single missing line (`*state = latest;`) in the CoreUpdated handler, accidentally removed on Apr 19 in the IMG-07 thumbnail commit, that froze the desktop UI's state mirror at AppState::default(). The fix is ~5 lines net in one file.

No broader onboarding/settings work is needed as part of this debug session — the existing desktop UI is feature-complete for the bare-ui-no-onboarding symptom. Once this fix is smoke-tested, the desktop "a lot of love" work can be scoped as a separate planned phase (polish, not a blocker).
