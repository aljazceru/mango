# Phase 26: Settings Submenus and Organization — Research

**Researched:** 2026-04-05
**Domain:** Native UI (SwiftUI / Jetpack Compose / iced) — settings screen sub-navigation and organization
**Confidence:** HIGH

## Summary

Phase 26 builds on the settings structure established in Phase 24 (grouped sections) and Phase 25 (memories toggle). The current settings screen has six sections in a flat scrollable list: PROVIDERS → DEFAULTS → MEMORY → TOOLS → APPEARANCE → Advanced. All six sections are always visible on scroll — there is no sub-navigation or drill-down.

The goal is to reorganize settings with **collapsible sections or sub-screens**. The key architectural decision is whether to use:
1. **Sub-screens (navigation drill-down):** Each section becomes a tappable row that pushes a new screen — e.g., "Providers" → ProviderSettingsScreen. Requires new `Screen` variants.
2. **Collapsible sections (in-place accordion):** Sections have a header that toggles content visibility — e.g., expand/collapse PROVIDERS in place. No new screens needed. Advanced Settings already uses this pattern.
3. **Hybrid:** Some sections get sub-screens (dense sections like Providers, Defaults), others remain inline (lightweight sections like Appearance, Memory).

**Current implementation audit:** The existing settings screen is a long scrollable list. The only section that already uses the collapsible pattern is "Advanced Settings" — an `OutlinedButton` toggle on Android, a `DisclosureGroup` on iOS, and a text button on Desktop that shows/hides the advanced content. The PROVIDERS section is the most visually dense; the MEMORY section is the lightest (just a toggle + nav row).

**Primary recommendation:** Use the hybrid approach. Dense sections (PROVIDERS, DEFAULTS) get sub-screens accessible via a tappable summary row on the main settings screen — matching platform UX norms (iOS Settings app, Android Settings app). Lightweight sections (MEMORY, TOOLS, APPEARANCE) remain inline — they are simple enough that drill-down would feel heavyweight. Advanced stays collapsible. This approach requires new `Screen` variants in the Rust core.

## Standard Stack

All implementation uses existing libraries. No new dependencies.

### Core (existing, no changes)
| Library | Version | Purpose |
|---------|---------|---------|
| SwiftUI | iOS 17+ | iOS settings UI — `Section`, `NavigationLink`, `DisclosureGroup` |
| Jetpack Compose / Material3 | API 28+ | Android settings UI — `LazyColumn`, `Card`, `AnimatedVisibility` |
| iced | 0.14.x (pinned) | Desktop settings UI — `column![]`, `button`, `toggler` |
| mango_core (Rust) | 0.1.0 | AppState / AppAction / Screen via UniFFI |

### No new runtime dependencies required

## Current Settings Structure (Post Phase 25 — Confirmed from Source)

### iOS (`ios/Mango/Mango/SettingsView.swift`)

```swift
List {
    providersSection     // Section("Providers") — provider cards + enable/remove
    defaultsSection      // Section("Defaults") — model picker + default instructions
    memorySection        // Section("Memory") — toggle + navigation row
    toolsSection         // Section("Tools") — Brave API key
    appearanceSection    // Section("Appearance") — theme picker
    advancedSection      // Section with DisclosureGroup — custom provider + attestation interval
}
```

State tracked: `@State presetKeys`, `showAdvanced`, `addName/addUrl/addApiKey/addTeeType`, `attestationIntervalInput`, `defaultModel`, `defaultInstructions`, `braveApiKeyInput`, `themePreference (@AppStorage)`.

### Android (`android/.../ui/SettingsScreen.kt`)

```kotlin
LazyColumn {
    item { /* PROVIDERS header */ }
    items(presets) { /* provider cards */ }
    item { /* DEFAULTS + model picker + instructions */ }
    item { /* MEMORY section: toggle Switch + Memories nav row */ }
    item { /* TOOLS section: Brave API key */ }
    item { /* APPEARANCE section: theme dropdown */ }
    item { /* Advanced toggle OutlinedButton */ }
    item { /* AnimatedVisibility advanced content */ }
    item { /* Spacer */ }
}
```

