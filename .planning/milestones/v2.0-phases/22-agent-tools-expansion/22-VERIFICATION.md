---
phase: 22-agent-tools-expansion
verified: 2026-04-04T16:10:00Z
status: passed
score: 13/13 must-haves verified
re_verification: false
---

# Phase 22: Agent Tools Expansion Verification Report

**Phase Goal:** Agents can search the web, read URLs, manipulate files, and perform precise math -- all integrated into the existing ReAct loop with step checkpointing
**Verified:** 2026-04-04T16:10:00Z
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1  | `build_agent_tools()` returns 7 tool schemas (3 existing + 4 new) | VERIFIED | `tools.rs` vec! literal has 7 entries; `test_agent_tools_count_seven` and `test_agent_tools_build` pass |
| 2  | `dispatch_tools` accepts `runtime`, `data_dir`, `brave_api_key` parameters | VERIFIED | Signature at `tools.rs:218-226`; call site at `lib.rs:1646-1654` |
| 3  | `web_search` dispatch returns error when API key is empty | VERIFIED | `tools.rs:367-370` empty-key guard; `test_web_search_no_api_key_returns_error` passes |
| 4  | `fetch_url` dispatch strips HTML and returns plain text from body element | VERIFIED | `tools.rs:477-498` uses `scraper::Html::parse_document` + `Selector::parse("body")`; unreachable-URL test passes |
| 5  | `file` dispatch rejects paths containing `..` with a clear error | VERIFIED | `tools.rs:513-518` check; `test_file_path_traversal_rejected` passes |
| 6  | `calculate` dispatch evaluates arithmetic expressions correctly | VERIFIED | `tools.rs:639` uses `evalexpr` crate; `test_calculate_basic` (2+3*4=14) passes |
| 7  | All dispatch functions return error strings on invalid input without panicking | VERIFIED | `test_calculate_invalid_no_panic`, `test_dispatch_tools_malformed_args_no_panic` pass |
| 8  | Agent can execute `web_search` tool via ReAct loop | VERIFIED | `dispatch_tools` match arm at `tools.rs:240`; `test_dispatch_all_known_tools` confirms no "unknown tool" |
| 9  | Agent can fetch a URL and receive stripped text content | VERIFIED | `dispatch_tools` match arm at `tools.rs:241`; same dispatch test |
| 10 | Agent can create, read, and edit files in the sandbox directory | VERIFIED | `dispatch_tools` match arm at `tools.rs:242`; `test_file_write_read_roundtrip` passes |
| 11 | Agent can evaluate math expressions via the calculate tool | VERIFIED | `dispatch_tools` match arm at `tools.rs:243`; `test_calculate_basic` passes |
| 12 | All four new tools dispatched in existing ReAct loop with step checkpointing | VERIFIED | `lib.rs:1646-1654` call site passes `runtime`, `data_dir`, `brave_api_key`; all 225 tests pass |
| 13 | Agent system prompt lists all 7 available tools | VERIFIED | `lib.rs:1501-1506` (launch) and `lib.rs:1865-1870` (resume) both enumerate all 7 tool names |

**Score:** 13/13 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `rust/src/agent/tools.rs` | 4 new tool schemas and 4 new dispatch functions | VERIFIED | 644 lines; contains `dispatch_web_search`, `dispatch_fetch_url`, `dispatch_file`, `dispatch_calculate`, `resolve_sandbox_path`, `format_brave_results` |
| `rust/Cargo.toml` | `scraper` and `evalexpr` dependencies | VERIFIED | Line 62: `scraper = "0.26"`, line 63: `evalexpr = "13.1"` |
| `rust/src/lib.rs` | `ActorState.data_dir` field, updated dispatch call sites, updated system prompts | VERIFIED | `data_dir: String` at line 675; dispatch wired at lines 1646-1654; both prompts updated at lines 1501 and 1865 |
| `rust/src/tests/agent.rs` | 11 new tests for all new tools and error paths | VERIFIED | All 11 new tests present and passing (30 total agent tests, 1 live test ignored) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `rust/src/agent/tools.rs` | `reqwest::Client` | `runtime.block_on()` in web_search and fetch_url | WIRED | `runtime.block_on(async move { ... })` at lines 390 and 456 |
| `rust/src/agent/tools.rs` | `evalexpr` crate | Direct call in `dispatch_calculate` | WIRED | `evalexpr::eval(&expression)` at line 639 (plan said `evaluate`; `eval` is the correct public API -- tests confirm) |
| `rust/src/agent/tools.rs` | `scraper::Html` | HTML parsing in `dispatch_fetch_url` | WIRED | `Html::parse_document(&html)` at line 477; import at line 18 |
| `rust/src/lib.rs` | `agent::dispatch_tools` | Updated call with `runtime`, `data_dir`, `brave_api_key` | WIRED | `dispatch_tools(..., &actor_state.runtime, &actor_state.data_dir, &brave_api_key)` at lines 1646-1654 |
| `rust/src/lib.rs` | `persistence::queries::get_setting` | `brave_api_key` lookup before dispatch | WIRED | `get_setting(actor_state.db.conn(), "brave_api_key")` at lines 1639-1644 |
| `rust/src/lib.rs` | `ActorState` | `data_dir` field stored at init | WIRED | `data_dir: vector_data_dir.clone()` in `ActorState` literal at line 2372 |

