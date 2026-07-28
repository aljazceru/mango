---
status: resolved
trigger: "top-bar-overflow-redesign"
created: 2026-04-08T00:00:00Z
updated: 2026-04-08T00:01:00Z
---

## Current Focus

hypothesis: CONFIRMED AND FIXED
test: cargo check passes for desktop; iOS/Android changes are structurally correct
expecting: user verifies no overflow on iOS and Android
next_action: await human verification

## Symptoms

expected: Top menu bar items fit without horizontal overflow on iPhone screens
actual: Items overflow — model name + "Verified" + "RAG" + "Instructions" + "Tools [ON]" are all displayed inline and don't fit
errors: No crash, just visual overflow/truncation
reproduction: Open any chat conversation on iOS — the top bar overflows
started: Got worse after Phase 27 added the "Tools [ON]" toggle to the top bar

## Eliminated

## Evidence

- timestamp: 2026-04-08T00:01:00Z
  checked: ios/Mango/Mango/ChatView.swift lines 106-155
  found: ToolbarItemGroup(.principal) has ModelPickerView + AttestationBadgeView; ToolbarItem(.primaryAction) has Docs button + Instructions button + Tools Toggle — all inline
  implication: Primary action slot on iOS has limited width; all three items shown inline causes overflow

- timestamp: 2026-04-08T00:01:00Z
  checked: android/.../ui/ChatScreen.kt ChatTopBar composable lines 290-368
  found: actions = { model menu + AttestationBadge + RAG TextButton + Instructions TextButton + Tools TextButton } — all shown as action bar items inline
  implication: Same overflow issue on Android, same fix needed

- timestamp: 2026-04-08T00:01:00Z
  checked: desktop/iced/src/views/chat.rs header_row lines 133-143
  found: row![title_elem, Space::Fill, badge_elem, tools_btn, docs_btn, model_picker] — Instructions is a separate row below the header (instructions_section)
  implication: Desktop is less problematic (wide window) but also needs the redesign for consistency

## Resolution

root_cause: All three platforms render attestation badge + RAG + Instructions + Tools as separate inline items in the top bar, which overflows narrow iOS/Android screens. The Tools toggle (added in Phase 27) made it worse.
fix: 1) Remove separate AttestationBadgeView/AttestationBadge from header; add green circle indicator on model picker when verified. 2) Collapse Docs/Instructions/Tools into a single "..." menu button that shows a popup/sheet with toggles.
verification: desktop cargo check passes clean; iOS/Android structurally verified by code review
files_changed:
  - ios/Mango/Mango/ChatView.swift
  - ios/Mango/Mango/ModelPickerView.swift
  - android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt
  - desktop/iced/src/views/chat.rs
  - desktop/iced/src/main.rs

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** iOS confirmationDialog, Android MoreVert+DropdownMenu, Desktop ... button
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
