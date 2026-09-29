#!/usr/bin/env bash
# Fails unless an SPDX JSON SBOM lists exactly the crates pinned in Cargo.lock.
# Usage: tests/packaging/sbom_matches_lockfile.sh <sbom.spdx.json> [Cargo.lock]
set -euo pipefail

SBOM="${1:?usage: sbom_matches_lockfile.sh <sbom.spdx.json> [Cargo.lock]}"
LOCKFILE="${2:-Cargo.lock}"

for path in "$SBOM" "$LOCKFILE"; do
    if [ ! -f "$path" ]; then
        echo "missing required file: $path" >&2
        exit 1
    fi
done

lock_crates() {
    awk -F'"' '
        /^\[\[package\]\]/ { in_package = 1; next }
        in_package && /^name = /    { name = $2 }
        in_package && /^version = / { print name "@" $2; in_package = 0 }
    ' "$LOCKFILE" | sort -u
}

# Path crates (honkhonk itself, vendor/winit) carry no registry purl, so select
# on "not the scanned file" rather than on a pkg:cargo reference.
sbom_crates() {
    jq -r '
        .packages[]
        | select(.primaryPackagePurpose != "FILE")
        | "\(.name)@\(.versionInfo)"
    ' "$SBOM" | sort -u
}

expected="$(lock_crates)"
actual="$(sbom_crates)"

if [ -z "$expected" ]; then
    echo "no crates parsed from $LOCKFILE" >&2
    exit 1
fi

if ! drift="$(diff <(printf '%s\n' "$expected") <(printf '%s\n' "$actual"))"; then
    cat >&2 <<MSG
SBOM does not match $LOCKFILE ('<' only in the lockfile, '>' only in the SBOM):
$drift
MSG
    exit 1
fi

echo "SBOM matches $LOCKFILE: $(printf '%s\n' "$expected" | wc -l) crates"
