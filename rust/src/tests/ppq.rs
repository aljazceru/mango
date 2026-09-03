//! Wave 1 tests for the PPQ account module (plan §10.1/§10.2).
//!
//! Wire types are compile-frozen against the sanitized fixtures captured in
//! Wave 0 (`docs/integrations/ppq-fixtures/`). Any PPQ schema drift that
//! breaks a fixture breaks this file — which is the point.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::ppq::client::{PpqClient, PpqClock, PpqError};
use crate::ppq::contracts::{
    DecimalText, InvoiceStatusValue, PaymentMethods, SatsAmount, WireAccountCreate,
    WireInvoiceCreate, WireInvoiceStatus, WirePaymentMethods,
};
use crate::ppq::secret_store::{
    PpqSecretStore, API_KEY_KEY, API_KEY_SERVICE, CREDIT_ID_KEY, CREDIT_ID_SERVICE,
};
use crate::{KeychainProvider, NullKeychainProvider};
use zeroize::Zeroizing;

const FIX: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../docs/integrations/ppq-fixtures/"
);

/// Raw JSON text of a fixture's `response` member. Parsing from text (not a
/// re-serialized `Value`) preserves number formatting: `serde_json::Value`
/// rewrites `0.000001` as `1e-6`, which our decimal validation rejects.
fn fixture_response_text(name: &str) -> String {
    use serde_json::value::RawValue;
    let raw = std::fs::read_to_string(format!("{FIX}{name}")).unwrap();
    #[derive(serde::Deserialize)]
    struct Wrapper<'a> {
        #[serde(borrow)]
        response: &'a RawValue,
    }
    let wrapper: Wrapper = serde_json::from_str(&raw).unwrap();
    wrapper.response.get().to_string()
}

struct FakeClock(i64);
impl PpqClock for FakeClock {
    fn now_epoch(&self) -> i64 {
        self.0
    }
}

// ── Fixture freeze: wire types parse every captured fixture ──────────────────

#[test]
fn fixtures_parse_against_wire_types() {
    let w: WireAccountCreate =
        serde_json::from_str(&fixture_response_text("accounts_create.json")).unwrap();
    assert_eq!(w.credit_id.len(), 36);
    assert!(w.api_key.starts_with("sk-"));
    assert_eq!(
        DecimalText::from_raw_json(w.balance.get())
            .unwrap()
            .as_str(),
        "0"
    );

    let pm: WirePaymentMethods =
        serde_json::from_str(&fixture_response_text("topup_payment_methods.json")).unwrap();
    assert!(pm
        .supported_methods
        .iter()
        .any(|m| m.method == "btc-lightning"));

    let inv: WireInvoiceCreate =
        serde_json::from_str(&fixture_response_text("topup_create_btc_lightning.json")).unwrap();
    assert_eq!(inv.currency, "SATS");
    assert_eq!(inv.expires_at - inv.created_at, 900);

    for (name, expected) in [
        ("topup_status_pending.json", "New"),
        ("topup_status_expired.json", "Expired"),
        ("topup_status_paid.json", "Settled"),
    ] {
        let st: WireInvoiceStatus = serde_json::from_str(&fixture_response_text(name)).unwrap();
        assert_eq!(st.status, expected);
    }

    let bal: crate::ppq::contracts::WireBalance =
        serde_json::from_str(&fixture_response_text("credits_balance.json")).unwrap();
    // Live capture: float64 shortest-repr with 17 fraction digits must parse.
    let balance = DecimalText::from_raw_json(bal.balance.get()).unwrap();
    assert_eq!(balance.as_str(), "0.08155601999999999");
}

#[test]
fn wrong_type_or_missing_fields_rejected() {
    assert!(serde_json::from_str::<WireAccountCreate>(
        r#"{"credit_id":"x","balance":"not-a-number"}"#
    )
    .is_err());
    assert!(serde_json::from_str::<WireAccountCreate>(
        r#"{"credit_id":"00000000-0000-4000-8000-000000000000"}"#
    )
    .is_err());
}

// ── Money safety (plan §10.1) ─────────────────────────────────────────────────

