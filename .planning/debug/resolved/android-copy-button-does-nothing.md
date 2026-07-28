---
status: resolved
trigger: "The copy button shown under chat messages does nothing on Android — tapping it produces no feedback and the clipboard is not updated."
created: 2026-04-21
updated: 2026-04-21
---

## Current Focus

hypothesis: Copy button onClick is a no-op or not wired to Android ClipboardManager
test: Read MessageBubble.kt to locate copy button and inspect handler
expecting: Either empty lambda, missing ClipboardManager call, or incorrect text source
next_action: Read MessageBubble.kt fully

## Symptoms

expected: Tapping the copy button under a chat message copies the message text to the Android clipboard (and ideally shows brief feedback).
actual: Nothing happens. No visible feedback, clipboard unchanged.
errors: None reported by user.
reproduction: Open a chat, tap the copy button under any message on Android.
started: Not specified.

## Eliminated

## Evidence

- checked: android/app/src/main/java/dev/disobey/mango/ui/MessageBubble.kt
  found: Copy TextButton exists (lines 159, 231), onClick = onCopy lambda passed in. Button UI is correctly wired.
- checked: android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt
  found: ChatScreen exposes onCopy: (String) -> Unit, passes `{ onCopy(message.content) }` to MessageBubble. Correctly forwards content.
- checked: android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt line 68
  found: `onCopy = { _ -> }` — EMPTY NO-OP. Content is discarded; nothing ever writes to clipboard.
- checked: ios/Mango/Mango/ContentView.swift line 65
  found: iOS has same bug `onCopy: { _ in }` — but out of scope (user reported Android only).
- checked: desktop/iced/src/main.rs line 798
  found: Desktop uses iced::clipboard::write(content) — fully implemented.
- checked: android/app/build.gradle.kts
  found: minSdk=28, targetSdk=36. Android 13+ (API 33+) shows OS-level "copied" UI automatically; <33 needs manual Toast for feedback.

## Resolution

root_cause: In MainApp.kt, the `onCopy` callback passed to ChatScreen is an empty lambda `{ _ -> }`. The button UI is correctly wired from MessageBubble → ChatScreen → MainApp, but the top-level handler discards the content without writing to the Android system clipboard, so taps produce nothing.
fix: Implement onCopy in MainApp.kt using ClipMan via LocalContext: obtain android.content.ClipboardManager, write ClipData.newPlainText, and show a Toast on API<33 for user feedback (API 33+ displays system-level confirmation automatically).
verification: Kotlin compile passed (./gradlew :app:compileDebugKotlin — BUILD SUCCESSFUL, only pre-existing deprecation warnings unrelated to this change). Awaiting on-device confirmation.
files_changed:
  - android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** MainApp.kt:108-121 onCopy via clipboard.setPrimaryClip(ClipData)
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
