#!/usr/bin/env bash
# Regenerates Flatpak Cargo vendored sources and fails if the committed copy is stale.
set -euo pipefail

GENERATOR="packaging/flatpak/flatpak-cargo-generator.py"
LOCKFILE="Cargo.lock"
SOURCES="packaging/flatpak/cargo-sources.json"

for path in "$GENERATOR" "$LOCKFILE" "$SOURCES"; do
    if [ ! -f "$path" ]; then
        echo "missing required file: $path" >&2
        exit 1
    fi
done

python3 "$GENERATOR" "$LOCKFILE" -o "$SOURCES"

if ! git diff --exit-code -- "$SOURCES"; then
    cat >&2 <<'EOF'

Flatpak Cargo sources are stale.
Regenerate them with:
  python3 packaging/flatpak/flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
EOF
    exit 1
fi
