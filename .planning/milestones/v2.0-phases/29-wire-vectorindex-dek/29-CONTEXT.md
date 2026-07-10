# Phase 29: Wire VectorIndex DEK End-to-End - Context

**Gathered:** 2026-04-09 (assumptions mode)
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire the Data Encryption Key (DEK) from authentication handlers through ActorState to all VectorIndex call sites, so usearch vector index files are actually encrypted at rest — closing the gap between the AES-256-GCM capability built in Phase 28-03 and its runtime invocation.

</domain>

<decisions>
## Implementation Decisions

### DEK Storage in ActorState
- **D-01:** Add `dek: Option<Zeroizing<[u8; 32]>>` field to `ActorState` struct. Set to `Some(dek)` in all three unlock handlers (`SetupPin`, `UnlockWithPin`, `BiometricResult`). Follows the `db: Option<Database>` lifecycle pattern.
- **D-02:** On `LockApp`, clear the DEK field (`actor_state.dek = None`) alongside `actor_state.db = None` — no key material remains in memory while locked. The `Zeroizing` wrapper ensures the bytes are zeroed on drop.

### VectorIndex Initialization Timing
- **D-03:** In encrypted mode (Case D: auth params exist), defer `VectorIndex` creation to post-unlock, matching the deferred `Database::open` pattern. During startup, set `vector_index` to a minimal fallback or `None`.
- **D-04:** After unlock (in `SetupPin`, `UnlockWithPin`, `BiometricResult`), create `VectorIndex::new(&vector_data_dir, actor_state.dek.as_ref().map(|d| d.as_ref()))` and assign it to `actor_state.vector_index`.
- **D-05:** In non-encrypted mode (Case B/C: no auth), continue creating `VectorIndex::new(&vector_data_dir, None)` at startup as today — no behavioral change for pre-encryption installs.

### Save Call Site Plumbing
- **D-06:** All 4 `vector_index.save(None)` call sites in `lib.rs` must pass the DEK from `ActorState`: (1) document deletion ~line 4020, (2) memory deletion ~line 4221, (3) document ingestion ~line 5276, (4) memory extraction ~line 5354. Change `save(None)` to `save(actor_state.dek.as_ref().map(|d| d.as_ref()))`.
- **D-07:** `dispatch_tools` in `agent/tools.rs` only calls `.search()` and `.add()` on VectorIndex, never `.save()` — it does not need a DEK parameter. The save after tool dispatch happens back in `lib.rs`.

### Backward Compatibility
- **D-08:** DEK remains `Option<&[u8; 32]>` in all VectorIndex APIs (already the case from Phase 28-03). Pre-encryption users (`encryption_enabled = false`) have `actor_state.dek = None`, and `save(None)` writes unencrypted — no behavioral change for existing installs.

### Claude's Discretion
- Whether to make `vector_index` an `Option<VectorIndex>` (matching `db: Option<Database>`) or always create an empty in-memory index at startup and replace it post-unlock
- Error handling strategy when VectorIndex operations are attempted before unlock (should not happen in normal flow, but defensive coding choice)
- Whether to add a debug assertion or log warning if save is called with `None` DEK in encrypted mode

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### VectorIndex encryption API (Phase 28-03)
- `rust/src/rag/index.rs` — VectorIndex::new() and VectorIndex::save() already accept `Option<&[u8; 32]>` DEK parameter; encrypt_file/decrypt_file with MGO1 header

### Actor state and auth handlers
- `rust/src/lib.rs` §772 — ActorState struct (needs DEK field)
- `rust/src/lib.rs` §2900-2913 — Actor startup, Case B/C/D auth routing, VectorIndex::new() call
- `rust/src/lib.rs` §4301 — SetupPin handler (derives DEK, opens encrypted DB)
- `rust/src/lib.rs` §4498 — UnlockWithPin handler (unwraps DEK, opens encrypted DB)
- `rust/src/lib.rs` §5542 — BiometricResult handler (loads DEK from keychain)
- `rust/src/lib.rs` §4529-4545 — LockApp handler (drops db, resets state)

### VectorIndex save call sites
- `rust/src/lib.rs` §4020 — document deletion save
- `rust/src/lib.rs` §4221 — memory deletion save
- `rust/src/lib.rs` §5276 — document ingestion save
- `rust/src/lib.rs` §5354 — memory extraction save

### Agent tools (no DEK needed)
- `rust/src/agent/tools.rs` §249 — dispatch_tools takes &VectorIndex, only calls search/add

### Requirements
- `.planning/REQUIREMENTS.md` — ENC-02: Usearch vector index files and cached documents encrypted with AES-256-GCM using DEK

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `VectorIndex::new()` and `VectorIndex::save()` already accept `Option<&[u8; 32]>` DEK parameter (Phase 28-03) — the encryption/decryption API is fully built, just not called with a real DEK
- `encrypt_file()` / `decrypt_file()` in `rag/index.rs` — AES-256-GCM with MGO1 magic header, random nonce, authenticated tag
- `Zeroizing<[u8; 32]>` pattern already used in all auth handlers for DEK handling
- `ActorState.db: Option<Database>` — established pattern for post-unlock resource initialization

### Established Patterns
- Deferred initialization: `db` is `None` until unlock, then `Database::open()` is called with the DEK for SQLCipher pragma. VectorIndex should follow this same lifecycle.
- Auth handler DEK flow: all three handlers (SetupPin, UnlockWithPin, BiometricResult) already have the DEK as a local `Zeroizing<[u8; 32]>` — it just needs to be stored in ActorState before going out of scope.
- `LockApp` cleanup: drops `db`, resets `app_state`, cancels streams — DEK field cleanup fits directly into this sequence.

### Integration Points
- `ActorState` struct: add one field
- Three auth handlers: store DEK in ActorState after existing DB open
- `LockApp` handler: clear DEK alongside DB drop
- Four save call sites: change `save(None)` to `save(dek_ref)`
- Actor startup Case D: defer VectorIndex creation (or create empty)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — this is a mechanical wiring task connecting existing encryption APIs to existing call sites through ActorState.

</specifics>

<deferred>
## Deferred Ideas

None — analysis stayed within phase scope

</deferred>

---

*Phase: 29-wire-vectorindex-dek*
*Context gathered: 2026-04-09*
