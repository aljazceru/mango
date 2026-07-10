# Phase 33 — MRSEAM Reconciliation

**Source:** `rust/tests/fixtures/venice/attestation-sample.json` (golden capture, hex-decoded `intel_quote` parsed via `dcap_qvl::quote::Quote::parse`)
**Extracted:** 2026-04-26
**Method:** One-shot temporary `#[ignore]` test `mrseam_dump_once` (since deleted) printed `Report::TD10` fields.

## Live MRSEAM (from golden capture)

```
7bf063280e94fb051f5dd7b1fc59ce9aac42bb961df8d44b709c9b0ff87a7b4df648657ba6d1189589feab1d5a3c9a9d
```

48 bytes, hex-encoded.

## Sibling fields captured (for Plan 02)

| Field | Hex value |
|-------|-----------|
| `td_attributes` (8B) | `0000001000000000` |
| `tee_tcb_svn` (16B) | `0b010300000000000000000000000000` |
| Quote header `version` | `4` (TDX 1.0 / DCAP v4) |
| Report variant | `TD10` |

## Comparison vs `TdxPolicy::default()::accepted_mr_seams`

`rust/src/attestation/policy.rs` line 27-32:

```rust
accepted_mr_seams: vec![
    "476a2997...c40e26afac75f12df3425b03eb59ea7c".to_string(),  // index 0
    "7bf06328...f648657ba6d1189589feab1d5a3c9a9d".to_string(),  // index 1  <-- MATCH
    "685f891e...85f1f6f3571539a91e104a1c96d75e04".to_string(),  // index 2
    "49b66faa...850fa20e3b1aa9a874d77a65380ee7e6".to_string(),  // index 3
],
```

✅ **Present** — Venice MRSEAM matches **index 1** of the existing default seed list (`7bf063280e94fb051f5dd7b1fc59ce9aac42bb961df8d44b709c9b0ff87a7b4df648657ba6d1189589feab1d5a3c9a9d`).

## Action for Plan 02

**No policy change required.** `TdxPolicy::default()` already accepts the Venice MRSEAM. Plan 02 must only:

1. Reuse the existing `TdxPolicy` (loaded from settings as today) when verifying Venice quotes.
2. Confirm `td_attributes[0] & 0x01 == 0` enforcement (debug-bit reject — VEN-06): the captured `td_attributes = 0x00 00 00 10 00 00 00 00`, so byte 0 = 0x00 — debug bit clear, production TDX. The Venice REPORTDATA decoder will pin this assertion.
3. Compare `tee_tcb_svn = 0b010300...` against `minimum_tee_tcb_svn = 03010200...` — Plan 02 must use the existing `tee_tcb_svn` comparator (the bytewise minimum check that the rest of the codebase uses); Venice's `0b 01 03 ...` is **above** the minimum `03 01 02 ...` in the high byte, so the live capture passes the default policy.

## Sanity sentinel

A comment in `rust/src/tests/common/venice_fixtures.rs` records the live MRSEAM hex so that if the golden capture is rotated, the next maintainer rediscovers and re-reconciles instead of silently extending the policy.
