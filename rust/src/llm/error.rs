/// Error taxonomy for LLM operations -- crosses UniFFI boundary as exception type.
/// Per D-10: 5 variants mapping HTTP/network conditions to human-readable messages.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum LlmError {
    #[error("Network error: {reason}")]
    NetworkError { reason: String },

    #[error("Authentication failed: {reason}")]
    AuthError { reason: String },

    #[error("Model not found: {model_id}")]
    ModelNotFound { model_id: String },

    #[error("Rate limited: {reason}")]
    RateLimited {
        reason: String,
        /// Seconds from Retry-After header, if the server provided one.
        retry_after_secs: Option<u64>,
    },

    #[error("API error ({status_code}): {reason}")]
    ApiError { status_code: u16, reason: String },
}

impl LlmError {
    /// Human-readable display string for AppState.last_error (per D-11).
    pub fn display_message(&self) -> String {
        match self {
            LlmError::NetworkError { reason } => {
                format!("Connection failed: {}", reason)
            }
            LlmError::AuthError { reason } => {
                format!("Authentication failed: {}", reason)
            }
            LlmError::ModelNotFound { model_id } => {
                format!("Model '{}' is not available on this backend", model_id)
            }
            LlmError::RateLimited {
                reason,
                retry_after_secs,
            } => {
                if let Some(secs) = retry_after_secs {
                    format!("Too many requests. Retry after {}s. {}", secs, reason)
                } else {
                    format!("Too many requests. {}", reason)
                }
            }
            LlmError::ApiError {
                status_code,
                reason,
            } => {
                format!("Server error ({}): {}", status_code, reason)
            }
        }
    }
}

/// Map async-openai OpenAIError to our LlmError taxonomy.
/// Handles both reqwest-level errors (network) and API-level errors (JSON body).
/// Per Pitfall 2: match both OpenAIError::Reqwest and OpenAIError::ApiError.
pub fn map_openai_error(e: async_openai::error::OpenAIError) -> LlmError {
    use async_openai::error::OpenAIError;
    match e {
        OpenAIError::Reqwest(re) => {
            if let Some(status) = re.status() {
                let code = status.as_u16();
                log::warn!(target: "llm_error", "[llm_error] reqwest error status={} detail={}", code, re);
                match code {
                    401 | 403 => LlmError::AuthError {
                        reason: "Invalid or missing API key".into(),
                    },
                    404 => LlmError::ModelNotFound {
                        model_id: "unknown".into(),
                    },
                    429 => LlmError::RateLimited {
                        reason: "Please try again later".into(),
                        retry_after_secs: None,
                    },
                    _ => LlmError::ApiError {
                        status_code: code,
                        reason: re.to_string(),
                    },
                }
            } else {
                log::warn!(target: "llm_error", "[llm_error] reqwest network error detail={}", re);
                LlmError::NetworkError {
                    reason: re.to_string(),
                }
            }
        }
        OpenAIError::ApiError(ae) => {
            // API-level error parsed from JSON response body
            let msg = ae.message.clone();
            let code_str = ae.code.as_deref().unwrap_or("");
            log::warn!(target: "llm_error", "[llm_error] api error code={} message={}", code_str, msg);
            if code_str.contains("invalid_api_key") || msg.contains("Incorrect API key") {
                LlmError::AuthError { reason: msg }
            } else if code_str.contains("model_not_found") || msg.contains("does not exist") {
                LlmError::ModelNotFound {
                    model_id: code_str.to_string(),
                }
            } else {
                LlmError::ApiError {
                    status_code: 0,
                    reason: msg,
                }
            }
        }
        other => {
            log::warn!(target: "llm_error", "[llm_error] other openai error detail={}", other);
            LlmError::NetworkError {
                reason: other.to_string(),
            }
        }
    }
}
