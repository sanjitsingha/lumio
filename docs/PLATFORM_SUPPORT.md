# Platform support

Legend: ✅ works and was verified · 🟡 implemented and compiled, not yet verified on real hardware · ⚠️ works with a limitation · ❌ not possible

## elementary OS 8: Wayland ("Secure Session", the default)

Verified on elementary OS 8 (Ubuntu 24.04 base), Pantheon, Gala, Wayland, WebKitGTK 2.52.

| Feature | Status | Notes |
|---|---|---|
| Spotlight window | ✅ | Opens, receives keyboard focus (Gala's input timestamp is used to satisfy focus-stealing prevention), closes on Esc or on blur. Gala centres new windows. The window has a fixed size and its input region is limited to the panel, so the empty area is click-through. |
| Global shortcuts | ✅ | Via Gala's `GrabAccelerator` D-Bus API. **Alt+Space is reserved** by Gala's window menu; Lummo reports this and **Ctrl+Space** works by default. |
| App search and launch | ✅ | XDG desktop entries, Flatpak exports, icons from the elementary icon theme; launched through GIO. |
| Web search | 🟡 | Browsers are the apps registered for `https` links (`g_app_info_get_all_for_type`); the search opens with `g_app_info_launch_uris`. |
| App actions | 🟡 | The selected app's `[Desktop Action]` groups (*New Window*, *New Private Window*…) are listed below it and run through GIO (`g_desktop_app_info_launch_action`). |
| File search and open | ✅ | Index plus inotify updates, opening with the default app, revealing in Files. File-type icons come from the icon theme (GIO content type by name). |
| Quick actions | ✅ | Settings (`io.elementary.settings`), Calculator, Files, default browser, Terminal. |
| Settings pages | 🟡 | *Turn on dark mode*, *Connect to Wi-Fi*… open System Settings at the page through the `settings://` links its plugs declare (`settings://desktop/appearance`, `settings://display/night-light`…), launched with GIO. Offered only when `settings://` is handled by `io.elementary.settings`. If System Settings is already open, it may come to the front without changing page. |
| Caps Lock detection | ✅ | Read from the keyboard's Caps Lock LED (`/sys/class/leds/*::capslock`), the only unprivileged system-wide source on Wayland. Polled every 150 ms (1 s while the indicator is disabled). Requires a keyboard that exposes a Caps Lock LED (practically all do). |
| Caps Lock indicator **at the text cursor** | ✅ | Verified with GTK 3 (`GtkTextView`, all positions along and across lines while the caret moves), LibreOffice Writer, Chrome started with `--force-renderer-accessibility`, and VS Code started with it (exact only in its *Screen Reader Optimized* mode; otherwise only the line is known). The caret comes from AT-SPI, the window origin from Gala's actors (see [ARCHITECTURE](ARCHITECTURE.md#caps-lock-indicator)). |
| Text cursor in GTK 4 apps (System Settings…) | ⚠️ | GTK 4.14 reports no character positions: in single-line fields the indicator appears below the field, in maximized, tiled or full-screen windows only (a floating GTK 4 window's position cannot be determined). Terminal (GTK 4) reports no text cursor at all. |
| Text cursor from the input method (IBus) | ✅ | GTK apps, the GTK 4 Terminal, VS Code, Chrome (with `--enable-wayland-ime --wayland-text-input-version=3`): the cursor every app reports to the input method, live while typing, including Chrome's address bar. Typing is unaffected (checked in GTK, Terminal and Chrome). |
| Indicator when a field gets focus with Caps Lock on | ✅ | Chrome field and Terminal prompt, no key press. Fields that take only numbers get none: spin buttons (AT-SPI), and digit, number, phone and PIN fields (the purpose apps give IBus). Not yet verified on a device. |
| Text cursor in Chrome (with the flag) | ✅ | Plain fields, text areas and rich editors (ChatGPT, WhatsApp Web, Gmail: the caret is followed into the paragraph holding it), the address bar (below its text: Chrome reports no character positions there), and Google Docs (it types into a field hidden outside the window and draws its own cursor, which Lummo reads instead). |
| Text cursor in Chromium/Electron apps | ⚠️ | Chrome, VS Code and other Chromium-based apps expose their text fields only when started with `--force-renderer-accessibility` (for Flatpak: `flatpak run com.google.Chrome --force-renderer-accessibility`). Otherwise no indicator is shown there, unless they report their cursor to the input method (VS Code does). |
| Caps Lock indicator **at the pointer** | ✅ | Pointer position comes from XWayland (see [ARCHITECTURE](ARCHITECTURE.md#caps-lock-indicator)). It follows the pointer. While the indicator is visible, apps under a *moving* pointer may see brief hover enter/leave flicker; clicks are never blocked at the pointer. Turn off *Follow the pointer* to avoid this. |
| No text cursor | ✅ | No indicator (it belongs to text fields). Where no floating indicator is possible at all, Gala's on-screen display is used. |
| Repeated toggles | ✅ | 8 of 8 consecutive on/off toggles visible (0.1.3 showed only the first one, see [ARCHITECTURE](ARCHITECTURE.md#caps-lock-indicator)). |
| System tray | ⚠️ | Stock elementary OS has no tray host, so no icon is shown. Install `wingpanel-indicator-ayatana` to get one. Opening Lummo from the Applications menu brings up Settings. |
| Launch at login | ✅ | XDG autostart entry (`~/.config/autostart/com.lumio.desktop.desktop`). |
| Light/dark/accent | ✅ | Follows the portal `color-scheme` and `accent-color` (elementary's accent). |

**Required:** XWayland must be enabled (it is by default) for the floating indicator. The text cursor position needs the accessibility bus (`at-spi2-core`, installed and running by default).

## elementary OS 8: X11 ("Classic Session")

| Feature | Status | Notes |
|---|---|---|
| Spotlight, apps, files, actions | 🟡 | Same code as Wayland. The window is positioned at about 22% from the top of the active monitor. |
| Global shortcuts | 🟡 | Gala backend (as on Wayland); `XGrabKey` through the Tauri plugin as fallback. |
| Caps Lock detection | 🟡 | XKB `StateNotify` events (no polling). |
| Caps Lock indicator at the text cursor | 🟡 | AT-SPI caret extents, which are screen coordinates on X11 (no Gala lookup needed). |
| Caps Lock indicator at the pointer | 🟡 | `QueryPointer` plus an override-redirect ARGB window. This is the same overlay code that was verified through XWayland. |

## Other Linux desktops

- **GNOME/KDE on X11:** global shortcuts via `XGrabKey`. The indicator and search work as on X11 above.
- **GNOME/KDE on Wayland:** GNOME Shell does not let ordinary apps use `GrabAccelerator`, so global shortcuts are unavailable. Bind a custom system shortcut to `lummo --toggle`. The Caps Lock indicator works through XWayland if it is running.

## Windows 10 / 11

> The Windows code compiles and passes clippy for `x86_64-pc-windows-msvc` with no warnings, but it **has not yet been run on a Windows machine**. Please run the test checklist in [TESTING.md](TESTING.md) and report results.

| Feature | Status | Implementation |
|---|---|---|
| Global shortcuts | 🟡 | `RegisterHotKey` (Tauri plugin). Conflicts are reported when Windows refuses a combination. |
| Spotlight window | 🟡 | Transparent undecorated window, positioned on the monitor with the pointer, resized to its content. |
| App search | 🟡 | `shell:AppsFolder` enumeration (classic and Store apps) with `IShellItemImageFactory` icons; launch via `ShellExecuteW("shell:AppsFolder\\<AUMID>")`. |
| Web search | 🟡 | Browsers come from `Clients\StartMenuInternet` (as in Default Apps); the chosen one is started with the search link, otherwise `ShellExecuteW` opens the default browser. |
| App actions | 🟡 | The app's jump-list tasks, as its taskbar button shows them: Chrome's *New Incognito window*, VS Code's *New Window*, Windows Terminal's profiles, Calculator's *Scientific*, Snipping Tool's *New screenshot in 5 seconds*. Windows has no API to read another app's jump list, so Lummo reads the file Windows keeps for it (`CustomDestinations\<CRC-64 of the AppUserModelID>.customDestinations-ms`), loading each task with the shell's own `ShellLink`. Programs start with `ShellExecuteW` (`runas` when the task asks to run as administrator), Store apps with `IApplicationActivationManager`. An app lists tasks once it has run; tasks for a browser profile that no longer exists (left by test tools) are skipped. |
| Files | 🟡 | Same index; recursive `ReadDirectoryChangesW`; hidden/system attributes respected; `ShellExecuteW` to open, `explorer /select,` to reveal. File-type icons from `IShellItemImageFactory`, as in Explorer. |
| Quick actions | 🟡 | `ms-settings:`, `calculator:`, Explorer, default browser (`AssocQueryStringW`), Windows Terminal or PowerShell. |
| Settings pages | 🟡 | *Turn on dark mode*, *Connect to Wi-Fi*… open Settings at the page through its `ms-settings:` links (`ms-settings:colors`, `ms-settings:nightlight`…), all of them present in Windows 11 build 26200's Settings app. |
| Caps Lock detection | 🟡 | `WH_KEYBOARD_LL` hook wakes a thread that reads `GetKeyState(VK_CAPITAL)`. |
| Indicator at the text cursor | 🟡 | The foreground thread's system caret (`GetGUIThreadInfo`), else the MSAA caret object (`OBJID_CARET`) of the focused window, else the caret UI Automation reports for the focused element (`TextPattern2` caret range, or the collapsed selection of a text pattern), which covers WinUI and UWP apps (Settings, the Start menu's search) and Windows Terminal. UI Automation comes last because Chromium reports the end of a placeholder as the caret of an empty field, and its caret is used only in text that can be edited, so clicking into a web page shows nothing. The caret must belong to the foreground window and lie inside it, so a field of a window behind it never gets the indicator, and switching windows hides it at once. Fields that take only numbers (`ES_NUMBER` edits, UI Automation spinners such as `<input type="number">`) get none. Apps that expose none of the three show no indicator. |
| Indicator at the pointer | 🟡 | `GetCursorPos`; per-pixel-alpha layered window (`WS_EX_LAYERED|TRANSPARENT|NOACTIVATE|TOOLWINDOW|TOPMOST`) via `UpdateLayeredWindow`. |
| Launch at login | 🟡 | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. |
| Tray | 🟡 | Standard notification-area icon and menu. |
| Theme/accent | 🟡 | `AppsUseLightTheme` and `DwmGetColorizationColor`. |

**Permissions:** none beyond a normal user account. Some antivirus products flag low-level keyboard hooks. Lummo's hook only looks for the Caps Lock key and does not record keystrokes.

## Known limitations

- **The text cursor is only as good as the app's accessibility.** Apps that draw text themselves without accessibility (games, some terminals, Chromium/Electron without `--force-renderer-accessibility`) report no caret.
- **Wayland pointer tracking needs XWayland.** If XWayland is disabled, Lummo uses the compositor OSD (on Gala) or reports the indicator as unavailable.
- **The hole-tracking approach is a deliberate trade-off.** It is the only consent-free way to observe the pointer on Wayland without screen-capture permissions (which would show a "screen is being shared" indicator).
- **Alt+Space cannot be the default on elementary OS** unless you rebind the window-menu shortcut in System Settings → Keyboard → Shortcuts.
- **Gala keeps accelerator grabs if Lummo is killed (`kill -9`).** The next Lummo start re-adopts and releases them correctly; a normal quit always releases them.
- **Indicator size on Wayland is in logical pixels**, because X clients render at 1× under XWayland. On 2× HiDPI the indicator is upscaled by the compositor.
