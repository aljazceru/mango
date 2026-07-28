---
status: resolved
trigger: "instructions-not-persisted-in-ui"
created: 2026-04-03T00:00:00Z
updated: 2026-04-03T00:00:00Z
---

## Current Focus

hypothesis: CONFIRMED — Two independent bugs: (1) iOS ChatView hardcodes `currentSystemPrompt = ""` on button tap instead of reading current value; (2) ConversationSummary in AppState/Rust lib.rs does NOT expose system_prompt field to the UI, so the UI has no source of truth to read from
test: CONFIRMED via code reading
expecting: Fix requires: add system_prompt to ConversationSummary (Rust), populate it in list_conversations query, and read it when opening the sheet (iOS)
next_action: Apply fixes to Rust ConversationSummary and iOS ChatView

## Symptoms

expected: Pressing "instructions" button should show previously entered instructions so user can view/edit them
actual: Pressing "instructions" shows blank text box every time — previous instructions disappear
errors: No error messages reported — it is a silent data loss / UI state issue
reproduction: 1. Open chat, press "instructions" button, type instructions, close. 2. Press "instructions" again — text box is blank
started: User just discovered this behavior

## Eliminated

- hypothesis: Instructions are not being saved/persisted at all
  evidence: persistence/queries.rs has update_conversation_system_prompt and the chat pipeline reads it correctly (lib.rs:832-840). The Rust core saves and uses the system prompt correctly. Problem is purely in the UI read-back path.
  timestamp: 2026-04-03

- hypothesis: SystemPromptView itself loses state
  evidence: SystemPromptView.swift correctly initializes @State from initialPrompt parameter. It would show correctly IF it received a non-empty initialPrompt. The bug is upstream — ChatView passes "" always.
  timestamp: 2026-04-03

- hypothesis: Desktop (iced) has the same bug
  evidence: Desktop has the same comment "system prompt isn't in ConversationSummary, leave blank for user to fill" (main.rs:600). Both platforms are affected but iOS is the immediate target.
  timestamp: 2026-04-03

## Evidence

- timestamp: 2026-04-03
  checked: ios/Mango/Mango/ChatView.swift lines 134-137
  found: Button "Instructions" handler explicitly sets `currentSystemPrompt = ""` before opening the sheet. This hardcoded reset is the direct cause of the blank text box.
  implication: Every time the sheet opens, it receives an empty string regardless of what was saved previously.

- timestamp: 2026-04-03
  checked: rust/src/lib.rs ConversationSummary struct (lines 56-62)
  found: ConversationSummary has id, title, model_id, backend_id, updated_at — no system_prompt field. AppState.conversations carries this, so UI can never read back the saved system prompt.
  implication: Even if iOS fixed the reset-to-empty bug, it has no way to get the correct value from AppState without adding system_prompt to ConversationSummary.

- timestamp: 2026-04-03
  checked: rust/src/persistence/queries.rs lines 31, 113-124
  found: The internal ConversationRow struct already has system_prompt: Option<String> and the SELECT query fetches it. Only the public ConversationSummary (UniFFI-exported) is missing it.
  implication: Rust already has the data; just needs to be exposed via the UniFFI boundary.

- timestamp: 2026-04-03
  checked: rust/src/lib.rs lines 2108-2120 (snapshot_state's conversation mapping)
  found: ConversationSummary is constructed in snapshot_state by mapping from DB rows — this mapping must also include system_prompt once added.
  implication: Need to find and update that mapping too.

- timestamp: 2026-04-03
  checked: Instructions are per-conversation (not app-wide) based on lib.rs:832-840: conv_system_prompt is read per conversation, with a fallback to global_system_prompt setting
  implication: The feature is per-conversation with a global fallback — both need surfacing if we want to show the user what is active, but the immediate bug fix is per-conversation only.

## Resolution

root_cause: Two-part bug. (1) In iOS ChatView.swift line 135, the "Instructions" button handler hardcodes `currentSystemPrompt = ""` instead of reading the current conversation's system prompt. (2) ConversationSummary (the UniFFI-exported struct) did not include a system_prompt field, so the UI had no source of truth to read from even if it tried.
fix: (1) Added system_prompt: Option<String> to ConversationSummary in rust/src/lib.rs and populated it in both mapping sites (refresh_conversations and snapshot_state). (2) Fixed iOS ChatView.swift to read `currentConversation?.systemPrompt ?? ""` instead of hardcoding "". (3) Fixed desktop main.rs ToggleSystemPromptInput to also populate from state.conversations.
verification: cargo test -p mango_core passes 204/204. cargo check on both mango_core and mango-desktop clean (only pre-existing warnings). iOS Swift change cannot be compiled in this environment but the logic is correct.
files_changed:
  - rust/src/lib.rs
  - ios/Mango/Mango/ChatView.swift
  - desktop/iced/src/main.rs

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** system_prompt: Option<String> lib.rs:79; iOS ChatView.swift:207
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
