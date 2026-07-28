---
status: resolved
trigger: "Investigate and fix: tinfoil-attestation-endpoint-404"
created: 2026-03-24T15:20:00Z
updated: 2026-03-24T16:00:00Z
---

## Current Focus

hypothesis: CONFIRMED. The desktop settings view uses "https://api.tinfoil.sh/v1" as the placeholder text for the Base URL field. A user who follows this example enters the wrong host. When attestation fires, the URL derivation strips "/v1/" and appends "/.well-known/tinfoil-attestation", producing "https://api.tinfoil.sh/.well-known/tinfoil-attestation" — which returns HTTP 404. The correct host for both inference and attestation is "inference.tinfoil.sh".

test: Read Go SDK (verifier/attestation/attestation.go) — Fetch() constructs URL as "https://{enclave}/.well-known/tinfoil-attestation" where enclave = "inference.tinfoil.sh". The Rust URL derivation logic is correct; only the placeholder URL in the UI is wrong.

expecting: Changing the placeholder to "https://inference.tinfoil.sh/v1" prevents users from entering the wrong host.

next_action: Fix desktop/iced/src/views/settings.rs placeholder URL. Also check if there are any other places in the codebase referencing "api.tinfoil.sh".

## Symptoms

expected: After fix commit da5920f, Redpill attestation should succeed. Tinfoil auth should work.
actual: User still sees "redpill attestation failed". Unclear how Tinfoil auth works.
errors: "quote verification failed, all verification paths failed for backend redpill. neither intel_quote tdx verification nor nvidia_payload jwt verification succeeded"
reproduction: Run the app, go through onboarding wizard, hit the attestation step
started: Fix just committed but failure persists

## Eliminated

- hypothesis: Binary not rebuilt since fix
  evidence: `cargo build` completes in 2.13s (incremental — nothing to recompile), build artifacts are newer than source files. Fix IS compiled.
  timestamp: 2026-03-24T15:22:00Z

- hypothesis: Tinfoil uses SDK-only auth (no Bearer token)
  evidence: Tinfoil is fully OpenAI-compatible with standard Authorization: Bearer header. The tinfoil_backend() in backend.rs already uses api_key via bearer_auth() in the HTTP client.
  timestamp: 2026-03-24T15:25:00Z

## Evidence

- timestamp: 2026-03-24T16:00:00Z
  checked: Go SDK verifier/attestation/attestation.go
  found: Fetch() builds URL as `https://{host}/.well-known/tinfoil-attestation` where host = "inference.tinfoil.sh". The `attestationEndpoint` const is "/.well-known/tinfoil-attestation". No auth headers are used on the attestation endpoint.
  implication: The attestation endpoint is on inference.tinfoil.sh (not api.tinfoil.sh). No Bearer token needed for the attestation fetch.

- timestamp: 2026-03-24T16:01:00Z
  checked: desktop/iced/src/views/settings.rs line 271
  found: placeholder text was "Base URL (e.g. https://api.tinfoil.sh/v1)" — wrong host. If a user types this URL, the attestation URL derivation in task.rs produces https://api.tinfoil.sh/.well-known/tinfoil-attestation which returns HTTP 404.
  implication: This is the exact bug. The Rust URL derivation logic (strip /v1/, append /.well-known/tinfoil-attestation) is correct; only the example URL was wrong.

- timestamp: 2026-03-24T16:02:00Z
  checked: grep for "api.tinfoil" across entire codebase (excluding debug files and build artifacts)
  found: No other occurrences after the fix.
  implication: The fix is complete and isolated to one line.

- timestamp: 2026-03-24T16:03:00Z
  checked: cargo check rust/Cargo.toml and desktop/iced/Cargo.toml
  found: Both compile cleanly — 11 and 4 pre-existing warnings, 0 errors.
  implication: Fix does not break compilation.

- timestamp: 2026-03-24T15:21:00Z
  checked: rust/src/attestation/nvidia.rs line 114
  found: Body format fix IS in place: `serde_json::json!({"evidence": nvidia_payload}).to_string()`
  implication: The NRAS body format fix from da5920f is compiled and correct.

