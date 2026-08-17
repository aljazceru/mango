//! Explicit inference-routing model (INFERENCE_ROUTING_PLAN.md).
//!
//! This module is the canonical route planner for the four user-facing
//! inference modes:
//!
//! - `LocalOnly`   — every turn runs on the selected local target.
//! - `RemoteOnly`  — every turn runs on a selected remote target.
//! - `RulesHybrid` — deterministic policy cascade (supersedes `HybridProfile`).
//! - `SmartRouting`— a local router model picks local vs. an allowed remote.
//!
//! It is deliberately self-contained: it depends only on [`super::RoutingPolicy`]
//! and [`super::BackendRole`] so it can be unit-tested end to end without an
//! actor, a database, or a live model. The actor loop adopts it via
//! [`resolve_turn_plan`]; existing `HybridProfile` rows migrate in via
//! [`HybridProfile::to_inference_profile`].
//!
//! Privacy invariants the planner enforces, regardless of caller:
//!   - `LocalOnly` never returns a remote target and never marks remote
//!     fallback allowed.
//!   - `RemoteOnly` returns remote targets by default; local override/fallback is
//!     only allowed when the caller explicitly opts in via the profile's
//!     [`FallbackPolicy`].
//!   - Smart routing only selects from targets that exist on the active
//!     profile and that satisfy the turn's capability requirements.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{hybrid_estimate_tokens, BackendRole, HybridProfile, RoutingPolicy};

// ── Mode + target shape ──────────────────────────────────────────────────────

/// User-facing inference mode. Selectable per-conversation and as a default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InferenceMode {
    LocalOnly,
    RemoteOnly,
    RulesHybrid,
    SmartRouting,
}

/// Role a [`RouteTarget`] plays in routing. Mirrors [`BackendRole`] but lives
/// on the target record so a single profile can carry several remotes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteTargetRole {
    Local,
    Remote,
}

/// Capability flags used to reject turns a target cannot serve (e.g. an image
/// turn routed to a text-only local model in `LocalOnly` mode).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RouteCapabilities {
    pub text: bool,
    pub vision: bool,
    pub tools: bool,
}

impl RouteCapabilities {
    fn satisfies(&self, needs_vision: bool, needs_tools: bool) -> bool {
        if needs_vision && !self.vision {
            return false;
        }
        if needs_tools && !self.tools {
            return false;
        }
        // text capability is implied; a target configured without text is not
        // useful and we don't model a turn that needs no text.
        true
    }
}

/// One selectable backend+model inside a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteTarget {
    pub backend_id: String,
    pub model_id: String,
    pub role: RouteTargetRole,
    pub capabilities: RouteCapabilities,
    /// True when the remote leg must pass attestation preflight before send.
    pub require_attestation: bool,
}

impl RouteTarget {
    fn decision_role(&self) -> BackendRole {
        match self.role {
            RouteTargetRole::Local => BackendRole::Local,
            RouteTargetRole::Remote => BackendRole::Remote,
        }
    }
}

/// Mode-aware fallback boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FallbackPolicy {
    /// Never fall back. Used by `LocalOnly` and the default for `RemoteOnly`.
    #[default]
    Never,
    /// Fall back only to other targets of the same role that are already on
    /// the profile. Used by `RulesHybrid` and `SmartRouting` first retry.
    SameRoleOnly,
    /// Fall back across roles (e.g. remote-only with an explicit local
    /// emergency fallback). Opt-in only.
    AllowCrossRole,
}

/// The new profile shape: routing policy separated from backend records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InferenceProfile {
    pub id: String,
    pub name: String,
    pub mode: InferenceMode,
    pub local_target: Option<RouteTarget>,
    /// Ordered remote candidates. Index 0 is the default remote for
    /// `RemoteOnly` / `RulesHybrid`.
    pub remote_targets: Vec<RouteTarget>,
    /// Local model used to make the smart-routing decision. Required for
    /// `SmartRouting`; ignored otherwise.
    pub router_target: Option<RouteTarget>,
    /// Deterministic rules for `RulesHybrid`. Reused from the legacy cascade.
    pub rules_policy: RoutingPolicy,
    pub fallback_policy: FallbackPolicy,
}

// ── Reason codes ─────────────────────────────────────────────────────────────

/// Stable, structured reason for a route decision. UI maps these to copy so
/// wording does not drift across platforms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReasonCode {
    LocalDefault,
    RemoteDefault,
    UserOverride,
    AttachmentPresent,
    RemoteUnavailable,
    MessageTooLong,
    SmartRouterLocal,
    SmartRouterRemote,
    SmartRouterFallback,
    Disabled,
    NoTarget,
    CapabilityMismatch,
}

impl ReasonCode {
    fn human(&self) -> &'static str {
        match self {
            ReasonCode::LocalDefault => "answered locally",
            ReasonCode::RemoteDefault => "routed to remote",
            ReasonCode::UserOverride => "user override",
            ReasonCode::AttachmentPresent => "attachment present",
            ReasonCode::RemoteUnavailable => "remote unavailable",
            ReasonCode::MessageTooLong => "message too long for local",
            ReasonCode::SmartRouterLocal => "local router chose local",
            ReasonCode::SmartRouterRemote => "local router chose remote",
            ReasonCode::SmartRouterFallback => "local router failed; fallback applied",
            ReasonCode::Disabled => "local inference disabled",
            ReasonCode::NoTarget => "no routing target available",
            ReasonCode::CapabilityMismatch => "target cannot serve this turn",
        }
    }
}

// ── Resolved route ───────────────────────────────────────────────────────────

/// Result of route planning for a single turn. Built before the user message
/// is persisted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedRoute {
    pub backend_id: String,
    pub model_id: String,
    pub decision: BackendRole,
    pub mode: InferenceMode,
    pub reason_code: ReasonCode,
    pub reason: String,
    pub attestation_required: bool,
    pub router_invoked: bool,
    pub fallback_allowed: bool,
    pub fallback_targets: Vec<RouteTarget>,
}

impl ResolvedRoute {
    /// Bridge to the legacy [`super::TurnRouting`] shape so the actor can adopt
    /// the new planner incrementally without rewriting its dispatch path.
    pub fn to_turn_routing(&self, profile_id: Option<String>) -> super::TurnRouting {
        super::TurnRouting {
            backend_id: self.backend_id.clone(),
            model_id: self.model_id.clone(),
            retrieval_query: None,
            decision: self.decision.clone(),
            reason: self.reason.clone(),
            profile_id,
        }
    }
}

// ── Errors ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlanError {
    #[error("no local target configured for this profile")]
    NoLocalTarget,
    #[error("no remote target configured for this profile")]
    NoRemoteTarget,
    #[error("no router target configured for smart routing")]
    NoRouterTarget,
    #[error("local inference is turned off")]
    LocalDisabled,
    #[error("remote target is currently unavailable: {backend_id}")]
    RemoteUnavailable { backend_id: String },
    #[error("target cannot serve this turn (capability mismatch): {backend_id}")]
    CapabilityMismatch { backend_id: String },
    #[error("remote attestation is required but unavailable for: {backend_id}")]
    AttestationUnavailable { backend_id: String },
    #[error("override target is not on the active profile: {backend_id}/{model_id}")]
    OverrideTargetNotFound {
        backend_id: String,
        model_id: String,
    },
    #[error("local router produced invalid output: {reason}")]
    RouterInvalid { reason: String },
    #[error("local_target must have the Local role: {backend_id}")]
    LocalTargetNotLocal { backend_id: String },
    #[error("remote target must have the Remote role: {backend_id}")]
    RemoteTargetNotRemote { backend_id: String },
    #[error("router_target must have the Local role for SmartRouting profiles: {backend_id}")]
    RouterTargetNotLocal { backend_id: String },
    #[error("local router chose a target not allowed by the profile: {backend_id}")]
    RouterTargetDisallowed { backend_id: String },
}

// ── Smart-router contract ────────────────────────────────────────────────────

