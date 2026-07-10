# Phase 35 Plan Check

**Verdict:** PASS-WITH-WARNINGS
**Audited:** 2026-05-08

## Summary

The 10 plans cover every CTX-NN requirement, honor every locked CONTEXT decision, and embed the full UI-SPEC copy contract verbatim. Wave/dependency graph is acyclic and conservative. Threat models cover the four required boundaries. Anti-OpenSSL audit is built into Plans 35-00 and 35-09. Tasks are specific and graspable — concrete code, grep-checkable acceptance criteria, named symbols and file:line targets — not "wire X up" hand-waving.

Two non-blocking warnings worth fixing during execution: (a) Plan 35-04's example code uses `async_openai::types::ChatCompletionTool` directly while the existing `build_chat_tools` returns `Vec<ChatCompletionTools>` (plural — the wrapper enum). The plan flags this in a NOTE but the executor should expect to use `ChatCompletionTools::Function(ChatCompletionTool { ... })` to match the existing return type; otherwise `tools.extend(...)` won't compile. (b) Plan 35-05 has a few "the executor must check" hedges where the actor's internal-event channel name (`internal_tx`) and `emit_state` helper are guessed from convention rather than read directly — minor friction, not a blocker.

## Per-dimension audit

| # | Dimension | Status | Notes |
|---|-----------|--------|-------|
| 1 | Goal-backward coverage | OK | Every CTX-NN has a clear owning plan and a green-able acceptance criterion. See Coverage Matrix. |
| 2 | CONTEXT compliance | OK | All D-01..D-13 honored. Claude's-discretion items locked: auto-discover heuristic (35-05 §F), connection lifecycle (35-02 one-shot), storage schema (35-01 dedicated table + ALTER), dispatch shape (35-04 in-memory map), refresh model (pull-on-open via LaunchedEffect/PushScreen), loading/empty states (UI-SPEC §C-§F mirrored in 35-06/07), auto-discover scope (once per conv, gated on `current_conv_contextvm_tools.is_empty()`). |
| 3 | UI-SPEC compliance | OK | Every locked copy string in UI-SPEC Copywriting Contract appears verbatim in 35-06 / 35-07 actions or acceptance criteria. All 5 states implemented per platform. Provenance badge present. |
| 4 | RESEARCH alignment | OK | Plans use `discover_servers`, `discover_tools_typed`, `NostrMCPProxy::new`, `EncryptionMode::Optional`, `JsonRpcMessage::Request` — the exact surface RESEARCH §A locked. Plan 35-02 picks `discover_tools_typed` (typed) per RESEARCH recommendation. |
| 5 | Task quality (anti-shallow) | WARNING | Most tasks are excellent (concrete code blocks, specific line numbers, grep checks). Two soft spots: 35-05 Task 2 has placeholder helper-name guesses (`emit_state`, `internal_tx`, `now_secs`) and a "executor must check" line; 35-07 Task 3 has a `style(/* reuse... */)` placeholder comment instead of inlined style closures. |
| 6 | Wave / dependency correctness | OK | Wave 0 (35-00) → Wave 1 (35-01/02/03) → Wave 2 (35-04/05) → Wave 3 (35-06/07/08) → Wave 4 (35-09). 35-04 depends on 35-01,02,03; 35-05 depends on 35-04; 35-06/07/08 depend on 35-05; 35-09 depends on 35-06/07/08. No cycles, no Wave-1 plan reading Wave-2 outputs. |
| 7 | Frontmatter integrity | OK | All 10 frontmatter blocks parse as valid YAML. `requirements`, `requirements_addressed`, `files_modified`, `must_haves` all populated. `depends_on` arrays are well-formed. |
| 8 | Threat model | OK | 35-02, 35-03, 35-04, 35-05, 35-07 carry STRIDE blocks. The four required threats are covered: untrusted Nostr event (T-35-S1), response→LLM context (T-35-T2/T7), prompt injection via tool descriptions (T-35-T4), pubkey identity (T-35-S3/S6). 35-06 lacks STRIDE — UI-only, low-risk, but flagged below. 35-08 (bindings) and 35-09 (verification) reasonably skip threat blocks. |
| 9 | Anti-OpenSSL constraint | OK | 35-00 Task 1 runs the cargo-tree audit at dep-add time. 35-09 Task 2 re-runs it as a post-implementation gate. Plan 35-02 verification reiterates "still shows ONLY pre-existing rusqlite edge". No other plans add Cargo deps, so no extra audits required. |
| 10 | Build sweep coverage | OK | 35-09 covers `cargo test -p mango_core --lib`, `cargo build -p mango_core --release`, `cargo build -p mango-desktop --release`, `cargo ndk -t arm64-v8a build`, and iOS via `just build-ios`. iOS is correctly noted as human-verify if non-macOS host (acceptable per CLAUDE.md). |
| 11 | Atomic-commit discipline | WARNING | Plans don't include explicit "commit this" instructions. The `<output>` block creates a SUMMARY.md but doesn't tell the executor to git-commit per plan. The `/gsd-execute-phase` workflow implicitly commits per plan, so this is mostly fine, but a `git commit -m "feat(35-NN): ..."` line in each plan would tighten the loop. |
| 12 | Anti-redundancy | OK | Dispatch routing is in 35-04 only. Persistence is in 35-01 only. Discovery service is in 35-02 only. Invocation is in 35-03 only. Actor wiring is in 35-05 only. Each platform UI is its own plan. No double-implementation. |

