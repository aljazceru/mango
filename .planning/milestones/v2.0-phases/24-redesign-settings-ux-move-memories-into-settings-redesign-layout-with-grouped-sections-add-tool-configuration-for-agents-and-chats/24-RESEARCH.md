# Phase 24: Redesign Settings UX — Research

**Researched:** 2026-04-05
**Domain:** Native UI (SwiftUI / Jetpack Compose / iced) — settings screen redesign across three platforms
**Confidence:** HIGH

## Summary

Phase 24 is a pure UI redesign phase touching three platform UI layers and the Rust core settings API. No new persistence schema is required — all settings keys already exist in the `settings` table. The work is scoped to three areas:

1. **Move Memories into Settings.** Currently "Memories" is a top-level navigation destination reachable from the home toolbar. The redesign moves it into the Settings screen as a section, accessible via a navigation link/row inside Settings rather than as a peer screen. The `Screen::Memories` route stays in place; only the entry point changes (from toolbar button to Settings row).

2. **Redesign Settings layout with grouped sections.** The current flat list (Providers → Defaults → Appearance → Advanced) gains new sections: Memory, Tools. Sections become clearly delineated with header labels and logical grouping. The toolbar-level "Memories" button is removed from the home screen.

3. **Add tool configuration for agents and chats.** The Brave Search API key is currently hard-coded via the settings table key `brave_api_key`, but there is no UI to set it. Phase 24 adds a "Tools" section in Settings that lets the user enter the Brave Search API key. Optionally, tool-level enable/disable toggles for agent tools (web search, URL fetch, file ops, math) can be exposed here.

**Primary recommendation:** Add `SetBraveApiKey` AppAction and surface `brave_api_key` in AppState so the UI can display the current key (masked). Add a Tools section to all three Settings screens. Remove the standalone Memories toolbar button and replace it with a NavigationLink/row inside the Settings Memory section. No new database migrations needed.

## User Constraints (from CONTEXT.md)

No CONTEXT.md exists for Phase 24. This is a new phase with no prior discuss session. All design decisions are at Claude's discretion.

## Standard Stack

All implementation is UI-only on existing libraries. No new dependencies.

### Core (existing, no changes)
| Library | Version | Purpose |
|---------|---------|---------|
| SwiftUI | iOS 17+ | iOS settings UI |
| Jetpack Compose / Material3 | API 28+ | Android settings UI |
| iced | 0.14.x (desktop Cargo.lock pinned) | Desktop settings UI |
| mango_core (Rust) | 0.1.0 | AppState / AppAction / Screen via UniFFI |

### No new dependencies required
All settings persistence uses existing `persistence::queries::get_setting` / `set_setting` helpers. The only Rust change is a new `AppAction::SetBraveApiKey` variant (and exposing `brave_api_key` in `AppState`) — same pattern as the existing `SetAttestationInterval` and `SetGlobalSystemPrompt` actions added in prior phases.

## Architecture Patterns

### Existing Settings Pattern (from code)

All three platforms follow the same structure:

```
Settings screen
  PROVIDERS section     — provider cards (existing)
  DEFAULTS section      — default model picker, default instructions (existing)
  APPEARANCE section    — theme picker (existing)
  [Advanced]            — collapsible custom provider + attestation interval (existing)
```

The redesign target:

```
Settings screen
  PROVIDERS section     — unchanged
  DEFAULTS section      — unchanged
  MEMORY section        — NEW: "View Memories" navigation row + count badge
  TOOLS section         — NEW: Brave Search API key field, optional per-tool toggles
  APPEARANCE section    — unchanged
  [Advanced]            — unchanged
```

### Pattern 1: Navigation Row Inside Settings (iOS)

iOS already does this for Documents — there is no dedicated Documents toolbar button; documents are accessed from the chat attachment overlay. The Memories section row pattern follows the iOS `NavigationLink` idiom inside a `List/Section`:

```swift
// Source: existing SettingsView.swift structure + ContentView.swift routing
Section("Memory") {
    NavigationLink(destination: EmptyView()) { // or dispatch-based navigation
        HStack {
            Label("Memories", systemImage: "brain")
            Spacer()
            Text("\(appState.memories.count)")
                .foregroundStyle(.secondary)
                .font(.caption)
        }
    }
    .simultaneousGesture(TapGesture().onEnded {
        appManager.dispatch(.pushScreen(screen: .memories))
    })
}
```

