# Mango UX evaluation

Revised: 2026-09-04  
Evaluated commit: `a03dd65` (`ppq: implement §7.5 rebindable biometric proxy + §4.6 file-based decoy reactivation`, 2026-09-04)  
Scope: Android (Compose), iOS (SwiftUI), Desktop (Iced)  
Method: source review of HEAD. Pixel captures in this repo are April 2026 and are **not** treated as current unless reconfirmed in source. Live recapture of the current build was not done in this revision.

This is an executable backlog. Each ticket has a status, platforms, and acceptance criteria. Do not implement from April screenshots without recapturing.

---

## Status vocabulary

Statuses are mutually exclusive for each platform-specific claim. A multi-platform ticket may state different statuses per platform.

| Status | Meaning |
|--------|---------|
| **Current** | Defect is observable from HEAD source at `a03dd65` (copy, routing, missing handlers). No screenshot is required to start work. |
| **Needs reproduction** | Suspected from an old capture or layout inference. HEAD does **not** confirm it. Do not implement a visual fix until reproduced on a current build. |
| **Historical** | HEAD contradicts the old claim, or the capture predates the relevant UI. Closed unless recapture revives it. |
| **Future** | Not a current defect. Parked until a flagged feature ships. |
| **Withdrawn** | Previous evaluation claim that was factually wrong. |

Visual sign-off (layout, contrast, clipping) still needs a durable recapture even for Current copy/routing tickets. Track the capture in git or attach it to the ticket; an otherwise-unreferenced gitignored PNG is not durable evidence.

---

## Capture inventory

The ignore rules (`*.png`, `artifacts/*`) apply to new or untracked files; they do not remove files that are already tracked. `git ls-files` confirms that the April Android PNG/XML set and the designer XML/README/index are already in git. Root PNGs, designer PNGs, and `artifacts/ios-runs/` remain local-only. Cite HEAD source as the durable behavioral evidence and use captures only for the historical visual state described below.

| Capture | Date | Provenance | Use |
|---------|------|------------|-----|
| `artifacts/android-screenshots/` | 2026-04-12 | Tracked PNG + XML | Historical. Do not treat as HEAD. |
| `artifacts/designer-screenshots/20260419-130612/` | 2026-04-19 | Mixed: XML/README/index tracked; PNG local-only | Historical Android Material, not iOS. |
| `screenshot-photo.png`, `screenshot-image.png` | 2026-04-19 | Local-only | Historical image-send path. |
| `screenshot-rag1.png` | 2026-04-21 | Local-only | Historical library rows. |
| `screenshot-tools.png`, `screenshot-instructions.png` | 2026-04-16 | Local-only | Historical chat sheets. |
| `artifacts/ios-runs/` | 2026-06-25 | Local-only | XCTest failure frames, not product screens. |

**Required before visual sign-off:** recapture at this commit (or later), with the git SHA in the filename or sidecar. Either attach the capture to the issue, or add a narrow `.gitignore` exception for a repository evidence path and confirm the file appears in `git ls-files`. Merely writing a PNG under `docs/` does not track it while the global `*.png` rule remains active.

---

## Corrections from review

These previous claims are **withdrawn**:

1. **Skip lands in chat.** `SkipOnboarding` sets `Screen::Home` (`rust/src/lib.rs`). Complete onboarding creates a chat; Skip does not.
2. **Android empty chat has no starter prefill.** `ChatScreen` prefills the composer from `StarterPromptList`. Home empty still discards the prompt.
3. **Android attestation is an unlabeled 7px dot.** Android chat uses a tappable labeled `AttestationBadge`. iOS still uses a 7px dot.
4. **Android Settings hub repeats PROVIDERS / Providers.** HEAD hub is link cards only. iOS still wraps `Section("Providers")` around a “Providers” row.
5. **Keep the PIN in the field until unlock succeeds.** iOS clears the secret immediately on submit by design. Show the error, clear the failed secret, refocus.
6. **Duress “erases everything.”** Duress wipes local Mango data and **preserves PPQ credentials** (`README.md`, actor wipe path).
7. **Replace iOS `homeView` with `ConversationListView` as-is.** That reusable list only exposes New Conversation and would drop RAG/Settings.
8. **Wrap all authenticated screens in another `NavigationStack`.** Home and Settings already own stacks. Nesting them is a first-use trap.
9. **Skip still requires PIN setup in the same session.** Skip and Complete do not open `PinSetup`. PIN is chosen on a **later** process start when onboarding is done and `has_auth_params` is false (`rust/src/lib.rs` post-unlock screen selection).
10. **Hybrid is on-device / hybrid must not say Verified TEE.** Hybrid is per-turn. Remote hybrid turns can be TEE-verified; the Android route chip already says so.
11. **UX-21 “no Agents entry” as a current defect.** Android/iOS flags are false; core rejects `Screen::Agents`. That acceptance already passes.

