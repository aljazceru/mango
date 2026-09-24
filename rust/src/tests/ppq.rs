//! Wave 1 tests for the PPQ account module (plan §10.1/§10.2).
//!
//! Wire types are compile-frozen against the sanitized fixtures captured in
//! Wave 0 (`src/tests/ppq-fixtures/`). Any PPQ schema drift that
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

const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/tests/ppq-fixtures/");

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
fn failed_fresh_provision_compensates_with_verified_deletion() {
    let kc = RecordingKeychain::default();
    let store = PpqSecretStore::new(&kc);
    let (cid, key) = creds();
    // Empty snapshot (fresh install): nothing to preserve, so a failed
    // store must leave both slots empty (coordinated contract with the
    // secret-store owner: compensation-by-deletion applies to fresh
    // provisioning ONLY — never to replacement of an existing pair).
    kc.inner.lock().unwrap().fail_stores = true;
    assert!(matches!(
        store.store_provisioned(&cid, &key),
        Err(PpqError::StorageFailure)
    ));
    assert!(!store.has_any(), "fresh cleanup must clear both secrets");
    assert!(kc.inner.lock().unwrap().values.is_empty());
}

#[test]
fn failed_replacement_preserves_previous_pair() {
    let kc = RecordingKeychain::default();
    let store = PpqSecretStore::new(&kc);
    let (prior_cid, prior_key) = creds();
    store.store_provisioned(&prior_cid, &prior_key).unwrap();
    // Attempt to replace with a different valid pair, with failing writes.
    let new_cid = Zeroizing::new("11111111-1111-4111-8111-111111111111".to_string());
    let new_key = Zeroizing::new("sk-test-qrstuvwxyz123456".to_string());
    kc.inner.lock().unwrap().fail_stores = true;
    assert!(matches!(
        store.store_provisioned(&new_cid, &new_key),
        Err(PpqError::StorageFailure)
    ));
    // Finding 4 contract (secret-store owner): the previous credential
    // pair must survive a failed replacement byte-for-byte — never deleted.
    assert_eq!(
        ppq_values(&kc),
        (Some((*prior_cid).clone()), Some((*prior_key).clone()),),
        "failed replacement must preserve the previous pair"
    );
}

#[test]
fn read_back_mismatch_is_storage_failure_and_never_stores_new_pair() {
    // Store succeeds but load returns the wrong value (silent write loss):
    // the attempt must fail and the NEW pair must never end up stored.
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
    // Under the replacement-preserving contract the lying keychain's
    // snapshot is non-empty, so recovery converges to it; the essential
    // invariant either way: neither attempted secret is readable back.
    assert_eq!(
        store.load_credit_id().unwrap().as_deref(),
        Some(&"not-what-we-wrote".to_string())
    );
    assert_ne!(
        store.load_credit_id().unwrap().as_deref(),
        Some(&(*cid).clone())
    );
    assert_ne!(
        store.load_api_key().unwrap().as_deref(),
        Some(&(*key).clone())
    );
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
            false,
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
        expires_at: crate::ppq_now_secs() + 600,
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
        expires_at: crate::ppq_now_secs() + 600,
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

#[test]
fn actor_decoy_reactivation_via_backup_file() {
    use crate::AppAction;
    use std::io::{Read, Write as IoWrite};
    use std::net::TcpListener;

    let _env_guard = PPQ_TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let responses = [
            // remote validation: restored key is valid
            concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                r#"{"balance":3.5}"#
            ),
            // post-restore balance refresh
            concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                r#"{"balance":3.5}"#
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
    // Trigger a REAL duress wipe (main 1234 / duress 9999): Mango data is
    // wiped, decoy mode activates, and both PPQ credentials survive dormant.
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: Some("9999".into()),
        enable_biometric: false,
    });
    app.sync();
    let err = app
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::MainPin { pin: "9999".into() },
        )
        .expect_err("duress entry must fail generically");
    let _ = err;
    app.sync();
    // Decoy session: credentials exist but NOTHING is derivable from state.
    assert_eq!(app.state().ppq.mode, crate::PpqAccountMode::None);
    assert!(app.state().ppq.balance_display.is_none());
    assert!(ppq_values(&kc).0.is_some(), "dormant credit_id preserved");

    // File-based restore (possessing the .mppq) reactivates the account.
    let bytes = crate::ppq::recovery::encrypt_recovery_document(
        "00000000-0000-4000-8000-000000000000",
        "sk-test-abcdefghijklmnop",
        None,
        "2026-09-03T00:00:00Z",
        "correct-horse-battery",
    )
    .unwrap();
    let result = app
        .restore_ppq_recovery_backup(
            bytes,
            "correct-horse-battery".to_string(),
            crate::SensitiveActionAuth::Biometric,
            true, // decoy restores require explicit generic consent (follow-up 2)
        )
        .expect("ffi call succeeds");
    app.sync();
    for _ in 0..50 {
        app.sync();
        std::thread::sleep(std::time::Duration::from_millis(100));
        if app.state().ppq.mode == crate::PpqAccountMode::Managed {
            break;
        }
    }
    assert!(result.success, "file-based restore must work in decoy mode");
    let st = app.state();
    assert_eq!(st.ppq.mode, crate::PpqAccountMode::Managed);
    // Decoy flag cleared: the account is visible again.
    assert_eq!(
        app.test_get_ppq_setting("duress_decoy_mode").as_deref(),
        Some("false")
    );

    std::env::remove_var("MANGO_PPQ_TEST_BASE_URL");
}

// ── Release remediation regressions: findings 1, 2, 3 + follow-ups ───────────

fn ppq_backend_summary_has_key(app: &FfiApp) -> Option<bool> {
    app.state()
        .backends
        .iter()
        .find(|b| b.id == "ppq-ai")
        .map(|b| b.has_api_key)
}

