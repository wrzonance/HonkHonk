# HonkHonk AppImage

## Runtime Requirements

The AppImage bundles the HonkHonk binary and its non-system shared libraries.
The following are **host dependencies** and are intentionally NOT bundled:

| Dependency | Why not bundled |
|---|---|
| **PipeWire** (`pipewire`, `libpipewire-0.3`) | Audio server — must match the running host daemon |
| **xdg-desktop-portal** | D-Bus portal — must be provided by your desktop environment |
| **glibc / libstdc++** | Always use the host's to avoid ABI incompatibilities |

## Running

```bash
chmod +x HonkHonk-x86_64.AppImage
./HonkHonk-x86_64.AppImage
```

Ensure before running:
- PipeWire is running on the host (`systemctl --user status pipewire`)
- A D-Bus session bus is available (`echo $DBUS_SESSION_BUS_ADDRESS` must be non-empty)
- A portal backend is installed (e.g. `xdg-desktop-portal-kde`, `xdg-desktop-portal-gnome`, or `xdg-desktop-portal-wlr`)

## Building

```bash
# 1. Build the release binary
cargo build --release

# 2. Copy binary into AppDir
cp target/release/honkhonk packaging/appimage/HonkHonk.AppDir/usr/bin/honkhonk

# 3. Bundle shared libraries (excludes glibc/libstdc++ automatically)
linuxdeploy \
  --appdir packaging/appimage/HonkHonk.AppDir \
  --executable packaging/appimage/HonkHonk.AppDir/usr/bin/honkhonk \
  --desktop-file assets/honkhonk.desktop \
  --icon-file assets/icons/hicolor/256x256/apps/honkhonk.png

# 4. Package as AppImage
ARCH=x86_64 appimagetool packaging/appimage/HonkHonk.AppDir HonkHonk-x86_64.AppImage
```

## Compatibility Target

Built on Ubuntu 22.04 (glibc 2.35) for maximum compatibility with:
- Ubuntu 22.04+
- Fedora 36+
- Arch Linux (rolling)
- Any distro with glibc ≥ 2.35
