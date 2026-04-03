//! NVIDIA CC attestation via NRAS JWT verification.
//!
//! Verifies NVIDIA Remote Attestation Service (NRAS) tokens for H100 CC nodes.
//! Per D-06: NVIDIA CC attestation maps to `AttestationStatus::Verified`.
//! Per Pitfall 3 from RESEARCH.md: pin algorithm to RS256 and issuer to NRAS URL.

use jsonwebtoken::{Algorithm, DecodingKey, Validation};

use super::error::AttestationError;
use super::AttestationEvent;

/// Expected NRAS JWT issuer -- must be pinned to prevent algorithm confusion attacks.
/// Per Pitfall 3 from RESEARCH.md.
const NRAS_ISSUER: &str = "https://nras.attestation.nvidia.com";

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
    /// Overall attestation result from NVIDIA ("true" if GPU is in a trusted state).
    #[serde(rename = "x-nvidia-overall-att-result")]
    pub nvidia_overall_att_result: Option<String>,
}

/// Verify an NVIDIA NRAS JWT token.
///
/// Validates:
/// 1. Signature using `nvidia_public_key_pem` with RS256 algorithm (pinned)
/// 2. Issuer must be `https://nras.attestation.nvidia.com`
/// 3. `eat_nonce` must match `expected_nonce_hex`
/// 4. `x-nvidia-overall-att-result` must be `"true"`
///
/// Per Pitfall 3 from RESEARCH.md: never use `Validation::default()` -- always
/// pin algorithm to RS256 and set explicit issuer validation.
#[allow(dead_code)]
pub fn verify_nvidia_jwt(
    jwt_token: &str,
    expected_nonce_hex: &str,
    nvidia_public_key_pem: &str,
) -> Result<NvidiaAttestationClaims, AttestationError> {
    let decoding_key = DecodingKey::from_rsa_pem(nvidia_public_key_pem.as_bytes()).map_err(|e| {
        AttestationError::JwtVerification {
            reason: format!("Invalid NVIDIA public key PEM: {}", e),
        }
    })?;

    let mut validation = Validation::new(Algorithm::RS256);
    // Pin issuer to prevent JWT confusion with other issuers
    validation.set_issuer(&[NRAS_ISSUER]);

    let token_data =
        jsonwebtoken::decode::<NvidiaAttestationClaims>(jwt_token, &decoding_key, &validation)
            .map_err(|e| AttestationError::JwtVerification {
                reason: format!("JWT decode failed: {}", e),
            })?;

    let claims = token_data.claims;

    // Validate nonce binding -- prevents replay of old NRAS tokens
    if claims.eat_nonce != expected_nonce_hex {
        return Err(AttestationError::NonceMismatch {
            expected: expected_nonce_hex.to_string(),
            actual: claims.eat_nonce.clone(),
        });
    }

    // Validate overall attestation result
    match claims.nvidia_overall_att_result.as_deref() {
        Some("true") => {}
        other => {
            return Err(AttestationError::JwtVerification {
                reason: format!(
                    "NVIDIA overall attestation result is not true: {:?}",
                    other
                ),
            });
        }
    }

    Ok(claims)
}

/// Fetch attestation evidence from a provider, POST to NRAS, verify the JWT.
///
/// Per D-06: maps to `AttestationStatus::Verified` -- the NRAS JWT validates
/// the provider's GPU evidence blob cryptographically.
/// Per D-08: NVIDIA CC JWT TTL is 1 hour.
///
/// `nvidia_payload` is the base64-encoded NRAS evidence blob from the provider's
/// attestation JSON response.
pub async fn fetch_and_verify_nvidia(
    nvidia_payload: &str,
    nonce_hex: &str,
    backend_id: &str,
) -> Result<AttestationEvent, AttestationError> {
    use std::time::SystemTime;

    log::debug!(target: "attestation", "[attestation] fetch_and_verify_nvidia backend={}", backend_id);

    let client = reqwest::Client::builder()
        .hickory_dns(false)
        .build()
        .map_err(|e| AttestationError::NetworkError {
            reason: e.to_string(),
        })?;

    // Step 1: POST nvidia_payload to NRAS GPU attestation endpoint
    log::debug!(target: "attestation", "[attestation] posting to NRAS backend={}", backend_id);
    let nras_response = client
        .post("https://nras.attestation.nvidia.com/v3/attest/gpu")
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .body(serde_json::json!({"evidence": nvidia_payload}).to_string())
        .send()
        .await
        .map_err(|e| AttestationError::NetworkError {
            reason: format!("NRAS request failed: {}", e),
        })?;

    let nras_status = nras_response.status();
    log::debug!(target: "attestation", "[attestation] NRAS response backend={} status={}", backend_id, nras_status.as_u16());
    if !nras_status.is_success() {
        log::warn!(target: "attestation", "[attestation] NRAS failed backend={} status={}", backend_id, nras_status.as_u16());
        return Err(AttestationError::NetworkError {
            reason: format!("NRAS returned HTTP {}", nras_status),
        });
    }

    let response_json: serde_json::Value =
        nras_response.json().await.map_err(|e| AttestationError::NetworkError {
            reason: format!("Failed to parse NRAS response: {}", e),
        })?;

    let jwt_token = response_json["token"]
        .as_str()
        .ok_or_else(|| AttestationError::JwtVerification {
            reason: "NRAS response missing 'token' field".to_string(),
        })?
        .to_string();

    // Step 2: Fetch NVIDIA JWKS to get public key
    log::debug!(target: "attestation", "[attestation] fetching NRAS JWKS backend={}", backend_id);
    let jwks_response = client
        .get("https://nras.attestation.nvidia.com/.well-known/jwks.json")
        .send()
        .await
        .map_err(|e| AttestationError::NetworkError {
            reason: format!("JWKS fetch failed: {}", e),
        })?;

    let jwks: serde_json::Value = jwks_response.json().await.map_err(|e| {
        AttestationError::NetworkError {
            reason: format!("Failed to parse JWKS: {}", e),
        }
    })?;

    // Extract first key from JWKS and construct PEM for verification
    // For production, parse the JWK properly; here we use DecodingKey::from_jwk
    let key = jwks["keys"]
        .as_array()
        .and_then(|keys| keys.first())
        .ok_or_else(|| AttestationError::JwtVerification {
            reason: "JWKS has no keys".to_string(),
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

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[NRAS_ISSUER]);

    log::debug!(target: "attestation", "[attestation] decoding NRAS JWT backend={}", backend_id);
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

    // Validate overall result
    match claims.nvidia_overall_att_result.as_deref() {
        Some("true") => {}
        other => {
            log::warn!(target: "attestation", "[attestation] NVIDIA overall result not true backend={} result={:?}", backend_id, other);
            return Err(AttestationError::JwtVerification {
                reason: format!("NVIDIA overall attestation result is not true: {:?}", other),
            });
        }
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
    })
}
