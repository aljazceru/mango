---
status: resolved
trigger: "tinfoil-onboarding-connect-error"
created: 2026-03-26T00:00:00Z
updated: 2026-03-26T00:04:00Z
---

## Current Focus

hypothesis: CONFIRMED via logcat. DNS resolution fails on Android because dcap-qvl forces the hickory-dns feature on reqwest, which tries to read /etc/resolv.conf (which does not exist on Android). All reqwest HTTP calls fail with ConnectError("dns error", HickoryDnsSystemConfError). Fix: use reqwest::ClientBuilder::hickory_dns(false) on all client instantiations.
test: Confirmed via logcat error: ConnectError("dns error", HickoryDnsSystemConfError(ResolveError { kind: Proto(ProtoError { kind: Io(Os { code: 2, kind: NotFound, message: "No such file or directory" }) }) }))
expecting: Disabling hickory-dns on all reqwest clients makes them fall back to getaddrinfo (Android system DNS), which works correctly.
next_action: Apply fix to spawn_health_check (lib.rs), attestation/task.rs, attestation/nvidia.rs, and llm/streaming.rs.

## Symptoms

expected: Tinfoil provider onboarding succeeds — preset URL is used, user enters their API key, connection test passes
actual: "could not connect. Check your API key and try again" during onboarding
errors: "could not connect. Check your API key and try again" (UI error message)
reproduction: Open app on Android, go through onboarding, select Tinfoil provider, enter API key
started: New issue — app just installed for the first time
platform: Android (phone connected via ADB)

## Eliminated

- hypothesis: INTERNET permission was the sole root cause
  evidence: User rebuilt and reinstalled APK with INTERNET permission added but still sees "Could not connect. Check your API key and try again." — permission fix was necessary but not sufficient.
  timestamp: 2026-03-26T00:03:00Z

- hypothesis: Wrong Tinfoil base URL in Android onboarding preset (api.tinfoil.sh instead of inference.tinfoil.sh)
  evidence: known_provider_presets() in rust/src/llm/backend.rs uses "https://inference.tinfoil.sh/v1/" — correct. MIGRATION_V1 seeds the same URL. No wrong URL in Android code.
  timestamp: 2026-03-26T00:01:00Z

- hypothesis: Health check URL construction is wrong
  evidence: spawn_health_check builds "{base_url.trim_end_matches('/')}/models" = "https://inference.tinfoil.sh/v1/models" — correct and returns HTTP 200 from the internet.
  timestamp: 2026-03-26T00:01:00Z

- hypothesis: TLS certificate issue with rustls-tls on Android
  evidence: inference.tinfoil.sh uses Google Trust Services WR1 certificate, which is in webpki-roots. Ping from device to inference.tinfoil.sh succeeds (253ms latency).
  timestamp: 2026-03-26T00:01:00Z

- hypothesis: Tinfoil API rejects invalid keys
  evidence: curl test with "Bearer invalid_key_here" returns HTTP 200 with full model list — Tinfoil doesn't gate /v1/models behind auth.
  timestamp: 2026-03-26T00:01:00Z

## Evidence

