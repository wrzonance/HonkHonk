# AUR packaging — `honkhonk-bin`

This directory holds the AUR PKGBUILD for the **binary** variant. Source (`honkhonk`) and VCS (`honkhonk-git`) variants are planned in follow-up PRs.

## What CI validates

`.github/workflows/aur.yml` runs on every push/PR touching `packaging/aur/**`:

1. `namcap PKGBUILD` — catches missing fields, deprecated patterns, style issues.
2. `.SRCINFO` freshness — diffs the committed file against a fresh `makepkg --printsrcinfo`. Fails if you edited the PKGBUILD without regenerating.
3. `updpkgsums` — refreshes `sha256sums` from the live release URL. Fails fast on 404 / hash mismatch.
4. `makepkg --noconfirm --syncdeps --install` — full build + install in `archlinux:base-devel`.
5. `pacman -Ql honkhonk-bin` — asserts `/usr/bin/honkhonk` and `/usr/share/applications/honkhonk.desktop` landed.

## Per-release bump runbook

Run on an Arch / Manjaro / EndeavourOS host with `base-devel` and `pacman-contrib` installed.

```bash
cd packaging/aur/honkhonk-bin

# 1. Bump version fields in PKGBUILD
#    pkgver uses dots:   0.1.0.alpha.1  (AUR forbids '-' in pkgver)
#    _pkgtag uses dashes: v0.1.0-alpha.1 (matches GitHub release tag)
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

## Why `-bin` first?

Smallest, lowest-risk channel. Zero changes to release workflow — reuses the existing `.deb` artifact already attached to every tagged release. Source and VCS variants follow in their own PRs.