/// Strict JSON contract the local router model must return. Unknown fields are
/// ignored; missing/typed-wrong fields fail parsing and trigger fallback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartRouterDecision {
    pub decision: String,
    pub backend_id: String,
    pub model_id: String,
    pub reason_code: String,
    pub confidence: f64,
}

impl SmartRouterDecision {
    /// Parse + structurally validate the router's raw JSON output.
    ///
    /// Returns the parsed decision plus its resolved role. Anything malformed
    /// becomes [`PlanError::RouterInvalid`] so the caller can apply fallback.
    pub fn parse(raw: &str) -> Result<(Self, BackendRole), PlanError> {
        let parsed: SmartRouterDecision =
            serde_json::from_str(raw).map_err(|e| PlanError::RouterInvalid {
                reason: format!("invalid json: {e}"),
            })?;
        let role = match parsed.decision.as_str() {
            "local" => BackendRole::Local,
            "remote" => BackendRole::Remote,
            other => {
                return Err(PlanError::RouterInvalid {
                    reason: format!("unknown decision `{other}`"),
                });
            }
        };
        Ok((parsed, role))
    }
}

/// Local, non-streaming router call. The actor supplies a real implementation
/// (local model inference); tests supply a canned one.
pub trait LocalRouterModel: Send + Sync {
    /// Run the router and return its raw JSON output (see [`SmartRouterDecision`]).
    fn decide(&self, prompt: &str) -> Result<String, String>;
}

/// Build the prompt handed to the local router. Exposed so it can be tested
/// without a model and so the actor and tests agree on the contract.
pub fn build_router_prompt(profile: &InferenceProfile, text: &str, has_image: bool) -> String {
    let local = profile
        .local_target
        .as_ref()
        .map(|t| format!("{} (model {})", t.backend_id, t.model_id))
        .unwrap_or_else(|| "none".to_string());
    let remotes: Vec<String> = profile
        .remote_targets
        .iter()
        .map(|t| format!("{} (model {})", t.backend_id, t.model_id))
        .collect();
    format!(
        "Pick exactly one route for this turn. Reply ONLY with the JSON object, no prose.\n\
         Local candidate: {local}\n\
         Remote candidates: {remotes:?}\n\
         Has image attachment: {has_image}\n\
         User message: {text:?}\n\
         JSON shape: {{\"decision\":\"local|remote\",\"backend_id\":\"...\",\"model_id\":\"...\",\"reason_code\":\"...\",\"confidence\":0.0}}"
    )
}

// ── Input + planner ──────────────────────────────────────────────────────────

/// Everything the planner needs. Health/attestation flags are precomputed by
/// the actor so this module stays free of I/O.
pub struct RouteInput<'a> {
    pub profile: &'a InferenceProfile,
    pub text: &'a str,
    pub has_image_attachment: bool,
    pub requires_tools: bool,
    pub local_inference_enabled: bool,
    /// Per-remote reachability predicate, keyed by backend id. Replaces a single
    /// boolean so multi-remote profiles (notably `SmartRouting`) get per-target
    /// health instead of treating all remotes as up or down together.
    pub is_remote_reachable: &'a dyn Fn(&str) -> bool,
    /// Attestation is currently available for remote targets that require it.
    pub attestation_available: bool,
    /// Per-turn override. `Local`/`Remote` forces a role; `None` is "auto".
    pub force_role: Option<BackendRole>,
    /// Local router for `SmartRouting`. Required when mode is `SmartRouting`.
    pub router: Option<&'a dyn LocalRouterModel>,
}

impl ResolvedRoute {
    fn from_target(
        target: &RouteTarget,
        mode: InferenceMode,
        reason_code: ReasonCode,
        router_invoked: bool,
        fallback_targets: Vec<RouteTarget>,
    ) -> Self {
        Self {
            backend_id: target.backend_id.clone(),
            model_id: target.model_id.clone(),
            decision: target.decision_role(),
            mode,
            reason_code,
            reason: reason_code.human().to_string(),
            attestation_required: target.role == RouteTargetRole::Remote
                && target.require_attestation,
            router_invoked,
            fallback_allowed: !fallback_targets.is_empty(),
            fallback_targets,
        }
    }
}

/// The core route planner. Replaces ad hoc route resolution.
///
/// Planning is pure: it performs no I/O and never sends anything. On error,
/// the caller must NOT persist the user message (per the plan's actor flow).
pub fn resolve_turn_plan(input: &RouteInput) -> Result<ResolvedRoute, PlanError> {
    let profile = input.profile;
    match profile.mode {
        InferenceMode::LocalOnly => plan_local_only(input),
        InferenceMode::RemoteOnly => plan_remote_only(input),
        InferenceMode::RulesHybrid => plan_rules_hybrid(input),
        InferenceMode::SmartRouting => plan_smart(input),
    }
}

fn local_enabled_check(input: &RouteInput, target: &RouteTarget) -> Result<(), PlanError> {
    if target.role == RouteTargetRole::Local && !input.local_inference_enabled {
        return Err(PlanError::LocalDisabled);
    }
    Ok(())
}

fn capability_check(input: &RouteInput, target: &RouteTarget) -> Result<(), PlanError> {
    if !target
        .capabilities
        .satisfies(input.has_image_attachment, input.requires_tools)
    {
        return Err(PlanError::CapabilityMismatch {
            backend_id: target.backend_id.clone(),
        });
    }
    Ok(())
}

fn attestation_check(input: &RouteInput, target: &RouteTarget) -> Result<(), PlanError> {
    if target.role == RouteTargetRole::Remote
        && target.require_attestation
        && !input.attestation_available
    {
        return Err(PlanError::AttestationUnavailable {
            backend_id: target.backend_id.clone(),
        });
    }
    Ok(())
}

/// Reject a remote target whose backend is currently unreachable. Local
/// targets always pass (reachability is a remote-only concern).
fn remote_reachability_check(input: &RouteInput, target: &RouteTarget) -> Result<(), PlanError> {
    if target.role == RouteTargetRole::Remote && !(input.is_remote_reachable)(&target.backend_id) {
        return Err(PlanError::RemoteUnavailable {
            backend_id: target.backend_id.clone(),
        });
    }
    Ok(())
}

fn profile_local_target(profile: &InferenceProfile) -> Result<&RouteTarget, PlanError> {
    let target = profile
        .local_target
        .as_ref()
        .ok_or(PlanError::NoLocalTarget)?;
    if target.role != RouteTargetRole::Local {
        return Err(PlanError::LocalTargetNotLocal {
            backend_id: target.backend_id.clone(),
        });
    }
    Ok(target)
}

fn ensure_remote_target(target: &RouteTarget) -> Result<(), PlanError> {
    if target.role != RouteTargetRole::Remote {
        return Err(PlanError::RemoteTargetNotRemote {
            backend_id: target.backend_id.clone(),
        });
    }
    Ok(())
}

fn profile_router_target(profile: &InferenceProfile) -> Result<&RouteTarget, PlanError> {
    let target = profile
        .router_target
        .as_ref()
        .ok_or(PlanError::NoRouterTarget)?;
    if target.role != RouteTargetRole::Local {
        return Err(PlanError::RouterTargetNotLocal {
            backend_id: target.backend_id.clone(),
        });
    }
    Ok(target)
}

fn same_route_target(a: &RouteTarget, b: &RouteTarget) -> bool {
    a.backend_id == b.backend_id && a.model_id == b.model_id
}

/// Run every gate for this turn against `target`. Used for primary-route
/// validation, smart-router target validation, and fallback filtering — so a
/// target the planner would reject as primary is never advertised as fallback.
fn validate_turn_target(input: &RouteInput, target: &RouteTarget) -> Result<(), PlanError> {
    local_enabled_check(input, target)?;
    capability_check(input, target)?;
    attestation_check(input, target)?;
    remote_reachability_check(input, target)
}

