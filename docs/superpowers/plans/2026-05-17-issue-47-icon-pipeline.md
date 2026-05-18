# Issue #47 Icon Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a fully automated icon export pipeline (Makefile + placeholder SVGs + committed generated outputs + CI freshness gate) producing every icon size HonkHonk needs, using `io.github.wrzonance.HonkHonk` as the canonical app-id.

**Architecture:** Two source SVGs (color + symbolic) live under `assets/icons/`. A `Makefile` drives `resvg` (PNG rasterization) and ImageMagick (ICO assembly + social card compositing). All outputs are committed under `assets/icons/generated/`. A dedicated CI workflow (`.github/workflows/icons.yml`) regenerates and asserts no drift on every push/PR touching icon files.

**Tech Stack:** GNU Make, `resvg` 0.47+ (cargo-installed), ImageMagick 7 `magick` binary, GitHub Actions.

**Scope guardrails (from spec):**
- Real Krita-designed art is OUT — placeholders only.
- AppImage / .desktop / Flatpak file renames are OUT — additive PR only.
- `assets/icons/hicolor/256x256/apps/honkhonk.png` stays untouched.

---

## File structure

```
assets/icons/
├── icon.svg                                  # NEW source (color)
├── icon-symbolic.svg                         # NEW source (monochrome)
├── Makefile                                  # NEW pipeline
├── README.md                                 # NEW runbook
├── hicolor/256x256/apps/honkhonk.png         # EXISTING — DO NOT TOUCH
└── generated/                                # NEW committed outputs
    ├── hicolor/
    │   ├── {16,22,24,32,48,64,128,256,512}x*/apps/io.github.wrzonance.HonkHonk.png
    │   ├── scalable/apps/io.github.wrzonance.HonkHonk.svg
    │   └── symbolic/apps/io.github.wrzonance.HonkHonk-symbolic.svg
    ├── favicon.ico
    └── social-preview.png
.github/workflows/icons.yml                   # NEW CI gate
README.md                                     # MODIFY (append Icon section)
```

---

### Task 1: Add placeholder color SVG source

**Files:**
- Create: `assets/icons/icon.svg`

**Why first:** Without a source SVG, the Makefile has nothing to rasterize. We start with the asset because every subsequent step depends on it.

- [ ] **Step 1: Write `assets/icons/icon.svg`**

Create the file with this exact content:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
  <title>HonkHonk</title>
  <!-- Placeholder geometric mark. Real Krita art replaces this in a follow-up PR. -->
  <rect x="0" y="0" width="1024" height="1024" rx="180" ry="180" fill="#1a1a2e"/>
  <!-- Left H -->
  <rect x="200" y="280" width="80" height="464" fill="#f7d34a"/>
  <rect x="380" y="280" width="80" height="464" fill="#f7d34a"/>
  <rect x="200" y="480" width="260" height="64" fill="#f7d34a"/>
  <!-- Right H -->
  <rect x="564" y="280" width="80" height="464" fill="#f7d34a"/>
  <rect x="744" y="280" width="80" height="464" fill="#f7d34a"/>
  <rect x="564" y="480" width="260" height="64" fill="#f7d34a"/>
</svg>
```

This is a 1024x1024 viewBox, content well inside the 80% safe area (200..824 horizontally, 280..744 vertically). No embedded raster, no fonts, no external refs.

- [ ] **Step 2: Sanity-render at 16x16 to confirm legibility**

Run: `~/.cargo/bin/resvg --width 16 --height 16 assets/icons/icon.svg /tmp/icon-16.png && file /tmp/icon-16.png`
Expected: `/tmp/icon-16.png: PNG image data, 16 x 16, 8-bit/color RGBA, non-interlaced`

- [ ] **Step 3: Commit**

```bash
git add assets/icons/icon.svg
git commit -m "feat(assets): add placeholder color SVG source"
```

---

### Task 2: Add placeholder symbolic SVG source

**Files:**
- Create: `assets/icons/icon-symbolic.svg`

Symbolic icons MUST use `fill="currentColor"` so DEs can recolor for light/dark themes.

- [ ] **Step 1: Write `assets/icons/icon-symbolic.svg`**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16">
  <title>HonkHonk symbolic</title>
  <!-- Single path, monochrome, currentColor so DE themes can recolor. -->
  <path fill="currentColor" d="M2 3 H3 V7 H5 V3 H6 V13 H5 V8 H3 V13 H2 Z M9 3 H10 V7 H12 V3 H13 V13 H12 V8 H10 V13 H9 Z"/>
</svg>
```

