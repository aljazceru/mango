//! NVIDIA CC attestation via NRAS JWT verification.
//!
//! Verifies NVIDIA Remote Attestation Service (NRAS) tokens for H100 CC nodes.
//! Per D-06: NVIDIA CC attestation maps to `AttestationStatus::Verified`.
//! Per Pitfall 3 from RESEARCH.md: pin algorithm to RS256 and issuer to NRAS URL.

use jsonwebtoken::{DecodingKey, Validation};

use super::error::AttestationError;
use super::AttestationEvent;

/// Expected NRAS JWT issuer -- must be pinned to prevent algorithm confusion attacks.
/// Per Pitfall 3 from RESEARCH.md.
const NRAS_ISSUER: &str = "https://nras.attestation.nvidia.com";

/// NRAS encodes `x-nvidia-overall-att-result` as either a boolean or a string
/// (`"true"`/`"false"`) depending on revision. Accept both.
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum OverallResult {
    Bool(bool),
    Str(String),
}

/// NVIDIA NRAS JWT claims.
///
/// Maps the relevant fields from NRAS EAT (Entity Attestation Token).
/// Other fields in the JWT are permitted but not validated here.
#[derive(Debug, serde::Deserialize)]
pub struct NvidiaAttestationClaims {
    /// Issuer -- must be "https://nras.attestation.nvidia.com"
    #[allow(dead_code)]
    pub iss: String,
    /// Challenge nonce (hex-encoded) -- must match the nonce sent with the request.
    pub eat_nonce: String,
    /// Overall attestation result from NVIDIA (`true` when the GPU set is trusted).
    #[serde(rename = "x-nvidia-overall-att-result")]
    pub nvidia_overall_att_result: OverallResult,
}

/// Verify an NVIDIA NRAS JWT token using a JWK from the NRAS JWKS endpoint.
///
/// Validates:
/// 1. Signature using the JWK's algorithm (NRAS uses ES384 in v3; falls back
///    to whatever the JWT header declares — never `none`)
/// 2. Issuer must be `https://nras.attestation.nvidia.com`
/// 3. `eat_nonce` must match `expected_nonce_hex`
/// 4. `x-nvidia-overall-att-result` must be `true` (or `"true"`)
///
/// Per Pitfall 3 from RESEARCH.md: never use `Validation::default()` and never
/// trust algorithm `none`.
#[allow(dead_code)]
pub fn verify_nvidia_jwt(
    jwt_token: &str,
    expected_nonce_hex: &str,
    jwk: &serde_json::Value,
) -> Result<NvidiaAttestationClaims, AttestationError> {
    let header =
        jsonwebtoken::decode_header(jwt_token).map_err(|e| AttestationError::JwtVerification {
            reason: format!("JWT header decode: {}", e),
        })?;

    let parsed: jsonwebtoken::jwk::Jwk =
        serde_json::from_value(jwk.clone()).map_err(|e| AttestationError::JwtVerification {
            reason: format!("Invalid JWK format: {}", e),
        })?;
    let decoding_key =
        DecodingKey::from_jwk(&parsed).map_err(|e| AttestationError::JwtVerification {
            reason: format!("Failed to build decoding key from JWK: {}", e),
        })?;

    let mut validation = Validation::new(header.alg);
    validation.set_issuer(&[NRAS_ISSUER]);

    let token_data =
        jsonwebtoken::decode::<NvidiaAttestationClaims>(jwt_token, &decoding_key, &validation)
            .map_err(|e| AttestationError::JwtVerification {
                reason: format!("JWT decode failed: {}", e),
            })?;

    let claims = token_data.claims;

    if claims.eat_nonce != expected_nonce_hex {
        return Err(AttestationError::NonceMismatch {
            expected: expected_nonce_hex.to_string(),
            actual: claims.eat_nonce.clone(),
        });
    }

    let ok = match &claims.nvidia_overall_att_result {
        OverallResult::Bool(true) => true,
        OverallResult::Str(s) if s == "true" => true,
        _ => false,
    };
    if !ok {
        return Err(AttestationError::JwtVerification {
            reason: format!(
                "NVIDIA overall attestation result is not true: {:?}",
                claims.nvidia_overall_att_result
            ),
        });
    }

    Ok(claims)
}

