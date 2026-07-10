# Phase 19: Test Coverage Gaps - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-29
**Phase:** 19-test-coverage-gaps
**Areas discussed:** Test organization, Mock strategy, Agent loop testing
**Mode:** Auto (all decisions auto-selected)

---

## Test Organization

| Option | Description | Selected |
|--------|-------------|----------|
| Extend existing files | Add tests to streaming.rs, agent.rs, attestation_cache.rs | ✓ |
| New dedicated files | Create 19-specific test files (e.g., test_streaming_cancel.rs) | |

**User's choice:** [auto] Extend existing files (recommended default)
**Notes:** Keeps tests co-located with related test infrastructure (make_app, make_record helpers). Consistent with project pattern.

---

## Mock Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Tokio drop semantics | Drop receiver mid-stream, assert clean shutdown | ✓ |
| Mock HTTP server | Spin up local HTTP server that sends partial SSE then hangs | |
| Live backend with timeout | Use real Tinfoil/PPQ with forced network partition | |

**User's choice:** [auto] Tokio drop semantics (recommended default)
**Notes:** Avoids external dependencies. Tests the actual cancellation path through the encrypted transport layer.

---

## Agent Loop Testing

| Option | Description | Selected |
|--------|-------------|----------|
| Mock LLM via crafted data | Use in-memory DB + AgentStepRow to simulate failure states | ✓ |
| Mock HTTP client | Inject mock reqwest client that returns errors | |
| Integration with test server | Full agent loop against local mock server | |

**User's choice:** [auto] Mock LLM via crafted data (recommended default)
**Notes:** Tests the persistence and state management layer directly. Simpler and more reliable than mocking HTTP.

---

## Claude's Discretion

- Test function naming, assertion messages, and helper structure
- tokio runtime configuration for async tests
- Whether agent timeout test uses mock HTTP or actor-level simulation

## Deferred Ideas

None — discussion stayed within phase scope.
