# Spike Conventions

Patterns and stack choices established across spike sessions. New spikes follow these unless the question requires otherwise.

## Stack

- **Investigation language:** prefer reading the provider's own open-source reference implementation first (e.g. `venice-cli` for Venice). Faster ground truth than docs alone.
- **Probes:** `curl` + `python3 -c` one-liners for response inspection. No new harness unless the spike demands one.
- **Cryptographic checks:** standard library tools (`openssl`, `python3 hashlib`) for sanity verification before writing Rust.
- **Production verification crates already on the recommended stack:** `dcap-qvl` (TDX), `sev` with `crypto_nossl` (SEV-SNP), `x509-cert`, `p256`/`p384`, `sha3`. Do not introduce new crypto crates inside spikes — if the spike needs something not in the stack, that itself is a finding worth surfacing.

## Structure

```
.planning/spikes/
  MANIFEST.md           — overall idea, requirements, spike index
  CONVENTIONS.md        — this file
  WRAP-UP-SUMMARY.md    — written by /gsd-spike-wrap-up
  NNN-spike-name/
    README.md           — frontmatter, research, investigation trail, results
    captures/           — real captured wire data, response samples, fixtures
```

`captures/` files double as golden fixtures for the eventual implementation's unit tests.

## Patterns

- **Probe before paying.** For provider integration spikes, exhaust unauthenticated public endpoints first. Only ask for an API key when nothing else can answer the question.
- **Capture once, reason offline.** When a wire format is unknown, fetch one real sample, save it, and decode it locally. Don't re-hit the endpoint repeatedly while exploring.
- **Decode REPORTDATA from a real capture, not from the docs.** Provider docs about the REPORTDATA layout are routinely incomplete or wrong — the only source of truth is a live attestation byte-decoded against a known-submitted nonce.
- **Compare against the reference verifier.** When a provider ships an open-source CLI verifier, read it end-to-end before designing the Rust path — but be prepared to do **better** than it (most reference verifiers trust server-side booleans).
- **Investigation trail in the README.** Document each step of the investigation, including dead ends and pivots. The trail is more valuable than the verdict for future build sessions.
- **Probe every routing variant.** Aggregator providers (Redpill is the canonical example) route across multiple backends with different response shapes. One capture is not enough — probe the unauthenticated endpoint against at least one model per backend before declaring the wire format known. Failures (e.g. an HTTP 502 from a broken relay) are themselves findings worth capturing.
- **Capture as golden fixtures.** The `captures/` files double as Rust unit-test fixtures. Save raw JSON responses and the submitted nonces alongside them so the binding assertions can be re-verified offline at any time. A small Python decoder script (`decode-report-data.py` style) that asserts every binding makes a clean reference for the eventual Rust port.
- **Cross-check the binding formula against the reference verifier source.** Provider docs about REPORTDATA layouts are routinely incomplete; the open-source verifier source is the only reliable second source. Subtle distinctions (string-concat vs byte-concat; client-fresh nonce vs enclave-baked nonce) live in the code, not in the docs.

## Tools & Libraries

**Used and validated:**
- `curl` for HTTP probes
- `openssl rand -hex 32` for client nonce generation in shell
- `python3 -c` with stdin JSON for live-response inspection — no script files needed
- `git clone --depth 1` into `/tmp/` for reading reference impls without polluting the workspace

**Avoid in spikes:**
- Building new harnesses or test rigs — exploit existing tools (`curl`, `python3`) instead
- Introducing new dependencies into `Cargo.toml` during a spike — if a real Rust probe is needed, write it standalone in `captures/probe.rs` first
- Hitting paid endpoints when free ones exist
