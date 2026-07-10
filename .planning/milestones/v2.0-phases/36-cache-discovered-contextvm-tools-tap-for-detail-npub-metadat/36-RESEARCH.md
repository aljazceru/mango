# Phase 36: Cache contextvm tools — tap-for-detail + search + used history — Research

**Researched:** 2026-05-08
**Domain:** Cross-platform native UI (Android Compose + Desktop iced) extending an existing Phase 35 sub-screen, plus Rust core additions for usage aggregation, npub bech32 encoding, search, and cache-first hydration.
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Area 1 — Cache scope & lifecycle**
- All discovered tools cached, including `enabled = 0` rows. Discovery upserts every row with the existing schema; the actor preserves the user's `enabled` flag across upserts (existing behavior).
- Optimistic render: on screen open, the actor populates `AppState.contextvm_tools` from `list_all_contextvm_tools` first, then auto-fires the existing `DiscoverContextvmTools` action to refresh in background. UI shows a subtle "Refreshing…" affordance only while the network query is in flight; the existing 5-state machine (`ContextvmDiscoveryState`) is reused — `Loading` keeps current "Searching Nostr relays…" copy.
- No auto-prune in v1 — disabled rows persist indefinitely. A future phase can add a manual "Clear cache" or time-based prune.
- Refresh trigger — manual Refresh button (already exists in the TopAppBar) plus the existing auto-fire on first composition stay as-is. No new periodic refresh.

**Area 2 — Tool detail screen**
- Whole-row navigation — tapping anywhere on a tool row (outside the trailing Switch) navigates to the detail screen. The Switch keeps its own click absorption so toggling enable/disable does not navigate.
- npub format — `provider_pubkey` (hex) is encoded to bech32 (`npub1…`) for display. Both the npub string and the truncated hex are shown; tap-to-copy with a Material Snackbar / iced toast confirmation.
- Schema display — `schema_json` is pretty-printed (`serde_json::to_string_pretty`) inside a monospaced, selectable, scrollable code block under a "Show schema" expander. Schema content remains plain text — no Markdown, no syntax injection surface.
- Copy actions — tap-to-copy for npub and full tool id; long-press select-all on schema. Confirmation surface uses platform-native ephemeral feedback (Snackbar on Android, a temporary status line in iced).

**Area 3 — Search UX**
- Always-visible search field below the TopAppBar on the existing `SettingsToolDiscoveryScreen` / iced equivalent. Single screen — no separate search route.
- Search scope — case-insensitive substring match across `tool_name`, `description`, and `provider_display_name` (NULL coalesces to empty). Match is in-memory over the already-loaded `AppState.contextvm_tools` vector.
- Live filter — filter applied per keystroke; no debounce.
- Empty result — show `No tools match "{query}"` caption while keeping the search field visible. The 5-state machine's existing Empty state stays for the unfiltered "0 tools discovered" case.

**Area 4 — Used-in-past tracking**
- Definition of "used" — any `agent_steps` row with `tool_origin = 'contextvm'` and matching `tool_name`, regardless of step status.
- Granularity — by `tool_name`. Justified by the existing `idx_contextvm_tools_name UNIQUE` index — tool_name is unique within the cache.
- Indicator on list row — small "Used N×" badge inline when `count > 0`, hidden otherwise. Full "Last used {relative time}" plus count surfaces on the detail screen.
- Compute location — actor runs a single `SELECT tool_name, COUNT(*), MAX(timestamp) FROM agent_steps WHERE tool_origin = 'contextvm' GROUP BY tool_name` query when populating `AppState.contextvm_tools` and after each invocation handler. Merged into a new `usage_count: u32` and `last_used_at: Option<i64>` field on `DiscoverableTool`. No denormalised column on `contextvm_tools` — keeps `agent_steps` as single source of truth.

### Claude's Discretion
- Choice of bech32 crate (`bech32` 0.11) vs reusing whatever `nostr` types contextvm-sdk already pulls in — pick whichever yields fewer dependencies after auditing `Cargo.lock`.
- Exact placement of relative-time formatting helper (reuse `relative_time_label` from Phase 32 plan 07 if signature fits; otherwise add a thin wrapper).
- Snackbar vs Toast vs inline confirmation — pick the existing pattern already used elsewhere on Android (likely Snackbar via `SnackbarHostState`).
- iced UI parity — match Android UX shape (search field, detail screen, badges) but follow existing iced patterns from Phase 32/34.1 (Column accumulator, `Task::perform` for async ops).

### Deferred Ideas (OUT OF SCOPE)
- iOS parity — same UX shape will be portable when iOS work resumes.
- Manual "Clear cache" action (no automatic pruning in v1).
- Per-provider grouping or filter (currently search by provider name covers the use case).
- Time-based auto-prune of stale disabled rows.
- Sorting toggles (name / last-used / last-seen) — list defaults to last-seen DESC for v1; sort UI deferred.
- Sealing/encrypting the contextvm_tools table beyond its current ActorState-managed protection.
- Showing relays each announcement was seen on (would require schema change).
</user_constraints>

## Project Constraints (from CLAUDE.md)

| Directive | How Phase 36 Honours It |
|-----------|--------------------------|
| RMP architecture: Rust core owns business logic, native UIs are thin | All search/aggregation/bech32 happens in Rust; UI consumes pre-computed UniFFI Records |
| Privacy: no telemetry, no cloud sync | Phase 36 adds zero network calls beyond the existing Phase 35 Nostr discovery already in scope; cache stays local |
| API compatibility: OpenAI-compatible only for LLMs | Out of scope — Phase 36 adds no LLM surface |
| Build: Nix flake + `just` + UniFFI bindings | Bindings regenerated on Rust struct changes (`just bindings-kotlin`, `just bindings-swift` even though iOS UI is deferred) |
| GSD enforcement: no direct edits | All file writes go through GSD plans/tasks |
| No OpenSSL in network stack | Bech32 + Nostr keys path stays pure-Rust (verified via `cargo tree` — see Standard Stack) |

## Summary

Phase 36 is a **mostly UI phase** with a **modest Rust-core delta**. All four user-visible features (cache-first render, tap-for-detail, search, "Used N×" badge) sit on top of foundations that already exist after Phase 35:

- The `contextvm_tools` table already persists every announced row [VERIFIED: `rust/src/persistence/schema.rs:364`].
- `AppState.contextvm_tools` is already hydrated from `list_all_contextvm_tools` at unlock-time [VERIFIED: `rust/src/lib.rs:4186`–`4274`] AND on every `DiscoverContextvmTools` completion [VERIFIED: `rust/src/lib.rs:8485`–`8499`].
- `agent_steps.tool_origin` already carries `"contextvm"` for remote tool calls [VERIFIED: `rust/src/lib.rs:2940`–`2948`].
- An existing helper `relative_time_label` produces the cross-platform "3d ago" labels [VERIFIED: `rust/src/lib.rs:937`].

**Key Rust core additions:**
1. **5 new `DiscoverableTool` fields** (UniFFI Record) — `usage_count: u32`, `last_used_at: Option<i64>`, `last_used_label: Option<String>`, `last_seen_at: i64`, `last_seen_label: String`, `npub: String`. The actor pre-computes all labels per the Phase 32 pattern; UI never branches on integers (UI-SPEC §Copywriting locks this).
2. **One new persistence query** — `aggregate_contextvm_tool_usage(conn) -> HashMap<String, (u32, i64)>` returning `(count, max_created_at)` keyed by `tool_name`.
3. **One new helper** — `encode_npub(provider_pubkey_hex) -> Result<String, _>` using the `nostr::nips::nip19::ToBech32` trait on `contextvm_sdk::signer::PublicKey` (the same type already imported at `rust/src/contextvm/discovery.rs:78`).
4. **One new `Screen` enum variant** — `Screen::ContextvmToolDetail { tool_id: String }`.
5. **One new AppAction** — `OpenContextvmToolDetail { tool_id: String }` (or reuse `PushScreen { screen: Screen::ContextvmToolDetail{..} }` directly; see §Architecture Patterns for the recommendation).
6. **No new SQLite migration.** All data lives in existing tables.

