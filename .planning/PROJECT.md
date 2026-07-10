# Confidential App

## Shipped: v1.0 MVP (2026-03-27)

10 phases, 26 plans, 69 requirements complete. Full RMP app with streaming LLM, multi-TEE attestation (TDX/DCAP + SEV-SNP + NVIDIA CC), local RAG, autonomous agents, onboarding wizard, and PPQ.AI as second confidential backend. [Archive](.planning/milestones/v1.0-ROADMAP.md)

## Shipped: v1.1 Mobile Embeddings (2026-03-27)

1 phase, 2 plans. Real ONNX Runtime embedding pipeline on iOS (CoreML EP) and Android (XNNPACK EP), replacing zero-vector stubs with all-MiniLM-L6-v2 INT8 quantized model.

## Shipped: v1.2 Hardening & Test Coverage (2026-03-29)

8 phases, 10 plans. ORT pin, panic elimination, HPKE key hygiene, embedding graceful degradation, rate limiting with 429 backoff, backend capability config, TEE runtime configuration, test coverage gaps.

## Shipped: v2.0 Memory & Agents (2026-07-10)

21 phases, 82 plans, 79 requirements complete. Persistent cross-conversation memory with automatic fact extraction and injection. Agent tools expansion (web search, URL fetch, file ops, calculator) with per-conversation tool toggle in chat. Local data encryption at rest (SQLCipher + AES-256-GCM) with biometric/PIN authentication and duress PIN wipe. Multimodal image attachments on all platforms. Directory-based RAG ingestion with periodic sync and file format extractors. Venice.ai and Redpill as TEE-attested LLM providers with E2EE. Nostr-based tool discovery via contextvm-sdk with cache-first UI. [Archive](.planning/milestones/v2.0-ROADMAP.md)

## What This Is

A multi-platform personal AI platform built with the RMP architecture (Rust core + native UIs via UniFFI). Users chat with LLMs, run autonomous agents, and ground conversations in their own documents via local RAG -- all routed through confidential computing backends with verified TEE attestation. The app targets iOS (SwiftUI), Android (Jetpack Compose), and Desktop (iced), with maximum shared logic in Rust and thin native UI layers. In v2.0, the app gained persistent memory, expanded agent tools, local data encryption, multimodal image support, directory-based RAG, two new TEE providers, and Nostr-based tool discovery.

## Core Value

Every inference request is provably confidential -- the user can verify via remote attestation that their data never leaves a Trusted Execution Environment, and all document/embedding storage stays local on-device.

## Requirements

### Validated

- [x] OpenAI-compatible API client in Rust core for all backends -- v1.0 (Phase 2)
- [x] Remote attestation verification (self-verify in Rust when possible, provider API fallback) -- v1.0 (Phase 3)
- [x] Full persistence of chat history, agent sessions, and application state -- v1.0 (Phase 4)
- [x] Chat interface as primary interaction surface -- v1.0 (Phase 5)
- [x] Visual attestation status shown to user per backend/conversation -- v1.0 (Phase 5)
- [x] Backend routing: failover chains, health tracking, and user manual override -- v1.0 (Phase 6)
- [x] Per-conversation backend selection and configuration -- v1.0 (Phase 6)
- [x] No telemetry guarantee enforced via cargo-deny ban list -- v1.0 (Phase 6)
- [x] Guided onboarding wizard (pick backends, verify attestation, first chat) -- v1.0 (Phase 7)
- [x] Local on-device embeddings for RAG -- v1.0 (Phase 8)
- [x] Local document ingestion and vector index storage (on-device only, maximum privacy) -- v1.0 (Phase 8)
- [x] Agent system with tool use, multi-turn autonomy, background execution, and document context -- v1.0 (Phase 9)
- [x] Long-running agent sessions that persist across app restarts -- v1.0 (Phase 9)
- [x] AMD SEV-SNP attestation support for PPQ.AI backend -- v1.0 (Phase 10)
- [x] Automatic memory extraction from conversations (facts, preferences, entities) -- v2.0 (Phase 20)
- [x] Memory injection into new conversations via semantic search -- v2.0 (Phase 21)
- [x] Memory management UI (view, delete, edit) -- v2.0 (Phase 23)
- [x] Agent tools: web search, URL fetch, file ops, calculator -- v2.0 (Phase 22)
- [x] Per-conversation tool use toggle in chat -- v2.0 (Phase 27)
- [x] Local data encryption at rest (SQLCipher + AES-256-GCM) with biometric/PIN auth -- v2.0 (Phases 28-29)
- [x] Duress PIN triggers full data wipe -- v2.0 (Phase 28)
- [x] Multimodal image attachments on all platforms -- v2.0 (Phase 31)
- [x] Directory-based RAG ingestion with periodic sync -- v2.0 (Phase 32)
- [x] Venice.ai as TEE-attested provider with E2EE -- v2.0 (Phase 33)
- [x] Redpill as TEE-attested aggregator with three response shapes -- v2.0 (Phases 34-34.1)
- [x] Nostr-based tool discovery via contextvm-sdk -- v2.0 (Phases 35-36)

### Active

- [ ] Support all 9 confidential inference backends (Tinfoil, Redpill, Chutes, NEAR AI, Maple, Privatemode, NanoGPT, PPQ.AI, Venice.ai)
- [ ] iOS UI for contextvm tool discovery (bindings ship, UI deferred)

