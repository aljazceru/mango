---
phase: 27-add-optional-tool-use-to-chat
plan: 03
subsystem: ui
tags: [uniffi, swift, kotlin, iced, tools-toggle, chat-ui]

requires:
  - phase: 27-01
    provides: ConversationSummary.tools_enabled field and SetConversationToolsEnabled AppAction in Rust core
  - phase: 27-02
    provides: end-to-end chat tool round-trip via Rust core

provides:
  - Regenerated UniFFI Swift bindings with toolsEnabled field and SetConversationToolsEnabled action
  - Regenerated UniFFI Kotlin bindings with toolsEnabled field and SetConversationToolsEnabled action
  - iOS ChatView.swift Tools toggle button in toolbar (Toggle with wrench.fill icon, dispatches setConversationToolsEnabled)
  - Android ChatScreen.kt Tools toggle TextButton in ChatTopBar (dispatches SetConversationToolsEnabled)
  - Desktop iced chat.rs Tools [ON/OFF] button in chat header row
  - Desktop iced main.rs ToggleConvToolsEnabled message variant and handler

affects: [27-04]

tech-stack:
  added: []
  patterns:
    - "Tools toggle follows the Instructions/system-prompt toggle pattern on all three platforms"
    - "ChatView.swift uses onSetToolsEnabled callback; ContentView.swift wires dispatch"
    - "Desktop uses Message::ToggleConvToolsEnabled -> AppAction::SetConversationToolsEnabled dispatch pattern"

key-files:
  created: []
  modified:
    - ios/Bindings/mango_core.swift
    - android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt
    - ios/Mango/Mango/ChatView.swift
    - ios/Mango/Mango/ContentView.swift
    - android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt
    - android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt
    - desktop/iced/src/main.rs
    - desktop/iced/src/views/chat.rs

key-decisions:
  - "iOS ChatView receives onSetToolsEnabled: (Bool) -> Void callback; dispatch lives in ContentView.swift coordinator (consistent with other callbacks)"
  - "Android onDispatchAction: (AppAction) -> Unit threaded through ChatScreen -> ChatTopBar to avoid per-action callback proliferation"
  - "Desktop tools button placed between badge_elem and docs_btn in header_row, mirroring visual priority"
  - "Bindings regenerated via just bindings-swift and just bindings-kotlin from the already-built release binary"

requirements-completed: [CHAT-TOOL-07, CHAT-TOOL-08]

duration: 20min
completed: 2026-04-07
---

# Phase 27 Plan 03: UniFFI Bindings Regeneration and Tools Toggle UI Summary

**UniFFI bindings regenerated with toolsEnabled/SetConversationToolsEnabled; tools toggle added to iOS, Android, and Desktop chat UIs**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-04-07T15:20:00Z
- **Completed:** 2026-04-07T15:40:04Z
- **Tasks:** 1 of 2 (Task 2 is checkpoint:human-verify)
- **Files modified:** 8

## Accomplishments

- Regenerated Swift and Kotlin UniFFI bindings to pick up `toolsEnabled: Bool` on `ConversationSummary` and `SetConversationToolsEnabled` in `AppAction`
- iOS ChatView.swift gets a Toggle button (wrench.fill icon, `.toggleStyle(.button)`) in the toolbar; tint reflects current `toolsEnabled` state
- Android ChatTopBar gets a TextButton that toggles tools on/off; `onDispatchAction` callback threaded through ChatScreen and wired in MainApp.kt
- Desktop chat header gets a "Tools" / "Tools [ON]" button with accent background when active; `ToggleConvToolsEnabled` message dispatches `SetConversationToolsEnabled` to Rust core
- Desktop build (`cargo build -p mango-desktop`) passes with no errors

## Task Commits

1. **Task 1: Regenerate UniFFI bindings and add tools toggle to all platforms** - `bd1426a` (feat)

## Files Created/Modified

- `ios/Bindings/mango_core.swift` - Regenerated with toolsEnabled field in ConversationSummary and SetConversationToolsEnabled in AppAction
- `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` - Regenerated with toolsEnabled and SetConversationToolsEnabled
- `ios/Mango/Mango/ChatView.swift` - Added Tools Toggle button in ToolbarItem(.primaryAction); onSetToolsEnabled callback
- `ios/Mango/Mango/ContentView.swift` - Wired onSetToolsEnabled to dispatch setConversationToolsEnabled
- `android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt` - Added onDispatchAction param; tools toggle TextButton in ChatTopBar
- `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` - Passed onDispatchAction to ChatScreen
- `desktop/iced/src/main.rs` - Added ToggleConvToolsEnabled message variant and handler
- `desktop/iced/src/views/chat.rs` - Added tools_btn in chat header row reading tools_enabled from state

## Decisions Made

- iOS: used `onSetToolsEnabled: (Bool) -> Void` callback pattern consistent with `onSetSystemPrompt` and other ChatView callbacks. The actual `setConversationToolsEnabled` dispatch lives in ContentView.swift to keep ChatView dumb.
- Android: used generic `onDispatchAction: (AppAction) -> Unit` instead of a dedicated `onToggleTools` callback to avoid parameter proliferation; follows existing capability bridge patterns.
- Desktop: tools button shows "Tools [ON]" with accent background when active, "Tools" with surface background when off — consistent with docs_btn visual pattern.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Package name for desktop is `mango-desktop` (hyphen) not `mango_desktop` (underscore) — corrected on first attempt.

## Known Stubs

None - all data is wired from `ConversationSummary.toolsEnabled` in live state; no placeholder values.

## Next Phase Readiness

- Task 2 (checkpoint:human-verify) requires user to visually confirm the tools toggle renders and functions on desktop.
- After checkpoint approval, Plan 04 can proceed.

---
*Phase: 27-add-optional-tool-use-to-chat*
*Completed: 2026-04-07*