**Two cross-cutting gotchas the planner MUST acknowledge:**

- **Chat-tool path does NOT write to `agent_steps`.** Phase 27's chat-tool round dispatches via `dispatch_tools` but does not insert an `agent_steps` row [VERIFIED: `rust/src/lib.rs:8197`–`8253` — only message-history + AppState writes, no persistence call]. Therefore "Used N×" computed from `agent_steps` reflects ONLY agent-session uses, not chat-with-tools uses. This is a **scope-honest limitation** that matches CONTEXT Area-4's literal definition ("any agent_steps row with tool_origin='contextvm'"), but it should be documented in the plan and either (a) accepted as v1 behaviour, or (b) extended to also count chat-tool rounds via a new persistence path. Recommendation: accept (a) for v1, file a follow-up.
- **`relative_time_label` is missing weeks formatting.** It returns `"3d ago"`, `"Yesterday"`, `"5m ago"`, `"Just now"` (Capitalised) — but UI-SPEC Phase 36 quotes `"2w ago"` and lowercase `"just now"` in places (UI-SPEC §Layout caption examples). The function as-of `rust/src/lib.rs:937` does NOT emit `"Xw ago"`. Recommendation: extend `relative_time_label` to add `delta >= 7 * 86400 → "{w}w ago"`, OR keep as-is and update UI-SPEC to drop the "weeks" example. The planner picks; I recommend extending the helper since UI-SPEC is locked and weeks is a normal expectation.

**Primary recommendation:** One Wave-0 plan adds tests + new fields + npub encoder. One Wave-1 plan adds usage aggregation + label pre-compute + Screen variant + UniFFI binding regen. Then two parallel Wave-2 plans: Android Compose (search, badge, detail screen, snackbar copy) + Desktop iced (search input, badge, detail screen, inline status confirmation). Total: ~4 plans.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Cache hydration on screen open | Rust core (actor) | — | Already implemented; no change needed (RMP rule: actor owns DB) |
| Search/filter (live, no debounce) | Native UI (Compose / iced) | — | Pure in-memory `Vec<DiscoverableTool>` filter, ≤ tens of items; no FFI roundtrip per keystroke. Mirrors Compose `derivedStateOf` pattern at `SettingsDefaultsScreen.kt:52` |
| npub bech32 encoding | Rust core | — | Crypto-adjacent; must be pure-Rust; surfaces as a pre-computed string field on `DiscoverableTool` (RMP: native never imports crypto crates) |
| Pretty-printing schema JSON | Rust core (pre-compute) OR Native UI | — | Either works. Recommend Rust pre-compute via `serde_json::to_string_pretty` once on detail-screen open and store on a transient field, OR compute in UI. Locking to Rust pre-compute is simpler (see §Architecture Patterns). |
| Usage count + last-used aggregation | Rust core (actor SQL) | — | RMP: actor owns DB. One `GROUP BY` query; results merged into `DiscoverableTool` records during hydration |
| Tap-to-copy clipboard write | Native UI (Compose / iced) | — | Each platform has its own clipboard API; ALREADY used at `MainApp.kt:81–87` (Android) and `desktop/iced/src/main.rs:875` (iced). Reuse exact patterns. |
| Detail screen rendering | Native UI | — | Pure presentation; consumes `DiscoverableTool` fields plus a derived "schema_pretty" string |
| Screen routing (push detail screen) | Rust core (Screen enum) → Native UI dispatcher | — | RMP: nav stack lives in `AppState.router.screen_stack` |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `nostr` | 0.43.1 | npub bech32 encoding via `ToBech32` trait | Already in `Cargo.lock` transitively (pulled by `contextvm-sdk` → `nostr-sdk` → `nostr-relay-pool`). The exact `PublicKey` type from this crate is what `contextvm_sdk::signer::PublicKey` re-exports — already imported at `rust/src/contextvm/discovery.rs:78`. Adding `nostr` as a direct dep gives access to `nostr::nips::nip19::ToBech32` without wiring custom bech32 encoding. [VERIFIED: `Cargo.lock` lines `name = "nostr" version = "0.43.1"`] [CITED: docs.rs/nostr/latest/nostr/key/public_key — `to_bech32()` method via `ToBech32` trait import] |
| `serde_json` | 1.x | Pretty-print `schema_json` to monospaced display string | Already a direct dep; `serde_json::to_string_pretty` is the standard call |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `bech32` | 0.11.1 | Alternative pure-bech32 encoding | If the planner decides not to add the full `nostr` dep. Already in `Cargo.lock` transitively. Requires manual conversion: 32-byte hex → 5-bit groups → `bech32::encode("npub", ...)`. [VERIFIED: `Cargo.lock`] More code than the `ToBech32` trait approach. **NOT recommended** unless `nostr` causes UniFFI/binding issues. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `nostr` (full) | `bech32 = "0.11"` direct | Avoids importing 30 KLOC of Nostr machinery just for one trait. But `nostr` is already a transitive — `cargo tree` shows it pulled by `contextvm-sdk`. Adding it as a direct dep is 0 new wall-clock build time and 0 new compile units. The planner can run `cargo tree -p mango_core 2>&1 \| grep -i nostr` to confirm before committing. |
| Pre-computed `npub` field on `DiscoverableTool` | Compute in UI | UI tier lacks bech32; Rust core is the right home. Pre-computing in the actor is ~40ns per row, called only at hydration time. |
| Pre-computed `schema_pretty` string in `DiscoverableTool` | Compute in UI on detail open | Compose has `JSON.serialize` in stdlib? No — Compose UI dep set deliberately excludes JSON parsers. iced has none either. Rust must pre-compute. We recommend exposing `schema_pretty: Option<String>` ONLY on a new "detailed" record (or computing on-demand via a new `FfiApp::get_pretty_schema(tool_id)` UniFFI fn). The simpler choice: include `schema_pretty: String` on `DiscoverableTool` itself — adds ~1KB per row × tens of rows = trivial. |

**Cargo.toml addition:**
```toml
nostr = { version = "0.43", default-features = false, features = ["std"] }
```
(Verify minimal feature set works for the `ToBech32` trait — see Wave 0 verification below.)

**Version verification:**
- `nostr 0.43.1` confirmed in `Cargo.lock`. [VERIFIED: `Cargo.lock`]
- `bech32 0.11.1` confirmed in `Cargo.lock`. [VERIFIED: `Cargo.lock`]
- `contextvm-sdk 0.1.1` confirmed in `Cargo.lock` (Phase 35 ships this). [VERIFIED: `Cargo.lock`, `rust/Cargo.toml:88`]

