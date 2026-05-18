# Design — Issue #84 sub-MVP: AUR `honkhonk-bin`

**Date:** 2026-05-17
**Branch:** `feat/issue-84-aur-bin`
**Closes:** part of #84 (AUR section — `honkhonk-bin` variant only)

## Goal

Ship the smallest, lowest-risk first packaging channel for #84: an AUR `honkhonk-bin` package that pulls the existing GitHub Release `.deb` artifact and installs the binary + .desktop on Arch / Manjaro / EndeavourOS systems. Provide a foothold for follow-up PRs covering `honkhonk` (source), `honkhonk-git` (VCS), AUR auto-publish, Flathub finishing, and signed apt/rpm repos.

## Scope

### In

| File | Purpose | LOC est. |
|---|---|---|
| `packaging/aur/honkhonk-bin/PKGBUILD` | Pull `.deb` from GH Releases, extract via `bsdtar`, install binary + .desktop | ~45 |
| `packaging/aur/honkhonk-bin/.SRCINFO` | Generated from PKGBUILD via `makepkg --printsrcinfo`, committed | ~25 |
| `.github/workflows/aur.yml` | `namcap` lint + `.SRCINFO` freshness diff + `makepkg --syncdeps --install` test build in `archlinux:base-devel` container | ~60 |
| `packaging/aur/README.md` | First-push + per-release bump runbook; records CI SSH pubkey for follow-up auto-publish PR | ~30 |
| `README.md` | Append "Arch Linux (AUR)" install section | ~10 |

**Total: ~170 LOC.** Under CLAUDE.md 500 LOC ceiling.

### Out (explicit — separate future PRs)

- AUR auto-publish workflow on release tag (SSH key + AUR account already prepared)
- `honkhonk` source PKGBUILD (compiles from tarball)
- `honkhonk-git` VCS PKGBUILD (clones `main`, auto-versions)
- Flathub rename `io.github.thewrz.*` → `io.github.wrzonance.*` + metainfo finish + submission
- Signed apt/rpm repos via GitHub Pages (GPG, reprepro, createrepo_c)
- Nix flake / Snap / winget
- aarch64 / non-x86_64 architectures

## Architecture

### `packaging/aur/honkhonk-bin/PKGBUILD`

```bash
# Maintainer: thewrz <adam@wrze.ski>
# GPG: B514 CBC5 B44C AACF 02EA  0D68 B461 236C F8EA 7961

pkgname=honkhonk-bin
pkgver=0.1.0.alpha.1
_pkgtag=v0.1.0-alpha.1   # GitHub tag — bumped manually with pkgver each release
pkgrel=1
pkgdesc="Wayland-native Linux soundboard — Iced GUI + PipeWire audio (binary release)"
arch=('x86_64')
url="https://github.com/wrzonance/HonkHonk"
license=('MIT')
depends=('pipewire' 'wayland' 'libxkbcommon')
provides=('honkhonk')
conflicts=('honkhonk' 'honkhonk-git')
source=("$pkgname-$pkgver.deb::$url/releases/download/$_pkgtag/honkhonk_${pkgver}-1_amd64.deb")
sha256sums=('SKIP')   # populated via updpkgsums per release; documented in README

prepare() {
    bsdtar -xf "$pkgname-$pkgver.deb" -C "$srcdir"
    bsdtar -xf "$srcdir/data.tar"* -C "$srcdir"
}

package() {
    install -Dm755 "$srcdir/usr/bin/honkhonk" "$pkgdir/usr/bin/honkhonk"
    install -Dm644 "$srcdir/usr/share/applications/honkhonk.desktop" \
        "$pkgdir/usr/share/applications/honkhonk.desktop"
}
```

**Key choices:**
- **`.deb` extraction** vs new tarball asset: zero CI/release-workflow changes. Standard AUR `-bin` pattern (e.g. `slack-desktop`, `vscode-bin`).
- **`pkgver` dots, `_pkgtag` dashes:** AUR `pkgver` rules forbid `-`. GitHub tag `v0.1.0-alpha.1` → PKGBUILD `pkgver=0.1.0.alpha.1`. Manual sync per release.
- **`provides=('honkhonk')` + `conflicts=('honkhonk' 'honkhonk-git')`:** allows future source/VCS variants to coexist on AUR without colliding on `/usr/bin/honkhonk`.
- **`sha256sums=('SKIP')` for first PR:** documented; per-release runbook step is `updpkgsums` before push. Hash computation will move into auto-publish CI in follow-up PR.

### `.github/workflows/aur.yml`

Triggers: push or PR touching `packaging/aur/**` or `.github/workflows/aur.yml`.

```yaml
name: AUR PKGBUILD validation
on:
  push:
    paths: ['packaging/aur/**', '.github/workflows/aur.yml']
  pull_request:
    paths: ['packaging/aur/**', '.github/workflows/aur.yml']

jobs:
  validate:
    runs-on: ubuntu-latest
    container:
      image: archlinux:base-devel
    steps:
      - uses: actions/checkout@v4
      - name: Install validation tooling
        run: pacman -Sy --noconfirm namcap pacman-contrib git
      - name: Create non-root build user
        run: |
          useradd -m builder
          echo 'builder ALL=(ALL) NOPASSWD: ALL' >> /etc/sudoers
          chown -R builder packaging/aur/honkhonk-bin
      - name: namcap lint
        working-directory: packaging/aur/honkhonk-bin
        run: sudo -u builder namcap PKGBUILD
      - name: .SRCINFO freshness check
        working-directory: packaging/aur/honkhonk-bin
        run: |
          sudo -u builder makepkg --printsrcinfo > /tmp/.SRCINFO.fresh
          diff -u .SRCINFO /tmp/.SRCINFO.fresh
      - name: Refresh sha256sums + build
        working-directory: packaging/aur/honkhonk-bin
        run: |
          sudo -u builder updpkgsums
          sudo -u builder makepkg --noconfirm --syncdeps --install
      - name: Installed file manifest
        run: pacman -Ql honkhonk-bin
```

