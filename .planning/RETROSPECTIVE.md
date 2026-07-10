# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v2.0 — Memory & Agents

**Shipped:** 2026-07-10
**Phases:** 21 | **Plans:** 82 | **Timeline:** 35 days (2026-04-03 → 2026-05-08)

### What Was Built

- **Persistent cross-conversation memory** — automatic LLM-driven fact extraction from completed conversations, stored in SQLite + usearch, semantically retrieved and injected into new conversation system prompts (Phases 20-21)
- **Agent tools expansion + chat tool use** — Brave Search, URL fetch, file operations, and calculator integrated into the ReAct loop; per-conversation tool toggle in chat with non-streaming first round for tool detection (Phases 22, 27)
- **Local data encryption & authentication** — SQLCipher database encryption, AES-256-GCM file/vector encryption with DEK, biometric/PIN unlock, duress PIN wipe, configurable lock timeout across all 3 platforms (Phases 28-29)
- **Multimodal image attachments** — camera/gallery on Android, camera/photo library on iOS, file picker on Desktop; images resized, JPEG-compressed, and sent as base64 data URLs to vision-capable LLMs (Phase 31)
- **Directory-based RAG ingestion** — whole-directory sources (e.g. Obsidian vault) with glob exclusions, periodic incremental sync via mtime+size fingerprints, cross-platform folder permissions (iOS bookmarks, Android SAF, Desktop notify watcher), .docx/.epub/.html/.rtf extractors (Phase 32)
- **Venice.ai + Redpill TEE-attested providers** — TDX+NRAS attestation verification, ECDH(secp256k1)+HKDF+AES-256-GCM E2EE channel for Venice, three Redpill response shapes (Phala-flat, Phala-orchestrated 3-quote, Chutes anti-tamper) with full attestation plumbing through native trust UI (Phases 33-34.1)
- **Nostr-based tool discovery (contextvm)** — tool marketplace via contextvm-sdk: Settings → Tools "Discover tools" with per-tool enable toggles, auto-discover checkbox, cache-first UI hydration, live search, "Used N×" badges, tap-for-detail screen with npub bech32 metadata (Phases 35-36)

### What Worked

- **Wave 0 TDD pattern** — writing RED test stubs (`#[ignore]`-gated) before implementation locked contracts early and prevented scope drift. Used successfully in Phases 27, 31, 34, 35, 36. Should be the default for any phase with >3 plans.
- **Golden fixture testing** — capturing real attestation API responses as JSON fixtures (Phases 33, 34) allowed comprehensive testing without network dependencies. Tests run in CI/sandbox and are deterministic.
- **Spike-then-implement for providers** — researching Venice/Redpill protocols via dedicated spike experiments before planning produced accurate phase contexts and caught protocol mismatches early.
- **Reusing existing infrastructure** — memory system reused the EmbeddingProvider trait + usearch HNSW index from Phase 8 RAG; chat tools reused agent tool dispatch from Phase 22; contextvm tools reused OpenAI-compatible `tools` array. No parallel subsystems were created.
- **Decimal phase insertion (34.1)** — when RED-09/RED-11 gaps were found post-verification, inserting Phase 34.1 as a decimal phase cleanly closed the gap without renumbering or disrupting the roadmap.

### What Was Inefficient

- **REQUIREMENTS.md chronic staleness** — requirements were not synced incrementally after phase completion. At milestone completion, 11 RED-* checkboxes were unchecked, 23 requirements (IMG/DIR/CTX/CTX36) were never registered, and the traceability table was stale. This required a large manual sync during archival.
- **Milestone audit only covered 9/21 phases** — the v2.0 audit (2026-04-09) was run after Phase 28 but the milestone grew to 36 phases. The audit became stale and its gaps (MEM-03, ENC-02, ENC-09) were closed by Phases 29-30 without re-auditing.
- **Phase 34 RED-09/RED-11 actor-loop drop** — the executor marked these GREEN based on data presence in AttestationEvent structs, but the roadmap success criteria explicitly required "display in the trust UI." The data was dropped at the actor-loop boundary (`freshness: _`, `orchestrated_components: _`). This required a full Phase 34.1 (7 plans) to fix.
- **UniFFI bindings regeneration as a frequent friction point** — multiple phases experienced issues with stale bindings, `strip = true` hiding UniFFI metadata symbols, and worktree-behind-main merge issues during binding regeneration.
- **41 verification debt items at completion** — 41 human_needed items (live API tests, UI tests) accumulated across Phases 31, 32, 34, 36. No mechanism existed to incrementally resolve these during development.

