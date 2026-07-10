# Roadmap: Confidential App

## Milestones

- ✅ **v1.0 MVP** - Phases 1-10 (shipped 2026-03-27)
- ✅ **v1.1 Mobile Embeddings** - Phase 11 (shipped 2026-03-27)
- ✅ **v1.2 Hardening & Test Coverage** - Phases 12-19 (shipped 2026-03-29)
- ✅ **v2.0 Memory & Agents** - Phases 20-36 (shipped 2026-07-10)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-10) — SHIPPED 2026-03-27</summary>

- [x] **Phase 1: RMP Foundation** — Actor scaffold, UniFFI bindings, AppState skeleton, native shells
- [x] **Phase 2: Streaming LLM Client** — OpenAI-compatible streaming, backend config, error taxonomy
- [x] **Phase 3: Attestation Verification Core** — Intel TDX/DCAP, NVIDIA CC JWT, nonce binding, TTL cache
- [x] **Phase 4: Persistence Layer** — SQLite schemas, migration runner, platform keychain
- [x] **Phase 5: Chat UI + Conversation Management** — Streaming chat across all three platforms
- [x] **Phase 6: Backend Routing + Settings** — Failover chains, health tracking, settings screen
- [x] **Phase 7: Onboarding Wizard** — First-run wizard with live attestation demo
- [x] **Phase 8: Local On-Device RAG** — On-device embedding, HNSW index, context injection
- [x] **Phase 9: Agent System + Background Execution** — Persisted agents, tool use, background execution
- [x] **Phase 10: PPQ.AI Backend Integration** — AMD SEV-SNP attestation, private model filtering

</details>

<details>
<summary>✅ v1.1 Mobile Embeddings (Phase 11) — SHIPPED 2026-03-27</summary>

- [x] **Phase 11: Mobile ONNX Embedding Pipeline** — Real CoreML/XNNPACK embedding on iOS and Android

</details>

<details>
<summary>✅ v1.2 Hardening & Test Coverage (Phases 12-19) — SHIPPED 2026-03-29</summary>

- [x] **Phase 12: ORT Pin & Stability** — ONNX Runtime version pinning and build stability
- [x] **Phase 13: Panic Elimination** — Remove unwrap/expect from production paths
- [x] **Phase 14: HPKE Key Hygiene** — Key lifecycle and zeroization improvements
- [x] **Phase 15: Embedding Graceful Degradation** — Fallback when embedding model unavailable
- [x] **Phase 16: Rate Limiting & 429 Backoff** — Exponential backoff with provider retry hints
- [x] **Phase 17: Backend Capability Config** — Per-backend feature flags and capability negotiation
- [x] **Phase 18: TEE Runtime Configuration** — Dynamic TEE type configuration at runtime
- [x] **Phase 19: Test Coverage Gaps** — Fill critical test coverage gaps across core modules

</details>

<details>
<summary>✅ v2.0 Memory & Agents (Phases 10, 12, 19-36) — SHIPPED 2026-07-10</summary>

