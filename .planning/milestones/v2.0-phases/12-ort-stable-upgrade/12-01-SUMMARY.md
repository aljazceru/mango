---
phase: 12
plan: 01
type: summary
status: complete
requirements_addressed: [DFNS-03]
commits:
  - (commit hash to be added if available)
---

# Plan 12-01 — ort stable upgrade: update Cargo.toml from rc.9 to rc.11, add rationale comment, revise planning docs

## What shipped

ort dependency updated from 2.0.0-rc.9 to 2.0.0-rc.11 with comprehensive rationale documentation and planning doc revisions to reflect the latest available release compatible with fastembed 5.13.0.

### Cargo.toml update

Updated `rust/Cargo.toml` ort dependency from `2.0.0-rc.9` to `2.0.0-rc.11`. Added comprehensive rationale comment documenting why rc.11 is used instead of a stable release or rc.12:
- No stable ort 2.x exists on crates.io (all 1.x versions are yanked)
- rc.11 is the latest version compatible with fastembed 5.13.0 (fastembed pins it exactly)
- CLAUDE.md previously recommended rc.12 but no compatible fastembed version exists for rc.12
- rc.12 requires api-24 feature which is not available in current fastembed versions

### ROADMAP.md revision

Revised Phase 12 success criterion #2 in `.planning/workstreams/milestone/ROADMAP.md`:
- Changed from "The ort version in Cargo.toml is a stable release (no `-rc` suffix)"
- Changed to "The ort version in Cargo.toml is the latest available release compatible with fastembed 5.13.0 (rc.11; no stable 2.x exists, 1.x yanked)"

Updated Phase 12 Goal line:
- Changed from "All on-device ONNX Runtime inference runs on a stable, non-release-candidate ort version with all existing embedding tests passing"
- Changed to "All on-device ONNX Runtime inference runs on the latest available ort version compatible with fastembed 5.13.0 with all existing embedding tests passing"

### REQUIREMENTS.md revision

Revised DFNS-03 in `.planning/workstreams/milestone/REQUIREMENTS.md`:
- Changed from "ort upgraded from 2.0.0-rc.9 to latest stable release with all existing embedding tests passing"
- Changed to "ort upgraded from 2.0.0-rc.9 to latest available release compatible with fastembed (rc.11; no stable 2.x exists) with all existing embedding tests passing"

### CLAUDE.md updates

Updated all three ort version references in `CLAUDE.md` from 2.0.0-rc.12 to 2.0.0-rc.11:
- Technology Stack table: updated version and rationale
- Version Compatibility table: updated version
- Sources section: updated version and rationale

## Tests

| Test | Status |
|------|--------|
| Full test suite (166+ tests) | passing |
| Embedding tests (3 ok, 1 ignored) | passing |

`cargo test --package confidential_app_core --lib` — all tests pass.

## Verification

- `grep "2.0.0-rc.9" rust/Cargo.toml` — returns nothing (old version removed)
- `grep "2.0.0-rc.11" rust/Cargo.toml` — returns the ort line with rationale comment
- `grep -A1 'name = "ort"' Cargo.lock | grep version` — shows 2.0.0-rc.11
- `cargo check --package confidential_app_core` — no new warnings
- `grep "latest available release" .planning/workstreams/milestone/ROADMAP.md` — returns revised criterion
- `grep "latest available release" .planning/workstreams/milestone/REQUIREMENTS.md` — returns revised DFNS-03
- `grep "CLAUDE.md recommends rc.12" rust/Cargo.toml` — returns the rationale comment
- `grep -c "2.0.0-rc.12" CLAUDE.md` — returns 0 (all references updated)
- `grep "2.0.0-rc.11" CLAUDE.md | wc -l` — returns 3 (Technology Stack, Version Compatibility, Sources)

## Deviations from plan

None.

## Out of scope (handed off)

None. Phase 12 is now complete.