Because the app uses a custom push-pop router (not SwiftUI NavigationStack link-based navigation), the correct pattern is a `Button` row inside the `List` that dispatches `.pushScreen(screen: .memories)` — same as how the toolbar button currently works. NavigationLink is not used for cross-screen navigation in this architecture.

### Pattern 2: Settings Row → Push Screen (all platforms)

The existing dispatch model for screen transitions:
- iOS: `appManager.dispatch(.pushScreen(screen: .memories))`
- Android: `onDispatch(AppAction.PushScreen(screen = Screen.Memories))`
- Desktop: `Message::DispatchAction(AppAction::PushScreen { screen: Screen::Memories })`

The Memory section row in Settings is a tappable row that dispatches this action. The `Screen::Memories` route is unchanged.

### Pattern 3: Brave API Key Setting (Rust core)

The Brave Search API key is already persisted in the settings table and loaded at dispatch time in the agent loop:

```rust
// Source: rust/src/lib.rs ~line 1673
let brave_api_key = persistence::queries::get_setting(
    actor_state.db.conn(),
    "brave_api_key",
).ok().flatten().unwrap_or_default();
```

The pattern for adding a new `SetBraveApiKey` action follows `SetGlobalSystemPrompt` exactly (added in quick task 260403-ft1):

```rust
// AppState addition
pub brave_api_key_set: bool,  // don't expose the raw key — just whether one is configured

// AppAction addition
SetBraveApiKey { api_key: String },

// Actor handler
AppAction::SetBraveApiKey { api_key } => {
    let trimmed = api_key.trim().to_string();
    let _ = persistence::queries::set_setting(
        actor_state.db.conn(),
        "brave_api_key",
        &trimmed,
    );
    actor_state.app_state.brave_api_key_set = !trimmed.is_empty();
}

// Startup load
let brave_api_key_set = persistence::queries::get_setting(
    actor_state.db.conn(), "brave_api_key",
).ok().flatten().map(|k| !k.trim().is_empty()).unwrap_or(false);
actor_state.app_state.brave_api_key_set = brave_api_key_set;
```

**Key decision:** Do NOT expose the raw API key in `AppState` — it would cross the UniFFI boundary and appear in state snapshots. Expose only a `bool` indicating whether a key is configured (for the "Configured / Not set" badge in the UI). The text field in Settings is always blank on load; the user re-enters it to update.

### Pattern 4: Memory Count in AppState

The `AppState.memories` field is a `Vec<MemorySummary>` that is populated only when `Screen::Memories` is active (lazy load via `ListMemories` action). A count badge on the Settings Memory row requires either:
- A separate `memory_count: u64` field loaded at startup, OR
- Loading memories eagerly on startup

The simpler option is adding `memory_count: u64` to `AppState`, populated at startup with a `SELECT COUNT(*) FROM memories` query. This does not require loading the full memory list.

### Anti-Patterns to Avoid

- **Don't expose raw API keys in AppState.** AppState is serialized and passed across the UniFFI boundary. Secrets should stay in the Rust actor's private ActorState, or in the platform keychain. For Brave API key, use a `brave_api_key_set: bool` indicator.
- **Don't add a new Screen variant.** The redesign is a layout change within the existing `Screen::Settings` render path. No new routes.
- **Don't remove `Screen::Memories`.** Memories still need their own full-screen view; the route stays. Only the entry point (toolbar button → Settings row) changes.
- **Don't duplicate memory loading.** The `ListMemories` action loads memories on demand. The Settings row should show a count (via `memory_count`), not the full list.
- **Don't use multiline text_input in iced for API key.** Use `text_input(...).secure(true)` for the Brave key field — same as the provider API key fields.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| Settings persistence | Custom file/prefs | Existing `get_setting`/`set_setting` in `persistence::queries` |
| API key storage for Brave | Keychain entry | Simple settings table entry (Brave key is not a high-value secret — it rate-limits per key but is not a user credential) |
| Navigation | New Screen variant | Existing `PushScreen` dispatch with `Screen::Memories` |
| Memory count | Load full memory list | `SELECT COUNT(*) FROM memories` query, new `memory_count: u64` in AppState |

## Common Pitfalls

