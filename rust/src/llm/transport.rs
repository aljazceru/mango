use std::time::Duration;

use async_openai::{config::OpenAIConfig, Client};

use super::{backend::BackendConfig, error::LlmError};
use crate::net::tls::pinned_reqwest_client;

/// Provider-specific transport selection for outbound inference traffic.
///
/// The core supports plain OpenAI-compatible HTTPS plus provider-owned secure
/// transports. Callers select by transport kind instead of hardcoding provider
/// behavior throughout the request path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderTransportKind {
    OpenAiCompatible,
    TinfoilSecure,
    PpqPrivateE2ee,
    VeniceE2ee,
    Redpill,
}

impl ProviderTransportKind {
    pub fn for_backend(backend: &BackendConfig) -> Self {
        if backend.provider_kind() == super::backend::ProviderKind::Tinfoil {
            return Self::TinfoilSecure;
        }

        if backend.provider_kind() == super::backend::ProviderKind::Venice {
            return Self::VeniceE2ee;
        }

        if backend.provider_kind() == super::backend::ProviderKind::Redpill {
            return Self::Redpill;
        }

        let base_url = backend.base_url.trim_end_matches('/');
        if backend.id == "ppq-ai"
            && (base_url.ends_with("/private") || base_url.contains("/private/"))
        {
            Self::PpqPrivateE2ee
        } else {
            Self::OpenAiCompatible
        }
    }

    pub fn openai_api_base(self, backend: &BackendConfig) -> Result<String, LlmError> {
        match self {
            Self::OpenAiCompatible => Ok(backend.base_url.trim_end_matches('/').to_string()),
            Self::TinfoilSecure => Err(tinfoil_secure_transport_error()),
            Self::PpqPrivateE2ee => Err(unsupported_private_transport_error()),
            Self::VeniceE2ee => Err(unsupported_venice_transport_error()),
            // Redpill IS OpenAI-compatible (no E2EE wrapper) — expose the canonical
            // /v1 api_base so the generic OpenAI client can be used. The transport
            // dispatcher in streaming.rs short-circuits Redpill BEFORE reaching the
            // generic OpenAI path, but if a future caller routes through it the api_base
            // is correct.
            Self::Redpill => {
                let trimmed = backend
                    .base_url
                    .trim_end_matches('/')
                    .trim_end_matches("/v1");
                Ok(format!("{}/v1", trimmed))
            }
        }
    }

    pub fn model_list_url(self, backend: &BackendConfig) -> Result<String, LlmError> {
        match self {
            Self::OpenAiCompatible => {
                Ok(format!("{}/models", backend.base_url.trim_end_matches('/')))
            }
            Self::TinfoilSecure => super::tinfoil_secure::model_list_url(backend),
            Self::PpqPrivateE2ee => super::ppq_private::model_list_url(backend),
            Self::VeniceE2ee => super::venice::model_list_url(backend),
            Self::Redpill => super::redpill::model_list_url(backend),
        }
    }

    pub fn build_reqwest_client(
        self,
        backend: &BackendConfig,
        pinned_tls_public_key_fp: Option<&str>,
        timeout: Duration,
    ) -> Result<(reqwest::Client, bool), LlmError> {
        match self {
            Self::OpenAiCompatible => {
                if let Some(fp) = pinned_tls_public_key_fp {
                    let client = pinned_reqwest_client(fp, timeout).map_err(|error| {
                        LlmError::NetworkError {
                            reason: format!(
                                "backend {} requires pinned TLS, but the pinned client could not be built: {}",
                                backend.id, error
                            ),
                        }
                    })?;
                    return Ok((client, true));
                }

                let client = reqwest::Client::builder()
                    .hickory_dns(false)
                    .timeout(timeout)
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());
                Ok((client, false))
            }
            Self::TinfoilSecure => Ok((super::tinfoil_secure::build_http_client(timeout)?, false)),
            Self::PpqPrivateE2ee => Ok((super::ppq_private::build_http_client(timeout)?, false)),
            Self::VeniceE2ee => Ok((super::venice::build_http_client(timeout)?, false)),
            Self::Redpill => Ok((super::redpill::build_http_client(timeout)?, false)),
        }
    }

    pub fn build_openai_client(
        self,
        backend: &BackendConfig,
        pinned_tls_public_key_fp: Option<&str>,
        timeout: Duration,
    ) -> Result<(Client<OpenAIConfig>, bool), LlmError> {
        let (http_client, used_pin) =
            self.build_reqwest_client(backend, pinned_tls_public_key_fp, timeout)?;
        let api_base = self.openai_api_base(backend)?;
        let config = OpenAIConfig::new()
            .with_api_key(&backend.api_key)
            .with_api_base(&api_base);
        Ok((
            Client::with_config(config).with_http_client(http_client),
            used_pin,
        ))
    }
}

fn tinfoil_secure_transport_error() -> LlmError {
    LlmError::NetworkError {
        reason: "Tinfoil secure transport does not use the generic OpenAI client path. Use the provider-specific Tinfoil secure transport implementation instead.".to_string(),
    }
}

fn unsupported_private_transport_error() -> LlmError {
    LlmError::NetworkError {
        reason: "PPQ private E2EE transport does not use the generic OpenAI client path. Use the provider-specific PPQ private transport implementation instead.".to_string(),
    }
}

fn unsupported_venice_transport_error() -> LlmError {
    LlmError::NetworkError {
        reason: "Venice E2EE transport does not use the generic OpenAI client path. Use the provider-specific Venice transport implementation instead.".to_string(),
    }
}