- [x] **Phase 10: PPQ.AI Backend Integration** — AMD SEV-SNP attestation, private E2EE transport, private model filtering (6 plans)
- [x] **Phase 12: ORT Stable Upgrade** — ONNX Runtime rc.9 → rc.11 pin (1 plan)
- [x] **Phase 19: Test Coverage Gaps** — Streaming cancellation, agent failure injection, attestation cache TTL (1 plan)
- [x] **Phase 20: Memory Core** — Automatic fact extraction, SQLite + usearch storage, background execution (2 plans)
- [x] **Phase 21: Memory Retrieval & Injection** — Semantic search over memories, injection into system prompts (1 plan)
- [x] **Phase 22: Agent Tools Expansion** — Brave Search, URL fetch, file ops, calculator in ReAct loop (2 plans)
- [x] **Phase 23: Memory Management UI + Agent UI** — Memory view/edit/delete screens, agent UI re-enabled (3 plans)
- [x] **Phase 24: Redesign Settings UX** — Grouped sections, Memories in Settings, Brave API key in Tools (3 plans)
- [x] **Phase 25: disable/enable making memories** — Toggle in Settings MEMORY section, persisted (2 plans)
- [x] **Phase 26: Settings Submenus** — Providers and Defaults as tappable sub-screens (3 plans)
- [x] **Phase 27: Add optional tool use to chat** — Per-conversation tools toggle, non-streaming tool detection (4 plans)
- [x] **Phase 28: Local Data Encryption & Authentication** — AES-256-GCM, biometric/PIN unlock, duress PIN wipe (8 plans)
- [x] **Phase 29: Wire VectorIndex DEK End-to-End** — DEK from auth handlers through ActorState to VectorIndex (1 plan)
- [x] **Phase 30: Milestone Verification & Requirements Sync** — Close MEM-03 orphan, sync REQUIREMENTS.md (1 plan)
- [x] **Phase 31: Multimodal image attachments** — Camera/gallery on mobile, file picker on desktop, base64 data URL (6 plans)
- [x] **Phase 32: Directory-based RAG ingestion** — Directory sources with periodic sync, glob exclusions, format extractors (9 plans)
- [x] **Phase 33: Integrate Venice.ai as TEE-attested provider** — TDX+NRAS attestation + ECDH E2EE channel (4 plans)
- [x] **Phase 34: Integrate Redpill as TEE-attested aggregator** — Three response shapes, TDX+NRAS verification (4 plans)
- [x] **Phase 34.1: Plumb shape/freshness through UniFFI + trust UI** — Close RED-09 + RED-11 actor-loop drop (7 plans)
- [x] **Phase 35: contextvm-sdk Nostr tool discovery** — Tool marketplace via Nostr, Settings → Tools Discover (10 plans)
- [x] **Phase 36: Cache contextvm tools + tap-for-detail** — Cache-first, search, Used N× badge, detail screen (4 plans)

**Total:** 21 phases, 82 plans — [Full archive](milestones/v2.0-ROADMAP.md)

</details>

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 10. PPQ.AI Backend Integration | v2.0 | 6/6 | Complete | 2026-03-27 |
| 12. ORT Stable Upgrade | v2.0 | 1/1 | Complete | 2026-03-27 |
| 19. Test Coverage Gaps | v2.0 | 1/1 | Complete | 2026-03-29 |
| 20. Memory Core | v2.0 | 2/2 | Complete | 2026-04-03 |
| 21. Memory Retrieval & Injection | v2.0 | 1/1 | Complete | 2026-04-04 |
| 22. Agent Tools Expansion | v2.0 | 2/2 | Complete | 2026-04-04 |
| 23. Memory Management UI + Agent UI | v2.0 | 3/3 | Complete | 2026-04-04 |
| 24. Redesign Settings UX | v2.0 | 3/3 | Complete | 2026-04-05 |
| 25. disable/enable making memories | v2.0 | 2/2 | Complete | 2026-04-05 |
| 26. Settings Submenus | v2.0 | 3/3 | Complete | 2026-04-05 |
| 27. Add optional tool use to chat | v2.0 | 4/4 | Complete | 2026-04-07 |
| 28. Local Data Encryption & Auth | v2.0 | 8/8 | Complete | 2026-04-09 |
| 29. Wire VectorIndex DEK End-to-End | v2.0 | 1/1 | Complete | 2026-04-09 |
| 30. Milestone Verification & Sync | v2.0 | 1/1 | Complete | 2026-04-20 |
| 31. Multimodal image attachments | v2.0 | 6/6 | Complete | 2026-04-19 |
| 32. Directory-based RAG ingestion | v2.0 | 9/9 | Complete | 2026-05-07 |
| 33. Venice.ai TEE-attested provider | v2.0 | 4/4 | Complete | 2026-04-26 |
| 34. Redpill TEE-attested aggregator | v2.0 | 4/4 | Complete | 2026-04-26 |
| 34.1. Plumb shape/freshness through UI | v2.0 | 7/7 | Complete | 2026-04-27 |
| 35. contextvm-sdk Nostr tool discovery | v2.0 | 10/10 | Complete | 2026-05-08 |
| 36. Cache contextvm tools + detail | v2.0 | 4/4 | Complete | 2026-05-08 |
