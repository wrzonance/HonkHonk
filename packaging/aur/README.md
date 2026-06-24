# AUR packaging

This directory holds the AUR PKGBUILDs for HonkHonk. Each variant has its own
subdirectory and per-variant README:

- [`honkhonk/`](honkhonk/) — **source build (recommended).** Arch-native, compiled
  from the tagged source tarball. No foreign-soname workarounds. See
  [`honkhonk/README.md`](honkhonk/README.md) for the per-dependency justification.
- [`honkhonk-bin/`](honkhonk-bin/) — binary variant: re-extracts the upstream
  `.deb` from GitHub Releases. Kept as a convenience secondary (built on a Debian
  base; see the source README for why source is preferred).
- [`honkhonk-git/`](honkhonk-git/) — VCS variant tracking `main`.

## What CI validates

`.github/workflows/aur.yml` runs a **matrix over `honkhonk`, `honkhonk-bin`, and
`honkhonk-git`** on every push/PR touching `packaging/aur/**`,
`.github/workflows/aur.yml`, or `Cargo.toml`. For each package, in
`archlinux:base-devel`:

1. `namcap PKGBUILD` — catches missing fields, deprecated patterns, style issues.
2. `.SRCINFO` freshness — diffs the committed file against a fresh `makepkg --printsrcinfo`. Fails if you edited the PKGBUILD without regenerating.
3. `makepkg --noconfirm --syncdeps --install` — full build + install (source compile for `honkhonk`, `.deb` re-extract for `honkhonk-bin`).
4. `pacman -Ql <pkg>` — asserts `/usr/bin/honkhonk` and `/usr/share/applications/honkhonk.desktop` landed.

The source variant's per-release runbook lives in
[`honkhonk/README.md`](honkhonk/README.md). The `-bin` runbook is below. The
shared notes (AUR account setup, reserved CI SSH key, future auto-publish) apply
to all three packages.

## Per-release bump runbook (`honkhonk-bin`)

Run on an Arch / Manjaro / EndeavourOS host with `base-devel` and `pacman-contrib` installed.

```bash
cd packaging/aur/honkhonk-bin

# 1. Bump version fields in PKGBUILD
#    Stable releases use bare tags, so pkgver == _pkgtag == 0.1.0.
#    For prereleases, pkgver uses dots (0.2.0.rc.1 — AUR forbids '-' in
#    pkgver) while _pkgtag keeps the dashes of the GitHub tag (0.2.0-rc.1).
$EDITOR PKGBUILD

# 2. Populate sha256sums from the live release URL
updpkgsums

# 3. Regenerate .SRCINFO
makepkg --printsrcinfo > .SRCINFO

# 4. Lint
namcap PKGBUILD

# 5. Smoke test locally
makepkg --noconfirm --syncdeps --install
honkhonk --version    # or just launch on a Wayland session
sudo pacman -Rns honkhonk-bin

# 6. Push to the AUR repo (separate clone)
git clone ssh://aur@aur.archlinux.org/honkhonk-bin.git /tmp/aur-honkhonk-bin
cp PKGBUILD .SRCINFO /tmp/aur-honkhonk-bin/
cd /tmp/aur-honkhonk-bin
git add PKGBUILD .SRCINFO
git commit -m "honkhonk-bin <new-version>"
git push origin master
```

## Rolling update runbook (`honkhonk-git`)

The VCS package usually needs no manual version bump: `pkgver()` derives the
installed version from `git describe` against `main`.

```bash
cd packaging/aur/honkhonk-git

# 1. Regenerate .SRCINFO after changing the PKGBUILD
makepkg --printsrcinfo > .SRCINFO

# 2. Lint
namcap PKGBUILD

# 3. Smoke test locally (compiles current main)
makepkg --noconfirm --syncdeps --install
honkhonk --version    # or launch on a Wayland session
sudo pacman -Rns honkhonk-git

# 4. Push to the AUR repo (separate clone)
git clone ssh://aur@aur.archlinux.org/honkhonk-git.git /tmp/aur-honkhonk-git
cp PKGBUILD .SRCINFO /tmp/aur-honkhonk-git/
cd /tmp/aur-honkhonk-git
git add PKGBUILD .SRCINFO
git commit -m "honkhonk-git update"
git push origin master
```

## First-time AUR account setup

1. Register at https://aur.archlinux.org/register
2. Add your SSH public key under Account Details
3. Verify access: `ssh aur@aur.archlinux.org`
4. Clone an empty package repo to bootstrap: `git clone ssh://aur@aur.archlinux.org/honkhonk-bin.git`

See https://wiki.archlinux.org/title/AUR_submission_guidelines for the full reference.

## Reserved CI SSH key (auto-publish — future PR)

The follow-up auto-publish PR (separate from this sub-MVP) will store the private half of this keypair in the `AUR_SSH_KEY` GitHub Secret. Public key reserved here so the AUR account can be pre-authorized:

```
ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOz4BBwATZ0HhlGlZvpx2DXSF2mqoGc9Xqg7zAJAQiaH honkhonk-ci@github-actions
```

When auto-publish lands:
- Workflow trigger: `release: { types: [published] }`
- Steps: bump pkgver, run `updpkgsums`, regen `.SRCINFO`, commit to the AUR clone, push via SSH using `AUR_SSH_KEY`.

## Package strategy

`honkhonk` (source) is the **primary** package: an Arch-native build compiled on
the user's machine from the tagged source release. `honkhonk-bin` re-extracts the
upstream `.deb` from GitHub Releases and is kept as a convenience secondary for
users who want a fast install. `honkhonk-git` is available for users who want
current `main` before the next tagged release.
