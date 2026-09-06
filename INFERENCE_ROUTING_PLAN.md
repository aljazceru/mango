# Inference Routing Improvement Plan

## Reader And Goal

This plan is for an internal engineer implementing Mango's next routing model.
After reading it, they should be able to replace the current local-to-remote
hybrid cascade with explicit inference modes:

- Local only
- Remote only
- Rules-based hybrid routing
- Smart routing where a local router model chooses whether to answer locally or
  route to an allowed remote backend/model

## Current Behavior

Mango already supports local inference, remote provider inference, and a hybrid
profile. The current hybrid profile is a virtual backend that pairs exactly one
local model with exactly one remote model. The route is selected by deterministic
rules:

- Attachments route to remote when enabled.
- A one-shot user override can force remote for the next text turn.
- Failed remote health can prefer local.
- Long prompts can route to remote.
- Ordinary turns default to local.

This is useful but too narrow for the desired product model. It does not provide
first-class local-only or remote-only modes, and it cannot let a local model
choose among several remote candidates.

## Target Product Model

Expose routing as a first-class conversation/default setting, not as a special
case of backend selection.

The user-facing modes should be:

### Local Only

All chat turns run on a selected local backend/model.

Required behavior:

- Never sends message content to a remote provider.
- Rejects unsupported features, such as image inputs when the selected local
  model cannot handle them.
- Does not fail over to remote on local errors.
- Shows clear errors when local inference is disabled, unavailable, or the model
  is missing.

### Remote Only

All chat turns run on a selected remote backend/model.

Required behavior:

- Never falls back to local unless the user explicitly enables an emergency
  fallback policy.
- Requires attestation preflight for confidential remote backends before the
  user message is accepted.
- Supports the existing remote transports, provider-specific secure transports,
  tool-use path, and vision path.

### Rules-Based Hybrid

This is the current hybrid feature, formalized as one routing mode.

Required behavior:

- Uses deterministic policy rules.
- Supports local default and remote escalation.
- Keeps existing attachment, long-prompt, offline, and one-shot override
  behavior.
- Enforces fallback boundaries defined by the profile.

### Smart Routing

A local router model decides the route for each turn. It may answer locally or
select one allowed remote target.

Required behavior:

- Runs route selection locally before any remote request is made.
- Uses a strict, validated JSON route decision.
- Only allows targets listed in the active profile.
- Requires remote attestation before sending the user turn remotely.
- Provides structured route provenance for UI display and auditability.
- Falls back according to explicit profile policy, not generic backend order.

## Core Design

### New Inference Profile

Introduce a new profile shape that separates routing policy from backend
records.

Suggested records:

```text
InferenceMode:
  - LocalOnly
  - RemoteOnly
  - RulesHybrid
  - SmartRouting

RouteTarget:
  - backend_id
  - model_id
  - role: local | remote
  - required_capabilities: text, vision, tools
  - require_attestation

InferenceProfile:
  - id
  - name
  - mode
  - local_target
  - remote_targets
  - router_target
  - rules_policy
  - fallback_policy
  - privacy_policy
```

Existing hybrid profiles should migrate into `RulesHybrid` profiles with one
local target and one remote target. Existing conversations that point at a
hybrid virtual backend should continue to resolve correctly after migration.

### Route Resolution API

Replace ad hoc route resolution with one core route planner.

```text
resolve_turn_plan(input) -> ResolvedRoute
```

The input should include:

- Active inference profile
- User text
- Attachment metadata
- Conversation metadata needed for routing
- Backend health
- Attestation state
- Optional user override

The result should include:

- Selected backend ID
- Selected model ID
- Decision: local or remote
- Mode used
- Structured reason code
- Human-readable reason
- Whether attestation is required
- Whether route selection involved a local router model
- Whether fallback is allowed, and to which targets

Route planning must finish before the user message is persisted. If planning
fails, no user message should be appended to the conversation.

### Smart Router Contract

Smart routing needs a local, non-streaming routing call. Do not reuse the chat
streaming channel for router output.

The local router should receive:

- Last user message
- Attachment presence and type
- Compact conversation summary or recent turns
- Candidate local target
- Candidate remote targets
- Target capabilities
- Privacy constraints
- Cost or latency hints if available

The router must return strict JSON:

```json
{
  "decision": "local",
  "backend_id": "local-model",
  "model_id": "qwen2.5",
  "reason_code": "simple_turn",
  "confidence": 0.82
}
```

For remote:

```json
{
  "decision": "remote",
  "backend_id": "tinfoil",
  "model_id": "qwen3-vl-30b",
  "reason_code": "needs_stronger_reasoning",
  "confidence": 0.77
}
```

Validation rules:

- Reject unknown backend or model IDs.
- Reject targets outside the active profile.
- Reject remote targets that are not configured or currently disallowed.
- Reject local targets when local inference is disabled.
- Reject route decisions incompatible with attachment requirements.
- Treat invalid JSON as a local-router failure and apply the profile fallback
  policy.

## Actor Flow

### Normal Route Flow

1. User submits message.
2. Actor identifies the current conversation and inference profile.
3. Actor prepares persistence text and wire text.
4. Actor resolves the route.
5. Actor performs preflight validation.
6. Actor persists the user message.
7. Actor builds RAG, memory, and tool context.
8. Actor dispatches local generation or remote streaming.
9. Actor records route summary for UI.