### Pitfall 1: Brave API Key in AppState
**What goes wrong:** Developer adds `brave_api_key: String` to `AppState`, exposing it in state snapshots and across UniFFI.
**Why it happens:** Following the `global_system_prompt` pattern too literally.
**How to avoid:** Use `brave_api_key_set: bool` in AppState. Keep the raw key in the settings table only. The UI text field starts empty; user enters the key to save/update it.
**Warning signs:** If the view needs to "pre-fill" the Brave key field on screen load.

### Pitfall 2: Memory Count Stale After Delete/Add
**What goes wrong:** `memory_count` in AppState is loaded at startup but not updated when memories are added or deleted.
**Why it happens:** Startup load pattern doesn't re-run on each memory mutation.
**How to avoid:** Update `actor_state.app_state.memory_count` in the `DeleteMemory` and `MemoryExtractionComplete` handlers (decrement/increment). Or, simpler: reload count after each mutation with a COUNT query.
**Warning signs:** Badge shows wrong number after the user deletes a memory and returns to Settings.

### Pitfall 3: "Memories" Toolbar Button Left on Home Screen
**What goes wrong:** The Memories row is added to Settings but the old toolbar button is not removed, creating two entry points.
**Why it happens:** Incomplete feature migration across three platforms.
**How to avoid:** The plan must explicitly remove the `Memories` button from the home toolbar on all three platforms (iOS ContentView.swift, Android MainApp.kt ConversationListScreen topBarActions, Desktop home.rs).
**Warning signs:** Two "Memories" navigation paths in the home view.

### Pitfall 4: iced text_input for multiline Brave key
**What goes wrong:** Using `text_input` for a potentially long Brave key — iced's `text_input` is single-line.
**Why it happens:** API keys don't need multiline. This is actually fine — iced's `text_input(...).secure(true)` handles it. The pitfall would be trying to add a multiline input, which iced 0.14 doesn't support.
**How to avoid:** Use `.secure(true)` on `text_input` as done for provider API keys.

### Pitfall 5: Android Memory Screen import missing after routing change
**What goes wrong:** After removing the Memories entry from the toolbar and making it Settings-internal, the `Screen.Memories` case in `MainApp.kt`'s `when(screen)` block is left without a corresponding visible entry.
**Why it happens:** The route still exists in the `when` block; no import error.
**How to avoid:** This is actually fine — `Screen.Memories` still renders `MemoryScreen` when navigated to via Settings. No change needed in `MainApp.kt` routing logic.

## Code Examples

### Memory Count Query Pattern
```rust
// Source: existing queries.rs pattern + lib.rs startup pattern
let memory_count: u64 = actor_state.db.conn()
    .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get::<_, i64>(0))
    .unwrap_or(0) as u64;
actor_state.app_state.memory_count = memory_count;
```

### Settings Section Row Pattern — iOS
```swift
// Source: existing SettingsView.swift Section + Button pattern
Section("Memory") {
    Button(action: { appManager.dispatch(.pushScreen(screen: .memories)) }) {
        HStack {
            Label("Memories", systemImage: "brain")
                .foregroundStyle(.primary)
            Spacer()
            if appState.memoryCount > 0 {
                Text("\(appState.memoryCount)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Image(systemName: "chevron.right")
                .font(.caption)
                .foregroundStyle(.tertiary)
        }
    }
}
```

### Settings Section Row Pattern — Android
```kotlin
// Source: existing SettingsScreen.kt Card pattern
item {
    Spacer(Modifier.height(16.dp))
    Text(
        "MEMORY",
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)
    )
    Card(
        modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
        shape = RoundedCornerShape(10.dp),
        onClick = { onDispatch(AppAction.PushScreen(screen = Screen.Memories)) }
    ) {
        Row(
            modifier = Modifier.padding(14.dp).fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("Memories", style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
            Spacer(Modifier.weight(1f))
            if (appState.memoryCount > 0L) {
                Text(appState.memoryCount.toString(), style = MaterialTheme.typography.labelSmall, color = ...)
            }
        }
    }
}
```

### Settings Section Row Pattern — Desktop (iced)
```rust
// Source: existing views/settings.rs section_header + container patterns
// Memory section row
let memory_row = container(
    button(
        row![
            text("Memories").size(14).color(vc.text),
            iced::widget::Space::new().width(Length::Fill),
            text(format!("{}", state.memory_count)).size(12).color(vc.muted),
        ].align_y(Alignment::Center).spacing(8)
    )
    .on_press(Message::DispatchAction(AppAction::PushScreen { screen: Screen::Memories }))
    .padding(Padding::from([10u16, 14]))
    .style(move |_, _| button::Style {
        background: Some(Background::Color(vc.card)),
        border: Border { radius: 8.0.into(), color: vc.border, width: 1.0 },
        ..Default::default()
    })
)
.width(Length::Fill);
```

