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
5. Open a PR titled `feat(assets): replace placeholder icon with real
   artwork`. CI will verify the regenerated outputs match what you
   committed.

## Tooling install

The pipeline needs `resvg` (PNG rasterizer) and ImageMagick's `convert`
binary (for ICO assembly + social-card compositing). The default Ubuntu
`imagemagick` apt package is IM6 and ships `convert`; Arch's IM7 ships
both `magick` and a `convert` compatibility wrapper, so the same
`Makefile` works on every distro.

```bash
# Arch / Manjaro
sudo pacman -S imagemagick
cargo install resvg --locked

# Fedora
sudo dnf install ImageMagick
cargo install resvg --locked
# See "ImageMagick policy note" below — ICO write is disabled by default.

# Ubuntu / Debian
sudo apt-get install imagemagick fonts-dejavu-core
cargo install resvg --locked
```

`resvg` is pinned to whatever version `cargo install --locked` resolves
when you run it (the publisher's `Cargo.lock` from that crate release
keeps the dependency graph reproducible within a given version).

## Symbolic icon requirement

`icon-symbolic.svg` MUST use `<path fill="currentColor">` (no hard-coded
fill colors, no background). Desktop environments recolor symbolic icons
to match the active theme; a hard-coded fill breaks dark-mode recoloring.

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
