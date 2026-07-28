---
status: resolved

trigger: "Chat LazyColumn doesn't scroll to the latest message — user sees stale messages while waiting for a reply, and when the reply arrives the scroll position doesn't reach the bottom."
created: 2026-03-26T00:00:00Z
updated: 2026-03-26T00:00:00Z
---

## Current Focus
<!-- OVERWRITE on each update - reflects NOW -->

hypothesis: CONFIRMED (revised). The index-arithmetic approach was still wrong. Two new root causes identified from human verification:

1. **Jitter**: `animateScrollToItem` is an animated scroll. During streaming, state changes on every token, so the LaunchedEffect fires repeatedly. Each call kicks off a new animated scroll to the "bottom", which visually fights the previous one — this is the jitter.

2. **Mid-conversation landing**: `scrollToItem(totalItems - 1)` requires totalItems to exactly match the LazyColumn's rendered item count. The `extraItems` calculation is still a parallel reimplementation. The robust idiom is `scrollToItem(Int.MAX_VALUE)` — Compose LazyColumn clamps this to the actual last item index without any arithmetic. No parallel computation, no drift.

Fix strategy:
- Replace `animateScrollToItem(totalItems - 1)` with `scrollToItem(Int.MAX_VALUE)` (instant, clamped)
- Keep the same LaunchedEffect keys (messages.size, streamingText, busyState, lastError)
- Remove ALL index arithmetic — no extraItems, no totalItems calculation

next_action: Apply revised fix to ChatScreen.kt

## Symptoms
<!-- Written during gathering, then IMMUTABLE -->

expected: |
  - While waiting for a reply (thinking dots showing), the list should be scrolled to the bottom so the thinking indicator is visible
  - As streaming tokens arrive, the list should stay pinned to the bottom so the user always sees the latest text
  - When a new user message is sent, the list should immediately scroll to show it
actual: |
  - With multiple messages in the chat, the scroll position doesn't update correctly when new items appear (thinking indicator, streaming bubble)
  - After a reply arrives the user has to manually scroll down to see it
  - The latest message is not displayed — an older message is visible instead
errors: none — purely a scroll positioning bug
reproduction: |
  1. Open a conversation with several existing messages
  2. Send a new message
  3. Observe: thinking dots may not be visible, or scroll doesn't follow the streaming reply
timeline: Introduced when ThinkingIndicatorBubble and streaming item were added to the LazyColumn
platform: Android (Jetpack Compose LazyColumn)

## Eliminated
<!-- APPEND only - prevents re-investigating -->

## Evidence
<!-- APPEND only - facts discovered -->

- timestamp: 2026-03-26T00:02:00Z
  checked: Human verification response after first fix (index arithmetic extraItems approach)
  found: |
    Still broken. Jitter observed. Scroll lands mid-conversation, not at bottom.
  implication: |
    The index arithmetic approach is fundamentally wrong regardless of whether the formula is correct.
    1. animateScrollToItem fires on every streamingText change (every token). Competing animated scrolls = jitter.
    2. Any parallel item-count formula will drift from the LazyColumn's actual rendered count. The robust Compose idiom is scrollToItem(Int.MAX_VALUE) — clamped by LazyColumn to actual last index, instant scroll avoids animation competition.

- timestamp: 2026-03-26T00:01:00Z
  checked: ChatScreen.kt LaunchedEffect (line 81-92) and LazyColumn item layout (lines 166-200)
  found: |
    LazyColumn renders: N message items (indices 0..N-1) + optional thinking item (index N) + optional streaming item (index N) + optional error item (index N or N+1 depending on what else is showing).
    LaunchedEffect computes: target = messageCount + (if hasStreaming || hasThinking) 0 else -1).
    Cases:
      - Idle, N messages: target = N-1 = correct
      - Loading/no text: target = N = correct (thinking item is at index N)
      - Streaming text: target = N = correct (streaming item is at index N)
      - Error shown (Idle): target = N-1 but error bubble is at index N = OFF BY ONE — scroll stops one short of the error item
    Additionally the variable `allItems` (line 154-156) is constructed but never used; the LazyColumn uses `items(state.messages)` directly. Dead code.
  implication: |
    Primary scroll bug: when an error bubble is shown, the list scrolls to the last message rather than the error item. The user does not see the error without scrolling.
    Secondary fragility: the scroll target formula is a parallel reimplementation of the LazyColumn item count. Any future item addition (e.g. a footer) that is not mirrored in the LaunchedEffect formula will silently break auto-scroll again.

## Resolution
<!-- OVERWRITE as understanding evolves -->

root_cause: |
  Two root causes after revised analysis (human verification confirmed first fix still broken):

  1. animateScrollToItem causes jitter during streaming. The LaunchedEffect fires on every token (streamingText changes each token). Each call kicks off a new animated scroll, which visually fights the previous animation — this produces the "jitter" the user observed.

  2. Index arithmetic `scrollToItem(totalItems - 1)` requires totalItems to match the LazyColumn's actual rendered item count exactly. Even with the corrected extraItems formula, this is a parallel reimplementation that will silently drift any time a new conditional item is added. Caused the "mid-conversation landing" when the count was wrong in any edge case.

fix: |
  Replaced both the animated scroll and the index arithmetic with the standard Compose idiom:
    listState.scrollToItem(Int.MAX_VALUE)
  LazyColumn clamps Int.MAX_VALUE to the actual last rendered item index — no arithmetic, no drift, no parallel item count needed. scrollToItem (instant) prevents jitter from competing animations during streaming.

verification: pending human verification
files_changed:
  - android/app/src/main/java/com/example/confidentialapp/ui/ChatScreen.kt

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** ChatScreen.kt:166-189 reverse-layout, scrollToItem(0)
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
