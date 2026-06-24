# AUR packaging — `honkhonk` (source build)

This is the **recommended** AUR package: an Arch-native build compiled from the
tagged source release. No foreign-soname workarounds, no Debian artifacts.

Alternatives live alongside this directory:

- `honkhonk-bin` — re-extracts the upstream `.deb` from GitHub Releases. Kept as
  a convenience for users who want a fast install, but it is built on a Debian
  base, so the source package remains the Arch-native default.
- `honkhonk-git` — VCS variant tracking `main`.

## Source

`source=` points at GitHub's auto-generated tag tarball
(`.../archive/v<version>.tar.gz`). No custom release artifact is required — every
tagged release already exposes this tarball. `sha256sums=('SKIP')` because GitHub
regenerates tag tarballs (a pinned hash is fragile across alpha re-tags); integrity
is instead enforced by compiling and running the full test suite in `check()`.

## No committed `Cargo.lock`

Upstream gitignores `Cargo.lock`, so the tarball ships without a lockfile. The
PKGBUILD therefore runs `cargo fetch` in `prepare()` (which resolves dependencies
and writes a `Cargo.lock` into the extracted tree), then `cargo build --frozen` /
`cargo test --frozen` in `build()`/`check()` so the compile and tests run fully
offline against that freshly-pinned lockfile.

## Per-dependency justification

Every entry in `depends=()` is dynamically linked or dlopened by the tagged
binary this package builds. The current `pkgver=0.1.0` tag still uses the old
GTK3 tray backend; drop the GTK/appindicator entries when `_pkgtag` points at a
release that contains the pure-Rust `ksni` tray backend.

| Dependency                  | Why it is required (binary links / dlopens it)                          | Arch | Fedora | Ubuntu/Debian |
|-----------------------------|-------------------------------------------------------------------------|------|--------|---------------|
| `pipewire`                  | `libpipewire-0.3.so` — audio engine (via the `pipewire` crate / -sys)   | extra | Everything | main |
| `gtk3`                      | `libgtk-3.so` / `libgdk-3.so` — tray menu in the 0.1.0 tag              | extra | Everything | main |
| `libayatana-appindicator`   | `libayatana-appindicator3.so` — SNI tray in the 0.1.0 tag               | extra | Everything | main |
| `wayland`                   | `libwayland-client.so` — Iced/winit Wayland backend (dlopened)         | extra | Everything | main |
| `libxkbcommon`              | `libxkbcommon.so` — keyboard mapping (winit, dlopened)                 | extra | Everything | main |
| `xdg-desktop-portal`        | D-Bus service for file chooser + global shortcuts (`ashpd`, no link)    | extra | Everything | main |

The current development branch uses `ksni` (StatusNotifierItem over zbus), so
future release bumps should re-audit this table with `namcap` and `cargo tree`
and remove the GTK/appindicator entries once the package no longer builds the
0.1.0 tag.

## Per-release bump runbook

Run on an Arch / Manjaro / EndeavourOS host with `base-devel` and
`pacman-contrib` installed.

```bash
cd packaging/aur/honkhonk

# 1. Bump version fields in PKGBUILD
#    Stable releases use bare tags, so pkgver == _pkgtag == 0.1.0.
#    For prereleases, pkgver uses dots (0.2.0.rc.1 — AUR forbids '-' in
#    pkgver) while _pkgtag keeps the dashes of the GitHub tag (0.2.0-rc.1).
$EDITOR PKGBUILD

# 2. Regenerate .SRCINFO
makepkg --printsrcinfo > .SRCINFO

# 3. Lint
namcap PKGBUILD

# 4. Smoke test locally (compiles from the live tag tarball)
makepkg --noconfirm --syncdeps --install
honkhonk --version    # or launch on a Wayland session
sudo pacman -Rns honkhonk

# 5. Push to the AUR repo (separate clone)
git clone ssh://aur@aur.archlinux.org/honkhonk.git /tmp/aur-honkhonk
cp PKGBUILD .SRCINFO /tmp/aur-honkhonk/
cd /tmp/aur-honkhonk
git add PKGBUILD .SRCINFO
git commit -m "honkhonk <new-version>"
git push origin master
```

## What CI validates

`.github/workflows/aur.yml` runs a matrix over `honkhonk`, `honkhonk-bin`, and
`honkhonk-git` on every push/PR touching `packaging/aur/**`,
`.github/workflows/aur.yml`, or `Cargo.toml`. For this package it runs, in
`archlinux:base-devel`:

1. `namcap PKGBUILD` — style / missing-field lint.
2. `.SRCINFO` freshness — diffs the committed file against a fresh
   `makepkg --printsrcinfo`. Fails if the PKGBUILD was edited without regenerating.
3. `makepkg --noconfirm --syncdeps --install` — full source compile + install.
4. `pacman -Ql honkhonk` — asserts `/usr/bin/honkhonk` and the `.desktop` landed.
