---
status: testing
phase: 29-wire-vectorindex-dek
source: [29-01-SUMMARY.md]
started: 2026-04-09T12:00:00Z
updated: 2026-04-09T12:00:00Z
---

## Current Test

number: 1
name: Encrypted startup defers vector index
expected: |
  With authentication enabled (PIN/biometric set up), launch the app fresh.
  The app boots to the lock screen without crash or error.
  No RAG or embedding operations run before unlock.
awaiting: user response

## Tests

### 1. Encrypted startup defers vector index
expected: With auth enabled, app boots to lock screen without crash. No unencrypted vector index files written before unlock.
result: [pending]

### 2. Unlock with PIN enables RAG search
expected: After unlocking with PIN, add or search a document via RAG. Embedding and search complete successfully — VectorIndex is open with the DEK.
result: [pending]

### 3. Lock clears DEK and resets vector index
expected: After locking the app (via timeout or manual lock), attempt to use RAG is not possible until re-unlock. The vector index is reset.
result: [pending]

### 4. Re-unlock restores previously indexed documents
expected: After lock → unlock cycle, documents that were embedded before locking are still searchable. The encrypted usearch index file is decrypted with the DEK on re-unlock.
result: [pending]

### 5. Pre-encryption backward compatibility
expected: On a fresh install with NO PIN/biometric set up, RAG document embedding and search work normally. The app uses unencrypted vector index (None DEK path) without errors.
result: [pending]

## Summary

total: 5
passed: 0
issues: 0
pending: 5
skipped: 0
blocked: 0

## Gaps

[none yet]
