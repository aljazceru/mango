# PPQ sanitized fixtures

These files freeze the PPQ wire contract for managed account orchestration
(plan Wave 0). `scripts/check_ppq_gate.sh` requires every fixture listed in
`docs/integrations/ppq-orchestration-approval.md` Section 4 to exist here
before managed PPQ can ship.

## Capture rules

1. Capture from PPQ directly or from a **throwaway zero-balance account**.
   Never capture fixtures from an account holding real funds.
2. Replace every secret with an obviously fake value of the same shape:
   - API keys -> `sk-SANITIZED...` or PPQ-agreed dummy format
   - `credit_id` -> the all-zero UUID or an explicitly fake UUID
   - BOLT11 -> a regtest/signet invoice or a truncated fake; keep field
     names and structure intact, since structure is the contract
3. Every fixture is a single JSON object with top-level `"sanitized": true`
   plus `endpoint`, `captured_at` (RFC3339), and `request` / `response`
   members (response including observed status code and headers that
   matter, e.g. `Retry-After` on 429/503 if captured).
4. No Authorization headers, no `x-credit-id` values, no real invoice
   strings, nothing PPQ marks confidential.
5. Record unknown/extra response fields as-is — unknown fields are signal,
   not noise.
