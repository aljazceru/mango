---
status: resolved
trigger: "attestation-vcek-429-and-cert-caching"
created: 2026-03-26T00:00:00Z
updated: 2026-03-26T00:00:00Z
---

## Current Focus

hypothesis: CONFIRMED — two distinct bugs. BUG-2 (VCEK cache) was fully fixed. BUG-1
  (stickiness) fix was too broad. The blanket "never downgrade Verified on any Failed"
  guard was rejected by the user. Correct fix: distinguish transient fetch errors
  (NetworkError, CollateralFetch — we never reached the TEE) from genuine verification
  failures (QuoteVerification, NonceMismatch, JwtVerification — the TEE report itself
  was invalid). Only transient errors preserve Verified; genuine failures always downgrade.
test: checkpoint response confirmed original fix was too broad
expecting: n/a, revised plan clear
next_action: |
  1. Add AttestationError::is_transient() → bool method (true for NetworkError + CollateralFetch)
  2. Add is_transient: bool field to AttestationEvent::Failed
  3. spawn_attestation_task: set is_transient from e.is_transient() when mapping Err
  4. lib.rs handler: stickiness guard checks is_transient before skipping update
  5. Tests: add test_attestation_genuine_failure_downgrades_verified;
           update test_attestation_verified_is_sticky_against_transient_failure to use is_transient=true path

## Symptoms

expected: |
  1. Once attestation status is Verified, a subsequent 429 from AMD's VCEK endpoint should
     NOT downgrade it — the verified status should remain sticky until the next successful
     re-verification attempt.
  2. AMD ARK/ASK certificates (fetched from kdsintf.amd.com) and VCEK certificates should
     be cached on disk or in-memory so repeated attestation runs don't hammer the AMD KDS
     endpoint and don't trigger rate limits.
actual: |
  1. A 429 response from the AMD VCEK endpoint causes the attestation status to flip from
     Verified back to Failed/Error, even though the previous verification was successful.
  2. Every attestation run fetches the VCEK certificate fresh from AMD KDS, leading to
     rate-limit 429s when the periodic attestation refresh (or app restarts) re-runs
     attestation repeatedly.
errors: "429 Too Many Requests" from amd.com VCEK/KDS endpoint; attestation status shown as failed after previously being verified
reproduction: |
  - Let attestation succeed once (status = Verified)
  - Trigger re-attestation quickly (or let periodic refresh fire)
  - 429 arrives from AMD KDS → status flips to error/failed
started: Observed after periodic-attestation-refresh was implemented

## Eliminated

(none — root cause confirmed on first pass)

## Evidence

- timestamp: 2026-03-26T00:00:00Z
  checked: lib.rs AttestationResult handler (lines 3132-3148)
  found: AttestationEvent::Failed arm unconditionally sets entry.status = Failed{reason}. No guard checks whether the existing status is already Verified.
  implication: Any transient error (including 429 from AMD KDS) will clobber a Verified status.

- timestamp: 2026-03-26T00:00:00Z
  checked: task.rs verify_snp_report (lines 300-338)
  found: vcek_url is built and fetched every single call with a fresh reqwest::Client. No cache lookup before the HTTP GET. No cache write after a successful fetch.
  implication: Every periodic re-attestation hammers the AMD KDS endpoint. When rate-limited (429), verify_snp_report returns Err(CollateralFetch), which propagates to AttestationEvent::Failed, which then clobbers the status (BUG-1).

- timestamp: 2026-03-26T00:00:00Z
  checked: attestation/cache.rs
  found: AttestationCache is a well-built SQLite cache for attestation *results* (AttestationRecord with status, report_blob, expires_at). It does NOT store VCEK DER bytes. VCEK caching must be added separately.
  implication: Need either (a) a new SQLite table for VCEK DER bytes, or (b) an in-memory cache keyed by vcek_url, passed into verify_snp_report. In-memory is simplest and sufficient since VCEK certs are stable for the chip_id+TCB combination.

- timestamp: 2026-03-26T00:00:00Z
  checked: schema.rs MIGRATIONS
  found: Current schema is at v8. A MIGRATION_V9 adding a vcek_cert_cache table is the cleanest approach for disk-persistent VCEK caching.
  implication: Alternatively, pass a &mut HashMap<String,Vec<u8>> into verify_snp_report for process-lifetime in-memory caching. Since attestation runs as a spawned task (not on the actor thread), an Arc<Mutex<HashMap>> or Arc<RwLock<HashMap>> would be needed for cross-task sharing. The disk approach avoids this concurrency complexity.

## Resolution

root_cause: |
  Two bugs with the same trigger path (periodic re-attestation of an AMD SEV-SNP backend):
  1. STATUS CLOBBER (lib.rs:3148): AttestationEvent::Failed unconditionally overwrites AppState.attestation_statuses regardless of the prior status. A transient fetch error (429, network timeout) downgrades a Verified backend to Failed.
  2. NO VCEK CACHE (task.rs:300-338): verify_snp_report fetches the VCEK DER certificate from AMD KDS on every invocation. No in-memory or disk cache exists for the VCEK bytes, so periodic re-attestation generates repeated KDS requests, triggering rate-limit 429s.

fix: |
  FIX-1 (revised — error.rs + mod.rs + task.rs + lib.rs):
    a) Added AttestationError::is_transient() → bool method.
       Returns true for NetworkError and CollateralFetch (transient: we never reached the TEE).
       Returns false for QuoteVerification, NonceMismatch, JwtVerification, CacheFailed, Unsupported
       (genuine failures: the TEE report was inspected and found invalid).
    b) Added is_transient: bool field to AttestationEvent::Failed.
       spawn_attestation_task sets it from e.is_transient() when converting Err(AttestationError).
    c) lib.rs stickiness guard revised:
       - Transient error + current status Verified → skip update (log but preserve Verified)
       - Genuine verification failure → always update, even from Verified
       - New Verified always accepted (updates expires_at)
       - Any other transition (Unverified→Failed, Expired→Failed, etc.) always accepted
  FIX-2 (unchanged): VCEK DER in-memory cache (VcekCache Arc<RwLock<HashMap>>) passed into
    verify_snp_report; SQLite vcek_cert_cache table for cross-process warmup. Already implemented.

verification: |
  Compiled with zero new errors. 5 pre-existing errors in unrelated modules
  (fastembed/usearch/pdf_extract not in Cargo.toml, desktop.rs type annotations)
  are unchanged. 2 pre-existing warnings unchanged.
  Test files updated for new API (is_transient field on all AttestationEvent::Failed
  constructions; pattern matches use .. where is_transient not needed). New/updated tests:
  - test_attestation_genuine_failure_downgrades_verified (NEW): proves genuine failures
    downgrade Verified status (is_transient=false path)
  - test_attestation_verified_is_sticky_against_transient_failure (updated): now uses
    is_transient=true to test the correct code path
  - test_attestation_error_is_transient (NEW): unit test for is_transient() method
  - test_attestation_failed_replaced_by_verified: updated with is_transient=false
  - test_attestation_status_upsert_non_verified: updated with is_transient=false
files_changed:
  - rust/src/lib.rs
  - rust/src/attestation/task.rs
  - rust/src/attestation/mod.rs
  - rust/src/attestation/tdx.rs
  - rust/src/attestation/nvidia.rs
  - rust/src/persistence/schema.rs
  - rust/src/tests/attestation_types.rs
  - rust/src/tests/attestation_integration.rs
