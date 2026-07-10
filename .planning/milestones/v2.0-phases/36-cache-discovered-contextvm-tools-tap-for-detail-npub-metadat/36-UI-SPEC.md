---
phase: 36
slug: cache-discovered-contextvm-tools-tap-for-detail-npub-metadata
status: draft
shadcn_initialized: false
preset: none
created: 2026-05-08
platforms: [android, desktop]
ios_status: out-of-scope
extends: phase-35
---

# Phase 36 — UI Design Contract

> Visual and interaction contract for the Phase 36 extensions to the
> Phase 35 Tool Discovery surface on Android (Jetpack Compose) and Desktop
> (iced). All tokens (spacing, typography, color, copy tone) are inherited
> verbatim from `35-UI-SPEC.md`. **No new tokens are defined.** Only new
> string keys, new sub-screen layout, and new row affordances.

> **Scope (additive on top of Phase 35):**
> 1. Cache-first render of the existing Tool Discovery screen.
> 2. Always-visible search field on Tool Discovery.
> 3. New `Used N×` badge on list rows (when `usage_count > 0`).
> 4. New `Tool Detail` sub-screen (Android + Desktop).
> 5. Tap-to-copy confirmation toast/snackbar for npub + tool id.
>
> **Out of scope:** iOS UI (deferred), manual "Clear cache" action,
> auto-prune, sort UI, per-provider filters, sealing of cache table —
> all per `36-CONTEXT.md` deferred list.

---

## Design System

| Property | Value |
|----------|-------|
| Tool | none (no shadcn — native UI per platform) |
| Preset | not applicable |
| Component library | Material 3 (Android) / iced 0.13 widgets (Desktop) |
| Icon library | `androidx.compose.material.icons` (Android) / inline glyphs (Desktop) |
| Font | Material 3 default (Roboto/system) (Android) / iced default system font (Desktop) |
| Inheritance | All tokens, copy conventions, and component patterns inherit from `35-UI-SPEC.md` |

Source files for inherited conventions (unchanged from Phase 35):
- Android theme: `android/app/src/main/java/dev/disobey/mango/ui/theme/`
- Desktop theme: `desktop/iced/src/theme.rs` (`view_colors(is_dark) -> ViewColors`)
- Phase 35 list-row pattern: `SettingsToolDiscoveryScreen.kt` (Android) / `desktop/iced/src/views/tool_discovery.rs` (Desktop) — see commit `0774fc9`.
- Phase 32 helper: `relative_time_label` for "last used 3d ago" — reuse as-is per `36-CONTEXT.md` Claude's Discretion.

**No new Compose dependency, no new iced dependency, no new icon set.** If a new icon
is genuinely needed, prefer composing existing Material outlined icons (`Search`,
`MoreVert`, `ContentCopy`) — already on the Compose runtime classpath.

---

## Spacing Scale

Inherited from Phase 35 (`35-UI-SPEC.md` "Spacing Scale" section). The exact tokens used in this phase:

| Token | Value | Where Phase 36 uses it |
|-------|-------|------------------------|
| xs    | 4dp / 4px  | Inter-row spacing in tool list (existing); badge inner padding (vertical) |
| sm    | 8dp / 8px  | Search field bottom margin to first row; gap between badge and prior text |
| md    | 16dp / 16px | Horizontal padding of list / detail content; section vertical gap on detail screen |
| lg    | 24dp / 24px | Detail-screen final spacer before bottom safe area; gap between detail sections |
| section_gap | 16dp | Gap before each labelled section on the detail screen |
| card_inner_v | 14dp (Android) / 10px (Desktop) | Inner vertical padding of detail-screen rows (matches Phase 35) |
| card_corner | 8px (Desktop) / Material default (Android) | Border radius of all cards including the new `Used N×` badge pill |
| badge_h_pad | 6dp / 6px | Horizontal inner padding of `Used N×` pill (matches Phase 35 `Remote` badge) |
| badge_v_pad | 2dp / 2px | Vertical inner padding of `Used N×` pill (matches Phase 35 `Remote` badge) |

**No new spacing tokens.** All values above are already declared in Phase 35.

Exceptions: none.

---

## Typography

Inherited from Phase 35 verbatim. The exact roles used in this phase:

### Android (Material 3)

| Role | Style ref | Phase 36 usage |
|------|-----------|----------------|
| `bodyMedium` (FontWeight.Medium) | `MaterialTheme.typography.bodyMedium` | Tool name (list + detail), search field input, detail-screen section labels — reuses Phase 35 row title style |
| `bodySmall` | `MaterialTheme.typography.bodySmall` | Description, provider line, "Used N×" badge text, "last used 3d ago" caption, copy-confirmation snackbar text |
| `labelSmall` (uppercase) | `MaterialTheme.typography.labelSmall` | Detail-screen section labels (`ADVERTISED BY`, `SCHEMA`, `USAGE`) — same style as `TOOLS` section header |
| `titleLarge` | `MaterialTheme.typography.titleLarge` (FontWeight.Medium) | Detail-screen tool-name heading (h1) |
| `bodySmall` (monospace) | `MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace)` | Pretty-printed schema JSON block + npub bech32 string |

