---
phase: 28-local-data-encryption-authentication
plan: "03"
subsystem: rag
tags: [encryption, rag, vector-index, aes-gcm, dek]
dependency_graph:
  requires: ["28-01"]
  provides: ["encrypted-vector-index"]
  affects: ["rust/src/rag/index.rs", "rust/src/lib.rs"]
tech_stack:
  added: []
  patterns: ["AES-256-GCM file encryption via DEK", "MGO1 magic header for format detection", "temp file pattern for usearch encrypt/decrypt"]
key_files:
  created: []
  modified:
    - rust/src/rag/index.rs
    - rust/src/lib.rs
    - rust/src/tests/agent.rs
decisions:
  - "Pass None for DEK at all call sites until Plan 28-02 wires the real DEK"
  - "Use 0600 permissions on temp files to mitigate T-28-15 (temp file info disclosure)"
  - "Legacy unencrypted files detected by absence of MGO1 header and loaded transparently"
metrics:
  duration: "~30 minutes"
  completed: "2026-04-09"
  tasks_completed: 1
  tasks_total: 1
  files_changed: 3
---

# Phase 28 Plan 03: DEK-Based VectorIndex Encryption Summary

AES-256-GCM encryption added to VectorIndex save/load using crypto::file_crypto with MGO1 header detection for legacy compatibility.

## What Was Built

`VectorIndex::new` and `VectorIndex::save` now accept `Option<&[u8; 32]>` DEK parameters:

- **Save with DEK:** saves usearch index to a temp file (0600 permissions), reads bytes, encrypts with `file_crypto::encrypt_file`, writes encrypted blob to disk, deletes temp file
- **Load with DEK:** reads file bytes, checks for MGO1 header — if present, decrypts to temp file, loads usearch from temp, deletes temp; if absent (legacy), loads directly
- **No DEK:** saves/loads unencrypted for backwards compatibility
- **Encrypted file + no DEK:** returns error (cannot load without key)

All call sites in `lib.rs` and `tests/agent.rs` updated to pass `None` (Plan 28-02 will wire the real DEK).

## Tasks

| # | Name | Commit | Files |
|---|------|--------|-------|
| 1 | Add DEK parameter to VectorIndex::new and save; encrypt/decrypt on disk | 255bc76 | rust/src/rag/index.rs, rust/src/lib.rs, rust/src/tests/agent.rs |

## Tests Added

4 new tests in `rag::index::tests`:
- `test_encrypted_save_and_load_round_trip` — verifies MGO1 header present, vectors survive round-trip
- `test_wrong_dek_returns_error` — wrong DEK returns Err
- `test_legacy_unencrypted_loads_transparently` — no MGO1 header loaded without error when DEK provided
- `test_encrypted_file_no_dek_returns_error` — MGO1 file + no DEK returns Err

All 41 rag tests pass.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Restored worktree to correct base state**
- **Found during:** Task 1 setup
- **Issue:** Soft reset left many files (Cargo.toml, crypto module, agent/llm/persistence modules) in pre-phase-27 state, causing 24 compilation errors
- **Fix:** Restored all affected files from b1a0a10 commit using `git checkout b1a0a10 -- <files>`
- **Files modified:** rust/Cargo.toml, rust/src/crypto/*, rust/src/agent/mod.rs, rust/src/agent/tools.rs, rust/src/llm/mod.rs, rust/src/llm/streaming.rs, rust/src/persistence/*, rust/src/tests/*

**2. [Rule 2 - Missing critical functionality] Updated test call sites in tests/agent.rs**
- **Found during:** Task 1 compilation
- **Issue:** Two `VectorIndex::new()` calls in `tests/agent.rs` used old 1-argument signature
- **Fix:** Added `None` as second argument to both calls
- **Files modified:** rust/src/tests/agent.rs

## Known Stubs

None. Call sites pass `None` intentionally — this is by design, not a stub. Plan 28-02 will replace `None` with `Some(&dek)` when the DEK is wired from the actor state.

## Threat Flags

None. All T-28-15 (temp file info disclosure) and T-28-16 (GCM tamper detection) mitigations from the threat model are implemented:
- Temp files use 0600 permissions on Unix
- Temp files deleted immediately after use
- GCM authentication tag rejects tampered ciphertext (decrypt_file returns Err)

## Self-Check

- [x] `rust/src/rag/index.rs` exists and modified
- [x] `rust/src/lib.rs` call sites updated
- [x] `rust/src/tests/agent.rs` call sites updated
- [x] Commit 255bc76 exists
- [x] `grep -q "fn new.*dek.*Option"` passes
- [x] `grep -q "fn save.*dek.*Option"` passes
- [x] `grep -q "encrypt_file"` passes
- [x] `grep -q "decrypt_file"` passes
- [x] `grep -q "MGO1"` passes
- [x] `cargo build -p mango_core` passes (1 dead_code warning, no errors)
- [x] 41 rag tests pass

## Self-Check: PASSED