The viewBox uses the standard 16x16 GTK symbolic dimensions. Two adjacent "H" glyphs drawn as a single path with `fill="currentColor"`.

- [ ] **Step 2: Confirm `currentColor` present**

Run: `grep -c 'fill="currentColor"' assets/icons/icon-symbolic.svg`
Expected: `1`

- [ ] **Step 3: Commit**

```bash
git add assets/icons/icon-symbolic.svg
git commit -m "feat(assets): add placeholder symbolic SVG source"
```

---

### Task 3: Write the Makefile

**Files:**
- Create: `assets/icons/Makefile`

The Makefile is the single source of truth for "how do I regenerate every icon?". It uses `resvg` for PNG rasterization and `magick` for ICO assembly + social card.

**Implementation note:** GNU Make pattern rules need a `%` on both sides; we use `%x%` so the same number drives width and height (`16x16`, `22x22`, etc.). Each size becomes its own target via `$(SIZES:%=$(OUT)/hicolor/%x%/apps/$(APPID).png)`.

- [ ] **Step 1: Write `assets/icons/Makefile`**

```makefile
SIZES        = 16 22 24 32 48 64 128 256 512
APPID        = io.github.wrzonance.HonkHonk
SVG_COLOR    = icon.svg
SVG_SYMBOLIC = icon-symbolic.svg
OUT          = generated

RESVG ?= resvg
MAGICK ?= magick

.PHONY: icons hicolor symbolic favicon social clean check-tools

icons: check-tools hicolor symbolic favicon social

check-tools:
	@command -v $(RESVG) >/dev/null || { echo "ERROR: resvg missing — cargo install resvg --locked"; exit 1; }
	@command -v $(MAGICK) >/dev/null || { echo "ERROR: ImageMagick missing"; exit 1; }

hicolor: $(SIZES:%=$(OUT)/hicolor/%x%/apps/$(APPID).png) \
         $(OUT)/hicolor/scalable/apps/$(APPID).svg

$(OUT)/hicolor/%x%/apps/$(APPID).png: $(SVG_COLOR)
	@mkdir -p $(dir $@)
	$(RESVG) --width $* --height $* $< $@

$(OUT)/hicolor/scalable/apps/$(APPID).svg: $(SVG_COLOR)
	@mkdir -p $(dir $@)
	cp $< $@

symbolic: $(OUT)/hicolor/symbolic/apps/$(APPID)-symbolic.svg

$(OUT)/hicolor/symbolic/apps/$(APPID)-symbolic.svg: $(SVG_SYMBOLIC)
	@mkdir -p $(dir $@)
	cp $< $@

favicon: $(OUT)/favicon.ico

$(OUT)/favicon.ico: $(SVG_COLOR)
	@mkdir -p $(OUT)
	$(RESVG) --width 256 --height 256 $< $(OUT)/favicon-256.png
	$(MAGICK) $(OUT)/favicon-256.png -background none \
	  -define icon:auto-resize="256,128,64,48,32,16" $@
	rm $(OUT)/favicon-256.png

social: $(OUT)/social-preview.png

$(OUT)/social-preview.png: $(SVG_COLOR)
	@mkdir -p $(OUT)
	$(RESVG) --width 512 --height 512 $< $(OUT)/social-icon.png
	$(MAGICK) -size 1280x640 xc:'#1a1a2e' \
	  $(OUT)/social-icon.png -gravity center -composite \
	  -font DejaVu-Sans-Bold -pointsize 96 -fill white \
	  -gravity south -annotate +0+80 'HonkHonk' $@
	rm $(OUT)/social-icon.png

clean:
	rm -rf $(OUT)
```

