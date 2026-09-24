//! PPQ wire-only serde models, frozen against the Wave 0 sanitized fixtures
//! in `src/tests/ppq-fixtures/`.
//!
//! Money rules (plan §6.3): PPQ sends balances and limits as bare JSON
//! numbers. They are captured as raw JSON text via `serde_json`'s
//! `RawValue` and validated into [`DecimalText`] — never routed through
//! `f64`. Sats are integer-only [`SatsAmount`].
//!
//! Invoice status vocabulary: only `"New"` has been observed live (fixture
//! `topup_status_pending.json`). Every other value maps to
//! [`InvoiceStatusValue::Unknown`] and MUST be handled conservatively by
//! callers (reconcile via balance, never guess). Extend this enum only when
//! a fixture captures the value.

use serde::Deserialize;
use serde_json::value::RawValue;
use zeroize::Zeroizing;

/// Maximum digits accepted in a decimal money string (whole + fraction).
const MAX_DECIMAL_LEN: usize = 32;
/// Maximum fractional digits accepted. Live capture (fixture
/// `credits_balance.json`, 2026-09-03) shows PPQ serializes credit balances
/// as float64 shortest-repr with up to 17 fraction digits
/// ("0.08155601999999999"); 18 accepts any f64 artifact while still
/// rejecting garbage precision.
const MAX_FRACTION_DIGITS: usize = 18;

/// A validated non-negative decimal number carried as its original text.
///
/// Constructed only through [`DecimalText::from_raw_json`] or
/// [`DecimalText::validate`], which reject negative values, exponents,
/// NaN-like strings, empty/oversized input, and excessive precision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecimalText(String);

impl DecimalText {
    /// Validate decimal text captured from raw JSON (RawValue::get() output,
    /// i.e. still JSON-number-shaped: `0`, `0.15`, possibly `1e3` which we
    /// reject).
    pub fn from_raw_json(raw: &str) -> Result<Self, DecimalError> {
        let trimmed = raw.trim();
        if trimmed.starts_with('"') || trimmed.starts_with('-') {
            return Err(DecimalError::Rejected);
        }
        Self::validate(trimmed)
    }

