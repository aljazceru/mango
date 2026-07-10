# Phase 23: Memory Management UI + Agent UI - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-04
**Phase:** 23-memory-management-ui-agent-ui
**Areas discussed:** Memory list layout, Memory editing flow, Agent tool step display, Navigation integration
**Mode:** --auto (all decisions auto-selected)

---

## Memory List Layout

| Option | Description | Selected |
|--------|-------------|----------|
| Simple chronological list | Newest first, content preview + source conversation | :heavy_check_mark: |
| Grouped by conversation | Memories clustered under source conversation headers | |
| Card-based grid | Memory cards in a grid layout | |

**User's choice:** [auto] Simple chronological list (recommended default — matches existing list patterns)
**Notes:** Follows ConversationListView/ChatScreen list patterns already established in the codebase.

---

## Memory Editing Flow

| Option | Description | Selected |
|--------|-------------|----------|
| Inline edit with save/cancel | Tap to view, edit text directly, confirm to save | :heavy_check_mark: |
| Dedicated edit screen | Navigate to a separate edit screen | |
| Modal dialog | Pop-up modal for editing | |

**User's choice:** [auto] Inline edit with save/cancel (recommended default — simplest UX)
**Notes:** Avoids navigation complexity. Delete available from list view with confirmation.

---

## Agent Tool Step Display

| Option | Description | Selected |
|--------|-------------|----------|
| Name + truncated input + truncated output | Show tool name, first ~200 chars of input and output per step | :heavy_check_mark: |
| Name + result only | Current behavior — tool name and result snippet only | |
| Full expandable I/O | Collapsible sections showing complete input and output | |

**User's choice:** [auto] Name + truncated input + truncated output (recommended default — satisfies AUI-02)
**Notes:** Requires extending AgentStepSummary with tool_input field.

---

## Navigation Integration

| Option | Description | Selected |
|--------|-------------|----------|
| Top-level nav items | Both Memory and Agent alongside existing nav entries | :heavy_check_mark: |
| Sub-menu under Settings | Nest Memory and Agent under Settings screen | |
| Tab bar (mobile) | Bottom tab bar with Memory and Agent tabs | |

**User's choice:** [auto] Top-level nav items (recommended default — consistent with existing flat navigation)
**Notes:** Re-enable agent by uncommenting hidden nav entries. Add new Memory nav entry.

---

## Claude's Discretion

- Memory list empty state messaging
- Exact truncation lengths for previews
- Memory count badge in navigation
- Swipe vs long-press for delete on mobile
- Agent step visual styling

## Deferred Ideas

None — discussion stayed within phase scope.