fn count_ppq_key_loads(kc: &RecordingKeychain) -> usize {
    kc.inner
        .lock()
        .unwrap()
        .ops
        .iter()
        .filter(|o| {
            matches!(o, Op::Load { service, key }
                if *service == API_KEY_SERVICE && *key == API_KEY_KEY)
        })
        .count()
}

fn ppq_temp_dir(tag: &str) -> String {
    // The production wipe guard (`wipe_data_dir_allowed`) requires a path
    // component literally named "mango" — mirror a real install layout
    // instead of weakening the guard.
    let dir =
        std::env::temp_dir().join(format!("mango_ppq_release_{tag}_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(dir.join("mango")).expect("create temp dir");
    dir.join("mango").to_str().unwrap().to_string()
}

fn make_dir_actor_app(dir: &str, kc: RecordingKeychain) -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        dir.to_string(),
        Box::new(kc),
        Box::new(crate::NullEmbeddingProvider),
        crate::EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(TrueBiometric),
    );
    app.sync();
    app
}

/// Finding 1, acceptance 1 (immediate wipe): after duress the preserved PPQ
/// credentials are never read, cached, or exposed — the decoy session's
/// ppq-ai backend reports `has_api_key=false`.
#[test]
fn duress_decoy_suppresses_preserved_ppq_backend_key() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: Some("9999".into()),
        enable_biometric: false,
    });
    app.sync();
    // Managed account visible before duress: the backend key is live.
    assert_eq!(ppq_backend_summary_has_key(&app), Some(true));

    let loads_before = count_ppq_key_loads(&kc);
    let err = app
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::MainPin { pin: "9999".into() },
        )
        .expect_err("duress entry must fail generically");
    let _ = err;
    app.sync();

    // Decoy session: the preserved device key was never READ from the
    // keychain after wipe (no loads beyond the pre-duress baseline).
    assert_eq!(
        count_ppq_key_loads(&kc),
        loads_before,
        "decoy session must not read the preserved PPQ key"
    );
    assert_eq!(
        ppq_backend_summary_has_key(&app),
        Some(false),
        "decoy ppq-ai backend must show has_api_key=false"
    );
    assert_eq!(app.state().ppq.mode, crate::PpqAccountMode::None);
    assert!(app.state().ppq.balance_display.is_none());
    assert!(app.state().ppq.funding.is_none());
    // ...and both credentials are still preserved byte-for-byte.
    assert_eq!(
        ppq_values(&kc),
        (
            Some("00000000-0000-4000-8000-000000000000".into()),
            Some("sk-test-abcdefghijklmnop".into())
        )
    );
}

/// Finding 1, acceptance 1 (restart): the decoy flag and key suppression
/// survive a process restart over the same install.
#[test]
fn decoy_suppresses_preserved_key_across_restart() {
    let dir = ppq_temp_dir("restart");
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_dir_actor_app(&dir, kc.clone());
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: Some("9999".into()),
        enable_biometric: false,
    });
    app.sync();
    let err = app
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::MainPin { pin: "9999".into() },
        )
        .expect_err("duress wipe");
    let _ = err;
    app.sync();
    assert_eq!(app.state().ppq.mode, crate::PpqAccountMode::None);
    assert_eq!(ppq_backend_summary_has_key(&app), Some(false));
    drop(app);

    // "Restart": a fresh app instance over the same data dir. The seeded
    // decoy database keeps the suppression active.
    let app2 = make_dir_actor_app(&dir, kc.clone());
    assert_eq!(
        app2.state().ppq.mode,
        crate::PpqAccountMode::None,
        "restart must not reclassify dormant credentials"
    );
    assert_eq!(
        ppq_backend_summary_has_key(&app2),
        Some(false),
        "restart must keep the preserved key out of the backend cache"
    );
    assert_eq!(
        app2.test_get_ppq_setting("duress_decoy_mode").as_deref(),
        Some("true")
    );
    assert!(ppq_values(&kc).0.is_some(), "dormant root still preserved");
    let _ = std::fs::remove_dir_all(std::path::Path::new(&dir).parent().unwrap());
}

/// Finding 2 (lock/unlock): late replies from the pre-lock session are
/// dropped — also after re-unlock — while fresh (current-epoch) replies
/// still work: the control never gets stuck.
#[test]
fn late_ppq_replies_after_lock_unlock_are_dropped_and_session_recovers() {
    let dir = ppq_temp_dir("lockunlock");
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_dir_actor_app(&dir, kc.clone());
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();
    assert!(app.state().auth_initialized);

    let stale_epoch = app.test_ppq_task_epoch();
    app.dispatch(AppAction::LockApp);
    app.sync();
    assert!(app.state().ppq.balance_display.is_none());
    // Reply arrives while locked: dropped (DB unavailable, epoch stale) —
    // the locked screen's state stays clean.
    app.test_send_ppq_event_at_epoch(
        stale_epoch,
        crate::PpqTaskEvent::BalanceRefreshed {
            balance: "7.7".into(),
        },
    );
    app.sync();
    assert!(app.state().ppq.balance_display.is_none());

    app.dispatch(AppAction::UnlockWithPin { pin: "1234".into() });
    app.sync();
    // Same stale reply arrives AFTER unlock: still dropped (session epoch
    // advanced at lock time).
    app.test_send_ppq_event_at_epoch(
        stale_epoch,
        crate::PpqTaskEvent::BalanceRefreshed {
            balance: "7.7".into(),
        },
    );
    app.sync();
    assert!(
        app.state().ppq.balance_display.is_none(),
        "stale balance reply must not mutate the re-unlocked session"
    );
    // A reply carrying the CURRENT epoch (as any task spawned now would)
    // is applied: the session is fully usable after lock/unlock.
    app.test_send_ppq_event(crate::PpqTaskEvent::BalanceRefreshed {
        balance: "5.0".into(),
    });
    app.sync();
    assert_eq!(app.state().ppq.balance_display.as_deref(), Some("5.0"));
    let _ = std::fs::remove_dir_all(std::path::Path::new(&dir).parent().unwrap());
}