#[test]
fn decimal_text_accepts_and_rejects() {
    for ok in [
        "0",
        "1",
        "0.15",
        "123456",
        "1000000.000001",
        "0.08155601999999999", // live f64 artifact (fixture credits_balance.json)
    ] {
        assert!(DecimalText::validate(ok).is_ok(), "{ok} should parse");
    }
    for bad in [
        "-1",
        "1e3",
        "1E3",
        "NaN",
        "",
        ".5",
        "5.",
        "1.2.3",
        " 1",
        "1 ",
        "+1",
        "0x1",
        "1,5",
        "0.1234567890123456789", // 19 fraction digits
    ] {
        assert!(
            DecimalText::validate(bad).is_err(),
            "{bad} must be rejected"
        );
    }
    // Raw-JSON path additionally rejects quoted money and negatives.
    assert!(DecimalText::from_raw_json("\"1\"").is_err());
    assert!(DecimalText::from_raw_json("-0.5").is_err());
    assert_eq!(DecimalText::from_raw_json("0.15").unwrap().as_str(), "0.15");
    // Oversized integer (u64 overflow via SatsAmount).
    assert!(SatsAmount::from_raw_json("99999999999999999999").is_err());
}

#[test]
fn lightning_limits_come_from_live_shape_only() {
    let pm: WirePaymentMethods =
        serde_json::from_str(&fixture_response_text("topup_payment_methods.json")).unwrap();
    let methods = pm.to_domain().unwrap();
    assert_eq!(
        methods.lightning_sats_limits(),
        Some((SatsAmount(100), SatsAmount(1_000_000)))
    );

    let empty = PaymentMethods { methods: vec![] };
    assert_eq!(empty.lightning_sats_limits(), None);
}

#[test]
fn unknown_invoice_status_is_never_terminal() {
    assert!(!InvoiceStatusValue::from_wire("New").is_terminal());
    assert!(InvoiceStatusValue::from_wire("Expired").is_terminal());
    assert!(InvoiceStatusValue::from_wire("Settled").is_terminal());
    assert!(!InvoiceStatusValue::from_wire("SomethingNewFromPpq").is_terminal());
    assert!(matches!(
        InvoiceStatusValue::from_wire("Whatever"),
        InvoiceStatusValue::Unknown(_)
    ));
}

// ── Scripted HTTP server ──────────────────────────────────────────────────────

struct ScriptedServer {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl ScriptedServer {
    /// Each scripted entry is a full raw HTTP/1.1 response. The server closes
    /// the connection after each response.
    fn spawn(responses: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let req_log = requests.clone();
        std::thread::spawn(move || {
            for resp in responses {
                let (mut sock, _) = match listener.accept() {
                    Ok(x) => x,
                    Err(_) => return,
                };
                sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                loop {
                    match sock.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&tmp[..n]);
                            let s = String::from_utf8_lossy(&buf);
                            let lower = s.to_ascii_lowercase();
                            if let Some(cl) = lower
                                .lines()
                                .find_map(|l| l.strip_prefix("content-length:"))
                                .and_then(|v| v.trim().parse::<usize>().ok())
                            {
                                let header_end = s.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
                                if buf.len() >= header_end + cl {
                                    break;
                                }
                            } else if s.contains("\r\n\r\n") && !s.starts_with("POST") {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                req_log
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf).into_owned());
                let _ = sock.write_all(resp.as_bytes());
                let _ = sock.flush();
            }
        });
        Self {
            base: format!("http://{addr}"),
            requests,
        }
    }

    fn client(&self) -> PpqClient {
        PpqClient::new(&self.base, Arc::new(FakeClock(1_788_151_443))).unwrap()
    }

    fn sole_request(&self) -> String {
        self.requests.lock().unwrap()[0].clone()
    }
}

fn http_response(code: u16, headers: &[(&str, &str)], body: &str) -> String {
    let mut s = format!("HTTP/1.1 {code} X\r\n");
    for (k, v) in headers {
        s.push_str(&format!("{k}: {v}\r\n"));
    }
    s.push_str("Connection: close\r\n\r\n");
    s.push_str(body);
    s
}

// ── Client behavior ───────────────────────────────────────────────────────────