### Brave API Key UI Pattern — Desktop (iced)
```rust
// Source: existing views/settings.rs provider key_input pattern
let brave_key_input = text_input(
    if *brave_api_key_set { "Key configured — enter new key to update" } else { "Brave Search API Key" },
    brave_api_key_input,
)
.secure(true)
.on_input(Message::SettingsBraveApiKeyChanged)
.size(13)
.padding(Padding::from([7u16, 10]));

let brave_save_el = action_btn(
    "Save",
    Message::SettingsSaveBraveApiKey,
    !brave_api_key_input.trim().is_empty(),
    vc,
);
```

## Phase Requirements (proposed)

Since REQUIREMENTS.md has no entries for Phase 24, these are proposed:

| ID | Description |
|----|-------------|
| SET-01 | Settings screen has a Memory section with a row that navigates to the Memories screen |
| SET-02 | Home screen no longer shows a standalone "Memories" toolbar button |
| SET-03 | Settings screen has a Tools section with a Brave Search API key field |
| SET-04 | Brave Search API key can be saved via the Tools section and persists across restarts |
| SET-05 | Settings sections are clearly grouped: Providers / Defaults / Memory / Tools / Appearance / Advanced |
| SET-06 | Memory section row shows a count badge of stored memories |
| SET-07 | All changes apply on iOS, Android, and Desktop |

## Work Breakdown

### Rust Core (rust/src/lib.rs)
1. Add `memory_count: u64` to `AppState` (loaded at startup via COUNT query, updated on delete/add)
2. Add `brave_api_key_set: bool` to `AppState` (loaded at startup)
3. Add `AppAction::SetBraveApiKey { api_key: String }` variant
4. Add handler for `SetBraveApiKey` (persist to settings table, update `brave_api_key_set` in state)
5. Update `DeleteMemory` handler to decrement `memory_count`
6. Update `MemoryExtractionComplete` handler to increment `memory_count`

### Desktop (desktop/iced/src/main.rs + views/settings.rs)
1. Add `settings_brave_api_key: String` and `settings_brave_api_key_set: bool` local state fields
2. Add `Message::SettingsBraveApiKeyChanged(String)` and `Message::SettingsSaveBraveApiKey`
3. Remove `Message::OpenMemories` from home view toolbar (or repurpose)
4. Update `views/settings::view()` signature to pass `brave_api_key_input`, `brave_api_key_set`, `memory_count`
5. Add MEMORY section with navigation row in `views/settings.rs`
6. Add TOOLS section with Brave key field in `views/settings.rs`
7. Remove Memories navigation from home view toolbar in `views/home.rs`

### iOS (ios/Mango/Mango/)
1. Add Memory section to `SettingsView.swift` with navigation row
2. Add Tools section to `SettingsView.swift` with Brave API key SecureField + save button
3. Remove "Memories" toolbar button from `ContentView.swift` homeView
4. Add `@State var braveApiKeyInput: String` and save logic in SettingsView

### Android (android/app/src/main/java/dev/disobey/mango/ui/)
1. Add MEMORY section to `SettingsScreen.kt` with navigation Card
2. Add TOOLS section to `SettingsScreen.kt` with Brave API key OutlinedTextField + save button
3. Remove "Memories" TextButton from `MainApp.kt` topBarActions in the Home screen

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| Memories as peer screen accessible from home toolbar | Memories as sub-section of Settings | Cleaner home screen, logical grouping |
| Brave API key only configurable via environment/settings table directly | Brave API key in Tools section of Settings | Users can configure web search without developer access |
| Flat settings list | Grouped sections with named headers | More navigable as feature count grows |

## Environment Availability