## Spot-check anchors

| Anchor | Status | Detail |
|---|---|---|
| `rust/src/agent/tools.rs:249` — dispatch_tools | VERIFIED | `pub fn dispatch_tools(` is at line 249 exactly. |
| `rust/src/agent/tools.rs:206` — tool array build | VERIFIED | `pub fn build_chat_tools(` is at line 206 exactly. |
| `rust/src/lib.rs:446` — AppAction enum | VERIFIED | `pub enum AppAction {` is at line 446 exactly. |
| `rust/src/lib.rs:97` — AgentStepSummary | DRIFTED-1 | The struct begins at line 98, not 97 (1-line drift — well within tolerance; doc comment owns 97). Plans citing :97 still work because they grep, not seek by line. |
| `rust/src/persistence/queries.rs:930-945` — settings persistence | VERIFIED | `pub fn get_setting` at 931, `pub fn set_setting` at 941. Range matches. |
| `rust/src/persistence/schema.rs:347` — last migration (V19) | VERIFIED | `pub const MIGRATION_V19: &str = "` at line 347 exactly. MIGRATIONS array entry at 373. |
| `SettingsScreen.kt:127-135` Tools section | DRIFTED-2 | `SettingsSectionLabel("Tools")` is at line 129; `SettingsLinkCard(...)` for the Tools row is at 130-135. Plan 35-06 uses 127-135 as a range and the actual section spans 127-135 with a 2-line shift on the section label. Plans grep for the `SettingsSectionLabel("Tools")` string, so this works. |
| `desktop/iced/src/views/settings.rs:135-164` providers_summary | VERIFIED | `let providers_summary = container(` at line 135. The button + style block extends through ~164. Match. |
| `desktop/iced/src/views/settings.rs:344-366` memory_toggle | DRIFTED-1 | `let memory_toggle = container(` at line 345 (research said 344). Single-line drift; plan-grep-by-pattern still works. |
| `desktop/iced/src/views/agents.rs:443-540` build_step_row | VERIFIED | `fn build_step_row<'a>(` at line 443. Tool-name render is at line 504-506 (research said ~505). Match. |
| `AgentScreen.kt:339-373` AgentStepItem | VERIFIED | `private fun AgentStepItem(step: AgentStepSummary) {` at line 339. Match. |

All anchors are within ±2 lines or use grep patterns rather than seek. None of the drift is plan-breaking.

## Issues (sorted by severity)

### BLOCKERS

(none)

### WARNINGS