### Desktop (iced)

| Role | Size | Phase 36 usage |
|------|------|----------------|
| Header title | 17 | Detail-screen header title bar (`text("Tool details").size(17)`) |
| Detail tool-name h1 | 20 | New, but **uses `text(...).size(20)` only on the detail screen body**. This is the single new size introduced this phase. Justification: a tool-name heading at 17 (header size) collides with the back-bar title; 20 gives clear hierarchy. **Documented exception, single use.** |
| Row title | 14 | List row tool name (existing); detail-screen "Used N× — last used 3d ago" caption uses 12 (already declared) |
| Section header | 11 (uppercase, `vc.muted`) | Detail-screen section labels (`ADVERTISED BY`, `SCHEMA`, `USAGE`) |
| Body / row description | 13 | Detail-screen description body |
| Hint / sublabel | 11 (`vc.muted`) | "Tap to copy" hint under npub line; copy confirmation status line |
| Search input | 14 | Search `text_input(...).size(14)` — same size as row title |
| Badge text (`Used N×`) | 11 (`vc.muted`) | Matches the existing `Remote` provenance badge (Phase 35 § I) |
| Schema monospace block | 12 (`Font::MONOSPACE`) | Pretty-printed JSON. iced exposes `Font::MONOSPACE`; no new font asset |
| Empty-search caption | 14 (`vc.muted`) | "No tools match `{query}`" |

**One new typography size introduced this phase: `20` on Desktop, only for the detail-screen tool-name h1. All other typography reuses Phase 35 declarations.**

---

## Color

Inherited verbatim from `35-UI-SPEC.md` § Color (both light + dark mode tables). No new color tokens.

