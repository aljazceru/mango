//! PPQ secret storage (plan §6.2).
//!
//! Centralizes every keychain name touching PPQ credentials so wipe,
//! restore, migration, and tests cannot drift. No other module loads
//! `credit_id` directly.
//!
//! Key names:
//! - device API key: service `mango`, key `ppq-ai` (existing backend
//!   convention — the LLM transport loads the same value)
//! - root credential: service `mango.ppq`, key `credit-id-v1`
//! - replacement journal: service `mango.ppq`, key `recovery-journal-v1`
//!   (internal to this module; holds prior+target pairs while a
//!   replacement is in flight — same protection domain as the credentials
//!   themselves, never exposed in UI/logs)
//!
//! Duress exception (plan §6.2): duress wipe must NOT call
//! [`PpqSecretStore::delete_all_verified`]; it preserves all three values
//! byte-for-byte (journal included) and suppresses them instead. Verified
//! deletion is reserved for user-requested full reset, provider removal,
//! confirmed replacement, and fresh-provisioning cleanup where no prior
//! credentials existed.
//!
//! Platform limits, stated honestly: the keychain has no atomic
//! multi-item transaction; `store`'s bool may lie in either direction;
//! `load`'s `None` cannot distinguish absence from an unreadable store;
//! and iOS `store` deletes the old item before `SecItemAdd`, so rewriting
//! an already-correct slot can destroy it when the add fails. The
//! protocol below is designed around those facts, not around guarantees
//! the platform does not provide.

use zeroize::Zeroizing;

use super::client::{validate_secret_shape, PpqError};

pub const API_KEY_SERVICE: &str = "mango";
pub const API_KEY_KEY: &str = "ppq-ai";
pub const CREDIT_ID_SERVICE: &str = "mango.ppq";
pub const CREDIT_ID_KEY: &str = "credit-id-v1";
/// Internal replacement journal slot (see module docs). Values are secret
/// material and never leave this module.
pub const JOURNAL_SERVICE: &str = "mango.ppq";
pub const JOURNAL_KEY: &str = "recovery-journal-v1";

/// Verified, ordered writer/reader for the two PPQ account secrets.
pub struct PpqSecretStore<'a> {
    keychain: &'a dyn crate::KeychainProvider,
}

/// In-flight replacement record staged in the keychain before canonical
/// credentials are touched. Empty fields mean "slot absent"; the target
/// pair is always present. All values are `Zeroizing`.
struct AttemptJournal {
    prior_credit: Option<Zeroizing<String>>,
    prior_key: Option<Zeroizing<String>>,
    new_credit: Zeroizing<String>,
    new_key: Zeroizing<String>,
}

impl AttemptJournal {
    /// Encode as four newline-separated fields (validated secrets never
    /// contain control characters, so the encoding is unambiguous).
    fn encode(&self) -> Zeroizing<String> {
        let mut buf = String::new();
        buf.push_str(self.prior_credit.as_ref().map_or("", |v| v.as_str()));
        buf.push('\n');
        buf.push_str(self.prior_key.as_ref().map_or("", |v| v.as_str()));
        buf.push('\n');
        buf.push_str(&self.new_credit);
        buf.push('\n');
        buf.push_str(&self.new_key);
        Zeroizing::new(buf)
    }

    /// Decode, rejecting anything structurally impossible (missing target
    /// pair, or a journal that claims no prior credentials — a fresh
    /// attempt never stages one).
    fn decode(raw: &Zeroizing<String>) -> Option<Self> {
        let mut fields = raw.splitn(4, '\n');
        let pc = fields.next()?;
        let pk = fields.next()?;
        let nc = fields.next()?;
        let nk = fields.next()?;
        let opt = |s: &str| {
            if s.is_empty() {
                None
            } else {
                Some(Zeroizing::new(s.to_string()))
            }
        };
        if (pc.is_empty() && pk.is_empty()) || nc.is_empty() || nk.is_empty() {
            return None;
        }
        Some(Self {
            prior_credit: opt(pc),
            prior_key: opt(pk),
            new_credit: Zeroizing::new(nc.to_string()),
            new_key: Zeroizing::new(nk.to_string()),
        })
    }
}

impl<'a> PpqSecretStore<'a> {
    pub fn new(keychain: &'a dyn crate::KeychainProvider) -> Self {
        Self { keychain }
    }

