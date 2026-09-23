# Dead Code & Dependency Mitigation Plan (rev 2)

Source: ponytail-audit + adversarial review by grok CLI (findings incorporated).
Scope: dead code / over-engineering only. Review verdict items marked **[grok]**.

## Current state (verified)

- `to-delete/` — 1.9 GB, zero references in justfile/scripts/.github/zapstore.yaml
- 6 stale locked git worktrees under `/home/lio/g/confidential-app/.claude/worktrees/` (~330 MB), registered in this clone's metadata
- `artifacts/` — 344 MB logs/PNGs, **nothing tracked** (`git ls-files artifacts/` = empty). Keep `artifacts/android-screenshots/` (2.6 MB, referenced by zapstore.yaml:16-20)
- Root process docs + screenshots (untracked; explicit list below)
- `macpass.txt` at repo root — password file
- Dead Rust symbols (prod-dead, word-boundary grep, excluding own file + own tests):

  | Cut | Location | Notes |
  |---|---|---|
  | `hydrate_from_profile_db` alias n/a — `hydrate_from_db` | contextvm/dispatch.rs:156 (~20 ln) | only "ref" is a dead logcat regex, scripts/contextvm_logcat.sh:61 — remove that pattern too |
  | `delete_contextvm_tool` | persistence/queries.rs:1626 (5 ln) | |
  | `update_agent_step_status` + its test | persistence/queries.rs:293 + tests/agent.rs:184-215 | |
  | `DirectoryFileRow.id` + `.source_id` dead fields | persistence/queries.rs:1242-1246 | **[grok]** plan rev1 said "ChunkRow" — wrong struct; `ChunkRow` (queries.rs:857) is live (text @ tools.rs:417, id @ tests/rag.rs:141) — do not touch |
  | `tinfoil_backend` | llm/backend.rs:116 (~19 ln) | rewrite its `to_summary` tests (backend_config.rs:16-33) on `known_provider_presets`, don't drop coverage **[grok]** |
  | `\bformat_attestation_url\b` (venice only) | llm/venice.rs:99 + its tests in tests/venice.rs | **[grok]** grep trap: prefix-matches live `format_redpill_attestation_url` — word-boundary + file-scoped |
  | `build_venice_chat_body_for_test` | llm/venice.rs:287-293 (7-ln wrapper) | **[grok]** rev1 overstated at ~180 ln; `build_venice_chat_body` (venice.rs:234, used @ 619, 702) is LIVE — keep |
  | `verify_nvidia_jwt` | attestation/nvidia.rs:53-124 + its direct tests in tests/attestation_nvidia.rs | **[grok]** live path `fetch_and_verify_nvidia` inlines JWT verify (nvidia.rs:200-317); KEEP `NvidiaAttestationClaims` (used there); only `.iss` field is dead |
  | `fetch_provider_profile` (single-pubkey fn ONLY) | contextvm/discovery.rs:241-324 (~83 ln) | **[grok]** `fetch_provider_profiles_batch` (discovery.rs:329-422, called @ 183) is LIVE — range must not spill into it; no fixtures exist to delete |

- 27 `#[allow(dead_code)]` — several stale on LIVE code (`delete_directory_file` @ queries.rs:1437 used @ lib.rs:12412; `AttestationCache` @ cache.rs:31; `ContextvmError` @ error.rs:11) — blanket strip unsafe; dead_code is a warning, not an error gate **[grok]**
- `scraper` dep used once: body-text extraction in `dispatch_fetch_url` (tools.rs:549-569) — WITH `MAX_CHARS = 8000` truncation (tools.rs:564-567) and 1 MB download cap (572)

## Wave 0 — prep

1. `git status` clean, branch `cleanup/dead-code`.

## Wave 1 — working-tree cleanup (untracked only)