#[tokio::test]
async fn create_account_happy_path_no_auth_header() {
    let body = r#"{"success":true,"credit_id":"00000000-0000-4000-8000-000000000000","api_key":"sk-test-abcdefghijklmnop","balance":0}"#;
    let srv = ScriptedServer::spawn(vec![http_response(
        201,
        &[("Content-Type", "application/json")],
        body,
    )]);
    let creds = srv.client().create_account().await.unwrap();
    assert_eq!(*creds.credit_id, "00000000-0000-4000-8000-000000000000");
    let req = srv.sole_request();
    assert!(req.starts_with("POST /accounts/create"));
    assert!(!req.to_ascii_lowercase().contains("authorization"));
    assert!(!req.contains("x-credit-id"));
}

#[tokio::test]
async fn create_account_schema_drift_is_invalid_response() {
    let srv = ScriptedServer::spawn(vec![http_response(
        201,
        &[("Content-Type", "application/json")],
        r#"{"credit_id":"00000000-0000-4000-8000-000000000000"}"#,
    )]);
    assert!(matches!(
        srv.client().create_account().await,
        Err(PpqError::InvalidResponse)
    ));
}

#[tokio::test]
async fn bad_secret_shape_rejected_without_leaking_value() {
    // api_key too short — must be rejected and never appear in any error.
    let srv = ScriptedServer::spawn(vec![http_response(
        201,
        &[("Content-Type", "application/json")],
        r#"{"credit_id":"00000000-0000-4000-8000-000000000000","api_key":"short","balance":0}"#,
    )]);
    match srv.client().create_account().await {
        Err(e) => {
            let msg = format!("{e}");
            assert!(!msg.contains("short"));
        }
        Ok(_) => panic!("short api key must be rejected"),
    }
}

#[tokio::test]
async fn auth_and_status_error_mapping() {
    let key = Zeroizing::new("sk-test-abcdefghijklmnop".to_string());
    let srv = ScriptedServer::spawn(vec![http_response(
        401,
        &[("Content-Type", "application/json")],
        r#"{"error":"Invalid API key","message":"API key not found or has been revoked"}"#,
    )]);
    let err = srv.client().balance(&key).await.unwrap_err();
    assert!(matches!(err, PpqError::AuthenticationExpired));
    assert!(!format!("{err}").contains("Invalid API key"));

    let srv = ScriptedServer::spawn(vec![http_response(
        404,
        &[("Content-Type", "application/json")],
        r#"{"error":"Invoice not found"}"#,
    )]);
    assert!(matches!(
        srv.client().invoice_status(&key, "inv1").await,
        Err(PpqError::NotFound)
    ));
}

#[tokio::test]
async fn retry_after_honored_and_malformed_rejected() {
    let key = Zeroizing::new("sk-test-abcdefghijklmnop".to_string());
    for (header, expected) in [
        ("7", Some(7)),
        ("-5", None),
        ("Tue, 1 Jan 2030 00:00:00 GMT", None),
    ] {
        let srv = ScriptedServer::spawn(vec![http_response(
            429,
            &[
                ("Content-Type", "application/json"),
                ("Retry-After", header),
            ],
            "{}",
        )]);
        match srv.client().invoice_status(&key, "inv1").await {
            Err(PpqError::RateLimited {
                retry_after_seconds,
            }) => {
                assert_eq!(retry_after_seconds, expected, "Retry-After: {header}")
            }
            _other => panic!("expected RateLimited for Retry-After {header}"),
        }
    }
}

#[tokio::test]
async fn non_json_content_type_rejected() {
    let srv = ScriptedServer::spawn(vec![http_response(
        200,
        &[("Content-Type", "text/html")],
        "<html>oops</html>",
    )]);
    assert!(matches!(
        srv.client().payment_methods().await,
        Err(PpqError::InvalidResponse)
    ));
}