/// Targets the planner may fall back to, per the profile's fallback policy and
/// the chosen role. Excludes the already-chosen target.
fn fallback_targets_for(profile: &InferenceProfile, chosen: &RouteTarget) -> Vec<RouteTarget> {
    let same_role: Vec<RouteTarget> = all_targets(profile)
        .into_iter()
        .filter(|t| t.role == chosen.role && !same_route_target(t, chosen))
        .collect();

    match profile.fallback_policy {
        FallbackPolicy::Never => Vec::new(),
        FallbackPolicy::SameRoleOnly => same_role,
        FallbackPolicy::AllowCrossRole => all_targets(profile)
            .into_iter()
            .filter(|t| !same_route_target(t, chosen))
            .collect(),
    }
}

fn all_targets(profile: &InferenceProfile) -> Vec<RouteTarget> {
    let mut out = Vec::new();
    if let Some(local) = &profile.local_target {
        out.push(local.clone());
    }
    out.extend(profile.remote_targets.iter().cloned());
    out
}

/// True when `target` passes every gate for this turn (local-enabled,
/// capability, attestation, reachability). Used to validate the advertised
/// fallback list and to pick a servable fallback after a smart-router failure.
fn passes_turn_gates(input: &RouteInput, target: &RouteTarget) -> bool {
    validate_turn_target(input, target).is_ok()
}

/// Targets the planner may fall back to, per the profile's fallback policy and
/// the chosen role, **filtered through this turn's validation gates** so the
/// advertised fallback list only contains targets the planner itself would
/// accept as the primary route.
fn validated_fallback_targets(
    profile: &InferenceProfile,
    input: &RouteInput,
    chosen: &RouteTarget,
) -> Vec<RouteTarget> {
    fallback_targets_for(profile, chosen)
        .into_iter()
        .filter(|t| passes_turn_gates(input, t))
        .collect()
}

/// Candidate set after a smart-router failure, given the conservative default
/// role. `SameRoleOnly` keeps only local targets (privacy-safe default);
/// `AllowCrossRole` admits remotes too; `Never` yields none.
fn policy_candidates(
    profile: &InferenceProfile,
    primary_role: RouteTargetRole,
) -> Vec<RouteTarget> {
    match profile.fallback_policy {
        FallbackPolicy::Never => Vec::new(),
        FallbackPolicy::SameRoleOnly => all_targets(profile)
            .into_iter()
            .filter(|t| t.role == primary_role)
            .collect(),
        FallbackPolicy::AllowCrossRole => all_targets(profile),
    }
}

/// Resolve the servable primary and its advertised fallback list.
///
/// - If `chosen` passes every gate, it is the primary.
/// - Otherwise (e.g. the chosen remote is unreachable) try to promote the
///   first same-role alternative that passes every gate, so the planner never
///   returns a known-unservable primary. Cross-role promotion is never done
///   here — that is a privacy boundary reserved for explicit fallback policy.
/// - Under `FallbackPolicy::Never`, or when no same-role alternative is
///   servable, the original validation error is returned.
///
/// The advertised fallback list is always the validated set for the resolved
/// primary.
fn finalize_primary(
    input: &RouteInput,
    chosen: RouteTarget,
    mode: InferenceMode,
    reason_code: ReasonCode,
    router_invoked: bool,
) -> Result<ResolvedRoute, PlanError> {
    let profile = input.profile;
    let primary = match validate_turn_target(input, &chosen) {
        Ok(()) => chosen,
        Err(err) => {
            if profile.fallback_policy == FallbackPolicy::Never {
                return Err(err);
            }
            let promoted = all_targets(profile)
                .into_iter()
                .filter(|t| t.role == chosen.role && !same_route_target(t, &chosen))
                .find(|t| validate_turn_target(input, t).is_ok());
            match promoted {
                Some(t) => t,
                None => return Err(err),
            }
        }
    };
    let fallback = validated_fallback_targets(profile, input, &primary);
    Ok(ResolvedRoute::from_target(
        &primary,
        mode,
        reason_code,
        router_invoked,
        fallback,
    ))
}

// ── LocalOnly ────────────────────────────────────────────────────────────────

fn plan_local_only(input: &RouteInput) -> Result<ResolvedRoute, PlanError> {
    let profile = input.profile;
    let target = profile_local_target(profile)?;

    validate_turn_target(input, target)?;

    // LocalOnly never allows remote fallback, regardless of policy.
    Ok(ResolvedRoute::from_target(
        target,
        InferenceMode::LocalOnly,
        ReasonCode::LocalDefault,
        false,
        Vec::new(),
    ))
}

// ── RemoteOnly ───────────────────────────────────────────────────────────────

fn plan_remote_only(input: &RouteInput) -> Result<ResolvedRoute, PlanError> {
    let profile = input.profile;
    let chosen = pick_remote(profile, input.force_role.clone())?;
    // `pick_remote` can return the local target under AllowCrossRole + Local
    // override; `finalize_primary` runs the full gate set (incl. local-enabled)
    // so a disabled local backend is never dispatched, and promotes a servable
    // same-role alternative when the chosen remote is unreachable.
    finalize_primary(
        input,
        chosen,
        InferenceMode::RemoteOnly,
        ReasonCode::RemoteDefault,
        false,
    )
}

fn pick_remote(
    profile: &InferenceProfile,
    force_role: Option<BackendRole>,
) -> Result<RouteTarget, PlanError> {
    // A Local override in RemoteOnly mode is only honored when the profile
    // explicitly allows cross-role fallback; otherwise it is rejected as a
    // privacy-boundary violation.
    if matches!(force_role, Some(BackendRole::Local)) {
        if profile.fallback_policy == FallbackPolicy::AllowCrossRole {
            return Ok(profile_local_target(profile)?.clone());
        }
        // Fall through to remote; the override is incompatible with the mode.
    }
    let target = profile
        .remote_targets
        .first()
        .cloned()
        .ok_or(PlanError::NoRemoteTarget)?;
    ensure_remote_target(&target)?;
    Ok(target)
}

// ── RulesHybrid ──────────────────────────────────────────────────────────────

fn plan_rules_hybrid(input: &RouteInput) -> Result<ResolvedRoute, PlanError> {
    let profile = input.profile;
    let local = profile_local_target(profile)?;
    let remote = profile
        .remote_targets
        .first()
        .ok_or(PlanError::NoRemoteTarget)?;
    ensure_remote_target(remote)?;

    // Exact priority order inherited from the legacy cascade
    // (resolve_turn_routing). Do not reorder without updating both.
    let (chosen, reason_code): (RouteTarget, ReasonCode) =
        if input.has_image_attachment && profile.rules_policy.escalate_if_attachment {
            (remote.clone(), ReasonCode::AttachmentPresent)
        } else if let Some(role) = input.force_role.clone() {
            match role {
                BackendRole::Remote => (remote.clone(), ReasonCode::UserOverride),
                BackendRole::Local => (local.clone(), ReasonCode::UserOverride),
            }
        } else if profile.rules_policy.prefer_local_when_offline
            && !(input.is_remote_reachable)(&remote.backend_id)
        {
            (local.clone(), ReasonCode::RemoteUnavailable)
        } else if profile
            .rules_policy
            .escalate_if_message_longer_than
            .is_some_and(|max| hybrid_estimate_tokens(input.text) > max)
        {
            (remote.clone(), ReasonCode::MessageTooLong)
        } else {
            (local.clone(), ReasonCode::LocalDefault)
        };

    // `finalize_primary` runs the full gate set (incl. reachability) on the
    // chosen target and promotes a servable same-role alternative if the
    // chosen remote is unreachable, so the cascade never returns a
    // known-unservable route.
    finalize_primary(
        input,
        chosen,
        InferenceMode::RulesHybrid,
        reason_code,
        false,
    )
}

// ── SmartRouting ─────────────────────────────────────────────────────────────

