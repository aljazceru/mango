# Android Remediation Implementation Plan

Status: ready for implementation decomposition

Primary audience: junior Android and Rust developers

Last updated: 2026-07-10

## 1. Purpose

This document turns the Android code-review findings into an ordered implementation plan. A developer who has not participated in the review should be able to select one work package, understand the defect, implement the intended behavior, add the required tests, and know when the package is complete.

The plan is intentionally prescriptive. If implementation reveals that a prescribed API or type name does not fit the surrounding code, preserve the invariant and acceptance criteria rather than forcing the exact spelling.

The final outcome is an Android application that:

- launches safely on every device supported by the Android manifest;
- offers LocalLLM only on devices that can safely execute its optimized native runtime;
- never exposes chat data before encryption enrollment is complete;
- treats lock and wipe as hard security boundaries;
- keeps SQLite state and the vector index consistent;
- accurately describes attestation and privacy guarantees;
- exposes all configured providers for management;
- passes Rust tests, Android unit tests, Android lint, release minification, and the required device smoke matrix.

### 1.1 Starting verification baseline

Before remediation, the observed baseline is:

- Android debug assembly succeeds.
- All 22 Android JVM unit tests pass.
- A deterministic mocked remote chat returns an assistant response on a recent Pixel.
- The application terminates with `SIGILL` during startup on an API-28 ARM64 phone because an optimized LocalLLM dependency is loaded before compatibility is known.
- Android debug lint fails with four errors, including an API-29 `MediaStore.Downloads` reference and API-33 `Cleaner` references in generated bindings.
- The Rust core suite has one stale migration assertion that expects schema version 23 while the implementation is at version 24.

These observations are reproduction targets, not permanent expectations. Work package 0 converts them into durable tests and sanitized artifacts.

## 2. Scope and non-goals

### 2.1 In scope

The following reviewed defects are in scope:

1. LocalLLM native startup crash on older ARM64 phones.
2. Onboarding reaching chat before PIN/encryption setup.
3. Non-atomic PIN/database migration and possible lockout.
4. Lock/wipe races with background and asynchronous events.
5. Sensitive pending state surviving lock or wipe.
6. Broken/destructive message editing UX.
7. Plaintext attachment cache cleanup gaps.
8. ContextVM provider/tool identity collisions.
9. Attestation UI making claims stronger than the verified state.
10. Missing screenshot/recents and clipboard protections.
11. Custom providers being unmanageable after creation.
12. Stale or indefinitely blocking biometric activity integration.
13. Unbounded Android document reads and incomplete file filters.
14. Deleted documents receiving late vectors and edited memories retaining stale vectors.
15. Ignored persistence errors that let UI state diverge from storage.
16. Android lint failures, stale tests, and weak smoke-test assertions.

### 2.2 Non-goals

- Redesigning the visual language of the application.
- Changing the remote provider protocol unless required for correctness.
- Adding LocalLLM to 32-bit devices.
- Supporting x86_64 release devices.
- Replacing SQLCipher, UniFFI, the Rust actor, or Compose.
- Making every local model run on every otherwise eligible phone. Model-specific RAM limits remain necessary.

## 3. Mandatory engineering rules

Every pull request produced from this plan must follow these rules:

1. Start by adding or identifying a test that demonstrates the defect.
2. Keep generated UniFFI bindings synchronized with Rust. Do not permanently hand-edit generated Kotlin.
3. After changing an FFI record, enum, action, callback, or error, regenerate Kotlin and Swift bindings and confirm all platforms still compile.
4. Never log prompts, messages, API keys, PINs, DEKs, attachment contents, decrypted filenames, or clipboard contents.
5. Never use a Kotlin `try/catch` as protection against an illegal CPU instruction. `SIGILL` terminates the process before Kotlin can recover.
6. Never load an optimized native library until compatibility has been established without executing that optimized library.
7. Treat lock and wipe as state transitions, not merely navigation changes.
8. Treat every asynchronous completion as untrusted and potentially late.
9. Do not ignore database mutation failures. Either complete the mutation and project it into `AppState`, or report failure and leave `AppState` unchanged.
10. Keep pull requests small enough that one invariant can be reviewed independently.

### 3.1 Repository orientation

Use `rg` to locate the named symbols rather than relying on line numbers. The main implementation areas are:

- `AppManager.kt`: Android construction of platform providers and the Rust FFI application.
- `AndroidLocalLlmProvider.kt`: LocalLLM downloads, passive capability logic, runtime load, model load, and generation.
- `SettingsScreen.kt`: Local inference and model eligibility UI.
- `MainActivity.kt` and `MainApp.kt`: activity lifecycle, lock dispatch, secure-window behavior, clipboard, sharing, and root routing.
- `BiometricProviderImpl.kt`: Android biometric callback bridge.
- `OnboardingScreen.kt`, `ChatScreen.kt`, `DocumentLibraryScreen.kt`, and `SettingsProvidersScreen.kt`: affected Compose feature surfaces.
- Rust `lib.rs`: the actor, `ActorState`, action handlers, internal-event handlers, lock/wipe, onboarding, auth, and feature orchestration.
- Rust `llm/local.rs` and `llm/streaming.rs`: LocalLLM interface and asynchronous event definitions.
- Rust persistence `mod.rs`, `schema.rs`, and `queries.rs`: encryption migration, schema migrations, and ContextVM queries.
- Rust `rag` modules: extraction, vector indexing, and retrieval.
- Rust test modules: actor, auth, persistence, chat, RAG, memory, LocalLLM, attestation, and ContextVM regression coverage.
- `mango_core.kt`: generated UniFFI Kotlin binding. Never make a lasting manual fix here; regenerate it from Rust.
- `justfile`: authoritative native/binding build recipes that must be updated with new CPU-probe and runtime-variant targets.

## 4. Product decisions adopted by this plan

### 4.1 Android support versus LocalLLM support

The application continues to support its existing Android minimum. Remote and custom inference must work on older supported phones.

LocalLLM has a separate, stricter eligibility policy. The initial policy is:

- Android 12 / API 31 or newer;
- a 64-bit process with `arm64-v8a` available;
- at least 8 GiB total physical RAM;
- enough free app-storage space for the selected model plus a 25% safety margin and 512 MiB working reserve;
- every CPU feature required by the shipped optimized llama build;
- the baseline CPU-probe library and selected optimized runtime variant are packaged and loadable.

API level is a product cutoff and a useful first filter, but it is not proof of CPU compatibility. A phone must pass all checks. The thresholds must live in one named policy object so they can be adjusted without duplicating logic in UI code.

On an unsupported device:

- the app launches normally;
- remote/custom providers remain usable;
- LocalLLM is shown as unavailable with a specific reason;
- model downloads cannot start;
- existing downloaded models may be deleted, but not loaded;
- selecting a stale LocalLLM default cannot trap the user in a broken chat.

