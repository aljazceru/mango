# Phase 36: Cache discovered contextvm tools — tap-for-detail + search + used-history - Context

**Gathered:** 2026-05-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Build on the Phase 35 contextvm Tool Discovery sub-screen (Android Compose + Desktop iced) so that:

1. **Cache** — all rows returned by Nostr discovery (kind 11317) are persisted in the existing `contextvm_tools` SQLite table even when `enabled = 0`, and the Discover Tools screen renders the cached list instantly on open while a background refresh fetches fresh announcements.
2. **Tap-for-detail** — tapping a row opens a tool detail screen that shows the advertising npub (bech32), full description, pretty-printed JSON schema, last-seen timestamp, and per-tool usage history (last used + count).
3. **Search** — a search field on the Discover Tools screen filters the cached list by tool name / description / provider display name (case-insensitive substring, live filter, no debounce).
4. **Used-in-past indicator** — list rows display a "Used N×" badge for tools the user has actually invoked (computed from `agent_steps` rows where `tool_origin = 'contextvm'`).

In scope: Rust core (queries, search, npub bech32 encoding, usage aggregation), UniFFI surface, persistence (no new tables — add columns/queries only as needed), Android Compose UI, Desktop iced UI.
Out of scope: iOS UI, automatic cache pruning, periodic auto-refresh, encrypted/sealed cache (the existing `contextvm_tools` table inherits whatever protection the actor already provides).

</domain>

<decisions>
## Implementation Decisions

### Area 1 — Cache scope & lifecycle
- **All discovered tools cached**, including `enabled = 0` rows. Discovery upserts every row with the existing schema; the actor preserves the user's `enabled` flag across upserts (existing behavior).
- **Optimistic render**: on screen open, the actor populates `AppState.contextvm_tools` from `list_all_contextvm_tools` first, then auto-fires the existing `DiscoverContextvmTools` action to refresh in background. UI shows a subtle "Refreshing…" affordance only while the network query is in flight; the existing 5-state machine (`ContextvmDiscoveryState`) is reused — `Loading` keeps current "Searching Nostr relays…" copy.
- **No auto-prune** in v1 — disabled rows persist indefinitely. A future phase can add a manual "Clear cache" or time-based prune.
- **Refresh trigger** — manual Refresh button (already exists in the TopAppBar) plus the existing auto-fire on first composition stay as-is. No new periodic refresh.

### Area 2 — Tool detail screen
- **Whole-row navigation** — tapping anywhere on a tool row (outside the trailing Switch) navigates to the detail screen. The Switch keeps its own click absorption so toggling enable/disable does not navigate.
- **npub format** — `provider_pubkey` (hex) is encoded to bech32 (`npub1…`) for display. Both the npub string and the truncated hex are shown; tap-to-copy with a Material Snackbar / iced toast confirmation.
- **Schema display** — `schema_json` is pretty-printed (`serde_json::to_string_pretty`) inside a monospaced, selectable, scrollable code block under a "Show schema" expander. Schema content remains plain text — no Markdown, no syntax injection surface.
- **Copy actions** — tap-to-copy for npub and full tool id; long-press select-all on schema. Confirmation surface uses platform-native ephemeral feedback (Snackbar on Android, a temporary status line in iced).

### Area 3 — Search UX
- **Always-visible search field** below the TopAppBar on the existing `SettingsToolDiscoveryScreen` / iced equivalent. Single screen — no separate search route.
- **Search scope** — case-insensitive substring match across `tool_name`, `description`, and `provider_display_name` (NULL coalesces to empty). Match is in-memory over the already-loaded `AppState.contextvm_tools` vector.
- **Live filter** — filter applied per keystroke; no debounce (lists are bounded by Nostr discovery cardinality and small enough for instant filter).
- **Empty result** — show "No tools match `{query}`" caption while keeping the search field visible. The 5-state machine's existing Empty state stays for the unfiltered "0 tools discovered" case.

### Area 4 — Used-in-past tracking
- **Definition of "used"** — any `agent_steps` row with `tool_origin = 'contextvm'` and matching `tool_name`, regardless of step status. (Errors still indicate user intent to invoke.)
- **Granularity** — by `tool_name`. Justified by the existing `idx_contextvm_tools_name UNIQUE` index — tool_name is unique within the cache.
- **Indicator on list row** — small "Used N×" badge inline when `count > 0`, hidden otherwise. Full "Last used {relative time}" plus count surfaces on the detail screen.
- **Compute location** — actor runs a single `SELECT tool_name, COUNT(*), MAX(timestamp) FROM agent_steps WHERE tool_origin = 'contextvm' GROUP BY tool_name` query when populating `AppState.contextvm_tools` and after each invocation handler. Merged into a new `usage_count: u32` and `last_used_at: Option<i64>` field on `DiscoverableTool`. No denormalized column on `contextvm_tools` — keeps `agent_steps` as single source of truth.

