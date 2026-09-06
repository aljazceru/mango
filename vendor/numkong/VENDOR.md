# Vendored crate: `numkong` 7.8.1

This directory is a verbatim copy of the crates.io package `numkong` 7.8.1
plus the local changes listed below. It is activated through
`[patch.crates-io]` in the workspace root `Cargo.toml`, replacing the
registry copy for all dependents (`usearch` requires `numkong >=7.5.0, <8`;
the vendored version 7.8.1 satisfies that requirement, so no other
dependency versions change).

## Provenance

- Crate: `numkong` 7.8.1 — <https://crates.io/crates/numkong/7.8.1>
- License: Apache-2.0 (see `LICENSE`, added below, and `Cargo.toml`)
- Upstream repository: <https://github.com/ashvardanian/NumKong>
- Published from upstream git commit `928a2143ac3370547f833eeb38dbd736f7d54d5e`
  (see `.cargo_vcs_info.json`, retained verbatim from the registry copy)
- crates.io `.crate` sha256:
  `32abf4fad33e67bc129973bf953f076803bd33f393ac17589cb05afb5b3b8420` —
  verified 2026-09-06 with
  `sha256sum ~/.cargo/registry/cache/index.crates.io-*/numkong-7.8.1.crate`
  against the `checksum` recorded in `Cargo.lock` before the patch was applied.

## Package contents vs. published crate

The published `.crate` ships only the files matched by its `include` list
(`rust/*.rs`, `c/*.c`, `c/*.h`, `include/**/*.h`, `probes/*.c`, `build.rs`)
plus `Cargo.toml`, `Cargo.toml.orig`, and `Cargo.lock`. The registry
extraction additionally records `.cargo-ok` and `.cargo_vcs_info.json`;
both were copied verbatim. No `.cargo-checksum.json` exists in the extracted
registry source (that file is produced by `cargo vendor`, not present in
`registry/src/`), so none was fabricated.

## Local changes

1. `LICENSE` — verbatim copy of upstream `LICENSE` (Apache-2.0) fetched from
   `https://raw.githubusercontent.com/ashvardanian/NumKong/928a2143ac3370547f833eeb38dbd736f7d54d5e/LICENSE`.
   The published `.crate` does not include a license file (its `include`
   list excludes it); upstream has no `NOTICE`/`NOTICE.md` at that commit.
2. `include/numkong/capabilities.h` — Android guard, see below.
3. `VENDOR.md` — this file.

A recursive diff against the extracted registry copy
(`~/.cargo/registry/src/index.crates.io-*/numkong-7.8.1`) shows **no other
source differences** — the only code change is the `capabilities.h` hunk
below.

## `include/numkong/capabilities.h` patch

Upstream 7.8.1 and 7.8.2 (and `main`, checked 2026-09-06) unconditionally
forward-declare `extern "C" long syscall(long, ...) noexcept;` when
`NK_DEFINED_LINUX_ && (NK_TARGET_X8664_ || NK_TARGET_RISCV64_)`. That matches
glibc — whose `<unistd.h>` declares `syscall` with `__THROW` — but Android's
Bionic declares `long syscall(long __number, ...);` without an exception
specification, so compiling any C++ TU that includes this header for
`x86_64-linux-android` fails with:

    error: exception specification in declaration does not match previous declaration

The patch adds an `#if defined(__ANDROID__)` branch that reuses Bionic's
declaration from `<unistd.h>` (already included earlier in the file under
`NK_DEFINED_LINUX_`) instead of redeclaring. The glibc and non-Android paths
are byte-for-byte unchanged.

## Removal conditions

Drop the `[patch.crates-io]` entry and this directory once a released
`numkong` version guards the `syscall` forward-declaration for
`__ANDROID__`/Bionic (or includes `<unistd.h>` and relies on the platform
declaration). Until then the guard is required for `x86_64` Android builds.

## Regression check

`scripts/check_numkong_android_headers.sh` (also `just check-numkong-headers`)
compiles a TU that includes `<unistd.h>` before `numkong/capabilities.h`
with the real NDK toolchains (arm64 + x86_64) and the host C++ compiler. All
three checks are mandatory; the script fails on the unpatched declaration
and on missing tools/headers.
