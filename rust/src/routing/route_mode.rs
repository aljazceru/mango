//! "Where inference runs" for new chats (settings status card).
//!
//! Everything here is a pure view over state the actor already owns: the
//! backend id that `default_backend_and_model` resolves for new chats. The
//! route itself is derived from that id and never persisted separately.

use super::{profile_id_from_backend_id, HybridProfile};
use crate::llm::local_models::{is_local_backend_id, local_backend_id, LocalModelSummary};
use crate::llm::{BackendSummary, HealthStatus, TeeType};

/// Keyless local-server preset. It is neither a confidential cloud route nor
/// an on-device model, so it is never auto-selected by a route switch.
const QVAC_LOCAL_ID: &str = "qvac-local";

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum InferenceRoute {
    Cloud,
    Hybrid,
    OnDevice,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct InferenceStatus {
    pub route: InferenceRoute,
    /// Backend id new chats use (`hybrid:<profile>` for hybrid).
    pub backend_id: String,
    /// Provider name, or the hybrid profile name. Empty when unresolved.
    pub backend_name: String,
    /// Model new chats use; the remote model for hybrid.
    pub model_id: String,
    /// TEE of the remote side. `None` on-device.
    pub tee_type: Option<TeeType>,
    /// Backend whose `attestation_statuses` entry the card shows.
    pub attested_backend_id: Option<String>,
    /// Display name of the local model (hybrid and on-device).
    pub local_model_name: Option<String>,
    pub hybrid_available: bool,
    pub on_device_available: bool,
}

impl Default for InferenceStatus {
    fn default() -> Self {
        Self {
            route: InferenceRoute::Cloud,
            backend_id: String::new(),
            backend_name: String::new(),
            model_id: String::new(),
            tee_type: None,
            attested_backend_id: None,
            local_model_name: None,
            hybrid_available: false,
            on_device_available: false,
        }
    }
}

pub fn route_for_backend_id(backend_id: &str) -> InferenceRoute {
    if profile_id_from_backend_id(backend_id).is_some() {
        InferenceRoute::Hybrid
    } else if is_local_backend_id(backend_id) {
        InferenceRoute::OnDevice
    } else {
        InferenceRoute::Cloud
    }
}

pub(crate) fn is_cloud_candidate(b: &BackendSummary) -> bool {
    route_for_backend_id(&b.id) == InferenceRoute::Cloud
        && b.id != QVAC_LOCAL_ID
        && b.has_api_key
        && !b.models.is_empty()
}

/// Verified on-device models as (backend id, summary), in catalog order.
pub(crate) fn installed_local(
    local_models: &[LocalModelSummary],
) -> impl Iterator<Item = (String, &LocalModelSummary)> {
    local_models.iter().filter(|m| m.verified).map(|m| {
        (
            m.backend_id.clone().unwrap_or_else(|| local_backend_id(&m.id)),
            m,
        )
    })
}

fn local_name(local_models: &[LocalModelSummary], backend_id: &str) -> Option<String> {
    installed_local(local_models)
        .find(|(id, _)| id == backend_id)
        .map(|(_, m)| m.name.clone())
}

pub fn inference_status(
    backends: &[BackendSummary],
    hybrid_profiles: &[HybridProfile],
    local_models: &[LocalModelSummary],
    local_supported: bool,
    new_chat_backend_id: &str,
    new_chat_model_id: &str,
) -> InferenceStatus {
    let find = |id: &str| backends.iter().find(|b| b.id == id);
    let base = InferenceStatus {
        route: route_for_backend_id(new_chat_backend_id),
        backend_id: new_chat_backend_id.to_string(),
        hybrid_available: !hybrid_profiles.is_empty(),
        on_device_available: local_supported && installed_local(local_models).next().is_some(),
        ..InferenceStatus::default()
    };
    match base.route {
        InferenceRoute::Hybrid => {
            let profile = profile_id_from_backend_id(new_chat_backend_id)
                .and_then(|pid| hybrid_profiles.iter().find(|p| p.id == pid));
            match profile {
                Some(p) => InferenceStatus {
                    backend_name: p.name.clone(),
                    model_id: p.remote_model_id.clone(),
                    tee_type: find(&p.remote_backend_id).map(|b| b.tee_type.clone()),
                    attested_backend_id: Some(p.remote_backend_id.clone()),
                    local_model_name: local_name(local_models, &p.local_backend_id)
                        .or_else(|| Some(p.local_model_id.clone())),
                    ..base
                },
                None => base,
            }
        }
        InferenceRoute::OnDevice => InferenceStatus {
            backend_name: "On this device".to_string(),
            model_id: new_chat_model_id.to_string(),
            local_model_name: local_name(local_models, new_chat_backend_id),
            ..base
        },
        InferenceRoute::Cloud => match find(new_chat_backend_id) {
            Some(b) => InferenceStatus {
                backend_name: b.name.clone(),
                model_id: new_chat_model_id.to_string(),
                tee_type: Some(b.tee_type.clone()),
                attested_backend_id: Some(b.id.clone()),
                ..base
            },
            None => InferenceStatus { model_id: new_chat_model_id.to_string(), ..base },
        },
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteTarget {
    Backend { backend_id: String, model_id: String },
    HybridProfile { profile_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteBlocker {
    NoCloudProvider,
    NoHybridProfile,
    LocalUnsupported,
    NoLocalModel,
}

pub struct RouteInputs<'a> {
    pub backends: &'a [BackendSummary],
    pub hybrid_profiles: &'a [HybridProfile],
    pub local_models: &'a [LocalModelSummary],
    pub local_supported: bool,
    pub current_backend_id: &'a str,
    pub current_model_id: &'a str,
    pub remembered_cloud_backend_id: Option<&'a str>,
    pub remembered_cloud_model_id: Option<&'a str>,
    pub remembered_local_backend_id: Option<&'a str>,
}

pub fn resolve_route_target(
    route: InferenceRoute,
    inp: &RouteInputs,
) -> Result<RouteTarget, RouteBlocker> {
    match route {
        InferenceRoute::Cloud => {
            let cands: Vec<&BackendSummary> =
                inp.backends.iter().filter(|b| is_cloud_candidate(b)).collect();
            let by_id = |id: &str| cands.iter().copied().find(|b| b.id == id);
            let pick = by_id(inp.current_backend_id)
                .or_else(|| inp.remembered_cloud_backend_id.and_then(by_id))
                .or_else(|| {
                    cands
                        .iter()
                        .copied()
                        .find(|b| !matches!(b.health_status, HealthStatus::Failed))
                })
                .or_else(|| cands.first().copied())
                .ok_or(RouteBlocker::NoCloudProvider)?;
            let model_id = [Some(inp.current_model_id), inp.remembered_cloud_model_id]
                .into_iter()
                .flatten()
                .find(|m| !m.is_empty() && pick.models.iter().any(|x| x == *m))
                .map(str::to_string)
                .unwrap_or_else(|| pick.models[0].clone());
            Ok(RouteTarget::Backend { backend_id: pick.id.clone(), model_id })
        }
        InferenceRoute::Hybrid => {
            let current = profile_id_from_backend_id(inp.current_backend_id)
                .filter(|pid| inp.hybrid_profiles.iter().any(|p| p.id == *pid));
            current
                .map(str::to_string)
                .or_else(|| inp.hybrid_profiles.first().map(|p| p.id.clone()))
                .map(|profile_id| RouteTarget::HybridProfile { profile_id })
                .ok_or(RouteBlocker::NoHybridProfile)
        }
        InferenceRoute::OnDevice => {
            if !inp.local_supported {
                return Err(RouteBlocker::LocalUnsupported);
            }
            let installed: Vec<(String, &LocalModelSummary)> =
                installed_local(inp.local_models).collect();
            let by_id = |id: &str| installed.iter().find(|(bid, _)| bid == id);
            let (backend_id, summary) = by_id(inp.current_backend_id)
                .or_else(|| inp.remembered_local_backend_id.and_then(by_id))
                .or_else(|| installed.first())
                .ok_or(RouteBlocker::NoLocalModel)?;
            let model_id = inp
                .backends
                .iter()
                .find(|b| &b.id == backend_id)
                .and_then(|b| b.models.first().cloned())
                .unwrap_or_else(|| summary.id.clone());
            Ok(RouteTarget::Backend { backend_id: backend_id.clone(), model_id })
        }
    }
}

pub fn blocker_message(b: &RouteBlocker) -> &'static str {
    match b {
        RouteBlocker::NoCloudProvider => "Add an API key for a confidential provider first.",
        RouteBlocker::NoHybridProfile => "Set up hybrid rules first.",
        RouteBlocker::LocalUnsupported => "This device can't run on-device models.",
        RouteBlocker::NoLocalModel => "Download an on-device model first.",
    }
}

pub fn blocker_setup_screen(b: &RouteBlocker) -> Option<crate::Screen> {
    match b {
        RouteBlocker::NoCloudProvider => Some(crate::Screen::SettingsProviders),
        RouteBlocker::NoHybridProfile => Some(crate::Screen::SettingsHybridRouting),
        RouteBlocker::NoLocalModel => Some(crate::Screen::SettingsLocalModels),
        RouteBlocker::LocalUnsupported => None,
    }
}
