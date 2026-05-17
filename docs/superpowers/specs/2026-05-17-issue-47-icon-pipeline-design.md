# Design — Issue #47 sub-MVP: icon pipeline (placeholder art)

**Date:** 2026-05-17
**Branch:** `feat/issue-47-icon-pipeline`
**Closes:** part of #47 (pipeline + placeholder art; real Krita art lands in follow-up PR)

## Goal

Ship the full automated icon-export pipeline (Makefile, generated outputs, CI freshness gate) using a temporary placeholder SVG. Real Krita-designed art lands later by swapping two SVG files and re-running `make icons` — pipeline stays unchanged. All generated outputs use the canonical `io.github.wrzonance.HonkHonk` app-id naming required by Flathub.

## Scope

### In

| File / dir | Purpose | LOC est. |
|---|---|---|
| `assets/icons/icon.svg` | Placeholder full-color source, 1024×1024 viewBox, stylized "HH" mark | ~40 |
| `assets/icons/icon-symbolic.svg` | Placeholder monochrome source, `currentColor` fill, single path | ~25 |
| `assets/icons/Makefile` | `make icons` target — resvg rasterization + ImageMagick ICO assembly + social preview | ~50 |
| `assets/icons/generated/hicolor/{16,22,24,32,48,64,128,256,512}x{N}/apps/io.github.wrzonance.HonkHonk.png` | PNG export per size | binary |
| `assets/icons/generated/hicolor/scalable/apps/io.github.wrzonance.HonkHonk.svg` | Scalable copy of source | binary |
| `assets/icons/generated/hicolor/symbolic/apps/io.github.wrzonance.HonkHonk-symbolic.svg` | Symbolic copy | binary |
| `assets/icons/generated/favicon.ico` | Multi-res ICO (16/32/48/64/128/256) | binary |
| `assets/icons/generated/social-preview.png` | 1280×640 GitHub social card | binary |
| `.github/workflows/icons.yml` | Drift gate — `make icons && git diff --exit-code` | ~50 |
| `assets/icons/README.md` | Swap-real-art runbook + tooling install hints | ~30 |
| `README.md` | Icon attribution + how-to-replace section | ~10 |

**Total source LOC: ~205.** Binary outputs do not count toward the CLAUDE.md 500 LOC ceiling.

### Out (explicit — separate future PRs)