**Why these steps:**
- `namcap` catches missing fields, bad arrays, deprecated patterns.
- `.SRCINFO` diff prevents PKGBUILD edits from landing without regenerating metadata.
- `updpkgsums` exercises the network fetch — broken release URL fails CI.
- `makepkg --install` is the real test: extraction works, install paths correct.
- `pacman -Ql` confirms expected files landed.

### `packaging/aur/README.md`

Captures:
- One-time AUR account/SSH setup (link to AUR docs)
- Per-release bump runbook:
  1. Bump `pkgver` (dots) and `_pkgtag` (dashes) in PKGBUILD
  2. `updpkgsums` to populate `sha256sums`
  3. `makepkg --printsrcinfo > .SRCINFO`
  4. `namcap PKGBUILD` clean
  5. `makepkg -si` local smoke
  6. Copy PKGBUILD + .SRCINFO to AUR clone, commit, `git push aur master`
- Reserved CI SSH pubkey for follow-up auto-publish PR:
  ```
  ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOz4BBwATZ0HhlGlZvpx2DXSF2mqoGc9Xqg7zAJAQiaH honkhonk-ci@github-actions
  ```
  Private half will live in `AUR_SSH_KEY` GitHub Secret when auto-publish work begins.

### `README.md` addition

```markdown
### Arch Linux (AUR)

```bash
yay -S honkhonk-bin    # or paru -S honkhonk-bin
```

Pre-built binary from GitHub Releases. Source build (`honkhonk`) and VCS (`honkhonk-git`) variants coming soon.
```

## Testing

### Automated (CI)

| Check | What it catches |
|---|---|
| `namcap PKGBUILD` | Missing fields, deprecated patterns, style |
| `.SRCINFO` diff vs `makepkg --printsrcinfo` | Metadata drift after PKGBUILD edits |
| `updpkgsums` | Release URL unreachable / hash mismatch |
| `makepkg --noconfirm --syncdeps --install` | Build/install path bugs |
| `pacman -Ql honkhonk-bin` | Files landed at expected paths |

### Manual smoke (post-merge, pre-AUR-push)

1. `git clone ssh://aur@aur.archlinux.org/honkhonk-bin.git /tmp/aur-honkhonk-bin`
2. Copy `packaging/aur/honkhonk-bin/{PKGBUILD,.SRCINFO}` into `/tmp/aur-honkhonk-bin/`
3. `cd /tmp/aur-honkhonk-bin && namcap PKGBUILD && makepkg -si`
4. Launch `honkhonk` on real Wayland session — verify window opens, tray icon appears
5. `git add PKGBUILD .SRCINFO && git commit -m "init: honkhonk-bin 0.1.0.alpha.1" && git push origin master`
6. Verify package at `https://aur.archlinux.org/packages/honkhonk-bin`

### Out of test scope

- Cross-arch (aarch64) — `.deb` is amd64-only
- Source-compile flow — separate PR ships `honkhonk` PKGBUILD
- Auto-publish flow — separate PR

## Error handling + edge cases

- **`.deb` URL 404:** `makepkg` fetch error surfaces with full URL. CI fails fast.
- **`sha256sums=('SKIP')` in first PR:** deliberate; documented in `packaging/aur/README.md`. Per-release runbook step is `updpkgsums`. Auto-publish PR will compute hashes from CI artifacts.
- **`.SRCINFO` drift:** CI diff gate blocks merge.
- **`makepkg` refuses root:** workflow creates `builder` user with `NOPASSWD: ALL` — standard `archlinux:base-devel` pattern.
- **`pipewire` runtime missing in CI:** install succeeds; binary cannot start (no audio server). Test asserts on `pacman -Ql` output, not runtime — separates packaging correctness from runtime requirements.
- **GitHub Release missing the `.deb`:** caught by `updpkgsums` step; CI fails with HTTP error before merge.

## TDD ordering (writing-plans will expand)

1. RED: add `aur.yml` workflow first with `makepkg -si` step — fails until PKGBUILD exists.
2. GREEN: write minimal PKGBUILD that lints + builds + installs.
3. RED: add `.SRCINFO` diff step — fails until `.SRCINFO` committed.
4. GREEN: commit generated `.SRCINFO`.
5. RED: add `pacman -Ql honkhonk-bin` assertion with explicit file list — fails until install paths correct.
6. GREEN: verify package() install paths match assertion.
7. REFACTOR: extract repeated `sudo -u builder` into job-level setting if cleaner.
8. Docs last: `packaging/aur/README.md` + `README.md` AUR section.

## References

- Issue #84: https://github.com/wrzonance/HonkHonk/issues/84
- AUR submission docs: https://wiki.archlinux.org/title/AUR_submission_guidelines
- `-bin` package convention: https://wiki.archlinux.org/title/Arch_package_guidelines#Package_etiquette
- Existing `.deb` build: `.github/workflows/deb.yml`, `[package.metadata.deb]` in `Cargo.toml`
- CLAUDE.md 500 LOC / sub-MVP rule