fn plan_smart(input: &RouteInput) -> Result<ResolvedRoute, PlanError> {
    let profile = input.profile;

    // Explicit per-turn override wins: honor it without invoking the router.
    // `Some(Local)` picks/validates the local target; `Some(Remote)` picks the
    // first servable remote. This restores parity with the hybrid per-turn
    // override model so a user's "Local/Remote this turn" choice is respected.
    // Handled before the router requirement so an override does not need a
    // router handle or router_target to be configured.
    if let Some(role) = input.force_role.clone() {
        return plan_smart_override(input, role);
    }

    let router = input.router.ok_or(PlanError::NoRouterTarget)?;
    let router_target = profile_router_target(profile)?;

    // No override: the router itself runs locally, so local inference is required.
    if !input.local_inference_enabled {
        return Err(PlanError::LocalDisabled);
    }
    if !router_target.capabilities.text {
        return Err(PlanError::CapabilityMismatch {
            backend_id: router_target.backend_id.clone(),
        });
    }

    // Any failure to produce a usable router decision — model error, invalid
    // JSON, a disallowed/unusable/unreachable target — triggers the profile
    // fallback path rather than rejecting the turn outright. `Never` re-surfaces.
    match run_smart_decision(profile, router, input) {
        Ok(chosen) => {
            let reason_code = if chosen.role == RouteTargetRole::Local {
                ReasonCode::SmartRouterLocal
            } else {
                ReasonCode::SmartRouterRemote
            };
            let fallback = validated_fallback_targets(profile, input, &chosen);
            Ok(ResolvedRoute::from_target(
                &chosen,
                InferenceMode::SmartRouting,
                reason_code,
                true,
                fallback,
            ))
        }
        Err(err) => apply_smart_fallback(input, err),
    }
}

/// Resolve an explicit per-turn override under SmartRouting. The router is
/// bypassed; the chosen role is resolved directly and validated against every
/// gate (no promotion — the user explicitly chose, so failures surface).
fn plan_smart_override(input: &RouteInput, role: BackendRole) -> Result<ResolvedRoute, PlanError> {
    let profile = input.profile;
    let chosen: RouteTarget = match role {
        BackendRole::Local => profile_local_target(profile)?.clone(),
        BackendRole::Remote => pick_servable_remote(profile, input)?,
    };
    validate_turn_target(input, &chosen)?;
    let reason_code = if chosen.role == RouteTargetRole::Local {
        ReasonCode::SmartRouterLocal
    } else {
        ReasonCode::SmartRouterRemote
    };
    let fallback = validated_fallback_targets(profile, input, &chosen);
    Ok(ResolvedRoute::from_target(
        &chosen,
        InferenceMode::SmartRouting,
        reason_code,
        false,
        fallback,
    ))
}

/// First remote target on the profile that passes every gate for this turn,
/// or the first remote's validation error if none is servable.
fn pick_servable_remote(
    profile: &InferenceProfile,
    input: &RouteInput,
) -> Result<RouteTarget, PlanError> {
    for target in &profile.remote_targets {
        ensure_remote_target(target)?;
        if validate_turn_target(input, target).is_ok() {
            return Ok(target.clone());
        }
    }
    profile
        .remote_targets
        .first()
        .map(|t| {
            ensure_remote_target(t)?;
            validate_turn_target(input, t)?;
            Ok(t.clone())
        })
        .unwrap_or(Err(PlanError::NoRemoteTarget))
}

/// Run the local router and validate its chosen target against this turn's
/// gates. Errors here are consumed by [`apply_smart_fallback`].
fn run_smart_decision(
    profile: &InferenceProfile,
    router: &dyn LocalRouterModel,
    input: &RouteInput,
) -> Result<RouteTarget, PlanError> {
    let prompt = build_router_prompt(profile, input.text, input.has_image_attachment);
    let raw = router
        .decide(&prompt)
        .map_err(|reason| PlanError::RouterInvalid { reason })?;
    let (decision, _role) = SmartRouterDecision::parse(&raw)?;
    validate_smart_target(profile, &decision, input)
}

/// Apply the profile fallback policy when the local router cannot produce a
/// usable decision. The conservative default role after a router failure is
/// `Local`; remote targets are only considered when the profile explicitly
/// opts into cross-role fallback. `Never` re-surfaces the original error.
fn apply_smart_fallback(
    input: &RouteInput,
    original_err: PlanError,
) -> Result<ResolvedRoute, PlanError> {
    let profile = input.profile;
    if profile.fallback_policy == FallbackPolicy::Never {
        return Err(original_err);
    }
    for target in policy_candidates(profile, RouteTargetRole::Local) {
        if passes_turn_gates(input, &target) {
            let fallback_adv = validated_fallback_targets(profile, input, &target);
            return Ok(ResolvedRoute::from_target(
                &target,
                InferenceMode::SmartRouting,
                ReasonCode::SmartRouterFallback,
                true,
                fallback_adv,
            ));
        }
    }
    Err(original_err)
}

/// Resolve the router's chosen target and run all validation gates.
fn validate_smart_target(
    profile: &InferenceProfile,
    decision: &SmartRouterDecision,
    input: &RouteInput,
) -> Result<RouteTarget, PlanError> {
    let candidate = if decision.decision == "local" {
        profile_local_target(profile)?
    } else {
        // remote: must be one of the profile's remote targets
        let target = profile
            .remote_targets
            .iter()
            .find(|t| t.backend_id == decision.backend_id && t.model_id == decision.model_id)
            .ok_or_else(|| PlanError::RouterTargetDisallowed {
                backend_id: decision.backend_id.clone(),
            })?;
        ensure_remote_target(target)?;
        target
    };

    // Reject a router that lies about backend/model for the local target.
    if candidate.backend_id != decision.backend_id || candidate.model_id != decision.model_id {
        return Err(PlanError::RouterTargetDisallowed {
            backend_id: decision.backend_id.clone(),
        });
    }

    // Full gate set (incl. reachability) so a router that picks an unreachable
    // remote is treated as a router failure and routed through fallback.
    validate_turn_target(input, candidate)?;
    Ok(candidate.clone())
}

// ── Migration bridge: HybridProfile -> InferenceProfile ──────────────────────