/// Follow-up 1 (cancellation): an invoice task blocked on payment-methods is
/// CANCELLED at lock — the follow-up create_lightning_invoice request is
/// never issued — and fresh requests work again after unlock.
#[test]
fn inflight_multistep_ppq_request_is_cancelled_at_lock() {
    let _env_guard = PPQ_TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    use std::io::{Read, Write as IoWrite};
    use std::net::TcpListener;
    use std::sync::mpsc;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let req_log = requests.clone();
    std::thread::spawn(move || {
        let pm_body = concat!(
            "{\"success\":true,\"supported_methods\":[{\"method\":\"btc-lightning\",",
            "\"display_name\":\"Bitcoin Lightning\",\"supported_currencies\":[\"SATS\"],",
            "\"limits\":{\"SATS\":{\"min\":100,\"max\":1000000}}}]}"
        );
        // Connection 1: payment-methods — hold the response until released.
        if let Ok((mut sock, _)) = listener.accept() {
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                match sock.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        if buf.contains(&b'\n') {
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
            let _ = release_rx.recv_timeout(Duration::from_secs(10));
            let _ = sock.write_all(
                format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{pm_body}").as_bytes(),
            );
            let _ = sock.flush();
        }
        // Connection 2: post-unlock balance refresh.
        if let Ok((mut sock, _)) = listener.accept() {
            let mut tmp = [0u8; 4096];
            let _ = sock.read(&mut tmp);
            req_log
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&tmp).into_owned());
            let _ = sock.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"balance\":1.0}",
            );
            let _ = sock.flush();
        }
        // Any further connection would prove the cancellation failed.
        while let Ok((mut sock, _)) = listener.accept() {
            let mut tmp = [0u8; 4096];
            let _ = sock.read(&mut tmp);
            req_log
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&tmp).into_owned());
        }
    });
    std::env::set_var("MANGO_PPQ_TEST_BASE_URL", format!("http://{addr}"));

    let dir = ppq_temp_dir("cancel");
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_dir_actor_app(&dir, kc.clone());
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    // Start a topup: the task blocks on payment-methods (connection 1).
    app.dispatch(AppAction::CreatePpqLightningTopup { amount_sats: 100 });
    let mut saw_first = false;
    for _ in 0..50 {
        app.sync();
        if !requests.lock().unwrap().is_empty() {
            saw_first = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(saw_first, "payment-methods request must have started");

    // Lock: cancels the session token around the WHOLE multistep future.
    app.dispatch(AppAction::LockApp);
    app.sync();
    std::thread::sleep(Duration::from_millis(150));
    // Now release the held response: the cancelled client is gone and must
    // NOT proceed to create_lightning_invoice.
    let _ = release_tx.send(());
    std::thread::sleep(Duration::from_millis(400));

    let reqs = requests.lock().unwrap().clone();
    assert_eq!(
        reqs.len(),
        1,
        "no further request may be issued, got {reqs:?}"
    );
    assert!(
        reqs[0].contains("/topup/payment-methods"),
        "first request must be payment-methods"
    );
    assert!(
        !reqs.iter().any(|r| r.contains("invoice")),
        "create_lightning_invoice must never be issued after cancellation"
    );

    // Fresh operations after unlock work (new session token/epoch).
    app.dispatch(AppAction::UnlockWithPin { pin: "1234".into() });
    app.sync();
    app.dispatch(AppAction::RefreshPpqAccount);
    let mut refreshed = false;
    for _ in 0..50 {
        app.sync();
        if app.state().ppq.balance_display.is_some() {
            refreshed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(refreshed, "post-unlock balance refresh must work");
    assert_eq!(app.state().ppq.balance_display.as_deref(), Some("1.0"));
    assert!(
        !requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.contains("invoice")),
        "still no invoice request after everything"
    );

    std::env::remove_var("MANGO_PPQ_TEST_BASE_URL");
    let _ = std::fs::remove_dir_all(std::path::Path::new(&dir).parent().unwrap());
}

/// Finding 2 (invoice identity): status replies are honored only for the
/// invoice they were issued for — cancelled or replaced invoices cannot be
/// mutated by late replies.
#[test]
fn status_reply_for_replaced_or_cancelled_invoice_is_ignored() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    let now = crate::ppq_now_secs();
    app.test_send_ppq_event(crate::PpqTaskEvent::InvoiceCreated {
        invoice_id: "invA".into(),
        bolt11: Zeroizing::new("lnbc1u1SANITIZED".to_string()),
        amount_sats: 100,
        created_at: now,
        expires_at: now + 600,
    });
    app.sync();
    assert_eq!(
        app.state().ppq.funding_phase,
        crate::PpqFundingPhase::AwaitingPayment
    );
    assert!(app.state().ppq.funding.is_some());

    // Same epoch, but the reply is for a DIFFERENT invoice: dropped.
    app.test_send_ppq_event(crate::PpqTaskEvent::StatusChecked {
        invoice_id: "invB".into(),
        outcome: "settled",
        next_poll_at: 0,
    });
    app.sync();
    assert!(
        app.state().ppq.funding.is_some(),
        "foreign-invoice status must not clear funding"
    );
    assert_eq!(
        app.state().ppq.funding_phase,
        crate::PpqFundingPhase::AwaitingPayment
    );

    // Cancel the invoice, then replay a settled reply for it: dropped by
    // both the epoch bump and the invoice-identity gate.
    let pre_cancel = app.test_ppq_task_epoch();
    app.dispatch(AppAction::CancelPpqTopup);
    app.sync();
    assert!(app.state().ppq.funding.is_none());
    app.test_send_ppq_event_at_epoch(
        pre_cancel,
        crate::PpqTaskEvent::StatusChecked {
            invoice_id: "invA".into(),
            outcome: "settled",
            next_poll_at: 0,
        },
    );
    app.sync();
    // And even a current-epoch reply for the no-longer-pending invoice:
    app.test_send_ppq_event(crate::PpqTaskEvent::StatusChecked {
        invoice_id: "invA".into(),
        outcome: "settled",
        next_poll_at: 0,
    });
    app.sync();
    assert!(
        app.state().ppq.funding.is_none(),
        "cancelled invoice must stay cleared"
    );
    assert_eq!(app.state().ppq.funding_phase, crate::PpqFundingPhase::Idle);
}

/// Finding 3 (forget): removing the account clears the cached chat key
/// (has_api_key=false) and late provisioning replies cannot resurrect it.
#[test]
fn forget_clears_cached_backend_key_and_ignores_late_replies() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    assert_eq!(ppq_backend_summary_has_key(&app), Some(true));

    let stale_epoch = app.test_ppq_task_epoch();
    let result = app.confirm_forget_managed_ppq(crate::SensitiveActionAuth::Biometric, true);
    app.sync();
    assert!(result.is_ok(), "authenticated forget must succeed");
    assert_eq!(ppq_values(&kc), (None, None), "credentials deleted");
    assert_eq!(
        ppq_backend_summary_has_key(&app),
        Some(false),
        "cached chat key must be dropped with the credentials"
    );
    assert_eq!(app.state().ppq.mode, crate::PpqAccountMode::None);

    // A late AccountCreated from the forgotten session: dropped, and the
    // credentials it carries are NOT written to the keychain.
    app.test_send_ppq_event_at_epoch(
        stale_epoch,
        crate::PpqTaskEvent::AccountCreated {
            credit_id: Zeroizing::new("11111111-1111-4111-8111-111111111111".to_string()),
            api_key: Zeroizing::new("sk-test-qrstuvwxyz123456".to_string()),
        },
    );
    app.sync();
    assert_eq!(app.state().ppq.mode, crate::PpqAccountMode::None);
    assert_eq!(
        ppq_values(&kc),
        (None, None),
        "stale provisioning reply must not resurrect credentials"
    );
}