The `RESVG ?=` / `MAGICK ?=` lets CI / contributors override the binary path if needed (e.g. CI installs to `~/.cargo/bin/resvg`).

- [ ] **Step 2: Run `make check-tools` to validate the dependency-check target**

Run: `cd assets/icons && PATH="$HOME/.cargo/bin:$PATH" make check-tools`
Expected: exit 0, no output (commands silently found).

- [ ] **Step 3: Commit**

```bash
git add assets/icons/Makefile
git commit -m "feat(assets): add icon export Makefile (resvg + ImageMagick)"
```

---

### Task 4: Generate and commit hicolor PNGs + scalable + symbolic outputs

**Files:**
- Create (binary): `assets/icons/generated/hicolor/{16,22,24,32,48,64,128,256,512}x*/apps/io.github.wrzonance.HonkHonk.png`
- Create: `assets/icons/generated/hicolor/scalable/apps/io.github.wrzonance.HonkHonk.svg`
- Create: `assets/icons/generated/hicolor/symbolic/apps/io.github.wrzonance.HonkHonk-symbolic.svg`

- [ ] **Step 1: Run `make hicolor symbolic`**

Run: `cd assets/icons && PATH="$HOME/.cargo/bin:$PATH" make hicolor symbolic`
Expected: 9 PNG files + 1 scalable SVG + 1 symbolic SVG created under `generated/hicolor/`.

- [ ] **Step 2: Verify all 9 PNG sizes exist and are valid**

Run from repo root:
```bash
for s in 16 22 24 32 48 64 128 256 512; do
  f="assets/icons/generated/hicolor/${s}x${s}/apps/io.github.wrzonance.HonkHonk.png"
  test -f "$f" && file "$f" | grep -q "PNG image data, $s x $s" && echo "OK $s" || echo "FAIL $s"
done
```
Expected: nine `OK` lines.

- [ ] **Step 3: Verify scalable + symbolic SVGs exist**

Run:
```bash
test -f assets/icons/generated/hicolor/scalable/apps/io.github.wrzonance.HonkHonk.svg && echo "scalable OK"
test -f assets/icons/generated/hicolor/symbolic/apps/io.github.wrzonance.HonkHonk-symbolic.svg && echo "symbolic OK"
grep -q 'currentColor' assets/icons/generated/hicolor/symbolic/apps/io.github.wrzonance.HonkHonk-symbolic.svg && echo "currentColor preserved"
```
Expected: three OK lines.

- [ ] **Step 4: Commit generated PNGs/SVGs**

```bash
git add assets/icons/generated/hicolor
git commit -m "feat(assets): commit generated hicolor PNGs + scalable + symbolic outputs"
```

---

### Task 5: Generate and commit favicon.ico

**Files:**
- Create (binary): `assets/icons/generated/favicon.ico`

- [ ] **Step 1: Run `make favicon`**

Run: `cd assets/icons && PATH="$HOME/.cargo/bin:$PATH" make favicon`
Expected: `generated/favicon.ico` exists.

- [ ] **Step 2: Verify ICO is multi-res Windows icon**

Run: `file assets/icons/generated/favicon.ico`
Expected output contains `MS Windows icon resource` and lists at least 6 icons (16/32/48/64/128/256).

- [ ] **Step 3: Commit**

```bash
git add assets/icons/generated/favicon.ico
git commit -m "feat(assets): commit generated multi-res favicon.ico"
```

---

### Task 6: Generate and commit social-preview.png

