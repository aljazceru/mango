# Mobile Remediation Plan for Junior Developers
## Scope

This plan is based on code review of the current repository only (no external docs).

Current review scope:
- **Target issue A**: iOS LocalLLM is still exposed on more devices than desired.
- **Target issue B**: iOS resume path can trigger directory sync after locking on timeout.

The plan below is ordered so a junior developer can implement it safely with predictable checkpoints.

---

## 1) Code-only findings (what to fix now)

1. **iOS LocalLLM is not uniformly gated by policy and feature flag**
   - `ios/Mango/Mango/IOSLocalLlmProvider.swift`:
     - `probeCapability()` checks only simulator, ABI (arm64/arm64e), and memory/3 (max = min(totalRam / 3, 1.5GB)).
     - No iOS feature flag equivalent to Android’s `FeatureFlags.LOCAL_LLM_ENABLED`.
     - No free-storage reserve or model-load storage margin check.
   - Result: older iPhones may still receive non-zero `maxModelBytes` and Local model UI can appear.

2. **iOS ScenePhase `.active` can still sync after a lock decision**
   - `ios/Mango/Mango/ContentView.swift`:
     - On `.active`, when timeout elapsed, it dispatches `.lockApp`.
     - It continues immediately to `DirectorySyncScheduler.syncAll(...)`.
   - Result: sync can begin while lock path should be active.

---

## 2) Target architecture decisions

Use one policy gate for LocalLLM capability in iOS, matching Android behavior by intent:
- A **feature flag** can disable LocalLLM globally without changing remote inference.
- Capability checks are strict and deterministic.
- Unsupported devices must always have `maxModelBytes == 0`, so:
  - iOS Settings should hide the on-device model section.
  - Download/start actions are blocked.

Recommended iOS policy baseline:
- Disabled flag => unsupported.
- Simulator => unsupported.
- ABI check `arm64 || arm64e` still required.
- Memory and storage checks required (not optional).
- Keep `maxModelBytes` computed from physical RAM and a cap similar to Android’s 4GB policy intent.

This is safe by default and can be tuned later without changing UI flow.

---

## 3) Workstream A — iOS LocalLLM feature flag and capability policy

### 3.1 File: `ios/Mango/Mango/FeatureFlags.swift`

### Changes
1. Add an iOS feature flag for LocalLLM.
2. Keep the existing `agentsEnabled` untouched.

### Concrete edits
- Add:
  - `static let localLlmEnabled = true`

### Acceptance
- Builds compile with old and new flag values.
- When flag is set `false`, all LocalLLM capability checks downstream treat device as unsupported.

---

### 3.2 File: `ios/Mango/Mango/IOSLocalLlmProvider.swift`

#### 3.2.1 Add policy constants and structured helper (near top of file)

Add a local policy block containing explicit constants and a pure helper for eligibility decisions.

Suggested constants:
- `LOCAL_LLM_MIN_RAM_BYTES` (e.g., 6_000_000_000 or 8_000_000_000 based on your product decision).
- `LOCAL_LLM_STORAGE_RESERVE_BYTES` = 512 * 1024 * 1024
- `LOCAL_LLM_STORAGE_MARGIN_PERCENT` = 25
- `LOCAL_LLM_MAX_CAP_BYTES` = 4_000_000_000

Suggested helper API:
- `private static func availableStorageBytes() -> UInt64`
- `private static func requiredStorageBytes(for modelBytes: UInt64) -> UInt64`
- `private static func isFeatureEnabled() -> Bool`
- `private static func supportedAbi(_ abi: String) -> Bool`
- `private static func isNewPhoneCandidate(totalRam: UInt64, storageBytes: UInt64) -> Bool`

#### 3.2.2 Update `probeCapability()`

Current structure:
- returns unsupported when simulator.
- returns supported for any arm64/arm64e with `maxModelBytes = min(totalRam / 3, 1.5GB)`.

New structure:
1. If `!isFeatureEnabled()`, return capability with:
   - `maxModelBytes = 0`
   - `supportsMmap = false`
   - reason: `"Local LLM disabled by feature flag"`
2. If simulator: unchanged unsupported reason but keep this branch explicit.
3. For non-simulator:
   - compute `totalRam`.
   - compute free storage.
   - check ABI support.
   - check `isNewPhoneCandidate(...)` policy (RAM + storage >= reserve, etc.).
   - compute `maxModelBytes` only when all checks pass:
     - `min(totalRam / 2, LOCAL_LLM_MAX_CAP_BYTES)` or policy-aligned variant.
   - if unsupported, reason should be specific:
     - `"LocalLLM disabled by feature flag"`
     - `"Unsupported ABI: ..."`
     - `"Insufficient RAM for LocalLLM"`
     - `"Insufficient free storage for LocalLLM"`
     - `"Unable to determine device capabilities"` if facts missing.
4. Return `DeviceCapability` with explicit `abi`, `totalRamBytes`, `maxModelBytes`, `supportsMmap`, `reason`.

#### 3.2.3 Update `loadModel(modelPath:)`

Current checks already include:
- file exists,
- `maxModelBytes != 0`,
- model size <= `maxModelBytes`.

Add:
1. Calculate current available storage before loading:
   - `storageBytes = availableStorageBytes()`
   - `requiredBytes = requiredStorageBytes(for: fileSize)`
2. Reject load if `storageBytes < requiredBytes`.
3. Error reason should mention required bytes and available bytes.

This prevents mid-load failures on near-full devices and improves consistency with Android’s margin behavior.

