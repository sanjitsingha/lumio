# Building and packaging

## Prerequisites

| | Linux (elementary OS 8 / Ubuntu 24.04) | Windows 10/11 |
|---|---|---|
| Rust | `curl https://sh.rustup.rs -sSf \| sh` | <https://rustup.rs> (MSVC toolchain) |
| Node.js | 20 or newer | 20 or newer |
| System | `sudo apt install build-essential curl file libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev` | Visual Studio Build Tools ("Desktop development with C++"); WebView2 is preinstalled on Windows 10 (21H2+) and 11 |

```bash
npm install
```

## Development

```bash
npm run tauri dev
```

This starts Vite with hot reload and runs the Rust app in debug mode. Rust changes trigger a rebuild and restart.

## Linux packages

```bash
npm run build:app -- --bundles deb
# → src-tauri/target/release/bundle/deb/Lumio_0.1.0_amd64.deb
sudo apt install ./src-tauri/target/release/bundle/deb/Lumio_0.1.0_amd64.deb
```

The `.deb` declares `libwebkit2gtk-4.1-0`, `libgtk-3-0` and `libayatana-appindicator3-1`. It installs `/usr/bin/lumio`, a launcher entry, and icons.

An AppImage is also possible (`--bundles appimage`). The Tauri CLI downloads `linuxdeploy` for this.

**Why not Flatpak?** A launcher must enumerate and start *host* applications, watch your files, and read keyboard LED state and XWayland pointer events. The Flatpak sandbox blocks or complicates all of these: they would need `--filesystem=host`, `--talk-name=org.freedesktop.Flatpak` and `flatpak-spawn --host` for every launch, which defeats the sandbox. A native `.deb` is the honest packaging for this kind of utility.

## Windows installer

On Windows:

```powershell
npm run build:app -- --bundles nsis
# → src-tauri\target\release\bundle\nsis\Lumio_0.1.0_x64-setup.exe
```

The installer is per-user (no admin rights) and bootstraps WebView2 if it is missing.

`build:app` produces unsigned installers. Signed installers with update artifacts come from the release workflow, or from `npm run tauri build` with `TAURI_SIGNING_PRIVATE_KEY` set (see [RELEASING.md](RELEASING.md)).

## CI

`.github/workflows/build.yml` builds the Linux `.deb` and AppImage on `ubuntu-24.04` and the NSIS installer on `windows-latest`, runs the unit tests and clippy on both, and uploads the bundles as artifacts.

## Building without root (how this project was developed)

The development machine had no `sudo`, so builds used a rootless sysroot:

1. the `-dev` packages and Ubuntu's own `gcc-13`/binutils, fetched with `apt-get download`;
2. extracted with `dpkg-deb -x`;
3. pointed to through `PKG_CONFIG_SYSROOT_DIR` and a gcc wrapper.

This produces normal host binaries. It is only needed if you cannot install the packages above.
