# PPQ Orchestration Approval and Contract Evidence

status: pending
updated: 2026-08-30
plan: .planning/PPQ_ORCHESTRATED_ACCOUNT_ANDROID_PLAN.md

This artifact is the Wave 0 release gate record for managed PPQ account
orchestration. Waves 1-6 of the plan must not begin, and no distributable
build may set `MANAGED_PPQ_ENABLED=true`, until this file declares
`status: approved` and every fixture listed in the inventory below is
present in `docs/integrations/ppq-fixtures/` and passes
`scripts/check_ppq_gate.sh`.

Do not paste confidential correspondence verbatim into this file.
Summarize the confirmation and link/attach the sanitized evidence.

## 1. PPQ confirmations (release-blocking)

Source of truth: plan Section 2, "Release-blocking PPQ confirmations".

| # | Confirmation required from PPQ | Status | Evidence / notes |
|---|---|---|---|
| 1 | Third-party apps may create anonymous PPQ accounts on behalf of end users (`POST /accounts/create`) | pending | — |
| 2 | Expected rate limits and abuse controls for `POST /accounts/create` | pending | — |
| 3 | Stable request/response schemas for: account creation, Lightning invoice creation, invoice status, balance, payment methods, key management | pending | Public docs document endpoints but not these response schemas (see Section 3 below) |
| 4 | Whether PPQ wants a client identifier / user-agent such as `Mango Android/<version>` | pending | — |
| 5 | Intended recovery policy when a user has `credit_id` but the backed-up API key is revoked | pending | — |
| 6 | Required product wording, attribution, terms links, support routing | pending | — |
| 7 | Whether prepaid balances expire / are refundable / transferable (Mango must not imply any of these without confirmation) | pending | — |

## 2. Recovery and status semantics (to be agreed with PPQ)

Fill in before flipping `status` to approved. These feed Rust wire types;
do not guess.

### 2.1 Invoice status enum (`GET /topup/status/{invoice_id}`)

Complete list of possible `status` values, exact spelling, and which are
terminal:

| Status value | Terminal? | Meaning | Fixture |
|---|---|---|---|
| _(pending PPQ)_ | — | — | topup_status_*.json |

Additional unknown fields observed in real responses must be recorded in
the fixtures; the Rust client treats unknown statuses conservatively
(unknown-after-create / reconcile-by-balance).

### 2.2 Restored device key

- Agreed key `name` for a device key created during recovery (must fit the
  1-25 char uniqueness constraint): _(pending PPQ)_
- Whether Mango should set `usage_limit_usd` / `expire_at`: _(pending PPQ)_

### 2.3 Revoked-key recovery flow

Steps PPQ agrees to support when `credit_id` is valid but the stored API
key is revoked: _(pending PPQ)_

## 3. Documented contract snapshot (public docs, 2026-08-30)

Captured from <https://ppq.ai/api-docs> on 2026-08-30. This is the
starting evidence only; it is NOT the frozen contract. Response schemas
for the orchestration endpoints are not published and must come from the
fixtures in Section 4.

| Purpose | Endpoint | Auth | Documented? | Response schema known? |
|---|---|---|---|---|
| Create account | `POST /accounts/create` | none | yes | **no — fixture required** |
| Payment methods + limits | `GET /topup/payment-methods` | none | yes | **no — fixture required** |
| Balance | `POST /credits/balance` | Bearer api key / `api-key` header / `credit_id` body | yes | **no — fixture required** |
| Lightning invoice | `POST /topup/create/btc-lightning` | Bearer api key | yes | **no — fixture required** |
| Invoice status | `GET /topup/status/{invoice_id}` | Bearer api key | yes | **no — fixture + status enum required** |
| List keys | `GET /keys` | `x-credit-id` | yes | fields documented; fixture for shape |
| Create key | `POST /keys` | `x-credit-id` | yes | fields documented; fixture for shape |

Documented `btc-lightning` constraints (Mango still fetches live values):
currencies USD/BTC/SATS; SATS limits 100-1,000,000; expiry 15 minutes;
"5% Lightning fee bonus".

Out of scope per plan: NWC auto-topup endpoints, 402/L402 content
endpoints (never used for chat), non-Lightning payment methods.

## 4. Sanitized fixture inventory

Directory: `docs/integrations/ppq-fixtures/`. Capture rules are in that
directory's README. Every file must be non-empty JSON with top-level
`"sanitized": true`; `scripts/check_ppq_gate.sh` enforces presence and the
sanitization marker. `expected` = required for `status: approved`.

| Fixture file | Endpoint | Notes | Present |
|---|---|---|---|
| accounts_create.json | `POST /accounts/create` | request + response; required | no |
| topup_payment_methods.json | `GET /topup/payment-methods` | full method list incl. limits | no |
| topup_create_btc_lightning.json | `POST /topup/create/btc-lightning` | SATS request + response incl. BOLT11 field name | no |
| topup_status_pending.json | `GET /topup/status/{id}` | non-terminal status | no |
| topup_status_paid.json | `GET /topup/status/{id}` | settled status + credited amount fields | no |
| topup_status_expired.json | `GET /topup/status/{id}` | terminal expired status | no |
| topup_status_error.json | `GET /topup/status/{id}` | optional; any other documented status value | no |
| credits_balance.json | `POST /credits/balance` | Bearer-auth variant; decimal-safe fields | no |
| keys_list.json | `GET /keys` | `show_key=false` variant | no |
| keys_create.json | `POST /keys` | full-key-once response | no |

## 5. Sign-off

- PPQ written approval reference: _(pending)_
- Date approved: _(pending)_
- Approved by (Mango): _(pending)_