State tracked: `presetKeys`, `showAdvanced`, `addName/addUrl/addApiKey/showApiKey/addTeeType/teeExpanded`, `attestationInterval`, `defaultModelExp/defaultModel`, `defaultInstructions`, `braveApiKeyInput`, `themeExpanded/themeMode`.

### Desktop (`desktop/iced/src/views/settings.rs`)

```rust
column![
    section_header("PROVIDERS", vc.muted),
    providers_col,
    section_header("DEFAULTS", vc.muted),
    defaults_content,
    section_header("MEMORY", vc.muted),
    memory_toggle_row,
    memory_row,
    section_header("TOOLS", vc.muted),
    tools_body,
    section_header("APPEARANCE", vc.muted),
    appearance_row,
    divider(),
    adv_toggle_row,          // text button "Advanced Settings ▼/▲"
    advanced_body,           // conditionally rendered based on show_advanced
]
```

`view()` function signature: `state, is_dark, add_name, add_url, add_key, add_tee, default_model_input, preset_keys, show_advanced, attestation_interval_input, default_instructions, brave_api_key_input, theme_override`.

State tracked in `main.rs`: `settings_show_advanced: bool`, `settings_add_name/url/key/tee/preset_keys`, `settings_default_model/instructions`, `settings_brave_api_key`, `settings_attestation_interval`.

## Architecture Patterns

### Pattern 1: Sub-Screen Navigation (recommended for dense sections)

The app already supports push-pop navigation via `AppAction::PushScreen { screen: Screen::X }`. The Memories screen is an existing example of a settings section that became a sub-screen. The same pattern works for Providers and Defaults settings.

New Screen variants needed in Rust:
```rust
// rust/src/lib.rs — Screen enum additions
pub enum Screen {
    // ... existing variants ...
    SettingsProviders,   // Provider management sub-screen
    SettingsDefaults,    // Defaults sub-screen (model picker + instructions)
}
```

Each new screen requires:
1. A new `Screen::SettingsX` variant in the Rust enum
2. UniFFI bindings regenerated (iOS Swift + Android Kotlin)
3. A new SwiftUI View, Android `@Composable`, and iced view function
4. A case in each platform's routing switch/when block

**The content of these sub-screens is extracted from the current settings view — no new Rust core logic needed.**

### Pattern 2: Summary Row on Main Settings Screen (for sub-screen sections)

When a section becomes a sub-screen, the main settings screen shows a summary row instead of the full section content. This is the iOS Settings.app and Android Settings paradigm.

For PROVIDERS, the summary row shows: "X providers enabled" count + chevron.
For DEFAULTS, the summary row shows: current default model name (truncated) + chevron.

This requires a `provider_count: u64` field in AppState (or derive from `backends.iter().filter(|b| b.has_api_key).count()`). Since `backends` is already in AppState and is always up to date, **no new AppState fields are needed** — the UI can derive the count from `appState.backends`.

### Pattern 3: Collapsible Section (keep for Appearance and Advanced)

The existing Advanced section already uses this pattern on all three platforms:
- iOS: `DisclosureGroup` inside a `Section`
- Android: `AnimatedVisibility` with `expandVertically/shrinkVertically` triggered by `OutlinedButton`
- Desktop: conditional render based on `show_advanced: bool` state, toggled by `SettingsToggleAdvanced` message

Appearance has one item (theme picker) — too lightweight for a sub-screen. It stays inline.
Memory has two items (toggle + nav row) — stays inline (nav row already drills to Memories screen).
Tools has one item (Brave API key) — stays inline.

### Pattern 4: Existing Collapsible Pattern (iOS DisclosureGroup)

Already used for Advanced Settings on iOS:

```swift
// Source: ios/Mango/Mango/SettingsView.swift — advancedSection
Section {
    DisclosureGroup(isExpanded: $showAdvanced) {
        // content
    } label: {
        Label("Advanced Settings", systemImage: "gearshape.2")
            .font(.subheadline).fontWeight(.medium)
    }
}
```

This can be reused for any section that should collapse without navigating away.

### Pattern 5: Summary Row → PushScreen (iOS)