**Files:**
- Create (binary): `assets/icons/generated/social-preview.png`

- [ ] **Step 1: Run `make social`**

Run: `cd assets/icons && PATH="$HOME/.cargo/bin:$PATH" make social`
Expected: `generated/social-preview.png` exists at 1280x640.

- [ ] **Step 2: Verify dimensions**

Run: `file assets/icons/generated/social-preview.png`
Expected output contains `1280 x 640`.

- [ ] **Step 3: Commit**

```bash
git add assets/icons/generated/social-preview.png
git commit -m "feat(assets): commit generated GitHub social preview card"
```

---

### Task 7: Add CI freshness gate workflow

**Files:**
- Create: `.github/workflows/icons.yml`

The workflow installs `resvg` via cargo, installs ImageMagick via apt, runs `make icons`, then asserts `git diff --exit-code` over `assets/icons/generated/` (drift gate) plus file-existence + ICO-format assertions.

- [ ] **Step 1: Write `.github/workflows/icons.yml`**

```yaml
name: Icon pipeline freshness

on:
  push:
    paths:
      - 'assets/icons/**'
      - '.github/workflows/icons.yml'
  pull_request:
    paths:
      - 'assets/icons/**'
      - '.github/workflows/icons.yml'

jobs:
  freshness:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install ImageMagick
        run: |
          sudo apt-get update
          sudo apt-get install -y imagemagick

      - name: Install resvg
        run: cargo install resvg --locked

      - name: Regenerate icons
        working-directory: assets/icons
        run: make icons

      - name: Diff committed vs regenerated
        run: git diff --exit-code assets/icons/generated/

      - name: Validate icon set
        run: |
          test -f assets/icons/generated/hicolor/16x16/apps/io.github.wrzonance.HonkHonk.png
          test -f assets/icons/generated/hicolor/22x22/apps/io.github.wrzonance.HonkHonk.png
          test -f assets/icons/generated/hicolor/24x24/apps/io.github.wrzonance.HonkHonk.png
          test -f assets/icons/generated/hicolor/32x32/apps/io.github.wrzonance.HonkHonk.png
          test -f assets/icons/generated/hicolor/48x48/apps/io.github.wrzonance.HonkHonk.png
          test -f assets/icons/generated/hicolor/64x64/apps/io.github.wrzonance.HonkHonk.png
          test -f assets/icons/generated/hicolor/128x128/apps/io.github.wrzonance.HonkHonk.png
          test -f assets/icons/generated/hicolor/256x256/apps/io.github.wrzonance.HonkHonk.png
          test -f assets/icons/generated/hicolor/512x512/apps/io.github.wrzonance.HonkHonk.png
          test -f assets/icons/generated/hicolor/scalable/apps/io.github.wrzonance.HonkHonk.svg
          test -f assets/icons/generated/hicolor/symbolic/apps/io.github.wrzonance.HonkHonk-symbolic.svg
          test -f assets/icons/generated/favicon.ico
          test -f assets/icons/generated/social-preview.png
          file assets/icons/generated/favicon.ico | grep -q 'MS Windows icon'
          grep -q 'currentColor' assets/icons/generated/hicolor/symbolic/apps/io.github.wrzonance.HonkHonk-symbolic.svg
```

- [ ] **Step 2: Validate YAML syntax**

Run: `python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/icons.yml"))' && echo "YAML OK"`
Expected: `YAML OK`.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/icons.yml
git commit -m "ci: add icon pipeline freshness gate"
```

---

### Task 8: Write `assets/icons/README.md` runbook

**Files:**
- Create: `assets/icons/README.md`

- [ ] **Step 1: Write the file**

```markdown
# HonkHonk icons

This directory holds the **icon export pipeline**. Two SVG sources
(`icon.svg` color, `icon-symbolic.svg` monochrome) generate every
PNG / SVG / ICO that HonkHonk ships, via a single `make icons` invocation.

