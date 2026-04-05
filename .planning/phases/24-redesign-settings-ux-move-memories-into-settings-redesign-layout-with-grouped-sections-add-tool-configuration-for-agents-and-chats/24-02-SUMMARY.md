---
phase: 24-redesign-settings-ux
plan: 02
subsystem: settings-ui
tags: [desktop, ios, android, settings, memory, brave-search, uniffi-bindings]

# Dependency graph
requires:
  - phase: 24-01
    provides: AppState.memory_count u64, AppState.brave_api_key_set bool, SetBraveApiKey action

provides:
  - Settings screens on all 3 platforms with MEMORY section (Memories row with count badge, chevron, PushScreen dispatch)
  - Settings screens on all 3 platforms with TOOLS section (Brave API key secure field, Save API Key button with enable/disable, context-aware placeholder)
  - Section order enforced: PROVIDERS > DEFAULTS > MEMORY > TOOLS > APPEARANCE > Advanced
  - Home toolbars without Memories button on all 3 platforms (Desktop sidebar, iOS toolbar, Android topBar)
  - Regenerated UniFFI Kotlin and Swift bindings with Phase 24 Wave 1 AppState fields

affects:
  - Any future plan that reads from Settings screens
  - 24-03 (if it exists)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "iced: section composition pattern with section_header() + composable content blocks"
    - "iOS: computed property sections (memorySection, toolsSection) composable into List body"
    - "Android: LazyColumn item blocks for new sections inserted between existing items"
    - "UniFFI binding regeneration required when Rust AppState fields change (just bindings-kotlin + just bindings-swift)"

key-files:
  created:
    - ios/Bindings/mango_core.swift
    - ios/Bindings/mango_coreFFI.h
    - ios/Bindings/mango_coreFFI.modulemap
  modified:
    - desktop/iced/src/main.rs
    - desktop/iced/src/views/settings.rs
    - desktop/iced/src/views/home.rs
    - ios/Mango/Mango/SettingsView.swift
    - ios/Mango/Mango/ContentView.swift
    - android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt
    - android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt
    - android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt

key-decisions:
  - "Regenerate UniFFI Kotlin and Swift bindings as part of this plan — Wave 1 Rust changes weren't reflected in the platform binding files"
  - "iOS Bindings/ directory committed to repo so Xcode picks up updated AppState (memoryCount, braveApiKeySet) without a separate CI regeneration step"
  - "Android SettingsScreen.kt: use Icons.Default.KeyboardArrowRight for chevron (ChevronRight not imported/available in existing icon set)"
  - "Desktop: brave_api_key_input passed as new parameter to settings::view() following established settings param pattern"

requirements-completed: [SET-01, SET-02, SET-03, SET-05, SET-07]

# Metrics
duration: 15min
completed: 2026-04-05
---

# Phase 24 Plan 02: Settings UI — MEMORY and TOOLS sections on all platforms

**MEMORY section (Memories count badge, chevron, PushScreen dispatch) and TOOLS section (Brave API key secure field with Save button) added to Settings on Desktop, iOS, and Android; Memories removed from all home toolbars**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-04-05T09:34:00Z
- **Completed:** 2026-04-05T09:49:00Z
- **Tasks:** 2
- **Files modified:** 8
- **Files created:** 3 (iOS Bindings)

## Accomplishments

**Task 1 — Desktop (5603f36):**
- Added `settings_brave_api_key: String` field to `App::Loaded` struct and `new()` initializer
- Added `SettingsBraveApiKeyChanged(String)` and `SettingsSaveBraveApiKey` Message variants
- Added handlers: `SettingsBraveApiKeyChanged` sets local state; `SettingsSaveBraveApiKey` trims, dispatches `AppAction::SetBraveApiKey`, clears field
- Updated `views::settings::view()` signature with `brave_api_key_input: &'a str` parameter
- Added `Screen` import to settings.rs for `PushScreen { screen: Screen::Memories }`
- Built MEMORY section: button row with count badge (hidden when 0) and `>` chevron dispatching PushScreen::Memories
- Built TOOLS section: "Web Search" header with "Configured" badge, description text, `.secure(true)` text_input with context-aware placeholder, `Save API Key` action_btn (disabled when input empty)
- Updated compose block to PROVIDERS > DEFAULTS > MEMORY > TOOLS > APPEARANCE > Advanced
- Removed `memories_btn` variable and its container entry from home sidebar bottom_nav

