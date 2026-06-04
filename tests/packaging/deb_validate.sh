#!/usr/bin/env bash
# Validates all prerequisites for `cargo deb` are in place.
# Exits non-zero if any check fails.
set -euo pipefail

PASS=0
FAIL=0

check() {
    local desc="$1"
    local result="$2"
    if [ "$result" = "ok" ]; then
        echo "  PASS  $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  $desc — $result"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== DEB packaging validation ==="

# ── Asset files ──────────────────────────────────────────────────────
DESKTOP="assets/honkhonk.desktop"
ICON="assets/icons/hicolor/256x256/apps/honkhonk.png"

[ -f "$DESKTOP" ] \
    && check ".desktop file exists" "ok" \
    || check ".desktop file exists" "missing: $DESKTOP"

[ -f "$ICON" ] \
    && check "256x256 icon exists" "ok" \
    || check "256x256 icon exists" "missing: $ICON"

if [ -f "$ICON" ]; then
    DIMS=$(identify -format "%wx%h" "$ICON" 2>/dev/null || echo "unknown")
    [ "$DIMS" = "256x256" ] \
        && check "icon dimensions 256x256" "ok" \
        || check "icon dimensions 256x256" "got: $DIMS"
fi

# ── .desktop content ─────────────────────────────────────────────────
if [ -f "$DESKTOP" ]; then
    grep -q "^Name=HonkHonk" "$DESKTOP" \
        && check ".desktop has Name=HonkHonk" "ok" \
        || check ".desktop has Name=HonkHonk" "missing or wrong"
    grep -q "^Exec=honkhonk" "$DESKTOP" \
        && check ".desktop has Exec=honkhonk" "ok" \
        || check ".desktop has Exec=honkhonk" "missing or wrong"
    grep -q "^Icon=honkhonk" "$DESKTOP" \
        && check ".desktop has Icon=honkhonk" "ok" \
        || check ".desktop has Icon=honkhonk" "missing or wrong"
    grep -q "^Categories=.*Audio" "$DESKTOP" \
        && check ".desktop has Audio category" "ok" \
        || check ".desktop has Audio category" "missing or wrong"
fi

# ── Cargo.toml metadata ──────────────────────────────────────────────
CARGO="Cargo.toml"

grep -q "\[package.metadata.deb\]" "$CARGO" \
    && check "Cargo.toml has [package.metadata.deb]" "ok" \
    || check "Cargo.toml has [package.metadata.deb]" "section missing"

grep -q "maintainer" "$CARGO" \
    && check "Cargo.toml deb has maintainer" "ok" \
    || check "Cargo.toml deb has maintainer" "field missing"

grep -q "depends.*pipewire\|depends.*\\\$auto" "$CARGO" \
    && check "Cargo.toml deb depends on pipewire or \$auto" "ok" \
    || check "Cargo.toml deb depends on pipewire or \$auto" "missing depends"

grep -q "recommends" "$CARGO" \
    && check "Cargo.toml deb has recommends" "ok" \
    || check "Cargo.toml deb has recommends" "field missing"

grep -q 'assets.*honkhonk.desktop' "$CARGO" \
    && check "Cargo.toml deb installs .desktop" "ok" \
    || check "Cargo.toml deb installs .desktop" "asset entry missing"

grep -q 'assets.*honkhonk.png' "$CARGO" \
    && check "Cargo.toml deb installs icon" "ok" \
    || check "Cargo.toml deb installs icon" "asset entry missing"

# ── conf.d drop-in (issue #49) ───────────────────────────────────────
CONFD="packaging/pipewire/50-honkhonk.conf"
[ -f "$CONFD" ] \
    && check "conf.d drop-in exists" "ok" \
    || check "conf.d drop-in exists" "missing: $CONFD"

if [ -f "$CONFD" ]; then
    grep -q 'node.name .*"honkhonk-mic"' "$CONFD" \
        && check "conf.d declares honkhonk-mic" "ok" \
        || check "conf.d declares honkhonk-mic" "node.name missing"
    grep -q 'object.linger .* true' "$CONFD" \
        && check "conf.d sets object.linger true" "ok" \
        || check "conf.d sets object.linger true" "missing"
fi

grep -q 'pipewire.conf.d' "$CARGO" \
    && check "Cargo.toml deb installs conf.d" "ok" \
    || check "Cargo.toml deb installs conf.d" "asset entry missing"

grep -q 'maintainer-scripts' "$CARGO" \
    && check "Cargo.toml deb has maintainer-scripts" "ok" \
    || check "Cargo.toml deb has maintainer-scripts" "field missing"

POSTRM="packaging/deb/postrm"
[ -x "$POSTRM" ] \
    && check "deb postrm exists and is executable" "ok" \
    || check "deb postrm exists and is executable" "missing or not +x: $POSTRM"

# ── Summary ──────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
