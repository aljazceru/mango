---
status: partial
phase: 31-multimodal-image-attachments-across-all-platforms-extend-rus
source: [31-VERIFICATION.md]
started: 2026-04-19
updated: 2026-04-19
---

## Current Test

[awaiting human testing]

## Tests

### 1. Android: Take Photo → vision model
expected: Model describes photographed image; compose-bar pill shows filename with photo icon; no crash
result: [pending]

### 2. Android: Choose Photo (gallery) → vision model
expected: Gallery image copied to cacheDir; multipart request succeeds; model describes image
result: [pending]

### 3. Android: Attach File (regression)
expected: Text attachment pill appears; send succeeds with text augmentation
result: [pending]

### 4. iOS: Take Photo (device) → vision model
expected: NSCameraUsageDescription prompt; JPEG written to temp; model describes image
result: [pending]

### 5. iOS: Choose Photo (PhotosPicker) → vision model
expected: PhotosPicker opens with NSPhotoLibraryUsageDescription; JPEG written; multipart succeeds
result: [pending]

### 6. iOS: simulator build
expected: xcodebuild Debug iphonesimulator exits 0; ImagePickerView.swift auto-picked by XcodeGen
result: [pending]

### 7. Desktop: pick .jpg → vision model
expected: Compose-bar pill shows '[image] filename'; multipart request succeeds
result: [pending]

### 8. Desktop: pick .txt (regression)
expected: Text-file AttachFile path unchanged; text augmentation present
result: [pending]

## Summary

total: 8
passed: 0
issues: 0
pending: 8
skipped: 0
blocked: 0

## Gaps
