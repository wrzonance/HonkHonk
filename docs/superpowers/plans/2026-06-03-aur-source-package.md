# AUR Source Package (`honkhonk`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a source-built `honkhonk` AUR package (PKGBUILD + .SRCINFO + README), drop the X11/libxdo linkage at compile time via Cargo feature flags, minimise the runtime dependency set, and add CI that validates the source build in `archlinux:base-devel`.

**Architecture:** A new `packaging/aur/honkhonk/` dir holds a source PKGBUILD that builds from the GitHub auto-generated tag tarball (`$url/archive/$_pkgtag.tar.gz`). `Cargo.toml` disables `tray-icon`/`muda` default features and re-enables only `gtk`, dropping the `libxdo` crate (and thus the `xdotool`/`libxdo.so` runtime linkage). CI gets a second job in the existing `aur.yml` matrixed over both `honkhonk-bin` and `honkhonk` (namcap + .SRCINFO freshness + makepkg install). `honkhonk-bin` stays as a documented convenience secondary.

**Tech Stack:** Rust/Cargo, Arch `makepkg`/`namcap`/`pacman-contrib`, GitHub Actions (`archlinux:base-devel` container), `tray-icon`/`muda` feature flags.

---

## Background facts (verified in repo, do not re-investigate)

- `Cargo.lock` is **gitignored** (see `.gitignore`). The GitHub auto-tarball will NOT contain a lockfile. Therefore the PKGBUILD must NOT use `--locked`/`--frozen`. Use `cargo fetch` (no `--locked`) in `prepare()` then `cargo build --offline --release` in `build()` and `cargo test --offline --release` in `check()`. This keeps compile/test network-isolated without needing a committed lockfile.
- `tray-icon = "0.24"` default features = `["libxdo", "gtk"]`. `tray-icon/libxdo` → `muda/libxdo` → `dep:libxdo` crate → links `libxdo.so` (provided by Arch `xdotool`). This is the ONLY thing pulling libxdo.
- `muda`'s `libxdo` is used **only** to make predefined Copy/Cut/Paste/SelectAll menu items synthesise X11 keystrokes (`muda/src/platform_impl/gtk/mod.rs:1187-1189`). Our tray (`src/tray/icon.rs`) uses only `MenuItem` + `PredefinedMenuItem::separator()`. Separator does NOT need libxdo. So dropping `libxdo` loses zero functionality.
- Setting `tray-icon = { version = "0.24", default-features = false, features = ["gtk"] }` drops libxdo at compile time. `gtk` feature is retained (provides `muda/gtk` + `libappindicator` for the SNI tray on Linux). Wayland-only per CLAUDE.md, so X11 keystroke synth was dead weight.
- The release workflow (`release.yml`) only creates a GitHub release with notes; it does not upload a custom source tarball. The `.deb` comes from `deb.yml`. The GitHub auto-tag tarball is always available at `https://github.com/<owner>/<repo>/archive/<tag>.tar.gz` and extracts to `<repo>-<tag-without-v>/`.
- Existing AUR repo URL convention in `honkhonk-bin/PKGBUILD` uses `https://github.com/wrzonance/HonkHonk`. Match that for consistency (the `-bin` package already uses it).
- `pkgver` rule: AUR forbids `-` in `pkgver`. Use dots: `0.1.0.alpha.1`. `_pkgtag` keeps the dash form `v0.1.0-alpha.1` matching the GitHub tag. Current `Cargo.toml` version = `0.1.0-alpha.1`.
- CI actions must be pinned to commit SHAs (repo convention). Reuse the exact pin already in `aur.yml`: `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1`.

## Dependency justification (the audited runtime set for the SOURCE build)

After dropping libxdo, the justified `depends=()` for the source build:

