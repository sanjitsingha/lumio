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
- **Placement geometry:** offsets, edge flipping, HiDPI, multi-monitor origins, never covering the pointer or the text cursor's line.
- **Text cursor on Wayland:** matching the focused window to Gala's window actors (exact surface size, Chromium's scaled window size, same-size windows, full-screen panels that must not match), window titles, empty fields.
- **Settings from 0.1.x:** anchored to the text cursor after an update.
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
| Caps Lock ON shows the indicator at the pointer | ✅ pointer (1116, 826) → indicator body (1132, 846) (0.1.3: confirmed only by the window being mapped, see below) |
| Indicator follows the pointer | ✅ indicator window moved with injected pointer motion |
| Caps Lock OFF flashes the off state, then hides | ✅ both windows unmapped after the duration |
| Indicator does not steal focus | ✅ override-redirect window, never focused; the typing app keeps focus |
| Indicator does not block clicks | ✅ empty input shape; the tracker leaves a click-through square at the pointer |
| Settings persist across restarts | ✅ |

Since 0.1.4 the indicator is checked on **screenshots**, not only through X window state: 0.1.3 mapped its window correctly every time, but Gala drew it only the first time.

| Check (0.1.4) | Result |
|---|---|
| Caps Lock toggled 8 times in a row: indicator visible every time | ✅ 8/8 on screenshots (0.1.3: 1/6) |
| Indicator at the text cursor (GTK 3 text view on Wayland) | ✅ 6/6 toggles; caret (745, 432) → indicator body (741, 451), just below the caret; lookup 24–33 ms |
| Indicator follows the text cursor | ✅ every caret position along a line, at the line end and on the next line, equal to the characters' extents |
| Chrome started with `--force-renderer-accessibility` | ✅ caret found in a `<textarea>`; screen position within 2 px of the drawn caret (prototype of the same lookup; the scaled-window matching is unit-tested with the measured values) |
| Indicator at the text cursor in LibreOffice Writer | ✅ just below the caret at the end of the typed text; lookup 32–59 ms (0.1.5 showed nothing: the search skipped the document's paragraphs) |
| Chrome (test profile; the flags file adds `--force-renderer-accessibility`) | ✅ indicator just below the textarea's caret, 93–122 ms (0.1.7 missed it: Chrome sets its window's active flag late, so the focused window is now found by Gala's title) |
| LibreOffice Writer, empty paragraph | ✅ below the caret at the text start (0.1.7 used the page edge) |
| Chrome rich editor (`contenteditable`, caret in the 2nd paragraph) | ✅ just after the caret's character (0.1.9 stayed at the editor's top-left: the editor's own caret points at a child paragraph) |
| Chrome rich editor, caret moving every 120 ms | ✅ the indicator follows along the line |
| Chrome address bar | ✅ below the URL text (0.1.9 used scaled coordinates) |
| ChatGPT input (empty) | ✅ at the start of the box |
| GTK 4 search field (System Settings, maximized) | ✅ indicator below the field; first press 310 ms, then 30 ms |
| GTK 4 Terminal | ✅ no indicator (VTE for GTK 4 reports no text) |
| No text cursor (VS Code, Chrome without the flag) | ✅ lookup returns nothing in 10–18 ms; no indicator (default since 0.1.5; 0.1.4 used the pointer) |
| Settings → Show Preview | ✅ always at the pointer |

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
| 7 | Caps Lock ON shows the indicator below the text cursor (in a text field) | ✅ | ☐ | ☐ | ☐ |
| 7a | …and nothing outside text fields (default *Without a text cursor: Don't show*) | ✅ | ☐ | ☐ | ☐ |
| 7b | Toggling 10 times in a row shows it every time | ✅ | ☐ | ☐ | ☐ |
| 8 | Caps Lock OFF hides it (brief off state) | ✅ | ☐ | ☐ | ☐ |
| 9 | Indicator follows the text cursor while typing, and the pointer when anchored to it | ✅ | ☐ | ☐ | ☐ |
| 10 | Indicator never blocks clicks or takes focus | ✅ | ☐ | ☐ | ☐ |
| 11 | Indicator correct on a second monitor / at 125–200% scale | ☐ (single 1080p monitor available) | ☐ | ☐ | ☐ |
| 12 | Tray menu items work | n/a (no tray host) | ☐ | ☐ | ☐ |
| 13 | Launch at login starts Lumio in the background | ☐ | ☐ | ☐ | ☐ |
| 14 | Quit releases shortcuts (Ctrl+Space works in other apps again) | ✅ | ☐ | ☐ | ☐ |

## Manual multi-monitor and scaling notes

- **Placement uses the monitor containing the text cursor or pointer.** The indicator flips to the other side at screen edges and never covers the caret's line (covered by `capslock::geometry` tests).
- **On X11 and Windows, sizes scale with the monitor's scale factor.** On Wayland, sizes are in XWayland logical pixels.