- **Plan 35-04 type-name confusion.** The plan code in `descriptors_to_chat_tools` uses `async_openai::types::ChatCompletionTool` directly. The existing `rust/src/agent/tools.rs:16` imports both `ChatCompletionTool` AND `ChatCompletionTools` and the `build_chat_tools` return type is `Vec<ChatCompletionTools>` (plural — a wrapper enum). To make `tools.extend(remote)` compile in `build_chat_tools_with_contextvm`, the helper must return `Vec<ChatCompletionTools>` and wrap each entry as `ChatCompletionTools::Function(ChatCompletionTool { ... })`. The plan flags this in a NOTE but the executor should treat the NOTE as a hard rule — substitute the wrapper enum, don't substitute the inner type. Rec: tighten the plan's example code or change the NOTE to a `<must>`.

- **Plan 35-05 helper-name guesses.** Lines 327, 329, 335 reference `emit_state(actor_state)` and `actor_state.internal_tx` as if they exist verbatim. The actor's actual channel name and state-emit helper aren't pre-verified — the plan defers this to "the executor must check". This is fine for an experienced executor but adds friction. Rec: Pre-grep `internal_tx` and the existing `InternalEvent` send pattern in lib.rs and pin the names before execution.

- **Plan 35-05 `now_secs()` not defined or imported.** Used at lines 357 and 436 with no `read_first` requirement to find or define it. If it doesn't exist (it's not a stdlib function), the plan needs `use std::time::{SystemTime, UNIX_EPOCH}` or a defined helper. Rec: Add `fn now_secs() -> i64 { ... }` to the helpers section in Task 2 step 10.

- **Plan 35-06 lacks a STRIDE block.** Even though Android UI is mostly low-risk, this plan touches the trust boundary where untrusted tool descriptions render in the discovery list. UI-SPEC §F says descriptions can be 1-2 lines ellipsised — the cap is enforced upstream at 500 chars in 35-04, and `Text(...)` is plain rendering, but acknowledging this in a 2-row STRIDE block matches the discipline of 35-07 (which DOES have a STRIDE block on the same surface). Rec: Mirror 35-07's `<threat_model>` block into 35-06.

- **Plan 35-07 Task 2 has a placeholder style block.** `style(/* reuse providers_summary style: ... */)` and `style(/* same card style as memory_toggle */)` are comments — not actual closure code. The executor will have to copy the closures from settings.rs:156 and :356 verbatim. The plan tells them to but doesn't paste. Rec: Inline the actual `move |_, _| button::Style { ... }` closures so the action is paste-ready.

- **Plan 35-08 Kotlin binding path.** Plan 35-06 Task 1 says `android/app/src/main/java/dev/disobey/mango/uniffi/mango_core/mango_core.kt` while Plan 35-08 frontmatter says `android/app/src/main/java/uniffi/mango_core/mango_core.kt`. The actual UniFFI Kotlin output path is normally `<...>/uniffi/mango_core/mango_core.kt`. The Plan-06 path is wrong (extra `dev/disobey/mango/` segment). Rec: Verify and unify the path in both plans.

- **Atomic-commit instruction missing per plan.** No `git add ... && git commit -m "..."` line in any plan's `<output>` block. The `/gsd-execute-phase` workflow handles commits, but if executed via `/gsd-quick` or directly the executor will need to remember. Rec: Add a one-line commit instruction at the bottom of each plan's `<output>` to make the contract explicit.

### NOTES

- The mismatch between CONTEXT D-08 ("contextvm-sdk default relays plus relay.nostr.net") and reality (no upstream defaults exist) is explicitly resolved in RESEARCH §A and the resolution propagates correctly into Plan 35-02's `DEFAULT_CONTEXTVM_RELAYS = [damus.io, nos.lol, relay.nostr.net]`. This is a CONTEXT amendment and is documented as such.

- Plan 35-03's `INVOCATION_TIMEOUT_SECS = 15` differs from RESEARCH §G's "30s recommended" and §H's discussion of "30s" — Plan 35-03 went tighter (15s). The trade-off is documented implicitly via the locked copy `Error: tool '<name>' timed out (15s)`. Either is defensible; the planner picked tighter for v1. Acceptable.

