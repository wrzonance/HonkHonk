# AUR `honkhonk-bin` Sub-MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a CI-validated AUR `honkhonk-bin` PKGBUILD that pulls the existing GitHub Release `.deb` and installs the binary + .desktop on Arch-based distros.

**Architecture:** Standard AUR `-bin` pattern — `bsdtar`-extract the upstream `.deb`, install the binary and desktop entry. CI runs in `archlinux:base-devel` container: `namcap` lints the PKGBUILD, a diff check enforces `.SRCINFO` freshness, `updpkgsums` exercises the release-URL fetch, `makepkg --syncdeps --install` proves the install path, and `pacman -Ql` asserts file layout.

**Tech Stack:** bash PKGBUILD, GitHub Actions, `archlinux:base-devel` container, `namcap`, `pacman-contrib` (`updpkgsums`), `makepkg`.

**Scope guardrails (from spec):**
- IN: `packaging/aur/honkhonk-bin/{PKGBUILD,.SRCINFO}`, `.github/workflows/aur.yml`, `packaging/aur/README.md`, `README.md` AUR section.
- OUT: AUR auto-publish, `honkhonk` source PKGBUILD, `honkhonk-git` VCS PKGBUILD, Flathub rename, signed apt/rpm repos, Nix/Snap/winget, aarch64.

**Target release:** `v0.1.0-alpha.1` (already published — confirmed asset name `honkhonk_0.1.0.alpha.1-1_amd64.deb` at `https://github.com/wrzonance/HonkHonk/releases/download/v0.1.0-alpha.1/honkhonk_0.1.0.alpha.1-1_amd64.deb`).

**Maintainer header:** `thewrz <adam@wrze.ski>` with GPG fingerprint `B514 CBC5 B44C AACF 02EA  0D68 B461 236C F8EA 7961`.

**Reserved CI SSH pubkey (recorded for follow-up auto-publish PR):**
`ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOz4BBwATZ0HhlGlZvpx2DXSF2mqoGc9Xqg7zAJAQiaH honkhonk-ci@github-actions`

---

## File Structure

| File | Status | Purpose | Est. LOC |
|---|---|---|---|
| `.github/workflows/aur.yml` | Create | CI validation — namcap, `.SRCINFO` diff, `updpkgsums`, makepkg --install, `pacman -Ql` | ~60 |
| `packaging/aur/honkhonk-bin/PKGBUILD` | Create | Pull `.deb`, extract via `bsdtar`, install binary + .desktop | ~45 |
| `packaging/aur/honkhonk-bin/.SRCINFO` | Create (generated) | Committed output of `makepkg --printsrcinfo` | ~25 |
| `packaging/aur/README.md` | Create | First-push + per-release runbook, reserved CI pubkey | ~30 |
| `README.md` | Modify | Append "Arch Linux (AUR)" install section | ~10 |

**Total source LOC:** ~170 (under CLAUDE.md 500 LOC ceiling). `.SRCINFO` is generated and excluded from logical LOC budget.

---

## Task 1: Add the AUR validation workflow (RED — fails until PKGBUILD exists)

**Files:**
- Create: `.github/workflows/aur.yml`

- [ ] **Step 1: Write the workflow file**

```yaml
name: AUR PKGBUILD validation

on:
  push:
    paths:
      - 'packaging/aur/**'
      - '.github/workflows/aur.yml'
  pull_request:
    paths:
      - 'packaging/aur/**'
      - '.github/workflows/aur.yml'

permissions:
  contents: read

jobs:
  validate:
    name: namcap + .SRCINFO + makepkg
    runs-on: ubuntu-latest
    container:
      image: archlinux:base-devel
    steps:
      - uses: actions/checkout@v4

      - name: Install validation tooling
        run: |
          pacman -Sy --noconfirm --needed namcap pacman-contrib git

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

      - name: Refresh sha256sums + build + install
        working-directory: packaging/aur/honkhonk-bin
        run: |
          sudo -u builder updpkgsums
          sudo -u builder makepkg --noconfirm --syncdeps --install

      - name: Installed file manifest
        run: |
          pacman -Ql honkhonk-bin
          pacman -Ql honkhonk-bin | grep -q '/usr/bin/honkhonk$'
          pacman -Ql honkhonk-bin | grep -q '/usr/share/applications/honkhonk.desktop$'
```

- [ ] **Step 2: Lint the YAML locally**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/aur.yml'))"`
Expected: no output (valid YAML).