Readiness must not be reconstructed as “enabled, attested remote provider.” Local-only can send without remote attestation. Hybrid can send locally **or** remotely per turn. Prefer a core `canSend` / `unavailableReason` (or equivalent) projected into all three UIs. Per-turn TEE wording is allowed only for remote verified turns.

Emergency vs Duress is a **setup-vs-settings naming split**, present on every platform, not an iOS-vs-Android difference.

---

## What still holds

- Onboarding headline *“Your conversations, provably private.”* is worth keeping if the TEE claim is scoped.
- Android settings-as-hub, conversation search/grouping/swipe, chat overflow, and delete confirmations are the right shape.
- Android chat starter chips and tappable attestation badge are already ahead of iOS.
- iOS memories empty state (brain icon) is better than Android’s trash can.
- Destructive confirmations exist on Android Security.

---

## P0 tickets

### UX-00 — First-run encryption gate (product decision)

**Status:** Current  
**Priority:** P0  
**Platforms:** Core + Android, iOS, Desktop

**Problem:** PIN setup copy and comments declare encryption mandatory and unskippable (D-14): iOS *“There is no Skip option — encryption is always on”*; Android *“Encryption is always on — there is no skip option.”* In HEAD, `CompleteOnboarding` goes to **Chat** and `SkipOnboarding` goes to **Home**. `Screen::PinSetup` is selected only on a **later startup** when onboarding is already complete and the bootstrap DB has no auth params. The same session can therefore chat in plaintext while the UI still claims encryption is always on.

**Why:** This is a security-boundary and trust-copy bug, not a skip-label nit. It is separate from send-availability (UX-02).

**Do — pick one and implement it end to end:**

- **Option A (keep D-14):** After both Complete and Skip, navigate to `PinSetup`. Preserve the intended continuation: Complete → PinSetup → the created first Chat; Skip → PinSetup → Home. No plaintext chat until `SetupPin` succeeds. PIN enrollment is a crash-safe state transition across the plaintext DB, encrypted replacement, bootstrap auth row, and optional keychain DEK. A failure or process kill at any boundary must recover to a retryable PinSetup or a valid Locked state — never a bootstrap record pointing at a plaintext DB, or an encrypted DB without recoverable auth.
- **Option B (make encryption optional):** Allow Chat/Home without a PIN. Remove “always on / no skip” claims from PinSetup. First-run and Settings must say data is unencrypted until a PIN is set, and offer a clear enroll path. Persist the user’s defer/decline state (or stop auto-routing to PinSetup) so every cold start does not reopen an allegedly skippable screen.

Do not ship a mix of A copy and B routing.

**Acceptance**

- Decision is recorded (A or B) and all three UIs match it.
- **If A:** Complete and Skip never land on Chat or Home with `has_auth_params == false`; after successful SetupPin, Complete resumes the first Chat and Skip resumes Home.
- **If A — failure matrix:** inject failure/process death after keychain staging, bootstrap staging, encrypted export/verification, and final DB replacement. Every restart must retain the original data and offer either retryable PinSetup or unlock with the new PIN. No orphaned keychain DEK, committed bootstrap/plaintext mismatch, or encrypted DB without recoverable auth.
- **If B:** PinSetup is skippable; welcome/security copy says encryption is optional; Settings shows “Not encrypted” until enrolled; the defer/decline behavior survives restart without a PinSetup loop.
- Same-session plaintext chat cannot coexist with “encryption is always on.”

**Evidence:** `CompleteOnboarding` / `SkipOnboarding` in `rust/src/lib.rs`; initial screen selection when `has_completed && !has_auth_params` → `PinSetup`; `PinSetupScreen.swift` D-14 comment; `PinSetupScreen.kt` header comment.

---

### UX-01 — Onboarding claims every message is in a TEE

**Status:** Current  
**Priority:** P0  
**Platforms:** Android, iOS, Desktop

**Problem:** Welcome copy says every message is processed inside a Trusted Execution Environment. Mango supports on-device LocalLLM and **hybrid** routing. Hybrid is **per-turn** (`resolve_turn_routing`): a turn may stay local or escalate remote. The Android hybrid chip already reports verified remote turns (*“Escalated to {provider} · {tee} verified”*). Conversation-level *“this conversation is routed to a TEE”* is still false for local-only and for hybrid conversations that include local turns.

**Why:** First-run trust copy is the product’s core claim. Overclaiming is worse than jargon. Banning “Verified TEE” on hybrid profiles would hide real remote attestation.

**Do:** Scope welcome copy: remote confidential routes can be TEE-attested; on-device turns stay on the device; hybrid chooses per turn. Keep “Verified TEE” for **remote verified turns** only. Conversation-level attestation UI must not imply every turn in a hybrid chat was remote/TEE.

**Acceptance**