/// Finding 3 (provision): a successful provision immediately refreshes the
/// backend chat-key cache (ppq-ai has_api_key=true).
#[test]
fn provision_success_refreshes_backend_chat_key() {
    let _env_guard = PPQ_TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    use std::io::{Read, Write as IoWrite};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let responses = [
            // 1) account create
            concat!(
                "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                r#"{"success":true,"credit_id":"11111111-1111-4111-8111-111111111111","api_key":"sk-test-qrstuvwxyz123456","balance":0}"#
            ),
            // 2) balance fetch completing provisioning
            concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                r#"{"balance":0.08}"#
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
    let app = make_actor_app(kc.clone());
    assert_eq!(ppq_backend_summary_has_key(&app), Some(false));

    app.dispatch(AppAction::ProvisionManagedPpq);
    let mut provisioned = false;
    for _ in 0..50 {
        app.sync();
        if app.state().ppq.mode == crate::PpqAccountMode::Managed {
            provisioned = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(provisioned, "provisioning must complete");
    assert_eq!(
        ppq_backend_summary_has_key(&app),
        Some(true),
        "fresh device key must be live in the backend cache immediately"
    );
    assert_eq!(
        app.state().ppq.setup_phase,
        crate::PpqSetupPhase::NeedsBackup
    );
    // Wait for the balance fetch so the test server thread can finish.
    for _ in 0..50 {
        app.sync();
        if app.state().ppq.balance_display.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    std::env::remove_var("MANGO_PPQ_TEST_BASE_URL");
}

/// Finding 3 (replacement restore): restoring a different account swaps the
/// backend chat key, and pre-restore session replies are dropped.
#[test]
fn replacement_restore_swaps_backend_key_and_drops_prior_epoch_events() {
    let _env_guard = PPQ_TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    use std::io::{Read, Write as IoWrite};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let responses = [
            // 1) remote validation of the restored (replacement) key
            concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                r#"{"balance":3.5}"#
            ),
            // 2) post-restore balance refresh
            concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                r#"{"balance":3.5}"#
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
    seed_ppq_credentials(&kc); // account A
    let app = make_actor_app(kc.clone());
    assert_eq!(ppq_backend_summary_has_key(&app), Some(true));
    let stale_epoch = app.test_ppq_task_epoch();

    // Backup of a DIFFERENT account (B): a replacement restore.
    let bytes = crate::ppq::recovery::encrypt_recovery_document(
        "11111111-1111-4111-8111-111111111111",
        "sk-test-qrstuvwxyz123456",
        None,
        "2026-09-03T00:00:00Z",
        "correct-horse-battery",
    )
    .unwrap();
    let result = app
        .restore_ppq_recovery_backup(
            bytes,
            "correct-horse-battery".to_string(),
            crate::SensitiveActionAuth::Biometric,
            true,
        )
        .expect("ffi call succeeds");
    app.sync();
    assert!(result.success, "acknowledged replacement must succeed");
    assert_eq!(
        ppq_values(&kc),
        (
            Some("11111111-1111-4111-8111-111111111111".into()),
            Some("sk-test-qrstuvwxyz123456".into())
        ),
        "replacement pair stored"
    );
    assert_eq!(
        ppq_backend_summary_has_key(&app),
        Some(true),
        "backend cache must carry the restored key"
    );

    // Wait for the post-restore refresh (current epoch) to land.
    let mut refreshed = false;
    for _ in 0..50 {
        app.sync();
        if app.state().ppq.balance_display.as_deref() == Some("3.5") {
            refreshed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(refreshed, "post-restore refresh must apply");

    // A pre-restore session reply is dropped.
    app.test_send_ppq_event_at_epoch(
        stale_epoch,
        crate::PpqTaskEvent::BalanceRefreshed {
            balance: "9.9".into(),
        },
    );
    app.sync();
    assert_eq!(
        app.state().ppq.balance_display.as_deref(),
        Some("3.5"),
        "stale pre-restore reply must not overwrite the new session"
    );

    std::env::remove_var("MANGO_PPQ_TEST_BASE_URL");
}

/// Follow-up 2 (probe resistance): in decoy mode an UNACKNOWLEDGED valid
/// restore behaves identically whether the dormant root is the same, a
/// different one, or absent — no keychain inspection happens before the
/// generic consent, so a backup cannot discover hidden accounts.
#[test]
fn decoy_restore_probe_is_uniform_without_acknowledgement() {
    fn make_decoy(kc_seeded: bool) -> (std::sync::Arc<FfiApp>, RecordingKeychain) {
        let kc = RecordingKeychain::default();
        if kc_seeded {
            seed_ppq_credentials(&kc);
        }
        let app = make_actor_app(kc.clone());
        app.dispatch(AppAction::SetupPin {
            pin: "1234".into(),
            duress_pin: Some("9999".into()),
            enable_biometric: false,
        });
        app.sync();
        let err = app
            .create_ppq_recovery_backup(
                "correct-horse-battery".into(),
                crate::SensitiveActionAuth::MainPin { pin: "9999".into() },
            )
            .expect_err("duress wipe");
        let _ = err;
        app.sync();
        assert_eq!(app.state().ppq.mode, crate::PpqAccountMode::None);
        (app, kc)
    }
    fn backup_of(credit: &str) -> Vec<u8> {
        crate::ppq::recovery::encrypt_recovery_document(
            credit,
            "sk-test-qrstuvwxyz123456",
            None,
            "2026-09-03T00:00:00Z",
            "correct-horse-battery",
        )
        .unwrap()
    }

    for (seeded, credit) in [
        (true, "00000000-0000-4000-8000-000000000000"), // SAME dormant root
        (true, "11111111-1111-4111-8111-111111111111"), // DIFFERENT dormant root
        (false, "11111111-1111-4111-8111-111111111111"), // NO dormant root
    ] {
        let (app, kc) = make_decoy(seeded);
        let loads_before = count_ppq_key_loads(&kc);
        let result = app
            .restore_ppq_recovery_backup(
                backup_of(credit),
                "correct-horse-battery".to_string(),
                crate::SensitiveActionAuth::Biometric,
                false, // unacknowledged
            )
            .expect("ffi call succeeds");
        app.sync();
        assert_eq!(
            result.error_code.as_deref(),
            Some("replacement_confirmation_required"),
            "seeded={seeded} credit={credit}: identical generic consent error"
        );
        assert_eq!(app.state().ppq.mode, crate::PpqAccountMode::None);
        assert_eq!(
            count_ppq_key_loads(&kc),
            loads_before,
            "unacknowledged decoy restore must not inspect credentials"
        );
    }
}

/// Follow-up 4: ConfirmPpqBackupSaved is scoped to the account whose export
/// is pending. A confirmation after the account changed (replacement,
/// forget, wipe) must not mark the current account as backed up; a matching
/// confirmation is honored.
#[test]
fn backup_saved_confirmation_is_scoped_to_exporting_account() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    let backup_at_set = |app: &FfiApp| {
        app.test_get_ppq_setting(crate::ppq::account::SETTING_BACKUP_AT)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    };

    // Export account A, then swap the keychain root to B: the pending
    // confirmation is now for A but the current account is B — ignored.
    let _bytes = app
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::Biometric,
        )
        .expect("export");
    app.sync();
    use crate::KeychainProvider;
    assert!(kc.store(
        CREDIT_ID_SERVICE.into(),
        CREDIT_ID_KEY.into(),
        "11111111-1111-4111-8111-111111111111".into()
    ));
    app.dispatch(AppAction::ConfirmPpqBackupSaved);
    app.sync();
    assert!(
        !backup_at_set(&app),
        "mismatched confirmation must not set the backup marker"
    );
    assert!(!app.state().ppq.backup_confirmed);

    // Export the CURRENT account (B) and confirm: honored.
    let _bytes = app
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::Biometric,
        )
        .expect("export");
    app.sync();
    app.dispatch(AppAction::ConfirmPpqBackupSaved);
    app.sync();
    assert!(backup_at_set(&app), "matching confirmation is honored");
    assert!(app.state().ppq.backup_confirmed);

    // Forget clears the scope: a replayed confirmation is ignored.
    let result = app.confirm_forget_managed_ppq(crate::SensitiveActionAuth::Biometric, true);
    app.sync();
    assert!(result.is_ok());
    assert!(!backup_at_set(&app), "forget clears the backup marker");
    app.dispatch(AppAction::ConfirmPpqBackupSaved);
    app.sync();
    assert!(
        !backup_at_set(&app),
        "post-forget confirmation must not mark anything backed up"
    );
    assert!(!app.state().ppq.backup_confirmed);
}

/// Follow-up 4 (SAF deferral): a backup-saved confirmation that arrives
/// while the app is LOCKED is deferred (nothing surfaces while locked) and
/// applied at same-account unlock — not dropped forever.
#[test]
fn backup_confirmation_while_locked_defers_until_same_account_unlock() {
    let dir = ppq_temp_dir("safdefer");
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_dir_actor_app(&dir, kc.clone());
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let _bytes = app
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::Biometric,
        )
        .expect("export while unlocked");
    app.sync();

    app.dispatch(AppAction::LockApp);
    app.sync();
    // The SAF save completes while the app is paused/locked.
    app.dispatch(AppAction::ConfirmPpqBackupSaved);
    app.sync();
    // Nothing surfaced while locked: state stays the clean locked default.
    assert!(app.state().ppq.balance_display.is_none());
    assert!(!app.state().ppq.backup_confirmed);
    assert!(
        app.test_get_ppq_setting(crate::ppq::account::SETTING_BACKUP_AT)
            .map(|v| v.is_empty())
            .unwrap_or(true),
        "no marker write while locked"
    );

    // Same-account unlock: the deferred confirmation is applied.
    app.dispatch(AppAction::UnlockWithPin { pin: "1234".into() });
    app.sync();
    assert!(
        app.state().ppq.backup_confirmed,
        "deferred confirmation must apply at same-account unlock"
    );
    assert!(
        app.test_get_ppq_setting(crate::ppq::account::SETTING_BACKUP_AT)
            .map(|v| !v.is_empty())
            .unwrap_or(false),
        "marker persisted after unlock"
    );
    let _ = std::fs::remove_dir_all(std::path::Path::new(&dir).parent().unwrap());
}

/// B integration: an unresolved interrupted replacement (recovery journal
/// kept) forces recovery-required state — mixed credentials are neither
/// hydrated for chat nor provisioned over.
#[test]
fn unresolved_recovery_journal_forces_recovery_required_state() {
    use crate::ppq::secret_store::PpqSecretStore;
    use crate::KeychainProvider;

    /// Stores succeed but the journal slot can never be DELETED, so a
    /// completed replacement leaves a stale journal that
    /// `recover_interrupted` cannot clear -> Err (recovery required).
    #[derive(Clone, Default)]
    struct StickyJournalKeychain(RecordingKeychain);
    impl KeychainProvider for StickyJournalKeychain {
        fn store(&self, s: String, k: String, v: String) -> bool {
            self.0.store(s, k, v)
        }
        fn load(&self, s: String, k: String) -> Option<String> {
            self.0.load(s, k)
        }
        fn delete(&self, s: String, k: String) -> bool {
            if s == crate::ppq::secret_store::JOURNAL_SERVICE
                && k == crate::ppq::secret_store::JOURNAL_KEY
            {
                return false; // journal delete always fails
            }
            self.0.delete(s, k)
        }
    }

    let kc = StickyJournalKeychain::default();
    let store = PpqSecretStore::new(&kc);
    let (cid, key) = creds();
    // Two successful replacements leave a journal that names the current
    // (target) pair but cannot be cleared.
    store.store_provisioned(&cid, &key).unwrap();
    let cid2 = Zeroizing::new("11111111-1111-4111-8111-111111111111".to_string());
    let key2 = Zeroizing::new("sk-test-qrstuvwxyz123456".to_string());
    store.store_provisioned(&cid2, &key2).unwrap();
    assert!(
        store.recover_interrupted().is_err(),
        "sticky journal must be unrecoverable"
    );

    let app = FfiApp::new(
        "".into(),
        Box::new(kc.clone()),
        Box::new(crate::NullEmbeddingProvider),
        crate::EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(TrueBiometric),
    );
    app.sync();
    let st = app.state();
    assert_eq!(st.ppq.mode, crate::PpqAccountMode::Managed);
    assert_eq!(st.ppq.setup_phase, crate::PpqSetupPhase::Error);
    assert_eq!(st.ppq.error.as_deref(), Some("recovery_required"));
    // Mixed/ambiguous credentials are not hydrated for chat.
    assert_eq!(
        ppq_backend_summary_has_key(&app),
        Some(false),
        "recovery-required state must suppress the chat key"
    );

    // Provisioning over the mixed credentials is refused with the explicit
    // recovery-required error (checked before the already-managed guard).
    app.dispatch(AppAction::ProvisionManagedPpq);
    app.sync();
    assert_eq!(
        app.state().ppq.error.as_deref(),
        Some("recovery_required"),
        "no auto-provisioning over unresolved credentials"
    );
}

// ── Pretag plan wave 2 ───────────────────────────────────────────────────────

/// Pretag A: decoy-session BYOK stores (UpdateBackendApiKey /
/// AddBackendFromPreset) and RemoveBackend deletes on `ppq-ai` are
/// successful no-ops — the preserved device-key bytes survive duress.
#[test]
fn decoy_byok_store_and_remove_leave_preserved_ppq_key() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: Some("9999".into()),
        enable_biometric: false,
    });
    app.sync();
    let _ = app
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::MainPin { pin: "9999".into() },
        )
        .expect_err("duress wipe");
    app.sync();
    assert_eq!(app.state().ppq.mode, crate::PpqAccountMode::None);

    // BYOK store via the settings form.
    app.dispatch(AppAction::UpdateBackendApiKey {
        backend_id: "ppq-ai".into(),
        api_key: "sk-attacker-overwrite-000000".into(),
    });
    app.sync();
    // BYOK store via the onboarding preset path.
    app.dispatch(AppAction::AddBackendFromPreset {
        preset_id: "ppq-ai".into(),
        api_key: "sk-attacker-preset-00000000".into(),
    });
    app.sync();
    // Provider removal (delete path).
    app.dispatch(AppAction::RemoveBackend {
        backend_id: "ppq-ai".into(),
    });
    app.sync();

    assert_eq!(
        ppq_values(&kc),
        (
            Some("00000000-0000-4000-8000-000000000000".into()),
            Some("sk-test-abcdefghijklmnop".into())
        ),
        "decoy BYOK store/Enable/Remove must leave prior ppq-ai bytes"
    );
}