Add a build-time LocalLLM kill switch and evaluate it before all other eligibility checks. This gives maintainers a simple release-level way to disable the feature while leaving remote inference intact. If the project later adopts a signed runtime configuration system, that system may add a versioned denylist, but this remediation must not introduce an unsigned remote switch.

### 4.2 Confidential-session invariant

For every on-disk install:

> The user must not reach Home or Chat until authentication parameters exist, the database is encrypted, the encrypted database has been reopened successfully, and the encryption key is held only in the unlocked session.

In-memory test databases may use an explicit test-only bypass. Production on-disk paths may not.

### 4.3 Lock/wipe invariant

After lock or wipe begins:

> No operation started in the previous unlocked session may read, write, display, persist, index, restore, or transmit data.

Cancellation improves resource usage, but generation/epoch validation is the correctness boundary. A task can finish after cancellation; its result must still be rejected.

### 4.4 Attestation wording invariant

Only a recent successful verification for the active backend may be called “verified.” Local inference, unknown custom backends, expired evidence, failed verification, and user-bypassed verification must use distinct wording.

## 5. Target LocalLLM architecture

### 5.1 Why the current initialization is unsafe

`AppManager` creates `AndroidLocalLlmProvider` during application startup. Its constructor calculates capability, and the capability probe calls `System.loadLibrary` for the llama runtime. One shipped library contains instructions generated for a newer ARM architecture. Loading it on an older ARM CPU executes an incompatible static initializer and terminates the process with `SIGILL` before the first screen appears.

The fix is not “catch `UnsatisfiedLinkError`.” The incompatible library must never be loaded on that CPU.

### 5.2 Two-stage capability model

Implement two stages:

1. **Passive support evaluation** runs during app initialization. It may read Android version, ABI, process bitness, RAM, storage, and results from a baseline-safe CPU probe. It must not load any optimized llama/ggml library.
2. **Runtime activation** runs only after the user enables LocalLLM or selects a local model. It rechecks eligibility, loads exactly one compatible runtime variant, and reports a recoverable error if loading fails.

Recommended flow:

```text
App start
  -> collect passive device facts
  -> baseline-safe CPU feature probe
  -> evaluate LocalLLM policy
      -> unsupported: inject lazy provider in disabled state; remote app continues
      -> supported: inject lazy provider in available-but-unloaded state

User selects LocalLLM
  -> re-evaluate policy and storage
  -> load compatible optimized runtime once
      -> success: load model and generate
      -> load failure: mark runtime unavailable; show error; remote app continues
```

### 5.3 Baseline-safe CPU probe

Create a very small JNI library dedicated to capability probing. Requirements:

- Compile it for baseline `armv8-a`, without `armv8.4-a`, dot-product, LSE/RCpc, or architecture-specific auto-vectorization.
- Keep it independent from llama, ggml, and `libllama-common`.
- Use `getauxval(AT_HWCAP)` and `getauxval(AT_HWCAP2)` where available.
- Return a bitset or record; do not return an ambiguous single Boolean.
- Include every feature required by the optimized runtime’s compiler flags. At minimum, explicitly account for dot-product and any LSE/RCpc instructions that the compiler may emit.
- If the probe library cannot load, return `RuntimeProbeUnavailable`; do not try the optimized runtime.
- Add a native unit test or instrumentation assertion that records the compiler architecture used for the probe.

Do not parse `/proc/cpuinfo` as the primary compatibility decision. Vendor kernels format it inconsistently. It may be logged in debug builds as non-authoritative diagnostics, without device identifiers.

### 5.4 Capability types

Extend the capability model so code does not infer support from `maxModelBytes > 0`.

Add a UniFFI-compatible status enum similar to:

```text
Supported
UnsupportedAndroidVersion
UnsupportedAbi
UnsupportedProcessBitness
UnsupportedCpuFeatures
InsufficientMemory
InsufficientStorage
ProbeUnavailable
RuntimeNotPackaged
RuntimeLoadFailed
```

Extend `DeviceCapability` with:

- `status`;
- stable `reason_code` suitable for tests;
- human-readable `reason` suitable for UI;
- `android_api_level`;
- `is_64_bit_process`;
- `total_ram_bytes`;
- `available_storage_bytes`;
- `max_model_bytes`;
- CPU feature flags or a non-sensitive feature summary;
- `runtime_variant`, if one has been selected;
- `runtime_loaded`.

Keep `reason` descriptive, but write tests against `status` or `reason_code`, not English strings.

### 5.5 Android class split

Refactor responsibilities as follows:

- `AndroidLocalLlmCapabilityProbe`: gathers passive device facts and invokes only the baseline probe.
- `LocalLlmEligibilityPolicy`: pure Kotlin function that converts facts into a capability result. It must be unit-testable without Android or JNI.
- `AndroidLocalLlmProvider`: remains the UniFFI callback implementation but its constructor performs no optimized native loading.
- `AndroidLlamaEngine`: owns synchronized runtime loading, model loading, generation, and unload.
- `LocalLlmRuntimeLoader`: selects and loads the approved variant only after eligibility passes.

`AppManager` may continue injecting a `LocalLlmProvider` into `FfiApp`, but construction must be safe on unsupported devices. A lazy provider is preferable to switching between a real and null provider because the callback object is fixed when the Rust app is constructed.

### 5.6 Native runtime variants

Choose one of the following before implementing the loader:

- **Recommended initial release:** package one optimized “new phone” variant plus the baseline probe. Unsupported phones use remote inference only.
- **Later expansion:** package baseline and optimized variants under distinct library names, then select one using the safe probe.

For the recommended initial release:

- document the exact compiler flags in the native build task;
- name the runtime variant explicitly, for example `arm64_v84_dotprod`;
- ensure the optimized dependencies all use compatible flags;
- avoid a generic library name that Android may resolve before selection;
- verify with `readelf`/`objdump` as part of the native build that the probe library remains baseline;
- keep `libmango_core.so` compatible with all app-supported ARM64 devices; only LocalLLM-specific libraries may require newer instructions.

Use separate, clean CMake build directories for the baseline probe and optimized runtime. Never reuse the existing llama build cache when changing architecture flags. The build must reject inherited `CMAKE_C_FLAGS`, `CMAKE_CXX_FLAGS`, or environment flags that silently raise the baseline. Inspect generated `compile_commands.json` in CI and fail if the probe or general core contains an optimized `-march` value.

### 5.7 LocalLLM selection and fallback UX

Update Settings behavior:

- Supported and unloaded: “Available on this device.”
- Supported and loaded: “On-device runtime ready.”
- Unsupported: disable the switch and show the specific reason.
- Insufficient RAM for a particular model: disable only that model.
- Insufficient storage: show required and available sizes before download.
- Runtime load failure: disable LocalLLM for the current app version, offer remote-provider selection, and retain model deletion controls.

If the stored active backend is local but the device is now unsupported:

1. Do not attempt to load the model.
2. Select the first healthy configured remote backend if one exists.
3. If none exists, route to provider settings with a clear explanation.
4. Preserve downloaded model files so the user may delete them manually; do not silently delete gigabytes of data.

