---
phase: 34
phase_name: integrate-redpill-api-redpill-ai-as-tee-attested-llm-aggregator
status: complete
plans_completed: [34-01, 34-02, 34-03, 34-04]
follow_up_phase: 34.1
follow_up_status: complete
---

# Phase 34: Integrate Redpill API as TEE-Attested LLM Aggregator — Summary

**One-liner:** Wired Redpill (https://api.redpill.ai) as a confidential-routing LLM aggregator with three attestation shapes (Flat / PerRequest, Orchestrated / PerRequest with three components, Chutes / PerEnclave), client-side TDX + NVIDIA NRAS verification, AttestationEvent enrichment, and live integration tests.

See per-plan summaries for detailed work:

- 34-01-SUMMARY.md — backend preset, transport routing, attestation URL plumbing
- 34-02-SUMMARY.md — TDX/NRAS verification path, three-shape detection
- 34-03-SUMMARY.md — orchestrated three-component breakdown, per-enclave freshness
- 34-04-SUMMARY.md — AttestationEvent::Verified field surfacing + live integration tests

## Verifier Audit Outcome

The Phase 34 verifier audit (34-VERIFICATION.md) closed at **9/11 must-haves verified**. RED-09 (PerEnclave freshness sub-line) and RED-11 (Orchestrated three-quote breakdown) were flagged **PARTIAL**: the cryptographic verification path was complete and unit-tested, but the actor-loop destructure at `rust/src/lib.rs:7421-7430` discarded `shape`, `freshness`, and `orchestrated_components` with `_` binds and a "deferred to future cache columns" comment, preventing the data from reaching the UniFFI boundary or native trust UI.

## Follow-up: Phase 34.1 closure (2026-04-27)

Phase 34's verifier audit (34-VERIFICATION.md) flagged RED-09 (PerEnclave freshness sub-line) and RED-11 (Orchestrated three-quote breakdown) as PARTIAL — the cryptographic verification was complete and tested, but the actor-loop destructure at `rust/src/lib.rs:7421-7430` discarded `shape`, `freshness`, and `orchestrated_components` with `_` binds and a "deferred to future cache columns" comment.

Phase 34.1 closes that drop:
- Helper `attestation::map_event_to_record_and_status` extracted; actor handler reduced to a single call (34.1-01).
- `AttestationStatus::Verified` promoted to a struct variant carrying optional `shape`, `freshness`, and `Vec<OrchestratedComponent>` across the UniFFI boundary; Kotlin and Swift bindings regenerated (34.1-02).
- SQLite migration V19 adds three nullable TEXT columns to `attestation_cache`; round-trip preserves the values; pre-V19 rows hydrate to None (34.1-03).
- Native trust UI on Android, iOS, and Desktop iced now renders "Verified for this enclave instance" sub-line on PerEnclave and "gateway ✓ • model ✓ • compose ✓" sub-line on Orchestrated, per UI-SPEC.md locked copy (34.1-04, 34.1-05, 34.1-06).

**RED-09 and RED-11 are FULL as of Phase 34.1.**