- Real Krita-designed icon artwork (user-driven follow-up; swap 2 SVGs + `make icons` + PR)
- AppImage migration from `honkhonk.png` → `io.github.wrzonance.HonkHonk.png` filename (paired with `.desktop` `Icon=` update)
- Flatpak manifest icon-path rename (covered by #84 Flathub sub-MVP)
- Removal of legacy `assets/icons/hicolor/256x256/apps/honkhonk.png` (deferred until packaging configs migrated; left in place to keep AppImage building)
- `.desktop` file `Icon=` field update
- 22x22 / 24x24 visual quality review (real-icon PR concern)
- Animated icon variants

## Architecture

### Directory layout

```
assets/icons/
├── icon.svg                            # placeholder source (color)
├── icon-symbolic.svg                   # placeholder source (monochrome)
├── Makefile
├── README.md                           # swap-real-art runbook
├── hicolor/256x256/apps/honkhonk.png   # EXISTING — untouched
└── generated/                          # committed; CI verifies freshness
    ├── hicolor/
    │   ├── 16x16/apps/io.github.wrzonance.HonkHonk.png
    │   ├── 22x22/apps/io.github.wrzonance.HonkHonk.png
    │   ├── 24x24/apps/io.github.wrzonance.HonkHonk.png
    │   ├── 32x32/apps/io.github.wrzonance.HonkHonk.png
    │   ├── 48x48/apps/io.github.wrzonance.HonkHonk.png
    │   ├── 64x64/apps/io.github.wrzonance.HonkHonk.png
    │   ├── 128x128/apps/io.github.wrzonance.HonkHonk.png
    │   ├── 256x256/apps/io.github.wrzonance.HonkHonk.png
    │   ├── 512x512/apps/io.github.wrzonance.HonkHonk.png
    │   ├── scalable/apps/io.github.wrzonance.HonkHonk.svg
    │   └── symbolic/apps/io.github.wrzonance.HonkHonk-symbolic.svg
    ├── favicon.ico
    └── social-preview.png
```

### `assets/icons/Makefile`

```makefile
SIZES        = 16 22 24 32 48 64 128 256 512
APPID        = io.github.wrzonance.HonkHonk
SVG_COLOR    = icon.svg
SVG_SYMBOLIC = icon-symbolic.svg
OUT          = generated

.PHONY: icons hicolor symbolic favicon social clean check-tools

icons: check-tools hicolor symbolic favicon social

check-tools:
	@command -v resvg >/dev/null || { echo "ERROR: resvg missing — cargo install resvg --locked"; exit 1; }
	@command -v magick >/dev/null || { echo "ERROR: ImageMagick missing"; exit 1; }

hicolor: $(SIZES:%=$(OUT)/hicolor/%x%/apps/$(APPID).png) \
         $(OUT)/hicolor/scalable/apps/$(APPID).svg

$(OUT)/hicolor/%x%/apps/$(APPID).png: $(SVG_COLOR)
	@mkdir -p $(dir $@)
	resvg --width $* --height $* $< $@

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
	resvg --width 256 --height 256 $< $(OUT)/favicon-256.png
	magick $(OUT)/favicon-256.png -background none \
	  -define icon:auto-resize="256,128,64,48,32,16" $@
	rm $(OUT)/favicon-256.png

social: $(OUT)/social-preview.png
$(OUT)/social-preview.png: $(SVG_COLOR)
	@mkdir -p $(OUT)
	resvg --width 512 --height 512 $< $(OUT)/social-icon.png
	magick -size 1280x640 xc:'#1a1a2e' \
	  $(OUT)/social-icon.png -gravity center -composite \
	  -font DejaVu-Sans-Bold -pointsize 96 -fill white \
	  -gravity south -annotate +0+80 'HonkHonk' $@
	rm $(OUT)/social-icon.png

clean:
	rm -rf $(OUT)
```

### Placeholder SVGs

**`icon.svg`** — 1024×1024 viewBox; rounded-square background (`#1a1a2e`) with stylized "HH" geometric mark (`#f7d34a`). All paths use relative stroke widths so 16×16 downscale remains legible. Content within 80% safe area (~102px inset). Single root `<svg>` element, no embedded raster, no fonts.

**`icon-symbolic.svg`** — same outline as color icon but single `<path fill="currentColor">`, no background, no decoration. Tested in `make` against both light and dark DE themes by manual visual smoke at merge time.

### `.github/workflows/icons.yml`

```yaml
name: Icon pipeline freshness
on:
  push:
    paths: ['assets/icons/**', '.github/workflows/icons.yml']
  pull_request:
    paths: ['assets/icons/**', '.github/workflows/icons.yml']

jobs:
  freshness:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install resvg + ImageMagick
        run: |
          sudo apt-get update
          sudo apt-get install -y imagemagick
          cargo install resvg --locked
      - name: Regenerate icons
        working-directory: assets/icons
        run: make icons
      - name: Diff committed vs regenerated
        run: git diff --exit-code assets/icons/generated/
      - name: Validate icon set
        run: |
          test -f assets/icons/generated/hicolor/16x16/apps/io.github.wrzonance.HonkHonk.png
          test -f assets/icons/generated/hicolor/symbolic/apps/io.github.wrzonance.HonkHonk-symbolic.svg
          test -f assets/icons/generated/favicon.ico
          file assets/icons/generated/favicon.ico | grep -q 'MS Windows icon'
```

### `assets/icons/README.md`

Captures:
- Placeholder notice: "Current `icon.svg` / `icon-symbolic.svg` are temporary geometric placeholders. Real Krita-designed art replaces them in a future PR."
- Swap-real-art runbook (5 steps): export from Krita → save → `make icons` → diff → PR.
- Tooling install (Arch / Fedora / Ubuntu).
- ImageMagick policy.xml note for Fedora/RHEL users (ICO write may need policy edit).
- `currentColor` requirement for symbolic.

### `README.md` addition

Short "Icon" section at end of repo README explaining the pipeline, the placeholder status, and pointing readers to `assets/icons/README.md` for contribution flow.

## Testing

### Automated (CI)

| Check | What it catches |
|---|---|
| `cargo install resvg --locked` | Toolchain availability + version pin |
| `make icons` exits 0 | All targets build with installed tools |
| `git diff --exit-code assets/icons/generated/` | Committed outputs match source SVGs (drift gate) |
| File-existence tests for key outputs | Makefile rules accidentally skipped |
| `file favicon.ico | grep 'MS Windows icon'` | Multi-res ICO format correct |

### Manual smoke (post-merge)

1. `cp -r assets/icons/generated/hicolor ~/.local/share/icons/hicolor && gtk-update-icon-cache ~/.local/share/icons/hicolor`
2. Open `gnome-tweaks` → Icon picker → verify `HonkHonk` appears at 16/22/24/32/48
3. Toggle DE theme light ↔ dark → verify symbolic icon recolors
4. Open `assets/icons/generated/favicon.ico` in Firefox + Chrome — all res tiers render
5. View `assets/icons/generated/social-preview.png` — confirm 1280×640, readable "HonkHonk" text

### Out of test scope

- Subjective rasterization quality at 16×16 (real-icon PR concern)
- Cross-DE tray icon rendering (tray-icon crate responsibility)
- Real-icon visual approval (gated by user on swap PR)

## Error handling + edge cases

- **`resvg` / `magick` not installed**: `check-tools` target fails fast with install hint.
- **ImageMagick ICO policy disabled**: documented in `assets/icons/README.md` with policy-edit one-liner for Fedora/RHEL. CI uses default-policy Ubuntu image.
- **Stale partial outputs after Makefile edit**: runbook documents `make clean && make icons` for full regen.
- **resvg version drift**: pinned via `cargo install resvg --locked` (uses `Cargo.lock` from latest resvg release crate). Recorded in `assets/icons/README.md`.
- **Real-icon swap diff is large**: expected — all PNGs regenerate. PR description should call out "binary-only diff under `generated/`".
- **Existing `assets/icons/hicolor/256x256/apps/honkhonk.png`** stays in place; AppImage references unchanged. Additive PR only.

## TDD ordering (writing-plans will expand)

1. RED: add `icons.yml` workflow first — fails because no Makefile / source SVGs exist.
2. GREEN: write minimal Makefile + placeholder `icon.svg` + `icon-symbolic.svg` to make targets pass.
3. RED: add file-existence assertions in CI — fails until generated/ tree committed.
4. GREEN: run `make icons` locally, commit outputs.
5. RED: add `file favicon.ico | grep 'MS Windows icon'` assertion — fails if ICO assembly broken.
6. GREEN: verify ImageMagick command produces correct multi-res ICO.
7. REFACTOR: extract repeated mkdir patterns in Makefile if cleaner.
8. Docs last: `assets/icons/README.md` + `README.md` icon section.

## References

- Issue #47: https://github.com/wrzonance/HonkHonk/issues/47
- [Freedesktop Icon Theme Spec](https://specifications.freedesktop.org/icon-theme/latest/)
- [Freedesktop Icon Naming Spec](https://specifications.freedesktop.org/icon-naming-spec/icon-naming-spec-latest.html)
- [Flathub MetaInfo Guidelines](https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines)
- [resvg](https://github.com/linebender/resvg)
- App-id rationale: aligned with #84 Flathub sub-MVP (`io.github.wrzonance.HonkHonk`)
- CLAUDE.md 500 LOC / sub-MVP rule