### Claude's Discretion
- Choice of bech32 crate (`bech32` 0.11) vs reusing whatever `nostr` types contextvm-sdk already pulls in — pick whichever yields fewer dependencies after auditing `Cargo.lock`.
- Exact placement of relative-time formatting helper (reuse `relative_time_label` from Phase 32 plan 07 if signature fits; otherwise add a thin wrapper).
- Snackbar vs Toast vs inline confirmation — pick the existing pattern already used elsewhere on Android (likely Snackbar via `SnackbarHostState`).
- iced UI parity — match Android UX shape (search field, detail screen, badges) but follow existing iced patterns from Phase 32/34.1 (Column accumulator, `Task::perform` for async ops).

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `rust/src/persistence/queries.rs:1330` — `ContextvmToolRow` (id, tool_name, display_name, description, provider_pubkey, provider_display_name, schema_json, enabled, last_seen_at). Already has `upsert_contextvm_tool`, `update_contextvm_tool_enabled`, `list_enabled_contextvm_tools`, `list_all_contextvm_tools`.
- `rust/src/persistence/schema.rs:355` — MIGRATION_V20 created `contextvm_tools` table; `agent_steps.tool_origin` column added in same migration.
- `rust/src/lib.rs:163` — `DiscoverableTool` UniFFI record (will gain `usage_count: u32`, `last_used_at: Option<i64>`, `last_seen_at: i64`).
- `rust/src/lib.rs:341` — `AppState.contextvm_tools: Vec<DiscoverableTool>` already populated from DB on discovery; will additionally be populated on screen open before discovery.
- `rust/src/lib.rs:752` — existing AppActions: `DiscoverContextvmTools`, `RetryContextvmDiscovery`, `SetContextvmToolEnabled { tool_id, enabled }`. Phase 36 adds: `LoadCachedContextvmTools`, `SearchContextvmTools { query }` (or keep filter local to UI), `OpenContextvmToolDetail { tool_id }`.
- `rust/src/contextvm/dispatch.rs` — `ContextvmToolDescriptor` and `from_row` projection. Detail screen reuses these where useful.
- Phase 32 `relative_time_label` helper in Rust core for cross-platform "last used 3d ago" formatting.

### Established Patterns
- Actor-only DB access — all reads/writes go through actor handlers; no direct rusqlite calls from FFI surface.
- UniFFI Records carry pre-computed display strings (e.g., `last_synced_label` in DirectorySource). Phase 36 follows: actor computes `last_seen_label` and `last_used_label` once.
- Sub-screens reached via `AppAction::PushScreen { screen: Screen::… }` enum variant; nav stack lives in AppState.
- Android Compose: `LaunchedEffect(Unit) { onDispatch(...) }` for screen-mount actions (Phase 35 pattern in `SettingsToolDiscoveryScreen`).
- iced Desktop: `Task::perform` for async dispatch; mutable `Column` accumulator for conditional UI; `Message::*` enum variants for typed dispatch.
- Search/filter pattern: in-memory `Vec` filter — see how memories/conversations are filtered today; reuse if a helper exists.

### Integration Points
- New `Screen::ContextvmToolDetail { tool_id }` variant added to the Screen enum in `rust/src/lib.rs` (or wherever Screen is defined).
- New routes in:
  - Android `MainApp.kt` nav switch.
  - Desktop `desktop/src/main.rs` view dispatch.
- New Compose file: `android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDetailScreen.kt`.
- New iced file: probably `desktop/src/screens/tool_detail.rs` (matching existing screen-per-file pattern).
- bech32 helper module under `rust/src/contextvm/` (e.g. `npub.rs`) with pure-Rust encode + tests.

</code_context>

<specifics>
## Specific Ideas

- Cache-first render must feel instant. Profile target: < 16ms paint after navigation on a mid-tier Pixel.
- "Used N×" badge color/shape should match the existing accent style used for the agent step `Remote` provenance label (Phase 35-07, commit 6d6ed77) so the user reads the two as related signals.
- npub copy must work with a single tap (no menu) — the user explicitly wants the npub easy to grab for verifying or pasting elsewhere.
- The search field should be a primary affordance, not a hidden menu — the user described "search the tools" as a first-class action.
- Detail screen content order: tool name (h1) → "Used N× — last used 3d ago" (caption) → description (body) → "Advertised by" (label) → display name + npub (selectable) → "Schema" expander.

</specifics>

<deferred>
## Deferred Ideas

- iOS parity — same UX shape will be portable when iOS work resumes.
- Manual "Clear cache" action (no automatic pruning in v1).
- Per-provider grouping or filter (currently search by provider name covers the use case).
- Time-based auto-prune of stale disabled rows.
- Sorting toggles (name / last-used / last-seen) — list defaults to last-seen DESC for v1; sort UI deferred.
- Sealing/encrypting the contextvm_tools table beyond its current ActorState-managed protection.
- Showing relays each announcement was seen on (would require schema change).

</deferred>