## Placeholder notice

`icon.svg` and `icon-symbolic.svg` are temporary geometric placeholders
("HH" mark). The real Krita-designed artwork lands in a follow-up PR
that simply swaps these two files and reruns `make icons` — the
pipeline itself does not change.

## Layout

```
assets/icons/
├── icon.svg              ← source (color, edit me)
├── icon-symbolic.svg     ← source (monochrome, edit me)
├── Makefile              ← `make icons` regenerates everything
└── generated/            ← committed outputs (CI verifies freshness)
    ├── hicolor/<size>/apps/io.github.wrzonance.HonkHonk.png
    ├── hicolor/scalable/apps/io.github.wrzonance.HonkHonk.svg
    ├── hicolor/symbolic/apps/io.github.wrzonance.HonkHonk-symbolic.svg
    ├── favicon.ico
    └── social-preview.png
```

App-id `io.github.wrzonance.HonkHonk` is the canonical Flathub
identifier — every filename in `generated/` uses it.

## Swap-real-art runbook

When the real artwork is ready:

1. Export new color SVG from Krita → overwrite `icon.svg`.
2. Export new symbolic SVG (single path, `fill="currentColor"`, no
   background, no decoration) → overwrite `icon-symbolic.svg`.
3. From `assets/icons/`, run `make clean && make icons`.
4. `git status` — every file under `generated/` will change. That is
   expected; review one or two PNGs visually then `git add generated/`.
5. Open a PR titled `feat(assets): replace placeholder icon with
   real artwork`. CI will verify the regenerated outputs match what
   you committed.

## Tooling install

The pipeline needs `resvg` (PNG rasterizer) and ImageMagick 7 (`magick`
binary for ICO assembly + social-card compositing).

```bash
# Arch / Manjaro
sudo pacman -S imagemagick
cargo install resvg --locked

# Fedora
sudo dnf install ImageMagick
cargo install resvg --locked
# See "ImageMagick policy note" below — ICO write is disabled by default.

# Ubuntu / Debian
sudo apt-get install imagemagick
cargo install resvg --locked
```

`resvg` is pinned to whatever the latest published crate version is when
you run `cargo install --locked` — the lockfile from the resvg release
keeps reproducibility within a given version.

## Symbolic icon requirement

`icon-symbolic.svg` MUST use `<path fill="currentColor">` (no hard-coded
fill colors, no background). DEs recolor symbolic icons to match the
active theme; a hard-coded fill breaks dark-mode recoloring.

## ImageMagick policy note (Fedora / RHEL)

Some distributions (notably Fedora and RHEL family) ship ImageMagick
with a security policy that disables ICO writes. If `make favicon`
fails with `attempt to perform an operation not allowed by the
security policy 'ICO'`, edit `/etc/ImageMagick-7/policy.xml` and
either remove the `<policy domain="coder" rights="none" pattern="ICO"/>`
line or change `rights="none"` to `rights="read|write"`.

The default Ubuntu CI runner does not need this workaround.

## Stale outputs

If you edit the Makefile or partially regenerate, run `make clean &&
make icons` for a guaranteed clean rebuild.
```

- [ ] **Step 2: Commit**

```bash
git add assets/icons/README.md
git commit -m "docs(assets): add icon pipeline runbook"
```

---

### Task 9: Append Icon section to repo `README.md`

**Files:**
- Modify: `README.md` (append a short section at the end)

- [ ] **Step 1: Read current end of README**

Run: `tail -5 README.md`
Expected: shows the last few lines so we can append cleanly.

- [ ] **Step 2: Append the Icon section**

Append exactly this block at the end of `README.md` (preserve a single blank line before it):

```markdown

## Icons

HonkHonk's icons are generated from two SVG sources via a small
`make`-driven pipeline. The current art is a placeholder geometric
"HH" mark — real Krita-designed artwork lands in a follow-up PR.

