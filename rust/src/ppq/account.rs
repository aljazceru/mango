//! PPQ account orchestration state machine (plan §6.8/§6.9, Waves 2/4/5).
//!
//! Pure domain logic: the actor in `lib.rs` owns the client/secret-store
//! instances and calls into here so every transition is unit-testable
//! without the actor loop. Secrets never leave `Zeroizing` wrappers and
//! never enter `AppState`.

use serde::{Deserialize, Serialize};

use super::client::{PpqClient, PpqError};
use super::contracts::{InvoiceStatusValue, SatsAmount};
use super::secret_store::PpqSecretStore;

/// Nonsecret persisted metadata keys (existing encrypted `settings` table —
/// plan §8 allows "nonsecret settings or a small PPQ table"; settings
/// avoids a schema migration).
pub const SETTING_MODE: &str = "ppq_account_mode"; // "managed" | "external"
pub const SETTING_BACKUP_AT: &str = "ppq_backup_confirmed_at";
pub const SETTING_FIRST_FUNDING_REMIND: &str = "ppq_first_funding_reminder_shown_at";
pub const SETTING_LAST_BALANCE: &str = "ppq_last_balance";
pub const SETTING_BALANCE_AT: &str = "ppq_last_balance_updated_at";
pub const SETTING_PENDING_INVOICE: &str = "ppq_pending_invoice";
pub const SETTING_CREATED_AT: &str = "ppq_account_created_at";

/// Safe pending-invoice metadata persisted for process-death recovery
/// (plan §6.9 step 4/6). `bolt11` is a payment request, not a credential.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PendingInvoice {
    pub invoice_id: String,
    pub bolt11: String,
    pub amount_sats: u64,
    pub created_at: i64,
    pub expires_at: i64,
    /// Earliest permitted next status poll (epoch secs), from Retry-After.
    pub next_poll_at: i64,
}

/// Startup classification (plan §8 migration matrix). Caller MUST have
/// already short-circuited on `duress_decoy_mode` — this function never
/// inspects the duress flag itself (decoy sessions must not touch PPQ).
#[derive(Debug, PartialEq, Eq)]
pub enum StartupMode {
    /// Fresh install / no PPQ data at all.
    None,
    /// PPQ backend key present but no root credential (BYOK user).
    ExternalKey,
    /// Both credentials + managed marker: healthy managed account.
    Managed,
    /// Both credentials but no managed marker: crash between secret-store
    /// write and marker write — repairable, never auto-delete (§6.2).
    RecoverablePartialState,
    /// Marker says managed but a credential is missing: recovery required.
    RecoveryRequired,
}

pub fn classify_startup(secrets: &PpqSecretStore<'_>, settings_mode: Option<&str>) -> StartupMode {
    let credit = secrets.load_credit_id().unwrap_or(None);
    let key = secrets.load_api_key().unwrap_or(None);
    match (credit.is_some(), key.is_some(), settings_mode) {
        // Marker promises a managed account but a credential is missing:
        // recovery required — never auto-provision a replacement (§8 rule 4).
        (_, false, Some("managed")) | (false, true, Some("managed")) => {
            StartupMode::RecoveryRequired
        }
        (false, false, _) => StartupMode::None,
        (false, true, _) => StartupMode::ExternalKey,
        (true, true, Some("managed")) => StartupMode::Managed,
        (true, _, _) => StartupMode::RecoverablePartialState,
    }
}

/// Provisioning (§6.8). `already_managed` guard, single call, ordered
/// secret-store persistence, then balance fetch. Returns the created
/// balance on success. The caller reloads backend config and writes the
/// `Managed` marker via settings ONLY after this returns Ok (order: §6.2
/// puts the marker last).
pub async fn provision_account(
    client: &PpqClient,
    secrets: &PpqSecretStore<'_>,
    already_managed: bool,
) -> Result<String, PpqError> {
    if already_managed {
        return Err(PpqError::InvalidResponse); // caller maps to explicit-replace-required
    }
    let creds = client.create_account().await?;
    secrets.store_provisioned(&creds.credit_id, &creds.api_key)?;
    let balance = client.balance(&creds.api_key).await?;
    Ok(balance.as_str().to_string())
}

/// Validate a sats amount against live Lightning limits (§6.9 steps 1-2).
/// Returns the (min,max) actually used, so callers never fall back to
/// hardcoded numbers.
pub async fn validate_sats_against_limits(
    client: &PpqClient,
    amount_sats: u64,
) -> Result<(u64, u64), PpqError> {
    let methods = client.payment_methods().await?;
    let (min, max) = methods
        .lightning_sats_limits()
        .ok_or(PpqError::InvalidResponse)?;
    let _ = SatsAmount(amount_sats); // u64 already integral
    if amount_sats < min.0 || amount_sats > max.0 {
        return Err(PpqError::InvalidResponse); // caller shows live range
    }
    Ok((min.0, max.0))
}

/// Decide whether a status poll may run now (§6.9 step 6: never before
/// `next_poll_at`, including timer, manual, and resume checks).
pub fn poll_allowed(pending: &PendingInvoice, now_epoch: i64) -> bool {
    now_epoch >= pending.next_poll_at && now_epoch < pending.expires_at
}

/// Map an observed status to the safe funding outcome (§6.9 steps 9-11).
#[derive(Debug, PartialEq, Eq)]
pub enum FundingOutcome {
    /// Still awaiting payment.
    Pending,
    /// Terminal, paid: clear pending, refresh balance, transition Ready.
    Settled,
    /// Terminal, unpaid: disable wallet/copy, offer new invoice.
    Expired,
    /// Unknown value: reconcile conservatively via balance when possible.
    Unknown,
}