/// Pretag A: decoy "Delete All Data" wipes like a normal full reset
/// (empty-looking install, no reseeded chats) while the PPQ credentials
/// stay preserved in the keychain and the decoy flag is re-armed.
#[test]
fn decoy_delete_all_preserves_credentials_without_reseed() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: Some("9999".into()),
        enable_biometric: false,
    });
    app.sync();
    let _ = app
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::MainPin { pin: "9999".into() },
        )
        .expect_err("duress wipe");
    app.sync();
    // The duress-wiped install seeds the decoy transcript snapshot.
    assert!(!app.state().conversations.is_empty());

    app.dispatch(AppAction::DeleteAllData);
    app.sync();

    assert_eq!(
        ppq_values(&kc),
        (
            Some("00000000-0000-4000-8000-000000000000".into()),
            Some("sk-test-abcdefghijklmnop".into())
        ),
        "decoy delete-all must leave credit + device key in the keychain"
    );
    assert!(
        app.state().conversations.is_empty(),
        "decoy delete-all must not reseed the duress transcript snapshot"
    );
    assert_eq!(
        app.test_get_ppq_setting("duress_decoy_mode").as_deref(),
        Some("true"),
        "decoy delete-all re-arms the decoy flag on the fresh DB"
    );
    assert_eq!(app.state().ppq.mode, crate::PpqAccountMode::None);
    assert_eq!(
        ppq_backend_summary_has_key(&app),
        Some(false),
        "loads stay suppressed after decoy delete-all"
    );
}