```swift
// Source: existing memorySection pattern in SettingsView.swift + AppManager dispatch
Button(action: { appManager.dispatch(.pushScreen(screen: .settingsProviders)) }) {
    HStack {
        Text("Providers")
            .font(.body).fontWeight(.medium)
            .foregroundStyle(.primary)
        Spacer()
        let enabledCount = appState.backends.filter { $0.hasApiKey }.count
        if enabledCount > 0 {
            Text("\(enabledCount) enabled")
                .font(.caption).foregroundStyle(.secondary)
        }
        Image(systemName: "chevron.right")
            .font(.caption).foregroundStyle(.tertiary)
    }
}
```

### Pattern 6: Summary Row → PushScreen (Android)

```kotlin
// Source: existing MEMORY section pattern in SettingsScreen.kt
Row(
    modifier = Modifier
        .clickable { onDispatch(AppAction.PushScreen(screen = Screen.SettingsProviders)) }
        .padding(16.dp)
        .fillMaxWidth(),
    verticalAlignment = Alignment.CenterVertically
) {
    Text("Providers", style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
    Spacer(Modifier.weight(1f))
    val enabledCount = appState.backends.count { it.hasApiKey }
    if (enabledCount > 0) {
        Text(
            "$enabledCount enabled",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(Modifier.width(8.dp))
    }
    Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, ..., modifier = Modifier.size(16.dp))
}
```

### Pattern 7: Summary Row → PushScreen (Desktop iced)

```rust
// Source: existing memory_row pattern in desktop/iced/src/views/settings.rs
let providers_summary_row = container(
    button(
        row![
            text("Providers").size(14).color(vc.text),
            iced::widget::Space::new().width(Length::Fill),
            text(format!("{} enabled", enabled_count)).size(12).color(vc.muted),
            text(">").size(12).color(vc.muted),
        ]
        .align_y(Alignment::Center)
        .spacing(8),
    )
    .on_press(Message::DispatchAction(AppAction::PushScreen {
        screen: Screen::SettingsProviders,
    }))
    .padding(Padding::from([8u16, 16]))
    .width(Length::Fill)
    .style(move |_, _| button::Style {
        background: Some(Background::Color(vc.card)),
        border: Border { radius: 8.0.into(), color: vc.border, width: 1.0 },
        ..Default::default()
    }),
)
.width(Length::Fill)
.padding(Padding::from([0u16, 16]));
```

### Recommended Settings Screen Structure After Phase 26

```
Settings (main screen — scrollable)
  [tappable row]  Providers →       "N enabled"
  [tappable row]  Defaults →        "model-name" or "None"
  MEMORY          (inline, unchanged)
    - Auto-extract Memories [toggle]
    - Memories → [nav row]
  TOOLS           (inline, unchanged)
    - Web Search / Brave API key
  APPEARANCE      (inline, unchanged)
    - Theme picker
  [collapsible]   Advanced Settings ▼
    - Re-attestation interval
    - Custom Provider form

SettingsProviders screen (new)
  Back button → Settings
  Provider cards (all existing content from current providersSection)

SettingsDefaults screen (new)
  Back button → Settings
  Default model picker + Default instructions (all existing content from defaultsSection)
```

### Anti-Patterns to Avoid

