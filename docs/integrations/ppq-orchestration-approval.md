# PPQ Orchestration Approval and Contract Evidence

status: approved
updated: 2026-09-03 (all fixtures captured; paid state settled live via real Lightning payment)
plan: .planning/PPQ_ORCHESTRATED_ACCOUNT_ANDROID_PLAN.md

This artifact is the Wave 0 release gate record for managed PPQ account
orchestration. Waves 1-6 of the plan must not begin, and no distributable
build may set `MANAGED_PPQ_ENABLED=true`, until this file declares
`status: approved` and every fixture listed in the inventory below is
present in `docs/integrations/ppq-fixtures/` and passes
`scripts/check_ppq_gate.sh`.

Do not paste confidential correspondence verbatim into this file.
Summarize the confirmation and link/attach the sanitized evidence.

Current position (2026-09-03): PPQ approval received (user-attested in
session; written reference still to be filed in §5). All contract
evidence captured live against PPQ production using a throwaway account,
including a REAL 100-sat Lightning payment (paid via a local Spark wallet,
payment hash 01a066cb-6335-7833-b5ae-9f61b716e93c, 3-sat fee). The paid
state settled immediately: status `Settled`, `amount_paid` 0.000001 BTC,
`amount_due` 0. All 10 gate fixtures present and compile-verified by
`rust/src/tests/ppq.rs`. Waves 1-6 are unblocked.

## 1. PPQ confirmations (release-blocking)

Source of truth: plan Section 2, "Release-blocking PPQ confirmations".

| # | Confirmation required from PPQ | Status | Evidence / notes |
|---|---|---|---|
| 1 | Third-party apps may create anonymous PPQ accounts on behalf of end users (`POST /accounts/create`) | approved (user-attested 2026-08-31) | Live capture: unauthenticated POST returns 201 `{credit_id, api_key, balance}` (fixture `accounts_create.json`) |
| 2 | Expected rate limits and abuse controls for `POST /accounts/create` | assumed standard; no specifics filed | No rate-limit headers observed on 201 responses. Mango applies local debounce + single-flight (plan §6.8). File specifics when received. |
| 3 | Stable request/response schemas for account creation, invoice creation/status, balance, payment methods, key management | captured (see §3/§4) | All schemas frozen in fixtures incl. `Settled` paid state; compile-verified by `rust/src/tests/ppq.rs`. |
| 4 | Whether PPQ wants a client identifier / user-agent such as `Mango Android/<version>` | no requirement filed; Mango sends `Mango/<version>` UA | Harmless default; adjust if PPQ requests a specific format. |
| 5 | Recovery policy when a user has `credit_id` but the backed-up API key is revoked | **verified live 2026-08-31** | Sequence executed on throwaway account: revoke key → Bearer balance returns 401 `{"error":"Invalid API key","message":"API key not found or has been revoked"}` → `POST /keys` with `x-credit-id` mints new key (201) → new key authenticates (200). Fixture `errors_401_402.json` + `keys_create.json`. |
| 6 | Required product wording, attribution, terms links, support routing | copy per plan §3.2 (labels say "PPQ balance"/"PPQ credit", never Mango custody); final wording in Wave 6 Zapstore pass | — |
| 7 | Whether prepaid balances expire / are refundable / transferable | **denomination confirmed: USD credit** (user-confirmed from PPQ correspondence 2026-09-03; consistent with docs `price_in_usd`, `usage_limit_usd`, USD top-up ranges, and observed sat→credit conversion). Expiry/refund/transferability STILL unconfirmed | Mango UI labels the balance "PPQ balance (USD)" and makes NO expiry/refund/transfer claims anywhere. Do not add any until PPQ states terms. |

## 2. Recovery and status semantics

### 2.1 Invoice status enum (`GET /topup/status/{invoice_id}`)

Observed live (fixture-frozen):

| Status value | Terminal? | Meaning | Fixture |
|---|---|---|---|
| `New` | no | invoice created, awaiting payment; `amount_paid: 0` | topup_status_pending.json |
| `Expired` | **yes** | 15-minute lifetime elapsed unpaid; `amount_paid` stays 0 | topup_status_expired.json |
| `Settled` | **yes** | paid & credited; `amount_paid` = BTC decimal, `amount_due` = 0; visible immediately after payment | topup_status_paid.json |

Post-expiry GC: ~3 days after expiry the status endpoint returns 404
`{"error":"Invoice not found"}` (observed 2026-09-03 on the Aug-31
invoices). Long-lived reconciliation must therefore go through balance,
not invoice status — exactly the conservative-unknown handling the client
enforces.

Other response fields (all observed): `invoice_id`, `amount`, `currency`,
`created_at`/`expires_at` (UNIX epoch **ints**, 900s apart), `payment_method`
("Bitcoin Lightning"), `amount_paid` (number), `amount_due` (BTC decimal,
e.g. 0.000001 for 100 sats), `lightning_invoice` (BOLT11). Unknown status
values MUST map to conservative unknown handling (reconcile via balance) —
enforced by `InvoiceStatusValue::Unknown` being never-terminal in code.