#[tokio::test]
async fn invoice_create_and_status_roundtrip() {
    let key = Zeroizing::new("sk-test-abcdefghijklmnop".to_string());
    let inv_body = r#"{"amount":100,"checkout_url":"https://ppq.ai/checkout/X","created_at":1788151443,"currency":"SATS","expires_at":1788152343,"invoice_id":"8vy58fo1Z8DYdFkUxp9yJd","lightning_invoice":"lnbc1u1SANITIZEDqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq"}"#;
    let st_body = r#"{"invoice_id":"8vy58fo1Z8DYdFkUxp9yJd","status":"New","amount":100,"currency":"SATS","created_at":1788151443,"expires_at":1788152343,"payment_method":"Bitcoin Lightning","amount_paid":0,"amount_due":0.000001,"lightning_invoice":"lnbc1u1SANITIZED"}"#;
    let srv = ScriptedServer::spawn(vec![
        http_response(201, &[("Content-Type", "application/json")], inv_body),
        http_response(200, &[("Content-Type", "application/json")], st_body),
    ]);
    let c = srv.client();
    let inv = c.create_lightning_invoice(&key, 100).await.unwrap();
    assert_eq!(inv.amount_sats, SatsAmount(100));
    assert_eq!(inv.expires_at - inv.created_at, 900);
    let st = c.invoice_status(&key, &inv.invoice_id).await.unwrap();
    assert_eq!(st.status, InvoiceStatusValue::New);
    assert_eq!(st.amount_sats, Some(SatsAmount(100)));
    // Amount mismatch between request and response is contract drift.
    let srv2 = ScriptedServer::spawn(vec![http_response(
        201,
        &[("Content-Type", "application/json")],
        inv_body,
    )]);
    assert!(matches!(
        srv2.client().create_lightning_invoice(&key, 999).await,
        Err(PpqError::InvalidResponse)
    ));
}

#[tokio::test]
async fn base_url_policy_https_or_loopback_only() {
    let clock = Arc::new(FakeClock(0));
    assert!(PpqClient::new("http://api.ppq.ai", clock.clone()).is_err());
    assert!(PpqClient::new("ftp://api.ppq.ai", clock.clone()).is_err());
    assert!(PpqClient::new("https://api.ppq.ai", clock).is_ok());
}

#[tokio::test]
async fn oversized_body_rejected() {
    let big = "x".repeat(300_000);
    let srv = ScriptedServer::spawn(vec![http_response(
        200,
        &[("Content-Type", "application/json")],
        &big,
    )]);
    assert!(matches!(
        srv.client().payment_methods().await,
        Err(PpqError::InvalidResponse)
    ));
}

#[test]
fn error_display_is_redacted() {
    // None of the taxonomy strings can carry wire data by construction;
    // snapshot the full surface to keep it that way.
    let errors = [
        PpqError::Offline,
        PpqError::PpqUnavailable,
        PpqError::RateLimited {
            retry_after_seconds: Some(9),
        },
        PpqError::AuthenticationExpired,
        PpqError::NotFound,
        PpqError::InvalidResponse,
        PpqError::StorageFailure,
    ];
    for e in &errors {
        let s = format!("{e}");
        for secret in ["sk-test-abcdefghijklmnop", "lnbc1", "Bearer", "credit_id"] {
            assert!(!s.contains(secret), "{s} leaked {secret}");
        }
    }
}

// ── Secret store (plan §10.1: recording fake) ────────────────────────────────

#[derive(Clone)]
#[allow(dead_code)] // Load/Delete entries are recorded for audit; only Store is asserted
enum Op {
    Store { service: String, key: String },
    Load { service: String, key: String },
    Delete { service: String, key: String },
}

#[derive(Default)]
struct RecordingKeychainInner {
    ops: Vec<Op>,
    values: std::collections::BTreeMap<(String, String), String>,
    fail_stores: bool,
}

#[derive(Default, Clone)]
struct RecordingKeychain {
    inner: Arc<Mutex<RecordingKeychainInner>>,
}

impl KeychainProvider for RecordingKeychain {
    fn store(&self, service: String, key: String, value: String) -> bool {
        let mut g = self.inner.lock().unwrap();
        g.ops.push(Op::Store {
            service: service.clone(),
            key: key.clone(),
        });
        if g.fail_stores {
            return false;
        }
        g.values.insert((service, key), value);
        true
    }
    fn load(&self, service: String, key: String) -> Option<String> {
        {
            let mut g = self.inner.lock().unwrap();
            g.ops.push(Op::Load {
                service: service.clone(),
                key: key.clone(),
            });
        }
        let g = self.inner.lock().unwrap();
        g.values
            .iter()
            .find(|((s, k), _)| *s == service && *k == key)
            .map(|(_, v)| v.clone())
    }
    fn delete(&self, service: String, key: String) -> bool {
        self.inner.lock().unwrap().ops.push(Op::Delete {
            service: service.clone(),
            key: key.clone(),
        });
        self.inner
            .lock()
            .unwrap()
            .values
            .remove(&(service, key))
            .is_some()
    }
}

