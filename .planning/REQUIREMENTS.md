# Requirements: Confidential App

**Defined:** 2026-04-02
**Core Value:** Every inference request is provably confidential -- verified via remote attestation, all data stays local

## v2.0 Requirements

Requirements for v2.0 Memory & Agents milestone. Each maps to roadmap phases.

### Memory

- [x] **MEM-01**: App automatically extracts facts, preferences, and entities from completed conversations
- [x] **MEM-02**: Extracted memories are stored locally in SQLite with vector embeddings in usearch index
- [x] **MEM-03**: Relevant memories are injected into new conversation system prompts via semantic search
- [x] **MEM-04**: User can view all stored memories in a dedicated memory management screen
- [x] **MEM-05**: User can delete individual memories
- [x] **MEM-06**: User can edit extracted memories to correct or refine them
- [x] **MEM-07**: Memory extraction runs in background without blocking chat flow

### Agent Tools

- [x] **TOOL-01**: Agent can search the web using Brave Search API and return results
- [x] **TOOL-02**: Agent can fetch and read content from URLs (HTML parsed to text)
- [x] **TOOL-03**: Agent can create, read, and edit files in the app sandbox
- [x] **TOOL-04**: Agent can evaluate mathematical expressions with precision
- [x] **TOOL-05**: Agent tool dispatch integrates with existing ReAct loop and step checkpointing

### Agent UI

- [x] **AUI-01**: Agent UI is re-enabled on all platforms with the expanded tool set visible
- [x] **AUI-02**: Agent tool usage is displayed step-by-step in the session detail view (tool name, input, output)

### Chat Tool Use

- [x] **CHAT-TOOL-01**: Migration V16 adds tools_enabled column to conversations table with DEFAULT 0
- [x] **CHAT-TOOL-02**: User can toggle tool use on/off per conversation, persisted across app restarts
- [x] **CHAT-TOOL-03**: Chat tool subset excludes finish tool, conditionally excludes web_search (no Brave key) and doc tools (no attached docs)
- [x] **CHAT-TOOL-04**: When tools enabled, chat uses non-streaming first round to detect tool calls via run_agent_step_for_backend
- [x] **CHAT-TOOL-05**: Tool dispatch runs on actor thread (not inside Tokio task) to avoid runtime.block_on panic
- [x] **CHAT-TOOL-06**: After tool dispatch, streaming follow-up includes full message history with tool results
- [x] **CHAT-TOOL-07**: Tools toggle is visible in chat toolbar on iOS, Android, and Desktop
- [x] **CHAT-TOOL-08**: Tool messages (assistant tool_calls + tool results) never appear in AppState.messages UI bubble list

### Local Data Encryption & Authentication

- [x] **ENC-01**: SQLCipher replaces vanilla SQLite as the bundled encryption engine (bundled-sqlcipher feature)
- [x] **ENC-02**: Usearch vector index files and cached documents encrypted with AES-256-GCM using DEK
- [x] **ENC-03**: AES-256-GCM file encryption uses MGO1 magic header, random nonce, and authenticated tag
- [x] **ENC-04**: 256-bit random DEK generated on first launch and stored in platform keychain
- [x] **ENC-05**: PIN/password fallback derives KEK via Argon2id (64MiB, 3 iterations, parallelism 1) to wrap/unwrap DEK
- [x] **ENC-06**: Bootstrap DB (mango_auth.db) stores salt, wrapped DEK, duress PIN hash, and KDF params
- [x] **ENC-07**: Screen::Locked gates all app content; app starts locked on cold launch
- [x] **ENC-08**: App locks after configurable timeout (default 5 min) when returning from background
- [x] **ENC-09**: Biometric unlock (Face ID/Touch ID on iOS, BiometricPrompt Class 3 on Android) with PIN fallback
- [x] **ENC-10**: Duress PIN triggers immediate full data wipe (DB, files, keychain) and resets to onboarding
- [x] **ENC-11**: First-time mandatory PIN setup after onboarding with optional duress PIN and biometric enrollment
- [x] **ENC-12**: Existing unencrypted databases migrated to SQLCipher via sqlcipher_export on first encrypted open
- [x] **ENC-13**: Lock timeout configurable in Settings: Immediately, 1 min, 5 min (default), 15 min, Never
- [x] **ENC-14**: All three platforms (iOS, Android, Desktop) support PIN/password as minimum auth; biometrics additive

### Venice.ai TEE-Attested Provider