- **Don't create new Screen variants without updating UniFFI bindings.** After adding `Screen::SettingsProviders` and `Screen::SettingsDefaults` in Rust, UniFFI bindings must be regenerated on all platforms before the Swift/Kotlin code compiles. Phase 25 had a binding mismatch issue that needed a merge — this phase must follow the same sequencing (Rust core first, then bindings, then UI).
- **Don't add AppState fields for derived counts.** The providers enabled count is derivable from `state.backends.filter(|b| b.has_api_key).count()` — no new `provider_count: u64` field needed. Adding one would create another stale-count update problem like the old `memory_count` pattern required.
- **Don't duplicate state between sub-screens and main settings.** The sub-screen views receive the same `AppState` and `onDispatch` — they do not need separate state fields. The desktop `main.rs` local UI state (e.g., `settings_add_name`, `settings_add_key`) stays in `main.rs` and is passed to whichever view function needs it.
- **Don't wrap sub-screen views in a new NavigationStack on iOS.** The app uses a custom push-pop router. Sub-settings screens are new SwiftUI `View` structs rendered directly in `ContentView`'s switch, not nested NavigationStacks.
- **Don't forget to add routing cases for new screens.** Each new `Screen` variant needs a case in: iOS `ContentView.swift` switch, Android `MainApp.kt` `when(screen)`, Desktop `main.rs` view dispatch.
- **Don't use NavigationLink on iOS for these screens.** The project uses `appManager.dispatch(.pushScreen(screen: .settingsProviders))` — not NavigationLink. All three existing sub-screen navigations (Chat, Documents, Memories, Agents) use this pattern.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| Section collapse animation | Custom animation | iOS `DisclosureGroup`, Android `AnimatedVisibility(enter=expandVertically)`, Desktop conditional render |
| Navigation to sub-screen | New navigation stack | Existing `PushScreen` dispatch, existing routing in ContentView/MainApp/main.rs |
| Provider count badge | New `AppState.provider_count` field | Derive from `appState.backends.filter(hasApiKey).count()` |
| Default model summary | New `AppState.default_model_display` field | Use existing `appState.backends.firstOrNull { it.isActive }?.models?.firstOrNull()` or `defaultModel` local state |
| Back navigation from sub-screen | New `PopSettingsProviders` action | Existing `AppAction::PopScreen` |

## Common Pitfalls

### Pitfall 1: UniFFI Binding Mismatch
**What goes wrong:** New `Screen::SettingsProviders` variant added in Rust but bindings not regenerated. iOS/Android fail to compile with "expected X cases but found Y" or similar enum exhaustion error.
**Why it happens:** Swift and Kotlin bindings are generated from compiled Rust library, not from source. Must rerun `uniffi-bindgen` after the new Screen enum variant is compiled.
**How to avoid:** Plan must sequence: (1) add Screen variants to Rust + compile, (2) regenerate UniFFI bindings, (3) add routing cases to all platform UI files. This is the same sequence used in Phase 25.
**Warning signs:** Build error referencing `mango_core.swift` or `mango_core.kt` with missing case in when/switch on Screen enum.

### Pitfall 2: Missing Routing Case
**What goes wrong:** New `Screen::SettingsProviders` added, bindings regenerated, but ContentView.swift switch doesn't have a `case .settingsProviders` arm. iOS renders a blank screen or crashes.
**Why it happens:** The ContentView switch needs a case for every Screen variant — it's a non-exhaustive pattern match in Swift. Kotlin `when` will warn but not error if `else` branch exists.
**How to avoid:** When adding new Screen variants, audit all three routing entry points: iOS ContentView.swift, Android MainApp.kt `when(screen)`, Desktop main.rs screen dispatch.

### Pitfall 3: Desktop State Parameters for Sub-Screen View Functions
**What goes wrong:** `SettingsProvidersView` in desktop needs access to `preset_keys`, `add_name`, `add_url`, `add_key`, `add_tee` — all currently passed as parameters to `settings::view()`. If these are not threaded through to the new sub-screen view function, compile error or missing data.
**Why it happens:** iced views are pure functions. All mutable state lives in `main.rs` `App` struct and is passed to view functions as parameters.
**How to avoid:** The new sub-screen view functions must receive the same parameters they need from `main.rs`. Alternatively, since `Screen::SettingsProviders` renders instead of `Screen::Settings`, the `main.rs` dispatch can call `views::settings_providers::view(state, preset_keys, add_name, ...)` with the exact same parameters currently threaded to `settings::view()`.

### Pitfall 4: iOS SettingsView @State Not Preserved Between Navigation
**What goes wrong:** User opens SettingsProviders, goes back, re-opens — local `@State` fields like `presetKeys` are reset.
**Why it happens:** iOS `@State` is tied to view identity. When `ContentView` switches from `.settingsProviders` back to `.settings`, the SettingsView struct is re-initialized with default state.
**How to avoid:** This is acceptable behavior for settings screens — the same behavior exists today when navigating away from and back to Settings. The only concern is form fields losing in-progress input, which is a minor UX friction. Alternatively, move the local state that should persist (like `presetKeys`) to `AppManager` — but this is scope creep for Phase 26.

