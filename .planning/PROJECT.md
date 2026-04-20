# Confidential App

## Shipped: v1.0 MVP (2026-03-27)

10 phases, 26 plans, 69 requirements complete. Full RMP app with streaming LLM, multi-TEE attestation (TDX/DCAP + SEV-SNP + NVIDIA CC), local RAG, autonomous agents, onboarding wizard, and PPQ.AI as second confidential backend. [Archive](.planning/milestones/v1.0-ROADMAP.md)

## Shipped: v1.1 Mobile Embeddings (2026-03-27)

1 phase, 2 plans. Real ONNX Runtime embedding pipeline on iOS (CoreML EP) and Android (XNNPACK EP), replacing zero-vector stubs with all-MiniLM-L6-v2 INT8 quantized model.

## Shipped: v1.2 Hardening & Test Coverage (2026-03-29)

8 phases, 10 plans. ORT pin, panic elimination, HPKE key hygiene, embedding graceful degradation, rate limiting with 429 backoff, backend capability config, TEE runtime configuration, test coverage gaps.

## Current Milestone: v2.0 Memory & Agents

**Goal:** Add persistent cross-conversation memory with automatic fact extraction, and expand the agent system with real-world tools (web search, URL fetching, file operations, calculator).

**Target features:**
- Automatic memory extraction from conversations (facts, preferences, entities)
- Local on-device knowledge graph (SQLite + usearch, privacy-preserving)
- Memory injection into new conversations via existing RAG context pipeline
- Brave Search tool for agents (web research)
- URL fetching tool (read/summarize web pages)
- File operations tool (create/edit/read files on device)
- Calculator/math tool (precise computation)
- Memory management UI (view, delete, edit extracted memories)

## What This Is

A multi-platform personal AI platform built with the RMP architecture (Rust core + native UIs via UniFFI). Users chat with LLMs, run autonomous agents, and ground conversations in their own documents via local RAG -- all routed through confidential computing backends with verified TEE attestation. The app targets iOS (SwiftUI), Android (Jetpack Compose), and Desktop (iced), with maximum shared logic in Rust and thin native UI layers.

## Core Value

Every inference request is provably confidential -- the user can verify via remote attestation that their data never leaves a Trusted Execution Environment, and all document/embedding storage stays local on-device.

## Requirements

### Validated

- [x] OpenAI-compatible API client in Rust core for all backends -- Validated in Phase 2: streaming-llm-client (async-openai with SSE streaming, error taxonomy, cancellation support)
- [x] Remote attestation verification (self-verify in Rust when possible, provider API fallback) -- Validated in Phase 3: attestation-verification-core (Intel TDX/DCAP via dcap-qvl, NVIDIA CC JWT, SQLite cache with TTL, nonce replay prevention, provider fallback)
- [x] Full persistence of chat history, agent sessions, and application state -- Validated in Phase 4: persistence-layer (rusqlite with bundled SQLite, WAL mode, migration runner with v1+v2, CRUD for backends/conversations/messages/agent sessions, KeychainProvider trait for API key storage, 62 tests)
- [x] Chat interface as primary interaction surface -- Validated in Phase 5: chat-ui-conversation-management (full chat UI on all 3 platforms, streaming markdown, conversation CRUD, system prompt, file attachment, attestation badges, 81 tests)
- [x] Visual attestation status shown to user per backend/conversation -- Validated in Phase 5: chat-ui-conversation-management (attestation badge widget on desktop/iOS/Android with 5 status states and detail overlay)
- [x] Backend routing: failover chains, health tracking, and user manual override -- Validated in Phase 6: backend-routing-settings (FailoverRouter with exponential backoff, per-conversation override, 101 tests)
- [x] Per-conversation backend selection and configuration -- Validated in Phase 6: backend-routing-settings (OverrideConversationBackend action, persisted via conversations.backend_id)
- [x] No telemetry guarantee enforced via cargo-deny ban list -- Validated in Phase 6: backend-routing-settings (deny.toml bans 11 analytics crates)
- [x] Guided onboarding wizard (pick backends, verify attestation, first chat) -- Validated in Phase 7: onboarding-wizard (4-step wizard with first-launch detection, live attestation demo, TEE education on iced/SwiftUI/Compose)
- [x] Local on-device embeddings for RAG -- Validated in Phase 8: local-on-device-rag (fastembed AllMiniLML6V2Q via ONNX Runtime, EmbeddingProvider trait for platform injection, DesktopEmbeddingProvider on desktop)
- [x] Local document ingestion and vector index storage (on-device only, maximum privacy) -- Validated in Phase 8: local-on-device-rag (PDF/txt/md ingestion, fixed-size chunker, usearch HNSW index serialized to disk, MIGRATION_V6 schema, document/chunk CRUD, per-conversation attachment, context injection on SendMessage)
- [x] Agent system with tool use, multi-turn autonomy, background execution, and document context -- Validated in Phase 9: agent-system-background-execution (ReAct loop with non-streaming function calling, 3 tools wired to RAG, per-step SQLite checkpointing, pause/resume/cancel, iOS BGProcessingTask, Android WorkManager, iced desktop exit checkpoint, notification tap routing, session list + detail UI on all 3 platforms)
- [x] Long-running agent sessions that persist across app restarts -- Validated in Phase 9: agent-system-background-execution (SQLite checkpoint per step, actor state restored from DB on LoadAgentSession, resume from last completed step)
- [x] AMD SEV-SNP attestation support for PPQ.AI backend -- Validated in Phase 10: ppq-ai-backend-integration (TeeType::AmdSevSnp enum variant, parse_tee_type, attestation_tinfoil_tdx dispatch, PPQ.AI preset, MIGRATION_V10 with 5 private/ model IDs, UniFFI bindings regenerated, all 3 platform UI labels/pickers updated, 170 tests green)

