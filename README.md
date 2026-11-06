# Lummo

A keyboard-first desktop utility for **elementary OS 8** and **Windows 10/11**, inspired by macOS:

- **Spotlight-style search**: apps, files, folders, quick actions and settings pages (“Turn on dark mode”) behind a global shortcut.
- **Caps Lock indicator**: a small ⇪ badge that appears **at the blinking text cursor** of the field you type in and follows it, like on a Mac, including on Wayland.

Built with Tauri 2 (Rust) and React. It is a native desktop app, not Electron or a website.

Lummo was called Lumio before version 0.1.15. Installed copies update to Lummo and keep their settings.

---

## Features

| | |
|---|---|
| **Spotlight** | Fuzzy search (prefix, whole-word, acronym, substring, typo tolerance), Top Hit, sections (Applications, Quick Actions, Settings, Folders, Files), recent items, inline suggestions from history, usage-based ranking. The selected app's own actions are listed below it, with the app's icon: its desktop entry's actions on Linux (*New Window*, *New Private Window*…), its jump-list tasks on Windows (Chrome's *New Incognito window*, Calculator's *Scientific*, Windows Terminal's profiles…). A calculator answers sums such as `3*6+6-45` as you type (Enter copies the answer). Every search ends with *Search Google for “…”*, which opens in the browser chosen in Settings (or the system default). Keyboard: `↑/↓` move, `Enter` open, `Ctrl+Enter` show in folder, `Tab`/`Shift+Tab` jump between sections, `Ctrl+Backspace` delete the previous word or path segment, `→` accept a suggestion, `Esc` close. |
| **Apps** | Linux: XDG desktop entries, including Flatpak and Snap, with icons from your icon theme. Windows: the Start menu's AppsFolder, including Store apps such as Calculator and Settings, with real shell icons. |
| **Files** | A background index of your home folder (or folders you choose). It is cached to disk and kept current with native change notifications (inotify / ReadDirectoryChangesW). Exclusions such as `node_modules`, `.git` or `*.tmp` keep it fast. |
| **Quick actions** | Open Settings, Calculator, File Manager, Browser, Terminal, and Lummo Settings, each with the icon of the app it opens. The list is extensible. |
| **Settings pages** | Tasks such as *Turn on dark mode*, *Connect to Wi-Fi*, *Turn on Night Light*, *Change wallpaper* or *Uninstall apps* open the system's settings app at the right page and show its icon: Windows Settings (`ms-settings:`), or System Settings on elementary OS (`settings://`): 89 pages on Windows, 63 on elementary OS. They are found however you type them (`darkmode`, `dark mode`, `night light settings`, `wifi`). |
| **Caps Lock indicator** | Anchored to the text cursor (read from the input method on Linux, where every app that types reports its cursor, and through accessibility; the system caret on Windows, or UI Automation in apps such as Settings, the Start menu's search and Windows Terminal), shown also when you click into a text field while Caps Lock is on: near, below or above the cursor. Where there is no text cursor it is not shown. You can set size, opacity, corner radius, duration ("until you type" as on a Mac, "while on", or a time) and animation speed. As on a Mac it only shows Caps Lock on: turning it off just hides it. It follows the app's light or dark appearance. It is click-through and never takes keyboard focus. |
| **Global shortcuts** | Configurable, with a live recorder, validation, duplicate and system-conflict detection (Lummo names the owner, e.g. *"Alt+Space is already used by the system shortcut “Activate window menu”"*), reset, clear, and persistence. |
| **Settings** | A macOS System Settings–style window: General (startup, updates, clearing the cache and history, resetting settings), Spotlight (including its keyboard shortcut), Caps Lock Indicator, Appearance (light/dark/system, accent colours including the system accent), About (with an update check). |
| **Background** | Runs in the background, single instance, launch at login, tray menu (where the desktop shows tray icons), clean shutdown that releases its shortcuts. |

## Quick start (development)

```bash
# elementary OS 8 / Ubuntu 24.04 – one-time system packages
sudo apt install build-essential curl libwebkit2gtk-4.1-dev libgtk-3-dev \
                 libayatana-appindicator3-dev librsvg2-dev
# Rust (https://rustup.rs) and Node.js 20+ are also required

npm install
npm run tauri dev        # starts Lummo with hot reload; Settings opens
```

Press **Ctrl+Space** to open Spotlight (**Alt+Space** on Windows); change it in Settings → Spotlight.

> **Running inside the VS Code Flatpak?** Use the elementary **Terminal** app instead; the VS Code sandbox does not contain WebKitGTK.

## Building installers

```bash
npm run build:app -- --bundles deb        # Linux .deb (recommended for elementary OS)
npm run build:app -- --bundles appimage   # Linux AppImage
npm run build:app -- --bundles nsis       # Windows installer (run on Windows)
```

Outputs are written to `src-tauri/target/release/bundle/`. See [docs/BUILDING.md](docs/BUILDING.md).

## Releases and automatic updates

Publish a release with one command. GitHub Actions builds the signed Windows `.exe`, Linux `.deb` and AppImage, and every installed Lummo updates itself (Settings → About shows the release notes and has **Update Now**; there is also an optional fully automatic mode).

```bash
npm run release -- patch --notes "What changed" --push
```

First-time setup (GitHub repository, signing-key secret) and local update testing are covered in **[docs/RELEASING.md](docs/RELEASING.md)**.

## Privacy

Lummo sends anonymous usage statistics to [Aptabase](https://aptabase.com) unless **Share usage statistics** is turned off in Settings → General. It sends one event when Lummo starts (how it was started, whether Spotlight and the Caps Lock indicator are on) and one a day with counts: how often Spotlight opened, how many apps, files, folders, actions, settings pages and web searches were opened from it, and how often the Caps Lock indicator appeared or found no text cursor. Aptabase adds the Lummo version, the OS name and version and the locale; on Linux Lummo adds the session type (X11/Wayland) and the desktop. Nothing identifies you, and nothing you type, search for or open is ever sent. The code is in [`src-tauri/src/analytics.rs`](src-tauri/src/analytics.rs).

## Tests

```bash
npm test                         # frontend unit tests (vitest)
npm run test:rust                # Rust unit tests
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored --test-threads=1   # real-OS integration tests
npm run lint:rust                # clippy, warnings are errors
```

## Command line

```
lummo               start (or bring up Settings if already running)
lummo --toggle      show/hide Spotlight (bind this to a desktop shortcut if needed)
lummo --settings    open Settings
lummo --quit        quit the running instance
lummo --autostart   used by the login entry; stays in the background
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

MIT. Inspired by macOS; not affiliated with Apple Inc. Lummo does not use Apple artwork.