### 5.8 LocalLLM acceptance tests

Unit tests for the pure policy must cover:

- API 28 + ARM64 + 4 GiB -> unsupported Android version;
- API 31 + 32-bit process -> unsupported bitness;
- API 31 + ARM64 + missing required CPU flag -> unsupported CPU;
- API 31 + ARM64 + required flags + 6 GiB -> insufficient memory;
- fully eligible device + missing probe library -> probe unavailable;
- fully eligible device + missing runtime variant -> runtime not packaged;
- fully eligible device + adequate storage -> supported;
- supported device where a selected model exceeds model/RAM limits -> model unavailable without changing whole-device support.

Instrumentation/device tests must prove:

- API-28 device launches without loading any llama library.
- API-28 device can configure a remote provider and receive an assistant response.
- LocalLLM controls are disabled on API 28 with a reason.
- A supported modern device can download, verify, load, generate, stop generation, unload, and delete a model.
- A forced runtime-load error does not terminate the process.
- Relaunch with a stale local default falls back safely.

## 6. Delivery sequence

Implement the work packages in the order below. Do not start broad UI polish while P0 security and crash work remains incomplete.

| Order | Work package | Priority | Depends on |
|---:|---|---|---|
| 0 | Freeze reproducible baselines | P0 | none |
| 1 | Safe LocalLLM eligibility and lazy native loading | P0 | 0 |
| 2 | Encrypted onboarding gate | P0 | 0 |
| 3 | Crash-safe PIN migration | P0 | 2 |
| 4 | Session epochs, cancellation, and lock/wipe cleanup | P0 | 0 |
| 5 | Message editing contract and UI | P1 | 4 |
| 6 | Attachment ownership and cleanup | P1 | 4 |
| 7 | RAG/memory asynchronous consistency | P1 | 4 |
| 8 | ContextVM composite identity and dispatch aliases | P1 | 4 |
| 9 | Accurate attestation states and wording | P1 | 2 |
| 10 | Screenshot, recents, sharing, and clipboard privacy | P1 | 4 |
| 11 | Custom provider management | P1 | 9 |
| 12 | Lifecycle-safe biometrics | P1 | 3, 4 |
| 13 | Bounded document ingestion and format parity | P1 | 7 |
| 14 | Persistence error propagation | P1 | 2–13 |
| 15 | Lint, generated bindings, smoke tests, and release gate | P0 | all prior packages |

## 7. Work package 0 — Freeze reproducible baselines

### Goal

Make each existing failure reproducible before changing behavior.

### Tasks

1. Record the exact debug and release Gradle commands in the mobile smoke README.
2. Add a checked-in device-test matrix containing API level, ABI, RAM class, and expected LocalLLM availability. Do not include serial numbers or personal device names.
3. Preserve a sanitized API-28 crash signature test artifact containing only process name, signal, library, and backtrace offsets.
4. Add a test helper that can create an `FfiApp`, dispatch internal events, and wait for a target revision without arbitrary sleeps.
5. Add a test-only embedding provider that blocks until released. This will make delete/lock/wipe races deterministic.
6. Add a test-only attestation/health provider or event injector that can deliver a completion after lock.
7. Add a fake LocalLLM provider/loader that counts constructor, probe, runtime-load, model-load, and generation calls.
8. Fix the stale migration assertion so the test checks the current schema version from the code rather than duplicating an obsolete numeric literal where possible.

### Acceptance criteria

- The API-28 startup failure can be reproduced on the pre-fix build.
- The late-event test harness can deliver an event after `LockApp` deterministically.
- The local loader fake proves whether startup attempts native loading.
- The full Rust test suite has no unrelated failure before feature work begins.

## 8. Work package 1 — Safe LocalLLM eligibility and lazy native loading

### Goal

Allow the app to run on older supported phones while making LocalLLM available only on safely eligible modern phones.

### Rust tasks

1. Add the explicit LocalLLM support status to the device-capability record.
2. Replace support decisions based on `max_model_bytes == 0` with status checks.
3. In `SetLocalInferenceEnabled`, reject enabling when the status is not supported and set a recoverable UI error.
4. In `DownloadLocalModel`, recheck device status, model RAM requirement, storage requirement, and download exclusivity.
5. In the local routing path, recheck support immediately before model loading.
6. If an active local backend becomes unsupported, return a typed routing error that lets the actor choose remote fallback rather than treating it as a network failure.
7. Ensure local download progress/completion events carry the active session epoch described in work package 4.

### Android tasks

1. Introduce the passive fact collector and pure eligibility policy described in section 5.
2. Build and package the baseline-safe CPU probe.
3. Remove `missingLocalRuntimeLibraries()` from provider construction.
4. Remove every optimized `System.loadLibrary()` call from startup/probe paths.
5. Move optimized loads behind `AndroidLlamaEngine.ensureInitialized()` and call it only after eligibility passes.
6. Make runtime initialization idempotent and synchronized.
7. Store a process-local terminal failure after runtime loading fails. Subsequent attempts should return the same recoverable failure without repeatedly loading.
8. Persist a non-crashing runtime-load failure keyed by app version/runtime variant so relaunch does not repeatedly attempt a known-broken variant. Clear it on app update or an explicit diagnostic retry.
9. Keep model download/delete UI reachable as defined in section 5.7.
10. Add stable UI reason mapping for every support status.
11. Ensure `AppManager` construction is safe with a null activity and on unsupported hardware.

### Native-build tasks

1. Audit compile flags for every LocalLLM `.so`, including transitive llama-common dependencies.
2. Ensure optimized flags never leak into `libmango_core.so` or the CPU probe.
3. Give incompatible runtime variants distinct filenames.
4. Add a build check that prints and verifies the architecture flags used for each native target.
5. Confirm native dependency loading order for the selected variant.

### Tests

Implement every test in section 5.8. Add a regression test whose core assertion is:

> Constructing `AppManager` and rendering Home/Onboarding invokes zero optimized LocalLLM library loads.

### Definition of done

- The original API-28 phone reaches the first UI screen repeatedly after force-stop and cold launch.
- Remote chat passes on that phone.
- LocalLLM is unavailable there and no llama library appears in its startup backtrace/maps.
- Supported-device LocalLLM generation still works.
- Debug and release builds both contain only intentional ABI/runtime variants.

## 9. Work package 2 — Encrypted onboarding gate

### Goal

Prevent all production on-disk sessions from reaching Home or Chat before PIN setup and database encryption succeed.

### State-machine change

Use this flow:

```text
First launch
  -> onboarding provider selection/attestation
  -> PIN setup
  -> encrypt and reopen database
  -> create first conversation
  -> Home or Chat
```

“Skip onboarding” means skip provider setup, not skip encryption setup.

### Tasks

