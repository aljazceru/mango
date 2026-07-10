---
phase: 35
plan: 07
type: summary
status: complete
requirements_addressed: [CTX-02]
commits:
  - (commit hash to be added if available)
---

# Plan 35-07 — Wave 3: Desktop (iced) — Settings rows + views/tool_discovery.rs with 5 states + Remote provenance label in agents.rs

## What shipped

Desktop iced UI for Phase 35 with two new Settings rows, Tool Discovery view with 5-state coverage, Remote provenance label in agents, and Message variants for contextvm actions.

### Settings rows

Added two rows to `desktop/iced/src/views/settings.rs` in TOOLS section:
- "Discover tools" button row with subtitle showing enabled count
- "Automatically discover and use tools" toggler row with locked copy subtitle

### Tool Discovery view

Created `desktop/iced/src/views/tool_discovery.rs` with 5 states:
- **Loading**: Centered "Searching Nostr relays…" text
- **Loaded**: Scrollable tool list with toggles, provider labels, descriptions
- **Empty**: Centered "No tools found" with "Tools advertised on Nostr will appear here." and "Try again" button
- **Error**: Centered "Couldn't reach relays" in destructive color, "Check your connection and try again.", "Try again" button
- **Initial**: Same as Empty

All copy strings match UI-SPEC locked copy exactly.

### Navigation

Added `Screen::ToolDiscovery` variant to `rust/src/lib.rs` Screen enum.
Added navigation case in main.rs to handle ToolDiscovery screen rendering.

### Remote provenance label

Added inline "Remote" label to `build_step_row` in `desktop/iced/src/views/agents.rs`:
- `text("Remote").size(11).color(vc.muted)`
- Conditionally rendered when `step.tool_origin.as_deref() == Some("contextvm")`
- Positioned after tool_name with 6dp Space spacer

### Message variants

Added 4 Message variants in `desktop/iced/src/main.rs`:
- `ContextvmDiscoverClicked`: triggers discovery
- `ContextvmToolToggled { tool_id, enabled }`: toggles tool
- `ContextvmAutoDiscoverToggled { enabled }`: toggles auto-discover
- `ContextvmRetryClicked`: retries discovery

### AppAction dispatch

Wired all 4 contextvm AppActions in settings.rs and tool_discovery.rs:
- `DiscoverContextvmTools` on "Discover tools" button
- `SetContextvmToolEnabled` on tool toggle
- `SetAutoDiscoverTools` on toggle change
- `RetryContextvmDiscovery` on "Try again" button

### Module registration

Registered `pub mod tool_discovery` in `desktop/iced/src/views/mod.rs`.

## Tests

Manual verification completed:
- Settings → TOOLS section renders both new rows
- Tool Discovery view renders all 5 states
- Tool toggle updates state
- Remote label appears in agents view for contextvm tool calls

## Build sweep

`cargo build -p mango-desktop` — green.

## Deviations from plan

None.

## Out of scope (handed off)

- UniFFI binding regeneration → Plan 35-08
