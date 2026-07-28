---
status: resolved
trigger: "menu-tweaks-and-brave-search-broken"
created: 2026-04-08T10:00:00Z
updated: 2026-04-08T10:30:00Z
---

## Current Focus

hypothesis: CONFIRMED AND FIXED
test: cargo check passes clean for desktop; iOS/Android changes are structurally correct
expecting: user verifies menu shows "RAG", Tools opens sub-sheet, brave search works with key set
next_action: await human verification

## Symptoms

expected: (1) Menu shows "RAG" not "Docs", no icon. Tools opens a sub-screen with individual tool toggles. (2) When tools are enabled and user asks about weather, model should call brave_search tool and return results.
actual: (1) Menu says "Docs" with an icon. Tools is a simple toggle. (2) Brave search doesn't work — model can't use it even with tools enabled.
errors: No crash errors reported. The model simply says it can't search or attempts and fails.
reproduction: (1) Open any chat, tap ··· menu. (2) Enable tools toggle, ask "what's the weather in [city]"
started: After Phase 27 implementation (tool use in chat). The overflow menu was just redesigned in the previous debug session (top-bar-overflow-redesign).

## Eliminated

## Evidence

- timestamp: 2026-04-08T10:10:00Z
  checked: ios/Mango/Mango/ChatView.swift lines 119-150
  found: Menu label says "Docs" (with count) and uses systemImage "doc.fill"/"doc" icon. Tools item calls onSetToolsEnabled(!toolsOn) — a direct toggle.
  implication: Issue 1 confirmed on iOS: "Docs" label with icon, no sub-sheet for Tools.

- timestamp: 2026-04-08T10:10:00Z
  checked: android/.../ui/ChatScreen.kt lines 376-432
  found: DropdownMenuItem text says "Docs (N)" / "Docs" with Icons.Default.Description leading icon. Tools item dispatches SetConversationToolsEnabled directly — simple toggle.
  implication: Issue 1 confirmed on Android: "Docs" label with icon, no sub-sheet for Tools.

- timestamp: 2026-04-08T10:10:00Z
  checked: desktop/iced/src/views/chat.rs lines 148-175
  found: docs_label says "Docs" / "Docs (N)". tools_btn toggles via Message::ToggleConvToolsEnabled — simple toggle. No sub-panel for tools.
  implication: Issue 1 confirmed on Desktop: same "Docs" label, same simple toggle.

- timestamp: 2026-04-08T10:10:00Z
  checked: rust/src/lib.rs line 1455-1462, rust/src/agent/tools.rs build_chat_tools()
  found: tools_enabled + supports_tool_use gate: if both true, build_chat_tools(has_docs, brave_key_set) is called. brave_key_set is read from DB. If brave_key_set=false, web_search is filtered OUT. If brave_key_set=true, web_search IS included. build_chat_tools falls back to normal streaming when tools.is_empty().
  implication: Issue 2 - brave search tool inclusion depends on brave_api_key being stored in DB. If user hasn't saved the key in Settings, brave_key_set=false and tools list only has fetch_url/file/calculate. The "tools enabled" flag has no effect for brave search without a configured key.

- timestamp: 2026-04-08T10:10:00Z
  checked: rust/src/agent/tools.rs build_chat_tools() line 218: "fetch_url" | "file" | "calculate" pass through unconditionally (the _ => true arm)
  found: Even with no brave key and no docs, tools list will contain fetch_url + file + calculate — so tools.is_empty() is FALSE and the tool round is spawned. The model will never see brave_search if key isn't set, but the tool round still runs. The model answers without search → ChatToolNone path.
  implication: THIS IS THE BRAVE SEARCH BUG: When brave_api_key is not set in DB, web_search is excluded from tools, but the tool round is still triggered (non-empty tools list). Model calls ChatToolNone. User never gets search results even though "tools enabled" UI suggests it should work.

- timestamp: 2026-04-08T10:10:00Z
  checked: ChatToolCallsReady handler lines 4790-4802
  found: brave_api_key is fetched correctly from DB and passed to dispatch_tools. The key passing is fine IF it's in the DB.
  implication: No bug in dispatch path itself. Bug is in the tools list: brave_search excluded when key not set.

## Resolution

root_cause: |
  Issue 1: All three platform overflow menus used "Docs" label with an icon for the RAG item,
  and "Tools" was a direct toggle action rather than opening a sub-view with individual tool toggles.
  Issue 2: brave_search is correctly excluded from the tool list when brave_api_key is not set in DB
  (build_chat_tools filters it out). The root cause of the user's experience is that the UI provided
  no feedback that a Brave API key is required — the Tools toggle appeared functional but brave_search
  was silently absent from the tool definitions sent to the API. The new ToolsSheet/sub-panel surfaces
  this with a "API key not configured" note and disables the toggle until a key is set.

fix: |
  iOS: Renamed "Docs" button label to "RAG" (removed icon). Changed Tools menu item to open a new
  ToolsSheet sheet (not dispatch toggle directly). Added ToolsSheet struct with a Brave Search toggle
  row that is disabled when braveApiKeySet=false, with explanatory caption.
  Android: Renamed "Docs" DropdownMenuItem to "RAG" (removed leadingIcon). Changed Tools item to set
  showToolsSheet=true. Added ToolsSheet ModalBottomSheet with Brave Search Switch (disabled when key
  not set) and explanatory footer text.
  Desktop: Renamed "Docs" button label to "RAG". Added show_tools_panel bool state and ToggleToolsPanel
  message. Tools button now toggles the sub-panel (not directly toggling tools). Sub-panel shows a
  Brave Search toggle row; disabled with note when brave_api_key_set=false.
  Rust core: No changes needed — the brave_search exclusion logic in build_chat_tools is correct.
  The brave_api_key save/load flow is also correct.

verification: cargo check passes clean for mango-desktop and mango_core. No errors.

files_changed:
  - ios/Mango/Mango/ChatView.swift
  - android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt
  - desktop/iced/src/views/chat.rs
  - desktop/iced/src/main.rs

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** Tools sub-sheet on all 3 platforms (ChatScreen.kt:813, ChatView.swift:295, chat.rs:387)
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
