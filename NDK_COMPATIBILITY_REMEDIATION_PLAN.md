# NumKong / Android NDK compatibility remediation

Date: 2026-09-06. Executor: Devin CLI, model SWE-1.7 (`swe-1-7`), YOLO (`--permission-mode dangerous`). Reviewer: Codex.

## Objective and scope

Make the existing Android native pipeline build mango_core for both arm64-v8a and x86_64 with the installed NDK 28.2.13676358, preserving vector-search behavior and the successful arm64 app build. Resolve the actual C++ platform incompatibility; do not merely omit x86_64, suppress compiler errors, disable optimizations globally, or downgrade the NDK without evidence and explicit justification.

Existing source and documentation changes in the working tree belong to the user and the prior app-remediation task. Preserve them. No commits, pushes, upstream issue/PR creation, deployment, personal device data changes, or unrelated dependency upgrades. Do not edit the global Cargo registry/cache as the permanent fix.

## Confirmed starting evidence

- Cargo.lock resolves usearch 2.26.1 and numkong 7.8.1; rust/Cargo.toml declares usearch with default features.
- NumKong include/numkong/capabilities.h line 138 forward-declares `extern "C" long syscall(long, ...) noexcept;` for Linux x86_64/RISC-V, assuming glibc.
- Android's NDK unistd.h declares `long syscall(long __number, ...);` without that exception specification. Android uses Bionic rather than glibc.
- `just build-android` builds arm64 and x86_64; `just build-android-release` builds arm64. Both currently use Rust release optimization. The arm64 pipeline and Android app have passed.
- SDK: /home/lio/Android/Sdk; NDK: /home/lio/Android/Sdk/ndk/28.2.13676358. cargo-ndk and the pinned llama.cpp checkout are available.

## Step 1 — Reproduce and bound the defect

Inspect applicable instructions, git diff/status, Cargo feature/dependency resolution and actual headers. Record compiler version and NDK path. Prefer a small compile-only reproducer with the real Android target/sysroot and NumKong header to establish the incompatible declaration, then confirm against the actual x86_64 mango_core native build. Use dedicated temporary output directories so failed native builds do not erase existing jniLibs or companion libraries.

## Step 2 — Select a reproducible minimal repair

Check the upstream NumKong source/release history for a narrowly applicable existing correction. Prefer a compatible released fix if it changes only the required dependency graph and is verifiably compatible with usearch. Record exact source/version evidence and review dependency changes.

If no suitable released fix exists, use a repository-owned, pinned Cargo patch/vendor dependency with provenance, license, and a small documented platform guard around the offending declaration. Reuse the platform syscall declaration on Bionic; retain required Linux/glibc behavior. Do not patch Cargo's shared registry in place, rely on ephemeral environment hacks, globally redefine noexcept, or silently turn off vector-search acceleration. Explain the selected tradeoff, pinning and removal conditions in the report. Keep build scripts portable and deterministic.

## Step 3 — Add meaningful regression verification

Add a reproducible compile-only regression check using the actual Android x86_64 target and sysroot, covering the conflicting include order. Include a host Linux C++ check and Android arm64 check to protect unaffected paths. The test must fail for the original declarations and pass with the chosen repair; avoid text-only tests that merely search for a new macro. If using a dependency upgrade, demonstrate the old failure separately without mutating shared cache.

## Step 4 — Verify full integration

1. Run actual cargo-ndk builds explicitly scoped to `-p mango_core` for x86_64 and arm64. Avoid accidentally compiling the desktop member for Android.
2. Run the existing `just build-android` pipeline after any safe prerequisite checks; verify success for both ABIs and required packaged shared libraries. Recipes currently clean jniLibs: retain/reconstruct all companion libraries using the established pinned sources. Do not leave previously working outputs incomplete.
3. If another compiler error in this same dependency path appears once the first one is fixed, diagnose and resolve it within the same narrow compatibility scope, documenting each separately.
4. Run vector-index / persistence regression tests and relevant host checks. If dependency resolution changes, run `cargo test -p mango_core` and a desktop check.
5. Build the Android APK against the rebuilt native library; verify Gradle success and packaged library architecture/content. Do not change shipped ABI filters solely to make the build green. No installation or data reset on the personal device is required for this build compatibility task.
6. Capture exact commands, exit codes, test counts and architecture-specific outcomes. Use pipefail if piping build output. Avoid reporting cached tests or stale libraries as newly verified.

## Step 5 — Report and independent review

Write NDK_COMPATIBILITY_IMPLEMENTATION_REPORT.md with root cause, selected repair, alternatives considered, dependency/lockfile impact, reproducibility instructions, test results and any unresolved issue. Include durable source references and relevant concise diagnostics, never secrets. Update the earlier app verification documents only to supersede the x86_64 blocker when it actually passes; retain unrelated iOS limitations.

Codex independently reviews the patch, verifies the dependency provenance and actual architecture-specific results, and reruns the focused regression. Any actionable finding is returned to Devin CLI SWE-1.7 in YOLO mode for repair and retesting. Completion requires no known unresolved defect in this scoped remediation; disclose external blockers rather than claiming success.

## Execution instruction to Devin

Implement this plan now. Start with a bounded reproduction and choose the minimal durable repair. Keep exploration focused on NumKong/usearch and the native build scripts; do not re-audit the prior UX work. Preserve all existing edits. Run tests/builds and correct introduced failures. Do not stop at planning or ask for routine implementation permission. Return the implemented change and real evidence for Codex review.