- Android / iOS / Desktop welcome copy no longer says *every* message is processed in a TEE.
- **Local turn:** on-device wording; no “Verified TEE.”
- **Remote verified turn** (including hybrid escalate): “Verified TEE” (or equivalent) is allowed.
- **Remote unverified / verifying / failed turn:** must not read as verified.
- Android `AttestationBadge` conversation detail does not claim the whole conversation is in a TEE when the route is local or hybrid.

**Evidence:** `OnboardingScreen.kt` / `OnboardingView.swift` / `views/onboarding.rs`; `rust/src/routing/mod.rs` `resolve_turn_routing`; `ChatScreen.kt` `hybridRouteChip`; `AttestationBadge.kt` `detailText`.

---

### UX-02 — First-use send readiness and “You're all set”

**Status:** Current  
**Priority:** P0  
**Platforms:** Core + Android, iOS, Desktop

**Problem:** Skip goes to **Home**, not chat (PIN enrollment is UX-00, not this ticket). The misleading *“You're all set! Send your first message to start a confidential conversation.”* appears after the user later opens or creates a chat (`show_first_chat_placeholder` / empty idle chat). Skip is labeled *“Skip setup”* even though it only skips **provider** setup. UIs reconstruct “ready to send” as if a remote attested provider were required; local-only can send without remote attestation, and hybrid can send locally or remotely per turn.

**Why:** Failed first send and false “all set” copy happen on the first chat, not on Skip itself.

**Do:**

1. Rename Skip to *“Skip provider setup.”*
2. Add a core send-availability projection, e.g. `can_send: bool` + `unavailable_reason: Option<String>`, covering local loaded, hybrid (local or remote), and remote-with-key. Do not require remote attestation for a local-only send.
3. Empty chat copy and Send enablement consume that state. If unusable: *“Connect a provider or enable on-device inference in Settings.”* with a Settings button.
4. Keep *“You're all set…”* only when `can_send` is true.

**Acceptance**

- Send-availability tests: local-only, hybrid local default, hybrid remote escalate, remote-unattested, no-backend.
- Android / iOS / Desktop: empty chat copy and Send follow core reason; no client-side “must be attested remote” gate for local sends.
- Skip button label is *Skip provider setup* on all three.
- Onboarding exit screens themselves are owned by UX-00.

**Evidence:** `rust/src/lib.rs` `SkipOnboarding` / `CompleteOnboarding`; `ChatScreen.kt` empty copy; iOS/Desktop same string.

---

### UX-03 — Desktop empty sidebar hides Settings and Library

**Status:** Current  
**Priority:** P0  
**Platforms:** Desktop

**Problem:** `conversations.is_empty()` returns before the RAG and Settings buttons. After Skip, there is no settings entry in the sidebar.

**Why:** First-run trap: skip provider setup, then cannot add a provider.

**Do:** Always render the bottom nav (Library + Settings, Agents if flagged). Keep the empty-list copy.

**Acceptance**

- Empty conversation list still shows Library and Settings.
- After Skip, Settings → Providers is reachable without creating a chat.

**Evidence:** `desktop/iced/src/views/home.rs` empty-state early return.

---

### UX-04 — iOS home list is a stub; keep Library and Settings

**Status:** Current  
**Priority:** P0  
**Platforms:** iOS

**Problem:** Live home is title-only buttons. `ConversationListView` has time, model, swipe-delete, rename, and empty state, but it is unused and its toolbar is **only** New Conversation.

**Do not:** Swap in `ConversationListView` as-is (drops RAG/Settings).  
**Do:** Keep `homeView`’s `NavigationStack` and trailing Library + Settings (rename RAG → Library). Replace the `List` body with the richer rows/empty state from `ConversationListView`, or give that view optional toolbar slots and pass Library/Settings through.

**Acceptance**

- Home rows show title, relative time, and model.
- Empty state CTA exists.
- Swipe delete and rename work.
- **Library and Settings remain in the home toolbar** after the change.
- No nested extra `NavigationStack` on Home.

**Evidence:** `ContentView.homeView`; `ConversationListView.swift` (definition only).

---

### UX-05 — iOS chat needs a Chat-only navigation wrapper

**Status:** Current  
**Priority:** P0  
**Platforms:** iOS

**Problem:** `ChatView` sets `.navigationTitle`, `.toolbar` (model picker), and takes `onBack`, but `ContentView` presents chat as a sibling of Home, not inside Home’s stack. `onBack` is never called. Home and Settings already own `NavigationStack`s.

**Do not:** Wrap the whole authenticated shell in another stack.  
**Do:** Wrap **only** `ChatView` in a `NavigationStack`. Leading Back calls `onBack` → `PopScreen`. Do not duplicate the title strip and the nav title.

**Acceptance**

- From a conversation, Back returns to Home.
- Model picker is visible in the chat nav bar.
- Home and Settings are not nested inside a second stack.
- Library and Settings remain reachable from Home.

