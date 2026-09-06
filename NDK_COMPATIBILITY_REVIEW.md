# Codex review of the NDK compatibility patch

The Android-specific header guard is narrow and the original failure was independently reproduced against the installed NDK. Devin's full x86_64/arm64 pipeline and APK results establish the intended build repair. Complete the following bounded corrections using SWE-1.7 / YOLO mode; do not modify application code or repeat broad investigation.

1. **Regression checker false success.** `check_numkong_android_headers.sh` prints SKIP when a compiler is absent, then exits 0 with RESULT: PASS even if neither Android target ran. Make all three required compiler checks mandatory on the supported Linux host and return nonzero for missing tools, missing headers, or unsupported host configuration. An optional skip mode, if added, must be explicit and must not claim full verification. Test normal success, original-header failure, and missing-NDK failure. Correct the comment claiming only NumKong `<7.8.2` is affected: the inspected 7.8.2 is also affected.

2. **Vendored license/provenance.** VENDOR.md names Apache-2.0 and references an upstream LICENSE file, but vendor/numkong currently has no license text. Add the exact license from the recorded upstream commit, and any upstream NOTICE applicable to this version. Preserve its source/provenance; do not synthesize terms. Verify the recorded crate checksum and commit against the original cached crate metadata; accurately state any omitted registry bookkeeping files. Confirm there are no additional source differences beyond the Android guard.

3. **Report confidence and residual failure.** The exact unpatched 7.8.1 control passed while multiple patched parallel runs failed and isolated/serial runs passed. The Android-only preprocessor difference makes a direct Linux semantic regression unlikely, but these results do not prove a pre-existing failure of this exact test or conclusively diagnose timing. Report observations: repeated parallel `db unlocked` panic in the enrollment test, passing exact-version control, passing patched serial suite, root cause not conclusively diagnosed in this remediation. Do not say all host tests pass unconditionally. Preserve that caveat in any superseding verification notes. There is no need to expand this NDK task into an actor fix.

Run the three regression-script cases with reliable exit codes, verify final Cargo resolution remains the local numkong 7.8.1 patch, and update NDK_COMPATIBILITY_IMPLEMENTATION_REPORT.md. Source runtime code is otherwise unchanged, so do not rerun the long host suite merely for documentation/license/script edits. Return results for final Codex review. No commits, pushes, app installs, or personal data changes.

## Final disposition — 2026-09-06

Devin addressed all three review findings. Codex independently reran the
patched header check (all three targets PASS), the original registry-header
check (expected x86_64 compilation failure), and the missing-NDK check
(expected nonzero failure), and checked script syntax. The implementation
report now explicitly retains the unresolved parallel-host-test observation.
Both staged native libraries were independently identified as the correct
Android 28 x86_64 and arm64 ELF artifacts. Final dependency resolution retains
the local NumKong 7.8.1 patch without other version changes.

The targeted NDK build conflict is resolved. This is not a claim that all
application tests pass: the parallel enrollment failure remains undiagnosed,
and prior iOS build/runtime verification limitations remain unchanged.