- [ ] **Step 3: Commit the workflow**

```bash
git add .github/workflows/aur.yml
git commit -m "ci(aur): add AUR PKGBUILD validation workflow

Runs namcap lint, .SRCINFO freshness diff, updpkgsums URL fetch, and
makepkg --syncdeps --install in archlinux:base-devel container.
Workflow expected to fail until PKGBUILD lands (Task 2)."
```

---

## Task 2: Write the `honkhonk-bin` PKGBUILD (GREEN — workflow passes namcap + makepkg)

**Files:**
- Create: `packaging/aur/honkhonk-bin/PKGBUILD`

- [ ] **Step 1: Write the PKGBUILD**

```bash
# Maintainer: thewrz <adam@wrze.ski>
# GPG: B514 CBC5 B44C AACF 02EA  0D68 B461 236C F8EA 7961

pkgname=honkhonk-bin
pkgver=0.1.0.alpha.1
_pkgtag=v0.1.0-alpha.1
pkgrel=1
pkgdesc="Wayland-native Linux soundboard — Iced GUI + PipeWire audio (binary release)"
arch=('x86_64')
url="https://github.com/wrzonance/HonkHonk"
license=('MIT')
depends=('pipewire' 'wayland' 'libxkbcommon')
provides=('honkhonk')
conflicts=('honkhonk' 'honkhonk-git')
source=("$pkgname-$pkgver.deb::$url/releases/download/$_pkgtag/honkhonk_${pkgver}-1_amd64.deb")
sha256sums=('SKIP')

prepare() {
    bsdtar -xf "$pkgname-$pkgver.deb" -C "$srcdir"
    bsdtar -xf "$srcdir"/data.tar* -C "$srcdir"
}

package() {
    install -Dm755 "$srcdir/usr/bin/honkhonk" "$pkgdir/usr/bin/honkhonk"
    install -Dm644 "$srcdir/usr/share/applications/honkhonk.desktop" \
        "$pkgdir/usr/share/applications/honkhonk.desktop"
    install -Dm644 "$srcdir/usr/share/icons/hicolor/256x256/apps/honkhonk.png" \
        "$pkgdir/usr/share/icons/hicolor/256x256/apps/honkhonk.png"
}
```

Note: spec listed only binary + .desktop in the `package()` body, but the upstream `.deb` (per `Cargo.toml [package.metadata.deb].assets`) also ships the 256x256 icon. Including it keeps the AUR package on par with the .deb — a `.desktop` entry without its referenced icon would silently fail in launchers.

- [ ] **Step 2: Verify bash syntax**

Run: `bash -n packaging/aur/honkhonk-bin/PKGBUILD`
Expected: no output (valid bash).

- [ ] **Step 3: Commit**

```bash
git add packaging/aur/honkhonk-bin/PKGBUILD
git commit -m "feat(packaging): add honkhonk-bin AUR PKGBUILD

Pulls the upstream .deb from GitHub Releases for v0.1.0-alpha.1,
extracts via bsdtar (standard AUR -bin pattern), installs binary,
.desktop, and 256x256 icon.

sha256sums=('SKIP') intentionally; CI runs updpkgsums to exercise
the release URL fetch. Per-release runbook (Task 4) requires
updpkgsums + .SRCINFO regen before push to AUR."
```

---

## Task 3: Generate and commit `.SRCINFO`

**Files:**
- Create: `packaging/aur/honkhonk-bin/.SRCINFO`

- [ ] **Step 1: Generate `.SRCINFO` from the PKGBUILD**

If `makepkg` is available locally:

```bash
cd packaging/aur/honkhonk-bin
makepkg --printsrcinfo > .SRCINFO
cd -
```

If `makepkg` is NOT available (non-Arch host), write `.SRCINFO` directly by hand mirroring the PKGBUILD. The CI `.SRCINFO` diff step (Task 1) is the authoritative check; producing it from a hand-written source is acceptable as long as it matches `makepkg --printsrcinfo` output byte-for-byte. Use this exact content:

```
pkgbase = honkhonk-bin
	pkgdesc = Wayland-native Linux soundboard — Iced GUI + PipeWire audio (binary release)
	pkgver = 0.1.0.alpha.1
	pkgrel = 1
	url = https://github.com/wrzonance/HonkHonk
	arch = x86_64
	license = MIT
	depends = pipewire
	depends = wayland
	depends = libxkbcommon
	provides = honkhonk
	conflicts = honkhonk
	conflicts = honkhonk-git
	source = honkhonk-bin-0.1.0.alpha.1.deb::https://github.com/wrzonance/HonkHonk/releases/download/v0.1.0-alpha.1/honkhonk_0.1.0.alpha.1-1_amd64.deb
	sha256sums = SKIP

pkgname = honkhonk-bin
```