Step 2.6: SKIPPED — Phase is code/config changes only. No external tool dependencies beyond existing Rust/Swift/Kotlin build toolchain.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (cargo test) |
| Config file | none (inline `#[test]` modules) |
| Quick run command | `cargo test -p mango_core settings 2>&1 \| tail -10` |
| Full suite command | `cargo test -p mango_core 2>&1 \| tail -10` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SET-01 | Memory section row navigates to Screen::Memories | manual-only | — | N/A |
| SET-02 | Home toolbar no longer shows Memories button | manual-only | — | N/A |
| SET-03 | Tools section renders Brave API key field | manual-only | — | N/A |
| SET-04 | SetBraveApiKey persists to settings table and updates brave_api_key_set | unit | `cargo test -p mango_core test_brave_api_key_persists` | ❌ Wave 0 |
| SET-05 | Sections present in settings view | manual-only | — | N/A |
| SET-06 | memory_count loaded at startup and updated on delete | unit | `cargo test -p mango_core test_memory_count` | ❌ Wave 0 |
| SET-07 | Cross-platform compile check | build | `cargo check -p mango-desktop` | ✅ exists |

**Note:** UI-only requirements (SET-01, SET-02, SET-03, SET-05, SET-07) are verified by build success and visual review. Rust core additions (SET-04, SET-06) can have unit tests following the existing pattern in `rust/src/tests/memory.rs` and `rust/src/tests/persistence.rs`.

### Sampling Rate
- **Per task commit:** `cargo check -p mango_core && cargo check -p mango-desktop`
- **Per wave merge:** `cargo test -p mango_core 2>&1 | tail -5` (full suite, currently 231 tests)
- **Phase gate:** Full suite green + visual review on all three platforms

### Wave 0 Gaps
- [ ] `rust/src/tests/settings.rs` or extension of `rust/src/tests/persistence.rs` — covers SET-04 (brave_api_key_set round-trip) and SET-06 (memory_count at startup and after delete)

## Open Questions

1. **Tool enable/disable toggles**
   - What we know: The phase description says "add tool configuration for agents and chats" — this could mean just the API key, or also per-tool toggle switches (enable/disable web search, URL fetch, file ops, math for chat vs agent contexts)
   - What's unclear: Scope of "tool configuration" — API key only, or full enable/disable matrix?
   - Recommendation: Start with Brave API key only (directly unblocks web search for users). Per-tool toggles can be added as a follow-on if needed. The settings table already supports arbitrary key-value pairs, so adding `tool_web_search_enabled`, etc. is trivial later.

2. **Memory count update on background extraction**
   - What we know: `MemoryExtractionComplete` fires when background extraction finishes; it increments a vector index and inserts to SQLite
   - What's unclear: Whether `memory_count` should be incremented there or just reloaded from DB
   - Recommendation: Reload via COUNT query in the `MemoryExtractionComplete` handler (simpler, avoids off-by-one from batch extraction).

3. **Android `Card` onClick parameter availability**
   - What we know: Jetpack Compose Material3 `Card` has an `onClick` parameter making it tappable
   - What's unclear: Whether the existing Android Material3 version in the project supports this
   - Recommendation: Check the existing `SettingsScreen.kt` patterns — it uses `Card` with `modifier` only. Use a `Row` with `clickable` modifier inside a `Card` as a safe fallback, consistent with existing patterns.

## Sources

### Primary (HIGH confidence)
- `/home/lio/g/confidential-app/ios/Mango/Mango/SettingsView.swift` — current iOS Settings structure
- `/home/lio/g/confidential-app/android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt` — current Android Settings structure
- `/home/lio/g/confidential-app/desktop/iced/src/views/settings.rs` — current Desktop Settings structure
- `/home/lio/g/confidential-app/rust/src/lib.rs` — AppAction, AppState, Screen enums; actor loop patterns
- `/home/lio/g/confidential-app/.planning/quick/260403-ft1-add-default-instructions-setting-in-sett/260403-ft1-PLAN.md` — reference implementation for adding a settings field (SetGlobalSystemPrompt pattern)
- `/home/lio/g/confidential-app/ios/Mango/Mango/ContentView.swift` — current home screen toolbar showing Memories button
- `/home/lio/g/confidential-app/android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` — current Android routing and home toolbar

### Secondary (MEDIUM confidence)
- `/home/lio/g/confidential-app/rust/src/tests/memory.rs` — test patterns for memory operations

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new libraries, existing patterns only
- Architecture: HIGH — patterns directly observed from codebase
- Pitfalls: HIGH — derived from actual code audit of all three platforms
- Scope definition: MEDIUM — "tool configuration" scope is ambiguous; recommended interpretation stated

**Research date:** 2026-04-05
**Valid until:** 2026-05-05 (stable UX domain, 30 days)
