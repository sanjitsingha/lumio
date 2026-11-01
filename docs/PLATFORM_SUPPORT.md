# Platform support

Legend: ✅ works and was verified · 🟡 implemented and compiled, not yet verified on real hardware · ⚠️ works with a limitation · ❌ not possible

## elementary OS 8: Wayland ("Secure Session", the default)

Verified on elementary OS 8 (Ubuntu 24.04 base), Pantheon, Gala, Wayland, WebKitGTK 2.52.

| Feature | Status | Notes |
|---|---|---|
| Spotlight window | ✅ | Opens, receives keyboard focus (Gala's input timestamp is used to satisfy focus-stealing prevention), closes on Esc or on blur. Gala centres new windows. The window has a fixed size and its input region is limited to the panel, so the empty area is click-through. |
| Global shortcuts | ✅ | Via Gala's `GrabAccelerator` D-Bus API. **Alt+Space is reserved** by Gala's window menu; Lumio reports this and **Ctrl+Space** works by default. |
| App search and launch | ✅ | XDG desktop entries, Flatpak exports, icons from the elementary icon theme; launched through GIO. |
| File search and open | ✅ | Index plus inotify updates, opening with the default app, revealing in Files. |
| Quick actions | ✅ | Settings (`io.elementary.settings`), Calculator, Files, default browser, Terminal. |
| Caps Lock detection | ✅ | Read from the keyboard's Caps Lock LED (`/sys/class/leds/*::capslock`), the only unprivileged system-wide source on Wayland. Polled every 150 ms (1 s while the indicator is disabled). Requires a keyboard that exposes a Caps Lock LED (practically all do). |
| Caps Lock indicator **at the pointer** | ✅ | Pointer position comes from XWayland (see [ARCHITECTURE](ARCHITECTURE.md#caps-lock-indicator)). It follows the pointer. While the indicator is visible, apps under a *moving* pointer may see brief hover enter/leave flicker; clicks are never blocked at the pointer. Turn off *Follow the pointer* to avoid this. |
| Indicator fallback | ✅ | If the pointer cannot be located (another app holds a pointer grab), Gala's on-screen display is shown instead. |
| System tray | ⚠️ | Stock elementary OS has no tray host, so no icon is shown. Install `wingpanel-indicator-ayatana` to get one. Opening Lumio from the Applications menu brings up Settings. |
| Launch at login | ✅ | XDG autostart entry (`~/.config/autostart/com.lumio.desktop.desktop`). |
| Light/dark/accent | ✅ | Follows the portal `color-scheme` and `accent-color` (elementary's accent). |

**Required:** XWayland must be enabled (it is by default) for the cursor-anchored indicator.

## elementary OS 8: X11 ("Classic Session")

| Feature | Status | Notes |
|---|---|---|
| Spotlight, apps, files, actions | 🟡 | Same code as Wayland. The window is positioned at about 22% from the top of the active monitor. |
| Global shortcuts | 🟡 | Gala backend (as on Wayland); `XGrabKey` through the Tauri plugin as fallback. |
| Caps Lock detection | 🟡 | XKB `StateNotify` events (no polling). |
| Caps Lock indicator at the pointer | 🟡 | `QueryPointer` plus an override-redirect ARGB window. This is the same overlay code that was verified through XWayland. |

## Other Linux desktops

- **GNOME/KDE on X11:** global shortcuts via `XGrabKey`. The indicator and search work as on X11 above.
- **GNOME/KDE on Wayland:** GNOME Shell does not let ordinary apps use `GrabAccelerator`, so global shortcuts are unavailable. Bind a custom system shortcut to `lumio --toggle`. The Caps Lock indicator works through XWayland if it is running.

## Windows 10 / 11

> The Windows code compiles and passes clippy for `x86_64-pc-windows-msvc` with no warnings, but it **has not yet been run on a Windows machine**. Please run the test checklist in [TESTING.md](TESTING.md) and report results.

| Feature | Status | Implementation |
|---|---|---|
| Global shortcuts | 🟡 | `RegisterHotKey` (Tauri plugin). Conflicts are reported when Windows refuses a combination. |
| Spotlight window | 🟡 | Transparent undecorated window, positioned on the monitor with the pointer, resized to its content. |
| App search | 🟡 | `shell:AppsFolder` enumeration (classic and Store apps) with `IShellItemImageFactory` icons; launch via `ShellExecuteW("shell:AppsFolder\\<AUMID>")`. |
| Files | 🟡 | Same index; recursive `ReadDirectoryChangesW`; hidden/system attributes respected; `ShellExecuteW` to open, `explorer /select,` to reveal. |
| Quick actions | 🟡 | `ms-settings:`, `calculator:`, Explorer, default browser (`AssocQueryStringW`), Windows Terminal or PowerShell. |
| Caps Lock detection | 🟡 | `WH_KEYBOARD_LL` hook wakes a thread that reads `GetKeyState(VK_CAPITAL)`. |
| Indicator at the pointer | 🟡 | `GetCursorPos`; per-pixel-alpha layered window (`WS_EX_LAYERED|TRANSPARENT|NOACTIVATE|TOOLWINDOW|TOPMOST`) via `UpdateLayeredWindow`. |
| Launch at login | 🟡 | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. |
| Tray | 🟡 | Standard notification-area icon and menu. |
| Theme/accent | 🟡 | `AppsUseLightTheme` and `DwmGetColorizationColor`. |

**Permissions:** none beyond a normal user account. Some antivirus products flag low-level keyboard hooks. Lumio's hook only looks for the Caps Lock key and does not record keystrokes.

## Known limitations

- **Wayland pointer tracking needs XWayland.** If XWayland is disabled, Lumio uses the compositor OSD (on Gala) or reports the indicator as unavailable.
- **The hole-tracking approach is a deliberate trade-off.** It is the only consent-free way to observe the pointer on Wayland without screen-capture permissions (which would show a "screen is being shared" indicator).
- **Alt+Space cannot be the default on elementary OS** unless you rebind the window-menu shortcut in System Settings → Keyboard → Shortcuts.
- **Gala keeps accelerator grabs if Lumio is killed (`kill -9`).** The next Lumio start re-adopts and releases them correctly; a normal quit always releases them.
- **Indicator size on Wayland is in logical pixels**, because X clients render at 1× under XWayland. On 2× HiDPI the indicator is upscaled by the compositor.