### Patterns Established

- **Cache-first hydration** — for contextvm tool discovery, cached data renders instantly on screen open before background refresh. Pattern applicable to any network-backed UI list.
- **Split-row click mitigation** — when a whole-row click target coexists with a toggler (Switch/checkbox), the toggler absorbs its own pointer event so row taps only navigate. Used in Android Compose (Phase 36) and Desktop iced.
- **UniFFI struct-variant promotion** — promoting `AttestationStatus::Verified` from unit to struct variant carrying Option fields (shape/freshness/orchestrated_components) is the pattern for extending FFI types without breaking existing consumers.
- **Relative-time labels pre-computed in Rust** — `relative_time_label` in Rust core with pre-computed `last_used_label`/`last_seen_label` fields avoids native-side time computation and keeps label logic in one place.
- **Inline copy-confirmation on Desktop** — iced 0.13 has no native Snackbar; inline status line with `Task::perform(tokio::time::sleep, ...)` timed-clear is the desktop pattern for ephemeral feedback.

### Key Lessons

1. **Sync REQUIREMENTS.md after every phase** — not just at milestone completion. Each phase's SUMMARY should trigger a checkbox + traceability update. The 23 unregistered requirements (IMG/DIR/CTX/CTX36) would have been caught incrementally.
2. **Run milestone audit in batches** — if the milestone grows beyond the audited scope, re-audit. The v2.0 audit covered 9 phases but the milestone shipped 21. A mid-milestone re-audit after Phase 32 would have caught the RED-09/RED-11 gap earlier.
3. **Verify against roadmap success criteria, not just code presence** — Phase 34's executor marked RED-09/RED-11 GREEN because the data existed in Rust structs, but the roadmap said "display in the trust UI." Goal-backward verification means checking the actual user-facing outcome.
4. **Plan UniFFI binding regeneration as an explicit wave** — not an afterthought. Multiple phases lost time to stale bindings, strip=true symbol hiding, and worktree merge issues. A dedicated "Wave N: bindings regen + compile sweep" plan should be standard for any phase touching UniFFI types.
5. **Resolution human_needed items incrementally** — batch them at the end of each phase, not at milestone completion. 41 pending items at archival is too many to triage efficiently.

### Cost Observations

- Model mix: ~30% opus (planning), ~70% sonnet (execution)
- Sessions: multiple across 35 days
- Notable: Wave 0 TDD pattern (RED stubs) added ~10-15% upfront cost but eliminated rework — net positive for phases with >3 plans

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Sessions | Phases | Key Change |
|-----------|----------|--------|------------|
| v1.0 | — | 10 | Initial RMP scaffold, streaming, attestation, RAG, agents |
| v1.1 | — | 1 | Mobile embedding pipeline (CoreML/XNNPACK) |
| v1.2 | — | 8 | Hardening pass — panics, key hygiene, rate limiting, test coverage |
| v2.0 | — | 21 | Memory, tools, encryption, multimodal, providers, tool discovery |

### Cumulative Quality

| Milestone | Tests | Coverage | Zero-Dep Additions |
|-----------|-------|----------|-------------------|
| v1.0 | ~170 | — | — |
| v1.2 | ~200 | — | — |
| v2.0 | 445+ | — | contextvm-sdk, nostr, docx-rs, epub, scraper, evalexpr, image, ignore |

### Top Lessons (Verified Across Milestones)

1. Reuse existing infrastructure (EmbeddingProvider, usearch, tool dispatch) rather than creating parallel subsystems — validated across memory (Phase 20), chat tools (Phase 27), and contextvm (Phase 35).
2. Wave 0 TDD pattern (RED stubs before implementation) prevents scope drift and locks contracts — validated across Phases 27, 31, 34, 35, 36.
3. Golden fixture testing for external API integrations enables deterministic CI — validated across Venice (Phase 33) and Redpill (Phase 34).
