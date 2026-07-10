---
phase: 32
slug: directory-based-rag-ingestion-with-periodic-sync-and-file-fo
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-19
---

# Phase 32 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `tokio::test` for async |
| **Config file** | None — inline tests in modules |
| **Quick run command** | `cargo test -p mango_core --lib rag::directory_sync -- --test-threads 1` |
| **Full suite command** | `cargo test -p mango_core` |
| **Estimated runtime** | ~60 seconds (full); ~10 seconds (quick) |

---

## Sampling Rate

- **After every task commit:** Run quick command (targeted module tests)
- **After every plan wave:** Run full suite command
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 32-01-01 | 01 | 0 | DIR-01 | — | N/A | unit | `cargo test -p mango_core test_directory_diff` | ❌ W0 | ⬜ pending |
| 32-01-02 | 01 | 0 | DIR-02 | T-32-V5 | exclusion globs cannot escape walk root | unit | `cargo test -p mango_core test_exclusion_globs` | ❌ W0 | ⬜ pending |
| 32-02-01 | 02 | 0 | DIR-03 | — | N/A | unit | `cargo test -p mango_core test_directory_source_queries` | ❌ W0 | ⬜ pending |
| 32-02-02 | 02 | 0 | DIR-04 | — | N/A | unit | `cargo test -p mango_core test_directory_file_fingerprints` | ❌ W0 | ⬜ pending |
| 32-03-01 | 03 | 1 | DIR-05 | T-32-V4 | reads scoped to granted folder only | integration | `cargo test -p mango_core test_sync_directory_files_handler` | ❌ W0 | ⬜ pending |
| 32-03-02 | 03 | 1 | DIR-06 | — | cascades chunks + vector keys | unit | `cargo test -p mango_core test_remove_directory_source` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

*Task IDs are provisional — planner may renumber based on final task breakdown.*

---

## Wave 0 Requirements

- [ ] `rust/src/rag/directory_sync.rs` — module + unit tests for `diff_files` and `walk_with_exclusions`
- [ ] `rust/src/persistence/queries.rs` — tests for `directory_sources` + `directory_files` CRUD
- [ ] `rust/src/tests/directory_rag.rs` — integration test for `SyncDirectoryFiles` actor handler
- [ ] Test fixtures: `tests/fixtures/mock_vault/` with a few `.md` + `.obsidian/` + `.tmp` files for glob exclusion tests

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| iOS security-scoped bookmark round-trip on physical device | D-14, D-15 | Simulator does not enforce sandbox; `.minimalBookmark` resolution must be validated on real iPhone | 1) Add a Files-app folder as source; 2) force-quit and relaunch; 3) trigger Sync Now; 4) confirm files ingest and `last_synced_at` updates |
| Android SAF URI persistence across reboot | D-18 | Requires real device reboot, not instrumentation | 1) Pick Obsidian vault via SAF picker; 2) reboot device; 3) reopen app; 4) confirm sync still succeeds without re-picking |
| Android WorkManager 15-min periodic sync firing | D-23 | WorkManager backoff/throttling behavior is timing-dependent on real device | 1) Add a source; 2) modify a file externally; 3) wait 15-30 min with app backgrounded; 4) reopen and confirm the change was indexed |
| Desktop `notify` FS watcher on Linux large vault | D-10, D-11 | inotify limits are host-specific; behavior under `ENOSPC` needs real exhaustion | 1) Add a 10k-file vault; 2) confirm either watcher works OR PollWatcher fallback activates with UI warning |
| iCloud-evicted file skip behavior | D-17 | Requires iCloud-synced folder with evicted files | 1) Add iCloud-synced folder; 2) offload a file via Files app; 3) trigger sync; 4) confirm file is skipped and UI surfaces "not downloaded locally" |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