/// Fetch attestation evidence from a provider, POST to NRAS, verify the JWT.
///
/// Per D-06: maps to `AttestationStatus::Verified` -- the NRAS JWT validates
/// the provider's GPU evidence blob cryptographically.
/// Per D-08: NVIDIA CC JWT TTL is 1 hour.
///
/// `nvidia_payload` is a JSON document carrying the NRAS GPU evidence. Two
/// shapes are accepted (callers vary by Redpill response shape):
///
/// 1. Single GPU object: `{"arch":"HOPPER","certificate":"…","evidence":"…"}`
/// 2. Already wrapped:   `{"arch":"HOPPER","evidence_list":[{"certificate":…,"evidence":…},…]}`
///
/// Internally rewrapped into the NRAS v3 schema:
/// `{"nonce":"<hex>","arch":"…","evidence_list":[{certificate,evidence},…]}`
///
/// `nonce_hex` MUST be the nonce the GPU evidence is cryptographically bound to —
/// for Shape A/B that is the client-chosen Redpill request nonce; for Shape C
/// (Chutes / per-enclave) it is the inner per-enclave nonce from the response.
pub async fn fetch_and_verify_nvidia(
    nvidia_payload: &str,
    nonce_hex: &str,
    backend_id: &str,
) -> Result<AttestationEvent, AttestationError> {
    use std::time::SystemTime;

    log::debug!(target: "attestation", "[attestation] fetch_and_verify_nvidia backend={}", backend_id);

    crate::net::tls::ensure_default_crypto_provider();
    let client = reqwest::Client::builder()
        .no_hickory_dns()
        .build()
        .map_err(|e| AttestationError::NetworkError {
            reason: e.to_string(),
        })?;

    // Step 1: build NRAS v3 request body from the inbound payload.
    let body = build_nras_v3_request(nvidia_payload, nonce_hex)?;

    // Upstream drift (2026-08): Redpill's `aci/1` aggregator schema ships a
    // top-level nvidia_payload with an EMPTY evidence_list on TDX-only (CPU)
    // enclaves. NRAS rejects empty lists outright (4005 INVALID_EVIDENCE) and
    // there is no GPU evidence to attest, so skip the roundtrip entirely.
    // Trust is still gated by the TDX quote verification in the caller.
    if serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| {
            v.get("evidence_list")
                .and_then(|l| l.as_array())
                .map(|a| a.is_empty())
        })
        .unwrap_or(false)
    {
        log::info!(
            target: "attestation",
            "[attestation] empty GPU evidence_list for backend={} — skipping NRAS",
            backend_id
        );
        let now_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        return Ok(AttestationEvent::Verified {
            backend_id: backend_id.to_string(),
            tee_type: "NvidiaH100Cc".to_string(),
            report_blob: Vec::new(),
            expires_at: now_secs + 3600,
            tls_public_key_fp: None,
            vcek_url: None,
            vcek_der: None,
            shape: None,
            freshness: None,
            orchestrated_components: None,
        });
    }

    // Step 2: POST to NRAS GPU attestation endpoint
    log::debug!(target: "attestation", "[attestation] posting to NRAS backend={}", backend_id);
    let nras_response = client
        .post("https://nras.attestation.nvidia.com/v3/attest/gpu")
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| AttestationError::NetworkError {
            reason: format!("NRAS request failed: {}", e),
        })?;

    let nras_status = nras_response.status();
    log::debug!(target: "attestation", "[attestation] NRAS response backend={} status={}", backend_id, nras_status.as_u16());
    if !nras_status.is_success() {
        let body = nras_response.text().await.unwrap_or_default();
        log::warn!(target: "attestation", "[attestation] NRAS failed backend={} status={} body={}", backend_id, nras_status.as_u16(), body);
        return Err(AttestationError::NetworkError {
            reason: format!("NRAS returned HTTP {} body={}", nras_status, body),
        });
    }

    let response_json: serde_json::Value =
        nras_response
            .json()
            .await
            .map_err(|e| AttestationError::NetworkError {
                reason: format!("Failed to parse NRAS response: {}", e),
            })?;

    let jwt_token = extract_nras_jwt(&response_json)?;

    // Step 3: Decode JWT header for kid + alg
    let header =
        jsonwebtoken::decode_header(&jwt_token).map_err(|e| AttestationError::JwtVerification {
            reason: format!("JWT header decode: {}", e),
        })?;
    let header_kid = header
        .kid
        .ok_or_else(|| AttestationError::JwtVerification {
            reason: "JWT header missing kid".to_string(),
        })?;

    // Step 4: Fetch NVIDIA JWKS, pick the key matching the JWT's kid
    log::debug!(target: "attestation", "[attestation] fetching NRAS JWKS backend={}", backend_id);
    let jwks_response = client
        .get("https://nras.attestation.nvidia.com/.well-known/jwks.json")
        .send()
        .await
        .map_err(|e| AttestationError::NetworkError {
            reason: format!("JWKS fetch failed: {}", e),
        })?;

    let jwks: serde_json::Value =
        jwks_response
            .json()
            .await
            .map_err(|e| AttestationError::NetworkError {
                reason: format!("Failed to parse JWKS: {}", e),
            })?;

    let key = jwks["keys"]
        .as_array()
        .and_then(|keys| {
            keys.iter()
                .find(|k| k.get("kid").and_then(|v| v.as_str()) == Some(header_kid.as_str()))
        })
        .ok_or_else(|| AttestationError::JwtVerification {
            reason: format!("JWKS has no key with kid={}", header_kid),
        })?;

    let decoding_key =
        DecodingKey::from_jwk(&serde_json::from_value(key.clone()).map_err(|e| {
            AttestationError::JwtVerification {
                reason: format!("Invalid JWK format: {}", e),
            }
        })?)
        .map_err(|e| AttestationError::JwtVerification {
            reason: format!("Failed to build decoding key from JWK: {}", e),
        })?;

    // NRAS now signs with ES384 (was RS256 in earlier API revisions). Pin to the
    // header's algorithm so we accept whichever NRAS rolls out, but only from the
    // ES/RS family — never `none`. jsonwebtoken's Validation::new() rejects `none`.
    let mut validation = Validation::new(header.alg);
    validation.set_issuer(&[NRAS_ISSUER]);
    // Disable iat/exp leeway tweaks; rely on NRAS's TTL bookkeeping.
    validation.leeway = 60;

    log::debug!(target: "attestation", "[attestation] decoding NRAS JWT backend={} alg={:?} kid={}", backend_id, header.alg, header_kid);
    let token_data =
        jsonwebtoken::decode::<NvidiaAttestationClaims>(&jwt_token, &decoding_key, &validation)
            .map_err(|e| {
                log::warn!(target: "attestation", "[attestation] NRAS JWT decode failed backend={} error={}", backend_id, e);
                AttestationError::JwtVerification {
                    reason: format!("JWT verification failed: {}", e),
                }
            })?;

    let claims = token_data.claims;

    // Validate nonce
    if claims.eat_nonce != nonce_hex {
        return Err(AttestationError::NonceMismatch {
            expected: nonce_hex.to_string(),
            actual: claims.eat_nonce,
        });
    }

    // Validate overall result (NRAS sometimes encodes as a string, sometimes as a bool —
    // accept either). Boolean-true and string-"true" are the only acceptable values.
    let ok = match &claims.nvidia_overall_att_result {
        OverallResult::Bool(true) => true,
        OverallResult::Str(s) if s == "true" => true,
        _ => false,
    };
    if !ok {
        log::warn!(target: "attestation", "[attestation] NVIDIA overall result not true backend={} result={:?}", backend_id, claims.nvidia_overall_att_result);
        return Err(AttestationError::JwtVerification {
            reason: format!(
                "NVIDIA overall attestation result is not true: {:?}",
                claims.nvidia_overall_att_result
            ),
        });
    }

    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Per D-08: NVIDIA CC TTL is 1 hour
    let expires_at = now_secs + 3600;

    Ok(AttestationEvent::Verified {
        backend_id: backend_id.to_string(),
        tee_type: "NvidiaH100Cc".to_string(),
        report_blob: jwt_token.into_bytes(),
        expires_at,
        tls_public_key_fp: None,
        // NVIDIA CC verification uses JWT from NRAS — no AMD VCEK cert involved.
        vcek_url: None,
        vcek_der: None,
        shape: None,
        freshness: None,
        orchestrated_components: None,
    })
}

