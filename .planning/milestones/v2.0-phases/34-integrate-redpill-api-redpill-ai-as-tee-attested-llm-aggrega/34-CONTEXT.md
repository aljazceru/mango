# Phase 34: Integrate Redpill — Context

**Gathered:** 2026-04-26
**Status:** Ready for planning
**Source:** Spike 002 (`.planning/spikes/002-redpill-tee-verification-research/`) — VALIDATED

<domain>
## Phase Boundary

Add Redpill (`api.redpill.ai`) as a fourth TEE-attested LLM provider alongside Tinfoil, PPQ.AI, and Venice. Redpill is an aggregator: a single endpoint routes across multiple confidential-compute backends (Phala-pure, Phala/NearAI orchestrated, Chutes, Tinfoil) with three different attestation response shapes. The client must dispatch on shape, verify every TDX quote in every shape via local `dcap-qvl`, verify NVIDIA NRAS JWTs where present, decode four distinct REPORTDATA layouts, enforce a debug-mode gate, and surface differing freshness semantics in the UI.

This phase does NOT include:
- E2EE handshake (Chutes uses HPKE; Phala-orchestrated uses ECDSA — defer to a v2 phase if/when needed; chat completions through Redpill are HTTPS-only at the wire and rely on TEE attestation for confidentiality)
- On-chain DCAP receipts via Automata smart contracts (deferred)
- Intel Trust Authority secondary appraisal (deferred)
- dstack-deep boot-replay verification (requires Docker, not viable on mobile)
- Tinfoil-via-Redpill integration (broken at Redpill's relay; existing direct-Tinfoil integration covers SEV-SNP)

</domain>

<decisions>
## Implementation Decisions

### Provider preset and routing

- **D-01 (RED-01):** Add `ProviderKind::Redpill` (or equivalent enum variant matching `ProviderKind::Venice` shape). Add a Redpill row to `known_provider_presets` in `llm/backend.rs` with base URL `https://api.redpill.ai/v1`, OpenAI-compatible chat completions, and the attestation endpoint as a separate URL.
- **D-02 (RED-02):** Attestation endpoint is `GET https://api.redpill.ai/v1/attestation/report?model=<model_id>&nonce=<64-hex>`. **No `Authorization` header** — endpoint is public. Client generates a fresh 32-byte random nonce per fetch (hex-encoded for the URL).
- **D-03:** Catalog of TEE-capable models is enumerated at install time from `GET https://api.redpill.ai/v1/models` (no auth). Cache the model→primary-provider mapping with a short TTL (e.g. 1 minute, mirroring the reference verifier).

### Response-shape dispatch

- **D-04 (RED-03):** Response-shape detector returns one of three variants. Detect in this priority order:
  1. `attestation_type == "chutes"` → **Shape C — Chutes**
  2. `gateway_attestation` present + `model_attestations[]` present → **Shape B — Orchestrated** (sub-flag `is_near_ai = model_attestations[0].compose_manager_attestation != null`)
  3. Top-level `signing_address` + `intel_quote` → **Shape A — Flat** (Phala-pure, Venice-identical)
  4. Anything else → fail closed with `RedpillError::UnknownShape`

### TDX quote verification (every shape)

- **D-05 (RED-04):** Every TDX quote across every shape uses `dcap-qvl::verify(quote_bytes, collateral, ts)`. Same path already used by ppq.ai and Phase 33 Venice — **no new crates**. Pin the same `dcap-qvl` version Venice ships.
- **D-06:** Quote bytes must be normalized before parse: auto-detect base64 vs hex with the helper `quote_bytes(&str)` (mirrors `redpill-verifier::toHexQuote`):
  ```rust
  fn quote_bytes(s: &str) -> Result<Vec<u8>, RedpillError> {
      let s = s.trim_start_matches("0x");
      if s.bytes().all(|b| b.is_ascii_hexdigit()) {
          hex::decode(s).map_err(...)
      } else {
          BASE64_STANDARD.decode(s).map_err(...)
      }
  }
  ```
- **D-07 (RED-08):** Reject any TDX quote with the debug bit set: `quote_bytes[48 + 120] & 0x01 != 0` ⇒ `RedpillError::DebugMode`. Apply across all three shapes (the Chutes verifier flags this as CRITICAL; we apply defensively to Shape A and B as well).

### REPORTDATA layout decoders (four)

REPORTDATA lives at TDX quote bytes `[568..632]` (header 48 B + body offset 520..584 = 64 bytes).

- **D-08 (RED-05a) — Model layout (ECDSA secp256k1, NVIDIA-CC):** Used for Shape A and the `model_attestations[i]` component of Shape B. **Byte-identical to Venice (Phase 33).** Reuse the Venice REPORTDATA decoder verbatim:
  ```
  [ 0..20]  signing_address  = keccak256(uncompressed_pubkey[1..65])[12..32]
  [20..32]  zero padding
  [32..64]  client nonce (raw)
  ```
- **D-09 (RED-05b) — Gateway layout (ed25519):** Shape B `gateway_attestation`:
  ```
  [ 0..32]  signing_address (raw ed25519 public key)
  [32..64]  client nonce (raw)
  ```
  Equality check is byte-slice equality on both halves; no hashing required.
- **D-10 (RED-05c) — Compose-manager layout:** Shape B `model_attestations[i].compose_manager_attestation`:
  ```
  [ 0..32]  actions_hash (sha256 over orchestration commit ledger; provided in the response)
  [32..64]  client nonce (raw)
  ```
  Verify `reportData[0..32] == response.compose_manager_attestation.actions_hash` and `reportData[32..64] == client_nonce`.
- **D-11 (RED-05d) — Chutes layout:** Shape C `all_attestations[i]`:
  ```
  [ 0..32]  SHA256(nonce_str ++ e2e_pubkey_str)   ← STRING concat of as-emitted ASCII bytes
  [32..64]  unconstrained — Chutes does NOT bind the client's ?nonce= here
  ```
  The "nonce" hashed here is **Chutes' enclave-baked nonce** returned in `all_attestations[i].nonce`, NOT the client's `?nonce=` query parameter. Verify with `sha2::Sha256::digest(format!("{}{}", a.nonce, a.e2e_pubkey))`.

### Composition rules per shape (RED-06)

- **D-12 — Shape A (Flat):** Open session iff: TDX quote sig verifies + REPORTDATA model layout binding holds + NVIDIA NRAS JWT verifies + debug-mode gate passes.
- **D-13 — Shape B (Orchestrated):** Open session iff: ALL THREE TDX quotes verify (gateway + model + compose-manager) + ALL THREE REPORTDATA layouts bind correctly + NVIDIA NRAS JWT for the model verifies + debug-mode gate passes on every quote. **Three-way AND.** Failure of any single component fails the whole attestation.
- **D-14 — Shape C (Chutes):** Open session iff: TDX quote sig verifies (per attestation in `all_attestations[]`) + Chutes anti-tamper binding holds + debug-mode gate passes + per-GPU `gpu_evidence` validation. **Verify at least the first attestation entry**; configurable strict-mode could verify all.

### NVIDIA GPU attestation (RED-07)

- **D-15:** Reuse `rust/src/attestation/nvidia.rs::fetch_and_verify_nvidia` unchanged. For Shape A and B, the `nvidia_payload` field is a JSON-stringified blob — `serde_json::from_str` it before forwarding to NRAS. For Shape C, each `all_attestations[i].gpu_evidence[]` entry is structured per-GPU evidence; iterate and verify each NRAS JWT.

### Tinfoil-via-Redpill (RED-10)

- **D-16:** Detect Tinfoil-routed models via `/v1/models` metadata (`providers: ["tinfoil"]`) and refuse to fetch attestation through Redpill — return a typed `RedpillError::TinfoilUnsupported` with a hint pointing the user to the existing direct-Tinfoil provider. Do not surface Tinfoil-routed Redpill models in the picker (or grey them out with the explanation).
- **D-17:** Re-test the relay quarterly (a `/schedule` agent will be set up after this phase ships). Once Redpill upgrades to `sev-snp-guest/v2`, lift this restriction.

### Freshness UI semantics (RED-09)

- **D-18:** The trust-status surface (Verified badge tooltip / model-detail screen) must display:
  - **Shape A and B:** "Verified for this request — attestation freshness bound to client nonce"
  - **Shape C:** "Verified for this enclave instance — attestation freshness bounded by enclave lifetime, not per-request"
- **D-19:** Internally, set `AttestationStatus::Verified { freshness: Freshness::PerRequest | Freshness::PerEnclave }` (or extend the existing enum variant with a freshness sub-field). The UI layer maps the sub-field to user-facing copy.

### Caching

- **D-20:** TTL-cache the verified attestation per `(model_id, response_hash)` for **5 minutes** (matches the existing TDX cache in `attestation/cache.rs`). Re-verify on cache miss or first request after TTL. For Shape C (enclave-baked nonce), the cache is naturally longer-lived because the same attestation can be reused for the enclave's lifetime — but we still re-fetch every 5 min as a defense-in-depth check against enclave restarts.

### Native UI

- **D-21 (RED-11):** Settings → Providers → Add Backend exposes "Redpill" as a preset. Same shape as the Venice preset added in Phase 33; all UniFFI-exported single source of truth via `known_provider_presets`. Native iOS/Android/Desktop layers render the row from the Rust list — no per-platform Redpill-specific code.
- **D-22:** The `Verified` attestation badge on the provider detail screen displays a sub-line for Orchestrated models showing the three verified components ("gateway ✓ • model ✓ • compose ✓") to make the multi-quote attestation visible to power users.

### Test fixtures

- **D-23:** The four live captures in `.planning/spikes/002-redpill-tee-verification-research/captures/` are golden fixtures for unit tests. Each REPORTDATA decoder gets at least one positive-path test against the corresponding capture. The Python decoder script in `captures/decode-report-data.py` shows the expected byte-slice assertions — port each to a Rust `#[test]`.
- **D-24:** Live integration tests gated by `#[ignore]` (mirrors Phase 33's pattern in `33-04-PLAN.md`): one per supported shape, hitting `api.redpill.ai` with a fresh nonce.

### Claude's Discretion

- File layout inside `rust/src/attestation/` and `rust/src/llm/`: follow whatever pattern Phase 33 established (`attestation/venice.rs` + `llm/venice.rs`). Suggested mirrors: `attestation/redpill.rs` + `llm/redpill.rs`. The internal sub-module split (one file vs sub-dir per shape) is implementation choice.
- Error type naming: extend the existing error enum or add a `RedpillError`. Keep cross-boundary errors UniFFI-friendly.
- Whether to reuse Venice's REPORTDATA decoder by importing it directly (best — single source of truth) or by copy-paste with a comment (acceptable if the file structure makes import awkward).
- Whether to model the response shapes as a single `RedpillResponse` enum or as three separate types behind a trait. Prefer the enum for ergonomic dispatch.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Spike findings (the primary research artifact)

- `.planning/spikes/002-redpill-tee-verification-research/README.md` — full spike report with REPORTDATA decoders, root-of-trust topology, shape-by-shape capture analysis, investigation trail
- `.planning/spikes/002-redpill-tee-verification-research/captures/` — live wire data (4 JSONs + nonce log + Python decoder), golden fixtures for unit tests
- `.claude/skills/spike-findings-confidential-app/references/redpill-attestation.md` — implementation blueprint extracted from the spike (auto-loaded via `Skill("spike-findings-confidential-app")`)
- `.claude/skills/spike-findings-confidential-app/references/venice-attestation.md` — Venice blueprint; the model-ecdsa REPORTDATA decoder is reused verbatim in Redpill

### Phase 33 (Venice) — the structural analog

- `.planning/phases/33-integrate-venice-ai-as-tee-attested-llm-provider-with-client/33-RESEARCH.md` — the most directly analogous prior research; Phase 34 reuses the TDX cryptographic verification path and the REPORTDATA model layout
- `.planning/phases/33-integrate-venice-ai-as-tee-attested-llm-provider-with-client/33-PATTERNS.md` — file-by-file pattern map established for Venice; Redpill should follow the same conventions
- `.planning/phases/33-integrate-venice-ai-as-tee-attested-llm-provider-with-client/33-0[1-4]-PLAN.md` — the four Venice plans; Redpill plans should mirror their wave structure

### Reference verifier (open-source, MIT)

- <https://github.com/redpill-ai/redpill-verifier> — TypeScript reference implementation; `js/src/verify.ts`, `js/src/verifiers/cloud-api.ts`, `js/src/verifiers/chutes.ts`, `js/src/providers/detect.ts`, `js/src/constants.ts`. Useful for cross-checking field names and binding formulas; we will be **stronger** than this reference (it delegates TDX to Phala's hosted API; we re-verify locally).

### Project guardrails

- `CLAUDE.md` — RMP architecture rules (Rust core owns business logic, no OpenSSL, all-Rust crypto), tech stack constraints, attestation requirements
- `.planning/REQUIREMENTS.md` — RED-01..RED-11 in `### Redpill TEE-Attested Provider`
- `rust/src/attestation/venice.rs` — the Venice REPORTDATA decoder we'll reuse for Redpill's model layout (created in Phase 33)
- `rust/src/attestation/nvidia.rs` — NRAS JWT verifier we reuse unchanged
- `rust/src/attestation/cache.rs` — TTL cache for verified attestations
- `rust/src/llm/venice.rs` and `rust/src/llm/backend.rs` — provider integration patterns to mirror

</canonical_refs>

<specifics>
## Specific Ideas

- **One provider, four backends, one verifier.** Treat Redpill as a single `ProviderKind::Redpill` with internal shape dispatch. Don't expose the routed sub-backends as separate providers in the UI — that's an implementation detail.
- **Reuse Phase 33 wave structure.** Phase 33 had four plans: Wave 0 (deps + golden fixtures + RED test stubs), Wave 1 (attestation layer), Wave 2 (LLM transport), Wave 3 (wiring + integration tests). Phase 34 should mirror this. The spike's captures replace Phase 33's golden fixture creation step (we already have them).
- **The Python decoder is the test plan.** Each assertion in `captures/decode-report-data.py` becomes a Rust `#[test]` against the corresponding JSON fixture. Free test design.
- **No E2EE in this phase.** Phase 33 added E2EE (ECDH+HKDF+AES-GCM) for Venice. Redpill's chat completions are vanilla OpenAI-compatible over HTTPS — the TEE attestation is the confidentiality root, not the wire layer. If Chutes' HPKE handshake or Phala's per-message ECDSA wrappers become required, that's a follow-up phase.
- **Three-way AND on Orchestrated is a sovereignty win.** Surface this in the UI as a positive — most providers expose one quote; Redpill exposes three. Users who care about confidential-compute provenance get more information here than anywhere else.
- **The compose-manager attestation = a verifiable orchestration commit ledger.** We can surface "model image last published by commit `<sha>`" in the trust UI without extra crypto work — the data is already in the response (`compose_manager_attestation.actions[]`).

</specifics>

<deferred>
## Deferred Ideas

- **E2EE handshakes.** HPKE for Chutes; ECDH for Phala/NearAI per-response signing keys. Not required for v1 — TEE attestation is the confidentiality root, HTTPS handles transport.
- **On-chain DCAP receipts via Automata.** Useful for an audit-receipts feature; requires an Ethereum RPC dependency on the device. Defer.
- **Intel Trust Authority secondary appraisal.** Requires a per-user API key. Defer indefinitely.
- **dstack-deep boot-replay.** Requires Docker + QEMU; not viable inside the iOS/Android sandbox. Could expose as a "verify on a server you control" feature in v3.
- **Tinfoil-via-Redpill integration.** Blocked upstream (`Unsupported Tinfoil attestation format: sev-snp-guest/v2`). Schedule a quarterly re-probe; lift the block when Redpill upgrades.
- **Sigstore container provenance check.** Reference verifier inspects Sigstore links from the orchestration ledger. Useful for an audit feature; not needed on the critical path.
- **`/v1/signature/{chatId}` after-the-fact response binding.** Lets users prove a specific response came from a specific attested enclave. Worth a follow-up phase but not required for "Redpill works as an attested provider."

</deferred>

---

*Phase: 34-integrate-redpill-api-redpill-ai-as-tee-attested-llm-aggrega*
*Context gathered: 2026-04-26 — derived from spike-002 findings (no separate /gsd-discuss-phase needed)*