impl HybridProfile {
    /// Convert a legacy hybrid profile into a `RulesHybrid` [`InferenceProfile`].
    ///
    /// Capabilities default to text-only on both legs; the actor should enrich
    /// these from `is_vision_model` / `supports_tool_use` when it adopts the
    /// new planner. Existing hybrid behavior is preserved bit-for-bit because
    /// [`plan_rules_hybrid`] reproduces the legacy cascade order.
    pub fn to_inference_profile(&self) -> InferenceProfile {
        let local = RouteTarget {
            backend_id: self.local_backend_id.clone(),
            model_id: self.local_model_id.clone(),
            role: RouteTargetRole::Local,
            capabilities: RouteCapabilities {
                text: true,
                vision: false,
                tools: false,
            },
            require_attestation: false,
        };
        let remote = RouteTarget {
            backend_id: self.remote_backend_id.clone(),
            model_id: self.remote_model_id.clone(),
            role: RouteTargetRole::Remote,
            capabilities: RouteCapabilities {
                text: true,
                // Remote hybrid leg historically handled attachments (vision).
                vision: true,
                tools: false,
            },
            require_attestation: true,
        };
        InferenceProfile {
            id: self.id.clone(),
            name: self.name.clone(),
            mode: InferenceMode::RulesHybrid,
            local_target: Some(local),
            remote_targets: vec![remote],
            router_target: None,
            rules_policy: self.policy.clone(),
            fallback_policy: FallbackPolicy::SameRoleOnly,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ── fixtures ─────────────────────────────────────────────────────────────

    fn text_local() -> RouteTarget {
        RouteTarget {
            backend_id: "local-qwen".into(),
            model_id: "qwen2.5".into(),
            role: RouteTargetRole::Local,
            capabilities: RouteCapabilities {
                text: true,
                vision: false,
                tools: false,
            },
            require_attestation: false,
        }
    }

    fn vision_local() -> RouteTarget {
        RouteTarget {
            backend_id: "local-qwen-vl".into(),
            model_id: "qwen2-vl".into(),
            role: RouteTargetRole::Local,
            capabilities: RouteCapabilities {
                text: true,
                vision: true,
                tools: false,
            },
            require_attestation: false,
        }
    }

    fn tinfoil_remote(vision: bool) -> RouteTarget {
        RouteTarget {
            backend_id: "tinfoil".into(),
            model_id: "kimi-k3".into(),
            role: RouteTargetRole::Remote,
            capabilities: RouteCapabilities {
                text: true,
                vision,
                tools: false,
            },
            require_attestation: true,
        }
    }

    fn ppq_remote() -> RouteTarget {
        RouteTarget {
            backend_id: "ppq".into(),
            model_id: "private/llama3-3-70b".into(),
            role: RouteTargetRole::Remote,
            capabilities: RouteCapabilities {
                text: true,
                vision: false,
                tools: false,
            },
            require_attestation: true,
        }
    }

    fn router_target() -> RouteTarget {
        RouteTarget {
            backend_id: "local-router".into(),
            model_id: "router-tiny".into(),
            role: RouteTargetRole::Local,
            capabilities: RouteCapabilities {
                text: true,
                vision: false,
                tools: false,
            },
            require_attestation: false,
        }
    }

    fn local_only_profile() -> InferenceProfile {
        InferenceProfile {
            id: "lo".into(),
            name: "Local only".into(),
            mode: InferenceMode::LocalOnly,
            local_target: Some(text_local()),
            remote_targets: vec![],
            router_target: None,
            rules_policy: RoutingPolicy::default(),
            fallback_policy: FallbackPolicy::Never,
        }
    }

    fn remote_only_profile(fb: FallbackPolicy) -> InferenceProfile {
        InferenceProfile {
            id: "ro".into(),
            name: "Remote only".into(),
            mode: InferenceMode::RemoteOnly,
            local_target: None,
            remote_targets: vec![tinfoil_remote(true), ppq_remote()],
            router_target: None,
            rules_policy: RoutingPolicy::default(),
            fallback_policy: fb,
        }
    }

    fn hybrid_profile() -> InferenceProfile {
        InferenceProfile {
            id: "hyb".into(),
            name: "Rules hybrid".into(),
            mode: InferenceMode::RulesHybrid,
            local_target: Some(text_local()),
            remote_targets: vec![tinfoil_remote(true)],
            router_target: None,
            rules_policy: RoutingPolicy {
                escalate_if_attachment: true,
                prefer_local_when_offline: true,
                escalate_if_message_longer_than: Some(8),
            },
            fallback_policy: FallbackPolicy::SameRoleOnly,
        }
    }

    fn smart_profile() -> InferenceProfile {
        InferenceProfile {
            id: "smart".into(),
            name: "Smart".into(),
            mode: InferenceMode::SmartRouting,
            local_target: Some(text_local()),
            remote_targets: vec![tinfoil_remote(true), ppq_remote()],
            router_target: Some(router_target()),
            rules_policy: RoutingPolicy::default(),
            fallback_policy: FallbackPolicy::SameRoleOnly,
        }
    }

    fn input<'a>(profile: &'a InferenceProfile, text: &'a str) -> RouteInput<'a> {
        RouteInput {
            profile,
            text,
            has_image_attachment: false,
            requires_tools: false,
            local_inference_enabled: true,
            is_remote_reachable: &always_reachable,
            attestation_available: true,
            force_role: None,
            router: None,
        }
    }

    // Reachability predicates for tests (named fns so they can be borrowed for
    // the lifetime of `RouteInput`; closures would not).
    fn always_reachable(_: &str) -> bool {
        true
    }
    fn never_reachable(_: &str) -> bool {
        false
    }
    fn tinfoil_unreachable(id: &str) -> bool {
        id != "tinfoil"
    }

    /// Canned router returning a fixed JSON blob.
    struct CannedRouter(&'static str);
    impl LocalRouterModel for CannedRouter {
        fn decide(&self, _prompt: &str) -> Result<String, String> {
            Ok(self.0.to_string())
        }
    }

    /// Router that fails N times then succeeds — used to exercise fallback.
    struct FailingRouter;
    impl LocalRouterModel for FailingRouter {
        fn decide(&self, _prompt: &str) -> Result<String, String> {
            Err("model offline".into())
        }
    }

    struct CountingRouter {
        calls: AtomicUsize,
        responses: Vec<&'static str>,
    }
    impl CountingRouter {
        fn new(responses: Vec<&'static str>) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                responses,
            }
        }
        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }
    impl LocalRouterModel for CountingRouter {
        fn decide(&self, _prompt: &str) -> Result<String, String> {
            let i = self.calls.fetch_add(1, Ordering::SeqCst);
            self.responses
                .get(i)
                .map(|s| s.to_string())
                .ok_or_else(|| "no more canned responses".to_string())
        }
    }

    // ── LocalOnly ────────────────────────────────────────────────────────────

    #[test]
    fn local_only_never_selects_remote() {
        let p = local_only_profile();
        let route = resolve_turn_plan(&input(&p, "hi")).unwrap();
        assert_eq!(route.decision, BackendRole::Local);
        assert_eq!(route.backend_id, "local-qwen");
        assert_eq!(route.mode, InferenceMode::LocalOnly);
        // Privacy boundary: no remote fallback ever.
        assert!(!route.fallback_allowed);
        assert!(route.fallback_targets.is_empty());
        assert!(!route.attestation_required);
    }

    #[test]
    fn local_only_rejects_image_when_target_is_text_only() {
        let p = local_only_profile();
        let mut i = input(&p, "describe this");
        i.has_image_attachment = true;
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(
            err,
            PlanError::CapabilityMismatch {
                backend_id: "local-qwen".into()
            }
        );
    }

    #[test]
    fn local_only_accepts_image_with_vision_target() {
        let mut p = local_only_profile();
        p.local_target = Some(vision_local());
        let mut i = input(&p, "describe this");
        i.has_image_attachment = true;
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.backend_id, "local-qwen-vl");
    }

    #[test]
    fn local_only_errors_when_local_disabled() {
        let p = local_only_profile();
        let mut i = input(&p, "hi");
        i.local_inference_enabled = false;
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(err, PlanError::LocalDisabled);
    }

    #[test]
    fn local_only_errors_without_local_target() {
        let mut p = local_only_profile();
        p.local_target = None;
        let err = resolve_turn_plan(&input(&p, "hi")).unwrap_err();
        assert_eq!(err, PlanError::NoLocalTarget);
    }

    // ── RemoteOnly ───────────────────────────────────────────────────────────

    #[test]
    fn remote_only_never_selects_local() {
        let p = remote_only_profile(FallbackPolicy::Never);
        let route = resolve_turn_plan(&input(&p, "hi")).unwrap();
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.backend_id, "tinfoil");
        assert!(route.attestation_required);
        assert!(!route.fallback_allowed);
    }

