#!/usr/bin/env python3
"""Decode REPORTDATA from each Redpill response shape against the submitted nonce.

Reads the captures alongside this script and prints layout assertions. Treat the
output as a golden trace — the byte slices below double as fixtures for the
Rust REPORTDATA-layout decoders.
"""
import json, hashlib, base64, pathlib, sys

HERE = pathlib.Path(__file__).parent
nonce_lines = (HERE / "nonce.txt").read_text().strip().splitlines()
nonces = dict(line.split("=", 1) for line in nonce_lines)


def slice_quote_reportdata(quote_str: str) -> bytes:
    s = quote_str.strip()
    # detect base64 vs hex
    if any(c not in "0123456789abcdefABCDEF" for c in s):
        raw = base64.b64decode(s)
    else:
        raw = bytes.fromhex(s)
    # TDX v4 REPORTDATA at offset 568..632 (header 48B + body offset 520..584)
    return raw[568:632]


def show_shape_a():
    print("=== Shape A — Phala-pure (flat) ===")
    nonce = nonces["phala_nonce"]
    d = json.loads((HERE / "attestation-phala-pure-raw.json").read_text())
    rd = slice_quote_reportdata(d["intel_quote"])
    addr = bytes.fromhex(d["signing_address"][2:])
    assert rd[:20] == addr, "addr binding"
    assert rd[20:32] == b"\x00" * 12, "12B zero pad"
    assert rd[32:64].hex() == nonce, "nonce binding"
    print(f"  signing_addr (20B) == reportData[0..20]    OK  {addr.hex()}")
    print(f"  zero pad     (12B) == reportData[20..32]   OK")
    print(f"  client nonce (32B) == reportData[32..64]   OK  {nonce}")


def show_shape_b():
    print("=== Shape B — Phala-orchestrated (gateway + model + composer) ===")
    nonce = nonces["nonce"]
    d = json.loads((HERE / "attestation-phala-raw.json").read_text())

    # gateway
    gw = d["gateway_attestation"]
    rd = bytes.fromhex(gw["report_data"])
    assert rd[:32].hex() == gw["signing_address"], "ed25519 pubkey binding"
    assert rd[32:].hex() == nonce, "gateway nonce binding"
    print(f"  GATEWAY  (ed25519): pubkey [0..32] + nonce [32..64]    OK")

    # model
    m = d["model_attestations"][0]
    rd = slice_quote_reportdata(m["intel_quote"])
    assert rd[:20].hex() == m["signing_address"][2:], "ecdsa addr binding"
    assert rd[20:32] == b"\x00" * 12, "12B pad"
    assert rd[32:64].hex() == nonce, "model nonce binding"
    print(f"  MODEL    (ecdsa  ): addr [0..20] + pad [20..32] + nonce [32..64]    OK")

    # compose-manager
    cm = m["compose_manager_attestation"]
    rd = bytes.fromhex(cm["report_data"])
    assert rd[:32].hex() == cm["actions_hash"], "actions-hash binding"
    assert rd[32:].hex() == nonce, "compose-manager nonce binding"
    print(f"  COMPOSER       : actions_hash [0..32] + nonce [32..64]    OK")


def show_shape_c():
    print("=== Shape C — Chutes ===")
    client_nonce = nonces["chutes_nonce"]
    d = json.loads((HERE / "attestation-chutes-raw.json").read_text())
    print(f"  client-submitted nonce (ignored by Chutes): {client_nonce}")
    print(f"  echoed top-level nonce (Chutes-baked)     : {d['nonce']}")
    print(f"  CHUTES uses an enclave-baked nonce; the client nonce is NOT bound.")
    print()
    for i, a in enumerate(d["all_attestations"]):
        rd = slice_quote_reportdata(a["intel_quote"])
        # Reference verifier formula (chutes.ts line 77):
        #   SHA256(nonce_string + e2e_pubkey_string) == reportData[0..32]
        # Note: STRING concatenation of the as-emitted ASCII forms.
        expected = hashlib.sha256((a["nonce"] + a["e2e_pubkey"]).encode()).digest()
        ok = rd[:32] == expected
        # td_attributes at body offset 120..128 — bit 0 of byte 0 = debug mode
        raw = base64.b64decode(a["intel_quote"])
        td_attr = raw[48 + 120 : 48 + 128]
        debug = bool(td_attr[0] & 1)
        print(f"  inst[{i}] {a['instance_id'][:8]}  binding [0..32]={ok}  debug_mode={debug}")
    print()
    print("  Freshness implication: Chutes attestations are valid for the lifetime")
    print("  of the enclave instance, not per-request. Client treats Chutes binding")
    print("  as 'this e2e_pubkey was bound at enclave boot under this baked nonce' —")
    print("  freshness is bounded by enclave lifetime + e2e_pubkey rotation, not by")
    print("  a per-request challenge.")


def show_tinfoil_502():
    print("=== Shape D — Tinfoil-via-Redpill (currently broken at relay) ===")
    body = (HERE / "attestation-tinfoil-raw.json").read_text()
    print(f"  HTTP 502 body: {body.strip()}")


if __name__ == "__main__":
    show_shape_a()
    print()
    show_shape_b()
    print()
    show_shape_c()
    print()
    show_tinfoil_502()