**Evidence:** `ContentView` `.chat` branch; `ChatView` toolbar / unused `onBack`; `SettingsView` and `homeView` already use `NavigationStack`.

---

### UX-06 — iOS lock failures are silent

**Status:** Current  
**Priority:** P0  
**Platforms:** iOS (Desktop already shows toast)

**Problem:** Wrong PIN / failed biometrics produce no inline error. Submit clears the field immediately (`LockScreen.swift`), which is correct for secret handling.

**Do:** Show `appState.toast` (or a dedicated auth error) on the lock screen. Keep immediate secret clear. Refocus the field after failure. Do not keep the PIN in memory until success.

**Acceptance**

- Failed PIN shows an error string on the lock screen.
- Field is empty after submit, focused for retry.
- Successful unlock still clears the secret before navigation.

**Evidence:** `ios/Mango/Mango/LockScreen.swift` `submitPin()`; desktop `lock_screen.rs` error path.

---

### UX-07 — Lock keyboard fallback, first-run copy, and Change PIN rewrap

**Status:** Current  
**Priority:** P0  
**Platforms:** Core + Android, iOS, Desktop

**Problem:** Lock/setup use a generic masked field. Android setup says *“You will need this PIN every time you open Mango,”* which conflicts with biometrics and “Never” timeout. There is **no** Change PIN action. The bootstrap record (`AuthParams`) stores salt, wrapped DEK, optional duress hash, and KDF params — **no PIN vs passphrase class**. Auto-detecting credential class from bootstrap is impossible.

**Why:** A numeric-only pad would lock out existing alphanumeric credentials. Changing the PIN is a DEK-rewrap, not a UI text field.

**Do — three concrete slices:**

1. **Keyboard (no schema migration required).** Default lock/setup to the **full keyboard** (legacy-safe). Optional explicit control: “Use numeric pad” (user-selected, stored in ordinary settings if desired — not in `AuthParams`). Never infer class from the bootstrap row.
2. **First-run copy.** PIN/passphrase unlocks the vault; biometrics are optional; timeout is configurable. Do not say the PIN is required on every open.
3. **Change PIN — new core operation** (no such `AppAction` exists today). Suggested contract:
   - `ChangePin { old_pin, new_pin }` (names as implemented).
   - Enforce the credential policy in core, not only in UI: non-empty/minimum length, confirmation in UI, and `new_pin` must not verify against the existing `duress_hash`.
   - Apply the established duress policy if `old_pin` is the duress PIN; do not return a distinct error that reveals the match.
   - Verify `old_pin` unwraps the current DEK; fail closed without writing.
   - Derive a new KEK from `new_pin` and rewrap the **same** DEK.
   - Use a dedicated transactional `UPDATE`/UPSERT for the main credential fields. Do not reuse the current `write_auth_params` replacement: it omits `cold_launch_bypass`.
   - Preserve `duress_hash` and `cold_launch_bypass`; preserve KDF fields unless they are intentionally rotated with the new salt.
   - Leave the platform-keychain DEK unchanged because the DEK does not rotate; verify that biometric and Never-timeout unlock still work.
   - On any write failure: previous wrapped DEK still unlocks; no half-applied row.

**Acceptance**

- Existing alphanumeric credentials still unlock if the user never touches “numeric pad.”
- New installs can type either digits or a passphrase on the default keyboard.
- First-run copy does not say the PIN is required on every open if biometrics or Never timeout exist.
- Core tests for Change PIN:
  - happy path: old PIN fails after change; new PIN unwraps the same DEK; messages still decrypt.
  - wrong old PIN: no write.
  - rejected new credential: too short/empty and equal to the configured duress PIN both produce no write.
  - old credential matching the duress PIN follows the same silent duress behavior as unlock/sensitive auth.
  - **rollback:** simulated bootstrap write failure leaves the old wrapped DEK intact.
  - **interruption:** process kill between unwrap and successful write does not strand the DB (old PIN still works).
  - duress hash still verifies after a successful change.
  - `cold_launch_bypass` unchanged.
  - biometric keychain DEK, if present, remains unchanged and still unlocks.
- Security UI: Change PIN (old + new + confirm) on Android, iOS, and Desktop; never keep `old_pin`/`new_pin` in view state after dispatch.

**Evidence:** `rust/src/crypto/bootstrap_db.rs` `AuthParams` (no credential-class field); `AppAction::SetupPin` only; no `ChangePin` in core; lock/setup screens.

---

## P1 tickets

### UX-08 — Android Instructions sheet ignores saved prompt

**Status:** Current  
**Priority:** P1  
**Platforms:** Android (iOS already prefills)

**Problem:** `SystemPromptSheet(initialPrompt = "")`. iOS sets `currentSystemPrompt` from the conversation.

**Acceptance**

- Reopening Instructions shows the saved prompt.
- Placeholder is an example, not a repeat of the helper caption.