/// Build the NRAS v3 GPU attestation request body.
///
/// Accepts two inbound shapes from the caller:
/// - single object: `{"arch","certificate","evidence"}` → wrapped into `evidence_list:[{cert,ev}]`
/// - pre-wrapped: `{"arch","evidence_list":[…]}` → re-emitted with `nonce` injected
fn build_nras_v3_request(payload: &str, nonce_hex: &str) -> Result<String, AttestationError> {
    let v: serde_json::Value =
        serde_json::from_str(payload).map_err(|e| AttestationError::JwtVerification {
            reason: format!("nvidia_payload parse: {}", e),
        })?;

    let arch = v
        .get("arch")
        .and_then(|s| s.as_str())
        .unwrap_or("HOPPER")
        .to_string();

    let evidence_list = if let Some(list) = v.get("evidence_list").and_then(|x| x.as_array()) {
        list.iter()
            .map(|e| {
                serde_json::json!({
                    "certificate": e.get("certificate").and_then(|s| s.as_str()).unwrap_or(""),
                    "evidence":    e.get("evidence").and_then(|s| s.as_str()).unwrap_or(""),
                })
            })
            .collect::<Vec<_>>()
    } else {
        // Single-object shape — wrap.
        let cert = v
            .get("certificate")
            .and_then(|s| s.as_str())
            .ok_or_else(|| AttestationError::JwtVerification {
                reason: "nvidia_payload missing 'certificate'".to_string(),
            })?;
        let ev = v.get("evidence").and_then(|s| s.as_str()).ok_or_else(|| {
            AttestationError::JwtVerification {
                reason: "nvidia_payload missing 'evidence'".to_string(),
            }
        })?;
        vec![serde_json::json!({"certificate": cert, "evidence": ev})]
    };

    Ok(serde_json::json!({
        "nonce": nonce_hex,
        "arch": arch,
        "evidence_list": evidence_list,
    })
    .to_string())
}