### Smart Route Flow

Smart routing adds an async pre-send stage.

1. User submits message.
2. Actor snapshots pending attachments and message text.
3. Actor enters `Choosing route` busy state.
4. Actor runs local router decision on a blocking/local task.
5. Actor receives route decision.
6. Actor validates the selected route.
7. Actor resumes the normal route flow at preflight.

If the remote route requires attestation, keep the existing pending attested
send behavior, but preserve the smart route decision in the pending send record
so the resumed send does not re-run route selection.

## Fallback Policy

Fallback must be mode-aware.

Local only:

- No remote fallback.
- Local failure is surfaced to the user.

Remote only:

- Remote fallback is allowed only to configured remote alternatives if the
  profile allows it.
- No local fallback unless explicitly enabled.

Rules hybrid:

- Local route can retry only the selected local target unless explicitly
  configured otherwise.
- Remote route can retry the selected remote target or configured confidential
  remote alternatives.

Smart routing:

- First retry within the same decision role when possible.
- If the selected route fails and policy permits re-routing, the actor can run
  a second local routing decision with failed target excluded.
- Never cross privacy boundaries without explicit policy.

## UI Changes

Replace the current single hybrid toggle with an inference mode selector.

Recommended controls:

- Segmented control: Local, Remote, Rules, Smart
- Local target picker
- Remote target list with model picker per backend
- Smart router model picker
- Fallback policy controls
- Per-turn route override menu: Auto, Local this turn, Remote this turn

Route chips should use structured route summaries:

- Local: "Answered locally"
- Remote: "Routed to <provider>"
- Rules: "Routed by rule: <reason>"
- Smart: "Local router chose <target>: <reason>"

Avoid platform-specific route wording drift by having Rust expose display-ready
route summary fields or stable reason codes with shared mapping.

## Persistence And Migration

Add migrations for:

- `inference_profiles`
- `inference_profile_targets`
- Optional `conversation_inference_profile_id`
- Optional route decision audit rows if durable route history is desired

Migration behavior:

- Convert existing hybrid rows to rules-based inference profiles.
- Preserve current default backend behavior by resolving old hybrid virtual IDs.
- Keep old hybrid read path temporarily for backward compatibility if needed.
- Add cleanup only after all platform bindings and UI code are migrated.

## Testing Plan

Core routing tests:

- Local-only never selects remote.
- Remote-only never selects local.
- Rules hybrid preserves existing attachment, long-prompt, offline, and override
  behavior.
- Smart route accepts valid local JSON.
- Smart route accepts valid remote JSON.
- Smart route rejects unknown targets.
- Smart route rejects remote when attestation is required but unavailable.
- Invalid smart JSON applies fallback policy.

Actor tests:

- Route failure does not persist the user message.
- Pending attachments survive smart route preflight and attestation preflight.
- Image turns route only to vision-capable targets.
- Local-only local failure does not trigger remote failover.
- Remote-only remote failure does not trigger local failover.
- Smart remote route records structured route summary.

UI tests or snapshot checks:

- Mode selector renders all modes.
- Per-turn override resets after send.
- Route chip reflects structured route result.
- Disallowed controls are disabled when required models/providers are missing.

## Implementation Phases

### Phase 1: Core Types And Compatibility

- Add `InferenceMode`, `RouteTarget`, `InferenceProfile`, and `ResolvedRoute`.
- Add conversion from existing hybrid profiles to rules-based profiles.
- Keep existing hybrid behavior unchanged.
- Add unit tests for compatibility.

### Phase 2: First-Class Local And Remote Modes

- Implement local-only and remote-only route planning.
- Enforce failover boundaries.
- Add UI mode selector with local and remote modes.
- Add tests for no-cross-boundary routing.

### Phase 3: Rules Hybrid Refactor

- Move current hybrid cascade into the new route planner.
- Keep existing user-visible behavior.
- Replace free-form reason strings with structured reason codes.
- Update route chip rendering to use the new summary.

### Phase 4: Smart Router Infrastructure

- Add a non-streaming local router call.
- Add pending smart route actor state.
- Add strict JSON parsing and target validation.
- Add fallback handling for local router failure.

### Phase 5: Smart Routing Product Surface

- Add smart mode UI.
- Add remote target list management.
- Add router model picker.
- Add route provenance display.

### Phase 6: Cleanup And Hardening

- Remove obsolete hybrid-only assumptions.
- Remove or alias old hybrid virtual backend paths.
- Add route decision telemetry/logging without message content.
- Review privacy copy and onboarding language for the new modes.

## Open Decisions

- Whether route decisions should be persisted as audit rows or only exposed as
  last-turn UI state.
- Whether smart routing may send a compact conversation summary to the local
  router, or only the current turn and metadata.
- Whether remote-only should allow explicit local emergency fallback.
- Whether smart routing should allow multiple remote targets per provider or one
  selected model per provider.
- Whether tool-use capability should be a route constraint or a post-route
  dispatch check.

## Definition Of Done

- Users can select local-only, remote-only, rules hybrid, or smart routing.
- Each mode enforces its privacy and fallback boundary.
- Smart routing chooses only from allowed targets.
- Attestation preflight still blocks remote sends before message persistence.
- Existing hybrid profiles continue to work after migration.
- Platform UIs show consistent route state and reasons.
- Tests cover route selection, validation, fallback, and preflight behavior.
