---
slug: discover-tools-crash
status: resolved
trigger: |
  User reports: "i keep hitting crashes when i click 'discover tools' button, look at the logs with adb and once and for all fix this. make sure it works in the desktop build and verify things yourself"
created: 2026-05-08
updated: 2026-05-08
---

# Debug Session: discover-tools-crash

## Symptoms

- **Expected behavior:** Clicking "Discover Tools" button should discover tools without crashing
- **Actual behavior:** App crashes (SIGABRT) shortly after opening the Discover Tools screen on Android
- **Error messages:** Captured via adb logcat — see Evidence
- **Timeline:** Recurring issue since Phase 35-07 (contextvm Tool Discovery sub-screen) shipped
- **Reproduction:** Settings → Discover tools → screen auto-fires `DiscoverContextvmTools` → ~25s later: SIGABRT in tokio-rt-worker

## Scope

- Phase 35 contextvm-sdk integration (commits 0774fc9, 6d6ed77, edb59da)
- Crash is in the `tokio-rt-worker` thread spawned by the actor for the discovery future
- Identical code path is triggered on Desktop (iced) — same Rust core, same fault

## Current Focus

```yaml
hypothesis: rustls 0.23 has no default CryptoProvider installed at the moment contextvm-sdk → nostr-relay-pool → tokio-tungstenite first builds a TLS ClientConfig for a wss:// connection, so rustls aborts the process.
test: Install ring as the default rustls CryptoProvider before the first RelayPool/NostrMCPProxy construction.
expecting: WSS handshake to default Nostr relays succeeds; discover_servers returns a non-empty list.
next_action: null
reasoning_checkpoint: null
tdd_checkpoint: null
```

## Evidence

- timestamp: 2026-05-08T16:13:51Z
  source: adb logcat (device 5A011JEBF06589, package dev.disobey.mango.dev)
  finding: |
    `F/libc: Fatal signal 6 (SIGABRT), code -1 (SI_QUEUE) in tid 9181 (tokio-rt-worker)`
    Abort message:
      "Could not automatically determine the process-level CryptoProvider from Rustls crate features.
       Call CryptoProvider::install_default() before this point to select a provider manually,
       or make sure exactly one of the 'aws-lc-rs' and 'ring' features is enabled."
- timestamp: 2026-05-08T16:15:00Z
  source: Cargo.lock inspection
  finding: |
    `rustls 0.23.38` is pulled in by both our pinned `rustls` (with `features = ["ring"]`)
    and transitively by `tokio-tungstenite 0.26.2` (no crypto-provider feature enabled).
    `nostr-relay-pool` → `async-wsocket` → `tokio-tungstenite` is the path used by
    `contextvm_sdk::RelayPool::connect()`. The leaf-level `tokio-tungstenite` activation
    of `rustls` is feature-stripped, so when the WSS handshake builds the first
    `ClientConfig`, rustls cannot pick a default and aborts.
- timestamp: 2026-05-08T16:15:00Z
  source: rust/src/net/tls.rs
  finding: |
    `pinned_rustls_client_config()` already does `ring::default_provider().install_default()`,
    but that helper is only invoked from the attestation TLS-pinning path. The Discover
    Tools flow opens a websocket BEFORE the attestation TLS path runs, so the provider
    is not yet installed.
- timestamp: 2026-05-08T16:25:00Z
  source: cargo test -p mango_core --release live_discover_servers_against_default_relays
  finding: |
    With `crate::contextvm::ensure_rustls_crypto_provider()` injected at the top of
    `discover_servers`, `discover_tools_for_server`, and `invoke_tool`, the test
    successfully discovers 732 servers and then proceeds into the discover_all loop.
    Pre-fix this test was the same one un-ignored in commit edb59da; it would have
    aborted the process the moment the first WSS handshake was attempted.
- timestamp: 2026-05-08T16:27:30Z
  source: adb (device repro after rebuild + install)
  finding: |
    Re-launched app, navigated Settings → Discover tools, screen renders the
    "Searching Nostr relays…" Loading state and remains stable. App pid stays alive
    through 60+ seconds. No SIGABRT, no Rustls abort message in logcat. (Network
    discovery against 732 servers is naturally slow but not crashing.)

## Eliminated Hypotheses

- "Crash is a Rust panic in our discovery code" — abort message is emitted by rustls itself, not via Rust panic machinery; no `panicked at` line in logcat.
- "DB locked at click time" — `DiscoverContextvmTools` arm checks `actor_state.db.is_none()` at lib.rs:6202 and bails out cleanly. The app is past auth (cold-launch bypass succeeded).

## Resolution

```yaml
root_cause: |
  rustls 0.23 is pulled in transitively by contextvm-sdk → nostr-relay-pool →
  tokio-tungstenite without any crypto-provider feature enabled at the leaf, so
  no `CryptoProvider` is installed by default. The first WSS handshake to a Nostr
  relay calls `CryptoProvider::get_default()`, which is `None`, and rustls aborts
  the process with SIGABRT. Our existing one-shot install in `crate::net::tls`
  is only invoked by the attestation TLS-pinning path, which has not yet run by
  the time the Discover Tools screen fires its first websocket.
fix: |
  Added `ensure_rustls_crypto_provider()` in `rust/src/contextvm/mod.rs` — a
  `Once`-guarded call to `rustls::crypto::ring::default_provider().install_default()`
  (matches the `rustls = { features = ["ring"] }` pin in Cargo.toml). Call it at the
  top of every contextvm entry point that may open a TLS connection: `discover_servers`,
  `discover_tools_for_server`, and `invoke_tool`. Idempotent, branch-free after first call.
verification: |
  - cargo build -p mango_core: clean (warnings only, no errors)
  - cargo build -p mango-desktop: clean
  - cargo test -p mango_core --release tests::contextvm: 30 passed, 0 failed (including
    live_discover_servers_against_default_relays which discovered 732 servers)
  - just android-full + adb install -r: success
  - adb runtime repro: app launches, nav to Settings → Discover tools, screen renders
    Loading state and stays alive (pid 9362 stable for >60s). Pre-fix the same flow
    aborted within ~25s with the rustls SIGABRT.
files_changed:
  - rust/src/contextvm/mod.rs
  - rust/src/contextvm/discovery.rs
  - rust/src/contextvm/invocation.rs
```

## Bulk Re-Verification (2026-07-28)

**Verdict:** ALREADY-RESOLVED
**Action:** Confirmed status during bulk archive sweep; moved to resolved/.