    /// Validate plain decimal text (`123`, `0.15`).
    pub fn validate(text: &str) -> Result<Self, DecimalError> {
        let mut saw_dot = false;
        let mut fraction_digits = 0usize;
        let mut digits = 0usize;
        for (idx, ch) in text.chars().enumerate() {
            match ch {
                '0'..='9' => {
                    digits += 1;
                    if saw_dot {
                        fraction_digits += 1;
                    }
                }
                '.' if idx > 0 && !saw_dot => saw_dot = true,
                _ => return Err(DecimalError::Rejected),
            }
        }
        if digits == 0
            || fraction_digits > MAX_FRACTION_DIGITS
            || text.len() > MAX_DECIMAL_LEN
            || (saw_dot && fraction_digits == 0)
        {
            return Err(DecimalError::Rejected);
        }
        // Reject strings that are all zeros after a dot ("0." / ".0" shapes
        // are already excluded; "0.000" is fine but "00.1" style is harmless).
        Ok(Self(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DecimalText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("decimal value rejected")]
pub enum DecimalError {
    Rejected,
}

/// Integer satoshis. MVP top-up input and Lightning method limits only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SatsAmount(pub u64);

impl SatsAmount {
    pub fn from_raw_json(raw: &str) -> Result<Self, DecimalError> {
        let text = DecimalText::from_raw_json(raw)?;
        if text.as_str().contains('.') {
            return Err(DecimalError::Rejected);
        }
        text.as_str()
            .parse::<u64>()
            .map(SatsAmount)
            .map_err(|_| DecimalError::Rejected)
    }
}

/// Domain credentials returned by `POST /accounts/create`.
///
/// Both secrets are `Zeroizing`; they never enter `AppState`, logs, or error
/// strings. `balance` is the zero-balance decimal observed at creation.
pub struct AccountCredentials {
    pub credit_id: Zeroizing<String>,
    pub api_key: Zeroizing<String>,
    pub balance: DecimalText,
}

/// One payment method from `GET /topup/payment-methods`.
pub struct PaymentMethod {
    pub method: String,
    pub display_name: String,
    pub supported_currencies: Vec<String>,
    /// Per-currency min/max as validated decimal text (JSON numbers on the wire).
    pub limits: Vec<(String, CurrencyLimits)>,
}

/// Min/max limits for one currency.
pub struct CurrencyLimits {
    pub min: DecimalText,
    pub max: DecimalText,
}

/// Full payment-method response.
pub struct PaymentMethods {
    pub methods: Vec<PaymentMethod>,
}

impl PaymentMethods {
    /// Live Lightning limits for integer-sats validation (plan §6.9 step 1).
    /// Returns `None` if the method or SATS limits are absent/malformed —
    /// callers must not fall back to hardcoded numbers.
    pub fn lightning_sats_limits(&self) -> Option<(SatsAmount, SatsAmount)> {
        let m = self.methods.iter().find(|m| m.method == "btc-lightning")?;
        if !m.supported_currencies.iter().any(|c| c == "SATS") {
            return None;
        }
        let l = m.limits.iter().find(|(c, _)| c == "SATS")?;
        Some((
            SatsAmount::from_raw_json(l.1.min.as_str()).ok()?,
            SatsAmount::from_raw_json(l.1.max.as_str()).ok()?,
        ))
    }
}

/// Observed invoice status vocabulary. Extend only with fixture evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvoiceStatusValue {
    /// Observed (fixture: topup_status_pending.json). Non-terminal.
    New,
    /// Observed (fixture: topup_status_expired.json). Terminal, unpaid.
    Expired,
    /// Observed (fixture: topup_status_paid.json). Terminal, paid & credited.
    Settled,
    /// Any value without a captured fixture. Never treat as terminal.
    Unknown(String),
}

impl InvoiceStatusValue {
    pub fn from_wire(status: &str) -> Self {
        match status {
            "New" => Self::New,
            "Expired" => Self::Expired,
            "Settled" => Self::Settled,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn is_terminal(&self) -> bool {
        // Only fixture-confirmed terminal states return true; Unknown is
        // never terminal (reconcile via balance).
        matches!(self, Self::Expired | Self::Settled)
    }
}

/// Domain invoice status (from `GET /topup/status/{invoice_id}`).
pub struct InvoiceStatus {
    pub invoice_id: String,
    pub status: InvoiceStatusValue,
    pub amount_sats: Option<SatsAmount>,
    pub amount_paid: DecimalText,
    /// Epoch seconds, PPQ clock.
    pub created_at: i64,
    pub expires_at: i64,
    pub lightning_invoice: Zeroizing<String>,
}

/// Domain Lightning invoice (from `POST /topup/create/btc-lightning`).
///
/// `bolt11` is a payment request, not an account credential: it may reach
/// the UI for QR/wallet handoff but is never logged.
pub struct LightningInvoice {
    pub invoice_id: String,
    pub bolt11: Zeroizing<String>,
    pub amount_sats: SatsAmount,
    pub created_at: i64,
    pub expires_at: i64,
    pub checkout_url: Option<String>,
}

/// A device API key created via `POST /keys` (root-credential recovery path,
/// plan §6.2). The key value is returned by PPQ exactly once.
pub struct CreatedKey {
    pub key_id: String,
    pub name: String,
    pub api_key: Zeroizing<String>,
    pub created_at: String,
}

// ── Wire structs (serde-only; never cross UniFFI) ─────────────────────────────

#[derive(Deserialize)]
pub(crate) struct WireAccountCreate {
    pub credit_id: String,
    pub api_key: String,
    pub balance: Box<RawValue>,
}

#[derive(Deserialize)]
pub(crate) struct WirePaymentMethods {
    #[serde(default)]
    pub supported_methods: Vec<WirePaymentMethod>,
}

impl WirePaymentMethods {
    /// Validate into the domain model. Any malformed limit rejects the whole
    /// response (callers must not fall back to hardcoded limits, plan §6.9).
    pub(crate) fn to_domain(&self) -> Result<PaymentMethods, DecimalError> {
        let mut methods = Vec::with_capacity(self.supported_methods.len());
        for m in &self.supported_methods {
            let mut limits = Vec::with_capacity(m.limits.len());
            for (currency, l) in &m.limits {
                limits.push((
                    currency.clone(),
                    CurrencyLimits {
                        min: DecimalText::from_raw_json(l.min.get())?,
                        max: DecimalText::from_raw_json(l.max.get())?,
                    },
                ));
            }
            methods.push(PaymentMethod {
                method: m.method.clone(),
                display_name: m.display_name.clone(),
                supported_currencies: m.supported_currencies.clone(),
                limits,
            });
        }
        Ok(PaymentMethods { methods })
    }
}

#[derive(Deserialize)]
pub(crate) struct WirePaymentMethod {
    pub method: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub supported_currencies: Vec<String>,
    #[serde(default)]
    pub limits: std::collections::BTreeMap<String, WireLimit>,
}

#[derive(Deserialize)]
pub(crate) struct WireLimit {
    pub min: Box<RawValue>,
    pub max: Box<RawValue>,
}

#[derive(Deserialize)]
pub(crate) struct WireBalance {
    pub balance: Box<RawValue>,
}

#[derive(Deserialize)]
pub(crate) struct WireInvoiceCreate {
    pub amount: Box<RawValue>,
    #[serde(default)]
    pub checkout_url: Option<String>,
    pub created_at: i64,
    #[serde(default)]
    pub currency: String,
    pub expires_at: i64,
    pub invoice_id: String,
    pub lightning_invoice: String,
}

#[derive(Deserialize)]
pub(crate) struct WireInvoiceStatus {
    pub invoice_id: String,
    pub status: String,
    #[serde(default)]
    pub amount: Option<Box<RawValue>>,
    #[serde(default)]
    pub currency: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
    #[serde(default)]
    pub amount_paid: Option<Box<RawValue>>,
    #[serde(default)]
    pub lightning_invoice: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct WireKeysResponse<D> {
    pub data: D,
}

#[derive(Deserialize)]
pub(crate) struct WireKeyObject {
    pub _id: String,
    #[serde(default)]
    pub name: String,
    /// Present only on create responses (or when show_key=true, which Mango
    /// never requests).
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub created_at: String,
}
