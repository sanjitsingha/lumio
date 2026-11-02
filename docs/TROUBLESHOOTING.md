# Troubleshooting

**Logs:**

- Linux: `~/.local/share/com.lumio.desktop/logs/lummo.log`
- Windows: `%LOCALAPPDATA%\com.lumio.desktop\logs\lummo.log`

The log records what Lummo detected at startup (session type, shortcut backend, Caps Lock method, index state). Attach it to bug reports.

## The shortcut does nothing

1. **Check the shortcut's status.** Open Settings → Spotlight → Keyboard shortcut. A red icon means the system refused the combination, and the message says why (for example, *Alt+Space is already used by the system shortcut “Activate window menu”*). Choose another combination, or free it in your system's keyboard settings.
2. **On a Wayland desktop other than elementary OS,** apps cannot register global shortcuts. Add a custom shortcut in your system settings that runs `lummo --toggle`.
3. **If Lummo was killed (`kill -9`) on elementary OS,** Gala may still hold its shortcut until Lummo starts again. Start Lummo once and quit it normally.

## Spotlight opens but typing goes elsewhere

On Wayland, focus is granted only for shortcut presses that the compositor saw. Opening Spotlight with `lummo --toggle` from a terminal may not focus it. Use the Gala shortcut, or your desktop's custom-shortcut feature, which provides an activation token.

## No Caps Lock indicator

- Settings → Caps Lock Indicator shows a message at the top when the indicator cannot work normally on this computer (for example *Caps Lock can't be detected*: no keyboard exposes a Caps Lock LED; check `ls /sys/class/leds | grep capslock`). The log lists the details.
- The indicator appears only in text fields. If nothing appears while you type, the app does not report its text cursor (see below).
- Lummo 0.1.3 and earlier showed the indicator only the first time on elementary OS (Gala treated it as a notification). Update to 0.1.4 or later.
- On X11 a compositing window manager is required. Gala and Mutter always composite.

## No indicator in a text field

The focused app did not report a text cursor.

- The focus is not in a text field (for example a button or a list).
- The app does not expose text accessibility. **Chrome, VS Code and other Chromium or Electron apps** do so only when started with `--force-renderer-accessibility`, for example `flatpak run com.google.Chrome --force-renderer-accessibility`. To make it permanent: for Flatpak Chrome add the lines `--force-renderer-accessibility`, `--enable-wayland-ime` and `--wayland-text-input-version=3` to `~/.var/app/com.google.Chrome/config/chrome-flags.conf` (the last two let Chrome report its cursor, including the address bar's, to the input method), then quit Chrome completely (menu → Exit) and start it again; for VS Code add `"force-renderer-accessibility": true` to `~/.vscode/argv.json`. VS Code then switches to *Screen Reader Optimized* mode (sound cues, changed editor behaviour), which gives the exact position; with `"editor.accessibilitySupport": "off"` it stays normal but only the cursor's line is known.
- To see what Lummo finds, quit it and start it from a terminal with `LUMMO_LOG=debug lummo`, toggle Caps Lock in the app, and look for `text cursor lookup` lines (also written to the log file).

## A file is not found

- Is its folder under *Folders to index* (default: your home folder) and not excluded?
- Hidden files need *Include hidden files and folders*.
- The index has a 250,000-entry cap. When it is reached, the index status line says so; add exclusions for large build or dependency folders.
- Use *Rebuild* (Spotlight pane) or *Clear Cache* (General → Maintenance).

## No tray icon on elementary OS

elementary OS does not show tray icons out of the box. Install `wingpanel-indicator-ayatana`, log out and back in, and restart Lummo. Without it, open Lummo from the Applications menu to reach Settings.

## Settings were reset

If the settings file was unreadable, Lummo keeps the old file as `settings.corrupt-<time>.json` in the config folder and shows a notice in Settings.