(Indentation is **tabs**, not spaces — matches `makepkg --printsrcinfo` output exactly. One blank line between the `pkgbase` block and the `pkgname` line.)

- [ ] **Step 2: Verify file has tabs not spaces**

Run: `grep -P "^    " packaging/aur/honkhonk-bin/.SRCINFO | head`
Expected: no output (no space-indented lines).

Run: `grep -cP "^\t" packaging/aur/honkhonk-bin/.SRCINFO`
Expected: `13` (tab-indented metadata lines).

- [ ] **Step 3: Commit**

```bash
git add packaging/aur/honkhonk-bin/.SRCINFO
git commit -m "chore(packaging): commit generated .SRCINFO

Output of 'makepkg --printsrcinfo' against the PKGBUILD. CI enforces
freshness via diff against a freshly regenerated copy."
```

---

## Task 4: Add the AUR runbook (`packaging/aur/README.md`)

**Files:**
- Create: `packaging/aur/README.md`

- [ ] **Step 1: Write the runbook**

````markdown
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
````

- [ ] **Step 2: Verify the file is markdown-parseable**

Run: `head -1 packaging/aur/README.md`
Expected: `# AUR packaging — \`honkhonk-bin\``

- [ ] **Step 3: Commit**

```bash
git add packaging/aur/README.md
git commit -m "docs(packaging): add AUR runbook + reserved CI pubkey

Documents per-release bump steps, first-time AUR account setup,
and reserves the ed25519 pubkey for the follow-up auto-publish PR."
```

---

## Task 5: Append AUR install section to root `README.md`

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Verify current README has no AUR install section yet**

Run: `grep -c "AUR" README.md`
Expected: `1` (only the existing "Distro-friendly" bullet at line 19).

- [ ] **Step 2: Add the install section before the `## Building` heading**

Use Edit to insert the new section. Find the exact text:

```markdown
See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design and roadmap.

## Building
```

Replace with:

```markdown
See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design and roadmap.

## Installing

### Arch Linux (AUR)

```bash
yay -S honkhonk-bin    # or: paru -S honkhonk-bin
```

Pre-built binary from GitHub Releases. Source build (`honkhonk`) and VCS (`honkhonk-git`) variants are planned. See [`packaging/aur/README.md`](packaging/aur/README.md) for maintainer notes.

## Building
```

- [ ] **Step 3: Verify the insertion**

Run: `grep -n "yay -S honkhonk-bin" README.md`
Expected: one matching line near line ~57.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: add AUR install section to README

Points users to honkhonk-bin on the AUR for Arch-based distros."
```

---

## Task 6: Push branch and validate CI

- [ ] **Step 1: Confirm branch state**

Run: `git branch --show-current`
Expected: `feat/issue-84`.

Run: `git log --oneline origin/main..HEAD`
Expected: at least 5 commits (workflow, PKGBUILD, .SRCINFO, runbook, README).

- [ ] **Step 2: Push the branch**

```bash
git push -u origin feat/issue-84
```

- [ ] **Step 3: Verify the AUR validation workflow runs**

```bash
gh run list --branch feat/issue-84 --workflow "AUR PKGBUILD validation" --limit 1
```

Expected: one queued/in-progress run.

- [ ] **Step 4: Watch the run**

```bash
gh run watch $(gh run list --branch feat/issue-84 --workflow "AUR PKGBUILD validation" --limit 1 --json databaseId --jq '.[0].databaseId')
```

Expected: run completes successfully. If `.SRCINFO` diff fails, regenerate the file (Task 3) and commit the corrected version.

---

## Task 7: Open PR

- [ ] **Step 1: Create the PR**

```bash
gh pr create \
  --base main \
  --title "feat(packaging): AUR honkhonk-bin PKGBUILD + CI validation" \
  --body "$(cat <<'EOF'
## Summary

Ships the smallest, lowest-risk first packaging channel for #84: an AUR \`honkhonk-bin\` package that pulls the existing GitHub Release \`.deb\` artifact and installs the binary + .desktop on Arch / Manjaro / EndeavourOS.

