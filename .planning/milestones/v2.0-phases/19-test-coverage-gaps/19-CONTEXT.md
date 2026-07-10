# Phase 19: Test Coverage Gaps - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Add unit tests for three specific coverage gaps identified in the CONCERNS.md audit: (1) streaming cancellation in encrypted transports (Tinfoil/PPQ), (2) agent mid-step failures (network timeout, 20-step max enforcement, malformed tool results), and (3) attestation cache TTL behavior (`get_latest_for_backend()` expiry rejection and `get_raw_report()` TTL bypass).

No production code changes — test-only phase.

</domain>

<decisions>
## Implementation Decisions

### Test Organization
- **D-01:** Extend existing test files rather than creating new ones. Streaming cancellation tests go in `rust/src/tests/streaming.rs`, agent failure tests in `rust/src/tests/agent.rs`, attestation cache TTL tests in `rust/src/tests/attestation_cache.rs`.

### Mock Strategy for Encrypted Transport Cancellation (TEST-01)
- **D-02:** Test mid-stream cancellation using tokio drop semantics — create an encrypted streaming response, drop the receiver/cancel the task mid-flight, assert the stream terminates cleanly without resource leaks. No live backends needed.
- **D-03:** Test both Tinfoil (HPKE-encrypted SSE) and PPQ (AES-GCM encrypted SSE) transport cancellation paths.

### Agent Failure Injection (TEST-02)
- **D-04:** Use existing in-memory Database + crafted AgentStepRow data to test max-step enforcement (20-step limit) and malformed tool results without live LLM backends.
- **D-05:** For network timeout injection, mock the HTTP response layer to simulate a timeout at an agent mid-step, then verify the session is checkpointed and resumable.

### Attestation Cache TTL (TEST-03)
- **D-06:** Add `test_get_latest_for_backend_expiry` — insert an expired cache entry, call `get_latest_for_backend()`, assert it returns `None`.
- **D-07:** Add `test_get_raw_report_bypasses_ttl` — insert an expired cache entry, call `get_raw_report()`, assert it returns the blob regardless of TTL.

### Claude's Discretion
- Exact test function names and assertion messages
- Whether to use helper functions or inline setup in each test
- tokio test runtime configuration (single-threaded vs multi-threaded)
- Whether agent timeout test needs a mock HTTP client or can simulate at the actor message level

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Test Files (extend these)
- `rust/src/tests/streaming.rs` — Existing streaming tests; add cancellation tests here
- `rust/src/tests/agent.rs` — Agent persistence/tool tests; add failure injection tests here
- `rust/src/tests/attestation_cache.rs` — Cache TTL tests; add `get_latest_for_backend` and `get_raw_report` TTL tests here

### Production Code Under Test
- `rust/src/llm/tinfoil_secure.rs` — Tinfoil HPKE-encrypted streaming transport (cancel target)
- `rust/src/llm/ppq_private.rs` — PPQ AES-GCM encrypted streaming transport (cancel target)
- `rust/src/llm/streaming.rs` — `InternalEvent` stream types, StopGeneration handling
- `rust/src/agent/tools.rs:43` — `"maximum": 20` step limit; line 150 clamps to `1..=20`
- `rust/src/attestation/cache.rs:99` — `get_latest_for_backend()` with TTL WHERE clause (`expires_at > now`)
- `rust/src/attestation/cache.rs` — `get_raw_report()` bypasses TTL by design

### Audit Source
- `.planning/codebase/CONCERNS.md` — Original audit identifying these coverage gaps

### Requirements
- `REQUIREMENTS.md` — TEST-01 (streaming cancel), TEST-02 (agent failures), TEST-03 (cache TTL)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `make_app()` helper in `streaming.rs` and `agent.rs` — creates `FfiApp` with in-memory DB and null providers
- `make_record()` helper in `attestation_cache.rs` — creates `AttestationRecord` for cache tests
- `now_secs()` helper in `attestation_cache.rs` — current Unix timestamp
- `Database::open(":memory:")` pattern used throughout for isolated test DBs

### Established Patterns
- Tests use `std::thread::sleep(Duration::from_millis(...))` for actor sync (not ideal but consistent)
- Agent tests use direct persistence queries (`insert_agent_session`, `insert_agent_step`) to set up state
- Cache tests create expired records by setting `expires_at = now - 1`
- `FfiApp::dispatch(AppAction::StopGeneration)` exists for cancellation signaling

### Integration Points
- `StopGeneration` action in actor dispatches cancellation to streaming layer
- Agent step count enforcement happens in `agent/tools.rs` via the `max(1).min(20)` clamp
- `get_latest_for_backend()` SQL uses `expires_at > ?2` for TTL enforcement
- `get_raw_report()` intentionally omits TTL check — queries by (backend_id, tee_type) without time filter

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. Success criteria are precisely defined in the roadmap (5 concrete test assertions).

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 19-test-coverage-gaps*
*Context gathered: 2026-03-29*