See [`assets/icons/README.md`](assets/icons/README.md) for:

- The swap-real-art runbook
- `resvg` + ImageMagick install hints (Arch / Fedora / Ubuntu)
- Why the symbolic SVG must use `fill="currentColor"`

CI enforces icon freshness via `.github/workflows/icons.yml`: every
push that touches `assets/icons/` regenerates outputs and fails if
the committed PNGs/ICO/SVGs drift from the sources.
```

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(readme): add icon pipeline pointer"
```

---

### Task 10: Local end-to-end smoke test

**Files:** none (read-only verification).

- [ ] **Step 1: Run a clean rebuild and confirm zero diff**

Run from `assets/icons/`:
```bash
PATH="$HOME/.cargo/bin:$PATH" make clean && PATH="$HOME/.cargo/bin:$PATH" make icons
```
Expected: every target built, no errors.

- [ ] **Step 2: Confirm git diff is empty over generated/**

Run from repo root: `git diff --exit-code assets/icons/generated/ && echo "NO DRIFT"`
Expected: `NO DRIFT`.

- [ ] **Step 3: Run the CI assertion block locally**

Run from repo root:
```bash
for s in 16 22 24 32 48 64 128 256 512; do
  test -f "assets/icons/generated/hicolor/${s}x${s}/apps/io.github.wrzonance.HonkHonk.png" || { echo "MISSING $s"; exit 1; }
done
test -f assets/icons/generated/hicolor/scalable/apps/io.github.wrzonance.HonkHonk.svg
test -f assets/icons/generated/hicolor/symbolic/apps/io.github.wrzonance.HonkHonk-symbolic.svg
test -f assets/icons/generated/favicon.ico
test -f assets/icons/generated/social-preview.png
file assets/icons/generated/favicon.ico | grep -q 'MS Windows icon' && echo "ICO format OK"
grep -q 'currentColor' assets/icons/generated/hicolor/symbolic/apps/io.github.wrzonance.HonkHonk-symbolic.svg && echo "currentColor OK"
echo "ALL ASSERTIONS PASSED"
```
Expected: `ICO format OK`, `currentColor OK`, `ALL ASSERTIONS PASSED`.

- [ ] **Step 4: Confirm legacy honkhonk.png untouched**

Run: `git log -1 -- assets/icons/hicolor/256x256/apps/honkhonk.png`
Expected: the last commit on that file predates this PR (no commits from this branch touched it).

- [ ] **Step 5: LOC sanity check (source files only — generated/ excluded)**

Run:
```bash
git diff --stat origin/main...HEAD -- \
  ':(exclude)assets/icons/generated' \
  ':(exclude)docs/superpowers' \
  | tail -1
```
Expected: total insertions well under 500 (target ~205 per spec).

No commit — verification only.

---

## Self-review checklist

- **Spec coverage:**
  - icon.svg / icon-symbolic.svg → Tasks 1, 2.
  - Makefile → Task 3.
  - generated/hicolor/{sizes,scalable,symbolic} → Task 4.
  - favicon.ico → Task 5.
  - social-preview.png → Task 6.
  - .github/workflows/icons.yml → Task 7.
  - assets/icons/README.md → Task 8.
  - README.md Icon section → Task 9.
  - End-to-end smoke + LOC gate → Task 10.
  - App-id `io.github.wrzonance.HonkHonk` used in every filename → ✓ (Tasks 3, 4, 7).
  - `currentColor` requirement → enforced in Task 2 step 2 + Task 7 grep + Task 10 step 3.
  - Existing legacy PNG untouched → confirmed Task 10 step 4.
- **No placeholders:** all file contents, all commands, all expected outputs are spelled out. No TBDs.
- **Type consistency:** app-id string identical everywhere (`io.github.wrzonance.HonkHonk`); Makefile variables (`SIZES`, `APPID`, `OUT`) referenced consistently in Tasks 3/4/7/10.