    /// Store both credentials with checked writes, read-back verification,
    /// and recoverable replacement semantics (finding 4).
    ///
    /// Fresh store (no prior credentials): write `credit_id`, then the
    /// API key, read both back; on failure, best-effort verified cleanup
    /// of partial new state. A crash between writes leaves partial state
    /// that startup must classify as repairable — nothing old is lost.
    ///
    /// Replacement (any prior credential exists):
    /// 1. stage a journal holding the prior and target pairs, requiring an
    ///    explicit `store` success and a matching read-back (a
    ///    false-but-visible journal is not durably staged — Android
    ///    `commit()` can leave the updated value readable in memory); if
    ///    staging cannot be verified, abort with canonical credentials
    ///    untouched;
    /// 2. write both canonical slots (checked) and verify by read-back;
    /// 3. on success, delete the journal — if that crash-window delete is
    ///    missed, the journal still names the target pair, so the next
    ///    [`Self::recover_interrupted`] recognizes the attempt as complete
    ///    and clears it without touching the verified new pair;
    /// 4. on failure, converge both slots back to the prior pair
    ///    (skipping slots that already match — rewrites are destructive on
    ///    iOS delete-then-add) and verify. If the prior pair is confirmed
    ///    restored, the journal is dropped; if not, the journal is
    ///    retained as the sole recoverable copy of the prior root
    ///    credential and [`Self::recover_interrupted`] re-applies it later.
    ///
    /// Both failure paths return [`PpqError::StorageFailure`]; the error
    /// taxonomy cannot say whether the prior pair was confirmed restored.
    /// Callers that need to know must call [`Self::recover_interrupted`]
    /// and then re-load.
    pub fn store_provisioned(
        &self,
        credit_id: &Zeroizing<String>,
        api_key: &Zeroizing<String>,
    ) -> Result<(), PpqError> {
        validate_secret_shape(credit_id, 30..=40)?;
        validate_secret_shape(api_key, 16..=128)?;

        // Retry hygiene: resolve any journal left by an interrupted earlier
        // attempt before snapshotting or writing anything.
        self.recover_interrupted()?;

        let prior_credit = self.snapshot(CREDIT_ID_SERVICE, CREDIT_ID_KEY);
        let prior_key = self.snapshot(API_KEY_SERVICE, API_KEY_KEY);
        if prior_credit.is_none() && prior_key.is_none() {
            return self.store_fresh(credit_id, api_key);
        }

        // Verified staging must precede any canonical mutation.
        let journal = AttemptJournal {
            prior_credit,
            prior_key,
            new_credit: credit_id.clone(),
            new_key: api_key.clone(),
        };
        if !self.stage_journal(&journal) {
            return Err(PpqError::StorageFailure);
        }

        if !self.keychain.store(
            CREDIT_ID_SERVICE.into(),
            CREDIT_ID_KEY.into(),
            credit_id.to_string(),
        ) {
            return self.recover_from_attempt(&journal);
        }
        if !self.keychain.store(
            API_KEY_SERVICE.into(),
            API_KEY_KEY.into(),
            api_key.to_string(),
        ) {
            return self.recover_from_attempt(&journal);
        }

        let read_credit = self.snapshot(CREDIT_ID_SERVICE, CREDIT_ID_KEY);
        let read_key = self.snapshot(API_KEY_SERVICE, API_KEY_KEY);
        if matches_secret(&read_credit, credit_id) && matches_secret(&read_key, api_key) {
            // Verified new pair in place; the journal is now stale. Its
            // removal is best-effort — a leftover is resolved by the next
            // recover_interrupted()/delete_all_verified().
            self.clear_journal();
            Ok(())
        } else {
            self.recover_from_attempt(&journal)
        }
    }

