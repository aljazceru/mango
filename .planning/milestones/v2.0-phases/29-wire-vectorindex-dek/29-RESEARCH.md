# Phase 29: Wire VectorIndex DEK End-to-End - Research

**Researched:** 2026-04-09
**Domain:** Rust actor state / encryption wiring (no new libraries)
**Confidence:** HIGH

## Summary

This is a pure mechanical wiring phase. The encryption infrastructure (VectorIndex AES-256-GCM with MGO1 header, DEK generation and unwrapping in all three auth handlers) was fully implemented in Phase 28. The only gap is that `actor_state.vector_index` never receives the DEK: it is created at startup with `None` and all four `save(None)` call sites stay `None`.

The work has three parts: (1) add `dek: Option<Zeroizing<[u8; 32]>>` to `ActorState` and set/clear it in the four unlock and lock handlers, (2) defer VectorIndex creation in Case D (auth params exist) to post-unlock so the index file is decrypted with the real key, and (3) plumb `actor_state.dek.as_ref().map(|d| d.as_ref())` into every `save()` call.

No new crates are needed. No API design decisions are open. All code patterns already exist in the codebase.

**Primary recommendation:** Follow the `db: Option<Database>` lifecycle exactly — `dek` field mirrors it, VectorIndex creation mirrors it, `LockApp` cleanup mirrors it.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**DEK Storage in ActorState**
- D-01: Add `dek: Option<Zeroizing<[u8; 32]>>` field to `ActorState` struct. Set to `Some(dek)` in all three unlock handlers (`SetupPin`, `UnlockWithPin`, `BiometricResult`). Follows the `db: Option<Database>` lifecycle pattern.
- D-02: On `LockApp`, clear the DEK field (`actor_state.dek = None`) alongside `actor_state.db = None`. The `Zeroizing` wrapper ensures the bytes are zeroed on drop.

**VectorIndex Initialization Timing**
- D-03: In encrypted mode (Case D: auth params exist), defer `VectorIndex` creation to post-unlock, matching the deferred `Database::open` pattern. During startup, set `vector_index` to a minimal fallback or `None`.
- D-04: After unlock (in `SetupPin`, `UnlockWithPin`, `BiometricResult`), create `VectorIndex::new(&vector_data_dir, actor_state.dek.as_ref().map(|d| d.as_ref()))` and assign it to `actor_state.vector_index`.
- D-05: In non-encrypted mode (Case B/C: no auth), continue creating `VectorIndex::new(&vector_data_dir, None)` at startup as today — no behavioral change for pre-encryption installs.

**Save Call Site Plumbing**
- D-06: All 4 `vector_index.save(None)` call sites in `lib.rs` must pass the DEK from `ActorState`: (1) document deletion ~line 4020, (2) memory deletion ~line 4221, (3) document ingestion ~line 5276, (4) memory extraction ~line 5354. Change `save(None)` to `save(actor_state.dek.as_ref().map(|d| d.as_ref()))`.
- D-07: `dispatch_tools` in `agent/tools.rs` only calls `.search()` and `.add()` on VectorIndex, never `.save()` — it does not need a DEK parameter. The save after tool dispatch happens back in `lib.rs`.

**Backward Compatibility**
- D-08: DEK remains `Option<&[u8; 32]>` in all VectorIndex APIs. Pre-encryption users have `actor_state.dek = None`, and `save(None)` writes unencrypted — no behavioral change for existing installs.

### Claude's Discretion

- Whether to make `vector_index` an `Option<VectorIndex>` (matching `db: Option<Database>`) or always create an empty in-memory index at startup and replace it post-unlock
- Error handling strategy when VectorIndex operations are attempted before unlock (should not happen in normal flow, but defensive coding choice)
- Whether to add a debug assertion or log warning if save is called with `None` DEK in encrypted mode

### Deferred Ideas (OUT OF SCOPE)