**OpenSSL audit reminder:** Adding `nostr` as a direct dep does NOT introduce new transitive crates (it is already pulled). The planner MUST still run `cargo tree -p mango_core 2>&1 | grep -iE "openssl-sys|native-tls"` after adding the line and confirm no new edges. Phase 35 RESEARCH §H locked the existing baseline (only `openssl-sys 0.9.113` via `rusqlite`'s SQLCipher vendored bundle). [CITED: `.planning/phases/35-add-contextvm-sdk-for-nostr-based-tool-discovery/35-RESEARCH.md` §H]

## Architecture Patterns

### System Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────────────┐
│ User opens "Discover Tools" screen                                        │
│   │                                                                       │
│   ▼                                                                       │
│ Native UI (Compose / iced)                                                │
│   ▼                                                                       │
│  AppState.contextvm_tools (already hydrated by actor at unlock-time)      │
│  ├─ Renders cached list IMMEDIATELY (no FFI await)                        │
│  └─ LaunchedEffect/Task fires DiscoverContextvmTools (existing)           │
│                                                                           │
│ User types in search box                                                  │
│   │                                                                       │
│   ▼                                                                       │
│  remember { mutableStateOf("") } / state.contextvm_search_query (UI-only) │
│  derivedStateOf { tools.filter(predicate) } — keystroke-by-keystroke      │
│                                                                           │
│ User taps a tool row                                                      │
│   │                                                                       │
│   ▼                                                                       │
│  AppAction::PushScreen{Screen::ContextvmToolDetail{tool_id}}              │
│   │                                                                       │
│   ▼                                                                       │
│  Rust actor pushes screen onto router stack, emits FullState              │
│   │                                                                       │
│   ▼                                                                       │
│  Native UI dispatches to ContextvmToolDetailScreen.kt / tool_detail.rs    │
│  Looks up DiscoverableTool by id in AppState.contextvm_tools (O(N), N≤30) │
│  Renders: name (h1) → caption (Used N× — last used 3d ago) → description  │
│           → ADVERTISED BY (display name + npub + hex) → USAGE → SCHEMA    │
│                                                                           │
│ User taps "Copy" on npub row                                              │
│   ▼                                                                       │
│  Compose: ClipboardManager.setPrimaryClip + SnackbarHostState.show("npub copied")
│  iced: iced::clipboard::write(npub) returned as Task; status line updated │
│                                                                           │
│                                                                           │
│ Concurrent path: agent dispatches a contextvm tool                        │
│   │                                                                       │
│   ▼                                                                       │
│  Actor inserts agent_steps row with tool_origin="contextvm" (existing)    │
│   ▼                                                                       │
│  Actor re-runs aggregate_contextvm_tool_usage() → updates                 │
│  AppState.contextvm_tools[*].usage_count + last_used_at                   │
│  emits FullState — UI shows updated badge on next render                  │
└──────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Path | Role |
|-----------|------|------|
| `DiscoverableTool` Record | `rust/src/lib.rs:163` (extend) | UniFFI surface; gains `usage_count`, `last_used_at`, `last_used_label`, `last_seen_at`, `last_seen_label`, `npub`, `schema_pretty` |
| `Screen::ContextvmToolDetail` | `rust/src/lib.rs:451` (Screen enum) | New variant; carries `tool_id: String` |
| `aggregate_contextvm_tool_usage` | `rust/src/persistence/queries.rs` (new fn) | One `GROUP BY tool_name` SQL query reading from `agent_steps` |
| `encode_npub` helper | `rust/src/contextvm/npub.rs` (new file) OR `mod.rs` (inline fn) | Pure-Rust hex → npub bech32 |
| `compute_discoverable_tools` (refactor existing) | `rust/src/lib.rs:3512` (`row_to_discoverable_tool` — extend) | Threads usage map + now_secs + npub through the projection |
| `SettingsToolDetailScreen` | `android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDetailScreen.kt` (new) | Compose detail screen |
| `tool_detail` view | `desktop/iced/src/views/tool_detail.rs` (new) | iced detail screen |
| Search field + Used badge + chevron + clickable row | `SettingsToolDiscoveryScreen.kt` (extend) + `tool_discovery.rs` (extend) | Modifications inside existing files; no new top-level screens |
| Android nav | `MainApp.kt:189` (extend `is Screen.ToolDiscovery` block + add new `is Screen.ContextvmToolDetail` arm) | Mirrors existing `SettingsToolDiscoveryScreen` arm at lines 189–195 |
| Desktop nav | `desktop/iced/src/main.rs:1905` (extend; add detail-screen overlay block) | Mirrors existing `Screen::ToolDiscovery` overlay |

### Pattern 1: UniFFI Record evolution with pre-computed display fields

**What:** Extend `DiscoverableTool` with new fields. Each new field crosses the UniFFI boundary; UI consumes them as plain Kotlin / Swift data classes.

**When to use:** Phase 32 plan 07 established the pattern (`DirectorySourceSummary.last_synced_label`). It's the project's canon: actor pre-computes all display strings; UI never formats integers.

**Example:**
```rust
// rust/src/lib.rs (extend struct)
#[derive(uniffi::Record, Clone, Debug)]
pub struct DiscoverableTool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub provider_pubkey: String,           // hex (existing)
    pub provider_display_name: Option<String>,
    pub enabled: bool,
    // ── Phase 36 additions ──
    pub usage_count: u32,                  // 0 if never used
    pub last_used_at: Option<i64>,         // unix seconds, None if never used
    pub last_used_label: Option<String>,   // pre-computed e.g. "3d ago" or None
    pub last_seen_at: i64,                 // unix seconds (mirror DB field)
    pub last_seen_label: String,           // pre-computed e.g. "Just now"
    pub npub: String,                      // bech32 npub1… encoding of provider_pubkey
    pub schema_pretty: String,             // serde_json::to_string_pretty applied to schema_json
}
```

[CITED: `rust/src/lib.rs:911`–`923` (DirectorySourceSummary.last_synced_label sets the precedent)]

### Pattern 2: Compose live filter via `derivedStateOf`

**What:** Per-keystroke filter over an in-memory list, recomputed on either `query` or `tools` change.

**Example:**
```kotlin
// SettingsToolDiscoveryScreen.kt — inside the @Composable
var query by remember { mutableStateOf("") }
val tools = appState.contextvmTools // list from FullState
val filtered by remember(tools) {
    derivedStateOf {
        val q = query.trim().lowercase()
        if (q.isEmpty()) tools
        else tools.filter { tool ->
            tool.name.lowercase().contains(q)
                || tool.description.lowercase().contains(q)
                || (tool.providerDisplayName ?: "").lowercase().contains(q)
        }
    }
}

OutlinedTextField(
    value = query,
    onValueChange = { query = it },
    placeholder = { Text("Search tools") },
    leadingIcon = { Icon(Icons.Outlined.Search, contentDescription = null) },
    singleLine = true,
    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
)
```

Reference filter idiom in this project: `SettingsDefaultsScreen.kt:52` (`.filter { ... }`). [CITED: `SettingsDefaultsScreen.kt:52`]
Reference TextField pattern: `SettingsToolsScreen.kt:92`. [CITED: `SettingsToolsScreen.kt:92`]

### Pattern 3: iced live filter via per-frame `view()` recompute

**What:** iced has no `derivedStateOf`; the entire view re-renders on every state change. Filter happens inline in `view()`.

**Example:**
```rust
// desktop/iced/src/views/tool_discovery.rs (extend)
let q = search_query.trim().to_lowercase();
let filtered: Vec<&DiscoverableTool> = state.contextvm_tools
    .iter()
    .filter(|t| {
        if q.is_empty() { return true; }
        t.name.to_lowercase().contains(&q)
            || t.description.to_lowercase().contains(&q)
            || t.provider_display_name.as_deref().unwrap_or("").to_lowercase().contains(&q)
    })
    .collect();

let search_input = text_input("Search tools", &search_query)
    .on_input(Message::ContextvmSearchChanged)
    .size(14)
    .padding(8)
    .width(Length::Fill);
```

The `search_query: String` lives in `desktop/iced/src/main.rs` as a top-level UI-state field (similar to existing `input_text`, `agent_task_input`, `system_prompt_text`). Cleared on `Screen::ToolDiscovery` exit (Compose `remember` semantics). [CITED: `desktop/iced/src/main.rs` for existing UI-state fields pattern]

### Pattern 4: Cache-first render is already in place

**What:** The Phase 35 actor already populates `AppState.contextvm_tools` from `list_all_contextvm_tools` at unlock-time (`rust/src/lib.rs:4186`–`4191`) AND on every set-tool-enabled / discovery-complete handler. The Compose / iced screens render `appState.contextvmTools` synchronously on first composition.

**Phase 36 implication:** **No new wiring needed for cache-first render.** The screen already renders cached rows on open. The only behavioural change is that the `LaunchedEffect(Unit) { onDispatch(DiscoverContextvmTools) }` at `SettingsToolDiscoveryScreen.kt:66` should NOT clear the list while loading — and it doesn't (the actor at `lib.rs:6219` only sets `contextvm_discovery_state = Loading` without touching `app_state.contextvm_tools`). **Verified by code reading.** [VERIFIED: `rust/src/lib.rs:6219`–`6234`]

The 5-state machine flow becomes:
- On screen open: `AppState.contextvm_tools` has cached rows; `state = Idle`. Screen renders the list.
- `LaunchedEffect` fires `DiscoverContextvmTools` → state goes to `Loading`. Refresh button disabled (existing). **List stays rendered.** This is what we want — "subtle Refreshing affordance" = the disabled Refresh button.
- On completion: state → `Loaded`, list refreshed via `list_all_contextvm_tools` reload.

The current `SettingsToolDiscoveryScreen.kt:97`–`102` switches to `LoadingState()` (centred spinner + "Searching Nostr relays…") for `Idle | Loading`. **This is the bug to fix in Phase 36:** during cache-first re-render, we should show the cached list, not the spinner. The fix is small — change the `when` clause so `Loading` only shows the spinner when `contextvmTools.isEmpty()`:

```kotlin
when (val ds = appState.contextvmDiscoveryState) {
    is ContextvmDiscoveryState.Idle, is ContextvmDiscoveryState.Loading -> {
        if (appState.contextvmTools.isEmpty()) LoadingState()
        else ToolList(appState.contextvmTools, /* with search */)
    }
    is ContextvmDiscoveryState.Error -> /* same */
    is ContextvmDiscoveryState.Loaded -> /* same */
}
```

Same fix in `desktop/iced/src/views/tool_discovery.rs:84`–`96`.

### Pattern 5: Detail-screen routing — Screen variant with parameter

**What:** Add a new `Screen` enum variant carrying the tool id; push via `AppAction::PushScreen`. Mirrors `Screen::Chat { conversation_id }` and `Screen::Onboarding { step }` patterns already in the enum.

**Example:**
```rust
// rust/src/lib.rs:451 (Screen enum, after ToolDiscovery)
ContextvmToolDetail { tool_id: String },
```

```kotlin
// SettingsToolDiscoveryScreen.kt list row Modifier:
Modifier.clickable {
    onDispatch(AppAction.PushScreen(
        screen = Screen.ContextvmToolDetail(toolId = tool.id)
    ))
}
```

```rust
// desktop/iced/src/views/tool_discovery.rs row wrapper:
button(row_content)
    .on_press(Message::DispatchAction(
        AppAction::PushScreen { screen: Screen::ContextvmToolDetail { tool_id: id.clone() } }
    ))
```

**Why no new AppAction needed:** `PushScreen` already dispatches navigation. Adding `OpenContextvmToolDetail` would be redundant. The existing `AppAction::PopScreen` handles back-button.

**Detail screen looks up the tool:** O(N) scan of `AppState.contextvm_tools` by id. N ≤ tens of rows; trivial.

### Anti-Patterns to Avoid

- **Don't compute `usage_count` or labels in the UI.** Native side has no SQLite access; would require a new FFI roundtrip per render. Pre-compute in actor.
- **Don't hand-roll bech32.** `nostr::nips::nip19::ToBech32` already exists in the dep graph; reuse it. [CITED: docs.rs/nostr]
- **Don't add a separate detail-screen `AppState.contextvm_tool_detail: Option<DetailRecord>` field.** The detail screen reads from the existing `contextvm_tools` Vec by id. Adding a parallel state would require de-sync handling on toggle/refresh. The Vec lookup is the canonical pattern (mirrors how `Screen::Chat { conversation_id }` looks up via `current_conversation_id` against `messages`/`conversations`). [CITED: `rust/src/lib.rs` AppState pattern]
- **Don't debounce the search filter.** CONTEXT D-search-3 explicitly forbids it; cardinality is bounded. UI-SPEC §Interaction confirms.
- **Don't store search query in AppState.** Per UI-SPEC §Layout: "persistence across screens is not required; clearing on screen pop is acceptable." Use Compose `remember` / iced top-level field. Storing in `AppState` would force a UniFFI roundtrip per keystroke and clutter `AppState`.
- **Don't render schema as Markdown.** UI-SPEC locks plain-text-only — eliminates injection surface.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Bech32 encoding (npub format) | Custom 5-bit-group conversion + checksum | `nostr::nips::nip19::ToBech32::to_bech32()` on `PublicKey` | Already in dep graph; battle-tested by Nostr ecosystem; correct HRP handling |
| Pretty-printing JSON | Custom indentation logic | `serde_json::to_string_pretty(&value)` | Already a direct dep; standard idiom |
| Relative-time labels | Per-platform date math | Reuse Phase 32 `relative_time_label(last, now)` (extend with weeks if UI-SPEC requires) | One canonical source; cross-platform consistency |
| List filter | Custom debounce + worker thread | Compose `derivedStateOf` / iced view recompute | List is small, recompute is < 1ms |
| Clipboard write | Reflection / JNI manipulation | Existing `ClipboardManager` (Android, in `MainApp.kt:81`) and `iced::clipboard::write` (Desktop, in `main.rs:875`) | Already plumbed in this codebase |

**Key insight:** Every cross-platform rendering primitive Phase 36 needs is already wired by an earlier phase. The work is **plumbing pre-computed strings through UniFFI**, not building new infrastructure.

## Common Pitfalls

### Pitfall 1: Aggregating usage from `agent_steps.action_payload` JSON
**What goes wrong:** `agent_steps.action_payload` is a JSON ARRAY of `{id, name, arguments}` — a single row may represent multiple parallel tool calls. A naive `GROUP BY tool_name` query won't work because there's no `tool_name` column on `agent_steps`.
**Why it happens:** The schema treats a "tool_call step" as an atomic checkpoint that may include >1 tool invocation. [VERIFIED: `rust/src/lib.rs:2922`–`2932`]
**How to avoid:** Two options:
- **Option A — pull-and-parse in Rust** (recommended): `SELECT action_payload, created_at FROM agent_steps WHERE tool_origin = 'contextvm' AND action_type = 'tool_call'`. For each row, parse the JSON array and extract each `name`. Aggregate in a `HashMap<String, (u32 count, i64 max_at)>`. Linear-time on number of tool_call steps; trivial for v1 cardinality.
- **Option B — `json_extract` SQLite function**: rusqlite has SQLite's JSON1 extension available with the `bundled-sqlcipher-vendored-openssl` feature (verify). Query like `SELECT json_extract(value, '$.name') AS tool_name, COUNT(*), MAX(created_at) FROM agent_steps, json_each(action_payload) WHERE tool_origin = 'contextvm' GROUP BY tool_name`. More elegant but ties to a SQLite extension.

**Recommendation:** Option A. Easier to test, no SQLite-extension portability question. Run once on hydration + once after each tool dispatch handler.

**Warning sign:** A query returning empty `Vec` even when agent has invoked remote tools — likely indicates the query expects a `tool_name` column that doesn't exist.

### Pitfall 2: `relative_time_label` lacks weeks formatting
**What goes wrong:** UI-SPEC §Copywriting examples include `"2w ago"` but the existing helper at `rust/src/lib.rs:937` only emits `"Xd ago"` and never weeks.
**Why it happens:** Helper added in Phase 32 only needed days for directory-source last-synced.
**How to avoid:** Extend with `delta >= 7 * 86400 → format!("{}w ago", delta / (7 * 86400))` BEFORE the existing `_ => format!("{}d ago", ...)` arm. Add a unit test mirroring `directory_rag.rs:534`. **Be careful not to regress `"Yesterday"` / `"3d ago"` outputs already locked by Phase 32 callers.**
**Warning signs:** Cross-platform display difference between Phase 32 directory-sync labels (currently "5d ago") and Phase 36 tool-usage labels (would say "5d ago" before fix, "5d ago" after fix). Both stable. The fix only adds a new branch (≥7d → weeks).

### Pitfall 3: Chat-tool-round path doesn't write `agent_steps`
**What goes wrong:** A user invokes a contextvm tool via chat-with-tools (Phase 27 path). No row is inserted into `agent_steps`. "Used N×" badge stays at 0 forever.
**Why it happens:** Phase 27's `ChatToolCallsReady` handler dispatches `agent::dispatch_tools` and synthesises tool-result messages directly into `messages: Vec<ChatCompletionRequestMessage>` — no DB write. [VERIFIED: `rust/src/lib.rs:8197`–`8253`]
**How to avoid:** Two options:
- **Accept the limitation for v1.** Document that "Used N×" reflects only agent-session uses. Matches CONTEXT Area-4's literal SQL.
- **Extend the chat-tool path to insert a step row** with `tool_origin = "contextvm"` whenever a contextvm tool name appears in the call set. Mirrors the existing logic at `lib.rs:2940`–`2948`. Slightly more invasive.
**Recommendation:** Accept v1; file follow-up. The user's primary path for "expert tools" is the agent loop, where the tracking already works.

### Pitfall 4: UniFFI bindings drift after struct change
**What goes wrong:** Rust adds new fields to `DiscoverableTool`; Android/iOS bindings not regenerated; Kotlin/Swift code references the new fields but `appState.contextvmTools[i].usageCount` doesn't exist.
**Why it happens:** UniFFI bindings are committed checked-in artifacts (proc-macro generation, no UDL). Phase 24/25 hit this. [CITED: STATE.md note "UniFFI bindings regenerated in Wave 2 plan (not Wave 1)"]
**How to avoid:** A dedicated bindings-regen plan after the Rust core changes land but before any UI plan starts. Run `just bindings-kotlin` AND `just bindings-swift` (even though iOS is deferred — bindings stay current per Phase 35 pattern). Commit the regen output as its own commit so reviewers can see the diff.
**Warning signs:** Compose compile error: "Unresolved reference: usageCount". Means bindings out of date.

### Pitfall 5: Live filter reverting on background refresh
**What goes wrong:** User has typed `"weather"` and is looking at filtered results. A `DiscoverContextvmTools` completes; `AppState.contextvm_tools` is replaced; the filter's `derivedStateOf` recomputes against the new list — but the search field's text input loses cursor / selection if the surrounding `Composable` recomposes too aggressively.
**Why it happens:** Compose `OutlinedTextField` is well-behaved here, but if the `query` state was nested inside a non-stable `remember`, recomposition can lose focus.
**How to avoid:** Hoist `query` to a `remember { mutableStateOf("") }` at the top of `SettingsToolDiscoveryScreen` (NOT inside any conditional `if/when` block). Pass it down to filter logic. Same lifecycle as the screen.
**Warning signs:** Cursor jumps to start of input on each refresh tick.

### Pitfall 6: npub encoding fails for malformed hex
**What goes wrong:** A row in `contextvm_tools` has `provider_pubkey = "deadbeef"` (32 hex chars instead of 64). `PublicKey::from_hex` returns `Err`. If the actor unwraps, panic. If silent, the detail screen shows an empty `npub` field.
**Why it happens:** Discovery from Phase 35 uses `discover_servers` which extracts pubkey from a Nostr event (always 32 bytes if event is well-formed). But a future bad upsert path could write garbage.
**How to avoid:** `encode_npub(hex) -> String` should return a fallback string `format!("invalid: {}", &hex[..8.min(hex.len())])` on error, never panic. UI displays it as-is — the user sees the broken state plainly. Add a unit test for "invalid hex" input.
**Warning signs:** Logs show `bad pubkey` warnings during tool detail open.

### Pitfall 7: Schema pretty-print on a non-object schema
**What goes wrong:** A provider announces a tool whose `inputSchema` is the empty string, `null`, or an unparseable mess. `serde_json::to_string_pretty` errors out.
**Why it happens:** Phase 35 `from_row` already validates the schema parses to `serde_json::Value` and rejects rows that don't [VERIFIED: `rust/src/contextvm/dispatch.rs:55`–`66`]. So `contextvm_tools` rows in DB are guaranteed parseable. **However**, if Phase 36 changes the `schema_pretty` projection path, it must reuse the same `serde_json::from_str` step.
**How to avoid:** Project `schema_pretty` from the parsed `serde_json::Value`, not from the raw string. Or call `serde_json::from_str(&schema_json).and_then(serde_json::to_string_pretty).unwrap_or_else(|_| schema_json.clone())` — fall back to the raw string on error.
**Warning signs:** Detail screen shows `null` or empty for the schema body when a row exists.

## Code Examples

Verified patterns from existing code:

### Tool-by-id lookup in detail screen (Compose)
```kotlin
// SettingsToolDetailScreen.kt — pattern
@Composable
fun SettingsToolDetailScreen(
    appState: AppState,
    toolId: String,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit = { onDispatch(AppAction.PopScreen) },
) {
    val tool = appState.contextvmTools.firstOrNull { it.id == toolId }
    if (tool == null) {
        // Edge case: id not in cache — should never happen if reached via row tap
        Scaffold(topBar = { /* back arrow only */ }) {
            Text("Tool not found", modifier = Modifier.padding(it).padding(32.dp))
        }
        return
    }
    // ... render heading / sections ...
}
```
[CITED: pattern matches `LoadConversation` lookup at `rust/src/lib.rs` and Compose `firstOrNull` idioms across screens]

### Snackbar copy confirmation (Compose)
```kotlin
val snackbarHostState = remember { SnackbarHostState() }
val scope = rememberCoroutineScope()
val context = LocalContext.current

Scaffold(
    snackbarHost = { SnackbarHost(snackbarHostState) },
    /* ... */
) { padding ->
    /* ... */
    IconButton(onClick = {
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
        if (clipboard != null) {
            clipboard.setPrimaryClip(ClipData.newPlainText("npub", tool.npub))
            scope.launch { snackbarHostState.showSnackbar("npub copied") }
        } else {
            scope.launch { snackbarHostState.showSnackbar("Couldn't copy — try again") }
        }
    }) {
        Icon(Icons.Outlined.ContentCopy, contentDescription = "Copy npub")
    }
}
```
[CITED: clipboard pattern from `MainApp.kt:81`–`87`]

### iced inline status confirmation
```rust
// desktop/iced/src/main.rs (UI state field, sibling to existing input_text)
copy_status: Option<String>,
copy_status_at: Option<std::time::Instant>,

// Message handler:
Message::CopyNpub(npub) => {
    let task = iced::clipboard::write(npub);
    *copy_status = Some("npub copied".into());
    *copy_status_at = Some(std::time::Instant::now());
    return Task::batch([
        task,
        Task::perform(
            tokio::time::sleep(std::time::Duration::from_secs(2)),
            |_| Message::ClearCopyStatus,
        ),
    ]);
}
Message::ClearCopyStatus => {
    *copy_status = None;
}
```
[CITED: `iced::clipboard::write` usage at `desktop/iced/src/main.rs:875`; Task::perform pattern from Phase 32]

### Aggregate usage SQL (option A — pull and parse in Rust)
```rust
// rust/src/persistence/queries.rs (new fn)
pub fn fetch_contextvm_tool_usage_rows(
    conn: &Connection,
) -> Result<Vec<(String, i64)>, PersistenceError> {
    // Returns (action_payload_json, created_at) for every tool_call step
    // tagged contextvm. Caller parses the JSON array and aggregates.
    let mut stmt = conn.prepare_cached(
        "SELECT action_payload, created_at \
         FROM agent_steps \
         WHERE tool_origin = 'contextvm' AND action_type = 'tool_call'",
    )?;
    let rows: Result<Vec<_>, _> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect();
    Ok(rows?)
}
```

```rust
// rust/src/lib.rs (new helper)
fn aggregate_contextvm_tool_usage(
    conn: &rusqlite::Connection,
) -> std::collections::HashMap<String, (u32, i64)> {
    let mut acc: std::collections::HashMap<String, (u32, i64)> = std::collections::HashMap::new();
    let rows = persistence::queries::fetch_contextvm_tool_usage_rows(conn).unwrap_or_default();
    for (payload, created_at) in rows {
        let calls: Vec<serde_json::Value> = serde_json::from_str(&payload).unwrap_or_default();
        for call in calls {
            if let Some(name) = call.get("name").and_then(|n| n.as_str()) {
                let entry = acc.entry(name.to_string()).or_insert((0, 0));
                entry.0 += 1;
                if created_at > entry.1 {
                    entry.1 = created_at;
                }
            }
        }
    }
    acc
}
```

### npub encoding helper
```rust
// rust/src/contextvm/mod.rs (or new file npub.rs)
pub fn encode_npub(provider_pubkey_hex: &str) -> String {
    use contextvm_sdk::signer::PublicKey;
    use nostr::nips::nip19::ToBech32;
    match PublicKey::from_hex(provider_pubkey_hex) {
        Ok(pk) => pk.to_bech32().unwrap_or_else(|_| {
            log::warn!("npub bech32 encoding failed for pubkey: {}", provider_pubkey_hex);
            format!("invalid:{}", provider_pubkey_hex.chars().take(8).collect::<String>())
        }),
        Err(e) => {
            log::warn!("invalid hex pubkey '{}': {}", provider_pubkey_hex, e);
            format!("invalid:{}", provider_pubkey_hex.chars().take(8).collect::<String>())
        }
    }
}
```
[CITED: `contextvm_sdk::signer::PublicKey` already imported at `rust/src/contextvm/discovery.rs:78`; `ToBech32` trait per docs.rs/nostr]

## Runtime State Inventory

> Phase 36 is additive (new fields + new screen + UI changes). NOT a rename / refactor / migration. Section omitted.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All Rust core changes | ✓ | 1.x (workspace MSRV) | — |
| `cargo` | Build/test | ✓ | (workspace) | — |
| `nostr` crate registry | Phase 36 npub encoding | ✓ | 0.43.1 (already in Cargo.lock transitively) | Could fall back to `bech32 = "0.11"` direct (also already in Cargo.lock) |
| Android SDK + Compose | Android UI plan | ✓ (existing build) | per project | — |
| iced 0.13 | Desktop UI plan | ✓ (existing build) | 0.13 | — |
| `just` task runner | Bindings regen | ✓ (existing) | — | — |
| `uniffi-bindgen` | Bindings regen | ✓ (existing build dep) | 0.29 | — |

**Missing dependencies with no fallback:** None.
**Missing dependencies with fallback:** None — both bech32 paths (full `nostr` and direct `bech32`) are already in `Cargo.lock`.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` (unit + integration tests under `rust/src/tests/`); Android Compose UI test framework; iced unit-style tests on view fns |
| Config file | `rust/Cargo.toml` (test deps already configured); Android `app/build.gradle.kts` |
| Quick run command | `cargo test -p mango_core --lib` (Rust unit tests, < 30s) |
| Full suite command | `cargo test -p mango_core` (all Rust tests including integration) |

### Phase Requirements → Test Map

(Note: Phase 36 has no formal `REQ-XX` IDs assigned yet in REQUIREMENTS.md; the planner should assign CTX36-01..CTX36-NN. Mapping below is by behavioural area from CONTEXT.)

| Behaviour | Test Type | Automated Command | File Exists? |
|-----------|-----------|-------------------|-------------|
| Cache-first render: tools hydrate on screen open before refresh completes | unit (Rust) | `cargo test -p mango_core test_appstate_contextvm_tools_hydrated_at_unlock` | Pattern exists at `rust/src/tests/contextvm.rs`; new test ❌ Wave 0 |
| Aggregate usage from `agent_steps` returns correct count + last_used | unit | `cargo test -p mango_core test_aggregate_contextvm_usage_groups_by_name` | ❌ Wave 0 |
| Aggregate handles JSON action_payload with multiple tool_calls per row | unit | `cargo test -p mango_core test_aggregate_handles_multi_tool_payload` | ❌ Wave 0 |
| Aggregate excludes `tool_origin = 'local'` rows | unit | `cargo test -p mango_core test_aggregate_excludes_local_origin` | ❌ Wave 0 |
| `encode_npub` produces correct npub1… for known hex | unit | `cargo test -p mango_core test_encode_npub_known_vector` | ❌ Wave 0 |
| `encode_npub` returns fallback string for invalid hex (no panic) | unit | `cargo test -p mango_core test_encode_npub_fallback_on_invalid` | ❌ Wave 0 |
| `relative_time_label` extension: weeks formatting | unit | `cargo test -p mango_core test_relative_time_labels_weeks` (extend existing test in `directory_rag.rs:534`) | Existing test ✅; add cases |
| `DiscoverableTool` projection includes pre-computed labels + npub | unit | `cargo test -p mango_core test_row_to_discoverable_tool_phase36_fields` | ❌ Wave 0 |
| Search filter: case-insensitive, matches name/description/provider | unit (or component) | Pure-function helper testable in Rust if extracted; otherwise Compose UI test | ❌ Wave 0 (recommended: Rust pure fn) |
| Detail screen routing: PushScreen variant constructable + dispatched | integration | `cargo test -p mango_core test_pushscreen_contextvm_tool_detail` | ❌ Wave 0 |
| Snackbar shows on Copy click (Android) | manual / Compose UI test | Manual smoke + optional Compose `composeTestRule.onNodeWithText("npub copied")` | manual-only acceptable per project pattern |
| iced inline status appears on Copy + clears after 2s | manual | manual smoke | manual-only |

### Sampling Rate
- **Per task commit:** `cargo test -p mango_core --lib` (fast unit tests, < 30s)
- **Per wave merge:** `cargo test -p mango_core` (full suite)
- **Phase gate:** Full suite green; Android `./gradlew :app:assembleDebug`; Desktop `cargo build -p mango-desktop`

### Wave 0 Gaps
- [ ] `rust/src/tests/contextvm.rs` — extend with `test_aggregate_contextvm_usage_*` and `test_encode_npub_*` test stubs (RED → GREEN in Wave 1)
- [ ] `rust/src/tests/directory_rag.rs:534` — extend `test_relative_time_labels` with weeks cases
- [ ] `rust/src/tests/contextvm.rs` — add `test_row_to_discoverable_tool_phase36_fields` to lock UniFFI Record shape

*(Existing test infrastructure from Phase 35 covers actor + UniFFI integration patterns; only new behavioural tests required.)*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Phase 36 adds no auth surface |
| V3 Session Management | no | No new sessions |
| V4 Access Control | no | All data is the user's own; cache table inherits actor protection |
| V5 Input Validation | yes | Search input is plain string compared in-memory; no SQL injection surface (no DB roundtrip per keystroke); npub fallback handles invalid hex; schema_pretty falls back to raw string on parse error |
| V6 Cryptography | yes (small) | bech32 encoding via `nostr` (no hand-roll). No keys generated. No signatures. |
| V7 Error Handling | yes | npub failure logs `warn!` and shows fallback string in UI; clipboard failure shows "Couldn't copy" copy |
| V11 Business Logic | yes | "Used N×" badge is informational only; no security claim derived from it |

### Known Threat Patterns for Phase 36 stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Untrusted tool description / display name rendered in UI | Tampering / Spoofing | Phase 35 already caps description at 500 chars and renders as plain `Text`/`text` (no Markdown). Phase 36 inherits. |
| Schema JSON injection in display | Tampering | Pretty-printed plain-text only; no Markdown parser; UI-SPEC explicitly locks plain-text-only |
| Search query containing SQL meta-characters | Tampering | No DB query — pure in-memory `String::contains` filter |
| npub bech32 collision / spoofed pubkey | Spoofing | Out of scope — npub is just a display projection of the same hex pubkey already validated by Nostr signature in Phase 35 discovery |
| Clipboard write of sensitive data | Information Disclosure | npub and tool ID are public identifiers; no secret data flows through clipboard |
| Detail screen tool-id mismatch (race with refresh) | DoS | Detail screen renders "Tool not found" gracefully if id no longer in `contextvm_tools` (e.g. user rolled cache prune in a future plan) — no panic |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `nostr 0.43.1`'s `ToBech32` trait works on `contextvm_sdk::signer::PublicKey` (the type they re-export from `nostr-sdk`) | Standard Stack, Code Examples | Low — both crates wrap the same `nostr::PublicKey`. If the trait import path differs in 0.43.1, planner picks `bech32 = "0.11"` direct path. Verify in Wave 0 with one-line test: `let n: String = pk.to_bech32().unwrap();` |
| A2 | `serde_json::to_string_pretty` works on the parsed schema `serde_json::Value` for all typical contextvm tool schemas | Code Examples | Low — `to_string_pretty` is total on `Value`. The risk is that `from_str` fails on a malformed schema_json, which Phase 35 `from_row` already filters [VERIFIED: `dispatch.rs:55`–`66`]. Fallback to raw string handles edge cases. |
| A3 | Compose `derivedStateOf` triggers recompute on `tools` change OR `query` change without manual key plumbing | Pattern 2 | Low — standard Compose idiom. If Compose recomposes too eagerly, planner adds `key(tools)` wrapper around the `derivedStateOf`. |
| A4 | Phase 27 chat-tool path will not be extended to write `agent_steps` rows in Phase 36 (CONTEXT defines "used" via agent_steps only) | Pitfall 3 | Medium — if user expects chat-tool uses counted, planner needs to make a v1 scope decision. CONTEXT D-Area-4 SQL is literal. Recommendation: accept as v1 limitation, document in 36-VERIFICATION.md. |

**If this table is empty:** N/A — assumptions are flagged for the planner to lock or break in discuss-phase.

## Open Questions

1. **Where does `query: String` live for the search field?**
   - What we know: CONTEXT marks "persistence across screens not required". UI-SPEC §Layout L permits per-screen state.
   - What's unclear: Compose `remember { mutableStateOf("") }` on the existing screen vs. an `AppState.contextvm_search_query` field added to AppState (would survive screen recomposition but not pop).
   - Recommendation: **Use Compose `remember`** for Android and an iced top-level `String` field for desktop. Keep `AppState` clean. UI-SPEC explicitly accepts clearing on screen pop.

2. **Should `schema_pretty` ship in `DiscoverableTool` or be computed on-demand via a new FFI call?**
   - What we know: `schema_pretty` is 100s of bytes per tool; cardinality is tens of tools.
   - What's unclear: pre-compute everywhere vs. compute lazily.
   - Recommendation: **pre-compute and ship in `DiscoverableTool`**. Total payload increase is < 50 KB even at the high end. Eliminates a UniFFI roundtrip on detail-screen open.

3. **Does the existing `relative_time_label` need extending, or do we update UI-SPEC to drop "weeks"?**
   - What we know: helper at `lib.rs:937` emits `"Just now" / "Xm ago" / "Xh ago" / "Yesterday" / "Xd ago"`.
   - What's unclear: UI-SPEC §Copywriting includes example `"2w ago"` — author intent ambiguous.
   - Recommendation: **extend** the helper. Adding a weeks branch is 4 lines; matches user expectation; centralised (Phase 32 callers unaffected for sub-7-day deltas).

4. **`Used 0×` badge edge case — never used:**
   - What we know: UI-SPEC §States J says "Hidden when usage_count == 0". Caption in heading is "only if usage_count > 0".
   - What's unclear: detail screen USAGE section copy. UI-SPEC says "When `usage_count == 0`: single line `Never used`". Confirmed.
   - Recommendation: implement as specified. No ambiguity.

5. **Does adding the `nostr` crate as a direct dep require regenerating Cargo.lock?**
   - What we know: `nostr 0.43.1` is already a transitive in `Cargo.lock`.
   - What's unclear: whether Cargo will re-resolve and accidentally bump a related crate (`nostr-sdk`, `nostr-relay-pool`).
   - Recommendation: in Wave 0, run `cargo add nostr@0.43 --dry-run --offline` first. If unified, commit. If it tries to bump anything else, pin via `nostr = "=0.43.1"`.

## Sources

### Primary (HIGH confidence)
- `rust/src/lib.rs` — Read at lines 13, 113, 150–220, 280–350, 451–486, 740–763, 905–952, 1241–1249, 2440–2530, 2920–2965, 3470–3536, 3920–4280, 4600–4880, 6160–6235, 8195–8255, 8485–8500
- `rust/src/contextvm/discovery.rs` — full read (`DiscoveredServer`, `DiscoveredTool`, `discover_all`)
- `rust/src/contextvm/dispatch.rs` — full read (`ContextvmToolDescriptor`, `RESERVED_LOCAL_NAMES`, `MAX_REMOTE_TOOLS_PER_TURN`, `DESCRIPTION_CAP_CHARS`)
- `rust/src/contextvm/invocation.rs` — first 80 lines (Nostr key persistence + result truncation)
- `rust/src/contextvm/mod.rs` — full read (`DEFAULT_CONTEXTVM_RELAYS`, rustls crypto provider install)
- `rust/src/persistence/queries.rs` — lines 170–310 (AgentStepRow + insert/list); lines 1300–1475 (ContextvmToolRow CRUD)
- `rust/src/persistence/schema.rs` — lines 340–395 (V19, V20 migrations)
- `rust/Cargo.toml` — full read
- `Cargo.lock` — confirmed `bech32 0.11.1`, `nostr 0.43.1`, `contextvm-sdk 0.1.1` already present
- `android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDiscoveryScreen.kt` — full read
- `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` — lines 1–95, 170–211 (clipboard + nav routing)
- `desktop/iced/src/views/tool_discovery.rs` — full read
- `desktop/iced/src/main.rs` — lines 860–890 (clipboard), 1890–1940 (overlay routing)
- `.planning/phases/35-add-contextvm-sdk-for-nostr-based-tool-discovery/35-RESEARCH.md` — Phase 35 architecture context
- `.planning/phases/36-cache-discovered-contextvm-tools-tap-for-detail-npub-metadat/36-CONTEXT.md` — locked decisions
- `.planning/phases/36-cache-discovered-contextvm-tools-tap-for-detail-npub-metadat/36-UI-SPEC.md` — locked UI contract

### Secondary (MEDIUM confidence)
- [docs.rs/nostr — `key/public_key/struct.PublicKey.html`](https://docs.rs/nostr/latest/nostr/key/public_key/struct.PublicKey.html) — confirms `ToBech32` trait import path `nostr::nips::nip19::ToBech32` (WebFetch cross-checked with WebSearch; current docs reflect 0.43.x API)
- [Rust Nostr Book — NIP-19](https://rust-nostr.org/sdk/nips/19.html) — bech32 npub format reference

### Tertiary (LOW confidence)
- None — Phase 36 builds entirely on already-shipped Phase 35 infrastructure that is fully verified by code reading.

## Implementation Outline (for the planner)

A direct conversion of this research into 4 plans. Each is bounded and parallelisable where noted.

### Wave 0 — Test stubs + dep audit (one plan)
- Add `nostr = { version = "0.43", default-features = false, features = ["std"] }` to `rust/Cargo.toml`. Run `cargo tree -p mango_core | grep -iE "openssl-sys|native-tls"` and confirm baseline unchanged.
- Add 6 `#[ignore]` test stubs in `rust/src/tests/contextvm.rs` (or new `contextvm_phase36.rs`) covering: aggregate-by-name, multi-tool-payload, exclude-local, npub-known-vector, npub-invalid-fallback, DiscoverableTool-fields shape lock.
- Extend `directory_rag.rs::test_relative_time_labels` with weeks cases (asserts `2w ago` for 14 days, `1w ago` for 7 days). All ignored until Wave 1 lands the helper extension.
- Verify `nostr::nips::nip19::ToBech32::to_bech32()` works on `contextvm_sdk::signer::PublicKey` with one compile-only test.

### Wave 1 — Rust core: usage aggregation + DiscoverableTool extension + Screen variant (one plan)
- Extend `relative_time_label` with weeks branch + tests pass.
- Add `encode_npub(hex: &str) -> String` (in `rust/src/contextvm/mod.rs` or new `npub.rs`).
- Add `fetch_contextvm_tool_usage_rows` query in `persistence/queries.rs`.
- Add `aggregate_contextvm_tool_usage(conn) -> HashMap<String, (u32, i64)>` helper in `lib.rs`.
- Extend `DiscoverableTool` Record with: `usage_count: u32`, `last_used_at: Option<i64>`, `last_used_label: Option<String>`, `last_seen_at: i64`, `last_seen_label: String`, `npub: String`, `schema_pretty: String`.
- Refactor `row_to_discoverable_tool` (currently `lib.rs:3512`) to take `(row, usage_map: &HashMap<...>, now_secs: i64) -> DiscoverableTool` so it can be called from all current call sites.
- Update all call sites (5 sites per grep: `lib.rs:4190`, `lib.rs:6190`, `lib.rs:8499`, `lib.rs:2524`-area projection in DiscoverContextvmTools handler, plus `aggregate_contextvm_tool_usage` invocation).
- Add agent-loop hook: after `insert_agent_step` for a contextvm tool_call (around `lib.rs:2961`), re-run aggregate + project + emit_state so the badge updates live.
- Add new `Screen::ContextvmToolDetail { tool_id: String }` variant.
- All Wave 0 tests un-ignored and green.
- `just bindings-kotlin` + `just bindings-swift` regen, commit binding diffs.

### Wave 2a — Android Compose UI (one plan, parallel with 2b)
- Modify `SettingsToolDiscoveryScreen.kt`:
  - Add `query` state (Compose `remember`).
  - Add `OutlinedTextField` search field below TopAppBar (UI-SPEC §Layout 1).
  - Add live filter via `derivedStateOf`.
  - Modify the `when (state)` so cached list renders during `Loading` if non-empty (cache-first fix).
  - Add empty-search state (UI-SPEC §States M).
  - Add `Used N×` badge + chevron + whole-row clickable Modifier in `ToolRow` (UI-SPEC §Layout 2).
- Create `SettingsToolDetailScreen.kt` — heading + caption + description + ADVERTISED BY + USAGE + SCHEMA expander + Tool ID row, with Snackbar copy confirmation per UI-SPEC §Copywriting locked strings.
- Add `is Screen.ContextvmToolDetail` branch in `MainApp.kt` (after the existing `is Screen.ToolDiscovery` branch at line 189).
- Manual smoke: open screen, type "weather", tap a tool, expand schema, copy npub.

### Wave 2b — Desktop iced UI (one plan, parallel with 2a)
- Modify `desktop/iced/src/views/tool_discovery.rs`:
  - Accept `search_query: &str` parameter on `view()`.
  - Add `text_input(...)` search field at top of body.
  - Apply filter in `tool_list`.
  - Cache-first fix in `match state.contextvm_discovery_state` block.
  - Modify `tool_row` to add `Used N×` badge, chevron, whole-row `button` wrapper that dispatches `PushScreen { ContextvmToolDetail }`.
- Create `desktop/iced/src/views/tool_detail.rs` — mirror Compose layout in iced widgets; inline `text(...)` status line for copy confirmation cleared via `Task::perform(sleep(2s))`.
- Add `Screen::ContextvmToolDetail` overlay branch in `desktop/iced/src/main.rs` near line 1905.
- Add `Message::ContextvmSearchChanged(String)`, `Message::CopyNpub(String)`, `Message::CopyHex(String)`, `Message::CopyToolId(String)`, `Message::ToggleSchemaExpanded`, `Message::ClearCopyStatus`. Plumb in main message-handler.
- Add UI-state fields: `contextvm_search_query: String`, `contextvm_copy_status: Option<String>`, `contextvm_schema_expanded: bool` (or per-screen state).
- Manual smoke: same as Android.

**Total estimated plans: 4 (Wave-0 + Wave-1 + 2 parallel Wave-2 plans).** Estimated effort: small-to-medium phase. No architectural risk.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — `nostr` and `bech32` both verified in Cargo.lock; `ToBech32` trait verified via docs.rs WebFetch
- Architecture: HIGH — directly extends Phase 35 with all touchpoints code-read and verified
- Pitfalls: HIGH for #1, #2, #3, #4 (all code-verified); MEDIUM for #5 (Compose `derivedStateOf` behaviour is well-documented but recompose stability needs testing)

**Research date:** 2026-05-08
**Valid until:** 2026-06-08 (30 days; stable phase building on shipped Phase 35 infrastructure)
