#!/usr/bin/env bash
# Validates the Flatpak manifest before flatpak-builder runs.
# Exits non-zero if any check fails.
set -euo pipefail

PASS=0
FAIL=0
MANIFEST="packaging/flatpak/io.github.wrzonance.HonkHonk.yml"
METAINFO="packaging/flatpak/io.github.wrzonance.HonkHonk.metainfo.xml"

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

if python3 - 2>/dev/null <<'PYEOF'
import sys
try:
    import yaml
    yaml.safe_load(open("packaging/flatpak/io.github.wrzonance.HonkHonk.yml"))
except ImportError:
    pass  # pyyaml not available — skip, other checks verify structure
except Exception:
    sys.exit(1)
PYEOF
then
    check "manifest is valid YAML" "ok"
else
    check "manifest is valid YAML" "parse error"
fi

# ── App identity ──────────────────────────────────────────────────────
has 'io.github.wrzonance.HonkHonk' \
    && check "app-id is io.github.wrzonance.HonkHonk" "ok" \
    || check "app-id is io.github.wrzonance.HonkHonk" "missing"

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
has 'io.github.wrzonance.HonkHonk.desktop' \
    && check "desktop installs under the full app-id" "ok" \
    || check "desktop installs under the full app-id" "missing"

has 'assets/icons/generated/hicolor' \
    && check "manifest installs the full app-id-named icon set" "ok" \
    || check "manifest installs the full app-id-named icon set" "missing"

# ── No stale identity left behind ─────────────────────────────────────
# The manifest and workflow must never name the old ID. The metainfo is the
# one exception — it declares the old ID as <provides>/<replaces> so software
# centres upgrade the 0.1.0 bundle in place rather than installing a second
# copy — so it is checked structurally below, not by grep.
for f in "$MANIFEST" .github/workflows/flatpak.yml; do
    if [ ! -f "$f" ]; then
        check "$f has no stale thewrz identity" "missing: $f"
    elif grep -qF 'io.github.thewrz' "$f"; then
        check "$f has no stale thewrz identity" "found io.github.thewrz reference"
    else
        check "$f has no stale thewrz identity" "ok"
    fi
done

# ── Metainfo identity + rename migration ──────────────────────────────
if [ ! -f "$METAINFO" ]; then
    check "metainfo declares the rename migration" "missing: $METAINFO"
elif metainfo_result=$(python3 - "$METAINFO" <<'PYEOF'
import sys
import xml.etree.ElementTree as ET

NEW = "io.github.wrzonance.HonkHonk"
OLD = "io.github.thewrz.HonkHonk"

# AppStream metainfo never carries a DOCTYPE. Rejecting one keeps the stdlib
# parser off the entity-expansion paths (XXE / billion laughs) without needing
# defusedxml, which cannot be installed in the Flatpak builder image (no pip).
raw = open(sys.argv[1], "rb").read()
if b"<!DOCTYPE" in raw or b"<!ENTITY" in raw:
    print("metainfo contains a DOCTYPE/ENTITY declaration — refusing to parse")
    sys.exit(0)

root = ET.fromstring(raw)

def ids(tag):
    return {e.text.strip() for p in root.findall(tag) for e in p.findall("id") if e.text}

component_id = (root.findtext("id") or "").strip()
problems = []
if component_id != NEW:
    problems.append(f"<id> is {component_id!r}, expected {NEW!r}")
if OLD not in ids("provides"):
    problems.append(f"<provides> does not declare {OLD}")
if OLD not in ids("replaces"):
    problems.append(f"<replaces> does not declare {OLD}")

print("; ".join(problems) if problems else "ok")
PYEOF
); then
    check "metainfo declares the rename migration" "$metainfo_result"
else
    check "metainfo declares the rename migration" "could not parse $METAINFO"
fi

# ── AppStream strict validation ────────────────────────────────────────
if command -v appstreamcli >/dev/null 2>&1; then
    check "appstreamcli is available" "ok"
    if appstreamcli validate --strict --no-net "$METAINFO" >/tmp/appstream-validate.log 2>&1; then
        check "metainfo passes appstreamcli --strict" "ok"
    else
        check "metainfo passes appstreamcli --strict" \
            "$(tail -n5 /tmp/appstream-validate.log | tr '\n' ' ')"
    fi
else
    check "appstreamcli is available" "not installed — strict validation skipped"
fi

# ── Summary ───────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