**Evidence:** `ChatScreen.kt` `initialPrompt = ""`; `ChatView.swift` `initialPrompt: currentSystemPrompt`.

---

### UX-09 — Long replies clip under the chat header

**Status:** Needs reproduction (historical tracked capture)  
**Priority:** P1  
**Platforms:** Android first

**Problem:** A tracked April 2026 chat capture showed the top of a long assistant bubble cut off, but it predates the current layout. HEAD already applies Scaffold `innerPadding` to the list container, then 8.dp `contentPadding` on the reverse `LazyColumn`. Extra padding is **not** prescribed until clipping is reproduced on a current build.

**Do:** Recapture a long markdown reply on HEAD. If clipping still happens, file a layout fix from that capture. If not, close as Historical.

**Acceptance**

- Either a tracked or issue-attached recapture at this SHA (or later) showing clipping, plus a fix verified on the same build, **or** an equivalently durable recapture showing no clipping and this ticket closed.

**Evidence (not sufficient to implement):** tracked historical `artifacts/android-screenshots/dark/chat.png` (2026-04-12). HEAD: `ChatScreen.kt` `padding(innerPadding)` then `contentPadding` 8.dp.

---

### UX-10 — Persistence image placeholder is shown in the bubble

**Status:** Current  
**Priority:** P1  
**Platforms:** Android, iOS, Desktop

**Problem:** Core persists `[Image: filename]` in `UiMessage.content`. Android `UserBubble` still renders `message.content` as text whenever content is non-empty, so the placeholder can appear next to the thumbnail.

**Do:** Strip or hide the persistence placeholder in all three UIs. Keep the thumbnail / attachment chip.

**Acceptance**

- A sent image never shows `[Image: …]` in the bubble.
- Copy/export can still include a human filename if needed.

**Evidence:** `rust/src/lib.rs` persistence placeholder; `MessageBubble.kt` renders `message.content`. April `screenshot-photo.png` is historical for transport failure.

---

### UX-11 — Auto-title uses raw first user text

**Status:** Current  
**Priority:** P1  
**Platforms:** Core (all UIs)

**Problem:** Title is `truncate_title` of the first user message. Short or placeholder text becomes titles like `re` or `[Image: …]`.

**Do:** Title from meaningful prompt text; ignore image placeholders; fall back to “New Conversation” or a date until there is real text.

**Acceptance**

- Image-only first message does not title the chat `[Image: …]`.
- One- or two-character first messages are not used as the lasting title without a better fallback.

**Evidence:** `rust/src/lib.rs` `truncate_title`; `rust/src/tests/chat.rs` `test_send_message_auto_title_from_text`.

---

### UX-12 — Vision send looked broken in April captures

**Status:** Historical until recapture  
**Priority:** P1  
**Platforms:** Android, iOS, Desktop

**Problem:** April captures show the model asking for the photo after an attach. HEAD already gates Take/Choose Photo with `modelSupportsVision`.

**Do:** Recapture image send on a vision model and a non-vision model at this SHA. File a new ticket only if transport still fails.

**Acceptance**

- Recapture: vision model describes the photo; non-vision model is blocked before send with a reason.

**Evidence:** `ChatScreen.kt` `showImageOptions`; historical `screenshot-photo.png` / `screenshot-image.png`.

---

### UX-13 — User-facing “RAG” naming

**Status:** Current on iOS and Desktop; Android home/library already say Library  
**Priority:** P1  
**Platforms:** iOS, Desktop; audit remaining Android strings

**Do:** One name: **Library**. Chat menu: *Use from Library*. Folders live in Library.

**Acceptance**

- No user-visible “RAG” on iOS home, iOS library title, iOS chat menu, desktop sidebar, or desktop documents title.
- Android has no remaining user-visible “RAG”.

**Evidence:** iOS `ContentView` / `DocumentLibraryView` / `ChatView`; desktop `home.rs` / `documents.rs` / `chat.rs`; Android `MainApp.kt` already uses Library.

---

### UX-14 — Library row labels are machine paths and bad plurals

**Status:** Current (April PNG is local-only / gitignored; defect is `doc.name` + `"${fileCount} files"` in HEAD)  
**Priority:** P1  
**Platforms:** Android first; check iOS/Desktop display names

**Do:** Show a basename (`canary1.md`). Keep the full path in accessibility/detail. Pluralize *1 file*. Prefer *indexed* over *chunks* if that is user-facing.

**Acceptance**

- Library list does not show `primary:Download/…`.
- `1 file` / `N files` are grammatically correct.

**Evidence:** `DocumentLibraryScreen.kt` `doc.name`, `"${source.fileCount} files"`; historical `screenshot-rag1.png`.

---

### UX-15 — Home chrome crowds the title

**Status:** Needs reproduction (the text controls are current; crowding and contrast are not confirmed)  
**Priority:** P1  
**Platforms:** Android, iOS

