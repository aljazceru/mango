# Phase 29: Wire VectorIndex DEK End-to-End - Discussion Log (Assumptions Mode)

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions captured in CONTEXT.md — this log preserves the analysis.

**Date:** 2026-04-09
**Phase:** 29-wire-vectorindex-dek
**Mode:** assumptions (--auto)
**Areas analyzed:** DEK Storage in ActorState, VectorIndex Initialization Timing, Save Call Site Plumbing, Backward Compatibility

## Assumptions Presented

### DEK Storage in ActorState
| Assumption | Confidence | Evidence |
|------------|-----------|----------|
| Add `dek: Option<Zeroizing<[u8; 32]>>` to ActorState, set on unlock, cleared on LockApp | Confident | lib.rs §772 (ActorState), §4301/§4498/§5542 (auth handlers), §4529 (LockApp) |
| On LockApp, clear DEK alongside db drop — no key material in memory while locked | Confident | lib.rs §4545 (db = None pattern), Zeroizing usage in auth handlers |

### VectorIndex Initialization Timing
| Assumption | Confidence | Evidence |
|------------|-----------|----------|
| Defer VectorIndex creation to post-unlock in encrypted mode (Case D) | Confident | lib.rs §2904-2913 (startup Case D), §2911 (VectorIndex::new with None) |
| Non-encrypted mode (Case B/C) continues creating VectorIndex at startup | Confident | lib.rs §2900-2903 (Case B/C routing) |

### Save Call Site Plumbing
| Assumption | Confidence | Evidence |
|------------|-----------|----------|
| All 4 save(None) sites must pass DEK from ActorState | Confident | lib.rs §4020, §4221, §5276, §5354 |
| dispatch_tools does not need DEK — only calls search/add, never save | Confident | agent/tools.rs §249 |

### Backward Compatibility
| Assumption | Confidence | Evidence |
|------------|-----------|----------|
| DEK remains Option in all VectorIndex APIs for pre-encryption installs | Confident | rag/index.rs §40, §128 (Option<&[u8; 32]> params) |

## Corrections Made

No corrections — all assumptions confirmed (auto mode, all Confident).

## Auto-Resolved

All assumptions were Confident — no Unclear items to auto-resolve.
