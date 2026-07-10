---
phase: 22
slug: agent-tools-expansion
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-04
---

# Phase 22 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test (`cargo test`) |
| **Config file** | None (workspace uses `cargo test`) |
| **Quick run command** | `cargo test -p mango_core agent -- --nocapture` |
| **Full suite command** | `cargo test -p mango_core` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p mango_core agent -- --nocapture`
- **After every plan wave:** Run `cargo test -p mango_core`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 22-01-01 | 01 | 0 | TOOL-01 | unit | `cargo test -p mango_core test_agent_tools_include_web_search` | Wave 0 | ⬜ pending |
| 22-01-02 | 01 | 0 | TOOL-01 | unit | `cargo test -p mango_core test_web_search_no_api_key_returns_error` | Wave 0 | ⬜ pending |
| 22-01-03 | 01 | 0 | TOOL-01 | live | `cargo test -p mango_core test_live_web_search -- --ignored` | Wave 0 | ⬜ pending |
| 22-01-04 | 01 | 0 | TOOL-02 | unit | `cargo test -p mango_core test_agent_tools_include_fetch_url` | Wave 0 | ⬜ pending |
| 22-01-05 | 01 | 0 | TOOL-02 | unit | `cargo test -p mango_core test_fetch_url_html_stripped` | Wave 0 | ⬜ pending |
| 22-01-06 | 01 | 0 | TOOL-02 | unit | `cargo test -p mango_core test_fetch_url_unreachable_returns_error` | Wave 0 | ⬜ pending |
| 22-01-07 | 01 | 0 | TOOL-03 | unit | `cargo test -p mango_core test_agent_tools_include_file` | Wave 0 | ⬜ pending |
| 22-01-08 | 01 | 0 | TOOL-03 | unit | `cargo test -p mango_core test_file_write_read_roundtrip` | Wave 0 | ⬜ pending |
| 22-01-09 | 01 | 0 | TOOL-03 | unit | `cargo test -p mango_core test_file_path_traversal_rejected` | Wave 0 | ⬜ pending |
| 22-01-10 | 01 | 0 | TOOL-04 | unit | `cargo test -p mango_core test_agent_tools_include_calculate` | Wave 0 | ⬜ pending |
| 22-01-11 | 01 | 0 | TOOL-04 | unit | `cargo test -p mango_core test_calculate_basic` | Wave 0 | ⬜ pending |
| 22-01-12 | 01 | 0 | TOOL-04 | unit | `cargo test -p mango_core test_calculate_invalid_no_panic` | Wave 0 | ⬜ pending |
| 22-01-13 | 01 | 0 | TOOL-05 | unit | `cargo test -p mango_core test_agent_tools_count_seven` | Wave 0 | ⬜ pending |
| 22-01-14 | 01 | 0 | TOOL-05 | unit | `cargo test -p mango_core test_dispatch_all_known_tools` | Wave 0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `rust/src/tests/agent.rs` — new test functions for web_search, fetch_url, file, calculate tool schemas and dispatch
- [ ] Live test tagged `#[ignore]` for Brave API (requires `BRAVE_API_KEY` env var at runtime)
- [ ] File I/O tests use `tempfile::tempdir()` for sandbox isolation (already available)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live Brave search returns results | TOOL-01 | Requires API key and network | Set `BRAVE_API_KEY` env var, run `cargo test -p mango_core test_live_web_search -- --ignored` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
