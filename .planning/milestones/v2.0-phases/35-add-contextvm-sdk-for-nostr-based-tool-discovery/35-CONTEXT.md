# Phase 35: Add contextvm-sdk for Nostr-based tool discovery — Context

**Gathered:** 2026-05-08
**Status:** Ready for planning
**Source:** Direct chat description from user (PRD-style express path)

<domain>
## Phase Boundary

Integrate the `contextvm-sdk` crate (Nostr-based tool discovery + invocation
protocol) into the Rust core so that conversations and agents can discover and
call **remote tools** advertised over Nostr — a "tool marketplace" model that
extends the existing local tool dispatch (Brave Search, URL fetch, file ops,
calculator) with arbitrary third-party tools.

Two user affordances under **Settings → Tools**:

1. **Discover tools** button — opens a screen that lists tools currently
   advertised on the configured Nostr relays. User picks which tools to enable
   for use in this app. Selection is persisted across launches.
2. **Automatically discover and use tools** checkbox — when on, the app
   queries Nostr for relevant tools each conversation/agent turn and offers
   them to the LLM automatically (no manual selection step).

Both affordances reuse the **existing agent/chat tool dispatch infrastructure**
that was built in Phase 22 (agent tools) and extended in Phase 27 (per-chat
tool toggle). A contextvm tool, once enabled, appears as just another callable
tool to the LLM via the existing OpenAI-compatible `tools` array.

**Platforms shipped this phase:** Android + Desktop (iced).
**iOS:** explicitly deferred — the iced/Compose flows are simpler to validate
end-to-end first; iOS UI mirror happens in a follow-up phase once the Rust
core API and the relay/persistence story have been proven on two platforms.

**Relays used:** the contextvm-sdk default relay set, plus `relay.nostr.net`
appended to the configured relay list.

</domain>

<decisions>
## Implementation Decisions

### Capability scope
- **D-01:** Phase 35 ships **both** discovery (subscribe to tool-announcement
  Nostr events) and invocation (request → response over Nostr) of remote
  tools. Discovery alone is not the deliverable.
- **D-02:** Discovered/enabled contextvm tools integrate with the **existing**
  tool-call dispatch path (agent ReAct loop and chat tool round). The LLM sees
  them as additional entries in the OpenAI-compatible `tools` array. No
  parallel dispatch path. The dispatch layer routes a tool call to either a
  local handler (existing) or a contextvm invocation (new) based on tool
  origin.

### Settings UI surface
- **D-03:** New entries live under **Settings → TOOLS** (the section
  introduced in Phase 24). Two new controls:
  - **"Discover tools"** tappable row → opens a Tool Discovery screen
    listing available contextvm tools with per-tool enable/disable toggles.
  - **"Automatically discover and use tools"** boolean toggle row.
- **D-04:** The Tool Discovery screen shows: tool name, description, provider
  (Nostr pubkey or display name from the announcement), and an enable toggle.
  Lists are populated from a live or recently-cached Nostr query.
- **D-05:** The auto-discover toggle defaults to **off**. When on, the app
  pulls tool announcements at conversation start (or per turn — Claude's
  discretion below) and offers a deduplicated set to the LLM without further
  user interaction.

### Platforms
- **D-06:** Ship Android (Jetpack Compose) and Desktop (iced) UI in this
  phase. iOS deferred to a follow-up phase that mirrors the same Rust core
  API. UniFFI bindings are still regenerated for all 3 platforms — only the
  Swift UI layer is unmodified.

### Persistence
- **D-07:** Manually-enabled contextvm tools are persisted across launches
  (per-device, in the existing SQLite database). The auto-discover toggle is
  persisted as a setting alongside `memories_enabled` etc.

### Relays
- **D-08:** The relay set used for contextvm queries = `contextvm-sdk` default
  relays ∪ `relay.nostr.net`. No user-facing relay management in this phase
  (deferred — see deferred section).

### Compatibility constraints
- **D-09:** contextvm-sdk and any transitive Nostr crates must be **pure-Rust,
  no OpenSSL** (CLAUDE.md hard constraint). If any dep pulls openssl-sys /
  native-tls, drop it or feature-gate it; document the workaround. Cross-
  compile for `aarch64-apple-ios`, `aarch64-linux-android`, and Linux/macOS
  desktop must succeed (iOS toolchain may need human-verify if not local).
- **D-10:** Rust core remains the single owner of business logic per RMP. UI
  layers stay thin: surface a `Vec<DiscoverableTool>` from the Rust core,
  raise actions back into the actor, never speak Nostr directly from
  Kotlin/iced.

