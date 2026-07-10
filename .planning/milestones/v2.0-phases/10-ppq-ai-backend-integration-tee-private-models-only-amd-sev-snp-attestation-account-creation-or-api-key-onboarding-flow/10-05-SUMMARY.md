---
phase: 10
plan: 05
type: summary
status: complete
commits:
  - (implementation completed 2026-03-26, pre-GSD workflow)
---

# Plan 10-05 — UI Integration Across All Platforms

## What shipped

Added PPQ.AI provider links and AmdSevSnp TEE type support to Android, iOS, and Desktop onboarding and settings screens.

### Android integration

Updated `android/app/src/main/java/dev/disobey/mango/ui/OnboardingScreen.kt`:
- Added ppq.ai tappable link to onboarding no-key state
- Link opens https://ppq.ai in browser via Intent.ACTION_VIEW
- Display text "ppq.ai" with primary color

Updated `android/app/src/main/java/dev/disobey/mango/ui/SettingsProvidersScreen.kt`:
- Added "AmdSevSnp" to TEE type picker options list
- Added teeTypeLabel() mapping: TeeType.AMD_SEV_SNP → "AMD SEV-SNP"
- Updated parseTeeType() to handle "AmdSevSnp" string → TeeType.AMD_SEV_SNP

### iOS integration

Updated `ios/Mango/Mango/OnboardingView.swift`:
- Added ppq.ai Link to onboarding no-key state
- Link opens https://ppq.ai via URL
- Display text "ppq.ai"

Updated `ios/Mango/Mango/SettingsProvidersView.swift`:
- Added "AMD SEV-SNP" Text with tag "AmdSevSnp" to TEE type picker
- Added .amdSevSnp case to teeTypeLabel() returning "AMD SEV-SNP"
- Added "AmdSevSnp" case to teeType(from:) returning .amdSevSnp

Updated `ios/Mango/Mango/OnboardingView.swift`:
- Added .amdSevSnp case to teeTypeLabel() returning "AMD SEV-SNP"

### Desktop integration

Updated `desktop/iced/src/views/onboarding.rs`:
- Added ppq.ai button to onboarding no-key state
- Button triggers Message::OpenUrl("https://ppq.ai")
- Display text "ppq.ai" with accent color

### UniFFI bindings

Regenerated UniFFI bindings:
- Ran `just bindings-swift` for iOS
- Ran `just bindings-kotlin` for Android
- Verified AmdSevSnp appears in both binding files

## Build sweep

Android build: green
iOS build: green
Desktop build: green

## Deviations from plan

None.

## Out of scope (handed off)

- Additional tests → Plan 10-06
