---
status: resolved
trigger: "desktop-new-chat-db-panic: panic 'db unlocked' at rust/src/lib.rs:4503:61 when opening a new chat in mango-desktop"
created: 2026-04-21T00:00:00Z
updated: 2026-04-21T00:00:00Z
resolved_commit: 797dd9f
---

## Current Focus

hypothesis: CONFIRMED — the `AppAction::NewConversation` handler unconditionally unwraps `actor_state.db` with `.expect("db unlocked")`, which panics whenever the action is dispatched while `db` is None (Case D: returning user, not yet unlocked).
test: Wrote a headless integration test (`tests/desktop_locked_new_conv.rs`) that creates a temp data_dir with valid bootstrap auth_params, constructs `FfiApp` without unlocking, dispatches `NewConversation`, and probes actor liveness via a `Noop` follow-up. Pre-fix: actor thread panics with exactly "db unlocked" at lib.rs:4503 and `state.rev` stays frozen → assertion fails. Post-fix: actor stays alive, rev increments, screen stays Locked, no conversation created.
expecting: Test failed pre-fix, passes post-fix.
next_action: Await user verification on real desktop hardware. Self-verification (headless core test) already passes.

## Symptoms

expected: Opening a new chat in desktop app creates a conversation and displays it.
actual: Process panics at rust/src/lib.rs:4503:61 with "db unlocked" inside AppAction::NewConversation handler — actor_state.db is None.
errors: |
  thread '<unnamed>' (1910134) panicked at rust/src/lib.rs:4503:61:
  db unlocked
reproduction: |
  1. cargo run -p mango-desktop
  2. Click/trigger "new chat" in the running app
  3. Process panics immediately
started: Desktop largely untested — unknown when it regressed. Latent hazard since Phase 28 (lock screen).

## Eliminated

- hypothesis: The cold-launch bypass commit 94036e4 regressed this by leaving db=None in a non-Locked state.
  evidence: Read the diff in full. Case D bypass path explicitly sets `(None, ..., false)` + `initial_state.router.current_screen = Screen::Locked` when open_encrypted fails, so bypass does not create a mixed state. The panic predates that commit — it's a latent hazard in the NewConversation handler itself.
  timestamp: 2026-04-21

- hypothesis: A keyboard shortcut or subscription auto-fires NewConversation before unlock.
  evidence: Grep of desktop/iced/src shows exactly one dispatch site (views/home.rs:60 button in sidebar). No shortcuts, no subscriptions dispatch NewConversation. The sidebar is not rendered while `Screen::Locked` is active.
  timestamp: 2026-04-21

## Evidence

- timestamp: 2026-04-21
  checked: rust/src/lib.rs:4498-4549 — the AppAction::NewConversation handler.
  found: Four `actor_state.db.as_ref().expect("db unlocked")` calls (lines 4503, 4511, 4535, 4558-ish for related handler). The first one panics before any guard runs.
  implication: Any dispatch of NewConversation while db is None crashes the actor thread. This is a handler-level hazard, independent of how the UI got there.

- timestamp: 2026-04-21
  checked: Repo-wide grep `grep -c 'expect("db unlocked")' rust/src/lib.rs` → 131 occurrences.
  found: The "db unlocked" expect pattern is spread across ~131 sites in action handlers. All share the same risk shape.
  implication: Fixing NewConversation alone is the minimal fix for the reported symptom. A broader sweep (convert these to graceful ignore / Result propagation) is worth a follow-up but is out of scope for this debug session.

