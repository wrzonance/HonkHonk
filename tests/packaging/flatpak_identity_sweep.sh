#!/usr/bin/env bash
# Repo-wide regression guard for the io.github.thewrz -> io.github.wrzonance
# Flatpak identity rename (#207).
#
# Pins two invariants:
#   1. The old app-id appears in NO tracked file content and NO tracked
#      filename, except an explicit allowlist (see ALLOWLIST below).
#   2. Every canonical identity-bearing field names the NEW app-id, and the
#      metainfo still declares the rename migration.
#
# Invariant 1 is deliberately a whole-repo sweep rather than a list of known
# files: the failure mode being guarded against is the old ID reappearing
# somewhere nobody thought to enumerate.
# Exits non-zero if any check fails.
set -euo pipefail

PASS=0
FAIL=0

OLD_ID="io.github.thewrz.HonkHonk"
NEW_ID="io.github.wrzonance.HonkHonk"
MANIFEST="packaging/flatpak/$NEW_ID.yml"
METAINFO="packaging/flatpak/$NEW_ID.metainfo.xml"

# Files permitted to name the old ID, and why:
#   - this script and flatpak_validate.sh assert the ID's ABSENCE, so they
#     must quote it to do so;
#   - the metainfo declares it as <provides>/<replaces> so software centres
#     upgrade the shipped 0.1.0 bundle in place (checked structurally below);
#   - docs/superpowers/** are dated, historical plan and spec documents —
#     rewriting them would falsify the record of what was decided when.
ALLOWLIST=(
    tests/packaging/flatpak_identity_sweep.sh
    tests/packaging/flatpak_validate.sh
    "$METAINFO"
    docs/superpowers
)

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

# True when $1 is, or lives under, an allowlisted path.
is_allowlisted() {
    local path="$1" allowed
    for allowed in "${ALLOWLIST[@]}"; do
        [ "$path" = "$allowed" ] && return 0
        case "$path" in "$allowed"/*) return 0 ;; esac
    done
    return 1
}

echo "=== Flatpak identity sweep (#207) ==="

# ── Invariant 1a: no stale ID in any tracked file's CONTENT ───────────
content_hits=""
while IFS= read -r f; do
    [ -n "$f" ] || continue
    is_allowlisted "$f" || content_hits="$content_hits $f"
done < <(git grep -l --fixed-strings -- "$OLD_ID" -- . || true)

if [ -z "$content_hits" ]; then
    check "no tracked file content names $OLD_ID (outside the allowlist)" "ok"
else
    check "no tracked file content names $OLD_ID (outside the allowlist)" \
        "found in:$content_hits"
fi

# ── Invariant 1b: no stale ID in any tracked FILENAME ─────────────────
# Catches the rename being reverted by moving a file back, which a
# content-only sweep would silently pass.
name_hits=$(git ls-files -- . | grep -F -- "$OLD_ID" || true)
if [ -z "$name_hits" ]; then
    check "no tracked filename contains $OLD_ID" "ok"
else
    check "no tracked filename contains $OLD_ID" \
        "found: $(echo "$name_hits" | tr '\n' ' ')"
fi

# ── Invariant 2: canonical identity fields name the new ID ────────────
# Each check names an exact file, and a MISSING file is a failure — never a
# silent pass, which is how a sweep over "files that happen to exist" gets
# defeated by deleting or renaming its own targets.
expect_file_contains() {
    local desc="$1" file="$2" needle="$3"
    if [ ! -f "$file" ]; then
        check "$desc" "missing: $file"
    elif grep -qF -- "$needle" "$file"; then
        check "$desc" "ok"
    else
        check "$desc" "$file does not contain '$needle'"
    fi
}

expect_file_contains "manifest is named for the new ID" "$MANIFEST" "app-id: $NEW_ID"
expect_file_contains "metainfo is named for the new ID" "$METAINFO" "<id>$NEW_ID</id>"
expect_file_contains "metainfo launchable matches the desktop ID" \
    "$METAINFO" "<launchable type=\"desktop-id\">$NEW_ID.desktop</launchable>"
expect_file_contains "metainfo declares <provides> for the old ID" \
    "$METAINFO" "<id>$OLD_ID</id>"
expect_file_contains "release workflow builds the new-ID manifest" \
    ".github/workflows/flatpak.yml" "$MANIFEST"
expect_file_contains "release workflow bundles the new ID" \
    ".github/workflows/flatpak.yml" "flatpak build-bundle repo honkhonk.flatpak $NEW_ID"

# The icon set must be installed under the app-id, or the desktop entry's
# Icon= key resolves to nothing.
if [ -f "assets/icons/generated/hicolor/64x64/apps/$NEW_ID.png" ]; then
    check "generated icons are named for the new ID" "ok"
else
    check "generated icons are named for the new ID" \
        "missing assets/icons/generated/hicolor/64x64/apps/$NEW_ID.png"
fi

# ── Summary ───────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
