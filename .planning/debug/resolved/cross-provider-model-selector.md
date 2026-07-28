---
status: resolved
trigger: "Two related model selector issues — (1) model picker only shows models from the currently active provider instead of all healthy providers; (2) the default model selector in Settings lacks the provider name annotation added to the chat model picker."
created: 2026-03-26T00:00:00Z
updated: 2026-03-26T00:00:00Z
---

## Current Focus
<!-- OVERWRITE on each update - reflects NOW -->

hypothesis: CONFIRMED (both bugs) — (1) ChatTopBar computes availableModels from the single activeBackend only; (2) SettingsScreen allModels flatMaps all backends but stores raw model IDs in DropdownMenuItems with no provider annotation
test: applying fix — refactor ChatTopBar to iterate all healthy backends, SettingsScreen to show provider name subtitle per model row
expecting: both pickers show models from all healthy backends with provider annotation
next_action: fix ChatScreen.kt and SettingsScreen.kt

## Symptoms
<!-- Written during gathering, then IMMUTABLE -->

expected: When multiple providers are configured and healthy, the model selector (both in chat and in Settings default model picker) should show TEE models from ALL healthy providers, each annotated with the provider name so the user knows which backend will handle the request.
actual: Model selector appears to only show models scoped to the current/single provider. Settings default model selector has no provider annotation on model rows.
errors: No crash — purely a UX/data scoping issue
reproduction: Configure 2+ providers (e.g. Tinfoil + PPQ.AI), both healthy. Open model selector in chat — only one provider's models appear. Open Settings default model — rows have no provider label.
started: Per-provider annotation was added to ChatScreen in ppq-attestation-and-model-filtering session. Aggregation across providers was never implemented.

## Eliminated
<!-- APPEND only - prevents re-investigating -->

## Evidence
<!-- APPEND only - facts discovered -->

- timestamp: 2026-03-26
  checked: ChatScreen.kt ChatTopBar (lines 262-265)
  found: availableModels = activeBackend?.models ?: emptyList() — scoped to exactly one backend (the active one). Only that backend's models appear in the chat model picker dropdown.
  implication: Bug 1 — need to iterate ALL backends where health_status == Healthy (or Degraded) and aggregate their models into a flat list of (modelId, backendName) pairs.

- timestamp: 2026-03-26
  checked: SettingsScreen.kt lines 85, 307-315
  found: allModels = appState.backends.flatMap { it.models }.distinct().sorted() — this does aggregate across all backends, but the DropdownMenuItem renders Text(model) with no provider annotation. Also .distinct() loses which backend a model comes from after deduplication.
  implication: Bug 2 — Settings has aggregation (good) but no provider label. Fix: derive a list of (modelId, providerName) pairs without .distinct() dedup, then show provider as subtitle (same pattern used in ChatTopBar fix from ppq-attestation-and-model-filtering session).

- timestamp: 2026-03-26
  checked: BackendSummary struct (llm/backend.rs lines 40-60) and HealthStatus enum
  found: BackendSummary has id, name, models, health_status fields. HealthStatus has Healthy, Degraded, Failed, Unknown variants.
  implication: Can filter to backends where health_status == Healthy or Degraded to build the cross-provider model list. The name field provides the provider label.

- timestamp: 2026-03-26
  checked: SetDefaultModel action (lib.rs line 372, 2460-2462)
  found: SetDefaultModel stores only model_id string in SQLite settings key "default_model_id". No backend_id is stored alongside the model.
  implication: When routing a new conversation with the default model, the code at lib.rs ~2089 fetches default_model_id then searches backends for one that has that model. This means model IDs must remain unique across backends, OR routing code selects the first backend that has the model. Need to verify routing logic but this is not the bug being fixed here — the selector UX bugs are purely in the Kotlin UI.

## Resolution
<!-- OVERWRITE as understanding evolves -->

root_cause:
  Bug 1 (ChatScreen model picker scoped to active provider): ChatTopBar computed availableModels = activeBackend?.models — only the single active backend's models. No iteration across other healthy backends.
  Bug 2 (Settings no provider annotation): SettingsScreen built allModels = appState.backends.flatMap { it.models }.distinct().sorted() which did aggregate across providers but lost the backend name association after .distinct(). Each DropdownMenuItem rendered Text(model) with no provider label.

fix:
  Bug 1: Replaced availableModels (List<String> from single active backend) with availableModelEntries (List<Pair<String,String>>) built by filtering all backends where healthStatus != FAILED, then flatMapping to (modelId, backend.name) pairs. Added HealthStatus import. Each dropdown item now shows model name (bold if selected) + provider name as labelSmall subtitle, unconditionally (no isEmpty guard needed).
  Bug 2: Replaced allModels (List<String>) with allModelEntries (List<Pair<String,String>>) using identical aggregation pattern, sorted by modelId. Updated empty-state check to allModelEntries.isEmpty(). Each DropdownMenuItem now shows Column with modelId (bold if selected) + backendName as labelSmall subtitle.

verification: cargo build --release succeeds; cargo ndk arm64-v8a build succeeds; ./gradlew :app:assembleDebug BUILD SUCCESSFUL; adb install -r Success. Awaiting user confirmation that both pickers show cross-provider models with provider annotations.

files_changed:
  - android/app/src/main/java/com/example/confidentialapp/ui/ChatScreen.kt
  - android/app/src/main/java/com/example/confidentialapp/ui/SettingsScreen.kt

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** ChatScreen.kt:533-535 aggregates across healthy backends
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