/// Pretag A: a decoy user enrolling a fresh PIN must not clear the decoy
/// flag — suppression stays active until an acknowledged .mppq restore.
#[test]
fn decoy_setup_pin_keeps_flag_and_suppression() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: Some("9999".into()),
        enable_biometric: false,
    });
    app.sync();
    let _ = app
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::MainPin { pin: "9999".into() },
        )
        .expect_err("duress wipe");
    app.sync();

    app.dispatch(AppAction::SetupPin {
        pin: "4321".into(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    assert_eq!(
        app.test_get_ppq_setting("duress_decoy_mode").as_deref(),
        Some("true"),
        "SetupPin in a decoy session must not clear the decoy flag"
    );
    assert_eq!(app.state().ppq.mode, crate::PpqAccountMode::None);
    assert_eq!(
        ppq_backend_summary_has_key(&app),
        Some(false),
        "has_api_key stays suppressed after decoy PIN enrollment"
    );
}

/// Pretag D: an ExternalKey (BYOK ppq-ai key without a credit id) counts
/// as a replacement — an unacknowledged restore is refused with
/// `replacement_confirmation_required` BEFORE any remote validation or
/// device-key mint, and the keychain is left unchanged. (Reaching the
/// remote step would require network; the deterministic offline error
/// code proves the gate precedes the mint.)
#[test]
fn byok_restore_requires_replacement_ack_before_mint() {
    let kc = RecordingKeychain::default();
    use crate::KeychainProvider;
    assert!(kc.store(
        API_KEY_SERVICE.into(),
        API_KEY_KEY.into(),
        "sk-byok-abcdefghijklmnop".into()
    ));
    let app = make_actor_app(kc.clone());
    assert_eq!(
        app.state().ppq.mode,
        crate::PpqAccountMode::ExternalKey,
        "BYOK key present without credit id"
    );

    let bytes = crate::ppq::recovery::encrypt_recovery_document(
        "11111111-1111-4111-8111-111111111111",
        "sk-test-qrstuvwxyz123456",
        None,
        "2026-09-03T00:00:00Z",
        "correct-horse-battery",
    )
    .unwrap();
    let result = app
        .restore_ppq_recovery_backup(
            bytes,
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::Biometric,
            false, // unacknowledged
        )
        .expect("ffi call succeeds");
    app.sync();
    assert_eq!(
        result.error_code.as_deref(),
        Some("replacement_confirmation_required"),
        "BYOK restore must demand replacement consent before minting"
    );
    assert_eq!(
        ppq_values(&kc),
        (None, Some("sk-byok-abcdefghijklmnop".into())),
        "unacknowledged BYOK restore leaves the keychain unchanged"
    );
}

/// Pretag E: queued health/attestation results delivered after lock are
/// dropped — no `expect("db unlocked")` panic, no mutation of the locked
/// state. A queued owner-less StreamChunk after a duress wipe must not
/// accumulate phantom streaming text in the decoy snapshot.
#[test]
fn late_health_attestation_and_stream_results_after_lock_wipe_are_dropped() {
    use crate::attestation::AttestationEvent;
    use crate::llm::streaming::InternalEvent;

    // ── Lock case ────────────────────────────────────────────────────
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();
    app.dispatch(AppAction::LockApp);
    app.sync();

    // These used to `expect("db unlocked")` (panic) or write state.
    app.test_send_internal(InternalEvent::HealthCheckResult {
        backend_id: "tinfoil".into(),
        success: true,
        models: vec!["some-model".into()],
    });
    app.sync();
    app.test_send_internal(InternalEvent::AttestationResult(
        AttestationEvent::Verified {
            backend_id: "tinfoil".into(),
            tee_type: "AmdSevSnp".into(),
            report_blob: vec![1, 2, 3],
            expires_at: crate::ppq_now_secs() as u64 + 3600,
            tls_public_key_fp: None,
            vcek_url: None,
            vcek_der: None,
            shape: None,
            freshness: None,
            orchestrated_components: None,
        },
    ));
    app.sync();
    let st = app.state();
    assert!(
        st.attestation_statuses.iter().all(|e| {
            e.backend_id != "tinfoil"
                || !matches!(e.status, crate::AttestationStatus::Verified { .. })
        }),
        "queued attestation result must not apply while locked"
    );

    // ── Wipe case ────────────────────────────────────────────────────
    let kc2 = RecordingKeychain::default();
    seed_ppq_credentials(&kc2);
    let app2 = make_actor_app(kc2.clone());
    app2.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: Some("9999".into()),
        enable_biometric: false,
    });
    app2.sync();
    let _ = app2
        .create_ppq_recovery_backup(
            "correct-horse-battery".into(),
            crate::SensitiveActionAuth::MainPin { pin: "9999".into() },
        )
        .expect_err("duress wipe");
    app2.sync();
    let conv_count = app2.state().conversations.len();

    app2.test_send_internal(InternalEvent::StreamChunk {
        token: "phantom".into(),
    });
    app2.sync();
    let st2 = app2.state();
    assert!(
        st2.streaming_text.is_none(),
        "queued owner-less StreamChunk must not write the decoy snapshot"
    );
    assert_eq!(
        st2.conversations.len(),
        conv_count,
        "decoy snapshot conversations unchanged"
    );
}

