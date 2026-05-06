#!/usr/bin/env bash
# Validates AppImage scaffold prerequisites before linuxdeploy/appimagetool run.
# Exits non-zero if any check fails.
set -euo pipefail

PASS=0
FAIL=0
APPDIR="packaging/appimage/HonkHonk.AppDir"

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

echo "=== AppImage packaging validation ==="

# ── AppDir structure ─────────────────────────────────────────────────
[ -d "$APPDIR" ] \
    && check "AppDir exists" "ok" \
    || check "AppDir exists" "missing: $APPDIR"

APPRUN="$APPDIR/AppRun"
[ -f "$APPRUN" ] \
    && check "AppRun exists" "ok" \
    || check "AppRun exists" "missing: $APPRUN"

[ -x "$APPRUN" ] \
    && check "AppRun is executable" "ok" \
    || check "AppRun is executable" "not executable"

# ── AppRun content ────────────────────────────────────────────────────
if [ -f "$APPRUN" ]; then
    grep -q '^#!/' "$APPRUN" \
        && check "AppRun has shebang" "ok" \
        || check "AppRun has shebang" "missing shebang"

    grep -q 'APPDIR' "$APPRUN" \
        && check "AppRun uses APPDIR" "ok" \
        || check "AppRun uses APPDIR" "APPDIR not referenced"

    # PipeWire must NOT be bundled — must NOT be exec'd from AppDir
    grep -qE 'pipewire|pw-' "$APPRUN" \
        && check "AppRun does NOT reference pipewire (host dep)" "AppRun tries to manage pipewire — must be host dep" \
        || check "AppRun does NOT reference pipewire (host dep)" "ok"
fi

# ── Desktop + icon in AppDir root (required by AppImage spec) ─────────
DESKTOP="$APPDIR/honkhonk.desktop"
ICON="$APPDIR/honkhonk.png"

[ -f "$DESKTOP" ] \
    && check "honkhonk.desktop in AppDir root" "ok" \
    || check "honkhonk.desktop in AppDir root" "missing: $DESKTOP"

[ -f "$ICON" ] \
    && check "honkhonk.png in AppDir root" "ok" \
    || check "honkhonk.png in AppDir root" "missing: $ICON"

if [ -f "$ICON" ]; then
    DIMS=$(identify -format "%wx%h" "$ICON" 2>/dev/null || echo "unknown")
    [ "$DIMS" = "256x256" ] \
        && check "AppDir root icon is 256x256" "ok" \
        || check "AppDir root icon is 256x256" "got: $DIMS"
fi

# ── usr/ layout ───────────────────────────────────────────────────────
[ -d "$APPDIR/usr/share/applications" ] \
    && check "usr/share/applications/ exists" "ok" \
    || check "usr/share/applications/ exists" "missing"

[ -f "$APPDIR/usr/share/applications/honkhonk.desktop" ] \
    && check "desktop file in usr/share/applications" "ok" \
    || check "desktop file in usr/share/applications" "missing"

[ -d "$APPDIR/usr/share/icons/hicolor/256x256/apps" ] \
    && check "hicolor icon dir exists" "ok" \
    || check "hicolor icon dir exists" "missing"

[ -f "$APPDIR/usr/share/icons/hicolor/256x256/apps/honkhonk.png" ] \
    && check "icon in hicolor path" "ok" \
    || check "icon in hicolor path" "missing"

# ── Runtime doc ───────────────────────────────────────────────────────
README="packaging/appimage/README.md"
[ -f "$README" ] \
    && check "AppImage README exists" "ok" \
    || check "AppImage README exists" "missing: $README"

if [ -f "$README" ]; then
    grep -qi 'pipewire' "$README" \
        && check "README documents PipeWire host requirement" "ok" \
        || check "README documents PipeWire host requirement" "not mentioned"

    grep -qi 'DBUS_SESSION_BUS_ADDRESS\|dbus' "$README" \
        && check "README documents D-Bus requirement" "ok" \
        || check "README documents D-Bus requirement" "not mentioned"
fi

# ── Summary ───────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
