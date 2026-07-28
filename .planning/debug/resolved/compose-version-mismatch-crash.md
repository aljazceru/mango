---
status: resolved
trigger: "App crashes on render of LLM streaming response with NoSuchMethodError in BasicText"
created: 2026-03-26T00:00:00Z
updated: 2026-03-26T00:01:00Z
---

## Current Focus

hypothesis: CONFIRMED — mikepenz 0.35.0 compiled against Foundation 1.8.2 (verified via com.mikepenz:version-catalog:0.3.9 toml); BOM 2025.01.01 ships Foundation 1.7.x → NoSuchMethodError at runtime
test: Applied fix — upgraded BOM to 2025.04.01 (verified it ships Foundation 1.8.0 via maven POM); activity-compose 1.9.0 and lifecycle-runtime-ktx 2.8.3 unchanged (not BOM-managed, remain compatible)
expecting: App builds and markdown renders without crash
next_action: Await human verification of build and runtime

## Symptoms

expected: LLM response renders in the chat view using markdown
actual: App crashes immediately when a streaming response arrives
errors: |
  java.lang.NoSuchMethodError: No static method BasicText-CL7eQgs(
    ...TextAutoSize;...
  ) in class Landroidx/compose/foundation/text/BasicTextKt

  at com.mikepenz.markdown.compose.elements.material.TextWrapperKt.MarkdownBasicText-eIOHA4g(TextWrapper.kt:102)
  at com.mikepenz.markdown.compose.elements.MarkdownTextKt.MarkdownText(...)
  at com.example.confidentialapp.ui.MessageBubbleKt.StreamingMessageBubble(MessageBubble.kt:213)
reproduction: Send any message to the LLM in the Android app — crashes when the first streaming token arrives and the message bubble tries to render markdown
started: New — appeared alongside other recent fixes (network permission, hickory-dns)

## Eliminated

(none yet)

## Evidence

- timestamp: 2026-03-26T00:00:00Z
  checked: android/app/build.gradle.kts
  found: composeBom = "androidx.compose:compose-bom:2025.01.01"; mikepenz 0.35.0 for both markdown renderer deps; activity-compose 1.9.0; lifecycle-runtime-ktx 2.8.3
  implication: BOM 2025.01.01 ships Compose Foundation 1.7.x. mikepenz 0.35.0 compiled against Foundation 1.8.0+ which introduced TextAutoSize parameter in BasicText — runtime signature mismatch.

- timestamp: 2026-03-26T00:01:00Z
  checked: BOM POMs for 2025.03.01, 2025.04.01 via dl.google.com maven; mikepenz version-catalog 0.3.9 via Maven Central
  found: BOM 2025.01.01 → Foundation 1.7.x; BOM 2025.03.01 → Foundation 1.7.8; BOM 2025.04.01 → Foundation 1.8.0; mikepenz version-catalog 0.3.9 (used by v0.35.0) declares compose = "1.8.2"
  implication: 2025.04.01 is the minimum BOM that ships Foundation 1.8.x, satisfying mikepenz 0.35.0's compile-time requirement. activity-compose and lifecycle-runtime-ktx are not managed by the Compose BOM — pinned values (1.9.0 / 2.8.3) remain valid.

- timestamp: 2026-03-26T00:01:00Z
  checked: Kotlin compiler plugin version in android/build.gradle.kts
  found: kotlin.android and kotlin.plugin.compose both at 2.0.21; Foundation 1.8.0 POM only requires kotlin-stdlib, no minimum Kotlin version pin
  implication: Kotlin 2.0.21 is fully compatible with Compose Foundation 1.8.x. No Kotlin upgrade needed.

## Resolution

root_cause: Compose BOM 2025.01.01 resolves to Foundation 1.7.x. mikepenz:multiplatform-markdown-renderer 0.35.0 was compiled against Foundation 1.8.2 (confirmed via com.mikepenz:version-catalog:0.3.9 — compose = "1.8.2"). Foundation 1.8.0 added a TextAutoSize parameter to BasicText. At runtime the APK contains Foundation 1.7.x (no TextAutoSize) but the pre-compiled mikepenz bytecode calls the 1.8.x method signature, causing NoSuchMethodError.
fix: Upgraded Compose BOM from 2025.01.01 to 2025.04.01 in android/app/build.gradle.kts. BOM 2025.04.01 resolves to Foundation 1.8.0 (verified via dl.google.com maven POM). activity-compose and lifecycle-runtime-ktx are not managed by the Compose BOM and require no changes (1.9.0 and 2.8.3 remain valid with Foundation 1.8.x).
verification: awaiting human build and runtime verification
files_changed:
  - android/app/build.gradle.kts

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** Compose BOM now 2026.03.00 (android/app/build.gradle.kts:183) — far exceeds the 2025.04.01 minimum the fix proposed.
**Verified by:** /gsd-debug bulk re-check vs current HEAD
