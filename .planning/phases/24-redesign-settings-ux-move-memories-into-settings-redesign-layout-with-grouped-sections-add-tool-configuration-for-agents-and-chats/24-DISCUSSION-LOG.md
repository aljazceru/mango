# Phase 24: Redesign Settings UX - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-05
**Phase:** 24-redesign-settings-ux
**Areas discussed:** Tool Configuration Scope, Memory Count Strategy, Section Grouping, Home Toolbar Migration
**Mode:** Auto (recommended defaults selected)

---

## Tool Configuration Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Brave API key only | Add only the Brave Search API key field. Per-tool toggles deferred. | ✓ |
| Full tool enable/disable matrix | Add per-tool toggle switches (web search, URL fetch, file ops, math) | |
| Brave key + tool toggles | Both API key and enable/disable switches in one phase | |

**User's choice:** Brave API key only (auto-selected: recommended default)
**Notes:** Research recommends starting with just the API key — directly unblocks web search for users. Settings table supports arbitrary key-value pairs for future toggles.

---

## Memory Count Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Re-query COUNT(*) | Reload count from DB on each mutation (DeleteMemory, MemoryExtractionComplete) | ✓ |
| Increment/decrement | Track count changes in-memory without DB round-trip | |

**User's choice:** Re-query COUNT(*) (auto-selected: recommended default)
**Notes:** Simpler approach, avoids off-by-one from batch extraction. COUNT query is trivial overhead.

---

## Section Grouping

| Option | Description | Selected |
|--------|-------------|----------|
| Platform-native headers only | Use iOS Section labels, Android uppercase Text, Desktop section_header — no extra dividers | ✓ |
| Custom visual separators | Add explicit divider lines between new sections | |

**User's choice:** Platform-native headers only (auto-selected: recommended default)
**Notes:** Consistent with existing PROVIDERS/DEFAULTS/APPEARANCE section style.

---

## Home Toolbar Migration

| Option | Description | Selected |
|--------|-------------|----------|
| Remove only Memories | Docs, Agents, Settings stay. Minimal change. | ✓ |
| Consolidate multiple items | Also move Documents or other items into Settings | |

**User's choice:** Remove only Memories (auto-selected: recommended default)
**Notes:** Minimal scope. Only Memories moves into Settings; other toolbar items remain unchanged.

---

## Claude's Discretion

- Memory section icon choice on iOS
- Exact border/padding values within platform conventions
- "Configured" indicator placement in Tools section
- Desktop Message variant naming

## Deferred Ideas

- Per-tool enable/disable toggles — future phase
- Memory categories display in Settings row
