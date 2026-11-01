# Architecture

```
lumio/
├── src/                         React frontend (JSX, Tailwind v4, Framer Motion, Lucide)
│   ├── App.jsx                  routes by window label: "spotlight" | "settings"
│   ├── pages/                   Spotlight.jsx, Settings.jsx, About.jsx
│   ├── components/
│   │   ├── spotlight/           SearchBar, ResultList, ResultRow, ResultIcon, SpotlightFooter
│   │   ├── settings/            Sidebar, PaneHeader, panes/* (one file per pane)
│   │   ├── shortcuts/           ShortcutRecorder
│   │   ├── capslock/            IndicatorPreview (SVG replica of the native artwork)
│   │   └── common/              controls (Switch, Segmented, Slider, Group/Row, KeyCaps…), Toaster
│   ├── hooks/                   useSearch, useAutoHeight, useSettings, useAppearance, usePlatform
│   ├── services/                tauri.js (command wrappers), events.js, updates.js
│   ├── stores/                  settingsStore (mirror of backend settings), toastStore
│   └── utils/                   keys (shortcut capture, Ctrl+Backspace), navigation, theme
├── src-tauri/src/
│   ├── lib.rs                   app assembly: plugins, protocol, commands, setup, shutdown
│   ├── app_state.rs             shared state + settings service (persist + side effects)
│   ├── cli.rs                   --toggle / --settings / --quit / --autostart
│   ├── commands/                frontend-callable commands (validated inputs)
│   ├── search/                  engine, fuzzy matcher, providers (apps, files, actions), icon protocol
│   ├── shortcuts/               accelerator model, manager, backends (Tauri plugin, Gala)
│   ├── capslock/                state machine, placement geometry, renderer, controller thread
│   ├── platform/{linux,windows} all OS-specific code behind one interface
│   ├── storage/                 settings (+migrations), history, atomic JSON writes
│   ├── tray/                    tray icon and menu
│   ├── window_manager/          Spotlight/Settings window lifecycle
│   └── dev.rs                   developer harness (feature `dev-snapshot`, never in releases)
├── tests/                       frontend unit tests
└── docs/
```

## Process model

One process, single instance (`tauri-plugin-single-instance`; D-Bus on Linux, a named mutex/pipe on Windows). Launching Lumio again forwards its arguments to the running instance, which is how `lumio --toggle` works.

Windows:

- **spotlight** is created hidden at startup so it appears instantly. It is transparent and undecorated, and the panel and its shadow are drawn in CSS.
- **settings** is created on demand and destroyed on close (to release memory). It uses native decorations.
- The **Caps Lock indicator** is not a webview. It is a native window drawn from a tiny bitmap rendered with tiny-skia, so it is cheap, instant and never focusable.

## Search

`SearchEngine` holds `SearchProvider`s (`apps`, `action`, `file`). Each provider returns raw match scores. The engine then:

1. applies kind weights and a frecency boost from history;
2. ranks the results and trims them to *Maximum results*;
3. promotes a strong first result to **Top Hit**, then groups the rest in a fixed section order.

Result ids are `provider:local-id`. **Execution re-resolves the id inside the provider:** apps must be in the app index and files must be live entries of the file index. The frontend therefore cannot make the backend open arbitrary paths. Executable files are revealed in the file manager instead of being run.

To add a feature such as a calculator, clipboard history or plugins, implement `SearchProvider` and register it in `SearchEngine::new`.

The file index:

- interns directory paths, so each entry stores only its name and a parent index;
- is built breadth-first with exclusions, a depth limit and an entry limit;
- is cached to disk, so search works right after login;
- is kept current by per-folder inotify watches (Linux) or recursive ReadDirectoryChangesW (Windows), with debounced incremental rescans;
- is fully rebuilt periodically as a safety net.

## Global shortcuts

`ShortcutManager` owns validation (syntax, modifiers, reserved combinations, duplicates) and per-slot status. It delegates OS registration to a `ShortcutBackend`:

| Platform | Backend |
|---|---|
| Windows | `tauri-plugin-global-shortcut` (`RegisterHotKey`) |
| Linux X11 | `tauri-plugin-global-shortcut` (`XGrabKey`) |
| elementary OS (Wayland and X11) | **Gala**: `org.gnome.Shell.GrabAccelerator` over D-Bus. On refusal Lumio reads the GSettings keybinding schemas to name the owner. |
| Other Wayland desktops | Unavailable; Lumio explains how to bind `lumio --toggle` instead |

A new binding is registered before the old one is released, so a refusal never leaves the user without a shortcut. While the recorder is open, all shortcuts are suspended. On Wayland, Gala's activation timestamp is passed to `gtk_window_present_with_time` so the compositor gives Spotlight keyboard focus.

## Caps Lock indicator

```
platform monitor ──state──▶ IndicatorState (pure) ──Show/Hide──▶ controller thread
                                                                      │  animation (fade + pop)
pointer tracker ─position─▶ geometry::place_body (edge flipping) ─────┤  follow pointer
                                                                      ▼
                                                     render::render (tiny-skia) ─▶ platform overlay
```

| | Windows | Linux X11 | Linux Wayland (elementary) |
|---|---|---|---|
| State | low-level keyboard hook + `GetKeyState` | XKB `StateNotify` events | keyboard LED via sysfs (150 ms read) |
| Pointer | `GetCursorPos` | `QueryPointer` | XWayland tracker window (below) |
| Overlay | layered `WS_EX_LAYERED|TRANSPARENT|NOACTIVATE|TOPMOST` window | override-redirect ARGB X window, empty input shape | the same, via XWayland |

**Wayland pointer tracking.** Wayland deliberately hides the pointer from ordinary clients. XWayland, however, receives real pointer events for X11 windows. While the indicator is visible, Lumio maps a transparent, never-focused override-redirect X window over the screen, and Mutter reports the real pointer position to it. Its input region excludes a square around the pointer, so clicks at the pointer always reach the app underneath. When the pointer leaves that square, the window receives a motion event, and the square and the indicator follow. The window is unmapped as soon as the indicator hides. If the pointer cannot be located, for example because another client grabs the pointer, Lumio falls back to Gala's on-screen display.

## Settings and storage

Settings are JSON files in the OS config directory (`~/.config/com.lumio.desktop/` on Linux, `%APPDATA%\com.lumio.desktop\` on Windows).

- **Writes are atomic:** the file is written to a temporary file, then renamed.
- **Loading is lenient:** each section is parsed independently, so one bad value never resets everything.
- **Values are validated:** they are clamped or repaired, and the user is told what changed.
- **Damaged files are kept:** they are moved to `settings.corrupt-<time>.json`.
- **Migrations are versioned:** `schemaVersion` with the step list in `storage/migrate.rs`.

History and usage counts are stored in the data directory. The file-index cache is stored in the cache directory.

## Updates

`updater.rs` wraps `tauri-plugin-updater`:

- **Feed:** `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`, derived from `repository` in `Cargo.toml`. It can be overridden with `LUMIO_UPDATE_ENDPOINT` for local tests.
- **Checks:** in the background (30 s after start, then every 6 h, when enabled) and on demand.
- **Install:** the artifact matching how Lumio was installed (NSIS / deb / AppImage, stamped into the binary at bundle time) is downloaded and verified against the embedded minisign public key. `requireSignedVersion` rejects downgrade attempts. It is then installed (`pkexec dpkg -i` for `.deb`), and Lumio restarts with `--after-update <pid>`, waiting for the old process to exit before taking the single-instance lock.
- **Development builds** can check and download but never install.

Releases are produced by `.github/workflows/release.yml` (tauri-action) when a `v*` tag is pushed (`npm run release`). See [RELEASING.md](RELEASING.md).

## Security

- **App commands are declared in `build.rs`.** Each window gets only what its capability file grants: Spotlight cannot change settings or read logs.
- **The CSP allows only the app itself, IPC and the `lumio-icon` protocol.** The webview makes no network requests; update checks run in Rust.
- **Updates must carry a valid signature** from the project's private key.
- **There is no shell execution.** Apps launch through GIO desktop entries or the Windows shell, and URLs must be `https://`.
- **The icon protocol serves only icons of indexed apps** (hex-encoded ids), never arbitrary paths.
