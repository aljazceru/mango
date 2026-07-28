---
slug: android-no-fork-chat-option
status: resolved
trigger: "on android i dont see \"fork chat\" option"
created: 2026-04-24
updated: 2026-04-24
---

# Debug: Fork chat option missing on Android

## Symptoms

- **Expected:** Fork chat option (added in quick task 260423-93w on 2026-04-23) should be visible/accessible on Android, parity with iOS.
- **Actual:** Option does not appear on the Android UI.
- **Error messages:** None reported.
- **Timeline:** Feature was implemented yesterday (2026-04-23) per state.json `last_activity`. User has not seen it on Android since.
- **Reproduction:** Open the Android app, navigate to a chat, look for the "fork chat" affordance (presumably on a message long-press menu or chat-level menu, mirroring iOS).

## Current Focus

- hypothesis: Stale APK on user device — Android Fork chat code IS present in source but a rebuild + reinstall is required to surface it.
- test: Verified the menu item is wired in current source.
- expecting: Rebuild + reinstall via `just android-full` shows the menu item.
- next_action: User rebuilds and reinstalls Android APK.

## Evidence

- timestamp: 2026-04-24
  - Quick task 260423-93w SUMMARY.md (lines 82–113) explicitly states "Per explicit plan scope, only Rust core + Desktop ships here" and labels iOS/Android as deferred follow-ups with sample 10-line snippets. **The summary is stale** — it was authored before the Android implementation was added.
  - STATE.md row 171 was correctly updated by commit `bae67a4` (08:03:36 on 2026-04-23) to read "Rust core + Desktop + Android; iOS deferred", reflecting the actual ship state.
- timestamp: 2026-04-24
  - Commit `4b0c3e6` (2026-04-23 08:03:10) "feat(quick/260423-93w): add Android Fork chat menu entry" added the DropdownMenuItem and regenerated Kotlin UniFFI bindings. Commit message documents that the prior `just bindings-kotlin` had silently produced no output due to release-profile `strip=true` stripping the ELF symbol table; bindings were regenerated against `target/debug/libmango_core.so`.
- timestamp: 2026-04-24
  - `android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt` lines 561–584 contain a `DropdownMenuItem(text = { Text("Fork chat", …) }, enabled = canFork, onClick = { … AppAction.ForkConversation(id = cid) … })` placed inside the conversation overflow `DropdownMenu` (toggled by the `MoreVert` `IconButton` at line 511). `canFork = state.currentConversationId != null && state.messages.isNotEmpty()`.
- timestamp: 2026-04-24
  - `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` contains `data class ForkConversation` (line 3427), the deserializer arm `13 -> AppAction.ForkConversation(...)` (line 3976), and dispatch handlers at lines 4239 and 4721. Variant tag `13` matches the Rust enum order in `rust/src/lib.rs` line 474 (PushScreen=0 ... ForkConversation=12, which serializes as 1-based tag 13 on the wire — UniFFI default).
- timestamp: 2026-04-24
  - `cargo check -p mango_core` is clean against current source. No build error blocks the binding.
- timestamp: 2026-04-24
  - jniLibs timestamp: `android/app/src/main/jniLibs/arm64-v8a/libmango_core.so` was last built 2026-04-23 08:41 — AFTER commit `4b0c3e6` (08:03). So the bundled native library is consistent with the Kotlin bindings file.
- timestamp: 2026-04-24
  - Subsequent commits 5d8144a (`strip="debuginfo"`), 845ccda (`panic=abort`), b712d0b (arm64-only release), 7d0c254 (CI recipe), and 873a5cd (regenerate Swift bindings) are all build/profile cleanup. The Kotlin bindings file in source already includes ForkConversation, so a `just bindings-kotlin` run against the new profile would not change the menu wiring; it might produce a slightly different generated file (different metadata hashes) but the ABI is unchanged.
