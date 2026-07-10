---
phase: 35
slug: add-contextvm-sdk-for-nostr-based-tool-discovery
status: draft
shadcn_initialized: false
preset: none
created: 2026-05-08
platforms: [android, desktop]
ios_status: deferred
---

# Phase 35 — UI Design Contract

> Visual and interaction contract for the Settings → Tools updates and the new
> Tool Discovery sub-screen on Android (Jetpack Compose) and Desktop (iced).
> Inherits all spacing, typography, and color tokens from prior phases — no new
> tokens defined.

> **Scope:** Two new rows in Settings → TOOLS, one new sub-screen ("Discover
> Tools"), one provenance affordance in `AgentStepSummary` rendering. iOS UI
> mirror is deferred to a follow-up phase per CONTEXT D-06.

---

## Design System

| Property | Value |
|----------|-------|
| Tool | none (no shadcn — native UI per platform) |
| Preset | not applicable |
| Component library | Material 3 (Android) / iced 0.13 widgets (Desktop) |
| Icon library | `androidx.compose.material.icons` (Android) / inline glyphs `>`, `▲`, `▼` (Desktop) |
| Font | Material 3 default (Roboto/system) (Android) / iced default system font (Desktop) |

Source files for inherited conventions:
- Android tokens come from existing Material 3 `MaterialTheme` (see `android/app/src/main/java/dev/disobey/mango/ui/theme/`)
- Desktop tokens come from `desktop/iced/src/theme.rs` (`view_colors(is_dark) -> ViewColors`)
- Settings sub-screen pattern: `SettingsScreen.kt` lines 66–145 (Android) / `desktop/iced/src/views/settings.rs` lines 133–238 (Desktop)
- Tappable summary row component: `SettingsLinkCard` (`SettingsScreen.kt` lines 168–201, Android) / inline `button(row![...])` pattern (`settings.rs` lines 135–164, Desktop)
- Boolean toggle row component: `SettingsMemoryScreen.kt` lines 64–83 (Android — `Switch`) / `settings.rs` lines 344–366 (Desktop — `toggler`)

---

## Spacing Scale

Inherited — declared values used in the existing Settings screens.

| Token | Value | Usage | Source of truth |
|-------|-------|-------|-----------------|
| xs | 4dp / 4px | Spacing between section header and card; intra-card vertical gap | `SettingsScreen.kt:64` `Arrangement.spacedBy(4.dp)`; `settings.rs:14` section header `bottom: 6.0` |
| sm | 8dp / 8px | Top of LazyColumn first item; spacing between toggle rows | `SettingsScreen.kt:67` `Spacer(Modifier.height(8.dp))`; `settings.rs` `.spacing(8)` |
| md | 16dp / 16px | Horizontal padding of LazyColumn / scrollable content; inner card padding | `SettingsScreen.kt:63` `.padding(horizontal = 16.dp)`; `settings.rs:14` section header `left: 16.0, right: 16.0` |
| lg | 24dp / 24px | (Desktop) bottom spacer before footer | `settings.rs:594` `Space::new().height(24)` |
| xl | 32dp / 32px | LazyColumn final spacer (Android) | `SettingsScreen.kt:147` `Spacer(Modifier.height(32.dp))` |
| section_gap | 16dp | Gap before each new section header on Android | `SettingsScreen.kt:77,87,...` `Spacer(Modifier.height(16.dp))` |
| card_inner_v | 14dp / 10dp | Vertical inner padding of toggle row card (Android M3 list-item style — 14dp; Desktop card content — 10dp) | `SettingsMemoryScreen.kt:68` `padding(horizontal = 16.dp, vertical = 14.dp)`; `settings.rs:356` `Padding::from([10u16, 16])` |
| card_corner | 8px (Desktop) / Material default (Android) | Card border radius | `settings.rs:156` `radius: 8.0.into()` |

Exceptions: none. All values are multiples of 4 or already-published Material defaults. **Do not introduce new spacing values in this phase.**

---

## Typography

Inherited — declared roles used in the existing Settings & memory screens.

### Android (Material 3 typography scale)

| Role | Style ref | Used for |
|------|-----------|----------|
| `bodyMedium` (FontWeight.Medium) | `MaterialTheme.typography.bodyMedium` | Row title (e.g. "Discover tools", "Auto-extract memories") — see `SettingsMemoryScreen.kt:72`, `SettingsScreen.kt:183` |
| `bodySmall` | `MaterialTheme.typography.bodySmall` | Row subtitle / one-line description — see `SettingsMemoryScreen.kt:75`, `SettingsScreen.kt:187` |
| `labelSmall` | `MaterialTheme.typography.labelSmall` (uppercase) | Section header ("TOOLS", "PROVIDERS") — see `SettingsScreen.kt:160–164` |
| `bodyMedium` | `MaterialTheme.typography.bodyMedium` | List row primary text in Tool Discovery list (tool name) |
| `bodySmall` | `MaterialTheme.typography.bodySmall` (color = onSurfaceVariant) | List row secondary text (description, provider) — match `AgentScreen.kt:364` pattern |
| TopAppBar title | `Text(...)` with `FontWeight.Medium` | Sub-screen title ("Discover Tools") — match `SettingsMemoryScreen.kt:45` |

### Desktop (iced inline `text(...).size(N)` — no font scale crate)

| Role | Size | Used for |
|------|------|----------|
| Header title | 17 | Settings header / sub-screen header — `settings.rs:111` |
| Row title | 14 | Summary row title ("Providers", "Memories") — `settings.rs:138` |
| Section header | 11 (color: `vc.muted`) | Section labels ("PROVIDERS", "TOOLS") — `settings.rs:13` |
| Row subtitle / metadata | 12 (color: `vc.muted`) | "{N} enabled" trailing summary — `settings.rs:140–142` |
| Body / row description | 13 (color: `vc.text` or `vc.text_dim`) | Toggle row title — `settings.rs:347` `.size(14)` (we use 14 for parity with summary row title) |
| Hint / sublabel | 11 (color: `vc.muted`) | One-line explanatory text under toggle | `settings.rs:325, 488` |
| Empty state heading | 16 | Empty/error headline (mirror `memories.rs:77`) |
| Empty state body | 14 (color: `vc.muted`) | Empty/error subtitle (mirror `memories.rs:79–80`) |
| Trailing chevron | 12 (color: `vc.muted`, glyph `>`) | Tappable row affordance — `settings.rs:143` |

**Do not introduce new sizes in this phase.** Where the spec calls for a size that does not appear in the table above, fall back to the closest existing size and document the choice in PLAN.md.

---

## Color

Inherited from `theme.rs::view_colors()` (Desktop) and `MaterialTheme.colorScheme` (Android). No new tokens.

### Light mode

| Role | Android source | Desktop source | Usage in Phase 35 |
|------|----------------|----------------|-------------------|
| Page background | `MaterialTheme.colorScheme.background` | `vc.bg` = `#F7F7F7` (`theme.rs:99`) | Settings + Tool Discovery scrollable area |
| Card / row surface | `MaterialTheme.colorScheme.surface` (Material 3 Card default) | `vc.card` = `#FFFFFF` (`theme.rs:102`) | Each settings row, each tool list row |
| Border | (Material 3 outline implicit) | `vc.border` = `#CCCCCC` (`theme.rs:104`) | Card stroke (Desktop only) |
| Primary text | `MaterialTheme.colorScheme.onSurface` | `vc.text` = `#1A1A1A` (`theme.rs:106`) | Row titles, tool names |
| Secondary text | `MaterialTheme.colorScheme.onSurfaceVariant` | `vc.text_dim` / `vc.muted` (`theme.rs:107–108`) | Subtitles, descriptions, "{N} enabled" counts, section labels |
| Accent | `MaterialTheme.colorScheme.primary` | `vc.accent` = `#008C64` (`theme.rs:109`) | "Try again" button, switch ON state |
| Destructive | `MaterialTheme.colorScheme.error` | `vc.destructive` = `#B71C1C` (`theme.rs:116`) | Error-state heading copy |
| Success | (inline `Color(0xFF2E7D32)`) | `vc.success` (`theme.rs:117`) | "Configured" / saved confirmation (already in Tools section, untouched) |

### Dark mode

| Role | Android source | Desktop source | Usage in Phase 35 |
|------|----------------|----------------|-------------------|
| Page background | `MaterialTheme.colorScheme.background` (Material 3 dark) | `vc.bg` = `#0E0E10` (`theme.rs:63`) | Same |
| Card / row surface | `MaterialTheme.colorScheme.surface` | `vc.card` = `#1E1F23` (`theme.rs:66`) | Same |
| Border | (Material 3 outline implicit) | `vc.border` = `#323338` (`theme.rs:68`) | Same |
| Primary text | `MaterialTheme.colorScheme.onSurface` | `vc.text` = `#FFFFFF` (`theme.rs:70`) | Same |
| Secondary text | `MaterialTheme.colorScheme.onSurfaceVariant` | `vc.text_dim` / `vc.muted` (`theme.rs:71–72`) | Same |
| Accent | `MaterialTheme.colorScheme.primary` | `vc.accent` = `#00C896` (`theme.rs:73`) | Same |
| Destructive | `MaterialTheme.colorScheme.error` | `vc.destructive` = `#E53E3E` (`theme.rs:75`) | Same |

**Accent reserved for:** the "Try again" button label (Tool Discovery error & empty states), the active-state of the new "Automatically discover and use tools" toggle (platform default), and the existing "Save" buttons. **Not** used for default row chevrons, tool-name text, or provenance markers.

**60/30/10 budget honoured:** dominant `bg` (60%), secondary `card` + `surface` (30%), accent `vc.accent` (≤10%, reserved as listed).

---

## Layout & Visual Hierarchy

### Settings screen — TOOLS section

**Existing layout:** Section header `"TOOLS"` (uppercase labelSmall) → single `SettingsLinkCard` with title `"Tools"` and subtitle `"Web search configured"`/`"Web search not configured"` linking to `SettingsTools` sub-screen (`SettingsScreen.kt:127–135`). Desktop equivalent: section header `"TOOLS"` → inline Brave Search card body inside `tools_body` (`settings.rs:587–588`).

**Phase 35 addition — Android (`SettingsScreen.kt`):**

Replace the single-row TOOLS section with **three** rows in this exact order:

1. **(Existing)** "Tools" → `Screen.SettingsTools` — kept as-is. Subtitle: existing Brave Search summary.
2. **(NEW — Row A)** "Discover tools" → `Screen.ToolDiscovery` — `SettingsLinkCard`-pattern row. Subtitle = enabled-tool count summary (see Copywriting Contract).
3. **(NEW — Row B)** "Automatically discover and use tools" — boolean toggle row, follows the `SettingsMemoryScreen.kt` toggle pattern (Card + Row + Switch). Subtitle = one-line behaviour description.

**Placement decision:** Both new rows are added **inside the existing "Tools" section block** — i.e., the `item { ... SettingsSectionLabel("Tools") ...}` at lines 127–135. The section label `"Tools"` is rendered **once**, then three Cards stack inside the same `item { ... }` block separated by `Arrangement.spacedBy(4.dp)` (matching the LazyColumn column spec at line 64). The "Appearance" section header that follows remains unchanged (currently rendered after a 16.dp spacer — `SettingsScreen.kt:138`).

**Phase 35 addition — Desktop (`desktop/iced/src/views/settings.rs`):**

After the existing `tools_body` (line 509–511), append two more elements **inside the same TOOLS section** (i.e., before `section_header("APPEARANCE", ...)` at line 589):

1. **(NEW — Row A — Discover tools summary row)** mirrors the existing `providers_summary` button (lines 135–164). On press → `AppAction::PushScreen { screen: Screen::ToolDiscovery }`.
2. **(NEW — Row B — Auto-discover toggle row)** mirrors the existing `memory_toggle` (lines 344–366). The `toggler` widget is bound to `state.auto_discover_tools_enabled` (new field) with `Message::SettingsAutoDiscoverToolsToggled`.

In both Row A and Row B, the subtitle/secondary text uses `vc.muted` and size 11 (Desktop) / `bodySmall` + `onSurfaceVariant` (Android), exactly matching the `memory_toggle_row` pattern.

**Visual hierarchy on the Settings screen:**
- **Primary:** the "TOOLS" section header (`labelSmall` uppercase, muted) anchors the section.
- **Secondary:** each row title (`bodyMedium`, weight Medium / size 14) is the tappable / actionable focus.
- **Tertiary:** subtitles (`bodySmall` / size 11–12, muted) explain state at a glance.
- **Quaternary:** trailing affordance — chevron `>` (Row A) or `Switch`/`toggler` (Row B). Never an accent colour fill on the card itself.

### Tool Discovery sub-screen

**File:** Android — new `ToolDiscoveryScreen.kt`. Desktop — new `desktop/iced/src/views/tool_discovery.rs`.

**Screen structure (top-to-bottom):**

```
┌─────────────────────────────────────────┐
│ [←]  Discover Tools         [↻ Refresh] │  ← TopAppBar / iced header
├─────────────────────────────────────────┤
│                                         │
│   <state-dependent body>                │
│   - loading: spinner + subtitle         │
│   - error: icon + headline + subtitle + │
│            "Try again"                  │
│   - empty: icon + headline + subtitle + │
│            "Try again"                  │
│   - success: scrollable list of rows    │
│                                         │
└─────────────────────────────────────────┘
```

**Top bar:**
- Leading: back button (Android `IconButton` with `Icons.AutoMirrored.Filled.ArrowBack`; Desktop "Back" text button per `settings.rs:99–110`)
- Title: `"Discover Tools"` (Android `FontWeight.Medium`, Desktop `text("Discover Tools").size(17)`)
- Trailing: refresh action — Android `IconButton` with `Icons.Filled.Refresh`; Desktop `button(text("Refresh").size(13))` styled like the existing "Apply" button (`settings.rs:303–319`). Disabled while a discovery query is in flight.

**List row — visual hierarchy (per row):**
- **Primary text:** tool display name — `bodyMedium` weight Medium (Android) / `text(...).size(14)` (Desktop). Color: primary text token.
- **Secondary text (1 line):** provider identifier — `bodySmall` (Android) / `text(...).size(12)` (Desktop). Color: `onSurfaceVariant` / `vc.muted`. If announcement provides a display name, show that. Otherwise show the Nostr pubkey truncated to first 8 hex chars + `…` (e.g. `npub1abc…` or `7c3a8e5d…`). Per CONTEXT D-04.
- **Tertiary text (1–2 lines, ellipsised):** tool description — `bodySmall` color `onSurfaceVariant` / `vc.muted`, max 2 lines, `TextOverflow.Ellipsis` (Android) / iced text wrap default with explicit `Length::Fill` width.
- **Trailing:** per-tool enable toggle — `Switch` (Android) / `toggler` (Desktop). `onCheckedChange` dispatches `AppAction::SetContextvmToolEnabled { tool_id, enabled }`.

**Row dimensions:**
- Android: `Card` with `Modifier.fillMaxWidth()`; inner `Row` padded `horizontal = 16.dp, vertical = 14.dp` (matches `SettingsMemoryScreen.kt:67–68`). Three-text Column on the left (`weight(1f)`, `Arrangement.spacedBy(2.dp)`), Switch on the right.
- Desktop: `container(...)` with `card_inner_v` = 10px, `horizontal = 16px` padding (matches `settings.rs:356`), `border-radius = 8.0`, `vc.card` background, 1px `vc.border` stroke.
- Inter-row spacing: 4dp (Android `Arrangement.spacedBy(4.dp)`) / `.spacing(8)` (Desktop, mirroring memory rows).

**Divider style:** Android — none between Cards (Cards self-separate via spacing). Desktop — none; cards stand alone with their own border. **Do not introduce a `HorizontalDivider`** (the existing pattern uses Card-as-separator). The only existing exception is `SettingsMemoryScreen.kt:85` where two rows live in **one** Card — Tool Discovery rows are independent Cards, so no divider needed.

---

## Interaction

### Tap targets

- **Android:** all `clickable {}` Cards/Rows are min 48dp tall (Material 3 default — `SettingsLinkCard` `padding(16.dp)` = 16+text+16 ≥ 48dp). Switches use Material 3 default touch target.
- **Desktop:** all `button(...)` rows have `Padding::from([8u16, 16])` (16dp horizontal, ≥8px vertical) wrapping ≥14sp text → comfortable mouse target.

### Gestures

- Single tap / click on Row A → push Tool Discovery screen.
- Single tap / click on Row B (anywhere on the row, not just the switch) is **not** required to be wired — the Switch itself is the only target, mirroring `SettingsMemoryScreen.kt` which only binds `Switch.onCheckedChange`. The wider row is informational. (This matches existing convention; do not change.)
- Single tap on a list row's Switch → toggle that tool. Tapping the row body (outside the switch) does **nothing** in v1 — same convention as the per-conversation tools toggle in `ChatScreen.kt:756–762`.
- Trailing "Refresh" → re-issue discovery query.
- "Try again" button (in error/empty state) → re-issue discovery query (same effect as Refresh).
- Back arrow / system back → pop screen; **selection state is already persisted** at toggle time (CONTEXT D-07), so no save/cancel pattern needed.

### Navigation transitions

- Inherits the existing screen-push pattern via `AppAction::PushScreen { screen: Screen::ToolDiscovery }` and `AppAction::PopScreen`. No new transition animation in this phase — Android uses Compose default; Desktop iced switches view synchronously.

### Focus / disabled states

- **Refresh button while discovery in flight:** disabled. Android — `IconButton(enabled = !state.contextvm_discovery_loading)`; Desktop — omit `.on_press(...)` (matches the existing `brave_key_field` disabled pattern at `settings.rs:433–438`).
- **Per-tool Switch while not yet hydrated:** enabled by default — toggling a tool the moment it appears must work, since persistence is independent of relay liveness.
- **Auto-discover toggle (Row B) when no enabled providers / no relay reachability:** **always enabled.** Defaulting to OFF (per CONTEXT D-05) means turning it ON is harmless; if relays are unreachable at conversation time, the failure path in CONTEXT D-11 takes over.

---

## States

For each state, the contract specifies:
- when it shows
- the visual elements it renders
- the exact English copy

### A — Settings → TOOLS section, Row A "Discover tools" subtitle states

| Condition | Subtitle copy |
|-----------|---------------|
| 0 contextvm tools enabled (the default at first launch and after disabling all) | `No tools enabled` |
| Exactly 1 contextvm tool enabled | `1 tool enabled` |
| ≥ 2 contextvm tools enabled | `{N} tools enabled` (e.g. `3 tools enabled`) |

### B — Settings → TOOLS section, Row B "Automatically discover and use tools" subtitle (always shown)

`Find new tools each conversation and offer them to the assistant automatically.`

(Matches the descriptive-style subtitle pattern of `SettingsMemoryScreen.kt:74–77`.)

### C — Tool Discovery sub-screen — initial loading

**When:** the screen opens and a discovery query is in flight (no cached results yet, per CONTEXT-discretion "pull on open" model).

**Visual:**
- Centred vertically and horizontally inside the screen body.
- Android: `CircularProgressIndicator(strokeWidth = 2.dp)` — same widget used by Brave key verification (`SettingsToolsScreen.kt:145`). 24dp diameter.
- Desktop: text-only fallback `text("Searching Nostr relays…").size(14).color(vc.muted)` — iced has no built-in spinner widget in our current dep set; do not add one in this phase.
- 16dp gap between spinner and subtitle (Android only).

**Copy:**
- Subtitle (Android, under spinner): `Searching Nostr relays…`
- (Desktop has no spinner; the subtitle stands alone, centred, mirroring `memories.rs:74–89`.)

### D — Tool Discovery sub-screen — empty (relays returned no announcements)

**When:** discovery query completed successfully but found 0 tools.

**Visual (mirrors `memories.rs:74–89`):**
- Vertically centred column, `align_x = Center`, padding 48px (Desktop) / 48dp (Android).
- Optional leading icon (Android only — `Icons.Outlined.SearchOff` at 48dp, color `onSurfaceVariant`). Desktop omits the icon (iced has no Material icon set wired up; do not add a new dep).
- Headline: size 16 (Desktop) / `bodyLarge` (Android), default text color.
- Body: size 14 (Desktop) / `bodyMedium` (Android), `vc.muted` / `onSurfaceVariant`.
- Primary action button: "Try again" — uses the existing `action_btn(...)` helper on Desktop (`settings.rs:29–64`); Android uses `Button(...)` with default Material 3 colours.

**Copy:**
- Headline: `No tools found`
- Body: `Tools advertised on Nostr will appear here.`
- Button: `Try again`

### E — Tool Discovery sub-screen — error (relays unreachable / query failed)

**When:** the discovery query failed (transport error, all relays unreachable, parse error).

**Visual:** identical layout to the empty state. Headline color uses `vc.destructive` / `MaterialTheme.colorScheme.error` to distinguish from the neutral empty state. Body uses muted color.

**Copy:**
- Headline: `Couldn't reach relays`
- Body: `Check your connection and try again.`
- Button: `Try again`

(Aligns with CONTEXT D-11: "couldn't reach relays" — exact phrasing locked here.)

### F — Tool Discovery sub-screen — success with tools

**When:** discovery query returned ≥ 1 tool announcement.

**Visual:** scrollable list of rows (see "List row" spec above). No section header inside the list. Rows are sorted by the order returned by the Rust core (`Vec<DiscoverableTool>` per CONTEXT D-10) — UI does not re-sort.

### G — Per-tool toggle states (within the list)

- **Disabled (default for newly-discovered tools):** Switch is OFF. Row uses default `vc.card` background.
- **Enabled (user toggled ON):** Switch is ON, accent-coloured (platform default). Row background unchanged — **do not** apply `card_enabled` highlighting; that token is reserved for provider rows (`theme.rs:67`) and would create visual noise here.
- **Persistence:** toggling fires an `AppAction` immediately; the state survives backgrounding and relaunch (CONTEXT D-07). No "Save" button.

### H — Auto-discover toggle (Row B) state

- **OFF (default):** Switch off, subtitle reads as in row B above.
- **ON:** Switch on, subtitle unchanged. No additional "On" indicator needed — the Switch's own state is sufficient (matches `SettingsMemoryScreen.kt`).

### I — Tool-call provenance affordance (CONTEXT D-13)

**Where:** every step rendered by `build_step_row` (Desktop: `desktop/iced/src/views/agents.rs:443–540`) and `AgentStepItem` (Android: `AgentScreen.kt:339–373`) for which the underlying `AgentStepSummary.tool_origin` is `"contextvm"` (new field; defaults to `"local"` for built-in tools).

**Visual contract — minimum:**

A small inline label rendered immediately after the existing `tool_name` in the header row:

- **Android:** small Surface badge with `RoundedCornerShape(8.dp)` (the same pill style used at `SettingsProvidersScreen.kt:175–186` for health), 6dp horizontal / 2dp vertical inner padding, label `Remote`, `MaterialTheme.typography.labelSmall`, color `MaterialTheme.colorScheme.onSurfaceVariant`, background `MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)`. **Not** accent-coloured.
- **Desktop:** inline `text("Remote").size(11).color(vc.muted)` immediately after the `tool_name` text at `agents.rs:505`. No background — keeps the step header low-noise.

**Copy:** `Remote`. (Single locked word — short, unambiguous, contrasts implicitly with "local"/built-in. Tooltip / longer disclosure deferred — out-of-scope for this phase.)

**Rule:** the badge is rendered **only** when `tool_origin == "contextvm"`. Local tools (Brave, fetch_url, file ops, calculator) render no badge — preserves the existing visual on every shipped local tool path.

---

## Copywriting Contract

**All strings locked. Future i18n is out-of-scope for this phase (no string keys defined).**

| Element | Surface | Locked copy |
|---------|---------|-------------|
| Settings → TOOLS Row A title | Settings | `Discover tools` |
| Settings → TOOLS Row A subtitle (0 enabled) | Settings | `No tools enabled` |
| Settings → TOOLS Row A subtitle (1 enabled) | Settings | `1 tool enabled` |
| Settings → TOOLS Row A subtitle (≥2 enabled) | Settings | `{N} tools enabled` |
| Settings → TOOLS Row B title | Settings | `Automatically discover and use tools` |
| Settings → TOOLS Row B subtitle | Settings | `Find new tools each conversation and offer them to the assistant automatically.` |
| Tool Discovery — top bar title | Sub-screen | `Discover Tools` |
| Tool Discovery — refresh action a11y label / Desktop button text | Sub-screen | `Refresh` |
| Tool Discovery — back action a11y label | Sub-screen | `Back` |
| Loading subtitle | Sub-screen | `Searching Nostr relays…` |
| Empty state heading | Sub-screen | `No tools found` |
| Empty state body | Sub-screen | `Tools advertised on Nostr will appear here.` |
| Error state heading | Sub-screen | `Couldn't reach relays` |
| Error state body | Sub-screen | `Check your connection and try again.` |
| Try-again button | Sub-screen (empty + error) | `Try again` |
| Per-tool list row — provider fallback when no display name | Sub-screen | `{first-8-pubkey-chars}…` (e.g. `7c3a8e5d…`) |
| Provenance badge label | Agent step / chat tool summary | `Remote` |

**Microcopy rules applied:**

- Sentence case throughout. Capitalise only proper nouns and the first word.
- Section headers are the only exception (`TOOLS`, `PROVIDERS` — uppercase via `style.uppercase()`), matching existing convention at `SettingsScreen.kt:160–164` / `settings.rs:13`.
- No exclamation marks, no emoji, no marketing voice ("Awesome!" / "Boom!"). Match the calm, factual tone of `SettingsMemoryScreen.kt:74` ("Extract memories after each conversation and store them locally.").
- Error copy specifies a path forward ("Check your connection and try again.") — never blames the user, never hides the cause behind "Something went wrong".
- Numbers are localised at render time using platform formatters (out-of-scope for the locked English source — but no string baking of numerals into copy).

---

## Registry Safety

| Registry | Blocks Used | Safety Gate |
|----------|-------------|-------------|
| shadcn official | none — no shadcn in this project | not applicable |
| Material 3 (Android) | `Card`, `Row`, `Column`, `Text`, `Switch`, `Scaffold`, `TopAppBar`, `LazyColumn`, `IconButton`, `CircularProgressIndicator`, `Icons.Filled.Refresh`, `Icons.AutoMirrored.Filled.ArrowBack`, `Icons.Outlined.SearchOff`, `HorizontalDivider` (only if reused — defaulted to "no") | not required — all components already shipping in current Compose dep set per `SettingsScreen.kt`/`SettingsMemoryScreen.kt` imports |
| iced 0.13 (Desktop) | `button`, `container`, `column`, `row`, `text`, `text_input`, `scrollable`, `toggler`, `rule::horizontal`, `Space` | not required — all widgets already shipping in current iced dep set per `settings.rs` imports |

**No new Compose dependency, no new iced dependency.** If the implementer reaches for a new icon set, a chip/pill library, or any third-party UI registry, they MUST flag it in PLAN.md before pulling it in — the Phase 35 contract assumes zero new UI deps.

---

## Cross-References to CONTEXT decisions

| Decision | Where reflected in this UI-SPEC |
|----------|---------------------------------|
| **D-03** (Settings → TOOLS gains Discover row + auto-discover toggle) | "Layout & Visual Hierarchy → Settings screen — TOOLS section" |
| **D-04** (Tool Discovery shows name, description, provider, enable toggle) | "Layout → Tool Discovery sub-screen → List row" |
| **D-05** (auto-discover defaults OFF) | "States → H Auto-discover toggle" |
| **D-06** (Android + Desktop only; iOS deferred) | Front-matter `platforms: [android, desktop]`, `ios_status: deferred` |
| **D-07** (per-tool toggle persists across launches) | "Interaction → no save/cancel needed"; "States → G Per-tool toggle states" |
| **D-10** (Rust core owns business logic; UI surfaces a `Vec<DiscoverableTool>`) | "States → F success" — list order comes from the core, UI does not re-sort |
| **D-11** (relay-unreachable degrades gracefully with "couldn't reach relays" copy) | "States → E error", "Copywriting Contract" |
| **D-13** (provenance for remote-vs-local tool calls in agent step summary) | "States → I Tool-call provenance affordance" |

CTX-02 (Discover tools row + screen on Android & Desktop) and CTX-04 (auto-discover toggle defaults off, persisted) are the two requirements this spec most directly governs; CTX-10 (provenance) is governed by section I.

---

## Checker Sign-Off

- [ ] Dimension 1 Copywriting: PASS
- [ ] Dimension 2 Visuals: PASS
- [ ] Dimension 3 Color: PASS
- [ ] Dimension 4 Typography: PASS
- [ ] Dimension 5 Spacing: PASS
- [ ] Dimension 6 Registry Safety: PASS

**Approval:** pending