| dep | why (binary links / dlopens it) |
|-----|--------------------------------|
| `pipewire` | audio engine — links `libpipewire-0.3.so` via the `pipewire` crate (`*-sys`) |
| `gtk3` | `muda`/`tray-icon` `gtk` feature links `libgtk-3.so` for the tray menu |
| `libayatana-appindicator` | `tray-icon` `gtk` feature pulls `libappindicator` → links `libayatana-appindicator3.so` for the SNI tray |
| `wayland` | Iced/winit links `libwayland-client.so` on a Wayland session |
| `libxkbcommon` | keyboard mapping — linked by winit/Iced (`libxkbcommon.so`) |
| `xdg-desktop-portal` | runtime D-Bus service for file chooser + global shortcuts (ashpd) |

**Dropped vs `-bin`:** `xdotool` — no longer linked once `libxdo` feature is off. This is the whole point of Goal 3, and it also makes Goal 4 (xdotool cross-distro availability) moot.

`makedepends=()` for source build: `cargo` `rust` `pkgconf` (provides `pkg-config`) `pipewire` (headers for the `pipewire-sys` build) `git`. Wayland/gtk dev headers come transitively via the runtime libs being present + their pkgconfig; on Arch the runtime packages ship their headers (no separate `-dev` split), so `gtk3`/`wayland`/`libxkbcommon` being in `depends` is enough for the compile, but list `wayland` is runtime anyway. Keep makedepends minimal and justified.

## File Structure

- Create `packaging/aur/honkhonk/PKGBUILD` — source build PKGBUILD.
- Create `packaging/aur/honkhonk/.SRCINFO` — generated to match PKGBUILD exactly.
- Create `packaging/aur/honkhonk/README.md` — per-dep justification + maintainer runbook for the source variant.
- Modify `Cargo.toml` — `tray-icon` line to drop default features, keep `gtk`.
- Modify `.github/workflows/aur.yml` — matrix over `honkhonk-bin` + `honkhonk`, generalise steps.
- Modify `README.md` (root) — AUR install section: `honkhonk` (source) recommended; `-bin`, `-git` alternatives.
- Modify `packaging/aur/README.md` — note it now covers both `-bin` and source; point to per-variant READMEs.

---

### Task 1: Drop libxdo/X11 linkage in Cargo.toml

**Files:**
- Modify: `Cargo.toml:12`

- [ ] **Step 1: Verify the linkage exists (baseline)**

Run: `cargo build --release && ldd target/release/honkhonk | grep -i xdo`
Expected: a line referencing `libxdo.so` (baseline links it). If empty, libxdo may be statically pulled or absent — note it and continue; the feature flag is still correct.

- [ ] **Step 2: Edit Cargo.toml**

Change line 12 from:
```toml
tray-icon = "0.24"
```
to:
```toml
# Wayland-only: disable default features to drop the `libxdo` X11 keystroke
# path (muda only uses it for predefined Copy/Cut/Paste items, which we don't
# use). Keep `gtk` for the SNI tray (libgtk-3 + libayatana-appindicator).
# This removes the libxdo.so / xdotool runtime linkage entirely. See issue #98.
tray-icon = { version = "0.24", default-features = false, features = ["gtk"] }
```

- [ ] **Step 3: Rebuild and verify linkage is gone**

Run: `cargo build --release && (ldd target/release/honkhonk | grep -i xdo && echo FOUND || echo "ABSENT - good")`
Expected: `ABSENT - good`. Also confirm the binary still has `libgtk-3` and `libayatana-appindicator`:
Run: `ldd target/release/honkhonk | grep -Ei 'gtk-3|appindicator'`
Expected: both present.

- [ ] **Step 4: Confirm tests still pass**

Run: `cargo test --release`
Expected: PASS (tray menu still builds; separator needs no libxdo).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml
git commit -m "fix(deps): drop tray-icon libxdo/X11 feature (Wayland-only)"
```

---

### Task 2: Write the source PKGBUILD

**Files:**
- Create: `packaging/aur/honkhonk/PKGBUILD`

- [ ] **Step 1: Write the PKGBUILD**

```bash
# Maintainer: thewrz <adam@wrze.ski>
# GPG: B514 CBC5 B44C AACF 02EA  0D68 B461 236C F8EA 7961