    /// Reconcile a journal persisted by an attempt interrupted by a crash
    /// or killer failure. Returns `Ok(true)` when a journal existed and was
    /// resolved, `Ok(false)` when there was nothing to do, and
    /// `Err(StorageFailure)` when reconciliation could not be completed
    /// (the journal is retained for the next try) or the journal is
    /// malformed — a malformed journal may hold the sole prior root
    /// credential and is preserved until explicit verified deletion.
    /// Reconciliation is idempotent: a journal whose delete did not durably
    /// land simply re-presents itself after restart. Never called by
    /// duress.
    ///
    /// Call on startup, before reads that must reflect a settled state,
    /// and before any retry (the latter happens automatically inside
    /// [`Self::store_provisioned`]).
    pub fn recover_interrupted(&self) -> Result<bool, PpqError> {
        let raw = match self.snapshot(JOURNAL_SERVICE, JOURNAL_KEY) {
            Some(raw) => raw,
            None => return Ok(false),
        };
        let journal = match AttemptJournal::decode(&raw) {
            Some(j) => j,
            None => {
                // Structurally unusable, yet possibly the sole surviving
                // copy of the prior root credential (e.g. truncated by a
                // crash mid-write). Never delete it blindly: fail
                // recoverably and preserve it for explicit user reset
                // (delete_all_verified) or manual recovery.
                return Err(PpqError::StorageFailure);
            }
        };

        let matches_target = matches_secret(
            &self.snapshot(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            &journal.new_credit,
        ) && matches_secret(
            &self.snapshot(API_KEY_SERVICE, API_KEY_KEY),
            &journal.new_key,
        );
        let matches_prior = slot_matches(
            &self.snapshot(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            &journal.prior_credit,
        ) && slot_matches(
            &self.snapshot(API_KEY_SERVICE, API_KEY_KEY),
            &journal.prior_key,
        );
        if matches_target || matches_prior {
            // The attempt completed (target verified) or never got past a
            // consistent prior state: only journal cleanup remains.
            return if self.journal_absent_after_clear() {
                Ok(true)
            } else {
                Err(PpqError::StorageFailure)
            };
        }

        // Mixed/partial canonical state: re-apply the prior pair.
        self.converge_slot(CREDIT_ID_SERVICE, CREDIT_ID_KEY, &journal.prior_credit);
        self.converge_slot(API_KEY_SERVICE, API_KEY_KEY, &journal.prior_key);
        let restored = slot_matches(
            &self.snapshot(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            &journal.prior_credit,
        ) && slot_matches(
            &self.snapshot(API_KEY_SERVICE, API_KEY_KEY),
            &journal.prior_key,
        );
        if restored && self.journal_absent_after_clear() {
            return Ok(true);
        }
        // Prior pair not re-appliable: keep the journal (sole recoverable
        // copy of the prior root credential) and report failure.
        Err(PpqError::StorageFailure)
    }

    pub fn load_credit_id(&self) -> Result<Option<Zeroizing<String>>, PpqError> {
        Ok(self
            .keychain
            .load(CREDIT_ID_SERVICE.into(), CREDIT_ID_KEY.into())
            .map(Zeroizing::new))
    }

    pub fn load_api_key(&self) -> Result<Option<Zeroizing<String>>, PpqError> {
        Ok(self
            .keychain
            .load(API_KEY_SERVICE.into(), API_KEY_KEY.into())
            .map(Zeroizing::new))
    }

    /// True when either PPQ credential exists locally. Only for startup
    /// classification outside duress decoy mode (plan §8 migration rules).
    /// A journal-only leftover does not count as credentials.
    pub fn has_any(&self) -> bool {
        self.keychain
            .load(CREDIT_ID_SERVICE.into(), CREDIT_ID_KEY.into())
            .is_some()
            || self
                .keychain
                .load(API_KEY_SERVICE.into(), API_KEY_KEY.into())
                .is_some()
    }

    /// Verified deletion of the two PPQ credentials and any staged
    /// replacement journal. Succeeds only after a read-back confirms all
    /// three are absent. Called by user-requested full reset, explicit
    /// provider removal, and confirmed replacement — never by duress and
    /// never to "compensate" a failed replacement of existing credentials.
    pub fn delete_all_verified(&self) -> Result<(), PpqError> {
        self.keychain
            .delete(CREDIT_ID_SERVICE.into(), CREDIT_ID_KEY.into());
        self.keychain
            .delete(API_KEY_SERVICE.into(), API_KEY_KEY.into());
        self.keychain
            .delete(JOURNAL_SERVICE.into(), JOURNAL_KEY.into());
        self.verify_absent()
    }

    // ── internals ────────────────────────────────────────────────────────────

    fn store_fresh(
        &self,
        credit_id: &Zeroizing<String>,
        api_key: &Zeroizing<String>,
    ) -> Result<(), PpqError> {
        if !self.keychain.store(
            CREDIT_ID_SERVICE.into(),
            CREDIT_ID_KEY.into(),
            credit_id.to_string(),
        ) {
            return self.compensate_fresh();
        }
        if !self.keychain.store(
            API_KEY_SERVICE.into(),
            API_KEY_KEY.into(),
            api_key.to_string(),
        ) {
            return self.compensate_fresh();
        }
        let read_credit = self.snapshot(CREDIT_ID_SERVICE, CREDIT_ID_KEY);
        let read_key = self.snapshot(API_KEY_SERVICE, API_KEY_KEY);
        if matches_secret(&read_credit, credit_id) && matches_secret(&read_key, api_key) {
            Ok(())
        } else {
            self.compensate_fresh()
        }
    }

    fn compensate_fresh(&self) -> Result<(), PpqError> {
        // No prior credentials existed, so partial new state is safe to
        // remove; failure to clean up is reported by the caller's error.
        let _ = self.delete_all_verified();
        Err(PpqError::StorageFailure)
    }

    /// Current content of one slot, as reported by `load`.
    fn snapshot(&self, service: &str, key: &str) -> Option<Zeroizing<String>> {
        self.keychain
            .load(service.into(), key.into())
            .map(Zeroizing::new)
    }

    /// Stage the journal. Requires an explicit `store` success AND a
    /// matching read-back: on Android, `EncryptedSharedPreferences.commit()`
    /// can return false while leaving the updated value visible in memory,
    /// so read-back alone cannot prove durability. A false-but-visible
    /// journal is not trusted and aborts the attempt.
    fn stage_journal(&self, journal: &AttemptJournal) -> bool {
        let encoded = journal.encode();
        if !self.keychain.store(
            JOURNAL_SERVICE.into(),
            JOURNAL_KEY.into(),
            encoded.to_string(),
        ) {
            return false;
        }
        matches_secret(&self.snapshot(JOURNAL_SERVICE, JOURNAL_KEY), &encoded)
    }

    fn clear_journal(&self) {
        let _ = self
            .keychain
            .delete(JOURNAL_SERVICE.into(), JOURNAL_KEY.into());
    }

    fn journal_absent_after_clear(&self) -> bool {
        self.clear_journal();
        self.snapshot(JOURNAL_SERVICE, JOURNAL_KEY).is_none()
    }

    /// Converge a failed attempt back toward the prior pair.
    fn recover_from_attempt(&self, journal: &AttemptJournal) -> Result<(), PpqError> {
        self.converge_slot(CREDIT_ID_SERVICE, CREDIT_ID_KEY, &journal.prior_credit);
        self.converge_slot(API_KEY_SERVICE, API_KEY_KEY, &journal.prior_key);
        let restored = slot_matches(
            &self.snapshot(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            &journal.prior_credit,
        ) && slot_matches(
            &self.snapshot(API_KEY_SERVICE, API_KEY_KEY),
            &journal.prior_key,
        );
        if restored {
            // Prior pair confirmed restored; the journal is stale
            // (it duplicates canonical) and is dropped best-effort.
            self.clear_journal();
            return Err(PpqError::StorageFailure);
        }
        // Rollback unverified: retain the journal — it is the sole
        // recoverable copy of the prior root credential once its slot was
        // overwritten. Surviving canonical bytes are left in place.
        Err(PpqError::StorageFailure)
    }

    /// Restore one slot toward its prior content, reading and comparing
    /// first: rewriting an already-correct slot is NOT idempotent on iOS,
    /// where `store` deletes the old item before `SecItemAdd` and the add
    /// may fail — a blind rewrite can destroy the only good copy. The
    /// `store`/`delete` results are not trusted; callers verify by
    /// re-reading.
    fn converge_slot(&self, service: &str, key: &str, prior: &Option<Zeroizing<String>>) {
        if slot_matches(&self.snapshot(service, key), prior) {
            return;
        }
        match prior {
            Some(prior_value) => {
                let _ = self
                    .keychain
                    .store(service.into(), key.into(), prior_value.to_string());
            }
            None => {
                let _ = self.keychain.delete(service.into(), key.into());
            }
        }
    }

    fn verify_absent(&self) -> Result<(), PpqError> {
        if self.snapshot(CREDIT_ID_SERVICE, CREDIT_ID_KEY).is_some()
            || self.snapshot(API_KEY_SERVICE, API_KEY_KEY).is_some()
            || self.snapshot(JOURNAL_SERVICE, JOURNAL_KEY).is_some()
        {
            return Err(PpqError::StorageFailure);
        }
        Ok(())
    }
}

/// Constant-time comparison of one slot's observed content against its
/// snapshot (both `None` counts as a match).
fn slot_matches(current: &Option<Zeroizing<String>>, prior: &Option<Zeroizing<String>>) -> bool {
    match (current, prior) {
        (Some(c), Some(p)) => constant_time_eq(c.as_bytes(), p.as_bytes()),
        (None, None) => true,
        _ => false,
    }
}

/// Constant-time comparison of a slot's observed content against an
/// expected secret.
fn matches_secret(observed: &Option<Zeroizing<String>>, expected: &Zeroizing<String>) -> bool {
    observed
        .as_ref()
        .is_some_and(|v| constant_time_eq(v.as_bytes(), expected.as_bytes()))
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    // Synthetic, non-secret fixture values only — never real credentials.
    // Assertions compare bytes and emit redacted messages so no secret
    // material ever reaches test output.
    const OLD_CREDIT: &str = "11111111-1111-4111-8111-111111111111";
    const NEW_CREDIT: &str = "22222222-2222-4222-8222-222222222222";
    const OLD_KEY: &str = "sk-synth-old-0123456789abcdef";
    const NEW_KEY: &str = "sk-synth-new-0123456789abcdef";

    fn zero_pair(credit: &str, key: &str) -> (Zeroizing<String>, Zeroizing<String>) {
        (
            Zeroizing::new(credit.to_string()),
            Zeroizing::new(key.to_string()),
        )
    }

    /// Scripted outcome of a single `store` call.
    #[derive(Clone, Copy)]
    enum StoreOutcome {
        /// Reports success and persists.
        OkLands,
        /// Reports failure and persists nothing.
        FailSilent,
        /// Reports failure even though the write landed (lying callback).
        FailButLands,
        /// Reports success but the value never lands (silent write loss).
        OkButDrops,
    }

    #[derive(Default)]
    struct ScriptedState {
        values: BTreeMap<(String, String), String>,
        /// Outcomes consumed from the front; `OkLands` once empty.
        script: Vec<StoreOutcome>,
        /// Every load returns a mutated copy of the stored value
        /// (unreadable/lying read channel).
        load_mutates: bool,
        /// `delete` refuses to remove anything and reports failure.
        refuse_delete: bool,
        /// iOS-style store: the existing item is deleted before the add,
        /// so a failed add loses the previous value.
        destructive_stores: bool,
        /// Slot labels in store-call order ("journal" / "credit" / "apikey").
        store_order: Vec<&'static str>,
        /// Slot labels of every attempted delete.
        delete_order: Vec<&'static str>,
    }

    struct ScriptedKeychain {
        state: Mutex<ScriptedState>,
    }

    impl ScriptedKeychain {
        fn new() -> Self {
            Self {
                state: Mutex::new(ScriptedState::default()),
            }
        }

        fn script(self, script: &[StoreOutcome]) -> Self {
            self.state.lock().unwrap().script = script.to_vec();
            self
        }

        fn load_mutates(self) -> Self {
            self.state.lock().unwrap().load_mutates = true;
            self
        }

        fn refuse_delete(self) -> Self {
            self.state.lock().unwrap().refuse_delete = true;
            self
        }

        /// iOS Keychain semantics: store() = SecItemDelete + SecItemAdd.
        fn destructive_stores(self) -> Self {
            self.state.lock().unwrap().destructive_stores = true;
            self
        }

        /// Seed an existing canonical pair directly, bypassing counters.
        fn seed(&self, credit: &str, key: &str) {
            let mut s = self.state.lock().unwrap();
            s.values.insert(
                (CREDIT_ID_SERVICE.to_string(), CREDIT_ID_KEY.to_string()),
                credit.to_string(),
            );
            s.values.insert(
                (API_KEY_SERVICE.to_string(), API_KEY_KEY.to_string()),
                key.to_string(),
            );
        }

        /// Seed a journal as if an attempt had staged it, bypassing counters.
        fn seed_journal(&self, prior: (&str, &str), new: (&str, &str)) {
            let encoded = AttemptJournal {
                prior_credit: some_zero(prior.0),
                prior_key: some_zero(prior.1),
                new_credit: Zeroizing::new(new.0.to_string()),
                new_key: Zeroizing::new(new.1.to_string()),
            }
            .encode()
            .to_string();
            self.state.lock().unwrap().values.insert(
                (JOURNAL_SERVICE.to_string(), JOURNAL_KEY.to_string()),
                encoded,
            );
        }

        fn store_order(&self) -> Vec<&'static str> {
            self.state.lock().unwrap().store_order.clone()
        }

        /// Number of delete attempts against canonical slots.
        fn canonical_deletes(&self) -> usize {
            self.state
                .lock()
                .unwrap()
                .delete_order
                .iter()
                .filter(|l| **l != "journal")
                .count()
        }

        fn journal_delete_attempts(&self) -> usize {
            self.state
                .lock()
                .unwrap()
                .delete_order
                .iter()
                .filter(|l| **l == "journal")
                .count()
        }

        fn raw_value(&self, service: &str, key: &str) -> Option<Zeroizing<String>> {
            self.state
                .lock()
                .unwrap()
                .values
                .get(&(service.to_string(), key.to_string()))
                .map(|v| Zeroizing::new(v.clone()))
        }

        fn journal_present(&self) -> bool {
            self.raw_value(JOURNAL_SERVICE, JOURNAL_KEY).is_some()
        }
    }

    fn some_zero(v: &str) -> Option<Zeroizing<String>> {
        Some(Zeroizing::new(v.to_string()))
    }

    fn slot_label(service: &str, key: &str) -> &'static str {
        // The journal and credit slots share service `mango.ppq`; the key
        // disambiguates them.
        match (service, key) {
            (CREDIT_ID_SERVICE, CREDIT_ID_KEY) => "credit",
            (JOURNAL_SERVICE, JOURNAL_KEY) => "journal",
            _ => "apikey",
        }
    }

    impl crate::KeychainProvider for ScriptedKeychain {
        fn store(&self, service: String, key: String, value: String) -> bool {
            let mut s = self.state.lock().unwrap();
            s.store_order.push(slot_label(&service, &key));
            let outcome = if let Some(o) = s.script.first().copied() {
                s.script.remove(0);
                o
            } else {
                StoreOutcome::OkLands
            };
            if s.destructive_stores {
                s.values.remove(&(service.clone(), key.clone()));
            }
            match outcome {
                StoreOutcome::OkLands => {
                    s.values.insert((service, key), value);
                    true
                }
                StoreOutcome::FailSilent => false,
                StoreOutcome::FailButLands => {
                    s.values.insert((service, key), value);
                    false
                }
                StoreOutcome::OkButDrops => true,
            }
        }

        fn load(&self, service: String, key: String) -> Option<String> {
            let s = self.state.lock().unwrap();
            s.values.get(&(service, key)).map(|v| {
                if s.load_mutates {
                    format!("{v}!")
                } else {
                    v.clone()
                }
            })
        }

        fn delete(&self, service: String, key: String) -> bool {
            let mut s = self.state.lock().unwrap();
            s.delete_order.push(slot_label(&service, &key));
            if s.refuse_delete {
                return false;
            }
            s.values.remove(&(service, key));
            true
        }
    }

    /// Assert a slot holds the expected secret without ever printing it.
    fn assert_secret(actual: Option<Zeroizing<String>>, expected: &str, ctx: &str) {
        let ok = matches!(&actual, Some(a) if constant_time_eq(a.as_bytes(), expected.as_bytes()));
        assert!(ok, "{ctx}: stored value mismatch (values redacted)");
    }

    fn assert_absent(actual: Option<Zeroizing<String>>, ctx: &str) {
        assert!(
            actual.is_none(),
            "{ctx}: expected absence, found a value (redacted)"
        );
    }

    // ── success paths (regression) ──────────────────────────────────────────

    #[test]
    fn fresh_success_writes_credit_then_key_and_reads_back() {
        let kc = ScriptedKeychain::new();
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        store.store_provisioned(&credit, &key).unwrap();
        assert_eq!(kc.store_order(), vec!["credit", "apikey"]);
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            NEW_CREDIT,
            "credit slot",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            NEW_KEY,
            "api key slot",
        );
        assert!(
            !kc.journal_present(),
            "fresh attempts never stage a journal"
        );
        assert_eq!(kc.canonical_deletes(), 0);
    }

    #[test]
    fn replacement_success_overwrites_existing_pair_and_clears_journal() {
        let kc = ScriptedKeychain::new();
        kc.seed(OLD_CREDIT, OLD_KEY);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        store.store_provisioned(&credit, &key).unwrap();
        assert_eq!(kc.store_order(), vec!["journal", "credit", "apikey"]);
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            NEW_CREDIT,
            "credit slot",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            NEW_KEY,
            "api key slot",
        );
        assert!(!kc.journal_present(), "success must clear the journal");
    }