**Problem:** Android home uses Library and Settings text buttons beside the title; iOS uses New, RAG, and Settings text toolbar controls. Source confirms those structures, but it does not prove title truncation or poor FAB contrast. The April Android capture predates HEAD, and no current compact-width iOS capture exists.

**Do:** Recapture Android at 360dp and iOS at a compact phone width. If controls truncate, collide, or squeeze the title, then choose an icon toolbar or bottom tabs: Chats, Library, Settings. Agents remains absent while its flag is false. Restyle the FAB only if the current capture or a contrast measurement confirms the problem.

**Acceptance**

- Tracked/issue-attached captures cover Android 360dp and a compact-width iPhone.
- Title and actions do not truncate, collide, or become ambiguous at those widths.
- Agents remains absent when the flag is false.

**Evidence:** `MainApp.kt` `topBarActions`; `FeatureFlags.AGENTS_ENABLED = false`; historical `light/home.png`.

---

### UX-16 — Android home starter chips discard the prompt

**Status:** Current  
**Priority:** P1  
**Platforms:** Android

**Problem:** Empty home `StarterPromptList(onPrompt = { onNew() })` does not pass the prompt. Chat empty state already prefills.

**Do:** Creating a chat from a home chip prefills the composer (same path as chat chips).

**Acceptance**

- Tapping a home starter opens chat with that text in the composer.

**Evidence:** `ConversationListScreen.kt` line with `onNew()`; contrast `ChatScreen.kt` `composerPrefill`.

---

### UX-17 — iOS and Desktop empty chat have no starter chips

**Status:** Current  
**Priority:** P1  
**Platforms:** iOS, Desktop

**Do:** Same 2–3 chips and composer prefill as Android chat. Show model + honest route state from UX-02, not a fake “attested” line for local.

**Acceptance**

- Empty chat on iOS and Desktop offers the same starters and prefills input.

**Evidence:** `ChatView.swift` / `views/chat.rs` welcome string only.

---

### UX-18 — iOS Tools sheet “set it in Settings” is non-interactive

**Status:** Current  
**Priority:** P1  
**Platforms:** iOS

**Problem:** When Brave is unset, the iOS tools sheet shows *“API key not configured — set it in Settings”* as caption text on a **disabled** toggle. There is no button or navigation to Settings → Tools. Android already has **Configure** → `Screen.SettingsTools`.

**Do:** Add a tappable control that pushes Settings → Tools (or the Brave key field). Keep the toggle disabled until a key exists. Do not treat Android or Agents as part of this ticket.

**Acceptance**

- iOS: from Chat → Tools, with no Brave key, one tap reaches the Brave API key field.
- The disabled toggle does not pretend to be the way to configure the key.

**Evidence:** `ios/Mango/Mango/ChatView.swift` `ToolsSheet` (`disabled(!braveApiKeySet)` + non-interactive caption). Android `ChatScreen.kt` `onOpenToolSettings` is already wired.

---

### UX-19 — Attestation affordance parity

**Status:** Current for iOS/Desktop affordance and Android dialog copy; Needs reproduction for Android screen-reader semantics  
**Priority:** P1  
**Platforms:** iOS, Desktop; Android a11y/copy only

**Do:** iOS: reuse `AttestationBadgeView` (or menu item) instead of a 7px unverified-hidden dot. Desktop: provide a tappable/clickable status. Android: keep the nested icon decorative (`contentDescription = null`) because the clickable badge already contains status text; verify the whole badge with TalkBack. Add a container label/state description only if TalkBack does not announce an unambiguous status and action. Rename the dialog button to *Close* and use the route-accurate detail text from UX-01.

**Acceptance**

- Unverified is visible, not “no indicator.”
- Tap/click explains the **current** route: local on-device, remote verified TEE, or remote unverified — not a single hybrid-profile slogan.
- Screen reader announces attestation status on all three.
- Android TalkBack verification records the announcement for Verified, Not Verified, Expired, and Failed; the decorative icon is not announced separately.

**Evidence:** Android `AttestationBadge.kt` clickable `Surface` contains `Text(badgeLabel(status))`; its nested icon is decorative. iOS `ModelPickerView.swift` uses a 7px circle and hides the unverified dot. Desktop uses a non-interactive dot.

---

### UX-20 — iOS conversation overflow parity

**Status:** Current  
**Priority:** P1  
**Platforms:** iOS

**Problem:** Chat menu is RAG / Instructions / Tools. `showDeleteConfirmation` is unused. No Fork, Delete, or Share.

**Do:** confirmationDialog: Use from Library, Instructions, Tools, Fork (if messages), Share, Delete (confirm). Wire `onBack` via UX-05.

**Acceptance**

- iOS can fork, share, and delete a conversation from chat.
- Delete requires confirmation.

**Evidence:** `ChatView.swift` confirmationDialog; unused `showDeleteConfirmation`.

---

