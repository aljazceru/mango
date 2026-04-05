# Phase 24: Redesign Settings UX - Context

**Gathered:** 2026-04-05
**Status:** Ready for planning
**Source:** Auto-mode (recommended defaults selected)

<domain>
## Phase Boundary

Redesign the Settings screen across all three platforms (iOS, Android, Desktop) to add Memory and Tools sections. Move the Memories entry point from the home toolbar into Settings as a navigation row with count badge. Add a Brave Search API key field in the new Tools section. The Settings screen section order becomes: Providers → Defaults → Memory → Tools → Appearance → Advanced. No new database migrations. No new Screen variants.

</domain>

<decisions>
## Implementation Decisions

### Tool Configuration Scope
- **D-01:** Phase 24 adds only the Brave Search API key field in the Tools section. No per-tool enable/disable toggles in this phase.
- **D-02:** Per-tool toggles (enable/disable web search, URL fetch, file ops, math) are deferred to a future phase. The settings table already supports arbitrary key-value pairs for this.

### Memory Count Strategy
- **D-03:** Add `memory_count: u64` to `AppState`, loaded at startup via `SELECT COUNT(*) FROM memories`.
- **D-04:** Re-query COUNT(*) on each mutation (`DeleteMemory`, `MemoryExtractionComplete`) rather than increment/decrement. Simpler and avoids off-by-one from batch extraction.

### Section Grouping
- **D-05:** Use platform-native section headers only — iOS `Section("Memory")` / `Section("Tools")` labels, Android uppercase `Text` headers with `labelSmall` style, Desktop `section_header()` helper.
- **D-06:** No extra visual separators or dividers beyond what the platform natively provides between sections.
- **D-07:** Section order locked: PROVIDERS → DEFAULTS → MEMORY → TOOLS → APPEARANCE → ADVANCED (all platforms).

### Home Toolbar Migration
- **D-08:** Remove only the "Memories" button from the home screen toolbar on all three platforms.
- **D-09:** Documents, Agents, and Settings toolbar buttons remain unchanged.
- **D-10:** `Screen::Memories` route stays in place — only the entry point changes (toolbar → Settings row).

### Brave API Key UX
- **D-11:** `brave_api_key_set: bool` in `AppState` — never expose the raw key across UniFFI boundary.
- **D-12:** Text field always starts blank on screen load. Placeholder changes based on whether key is configured.
- **D-13:** No toast on successful save — field clears and placeholder switches to "Key configured — enter new key to update".
- **D-14:** Save button enabled only when input field is non-empty after trim.

### Memory Section Row
- **D-15:** Single tappable row labeled "Memories" that dispatches `PushScreen(screen: .memories)`.
- **D-16:** Count shown as muted numeral (not a badge pill) when memory_count > 0. Hidden when count is 0.
- **D-17:** Chevron/arrow indicator on the right side of the row.

### Rust Core
- **D-18:** New `AppAction::SetBraveApiKey { api_key: String }` — handler persists to settings table via `set_setting`, updates `brave_api_key_set` in state.
- **D-19:** Pattern follows `SetGlobalSystemPrompt` exactly (quick task 260403-ft1 reference implementation).

### Claude's Discretion
- Memory section icon choice on iOS (brain, bookmark, or other SF Symbol)
- Exact border radius and padding values within platform conventions
- Whether "Configured" text appears next to the "Web Search" sub-header or as a separate badge
- Desktop Message variant naming (`SettingsBraveApiKeyChanged` vs alternatives)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Settings UI (current state — modify these files)
- `ios/Mango/Mango/SettingsView.swift` — iOS Settings structure (add Memory + Tools sections)
- `android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt` — Android Settings structure
- `desktop/iced/src/views/settings.rs` — Desktop Settings structure
- `desktop/iced/src/main.rs` — Desktop Message enum, state fields, update loop

### Home Screen (remove Memories toolbar button)
- `ios/Mango/Mango/ContentView.swift` — iOS home toolbar
- `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` — Android home toolbar
- `desktop/iced/src/views/home.rs` — Desktop home toolbar

### Rust Core (add AppState fields + action)
- `rust/src/lib.rs` — AppState, AppAction, Screen enum, actor loop handlers
- `rust/src/persistence/queries.rs` — get_setting/set_setting helpers

### Reference Implementation
- `.planning/quick/260403-ft1-add-default-instructions-setting-in-sett/260403-ft1-PLAN.md` — SetGlobalSystemPrompt pattern (exact reference for adding SetBraveApiKey)

### UI Design Contract
- `.planning/phases/24-redesign-settings-ux-move-memories-into-settings-redesign-layout-with-grouped-sections-add-tool-configuration-for-agents-and-chats/24-UI-SPEC.md` — Visual/interaction spec (authoritative for all platforms)

### Phase Research
- `.planning/phases/24-redesign-settings-ux-move-memories-into-settings-redesign-layout-with-grouped-sections-add-tool-configuration-for-agents-and-chats/24-RESEARCH.md` — Architecture patterns, code examples, anti-patterns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `persistence::queries::get_setting/set_setting` — Settings table read/write (used by all existing settings actions)
- `SetGlobalSystemPrompt` pattern in lib.rs — Exact template for SetBraveApiKey action handler
- iOS `Section` + `Button` pattern in SettingsView.swift — Template for Memory section row
- Android `Card` + `Modifier.clickable` pattern in SettingsScreen.kt — Template for Memory row
- Desktop `section_header` + `container(button(...))` pattern in views/settings.rs — Template for Memory row

### Established Patterns
- `AppState` boolean flags for key presence: `has_api_key: bool` on BackendSummary (same pattern for `brave_api_key_set`)
- `PushScreen` dispatch for navigation across all platforms
- Lazy-loaded lists (memories, conversations) with eager count fields for badges

### Integration Points
- `lib.rs` actor loop — new SetBraveApiKey handler, memory_count updates in DeleteMemory and MemoryExtractionComplete handlers
- `lib.rs` startup — load memory_count and brave_api_key_set from DB
- Three platform settings screens — add 2 new sections between DEFAULTS and APPEARANCE
- Three platform home screens — remove Memories toolbar button

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches following the UI-SPEC contract.

</specifics>

<deferred>
## Deferred Ideas

- **Per-tool enable/disable toggles** — Allow users to toggle individual agent tools (web search, URL fetch, file ops, math) on/off. Trivial to add later via settings table key-value pairs. Not needed for MVP tool configuration.
- **Memory categories/tags display in Settings** — Show memory breakdown by category in the Memory section row description.

</deferred>

---

*Phase: 24-redesign-settings-ux*
*Context gathered: 2026-04-05 via auto mode*
