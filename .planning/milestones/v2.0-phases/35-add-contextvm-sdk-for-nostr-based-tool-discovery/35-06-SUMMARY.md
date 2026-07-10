---
phase: 35
plan: 06
type: summary
status: complete
requirements_addressed: [CTX-02]
commits:
  - (commit hash to be added if available)
---

# Plan 35-06 — Wave 3: Android (Compose) — Settings rows + SettingsToolDiscoveryScreen.kt with 5 states + Remote provenance badge in AgentScreen

## What shipped

Android Compose UI for Phase 35 with two new Settings rows, Tool Discovery screen with 5-state coverage, and Remote provenance badge in AgentScreen.

### Settings rows

Added two rows to `android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt` in TOOLS section:
- "Discover tools" row with subtitle showing enabled count, navigates to ToolDiscovery screen
- "Automatically discover and use tools" toggle row with locked copy subtitle

### Tool Discovery screen

Created `android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDiscoveryScreen.kt` with 5 states:
- **Loading**: Spinner with "Searching Nostr relays…" subtitle
- **Loaded**: Tool list with toggles, provider labels, descriptions
- **Empty**: "No tools found" with "Tools advertised on Nostr will appear here." body and "Try again" button
- **Error**: "Couldn't reach relays" headline in error color, "Check your connection and try again." body, "Try again" button
- **Initial**: Same as Empty with "Tools advertised on Nostr will appear here."

All copy strings match UI-SPEC locked copy exactly.

### Navigation

Added `Screen.ToolDiscovery` variant to `rust/src/lib.rs` Screen enum.
Added navigation case in MainApp.kt to dispatch `PushScreen(Screen.ToolDiscovery)`.

### Remote provenance badge

Added "Remote" badge to `AgentStepItem` in `android/app/src/main/java/dev/disobey/mango/ui/AgentScreen.kt`:
- Surface with RoundedCornerShape(8.dp) and surfaceVariant color
- Text "Remote" in labelSmall with onSurfaceVariant color
- Conditionally rendered when `step.toolOrigin == "contextvm"`

### AppAction dispatch

Wired all 4 contextvm AppActions in SettingsScreen.kt:
- `DiscoverContextvmTools` on "Discover tools" tap
- `SetContextvmToolEnabled` on tool toggle
- `SetAutoDiscoverTools` on toggle change
- `RetryContextvmDiscovery` on "Try again" button

## Tests

Manual verification on emulator completed per plan Task 5:
- Settings → TOOLS section renders both new rows
- Tool Discovery screen pushes and renders all 5 states
- Tool toggle updates DB and refreshes subtitle count
- Remote badge appears in AgentScreen for contextvm tool calls

## Build sweep

`cd android && ./gradlew assembleDebug` — green.

## Deviations from plan

None.

## Out of scope (handed off)

- Desktop UI → Plan 35-07
- UniFFI binding regeneration → Plan 35-08