- Plan 35-04's `MAX_REMOTE_TOOLS_PER_TURN = 8` matches RESEARCH §F exactly. `DESCRIPTION_CAP_CHARS = 500` is new (not in RESEARCH but motivated by threat model T-35-T4). Both locked correctly.

- The CTX-09 binding-availability test in 35-08 is permissive on Linux re: Swift bindings (the `if let Ok(content) = ...` guard). This is correct and intentional — Linux dev box has no Swift toolchain assumption.

- The RESERVED_LOCAL_NAMES list in 35-04 (`search_documents, read_document, finish, web_search, fetch_url, file, calculate`) exactly mirrors the 7 match arms in the existing dispatch_tools at `rust/src/agent/tools.rs:249`. Verified.

- 35-05's `AgentStepRow` extension (Task 1 step 4) correctly notes that the persistence struct + SELECT/INSERT helpers must all be touched. The executor must follow through; the plan flags it but doesn't pre-grep the affected statements. This is consistent with the plan's general "executor verifies the actual code shape" posture.

- Plans 35-06, 35-07, 35-08 are wave-3 parallel and have no shared file edits. Safe to run in parallel.

## Coverage matrix

| Requirement | Plan(s) | Plan-coverage status |
|---|---|---|
| CTX-01 (contextvm-sdk integrated, pure-Rust, no OpenSSL) | 35-00, 35-09 | OK — dep add + cargo-tree audit at start, re-audit at end. |
| CTX-02 (Settings → Tools "Discover tools" row + screen on Android & Desktop) | 35-05, 35-06, 35-07 | OK — Screen variant + UniFFI in 35-05; Android in 35-06; Desktop in 35-07. |
| CTX-03 (per-tool enable persisted across launches) | 35-01, 35-05 | OK — table+CRUD in 35-01, AppAction handler + reload in 35-05. |
| CTX-04 (auto-discover toggle defaults off, persisted) | 35-01, 35-05 | OK — settings key in 35-01; SetAutoDiscoverTools handler + hydration at unlock in 35-05. |
| CTX-05 (enabled tools surface in OpenAI-compatible `tools` array via existing dispatch path) | 35-04, 35-05 | OK — `build_chat_tools_with_contextvm` in 35-04; map hydration at conv start in 35-05. |
| CTX-06 (invocation routes through Nostr; result returned to LLM as tool-call response) | 35-03, 35-04, 35-05 | OK — `invoke_tool` in 35-03; `dispatch_tools` fallback arm in 35-04; map population in 35-05. |
| CTX-07 (relay set = contextvm-sdk defaults + `relay.nostr.net`) | 35-02 | OK — `DEFAULT_CONTEXTVM_RELAYS` const + ctx_07 test. |
| CTX-08 (graceful degradation on relay/announcement/invocation failure) | 35-02, 35-03, 35-05 | OK — `ContextvmError` enum in 35-02; error-string mapping in 35-03; `Error { message }` AppState in 35-05. |
| CTX-09 (UniFFI bindings regenerated for all 3 platforms; iOS Swift UI deferred but bindings updated) | 35-08 | OK — `just bindings-swift` + `just bindings-kotlin` + ctx_09 grep test. |
| CTX-10 (tool-call provenance — agent step summary surfaces remote-vs-local origin) | 35-01, 35-05, 35-06, 35-07 | OK — `tool_origin` column in 35-01; AgentStepSummary field + population in 35-05; "Remote" badge in 35-06 (Android) and 35-07 (Desktop). |

## Recommendation

**Proceed with warnings noted in PLAN.md frontmatter.** The plans are detailed enough to execute without further planning rounds. Before kicking off Wave 2 specifically, the executor of 35-04 should verify the `ChatCompletionTools` (plural) wrapping pattern in the actual codebase and adjust `descriptors_to_chat_tools` accordingly. Before kicking off 35-05, run a 5-minute pre-grep of `internal_tx`, `emit_state`, and existing `InternalEvent::` send sites to pin the canonical names. The remaining warnings (35-06 STRIDE, 35-07 style closures, atomic-commit instruction) are quality-of-life nits that don't block correctness.
