---
phase: 29-wire-vectorindex-dek
plan: "01"
subsystem: rust-core
tags: [encryption, dek, vector-index, rag, auth]
dependency_graph:
  requires: [28-03]
  provides: [ENC-02]
  affects: [rag/index.rs, lib.rs]
tech_stack:
  added: []
  patterns: [zeroizing-dek-lifecycle, option-none-post-unlock-init]
key_files:
  modified:
    - rust/src/lib.rs
decisions:
  - ActorState.dek field follows db: Option<Database> lifecycle — None at startup, Some after unlock, None on lock
  - Case D startup defers VectorIndex to empty fallback; real index opened post-unlock with DEK (D-03/D-04)
  - UnlockWithDek and BiometricResult use hex::decode + try_from to convert keychain string to [u8; 32]
  - All 4 save call sites replaced with actor_state.dek.as_ref().map(|d| d.as_ref()) — backward compat when dek is None
metrics:
  duration: 8min
  completed: 2026-04-09
  tasks_completed: 2
  files_modified: 1
requirements:
  - ENC-02
---

# Phase 29 Plan 01: Wire VectorIndex DEK End-to-End Summary

**One-liner:** DEK wired from all four auth handlers through ActorState into every VectorIndex save/new call site, encrypting usearch index files with AES-256-GCM via Zeroizing lifecycle.

## What Was Built

Closed the gap between Phase 28-03's VectorIndex AES-256-GCM encryption API and its runtime invocation. Every VectorIndex operation now uses the real DEK from ActorState:

- `ActorState.dek: Option<Zeroizing<[u8; 32]>>` field added, initialized to `None`
- Case D startup (has_auth) defers VectorIndex to empty in-memory fallback — no unencrypted disk access before unlock
- SetupPin, UnlockWithDek, UnlockWithPin, BiometricResult all populate `actor_state.dek` and re-open VectorIndex with the real key
- LockApp clears `actor_state.dek = None` (Zeroizing zeros the 32 bytes on drop) and resets VectorIndex to empty fallback
- All 4 `vector_index.save(None)` call sites updated to `save(actor_state.dek.as_ref().map(|d| d.as_ref()))` — pre-encryption users (None DEK) continue writing unencrypted

## Tasks Completed

| Task | Description | Commit |
|------|-------------|--------|
| 1 | Add DEK field to ActorState and wire through all auth handlers | d428d6f |
| 2 | Plumb DEK from ActorState into all 4 VectorIndex save call sites | d428d6f |

## Verification Results

- `cargo build -p mango_core`: success
- `cargo test -p mango_core`: 265 passed, 0 failed
- `grep -c 'vector_index.save(None)' rust/src/lib.rs`: 0
- `grep -c 'save(actor_state.dek.as_ref().map' rust/src/lib.rs`: 4
- `grep -c 'actor_state.dek = Some' rust/src/lib.rs`: 4
- `grep 'actor_state.dek = None' rust/src/lib.rs`: present in LockApp handler

## Deviations from Plan

None — plan executed exactly as written. The plan's code snippets matched the actual codebase structure precisely.

## Known Stubs

None — all VectorIndex call sites are fully wired with real DEK.

## Threat Flags

No new security-relevant surface introduced. All mitigations from threat register implemented:
- T-29-01: `actor_state.dek = None` in LockApp triggers Zeroizing drop
- T-29-03: hex::decode intermediate Vec<u8> is short-lived in same expression block, result wrapped in Zeroizing immediately
- T-29-06: Case D creates empty in-memory fallback at startup — no disk file access before DEK is available

## Self-Check: PASSED

- rust/src/lib.rs: modified with all required changes
- Commit d428d6f: confirmed in git log
- All acceptance criteria met
