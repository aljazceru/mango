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

**Coverage:**
- v2.0 requirements: 14 total
- Mapped to phases: 14
- Unmapped: 0

---
*Requirements defined: 2026-04-02*
*Last updated: 2026-04-02 after roadmap creation*
