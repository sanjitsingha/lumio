# Lumio

A keyboard-first desktop utility for **elementary OS 8** and **Windows 10/11**, inspired by macOS:

- **Spotlight-style search**: apps, files, folders and quick actions behind a global shortcut.
- **Caps Lock indicator**: a small ⇪ badge that appears **next to the mouse pointer** and follows it, including on Wayland.

Built with Tauri 2 (Rust) and React. It is a native desktop app, not Electron or a website.

---

## Features

| | |
|---|---|
| **Spotlight** | Fuzzy search (prefix, whole-word, acronym, substring, typo tolerance), Top Hit, sections (Applications, Quick Actions, Folders, Files), recent items, inline suggestions from history, usage-based ranking. Keyboard: `↑/↓` move, `Enter` open, `Ctrl+Enter` show in folder, `Tab`/`Shift+Tab` jump between sections, `Ctrl+Backspace` delete the previous word or path segment, `→` accept a suggestion, `Esc` close. |
| **Apps** | Linux: XDG desktop entries, including Flatpak and Snap, with icons from your icon theme. Windows: the Start menu's AppsFolder, including Store apps such as Calculator and Settings, with real shell icons. |
| **Files** | A background index of your home folder (or folders you choose). It is cached to disk and kept current with native change notifications (inotify / ReadDirectoryChangesW). Exclusions such as `node_modules`, `.git` or `*.tmp` keep it fast. |
| **Quick actions** | Open Settings, Calculator, File Manager, Browser, Terminal, and Lumio Settings. The list is extensible. |
| **Caps Lock indicator** | Anchored to the pointer: near, below or above it. You can set size, opacity, duration (or "while on"), animation speed, light/dark appearance, and a brief "off" flash. It is click-through and never takes keyboard focus. |
| **Global shortcuts** | Configurable, with a live recorder, validation, duplicate and system-conflict detection (Lumio names the owner, e.g. *"Alt+Space is already used by the system shortcut “Activate window menu”"*), reset, clear, and persistence. |
| **Settings** | A macOS System Settings–style window: General, Spotlight, Keyboard Shortcuts, Caps Lock Indicator, Appearance (light/dark/system, accent colours including the system accent), Advanced (cache, history, reset, diagnostics, live logs), About (with an update check). |
| **Background** | Runs in the background, single instance, launch at login, tray menu (where the desktop shows tray icons), clean shutdown that releases its shortcuts. |

## Quick start (development)

```bash
# elementary OS 8 / Ubuntu 24.04 – one-time system packages
sudo apt install build-essential curl libwebkit2gtk-4.1-dev libgtk-3-dev \
                 libayatana-appindicator3-dev librsvg2-dev
# Rust (https://rustup.rs) and Node.js 20+ are also required

npm install
npm run tauri dev        # starts Lumio with hot reload; Settings opens
```

Press **Ctrl+Space** to open Spotlight. Alt+Space is the window menu shortcut on elementary OS, so Lumio reports that conflict and uses the alternative.

> **Running inside the VS Code Flatpak?** Use the elementary **Terminal** app instead; the VS Code sandbox does not contain WebKitGTK.

## Building installers

```bash
npm run build:app -- --bundles deb        # Linux .deb (recommended for elementary OS)
npm run build:app -- --bundles appimage   # Linux AppImage
npm run build:app -- --bundles nsis       # Windows installer (run on Windows)
```

Outputs are written to `src-tauri/target/release/bundle/`. See [docs/BUILDING.md](docs/BUILDING.md).

## Releases and automatic updates

Publish a release with one command. GitHub Actions builds the signed Windows `.exe`, Linux `.deb` and AppImage, and every installed Lumio updates itself (Settings → About shows the release notes and has **Update Now**; there is also an optional fully automatic mode).

```bash
npm run release -- patch --notes "What changed" --push
```

First-time setup (GitHub repository, signing-key secret) and local update testing are covered in **[docs/RELEASING.md](docs/RELEASING.md)**.

## Tests

```bash
npm test                         # frontend unit tests (vitest)
npm run test:rust                # Rust unit tests
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored --test-threads=1   # real-OS integration tests
npm run lint:rust                # clippy, warnings are errors
```

## Command line

```
lumio               start (or bring up Settings if already running)
lumio --toggle      show/hide Spotlight (bind this to a desktop shortcut if needed)
lumio --settings    open Settings
lumio --quit        quit the running instance
lumio --autostart   used by the login entry; stays in the background
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Platform support and limitations](docs/PLATFORM_SUPPORT.md)
- [Building and packaging](docs/BUILDING.md)
- [Releasing and updates](docs/RELEASING.md)
- [Testing and results](docs/TESTING.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)

## Renaming the app

The product name lives in `src-tauri/tauri.conf.json` (`productName`, `mainBinaryName`, `identifier`). The UI reads the name at runtime. Author, licence and the repository URL (used by About and the update check) are set in `src-tauri/Cargo.toml`.

## Licence

MIT. Inspired by macOS; not affiliated with Apple Inc. Lumio does not use Apple artwork.
