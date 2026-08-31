//! Typed PPQ account HTTP client (plan §6.7).
//!
//! Rules enforced here:
//! - Injectable base URL + clock; HTTPS required outside loopback test fakes.
//! - Connect/total timeouts and a hard response-body cap.
//! - Only expected success codes and JSON content types are accepted.
//! - Errors form a small redacted taxonomy: authorization headers, request
//!   bodies, raw response bodies, `credit_id`, API keys, and BOLT11 values
//!   never appear in error strings or logs.
//! - `Retry-After` on 429/503 is parsed into `RateLimited` so callers can
//!   persist a `next_poll_at` deadline. Reads are idempotent; account and
//!   invoice creation is never retried automatically.

use std::sync::Arc;
use std::time::Duration;

use super::contracts::{
    AccountCredentials, CreatedKey, DecimalText, InvoiceStatus, InvoiceStatusValue,
    LightningInvoice, PaymentMethods, SatsAmount, WireAccountCreate, WireBalance,
    WireInvoiceCreate, WireInvoiceStatus, WireKeyObject, WireKeysResponse, WirePaymentMethods,
};
use zeroize::Zeroizing;

pub const PRODUCTION_BASE_URL: &str = "https://api.ppq.ai";

/// Hard cap on any PPQ response body (fixtures are < 4 KiB).
const MAX_RESPONSE_BYTES: usize = 256 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Redacted public error taxonomy (plan §6.7 subset used by the client and
/// secret store; later waves surface it across UniFFI).
#[derive(Debug, thiserror::Error)]
pub enum PpqError {
    #[error("network unreachable")]
    Offline,
    #[error("PPQ service unavailable")]
    PpqUnavailable,
    #[error("rate limited")]
    RateLimited { retry_after_seconds: Option<i64> },
    #[error("authentication expired or invalid")]
    AuthenticationExpired,
    #[error("resource not found")]
    NotFound,
    #[error("PPQ response did not match the agreed contract")]
    InvalidResponse,
    #[error("local secure storage failure")]
    StorageFailure,
}

/// Injectable clock so invoice expiry / Retry-After math is testable.
pub trait PpqClock: Send + Sync {
    fn now_epoch(&self) -> i64;
}

pub struct SystemPpqClock;

impl PpqClock for SystemPpqClock {
    fn now_epoch(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

enum WireAuth<'a> {
    None,
    Bearer(&'a Zeroizing<String>),
    CreditId(&'a Zeroizing<String>),
}

pub struct PpqClient {
    http: reqwest::Client,
    base_url: String,
    #[allow(dead_code)] // used by Wave 2+ (invoice polling / next_poll_at math)
    clock: Arc<dyn PpqClock>,
}

impl PpqClient {
    /// Build a client for `base_url`. HTTPS is required unless the host is a
    /// loopback address (local scripted test server).
    pub fn new(base_url: &str, clock: Arc<dyn PpqClock>) -> Result<Self, PpqError> {
        let trimmed = base_url.trim_end_matches('/');
        let scheme_ok = trimmed.starts_with("https://");
        let host = trimmed
            .strip_prefix("https://")
            .or_else(|| trimmed.strip_prefix("http://"))
            .unwrap_or("")
            .split([':', '/'])
            .next()
            .unwrap_or("")
            .to_string();
        let is_loopback = matches!(host.as_str(), "127.0.0.1" | "localhost" | "::1");
        if !scheme_ok && !is_loopback {
            return Err(PpqError::InvalidResponse);
        }
        if host.is_empty() {
            return Err(PpqError::InvalidResponse);
        }

        let http =
            crate::net::tls::plain_https_client(REQUEST_TIMEOUT).map_err(|_| PpqError::Offline)?;

        Ok(Self {
            http,
            base_url: trimmed.to_string(),
            clock,
        })
    }

    pub fn production(clock: Arc<dyn PpqClock>) -> Result<Self, PpqError> {
        Self::new(PRODUCTION_BASE_URL, clock)
    }

    /// `POST /accounts/create` — unauthenticated, called exactly once per
    /// provisioning attempt (plan §6.8; no automatic retries).
    pub async fn create_account(&self) -> Result<AccountCredentials, PpqError> {
        let (_status, _headers, body) = self
            .request("POST", "/accounts/create", WireAuth::None, None, 201)
            .await?;
        let wire: WireAccountCreate = parse_json(&body)?;
        let credentials = AccountCredentials {
            credit_id: Zeroizing::new(wire.credit_id),
            api_key: Zeroizing::new(wire.api_key),
            balance: DecimalText::from_raw_json(wire.balance.get())
                .map_err(|_| PpqError::InvalidResponse)?,
        };
        validate_secret_shape(&credentials.credit_id, 30..=40)?;
        validate_secret_shape(&credentials.api_key, 16..=128)?;
        Ok(credentials)
    }

    /// `GET /topup/payment-methods` — unauthenticated.
    pub async fn payment_methods(&self) -> Result<PaymentMethods, PpqError> {
        let (_status, _headers, body) = self
            .request("GET", "/topup/payment-methods", WireAuth::None, None, 200)
            .await?;
        let wire: WirePaymentMethods = parse_json(&body)?;
        wire.to_domain().map_err(|_| PpqError::InvalidResponse)
    }

    /// `POST /credits/balance` with Bearer auth (no `credit_id` in the body,
    /// plan §2 table).
    pub async fn balance(&self, api_key: &Zeroizing<String>) -> Result<DecimalText, PpqError> {
        let (_status, _headers, body) = self
            .request(
                "POST",
                "/credits/balance",
                WireAuth::Bearer(api_key),
                None,
                200,
            )
            .await?;
        let wire: WireBalance = parse_json(&body)?;
        DecimalText::from_raw_json(wire.balance.get()).map_err(|_| PpqError::InvalidResponse)
    }

    /// `POST /topup/create/btc-lightning` with integer sats (MVP only form).
    pub async fn create_lightning_invoice(
        &self,
        api_key: &Zeroizing<String>,
        amount_sats: u64,
    ) -> Result<LightningInvoice, PpqError> {
        let body_json = serde_json::json!({ "amount": amount_sats, "currency": "SATS" });
        let (_status, _headers, body) = self
            .request(
                "POST",
                "/topup/create/btc-lightning",
                WireAuth::Bearer(api_key),
                Some(body_json.to_string()),
                201,
            )
            .await?;
        let wire: WireInvoiceCreate = parse_json(&body)?;
        if wire.currency != "SATS" {
            return Err(PpqError::InvalidResponse);
        }
        let amount =
            SatsAmount::from_raw_json(wire.amount.get()).map_err(|_| PpqError::InvalidResponse)?;
        if amount.0 != amount_sats {
            return Err(PpqError::InvalidResponse);
        }
        if wire.expires_at <= wire.created_at || wire.lightning_invoice.len() < 20 {
            return Err(PpqError::InvalidResponse);
        }
        Ok(LightningInvoice {
            invoice_id: wire.invoice_id,
            bolt11: Zeroizing::new(wire.lightning_invoice),
            amount_sats: amount,
            created_at: wire.created_at,
            expires_at: wire.expires_at,
            checkout_url: wire.checkout_url,
        })
    }

    /// `GET /topup/status/{invoice_id}` with Bearer auth. Idempotent read;
    /// eligible for Retry-After-driven scheduling.
    pub async fn invoice_status(
        &self,
        api_key: &Zeroizing<String>,
        invoice_id: &str,
    ) -> Result<InvoiceStatus, PpqError> {
        let path = format!("/topup/status/{invoice_id}");
        let (_status, _headers, body) = self
            .request("GET", &path, WireAuth::Bearer(api_key), None, 200)
            .await?;
        let wire: WireInvoiceStatus = parse_json(&body)?;
        let amount_sats = match (&wire.amount, &wire.currency) {
            (Some(raw), Some(c)) if c == "SATS" => SatsAmount::from_raw_json(raw.get()).ok(),
            _ => None,
        };
        let amount_paid = wire
            .amount_paid
            .as_deref()
            .map(|raw| DecimalText::from_raw_json(raw.get()))
            .transpose()
            .map_err(|_| PpqError::InvalidResponse)?
            .unwrap_or(DecimalText::validate("0").unwrap());
        if wire.expires_at <= wire.created_at {
            return Err(PpqError::InvalidResponse);
        }
        Ok(InvoiceStatus {
            invoice_id: wire.invoice_id,
            status: InvoiceStatusValue::from_wire(&wire.status),
            amount_sats,
            amount_paid,
            created_at: wire.created_at,
            expires_at: wire.expires_at,
            lightning_invoice: Zeroizing::new(wire.lightning_invoice.unwrap_or_default()),
        })
    }

    /// `POST /keys` with the root credential — the agreed recovery path when
    /// a stored device key is revoked (verified live against PPQ, see
    /// `docs/integrations/ppq-orchestration-approval.md` §2.3).
    pub async fn create_device_key(
        &self,
        credit_id: &Zeroizing<String>,
        name: &str,
    ) -> Result<CreatedKey, PpqError> {
        let body_json = serde_json::json!({ "name": name });
        let (_status, _headers, body) = self
            .request(
                "POST",
                "/keys",
                WireAuth::CreditId(credit_id),
                Some(body_json.to_string()),
                201,
            )
            .await?;
        let wire: WireKeysResponse<WireKeyObject> = parse_json(&body)?;
        let key = wire.data;
        let api_key = key.api_key.ok_or(PpqError::InvalidResponse)?;
        validate_secret_shape(&Zeroizing::new(api_key.clone()), 16..=128)?;
        Ok(CreatedKey {
            key_id: key._id,
            name: key.name,
            api_key: Zeroizing::new(api_key),
            created_at: key.created_at,
        })
    }

    // ── internals ────────────────────────────────────────────────────────────

    async fn request(
        &self,
        method: &str,
        path: &str,
        auth: WireAuth<'_>,
        body: Option<String>,
        expected_code: u16,
    ) -> Result<(u16, reqwest::header::HeaderMap, Zeroizing<Vec<u8>>), PpqError> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self
            .http
            .request(
                reqwest::Method::from_bytes(method.as_bytes())
                    .map_err(|_| PpqError::InvalidResponse)?,
                &url,
            )
            // Client identifier per approval artifact §1 item 4 (no specific
            // format requested by PPQ; adjust on request).
            .header(reqwest::header::USER_AGENT, "Mango/0.3");
        if let Some(b) = &body {
            req = req
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(b.clone());
        }
        req = match &auth {
            WireAuth::None => req,
            WireAuth::Bearer(key) => req.bearer_auth(key.as_str()),
            WireAuth::CreditId(id) => req.header("x-credit-id", id.as_str()),
        };

        let response = req.send().await.map_err(classify_network)?;
        let status = response.status();
        let headers = response.headers().clone();

        let mut buf: Vec<u8> = Vec::with_capacity(2048);
        let mut stream = response;
        while let Some(chunk) = stream.chunk().await.map_err(classify_network)? {
            if buf.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(PpqError::InvalidResponse);
            }
            buf.extend_from_slice(&chunk);
        }
        let body = Zeroizing::new(buf);

        let code = status.as_u16();
        if code == expected_code {
            let ct = headers
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if !ct.starts_with("application/json") {
                return Err(PpqError::InvalidResponse);
            }
            return Ok((code, headers, body));
        }

        match code {
            401 | 403 => Err(PpqError::AuthenticationExpired),
            404 => Err(PpqError::NotFound),
            429 => Err(PpqError::RateLimited {
                retry_after_seconds: parse_retry_after(&headers),
            }),
            503 => Err(PpqError::RateLimited {
                retry_after_seconds: parse_retry_after(&headers),
            }),
            500..=599 => Err(PpqError::PpqUnavailable),
            _ => Err(PpqError::InvalidResponse),
        }
    }
}

fn classify_network(e: reqwest::Error) -> PpqError {
    if e.is_connect() || e.is_timeout() {
        PpqError::Offline
    } else {
        PpqError::PpqUnavailable
    }
}

/// Parse `Retry-After` as delta-seconds. Rejects negative and absurd values;
/// callers clamp to a reviewed maximum (plan §6.9 step 6).
fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<i64> {
    let raw = headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .to_string();
    // Only the delta-seconds form is honored; HTTP-date form yields None and
    // the caller falls back to its default backoff.
    let secs: i64 = raw.parse().ok()?;
    if !(0..=86_400).contains(&secs) {
        return None;
    }
    Some(secs)
}

fn parse_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, PpqError> {
    serde_json::from_slice(body).map_err(|_| PpqError::InvalidResponse)
}

/// Validate secret shape (length bounds, no control characters, plausible
/// character set). The value itself is never included in any error.
pub(crate) fn validate_secret_shape(
    value: &Zeroizing<String>,
    bounds: std::ops::RangeInclusive<usize>,
) -> Result<(), PpqError> {
    let len = value.len();
    if !bounds.contains(&len) {
        return Err(PpqError::InvalidResponse);
    }
    if value
        .chars()
        .any(|c| c.is_ascii_control() || c.is_whitespace())
    {
        return Err(PpqError::InvalidResponse);
    }
    Ok(())
}