#### 3.2.4 Optional hardening for model download path

`downloadModelFile(...)` currently has no storage pre-check. Add before creating/streaming:
1. Evaluate expected final file size (`Content-Length` if present).
2. If final size unknown, you can:
   - allow download with same post-download `loadModel` guard only, or
   - fail early with explicit guidance.
3. On known-size responses, apply same storage + margin + reserve rule as load path.

This is optional for first pass but recommended for better UX consistency.

#### 3.2.5 New-phone-only behavior interpretation

Answering your question directly:
- **Yes, we can make LocalLLM available only on new phones.**
- In practice, this is achieved by setting the device policy to only pass newer/stronger-memory/stronger-storage devices.
- Devices that do not pass policy have `maxModelBytes = 0`, so UI and actions treat local inference as unavailable.

---

## 4) Workstream B — iOS resume lock and directory sync ordering

### 4.1 File: `ios/Mango/Mango/ContentView.swift`

Current behavior:
- On `.active`, lock is dispatched then sync runs unconditionally if sources exist.

Change plan:
1. In the `.active` branch, compute:
   - `let shouldLock = timeout >= 0 && elapsed >= timeout`.
2. If `shouldLock`:
   - dispatch `.lockApp`
   - set `backgroundedAt = nil`
   - `return` immediately (do not sync in same cycle).
3. If `appManager.appState.router.currentScreen` is `.locked`, **skip** sync.
4. Only run sync when:
   - not locking now,
   - not currently in locked screen,
   - directory sources exist.

This prevents lock-timeout-triggered sync paths from running after lock decision.

### 4.2 File: `ios/Mango/Mango/DirectorySourcesView.swift` (`DirectorySyncScheduler.syncAll`)

Current behavior:
- Always iterates and starts detached sync tasks for cached sources.

Add defensive guard:
1. At scheduler entry, check `appManager.appState.router.currentScreen == .locked`.
2. If locked, log and return.
3. In each loop, optional short-circuit check:
   - skip source if lock state changes while task is scheduling.

This protects against accidental sync dispatches from alternate call sites.

---

## 5) Validation and test plan

## 5.1 iOS-specific checks (manual + automated where possible)

### A. Static/code review checks
- Verify `FeatureFlags.localLlmEnabled` exists and defaults true.
- Verify `probeCapability()` now:
  - returns `maxModelBytes = 0` for flag-off case,
  - simulator,
  - insufficient RAM/ storage,
  - unsupported ABI.
- Verify `loadModel()` rejects when storage margin+reserve cannot be met.
- Verify `ContentView` stops sync when lock condition is triggered in same `.active` transition.
- Verify `DirectorySyncScheduler.syncAll` returns immediately when locked.

### B. Scenario matrix (expected behavior)
1. **Flag off**
   - `FeatureFlags.localLlmEnabled = false`
   - `SettingsView` local section should not appear (`maxModelBytes == 0` path).
2. **Simulator**
   - Local model capabilities remain unsupported.
3. **Old/low RAM device (by simulation or mocked policy)**:
   - `capability.maxModelBytes == 0`
   - reason explains unsupported hardware/memory.
4. **High RAM + enough storage device**:
   - capability has non-zero `maxModelBytes`.
   - model list can render if model fits capability.
5. **Resume lock timeout**
   - On timeout path, app lock action dispatches and no sync job starts.

### C. Runtime smoke (if you have CI devices/simulators)
- Start with directory sources configured.
- Set lock timeout to immediate / very small.
- Move app to background, return to foreground.
- Confirm no directory sync action is dispatched when lock timer elapsed.
- Confirm normal foreground resume sync still occurs when lock condition is not met.

### D. Regression command suggestions
- Use existing repo checks already used in previous work:
  - Rust suite command from code review pass:
    - `cargo test -p mango_core --lib security_regressions -- --nocapture`
- Run platform builds/tests relevant to your release pipeline.
  - Android path where unchanged fixes already exist.
  - iOS app build + UITests (if configured in your environment).

---

## 6) Implementation order and handoff strategy

1. Update `FeatureFlags.swift` (fast, low risk).
2. Update `IOSLocalLlmProvider.swift`:
   - constants + helper policy,
   - `probeCapability`,
   - `loadModel` checks,
   - optional download-path storage check.
3. Fix lock/sync ordering in `ContentView.swift`.
4. Add lock guard in `DirectorySourcesView.swift`.
5. Run code validation matrix from section 5 and capture evidence.

## 7) Definition of done

- On unsupported iOS phones, `deviceCapability().maxModelBytes == 0`, `reason` indicates why.
- Old phone / low-memory / storage-restricted / simulator paths never allow LocalLLM UX entry.
- Resume flow does not start directory sync when lock path is selected.
- Existing features unaffected: remote inference, settings and normal sync flows remain functional on supported phones.

---

Appendix: file references used in this plan
- `ios/Mango/Mango/FeatureFlags.swift`
- `ios/Mango/Mango/IOSLocalLlmProvider.swift`
- `ios/Mango/Mango/ContentView.swift`
- `ios/Mango/Mango/DirectorySourcesView.swift`
- `android/app/src/main/java/dev/disobey/mango/FeatureFlags.kt` (for parity behavior pattern)
- `android/app/src/main/java/dev/disobey/mango/AndroidLocalLlmProvider.kt` (for policy precedence pattern)
- `android/app/src/main/java/dev/disobey/mango/MainActivity.kt` (for lock+sync ordering pattern already applied on Android)