- timestamp: 2026-04-24
  - The Fork chat menu item lives **only** in the active chat's MoreVert (⋮) overflow menu, not in the conversation list (sidebar) long-press menu. Conversation list (`ConversationListScreen.kt`) only exposes Rename + Delete affordances — no Fork. When `messages.isEmpty()` (empty new chat) or `currentConversationId == null` the Fork item is rendered with `enabled = false` and a muted color rather than hidden.

## Eliminated

- **Feature-parity gap** — INITIAL HYPOTHESIS WRONG. The Android UI was actually wired in commit `4b0c3e6`; the SUMMARY.md was stale and misled the initial diagnosis.
- **Stale UniFFI bindings** — Kotlin bindings file contains `ForkConversation` at the correct enum tag (13), matching the Rust core enum order. No mismatch.
- **Build-profile masking the binding regen** — was a real risk earlier (commit 5d8144a explicitly fixed `strip=true` silently dropping UniFFI metadata) but commit `4b0c3e6` worked around it by regenerating against the debug `.so`, and the committed `mango_core.kt` already contains the variant.
- **Rust-core gating flag** — `AppAction::ForkConversation` is a plain enum variant with no feature flag or platform guard.
- **Wrong Compose package** — file is at the expected `dev.disobey.mango.ui` package (matches the convention noted in STATE.md line 137 for Phase 32 Plan 06).

## Resolution

### Root cause

The Fork chat affordance is present in the Android source code (added in commit `4b0c3e6`, 2026-04-23 08:03) and is wired correctly through to the Rust core via the regenerated UniFFI bindings. The user is most likely running an APK that was built/installed **before** that commit landed, or has not rebuilt the Android app since the feature was added. A secondary contributing factor is **discoverability**: the Fork option lives only inside the per-chat overflow (⋮) menu in the chat top bar — there is no Fork affordance on the conversation list (where Rename/Delete live), which is where a user might intuitively look first.

This is **not a code bug** — no source change is required. The fix is operational: rebuild and reinstall the Android app.

### Fix

No code change applied. Recommended user action:

1. Rebuild + install the Android app:
   ```
   just android-full
   adb install -r android/app/build/outputs/apk/debug/app-debug.apk
   ```
   (or `just android-release` + install the release APK if that's the configuration the user is testing.)
2. Open any conversation that has at least one message.
3. Tap the **⋮** (MoreVert) icon in the top bar (next to the model selector).
4. The dropdown will show: RAG · Instructions · Tools · **Fork chat**.

If the user is on an empty new chat with no messages, "Fork chat" is intentionally rendered greyed-out (`enabled = canFork`) — send or load at least one message to enable it.

### Verification

- Source verification (no rebuild required to confirm presence): `grep -n 'Fork chat' android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt` returns line 569.
- Binding verification: `grep -n 'ForkConversation' android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` returns the `data class`, deserializer arm, and two dispatch handlers.
- Native lib timestamp: `stat android/app/src/main/jniLibs/arm64-v8a/libmango_core.so` = 2026-04-23 08:41, after the feature commit.
- After rebuild + reinstall, the user should see "Fork chat" in the ⋮ menu of any non-empty conversation.

### Optional follow-ups (not required, suggested for UX)

- Consider adding a "Fork" entry to the conversation list long-press menu in `ConversationListScreen.kt` next to Rename/Delete, since that is a common discoverability path. This would parallel the Desktop overflow placement and reduce "where is it?" reports.
- Update `.planning/quick/260423-93w-fork-chat/260423-93w-SUMMARY.md` lines 82–113 to remove the "Android deferred" wording and document the actual Android implementation that shipped in commit `4b0c3e6`. The stale summary contradicts STATE.md row 171 and contributed to the initial misdiagnosis here.

## Bulk Re-Verification (2026-07-28)

**Verdict:** ALREADY-RESOLVED
**Action:** Confirmed status during bulk archive sweep; moved to resolved/.