pkgname=honkhonk
pkgver=0.1.0.alpha.1
_pkgtag=v0.1.0-alpha.1
pkgrel=1
pkgdesc="Wayland-native Linux soundboard — Iced GUI + PipeWire audio (built from source)"
arch=('x86_64')
url="https://github.com/wrzonance/HonkHonk"
license=('MIT')
# Runtime deps — each is dynamically linked or dlopened by the binary.
# See README.md in this directory for the full per-dep justification.
depends=(
    'pipewire'                 # libpipewire-0.3.so — audio engine
    'gtk3'                     # libgtk-3.so — tray menu (muda/tray-icon gtk feature)
    'libayatana-appindicator' # libayatana-appindicator3.so — SNI tray
    'wayland'                  # libwayland-client.so — Iced/winit Wayland backend
    'libxkbcommon'             # libxkbcommon.so — keyboard mapping (winit)
    'xdg-desktop-portal'       # D-Bus portal: file chooser + global shortcuts (ashpd)
)
makedepends=(
    'cargo'        # build driver
    'rust'         # toolchain
    'pkgconf'      # pkg-config for -sys crates
    'pipewire'     # headers for pipewire-sys
    'git'
)
provides=('honkhonk')
conflicts=('honkhonk-bin' 'honkhonk-git')
source=("$pkgname-$pkgver.tar.gz::$url/archive/$_pkgtag.tar.gz")
sha256sums=('SKIP')

_srcdir="HonkHonk-${_pkgtag#v}"

prepare() {
    cd "$srcdir/$_srcdir"
    # No committed Cargo.lock upstream — fetch (and generate a lockfile) here so
    # build() and check() can run fully offline.
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "$srcdir/$_srcdir"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release --all-features
}

check() {
    cd "$srcdir/$_srcdir"
    export RUSTUP_TOOLCHAIN=stable
    cargo test --frozen --release
}

