---
phase: 36
plan: 02
subsystem: android/compose-ui
tags: [contextvm, phase36, wave2, android, compose, ui]
requires:
  - "Plan 36-01 DiscoverableTool +7 fields (usage_count, last_used_at, last_used_label, last_seen_at, last_seen_label, npub, schema_pretty) wired through UniFFI"
  - "Plan 36-01 Screen::ContextvmToolDetail { tool_id } enum variant with Kotlin codegen as Screen.ContextvmToolDetail"
  - "Plan 36-01 agent-loop hook re-aggregating usage_count + last_used_at on tool_origin='contextvm' agent_steps insert"
  - "Phase 35-07 SettingsToolDiscoveryScreen.kt baseline (5-state coverage + Discover/Auto-discover Settings rows)"
  - "Phase 35-06 Remote provenance pill style for visual parity of Used N× badge"
provides:
  - "Cache-first discovery render: cached contextvm tools paint immediately on screen open; refresh runs in-flight without blocking the list"
  - "Always-visible search field with placeholder `Search tools` filtering keystroke-by-keystroke across name/description/providerDisplayName (case-insensitive substring)"
  - "Used N× muted pill on rows where usageCount > 0 (singular `Used 1×`, plural `Used {N}×`, U+00D7 multiplication sign)"
  - "Whole-row click → AppAction.PushScreen(Screen.ContextvmToolDetail(toolId)); Switch retains its own absorption (toggle does NOT navigate)"
  - "SettingsToolDetailScreen with five sections: Heading (title + optional Used N× — last used X subtitle + description), ADVERTISED BY (provider + npub Copy + Hex Copy), USAGE (Never used / Used N times + Last used X), SCHEMA expander (Show/Hide → monospace scrollable JSON or `No schema published`), Tool ID row with Copy"
  - "Snackbar copy confirmations: `npub copied`, `Pubkey copied`, `Tool ID copied`, fallback `Couldn't copy — try again`"
  - "MainApp.kt nav arm `is Screen.ContextvmToolDetail -> SettingsToolDetailScreen(...)` placed immediately after the existing Screen.ToolDiscovery arm"
  - "Empty-search caption `No tools match \"{query}\"` with straight ASCII quotes, rendered when filter applied and result list empty"
  - "Trailing KeyboardArrowRight chevron (Material AutoMirrored, 18dp, onSurfaceVariant tint) on every row, sitting between the Used N× pill and the Switch"
affects:
  - "android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDiscoveryScreen.kt — Phase 35 ToolList replaced by ToolListOrEmptySearch + ToolRow refactor with badge/chevron/whole-row clickable"
  - "android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDetailScreen.kt — new screen (327 lines)"
  - "android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt — nav switch gains is Screen.ContextvmToolDetail arm at line 197"
tech-stack:
  added: []
  patterns:
    - "Cache-first state composition: split Loading branch into `cachedTools.isEmpty() ? LoadingState() : ToolListOrEmptySearch(...)` so an in-flight refresh never blanks the cached list"
    - "Hoisted `var query by remember { mutableStateOf(\"\") }` + `val filteredTools by remember(appState.contextvmTools) { derivedStateOf { ... } }` keystroke filter — O(N) substring scan, no debounce, bounded cardinality (tens of tools) per threat-model accept disposition"
    - "Locked-copy contract enforcement: 23 verbatim Phase 36 strings literal-grep verifiable in source; UI never branches on integers — singular vs. plural and relative-time labels all consume Rust-pre-computed `last_used_label` / `last_seen_label` / `npub` / `schema_pretty` fields from DiscoverableTool"
    - "Switch click absorption pattern: Card.Modifier.clickable + Switch onCheckedChange gives row-tap → detail nav while toggle stays local (Compose absorbs the inner pointer event)"
    - "Snackbar via shared `SnackbarHostState` + `rememberCoroutineScope().launch { snackbarHostState.showSnackbar(...) }` for copy confirmations; clipboard write failure surfaces fallback string"
key-files:
  created:
    - "android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDetailScreen.kt (327 lines)"
    - ".planning/phases/36-cache-discovered-contextvm-tools-tap-for-detail-npub-metadat/36-02-SUMMARY.md (this file)"
  modified:
    - "android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDiscoveryScreen.kt — +275/-56 (search field, derivedStateOf filter, ToolListOrEmptySearch, ToolRow refactor, UsedBadge composable, chevron, whole-row clickable, cache-first Loading branch)"
    - "android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt — +9 (is Screen.ContextvmToolDetail nav arm at line 197)"