Unknown-invoice error: 404 `{"error":"Invoice not found"}` (fixture
`topup_status_error.json`).

### 2.2 Restored device key

- Key name convention: `mango-device-<n>` (docs: 1-25 chars, unique per
  account). Captured example used `mango-device-test`.
- No `usage_limit_usd` / `expire_at` on Mango-created device keys
  (observed `null` in create response).

### 2.3 Revoked-key recovery flow

Verified live (see §1 item 5). `credit_id` via `x-credit-id` header can
always mint a fresh device key after revocation; the account-creation API
key itself is independent of the `/keys` registry.

## 3. Documented contract snapshot (public docs + live capture, 2026-08-31)

Public docs: <https://ppq.ai/api-docs>. Live capture base:
`https://api.ppq.ai`, throwaway zero-balance account, all responses
`Content-Type: application/json`.

| Purpose | Endpoint | Auth used by Mango | Success code | Response schema |
|---|---|---|---|---|
| Create account | `POST /accounts/create` | none (no body) | 201 | frozen — `accounts_create.json` |
| Payment methods | `GET /topup/payment-methods` | none | 200 | frozen — `topup_payment_methods.json` |
| Balance | `POST /credits/balance` | `Authorization: Bearer <api_key>` (no body) | 200 | frozen — `credits_balance.json`; live credited balance observed as `0.08155601999999999` (f64 shortest-repr, 17 fraction digits) |
| Lightning invoice | `POST /topup/create/btc-lightning` | Bearer + `{"amount": <int>, "currency": "SATS"}` | 201 | frozen — `topup_create_btc_lightning.json` |
| Invoice status | `GET /topup/status/{invoice_id}` | Bearer | 200 | frozen pending/expire/error; **paid outstanding** |
| List keys | `GET /keys` | `x-credit-id` | 200 | frozen — `keys_list.json` |
| Create key | `POST /keys` | `x-credit-id` + `{"name": <str>}` | 201 | frozen — `keys_create.json` |

Error bodies (fixtures `errors_401_402.json`):
- 401 revoked key: `{"error":"Invalid API key","message":"API key not found or has been revoked"}`
- 402 insufficient balance on `/v1/chat/completions`: `{"error":"Payment Required","message":"Insufficient balance"}` — PPQ-specific 402 mapping for plan §6.10.

Money on the wire: balances/limits/amount_paid/amount_due are bare JSON
**numbers** rendered as plain decimals (no exponents observed from PPQ;
exponent forms are rejected by the client). Balances can carry float64
serialization noise (17 fraction digits observed live) — the client parses
raw text into validated `DecimalText` (fraction cap 18) and never
round-trips money through `f64` (plan §6.3). Display layers round for
rendering only.

Documented `btc-lightning` constraints (live values still fetched at
runtime): currencies USD/BTC/SATS; SATS limits 100-1,000,000; expiry 15
minutes (900s epoch delta observed); "5% Lightning fee bonus".

Out of scope per plan: NWC auto-topup endpoints, 402/L402 content
endpoints (never used for chat), non-Lightning payment methods.

## 4. Sanitized fixture inventory

Directory: `docs/integrations/ppq-fixtures/`. Capture rules are in that
directory's README. Every file must be non-empty JSON with top-level
`"sanitized": true`; `scripts/check_ppq_gate.sh` enforces presence and the
sanitization marker.

| Fixture file | Endpoint | Notes | Present |
|---|---|---|---|
| accounts_create.json | `POST /accounts/create` | request + response; required | **yes** |
| topup_payment_methods.json | `GET /topup/payment-methods` | full method list incl. limits | **yes** |
| topup_create_btc_lightning.json | `POST /topup/create/btc-lightning` | SATS request + response incl. BOLT11 field | **yes** |
| topup_status_pending.json | `GET /topup/status/{id}` | `New` (non-terminal) | **yes** |
| topup_status_paid.json | `GET /topup/status/{id}` | `Settled` (terminal); paid with real 100 sats 2026-09-03 | **yes** |
| topup_status_expired.json | `GET /topup/status/{id}` | `Expired` (terminal) | **yes** |
| topup_status_error.json | `GET /topup/status/{id}` | 404 unknown-invoice shape | **yes** |
| credits_balance.json | `POST /credits/balance` | Bearer-auth variant; live 17-digit float form frozen | **yes** |
| keys_list.json | `GET /keys` | `show_key=false` variant | **yes** |
| keys_create.json | `POST /keys` | full-key-once response | **yes** |
| errors_401_402.json | (error shapes) | extra evidence, not gate-required | **yes** |

## 5. Sign-off

- PPQ written approval reference: user-attested approval received
  2026-08-31 (session) and reconfirmed 2026-09-03, including the balance
  denomination (USD credit). PPQ's written confirmation to be attached
  here verbatim when filed for the archive.
- Date approved: 2026-09-03 (all gate evidence complete)
- Approved by (Mango): lio (product owner, session 2026-09-03) — release
  authorization granted with MANAGED_PPQ_ENABLED defaulting to true
- Gate check: `scripts/check_ppq_gate.sh` passing as of this date
