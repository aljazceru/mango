//! Attestation task spawner.
//!
//! Runs the provider-selected attestation flow in the background and reports
//! the result back into the actor loop.

use super::{AttestationError, AttestationEvent};
use crate::llm::{BackendConfig, ProviderKind, TeeType};

pub type CertificateCache = super::endpoint::CertificateCache;

pub fn spawn_attestation_task(
    runtime: &tokio::runtime::Runtime,
    backend: &BackendConfig,
    core_tx: flume::Sender<crate::CoreMsg>,
    certificate_cache: CertificateCache,
    policy: crate::attestation::TeePolicy,
) {
    let backend = backend.clone();
    let backend_id = backend.id.clone();

    runtime.spawn(async move {
        let result = run_attestation(backend.clone(), certificate_cache, policy).await;

        let att_event = match result {
            Ok(event) => event,
            Err(e) => AttestationEvent::Failed {
                backend_id,
                is_transient: e.is_transient(),
                reason: e.to_string(),
            },
        };

        let internal = crate::llm::InternalEvent::AttestationResult(att_event);
        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(internal)));
    });
}

async fn run_attestation(
    backend: BackendConfig,
    certificate_cache: CertificateCache,
    policy: crate::attestation::TeePolicy,
) -> Result<AttestationEvent, AttestationError> {
    log::info!(
        target: "attestation",
        "[attestation] start backend={} tee_type={:?} provider={:?}",
        backend.id,
        backend.tee_type,
        backend.provider_kind()
    );

    match backend.tee_type {
        TeeType::IntelTdx | TeeType::NvidiaH100Cc | TeeType::AmdSevSnp => {}
        TeeType::Unknown => return Err(AttestationError::Unsupported),
    }

    let event = match backend.provider_kind() {
        ProviderKind::Tinfoil => {
            crate::llm::tinfoil_secure::verify_backend_attestation(&backend, &policy.snp).await?
        }
        ProviderKind::Custom => {
            super::endpoint::verify_attestation_endpoint(&backend, certificate_cache, &policy)
                .await?
        }
        ProviderKind::Ppq => {
            if backend.transport_kind() == crate::llm::ProviderTransportKind::PpqPrivateE2ee {
                crate::llm::ppq_private::verify_backend_attestation(&backend, &policy.snp).await?
            } else {
                super::endpoint::verify_attestation_endpoint(
                    &backend,
                    certificate_cache,
                    &policy,
                )
                .await?
            }
        }
    };

    log::info!(target: "attestation", "[attestation] success backend={}", backend.id);
    Ok(event)
}