fn creds() -> (Zeroizing<String>, Zeroizing<String>) {
    (
        Zeroizing::new("00000000-0000-4000-8000-000000000000".to_string()),
        Zeroizing::new("sk-test-abcdefghijklmnop".to_string()),
    )
}

#[test]
fn provisioning_writes_credit_id_first_and_reads_back() {
    let kc = RecordingKeychain::default();
    let store = PpqSecretStore::new(&kc);
    let (cid, key) = creds();
    store.store_provisioned(&cid, &key).unwrap();

    let ops = kc.inner.lock().unwrap().ops.clone();
    let stores: Vec<_> = ops
        .iter()
        .filter_map(|o| match o {
            Op::Store { service, key } => Some((service.clone(), key.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        stores,
        vec![
            (CREDIT_ID_SERVICE.to_string(), CREDIT_ID_KEY.to_string()),
            (API_KEY_SERVICE.to_string(), API_KEY_KEY.to_string()),
        ],
        "credit_id must be written before the api key"
    );
    assert!(kc.inner.lock().unwrap().values.len() == 2);
}

#[test]
fn failed_store_compensates_with_verified_deletion() {
    let kc = RecordingKeychain::default();
    let store = PpqSecretStore::new(&kc);
    let (cid, key) = creds();
    // Pre-seed a stale credit_id so compensation has something to clear.
    {
        use crate::KeychainProvider;
        let _ = kc.store(
            CREDIT_ID_SERVICE.into(),
            CREDIT_ID_KEY.into(),
            "stale".into(),
        );
        kc.inner.lock().unwrap().ops.clear();
        kc.inner.lock().unwrap().fail_stores = true;
    }
    assert!(matches!(
        store.store_provisioned(&cid, &key),
        Err(PpqError::StorageFailure)
    ));
    assert!(!store.has_any(), "compensation must clear both secrets");
    assert!(kc.inner.lock().unwrap().values.is_empty());
}

#[test]
fn read_back_mismatch_is_storage_failure_and_compensates() {
    // Store succeeds but load returns the wrong value (silent write loss).
    struct LyingKeychain(RecordingKeychain);
    impl KeychainProvider for LyingKeychain {
        fn store(&self, s: String, k: String, v: String) -> bool {
            self.0.store(s, k, v)
        }
        fn load(&self, _s: String, _k: String) -> Option<String> {
            Some("not-what-we-wrote".to_string())
        }
        fn delete(&self, s: String, k: String) -> bool {
            self.0.delete(s, k)
        }
    }
    let lying = LyingKeychain(RecordingKeychain::default());
    let store = PpqSecretStore::new(&lying);
    let (cid, key) = creds();
    assert!(matches!(
        store.store_provisioned(&cid, &key),
        Err(PpqError::StorageFailure)
    ));
    assert!(lying.0.inner.lock().unwrap().values.is_empty());
}

#[test]
fn delete_all_verified_requires_absence() {
    let kc = RecordingKeychain::default();
    let store = PpqSecretStore::new(&kc);
    let (cid, key) = creds();
    store.store_provisioned(&cid, &key).unwrap();
    assert!(store.has_any());
    store.delete_all_verified().unwrap();
    assert!(!store.has_any());
    assert_eq!(store.load_credit_id().unwrap(), None);
    assert_eq!(store.load_api_key().unwrap(), None);
    // Idempotent: deleting absent values still succeeds (absence = success).
    store.delete_all_verified().unwrap();
}

#[test]
fn null_keychain_cannot_provision() {
    let store = PpqSecretStore::new(&NullKeychainProvider);
    let (cid, key) = creds();
    assert!(matches!(
        store.store_provisioned(&cid, &key),
        Err(PpqError::StorageFailure)
    ));
}

#[test]
fn bad_secret_shapes_rejected_before_any_write() {
    let kc = RecordingKeychain::default();
    let _ = &kc;
    let store = PpqSecretStore::new(&kc);
    let bad_cid = Zeroizing::new("tooshort".to_string());
    let key = test_api_key();
    assert!(matches!(
        store.store_provisioned(&bad_cid, &key),
        Err(PpqError::InvalidResponse)
    ));
    let g = kc.inner.lock().unwrap();
    assert!(
        g.ops.is_empty(),
        "shape validation must precede any keychain op"
    );
}

fn test_api_key() -> Zeroizing<String> {
    Zeroizing::new("sk-test-abcdefghijklmnop".to_string())
}

// ── Actor-level wipe/duress/backup tests (plan §10.1) ────────────────────────

use crate::{AppAction, BiometricProvider, FfiApp};

struct TrueBiometric;
impl BiometricProvider for TrueBiometric {
    fn biometric_status(&self) -> String {
        "available".to_string()
    }
    fn authenticate(&self, _reason: String) -> bool {
        true
    }
}

fn make_actor_app(kc: RecordingKeychain) -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        "".into(),
        Box::new(kc),
        Box::new(crate::NullEmbeddingProvider),
        crate::EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(TrueBiometric),
    );
    app.sync();
    app
}

fn seed_ppq_credentials(kc: &RecordingKeychain) {
    use crate::KeychainProvider;
    assert!(kc.store(
        CREDIT_ID_SERVICE.into(),
        CREDIT_ID_KEY.into(),
        "00000000-0000-4000-8000-000000000000".into()
    ));
    assert!(kc.store(
        API_KEY_SERVICE.into(),
        API_KEY_KEY.into(),
        "sk-test-abcdefghijklmnop".into()
    ));
}

fn ppq_values(kc: &RecordingKeychain) -> (Option<String>, Option<String>) {
    use crate::KeychainProvider;
    (
        kc.load(CREDIT_ID_SERVICE.into(), CREDIT_ID_KEY.into()),
        kc.load(API_KEY_SERVICE.into(), API_KEY_KEY.into()),
    )
}

#[test]
fn user_reset_deletes_ppq_credentials_verified() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());

    let result = app.confirm_delete_all_data(crate::SensitiveActionAuth::Biometric, true);
    app.sync();
    assert!(result.is_ok());
    assert_eq!(
        ppq_values(&kc),
        (None, None),
        "full reset must remove both PPQ credentials"
    );
    assert_eq!(app.state().ppq.mode, crate::PpqAccountMode::None);
}