- [ ] **VEN-01**: Add Venice.ai as a known provider preset on the Add Backend form
- [ ] **VEN-02**: Fetch attestation from `GET /api/v1/tee/attestation?model=&nonce=` (public, no API key)
- [ ] **VEN-03**: Verify Intel TDX quote signature, PCK chain, TCB, and CRLs client-side via `dcap-qvl`
- [ ] **VEN-04**: Decode REPORTDATA with Venice layout (20B keccak-addr / 12B zeros / 32B nonce) and verify all three bindings
- [ ] **VEN-05**: POST `nvidia_payload` to NRAS, verify returned JWT via existing `attestation/nvidia.rs`
- [ ] **VEN-06**: Reject TDX quote in debug mode (`td_attributes[0] & 0x01 != 0`)
- [ ] **VEN-07**: Establish ECDH(secp256k1) + HKDF-SHA256(`"ecdsa_encryption"`) + AES-256-GCM E2EE channel using attested signing key
- [ ] **VEN-08**: Send and stream chat completions over the E2EE channel with `enable_e2ee: true` and three `X-Venice-TEE-*` headers
- [ ] **VEN-09**: Display Venice as a backend in Settings → Providers with `Verified` attestation badge once attestation passes

## Future Requirements

### Memory Enhancements

- **MEM-F01**: Memory extraction from images and voice transcripts
- **MEM-F02**: Memory categories and tagging system
- **MEM-F03**: Memory importance ranking and decay

### Agent Enhancements

- **TOOL-F01**: Code execution in sandboxed environment
- **TOOL-F02**: MCP protocol integration for third-party tools
- **TOOL-F03**: Multi-agent collaboration and delegation

## Out of Scope

| Feature | Reason |
|---------|--------|
| Cloud-synced memories | Local-only for privacy (core value) |
| Agent code execution sandbox | Security complexity too high for v2.0 |
| Voice/image memory extraction | Text-only for v2.0 |
| Third-party MCP tool integration | Custom tool dispatch is simpler for now |
| Brave Search API key management UI | Use settings/environment for now |
| Multi-round tool calls in chat | Single tool round for Phase 27; multi-round is the agent system's job |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| MEM-01 | Phase 20 | Done |
| MEM-02 | Phase 20 | Done |
| MEM-07 | Phase 20 | Done |
| MEM-03 | Phase 21 | Complete |
| TOOL-01 | Phase 22 | Complete |
| TOOL-02 | Phase 22 | Complete |
| TOOL-03 | Phase 22 | Complete |
| TOOL-04 | Phase 22 | Complete |
| TOOL-05 | Phase 22 | Complete |
| MEM-04 | Phase 23 | Complete |
| MEM-05 | Phase 23 | Complete |
| MEM-06 | Phase 23 | Complete |
| AUI-01 | Phase 23 | Complete |
| AUI-02 | Phase 23 | Complete |
| CHAT-TOOL-01 | Phase 27 | Complete |
| CHAT-TOOL-02 | Phase 27 | Complete |
| CHAT-TOOL-03 | Phase 27 | Complete |
| CHAT-TOOL-04 | Phase 27 | Complete |
| CHAT-TOOL-05 | Phase 27 | Complete |
| CHAT-TOOL-06 | Phase 27 | Complete |
| CHAT-TOOL-07 | Phase 27 | Complete |
| CHAT-TOOL-08 | Phase 27 | Complete |
| ENC-01 | Phase 28 | Complete |
| ENC-02 | Phase 29 | Complete |
| ENC-03 | Phase 28 | Complete |
| ENC-04 | Phase 28 | Complete |
| ENC-05 | Phase 28 | Complete |
| ENC-06 | Phase 28 | Complete |
| ENC-07 | Phase 28 | Complete |
| ENC-08 | Phase 28 | Complete |
| ENC-09 | Phase 28 | Complete |
| ENC-10 | Phase 28 | Complete |
| ENC-11 | Phase 28 | Complete |
| ENC-12 | Phase 28 | Complete |
| ENC-13 | Phase 28 | Complete |
| ENC-14 | Phase 28 | Complete |
| VEN-01 | Phase 33 | Pending |
| VEN-02 | Phase 33 | Pending |
| VEN-03 | Phase 33 | Pending |
| VEN-04 | Phase 33 | Pending |
| VEN-05 | Phase 33 | Pending |
| VEN-06 | Phase 33 | Pending |
| VEN-07 | Phase 33 | Pending |
| VEN-08 | Phase 33 | Pending |
| VEN-09 | Phase 33 | Pending |

**Coverage:**
- v2.0 + Phase 33 requirements: 45 total
- Mapped to phases: 45
- Unmapped: 0
- Complete: 36
- Pending (Phase 33): 9

---
*Requirements defined: 2026-04-02*
*Last updated: 2026-04-19 — ENC-02 and ENC-09 synced to Complete after Phase 29 delivery confirmation*