### Pitfall 5: Android LazyColumn State Lost on Sub-Screen Navigation
**What goes wrong:** `presetKeys` (a `mutableStateMapOf`) is defined inside `SettingsScreen` composable. If user navigates to SettingsProvidersScreen and back, the `remember {}` is not preserved.
**Why it happens:** Same as iOS — composable state is local to the composable instance.
**How to avoid:** Same mitigation: acceptable for v1. The provider enable form is a one-time operation. If state persistence becomes important, hoist state to `MainApp` `ViewModel`.

### Pitfall 6: Defaults Screen — defaultInstructions Initial Value
**What goes wrong:** `defaultInstructions` local state is initialized from `appState.globalSystemPrompt` on first composition. If the user navigates away and back, it re-reads from appState, which may have unsaved changes.
**Why it happens:** `var defaultInstructions by remember { mutableStateOf(appState.globalSystemPrompt ?: "") }` only runs once per composable instance on Android. iOS uses `@State private var defaultInstructionsInitialized` guard.
**How to avoid:** Same pattern as iOS: use `defaultInstructionsInitialized: Bool` flag to only initialize once.

## Code Examples

### Adding Screen Variants (Rust)
```rust
// Source: rust/src/lib.rs — Screen enum, currently has 7 variants
#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum Screen {
    Home,
    Settings,
    Chat { conversation_id: String },
    Onboarding { step: OnboardingStep },
    Documents,
    Agents,
    Memories,
    SettingsProviders,   // NEW — provider management sub-screen
    SettingsDefaults,    // NEW — defaults sub-screen
}
```

### iOS Routing for New Screens (ContentView.swift)
```swift
// Source: ios/Mango/Mango/ContentView.swift — add after existing cases
case .settingsProviders:
    SettingsProvidersView()
        .environmentObject(appManager)
case .settingsDefaults:
    SettingsDefaultsView()
        .environmentObject(appManager)
```

### Android Routing for New Screens (MainApp.kt)
```kotlin
// Source: android/.../MainApp.kt — add to when(screen) block
is Screen.SettingsProviders -> {
    SettingsProvidersScreen(
        appState = state,
        onDispatch = { action -> manager.dispatch(action) }
    )
}
is Screen.SettingsDefaults -> {
    SettingsDefaultsScreen(
        appState = state,
        onDispatch = { action -> manager.dispatch(action) }
    )
}
```

### Desktop Routing for New Screens (main.rs)
```rust
// Source: desktop/iced/src/main.rs — add to screen dispatch in view()
Screen::SettingsProviders => {
    views::settings_providers::view(
        &app.core.state(),
        app.is_dark,
        &app.settings_add_name,
        &app.settings_add_url,
        &app.settings_add_key,
        &app.settings_add_tee,
        &app.settings_preset_keys,
        app.settings_attestation_interval_input.as_deref().unwrap_or(""),
    )
}
Screen::SettingsDefaults => {
    views::settings_defaults::view(
        &app.core.state(),
        app.is_dark,
        &app.settings_default_model,
        &app.settings_default_instructions,
    )
}
```