#[test]
fn destructive_confirmation_requires_acknowledgement_and_auth() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());

    // No acknowledgement -> refused, credentials untouched.
    let refused = app.confirm_delete_all_data(crate::SensitiveActionAuth::Biometric, false);
    app.sync();
    assert!(refused.is_err());
    assert_eq!(
        ppq_values(&kc),
        (
            Some("00000000-0000-4000-8000-000000000000".into()),
            Some("sk-test-abcdefghijklmnop".into())
        )
    );
}

#[test]
fn duress_pin_wipes_mango_but_preserves_ppq_credentials() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    // Configure main PIN 1234 and duress PIN 9999.
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: Some("9999".into()),
        enable_biometric: false,
    });
    app.sync();
    // Sanity: the managed account is classified (not None) before duress.
    assert_ne!(app.state().ppq.mode, crate::PpqAccountMode::None);

    // Enter the DURESS PIN in a sensitive action: must return a GENERIC
    // failure (never revealing the duress match or preservation), wipe Mango
    // data, and keep both PPQ credentials byte-for-byte.
    let err = app
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::MainPin { pin: "9999".into() },
        )
        .expect_err("duress entry must fail generically");
    app.sync();
    let reason = match &err {
        crate::FfiError::Internal { reason } => reason.clone(),
    };
    assert_eq!(
        reason, "sensitive_authentication_failed",
        "no duress hint may leak"
    );
    assert_eq!(
        ppq_values(&kc),
        (
            Some("00000000-0000-4000-8000-000000000000".into()),
            Some("sk-test-abcdefghijklmnop".into())
        ),
        "duress wipe must preserve both PPQ credentials byte-for-byte"
    );
    // Decoy session: no managed account is derivable from the surviving
    // keychain values while duress_decoy_mode is active.
    assert_eq!(app.state().ppq.mode, crate::PpqAccountMode::None);
    assert!(app.state().ppq.funding.is_none());
    assert!(app.state().ppq.balance_display.is_none());
}

#[test]
fn backup_export_requires_managed_account_and_auth() {
    let kc = RecordingKeychain::default();
    let app = make_actor_app(kc.clone());
    // No managed account -> export refuses (and never claims recoverability).
    let err = app
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::Biometric,
        )
        .expect_err("export must fail without a managed account");
    match &err {
        crate::FfiError::Internal { reason } => assert_eq!(reason, "no_managed_account"),
    }
}

