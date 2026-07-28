---
status: resolved
trigger: "markdown-streaming-flicker-and-sizing"
created: 2026-04-04T00:00:00Z
updated: 2026-04-04T00:00:00Z
---

## Current Focus

hypothesis: CONFIRMED. Two distinct bugs in MessageBubble.kt:
  1. StreamingMessageBubble uses Markdown{} composable — re-parses full markdown AST on every streaming token, causing full recomposition and visual flicker
  2. AssistantBubble uses Markdown{} composable with default markdownTypography() — H1/H2 use full headline sizes unscaled for chat bubble context
test: Verified by reading MessageBubble.kt directly
expecting: Fix = replace Markdown in StreamingMessageBubble with plain Text, and either replace Markdown in AssistantBubble or override typography scale for headings
next_action: Implement fix in MessageBubble.kt

## Symptoms

expected: Chat messages stream smoothly without flickering. Headings sized reasonably within chat bubbles.
actual: Screen flickers during streaming as markdown gets re-parsed. Headings like "Complexity" and "7. Scope Management Tension" render extremely large (full H1/H2 size) inside chat message bubbles.
errors: No crash — just bad UX (flickering + oversized text)
reproduction: Send any message that causes the LLM to respond with markdown headings. Observe flickering during streaming and oversized headings in final render.
started: Recently added — new feature. Previous behavior (before markdown rendering) worked fine.

## Eliminated

(none yet)

## Evidence

- timestamp: 2026-04-04T00:00:00Z
  checked: MessageBubble.kt — StreamingMessageBubble composable (lines 206-235)
  found: Uses Markdown{} composable with full markdown AST parsing on every recomposition. Every streaming token triggers a recompose which re-parses the entire markdown document.
  implication: Root cause of flickering — Markdown library rebuilds the entire parsed tree and redraws all elements on each token arrival.

- timestamp: 2026-04-04T00:00:00Z
  checked: MessageBubble.kt — AssistantBubble composable (lines 124-182)
  found: Uses Markdown{} with default markdownTypography() which uses MaterialTheme.typography headline sizes (displayLarge, headlineLarge etc.) for H1/H2/H3. These are full app-level heading sizes (32sp+), not scaled for chat bubble context.
  implication: Root cause of oversized headings — no custom typography override scales headings down for the bubble context.

- timestamp: 2026-04-04T00:00:00Z
  checked: build.gradle.kts (line 98-99)
  found: multiplatform-markdown-renderer-m3:0.35.0 — this is the mikepenz library providing the Markdown composable
  implication: The markdownTypography() defaults map headings to MaterialTheme.typography display/headline styles.

## Resolution

root_cause: |
  Two issues in MessageBubble.kt:
  1. StreamingMessageBubble (line 218) uses Markdown{} composable — triggers full markdown AST re-parse on every streaming token, causing flicker.
  2. Both StreamingMessageBubble and AssistantBubble use default markdownTypography() which maps H1/H2 to full MaterialTheme display/headline sizes, making headings huge inside chat bubbles.
fix: |
  1. Replace Markdown{} in StreamingMessageBubble with plain Text{} — streaming is in-progress text, markdown rendering only needed for final output.
  2. Override markdownTypography() in AssistantBubble with scaled-down heading styles (bodyLarge/titleSmall/bodyMedium instead of headlineLarge etc.).
verification: Self-verified by reading changed code. StreamingMessageBubble now uses Text{} — no markdown AST parse on each token. AssistantBubble now uses markdownTypography() with h1-h6 overridden to body/title scale sizes with bold weight.
files_changed: [android/app/src/main/java/dev/disobey/mango/ui/MessageBubble.kt]

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** MessageBubble.kt:434 Text(), AssistantBubble 250-267 scaled typography
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