- timestamp: 2026-03-26T00:03:30Z
  checked: logcat output from device (PID 8417, tag confidential_app) after fresh app launch
  found: "[attestation] failed backend=tinfoil error=Network error: Failed to fetch attestation from https://inference.tinfoil.sh/.well-known/tinfoil-attestation: error sending request for url ... ConnectError("dns error", HickoryDnsSystemConfError(ResolveError { kind: Proto(ProtoError { kind: Io(Os { code: 2, kind: NotFound, message: "No such file or directory" }) }) }))"
  implication: DNS resolution fails entirely. hickory-dns (reqwest's async DNS resolver) tries to read /etc/resolv.conf, which does not exist on Android. This causes ALL reqwest HTTP calls to fail immediately.

- timestamp: 2026-03-26T00:03:30Z
  checked: dcap-qvl Cargo.toml in registry (/home/lio/.cargo/registry/src/.../dcap-qvl-0.3.x/Cargo.toml)
  found: dcap-qvl explicitly enables hickory-dns on its reqwest dependency: features = ["rustls-tls", "blocking", "hickory-dns"]
  implication: Cargo feature unification propagates hickory-dns to ALL uses of reqwest in the build, including health checks and streaming. This is forced on by a transitive dependency, not by our code.

- timestamp: 2026-03-26T00:03:30Z
  checked: reqwest 0.12.28 ClientBuilder API
  found: ClientBuilder::hickory_dns(false) is available and disables the async hickory resolver, falling back to getaddrinfo (system DNS)
  implication: Passing .hickory_dns(false) on every reqwest ClientBuilder call will fix DNS resolution on Android without affecting other platforms.

- timestamp: 2026-03-26T00:03:30Z
  checked: async-openai 0.33.1 client.rs in registry
  found: Client::with_http_client(reqwest::Client) method exists - can pass a custom reqwest client with hickory disabled
  implication: The streaming LLM client can also be fixed by building a custom reqwest client and injecting it.

- timestamp: 2026-03-26T00:00:00Z
  checked: .planning/debug/redpill-attestation-tinfoil-auth.md (prior session)
  found: Prior session fixed desktop placeholder URL from api.tinfoil.sh to inference.tinfoil.sh.
  implication: Not relevant here — Android preset URL is already correct.

- timestamp: 2026-03-26T00:01:00Z
  checked: rust/src/llm/backend.rs known_provider_presets(), rust/src/lib.rs spawn_health_check()
  found: Tinfoil preset URL = "https://inference.tinfoil.sh/v1/" (correct). Health check hits "{base_url}/models" with 5s timeout. Success path: resp.status().is_success(). Failure path: non-2xx or Err(e) → triggers "Could not connect" message.
  implication: The only way this fails on Android is if the HTTP request itself fails (network error).

- timestamp: 2026-03-26T00:01:00Z
  checked: android/app/src/main/AndroidManifest.xml
  found: No INTERNET permission. Only has allowBackup, label, theme, and one activity. No <uses-permission> tags at all.
  implication: Android blocks ALL outbound network connections from apps without android.permission.INTERNET. This is a fatal missing permission.

- timestamp: 2026-03-26T00:01:00Z
  checked: android/app/build/intermediates/merged_manifests/debug/processDebugManifest/AndroidManifest.xml
  found: Merged manifest contains WAKE_LOCK, ACCESS_NETWORK_STATE, RECEIVE_BOOT_COMPLETED, FOREGROUND_SERVICE (all from WorkManager/job deps) — but NO android.permission.INTERNET.
  implication: Confirms INTERNET permission is absent from the entire build. Every network call in the app fails with a connection error on Android.

- timestamp: 2026-03-26T00:01:00Z
  checked: Device network reachability via ADB
  found: `adb shell ping -c 1 inference.tinfoil.sh` succeeds with 253ms RTT. Device is online.
  implication: The device has network access. The failure is Android's permission enforcement blocking the app's socket calls.

## Resolution

root_cause: dcap-qvl (an attestation dependency) forces the hickory-dns feature on reqwest. hickory-dns tries to read /etc/resolv.conf for DNS configuration, but Android does not have this file (Android uses a different DNS system via bionic libc). This causes ALL reqwest HTTP calls to fail immediately with ConnectError("dns error", HickoryDnsSystemConfError). The INTERNET permission fix was necessary but not sufficient -- hickory-dns was the actual blocker preventing any network connectivity.
fix: Added .hickory_dns(false) to all reqwest::ClientBuilder calls in the codebase: rust/src/lib.rs spawn_health_check, rust/src/attestation/task.rs (tinfoil fetch and VCEK fetch), rust/src/attestation/nvidia.rs, and rust/src/llm/streaming.rs (with custom http client injected via async_openai::Client::with_http_client). This forces reqwest to fall back to getaddrinfo (system DNS) which works correctly on Android.
verification: Built and installed new APK. Logcat shows attestation completing successfully: DNS resolves, TLS handshake succeeds to inference.tinfoil.sh and kdsintf.amd.com, SNP verification PASSED.
files_changed:
  - android/app/src/main/AndroidManifest.xml
  - rust/src/lib.rs
  - rust/src/attestation/task.rs
  - rust/src/attestation/nvidia.rs
  - rust/src/llm/streaming.rs

## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** .no_hickory_dns() on all clients (net/tls.rs:129,143 + 11 more sites)
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