1. `rm -rf to-delete/`
2. `git worktree remove --force` × 6 stale locked trees, `git worktree prune` — confirm no active agent session owns one first
3. `rm -rf artifacts/ios-runs artifacts/mobile-runs artifacts/android-videos artifacts/designer-screenshots artifacts/desktop-runs` (~341 MB). Keep `artifacts/android-screenshots/`.
4. Delete root process docs by EXPLICIT list (never a `*.md` glob — `README.md`/`SECURITY.md`/`CLAUDE.md` stay): ANDROID_REMEDIATION_IMPLEMENTATION_PLAN, APP_REVIEW_FINAL_UI_PASS, APP_REVIEW_FOLLOWUP, APP_REVIEW_IMPLEMENTATION_REPORT, APP_REVIEW_REMEDIATION_PLAN, APP_REVIEW_SECOND_PASS, APP_REVIEW_VERIFICATION, BUILD_ISSUES_IOS_MAC, INFERENCE_ROUTING_PLAN, MOBILE_REMEDIATION_PLAN_JUNIOR_DEVS, NDK_COMPATIBILITY_IMPLEMENTATION_REPORT, NDK_COMPATIBILITY_REMEDIATION_PLAN, NDK_COMPATIBILITY_REVIEW, PRE_RELEASE_FINDINGS, RELEASE_NOTES_PPQ, SECURITY_REVIEW, UX_EVALUATION (.md), plus `screenshot-*.png`, `artifacts-mango-sim-launch.png`, `bug-report.png`
5. SECURITY: delete `macpass.txt` AND rotate every credential in it

## Wave 2 — dead code cuts (one commit each; `cargo check && cargo test` green after each)

Order = smallest risk first:

1. `delete_contextvm_tool`
2. `hydrate_from_db` + logcat pattern (scripts/contextvm_logcat.sh:61)
3. `update_agent_step_status` + tests/agent.rs:184-215
4. `DirectoryFileRow` dead `.id`/`.source_id` fields (queries.rs:1242-1246) — compiler confirms
5. `tinfoil_backend`; rewrite backend_config.rs `to_summary` tests over `known_provider_presets`
6. venice: `build_venice_chat_body_for_test` (7-ln wrapper), then `\bformat_attestation_url\b` file-scoped to llm/venice.rs + its two tests in tests/venice.rs. KEEP: `build_venice_chat_body`, all other tests in tests/venice.rs, everything in live_venice.rs
7. nvidia: `verify_nvidia_jwt` + `NvidiaAttestationClaims.iss` + only the tests that call `verify_nvidia_jwt`; KEEP `NvidiaAttestationClaims` struct and `fetch_and_verify_nvidia`
8. `fetch_provider_profile` exactly discovery.rs:241-324
9. `#[allow(dead_code)]` sweep, one attribute at a time: remove → `cargo check` → if a NEW dead-code warning appears, delete that item too; keep the attribute only on deliberate future-API (`PpqClient.clock` ppq/client.rs:79, `COPY_FAILED` desktop/iced/src/views/tool_detail.rs:25). Remove module-level `#![allow(dead_code)]` on llm/venice.rs + llm/redpill.rs LAST — the resulting warnings are the real dead-list for those files

## Wave 3 — dependency cut (behavior-preserving)

1. tools.rs `dispatch_fetch_url`: replace scraper extraction with `html2text::from_read(bytes, 80)` **but keep the `MAX_CHARS = 8000` truncation** (apply after conversion); on `from_read` Err fall back to `String::from_utf8_lossy` + same truncation. Accepted deltas **[grok]**: script/style content dropped (current scraper `.text()` leaks it — improvement), output is 80-col wrapped markdown-ish text (LLM-fine, matches rag/mod.rs:150)
2. Remove `scraper` from rust/Cargo.toml
3. Verify: `cargo tree -i scraper` → "package not found"; `cargo test` agent/tools/streaming green **[grok]** (rev1's `cargo tree -p scraper` always fails — wrong flag)

## Verification gate (every wave)

- `cargo check && cargo test` on host
- Word-boundary grep for each deleted symbol → 0 hits (mind `format_attestation_url`/`format_redpill_attestation_url` prefix overlap)
- Final: `git worktree list` = main only; `du -sh` before/after

## Expected net

~250-300 lines, −1 dep tree (scraper → html5ever/selectors/cssparser), ~2.25 GB disk, 6 worktrees pruned. (Rev1's ~480 lines included the live batch helper and overstated venice wrappers **[grok]**.)

## Review log

- grok CLI (headless, read-only): 8 findings — 7 accepted (line ranges, wrong struct, grep trap, blanket-allow risk, truncation loss, wrong verify flag, test rewrites), 1 rejected (claimed tracked files in artifacts/designer-screenshots; `git ls-files artifacts/` is empty in this clone).