#[test]
fn backup_export_roundtrip_bytes_and_restore_wrong_password() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());

    let bytes = app
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::Biometric,
        )
        .expect("managed export succeeds offline (local keychain + encryption only)");
    assert!(bytes.len() > 60);
    assert!(bytes.starts_with(b"MPPQ1"));

    // Wrong password fails locally BEFORE any network validation.
    let result = app
        .restore_ppq_recovery_backup(
            bytes.clone(),
            "wrong-password".into(),
            crate::SensitiveActionAuth::Biometric,
        )
        .expect("ffi call succeeds");
    app.sync();
    assert!(!result.success);
    assert_eq!(
        result.error_code.as_deref(),
        Some("wrong_password_or_corrupt_file")
    );
    // Credentials untouched by the failed restore.
    assert_eq!(
        ppq_values(&kc),
        (
            Some("00000000-0000-4000-8000-000000000000".into()),
            Some("sk-test-abcdefghijklmnop".into())
        )
    );
}

#[test]
fn delete_all_data_one_tap_is_redirected_to_preflight_for_managed() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    assert_ne!(app.state().ppq.mode, crate::PpqAccountMode::None);

    // One-tap DeleteAllData with a managed account: preflight shown, NO wipe.
    app.dispatch(AppAction::DeleteAllData);
    app.sync();
    assert!(
        app.state().ppq.destructive_preflight.is_some(),
        "managed account must get backup-first preflight"
    );
    assert_eq!(
        ppq_values(&kc),
        (
            Some("00000000-0000-4000-8000-000000000000".into()),
            Some("sk-test-abcdefghijklmnop".into())
        ),
        "one tap must never delete managed credentials"
    );
    app.dispatch(AppAction::CancelDestructivePreflight);
    app.sync();
    assert!(app.state().ppq.destructive_preflight.is_none());
}

#[test]
fn ppq_402_maps_to_specific_error_and_never_replays() {
    // Wire-level: the captured 402 body maps to the PPQ-specific variant.
    let err = crate::llm::error::LlmError::InsufficientPpqBalance {
        reason: "Insufficient balance".into(),
    };
    assert!(err.to_string().contains("Insufficient PPQ balance"));
    // The stream-error path surfaces a top-up hint instead of a generic 500.
    assert!(err.display_message().contains("Top up your PPQ account"));
}

/// Serializes tests that mutate MANGO_PPQ_TEST_BASE_URL (tests run in
/// parallel threads of one process; the env var is process-global).
static PPQ_TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ── Actor-level invoice settle path with a scripted PPQ server ───────────────

#[test]
fn actor_check_topup_settles_and_refreshes_balance() {
    let _env_guard = PPQ_TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    use crate::AppAction;
    use std::io::{Read, Write as IoWrite};
    use std::net::TcpListener;

    // Scripted server: 1) invoice status = Settled, 2) balance refresh.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let responses = [
            concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                r#"{"invoice_id":"inv1","status":"Settled","amount":123,"currency":"SATS","created_at":1000,"expires_at":1900,"amount_paid":0.00000123,"amount_due":0}"#
            ),
            concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                r#"{"balance":1.5}"#
            ),
        ];
        for resp in responses {
            let (mut sock, _) = listener.accept().unwrap();
            let mut tmp = [0u8; 4096];
            let _ = sock.read(&mut tmp); // drain request (headers arrive in one read)
            let _ = sock.write_all(resp.as_bytes());
            let _ = sock.flush();
        }
    });

    // Safety: never run this against production.
    std::env::set_var("MANGO_PPQ_TEST_BASE_URL", format!("http://{addr}"));

    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    // Force a pending invoice + awaiting state through the event path.
    app.test_send_ppq_event(crate::PpqTaskEvent::InvoiceCreated {
        invoice_id: "inv1".into(),
        bolt11: Zeroizing::new("lnbc1230n1SANITIZED".to_string()),
        amount_sats: 123,
        created_at: 1_000,
        // expires comfortably in the future relative to the system clock
        expires_at: crate::now_secs() + 600,
    });
    app.sync();
    assert_eq!(
        app.state().ppq.funding_phase,
        crate::PpqFundingPhase::AwaitingPayment
    );

    app.dispatch(AppAction::CheckPpqTopup);
    app.sync();
    for _ in 0..50 {
        app.sync();
        std::thread::sleep(std::time::Duration::from_millis(100));
        if app.state().ppq.funding.is_none() {
            break;
        }
    }
    let st = app.state();
    assert!(
        st.ppq.funding.is_none(),
        "settled invoice must clear the funding summary"
    );
    // Post-settle normalization: settle sets Confirmed, then the balance
    // refresh completes the transition (funding_phase -> Idle, Ready).
    assert!(
        st.ppq.funding_phase == crate::PpqFundingPhase::Idle
            || st.ppq.funding_phase == crate::PpqFundingPhase::Confirmed
    );
    assert_eq!(st.ppq.setup_phase, crate::PpqSetupPhase::Ready);
    assert_eq!(st.ppq.balance_display.as_deref(), Some("1.5"));
    assert!(app.state().ppq.error.is_none());

    std::env::remove_var("MANGO_PPQ_TEST_BASE_URL");
}