    #[test]
    fn remote_only_local_override_rejected_by_default() {
        let p = remote_only_profile(FallbackPolicy::Never);
        let mut i = input(&p, "hi");
        i.force_role = Some(BackendRole::Local);
        // Override ignored; still routes remote.
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Remote);
    }

    #[test]
    fn remote_only_local_override_honored_only_with_cross_role_policy() {
        let mut p = remote_only_profile(FallbackPolicy::AllowCrossRole);
        p.local_target = Some(text_local());
        let mut i = input(&p, "hi");
        i.force_role = Some(BackendRole::Local);
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Local);
    }

    #[test]
    fn remote_only_attestation_unavailable_blocks() {
        let p = remote_only_profile(FallbackPolicy::Never);
        let mut i = input(&p, "hi");
        i.attestation_available = false;
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(
            err,
            PlanError::AttestationUnavailable {
                backend_id: "tinfoil".into()
            }
        );
    }

    #[test]
    fn remote_only_same_role_fallback_lists_alternatives() {
        let p = remote_only_profile(FallbackPolicy::SameRoleOnly);
        let route = resolve_turn_plan(&input(&p, "hi")).unwrap();
        assert!(route.fallback_allowed);
        assert_eq!(route.fallback_targets.len(), 1);
        assert_eq!(route.fallback_targets[0].backend_id, "ppq");
    }

    // ── P1: LocalOnly rejects remote-role targets in the local slot ─────────

    #[test]
    fn local_only_rejects_remote_role_in_local_slot() {
        // A misconfigured profile (or hostile migration) puts a Remote target
        // in the local_target slot. LocalOnly must not emit a remote decision.
        let mut p = local_only_profile();
        p.local_target = Some(tinfoil_remote(true));
        let err = resolve_turn_plan(&input(&p, "hi")).unwrap_err();
        assert_eq!(
            err,
            PlanError::LocalTargetNotLocal {
                backend_id: "tinfoil".into()
            }
        );
    }

    // ── P2b: advertised fallback targets pass the turn's gates ──────────────

    #[test]
    fn fallback_targets_excluded_when_capability_unservable() {
        // Image turn, primary remote is vision-capable, same-role alternative
        // is text-only. The text-only remote must NOT be advertised as a
        // fallback -- the planner would have rejected it as primary.
        let p = remote_only_profile(FallbackPolicy::SameRoleOnly);
        let mut i = input(&p, "describe this");
        i.has_image_attachment = true;
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.backend_id, "tinfoil"); // vision-capable primary
        assert!(
            !route.fallback_allowed,
            "text-only ppq must not be a fallback for an image turn"
        );
        assert!(route.fallback_targets.is_empty());
    }

    #[test]
    fn same_backend_alternate_model_can_be_promoted_as_fallback() {
        // Same backend, different model: the text-only default cannot serve an
        // image turn, but the alternate model can. Fallback identity must be
        // backend+model, not backend only.
        let mut text_only = tinfoil_remote(false);
        text_only.model_id = "text-only".into();
        let mut vision_alt = tinfoil_remote(true);
        vision_alt.model_id = "vision-alt".into();
        let mut p = remote_only_profile(FallbackPolicy::SameRoleOnly);
        p.remote_targets = vec![text_only, vision_alt];
        let mut i = input(&p, "describe this");
        i.has_image_attachment = true;

        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.backend_id, "tinfoil");
        assert_eq!(route.model_id, "vision-alt");
        assert_eq!(route.decision, BackendRole::Remote);
    }

    #[test]
    fn fallback_targets_excluded_when_local_disabled_for_local_fallback() {
        // RemoteOnly AllowCrossRole with a local target; local inference off.
        // The local target must not appear in the fallback list.
        let mut p = remote_only_profile(FallbackPolicy::AllowCrossRole);
        p.local_target = Some(text_local());
        let mut i = input(&p, "hi");
        i.local_inference_enabled = false;
        let route = resolve_turn_plan(&i).unwrap();
        assert!(route
            .fallback_targets
            .iter()
            .all(|t| t.role != RouteTargetRole::Local));
    }

    // ── P2d: RemoteOnly local override checks local availability ─────────────

    #[test]
    fn remote_only_local_override_when_local_disabled_errors() {
        let mut p = remote_only_profile(FallbackPolicy::AllowCrossRole);
        p.local_target = Some(text_local());
        let mut i = input(&p, "hi");
        i.force_role = Some(BackendRole::Local);
        i.local_inference_enabled = false;
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(err, PlanError::LocalDisabled);
    }

    // ── P2e: RemoteOnly surfaces unavailable when no servable fallback ───────

    #[test]
    fn remote_only_unreachable_single_remote_errors() {
        // SameRoleOnly with one remote; remote unreachable; no alternative.
        let mut p = remote_only_profile(FallbackPolicy::SameRoleOnly);
        p.remote_targets = vec![tinfoil_remote(true)];
        let mut i = input(&p, "hi");
        i.is_remote_reachable = &never_reachable;
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(
            err,
            PlanError::RemoteUnavailable {
                backend_id: "tinfoil".into()
            }
        );
    }

    #[test]
    fn remote_only_unreachable_with_servable_alternate_promotes_alternate() {
        // Two remotes; primary (tinfoil) unreachable but ppq is servable. The
        // planner promotes ppq to the primary rather than returning a
        // known-dead route, and advertises no further same-role fallback.
        let p = remote_only_profile(FallbackPolicy::SameRoleOnly);
        let mut i = input(&p, "hi");
        i.is_remote_reachable = &tinfoil_unreachable;
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.backend_id, "ppq");
        assert!(!route.fallback_allowed);
    }

    // ── RulesHybrid ──────────────────────────────────────────────────────────

    #[test]
    fn hybrid_attachment_routes_remote() {
        let p = hybrid_profile();
        let mut i = input(&p, "hi");
        i.has_image_attachment = true;
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.reason_code, ReasonCode::AttachmentPresent);
    }

    #[test]
    fn hybrid_offline_routes_local() {
        let p = hybrid_profile();
        let mut i = input(&p, "hi");
        i.is_remote_reachable = &never_reachable;
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Local);
        assert_eq!(route.reason_code, ReasonCode::RemoteUnavailable);
    }

    #[test]
    fn hybrid_long_message_routes_remote() {
        let p = hybrid_profile();
        let route =
            resolve_turn_plan(&input(&p, "this message is deliberately long enough")).unwrap();
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.reason_code, ReasonCode::MessageTooLong);
    }

    #[test]
    fn hybrid_remote_override_wins_for_text_turn() {
        let p = hybrid_profile();
        let mut i = input(&p, "hi");
        i.force_role = Some(BackendRole::Remote);
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.reason_code, ReasonCode::UserOverride);
    }

    #[test]
    fn hybrid_default_is_local() {
        let p = hybrid_profile();
        let route = resolve_turn_plan(&input(&p, "hi")).unwrap();
        assert_eq!(route.decision, BackendRole::Local);
        assert_eq!(route.reason_code, ReasonCode::LocalDefault);
    }

    #[test]
    fn hybrid_attachment_ignores_local_override() {
        let p = hybrid_profile();
        let mut i = input(&p, "hi");
        i.has_image_attachment = true;
        i.force_role = Some(BackendRole::Local);
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.reason_code, ReasonCode::AttachmentPresent);
    }

    #[test]
    fn hybrid_rejects_remote_role_in_local_slot() {
        let mut p = hybrid_profile();
        p.local_target = Some(tinfoil_remote(true));

        let err = resolve_turn_plan(&input(&p, "hi")).unwrap_err();
        assert_eq!(
            err,
            PlanError::LocalTargetNotLocal {
                backend_id: "tinfoil".into()
            }
        );
    }

    // ── Smart routing ────────────────────────────────────────────────────────

    #[test]
    fn smart_accepts_valid_local_decision() {
        let p = smart_profile();
        let r = CannedRouter(
            r#"{"decision":"local","backend_id":"local-qwen","model_id":"qwen2.5","reason_code":"simple_turn","confidence":0.9}"#,
        );
        let mut i = input(&p, "hi");
        i.router = Some(&r);
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Local);
        assert_eq!(route.backend_id, "local-qwen");
        assert_eq!(route.reason_code, ReasonCode::SmartRouterLocal);
        assert!(route.router_invoked);
    }

    #[test]
    fn smart_accepts_valid_remote_decision() {
        let p = smart_profile();
        let r = CannedRouter(
            r#"{"decision":"remote","backend_id":"tinfoil","model_id":"kimi-k3","reason_code":"needs_reasoning","confidence":0.77}"#,
        );
        let mut i = input(&p, "complex question");
        i.router = Some(&r);
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.backend_id, "tinfoil");
        assert_eq!(route.reason_code, ReasonCode::SmartRouterRemote);
        assert!(route.attestation_required);
    }

    // ── Smart routing: validation rejections ───────────────────────────────
    // Under a Never fallback policy, unusable router output surfaces the raw
    // validation error. Under a fallback policy it is converted into a
    // SmartRouterFallback route (covered in the next section).

    #[test]
    fn smart_rejects_unknown_backend_under_never_policy() {
        let mut p = smart_profile();
        p.fallback_policy = FallbackPolicy::Never;
        let r = CannedRouter(
            r#"{"decision":"remote","backend_id":"evil","model_id":"x","reason_code":"r","confidence":0.5}"#,
        );
        let mut i = input(&p, "hi");
        i.router = Some(&r);
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(
            err,
            PlanError::RouterTargetDisallowed {
                backend_id: "evil".into()
            }
        );
    }

    #[test]
    fn smart_rejects_remote_when_attestation_unavailable_under_never_policy() {
        let mut p = smart_profile();
        p.fallback_policy = FallbackPolicy::Never;
        let r = CannedRouter(
            r#"{"decision":"remote","backend_id":"tinfoil","model_id":"kimi-k3","reason_code":"r","confidence":0.5}"#,
        );
        let mut i = input(&p, "hi");
        i.router = Some(&r);
        i.attestation_available = false;
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(
            err,
            PlanError::AttestationUnavailable {
                backend_id: "tinfoil".into()
            }
        );
    }

    #[test]
    fn smart_rejects_remote_model_mismatch_under_never_policy() {
        let mut p = smart_profile();
        p.fallback_policy = FallbackPolicy::Never;
        // tinfoil exists but wrong model id
        let r = CannedRouter(
            r#"{"decision":"remote","backend_id":"tinfoil","model_id":"wrong","reason_code":"r","confidence":0.5}"#,
        );
        let mut i = input(&p, "hi");
        i.router = Some(&r);
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(
            err,
            PlanError::RouterTargetDisallowed {
                backend_id: "tinfoil".into()
            }
        );
    }

    #[test]
    fn smart_invalid_json_errors_under_never_policy() {
        let mut p = smart_profile();
        p.fallback_policy = FallbackPolicy::Never;
        let r = CannedRouter("not json at all");
        let mut i = input(&p, "hi");
        i.router = Some(&r);
        let err = resolve_turn_plan(&i).unwrap_err();
        assert!(matches!(err, PlanError::RouterInvalid { .. }));
    }

    #[test]
    fn smart_router_call_failure_errors_under_never_policy() {
        let mut p = smart_profile();
        p.fallback_policy = FallbackPolicy::Never;
        let mut i = input(&p, "hi");
        i.router = Some(&FailingRouter);
        let err = resolve_turn_plan(&i).unwrap_err();
        assert!(matches!(err, PlanError::RouterInvalid { .. }));
    }

    // ── Smart routing: fallback on router failure (P2a) ─────────────────────

    #[test]
    fn smart_router_failure_falls_back_to_local() {
        // SameRoleOnly: router fails -> conservative local fallback.
        let p = smart_profile(); // SameRoleOnly
        let mut i = input(&p, "hi");
        i.router = Some(&FailingRouter);
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.mode, InferenceMode::SmartRouting);
        assert_eq!(route.decision, BackendRole::Local);
        assert_eq!(route.backend_id, "local-qwen");
        assert_eq!(route.reason_code, ReasonCode::SmartRouterFallback);
        assert!(route.router_invoked);
    }

    #[test]
    fn smart_invalid_json_falls_back_to_local() {
        let p = smart_profile();
        let r = CannedRouter("not json");
        let mut i = input(&p, "hi");
        i.router = Some(&r);
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.reason_code, ReasonCode::SmartRouterFallback);
        assert_eq!(route.backend_id, "local-qwen");
    }

    #[test]
    fn smart_disallowed_target_falls_back_to_local() {
        // Router picked an unknown remote; with a fallback policy we must not
        // reject the whole turn -- fall back to local.
        let p = smart_profile();
        let r = CannedRouter(
            r#"{"decision":"remote","backend_id":"evil","model_id":"x","reason_code":"r","confidence":0.5}"#,
        );
        let mut i = input(&p, "hi");
        i.router = Some(&r);
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.reason_code, ReasonCode::SmartRouterFallback);
        assert_eq!(route.decision, BackendRole::Local);
    }

    #[test]
    fn smart_attestation_unavailable_falls_back_to_local() {
        let p = smart_profile();
        let r = CannedRouter(
            r#"{"decision":"remote","backend_id":"tinfoil","model_id":"kimi-k3","reason_code":"r","confidence":0.5}"#,
        );
        let mut i = input(&p, "hi");
        i.router = Some(&r);
        i.attestation_available = false;
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.reason_code, ReasonCode::SmartRouterFallback);
        assert_eq!(route.decision, BackendRole::Local);
    }

    #[test]
    fn smart_failure_with_no_servable_fallback_errors() {
        // Local disabled AND Never-irrelevant: SameRoleOnly but the only local
        // candidate is the text-only local serving an image turn -> nothing
        // servable -> original error re-surfaces.
        let p = smart_profile();
        let mut i = input(&p, "describe this");
        i.has_image_attachment = true;
        i.router = Some(&FailingRouter);
        let err = resolve_turn_plan(&i).unwrap_err();
        assert!(matches!(err, PlanError::RouterInvalid { .. }));
    }

    #[test]
    fn smart_failure_cross_role_can_fall_back_to_remote() {
        // No local target at all; AllowCrossRole lets the router failure fall
        // back to the first servable remote (attestation available).
        let mut p = smart_profile();
        p.local_target = None;
        p.fallback_policy = FallbackPolicy::AllowCrossRole;
        let mut i = input(&p, "hi");
        i.router = Some(&FailingRouter);
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.reason_code, ReasonCode::SmartRouterFallback);
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.backend_id, "tinfoil");
    }

    #[test]
    fn smart_requires_router_target() {
        let mut p = smart_profile();
        p.router_target = None;
        let r = CannedRouter(
            r#"{"decision":"local","backend_id":"local-qwen","model_id":"qwen2.5","reason_code":"r","confidence":0.5}"#,
        );
        let mut i = input(&p, "hi");
        i.router = Some(&r);
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(err, PlanError::NoRouterTarget);
    }

    #[test]
    fn smart_requires_router_handle() {
        let p = smart_profile();
        let err = resolve_turn_plan(&input(&p, "hi")).unwrap_err();
        assert_eq!(err, PlanError::NoRouterTarget);
    }

    #[test]
    fn smart_rejects_remote_role_router_target_without_invoking_router() {
        let mut p = smart_profile();
        p.router_target = Some(tinfoil_remote(true));
        let r = CountingRouter::new(vec![
            r#"{"decision":"local","backend_id":"local-qwen","model_id":"qwen2.5","reason_code":"r","confidence":0.5}"#,
        ]);
        let mut i = input(&p, "hi");
        i.router = Some(&r);

        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(
            err,
            PlanError::RouterTargetNotLocal {
                backend_id: "tinfoil".into()
            }
        );
        assert_eq!(r.calls(), 0);
    }

    // ── SmartRouting: per-turn override honored (review finding 2) ───────────

    #[test]
    fn smart_override_local_skips_router_and_picks_local() {
        // Router would pick remote, but the user forced Local this turn.
        let p = smart_profile();
        let r = CannedRouter(
            r#"{"decision":"remote","backend_id":"tinfoil","model_id":"kimi-k3","reason_code":"r","confidence":0.9}"#,
        );
        let mut i = input(&p, "hi");
        i.router = Some(&r);
        i.force_role = Some(BackendRole::Local);
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Local);
        assert_eq!(route.backend_id, "local-qwen");
        // Override path bypasses the router.
        assert!(!route.router_invoked);
        assert_eq!(route.reason_code, ReasonCode::SmartRouterLocal);
    }

    #[test]
    fn smart_override_local_rejects_remote_role_in_local_slot() {
        let mut p = smart_profile();
        p.local_target = Some(tinfoil_remote(true));
        let mut i = input(&p, "hi");
        i.force_role = Some(BackendRole::Local);

        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(
            err,
            PlanError::LocalTargetNotLocal {
                backend_id: "tinfoil".into()
            }
        );
    }

    #[test]
    fn smart_router_local_decision_rejects_remote_role_in_local_slot() {
        let mut p = smart_profile();
        p.local_target = Some(tinfoil_remote(true));
        let r = CannedRouter(
            r#"{"decision":"local","backend_id":"tinfoil","model_id":"kimi-k3","reason_code":"r","confidence":0.9}"#,
        );
        let mut i = input(&p, "hi");
        i.router = Some(&r);

        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(
            err,
            PlanError::LocalTargetNotLocal {
                backend_id: "tinfoil".into()
            }
        );
    }

    #[test]
    fn smart_override_remote_picks_servable_remote() {
        let p = smart_profile();
        // Router is irrelevant; force Remote.
        let mut i = input(&p, "hi");
        i.force_role = Some(BackendRole::Remote);
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.backend_id, "tinfoil");
        assert!(!route.router_invoked);
    }

    #[test]
    fn smart_override_remote_skips_dead_primary_and_promotes_alternate() {
        // force Remote; default remote (tinfoil) unreachable; ppq is servable.
        let p = smart_profile();
        let mut i = input(&p, "hi");
        i.force_role = Some(BackendRole::Remote);
        i.is_remote_reachable = &tinfoil_unreachable;
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.backend_id, "ppq");
    }

    #[test]
    fn smart_override_remote_errors_when_all_remotes_unreachable() {
        let p = smart_profile();
        let mut i = input(&p, "hi");
        i.force_role = Some(BackendRole::Remote);
        i.is_remote_reachable = &never_reachable;
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(
            err,
            PlanError::RemoteUnavailable {
                backend_id: "tinfoil".into()
            }
        );
    }

    #[test]
    fn smart_override_remote_does_not_require_local_enabled() {
        // force Remote bypasses the router, so local inference being off must
        // not block the turn.
        let p = smart_profile();
        let mut i = input(&p, "hi");
        i.force_role = Some(BackendRole::Remote);
        i.local_inference_enabled = false;
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Remote);
    }

    #[test]
    fn smart_router_chooses_unreachable_remote_falls_back_to_local() {
        // Router picked a remote that is currently down. The reachability gate
        // inside validate_smart_target turns this into a router failure, which
        // the fallback path resolves to local.
        let p = smart_profile();
        let r = CannedRouter(
            r#"{"decision":"remote","backend_id":"tinfoil","model_id":"kimi-k3","reason_code":"r","confidence":0.9}"#,
        );
        let mut i = input(&p, "hi");
        i.router = Some(&r);
        i.is_remote_reachable = &tinfoil_unreachable;
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.reason_code, ReasonCode::SmartRouterFallback);
        assert_eq!(route.decision, BackendRole::Local);
    }

    // ── RulesHybrid: remote reachability on remote-forced branches ───────────

    #[test]
    fn hybrid_attachment_with_unreachable_remote_and_no_alternative_errors() {
        // Attachment forces remote; the only remote is down; no same-role
        // alternative to promote -> RemoteUnavailable, not a dead route.
        let p = hybrid_profile(); // single remote tinfoil
        let mut i = input(&p, "hi");
        i.has_image_attachment = true;
        i.is_remote_reachable = &never_reachable;
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(
            err,
            PlanError::RemoteUnavailable {
                backend_id: "tinfoil".into()
            }
        );
    }

    #[test]
    fn hybrid_attachment_promotes_servable_remote_alternative() {
        // Attachment forces remote; primary remote down; a second, vision-capable
        // remote is up and can serve the image turn.
        let mut p = hybrid_profile();
        let mut ppq_vl = ppq_remote();
        ppq_vl.backend_id = "ppq-vl".into();
        ppq_vl.capabilities.vision = true;
        p.remote_targets.push(ppq_vl);
        let mut i = input(&p, "hi");
        i.has_image_attachment = true;
        i.is_remote_reachable = &tinfoil_unreachable;
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.backend_id, "ppq-vl");
        assert_eq!(route.reason_code, ReasonCode::AttachmentPresent);
    }

    // ── Per-target reachability with multiple remotes ────────────────────────

    #[test]
    fn remote_only_per_target_reachability_promotes_servable_remote() {
        // Multi-remote RemoteOnly: only the default is down; ppq is up. The
        // planner promotes ppq (per-target health, not a single boolean).
        let p = remote_only_profile(FallbackPolicy::SameRoleOnly);
        let mut i = input(&p, "hi");
        i.is_remote_reachable = &tinfoil_unreachable;
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.backend_id, "ppq");
    }

    #[test]
    fn smart_requires_local_enabled() {
        let p = smart_profile();
        let r = CannedRouter(
            r#"{"decision":"local","backend_id":"local-qwen","model_id":"qwen2.5","reason_code":"r","confidence":0.5}"#,
        );
        let mut i = input(&p, "hi");
        i.router = Some(&r);
        i.local_inference_enabled = false;
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(err, PlanError::LocalDisabled);
    }

    #[test]
    fn smart_rejects_image_when_local_target_text_only() {
        // Router wrongly picks text-only local for an image turn → must reject.
        let p = smart_profile();
        let r = CannedRouter(
            r#"{"decision":"local","backend_id":"local-qwen","model_id":"qwen2.5","reason_code":"r","confidence":0.5}"#,
        );
        let mut i = input(&p, "describe this");
        i.has_image_attachment = true;
        i.router = Some(&r);
        let err = resolve_turn_plan(&i).unwrap_err();
        assert_eq!(
            err,
            PlanError::CapabilityMismatch {
                backend_id: "local-qwen".into()
            }
        );
    }

    #[test]
    fn smart_router_prompt_lists_candidates() {
        let p = smart_profile();
        let prompt = build_router_prompt(&p, "hello", true);
        assert!(prompt.contains("local-qwen"));
        assert!(prompt.contains("tinfoil"));
        assert!(prompt.contains("ppq"));
        assert!(prompt.contains("Has image attachment: true"));
    }

    #[test]
    fn smart_router_decision_parse_rejects_bad_decision_value() {
        let raw = r#"{"decision":"quantum","backend_id":"x","model_id":"y","reason_code":"r","confidence":0.5}"#;
        let err = SmartRouterDecision::parse(raw).unwrap_err();
        assert!(matches!(err, PlanError::RouterInvalid { .. }));
    }

    // ── Migration bridge ─────────────────────────────────────────────────────

    #[test]
    fn hybrid_profile_converts_to_rules_hybrid_inference_profile() {
        let legacy = HybridProfile {
            id: "default".into(),
            name: "Local to Tinfoil".into(),
            local_backend_id: "local-qwen".into(),
            local_model_id: "qwen2.5".into(),
            remote_backend_id: "tinfoil".into(),
            remote_model_id: "kimi-k3".into(),
            policy: RoutingPolicy {
                escalate_if_attachment: true,
                prefer_local_when_offline: true,
                escalate_if_message_longer_than: Some(8),
            },
            preprocessing: Default::default(),
        };
        let p = legacy.to_inference_profile();
        assert_eq!(p.mode, InferenceMode::RulesHybrid);
        assert_eq!(p.local_target.as_ref().unwrap().backend_id, "local-qwen");
        assert_eq!(p.remote_targets.len(), 1);
        assert_eq!(p.remote_targets[0].backend_id, "tinfoil");
        assert_eq!(p.fallback_policy, FallbackPolicy::SameRoleOnly);

        // And planning through the converted profile reproduces legacy behavior.
        let mut i = input(&p, "hi");
        i.has_image_attachment = true;
        let route = resolve_turn_plan(&i).unwrap();
        assert_eq!(route.decision, BackendRole::Remote);
        assert_eq!(route.reason_code, ReasonCode::AttachmentPresent);
    }

    // ── Reason-code stability ────────────────────────────────────────────────

    #[test]
    fn resolved_route_bridges_to_legacy_turn_routing() {
        let p = local_only_profile();
        let route = resolve_turn_plan(&input(&p, "hi")).unwrap();
        let legacy = route.to_turn_routing(Some("lo".into()));
        assert_eq!(legacy.backend_id, "local-qwen");
        assert_eq!(legacy.decision, BackendRole::Local);
        assert_eq!(legacy.profile_id.as_deref(), Some("lo"));
    }

    #[test]
    fn reason_code_human_strings_are_stable() {
        // UI maps these to copy; changing them is a breaking change.
        assert_eq!(ReasonCode::AttachmentPresent.human(), "attachment present");
        assert_eq!(
            ReasonCode::SmartRouterLocal.human(),
            "local router chose local"
        );
    }
}