None — analysis stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ENC-02 | Usearch vector index files and cached documents encrypted with AES-256-GCM using DEK | VectorIndex.save() and new() both accept Option<&[u8; 32]>. Wiring the real DEK from ActorState closes the gap. All relevant call sites identified at lines 4020, 4221, 5276, 5354. |
</phase_requirements>

## Standard Stack

No new libraries required. All tools already in `Cargo.toml`.

### Core (existing)
| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| `zeroize` | 1.x | `Zeroizing<[u8; 32]>` wrapper for DEK | Already imported in `lib.rs` line 7 (`use zeroize::Zeroizing`). |
| `usearch` (via `rag::VectorIndex`) | per Cargo.toml | HNSW index persistence | `save(dek)` and `new(dir, dek)` already accept `Option<&[u8; 32]>`. |

**Installation:** None — no new dependencies.

## Architecture Patterns

### Existing Pattern: `db: Option<Database>` Lifecycle

The entire DEK + VectorIndex wiring follows this established pattern verbatim.

```
ActorState {
    db: Option<Database>,     // None until unlock
    // ADD:
    dek: Option<Zeroizing<[u8; 32]>>,  // None until unlock
    vector_index: rag::VectorIndex,    // currently always created at startup
}
```

**Startup (Case D — auth params exist):**
- `db = None` (deferred) — already done
- `dek = None` (new field, initialized to None)
- `vector_index` = create with `None` or an empty fallback (Claude's discretion — see below)

**After unlock (all three handlers + UnlockWithDek):**
- `actor_state.db = Some(db)` — already done
- `actor_state.dek = Some(dek.clone())` — new line
- `actor_state.vector_index = VectorIndex::new(&data_dir, dek_ref)?` — new line

**LockApp handler:**
- `actor_state.db = None` — already done (line 4545)
- `actor_state.dek = None` — new line (zeroing happens on drop via `Zeroizing`)

### DEK Reference Pattern (for save call sites)

```rust
// Source: established in Phase 28 auth handlers
let dek_ref = actor_state.dek.as_ref().map(|d| d.as_ref());
let _ = actor_state.vector_index.save(dek_ref);
```

This pattern is idiomatic and avoids cloning the key material. It produces `Option<&[u8; 32]>` which matches VectorIndex::save's signature.

### VectorIndex Deferred Init Strategies (Claude's Discretion)

Two viable approaches for Case D startup:

**Option A: `Option<VectorIndex>` field (full parity with `db`)**
```rust
vector_index: Option<rag::VectorIndex>,
```
- Pro: structurally identical to `db`, explicit None signals "not open"
- Con: all existing call sites must change `actor_state.vector_index.search(...)` to `actor_state.vector_index.as_ref().expect("vector_index unlocked").search(...)`
- Impact: search and add call sites in `lib.rs` and `agent/tools.rs` need updating

**Option B: Empty in-memory fallback at startup (existing field type unchanged)**
```rust
// At startup in Case D:
let vector_index = rag::VectorIndex::new("", None).expect("empty fallback");
// After unlock:
actor_state.vector_index = rag::VectorIndex::new(&data_dir, dek_ref)?;
```
- Pro: zero changes to existing search/add call sites — `vector_index` is always a valid `VectorIndex`
- Pro: `VectorIndex::new("", None)` is already the existing fallback pattern (line 2913)
- Con: a pre-unlock search returns empty results (correct behavior for locked state — no data accessible)
- Recommendation: **Option B** — less disruption, pre-unlock empty index is semantically correct (no document data accessible before unlock anyway)

### Recommended Project Structure (no changes to file layout)

All edits are within `rust/src/lib.rs`. One field addition to `ActorState` struct, changes to handler sections.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| DEK zeroing | Manual `memset` / `for byte in ...` | `Zeroizing<[u8; 32]>` — already used everywhere |
| Byte reference to slice | Manual pointer cast | `.as_ref()` on `Zeroizing` dereferences to `[u8; 32]` |

## Runtime State Inventory

This phase does not rename any stored strings or identifiers. **No runtime state migration required.**

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | `embeddings.usearch` file on disk — unencrypted in Case D pre-phase | Will be re-saved encrypted on next save call after unlock (no migration script needed — file is rebuilt incrementally) |
| Live service config | None | None |
| OS-registered state | None | None |
| Secrets/env vars | None — DEK stays in keychain, never in env vars | None |
| Build artifacts | None | None |

**Note on existing unencrypted index files:** After this phase is wired, the first `save()` call with a real DEK will write an encrypted file. Subsequent `VectorIndex::new()` calls at next unlock will detect the MGO1 header and decrypt correctly. Users who had an unencrypted index will transparently transition on the next document/memory mutation. No explicit migration step is needed because `VectorIndex::new()` already handles "legacy unencrypted files load transparently" (verified in `index.rs` line 79-82 and test `test_legacy_unencrypted_loads_transparently`).

## Common Pitfalls

### Pitfall 1: UnlockWithDek Handler Also Needs DEK Threading

**What goes wrong:** The CONTEXT.md lists three handlers (SetupPin, UnlockWithPin, BiometricResult). But there is a fourth unlock handler — `AppAction::UnlockWithDek { dek_hex }` (line 4396) — that also opens the encrypted DB and calls `load_post_unlock`. It is missing from the CONTEXT.md list.

**Why it happens:** `UnlockWithDek` appears to be an internal/test path (the action name matches `D-06` from Phase 28 bootstrap flow). It exists in `lib.rs` between `SetupPin` and `UnlockWithPin` handlers and also calls `load_post_unlock`.

**How to avoid:** Also thread DEK into `ActorState` in the `UnlockWithDek` handler. The DEK is available as `dek_hex` (a hex string) — convert to bytes before storing:
```rust
// In UnlockWithDek, after setting actor_state.db:
if actor_state.db_path != ":memory:" {
    // dek_hex is available; parse to bytes for ActorState.dek
    let bytes = hex::decode(&*dek_hex)...;
    actor_state.dek = Some(Zeroizing::new(bytes.try_into()...));
}
```
Or alternatively, skip DEK storage for UnlockWithDek if it is test-only and only used when `db_path == ":memory:"` (in which case `dek = None` is correct for the in-memory/unencrypted path).

**Investigation:** Read lines 4396-4419 carefully. In-memory path opens plaintext DB — `dek = None` is correct there. The non-`:memory:` path opens an encrypted DB — DEK should be stored. However, the `dek_hex` input is already a hex string, not `Zeroizing<[u8; 32]>`. Need hex-decode step if threading.

**Warning signs:** If `UnlockWithDek` is used as a real unlock path (not just tests), skipping it means vector index stays unencrypted on that path.

### Pitfall 2: VectorIndex Created Before DEK Available (Case D)

**What goes wrong:** Line 2911 creates `VectorIndex::new(&vector_data_dir, None)` unconditionally — even in Case D where an encrypted index file may already exist on disk. If `embeddings.usearch` has a MGO1 header (user already used the app post-Phase 28), calling `VectorIndex::new(..., None)` will return `Err("index file is encrypted but no DEK was provided")` — triggering the `.unwrap_or_else` fallback to an empty in-memory index.

**Current behavior:** The `unwrap_or_else` at line 2911-2913 silently swallows this error and creates an empty index. Post-unlock, the old code never replaces it with the real encrypted index. All prior embeddings are invisible after unlock.

**How to avoid:** In Case D startup, skip `VectorIndex::new` entirely (use empty fallback immediately). After unlock, call `VectorIndex::new(&data_dir, Some(&*dek))` with the real DEK. This is exactly what D-03/D-04 prescribe.

**Warning signs:** User uploads documents, locks app, unlocks — documents are no longer found in RAG search.

### Pitfall 3: Zeroizing Clone for ActorState Field

**What goes wrong:** `Zeroizing<[u8; 32]>` implements `Clone` (because `[u8; 32]: Clone`). Cloning is needed when storing the DEK from a local `Zeroizing<[u8; 32]>` variable into `actor_state.dek`. This is correct and safe — `Zeroizing` zeros the clone's memory when the clone is dropped.

**How to avoid:** Use `.clone()` explicitly when storing. Do not use `std::mem::take` or `as_ref` to steal the local variable — the local `dek` is still needed for `VectorIndex::new` and potentially `dek_hex` construction if biometric path uses it.

### Pitfall 4: BiometricResult DEK is a Hex String

**What goes wrong:** In `BiometricResult`, the DEK comes from the keychain as a hex string (`dek_hex: Zeroizing<String>`) not as `Zeroizing<[u8; 32]>`. To store it in `actor_state.dek`, hex-decode is needed.

**Current code (lines 5541-5542):**
```rust
let maybe_dek_hex = actor_state.keychain.load("mango".to_string(), "dek".to_string());
if let Some(raw_dek) = maybe_dek_hex {
    let dek_hex = zeroize::Zeroizing::new(raw_dek);
    // opens DB using dek_hex...
}
```
The `dek_hex` string is available but must be hex-decoded to get `[u8; 32]`. Use:
```rust
let dek_bytes: [u8; 32] = hex::decode(dek_hex.as_str())
    .ok()
    .and_then(|v| v.try_into().ok())
    .expect("DEK hex from keychain is always valid 32 bytes");
actor_state.dek = Some(Zeroizing::new(dek_bytes));
```
Check whether `hex` crate is available in `Cargo.toml` — or use existing DEK-from-hex patterns elsewhere in the codebase.

**Warning signs:** Compiler error at hex-decode step if `hex` crate is not in scope. Search for existing hex-decode patterns in auth handlers.

### Pitfall 5: load_post_unlock Does Not Initialize VectorIndex

**What goes wrong:** `load_post_unlock` is the common post-unlock initialization function (called by all handlers). It does NOT currently create the VectorIndex. VectorIndex init after unlock must happen in each handler individually, AFTER `actor_state.dek` is set, and BEFORE `load_post_unlock` is called (or alternatively, pass the DEK into `load_post_unlock`).

**Recommended approach:** Set `actor_state.dek` first, then create VectorIndex (referencing `actor_state.dek`), assign to `actor_state.vector_index`, then call `load_post_unlock`. This avoids changing `load_post_unlock`'s signature.

**Alternative:** Extend `load_post_unlock` to create VectorIndex using `actor_state.dek`. This is cleaner (single init point) but changes the function signature or relies on `actor_state.dek` being set first.

## Code Examples

### D-01: ActorState Field Addition

```rust
// Source: rust/src/lib.rs, ActorState struct (~line 772)
// Add after existing Phase 28 additions (bootstrap, biometric_provider, pre_lock_screen, db_path):
/// Data Encryption Key for VectorIndex file encryption (Phase 29, ENC-02).
/// None until unlock; zeroed on drop via Zeroizing wrapper.
dek: Option<Zeroizing<[u8; 32]>>,
```

### D-02: ActorState Initialization (startup)

```rust
// Source: rust/src/lib.rs, ActorState construction (~line 2937)
// Add dek field to ActorState literal:
let mut actor_state = ActorState {
    // ... existing fields ...
    db_path: db_path.clone(),
    dek: None,  // ADD THIS — populated after unlock
};
```

### D-03/D-04: VectorIndex Startup (Case D deferral)

```rust
// Source: rust/src/lib.rs (~line 2909-2914) — replace existing VectorIndex::new call
// BEFORE (current):
let vector_index = rag::VectorIndex::new(&vector_data_dir, None).unwrap_or_else(|_e| {
    rag::VectorIndex::new("", None).expect("fallback VectorIndex creation failed")
});

// AFTER:
let vector_index = if has_auth {
    // Case D: auth params exist — encrypted index on disk, DEK not yet available.
    // Use empty in-memory fallback; real index loaded post-unlock (D-04).
    rag::VectorIndex::new("", None).expect("fallback VectorIndex creation failed")
} else {
    // Case A/B/C: no auth — open unencrypted index directly.
    rag::VectorIndex::new(&vector_data_dir, None).unwrap_or_else(|_e| {
        rag::VectorIndex::new("", None).expect("fallback VectorIndex creation failed")
    })
};
```

### D-04: VectorIndex Creation Post-Unlock (example from SetupPin)

```rust
// Source: rust/src/lib.rs, SetupPin handler (~line 4391-4393)
// After actor_state.db = Some(db) and before load_post_unlock:

// Store DEK in ActorState (ENC-02 / Phase 29 D-01).
actor_state.dek = Some(dek.clone());  // dek is already Zeroizing<[u8; 32]>

// Open VectorIndex with real DEK so encrypted index file is decrypted (D-04).
let dek_ref: Option<&[u8; 32]> = actor_state.dek.as_ref().map(|d| d.as_ref());
actor_state.vector_index = rag::VectorIndex::new(&actor_state.data_dir, dek_ref)
    .unwrap_or_else(|_e| {
        log::warn!("[auth] SetupPin: VectorIndex open failed, using empty fallback: {_e}");
        rag::VectorIndex::new("", None).expect("empty fallback")
    });

actor_state.app_state.auth_initialized = true;
// ...
load_post_unlock(&mut actor_state, core_tx_for_thread.clone(), false);
```

### D-06: Save Call Site Change (all four sites identical pattern)

```rust
// Source: rust/src/lib.rs — replace each save(None) with:
let _ = actor_state.vector_index.save(
    actor_state.dek.as_ref().map(|d| d.as_ref())
);
```

### D-02: LockApp Cleanup

```rust
// Source: rust/src/lib.rs, LockApp handler (~line 4545)
// Add alongside actor_state.db = None:
actor_state.db = None;
actor_state.dek = None;  // ADD: Zeroizing zeros the bytes on drop
// Reset to empty in-memory fallback to avoid stale encrypted file access:
actor_state.vector_index = rag::VectorIndex::new("", None)
    .expect("empty fallback VectorIndex");
```

## State of the Art

No ecosystem changes affect this phase. All APIs used are from Phase 28 implementation.

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `save(None)` always | `save(dek_ref)` | Phase 29 | Index files encrypted at rest |
| `VectorIndex::new(..., None)` at startup | Deferred in Case D | Phase 29 | Encrypted files load correctly after unlock |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `UnlockWithDek` is used as a real (non-test-only) unlock path for non-`:memory:` DB paths | Common Pitfalls - Pitfall 1 | If it is test-only and `:memory:`-only, DEK threading there is unnecessary but harmless. If it IS a real path, missing it leaves a security gap. | 

**Verification needed:** Grep for callers of `AppAction::UnlockWithDek` across Swift/Kotlin/Desktop sources to determine if it is dispatched in production flows. [ASSUMED: based on reading handler code only]

## Open Questions (RESOLVED)

1. **UnlockWithDek: real path or test-only?** — RESOLVED: Real path on Android. `mango_core.kt` dispatches `AppAction.UnlockWithDek` with actual `db_path`. Plan wires DEK there conservatively (correct).

2. **BiometricResult: hex crate availability** — RESOLVED: `hex = "0.4"` present in `Cargo.toml` line 33. Plan uses `hex::decode()` for keychain hex string → `[u8; 32]`.

3. **Option A vs Option B for VectorIndex field type** — RESOLVED: Plan chose Option B (empty in-memory fallback). Fewer call site changes, semantically correct (empty results while locked), matches existing fallback pattern at line 2913.

## Environment Availability

Step 2.6: SKIPPED — no external dependencies. Phase is pure Rust code edits within the existing crate. No new tools, runtimes, databases, or services.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`#[test]`) |
| Config file | `rust/Cargo.toml` (no separate config file) |
| Quick run command | `cargo test -p confidential-app rag::index -- --nocapture` |
| Full suite command | `cargo test -p confidential-app` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ENC-02 | Encrypted VectorIndex round-trip (save with DEK, load with DEK, vectors intact) | unit | `cargo test -p confidential-app test_encrypted_save_and_load_round_trip` | YES (index.rs line 292) |
| ENC-02 | Wrong DEK returns error on load | unit | `cargo test -p confidential-app test_wrong_dek_returns_error` | YES (index.rs line 323) |
| ENC-02 | Legacy unencrypted file loads transparently with DEK provided | unit | `cargo test -p confidential-app test_legacy_unencrypted_loads_transparently` | YES (index.rs line 338) |
| ENC-02 | Encrypted file with no DEK returns error | unit | `cargo test -p confidential-app test_encrypted_file_no_dek_returns_error` | YES (index.rs line 363) |
| ENC-02 | ActorState compiles with dek field + all handlers store/clear DEK | compile | `cargo build -p confidential-app` | Wave 0 setup |
| ENC-02 | Save call sites use ActorState DEK (integration) | manual smoke | Lock app, add document, lock again, unlock — document still searchable | manual |

