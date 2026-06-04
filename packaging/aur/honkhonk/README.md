# AUR packaging — `honkhonk` (source build)

This is the **recommended** AUR package: an Arch-native build compiled from the
tagged source release. No foreign-soname workarounds, no Debian artifacts.

Alternatives live alongside this directory:

- `honkhonk-bin` — re-extracts the upstream `.deb` from GitHub Releases. Kept as
  a convenience for users who want a fast install, but it is built on a Debian
  base (see the libxdo note below for why source is preferred).
- `honkhonk-git` — VCS variant tracking `main` (planned, separate PR).

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

Every entry in `depends=()` is dynamically linked or dlopened by the binary. The
set was audited against issue #98 — anything not actually linked was dropped.

| Dependency                  | Why it is required (binary links / dlopens it)                          | Arch | Fedora | Ubuntu/Debian |
|-----------------------------|-------------------------------------------------------------------------|------|--------|---------------|
| `pipewire`                  | `libpipewire-0.3.so` — audio engine (via the `pipewire` crate / -sys)   | extra | Everything | main |
| `gtk3`                      | `libgtk-3.so` / `libgdk-3.so` — tray menu (`muda`/`tray-icon` `gtk`)    | extra | Everything | main |
| `xdotool`                   | `libxdo.so` — `muda` `libxdo` feature (released tag only — see below)    | extra | Everything | main/universe |
| `libayatana-appindicator`   | `libayatana-appindicator3.so` — SNI tray (dlopened via `libappindicator`)| extra | Everything | universe |
| `wayland`                   | `libwayland-client.so` — Iced/winit Wayland backend (dlopened)         | extra | Everything | main |
| `libxkbcommon`              | `libxkbcommon.so` — keyboard mapping (winit, dlopened)                 | extra | Everything | main |
| `xdg-desktop-portal`        | D-Bus service for file chooser + global shortcuts (`ashpd`, no link)    | extra | Everything | main |

`pipewire` and `gtk3` pull `glib2` / `gdk-pixbuf2` transitively (namcap reports
those as implicitly satisfied), so they are not listed explicitly.

### The `xdotool` / `libxdo` situation

`tray-icon`'s and `muda`'s default Cargo features enable `libxdo`, which links
`libxdo.so` to synthesize X11 keystrokes for the predefined Copy/Cut/Paste/SelectAll
menu items. HonkHonk uses none of those (its tray menu is custom `MenuItem`s plus a
separator) and is Wayland-only per `CLAUDE.md`, so the X11 path is dead weight.

The fix on `main` disables those features:

```toml
tray-icon = { version = "0.24", default-features = false, features = ["gtk"] }
muda     = { version = "0.19", default-features = false, features = ["gtk"] }
```

Both edges must opt out — HonkHonk depends on `muda` directly *and* transitively
(via `tray-icon`), and Cargo feature unification means `muda`'s default features
(which include `libxdo`) win if *either* edge requests them. With both disabled,
`cargo tree -i libxdo` is empty and `readelf -d` shows no `libxdo.so` in `NEEDED`.

**But this package builds the released tag (`$_pkgtag`), not `main`.** The current
tag `v0.1.0-alpha.1` predates the fix, so the binary it produces *still links
`libxdo.so`* — verified by `namcap` on the built package
(`Dependency xdotool detected and not included`). Therefore `xdotool` is declared
in `depends` for now. **When a release containing the libxdo fix is tagged, bump
`_pkgtag` past it and drop `xdotool`.** At that point this source package also
sidesteps the Debian-vs-Arch `libxdo.so.3` / `libxdo.so.4` soname mismatch that
blocked `honkhonk-bin` on a clean Arch base.

## Per-release bump runbook

Run on an Arch / Manjaro / EndeavourOS host with `base-devel` and
`pacman-contrib` installed.

```bash
cd packaging/aur/honkhonk

# 1. Bump version fields in PKGBUILD
#    pkgver uses dots:    0.1.0.alpha.1  (AUR forbids '-' in pkgver)
#    _pkgtag uses dashes: v0.1.0-alpha.1 (matches the GitHub release tag)
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

`.github/workflows/aur.yml` runs a matrix over `honkhonk-bin` and `honkhonk` on
every push/PR touching `packaging/aur/**`, `.github/workflows/aur.yml`, or
`Cargo.toml`. For this package it runs, in `archlinux:base-devel`:

1. `namcap PKGBUILD` — style / missing-field lint.
2. `.SRCINFO` freshness — diffs the committed file against a fresh
   `makepkg --printsrcinfo`. Fails if the PKGBUILD was edited without regenerating.
3. `makepkg --noconfirm --syncdeps --install` — full source compile + install.
4. `pacman -Ql honkhonk` — asserts `/usr/bin/honkhonk` and the `.desktop` landed.