### UX-21 — Agents IA when the feature ships

**Status:** Future  
**Priority:** P2 (parked)  
**Platforms:** All

**Problem:** Chat, chat tools, and Agents are three IA concepts. That is not a current defect: Android `AGENTS_ENABLED` and iOS `agentsEnabled` are false; core rejects `PushScreen(Agents)` with a toast. Home already omits Agents when the flag is off. The “no Agents entry while flagged off” bar already passes.

**Do:** When Agents is enabled, decide whether it stays a separate top-level surface or folds into chat tools. Until then, do not add Agents entries or “agent web search” copy. iOS Tools configuration is UX-18, not this ticket.

**Acceptance (only when the flag is turned on)**

- One-sentence explanation of agent vs chat is visible before first launch.
- Tools for ordinary chat remain reachable without opening Agents.

**Evidence:** `FeatureFlags.kt` / `FeatureFlags.swift`; `rust/src/lib.rs` `PushScreen` Agents reject.

---

### UX-22 — Android memories empty uses a trash icon

**Status:** Current  
**Priority:** P1  
**Platforms:** Android

**Do:** Brain/spark icon. *“Memories appear after a few chats if auto-extract is on.”*

**Acceptance**

- Empty memories does not use a delete icon.

**Evidence:** `MemoryScreen.kt` `Icons.Filled.Delete` for empty state.

---

### UX-23 — Provider status density

**Status:** Current (April PNGs are local-only; HEAD still packs badges and re-attestation on the same screen)  
**Priority:** P1  
**Platforms:** All

**Do:** One status line (*Ready*, *Needs API key*, *On-device*). Move re-attestation under Advanced. Confirm before Remove on the default provider.

**Acceptance**

- New users can enable a provider without seeing re-attestation interval as a peer of API key.
- Remove default provider asks for confirmation.

**Evidence:** `SettingsProvidersScreen.kt`; historical `screenshot-providers.png`.

---

### UX-24 — Duress naming and accurate wipe copy

**Status:** Current  
**Priority:** P1  
**Platforms:** All (setup vs settings, not iOS vs Android)

**Problem:** Setup says **Emergency PIN**; Settings says **Duress PIN**. Wipe copy implies total erasure. Duress preserves PPQ credentials.

**Do:** One name in setup and settings (prefer **Duress PIN** with a one-line definition). Copy must state: *silently erases local Mango data; PPQ credentials remain dormant on this device; restoring PPQ access requires possession of the encrypted `.mppq` backup and its password.* Do not imply that preserved credentials automatically reappear. Keep the lock screen and decoy session silent about duress and about whether dormant credentials exist.

**Acceptance**

- No “Emergency PIN” vs “Duress PIN” split.
- No “erases everything / all data” without the PPQ exception.
- Before enabling duress, the user is told that PPQ reactivation requires both the encrypted `.mppq` file and its password.
- After duress, decoy UI never reveals whether PPQ credentials were preserved; only a valid file-based restore can reactivate them.

**Evidence:** `PinSetupScreen.kt` / `.swift` / `pin_setup_screen.rs`; `SettingsSecurityScreen.kt`; `README.md` PPQ duress note; `rust/src/lib.rs` duress wipe log.

---

### UX-25 — Errors swallowed at the shell

**Status:** Current  
**Priority:** P1  
**Platforms:** iOS first; verify Android/Desktop toast shell

**Do:** One toast/banner on `appState.toast` at the iOS root (including lock — see UX-06). File import and attach failures show a user-actionable sentence.

**Acceptance**

- Failed document import and failed attach are visible without opening a log.
- Lock errors use the same channel without leaking the PIN.

**Evidence:** iOS document import comments; `ContentView` has no global toast observer.

---

### UX-26 — iOS tool discovery route is a dead end

**Status:** Current  
**Priority:** P1  
**Platforms:** iOS

**Do:** Port Discover Tools, or stop routing `.toolDiscovery` / `.contextvmToolDetail` to `SettingsToolsView` and hide the entry.

**Acceptance**

- No iOS navigation path that claims Discover Tools and opens only the Brave key screen.

**Evidence:** `ContentView.swift` `.toolDiscovery, .contextvmToolDetail` → `SettingsToolsView`.

---

## P2 tickets

### UX-27 — Settings hub section labels repeat row titles

**Status:** Historical on Android. Current on iOS (`Section("Providers")` + row “Providers”). Desktop is one long page.  
**Priority:** P2  
**Platforms:** iOS

**Do:** iOS hub rows only (title + one-line status). Move hybrid/local to subpages if the root is crowded.

**Acceptance**

- iOS Settings root does not repeat the section name as the only row title.

**Evidence:** `SettingsView.swift` `providersSection`; Android `SettingsScreen.kt` has no `SettingsSectionLabel` on hub cards.

---

### UX-28 — Copy bugs

**Status:** Current  
**Priority:** P2  
**Platforms:** as listed

