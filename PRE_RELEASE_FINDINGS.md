# Mango — Pre-Open-Source Release Review Findings

Date: 2026-07-04
Reviewer: Claude (Fable 5)
Scope: release-readiness of `main` at HEAD `959d364` ("fixes for pre release review")
Method: verified all prior blockers (`RELEASE_REVIEW.md`, `CODEX_REVIEW.md`, `REVIEW_VERIFIED.md`)
are closed at HEAD, then reviewed the fix commit itself for regressions, ran the build/tests,
and did an open-source hygiene sweep.

## Health snapshot

- **Tests:** 573 passed, 0 failed, 21 ignored (`cargo test --lib`, ~22 s).
- **Build:** clean.
- **Secrets:** none found in git history; keystores / `local.properties` / `macpass.txt` / `.env*`
  are gitignored and untracked.

## Prior blockers — all confirmed fixed at HEAD

| Prior ID | Item | Status at HEAD |
|----------|------|----------------|
| C1 | Mobile wipe guard no-ops | Fixed — `wipe_data_dir_allowed` accepts Android `filesDir` + iOS App Support, with regression tests |
| C2 | Cold-launch bypass across Never→finite | Fixed — bypass flag cleared on any finite timeout regardless of biometric state |
| C3 / M-1 | Sync-after-lock panics / 143 `expect("db unlocked")` | Fixed — `let Some(db) = … else { return }` guards pushed into the hot helpers |
| C4 | CI never builds llama.cpp libs | Fixed — CI builds and copies the four `.so` before `assembleRelease` |
| C5 | Release falls back to debug signing | Fixed — `requireReleaseSigning` throws `GradleException`, wired to `assembleRelease`/`bundleRelease` (no debug/unsigned fallback) |
| C6 | `allowBackup=true` | Fixed — `allowBackup="false"` + `dataExtractionRules` + `fullBackupContent` |
| C7/C8 | Android/iOS plaintext image temp never cleaned | Fixed via `remove_plaintext_image_file` — but see Finding 1 (broke desktop) |
| C9 | `AppManager.shared` duplicate instance | Fixed — assigned in `init`, `preconditionFailure` guard |
| C10 | Missing BG task identifiers | Fixed — `BGTaskSchedulerPermittedIdentifiers` + `UIBackgroundModes: [processing]` in Info.plist |
| C11 | `fetch_url` SSRF + unbounded buffering | Partially — scheme/host/IP guard + 1 MiB stream cap added, but see Finding 2 (bypass) |
| R-1 | Dormant inference planner leaks into FFI bindings | Fixed — `uniffi` derives removed; regression test asserts types absent from Kotlin bindings |

---

## New findings (introduced by the fix commit `959d364`)

### Finding 1 — HIGH — Desktop: sending an attached image deletes the user's original file (data loss)

- **Location:** `rust/src/lib.rs:2059` (`clear_pending_image_attachment` → `remove_plaintext_image_file`),
  invoked on the send path at `rust/src/lib.rs:4103` (and error path `:4094`).
  Desktop attach flow: `desktop/iced/src/main.rs:1047-1058`.
- **Mechanism:** The cleanup was written to delete the *temporary* plaintext copies that Android
  (`cacheDir/img_*.jpg`) and iOS (`temporaryDirectory/gallery_*.jpg`) create. The desktop attach
  flow instead dispatches `AttachImage` with `path.canonicalize()` of the **user's own picked file** —
  no temp copy is made. The `AttachImage` handler stores that path directly in
  `pending_image_attachment.file_path` (no copy), and on send `clear_pending_image_attachment`
  calls `std::fs::remove_file` on it.
- **Failure scenario:** In the desktop app, attach `~/Pictures/vacation.jpg` and send the message →
  the request is built (image is read into a data URL first, so the send succeeds) → then
  `vacation.jpg` is deleted from disk. Every desktop image send silently destroys the source file.
- **Verdict:** CONFIRMED. `remove_plaintext_image_file` has no guard; desktop path is a real user file.
- **Fix options:**
  - Have the desktop attach flow copy the picked file into an app temp path before dispatching
    `AttachImage` (mirror mobile), so the deleted file is always an app-owned copy; or
  - Gate `remove_plaintext_image_file` to only delete paths under the app sandbox / temp directory,
    and carry a flag on `PendingImageAttachment` marking whether the source is app-owned.

### Finding 2 — MEDIUM — SSRF guard bypassed by IPv4-mapped IPv6 addresses

- **Location:** `rust/src/agent/tools.rs`, `is_blocked_fetch_ipv6`.
- **Mechanism:** The IPv6 branch checks loopback / ULA (`fc00::/7`) / link-local (`fe80::/10`) /
  multicast / unspecified, but never unwraps the IPv4-mapped range `::ffff:0:0/96`. A mapped address
  such as `::ffff:169.254.169.254` is none of those, so the guard returns `false` (allowed). On a
  dual-stack host the OS routes it to the underlying IPv4 address.
- **Confirmation:** standalone check — `::ffff:169.254.169.254`, `::ffff:127.0.0.1`, `::ffff:10.0.0.1`,
  and `::ffff:192.168.1.1` all return `blocked_by_guard=false`; all have `to_ipv4_mapped() = Some(...)`.
- **Failure scenario:** a prompt-injected tool call
  `fetch_url("http://[::ffff:169.254.169.254]/latest/meta-data/")` reaches the cloud metadata
  endpoint; `::ffff:127.0.0.1` / `::ffff:10.0.0.1` reach loopback and LAN — exactly what the guard
  exists to prevent. The existing `security_regressions` test only covers `[::1]`, so it misses this.
- **Verdict:** CONFIRMED (guard logic); reachability is standard dual-stack behavior.
- **Fix:** in `is_blocked_fetch_ipv6`, if `ip.to_ipv4_mapped()` is `Some(v4)`, delegate to
  `is_blocked_fetch_ipv4(v4)` (also consider `to_ipv4()` for `::a.b.c.d` compat addresses). Add the
  mapped forms to the regression test.

---

## Release-hygiene notes (decisions, not defects)

- **Internal planning material ships publicly.** `.planning/` (99 tracked files, ~6.7 MB of GSD phase
  docs, reviews, research) and `artifacts/` (CI screenshots + a 3.9 MB smoke-test `.mp4` + an 852 KB
  `logcat.txt`) are git-tracked and will appear in the public repo. None contains secrets, but decide
  whether the full internal development trail and binary test artifacts should be public. Glance at
  `artifacts/mobile-runs/mango/smoke/20260415-094016/logcat.txt` for device paths before publishing.
- **DNS-rebinding TOCTOU in `fetch_url`** (lower priority): `validate_fetch_url` resolves and checks
  the host, then `reqwest` re-resolves independently on connect. A hostname that resolves to a public
  IP at validation and a private IP at fetch bypasses the guard. Hard to fully close; acceptable to
  defer, but it coexists with Finding 2.

## Recommendation

Finding 1 is a hard blocker for a public build — silent data loss for every desktop user who sends an
image. Finding 2 should ship fixed given this is a privacy-positioned product where `fetch_url` runs
under model control. The hygiene notes are maintainer decisions to make before tagging the release.
