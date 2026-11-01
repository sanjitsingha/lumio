# Testing

## Automated tests

| Suite | Command | Count | Result (elementary OS 8, Wayland) |
|---|---|---|---|
| Rust unit tests | `cargo test --manifest-path src-tauri/Cargo.toml` | 115 | ✅ all pass |
| Rust integration tests (real OS, need a desktop session) | `cargo test --manifest-path src-tauri/Cargo.toml -- --ignored --test-threads=1` | 5 | ✅ all pass |
| Frontend unit tests | `npm test` | 25 | ✅ all pass |
| Clippy, Linux | `cargo clippy --all-targets -- -D warnings` | – | ✅ no warnings |
| Clippy / type check, Windows target | `cargo clippy --target x86_64-pc-windows-msvc` | – | ✅ no warnings (compile only) |

What the unit tests cover:

- **Fuzzy matching and ranking:** tier ordering, multi-word queries, acronyms, typos, diacritics, the prefilter's soundness, Top Hit and section grouping, frecency.
- **Shortcut validation:** parsing and canonical form, GTK accelerator round trips, reserved and invalid combinations, the manager's register-before-release semantics, conflict reporting, recording suspend and resume, auto-repeat suppression.
- **Settings:** serialization, lenient loading of a damaged section, range clamping, patching, corrupt-file recovery, first run.
- **Configuration migration:** chained steps, a missing version, a newer version, gaps in the chain, invalid versions.
- **Caps Lock state handling:** the initial reading, on/off flash, persistent mode, duplicates, disabling, preview.
- **Placement geometry:** offsets, edge flipping, HiDPI, multi-monitor origins, never covering the pointer.
- **Indicator renderer:** size, transparency, premultiplication.
- **XWayland input region:** the hole around the pointer.
- **File index:** exclusions, hidden files, limits, incremental rescans, the on-disk cache.
- **Linux helpers:** the desktop entry parser, icon theme lookup, autostart quoting, executable detection, history, atomic writes.

The integration tests (`src-tauri/src/integration_tests.rs`) cover:

- Gala shortcut registration, conflict detection that names the owner, and release;
- launching a real `.desktop` entry;
- live Caps Lock monitoring;
- the real pointer position through the XWayland tracker;
- incremental file indexing.

## End-to-end verification performed (elementary OS 8, Wayland, Gala)

These were driven on the real desktop. Key presses and pointer moves were injected through Mutter's RemoteDesktop API, and windows were inspected with X11 queries and webview snapshots.

| Check | Result |
|---|---|
| Ctrl+Space (Gala accelerator) opens Spotlight | ✅ |
| Spotlight receives keyboard focus on Wayland | ✅ `document.hasFocus() == true` after the shortcut |
| Shortcut toggles the window; Esc closes it | ✅ |
| Typing searches immediately; results update | ✅ "term" → Top Hit Terminal, Quick Actions, Folders, Files |
| Arrow navigation, Tab section jump, Ctrl+Backspace segment delete | ✅ |
| Enter opens the selected result (app) | ✅ LibreOffice launched and appeared under Recent |
| File search and open, including a file created *after* indexing | ✅ found about 1 s after creation (inotify), opened in the default editor |
| Paths outside the index are refused | ✅ `file:/etc/passwd` → "This item is not part of the search index." |
| Alt+Space conflict reported with its owner | ✅ "already used by the system shortcut “Activate window menu”" |
| Caps Lock ON shows the indicator at the pointer | ✅ pointer (1116, 826) → indicator body (1132, 846) |
| Indicator follows the pointer | ✅ indicator window moved with injected pointer motion |
| Caps Lock OFF flashes the off state, then hides | ✅ both windows unmapped after the duration |
| Indicator does not steal focus | ✅ override-redirect window, never focused; the typing app keeps focus |
| Indicator does not block clicks | ✅ empty input shape; the tracker leaves a click-through square at the pointer |
| Settings persist across restarts | ✅ |

## Platform checklist

Run this on each platform and record the results.

| # | Check | elementary 8 Wayland | elementary 8 X11 | Windows 10 | Windows 11 |
|---|---|---|---|---|---|
| 1 | Shortcut opens Spotlight, typing works immediately | ✅ | ☐ | ☐ | ☐ |
| 2 | Shortcut again / Esc / click outside hides it | ✅ | ☐ | ☐ | ☐ |
| 3 | Apps (including Settings, Calculator) found and launch | ✅ | ☐ | ☐ | ☐ |
| 4 | Files and folders found, open, Ctrl+Enter reveals | ✅ | ☐ | ☐ | ☐ |
| 5 | Search stays responsive with a large home folder | ✅ about 1,900 entries here; the 250k cap is tested by unit tests | ☐ | ☐ | ☐ |
| 6 | Change Shortcut records, validates, saves, survives restart | ✅ | ☐ | ☐ | ☐ |
| 7 | Caps Lock ON shows the indicator near the pointer | ✅ | ☐ | ☐ | ☐ |
| 8 | Caps Lock OFF hides it (brief off state) | ✅ | ☐ | ☐ | ☐ |
| 9 | Indicator follows the pointer | ✅ | ☐ | ☐ | ☐ |
| 10 | Indicator never blocks clicks or takes focus | ✅ | ☐ | ☐ | ☐ |
| 11 | Indicator correct on a second monitor / at 125–200% scale | ☐ (single 1080p monitor available) | ☐ | ☐ | ☐ |
| 12 | Tray menu items work | n/a (no tray host) | ☐ | ☐ | ☐ |
| 13 | Launch at login starts Lumio in the background | ☐ | ☐ | ☐ | ☐ |
| 14 | Quit releases shortcuts (Ctrl+Space works in other apps again) | ✅ | ☐ | ☐ | ☐ |

## Manual multi-monitor and scaling notes

- **Placement uses the monitor containing the pointer.** The indicator flips to the other side of the pointer at screen edges (covered by `capslock::geometry` tests).
- **On X11 and Windows, sizes scale with the monitor's scale factor.** On Wayland, sizes are in XWayland logical pixels.