### Data-Flow Trace (Level 4)

Not applicable -- `tools.rs` is a dispatch/computation module, not a rendering component. The dispatch functions return `String` values consumed by the agent ReAct loop. The `lib.rs` wiring feeds real runtime, DB, and data_dir values -- the Plan 01 temporary stubs (empty strings) were confirmed removed in Plan 02 at lines 1651-1652.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| All 30 agent tests pass | `cargo test -p mango_core agent` | 30 passed, 0 failed, 1 ignored | PASS |
| Full test suite passes | `cargo test -p mango_core` | 225 passed, 0 failed, 9 ignored | PASS |
| `build_agent_tools()` returns 7 tools | `test_agent_tools_count_seven` | ok | PASS |
| Path traversal rejected | `test_file_path_traversal_rejected` | ok | PASS |
| Math evaluation correct (2+3*4=14) | `test_calculate_basic` | ok | PASS |
| Empty API key returns error | `test_web_search_no_api_key_returns_error` | ok | PASS |
| File write/read roundtrip | `test_file_write_read_roundtrip` | ok | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| TOOL-01 | 22-01, 22-02 | Agent can search the web using Brave Search API | SATISFIED | `dispatch_web_search` with Brave API call; wired via `dispatch_tools` match arm; error guard for missing key |
| TOOL-02 | 22-01, 22-02 | Agent can fetch and read content from URLs (HTML parsed to text) | SATISFIED | `dispatch_fetch_url` with `scraper` HTML parsing; body text extraction; 8000-char truncation |
| TOOL-03 | 22-01, 22-02 | Agent can create, read, and edit files in the app sandbox | SATISFIED | `dispatch_file` with `resolve_sandbox_path`; read/write/append ops; `..` rejected; sandbox at `data_dir/agent_files/` |
| TOOL-04 | 22-01, 22-02 | Agent can evaluate mathematical expressions with precision | SATISFIED | `dispatch_calculate` using `evalexpr` crate; 200-char limit; graceful error on invalid input |
| TOOL-05 | 22-01, 22-02 | Agent tool dispatch integrates with existing ReAct loop and step checkpointing | SATISFIED | `lib.rs:1646-1654` in `handle_agent_step_complete`; step checkpointing unchanged; both system prompts updated |

All 5 requirements satisfied. No orphaned requirements -- REQUIREMENTS.md marks TOOL-01 through TOOL-05 as Complete at Phase 22.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `rust/src/lib.rs` | 189, 246, 440 | `show_first_chat_placeholder` references | Info | Pre-existing Phase 17 feature flag; not introduced by Phase 22; not a stub |

No Phase-22-introduced anti-patterns found. The Plan 01 temporary stubs (empty `data_dir` and `brave_api_key` in the call site) were removed in Plan 02 as planned.

One API note: the plan specified `evalexpr::evaluate` but the implementation uses `evalexpr::eval`. This is the correct public API for the `evalexpr 13.1` crate -- both names refer to the same operation and the tests confirm correct evaluation behavior.

### Human Verification Required

None. All tool behaviors are verifiable programmatically:
- Web search: guarded by API key check (testable with empty key)
- URL fetch: testable with unreachable address (RFC 5737 TEST-NET 192.0.2.1)
- File sandbox: testable with tempfile
- Math: pure computation

Live Brave API integration (requiring a valid key) is handled by the existing ignored test `test_live_agent_session_completes`.

### Gaps Summary

No gaps found. Phase goal fully achieved:
- Agents can search the web (web_search tool, Brave API, empty-key guard)
- Agents can read URLs (fetch_url tool, scraper HTML parsing, text extraction)
- Agents can manipulate files (file tool, sandbox isolation, path traversal prevention)
- Agents can perform precise math (calculate tool, evalexpr crate, error handling)
- All four new tools are integrated into the existing ReAct loop via the single `dispatch_tools` call site in `handle_agent_step_complete` with step checkpointing intact
- Both agent system prompts (launch and resume) enumerate all 7 tools
- 225 tests pass, 0 failures

---

_Verified: 2026-04-04T16:10:00Z_
_Verifier: Claude (gsd-verifier)_