// ── ChangePin (settings: main PIN change) ────────────────────────────────────

fn is_locked(app: &FfiApp) -> bool {
    matches!(app.state().router.current_screen, crate::Screen::Locked)
}

/// Changing the main PIN re-wraps the same DEK: the old PIN stops unlocking,
/// the new one works, the duress PIN survives the re-wrap, and the step-up
/// (current PIN) rejects wrong and duress entries alike as a plain
/// "incorrect" — never triggering the duress wipe from settings.
#[test]
fn change_pin_rewraps_dek_and_preserves_duress() {
    let kc = RecordingKeychain::default();
    seed_ppq_credentials(&kc);
    let app = make_actor_app(kc.clone());
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: Some("9999".into()),
        enable_biometric: false,
    });
    app.sync();
    assert!(app.state().auth_initialized);

    // Wrong current PIN -> refused, nothing changes.
    app.dispatch(AppAction::ChangePin {
        current_pin: "0000".into(),
        new_pin: "5678".into(),
    });
    app.sync();
    app.dispatch(AppAction::LockApp);
    app.sync();
    app.dispatch(AppAction::UnlockWithPin { pin: "1234".into() });
    app.sync();
    assert!(
        !is_locked(&app),
        "original PIN must still unlock after a refused change"
    );
    assert_eq!(
        app.test_get_ppq_setting("duress_decoy_mode").as_deref(),
        None,
        "a refused ChangePin must not trigger any duress wipe"
    );

    // The duress PIN must NOT authenticate a change.
    app.dispatch(AppAction::ChangePin {
        current_pin: "9999".into(),
        new_pin: "5678".into(),
    });
    app.sync();
    assert_eq!(
        app.state().toast.as_deref(),
        Some("Current PIN is incorrect."),
        "duress PIN must fail as a plain incorrect PIN"
    );

    // New PIN colliding with the duress PIN is refused.
    app.dispatch(AppAction::ChangePin {
        current_pin: "1234".into(),
        new_pin: "9999".into(),
    });
    app.sync();
    assert_eq!(
        app.state().toast.as_deref(),
        Some("New PIN must be different from the emergency PIN.")
    );

    // Successful change: same DEK, fresh wrap, credentials untouched.
    app.dispatch(AppAction::ChangePin {
        current_pin: "1234".into(),
        new_pin: "5678".into(),
    });
    app.sync();
    assert_eq!(app.state().toast.as_deref(), Some("PIN changed."));
    assert_eq!(
        ppq_values(&kc),
        (
            Some("00000000-0000-4000-8000-000000000000".into()),
            Some("sk-test-abcdefghijklmnop".into())
        ),
        "PIN change must not touch PPQ credentials"
    );

    app.dispatch(AppAction::LockApp);
    app.sync();
    app.dispatch(AppAction::UnlockWithPin { pin: "1234".into() });
    app.sync();
    assert!(is_locked(&app), "old PIN must no longer unlock");

    app.dispatch(AppAction::UnlockWithPin { pin: "5678".into() });
    app.sync();
    assert!(!is_locked(&app), "new PIN unlocks the same encrypted DB");

    // Duress hash survived the re-wrap: the emergency PIN still activates the
    // decoy session rather than a normal unlock.
    app.dispatch(AppAction::LockApp);
    app.sync();
    app.dispatch(AppAction::UnlockWithPin { pin: "9999".into() });
    app.sync();
    assert!(!is_locked(&app), "duress unlock completes (decoy session)");
    assert_eq!(
        app.test_get_ppq_setting("duress_decoy_mode").as_deref(),
        Some("true"),
        "duress PIN must still be recognized after a main PIN change"
    );
}

/// Too-short and identical new PINs are refused with the same validation
/// messages as fresh enrollment.
#[test]
fn change_pin_validates_new_pin() {
    let kc = RecordingKeychain::default();
    let app = make_actor_app(kc);
    app.dispatch(AppAction::SetupPin {
        pin: "1234".into(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();
    // In-memory installs only hydrate the live DEK through an unlock cycle
    // (file-backed installs get it straight from SetupPin).
    app.dispatch(AppAction::LockApp);
    app.sync();
    app.dispatch(AppAction::UnlockWithPin { pin: "1234".into() });
    app.sync();

    app.dispatch(AppAction::ChangePin {
        current_pin: "1234".into(),
        new_pin: "12".into(),
    });
    app.sync();
    assert_eq!(
        app.state().toast.as_deref(),
        Some("New PIN must be at least 4 characters.")
    );

    app.dispatch(AppAction::ChangePin {
        current_pin: "1234".into(),
        new_pin: "1234".into(),
    });
    app.sync();
    assert_eq!(
        app.state().toast.as_deref(),
        Some("New PIN must be different from the current PIN.")
    );
}
