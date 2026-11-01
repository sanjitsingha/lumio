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
- Use **Show Preview** to see the real indicator at your pointer.
- On X11 a compositing window manager is required. Gala and Mutter always composite.

## Hover effects flicker while the indicator is visible (Wayland)

This is expected while the pointer *moves* during the indicator's display time. It comes from following the pointer through XWayland. Turn off *Follow the pointer* to keep the indicator still and avoid it.

## A file is not found

- Is its folder under *Folders to index* (default: your home folder) and not excluded?
- Hidden files need *Include hidden files and folders*.
- The index has a 250,000-entry cap. When it is reached, the index status line says so; add exclusions for large build or dependency folders.
- Use *Rebuild* (Spotlight pane) or *Clear Cache* (Advanced).

## No tray icon on elementary OS

elementary OS does not show tray icons out of the box. Install `wingpanel-indicator-ayatana`, log out and back in, and restart Lumio. Without it, open Lumio from the Applications menu to reach Settings.

## Settings were reset

If the settings file was unreadable, Lumio keeps the old file as `settings.corrupt-<time>.json` in the config folder and shows a notice in Settings.
