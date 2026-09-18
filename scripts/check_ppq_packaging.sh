#!/usr/bin/env bash
# check_ppq_packaging.sh — managed-PPQ packaging gate for Mango Android APKs.
#
# Managed PPQ ships in every build. This gate verifies the actual bytes of a
# built APK instead of trusting build flags: it scans EVERY classes*.dex entry
# (multi-dex / R8-split safe) for the PPQ setup CTA string that only exists in
# the managed-PPQ UI tree.
#
# Usage:
#   scripts/check_ppq_packaging.sh present <apk>   # fail unless the string is packaged
#   scripts/check_ppq_packaging.sh selftest        # build synthetic APKs and check the gate
#
# Exit codes: 0 = gate satisfied, 1 = gate violated, 2 = usage/bad input.
# Never reads or prints secrets; APKs are local build artifacts only.

set -euo pipefail

NEEDLE="Set up PPQ automatically"

fail() { echo "FAIL: $*" >&2; exit 1; }

scan_apk() {
    # Prints "1" if NEEDLE appears in any classes*.dex of $1, else "0".
    #
    # SIGPIPE hardening: `unzip -p | strings | grep -Fq` looks natural but breaks
    # with `set -o pipefail` on real APKs — grep -q exits at the first match and
    # SIGPIPEs the upstream strings/unzip still writing a large DEX (observed on
    # app-debug.apk where the CTA lives in classes5.dex), turning a hit into a
    # false negative. We therefore stage each DEX and its strings output to temp
    # files (producers always run to completion; only grep -q on a plain file can
    # exit early, which harms nothing) so the gate reflects actual build output.
    local apk="$1"
    [[ -f "$apk" ]] || fail "APK not found: $apk"
    local dexes
    dexes="$(unzip -Z1 "$apk" | LC_ALL=C grep -E '^classes[0-9]*\.dex$' || true)"
    [[ -n "$dexes" ]] || fail "no classes*.dex entries found inside $apk (malformed APK?)"
    local tmpdex tmpstr
    tmpdex="$(mktemp)"
    tmpstr="$(mktemp)"
    trap 'rm -f "$tmpdex" "$tmpstr"' RETURN
    local dex
    while IFS= read -r dex; do
        unzip -p "$apk" "$dex" > "$tmpdex"
        LC_ALL=C strings "$tmpdex" > "$tmpstr"
        if LC_ALL=C grep -Fq -- "$NEEDLE" "$tmpstr"; then
            echo "  [gate] '$NEEDLE' found in $dex" >&2
            echo 1
            return 0
        fi
    done <<< "$dexes"
    echo 0
}

main() {
    local mode="${1:-}" apk="${2:-}"
    case "$mode" in
        present) ;;
        selftest) selftest; exit 0 ;;
        *) fail "usage: $0 present <apk> | selftest" ;;
    esac
    [[ -n "$apk" ]] || fail "usage: $0 present <apk>"
    local found
    found="$(scan_apk "$apk")"
    [[ "$found" == 1 ]] || fail "'$NEEDLE' missing from every dex in $apk — every build must ship the managed-PPQ UI"
    echo "OK: managed-PPQ UI packaged in $apk"
}

selftest() {
    # Focused regression test for the gate itself (release remediation finding 7):
    # proves the scan covers secondary dex files, survives a large DEX whose match
    # lands early with megabytes of trailing data (SIGPIPE regression), and that
    # the gate rejects an APK missing the CTA.
    command -v unzip >/dev/null || fail "selftest requires unzip"
    command -v zip >/dev/null || fail "selftest requires zip"
    command -v strings >/dev/null || fail "selftest requires strings"
    local tmp
    tmp="$(mktemp -d)"
    trap 'rm -rf "${tmp:-}"' EXIT

    # APK A: needle ONLY in classes2.dex (multi-dex placement) -> "present" passes.
    mkdir -p "$tmp/a"
    echo "ordinary dex payload without the cta" > "$tmp/a/classes.dex"
    printf 'some\x00padding\x00Set up PPQ automatically\x00trailer\n' > "$tmp/a/classes2.dex"
    (cd "$tmp/a" && zip -qr "$tmp/on.apk" classes.dex classes2.dex)

    # APK B: needle nowhere -> present-mode must reject it.
    mkdir -p "$tmp/b"
    echo "another ordinary dex payload" > "$tmp/b/classes.dex"
    echo "and a second one" > "$tmp/b/classes3.dex"
    (cd "$tmp/b" && zip -qr "$tmp/off.apk" classes.dex classes3.dex)

    # APK C: needle near the START of a multi-megabyte DEX. With the old
    # `grep -Fq` pipe this triggered SIGPIPE under pipefail and reported a miss;
    # the staged scan must report the hit (SIGPIPE regression, real-world shape:
    # app-debug.apk with the CTA early in classes5.dex).
    mkdir -p "$tmp/c"
    {
        printf 'header\x00Set up PPQ automatically\x00'
        head -c $((8 * 1024 * 1024)) /dev/zero
    } > "$tmp/c/classes5.dex"
    (cd "$tmp/c" && zip -qr0 "$tmp/big.apk" classes5.dex)

    local rc=0
    "$0" present "$tmp/on.apk" >&2 || rc=1
    "$0" present "$tmp/big.apk" >&2 || rc=1
    if "$0" present "$tmp/off.apk" >/dev/null 2>&1; then
        fail "selftest: present-mode must reject an APK missing the CTA"
    fi
    [[ $rc == 0 ]] || fail "selftest: expected modes failed"
    echo "OK: check_ppq_packaging selftest passed (multi-dex, 8 MiB early-hit DEX, missing-CTA rejection)"
}

main "$@"