| Copy | Where | Do |
|------|--------|----|
| *Use Face ID, Touch ID, or device biometrics* | Android Security | Fingerprint or face unlock |
| *No "Forgot PIN" option — recovery requires reinstalling* | iOS lock footer | Move to PIN setup, not every unlock |
| Desktop has no recovery line | Desktop lock | One line on PIN setup, not lock |
| Onboarding API-key help | Android/iOS | Include shipped presets (Venice, etc.) |

**Acceptance:** Each row’s “Do” is true on that platform.

---

### UX-29 — Composer and lock chrome

**Status:** Current  
**Priority:** P2  
**Platforms:** All

**Do:** Composer placeholder more specific than *Message*. Filled send when `can_send` and text/attachment present. Attach remove control ≥ 48dp / 44pt. Auto-focus lock field. Recapture lock disabled-button contrast.

**Acceptance:** Hit targets meet 48dp/44pt. Send is disabled with `unavailable_reason` from UX-02, not only empty text.

**Evidence:** `ComposeBar.kt` 24.dp remove; placeholder *Message*.

---

### UX-30 — Accessibility leftovers

**Status:** Current  
**Priority:** P2  
**Platforms:** All

**Do:** Progress dots *Step N of 4*. Search/share/directory icons labeled. Attestation covered by UX-19.

**Acceptance:** Icon-only controls have names. Onboarding step is announced.

---

### UX-31 — Visual identity

**Status:** Needs reproduction  
**Priority:** P2  
**Platforms:** Android first

**Do:** Pick one accent (spec teal vs implemented blue) and apply it, including light FAB. Recapture before restyling from April light shots.

**Acceptance**

- A tracked or issue-attached current capture and contrast measurement establish the defect before colors change.
- The chosen accent is recorded and applied consistently to primary Android actions in light and dark themes.
- Updated captures show that text and controls retain accessible contrast.

**Evidence:** `Color.kt` `#4D9EFF`; Phase 24 UI-SPEC teal.

---

### UX-32 — First-chat vs later empty copy

**Status:** Current  
**Priority:** P2  
**Platforms:** All

**Do:** After UX-02, empty thread that is *not* the onboarding first chat is quieter: model + route + chips, not a second “You're all set.”

**Acceptance:** `show_first_chat_placeholder` and later empty chats use different copy.

---

## Cross-platform snapshot (`a03dd65`)

| Area | Android | iOS | Desktop |
|------|---------|-----|---------|
| Home | Time + model, search, Library/Settings text links | Title-only list; New / RAG / Settings | Sidebar; **empty list drops Library/Settings** |
| Chat chrome | Back + title + labeled attestation badge + model + `⋮` | Custom strip; toolbar likely inert; no Back | Always-on header |
| Attestation | Tappable *Verified* badge | 7px dot; unused badge view | Dot |
| Starter chips | Chat prefills; **home chips discard prompt** | None | None |
| Instructions | Sheet opens empty | Prefills | Has prompt UI |
| Fork / delete / share | Menu items | Missing from overflow | Fork present |
| Agents | Flag false; core rejects route | Flag false; core rejects route | Core flag false; route rejected |
| Tool discovery | Shipped | Route opens Brave-only tools | Full UI |
| Library naming | Library | RAG | RAG |
| Memories empty | Trash icon | Brain icon | Screen, no nav |
| Lock errors | Possible via toast/inline | Silent; secret cleared (keep that) | Toast |
| PIN / duress names | Setup Emergency, settings Duress | Same split | Same split |
| Skip | Home, not chat | Home | Home, and empty sidebar has no Settings |
| PIN after onboarding | Next cold start if no auth params; same-session plaintext chat possible | Same | Same |
| TEE welcome copy | “Every message…” | Same | Same |

---

## Suggested order

| Wave | Tickets | Why first |
|------|---------|-----------|
| 1 | UX-00, UX-01, UX-02, UX-03, UX-04, UX-05, UX-06, UX-07 | Encryption-gate decision, TEE/hybrid copy, send-availability, iOS nav, **Change PIN rewrap** |
| 2 | UX-08, UX-10, UX-11, UX-12 | Daily chat correctness; recapture vision. **UX-09 only after reproduction** |
| 3 | UX-13–UX-20, UX-22–UX-26 | Understandable product, parity. UX-18 is iOS Tools link only |
| 4 | UX-27–UX-32; UX-21 when Agents ships | Polish; Agents IA is Future |

Wave 1 is **not** copy-and-wiring. It includes a crash-safe first-run encryption transition (UX-00), a core DEK rewrap with duress and rollback tests (UX-07), and send-availability. Before closing visual tickets, attach a current capture to the issue or add a narrow ignore exception and confirm the evidence is tracked. April captures are stale regardless of whether they are already tracked (especially UX-09, UX-12, UX-15, UX-23, UX-31).