**Task 2 — iOS, Android, Bindings (d24e734):**
- iOS SettingsView: added `@State private var braveApiKeyInput: String = ""`; added `memorySection` (Section("Memory"), Button with `.pushScreen(screen: .memories)`, brain Label, count badge hidden when 0, chevron); added `toolsSection` (Section("Tools"), VStack with "Web Search" header, "Configured" badge, description, SecureField, "Save API Key" Button with `.setBraveApiKey(apiKey:)` dispatch); inserted both between defaultsSection and appearanceSection in body
- iOS ContentView: removed `Button("Memories")` from home toolbar (`.memories` screen route preserved)
- Android SettingsScreen: added `braveApiKeyInput` state variable; added `clickable`, `width`, `size` imports; added `Screen` import; added MEMORY item (Card/Row/clickable/KeyboardArrowRight icon/memoryCount badge); added TOOLS item (Card/Column/OutlinedTextField with PasswordVisualTransformation/Save API Key Button dispatching SetBraveApiKey)
- Android MainApp: removed `Text("Memories")` TextButton from home topBarActions (Screen.Memories route in when block preserved)
- Regenerated Kotlin bindings (`just bindings-kotlin`) — added memoryCount and braveApiKeySet to AppState data class and serializer
- Regenerated Swift bindings (`just bindings-swift`) — created ios/Bindings/ with mango_core.swift containing memoryCount and braveApiKeySet in AppState

## Task Commits

1. **Task 1: Desktop settings + home** - `5603f36` (feat)
2. **Task 2: iOS, Android, Bindings** - `d24e734` (feat)

## Files Created/Modified

**Created:**
- `ios/Bindings/mango_core.swift` - Regenerated Swift UniFFI bindings with Phase 24 AppState fields
- `ios/Bindings/mango_coreFFI.h` - C header for Swift bindings
- `ios/Bindings/mango_coreFFI.modulemap` - Module map for Swift bindings

**Modified:**
- `desktop/iced/src/main.rs` - settings_brave_api_key field, Message variants, handlers, view destructure, settings call site
- `desktop/iced/src/views/settings.rs` - brave_api_key_input param, Screen import, MEMORY section, TOOLS section, compose block order
- `desktop/iced/src/views/home.rs` - memories_btn removed from bottom_nav
- `ios/Mango/Mango/SettingsView.swift` - braveApiKeyInput state, memorySection, toolsSection computed properties, body List updated
- `ios/Mango/Mango/ContentView.swift` - Memories toolbar button removed
- `android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt` - new imports, braveApiKeyInput state, MEMORY item, TOOLS item
- `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` - Memories TextButton removed
- `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` - Regenerated Kotlin UniFFI bindings

## Decisions Made

- Regenerate UniFFI bindings as part of Wave 2 — the Rust AppState changes from Wave 1 weren't propagated to platform binding files
- Commit ios/Bindings/ directory so Xcode build picks up updated AppState struct without requiring a local bindings regeneration step in CI
- Android: use `Icons.Default.KeyboardArrowRight` for the memory chevron (no `ChevronRight` icon variant in existing imports)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Regenerated UniFFI bindings for both platforms**
- **Found during:** Task 2 (Android SettingsScreen.kt — `appState.memoryCount` and `appState.braveApiKeySet` unavailable in generated Kotlin type)
- **Issue:** Wave 1 added `memory_count` and `brave_api_key_set` to Rust AppState but did not regenerate the Kotlin/Swift bindings files, so the new fields were absent from platform-side AppState type
- **Fix:** Ran `just bindings-kotlin` and `just bindings-swift` to regenerate from compiled `libmango_core.so`
- **Files modified:** android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt, ios/Bindings/ (new directory)
- **Verification:** `grep memoryCount mango_core.kt` shows field present; `grep memoryCount ios/Bindings/mango_core.swift` shows field present
- **Committed in:** d24e734 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 2 — missing critical: UniFFI bindings must include new Rust fields before platform UI can compile)
**Impact on plan:** Regeneration was required for both platforms to use the new AppState fields. No scope creep.

## Known Stubs

None. All sections are wired to live AppState fields (`memory_count`, `brave_api_key_set`) and dispatch real actions (`PushScreen::Memories`, `SetBraveApiKey`).

## Self-Check: PASSED

All created files found on disk:
- FOUND: desktop/iced/src/views/settings.rs
- FOUND: ios/Mango/Mango/SettingsView.swift
- FOUND: android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt
- FOUND: ios/Bindings/mango_core.swift

All commits verified:
- FOUND: 5603f36 (Task 1 — Desktop)
- FOUND: d24e734 (Task 2 — iOS, Android, Bindings)