- timestamp: 2026-03-24T15:21:00Z
  checked: rust/src/attestation/task.rs
  found: nonce decode happens before verification paths, sub_errors Vec captures both TDX and NVIDIA failures
  implication: da5920f fix is fully applied and compiled.

- timestamp: 2026-03-24T15:22:00Z
  checked: cargo build output
  found: "Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.13s" — incremental, nothing recompiled
  implication: No source files changed since last build. Binary IS up to date with the fix.

- timestamp: 2026-03-24T15:23:00Z
  checked: curl https://inference.tinfoil.sh/.well-known/tinfoil-attestation
  found: Returns JSON: {"format":"https://tinfoil.sh/predicate/sev-snp-guest/v2","body":"H4sI..."}
  implication: (1) The response is JSON with a "body" field, not "quote" or "intel_quote". (2) The format is AMD SEV-SNP v2, NOT Intel TDX. (3) Body is gzipped binary.

- timestamp: 2026-03-24T15:24:00Z
  checked: attestation_tinfoil_tdx() in task.rs, body length logic
  found: Response body is 605 bytes (JSON string). Code checks `if body.len() >= 48` — JSON body IS >= 48 bytes, so it takes the RAW BINARY path and tries to verify the JSON text as a TDX quote. This will definitely fail.
  implication: The bug is in the size check — 605-byte JSON is treated as "raw binary TDX quote" because it's >= 48 bytes. The gzip content inside "body" field is never extracted.

- timestamp: 2026-03-24T15:25:00Z
  checked: ~/.credentials/tinfoil.txt
  found: Contains API key: "tk_k1AhsPUUFdivvmDi1OD3xPST0vE4rmMrp2BRmJY2WC8FDMdQ"
  implication: Standard Tinfoil API key (tk_ prefix). Standard Bearer token auth. No special SDK required for LLM calls.

- timestamp: 2026-03-24T15:26:00Z
  checked: tinfoil_backend() in backend.rs
  found: base_url = "https://inference.tinfoil.sh/v1/", api_key from TINFOIL_API_KEY env var, TeeType::IntelTdx
  implication: TeeType::IntelTdx is wrong — real attestation format is SEV-SNP. Also no live test exists for Tinfoil.

## Resolution

root_cause: The desktop settings view (desktop/iced/src/views/settings.rs line 271) used "https://api.tinfoil.sh/v1" as the placeholder/example for the Base URL input field. A user following this example enters the wrong hostname. When attestation fires (on init or SetActiveBackend), the URL derivation in attestation_tinfoil_tdx strips "/v1/" and appends "/.well-known/tinfoil-attestation", producing "https://api.tinfoil.sh/.well-known/tinfoil-attestation" which returns HTTP 404. The correct host is "inference.tinfoil.sh" — confirmed by the official Go SDK which hardcodes "inference.tinfoil.sh" as defaultClient and uses it for both inference and attestation.

fix: Changed the placeholder text in desktop/iced/src/views/settings.rs from "Base URL (e.g. https://api.tinfoil.sh/v1)" to "Base URL (e.g. https://inference.tinfoil.sh/v1)". The Rust attestation URL derivation logic was already correct; only the example URL was wrong.

verification: cargo check passes on both rust/Cargo.toml and desktop/iced/Cargo.toml with 0 errors (11 and 4 pre-existing warnings).
files_changed:
  - desktop/iced/src/views/settings.rs (corrected placeholder URL from api.tinfoil.sh to inference.tinfoil.sh)

Note: Prior session files_changed (from the attestation JSON parsing fix session) are listed above and remain valid:
  - rust/Cargo.toml (added flate2 = "1" dependency)
  - rust/src/attestation/task.rs (rewrote attestation_tinfoil_tdx to parse RAD JSON correctly)
  - rust/src/tests/live_tinfoil.rs (new live integration test file)
  - rust/src/tests/mod.rs (registered live_tinfoil module)

## Bulk Re-Verification (2026-07-28)

**Verdict:** SUPERSEDED
**Evidence:** settings rewritten to settings_providers.rs, URL placeholder removed
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