pub fn outcome_for(status: &InvoiceStatusValue) -> FundingOutcome {
    match status {
        InvoiceStatusValue::New => FundingOutcome::Pending,
        InvoiceStatusValue::Settled => FundingOutcome::Settled,
        InvoiceStatusValue::Expired => FundingOutcome::Expired,
        InvoiceStatusValue::Unknown(_) => FundingOutcome::Unknown,
    }
}

/// Balance-reconciliation for unknown statuses (§6.9 step 11): if the
/// balance rose above the last persisted snapshot, treat as funded.
pub fn reconcile_unknown_by_balance(last: Option<&str>, current: &str) -> FundingOutcome {
    match last {
        Some(prev) if decimals_gt(current, prev) => FundingOutcome::Settled,
        _ => FundingOutcome::Unknown,
    }
}

/// Compare two non-negative decimal strings numerically without floats.
fn decimals_gt(a: &str, b: &str) -> bool {
    let (ai, af) = split_decimal(a);
    let (bi, bf) = split_decimal(b);
    if ai != bi {
        return ai > bi;
    }
    let af = pad(af, bf.len());
    let bf = pad(bf, af.len());
    af > bf
}

fn split_decimal(s: &str) -> (String, String) {
    let (i, f) = match s.split_once('.') {
        Some((i, f)) => (i, f),
        None => (s, ""),
    };
    // Threat review (low): normalize leading zeros so "01.00" == "1.00".
    let whole = i.trim_start_matches('0');
    (whole.to_string(), f.to_string())
}

fn pad(s: String, len: usize) -> String {
    let mut s = s;
    while s.len() < len {
        s.push('0');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_matrix() {
        use crate::ppq::secret_store::PpqSecretStore;
        use crate::{KeychainProvider, NullKeychainProvider};
        // Recording map keychain
        struct Map(std::sync::Mutex<std::collections::BTreeMap<(String, String), String>>);
        impl KeychainProvider for Map {
            fn store(&self, s: String, k: String, v: String) -> bool {
                self.0.lock().unwrap().insert((s, k), v);
                true
            }
            fn load(&self, s: String, k: String) -> Option<String> {
                self.0.lock().unwrap().get(&(s, k)).cloned()
            }
            fn delete(&self, s: String, k: String) -> bool {
                self.0.lock().unwrap().remove(&(s, k)).is_some()
            }
        }
        let mk = |credit: bool, key: bool, mode: Option<&str>| {
            let m = Map(std::sync::Mutex::new(std::collections::BTreeMap::new()));
            if credit {
                m.store("mango.ppq".into(), "credit-id-v1".into(), "c".into());
            }
            if key {
                m.store("mango".into(), "ppq-ai".into(), "k".into());
            }
            let store = PpqSecretStore::new(&m);
            classify_startup(&store, mode)
        };
        assert_eq!(mk(false, false, None), StartupMode::None);
        assert_eq!(mk(false, true, Some("external")), StartupMode::ExternalKey);
        assert_eq!(mk(true, true, Some("managed")), StartupMode::Managed);
        assert_eq!(mk(true, true, None), StartupMode::RecoverablePartialState);
        assert_eq!(
            mk(true, false, Some("managed")),
            StartupMode::RecoveryRequired
        );
        // Null keychain can never classify as managed/external.
        let store = PpqSecretStore::new(&NullKeychainProvider);
        assert_eq!(
            classify_startup(&store, Some("managed")),
            StartupMode::RecoveryRequired
        );
    }

    #[test]
    fn poll_deadline_and_outcomes() {
        let p = PendingInvoice {
            invoice_id: "i".into(),
            bolt11: "lnbc".into(),
            amount_sats: 100,
            created_at: 0,
            expires_at: 900,
            next_poll_at: 700,
        };
        assert!(!poll_allowed(&p, 699));
        assert!(poll_allowed(&p, 700));
        assert!(!poll_allowed(&p, 900), "expired invoice is never polled");

        assert_eq!(
            outcome_for(&InvoiceStatusValue::New),
            FundingOutcome::Pending
        );
        assert_eq!(
            outcome_for(&InvoiceStatusValue::Settled),
            FundingOutcome::Settled
        );
        assert_eq!(
            outcome_for(&InvoiceStatusValue::Expired),
            FundingOutcome::Expired
        );
        assert_eq!(
            outcome_for(&InvoiceStatusValue::Unknown("x".into())),
            FundingOutcome::Unknown
        );
    }

    #[test]
    fn balance_reconciliation_is_decimal_not_float() {
        assert_eq!(
            reconcile_unknown_by_balance(Some("0.000001"), "0.000002"),
            FundingOutcome::Settled
        );
        assert_eq!(
            reconcile_unknown_by_balance(Some("0.08155601999999999"), "0.08155601999999999"),
            FundingOutcome::Unknown
        );
        assert_eq!(
            reconcile_unknown_by_balance(Some("0.1"), "0.09"),
            FundingOutcome::Unknown
        );
        assert_eq!(
            reconcile_unknown_by_balance(None, "0.5"),
            FundingOutcome::Unknown
        );
        // 17-digit float noise compares numerically, not lexically:
        assert_eq!(
            reconcile_unknown_by_balance(Some("0.08155602"), "0.08155602000000001"),
            FundingOutcome::Settled
        );
    }

    #[test]
    fn pending_invoice_roundtrips_json() {
        let p = PendingInvoice {
            invoice_id: "i".into(),
            bolt11: "lnbc1...".into(),
            amount_sats: 100,
            created_at: 1,
            expires_at: 901,
            next_poll_at: 5,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: PendingInvoice = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
        // Secrets contract: no credential fields exist on this struct.
        assert!(!json.contains("credit_id") && !json.contains("api_key"));
    }
}
