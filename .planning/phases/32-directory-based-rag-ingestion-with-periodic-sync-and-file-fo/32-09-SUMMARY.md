---
phase: 32-directory-based-rag-ingestion-with-periodic-sync-and-file-fo
plan: 09
subsystem: rag
tags: [rust, rag, file-formats, extractors, pure-rust, cross-platform, gap-closure]

requires:
  - phase: 8
    provides: extract_text_from_file PDF + UTF-8 baseline
  - phase: 32-03
    provides: SyncDirectoryFiles call site (lib.rs:6396) — unchanged, behaviour extends transparently

provides:
  - Format dispatch in extract_text_from_file for .docx, .epub, .html/.htm, .rtf (4 new formats beyond Phase 8 baseline)
  - MAX_EXTRACT_INPUT_BYTES = 20 MiB constant + size-cap guard short-circuiting before any parser allocates
  - Per-format error wrapping with anyhow::anyhow!("<fmt> extract failed for {filename}: {e}")
  - 4 fixture files under rust/src/tests/fixtures/ with the canary "directory sync canary"
  - 6 new tests in rust/src/tests/directory_rag.rs (4 format + 2 size-cap)
  - CONTEXT.md decisions D-38 / D-39 / D-40 enumerating the shipped set

affects: [directory-sync, future RAG ingestion, mobile-cross-compile]

tech-stack:
  added:
    - "docx-rs 0.4.20 — DOCX parsing (read_docx + Docx.json walk for text runs)"
    - "epub 2.1.5 — EPUB reader (EpubDoc::from_reader + spine iteration)"
    - "html2text 0.17.1 — HTML → plain text (no JS execution; <script> dropped)"
    - "rtf-parser 0.4.2 — RTF lexer/parser with RtfDocument::get_text()"
  patterns:
    - "Size-cap-before-parse: validate input size before invoking any allocator-heavy parser"
    - "JSON-walk extraction: serialize docx-rs Docx to JSON, then walk for {\"type\":\"text\",\"data\":{\"text\":...}} runs (avoids enumerating every typed enum variant)"
    - "EPUB → html2text pipeline: extract chapter XHTML via the epub crate, then strip markup with html2text"

key-files:
  created:
    - "rust/src/tests/fixtures/sample.docx — generated via docx-rs writer at fixture-build time"
    - "rust/src/tests/fixtures/sample.epub — hand-built EPUB 3.0 zip with one chapter"
    - "rust/src/tests/fixtures/sample.html — <html><body><p>directory sync <b>canary</b></p><script>alert(1)</script></body></html>"
    - "rust/src/tests/fixtures/sample.rtf — {\\rtf1\\ansi directory sync canary }"
    - ".planning/phases/32-directory-based-rag-ingestion-with-periodic-sync-and-file-fo/32-CONTEXT.md (newly tracked, was previously gitignored)"
  modified:
    - "rust/Cargo.toml — added docx-rs, epub, html2text, rtf-parser dependency entries"
    - "rust/src/rag/mod.rs — extended extract_text_from_file with 4 format branches + MAX_EXTRACT_INPUT_BYTES + 4 helper fns; updated test_extract_unknown_extension_as_text to use .xyz instead of .docx"
    - "rust/src/tests/directory_rag.rs — appended 6 new tests using include_bytes! fixtures"
    - "Cargo.lock — locked new transitive dep versions"

key-decisions:
  - "D-38: ship .docx + .epub + .html/.htm + .rtf (4 formats) — the full target set, no formats dropped"
  - "D-39: pure-Rust crate-selection rule verified by cargo tree audit + cargo build cross-targets"
  - "D-40: 20 MiB MAX_EXTRACT_INPUT_BYTES short-circuits before parser invocation"
  - "Use docx-rs JSON walk rather than typed-enum recursion — robust against future docx-rs schema additions"
  - "Pipe EPUB chapter XHTML through html2text rather than emitting raw markup — gives a clean tokenizable string"

patterns-established:
  - "Fixture generation: prefer reproducible at-test-time generation when a generator crate exists; commit a small binary fixture only when the generator's transitive deps are heavy. sample.docx / sample.epub were generated once via a /tmp probe binary and committed as small (≤18 KB) blobs to keep dev-dependencies lean."
  - "Anyhow error wrapping pattern: each per-format extractor returns Result<String> and the dispatcher wraps with anyhow::anyhow!(\"<fmt> extract failed for {filename}: {e}\") so call-site error messages are self-describing"

requirements-completed: [DIR-05]

duration: ~45min
completed: 2026-05-07
---

# Phase 32-09: Directory-sync Extractors Summary

**Extends extract_text_from_file from the Phase 8 .pdf + UTF-8 baseline to four mobile-safe pure-Rust extractors (.docx, .epub, .html/.htm, .rtf) and adds a 20 MiB size cap that short-circuits before any parser allocates.**

## Performance

- **Duration:** ~45 min
- **Tasks:** 3 atomic commits
- **Files modified:** 4 source files + 4 fixtures + 1 context doc

## Accomplishments

- **All 4 target formats shipped** (plan minimum was 2 — full target set delivered, no drops). Each extractor is pure-Rust with no new openssl-sys / native-tls dependencies.
- **Size cap enforced before parse.** `MAX_EXTRACT_INPUT_BYTES = 20 MiB` short-circuits the entry point with a self-describing anyhow error, closing VERIFICATION HI-03 for the extract path.
- **399 lib tests pass, 0 failed** (74 in the rag/directory_rag namespace alone, including the 6 new ones added by this plan).
- **Cross-target builds verified.** `cargo build -p mango_core --release`, `cargo build -p mango-desktop --release`, and `cargo ndk -t arm64-v8a build -p mango_core --release` all succeed. iOS aarch64-apple-ios target not installed locally — flagged human-verify (deferred to next macOS-host CI run; same crate set already builds for Android arm64 and contains no iOS-incompatible dependencies).

