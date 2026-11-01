# Troubleshooting

**Logs:**

- Linux: `~/.local/share/com.lumio.desktop/logs/lumio.log`
- Windows: `%LOCALAPPDATA%\com.lumio.desktop\logs\lumio.log`

You can also use Settings → Advanced → *Application logs*. Settings → Advanced → *Diagnostic information* shows what Lumio detected (session type, shortcut backend, Caps Lock method, index state). Use **Copy** to include it in bug reports.

## The shortcut does nothing

1. **Check the shortcut's status.** Open Settings → Keyboard Shortcuts. A red icon means the system refused the combination, and the message says why (for example, *Alt+Space is already used by the system shortcut “Activate window menu”*). Choose another combination, or free it in your system's keyboard settings.
2. **On a Wayland desktop other than elementary OS,** apps cannot register global shortcuts. Add a custom shortcut in your system settings that runs `lumio --toggle`.
3. **If Lumio was killed (`kill -9`) on elementary OS,** Gala may still hold its shortcut until Lumio starts again. Start Lumio once and quit it normally.

## Spotlight opens but typing goes elsewhere

On Wayland, focus is granted only for shortcut presses that the compositor saw. Opening Spotlight with `lumio --toggle` from a terminal may not focus it. Use the Gala shortcut, or your desktop's custom-shortcut feature, which provides an activation token.

## No Caps Lock indicator

- Check Settings → Caps Lock Indicator → *Compatibility*:
  - *Caps Lock detection ✕* on Wayland means no keyboard exposes a Caps Lock LED (check `ls /sys/class/leds | grep capslock`).
  - *Pointer position ✕* on Wayland means XWayland is not running. Lumio then uses the system on-screen display on elementary OS.
  - *Text cursor position ✕* means the accessibility bus is not running (`at-spi2-core`).
- By default the indicator appears only in text fields. If nothing appears while you type, the app does not report its text cursor (see below); *Without a text cursor* lets you show it at the pointer or as the system OSD there instead.
- Use **Show Preview** to see the real indicator at your pointer.
- Lumio 0.1.3 and earlier showed the indicator only the first time on elementary OS (Gala treated it as a notification). Update to 0.1.4 or later.
- On X11 a compositing window manager is required. Gala and Mutter always composite.

## No indicator in a text field (or it appears at the pointer)

The focused app did not report a text cursor, so the *Without a text cursor* setting applied (by default: no indicator).

- The focus is not in a text field (for example a button or a list).
- The app does not expose text accessibility. **Chrome, VS Code and other Chromium or Electron apps** do so only when started with `--force-renderer-accessibility`, for example `flatpak run com.google.Chrome --force-renderer-accessibility`. Add the flag to the app's launcher to make it permanent.
- To see what Lumio finds, quit it and start it from a terminal with `LUMIO_LOG=debug lumio`, toggle Caps Lock in the app, and look for `text cursor lookup` lines (also shown in Settings → Advanced → Application logs).

## Hover effects flicker while the indicator is visible (Wayland)

This can happen only when the indicator is shown at the pointer (*Show next to: Mouse pointer*, or the pointer fallback) and the pointer *moves* during the display time. It comes from following the pointer through XWayland. Turn off *Follow the pointer* to keep the indicator still and avoid it.

## A file is not found

- Is its folder under *Folders to index* (default: your home folder) and not excluded?
- Hidden files need *Include hidden files and folders*.
- The index has a 250,000-entry cap. When it is reached, the index status line says so; add exclusions for large build or dependency folders.
- Use *Rebuild* (Spotlight pane) or *Clear Cache* (Advanced).

## No tray icon on elementary OS

elementary OS does not show tray icons out of the box. Install `wingpanel-indicator-ayatana`, log out and back in, and restart Lumio. Without it, open Lumio from the Applications menu to reach Settings.

## Settings were reset

If the settings file was unreadable, Lumio keeps the old file as `settings.corrupt-<time>.json` in the config folder and shows a notice in Settings.
