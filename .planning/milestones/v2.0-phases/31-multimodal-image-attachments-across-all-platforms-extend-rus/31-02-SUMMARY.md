---
phase: 31
plan: 02
subsystem: uniffi-bindings
tags: [multimodal, images, uniffi, bindings-regen, ios, android]
requires:
  - 31-01 (AttachmentInfo.is_image field + AppAction::AttachImage variant landed in rust/src/lib.rs)
provides:
  - Swift `AppAction.attachImage(filename:filePath:mimeType:)` case for SwiftUI dispatch
  - Swift `AttachmentInfo.isImage: Bool` for iOS pill rendering
  - Kotlin `AppAction.AttachImage(filename, filePath, mimeType)` data class for Jetpack Compose dispatch
  - Kotlin `AttachmentInfo.isImage: Boolean` for Android pill rendering
affects:
  - Plan 31-03 (Android UI: ChatScreen can now dispatch AppAction.AttachImage)
  - Plan 31-04 (iOS UI: ComposerBar can now dispatch AppAction.attachImage)
  - Plan 31-05 (Desktop: uses Rust directly — no impact)
tech-stack:
  added: []
  patterns:
    - iOS Bindings/ committed to repo (Phase 24 precedent) so Xcode builds without local Rust toolchain
    - Disable `profile.release.strip` for bindings-generation build so UniFFI metadata section survives
key-files:
  created:
    - .planning/phases/31-multimodal-image-attachments-across-all-platforms-extend-rus/31-02-SUMMARY.md
  modified:
    - ios/Bindings/mango_core.swift
    - android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt
decisions:
  - Root Cargo.toml ships `[profile.release] strip = true`, which strips the uniffi proc-macro metadata section from libmango_core.so. The justfile recipes (`bindings-swift` / `bindings-kotlin`) depend on `_host-build` which uses that stripped profile — when run as-is, uniffi-bindgen silently emits zero output (exit 0, no files written, no diff). Fix for this plan: rebuild once with `cargo build -p mango_core --release --config 'profile.release.strip=false'` before invoking bindgen. Long-term: either add a dedicated `bindings-build` profile with strip=false, or have the justfile recipes pass the `--config` override. Out of scope for 31-02 — captured here as follow-up work.
  - mango_coreFFI.h and mango_coreFFI.modulemap were regenerated but produced byte-identical output (AttachImage is a data-only enum variant addition and is_image is a Record field addition — neither changes the C FFI function surface). Only the high-level Swift and Kotlin files diffed; the .h/.modulemap stayed on disk but didn't need to be committed (git showed no diff). This is expected.
metrics:
  duration: ~8min
  tasks: 1
  files: 2
  completed: 2026-04-19
---

# Phase 31 Plan 02: UniFFI Bindings Regen Summary

**One-liner:** Regenerated Swift + Kotlin UniFFI bindings so `AppAction::AttachImage` and `AttachmentInfo.is_image` (landed by 31-01) are visible as `AppAction.attachImage` / `isImage` (Swift) and `AppAction.AttachImage` / `isImage` (Kotlin) to the platform UIs; Android Kotlin compileDebugKotlin passes cleanly.

## Outcome

- iOS Bindings (`ios/Bindings/mango_core.swift`) updated: adds `case attachImage(filename: String, filePath: String, mimeType: String)` on `AppAction` (discriminant 20) and `public var isImage: Bool` on `AttachmentInfo`.
- Android Bindings (`android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt`) updated: adds `data class AttachImage(val filename, val filePath, val mimeType)` on `AppAction` and `var isImage: kotlin.Boolean` on `AttachmentInfo`.
- `./gradlew :app:compileDebugKotlin` exits 0 (Kotlin parses cleanly, no downstream compile breakage).
- `mango_coreFFI.h` and `mango_coreFFI.modulemap` regenerated but byte-identical (no new FFI functions — only new enum variant data + new Record field, both carried through the existing FFI buffer path).
- Plans 31-03 (Android UI) and 31-04 (iOS UI) now have a symbolically valid `AttachImage` dispatcher available in both native languages.

## Tasks Completed

| # | Name | Commit | Files |
|---|------|--------|-------|
| 1 | Regenerate iOS + Android bindings, verify diff, commit | a62fc32 | ios/Bindings/mango_core.swift, android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt |

## Symbol Inventory

### Swift (ios/Bindings/mango_core.swift)

| Symbol | Kind | Line |
|--------|------|------|
| `AttachmentInfo.isImage: Bool` | struct field | 1659 |
| `AppAction.attachImage(filename:filePath:mimeType:)` | enum case | 3109 |

### Kotlin (android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt)