### Error handling
- **D-11:** Relay-unreachable, no-tools-found, malformed-announcement, and
  invocation-failure paths must all degrade gracefully — never crash the
  conversation. A relay-unreachable state surfaces as an empty discovery
  list with a small error/toast indicating "couldn't reach relays" (exact
  copy at Claude's discretion).
- **D-12:** Invocation failures surface as a tool-call error result that the
  LLM can read in its next round (consistent with how local tools handle
  errors today — they return a string error, not a Rust panic).

### Tool-call origin tracking
- **D-13:** When a contextvm tool fires, the agent step / chat round should
  surface (visually, somewhere in the UI) that this tool came from a remote
  Nostr provider, not a local built-in. The exact placement is Claude's
  discretion — at minimum it MUST appear in agent step summaries
  (`AgentStepSummary.tool_input` already carries provenance fields it can
  extend). Invocation provenance should be visible enough that a curious
  user can answer "where did this answer come from?" without reading code.

### Claude's Discretion

The following decisions are intentionally not pre-locked. The planner
researches contextvm-sdk's actual API and chooses what fits best, documenting
its choice in PLAN.md frontmatter:

- **Auto-discover heuristic.** "Automatically discover and use tools" must
  produce *something* useful. Options range from "subscribe at conversation
  start, send the union of all announced tool schemas to the LLM each turn,
  let it pick" to "rank announcements by tag-match against recent message
  text". Pick the simplest path that's actually useful given contextvm-sdk's
  query primitives. Document the choice as a CONTEXT amendment if non-obvious.
- **Relay connection lifecycle.** Persistent subscription vs on-demand connect
  per discovery query — pick what contextvm-sdk's API steers toward and what
  conserves battery on Android. Note: app may sleep / background; assume the
  Nostr connection is best-effort and may need to reconnect on resume.
- **Storage schema.** New SQLite migration vs JSON-blob column on an existing
  table — Claude's call. Schema must round-trip enabled-tool list (tool id,
  display name, provider pubkey, schema JSON, enabled bool) and must match
  RMP migration patterns from prior phases.
- **Tool-call routing dispatch shape.** How the existing dispatch_tools
  function in the Rust core decides "local vs contextvm" — a tag on the tool
  schema, an in-memory map of "tools we know are remote", or a closure
  registered at startup. Claude picks; the chosen pattern must NOT require
  every existing local tool to be touched.
- **Discovery refresh model.** Pull-on-open vs background pull vs cached-
  with-stale-while-revalidate. Lean toward simple (pull on open) for v1.
- **Loading & empty states.** Spinner vs skeleton vs nothing during
  discovery query. UI-SPEC-level detail; defer to UI design contract.
- **Auto-discover scope per turn.** Whether the auto-discover query runs
  once per conversation, once per turn, or on a timer. Pick what's both
  cheap and useful.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project guidelines & stack
- `CLAUDE.md` — Tech stack constraints (no OpenSSL, pure-Rust crates,
  cross-platform mobile build, OpenAI-compatible API)
- `.claude/skills/spike-findings-confidential-app/SKILL.md` — validated
  patterns from prior provider-integration spikes (auto-loaded via Skill())

### Existing tool infrastructure (must be respected, not duplicated)
- `rust/src/agent/` — Phase 22 ReAct loop with tool dispatch (Brave, URL
  fetch, file ops, calculator). New contextvm tools must plug into this.
- Phase 27 `tools_enabled` per-conversation toggle — Phase 35 must coexist
  cleanly with this; users with tools off should see no contextvm calls.
- `rust/src/lib.rs::dispatch_tools` (or equivalent) — current local-tool
  dispatch fan-out point.

### UI surface conventions
- `rust/src/Settings/` Tools section (Phase 24) — where new rows insert
- `Phase 26 sub-screen pattern` — Settings sub-screens use tappable summary
  rows + a dedicated screen variant (mirror this for "Tool Discovery")
- `AgentStepSummary` UniFFI record — likely needs extension for provenance

### contextvm-sdk
- `https://crates.io/crates/contextvm-sdk` — primary dependency. The
  planner MUST resolve current version + read live docs via Context7
  before locking API shapes into PLAN.md.

</canonical_refs>

<specifics>
## Specific Ideas

- The "tool marketplace" mental model: discovered tools should feel
  comparable to "extensions" or "apps from a store" — the user opts in
  per-tool (like browser extension permissions), and once enabled the LLM
  treats them as native capabilities.
- The auto-discover checkbox is for users who want the marketplace to
  "just work" — they trust that any announced tool of a given shape can
  be safely offered to the LLM without per-tool review.
- Existing tool dispatch infrastructure from Phase 22 + Phase 27 is the
  scaffold. No parallel "remote tools" subsystem.
- Default contextvm relays plus `relay.nostr.net`. No user-editable relay
  list this phase.

</specifics>

<deferred>
## Deferred Ideas

- **iOS UI.** Swift UI mirror of the Tool Discovery screen + the two
  settings rows. Defer to a follow-up phase once Android/Desktop have
  validated the Rust API surface.
- **User-editable relay list.** This phase ships with the contextvm-sdk
  defaults + `relay.nostr.net` hardcoded. A future phase can expose
  Settings → Relays for adding/removing relay URLs.
- **Per-tool permission scopes.** "This tool can read URLs but not files"
  granular permissions are out-of-scope. v1 is a binary enable/disable per
  tool.
- **Tool reputation / signed announcements.** A future phase may add
  pubkey allowlists, NIP-05 verification, or signed-by-trusted-keys
  filtering. Not this phase.
- **Tool authoring / publishing from the app.** Read-only consumer
  experience this phase. The app does not announce its own tools to
  Nostr.
- **Caching announcements offline.** Pull-on-demand only. Caching tool
  announcements for offline browsing of "what was last seen" is not
  required.

</deferred>

---

*Phase: 35-add-contextvm-sdk-for-nostr-based-tool-discovery*
*Context gathered: 2026-05-08 via direct chat description*