1. Change `CompleteOnboarding` so it persists completion intent and routes to `PinSetup`; it must not create a conversation or enter Chat.
2. Change `SkipOnboarding` so it also routes to `PinSetup` on an on-disk install.
3. Create the first conversation only after `SetupPin` commits successfully.
4. Add one core helper that determines whether an on-disk install may enter an unlocked application screen. Use it at startup, onboarding completion, PIN completion, and navigation restoration.
5. If onboarding is complete but auth is absent, always route to `PinSetup` immediately, including in the same process.
6. Prevent `PushScreen(Home)` and `PushScreen(Chat)` from bypassing the gate while auth setup is incomplete.
7. Keep the current first-chat placeholder behavior, but set it after successful PIN setup.
8. Update PIN setup copy so it accurately states when encryption will become active.
9. Decide whether the PIN screen permits back navigation. Recommended behavior: back returns to the final onboarding step; it must never enter Home.
10. Preserve a test-only path for `:memory:` apps so core unit tests do not need production key setup unless the test concerns authentication.

### Required tests

- Fresh on-disk install cannot reach Home before `SetupPin` succeeds.
- `CompleteOnboarding` routes to PIN setup and creates no conversation.
- `SkipOnboarding` routes to PIN setup and creates no conversation.
- Back navigation from PIN setup cannot enter Home.
- Process death between onboarding completion and PIN submission returns to PIN setup.
- Successful PIN setup creates exactly one initial conversation.
- Failed PIN setup creates no conversation and does not set the first-chat placeholder.
- Existing encrypted users still unlock and restore their previous screen.

### Definition of done

There is no production navigation path in code or instrumentation tests that shows Home/Chat with `auth_initialized == false` on an on-disk database.

## 10. Work package 3 — Crash-safe PIN and database migration

### Goal

Make encryption enrollment recoverable across every error and process-death boundary.

### Required migration state machine

Split migration into prepare and commit phases. Store a small state in the bootstrap database:

```text
None
Prepared
Committed
```

`Prepared` must contain enough non-secret metadata to recover the operation. Authentication parameters contain the wrapped DEK, not a plaintext DEK.

Recommended sequence:

1. Validate PIN and duress-PIN policy.
2. Generate salt, DEK, KEK, wrapped DEK, and optional duress hash in zeroizing containers.
3. Export the plaintext database to a separately named encrypted temporary database.
4. Set and verify the copied schema version.
5. Open the encrypted copy using the DEK and run integrity checks.
6. Persist bootstrap authentication parameters and migration state `Prepared` transactionally.
7. Fsync the encrypted copy and bootstrap database as supported.
8. Atomically replace the plaintext database with the verified encrypted copy.
9. Reopen the final path with the DEK and verify required tables/settings.
10. Mark bootstrap state `Committed`.
11. Only then expose `auth_initialized`, store optional biometric DEK material, load post-unlock state, and navigate onward.

### Recovery behavior on startup

When bootstrap state is `Prepared`:

- If the final database is encrypted and opens with the wrapped DEK after PIN entry, finish the commit.
- If the final database is plaintext and the verified temporary copy exists, finish the atomic replacement after PIN entry.
- If the final database is plaintext and no valid temporary copy exists, clear the prepared auth record and return to PIN setup without deleting the plaintext database.
- If both files exist but neither validates, stop and show a recoverable storage error. Do not wipe either file automatically.

### Implementation tasks

1. Refactor the persistence migration function into `prepare_encrypted_copy`, `verify_encrypted_copy`, and `commit_encrypted_copy` or equivalent operations.
2. Handle SQLite `-wal` and `-shm` files explicitly before export/replacement.
3. Remove stale temporary encrypted copies before starting a new migration only after validating that they are app-owned migration artifacts.
4. Make bootstrap writes transactional.
5. Never store biometric DEK material until the migration is committed.
6. On any ordinary error, zero temporary keys, close handles, keep the original data, and present a retry action.
7. Make setup idempotent. Re-dispatching after a recoverable failure must not create multiple conversations or overwrite a committed encrypted database.
8. Replace generic toasts with error categories: invalid PIN policy, insufficient storage, export failure, verification failure, commit failure, reopen failure.

### Fault-injection tests

Add test-only hooks at each numbered migration step and simulate failure/process death after every step. For each checkpoint assert:

- the original database or verified encrypted copy remains available;
- no state reports encryption enabled prematurely;
- restart reaches either PIN setup or a recoverable prepared state;
- the correct PIN can complete recovery where applicable;
- a wrong PIN cannot open the encrypted database;
- no raw DEK is persisted in bootstrap storage or logs.

Also test disk-full, rename failure, malformed bootstrap data, stale temporary file, wrong SQLCipher key, and corrupted encrypted copy.

### PIN-attempt controls

1. Enforce the same minimum length and confirmation checks for standard and duress PINs.
2. Reject equal standard and duress PINs.
3. Persist failed-attempt count and last-failure time in bootstrap storage.
4. Apply bounded exponential backoff after failures.
5. Reset counters only after a successful unlock.
6. Keep duress-PIN comparison before normal unwrap, but avoid logging which path matched.

### Definition of done

No single storage failure or process-death checkpoint can transform a readable pre-enrollment database into an install that neither unlocks nor resumes enrollment.

## 11. Work package 4 — Session epochs, cancellation, lock, and wipe

### Goal

Make late background results harmless and clear all sensitive state at the lock boundary.

### Core design

Add two actor fields:

- `session_epoch: u64`, incremented whenever an unlocked session is invalidated;
- `session_cancel: CancellationToken`, replaced after successful unlock.

Every asynchronous task spawned while unlocked captures the current epoch and a child cancellation token. Every completion event carries its captured epoch and, where useful, a unique operation ID.

At the top of each relevant internal-event handler:

1. Compare the event epoch to `actor_state.session_epoch`.
2. If it differs, discard the event without touching DB, vector index, `AppState`, pending state, or UI errors.
3. If the app is locked/wiping or the required DB is absent, discard or return a controlled error; never call `expect("db unlocked")`.

### Events requiring epoch validation

- stream chunks, completion, cancellation, and failure;
- attestation results and timer ticks;
- backend health results;
- local model download progress/completion if wipe removes model data;
- document embedding completion;
- memory extraction completion;
- ContextVM discovery and invocation completion;
- chat tool-call rounds;
- agent step completion;
- biometric results;
- directory sync batches.

### Central cleanup helpers

Create one `invalidate_unlocked_session(reason)` helper used by both lock and wipe. It must:

1. Increment the session epoch.
2. Cancel the root token and all known specialized tokens.
3. Cancel the active stream and attestation timer.
4. Cancel or detach active agent sessions.
5. Clear streaming ownership fields and buffers.
6. Clear failover state.
7. Clear current conversation tool state and ContextVM dispatch maps.
8. Clear attestation TLS pins and expiry maps.
9. Delete app-owned pending plaintext image files.
10. Clear pending file, image, and attested-send snapshots.
11. Clear pending RAG counters and ingestion progress.
12. Clear actor-only backend configurations containing API keys.
13. Drop DB and DEK.
14. Replace the vector index with a locked empty instance.

