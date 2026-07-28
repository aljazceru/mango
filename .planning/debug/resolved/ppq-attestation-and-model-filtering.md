---
status: resolved
trigger: "Two related PPQ.AI bugs: (1) attestation verification fails, (2) model list includes non-TEE models (Claude, GPT) when only `private/` prefix models run in TEE."
created: 2026-03-26T00:00:00Z
updated: 2026-03-26T00:20:00Z
---

## Current Focus
<!-- OVERWRITE on each update - reflects NOW -->

hypothesis: CONFIRMED — filter in spawn_health_check is correct but stale SQLite data (331 models) is never flushed because there is no startup health check. Filter must also be applied at read time in reload_backends() and startup backend loading.
test: applying filter in reload_backends() and startup loading code path so stale DB rows are filtered immediately on every read, regardless of when health check last ran
expecting: after rebuild+install, UI shows only private/ models for PPQ.AI even without triggering a new health check
next_action: fix rust/src/lib.rs reload_backends() and startup loading; fix ChatScreen.kt model picker to show provider name; rebuild and install

## Symptoms
<!-- Written during gathering, then IMMUTABLE -->

expected: PPQ.AI attestation succeeds; model list shows only TEE-capable models (those with `private/` prefix, e.g. `private/deepseek-r1-0528`)
actual: Attestation fails; model list shows all models including Claude and GPT models that cannot run in TEE
errors: Attestation failure (exact error unknown — investigate from code); wrong models shown in UI
reproduction: Add PPQ.AI provider with API key, open model selector — non-TEE models appear; enable attestation — it fails
started: Since PPQ.AI was added (commit 53aea4c)

## Eliminated
<!-- APPEND only - prevents re-investigating -->

- hypothesis: PPQ.AI has its own Tinfoil-compatible attestation endpoint at api.ppq.ai
  evidence: curl to https://api.ppq.ai/.well-known/tinfoil-attestation returns HTTP 200 but body is {"message":"OOPS! - 404 Not Found"} — a fake-200/404 with no format/body fields
  timestamp: 2026-03-26

## Evidence
<!-- APPEND only - facts discovered -->

- timestamp: 2026-03-26
  checked: PPQ.AI /v1/models endpoint (live API call)
  found: Returns 331 total models including Claude, GPT, Gemini, etc. Only 5 have `private/` prefix. All 5 private/ models are owned_by="Tinfoil" — PPQ.AI routes them through Tinfoil's TEE infrastructure.
  implication: Bug 2 (model list) — health check at spawn_health_check() fetches all 331 models with no filtering, then persists them all to SQLite, then UI shows all 331 models for PPQ.AI.

- timestamp: 2026-03-26
  checked: attestation_tinfoil_tdx() in rust/src/attestation/task.rs
  found: Strips /v1/ from base_url to get host root, then calls /.well-known/tinfoil-attestation. For PPQ.AI base_url=https://api.ppq.ai/v1/, host becomes https://api.ppq.ai, and api.ppq.ai returns a fake-200 non-TinfoilAttestationDoc JSON. serde_json::from_slice fails with a QuoteVerification error.
  implication: Bug 1 (attestation) — the attestation URL is wrong for PPQ.AI. PPQ.AI's private models run on Tinfoil's infrastructure (inference.tinfoil.sh), so the attestation should be fetched from https://inference.tinfoil.sh/.well-known/tinfoil-attestation.

- timestamp: 2026-03-26
  checked: https://inference.tinfoil.sh/.well-known/tinfoil-attestation (live API call)
  found: Returns HTTP 200 with valid TinfoilAttestationDoc: format="https://tinfoil.sh/predicate/sev-snp-guest/v2", body= base64+gzip SNP report. This is the correct attestation source for PPQ.AI.
  implication: PPQ.AI attestation should point at Tinfoil's attestation URL, not PPQ.AI's base URL.

- timestamp: 2026-03-26
  checked: spawn_health_check() in rust/src/lib.rs lines 775-824
  found: No per-backend model filtering. Fetches raw /models JSON, extracts all model IDs from data[].id, persists them all. No mechanism to filter by prefix for PPQ.AI.
  implication: Fix needs to add filtering in spawn_health_check OR in the HealthCheckResult handler for the ppq-ai backend.

- timestamp: 2026-03-26T00:10:00Z
  checked: startup flow (lib.rs ~1700-1916) for health check spawning
  found: On startup, only an attestation task is spawned (lines 1907-1916). No health check is spawned at startup for any backend. The model_list is read directly from SQLite at line 1719 (and again in reload_backends at line 730) without any per-backend filtering. The previous session's spawn_health_check filter is correct, but the stale 331-model row from before the fix is never overwritten because no health check runs automatically on app launch.
  implication: filter must also be applied at read time in reload_backends() and in the startup backend-loading code so stale SQLite data is always filtered, regardless of when health check last ran.

- timestamp: 2026-03-26T00:10:00Z
  checked: ChatTopBar model picker in ChatScreen.kt lines 261-313
  found: availableModels comes from state.backends.firstOrNull { it.id == backendId }?.models — only the active backend's models are shown. Each DropdownMenuItem shows only shortModelName(modelId) with no provider label. The BackendSummary struct has a `name` field but it is not shown. The additional issue requires provider name to be shown per model row.
  implication: model picker needs to show provider name alongside model name. Since all models in the dropdown come from the same active backend, adding a subtitle or secondary text showing the backend name suffices.

## Resolution
<!-- OVERWRITE as understanding evolves -->

root_cause:
  Bug 1 (attestation): attestation_tinfoil_tdx() derives the attestation URL from the backend's base_url (api.ppq.ai), but PPQ.AI's private models run on Tinfoil infrastructure (inference.tinfoil.sh). The correct attestation endpoint is Tinfoil's, not PPQ.AI's.
  Bug 2 (model list — original fix insufficient): spawn_health_check() filter was added in previous session but there is no startup health check. SQLite model_list for ppq-ai still held the stale 331-model list from before the fix. reload_backends() and the startup backend loading path both read model_list from SQLite without filtering, so the 331 models were served to the UI every launch regardless. The fix needed to apply at read time, not just at health-check time.
  Bug 3 (new — model picker provider label): ChatTopBar model picker showed only shortModelName(modelId) per row with no indication of which provider supplies each model.

fix:
  Bug 1 (attestation): In run_attestation() in rust/src/attestation/task.rs, added attestation_base_url override: when backend_id=="ppq-ai", use "https://inference.tinfoil.sh/v1/" instead of the PPQ.AI base_url.
  Bug 2 (model list): Added filter_models_for_backend() helper in rust/src/lib.rs. Called at read time in both reload_backends() and the startup backend loading path. For backend_id=="ppq-ai", filters model list to only those starting with "private/". This applies to stale SQLite data immediately on every app launch, independent of when a health check last ran.
  Bug 3 (provider label): In ChatScreen.kt ChatTopBar, extracted activeBackend to get both models and name. Each DropdownMenuItem now shows a Column with model name (bodyMedium, bold if selected) and provider name (labelSmall, onSurfaceVariant color) as subtitle.

verification: cargo build succeeds; Android APK builds and installs; awaiting user confirmation
files_changed:
  - rust/src/attestation/task.rs
  - rust/src/lib.rs
  - android/app/src/main/java/com/example/confidentialapp/ui/ChatScreen.kt

## Bulk Re-Verification (2026-07-28)

**Verdict:** SUPERSEDED
**Evidence:** filter_models_for_backend lib.rs:2062-2070; attestation redesigned in ppq_private.rs
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
