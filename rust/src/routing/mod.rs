use thiserror::Error;

#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum BackendRole {
    Local,
    Remote,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct RoutingPolicy {
    pub escalate_if_attachment: bool,
    pub prefer_local_when_offline: bool,
    pub escalate_if_message_longer_than: Option<u64>,
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        Self {
            escalate_if_attachment: true,
            prefer_local_when_offline: true,
            escalate_if_message_longer_than: Some(4_000),
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Default)]
pub struct LocalPreprocessing {
    pub compress_history: bool,
    pub rewrite_rag_query: bool,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct HybridProfile {
    pub id: String,
    pub name: String,
    pub local_backend_id: String,
    pub local_model_id: String,
    pub remote_backend_id: String,
    pub remote_model_id: String,
    pub policy: RoutingPolicy,
    pub preprocessing: LocalPreprocessing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnRouting {
    pub backend_id: String,
    pub model_id: String,
    pub retrieval_query: Option<String>,
    pub decision: BackendRole,
    pub reason: String,
    pub profile_id: Option<String>,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct TurnRoutingSummary {
    pub conversation_id: Option<String>,
    pub profile_id: Option<String>,
    pub backend_id: String,
    pub model_id: String,
    pub decision: BackendRole,
    pub reason: String,
    pub provider_name: String,
    pub tee_label: String,
    pub tee_verified: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteError {
    #[error("hybrid profile not found: {profile_id}")]
    ProfileNotFound { profile_id: String },
    #[error("hybrid route backend is missing: {backend_id}")]
    BackendMissing { backend_id: String },
    #[error("hybrid route model is missing: {backend_id}/{model_id}")]
    ModelMissing {
        backend_id: String,
        model_id: String,
    },
    #[error("local inference is turned off")]
    LocalDisabled,
    #[error("remote attestation is unavailable: {reason}")]
    AttestationUnavailable { reason: String },
}

pub fn hybrid_backend_id(profile_id: &str) -> String {
    format!("hybrid:{profile_id}")
}

pub fn profile_id_from_backend_id(backend_id: &str) -> Option<&str> {
    backend_id.strip_prefix("hybrid:")
}

pub fn is_hybrid_backend_id(backend_id: &str) -> bool {
    profile_id_from_backend_id(backend_id).is_some()
}

pub fn resolve_turn_routing(
    profile: &HybridProfile,
    text: &str,
    has_attachment: bool,
    force_role: Option<BackendRole>,
    remote_reachable: bool,
) -> TurnRouting {
    let (decision, reason) = if has_attachment && profile.policy.escalate_if_attachment {
        (BackendRole::Remote, "attachment present".to_string())
    } else if let Some(role) = force_role {
        (role, "user override".to_string())
    } else if profile.policy.prefer_local_when_offline && !remote_reachable {
        (BackendRole::Local, "remote unavailable".to_string())
    } else if profile
        .policy
        .escalate_if_message_longer_than
        .is_some_and(|max| estimate_tokens(text) > max)
    {
        (
            BackendRole::Remote,
            "message too long for local".to_string(),
        )
    } else {
        (BackendRole::Local, "local default".to_string())
    };

    match decision {
        BackendRole::Local => TurnRouting {
            backend_id: profile.local_backend_id.clone(),
            model_id: profile.local_model_id.clone(),
            retrieval_query: None,
            decision,
            reason,
            profile_id: Some(profile.id.clone()),
        },
        BackendRole::Remote => TurnRouting {
            backend_id: profile.remote_backend_id.clone(),
            model_id: profile.remote_model_id.clone(),
            retrieval_query: None,
            decision,
            reason,
            profile_id: Some(profile.id.clone()),
        },
    }
}

pub fn single_backend_routing(backend_id: String, model_id: String) -> TurnRouting {
    TurnRouting {
        backend_id,
        model_id,
        retrieval_query: None,
        decision: BackendRole::Remote,
        reason: "selected backend".to_string(),
        profile_id: None,
    }
}

fn estimate_tokens(text: &str) -> u64 {
    let chars = text.chars().count() as u64;
    (chars / 4).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> HybridProfile {
        HybridProfile {
            id: "default".to_string(),
            name: "Local to Tinfoil".to_string(),
            local_backend_id: "local-qwen".to_string(),
            local_model_id: "qwen".to_string(),
            remote_backend_id: "tinfoil".to_string(),
            remote_model_id: "llama3".to_string(),
            policy: RoutingPolicy::default(),
            preprocessing: LocalPreprocessing::default(),
        }
    }

    #[test]
    fn deterministic_cascade_routes_attachment_remote() {
        let route = resolve_turn_routing(&profile(), "hello", true, None, true);
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.backend_id, "tinfoil");
        assert_eq!(route.reason, "attachment present");
    }

    #[test]
    fn deterministic_cascade_routes_offline_local() {
        let route = resolve_turn_routing(&profile(), "hello", false, None, false);
        assert_eq!(route.decision, BackendRole::Local);
        assert_eq!(route.backend_id, "local-qwen");
        assert_eq!(route.reason, "remote unavailable");
    }

    #[test]
    fn attachment_routes_remote_even_with_local_override() {
        let route = resolve_turn_routing(&profile(), "hello", true, Some(BackendRole::Local), true);
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.backend_id, "tinfoil");
        assert_eq!(route.reason, "attachment present");
    }

    #[test]
    fn remote_override_wins_for_text_turn() {
        let route =
            resolve_turn_routing(&profile(), "hello", false, Some(BackendRole::Remote), true);
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.backend_id, "tinfoil");
        assert_eq!(route.reason, "user override");
    }
}
