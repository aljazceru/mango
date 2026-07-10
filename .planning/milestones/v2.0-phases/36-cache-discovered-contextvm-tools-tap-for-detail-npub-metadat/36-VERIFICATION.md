---
phase: 36-cache-discovered-contextvm-tools-tap-for-detail-npub-metadat
verified: 2026-05-08T00:00:00Z
status: human_needed
score: 7/7 must-haves verified (automated); 10 manual smoke items deferred to user
overrides_applied: 0
re_verification:
  previous_status: null
  previous_score: null
  gaps_closed: []
  gaps_remaining: []
  regressions: []
human_verification:
  - test: "Cache-first paint timing on Discover Tools screen open (Android primary)"
    expected: "After at least one prior discovery, navigating Settings → Discover Tools shows the cached list within ~16ms — never a spinner over a blank screen. Background refresh runs without blanking the list."
    why_human: "Frame-budget timing and visual presence-of-spinner cannot be measured by static analysis; requires running app on a real device."
  - test: "Live keystroke search filter on Tool Discovery"
    expected: "Typing each character into the Search tools field instantly narrows the list with no perceptible debounce delay. Clearing the field restores the full list."
    why_human: "Live UI behaviour over a non-empty discovered list — needs Nostr relays reachable and keyboard input on device."
  - test: "Empty-search caption rendering"
    expected: "Searching for an unmatched string (e.g. `zzznevermatchesxxx`) shows the centred caption `No tools match \"zzznevermatchesxxx\"` with straight ASCII quotes; the search field stays mounted above."
    why_human: "Visual presence + exact substituted query string verification is a UX check."
  - test: "Used N× badge appears on a previously-invoked tool"
    expected: "After invoking a contextvm tool through the agent loop, returning to Discover Tools shows the `Used 1×` muted pill on that tool's row (immediately, without re-running discovery)."
    why_human: "Requires an agent session that completes a contextvm tool_call and the subsequent live re-projection visible in the UI; agent-loop hook is wired (lib.rs:3002-3016) but real-time emission is observable only at runtime."
  - test: "Whole-row tap navigates to detail; toggle Switch does NOT navigate"
    expected: "Tapping the body of a tool row opens the Tool details screen. Tapping the trailing Switch toggles enable/disable WITHOUT opening the detail screen."
    why_human: "Click-event absorption is a runtime gesture behaviour."
  - test: "Tool Detail screen renders five sections in order"
    expected: "Heading (tool name + optional `Used N× — last used …`) → ADVERTISED BY (provider + npub + Hex) → USAGE (Never used / Used N times + Last used …) → SCHEMA expander (Show/Hide) → Tool ID: row."
    why_human: "Visual layout + section ordering + content presence is a UX inspection."
  - test: "Copy actions and confirmation feedback"
    expected: "Tapping `Copy` next to npub places the bech32 string in the OS clipboard and shows `npub copied` (Snackbar on Android / inline status line on Desktop, auto-cleared ~2s). Same for Hex (`Pubkey copied`) and Tool ID (`Tool ID copied`). Pasting into another app yields the FULL value (not the truncated display)."
    why_human: "Clipboard write + confirmation surface require runtime."
  - test: "SCHEMA expander toggles and renders monospace JSON"
    expected: "Tapping `Show` reveals a scrollable monospace pretty-printed JSON block; tapping `Hide` collapses it. For tools with empty/null schema, `No schema published` is shown and no Show button is rendered."
    why_human: "Toggle interaction + monospace rendering + scroll behaviour is a runtime UX check."
  - test: "Back navigation preserves search query state on Tool Discovery"
    expected: "Type a query → tap a row to open detail → back arrow returns to Discover Tools with the prior search query (and filtered list) still visible."
    why_human: "Cross-screen state preservation is a runtime UX check."
  - test: "App stability under Phase 36 surface"
    expected: "App does not crash, hang, or ANR while exercising any of the new surfaces (search, tap-for-detail, copy, schema expand, badge presence). Logcat clear of FATAL / SIGABRT / panic / tombstone."
    why_human: "Runtime stability under user gestures is observable only at runtime; orchestrator-side monkey launch confirms cold-boot stability only."