### Active

- [ ] Multi-platform RMP architecture (iOS, Android, Desktop) with shared Rust core
- [ ] Support all 9 confidential inference backends (Tinfoil, Redpill, Chutes, NEAR AI, Maple, Privatemode, NanoGPT, PPQ.AI, Venice.ai)
- [ ] OpenAI-compatible API client in Rust core for all backends
- [x] Agent system with tool use, multi-turn autonomy, background execution, and document context (see Validated)
- [x] Long-running agent sessions that persist across app restarts (see Validated)
- [x] Full persistence of chat history, agent sessions, and application state (see Validated)
- [x] Guided onboarding wizard (pick backends, verify attestation, first chat) (see Validated)

### Out of Scope

- Cloud sync of documents or embeddings -- local-only for v1 (privacy-first)
- Full local LLM inference -- v1 uses local embeddings only, generation via confidential backends
- CLI target -- v1 focuses on iOS, Android, Desktop
- Custom model fine-tuning or training
- Multi-user / shared workspaces
- Payment or subscription management for backend providers

## Context

**Architecture foundation:** The app follows the RMP Architecture Bible -- TEA/Elm unidirectional data flow, actor model (AppCore on a dedicated thread with flume channels), UniFFI for FFI bindings, and the capability bridge pattern for platform-specific features (camera, file picker, NPU access for embeddings).

**Confidential inference ecosystem (as of March 2026):** Nine providers offer OpenAI-compatible APIs with TEE-backed inference:

| Provider | TEE Approach | Notable |
|----------|-------------|---------|
| Tinfoil | Intel TDX + NVIDIA H100 CC | Intel DCAP attestation |
| Redpill | Phala GPU TEE | Intel DCAP + on-chain attestation |
| Chutes | AMD SEV-SNP + TDX | AMD SEV-SNP attestation |
| NEAR AI | Intel TDX + NVIDIA H200 TEE | On-chain attestation |
| Maple | AMD SEV-SNP | Attestation reporting |
| Privatemode | SEV-SNP + TDX | Cosmian VM attestation |
| NanoGPT | H100 CC | Intel/NVIDIA attestation + ECDSA per-request signatures |
| PPQ.AI | SEV-SNP | Hardware TEE attestation |
| Venice.ai | H100 CC | NVIDIA Confidential Computing attestation |

All providers expose OpenAI-compatible endpoints (chat completions, streaming). Attestation verification involves different chains depending on TEE type: AMD SEV-SNP report verification, Intel TDX/DCAP quote verification, and NVIDIA CC attestation.

**Local inference:** On-device embedding models (e.g., all-MiniLM, nomic-embed-text) run via ONNX Runtime or platform-native ML frameworks (Core ML on iOS, NNAPI on Android) for RAG vector computation. Full generative model support deferred to future versions.

**RMP reference:** The `rmp/` subdirectory contains the architecture bible, the `rmp` CLI scaffolding tool, and a working hello-chat example app. The CLI can scaffold the initial project structure with `rmp init`.

## Constraints

- **Architecture**: Must follow RMP Architecture Bible -- Rust core owns all business logic, native layers are thin UI + capability bridges only
- **Privacy**: All document storage and vector indices must remain on-device. No telemetry, no cloud sync in v1
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
*Last updated: 2026-04-20 — Phase 30 complete: v2.0 milestone documentation gap closed — Phase 21 VERIFICATION.md written (MEM-03 SATISFIED), REQUIREMENTS.md synced to 36/36 complete (ENC-02/ENC-09 traceability corrected). All v2.0 milestone requirements verified.*