decisions:
  - "Reuse Phase 35-06 `Remote` provenance pill style (Surface + RoundedCornerShape(8.dp) + surfaceVariant alpha 0.5 + labelSmall + onSurfaceVariant) for the Used N× badge so the two muted pills share the same visual language"
  - "Filter executes on every keystroke without debounce — discovered cardinality is bounded to tens of tools per UI-SPEC §States; threat-model T-36-02-D1 accepts the O(N) cost"
  - "Hex pubkey display truncated at 8 chars + ellipsis on the detail screen; full hex still goes to the clipboard via `fullValueToCopy` parameter on CopyRow — keeps the row visually balanced with the npub line above"
  - "SCHEMA expander defaults to collapsed (`var schemaExpanded by remember { mutableStateOf(false) }`) so the detail screen height stays predictable when JSON is large; expanded body uses `heightIn(max = 320.dp)` + verticalScroll so a multi-kilobyte schema cannot push the scroll position off-screen"
  - "When schema_pretty is blank, render `No schema published` and OMIT the Show button entirely (button only visible when schema is available) — keeps the user from tapping a no-op control"
metrics:
  duration: "~10min (two task commits 18:16:49 → 18:18:53; orchestrator-side verification + checkpoint approval extend wall-clock to plan-end)"
  completed: "2026-05-08"
---

# Phase 36 Plan 02: Android Compose UI for cached contextvm tools, search, Used N× badge, and tap-for-detail Summary

Wave 2a Android UI surface for Phase 36: SettingsToolDiscoveryScreen.kt now renders cached contextvm tools cache-first under an always-visible search filter with Used N× badges and chevrons, and a new SettingsToolDetailScreen.kt exposes provider npub/hex, usage history, pretty-printed schema (collapsible), and Tool ID — all wired through MainApp's Screen.ContextvmToolDetail nav arm with locked UI-SPEC §Copywriting strings and Snackbar copy confirmations.

## Files Modified

**Created:**
- `android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDetailScreen.kt` (327 lines)

**Modified:**
- `android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDiscoveryScreen.kt` (+275 / -56)
- `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` (+9, nav arm at line 197)

Per-task commits: `70a1a8d` (Task 1: SettingsToolDiscoveryScreen extension) and `879ffc7` (Task 2: SettingsToolDetailScreen + MainApp nav arm).

## Tasks Executed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Extend SettingsToolDiscoveryScreen.kt — search field, cache-first render, Used N× badge, chevron, whole-row click | `70a1a8d` | SettingsToolDiscoveryScreen.kt |
| 2 | Create SettingsToolDetailScreen.kt + wire MainApp.kt nav arm | `879ffc7` | SettingsToolDetailScreen.kt, MainApp.kt |
| 3 | Manual smoke checkpoint (cache-first, search, badge, copy, schema) | (orchestrator-verified) | n/a |

## Locked Copy Verification

All 23 Phase 36 locked copy strings appear verbatim in the two Kotlin files (literal-grep verified):

| String | Source | Count |
|--------|--------|-------|
| `Search tools` (placeholder) | DiscoveryScreen | 1 |
| `No tools match "{query}"` (empty-search caption) | DiscoveryScreen | 1 |
| `Used 1×` (singular badge & subtitle) | both | 4 |
| `Used ${tool.usageCount}×` (plural badge) | DiscoveryScreen | 1 |
| `Tool details` (TopAppBar title) | DetailScreen | 1 |
| `ADVERTISED BY` (section label) | DetailScreen | 1 |
| `USAGE` (section label) | DetailScreen | 1 |
| `SCHEMA` (section label) | DetailScreen | 1 |
| `Show` / `Hide` (schema expander toggle) | DetailScreen | 1 each |
| `No schema published` (empty-schema body) | DetailScreen | 1 |
| `Tool ID:` (row prefix) | DetailScreen | 1 |
| `Tool ID copied` (snackbar) | DetailScreen | 1 |
| `npub copied` (snackbar) | DetailScreen | 1 |
| `Pubkey copied` (snackbar) | DetailScreen | 1 |
| `Couldn't copy — try again` (snackbar fallback) | DetailScreen | 1 |
| `Never used` (USAGE empty body) | DetailScreen | 1 |
| `Used 1 time` (USAGE singular) | DetailScreen | 1 |
| `Used ${tool.usageCount} times` (USAGE plural) | DetailScreen | 1 |
| `Last used` (USAGE prefix) | DetailScreen | 1 |
| `Unnamed provider` (ADVERTISED BY fallback) | DetailScreen | 1 |
| `Tool not found` (edge-case body) | DetailScreen | 1 |
| `Hex:` (CopyRow prefix for full pubkey) | DetailScreen | 1 |

(Counts may exceed 1 where the same string is referenced in both singular/plural branches or used in the heading subtitle as well as the USAGE section.)

## Smoke Results (Orchestrator-Verified Subset)

The Task 3 checkpoint was approved by the orchestrator after the following independent verification:

| Item | Result |
|------|--------|
| `cd android && ./gradlew :app:assembleDebug` | BUILD SUCCESSFUL |
| `adb -s 5A011JEBF06589 install -r app-debug.apk` | Success |
| App launch via `monkey -p dev.disobey.mango.dev -c LAUNCHER 1` on device 5A011JEBF06589 | pid 17923 — no FATAL / SIGABRT / panic / crash / tombstone in `logcat -d *:E` after 6s |
| `cargo build` of rust core (Wave 1 baseline) | green |
| 23 locked Phase 36 copy strings | grep-verified (table above) |
| Manual touch-and-tap UI verification (cache-first paint timing, search keystroke filter, Used N× pill on previously-invoked tool, copy snackbar visibility, schema expander toggle, navigation back) | DEFERRED to user — orchestrator-side build + boot + crash-scan + copy-string verification accepted as sufficient to land Plan 36-02 |

## Cross-Platform Note (for Plan 36-03 Desktop)

Compose-specific patterns Plan 36-03 (Desktop iced) should mirror or consciously diverge from:

1. **Cache-first composition.** Compose split the `Loading` arm into `if (cachedTools.isEmpty()) LoadingState() else ToolListOrEmptySearch(...)`. iced should mirror this exactly: never show the spinner overlay when the cached list is non-empty.
2. **Empty-search caption rendering.** Compose centers `No tools match "{query}"` with a 32dp padding box. iced should choose an equivalent centered-caption layout at the same point in the state-driven body — the caption is part of the search-field state, NOT the discovery-state machine.
3. **Used N× pill style.** Compose reused the Phase 35-06 `Remote` provenance pill style (Surface + RoundedCornerShape(8.dp) + surfaceVariant alpha 0.5 + labelSmall + onSurfaceVariant). iced should adopt the same muted background tint convention used by its Phase 35-07 Remote badge — visual parity matters.
4. **Switch click absorption.** Compose relies on Material's built-in pointer absorption to keep the row's `Modifier.clickable` from firing when the user taps the Switch. iced lacks this; Plan 36-03 should use a button-style row with the toggle dispatching a sibling Message rather than a nested clickable region.
5. **Snackbar vs. inline status.** Compose uses a SnackbarHostState (transient). Plan 36-03 explicitly calls for "inline status copy confirmations" in the iced screen — divergence is intentional and locked in the UI-SPEC.
6. **Hex truncation policy.** Compose truncates the pubkey hex at 8 chars + ellipsis on the detail screen but copies the full hex to clipboard via the `fullValueToCopy` parameter. iced should preserve the same truncation-on-display / copy-full pattern.
7. **Schema expander default-collapsed + max-height-clamp.** Compose collapses the schema expander by default and clamps the expanded body to `heightIn(max = 320.dp)` with internal vertical scroll. iced should pick an equivalent fixed-height-with-scroll container (e.g., `scrollable!` with a fixed `height(Length::Fixed(320.0))`) so multi-kilobyte schemas cannot blow out the scroll position.
8. **Schema button visibility.** When `schema_pretty` is blank, Compose hides the Show button entirely (only renders `No schema published`). Plan 36-03 should follow the same rule — do not render a no-op Show/Hide control.

## Deviations from Plan

### Auto-fixed Issues

None. The plan was executed as written; the two task commits map 1:1 to the two `type="auto"` tasks.

### Auth Gates

None.

### Checkpoint Resolution

**Task 3 (`checkpoint:human-verify`)** was approved by the orchestrator after independent build + install + launch + crash-scan verification on physical device 5A011JEBF06589. The plan body and acceptance contract list 10 manual touch-and-tap verification items (search keystroke filter, badge presence, copy snackbar, schema expander, back nav, etc.); the user explicitly deferred those to themselves while landing the plan. This is documented in the Smoke Results table above so any subsequent regression in those areas is traceable to a verification gap and not to "the plan was reported done without testing."

## Threat Flags

None. The plan's `<threat_model>` STRIDE register (T-36-02-T1 plain Text composable, T-36-02-I1 public-identifier clipboard, T-36-02-D1 bounded-cardinality keystroke filter, T-36-02-V1 pure-Kotlin substring search) was honored without introducing additional surface. No new network endpoints, auth paths, file access patterns, or schema-changes-at-trust-boundaries were added.

## Self-Check: PASSED

- FOUND: `android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDetailScreen.kt` (327 lines)
- FOUND: `android/app/src/main/java/dev/disobey/mango/ui/SettingsToolDiscoveryScreen.kt` (410 lines, modified)
- FOUND: `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` (Screen.ContextvmToolDetail nav arm at line 197)
- FOUND commit: `70a1a8d` (`feat(36-02): cache-first render + search field + Used N× badge + chevron + whole-row nav on Discover Tools`)
- FOUND commit: `879ffc7` (`feat(36-02): SettingsToolDetailScreen + MainApp nav arm for ContextvmToolDetail`)
- VERIFIED: all 23 locked Phase 36 copy strings present verbatim in source (literal-grep table above)
- VERIFIED: `./gradlew :app:assembleDebug` BUILD SUCCESSFUL (orchestrator)
- VERIFIED: app launches and runs without crash on device 5A011JEBF06589 (orchestrator)