After successful unlock, reload backend secrets and other actor-only state from protected storage.

### Lock behavior

1. Reject duplicate lock transitions safely.
2. Capture only the non-sensitive screen identifier needed for restoration.
3. Run session invalidation before publishing `Screen::Locked`.
4. Publish a minimal locked `AppState` containing only lock UI fields.
5. Directory foreground sync and WorkManager must test an explicit unlocked state immediately before every dispatch, not just once before traversal.

### Wipe behavior

1. Enter `Wiping` and invalidate the session before deleting data.
2. Delete keychain secrets, bootstrap auth data, database files, vector index, model metadata as intended, attachment caches, and migration temporary files.
3. Preserve only product-approved non-sensitive preferences.
4. Recreate storage and a fresh epoch.
5. Ignore all events from the pre-wipe epoch.
6. If wipe partially fails, remain in a non-chat recovery screen. Do not reopen surviving old data as a new session.

### Android lifecycle tasks

1. Make directory sync stop when state becomes locked while a traversal is in progress.
2. Ensure the periodic worker does not initialize sensitive functionality merely to discover that the app is locked.
3. Make callbacks check epoch/unlocked state at dispatch time.
4. Avoid placing sensitive URI or filename data in WorkManager logs.

### Required race tests

For each operation below: start it, lock or wipe, deliver its completion, and assert no panic, DB write, vector write, stale UI update, or file restoration occurs.

- remote stream;
- attestation;
- health check;
- embedding;
- memory extraction;
- directory sync;
- ContextVM discovery/invocation;
- biometric unlock;
- local model download;
- agent step.

Run each case for both `LockApp` and wipe where applicable.

### Definition of done

The actor contains no `expect("db unlocked")` reachable from an asynchronous completion, and the race suite demonstrates that every pre-boundary event is discarded.

## 12. Work package 5 — Message editing

### Goal

Make “Edit” collect new text explicitly and communicate its history-regeneration behavior.

### Product behavior

- Only user messages are editable.
- Selecting Edit opens a dialog or inline editor prefilled with the original text.
- Save is disabled for blank or unchanged text.
- If later messages exist, the confirmation states that later messages will be removed and a new response generated.
- Cancel changes nothing.
- Confirm replaces the user turn, truncates later turns transactionally, and starts one regeneration.

### Android tasks

1. Replace the immediate `onEdit(message.id, message.content)` dispatch with local editor state.
2. Store message ID, original text, draft text, and whether descendants exist.
3. Add accessible labels for edit, cancel, and confirm controls.
4. Keep the dialog draft across ordinary recomposition; decide whether rotation should preserve it using saveable state.
5. Disable edit while a send, attestation preflight, or stream is active.
6. Do not expose Edit on assistant/tool/system messages.

### Rust tasks

1. Validate that the target exists, belongs to the current conversation, and has user role.
2. Validate new text after trimming without silently changing meaningful internal whitespace.
3. Perform target update and descendant deletion in one SQLite transaction.
4. Do not delete anything until all validation succeeds.
5. Clear/cancel incompatible pending send state before regeneration.
6. If regeneration fails to start, retain the edited message and expose a retry action.
7. Define attachment behavior. Recommended initial behavior: an edited historical message retains its already-persisted attachment metadata; pending compose attachments are unrelated and must not be consumed.

### Tests

- Opening and cancelling Edit dispatches no action.
- Unchanged and blank drafts cannot save.
- Editing a user message truncates only later messages.
- Editing an assistant message is rejected without deletion.
- A missing/cross-conversation ID is rejected.
- A database failure leaves the entire history unchanged.
- One confirmed edit starts exactly one regeneration.

### Definition of done

The Android UI never dispatches an edit until the user supplies changed text and confirms, and the Rust transaction cannot leave a partially truncated conversation.

## 13. Work package 6 — Attachment ownership and cleanup

### Goal

Guarantee that app-owned plaintext attachment caches are deleted when replaced, cleared, sent, locked, or wiped.

### Tasks

1. Add a single `clear_pending_attachments` helper that clears file, image, public attachment summary, and pending attested-send snapshots.
2. Add `replace_pending_file` and `replace_pending_image` helpers. Each must clean the previous app-owned image before installing the new attachment.
3. Make file and image attachments mutually exclusive through those helpers rather than scattered assignments.
4. Use the existing app-ownership path check before deleting any file.
5. Never delete a user-owned source URI/path. Stage images into an app-owned cache before dispatching them to core.
6. Clean staged data on send success, explicit clear, replacement, lock, wipe, and unrecoverable send cancellation.
7. When attestation preflight temporarily snapshots an attachment, define a single owner. Restoring or consuming the snapshot must not double-delete.
8. On process startup, prune orphaned staging files older than a conservative threshold if no durable message references them.

### Tests

- image -> file replacement deletes the staged image;
- image A -> image B deletes A only;
- clear deletes staged image;
- lock and wipe delete staged image;
- rejected non-app-owned path is not deleted;
- attestation retry restores exactly one valid attachment;
- successful send removes plaintext staging after encrypted persistence;
- process-restart cleanup never deletes durable encrypted message data.

### Definition of done

An ownership-focused test proves that every exit path removes app-owned plaintext staging and that no path can delete a user-owned source.

## 14. Work package 7 — RAG and memory asynchronous consistency

### Goal

Keep document/memory rows and vector entries consistent across deletion, editing, lock, wipe, and late completions.

### Document ingestion design

Give each ingestion an `operation_id` in addition to the session epoch. `EmbeddingComplete` must carry both.

Before adding vectors, the handler must verify:

- epoch matches;
- document still exists;
- ingestion operation is still current for that document;
- every chunk row ID still belongs to that document;
- embedding dimensions match the index configuration.

If validation fails, discard the result. Do not report ingestion success.

### Document deletion tasks

1. Mark/cancel the active ingestion operation for the document.
2. Remove vector entries and rows in a defined order with recoverable errors.
3. Prefer a transaction/outbox pattern if vector persistence can fail after SQLite commit.
4. Rebuild or repair the index from SQLite if a partial vector mutation is detected.
5. Ensure a late completion cannot recreate vectors.

### Memory-edit tasks

1. Identify the stable vector IDs belonging to the memory.
2. On edit, compute the new embedding asynchronously with operation ID and epoch.
3. Until completion, either retain the previous text/vector pair or mark the memory as reindexing; never pair new text with an old vector silently.
4. Replace the vector only if the memory still exists and its edit revision matches.
5. On embedding failure, keep the last consistent pair and show a recoverable error.
6. On memory deletion, invalidate any pending edit embedding.

### Consistency repair

Add a debug/test consistency checker that reports:

- vector IDs with no SQLite owner;
- document chunks with no vector;
- memories with missing or stale vector revision;
- dimension mismatch.

Do not log document or memory text.