### Regenerating UniFFI Bindings
```bash
# Source: Phase 25 plan 25-02 — exact sequence used in prior phase
just generate-bindings   # or equivalent just task
# Manually if just task not available:
# cargo build -p mango_core
# uniffi-bindgen generate rust/src/mango_core.udl --language swift --out-dir ios/Bindings/
# uniffi-bindgen generate rust/src/mango_core.udl --language kotlin --out-dir android/app/src/main/java/dev/disobey/mango/rust/
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Flat settings list (Providers→Defaults→Appearance→Advanced) | Grouped sections with named headers (Phase 24) | 2026-04-05 | Added MEMORY + TOOLS sections |
| Memories in home toolbar | Memories as navigation row inside Settings MEMORY section | Phase 24 | Cleaned home screen |
| Advanced always collapsed by default | Advanced collapsible with DisclosureGroup/AnimatedVisibility | Phase 6 original | Hides complexity for casual users |
| No memories toggle | Auto-extract Memories toggle in MEMORY section (Phase 25) | 2026-04-05 | User control over memory extraction |

**PROVIDERS section is the visually heaviest section** — contains multiple provider cards with health badges, attestation status, model lists, Set Default/Remove buttons, and API key entry for disabled providers. Moving it to a sub-screen will dramatically shorten the main settings scroll.

**DEFAULTS section contains two complex form elements** — a dropdown picker and a multi-line text editor. Moving it to a sub-screen is appropriate.

## Open Questions

1. **Scope of sub-screens: only Providers+Defaults, or also Tools?**
   - What we know: Tools section currently has one item (Brave API key). It's lightweight.
   - What's unclear: Whether Tools should become a sub-screen as more tool integrations are added later.
   - Recommendation: Keep Tools inline for now. It's a single field. If more tools are added in a future phase, promote it to a sub-screen then.

2. **Should the main settings screen preserve or drop the section headers for sub-screen sections?**
   - What we know: Summary rows have enough labeling (name + count + chevron) without a separate header label.
   - What's unclear: Whether a "PROVIDERS" label above the tappable row adds clarity or visual noise.
   - Recommendation: Keep section headers above summary rows for visual consistency with the other sections (MEMORY, TOOLS, APPEARANCE all have headers). Providers and Defaults rows should still be preceded by their uppercase section label.

3. **How many new Screen variants?**
   - Minimum: `SettingsProviders` and `SettingsDefaults` (2 new variants).
   - Optional: `SettingsTools` — only worth it if Tools grows beyond one field.
   - Recommendation: 2 new Screen variants. This is the minimum change that meaningfully reduces settings scroll depth and follows the app's existing sub-screen pattern.

4. **Desktop sidebar navigation vs. sub-screen for Providers?**
   - What we know: Desktop uses a sidebar layout (240px fixed sidebar + main content area). When Settings is pushed, it fills the main content area as a scrollable view.
   - What's unclear: Whether sub-screen navigation (replace entire settings view with a sub-screen) or a settings-internal panel swap (replace the scroll content without changing the outer chrome) is more idiomatic for a desktop app.
   - Recommendation: Use the same `PushScreen` pattern as mobile for consistency. The existing Memories, Documents, Agents screens are all full replacements of the main content area on Desktop — the sidebar stays. Settings sub-screens follow the same model.

## Environment Availability

Step 2.6: SKIPPED — Phase is code/config changes only. No external tool dependencies beyond existing Rust/Swift/Kotlin build toolchain.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (cargo test) |
| Config file | none (inline `#[test]` modules) |
| Quick run command | `cargo check -p mango_core && cargo check -p mango-desktop` |
| Full suite command | `cargo test -p mango_core 2>&1 \| tail -10` |

### Phase Requirements → Test Map

This phase has no assigned requirement IDs yet (TBD per ROADMAP). Proposed requirements and their test mapping:

| Proposed Req ID | Behavior | Test Type | Automated Command | File Exists? |
|-----------------|----------|-----------|-------------------|-------------|
| SET-08 | Settings main screen shows Providers as a tappable summary row with enabled count | manual-only | — | N/A |
| SET-09 | Tapping Providers row navigates to a Providers sub-screen | manual-only | — | N/A |
| SET-10 | Settings main screen shows Defaults as a tappable summary row | manual-only | — | N/A |
| SET-11 | Tapping Defaults row navigates to a Defaults sub-screen | manual-only | — | N/A |
| SET-12 | Provider sub-screen has Back → Settings navigation | manual-only | — | N/A |
| SET-13 | Defaults sub-screen has Back → Settings navigation | manual-only | — | N/A |
| SET-14 | All changes apply on iOS, Android, and Desktop | build | `cargo check -p mango_core && cargo check -p mango-desktop` | ✅ exists |

**Note:** New `Screen::SettingsProviders` and `Screen::SettingsDefaults` variants are verified by successful compilation after UniFFI bindings are regenerated. No new Rust unit tests are needed — the Screen enum addition is a pure structural change with no business logic.