    // ── failed replacement onto an existing pair (finding 4) ────────────────

    #[test]
    fn replacement_first_write_failure_preserves_prior_pair() {
        // Store order: journal, credit(write fails). Rollback finds both
        // canonical slots already matching the prior pair and must not
        // rewrite either.
        let kc = ScriptedKeychain::new().script(&[StoreOutcome::OkLands, StoreOutcome::FailSilent]);
        kc.seed(OLD_CREDIT, OLD_KEY);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            OLD_CREDIT,
            "credit slot after failed first write",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            OLD_KEY,
            "api key slot after failed first write",
        );
        assert_eq!(
            kc.store_order(),
            vec!["journal", "credit"],
            "rollback must not rewrite already-matching slots"
        );
        assert_eq!(kc.canonical_deletes(), 0);
        assert!(
            !kc.journal_present(),
            "confirmed rollback drops the journal"
        );
    }

    #[test]
    fn replacement_first_write_false_but_landed_rolls_back() {
        // `store` returned false, yet the new credit_id landed. Recovery
        // overwrites it with the prior value; the untouched api key slot
        // is skipped entirely.
        let kc =
            ScriptedKeychain::new().script(&[StoreOutcome::OkLands, StoreOutcome::FailButLands]);
        kc.seed(OLD_CREDIT, OLD_KEY);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            OLD_CREDIT,
            "credit slot after lying failure",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            OLD_KEY,
            "api key slot after lying failure",
        );
        assert_eq!(
            kc.store_order(),
            vec!["journal", "credit", "credit"],
            "only the landed credit slot is rewritten"
        );
        assert_eq!(kc.canonical_deletes(), 0);
    }

    #[test]
    fn replacement_second_write_failure_restores_prior_pair() {
        let kc = ScriptedKeychain::new().script(&[
            StoreOutcome::OkLands,
            StoreOutcome::OkLands,
            StoreOutcome::FailSilent,
        ]);
        kc.seed(OLD_CREDIT, OLD_KEY);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            OLD_CREDIT,
            "credit slot after failed second write",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            OLD_KEY,
            "api key slot after failed second write",
        );
        assert_eq!(kc.canonical_deletes(), 0);
    }

    #[test]
    fn replacement_readback_mismatch_restores_prior_pair() {
        // The api key write claims success but never lands, so the
        // read-back sees the old key and mismatches.
        let kc = ScriptedKeychain::new().script(&[
            StoreOutcome::OkLands,
            StoreOutcome::OkLands,
            StoreOutcome::OkButDrops,
        ]);
        kc.seed(OLD_CREDIT, OLD_KEY);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            OLD_CREDIT,
            "credit slot after read-back mismatch",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            OLD_KEY,
            "api key slot after read-back mismatch",
        );
        assert_eq!(kc.canonical_deletes(), 0);
    }

    #[test]
    fn replacement_with_identical_values_succeeds() {
        // Decoy reactivation restores the SAME pair that is already stored:
        // staging, writes, and read-back must all succeed and the journal
        // must not linger afterwards.
        let kc = ScriptedKeychain::new();
        kc.seed(OLD_CREDIT, OLD_KEY);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(OLD_CREDIT, OLD_KEY);
        store.store_provisioned(&credit, &key).unwrap();
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            OLD_CREDIT,
            "credit slot after same-value restore",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            OLD_KEY,
            "api key slot after same-value restore",
        );
        assert!(!kc.journal_present());
    }

    #[test]
    fn staging_failure_aborts_without_mutating_canonical_credentials() {
        // The very first write is the journal; if it cannot be staged and
        // verified, canonical slots must not be touched at all.
        let kc = ScriptedKeychain::new().script(&[StoreOutcome::FailSilent]);
        kc.seed(OLD_CREDIT, OLD_KEY);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert_eq!(
            kc.store_order(),
            vec!["journal"],
            "no canonical write may happen before verified staging"
        );
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            OLD_CREDIT,
            "credit slot after aborted staging",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            OLD_KEY,
            "api key slot after aborted staging",
        );
        assert_eq!(kc.canonical_deletes(), 0);
    }

    #[test]
    fn lying_reads_abort_before_canonical_writes() {
        // Every load returns a mutated value: staging cannot be verified,
        // so the attempt aborts with the underlying canonical bytes intact.
        let kc = ScriptedKeychain::new().load_mutates();
        kc.seed(OLD_CREDIT, OLD_KEY);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert_eq!(kc.store_order(), vec!["journal"]);
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            OLD_CREDIT,
            "credit bytes survive an unreadable read channel",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            OLD_KEY,
            "api key bytes survive an unreadable read channel",
        );
        assert_eq!(kc.canonical_deletes(), 0);
    }

    // ── iOS-style destructive store (delete-before-add) ─────────────────────

    #[test]
    fn ios_delete_before_failed_add_first_write_restores_pair() {
        // The platform deletes the old credit_id before a SecItemAdd that
        // then fails. The untouched api key must never be rewritten (its
        // rewrite could lose it the same way), and the credit slot must be
        // restored from the staged journal's prior pair.
        let kc = ScriptedKeychain::new()
            .destructive_stores()
            .script(&[StoreOutcome::OkLands, StoreOutcome::FailSilent]);
        kc.seed(OLD_CREDIT, OLD_KEY);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert_eq!(
            kc.store_order(),
            vec!["journal", "credit", "credit"],
            "the untouched api key slot must never be rewritten"
        );
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            OLD_CREDIT,
            "credit slot restored after destructive failed add",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            OLD_KEY,
            "untouched old api key survives rollback",
        );
        assert_eq!(kc.canonical_deletes(), 0);
    }

    #[test]
    fn ios_rollback_skips_untouched_slot_and_keeps_old_key() {
        // First write lands but reports failure; rollback rewrites only the
        // credit slot. On a destructive platform, rewriting the matching
        // api key slot would risk destroying the only good device key.
        let kc = ScriptedKeychain::new()
            .destructive_stores()
            .script(&[StoreOutcome::OkLands, StoreOutcome::FailButLands]);
        kc.seed(OLD_CREDIT, OLD_KEY);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert_eq!(
            kc.store_order(),
            vec!["journal", "credit", "credit"],
            "only the diverging slot is converged"
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            OLD_KEY,
            "old api key survives a destructive rollback",
        );
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            OLD_CREDIT,
            "credit slot converged back",
        );
    }

    // ── journal retention and recovery ──────────────────────────────────────

    #[test]
    fn rollback_failure_retains_journal_then_recovers_prior_pair() {
        // The credit write lands-but-fails and the rollback rewrite fails:
        // canonical ends mixed, the journal is retained as the sole
        // recoverable copy, and recover_interrupted() then restores the
        // prior pair and clears the journal.
        let kc = ScriptedKeychain::new().script(&[
            StoreOutcome::OkLands,      // journal
            StoreOutcome::FailButLands, // credit write: lands, reports false
            StoreOutcome::FailSilent,   // rollback rewrite of credit fails
        ]);
        kc.seed(OLD_CREDIT, OLD_KEY);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert!(
            kc.journal_present(),
            "unconfirmed rollback must retain the journal"
        );
        assert_eq!(kc.canonical_deletes(), 0);
        // Restart/retry reconciliation:
        assert!(matches!(store.recover_interrupted(), Ok(true)));
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            OLD_CREDIT,
            "prior credit restored from the journal",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            OLD_KEY,
            "prior api key restored from the journal",
        );
        assert!(
            !kc.journal_present(),
            "resolved recovery clears the journal"
        );
    }

    #[test]
    fn recover_interrupted_recognizes_completed_attempt() {
        // Crash between verified success and journal cleanup: canonical
        // matches the journal's target pair, so reconciliation only clears
        // the journal and never rolls back a good replacement.
        let kc = ScriptedKeychain::new();
        kc.seed(NEW_CREDIT, NEW_KEY);
        kc.seed_journal((OLD_CREDIT, OLD_KEY), (NEW_CREDIT, NEW_KEY));
        let store = PpqSecretStore::new(&kc);
        assert!(matches!(store.recover_interrupted(), Ok(true)));
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            NEW_CREDIT,
            "completed replacement must not be rolled back",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            NEW_KEY,
            "completed replacement api key",
        );
        assert!(!kc.journal_present());
    }

    #[test]
    fn recover_interrupted_without_journal_is_noop() {
        let kc = ScriptedKeychain::new();
        let store = PpqSecretStore::new(&kc);
        assert!(matches!(store.recover_interrupted(), Ok(false)));
    }

    #[test]
    fn malformed_journal_is_preserved_and_blocks_attempts() {
        // A structurally invalid journal may be a truncated sole copy of
        // the prior root credential: it must never be deleted blindly.
        // Reconciliation fails recoverably, later attempts abort before
        // mutating anything, and only explicit verified deletion removes it.
        let kc = ScriptedKeychain::new();
        kc.seed(OLD_CREDIT, OLD_KEY);
        kc.state.lock().unwrap().values.insert(
            (JOURNAL_SERVICE.to_string(), JOURNAL_KEY.to_string()),
            "not-a-journal".to_string(),
        );
        let store = PpqSecretStore::new(&kc);
        assert!(matches!(
            store.recover_interrupted(),
            Err(PpqError::StorageFailure)
        ));
        assert!(kc.journal_present(), "malformed journal must be preserved");
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert!(
            kc.store_order().is_empty(),
            "no writes while a malformed journal blocks"
        );
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            OLD_CREDIT,
            "credit slot must stay untouched",
        );
        // Explicit verified deletion is the only way past it.
        store.delete_all_verified().unwrap();
        assert!(!kc.journal_present());
    }

    #[test]
    fn android_false_but_visible_journal_staging_aborts_untouched() {
        // EncryptedSharedPreferences.commit() can return false while the
        // updated value stays readable in memory: staging must require the
        // explicit success, not just a matching read-back.
        let kc = ScriptedKeychain::new().script(&[StoreOutcome::FailButLands]);
        kc.seed(OLD_CREDIT, OLD_KEY);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert_eq!(
            kc.store_order(),
            vec!["journal"],
            "no canonical write after an unconfirmed staging"
        );
        assert_secret(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            OLD_CREDIT,
            "credit slot after false-but-visible staging",
        );
        assert_secret(
            kc.raw_value(API_KEY_SERVICE, API_KEY_KEY),
            OLD_KEY,
            "api key slot after false-but-visible staging",
        );
        assert_eq!(kc.canonical_deletes(), 0);
    }

    #[test]
    fn delete_all_verified_also_removes_journal() {
        let kc = ScriptedKeychain::new();
        kc.seed(OLD_CREDIT, OLD_KEY);
        kc.seed_journal((OLD_CREDIT, OLD_KEY), (NEW_CREDIT, NEW_KEY));
        let store = PpqSecretStore::new(&kc);
        store.delete_all_verified().unwrap();
        assert_absent(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            "credit slot",
        );
        assert_absent(kc.raw_value(API_KEY_SERVICE, API_KEY_KEY), "api key slot");
        assert!(
            !kc.journal_present(),
            "verified deletion covers the journal"
        );
        assert!(kc.journal_delete_attempts() >= 1);
    }

    // ── fresh provisioning (empty store) ────────────────────────────────────

    #[test]
    fn fresh_first_write_failure_reports_and_stays_empty() {
        let kc = ScriptedKeychain::new().script(&[StoreOutcome::FailSilent]);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert!(!store.has_any());
        assert_absent(
            kc.raw_value(CREDIT_ID_SERVICE, CREDIT_ID_KEY),
            "credit slot",
        );
        assert_absent(kc.raw_value(API_KEY_SERVICE, API_KEY_KEY), "api key slot");
    }

    #[test]
    fn fresh_partial_write_is_cleaned_up() {
        let kc = ScriptedKeychain::new().script(&[StoreOutcome::OkLands, StoreOutcome::FailSilent]);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert!(!store.has_any(), "fresh cleanup must clear partial state");
        assert!(kc.canonical_deletes() >= 1, "fresh cleanup must delete");
    }

    #[test]
    fn fresh_readback_drop_is_cleaned_up() {
        let kc = ScriptedKeychain::new().script(&[StoreOutcome::OkLands, StoreOutcome::OkButDrops]);
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert!(!store.has_any());
    }

    #[test]
    fn fresh_cleanup_delete_refused_leaves_partial_state_visible() {
        let kc = ScriptedKeychain::new()
            .script(&[StoreOutcome::OkLands, StoreOutcome::FailSilent])
            .refuse_delete();
        let store = PpqSecretStore::new(&kc);
        let (credit, key) = zero_pair(NEW_CREDIT, NEW_KEY);
        assert!(matches!(
            store.store_provisioned(&credit, &key),
            Err(PpqError::StorageFailure)
        ));
        assert!(
            kc.canonical_deletes() >= 1,
            "cleanup must still attempt deletion"
        );
        assert!(
            store.has_any(),
            "stuck partial state must remain observable"
        );
    }
}