### Tests

- delete document before embedding completion;
- lock/wipe before embedding completion;
- delete and recreate a document ID before old completion;
- edit memory twice and deliver completions in reverse order;
- delete memory during edit embedding;
- vector write failure after DB mutation;
- index rebuild restores exactly the live SQLite objects.

### Definition of done

The consistency checker is clean after normal operations and every forced race/failure, and stale operation completions are demonstrably no-ops.

## 15. Work package 8 — ContextVM identity and dispatch

### Goal

Prevent providers with equal tool names from overwriting one another or receiving calls intended for another provider.

### Persistence migration

1. Add the next schema migration.
2. Drop the unique index on `tool_name`.
3. Add a unique index on `(provider_pubkey, tool_name)`; retain the composite `id` primary key.
4. Replace name-only lookup with composite lookup.
5. Update discovery upserts to preserve `enabled` only for the same provider/tool pair.
6. Add migration tests with two providers exposing the same name.

### Wire-name aliasing

LLM tool names must remain unique within one request. Introduce a deterministic wire alias such as a sanitized tool name plus a short provider fingerprint. Requirements:

- stable for the same provider/tool pair;
- valid for the target provider’s tool-name constraints;
- collision checked after truncation;
- distinct from built-in local tool names;
- never treated as the human display name.

Build dispatch maps from wire alias to full descriptor. Never dispatch by raw `tool_name` alone.

### Trust tasks

1. Apply trusted-provider filtering to manually persisted, automatically discovered, and conversation-restored tools.
2. Enabling one provider’s tool must not enable another provider’s same-name tool.
3. Persist and display provider identity beside the tool.
4. Record provider identity with tool-use history, not only the raw name.
5. If a legacy history row lacks provider identity, label it unknown rather than guessing.

### Tests

- two providers with `get_weather` coexist in SQLite;
- enabling one leaves the other disabled;
- discovery refresh preserves each independent flag;
- both receive distinct wire aliases;
- dispatch reaches the descriptor associated with the alias;
- a built-in tool-name collision cannot override the built-in;
- untrusted provider tools are excluded in every discovery mode;
- migration preserves existing rows and enabled flags.

### Definition of done

Two providers may safely expose the same human tool name, and every persisted toggle, LLM wire name, dispatch lookup, trust decision, and history record retains the correct provider identity.

## 16. Work package 9 — Accurate attestation and privacy wording

### Goal

Make every trust statement correspond to the actual active route and evidence state.

### State model

Ensure the UI can distinguish:

- local/on-device inference;
- verified remote backend;
- unverified remote backend;
- verification in progress;
- expired verification;
- failed verification;
- backend that does not support attestation;
- user explicitly continued after failure.

Do not collapse “unsupported,” “not attempted,” and “failed” into one state.

### Onboarding tasks

1. Replace the global claim that every message is processed in a TEE with conditional provider-specific wording.
2. Pass the actual attestation outcome into the Ready step.
3. After “Continue anyway,” show a warning-ready state such as “Configured, not verified.”
4. Require an explicit acknowledgement before continuing after failure.
5. Never show a green verified icon after bypass.
6. Explain that choosing a different backend can change trust status.

### Chat/settings tasks

1. Derive the badge from the active route for the current conversation/turn.
2. Show “On device” for LocalLLM, not “Verified TEE.”
3. Show expiry and last verification without implying current validity after expiry.
4. For custom backends with no attestation implementation, show “Attestation unavailable,” not failure or verification.
5. Ensure hybrid routing communicates which remote leg was verified.

### Tests

Create a UI/state table test for every state above. Include onboarding bypass, backend switch, evidence expiry, failed refresh after previous success, local route, and unknown custom backend.

### Definition of done

No screen labels a route verified unless current state contains valid successful evidence for that active remote backend, and bypass/local/unsupported states remain visibly distinct.

## 17. Work package 10 — Android privacy surfaces

### Goal

Prevent accidental exposure through screenshots, recents thumbnails, and clipboard persistence.

### Screenshot and recents tasks

1. Add `FLAG_SECURE` before setting Compose content.
2. Apply it to the whole confidential activity unless product requirements explicitly designate non-sensitive screens.
3. Verify screenshots are blocked and recents thumbnails are obscured on API 28 and a modern Android version.
4. Verify biometric and document-picker transitions still function.

### Clipboard tasks

1. On API 33+, mark copied clips with `ClipDescription.EXTRA_IS_SENSITIVE`.
2. Schedule clipboard clearing after a documented timeout, recommended 60 seconds.
3. Clear only if the clipboard still contains the clip written by Mango. Never erase newer content copied by another app.
4. Cancel/replace the previous clear task when Mango copies a new message.
5. Keep the pre-API-33 confirmation behavior without displaying message text.
6. Consider a setting to disable automatic clipboard clearing only if product explicitly requires it.

### Sharing tasks

1. Keep sharing as an explicit user action.
2. Show a confirmation that the conversation will leave Mango’s protected storage.
3. Do not write the exported Markdown to shared storage unless the user selects a destination.
4. Confirm temporary share data is not retained by Mango.

### Tests

- secure flag is set during activity creation;
- copied clips are marked sensitive on API 33+;
- timeout clears Mango’s unchanged clip;
- timeout preserves a newer external clip;
- copy does not log content;
- share requires explicit confirmation.

### Definition of done

Device checks confirm screenshots and recents capture no app content, clipboard cleanup does not erase newer user data, and sharing always requires an explicit disclosure step.

## 18. Work package 11 — Custom provider management

### Goal

Make every configured custom backend visible and manageable after creation.

### Android tasks

1. Split `AppState.backends` into built-in, custom, and local presentation groups without losing ordering.
2. Render custom provider cards using stable backend IDs.
3. Support edit name, base URL, model, API key replacement, TEE type, and attestation interval where applicable.
4. Support connection test, set default, and remove.
5. Require confirmation before removing the active/default backend.
6. Never prefill an existing API key. Display only whether a key is present.
7. Validate URL and model fields before dispatch.
8. Permit cleartext localhost only in debug builds if needed for smoke tests; require HTTPS in release for non-local hosts.

### Rust tasks

1. Make add/update operations transactional.
2. Ensure backend IDs are collision resistant and never derived solely from display name.
3. When removing the active/default backend, choose a deterministic healthy fallback or clear the default and route to settings.
4. Remove its keychain secret only after the database mutation succeeds, with recovery handling if one side fails.
5. Cancel health/attestation work for removed backends.
6. Ensure custom provider summaries never expose API keys.

### Tests

- add custom provider, navigate away/restart, and find it again;
- edit fields without exposing the old key;
- replace API key;
- set default and send a mocked chat;
- remove non-default and default providers;
- duplicate display names remain independent;
- invalid URL is rejected;
- release rejects insecure remote HTTP;
- removing a provider invalidates its late health/attestation results.

### Definition of done