### Sampling Rate
- **Per task commit:** `cargo check -p mango_core && cargo check -p mango-desktop`
- **Per wave merge:** `cargo test -p mango_core 2>&1 | tail -5`
- **Phase gate:** Full suite green + visual review on all three platforms

### Wave 0 Gaps

None — existing test infrastructure covers all phase requirements. Phase 26 is a UI reorganization. The Rust core change is adding new Screen enum variants (no logic, no new actions, no new persistence). No new unit tests are required.

## Work Breakdown Preview (for Planner)

### Wave 1: Rust Core + Bindings

**Plan 26-01:** Add new Screen variants to Rust + regenerate UniFFI bindings

1. Add `SettingsProviders` and `SettingsDefaults` to `Screen` enum in `rust/src/lib.rs`
2. Build `mango_core` to verify compilation
3. Regenerate UniFFI bindings: `ios/Bindings/mango_core.swift`, `ios/Bindings/mango_coreFFI.h`, `ios/Bindings/mango_coreFFI.modulemap`, `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt`

### Wave 2: UI Implementation

**Plan 26-02:** Main settings screen summary rows + new sub-screen views on all 3 platforms

1. iOS: Replace `providersSection` in SettingsView with summary row → `PushScreen(.settingsProviders)`
2. iOS: Replace `defaultsSection` in SettingsView with summary row → `PushScreen(.settingsDefaults)`
3. iOS: Create `SettingsProvidersView.swift` (extracted provider cards content)
4. iOS: Create `SettingsDefaultsView.swift` (extracted defaults content)
5. iOS: Add routing cases in `ContentView.swift`
6. Android: Replace PROVIDERS item block in SettingsScreen with summary row → `PushScreen(Screen.SettingsProviders)`
7. Android: Replace DEFAULTS item block in SettingsScreen with summary row → `PushScreen(Screen.SettingsDefaults)`
8. Android: Create `SettingsProvidersScreen.kt` composable
9. Android: Create `SettingsDefaultsScreen.kt` composable
10. Android: Add routing cases in `MainApp.kt`
11. Desktop: Replace providers_col + defaults_content in settings.rs with summary rows
12. Desktop: Create `views/settings_providers.rs` with extracted provider card content
13. Desktop: Create `views/settings_defaults.rs` with extracted defaults content
14. Desktop: Add routing cases in `main.rs` view dispatch

## Sources

### Primary (HIGH confidence)
- `/home/lio/g/confidential-app/ios/Mango/Mango/SettingsView.swift` — current iOS Settings structure (post Phase 25)
- `/home/lio/g/confidential-app/android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt` — current Android Settings structure (post Phase 25)
- `/home/lio/g/confidential-app/desktop/iced/src/views/settings.rs` — current Desktop Settings structure (post Phase 25)
- `/home/lio/g/confidential-app/rust/src/lib.rs` — AppState, AppAction, Screen enum (post Phase 25)
- `/home/lio/g/confidential-app/ios/Mango/Mango/ContentView.swift` — iOS routing switch (confirms 7 current Screen cases)
- `/home/lio/g/confidential-app/android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` — Android routing (confirms Screen.Memories case exists)
- `/home/lio/g/confidential-app/desktop/iced/src/views/home.rs` — Desktop sidebar navigation (Agents/Documents/Settings buttons)
- `.planning/phases/25-disable-enable-making-memories-in-the-app/25-02-SUMMARY.md` — UniFFI bindings regeneration procedure (exact commands and file paths)

### Secondary (MEDIUM confidence)
- `.planning/phases/24-redesign-settings-ux-.../24-CONTEXT.md` — locked decisions from Phase 24 (section order, section content)
- `.planning/phases/24-redesign-settings-ux-.../24-RESEARCH.md` — navigation patterns, anti-patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new libraries, all patterns directly observed from codebase
- Architecture: HIGH — sub-screen pattern directly derived from existing Memories screen implementation
- Pitfalls: HIGH — UniFFI binding pitfall confirmed by Phase 25 execution summary
- Work breakdown: HIGH — file list is complete and paths are verified

**Research date:** 2026-04-05
**Valid until:** 2026-05-05 (stable UI domain, 30 days)
