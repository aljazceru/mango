# Phase 10: PPQ.AI Backend Integration - Context

**Gathered:** 2026-05-08 (retrospective - phase was implemented 2026-03-26)
**Status:** Implementation complete, creating retrospective documentation

<domain>
## Phase Boundary

Integrate PPQ.AI as a TEE-attested LLM provider with AMD SEV-SNP attestation verification, PPQ private E2EE transport, and private model filtering. PPQ.AI exposes AMD SEV-SNP protected private models accessible only with an API key.

</domain>

<decisions>
## Implementation Decisions

### Architecture
- **D-01:** Create dedicated `rust/src/llm/ppq_private.rs` module for PPQ-specific transport and attestation
- **D-02:** Add `PpqPrivateE2ee` to `ProviderTransportKind` enum for transport routing
- **D-03:** Add `AmdSevSnp` to `TeeType` enum for attestation type support
- **D-04:** Use EHBP (Encrypted HTTP Body Protocol) for PPQ private transport with HPKE encryption
- **D-05:** Implement PPQ-specific attestation verification using AMD SEV-SNP report parsing

### Database
- **D-06:** MIGRATION_V10 seeds PPQ.AI backend with initial public base URL
- **D-07:** MIGRATION_V11 switches seeded PPQ.AI to private transport base URL
- **D-08:** PPQ.AI backend uses `tee_type: "AmdSevSnp"` and `transport_kind: PpqPrivateE2ee`

### Testing
- **D-09:** Create `rust/src/tests/live_ppq_private.rs` for live PPQ attestation tests
- **D-10:** Add PPQ transport tests to `rust/src/tests/transport.rs`
- **D-11:** Add PPQ preset tests to `rust/src/tests/backend_config.rs`
- **D-12:** Add migration tests for V10/V11 in `rust/src/tests/persistence.rs`

### UI Integration
- **D-13:** Add ppq.ai link to onboarding screens on all platforms
- **D-14:** Add `AmdSevSnp` to TEE type picker in settings on all platforms
- **D-15:** Add `teeTypeLabel()` support for `AmdSevSnp` on all platforms

</decisions>

<canonical_refs>
## Canonical References

**Implementation artifacts (created during implementation):**
- `rust/src/llm/ppq_private.rs` — PPQ private transport module
- `rust/src/llm/transport.rs` — PpqPrivateE2ee transport kind
- `rust/src/llm/backend.rs` — PPQ provider preset and TeeType::AmdSevSnp
- `rust/src/attestation/task.rs` — PPQ attestation routing
- `rust/src/persistence/schema.rs` — MIGRATION_V10 and MIGRATION_V11
- `rust/src/llm/streaming.rs` — PPQ streaming support
- `rust/src/agent/loop.rs` — PPQ dispatch in agent loop

**Test files:**
- `rust/src/tests/live_ppq_private.rs` — Live PPQ attestation tests
- `rust/src/tests/transport.rs` — PPQ transport tests
- `rust/src/tests/backend_config.rs` — PPQ preset tests
- `rust/src/tests/persistence.rs` — Migration V11 test
- `rust/src/tests/streaming.rs` — PPQ streaming cancellation test

**UI files:**
- `android/app/src/main/java/dev/disobey/mango/ui/OnboardingScreen.kt` — ppq.ai link
- `ios/Mango/Mango/OnboardingView.swift` — ppq.ai link and AmdSevSnp support
- `desktop/iced/src/views/onboarding.rs` — ppq.ai link
- `android/app/src/main/java/dev/disobey/mango/ui/SettingsProvidersScreen.kt` — AmdSevSnp picker
- `ios/Mango/Mango/SettingsProvidersView.swift` — AmdSevSnp picker

**Design:**
- `10-UI-SPEC.md` — UI specification for PPQ.AI integration

</canonical_refs>

<code_context>
## Existing Code Insights

Phase 10 was implemented before GSD workflow adoption. All implementation artifacts exist in codebase. This retrospective documentation captures what was built.

</code_context>

<specifics>
## Specific Ideas

Retrospective documentation only - implementation already complete.

</specifics>

<deferred>
## Deferred Ideas

None - phase was fully implemented.

</deferred>

---

*Phase: 10-ppq-ai-backend-integration*
*Retrospective context: 2026-05-08*
*Original implementation: 2026-03-26*
