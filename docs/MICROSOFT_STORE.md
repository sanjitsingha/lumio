# Lummo in the Microsoft Store

Lummo goes to the Store as an **MSIX package**. Microsoft signs it, so no code-signing certificate is needed. The GitHub installer (`Lummo_x.y.z_x64-setup.exe`) carries on as before; both are the same `lummo.exe`.

The Store's copy notices it runs from a package (`platform::from_store`) and behaves accordingly:

| | GitHub installer | Microsoft Store |
|---|---|---|
| Updates | Lummo updates itself from GitHub releases | The Store updates it. Lummo never checks, and About says so |
| Open at login | `HKCU\…\Run` | the package's startup task (`LummoStartup`), also shown in Task Manager › Startup apps |
| Windows switches (dark mode, taskbar, File Explorer…) | write the user's registry | the same, because the package turns registry and file virtualization off (`unvirtualizedResources`) |

The package is described by [`src-tauri/msix/AppxManifest.xml`](../src-tauri/msix/AppxManifest.xml) and built by [`scripts/build-msix.ps1`](../scripts/build-msix.ps1).

---

## One-time setup

### 1. Reserve the name and copy the package identity

1. [Partner Center](https://partner.microsoft.com/dashboard/apps-and-games/overview) → **Apps and games** → **New product** → **MSIX or PWA app**, and reserve **Lummo**. If it is taken, reserve another name (for example *Lummo Search*) and change `DisplayName` in the manifest to match.
2. Open the product → **Product management** → **Product identity**. Copy its three values into [`src-tauri/msix/identity.json`](../src-tauri/msix/identity.json):

   ```json
   {
     "name": "<Package/Identity/Name>",
     "publisher": "<Package/Identity/Publisher, starts with CN=>",
     "publisherDisplayName": "<Package/Properties/PublisherDisplayName>"
   }
   ```

   These are not secrets; commit the file.

### 2. Windows SDK

`makeappx` and `makepri` come with the Windows SDK, already installed on the Windows laptop. On another PC:

```powershell
winget install Microsoft.WindowsSDK.10.0.26100
```

---

## Every Store release

Release on GitHub first (`npm run release -- patch --notes "…" --push`). That way the version in `Cargo.toml` is the one the Store gets, and Store versions only ever go up. Then:

```powershell
npm run build:msix
```

This builds Lummo and writes `src-tauri\target\msix\Lummo_<version>.0_x64.msix`. The Store requires the fourth part of the version to be 0.

Then in Partner Center → Lummo → **Start your submission** (or **Update** for later releases):

| Page | What to enter |
|---|---|
| **Pricing and availability** | Free; all markets, or the ones you want |
| **Properties** | Category **Productivity** (subcategory none). Privacy policy URL: `https://lummo.space/privacy-policy`. Website: `https://lummo.space`. Support contact: your support email or `https://github.com/sanjitsingha/lumio/issues`. Under **Product declarations**, leave *This app depends on non-Microsoft drivers or NT services* off. |
| **Age ratings** | Answer the questionnaire: no violence, no user-generated content shared with others, no purchases, no location shared. That gives the lowest rating (3+ / Everyone). |
| **Packages** | Upload the `.msix`. Device family: Desktop only. |
| **Store listings** | English (United States): the text below, plus screenshots |
| **Submission options** | **Notes for certification**: paste the text under *Restricted capabilities* below |

Certification usually takes 1 to 3 business days.

### Restricted capabilities

The package declares two restricted capabilities. Partner Center asks why when you upload it; paste this in the box it shows, and again in **Notes for certification**:

> **runFullTrust**: Lummo is a desktop app (Tauri/Win32). It registers global keyboard shortcuts, shows a search window over other apps, indexes the user's files, opens apps and files the user picks, and reads the text cursor position (UI Automation) to show a Caps Lock indicator next to it.
>
> **unvirtualizedResources**: Lummo's system commands change Windows settings on request, as the Settings app does: light or dark mode, taskbar alignment and auto-hide, File Explorer's hidden files and file extensions, and app access to the camera and microphone. These settings live in the user's registry (HKEY_CURRENT_USER). With registry virtualization, the writes would go to a private copy and do nothing. Lummo only changes a setting when the user runs that command, and the commands that could lose work ask first. To test: open Lummo with Ctrl+Space (or Alt+Space) and type "toggle dark mode", then press Enter; Windows switches between light and dark.

### Screenshots

At least one, up to ten: PNG, 1366×768 or larger (1920×1080 is best). Good ones:

1. Spotlight with results for an app (for example "photo"), showing the sections.
2. Spotlight showing a system command ("toggle dark mode" or "volume 50").
3. Quick files: clipboard history and screenshots (Tab in an empty Spotlight).
4. Settings → Shortcut with the category dropdown open.
5. The Caps Lock indicator next to a text cursor.

---

## Store listing (English)

**Product name**: Lummo

**Short description** (shown in search results)

> Spotlight-style search for Windows: apps, files, settings and 300 system commands, one shortcut away.

**Description**

> Lummo brings a fast, keyboard-first search to Windows. Press one shortcut and type: open apps, find files and folders, jump to the right page of Settings, or run a system command, without reaching for the mouse.
>
> Search that understands you
> • Finds apps, files, folders and Settings pages however you type them: "darkmode", "dark mode", "wifi" or "wi-fi"
> • Learns what you open most and puts it first
> • A calculator, currency and unit answers as you type
> • Every search can also go to the web, in the browser you choose
>
> More than 300 system commands
> • Turn Wi-Fi and Bluetooth on or off, set the volume, mute the microphone
> • Keep your PC awake, change the power mode, the screen and sleep timeouts
> • Show hidden files and file extensions, auto-hide or align the taskbar, switch dark mode
> • Snap, center or pin windows on top, switch virtual desktops, take screenshots
> • Give any command, app or Settings page its own alias or keyboard shortcut
>
> Quick files
> • Press Tab for your clipboard history and recent screenshots, and copy any of them again
>
> Caps Lock indicator
> • A small badge right at your text cursor when Caps Lock is on, as on a Mac
>
> Private by design
> • Your searches, files and clipboard stay on your PC
> • Optional anonymous usage statistics that never include what you search or open

**What's new in this version**: use the release notes of that version (the tag message).

**Product features** (one per line, up to 20)

```
Spotlight-style search behind one keyboard shortcut
Apps, files, folders and Settings pages in one search
More than 300 system commands that do the action
Wi-Fi, Bluetooth, volume and microphone controls
Keep awake, power mode and sleep timeouts
File Explorer and taskbar options in a keystroke
Window snapping, centering and always on top
An alias and a hotkey for any command or app
Clipboard history and recent screenshots
Calculator, currency and unit answers
Caps Lock indicator at the text cursor
Light and dark, follows Windows
```

**Search terms** (up to 7)

```
spotlight
launcher
search
productivity
shortcuts
clipboard
caps lock
```

**Copyright**: © 2026 Sanjit Singha

---

## Testing the package before uploading

Optional. It needs **Developer Mode** (Settings → System → For developers):

```powershell
npm run build:msix -- -SkipBuild          # package the last release build
Add-AppxPackage -Register src-tauri\target\msix\layout\AppxManifest.xml
```

Lummo appears in Start as the packaged app. About should say *The Microsoft Store keeps Lummo up to date*, and Settings → General → Open at login should show up in Task Manager › Startup apps. Remove it with `Get-AppxPackage *Lummo* | Remove-AppxPackage`.

To check only that the package builds, without your Partner Center identity: `npm run build:msix -- -Test`. The Store does not accept that package.