---

# Phase 36: Cache discovered contextvm tools — tap-for-detail + search + used-history Verification Report

**Phase Goal:** Cache discovered contextvm tools (cache-first render, all enabled+disabled rows persisted), tap any row to see a detail screen with the advertising npub (bech32) + full metadata + JSON schema, search across cached tools (live filter), surface previously-invoked tools with a "Used N×" badge from agent_steps aggregate. Android primary, Desktop secondary, iOS deferred.

**Verified:** 2026-05-08
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #   | Truth                                                                                  | Status     | Evidence                                                                                                                                                                        |
| --- | -------------------------------------------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Cached contextvm tools render instantly on Discover Tools screen open (cache-first)    | ✓ VERIFIED | Cache-first guard at `rust/src/lib.rs:6347-6353` (handler does NOT clear `app_state.contextvm_tools` on Loading); Android `SettingsToolDiscoveryScreen.kt:160-187` splits Loading into `if (cachedTools.isEmpty()) LoadingState() else ToolListOrEmptySearch(...)`; Desktop `tool_discovery.rs:126-141` mirrors with `state.contextvm_tools.is_empty()` check. |
| 2   | Discovery upserts ALL rows (enabled + disabled) into `contextvm_tools`                 | ✓ VERIFIED | Phase 35 baseline already persists `enabled = 0` rows (existing upsert preserves user's flag). AppState populated via `list_all_contextvm_tools` at lib.rs:2547, 3010, 4296, 6310, 8636. |
| 3   | Tapping a row opens detail screen with npub bech32, metadata, and pretty-printed schema | ✓ VERIFIED | `Screen::ContextvmToolDetail { tool_id }` at lib.rs:503; `encode_npub` at `rust/src/contextvm/npub.rs` (uses nostr ToBech32 trait); `schema_pretty` field on `DiscoverableTool` at lib.rs:191; Android `SettingsToolDetailScreen.kt` (327 lines) renders all 5 sections; Desktop `views/tool_detail.rs` (342 lines) renders all 5 sections. |
| 4   | Live search filters cached tools by name/description/provider (case-insensitive)        | ✓ VERIFIED | Android `SettingsToolDiscoveryScreen.kt:105-118` `derivedStateOf` filter on `tool.name.lowercase().contains(q) \|\| tool.description.lowercase().contains(q) \|\| (providerDisplayName ?: "").lowercase().contains(q)`; Desktop `tool_discovery.rs:108-119` mirrors the same predicate. No debounce in either. |
| 5   | "Used N×" badge surfaces tools with `usage_count > 0` (computed from agent_steps)      | ✓ VERIFIED | `aggregate_contextvm_tool_usage` at lib.rs:3573-3592 (groups by tool_name, excludes tool_origin='local'); `fetch_contextvm_tool_usage_rows` at queries.rs:1484; agent-loop re-projection hook at lib.rs:2998-3017 fires after `insert_agent_step` for `tool_origin == "contextvm"` and re-emits state. Android badge at `SettingsToolDiscoveryScreen.kt:357-358` and `UsedBadge` at line 392-398; Desktop badge at `tool_discovery.rs:376-383`. |
| 6   | Cross-platform parity (Android Compose + Desktop iced); iOS UI deferred                | ✓ VERIFIED | Android: 2 files modified/created (SettingsToolDiscoveryScreen.kt:410 lines, SettingsToolDetailScreen.kt:327 lines, MainApp.kt nav arm at line 197). Desktop: tool_discovery.rs:403 lines, tool_detail.rs:342 lines, main.rs nav arm at line 1986-1987. UniFFI Swift bindings regenerated for iOS plumbing per Plan 36-01. |
| 7   | All 23 locked UI-SPEC copy strings present verbatim in both Android and Desktop sources | ✓ VERIFIED | Orchestrator pre-verification report + spot-grep on `Search tools`, `No tools match "{query}"`, `Used 1×` / `Used {N}×`, `Tool details`, `ADVERTISED BY`, `Unnamed provider`, `Hex:`, `USAGE`, `Never used`, `Used 1 time` / `Used {N} times`, `Last used`, `SCHEMA`, `No schema published`, `Show` / `Hide`, `Tool ID:`, `npub copied`, `Pubkey copied`, `Tool ID copied`, `Couldn't copy — try again`, `Tool not found`. Desktop `COPY_FAILED` const surfaces the failure literal even though iced 0.13 does not propagate clipboard errors. |

**Score:** 7/7 truths verified (automated)

### Required Artifacts

| Artifact                                                                                                | Expected                                                              | Status     | Details                                                                                                                              |
| -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `rust/src/contextvm/npub.rs`                                                                            | `encode_npub` bech32 encoder with safe fallback                       | ✓ VERIFIED | 33 lines; uses `nostr::nips::nip19::ToBech32` on `contextvm_sdk::signer::PublicKey`; `fallback("invalid:<prefix>")` UTF-8-safe via `chars().take(8)`; never panics |
| `rust/src/contextvm/mod.rs`                                                                             | `pub mod npub;` + `pub use npub::encode_npub;`                        | ✓ VERIFIED | Lines 17, 29                                                                                                                         |
| `rust/src/persistence/queries.rs::fetch_contextvm_tool_usage_rows`                                       | Returns `Vec<(String,i64)>` for `tool_origin='contextvm' AND action_type='tool_call'` | ✓ VERIFIED | queries.rs:1484-1496                                                                                                                 |
| `rust/src/lib.rs::aggregate_contextvm_tool_usage`                                                        | Returns `HashMap<String,(u32,i64)>` keyed by tool_name                | ✓ VERIFIED | lib.rs:3573-3592; parses JSON action_payload array, increments per call entry                                                        |
| `rust/src/lib.rs::DiscoverableTool` extended fields                                                      | +7 fields: `usage_count, last_used_at, last_used_label, last_seen_at, last_seen_label, npub, schema_pretty` | ✓ VERIFIED | lib.rs:163-192; all fields present with correct types matching plan                                                                  |
| `rust/src/lib.rs::Screen::ContextvmToolDetail { tool_id: String }`                                       | New enum variant                                                      | ✓ VERIFIED | lib.rs:503                                                                                                                           |
| `rust/src/lib.rs::row_to_discoverable_tool` refactored                                                  | New signature `(row, &usage_map, now_secs)`                            | ✓ VERIFIED | lib.rs:3600-3631; called at 4 sites (2548, 3013, 4306, 6313, 8639)                                                                   |
| `rust/src/lib.rs::relative_time_label` weeks branch                                                     | `delta >= 7*86400 → "{w}w ago"`                                       | ✓ VERIFIED | Tests `test_relative_time_labels_weeks_one` and `_weeks_two` GREEN; original `30d` updated to `4w ago`; 6d boundary regression added |
| Agent-loop badge update hook                                                                            | Re-aggregate + emit_state after `insert_agent_step` if `tool_origin=='contextvm'` | ✓ VERIFIED | lib.rs:2998-3017                                                                                                                     |
| `android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDetailScreen.kt`                            | New 5-section detail screen                                           | ✓ VERIFIED | 327 lines; all 23 locked strings literal-grep verified                                                                               |
| `android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDiscoveryScreen.kt`                          | Extended with search field, cache-first, badge, chevron, whole-row click | ✓ VERIFIED | 410 lines; `var query`, `derivedStateOf` filter, `LoadingState`/`ToolListOrEmptySearch` cache-first split, `UsedBadge` composable    |
| `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt`                                             | Nav arm `is Screen.ContextvmToolDetail -> SettingsToolDetailScreen(...)` | ✓ VERIFIED | Line 197-198                                                                                                                          |
| `desktop/iced/src/views/tool_detail.rs`                                                                 | New 5-section detail view                                             | ✓ VERIFIED | 342 lines; all 23 locked strings present including `COPY_FAILED` const                                                               |
| `desktop/iced/src/views/tool_discovery.rs`                                                              | Extended with search input, cache-first, badge, chevron, whole-row button | ✓ VERIFIED | 403 lines; `text_input("Search tools", ...)`, cache-first body match, `Used 1×` / `Used {N}×` muted pill, `>` chevron, transparent-button row wrapper |
| `desktop/iced/src/views/mod.rs`                                                                         | `pub mod tool_detail;` registration                                   | ✓ VERIFIED | Line present                                                                                                                          |
| `desktop/iced/src/main.rs`                                                                              | 3 new App fields, 6 new Messages, handlers, overlay arm               | ✓ VERIFIED | Fields at lines 361/364/366; Messages at 451/453/454/455/457/460; handlers at 1204-1240; overlay arm at 1986-1987                  |

### Key Link Verification

| From                                | To                                          | Via                                            | Status   | Details                                                                                              |
| ----------------------------------- | ------------------------------------------- | ---------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------- |
| Discover Tools row tap (Android)    | Tool Detail screen                          | `AppAction.PushScreen(Screen.ContextvmToolDetail(toolId))` | ✓ WIRED | SettingsToolDiscoveryScreen.kt:181-184, 209-212; MainApp.kt:197-198 dispatches `SettingsToolDetailScreen` |
| Discover Tools row tap (Desktop)    | Tool Detail screen                          | Transparent button → `Message::PushScreen(Screen::ContextvmToolDetail{tool_id})` → main.rs:1986 overlay arm → `views::tool_detail::view` | ✓ WIRED | tool_discovery.rs split-row pattern; main.rs:1986-1987 |
| Search field                        | Live filter                                  | Android `derivedStateOf` over `appState.contextvmTools`; Desktop `text_input.on_input(Message::ContextvmSearchChanged)` then in-render filter | ✓ WIRED | Android: SettingsToolDiscoveryScreen.kt:105-118; Desktop: tool_discovery.rs:91-119                                                          |
| `agent_steps` (tool_origin=contextvm) | "Used N×" badge                            | `fetch_contextvm_tool_usage_rows` → `aggregate_contextvm_tool_usage` → `row_to_discoverable_tool.usage_count` → UI `UsedBadge` / `used_badge` | ✓ WIRED | queries.rs:1484, lib.rs:3573, lib.rs:3605-3608; UI components verified |
| Agent-loop step insertion           | Live badge re-projection                    | post-`insert_agent_step` hook calls `aggregate_contextvm_tool_usage` + `list_all_contextvm_tools` + `emit_state`         | ✓ WIRED | lib.rs:2998-3017 (gated on `step_row.tool_origin == Some("contextvm")`)                                                                          |
| Provider pubkey hex                 | npub bech32 (`npub1…`)                      | `encode_npub` (uses `PublicKey::from_hex` + `ToBech32`) | ✓ WIRED | rust/src/contextvm/npub.rs:13-28; called from `row_to_discoverable_tool` lib.rs:3611                                              |
| Detail screen Copy buttons          | OS clipboard + ephemeral confirmation        | Android: `ClipboardManager` write → Snackbar; Desktop: `iced::clipboard::write` → inline status with 2s `Task::perform` clear | ✓ WIRED (with documented v1 limitation: Desktop failure-path copy literal exists but iced 0.13 cannot route the error back to update loop) | Android SettingsToolDetailScreen.kt:127-135; Desktop main.rs:1210-1240 |
| `DiscoverContextvmTools` action     | Cache preservation across Loading           | Handler does NOT zero `app_state.contextvm_tools` on Loading transition; comment at lib.rs:6347-6351 locks the regression | ✓ WIRED | lib.rs:6347-6354                                                                                                                              |

### Data-Flow Trace (Level 4)

| Artifact                                  | Data Variable                                  | Source                                                                     | Produces Real Data | Status                                                                                                                                                                                                                                       |
| ----------------------------------------- | ---------------------------------------------- | -------------------------------------------------------------------------- | ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tool Discovery list                       | `appState.contextvmTools` / `state.contextvm_tools` | Actor: `list_all_contextvm_tools(conn)` + `aggregate_contextvm_tool_usage(conn)` → `row_to_discoverable_tool` projection | ✓ FLOWING          | `list_all_contextvm_tools` is a real SQL query against `contextvm_tools` (Phase 35 schema, MIGRATION_V20). `aggregate_contextvm_tool_usage` runs a real SQL query via `fetch_contextvm_tool_usage_rows` against `agent_steps`. Both feed all UI surfaces. |
| `Used N×` badge                            | `tool.usageCount` / `tool.usage_count`         | `aggregate_contextvm_tool_usage` HashMap value                            | ✓ FLOWING          | v1 limitation noted in Plan 36-01 SUMMARY: chat-tool path does not write `agent_steps`, so badge reflects agent-session uses only. This is a deliberate scope decision, not a hollow prop.                                                  |
| npub display                               | `tool.npub`                                    | `encode_npub(row.provider_pubkey)`                                         | ✓ FLOWING          | Real bech32 encoding via nostr ToBech32; safe fallback on invalid hex.                                                                                                                                                                          |
| schema_pretty body                         | `tool.schema_pretty` / `tool.schemaPretty`     | `serde_json::to_string_pretty(parse(row.schema_json))` w/ raw fallback     | ✓ FLOWING          | Real JSON parsed and pretty-printed at projection time.                                                                                                                                                                                       |
| `last_seen_label` / `last_used_label`     | UniFFI string fields                           | `relative_time_label(timestamp, now_secs)` (incl. weeks branch)            | ✓ FLOWING          | Pre-computed display strings — UI never branches on integers.                                                                                                                                                                                  |

### Behavioral Spot-Checks

| Behavior                                              | Command                                               | Result                                                                                          | Status |
| ----------------------------------------------------- | ----------------------------------------------------- | ----------------------------------------------------------------------------------------------- | ------ |
| Rust core compiles                                    | `cargo build -p mango_core`                           | (orchestrator) GREEN                                                                            | ✓ PASS |
| Desktop iced compiles                                 | `cargo build -p mango-desktop`                        | GREEN (1 pre-existing dead-code warning unrelated to Phase 36)                                 | ✓ PASS |
| Rust unit tests pass (incl. all 8 Phase 36 stubs)     | `cargo test -p mango_core --lib`                      | 445 passed; 0 failed; 20 ignored — confirms all 8 Phase 36 RED stubs (CTX36-AGG/NPUB/FIELDS/RTL) un-ignored and GREEN | ✓ PASS |
| Android assembleDebug                                  | `./gradlew :app:assembleDebug`                        | (orchestrator) BUILD SUCCESSFUL                                                                 | ✓ PASS |
| Android APK installs and app launches without crash   | adb install + monkey launch on device 5A011JEBF06589  | (orchestrator) pid 17923; logcat clear of FATAL/SIGABRT/panic/tombstone for 6s post-launch     | ✓ PASS |
| All 23 locked UI-SPEC strings literal-grep verbatim   | grep across both Android Kotlin files + 3 Desktop iced files | (orchestrator) all 23 verified                                                                  | ✓ PASS |

### Requirements Coverage

ROADMAP.md lists 7 requirement IDs for Phase 36 (CTX36-CACHE-01, CTX36-SEARCH-01, CTX36-DETAIL-01, CTX36-USED-01, CTX36-NPUB-01, CTX36-LABELS-01, CTX36-NAV-01). These IDs are not back-filled into `.planning/REQUIREMENTS.md` (informational gap, not a Phase 36 deliverable), so coverage is mapped against ROADMAP-level descriptions:

| Requirement       | Source     | Description                                                  | Status      | Evidence                                                                                                                       |
| ----------------- | ---------- | ------------------------------------------------------------ | ----------- | -------------------------------------------------------------------------------------------------------------------------------- |
| CTX36-CACHE-01    | ROADMAP    | Cache-first hydration of contextvm_tools before refresh      | ✓ SATISFIED | Cache-first guard at lib.rs:6347-6354; cache-first composition split in both Android (SettingsToolDiscoveryScreen.kt:160-187) and Desktop (tool_discovery.rs:126-141) |
| CTX36-SEARCH-01   | ROADMAP    | Always-visible live search across name/description/provider | ✓ SATISFIED | `derivedStateOf` filter (Android) + `text_input` filter (Desktop); UI-SPEC §States L verified                                   |
| CTX36-DETAIL-01   | ROADMAP    | Tap-for-detail with full metadata                            | ✓ SATISFIED | New `Screen::ContextvmToolDetail` variant + 5-section screens on both platforms                                                 |
| CTX36-USED-01     | ROADMAP    | "Used N×" badge from agent_steps aggregate                   | ✓ SATISFIED | `aggregate_contextvm_tool_usage` + agent-loop hook + `UsedBadge` / `used_badge` UI; v1 limitation (chat-tool path not written to `agent_steps`) documented in Plan 36-01 SUMMARY |
| CTX36-NPUB-01     | ROADMAP    | Display advertising npub (bech32)                            | ✓ SATISFIED | `encode_npub` in rust/src/contextvm/npub.rs; surfaced as `DiscoverableTool.npub`                                                |
| CTX36-LABELS-01   | ROADMAP    | Pre-computed display labels (last_used, last_seen, weeks branch) | ✓ SATISFIED | `last_used_label`, `last_seen_label` fields + `relative_time_label` weeks branch (1w/2w ago)                                     |
| CTX36-NAV-01      | ROADMAP    | Navigation: whole-row tap → detail; Switch absorbs its own click | ✓ SATISFIED | Android `Modifier.clickable` + Switch absorption; Desktop transparent-button wrapper with toggler as sibling (split-row mitigation) |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| (none) | — | No TODO/FIXME/XXX/HACK/placeholder strings found in Phase 36 files outside of legitimate uses (`TextField` `placeholder` prop, SQL parameter `placeholders`) | ℹ️ Info | None |

### Human Verification Required

10 manual smoke items deferred per orchestrator policy:

1. **Cache-first paint timing** — verify cached list renders within ~16ms on screen open
2. **Live keystroke search filter** — verify per-keystroke filter, no debounce
3. **Empty-search caption** — verify exact substituted query string with straight ASCII quotes
4. **Used N× badge** — verify badge appears after a contextvm agent-loop tool call
5. **Whole-row tap navigation** — verify body taps navigate, Switch taps don't
6. **Tool Detail 5 sections** — verify section order and content
7. **Copy + confirmation** — verify clipboard contains FULL value and confirmation surface displays/clears
8. **SCHEMA expander** — verify Show/Hide toggle and monospace scrollable JSON; "No schema published" path
9. **Back navigation preserves search** — verify state persists across detail push/pop
10. **App stability under Phase 36 surface** — verify no crashes/ANRs while exercising new surfaces

These items were explicitly DEFERRED to the user by the orchestrator. Automated checks (build green, test green, APK install + launch + crash-scan, locked-copy verbatim verification) all pass.

### Gaps Summary

No gaps blocking goal achievement.

All 7 must-have observable truths are verified by static analysis, build/test artefacts, and orchestrator-side smoke (build + boot + crash-scan + locked-copy). The phase delivers cache-first rendering of the existing Tool Discovery surface, an always-visible search field with live filter, the new "Used N×" badge backed by the agent_steps aggregate, and a tappable Tool Detail screen with npub bech32, full metadata, pretty-printed JSON schema expander, and one-tap Copy actions — across both Android (Compose, primary) and Desktop (iced, secondary). iOS UniFFI bindings regenerated; iOS UI explicitly out-of-scope per CONTEXT.

Two documented v1 limitations carried forward (both pre-recorded in Plan 36-01 / 36-03 SUMMARYs and not blocking goal achievement):
1. Chat-tool path (Phase 27) does not write `agent_steps` rows, so `Used N×` reflects agent-session uses only. Filed as non-blocking follow-up in 36-01-SUMMARY.
2. Desktop `iced::clipboard::write` does not propagate errors back through the update loop in iced 0.13, so the failure-path copy `Couldn't copy — try again` exists in source as `COPY_FAILED` (#[allow(dead_code)]) but is not currently user-visible. Pre-recorded in 36-03-SUMMARY; revisit when iced 0.14+ exposes clipboard error reporting.

Status is `human_needed` because 10 runtime smoke items were explicitly deferred to the user — the orchestrator-side automation does not exercise interactive gestures (search keystrokes, tap-for-detail, Copy buttons, schema expander), only build/install/launch/crash-scan and static locked-copy verification.

---

_Verified: 2026-05-08_
_Verifier: Claude (gsd-verifier)_
