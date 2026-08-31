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
//!
//! Duress exception (plan §6.2): duress wipe must NOT call
//! [`PpqSecretStore::delete_all_verified`]; it preserves both values
//! byte-for-byte and suppresses them instead. Verified deletion is reserved
//! for user-requested full reset, provider removal, confirmed replacement,
//! and failed-provisioning compensation.

use zeroize::Zeroizing;

use super::client::{validate_secret_shape, PpqError};

pub const API_KEY_SERVICE: &str = "mango";
pub const API_KEY_KEY: &str = "ppq-ai";
pub const CREDIT_ID_SERVICE: &str = "mango.ppq";
pub const CREDIT_ID_KEY: &str = "credit-id-v1";

/// Verified, ordered writer/reader for the two PPQ account secrets.
///
/// The provisioning order (plan §6.2) is intentionally recoverable:
/// 1. validate shapes, 2. write `credit_id`, 3. write API key,
///    4. read both back and compare, 5. caller writes the nonsecret
///    `Managed` marker.
///
/// A crash between writes leaves the root credential, which can repair or
/// mint a new device key — startup reconciliation must never silently
/// delete such a partial state.
pub struct PpqSecretStore<'a> {
    keychain: &'a dyn crate::KeychainProvider,
}

impl<'a> PpqSecretStore<'a> {
    pub fn new(keychain: &'a dyn crate::KeychainProvider) -> Self {
        Self { keychain }
    }

    /// Store both credentials with checked writes and read-back verification.
    /// On any failure, best-effort verified deletion of both items and
    /// `StorageFailure`.
    pub fn store_provisioned(
        &self,
        credit_id: &Zeroizing<String>,
        api_key: &Zeroizing<String>,
    ) -> Result<(), PpqError> {
        validate_secret_shape(credit_id, 30..=40)?;
        validate_secret_shape(api_key, 16..=128)?;

        if !self.keychain.store(
            CREDIT_ID_SERVICE.into(),
            CREDIT_ID_KEY.into(),
            (&**credit_id).into(),
        ) {
            return self.compensate();
        }
        if !self.keychain.store(
            API_KEY_SERVICE.into(),
            API_KEY_KEY.into(),
            (&**api_key).into(),
        ) {
            return self.compensate();
        }

        // Read-back comparison: catches silent write loss (e.g. async
        // persistence that never landed).
        let read_credit = self
            .keychain
            .load(CREDIT_ID_SERVICE.into(), CREDIT_ID_KEY.into());
        let read_key = self
            .keychain
            .load(API_KEY_SERVICE.into(), API_KEY_KEY.into());
        match (read_credit, read_key) {
            (Some(c), Some(k))
                if constant_time_eq(c.as_bytes(), credit_id.as_bytes())
                    && constant_time_eq(k.as_bytes(), api_key.as_bytes()) =>
            {
                Ok(())
            }
            _ => self.compensate(),
        }
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
    pub fn has_any(&self) -> bool {
        self.keychain
            .load(CREDIT_ID_SERVICE.into(), CREDIT_ID_KEY.into())
            .is_some()
            || self
                .keychain
                .load(API_KEY_SERVICE.into(), API_KEY_KEY.into())
                .is_some()
    }

    /// Verified deletion of both PPQ credentials. Succeeds only after a
    /// read-back confirms both are absent. Called by user-requested full
    /// reset, explicit provider removal, confirmed replacement, and
    /// failed-provisioning compensation — never by duress.
    pub fn delete_all_verified(&self) -> Result<(), PpqError> {
        self.keychain
            .delete(CREDIT_ID_SERVICE.into(), CREDIT_ID_KEY.into());
        self.keychain
            .delete(API_KEY_SERVICE.into(), API_KEY_KEY.into());
        self.verify_absent()
    }

    fn compensate(&self) -> Result<(), PpqError> {
        let _ = self.delete_all_verified();
        Err(PpqError::StorageFailure)
    }

    fn verify_absent(&self) -> Result<(), PpqError> {
        if self
            .keychain
            .load(CREDIT_ID_SERVICE.into(), CREDIT_ID_KEY.into())
            .is_some()
            || self
                .keychain
                .load(API_KEY_SERVICE.into(), API_KEY_KEY.into())
                .is_some()
        {
            return Err(PpqError::StorageFailure);
        }
        Ok(())
    }
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