- New: \`packaging/aur/honkhonk-bin/{PKGBUILD,.SRCINFO}\` — standard AUR \`-bin\` pattern, \`bsdtar\`-extract upstream \`.deb\`.
- New: \`.github/workflows/aur.yml\` — \`namcap\` lint, \`.SRCINFO\` freshness diff, \`updpkgsums\` URL fetch, \`makepkg --syncdeps --install\` in \`archlinux:base-devel\`, \`pacman -Ql\` file-layout assertion.
- New: \`packaging/aur/README.md\` — first-push + per-release runbook, reserved CI SSH pubkey for the follow-up auto-publish PR.
- Modify: \`README.md\` — appended "Arch Linux (AUR)" install section.

## Scope — sub-MVP of #84

This PR delivers **only** the AUR \`honkhonk-bin\` slice. Out of scope (separate future PRs):
- AUR auto-publish workflow on release tag
- \`honkhonk\` source PKGBUILD and \`honkhonk-git\` VCS PKGBUILD
- Flathub rename + metainfo finish + submission
- Signed apt/rpm repos via GitHub Pages
- Nix flake / Snap / winget / aarch64

Closes part of #84.

## Test plan

CI does the heavy lifting — see \`.github/workflows/aur.yml\` (\`namcap\`, \`.SRCINFO\` diff, \`updpkgsums\`, \`makepkg --install\`, \`pacman -Ql\` assertion).

Manual smoke (post-merge, pre-AUR-push) — Arch / Manjaro host:

- [ ] \`cd packaging/aur/honkhonk-bin && namcap PKGBUILD\` — clean.
- [ ] \`updpkgsums && makepkg --printsrcinfo > .SRCINFO\` — no drift.
- [ ] \`makepkg -si\` — installs cleanly.
- [ ] \`honkhonk\` launches on a real Wayland session, tray icon appears.
- [ ] \`pacman -Rns honkhonk-bin\` — clean uninstall, no orphaned files.
- [ ] Copy \`PKGBUILD\` + \`.SRCINFO\` to AUR clone, push, verify at https://aur.archlinux.org/packages/honkhonk-bin.

## Notes

- Maintainer header: \`thewrz <adam@wrze.ski>\` with GPG fingerprint \`B514 CBC5 B44C AACF 02EA  0D68 B461 236C F8EA 7961\`.
- \`pkgver=0.1.0.alpha.1\` (dots — AUR forbids \`-\` in \`pkgver\`); \`_pkgtag=v0.1.0-alpha.1\` (GitHub tag).
- \`sha256sums=('SKIP')\` is intentional for this first PR; CI \`updpkgsums\` exercises the URL fetch on every push. Hash computation will move into the auto-publish workflow in the follow-up PR.
- Total source LOC: ~170, under CLAUDE.md 500 LOC ceiling.
EOF
)"
```

- [ ] **Step 2: Capture PR URL**

Run: `gh pr view --json url --jq .url`
Expected: PR URL on `wrzonance/HonkHonk`.

---

## Self-review checklist (run after plan is written)

- **Spec coverage:**
  - PKGBUILD with maintainer header, dotted `pkgver`, dashed `_pkgtag`, `provides`/`conflicts`, `bsdtar` extraction, `SKIP` hashes — Task 2.
  - `.SRCINFO` committed — Task 3.
  - CI workflow (namcap, `.SRCINFO` diff, `updpkgsums`, makepkg install, `pacman -Ql`) — Task 1.
  - `packaging/aur/README.md` with runbook + reserved CI pubkey — Task 4.
  - `README.md` AUR section — Task 5.
  - PR with `Closes part of #84` and correct title — Task 7.
- **Deviation from spec:** Task 2 adds the icon to `package()`. Justification documented inline — the upstream `.deb` ships the icon, omitting it would leave the `.desktop` entry without its visual asset. Net effect: PKGBUILD goes from ~14 lines of body to ~15 lines, still under the ~45 LOC estimate.
- **Placeholders:** none. Every code block is complete; every command shows expected output.
- **Type/name consistency:** `pkgname=honkhonk-bin`, `pkgver=0.1.0.alpha.1`, `_pkgtag=v0.1.0-alpha.1` repeated identically across PKGBUILD, `.SRCINFO`, runbook, and PR body.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-17-issue-84-aur-honkhonk-bin.md`. Proceeding with subagent-driven execution per harness instructions.