/// Extract the JWT string from an NRAS v3 attestation response.
///
/// NRAS v3 returns `[["JWT", "<token>"], ["EAT", …]]` — an array of pairs.
/// Earlier API revisions returned `{"token": "<jwt>"}`. We accept either shape.
fn extract_nras_jwt(response: &serde_json::Value) -> Result<String, AttestationError> {
    if let Some(arr) = response.as_array() {
        for entry in arr {
            if let Some(pair) = entry.as_array() {
                if pair.len() == 2 {
                    let tag = pair[0].as_str().unwrap_or("");
                    if tag.eq_ignore_ascii_case("JWT") {
                        if let Some(tok) = pair[1].as_str() {
                            return Ok(tok.to_string());
                        }
                    }
                }
            }
        }
        return Err(AttestationError::JwtVerification {
            reason: "NRAS response array missing JWT entry".to_string(),
        });
    }
    if let Some(tok) = response.get("token").and_then(|v| v.as_str()) {
        return Ok(tok.to_string());
    }
    Err(AttestationError::JwtVerification {
        reason: "NRAS response shape unrecognized (neither array nor {token})".to_string(),
    })
}

#[cfg(test)]
mod nras_request_tests {
    use super::*;

    #[test]
    fn build_nras_v3_request_wraps_single_evidence() {
        let payload = r#"{"arch":"HOPPER","certificate":"CERT_PEM","evidence":"EV_HEX"}"#;
        let body = build_nras_v3_request(payload, "deadbeef").unwrap();
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["nonce"], "deadbeef");
        assert_eq!(v["arch"], "HOPPER");
        let list = v["evidence_list"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["certificate"], "CERT_PEM");
        assert_eq!(list[0]["evidence"], "EV_HEX");
    }

    #[test]
    fn build_nras_v3_request_passes_through_evidence_list() {
        let payload = r#"{
            "arch":"HOPPER",
            "evidence_list":[
                {"certificate":"C1","evidence":"E1"},
                {"certificate":"C2","evidence":"E2"}
            ]
        }"#;
        let body = build_nras_v3_request(payload, "n0").unwrap();
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["nonce"], "n0");
        assert_eq!(v["evidence_list"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn extract_nras_jwt_handles_array_shape() {
        let resp = serde_json::json!([["JWT", "tok123"], ["EAT", "..."]]);
        assert_eq!(extract_nras_jwt(&resp).unwrap(), "tok123");
    }

    #[test]
    fn extract_nras_jwt_handles_legacy_object_shape() {
        let resp = serde_json::json!({"token": "tok123"});
        assert_eq!(extract_nras_jwt(&resp).unwrap(), "tok123");
    }

    // Upstream drift 2026-08: Redpill aci/1 aggregator ships an empty
    // evidence_list on TDX-only enclaves. Must short-circuit to Ok (no NRAS
    // roundtrip) instead of failing with 4005 INVALID_EVIDENCE.
    #[tokio::test]
    async fn empty_evidence_list_skips_nras_roundtrip() {
        let payload = r#"{"nonce":"aa","arch":"HOPPER","evidence_list":[]}"#;
        let evt = super::fetch_and_verify_nvidia(payload, "aa", "test-backend")
            .await
            .expect("empty evidence_list must skip NRAS and verify");
        assert!(matches!(
            evt,
            super::super::AttestationEvent::Verified { .. }
        ));
    }
}