**Phase 36 accent reservations (additive to Phase 35's existing list):**

The accent color (`vc.accent` / `MaterialTheme.colorScheme.primary`) is **NOT** used for:
- the `Used N×` badge — reuses `vc.muted` background + `vc.text_dim` text, identical to the `Remote` provenance badge (keeps the visual story "muted = informational metadata, accent = action")
- the search field's focus ring on Android — Material 3 default outlined `TextField` already supplies primary color on focus; we accept the platform default and do not customize. No accent on Desktop's `text_input` (iced default).
- the copy-confirmation snackbar / toast — uses platform default snackbar color (Android `Snackbar` defaults) / muted card on Desktop.

The accent **is** used for:
- the existing "Try again" button (inherited from Phase 35) — unchanged.
- the existing per-tool enable Switch ON state (inherited) — unchanged.
- (no new accent surfaces this phase).

**60/30/10 budget honoured:** dominant `bg` (60%), secondary `card` + `surface` (30%, including the new detail screen's three sections), accent `vc.accent` (≤10%, reservations unchanged from Phase 35).

---

## Layout & Visual Hierarchy

Phase 35 already specifies the Settings → TOOLS section and the top-level Tool Discovery screen frame. Phase 36 only modifies the **body** of that screen and adds a new detail sub-screen.

### Tool Discovery sub-screen (modifications)

**File:** Android — existing `SettingsToolDiscoveryScreen.kt` (modified). Desktop — existing `desktop/iced/src/views/tool_discovery.rs` (modified).

**New screen structure (top-to-bottom):**

```
┌─────────────────────────────────────────────┐
│ [←]  Discover Tools             [↻ Refresh] │  ← TopAppBar — UNCHANGED from Phase 35
├─────────────────────────────────────────────┤
│  [🔍 Search tools                        ]  │  ← NEW: always-visible search field
├─────────────────────────────────────────────┤
│                                             │
│  ┌───────────────────────────────────────┐  │
│  │ Tool name              [Used 3×] [⏵] │  │  ← list row, NEW badge (when usage > 0)
│  │ Description (1-2 lines, ellipsis)     │  │       NEW chevron, whole-row tappable
│  │ Provider name / npub1abc…       [ ⬤] │  │       Switch on right (existing)
│  └───────────────────────────────────────┘  │
│                                             │
│  ... (more rows, sorted last_seen DESC) ... │
│                                             │
└─────────────────────────────────────────────┘
```

**1. Search field — always visible directly below the TopAppBar:**

- **Android:** Material 3 `OutlinedTextField` (or `TextField`) with leading `Icons.Outlined.Search` icon, single-line, placeholder copy `Search tools`. `.fillMaxWidth()`, `.padding(horizontal = 16.dp, vertical = 8.dp)` (uses `md` + `sm` tokens). No trailing clear-X button in v1 — user clears via system soft keyboard backspace; deferred per CONTEXT live-filter scope.
- **Desktop:** `text_input("Search tools", &state.contextvm_search_query)` with `.size(14)`, `.padding(8)`, `.width(Length::Fill)`. Wrapped in a `container(...).padding(Padding { top: 8.0, bottom: 8.0, left: 16.0, right: 16.0, .. })`. No leading icon (iced has no built-in icon dep wired up — match Phase 35 desktop convention of glyph-or-text-only).
- The search field is **always rendered**, even in Empty (D), Error (E), and Loading (C) states. It is **never** rendered in the new Empty-Search state (M) — that state only differs in the body, the field is still present.
- Search input `state.contextvm_search_query` (new field) lives in `AppState` (or per-screen UI-state, executor's call — but persistence across screens is **not** required; clearing on screen pop is acceptable).

**2. List row — additive changes only:**

The Phase 35 list row is preserved. Phase 36 adds:

- **Whole-row tap target:** the entire row, **excluding** the trailing `Switch` / `toggler`, becomes a tap target that navigates to the new Tool Detail screen. The `Switch` retains its own click absorption — toggling enable does **not** navigate. Implementation note (Android): wrap the existing `Card` content in a `Modifier.clickable { onDispatch(OpenContextvmToolDetail(tool_id)) }`; the `Switch` already absorbs its own clicks via Compose default. (Desktop: add an outer `button(...)` wrapper around the row content excluding the `toggler`; the `toggler` consumes its own click.)
- **Trailing chevron `>`:** rendered between the `Used N×` badge (when present) and the `Switch`, signalling the row is now drillable. Android: `Icons.AutoMirrored.Filled.KeyboardArrowRight`, size 18dp, color `onSurfaceVariant`. Desktop: inline glyph `text(">").size(12).color(vc.muted)` — same convention as `settings.rs:143`.
- **`Used N×` badge:** rendered immediately to the **left of the chevron**, only when `usage_count > 0`. Visual: identical to the Phase 35 `Remote` provenance badge (same muted pill).
  - Android: `Surface` with `RoundedCornerShape(8.dp)`, `padding(horizontal = 6.dp, vertical = 2.dp)`, `MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)` background, label `Used {N}×`, `MaterialTheme.typography.labelSmall`, color `MaterialTheme.colorScheme.onSurfaceVariant`.
  - Desktop: `container(text("Used {N}×").size(11).color(vc.muted)).padding(Padding::from([2u16, 6])).style(...)` — match the `Remote` badge styling at `desktop/iced/src/views/agents.rs:505` (Phase 35 § I).
  - **Crucially: the badge is muted, not accent** — same logic as Phase 35: muted = metadata, accent = action.
- **Existing description, provider line, and `Switch` are unchanged.**

**Row vertical layout (Android — top to bottom inside the Card's inner Column):**
1. Row(`spacedBy = 8dp`): `[Tool name]` (weight 1f) — `[Used N×]` — `[>]` — `[Switch]`
2. `Description` (1–2 lines, ellipsis) — unchanged from Phase 35
3. `Provider line` — unchanged from Phase 35

(Desktop mirrors the same vertical order, with the row 1 horizontal arrangement realised as `row![ name_text, Space::with_width(Length::Fill), badge_or_space, chevron, toggler ].spacing(8)`.)

### Tool Detail sub-screen (new)

**File:** Android — new `SettingsToolDetailScreen.kt`. Desktop — new `desktop/iced/src/views/tool_detail.rs`.

**Screen structure (top-to-bottom):**

```
┌─────────────────────────────────────────────┐
│ [←]  Tool details                            │  ← TopAppBar (no trailing action)
├─────────────────────────────────────────────┤
│                                             │
│  Tool name (h1, titleLarge / size 20)       │
│  Used 3× — last used 3d ago  (caption)      │  ← only if usage_count > 0
│                                             │
│  Description body (full text, no ellipsis)  │
│                                             │
│  ─────────────────────────────────────────  │
│  ADVERTISED BY                              │  ← labelSmall uppercase
│  Provider display name                      │  ← bodyMedium (or "Unnamed provider")
│  npub1abc…xyz                  [Copy]       │  ← bodySmall monospace, tap-to-copy
│  Hex: 7c3a8e5d…                [Copy]       │  ← bodySmall monospace, tap-to-copy
│                                             │
│  ─────────────────────────────────────────  │
│  USAGE                                      │  ← labelSmall uppercase
│  Used 3 times                               │  ← bodySmall (or "Never used")
│  Last used 3d ago                           │  ← bodySmall (omitted if never used)
│                                             │
│  ─────────────────────────────────────────  │
│  SCHEMA                          [▼ Show]   │  ← labelSmall uppercase + expander
│  ┌───────────────────────────────────────┐  │
│  │ {                                     │  │  ← monospace, selectable, scrollable
│  │   "type": "object",                   │  │
│  │   ...                                 │  │
│  │ }                                     │  │
│  └───────────────────────────────────────┘  │
│                                             │
│  Tool ID: tool_abc123…             [Copy]   │  ← bodySmall monospace, tap-to-copy
│                                             │
└─────────────────────────────────────────────┘
```

**Sections (vertical order, fixed):**

1. **Heading block** (no section label):
   - Tool name — Android `titleLarge` `FontWeight.Medium` / Desktop `text(...).size(20)`. Single line allowed to wrap to 2.
   - Caption (only when `usage_count > 0`) — `bodySmall` / size 12, color `vc.muted` / `onSurfaceVariant`. Copy: `Used {N}× — last used {relative}` where `{relative}` reuses Phase 32 `relative_time_label` (e.g. `3d ago`, `2w ago`, `just now`).
   - Description body — `bodyMedium` / size 13, full text, no truncation.

2. **`ADVERTISED BY` section:**
   - Section label: `labelSmall` uppercase, color `vc.muted`. Copy: `ADVERTISED BY`.
   - Provider display name (one line): `bodyMedium` / size 14. Copy: `{provider_display_name}` or `Unnamed provider` if NULL.
   - npub row: `bodySmall` monospace + trailing `Copy` action.
     - Android: `Row` with the npub `Text(...)` (`weight(1f)`, `style = bodySmall.copy(fontFamily = Monospace)`, `Modifier.clickable { copyToClipboard("npub") }`), and a trailing `IconButton(Icons.Outlined.ContentCopy)` content-description `Copy npub`. Tapping anywhere on the row triggers the copy.
     - Desktop: `row![ text(npub).size(12).font(Font::MONOSPACE), Space::with_width(Length::Fill), button(text("Copy").size(11)).on_press(Message::CopyNpub) ]` — text is `selectable` via the iced `text` widget's default behaviour where applicable; an explicit `Copy` button is the primary mechanism.
   - Hex row: identical structure to npub row, label `Hex:` prefix, text shows `{first-8-hex}…` for visual brevity but the Copy action copies the **full** hex.
   - Tap-to-copy confirmation:
     - **Android:** `Snackbar` via `SnackbarHostState` (one shared host at the screen Scaffold level). Duration: short. Copy: `npub copied`, `Pubkey copied`, `Tool ID copied` — see Copywriting Contract.
     - **Desktop:** an inline status line beneath the section, `text("npub copied").size(11).color(vc.success)`, auto-cleared after 2 seconds via `Task::perform(tokio::time::sleep, ...)`. Match the existing "Saved" / "Verified" iced confirmation pattern in `settings.rs`.

3. **`USAGE` section:**
   - Section label: `labelSmall` uppercase. Copy: `USAGE`.
   - When `usage_count == 0`: single line `bodySmall`, color `vc.muted`. Copy: `Never used`.
   - When `usage_count > 0`: two stacked lines.
     - Line 1 — `bodySmall`, default text. Copy: `Used 1 time` (singular) or `Used {N} times` (plural).
     - Line 2 — `bodySmall`, color `vc.muted`. Copy: `Last used {relative}` where `{relative}` reuses Phase 32 `relative_time_label`.

4. **`SCHEMA` expander section:**
   - Section label `labelSmall` uppercase + trailing affordance.
     - **Default state: collapsed.** Schema body NOT rendered.
     - Android: `Row` containing `Text("SCHEMA")` weight 1, then `TextButton(onClick = { expanded = !expanded }) { Text(if (expanded) "Hide" else "Show") }`.
     - Desktop: `row![ section_header("SCHEMA"), Space::with_width(Length::Fill), button(text(if expanded "▲ Hide" else "▼ Show").size(11)).on_press(Message::ToggleSchema) ]`.
   - When expanded: `serde_json::to_string_pretty(&schema_json)` rendered inside a monospaced, selectable, scrollable container.
     - Android: `SelectionContainer { Text(text = pretty, fontFamily = FontFamily.Monospace, style = bodySmall) }` inside a `Card` with `Modifier.heightIn(max = 320.dp).verticalScroll(rememberScrollState())`. Card uses `surfaceVariant.copy(alpha = 0.5f)` background to distinguish from page surface.
     - Desktop: `scrollable(text(pretty).size(12).font(Font::MONOSPACE)).max_height(320.0)` inside a `container(...)` with `card_corner = 8`, `vc.card` background, `vc.border` 1px stroke.
   - **Schema content is never rendered as Markdown.** Plain text only — no parser, no link auto-detect, no syntax highlighting. (Eliminates injection surface per CONTEXT D-Schema decision.)

5. **Tool ID row** (last, after schema):
   - Single row at body padding, no section label.
   - Android / Desktop: same Row pattern as the npub row above. Label prefix `Tool ID:` then truncated `{tool_id_first8}…`, trailing `Copy` action that copies the full id string. Same confirmation pattern (Snackbar / iced status line).

**Detail-screen vertical rhythm:**
- Section gap = `lg` (24dp / 24px) between section blocks (heading → ADVERTISED BY → USAGE → SCHEMA → tool ID).
- Intra-section gap = `xs` (4dp / 4px) between stacked lines within a single section.
- Outer page padding = `md` (16dp / 16px) horizontal, `md` top, `lg` bottom.
- Each section's body sits directly under its label with `xs` (4dp) spacing — no card wrapping for the ADVERTISED BY and USAGE sections (they read as inline metadata, matching the Settings sub-screen style of `SettingsTools` Brave key block). The SCHEMA expander **does** wrap its expanded body in a Card to mark the boundary of the monospace block.

**Visual hierarchy:**
- **Primary:** the tool-name heading (h1) — the answer to "what tool is this?".
- **Secondary:** the `Used N× — last used 3d ago` caption (when present) — the answer to "have I touched this before?".
- **Tertiary:** description body — the answer to "what does it do?".
- **Quaternary:** the three labelled sections (ADVERTISED BY, USAGE, SCHEMA) — verifiable metadata and source-of-truth artefacts.

---

## Interaction

### Tap targets

- **List row whole-row tap target (NEW):** ≥ 48dp tall already (Phase 35 row height). Switch retains its own touch absorption; chevron is decorative and not separately clickable (the row is one tap target, the Switch another).
- **Search field:** Material 3 `TextField` default ≥ 56dp tall (Android); Desktop `text_input` 32–36px tall — adequate for keyboard-first interaction.
- **Copy buttons / rows:** Android `IconButton` 48dp default; tapping the surrounding text Row also triggers copy (whole-row clickable). Desktop `button(text("Copy"))` is a click target ≥ 24px high — adequate for mouse.
- **`Show` / `Hide` schema toggle:** Android `TextButton` 48dp; Desktop `button` ≥ 24px.

### Gestures

- Single tap on a list-row body (excluding Switch) → `OpenContextvmToolDetail { tool_id }` → push detail screen.
- Single tap on list-row Switch → toggle (existing).
- Type into search field → live filter (no debounce, per CONTEXT D-search-3). Filter applies immediately in `@Composable` `derivedStateOf` (Android) / `view()` re-render (Desktop) using a pure in-memory `filter` over `AppState.contextvm_tools`.
- Press `Refresh` (in TopAppBar, existing) → re-issue discovery; clears filter? **No** — search query persists across refresh. Cached rows render instantly; new rows appear and are filtered live.
- On detail screen: tap the npub row, hex row, or tool-ID row → copy to clipboard + show ephemeral confirmation. Tap the Copy button has identical effect. Tap the `Show` / `Hide` button → toggle schema body. Tap back arrow → pop screen.
- System back: pops detail screen, then pops Tool Discovery screen, restoring Settings.

### Live search filter

- Filter: `tool.tool_name.lowercase().contains(q) || tool.description.lowercase().contains(q) || tool.provider_display_name.unwrap_or("").lowercase().contains(q)`, where `q = query.trim().lowercase()`.
- Empty query (or all-whitespace) → no filter; render full list (state F or G as appropriate).
- Non-empty query with no matches → render the new Empty-Search state (M below). The 5-state Phase 35 machine is **not** changed; the empty-search state is rendered **inside** the success body when `filtered.is_empty() && !query.is_empty()`.
- Live filter is keystroke-by-keystroke, **no debounce**. Justification: `Vec<DiscoverableTool>` cardinality is bounded by Nostr discovery (tens, not thousands); `O(N×M)` substring scan on each keystroke is trivially fast.

### Optimistic / cache-first render

- On screen open, the actor populates `AppState.contextvm_tools` from `list_all_contextvm_tools` (cached), then auto-fires the existing `DiscoverContextvmTools` action (Phase 35 behaviour — unchanged from current code; UI sees cached rows immediately).
- During the in-flight refresh, the existing TopAppBar `Refresh` button is **disabled** (Phase 35 spec § Interaction → Focus / disabled states — unchanged).
- A subtle "Refreshing…" status — explicitly **NOT** added in this phase per CONTEXT optimistic-render scope. The cached list renders; the disabled Refresh button is the sole in-flight signal. (This is a deliberate minimalism: the user already sees content, they don't need a second spinner.)

### Copy confirmation

- **Android:** single `SnackbarHostState` at the detail screen's Scaffold root. `LaunchedEffect(...)` triggered by a one-shot copy event surfaces a Snackbar with locked copy strings (see Copywriting Contract). Snackbar duration: `Short`. No action button.
- **Desktop:** an inline status line near the bottom of the screen — `text("npub copied").size(11).color(vc.success)` — set by a `Message::CopyConfirmation(label)` and cleared after 2 seconds via `Task::perform(tokio::time::sleep(Duration::from_secs(2)), |_| Message::ClearCopyConfirmation)`. Pattern mirrors existing iced "Saved" feedback.

### Focus / disabled states

- **Search field while loading (state C):** enabled. The user can type; the filter applies to whatever is currently in `AppState.contextvm_tools` (which is the cached list). Once the refresh resolves, the new list is filtered with the same query.
- **Schema expander while no schema (rare — `schema_json` is empty/NULL):** the `Show` / `Hide` button is hidden; the section label is followed by `text("No schema published").size(13).color(vc.muted)` (Desktop) / `bodySmall` muted (Android).
- **Copy actions while clipboard unavailable (Desktop wayland edge case):** if the platform clipboard write fails, the confirmation copy reads `Couldn't copy — try again` (color `vc.destructive`); see Copywriting Contract.

---

## States

For each state, the contract specifies when it shows, the visual elements it renders, and the exact English copy.

States A–I are **inherited verbatim from Phase 35**. Phase 36 adds states J–N below. The Phase 35 5-state machine for the discovery query (`Loading / Empty / Error / Success / Disabled`) is unchanged.

### J — List row with `Used N×` badge (NEW)

**When:** `tool.usage_count > 0`.

**Visual:** identical to Phase 35 list row, with the badge inserted between the trailing description column and the chevron. See "Layout → List row → additive changes". Badge styling **identical** to the Phase 35 `Remote` provenance badge.

**Copy:** `Used {N}×` (e.g. `Used 1×`, `Used 3×`, `Used 12×`). The `×` glyph is U+00D7 (multiplication sign) — Unicode-stable, no font fallback required on Android or Desktop.

**Hidden when:** `usage_count == 0`. The chevron remains; only the badge is conditional.

### K — List row with chevron (NEW, applies to all rows)

**When:** always, on every list row.

**Visual:** trailing `>` glyph (Desktop) / `KeyboardArrowRight` icon (Android), placed between the `Used N×` slot (whether populated or not) and the `Switch`. Color `vc.muted` / `onSurfaceVariant`. Conveys "this row is drillable".

### L — Search field (NEW, all sub-screen states)

**When:** Tool Discovery screen is mounted, **always rendered** (in Loading C, Empty D, Error E, Success F, and Empty-Search M).

**Visual:** see "Layout → Tool Discovery sub-screen → 1. Search field".

**Copy:**
- Placeholder (empty input): `Search tools`
- Live input echoes user keystrokes (no transform).

### M — Empty-search state (NEW)

**When:** discovery state is Success (F), the user has typed a non-empty query, and `filtered.is_empty()`.

**Visual:** Inside the screen body (the search field stays mounted above):
- Vertically centred caption.
- Android: `Text(...)` with `bodyMedium` weight Normal, color `onSurfaceVariant`, padding 32dp.
- Desktop: `text("...").size(14).color(vc.muted)` centred in a `container(...).padding(32)`.
- No icon, no button. (Distinct from D Empty: D is a "0 tools discovered ever" state with a `Try again` action; M is a "0 tools match this query" state where the user clears the search to recover.)

**Copy:**
- Body: `No tools match "{query}"` — `{query}` is the user's exact input, displayed in straight quotes (`"`), unmodified.

### N — Tool Detail sub-screen states (NEW)

The detail screen is only reached via tap on a list row, which means the tool already exists in `AppState.contextvm_tools`. Therefore the detail screen has no Loading state and no Error state — the row is always available, instant render.

The **only** detail-screen state variation is the SCHEMA expander:

- **Schema collapsed (default):** label + `Show` button only.
- **Schema expanded:** label + `Hide` button + scrollable monospace body.
- **Schema absent (`schema_json` is empty/NULL):** label + muted text "No schema published".

The USAGE section also varies:
- **Never used:** single muted line `Never used`.
- **Used:** two-line block — count + relative-time.

### O — Copy-action ephemeral confirmation (NEW)

**When:** any of {npub Copy, Hex Copy, Tool ID Copy} succeeds.

**Visual & copy:** see "Interaction → Copy confirmation".

**Failure variant (clipboard write failed):**
- Android: `Snackbar` with copy `Couldn't copy — try again` and color the Snackbar's text using `MaterialTheme.colorScheme.error` (or just default — the text content is the primary signal).
- Desktop: status line `text("Couldn't copy — try again").size(11).color(vc.destructive)`.

---

## Copywriting Contract

**All strings locked. Future i18n is out-of-scope for this phase.**

Phase 36 introduces the following new locked strings. **All Phase 35 copy is preserved verbatim**; nothing in `35-UI-SPEC.md` § Copywriting Contract is changed or replaced.

| Element | Surface | Locked copy |
|---------|---------|-------------|
| Search field placeholder | Tool Discovery sub-screen | `Search tools` |
| Empty-search caption | Tool Discovery sub-screen | `No tools match "{query}"` |
| List row `Used N×` badge — singular | Tool Discovery list row | `Used 1×` |
| List row `Used N×` badge — plural | Tool Discovery list row | `Used {N}×` (e.g. `Used 3×`) |
| Detail screen — TopAppBar title | Tool Detail sub-screen | `Tool details` |
| Detail screen — heading caption (used) | Tool Detail | `Used {N}× — last used {relative}` (e.g. `Used 3× — last used 3d ago`; `Used 1× — last used just now`) |
| Detail screen — section label 1 | Tool Detail | `ADVERTISED BY` |
| Detail screen — provider fallback | Tool Detail | `Unnamed provider` |
| Detail screen — npub label prefix | Tool Detail | (none — npub displayed as `npub1abc…xyz`) |
| Detail screen — hex label prefix | Tool Detail | `Hex:` |
| Detail screen — section label 2 | Tool Detail | `USAGE` |
| Detail screen — usage line 1 (singular) | Tool Detail | `Used 1 time` |
| Detail screen — usage line 1 (plural) | Tool Detail | `Used {N} times` |
| Detail screen — usage line 2 (used) | Tool Detail | `Last used {relative}` (e.g. `Last used 3d ago`) |
| Detail screen — usage when never used | Tool Detail | `Never used` |
| Detail screen — section label 3 | Tool Detail | `SCHEMA` |
| Detail screen — schema expander (collapsed) | Tool Detail | `Show` |
| Detail screen — schema expander (expanded) | Tool Detail | `Hide` |
| Detail screen — schema absent body | Tool Detail | `No schema published` |
| Detail screen — Tool ID label prefix | Tool Detail | `Tool ID:` |
| Detail screen — Copy button text (Desktop) / a11y label (Android) | Tool Detail | `Copy` |
| Confirmation snackbar — npub copied | Tool Detail | `npub copied` |
| Confirmation snackbar — hex pubkey copied | Tool Detail | `Pubkey copied` |
| Confirmation snackbar — tool id copied | Tool Detail | `Tool ID copied` |
| Confirmation — copy failed | Tool Detail | `Couldn't copy — try again` |
| Settings → TOOLS Row A subtitle (cached count surfaces here too) | Settings (already locked in Phase 35) | (unchanged from Phase 35: `No tools enabled` / `1 tool enabled` / `{N} tools enabled`) |

**Microcopy rules applied (continuing from Phase 35):**
- Sentence case throughout. Exceptions: section headers (`ADVERTISED BY`, `USAGE`, `SCHEMA`) — uppercase via `style.uppercase()`, matching Phase 35 `TOOLS` / `PROVIDERS`.
- Em-dashes (`—`) used for clause separation in the heading caption (`Used 3× — last used 3d ago`) and copy-failure (`Couldn't copy — try again`). U+2014 — preserves the calm tone of Phase 35 error copy.
- `{relative}` substitution is delegated to the Phase 32 `relative_time_label` helper. The helper's output strings (e.g. `just now`, `3d ago`, `2w ago`) are themselves locked by Phase 32.
- Singular/plural toggling for `Used 1×` vs `Used 3×` and `Used 1 time` vs `Used 3 times` is handled in the actor when computing the pre-rendered display strings (matching Phase 32 pattern of pre-computed labels in UniFFI Records, e.g. `last_synced_label`). UI does not branch on integers.
- Quoted user input in the empty-search caption uses straight ASCII quotes (`"`) for portability across Android `TextField` and iced `text_input` rendering. **Not** smart curly quotes.

---

## Registry Safety

| Registry | Blocks Used | Safety Gate |
|----------|-------------|-------------|
| shadcn official | none — no shadcn in this project | not applicable |
| Material 3 (Android) | (Phase 35 inventory) + `OutlinedTextField` (or `TextField`), `Snackbar`, `SnackbarHostState`, `SelectionContainer`, `TextButton`, `Icons.Outlined.Search`, `Icons.Outlined.ContentCopy`, `Icons.AutoMirrored.Filled.KeyboardArrowRight`. All ship in the existing Compose / Material 3 dep set already on the runtime classpath (no new artifacts). | not required — verified by inspection of existing imports in `SettingsToolsScreen.kt`, `ChatScreen.kt`, `OnboardingScreen.kt` (Snackbar) and `SettingsMemoryScreen.kt` (TextField pattern). |
| iced 0.13 (Desktop) | (Phase 35 inventory) + `text_input`, `scrollable`, `Font::MONOSPACE` (built into iced core), `Task::perform` (tokio sleep already pulled in by Phase 32). No new widgets; no new fonts. | not required — verified by inspection of `settings.rs` (text_input usage), `memories.rs` (scrollable usage), `agents.rs` (monospace text where applicable). |
| `bech32` crate (Rust core, NEW dependency) | npub bech32 encoding | **Cargo audit gate required.** Per `36-CONTEXT.md` Claude's Discretion: pick `bech32` 0.11 OR reuse a transitive `nostr` types crate already pulled in by `contextvm-sdk`. Whichever lowers the dep count after `cargo tree` audit. The audit happens in the planner (RESEARCH.md), not here — this UI-SPEC only declares that bech32 encoding is needed for the `Advertised by → npub1…` display string. The encoded value is computed in the Rust core (per the actor-only DB-access invariant) and surfaces via a UniFFI Record string — UI never invokes bech32 directly. |

**No new Compose dependency, no new iced dependency, no new icon set, no new font asset.**
The only candidate new Rust dep is the bech32 crate, gated by the planner.

---

## Cross-References to CONTEXT decisions

| Decision (`36-CONTEXT.md`) | Where reflected in this UI-SPEC |
|----------------------------|--------------------------------|
| Area 1 — All discovered tools cached, optimistic render | "Interaction → Optimistic / cache-first render" |
| Area 1 — No "Refreshing…" affordance beyond the disabled Refresh button | "Interaction → Optimistic / cache-first render" (deliberate minimalism note) |
| Area 1 — Refresh trigger unchanged | Phase 35 § Interaction inherited verbatim |
| Area 2 — Whole-row tap navigates to detail; Switch absorbs its own click | "Interaction → Gestures" + "Layout → List row → Whole-row tap target" |
| Area 2 — npub bech32 + truncated hex both shown, tap-to-copy | "Layout → Tool Detail → ADVERTISED BY section" |
| Area 2 — Schema pretty-printed, monospace, selectable, scrollable, expander | "Layout → Tool Detail → SCHEMA expander section" |
| Area 2 — Plain-text schema, no Markdown | "Layout → Tool Detail → SCHEMA expander section" (injection-surface note) |
| Area 2 — Snackbar (Android) / inline status (Desktop) for copy confirmation | "Interaction → Copy confirmation" + "States → O" |
| Area 3 — Always-visible search field, single screen | "Layout → Search field — always visible" + "States → L" |
| Area 3 — Search across name + description + provider, case-insensitive substring | "Interaction → Live search filter" |
| Area 3 — Live filter, no debounce | "Interaction → Live search filter" (justification recorded) |
| Area 3 — Empty result keeps the search field visible | "States → M" |
| Area 4 — `Used N×` badge on rows when count > 0 | "States → J" |
| Area 4 — "Last used {relative}" + count on detail screen | "Layout → Tool Detail → USAGE section" |
| Area 4 — Compute via `agent_steps` aggregate, no denormalised column | UI does not depend on this — actor surfaces pre-computed `usage_count` + `last_used_at` in the UniFFI `DiscoverableTool` record (UI rule: never compute aggregates in the view). |
| Specifics — < 16ms paint after navigation | "Interaction → Optimistic / cache-first render" + the cache-first contract |
| Specifics — `Used N×` badge matches the `Remote` provenance pill style | "Layout → List row → Used N× badge" (explicit re-use of Phase 35 § I styling) |
| Specifics — npub copy must be one-tap | "Layout → Tool Detail → ADVERTISED BY" + "Interaction → Gestures" (whole-row clickable + dedicated Copy button) |
| Specifics — search is a primary affordance | "Layout → Search field" (always visible, top-of-body, never hidden) |
| Specifics — detail content order (name → caption → desc → advertised by → schema) | "Layout → Tool Detail → Sections" (matches verbatim, with USAGE inserted between ADVERTISED BY and SCHEMA — minor adjustment to surface usage as a top-level labelled section, justified by Area 4 making usage a peer concern with provider attribution) |

---

## Checker Sign-Off

- [ ] Dimension 1 Copywriting: PASS
- [ ] Dimension 2 Visuals: PASS
- [ ] Dimension 3 Color: PASS
- [ ] Dimension 4 Typography: PASS
- [ ] Dimension 5 Spacing: PASS
- [ ] Dimension 6 Registry Safety: PASS

**Approval:** pending