- timestamp: 2026-04-21
  checked: Local state at $XDG_DATA_HOME/mango — `mango.db` is SQLCipher-encrypted ("file ... data"), `mango_auth.db` has one auth_params row with cold_launch_bypass=0.
  found: This machine is a returning user in Case D (locked). FfiApp::new puts db=None and screen=Locked, which matches the reported panic's preconditions.
  implication: The bug reproduces on this very machine in any scenario where NewConversation reaches the actor while db is None (e.g. stray dispatch during unlock transition, future UI code path added without a `db.is_some()` gate, or an integration path we haven't traced yet).

- timestamp: 2026-04-21
  checked: Headless reproduction via rust/src/tests/desktop_locked_new_conv.rs.
  found: Pre-fix output: `thread '<unnamed>' (1945219) panicked at rust/src/lib.rs:4503:61: db unlocked`. The liveness assertion (`state_probe.rev > rev_before_new_conv`) fails with "rev stuck at 0 (pre_rev was 0)". Post-fix: both tests pass in 4s.
  implication: Root cause confirmed. Test provides a repeatable regression guard for this handler and a template for hardening other handlers.

- timestamp: 2026-04-21
  checked: `cargo test -p mango_core --lib tests::chat` after fix.
  found: 29 passed, 0 failed — no regression in existing conversation/chat flow (test_new_conversation_creates_and_navigates still passes because it uses in-memory DB, Case A, where db is always Some).
  implication: Fix is minimal and does not break the unlocked path.

## Resolution

root_cause: |
  `AppAction::NewConversation` in rust/src/lib.rs:4498 unwraps `actor_state.db` with `.expect("db unlocked")` on every invocation. This precondition is never checked by the dispatch layer: the UI gates the "new chat" button on `Screen::Locked` (so the button isn't visible), but `FfiApp::dispatch` is an async channel send with no screen gating. Any code path that enqueues `NewConversation` while the user is locked (unlock transitions, stray message during teardown, or any future UI path added without a lock check) panics the actor thread. Once the actor thread dies, the whole process goes down and the UI freezes. This is a latent hazard introduced when the lock screen was added (Phase 28) — NOT a regression of quick 94036e4.

fix: |
  Add an explicit `if actor_state.db.is_none() { log::warn!(...); continue; }` guard at the top of the `AppAction::NewConversation` match arm. This keeps the actor alive and drops the stray action silently with a warning log. The UI re-renders on the next state emit, and the user sees the lock screen as expected.

verification: |
  - New headless test `tests/desktop_locked_new_conv.rs`:
    * `returning_user_starts_on_lock_screen_with_no_db` — pins the Case D preconditions (auth_initialized + encryption_enabled + Screen::Locked).
    * `new_conversation_while_locked_does_not_panic` — dispatches NewConversation, checks screen still Locked + 0 conversations, then probes actor liveness via Noop (rev must increase).
  - Verified pre-fix: the regression test fails with `actor thread appears dead after NewConversation: rev stuck at 0` AND the actor panic message matches the reported bug verbatim ("db unlocked" at rust/src/lib.rs:4503:61).
  - Verified post-fix: both tests pass in ~4s.
  - Regression: `cargo test -p mango_core --lib tests::chat` — 29/29 pass, including test_new_conversation_creates_and_navigates which exercises the unlocked path.

files_changed:
  - rust/src/lib.rs:4498-4519 (guard NewConversation against db=None)
  - rust/src/tests/desktop_locked_new_conv.rs (new regression test, 2 cases)
  - rust/src/tests/mod.rs (register new test module)
  - rust/src/tests/attestation_integration.rs (tangential: unrelated pre-existing compile error — added wildcard arm for CoreMsg::ExportConversationMarkdown that was blocking test compilation)

## Testing strategy (desktop going forward)

The user asked how to self-test desktop in this headless environment. Findings:

1. **`cargo run -p mango-desktop`** — requires a display server. This environment has no Wayland/X11 and no Xvfb installed. Useful on developer machines only.

2. **`cargo test -p mango_core --lib`** — no display needed, fast (~4s for relevant suite). This is the recommended first-line approach for DB-lifecycle bugs, lock-screen flows, and any logic that lives in the actor. See rust/src/tests/chat.rs and the new rust/src/tests/desktop_locked_new_conv.rs for patterns.

3. **Driving `FfiApp` directly from a Rust integration test** (RECOMMENDED for regressions like this one) — instantiate `FfiApp::new` with `NullKeychainProvider` + temp data_dir, seed bootstrap_db with auth_params to force Case D, then dispatch `AppAction`s and assert against `app.state()`. This is exactly the shape of the new regression test. Strengths:
   - No GUI framework required (iced is not pulled in).
   - Deterministic — no wall-clock races beyond the 150ms actor-dispatch sleep.
   - Catches actor-thread panics via the rev-bump liveness probe (since panics in spawned threads are silent, you can't rely on `#[should_panic]`).

4. **GUI smoke tests (future work)** — if end-to-end UI flow verification becomes necessary, install Xvfb + add a `tests/smoke.rs` target in `desktop/iced/` that spawns the binary under `xvfb-run` and uses `xdotool` or (better) a headless wgpu backend. Not done here — out of scope for this debug session and the RMP architecture already favors actor-level tests over UI-level tests.

**Recommended discipline going forward:** every new `AppAction` handler that touches `actor_state.db` should have:
  (a) either an explicit `if actor_state.db.is_none() { ...; continue; }` guard, or
  (b) a comment explaining why the action is only reachable in the unlocked path.

The 131 other `.expect("db unlocked")` sites are a tech-debt sweep worth doing in a follow-up but are not required to close this bug.