**Note:** All VectorIndex-layer unit tests already pass (Phase 28-03). New tests needed only for the actor-layer wiring (compile + integration). No new unit test files needed — the existing tests in `rag/index.rs` already cover the encryption API.

### Sampling Rate
- **Per task commit:** `cargo build -p confidential-app` (confirms compile after each structural change)
- **Per wave merge:** `cargo test -p confidential-app` (full test suite)
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
None — existing test infrastructure covers the VectorIndex encryption layer. Wave 0 only needs to verify the codebase compiles after `dek` field is added to `ActorState`.

## Security Domain

`security_enforcement` is not explicitly set to false in config. Included.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | DEK is a key, not a credential; auth is handled by Phase 28 |
| V3 Session Management | no | Lock/unlock session handled by Phase 28 |
| V4 Access Control | yes | DEK must only be available post-unlock; `Option<Zeroizing<[u8; 32]>>` enforces this structurally |
| V5 Input Validation | no | No new user inputs |
| V6 Cryptography | yes | DEK zeroing on lock/drop via `Zeroizing`; key never cloned unnecessarily |

### Known Threat Patterns for This Phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| DEK lingering in memory after lock | Information Disclosure | `Zeroizing<[u8; 32]>` zeros on drop; `actor_state.dek = None` on LockApp triggers drop |
| Unencrypted temp file during VectorIndex save | Information Disclosure | Already mitigated in Phase 28-03 — `save()` deletes temp file immediately, sets 0600 permissions |
| VectorIndex opened without DEK in Case D | Tampering / Info Disclosure | Fixed by this phase: Case D defers VectorIndex init to post-unlock |

## Sources

### Primary (HIGH confidence)
- `rust/src/rag/index.rs` — VectorIndex::new() and save() APIs, MGO1 encryption, existing test coverage (read directly from codebase)
- `rust/src/lib.rs` — ActorState struct (lines 772-833), startup flow (lines 2890-2961), SetupPin handler (lines 4297-4393), UnlockWithDek handler (4396-4419), UnlockWithPin handler (4421-4527), LockApp handler (4529-4561), BiometricResult handler (5532-5589), all four save call sites (4020, 4221, 5276, 5354) (read directly from codebase)
- `.planning/phases/29-wire-vectorindex-dek/29-CONTEXT.md` — locked decisions D-01 through D-08 (read directly)

### Secondary (MEDIUM confidence)
- `.planning/REQUIREMENTS.md` — ENC-02 definition and status (read directly)

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new libraries, all APIs verified from source
- Architecture: HIGH — all patterns verified from codebase source, no assumptions about library behavior
- Pitfalls: HIGH — identified from direct code reading (Pitfall 1 and 4 are code-verified, not assumed)

**Research date:** 2026-04-09
**Valid until:** 2026-05-09 (stable codebase, no external dependency churn)