| Symbol | Kind | Line |
|--------|------|------|
| `AttachmentInfo.isImage: Boolean` | data class field | 2311 |
| `AppAction.AttachImage(filename, filePath, mimeType)` | sealed class data class | 3122 |

## Verification Results

- `grep -c 'attachImage' ios/Bindings/mango_core.swift` → **10** ✓ (≥ 1)
- `grep -c 'isImage' ios/Bindings/mango_core.swift` → **10** ✓ (≥ 1)
- `grep -c 'AttachImage' android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` → **4** ✓ (≥ 1)
- `grep -c 'isImage' android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` → **3** ✓ (≥ 1)
- `cd android && ./gradlew :app:compileDebugKotlin --quiet` → **exit 0** ✓

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `just bindings-*` recipes silently emit zero output because stripped release profile removes UniFFI metadata**

- **Found during:** Task 1 — after running `just bindings-swift` and `just bindings-kotlin`, grep returned 0 matches for the new symbols and `git diff --stat` showed no changes to the binding files. The recipes exit 0 with no warnings, so the failure is silent.
- **Root cause:** Root `Cargo.toml` sets `[profile.release] strip = true` (added in commit `abf4307` — "logos, deletion, duress", April 13). UniFFI's library-mode bindgen reads the proc-macro metadata section out of the compiled cdylib. `strip = true` removes that section. `libmango_core.so` still contains the live `AttachImage` / `is_image` string literals (confirmed via `strings target/release/libmango_core.so | grep AttachImage`), but the metadata section used by bindgen is gone.
- **Fix:** Rebuilt once with `cargo build -p mango_core --release --config 'profile.release.strip=false'`, then re-ran bindgen directly. Output files were correctly regenerated with the new symbols. Committed only the regen diff; did NOT modify the justfile or Cargo.toml as part of this plan (out of scope — follow-up work captured in `decisions` above).
- **Files modified:** ios/Bindings/mango_core.swift, android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt.
- **Commit:** a62fc32.

**2. [Rule 3 - Blocking] Worktree working tree contained an inverted "remove 31-01 changes" state**

- **Found during:** Task 1 preflight — `grep AttachImage rust/src/lib.rs` returned 0 despite HEAD containing 31-01's `be6dc7c` and `3f6834c` commits.
- **Root cause:** The orchestrator's initial `git reset --soft 5959d9a` kept the index pointing at HEAD, but the worktree carried pre-existing modifications that effectively reverted the 31-01 additions (the `M rust/src/lib.rs` staged entry had a diff that *removed* `AttachImage`, `pub is_image: bool`, and related lines). First Swift regen ran against a .so built from that stale source and therefore produced no new symbols.
- **Fix:** `git checkout HEAD -- rust/src/lib.rs rust/Cargo.toml Cargo.lock .planning/phases/31-.../31-01-SUMMARY.md` to align the worktree with HEAD. Remaining `M` entries are unrelated onboarding-screen changes left in the worktree from a prior session and were left untouched.
- **Files modified:** rust/src/lib.rs (reverted to HEAD), rust/Cargo.toml (reverted to HEAD), Cargo.lock (reverted to HEAD after a later gradle run also rewrote lockfile).
- **Commit:** Included implicitly by the rebuild; the binding regen commit a62fc32 contains only the binding files.

### Auth Gates

None — this is a pure bindings-regeneration plan.

## Threat Flags

None. Bindings regen does not introduce new network endpoints, auth paths, file access, or schema surface. The underlying `AttachImage` validation (absolute path, MIME allowlist, 50 MB cap) lives in the Rust actor and is unchanged; Kotlin/Swift simply get a typed dispatcher for it.

## Known Stubs

None. The regenerated bindings are complete and the Kotlin compile proves all types resolve.

## Follow-up Work (Out of Scope for 31-02)

- **justfile / Cargo.toml fix for bindings regen profile:** Future plans that regenerate bindings must either pass `--config 'profile.release.strip=false'` through `_host-build` or introduce a dedicated `[profile.bindings]` inheriting from release with `strip = false`. As-is, a naive `just bindings-kotlin` invocation is a silent no-op. Consider a small infra-level plan to fix this before the next bindings regen is required.

## Self-Check: PASSED

- ios/Bindings/mango_core.swift contains `case attachImage(filename: String, filePath: String, mimeType: String)` — FOUND (line 3109).
- ios/Bindings/mango_core.swift contains `public var isImage: Bool` — FOUND (line 1659).
- android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt contains `data class AttachImage(` — FOUND (line 3122).
- android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt contains `isImage` — FOUND (line 2311).
- Commit a62fc32 — FOUND on worktree-agent-ad5a0288.
- `./gradlew :app:compileDebugKotlin --quiet` — exit 0.