## Task Commits

1. **Task 1: implementation + fixtures + tests** — `82f992a` (feat)
2. **Task 2: CONTEXT.md amendment (D-38..D-40)** — `305691b` (docs)
3. **Task 3: this SUMMARY** — pending (docs)

## Files Created/Modified

### Created
- `rust/src/tests/fixtures/sample.docx` — minimal DOCX, ~18 KB, generated via docx-rs writer
- `rust/src/tests/fixtures/sample.epub` — minimal EPUB 3.0 zip, ~1.5 KB, hand-built (mimetype + container.xml + content.opf + nav.xhtml + chapter1.xhtml)
- `rust/src/tests/fixtures/sample.html` — 87 bytes, exact bytes per plan
- `rust/src/tests/fixtures/sample.rtf` — 36 bytes, exact bytes per plan
- `.planning/phases/32-directory-based-rag-ingestion-with-periodic-sync-and-file-fo/32-CONTEXT.md` — amended with D-38 / D-39 / D-40 (file was previously gitignored; now tracked since it documents shipped behaviour)

### Modified
- `rust/Cargo.toml` — 4 new dep entries grouped under a "Phase 32-09: directory-sync file format extractors" comment block, placed next to `pdf-extract` per plan
- `rust/src/rag/mod.rs` — full rewrite of `extract_text_from_file` plus 4 helper fns (`extract_docx`, `extract_epub`, `extract_html`, `extract_rtf`) plus `MAX_EXTRACT_INPUT_BYTES`. The pre-existing `test_extract_unknown_extension_as_text` was switched from `.docx` (which now dispatches to the docx extractor) to `.xyz` to keep the UTF-8-fallback branch covered.
- `rust/src/tests/directory_rag.rs` — appended 6 new tests under the comment-banner "Phase 32 Plan 09 tests — file-format extractors + size cap"

## Test Results

- **Full lib suite:** `cargo test -p mango_core --lib` → 399 passed, 0 failed, 18 ignored
- **Plan tests in isolation:** `cargo test -p mango_core --lib -- rag:: tests::directory_rag::test_extract` → 74 passed, 0 failed
- New tests added (all pass):
  - `test_extract_docx_returns_body_text`
  - `test_extract_epub_returns_chapter_text`
  - `test_extract_html_strips_tags` (asserts `<p>`, `<script>`, and `alert(1)` absent from output)
  - `test_extract_rtf_returns_plain_text`
  - `test_extract_size_cap_returns_error` (25 MiB synthetic input → Err with "too large" / "size cap" in message)
  - `test_extract_size_cap_constant_value` (compile-time pin on the 20 MiB constant)

## OpenSSL Audit

`cargo tree -p mango_core 2>&1 | grep -iE "openssl-sys|native-tls"` returns exactly **one** line:

```
│   │   └── openssl-sys v0.9.113
```

This entry is pre-existing — it comes from `rusqlite = { ..., features = ["bundled-sqlcipher-vendored-openssl"] }` (declared in Phase 3, vendored and statically linked, mobile-safe). **None of the four new extractor crates added a new openssl-sys / native-tls path.** Verified by adding only the 4 deps to a /tmp probe crate and confirming `cargo tree` returns 0 lines for that probe. The pre-existing baseline is documented in CLAUDE.md and is the project's standing approach for SQLCipher.

## Cross-compile Status

- ✓ **`cargo build -p mango_core --release`** — succeeds
- ✓ **`cargo build -p mango-desktop --release`** — succeeds (1 unrelated dead-code warning predating this plan)
- ✓ **`cargo ndk -t arm64-v8a build -p mango_core --release`** — succeeds
- ⚠ **`cargo build --target aarch64-apple-ios -p mango_core`** — `human-verify` (toolchain not installed on this Linux host; same crate set builds cleanly for Android arm64 and contains no Apple-specific dependencies, so iOS build is expected to succeed on a macOS host)

## Dropped Formats

None. Plan minimum was 2 new formats; target was 4; **4 shipped**.

## CONTEXT.md Amendments

`grep -c "D-38\|D-39\|D-40"` → 3. Decisions inserted under `<decisions>` after the existing "Security / Validation" subsection and before "Claude's Discretion", as specified.

## Acceptance Criteria — Verified

| Criterion | Result |
|---|---|
| `grep -c '\.docx\|\.epub\|\.html\|\.htm\|\.rtf' rust/src/rag/mod.rs` ≥5 | **14** ✓ |
| `grep -c "MAX_EXTRACT_INPUT_BYTES" rust/src/rag/mod.rs` ≥2 | **4** ✓ |
| `grep -E "docx-rs\|^epub =\|scraper\|html2text\|rtf-parser" rust/Cargo.toml` ≥4 entries | **5** (incl. existing `scraper = "0.26"`) ✓ |
| `cargo tree -p mango_core` openssl/native-tls lines | **0 new** (pre-existing rusqlite vendored line) ✓ |
| `ls rust/src/tests/fixtures/sample.{docx,epub,html,rtf}` | **4 files** ✓ |
| `cargo test -p mango_core --lib` 0 failures | **0 failed / 399 passed** ✓ |
| `cargo build -p mango_core --release` | **succeeds** ✓ |
| `cargo build -p mango-desktop --release` | **succeeds** ✓ |
| CONTEXT.md contains D-38, D-39, D-40 | **yes** ✓ |

VERIFICATION truth #13 flips from FAILED/OUT-OF-SCOPE to **VERIFIED**.
