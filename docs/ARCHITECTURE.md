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

Result ids are `provider:local-id`. **Execution re-resolves the id inside the provider:** apps must be in the app index and files must be live entries of the file index. An app's own actions have ids `app:<key>#<action>` and run only if the indexed app declares that action. The web search (`web:<query>`, `search/web.rs`) is placed after the ranking so it is always last; its link is built in Rust and is always a Google results page, opened in the browser chosen in Settings (`platform::browser`: Windows' `Clients\StartMenuInternet` registrations, or Linux's `https` handlers) or the system default. The frontend therefore cannot make the backend open arbitrary paths. Executable files are revealed in the file manager instead of being run.

To add a feature such as clipboard history or plugins, implement `SearchProvider` and register it in `SearchEngine::new`.

The **calculator** is the exception: it needs no system access, so the page computes it (`src/utils/calc.js`, a small parser; the CSP forbids `eval`) on every keystroke, without waiting for the backend, and shows it as a *Calculator* section above the backend's results. Enter copies the answer.

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
caret tracker ──caret rect─▶ geometry::place_at_caret ───────────────┤  follow caret (50 ms)
pointer tracker ─position──▶ geometry::place_body (edge flipping) ────┤  follow pointer
                                                                      ▼
                                                     render::render (tiny-skia) ─▶ platform overlay
```

**Gliding.** A new caret or pointer position is a target: the indicator glides there at 60 fps on a critically damped spring (`smooth_damp`: gentle start, no overshoot, momentum kept when the next keystroke moves the caret again), settling in about 0.25 s at the *Normal* animation speed. Animation speed *Off*, a first appearance and a move to another monitor place it at once.

| | Windows | Linux X11 | Linux Wayland (elementary) |
|---|---|---|---|
| State | low-level keyboard hook + `GetKeyState` | XKB `StateNotify` events | keyboard LED via sysfs (150 ms read) |
| Text cursor | `GetGUIThreadInfo` system caret, then MSAA `OBJID_CARET` | AT-SPI `Text` caret extents | AT-SPI, plus the window origin from Gala's actors (below) |
| Pointer | `GetCursorPos` | `QueryPointer` | XWayland tracker window (below) |
| Overlay | layered `WS_EX_LAYERED|TRANSPARENT|NOACTIVATE|TOPMOST` window | override-redirect ARGB X window, empty input shape | the same, via XWayland |

**Text cursor from the input method (Linux).** So that candidate windows appear at the text cursor, apps that type through IBus report their cursor to it every time it moves: GTK and Qt apps directly (`SetCursorLocationRelative`, relative to their window surface), Wayland text-input clients (Chromium, Electron, Firefox) through Gala (`SetCursorLocation`, screen coordinates, scaled like the window for Chromium), X11 apps through ibus-x11. `platform/linux/ime.rs` watches exactly these calls and `FocusIn`/`FocusOut` on IBus's bus with eavesdropping match rules; key events are never subscribed to, and the watched copies are swallowed so nothing answers them. The controller keeps the focused context's last cursor; relative positions get the focused window's surface origin (Gala's actor, found as below), Chromium's are unscaled around it. A context's position is forgotten when it gets focus (positions sent while a page was still being laid out are stale), and at each Caps Lock press the input method's position is cross-checked against the field accessibility reports as focused: inside it, the input method wins (it is exact even where accessibility only knows the field, such as Chrome's address bar); outside it, accessibility's fresh answer wins. Apps that report only to the input method (VS Code, the GTK 4 Terminal) use it alone. While the indicator is visible, cursor moves arrive as events. When a text field gets focus while Caps Lock is on (and *Show when you click into a text field* is on), the indicator is shown once its cursor arrives (or after 250 ms through accessibility). Chrome takes part only when started with `--enable-wayland-ime --wayland-text-input-version=3`.

**Text cursor through accessibility (Linux).** When Caps Lock changes, `platform/linux/caret.rs` reads the accessibility bus (AT-SPI, the bus screen readers use). It picks the active window whose title matches the window Gala reports as focused and finds its focused object with one `Collection.GetMatches` call (GTK 3, LibreOffice, Firefox, Chromium), or else by searching the visible tree (GTK 4). It accepts the object if it is editable (or a terminal). The caret is the left edge of the character at the caret offset (or the right edge of the previous one at the end of the text). Nothing is registered with the accessibility registry, so no app starts sending events because of Lumio. While the indicator is visible, only that field's caret is re-read every 50 ms.

On X11, and for XWayland apps, toolkits report screen coordinates. Native Wayland apps cannot know where their window is and report positions relative to their window surface. Gala exposes its window actors on the same bus (Clutter accessibility) with their real screen rectangles. The actor whose size equals the window surface, or else one that surrounds the frame Gala reports with the same aspect ratio (Chromium scales its window size with the text scaling factor), supplies the origin. Toolkits embedded in another one's window add their own hidden origin: LibreOffice's VCL pane inside the GTK window claims to sit at (0, 0) although it starts 61 px lower; an object at (0, 0) with exactly its parent's size, under a parent elsewhere, marks such an embedding. GTK 4.14 (elementary OS 8's system GTK 4) reports (0, 0) for every screen position and no character positions at all; its window positions are correct, so a single-line field is placed by its window extents (the indicator goes below the field), but a GTK 4 window with a shadow border cannot be located (its frame sits inside Gala's actor at an uneven inset) and reports no text cursor. Where `Collection` works its answer is authoritative (Chromium keeps the focused flag on its address bar after focus moves into a page), and the path found last time only saves walking up from the field again; for toolkits without `Collection` (GTK 4) the field found last is checked first, so pressing Caps Lock again in the same field needs no search. A typical lookup takes 15–60 ms (up to about 0.3 s for the first press in a large GTK 4 window).

Rich editors in Chromium (`contenteditable`: ChatGPT, WhatsApp Web, Gmail) keep their text in child objects: the focused field's text holds one object-replacement character per child and its caret offset points at the child holding the caret, so Lumio follows `Hypertext` links down to the object whose own caret is the real one. Chromium also reports its window and its own controls (the address bar) scaled by the text scaling factor, while web pages are not scaled; fields outside a `document web` are therefore divided by the window's scale.

Where neither source knows a text cursor, the indicator is not shown (since 0.1.14 it is always anchored to the text cursor; the pointer anchor and its XWayland tracker were removed). Chromium and Electron apps (Chrome, VS Code, Slack…) build their accessibility tree only when started with `--force-renderer-accessibility`; the desktop accessibility switch alone does not enable it.

**Override-redirect windows have no window type.** Gala moves windows typed `_NET_WM_WINDOW_TYPE_NOTIFICATION` into its notification stack, which animated the first indicator and left every later one invisible (0.1.3 and earlier). The indicator and tracker windows therefore set only `WM_CLASS`.

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
