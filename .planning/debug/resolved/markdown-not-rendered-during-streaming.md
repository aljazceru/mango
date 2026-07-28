---
status: resolved
trigger: "markdown-not-rendered-during-streaming"
created: 2026-04-03T00:00:00Z
updated: 2026-04-03T00:01:00Z
---

## Current Focus

hypothesis: CONFIRMED — StreamingMessageBubble uses plain Text() instead of Markdown() during streaming, with an explicit comment justifying it as a performance decision
test: Read MessageBubble.kt lines 206-237
expecting: Plain Text() call during streaming
next_action: Replace Text() with Markdown() in StreamingMessageBubble

## Symptoms

expected: Markdown should render progressively as tokens stream in — the user should see formatted text building up in real-time
actual: During streaming, raw markdown text is shown (e.g., literal **bold**, triple backticks visible). Only when streaming completes does the rendered markdown appear
errors: No errors — it's a rendering behavior issue
reproduction: Send any message that produces a response with markdown formatting. Watch the streaming response — raw markdown visible. Wait for stream to finish — markdown renders
started: Unknown — may have always been this way. User noticed it now

## Eliminated

(none yet)

## Evidence

- timestamp: 2026-04-03T00:00:00Z
  checked: MessageBubble.kt StreamingMessageBubble composable (lines 206-237)
  found: Uses plain androidx.compose.material3.Text() during streaming. Comment at line 218-220 explicitly states "Plain Text during streaming — avoids full markdown AST re-parse on every token. The completed message in state.messages renders with Markdown once streaming ends."
  implication: This is the intentional design that causes the symptom. The fix is to replace Text() with Markdown() in StreamingMessageBubble.

- timestamp: 2026-04-03T00:00:00Z
  checked: ChatScreen.kt message list rendering (lines 185-213)
  found: During streaming, ChatScreen renders a StreamingMessageBubble with state.streamingText. When streaming completes, the finalized message appears in state.messages and is rendered via AssistantBubble which uses Markdown(). Two separate composables — one for in-progress, one for finalized.
  implication: Fix must be applied to StreamingMessageBubble, not AssistantBubble.

- timestamp: 2026-04-03T00:00:00Z
  checked: AssistantBubble composable (lines 124-182)
  found: Uses com.mikepenz.markdown.m3.Markdown() correctly with markdownColor() and markdownTypography(). Markdown import already present in the file.
  implication: Markdown() composable is already imported and configured — can be reused in StreamingMessageBubble with no new dependencies.

## Resolution

root_cause: StreamingMessageBubble in MessageBubble.kt uses plain Text() instead of Markdown() during streaming. The comment in code explicitly documents this as a deliberate performance trade-off ("avoids full markdown AST re-parse on every token"), but the trade-off sacrifices the user-visible formatting experience.
fix: Replace the Text() composable inside StreamingMessageBubble with Markdown(), using the same markdownColor() and markdownTypography() configuration already used by AssistantBubble.
verification: Fix applied — replaced Text() with Markdown() in StreamingMessageBubble. The Markdown composable and its markdownColor()/markdownTypography() helpers were already imported. The semantics modifier (liveRegion = Polite) was preserved for accessibility. Streaming cursor (▋) unchanged. Awaiting human verification in real device streaming session.
files_changed: [android/app/src/main/java/dev/disobey/mango/ui/MessageBubble.kt]

## Bulk Re-Verification (2026-07-28)

**Verdict:** SUPERSEDED
**Evidence:** deliberately reverted by session markdown-streaming-flicker
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