### Out of Scope

- Cloud sync of documents or embeddings -- local-only for privacy (core value)
- Full local LLM inference -- v1/v2 uses local embeddings only, generation via confidential backends
- CLI target -- focus on iOS, Android, Desktop
- Custom model fine-tuning or training
- Multi-user / shared workspaces
- Payment or subscription management for backend providers

## Context

**Architecture foundation:** The app follows the RMP Architecture Bible -- TEA/Elm unidirectional data flow, actor model (AppCore on a dedicated thread with flume channels), UniFFI for FFI bindings, and the capability bridge pattern for platform-specific features (camera, file picker, NPU access for embeddings).

**Current codebase state (v2.0):**
- 445+ Rust unit tests passing (20 ignored for live integration)
- 5 TEE-attested providers integrated: Tinfoil, PPQ.AI, Venice.ai, Redpill + contextvm tool discovery
- SQLite migrations through V20 (including SQLCipher encryption, memory, directory sources, contextvm tools)
- Cross-platform: iOS 17+ (SwiftUI), Android API 28+ (Jetpack Compose), Desktop (iced)
- 105K+ LOC added in v2.0 across 346 commits

**Confidential inference ecosystem (as of July 2026):** Nine providers offer OpenAI-compatible APIs with TEE-backed inference:

| Provider | TEE Approach | Status |
|----------|-------------|--------|
| Tinfoil | Intel TDX + NVIDIA H100 CC | Integrated (v1.0) |
| PPQ.AI | AMD SEV-SNP | Integrated (v1.0/v2.0) |
| Venice.ai | H100 CC (TDX + NRAS) | Integrated (v2.0) |
| Redpill | Phala GPU TEE (TDX + NRAS) | Integrated (v2.0) |
| Chutes | AMD SEV-SNP + TDX | Not directly integrated |
| NEAR AI | Intel TDX + NVIDIA H200 TEE | Not integrated |
| Maple | AMD SEV-SNP | Not integrated |
| Privatemode | SEV-SNP + TDX | Not integrated |
| NanoGPT | H100 CC | Not integrated |

**Local inference:** On-device embedding models (all-MiniLM-L6-v2 INT8 quantized) run via ONNX Runtime with CoreML EP (iOS), XNNPACK EP (Android), and CPU (Desktop) for RAG vector computation. Full generative model support deferred to future versions.

## Constraints

- **Architecture**: Must follow RMP Architecture Bible -- Rust core owns all business logic, native layers are thin UI + capability bridges only
- **Privacy**: All document storage and vector indices must remain on-device. No telemetry, no cloud sync. All local data encrypted at rest (v2.0+)
- **API compatibility**: All backend integrations must use OpenAI-compatible chat completions API to minimize per-provider code
- **Attestation**: Must support at least AMD SEV-SNP and Intel TDX attestation report verification in Rust
- **Platforms**: iOS 17+, Android API 28+, macOS 13+ / Linux (iced)
- **Build system**: Nix flake for reproducible builds, `just` for task running, UniFFI for bindings generation

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| RMP architecture (Rust + UniFFI + native UIs) | Maximum code sharing, native UX quality, Rust safety for crypto/attestation | Validated Phase 1 |
| OpenAI-compatible API as universal backend interface | All 9 providers already expose this; minimizes integration surface | Validated Phase 2 |
| Local-only document storage for v1 | Privacy-first aligns with confidential computing mission | Validated Phase 8 |
| On-device embeddings, remote generation | Balances privacy (embeddings never leave device) with capability (large models need TEE backends) | Validated Phase 8 |
| Self-verify attestation in Rust | Sovereign verification -- don't trust provider claims without checking the cryptographic proof | Validated Phase 3 |
| Memory reuses EmbeddingProvider trait + usearch HNSW index | No parallel infrastructure; memories share the same vector index as RAG chunks | Validated Phase 20 |
| SQLCipher for database encryption | Bundled-sqlcipher compiles into binary, zero system deps on mobile; same rusqlite API | Validated Phase 28 |
| ECDH(secp256k1) + HKDF + AES-256-GCM for Venice E2EE | Matches Venice protocol; per-request encryption rooted in attested signing key | Validated Phase 33 |
| Reuse Venice REPORTDATA decoder for Redpill model component | Single source of truth; Redpill Shape A is Venice-identical | Validated Phase 34 |
| contextvm-sdk for Nostr-based tool discovery | Extends existing tool dispatch without parallel subsystem; pure-Rust, no OpenSSL | Validated Phase 35 |
| Wave 0 TDD pattern (RED test stubs before implementation) | Contracts locked before code; prevents scope drift during multi-plan phases | Validated Phases 27, 31, 34, 35, 36 |
| Golden fixture testing for attestation providers | Captures from real provider APIs as test fixtures; tests run without network | Validated Phases 33, 34 |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? -> Move to Out of Scope with reason
2. Requirements validated? -> Move to Validated with phase reference
3. New requirements emerged? -> Add to Active
4. Decisions to log? -> Add to Key Decisions
5. "What This Is" still accurate? -> Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check -- still the right priority?
3. Audit Out of Scope -- reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-07-10 after v2.0 milestone*