A custom provider can be created, rediscovered after process restart, edited, made default, used for a mocked chat, and removed without leaving stale secrets or callbacks.

## 19. Work package 12 — Lifecycle-safe biometrics

### Goal

Keep one stable biometric callback object while rebinding it to the current activity and guaranteeing every authentication attempt terminates.

### Android design

1. Replace the provider’s constructor-only activity reference with an atomic/current weak reference.
2. Add `bindActivity(activity)` and `unbindActivity(activity)` methods.
3. Keep the same provider instance injected into `FfiApp`; update only its activity binding.
4. Bind from the current activity lifecycle and unbind only if the destroyed activity is still the bound one.
5. This must work when WorkManager created `AppManager` before any activity.

### Authentication behavior

1. Check activity validity and lifecycle before showing the prompt.
2. Add a finite wait timeout, recommended 60 seconds.
3. On timeout, cancel the prompt on the main thread and return a typed failure.
4. Ensure success, terminal error, cancellation, timeout, and activity destruction each complete the latch exactly once.
5. Do not count ordinary fingerprint mismatch as terminal while the system prompt allows retry.
6. Carry session epoch with the Rust biometric result so a result from an old lock screen cannot unlock a new session.

### Tests

- worker initializes manager before activity, then biometric works after bind;
- activity rotation before prompt;
- activity destruction while prompt is open;
- user cancellation;
- timeout with no callback;
- late success after timeout is ignored;
- result from an old session epoch is ignored;
- no activity returns a prompt-unavailable result without blocking.

### Definition of done

Every authentication attempt terminates within the configured timeout, activity rotation/background initialization remains recoverable, and only a result from the current lock session may unlock.

## 20. Work package 13 — Bounded document ingestion and format parity

### Goal

Prevent unbounded allocations and expose the formats the Rust extractor actually supports.

### Android tasks

1. Centralize the maximum accepted size so Android and Rust share or test the same 20 MiB value.
2. Query `OpenableColumns.SIZE` or `AssetFileDescriptor.length` before reading when available.
3. Reject known oversized files before allocation.
4. For unknown lengths, read through a bounded helper that stops at limit + 1 byte and reports “file too large.” Never call unbounded `readBytes()`.
5. Add MIME types/extensions for PDF, plain text, Markdown, DOCX, EPUB, HTML/HTM, and RTF.
6. Because document providers may return generic MIME types, validate the filename extension in core as well.
7. Show parser/size failures from core without exposing file contents.
8. Ensure the Android process keeps at most one bounded source buffer plus unavoidable FFI copies. Consider staged-file ingestion later if profiling shows 20 MiB still causes unacceptable pressure.

### Rust tasks

1. Preserve the actual normalized format in document metadata.
2. Reject unsupported or malformed formats explicitly rather than silently labeling them text.
3. Keep the size check before parser allocation.
4. Validate supplied size against actual bytes.
5. Ensure directory ingestion uses the same extractor and format normalization.

### Tests

- each supported extension is selectable and reaches the correct parser;
- known 20 MiB + 1 byte file is rejected before read;
- unknown-length stream is stopped at limit + 1;
- exactly-at-limit input follows the parser path;
- malformed DOCX/EPUB/PDF reports a controlled error;
- generic MIME plus valid extension works;
- unsupported binary input is rejected or handled according to an explicit policy;
- cancellation/lock during ingestion produces no late vectors.

### Definition of done

No Android document path performs an unbounded read, every advertised format reaches the matching core extractor, and oversized/malformed input produces a controlled user-visible error.

## 21. Work package 14 — Persistence error propagation

### Goal

Eliminate silent storage failures in security- and user-visible mutations.

### Audit method

Search actor and persistence code for ignored results, especially `let _ =`, `unwrap_or_default`, and log-only mutation failures. Classify each occurrence:

- best-effort cleanup;
- optional read with safe default;
- required read;
- required mutation.

Only the first two categories may intentionally suppress an error, and each suppression must explain why the fallback is safe.

### Mutation rule

For every required mutation:

1. Validate input.
2. Execute the database transaction.
3. Update in-memory actor state.
4. Project to `AppState`.
5. Emit the revision.

If step 2 fails, steps 3–5 must not pretend success. Report a typed, user-safe error and retain the prior state.

### Priority paths

Audit first:

- onboarding completion;
- PIN/auth settings;
- send/edit/delete message;
- provider add/update/remove/default;
- document/memory mutation;
- ContextVM enable/trust;
- directory source and fingerprint sync;
- attestation cache updates.

### Tests

Use fault injection or a deliberately failing database connection to verify that every priority action leaves `AppState` consistent with persisted state. Add at least one test for each priority path.

### Definition of done

All ignored-result sites are classified, every required mutation propagates failure, and fault tests prove that projected UI state never claims a mutation that storage rejected.

## 22. Work package 15 — Build, lint, smoke, and release gates

### Android lint

1. Guard API-29 `MediaStore.Downloads` references in an API-aware function annotated for lint, or use an API-28-safe alternative.
2. Do not hand-edit generated `mango_core.kt` to hide `Cleaner` warnings.
3. Fix the binding generation source/template or apply a narrowly scoped generated-binding lint policy only after proving the runtime fallback on API 28.
4. Keep `NewApi` enabled for handwritten Android code.
5. Resolve or explicitly triage all remaining warnings that affect compatibility, ABI packaging, permissions, or security.

### ABI policy

1. Keep release ARM64-only if that is the product decision.
2. Ensure debug comments and emulator instructions match the actual ABI filters.
3. Either package complete x86_64 dependencies for debug or standardize smoke tests on ARM64 emulators/devices.
4. Document that LocalLLM support is narrower than general ARM64 app support.

### Smoke scenarios

1. Remove assertions for feature-flagged UI such as Agents when disabled.
2. Distinguish biometric “Unlock” from a PIN lock screen using resource IDs, content descriptions, or screen-specific selectors.
3. Require a unique assistant response, not merely the sent user message or a Retry button.
4. Assert active backend identity.
5. Make mock-server responses deterministic.
6. Capture sanitized screenshots/UI XML and a summary JSON on failure.
7. Restore test-created custom providers or use an isolated test profile/database.

### Required CI commands

Run from a clean checkout with generated artifacts synchronized:

```bash
cargo test -p mango_core --lib --all-features
cd android
./gradlew :app:testDebugUnitTest --rerun-tasks
./gradlew :app:lintDebug
./gradlew :app:assembleDebug
./gradlew :app:assembleRelease
./gradlew :app:minifyReleaseWithR8
```

Also run the remote mocked-chat smoke test and supported-device LocalLLM test. The release build must use CI test signing or an explicitly documented non-production signing configuration; it must not require developers to possess production keys.

### Developer build workflow

When Rust FFI types or actions change, use the repository recipes so Kotlin bindings and packaged native libraries come from the same Rust revision:

```bash
just bindings-kotlin
just bindings-swift
just android-full
```

When native LocalLLM code or compiler flags change, run the pinned llama build before the full Android build:

```bash
just build-llama-android
just android-full
```

Update these recipes as part of work package 1 so the baseline CPU probe and optimized runtime use separate build directories. After native changes, a Gradle-only build is not sufficient evidence because it may reuse stale `.so` files.

### Release-blocking device matrix

| Device class | App launch | Remote chat | LocalLLM UI | Local generation |
|---|---|---|---|---|
| API 28, older ARM64 CPU, 4 GiB | must pass | must pass | disabled with reason | must not attempt load |
| API 30, ARM64, 6 GiB | must pass | must pass | disabled with reason | must not attempt load |
| API 31+, missing required CPU feature | must pass | must pass | disabled with reason | must not attempt load |
| API 31+, eligible CPU, 8 GiB | must pass | must pass | enabled | must pass supported small model |
| Recent flagship, 12+ GiB | must pass | must pass | enabled | must pass download/load/generate/stop |
| Unsupported ABI debug environment | must pass if app ABI supports it | must pass | disabled | must not attempt load |

### Definition of done

Every required command exits successfully from a clean generated state, lint has zero errors, and all rows of the release-blocking device matrix match their expected LocalLLM behavior.

## 23. End-to-end feature validation checklist

Run this checklist on at least one API-28 remote-only phone and one LocalLLM-eligible modern phone. Use fresh app data for onboarding cases and an upgrade fixture for migration cases.

### Installation and onboarding

- Fresh install opens onboarding without a Home flash.
- Provider setup validates success and failure accurately.
- Continue-after-attestation-failure remains visibly unverified.
- Complete and Skip both require PIN setup.
- PIN migration completes and survives process restart.
- Wrong PIN, duress PIN, and biometric paths behave as specified.

### Lock and wipe

- Every timeout option locks at the correct boundary.
- Lock during stream, attachment staging, ingestion, directory sync, health check, and attestation does not crash.
- Unlock restores the intended non-sensitive screen only.
- Wipe removes conversations, keys, attachments, documents, memories, vector data, custom providers, and auth state according to product policy.
- Late operations cannot repopulate wiped data.

### Providers and chat

- Built-in and custom providers can be added, tested, selected, edited, and removed.
- Default fallback works after provider removal or LocalLLM ineligibility.
- Send, stream, stop, retry, edit, copy, and share behave correctly.
- Message editing requires changed text and confirms history truncation.
- Attestation badge follows the actual active route.

### Attachments, documents, and memory

- Text and image attachments replace and clear safely.
- Plaintext staging disappears after send/clear/lock/wipe.
- Every supported document format ingests or reports a controlled parser error.
- Oversized documents are rejected without memory spikes.
- Document attach/detach and retrieval work after restart.
- Delete during ingestion leaves no vectors.
- Memory edit changes subsequent semantic retrieval.

### LocalLLM

- Unsupported phone never loads optimized libraries.
- Supported phone explains per-model RAM/storage eligibility.
- Download interruption is recoverable.
- Integrity verification rejects a non-GGUF or corrupted file.
- Load, generate, stop, unload, delete, restart, and remote fallback pass.
- Changing the active backend from local to remote releases local runtime/model resources.

### ContextVM

- Trusted-provider filtering works.
- Same-name tools from different providers remain distinct.
- Tool calls route to the selected provider identity.
- Lock/wipe during discovery or invocation cannot update the new session.

## 24. Pull-request decomposition for junior developers

Do not submit this entire plan as one change. Use the following PR boundaries:

1. **Test baselines and stale migration assertion.** No production behavior changes.
2. **LocalLLM pure capability policy and tests.** No native loading changes yet.
3. **Baseline CPU probe and native-build verification.** No UI changes.
4. **Lazy LocalLLM provider and startup-crash fix.** Includes API-28 launch proof.
5. **LocalLLM settings/fallback UX.** Depends on the safe provider.
6. **Onboarding encryption gate.** State-machine and tests.
7. **Prepared/committed PIN migration.** Persistence and fault tests.
8. **Session epoch infrastructure.** Add event fields and discard logic.
9. **Lock/wipe centralized cleanup.** Uses session epochs.
10. **Attachment ownership cleanup.** Narrow core/Android tests.
11. **RAG and memory operation IDs.** Consistency tests.
12. **Message edit UI and transactional core behavior.**
13. **ContextVM schema identity migration.**
14. **ContextVM wire aliases and trust enforcement.**
15. **Attestation state/copy correction.**
16. **FLAG_SECURE, clipboard, and share confirmation.**
17. **Custom provider cards and core fallback.**
18. **Biometric activity rebinding and timeout.**
19. **Bounded document reader and format parity.**
20. **Persistence error-propagation sweep.** Split further if the diff is broad.
21. **Lint/generated-binding cleanup and final smoke hardening.**

Each PR description must include:

- the invariant it implements;
- pre-fix failing test or reproduction;
- post-fix commands and device used;
- security-sensitive state added or removed;
- FFI/binding regeneration status;
- rollback behavior;
- remaining work-package dependencies.

## 25. Code-review checklist for every PR

The reviewer should answer all applicable questions:

### Correctness

- Can an event arrive after its owner was deleted, locked, or wiped?
- Is persisted state updated before UI state reports success?
- Does failure leave a retryable and internally consistent state?
- Are repeated actions idempotent?

### Security and privacy

- Does any new log contain content or secrets?
- Is plaintext staged only under an app-owned directory?
- Is deletion restricted to proven app-owned paths?
- Can a new navigation path bypass auth?
- Can a stale callback cross the session epoch?

### Android lifecycle

- What happens after rotation?
- What happens if WorkManager starts the process first?
- What happens if the activity disappears during a platform prompt/picker?
- Does API 28 verify/load every referenced class safely?

### Native compatibility

- Which library is loaded, and when?
- What compiler ISA does it require?
- Was compatibility established before load?
- Can a failure terminate the process rather than return an error?

### Tests

- Is there a negative test?
- Is there a restart/late-result test?
- Is the test deterministic, without arbitrary sleeps?
- Does the smoke test assert the outcome rather than an intermediate action?

## 26. Completion criteria

This remediation program is complete only when all of the following are true:

1. All work-package definitions of done are satisfied.
2. Rust tests pass with no ignored new failures.
3. Android unit tests pass from a clean build.
4. Android lint reports zero errors.
5. Debug and minified release builds succeed.
6. API-28 startup and remote-chat smoke tests pass.
7. The modern-device LocalLLM scenario passes.
8. Lock/wipe race tests cover every asynchronous event category.
9. PIN migration fault injection passes at every checkpoint.
10. The feature validation checklist has recorded pass/fail evidence for both device classes.
11. No optimized LocalLLM library is loaded during general app startup.
12. No on-disk install can enter Home/Chat before encryption setup commits.
13. No pre-lock/pre-wipe event can mutate the next session.

Until criteria 5, 6, 9, 11, 12, and 13 pass, Android release distribution should remain blocked.