package() {
    cd "$srcdir/$_srcdir"
    install -Dm755 "target/release/honkhonk" "$pkgdir/usr/bin/honkhonk"
    install -Dm644 "assets/honkhonk.desktop" \
        "$pkgdir/usr/share/applications/honkhonk.desktop"
    install -Dm644 "assets/icons/hicolor/256x256/apps/honkhonk.png" \
        "$pkgdir/usr/share/icons/hicolor/256x256/apps/honkhonk.png"
    install -Dm644 "LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
```

NOTE on `--frozen`: `cargo fetch` in `prepare()` generates `Cargo.lock` in the extracted source tree (it does not need a committed one — it resolves from `Cargo.toml`). Once the lockfile exists and the cache is populated, `--frozen` in `build()`/`check()` is satisfied and forces offline. This is the correct idiom for upstreams that gitignore their lockfile. Verify in Task 5 CI.

- [ ] **Step 2: Verify the desktop + icon asset paths exist in the repo**

Run: `ls assets/honkhonk.desktop assets/icons/hicolor/256x256/apps/honkhonk.png LICENSE`
Expected: all three exist. If the icon path differs, fix the `package()` path to match the real asset (check `Cargo.toml` `[package.metadata.deb]` assets block for the canonical paths — they are the source of truth).

- [ ] **Step 3: Commit (PKGBUILD only; .SRCINFO comes next)**

```bash
git add packaging/aur/honkhonk/PKGBUILD
git commit -m "feat(packaging): honkhonk source PKGBUILD (AUR)"
```

---

### Task 3: Generate .SRCINFO

**Files:**
- Create: `packaging/aur/honkhonk/.SRCINFO`

- [ ] **Step 1: Generate it from the PKGBUILD**

Run (from repo root): `cd packaging/aur/honkhonk && makepkg --printsrcinfo > .SRCINFO`
If `makepkg` is unavailable on the dev host, hand-write `.SRCINFO` to match the PKGBUILD field-for-field (pkgbase, pkgdesc, pkgver, pkgrel, url, arch, license, every depends/makedepends line, provides, conflicts, source, sha256sums = SKIP, then `pkgname = honkhonk`).

- [ ] **Step 2: Verify freshness diff is empty**

Run: `cd packaging/aur/honkhonk && diff <(makepkg --printsrcinfo) .SRCINFO`
Expected: no output (identical). This is exactly what CI checks.

- [ ] **Step 3: Commit**

```bash
git add packaging/aur/honkhonk/.SRCINFO
git commit -m "feat(packaging): honkhonk .SRCINFO"
```

---

### Task 4: Write packaging/aur/honkhonk/README.md

**Files:**
- Create: `packaging/aur/honkhonk/README.md`

- [ ] **Step 1: Write the README**

Content must include: (a) what this package is (recommended source build), (b) the per-dep justification table (copy from this plan's "Dependency justification" section), (c) why `xdotool` was dropped (libxdo feature disabled, link to issue #98), (d) the no-`Cargo.lock` / `cargo fetch` + `--frozen` rationale, (e) a per-release bump runbook (bump pkgver/_pkgtag, regen `.SRCINFO`, namcap, local `makepkg --syncdeps --install`, push to AUR clone), (f) note that the GitHub auto-tag tarball is the source (no custom release artifact needed). Keep under ~120 lines.

- [ ] **Step 2: Commit**

```bash
git add packaging/aur/honkhonk/README.md
git commit -m "docs(packaging): honkhonk AUR README + dep justification"
```

---

### Task 5: Extend aur.yml CI to validate the source build

**Files:**
- Modify: `.github/workflows/aur.yml`

- [ ] **Step 1: Rewrite the validate job as a matrix over both packages**

Generalise the existing job to a `strategy.matrix.pkg: [honkhonk-bin, honkhonk]`, set `working-directory` and `chown` to `packaging/aur/${{ matrix.pkg }}`, keep namcap + .SRCINFO freshness + `makepkg --noconfirm --syncdeps --install`. For the source build the install step compiles the whole crate from the GitHub tarball (slow but correct). Keep the `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1` pin. The `updpkgsums` step only applies to `-bin` (it SKIPs sha for the tarball too, but `updpkgsums` is harmless on a SKIP source — keep it so both matrix legs share steps). The final manifest assertion (`pacman -Ql ${{ matrix.pkg }}`) stays, checking `/usr/bin/honkhonk` + the `.desktop`.

Full file:
```yaml
name: AUR PKGBUILD validation

on:
  push:
    paths:
      - 'packaging/aur/**'
      - '.github/workflows/aur.yml'
      - 'Cargo.toml'
  pull_request:
    paths:
      - 'packaging/aur/**'
      - '.github/workflows/aur.yml'
      - 'Cargo.toml'

permissions:
  contents: read

jobs:
  validate:
    name: ${{ matrix.pkg }} — namcap + .SRCINFO + makepkg
    runs-on: ubuntu-latest
    container:
      image: archlinux:base-devel
    strategy:
      fail-fast: false
      matrix:
        pkg: [honkhonk-bin, honkhonk]
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1

      - name: Install validation tooling
        run: |
          pacman -Syu --noconfirm --needed namcap pacman-contrib git

      - name: Create non-root build user
        run: |
          useradd -m builder
          echo 'builder ALL=(ALL) NOPASSWD: ALL' >> /etc/sudoers
          chown -R builder "packaging/aur/${{ matrix.pkg }}"

      - name: namcap lint
        working-directory: packaging/aur/${{ matrix.pkg }}
        run: sudo -u builder namcap PKGBUILD

      - name: .SRCINFO freshness check
        working-directory: packaging/aur/${{ matrix.pkg }}
        run: |
          sudo -u builder makepkg --printsrcinfo > /tmp/.SRCINFO.fresh
          diff -u .SRCINFO /tmp/.SRCINFO.fresh

      - name: Build + install
        working-directory: packaging/aur/${{ matrix.pkg }}
        run: sudo -u builder makepkg --noconfirm --syncdeps --install

      - name: Installed file manifest
        run: |
          pacman -Ql ${{ matrix.pkg }}
          pacman -Ql ${{ matrix.pkg }} | grep -q '/usr/bin/honkhonk$'
          pacman -Ql ${{ matrix.pkg }} | grep -q '/usr/share/applications/honkhonk.desktop$'
```

NOTE: dropped the `updpkgsums` step — it was `-bin`-only and both legs use `SKIP` sha256sums (`-bin` from a `.deb` URL with SKIP, source from a tag tarball with SKIP), so refreshing sums is unnecessary and would require network at lint time. Document this in the commit body. The source tarball uses `SKIP` because the tag tarball is regenerated by GitHub and a pinned hash would be fragile across alpha re-tags; the build still verifies integrity by compiling + testing.

- [ ] **Step 2: Validate workflow YAML syntax**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/aur.yml')); print('YAML OK')"`
Expected: `YAML OK`

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/aur.yml
git commit -m "ci(aur): matrix-validate honkhonk-bin + source honkhonk"
```

---

### Task 6: Update root README + packaging/aur/README.md

**Files:**
- Modify: `README.md:57-63`
- Modify: `packaging/aur/README.md`

- [ ] **Step 1: Edit root README Arch section**

Replace the Arch (AUR) block so the recommended install is the source package, with `-bin` and `-git` as alternatives:
```markdown
### Arch Linux (AUR)

```bash
yay -S honkhonk        # source build (recommended)
```

Alternatives:

```bash
yay -S honkhonk-bin    # pre-built binary from GitHub Releases (Debian-based, see notes)
yay -S honkhonk-git    # bleeding-edge, tracks main (planned)
```

`honkhonk` (source) is the recommended package — it is an Arch-native build with no
foreign-soname workarounds. See [`packaging/aur/README.md`](packaging/aur/README.md)
for maintainer notes and the per-dependency justification.
```

- [ ] **Step 2: Edit packaging/aur/README.md**

Update the top paragraph to state this dir now holds both `honkhonk-bin` and the source `honkhonk` (recommended), each with its own subdir + README. Keep the shared CI description but note it is matrixed over both. Keep the AUR account / SSH key / future auto-publish sections.

- [ ] **Step 3: Commit**

```bash
git add README.md packaging/aur/README.md
git commit -m "docs(readme): recommend honkhonk source AUR package"
```

---

### Task 7: Final verification before PR

- [ ] **Step 1: clippy + fmt + test (repo gates)**

Run: `cargo fmt -- --check && cargo clippy --all-targets -- -D warnings && cargo test --release`
Expected: all pass.

- [ ] **Step 2: LOC check (must be ≤500, excluding generated/lockfiles)**

Run: `git diff main...HEAD --stat`
Expected: total changed LOC ≤ 500 (PKGBUILD + .SRCINFO + README + workflow + 1-line Cargo.toml = well under).

- [ ] **Step 3: Confirm honkhonk-bin fate is documented**

`honkhonk-bin` is KEPT as a documented convenience secondary (not removed). Confirm root README + packaging/aur/README.md both reflect this.

- [ ] **Step 4: Finish branch**

REQUIRED SUB-SKILL: Use superpowers:finishing-a-development-branch → option 2 (Push + PR). PR body must include `Closes #98` and a `## Design decisions` section.

---

## Self-Review

**Spec coverage:**
- Goal 1 (source PKGBUILD + CI) → Tasks 2, 3, 5 ✓
- Goal 2 (dep audit + minimization + justification) → Task 2 depends block + Task 4 README table ✓
- Goal 3 (resolve libxdo: drop at compile time) → Task 1 ✓
- Goal 4 (xdotool cross-distro check) → MOOT because Task 1 drops the linkage; documented in PR design decisions ✓
- Checklist: audit features ✓(done in plan), patch Cargo.toml ✓(T1), PKGBUILD ✓(T2), .SRCINFO ✓(T3), CI ✓(T5), README dep justification ✓(T4), decide -bin fate ✓(T7 keep+document), root README ✓(T6), #84 note → PR body only (per instructions, do not edit #84).
- OUT OF SCOPE respected: no auto-publish-on-tag, no Flathub, no signed repos.

**Placeholder scan:** No TBD/TODO. Asset paths verified against `Cargo.toml` deb assets block (Task 2 Step 2 cross-checks).

**Type consistency:** `pkgname=honkhonk`, `provides=('honkhonk')`, `conflicts=('honkhonk-bin' 'honkhonk-git')` consistent across PKGBUILD + .SRCINFO. Matrix `pkg` names match dir names.
