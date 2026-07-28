# Bulk Re-Verification Audit — 2026-07-28

**Trigger:** `/gsd-debug` bulk re-verify of 20 `awaiting_human_verify` sessions.
**Method:** Read each debug file, cross-referenced fix targets against current HEAD (post-v2.0 + local-LLM work).
**Verifier:** explore subagent (very thorough) + orchestrator review.

## Summary

| Verdict | Count |
|---------|-------|
| FIXED-IN-CODE | 16 |
| SUPERSEDED | 4 |
| STILL-OPEN | 0 |
| AMBIGUOUS | 0 |

**All 20 sessions archived.** The v2.0 milestone (Phases 20-36) and post-archive local-LLM work (Phases 37-38) absorbed or rewrote every fix described in the original March/April 2026 sessions.

## Detail

| # | Slug | Verdict | Evidence |
|---|------|---------|----------|
| 1 | chat-completion-empty-body | FIXED-IN-CODE | lib.rs:8010-8022 keychain.store() before guard, comment cites this bug |
| 2 | redpill-attestation-tinfoil-auth | SUPERSEDED | settings rewritten to settings_providers.rs, URL placeholder removed |
| 3 | settings-provider-list-display | FIXED-IN-CODE | lib.rs:6176-6198 cache load; iOS SettingsProvidersView.swift:215,315 |
| 4 | periodic-attestation-refresh | FIXED-IN-CODE | SetAttestationInterval lib.rs:789,8420; AttestationTick streaming.rs:73 |
| 5 | cross-provider-model-selector | FIXED-IN-CODE | ChatScreen.kt:533-535 aggregates across healthy backends |
| 6 | chat-autoscroll-not-reaching-latest | FIXED-IN-CODE | ChatScreen.kt:166-189 reverse-layout, scrollToItem(0) |
| 7 | rag-pipeline-silent-failure-android | SUPERSEDED | Phase 11 MobileEmbeddingProvider.kt:79-93 ONNX Runtime replaces stub |
| 8 | ppq-provider-missing-from-ui | FIXED-IN-CODE | has_api_key backend.rs:75,89; MIGRATION_V10 schema.rs:198-208 |
| 9 | tinfoil-onboarding-connect-error | FIXED-IN-CODE | .no_hickory_dns() on all clients (net/tls.rs:129,143 + 11 more) |
| 10 | ppq-attestation-and-model-filtering | SUPERSEDED | filter_models_for_backend lib.rs:2062; attestation redesigned in ppq_private.rs |
| 11 | instructions-not-persisted-in-ui | FIXED-IN-CODE | system_prompt: Option<String> lib.rs:79; iOS ChatView.swift:207 |
| 12 | markdown-not-rendered-during-streaming | SUPERSEDED | deliberately reverted by session markdown-streaming-flicker |
| 13 | onboarding-flash | FIXED-IN-CODE | isReady AppManager.kt:46,217; MainApp.kt:42-45 blank Box guard |
| 14 | markdown-streaming-flicker-and-sizing | FIXED-IN-CODE | MessageBubble.kt:434 Text(), AssistantBubble 250-267 scaled typography |
| 15 | brave-api-key-save-no-feedback | FIXED-IN-CODE | brave_api_key_validating lib.rs:391; ValidateBraveApiKey lib.rs:789,8535 |
| 16 | menu-tweaks-and-brave-search-broken | FIXED-IN-CODE | Tools sub-sheet on all 3 platforms |
| 17 | top-bar-overflow-redesign | FIXED-IN-CODE | iOS confirmationDialog, Android MoreVert, Desktop ... button |
| 18 | android-back-swipe-closes-app | FIXED-IN-CODE | push_nav_history lib.rs:1863; BackHandler MainApp.kt:9,77 |
| 19 | android-copy-button-does-nothing | FIXED-IN-CODE | MainApp.kt:108-121 onCopy via clipboard.setPrimaryClip |
| 20 | image-upload-still-broken-after-fix | FIXED-IN-CODE | run_streaming_chat_completion_from_api_messages tinfoil_secure.rs:304 |

## Cosmetic Follow-ups (NOT original bugs; deferred to backlog)

1. **Android "Documents" vs iOS/Desktop "RAG" label** — `ChatScreen.kt:677` shows "Documents (N)" while other platforms use "RAG (N)". Cosmetic only.
2. **Android standalone AttestationBadge** — `ChatScreen.kt:575` still renders a standalone badge; iOS integrated into ModelPickerView as colored dot. Cosmetic only.
3. **PPQ attestation runtime check** — new `api.ppq.ai/private/attestation` path unverified at runtime in this audit (cannot be done by code inspection). Worth a one-line curl test before final sign-off.
