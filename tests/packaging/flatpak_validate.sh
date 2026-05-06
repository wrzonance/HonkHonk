#!/usr/bin/env bash
# Validates the Flatpak manifest before flatpak-builder runs.
# Exits non-zero if any check fails.
set -euo pipefail

PASS=0
FAIL=0
MANIFEST="packaging/flatpak/io.github.thewrz.HonkHonk.yml"

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

has() { grep -qF -- "$1" "$MANIFEST"; }

echo "=== Flatpak manifest validation ==="

# ── File exists and is valid YAML ─────────────────────────────────────
[ -f "$MANIFEST" ] \
    && check "manifest file exists" "ok" \
    || check "manifest file exists" "missing: $MANIFEST"

[ -f "$MANIFEST" ] || { echo ""; echo "Results: $PASS passed, $FAIL failed"; exit 1; }

python3 -c "import yaml, sys; yaml.safe_load(open('$MANIFEST'))" 2>/dev/null \
    && check "manifest is valid YAML" "ok" \
    || check "manifest is valid YAML" "parse error"

# ── App identity ──────────────────────────────────────────────────────
has 'io.github.thewrz.HonkHonk' \
    && check "app-id is io.github.thewrz.HonkHonk" "ok" \
    || check "app-id is io.github.thewrz.HonkHonk" "missing"

has 'org.freedesktop.Platform' \
    && check "runtime is org.freedesktop.Platform" "ok" \
    || check "runtime is org.freedesktop.Platform" "missing"

has 'command: honkhonk' \
    && check "command is honkhonk" "ok" \
    || check "command is honkhonk" "missing"

# ── finish-args: required permissions ─────────────────────────────────
has '--socket=wayland' \
    && check "finish-args: --socket=wayland" "ok" \
    || check "finish-args: --socket=wayland" "missing"

has '--socket=pulseaudio' \
    && check "finish-args: --socket=pulseaudio (PipeWire compat)" "ok" \
    || check "finish-args: --socket=pulseaudio (PipeWire compat)" "missing"

has '--device=dri' \
    && check "finish-args: --device=dri (GPU for wgpu)" "ok" \
    || check "finish-args: --device=dri (GPU for wgpu)" "missing"

has '--talk-name=org.kde.StatusNotifierWatcher' \
    && check "finish-args: StatusNotifierWatcher (SNI tray)" "ok" \
    || check "finish-args: StatusNotifierWatcher (SNI tray)" "missing"

has '--filesystem=xdg-music' \
    && check "finish-args: --filesystem=xdg-music (sound library)" "ok" \
    || check "finish-args: --filesystem=xdg-music (sound library)" "missing"

# ── Build: Rust SDK extension ─────────────────────────────────────────
has 'rust-stable' \
    && check "uses org.freedesktop.Sdk.Extension.rust-stable" "ok" \
    || check "uses org.freedesktop.Sdk.Extension.rust-stable" "missing"

# ── Module: binary installed to correct Flatpak path ─────────────────
has '/app/bin/honkhonk' \
    && check "binary installed to /app/bin/honkhonk (on PATH)" "ok" \
    || check "binary installed to /app/bin/honkhonk (on PATH)" "missing"

# /app/usr/{bin,share} are NOT on PATH/XDG_DATA_DIRS inside the sandbox
if grep -qE '/app/usr/(bin|share)/' "$MANIFEST"; then
    check "no installs under /app/usr/ (wrong Flatpak path)" \
        "found /app/usr/ — use /app/bin/ and /app/share/ instead"
else
    check "no installs under /app/usr/ (wrong Flatpak path)" "ok"
fi

# ── Assets: .desktop and icon ─────────────────────────────────────────
has 'honkhonk.desktop' \
    && check "manifest references honkhonk.desktop" "ok" \
    || check "manifest references honkhonk.desktop" "missing"

has 'honkhonk.png' \
    && check "manifest references icon" "ok" \
    || check "manifest references icon" "missing"

# ── Summary ───────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