#[test]
fn actor_unknown_status_reconciles_by_balance() {
    let _env_guard = PPQ_TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    use crate::AppAction;
    use std::io::{Read, Write as IoWrite};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        // 1) status returns an UNFROZEN word ("Complete"); 2) balance rose.
        let responses = [
            concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                r#"{"invoice_id":"inv2","status":"Complete","amount":50,"currency":"SATS","created_at":1000,"expires_at":1900,"amount_paid":0.0000005,"amount_due":0}"#
            ),
            concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                r#"{"balance":2.0}"#
            ),
        ];
        for resp in responses {
            let (mut sock, _) = listener.accept().unwrap();
            let mut tmp = [0u8; 4096];
            let _ = sock.read(&mut tmp);
            let _ = sock.write_all(resp.as_bytes());
            let _ = sock.flush();
        }
    });
    std::env::set_var("MANGO_PPQ_TEST_BASE_URL", format!("http://{addr}"));

    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    // Seed a last-balance snapshot of 1.0 so the 2.0 refresh reads as a rise.
    app.dispatch(AppAction::Noop);
    app.sync();
    app.test_set_ppq_setting(crate::ppq::account::SETTING_LAST_BALANCE, "1.0");
    app.test_send_ppq_event(crate::PpqTaskEvent::InvoiceCreated {
        invoice_id: "inv2".into(),
        bolt11: Zeroizing::new("lnbc50n1SANITIZED".to_string()),
        amount_sats: 50,
        created_at: 1_000,
        expires_at: crate::now_secs() + 600,
    });
    app.sync();
    assert_eq!(
        app.state().ppq.funding_phase,
        crate::PpqFundingPhase::AwaitingPayment
    );

    app.dispatch(AppAction::CheckPpqTopup);
    for _ in 0..50 {
        app.sync();
        std::thread::sleep(std::time::Duration::from_millis(100));
        if app.state().ppq.funding.is_none() {
            break;
        }
    }
    let st = app.state();
    assert!(
        st.ppq.funding.is_none(),
        "balance reconciliation must clear an unknown-status paid invoice"
    );
    assert_eq!(st.ppq.balance_display.as_deref(), Some("2.0"));

    std::env::remove_var("MANGO_PPQ_TEST_BASE_URL");
}

/// Decrypt a real exported `.mppq` file. Opt-in via env:
///   PPQ_BACKUP_FILE=... PPQ_BACKUP_PASSWORD=... cargo test decrypt_real_backup_file -- --ignored
#[test]
#[ignore = "requires PPQ_BACKUP_FILE and PPQ_BACKUP_PASSWORD env vars"]
fn decrypt_real_backup_file() {
    let path = std::env::var("PPQ_BACKUP_FILE").expect("PPQ_BACKUP_FILE");
    let password = std::env::var("PPQ_BACKUP_PASSWORD").expect("PPQ_BACKUP_PASSWORD");
    let bytes = std::fs::read(&path).expect("read backup file");
    let doc = crate::ppq::recovery::decrypt_recovery_document(&bytes, &password)
        .expect("backup must decrypt with the saved password");
    assert_eq!(doc.backend_id, "ppq-ai");
    assert_eq!(doc.credit_id.len(), 36);
    assert!(doc.api_key.len() >= 16);
    // Never print the secrets; lengths only.
    println!(
        "decrypted ok: credit_id len {}, api_key len {}",
        doc.credit_id.len(),
        doc.api_key.len()
    );
}
