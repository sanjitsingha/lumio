//! Pages of the system's settings app ("Open dark mode settings", "Connect to
//! Wi-Fi"), opened at the right page through the OS's own deep links:
//! `ms-settings:` on Windows, `settings://` on elementary OS (System
//! Settings, formerly Switchboard). On elementary OS the on/off ones are
//! real switches instead (`toggle:`, see `platform::linux::toggles`): they
//! change the setting and open nothing.
//!
//! To add a page, add an entry to [`PAGES`] with its icon and its link on
//! each platform that has it. A page with no link for the running platform is
//! not offered there. Each page shows its own Lucide icon, drawn bare in white
//! (the frontend's `SYMBOLS` must have it).

use std::sync::RwLock;

use super::fuzzy::{fold, Query, CONTIGUOUS_MATCH, PREFIX_MATCH};
use super::types::{ExecuteMode, ResultIcon, ResultKind, SearchResult};
use super::{ExecutedItem, SearchContext, SearchProvider};
use crate::error::{AppError, AppResult};
use crate::platform;
use crate::storage::settings::SpotlightSettings;

/// Where a page is on one platform.
pub struct Link {
    /// `ms-settings:…` (Windows), `settings://…` (elementary OS) or
    /// `toggle:…` (a switch on elementary OS).
    pub uri: &'static str,
    /// Its place in the settings app, shown below the title.
    pub path: &'static str,
    /// What it is called on this platform, where that differs from the page's title.
    pub title: Option<&'static str>,
    /// More words it is found by on this platform.
    pub keywords: &'static [&'static str],
}

pub struct SettingPage {
    pub id: &'static str,
    /// The Lucide icon it shows ("moon"), the same on every platform.
    pub icon: &'static str,
    /// What the page lets you do, phrased the way people search for it. A page
    /// for an on/off setting says it opens Settings ("Open dark mode settings"),
    /// never "Turn on …": that would read like the quick action next to it that
    /// changes the setting at once. Keep "turn on …" as a keyword instead.
    pub title: &'static str,
    pub keywords: &'static [&'static str],
    pub windows: Option<Link>,
    /// elementary OS 8 System Settings. Each plug declares the links it
    /// accepts (`supported_settings` in its `Plug.vala`); an unknown one
    /// falls back to the closest plug, or to the overview.
    pub elementary: Option<Link>,
}

const fn page(
    id: &'static str,
    icon: &'static str,
    title: &'static str,
    keywords: &'static [&'static str],
    windows: Option<Link>,
    elementary: Option<Link>,
) -> SettingPage {
    SettingPage { id, icon, title, keywords, windows, elementary }
}

const fn win(uri: &'static str, path: &'static str) -> Option<Link> {
    Some(Link { uri, path, title: None, keywords: &[] })
}

const fn eos(uri: &'static str, path: &'static str) -> Option<Link> {
    Some(Link { uri, path, title: None, keywords: &[] })
}

/// A switch on elementary OS: running it flips the setting. `title` starts
/// with "Toggle ", and "turn on …", "disable …" and the like find it too.
const fn switch(
    title: &'static str,
    uri: &'static str,
    path: &'static str,
    keywords: &'static [&'static str],
) -> Option<Link> {
    Some(Link { uri, path, title: Some(title), keywords })
}

impl SettingPage {
    /// Its name on the running platform.
    pub fn title_here(&'static self) -> &'static str {
        Desktop::current().link(self).and_then(|l| l.title).unwrap_or(self.title)
    }
}

#[rustfmt::skip]
pub const PAGES: &[SettingPage] = &[
    // Appearance
    page("dark-mode", "moon", "Open dark mode settings",
        &["dark mode", "turn on dark mode", "dark theme", "dark style", "night mode", "black theme", "theme", "appearance", "color mode", "light or dark"],
        win("ms-settings:colors", "Personalization › Colors"),
        switch("Toggle dark mode", "toggle:dark-mode", "Desktop › Appearance",
            &["light mode", "light theme", "light style", "white theme", "bright theme", "day mode", "turn on light mode", "turn off light mode"])),
    page("light-mode", "sun", "Open light mode settings",
        &["light mode", "turn on light mode", "light theme", "light style", "white theme", "bright theme", "day mode", "color mode"],
        win("ms-settings:colors", "Personalization › Colors"),
        None),
    page("accent-color", "palette", "Change accent color",
        &["accent color", "accent colour", "highlight color", "window color", "colors", "colours"],
        win("ms-settings:colors", "Personalization › Colors"),
        eos("settings://desktop/appearance", "Desktop › Appearance")),
    page("wallpaper", "image", "Change wallpaper",
        &["wallpaper", "background", "desktop background", "desktop picture", "backdrop", "slideshow", "dim wallpaper"],
        win("ms-settings:personalization-background", "Personalization › Background"),
        eos("settings://desktop/appearance/wallpaper", "Desktop › Wallpaper")),
    page("themes", "paintbrush", "Change theme",
        &["themes", "theme", "personalize", "personalization", "desktop icons", "sounds theme"],
        win("ms-settings:themes", "Personalization › Themes"),
        None),
    page("lock-screen", "lock", "Lock screen settings",
        &["lock screen", "lockscreen", "screen lock", "lock automatically", "auto lock", "lock on suspend", "screen saver", "screensaver"],
        win("ms-settings:lockscreen", "Personalization › Lock screen"),
        eos("settings://security/locking", "Security & Privacy › Locking")),
    page("start-menu", "layout-grid", "Start menu settings",
        &["start menu", "start", "pinned apps", "recommended", "recently added apps"],
        win("ms-settings:personalization-start", "Personalization › Start"),
        None),
    page("taskbar", "panel-bottom", "Taskbar settings",
        &["taskbar", "task bar", "system tray", "tray icons", "hide taskbar", "auto hide taskbar", "taskbar alignment", "widgets"],
        win("ms-settings:taskbar", "Personalization › Taskbar"),
        None),
    page("dock", "panel-bottom", "Dock & panel settings",
        &["dock", "panel", "top bar", "hide dock", "auto hide dock", "dock icon size", "panel translucency"],
        None,
        eos("settings://desktop/dock", "Desktop › Dock & Panel")),
    page("fonts", "type", "Manage fonts",
        &["fonts", "font", "typeface", "install font", "font family"],
        win("ms-settings:fonts", "Personalization › Fonts"),
        None),
    page("text-size", "a-large-small", "Change text size",
        &["text size", "font size", "bigger text", "larger text", "smaller text", "make text bigger", "text scaling", "dyslexia"],
        win("ms-settings:easeofaccess-display", "Accessibility › Text size"),
        eos("settings://desktop/text", "Desktop › Text")),

    // Display
    page("display", "monitor", "Display settings",
        &["display", "monitor", "screen", "resolution", "screen resolution", "scale", "scaling", "rotation", "rotate screen",
          "orientation", "second monitor", "external monitor", "multiple displays", "mirror", "refresh rate", "hdr"],
        win("ms-settings:display", "System › Display"),
        eos("settings://display", "Displays")),
    page("brightness", "sun-medium", "Change brightness",
        &["brightness", "screen brightness", "dim screen", "dimmer", "brighter", "adaptive brightness", "backlight"],
        win("ms-settings:display", "System › Display"),
        eos("settings://power", "Power")),
    page("night-light", "sunset", "Open Night Light settings",
        &["night light", "turn on night light", "blue light", "night mode", "night shift", "warm colors", "color temperature", "eye strain", "redshift"],
        win("ms-settings:nightlight", "System › Display › Night light"),
        switch("Toggle Night Light", "toggle:night-light", "Displays › Night Light",
            &[])),
    page("color-filters", "blend", "Open color filter settings",
        &["color filters", "turn on color filters", "colour filters", "color blind", "colorblind", "grayscale", "greyscale", "black and white", "monochrome",
          "invert colors"],
        win("ms-settings:easeofaccess-colorfilter", "Accessibility › Color filters"),
        switch("Toggle grayscale", "toggle:grayscale", "Displays › Filters",
            &["turn on color filters", "turn off color filters"])),
    page("graphics", "monitor-cog", "Graphics settings",
        &["graphics", "gpu", "graphics card", "gpu preference", "high performance gpu", "hardware accelerated gpu scheduling"],
        win("ms-settings:display-advancedgraphics", "System › Display › Graphics"),
        None),

    // Sound
    page("sound", "volume-2", "Sound settings",
        &["sound", "audio", "volume", "speakers", "headphones", "output device", "audio output", "mono audio", "balance"],
        win("ms-settings:sound", "System › Sound"),
        eos("settings://sound/output", "Sound › Output")),
    page("microphone", "mic", "Microphone settings",
        &["microphone", "mic", "input device", "audio input", "input volume", "recording", "headset mic"],
        win("ms-settings:sound", "System › Sound"),
        eos("settings://sound/input", "Sound › Input")),
    page("volume-mixer", "sliders-horizontal", "Volume mixer",
        &["volume mixer", "mixer", "app volume", "per app volume", "application volume"],
        win("ms-settings:apps-volume", "System › Sound › Volume mixer"),
        eos("settings://sound/applications", "Sound › Applications")),

    // Notifications
    page("notifications", "bell", "Notification settings",
        &["notifications", "notification", "alerts", "banners", "notification sounds", "badges"],
        win("ms-settings:notifications", "System › Notifications"),
        eos("settings://notifications", "Notifications")),
    page("do-not-disturb", "bell-off", "Open Do Not Disturb settings",
        &["do not disturb", "turn on do not disturb", "dnd", "focus", "focus assist", "quiet hours", "silence notifications", "mute notifications"],
        win("ms-settings:notifications", "System › Notifications"),
        switch("Toggle Do Not Disturb", "toggle:do-not-disturb", "Notifications",
            &[])),

    // Power and storage
    page("power", "battery", "Power & battery settings",
        &["power", "battery", "power mode", "power plan", "power profile", "battery percentage", "battery usage", "lid",
          "power button", "energy"],
        win("ms-settings:powersleep", "System › Power & battery"),
        eos("settings://power", "Power")),
    page("sleep", "bed", "Sleep and screen timeout",
        &["sleep", "screen timeout", "turn off screen", "screen off", "suspend", "hibernate", "close lid", "idle"],
        win("ms-settings:powersleep", "System › Power & battery"),
        eos("settings://power", "Power")),
    page("battery-saver", "battery-low", "Open battery saver settings",
        &["battery saver", "turn on battery saver", "energy saver", "power saver", "low power mode", "save battery"],
        win("ms-settings:batterysaver", "System › Power & battery"),
        switch("Toggle battery saver", "toggle:battery-saver", "Power",
            &[])),
    page("storage", "hard-drive", "Free up storage space",
        &["storage", "disk space", "free up space", "free space", "disk cleanup", "drives", "hard drive", "ssd", "disk usage"],
        win("ms-settings:storagesense", "System › Storage"),
        None),
    page("auto-cleanup", "brush-cleaning", "Clean up files automatically",
        &["storage sense", "housekeeping", "empty trash", "empty recycle bin", "delete temporary files", "temp files",
          "auto delete", "clean up", "cleanup"],
        win("ms-settings:storagepolicies", "System › Storage › Storage Sense"),
        eos("settings://privacy/trash", "Security & Privacy › Housekeeping")),

    // System
    page("multitasking", "layout-dashboard", "Multitasking settings",
        &["multitasking", "snap windows", "snap layouts", "window snapping", "virtual desktops", "workspaces", "hot corners",
          "alt tab", "shake window"],
        win("ms-settings:multitasking", "System › Multitasking"),
        eos("settings://desktop/multitasking", "Desktop › Multitasking")),
    page("clipboard", "clipboard", "Clipboard history",
        &["clipboard", "clipboard history", "copy paste", "paste history", "sync clipboard"],
        win("ms-settings:clipboard", "System › Clipboard"),
        None),
    page("remote-desktop", "screen-share", "Remote Desktop settings",
        &["remote desktop", "rdp", "remote access", "remote control", "connect remotely"],
        win("ms-settings:remotedesktop", "System › Remote Desktop"),
        None),
    page("sharing", "share-2", "Sharing settings",
        &["sharing", "file sharing", "media sharing", "share files", "network sharing", "dlna", "media library"],
        None,
        eos("settings://network/share", "Sharing")),
    page("about", "info", "About this computer",
        &["about", "system information", "system info", "computer name", "device name", "rename pc", "rename computer",
          "hostname", "os version", "windows version", "version", "edition", "product id"],
        win("ms-settings:about", "System › About"),
        eos("settings://about/os", "System › Operating System")),
    page("hardware", "cpu", "View hardware specs",
        &["hardware", "specs", "specifications", "device specifications", "ram", "memory", "processor", "cpu", "graphics card",
          "system type", "64 bit"],
        win("ms-settings:about", "System › About"),
        eos("settings://about/hardware", "System › Hardware")),
    page("drivers", "package", "Install drivers",
        &["drivers", "driver", "nvidia", "graphics driver", "proprietary drivers", "additional drivers"],
        None,
        eos("settings://about/drivers", "System › Drivers")),
    page("firmware", "circuit-board", "Update firmware",
        &["firmware", "bios", "uefi", "fwupd", "firmware update"],
        None,
        eos("settings://about/firmware", "System › Firmware")),
    page("updates", "refresh-cw", "Check for updates",
        &["update", "updates", "windows update", "system update", "software update", "os update", "upgrade", "patches",
          "restart to update"],
        win("ms-settings:windowsupdate", "Windows Update"),
        eos("settings://about/os", "System › Operating System")),
    page("update-history", "history", "View update history",
        &["update history", "installed updates", "uninstall updates", "recent updates"],
        win("ms-settings:windowsupdate-history", "Windows Update › Update history"),
        None),
    page("troubleshoot", "wrench", "Troubleshoot problems",
        &["troubleshoot", "troubleshooter", "fix problems", "diagnose", "get help", "repair"],
        win("ms-settings:troubleshoot", "System › Troubleshoot"),
        None),
    page("recovery", "life-buoy", "Recovery options",
        &["recovery", "reset pc", "reset this pc", "factory reset", "reinstall windows", "advanced startup", "safe mode",
          "go back", "restore"],
        win("ms-settings:recovery", "System › Recovery"),
        None),
    page("activation", "key-round", "Activate Windows",
        &["activation", "activate", "product key", "license", "licence", "windows license", "upgrade edition"],
        win("ms-settings:activation", "System › Activation"),
        None),
    page("optional-features", "blocks", "Optional features",
        &["optional features", "windows features", "add a feature", "features on demand", "openssh", "language features"],
        win("ms-settings:optionalfeatures", "System › Optional features"),
        None),
    page("developers", "code", "Developer settings",
        &["developer mode", "developers", "developer", "dev mode", "sudo", "dev drive", "sideload"],
        win("ms-settings:developers", "System › For developers"),
        None),

    // Bluetooth and devices
    page("bluetooth", "bluetooth", "Open Bluetooth settings",
        &["bluetooth", "turn on bluetooth", "bt", "bluetooth on", "bluetooth off", "wireless devices"],
        win("ms-settings:bluetooth", "Bluetooth & devices"),
        switch("Toggle Bluetooth", "toggle:bluetooth", "Bluetooth",
            &[])),
    page("bluetooth-pair", "bluetooth-searching", "Pair a Bluetooth device",
        &["pair", "pairing", "add device", "add a device", "connect headphones", "bluetooth headphones", "wireless headphones",
          "earbuds", "airpods", "bluetooth speaker", "wireless mouse", "wireless keyboard", "controller"],
        win("ms-settings:bluetooth", "Bluetooth & devices"),
        eos("settings://network/bluetooth", "Bluetooth")),
    page("printers", "printer", "Add a printer or scanner",
        &["printer", "printers", "scanner", "scanners", "print", "add printer", "print queue", "default printer"],
        win("ms-settings:printers", "Bluetooth & devices › Printers & scanners"),
        eos("settings://printer", "Printers")),
    page("mouse", "mouse", "Mouse settings",
        &["mouse", "mouse speed", "pointer speed", "scroll", "scrolling", "scroll direction", "natural scrolling",
          "scroll speed", "left handed", "primary button", "double click", "mouse acceleration"],
        win("ms-settings:mousetouchpad", "Bluetooth & devices › Mouse"),
        eos("settings://input/pointer/mouse", "Mouse & Touchpad › Mouse")),
    page("touchpad", "touchpad", "Touchpad settings",
        &["touchpad", "trackpad", "tap to click", "two finger scroll", "disable touchpad", "palm rejection"],
        win("ms-settings:devices-touchpad", "Bluetooth & devices › Touchpad"),
        eos("settings://input/pointer/touch", "Mouse & Touchpad › Touchpad")),
    page("gestures", "hand", "Touchpad gestures",
        &["gestures", "swipe", "three finger", "four finger", "multitouch", "pinch to zoom", "multi finger"],
        win("ms-settings:devices-touchpad", "Bluetooth & devices › Touchpad"),
        eos("settings://input/pointer/gestures", "Mouse & Touchpad › Gestures")),
    page("keyboard", "keyboard", "Keyboard settings",
        &["keyboard", "typing", "key repeat", "repeat delay", "repeat rate", "autocorrect", "spell check", "spellcheck",
          "touch keyboard", "text suggestions", "cursor blinking", "num lock"],
        win("ms-settings:typing", "Bluetooth & devices › Typing"),
        eos("settings://input/keyboard/behavior", "Keyboard › Behavior")),
    page("keyboard-layout", "languages", "Change keyboard layout",
        &["keyboard layout", "input language", "input method", "input source", "add keyboard", "switch keyboard", "qwerty",
          "azerty", "qwertz", "dvorak", "ime", "ibus"],
        win("ms-settings:regionlanguage", "Time & language › Language & region"),
        eos("settings://input/keyboard/layout", "Keyboard › Layout")),
    page("shortcuts", "command", "Keyboard shortcuts",
        &["keyboard shortcuts", "shortcuts", "hotkeys", "key bindings", "keybindings", "custom shortcut"],
        None,
        eos("settings://input/keyboard/shortcuts", "Keyboard › Shortcuts")),
    page("pen", "pen-tool", "Pen & tablet settings",
        &["pen", "stylus", "wacom", "drawing tablet", "graphics tablet", "windows ink", "pen buttons"],
        win("ms-settings:pen", "Bluetooth & devices › Pen & Windows Ink"),
        eos("settings://input/pointing/stylus", "Pen & Drawing")),
    page("autoplay", "circle-play", "AutoPlay settings",
        &["autoplay", "auto play", "usb drive", "memory card", "removable drive", "when i insert"],
        win("ms-settings:autoplay", "Bluetooth & devices › AutoPlay"),
        None),
    page("usb", "usb", "USB settings",
        &["usb", "usb notifications", "usb battery saver", "usb devices"],
        win("ms-settings:usb", "Bluetooth & devices › USB"),
        None),
    page("camera", "camera", "Camera settings",
        &["camera", "webcam", "web cam", "cameras", "camera brightness"],
        win("ms-settings:camera", "Bluetooth & devices › Cameras"),
        None),
    page("phone", "smartphone", "Link your phone",
        &["phone", "phone link", "link phone", "mobile", "mobile devices", "android", "iphone"],
        win("ms-settings:mobile-devices", "Bluetooth & devices › Mobile devices"),
        None),

    // Network
    page("wifi", "wifi", "Connect to Wi-Fi",
        &["wifi", "wi-fi", "wireless", "wireless network", "wlan", "internet", "ssid", "wifi password", "join network",
          "hidden network"],
        win("ms-settings:network-wifi", "Network & internet › Wi-Fi"),
        eos("settings://network", "Network")),
    page("wifi-switch", "wifi", "Toggle Wi-Fi",
        &["wifi on", "wifi off", "wi-fi on", "wi-fi off", "wireless on", "wireless off", "toggle wifi", "turn on wifi", "turn off wifi",
          "enable wifi", "disable wifi"],
        None,
        switch("Toggle Wi-Fi", "toggle:wifi", "Network", &[])),
    page("network", "network", "Network settings",
        &["network", "internet", "connection", "connections", "ip address", "dns", "network adapter", "no internet",
          "offline", "metered connection"],
        win("ms-settings:network-status", "Network & internet"),
        eos("settings://network", "Network")),
    page("ethernet", "ethernet-port", "Ethernet settings",
        &["ethernet", "wired", "wired connection", "lan", "network cable", "cable"],
        win("ms-settings:network-ethernet", "Network & internet › Ethernet"),
        eos("settings://network", "Network")),
    page("vpn", "shield", "Add a VPN connection",
        &["vpn", "virtual private network", "openvpn", "wireguard", "vpn connection"],
        win("ms-settings:network-vpn", "Network & internet › VPN"),
        eos("settings://network", "Network")),
    page("airplane-mode", "plane", "Open airplane mode settings",
        &["airplane mode", "turn on airplane mode", "aeroplane mode", "flight mode", "plane mode", "offline mode", "wireless off", "radios"],
        win("ms-settings:network-airplanemode", "Network & internet › Airplane mode"),
        switch("Toggle airplane mode", "toggle:airplane-mode", "Network",
            &[])),
    page("hotspot", "radio-tower", "Open mobile hotspot settings",
        &["hotspot", "mobile hotspot", "turn on mobile hotspot", "share internet", "share connection", "tethering", "tether"],
        win("ms-settings:network-mobilehotspot", "Network & internet › Mobile hotspot"),
        None),
    page("proxy", "route", "Proxy settings",
        &["proxy", "proxy server", "pac", "pac file", "socks"],
        win("ms-settings:network-proxy", "Network & internet › Proxy"),
        eos("settings://network", "Network")),
    page("data-usage", "chart-column", "Check data usage",
        &["data usage", "data limit", "data used", "bandwidth", "mobile data"],
        win("ms-settings:datausage", "Network & internet › Data usage"),
        None),

    // Apps
    page("uninstall", "trash-2", "Uninstall apps",
        &["uninstall", "remove app", "remove program", "installed apps", "apps and features", "apps & features",
          "add or remove programs", "programs and features", "programs", "app size"],
        win("ms-settings:appsfeatures", "Apps › Installed apps"),
        None),
    page("default-apps", "app-window", "Change default apps",
        &["default apps", "default app", "default browser", "default programs", "default email", "default music player",
          "default video player", "open with", "file associations", "file types"],
        win("ms-settings:defaultapps", "Apps › Default apps"),
        eos("settings://applications/defaults", "Applications › Defaults")),
    page("startup-apps", "rocket", "Change startup apps",
        &["startup apps", "startup", "start at login", "autostart", "launch at login", "run at startup", "login items",
          "startup programs", "boot"],
        win("ms-settings:startupapps", "Apps › Startup"),
        eos("settings://applications/startup", "Applications › Startup")),
    page("app-permissions", "lock-open", "App permissions",
        &["app permissions", "permissions", "flatpak permissions", "sandbox", "allow access", "app access"],
        win("ms-settings:privacy", "Privacy & security"),
        eos("settings://applications/permissions", "Applications › Permissions")),

    // Accounts
    page("account", "circle-user", "Your account",
        &["account", "accounts", "user account", "your info", "profile", "profile picture", "account picture", "avatar",
          "user name", "username", "full name", "microsoft account"],
        win("ms-settings:yourinfo", "Accounts › Your info"),
        eos("settings://accounts", "User Accounts")),
    page("password", "rectangle-ellipsis", "Change your password",
        &["password", "change password", "pin", "windows hello", "fingerprint", "face recognition", "sign in", "sign-in",
          "login", "log in", "automatic login", "auto login"],
        win("ms-settings:signinoptions", "Accounts › Sign-in options"),
        eos("settings://accounts", "User Accounts")),
    page("other-users", "users", "Add another user",
        &["add user", "add account", "other users", "new user", "create account", "family", "guest", "guest account",
          "user accounts", "administrator", "admin", "standard user"],
        win("ms-settings:otherusers", "Accounts › Other users"),
        eos("settings://accounts", "User Accounts")),
    page("online-accounts", "at-sign", "Add an online account",
        &["online accounts", "email accounts", "email", "mail accounts", "google account", "microsoft account", "nextcloud",
          "caldav", "calendar accounts", "sync accounts"],
        win("ms-settings:emailandaccounts", "Accounts › Email & accounts"),
        eos("settings://accounts/online", "Online Accounts")),
    page("backup", "cloud-upload", "Back up this PC",
        &["backup", "back up", "windows backup", "sync settings", "onedrive backup", "remember my apps", "restore apps"],
        win("ms-settings:backup", "Accounts › Windows backup"),
        None),
    page("work-school", "briefcase", "Access work or school",
        &["work account", "school account", "work or school", "azure ad", "entra", "join domain", "domain", "mdm", "enroll"],
        win("ms-settings:workplace", "Accounts › Access work or school"),
        None),

    // Time and language
    page("date-time", "clock", "Change date and time",
        &["date", "time", "date and time", "clock", "time zone", "timezone", "set time", "sync time", "24 hour", "24-hour",
          "daylight saving", "automatic time"],
        win("ms-settings:dateandtime", "Time & language › Date & time"),
        eos("settings://time", "Date & Time")),
    page("language", "globe", "Change display language",
        &["language", "languages", "display language", "system language", "locale", "add language", "translate"],
        win("ms-settings:regionlanguage", "Time & language › Language & region"),
        eos("settings://language", "Language & Region")),
    page("region", "earth", "Change region and formats",
        &["region", "country", "formats", "regional format", "date format", "time format", "number format", "currency",
          "units", "measurement", "metric", "first day of week"],
        win("ms-settings:regionformatting", "Time & language › Language & region"),
        eos("settings://language", "Language & Region")),
    page("speech", "speech", "Speech settings",
        &["speech", "speech recognition", "voice", "voice typing", "voice access", "text to speech", "tts", "dictation"],
        win("ms-settings:speech", "Time & language › Speech"),
        None),

    // Gaming
    page("game-mode", "gamepad-2", "Open Game Mode settings",
        &["game mode", "turn on game mode", "gaming", "games", "gaming performance"],
        win("ms-settings:gaming-gamemode", "Gaming › Game Mode"),
        None),
    page("game-bar", "joystick", "Game Bar settings",
        &["game bar", "xbox game bar", "record screen", "screen recording", "record gameplay", "captures"],
        win("ms-settings:gaming-gamebar", "Gaming › Game Bar"),
        None),

    // Accessibility
    page("accessibility", "accessibility", "Accessibility settings",
        &["accessibility", "universal access", "ease of access", "a11y", "assistive"],
        win("ms-settings:easeofaccess", "Accessibility"),
        eos("settings://universal-access", "Universal Access")),
    page("magnifier", "zoom-in", "Open Magnifier settings",
        &["magnifier", "turn on magnifier", "magnify", "zoom", "zoom in", "screen zoom", "enlarge screen"],
        win("ms-settings:easeofaccess-magnifier", "Accessibility › Magnifier"),
        eos("settings://universal-access", "Universal Access")),
    page("high-contrast", "contrast", "Open high contrast settings",
        &["high contrast", "turn on high contrast", "contrast", "contrast themes", "increase contrast"],
        win("ms-settings:easeofaccess-highcontrast", "Accessibility › Contrast themes"),
        eos("settings://universal-access", "Universal Access")),
    page("screen-reader", "audio-lines", "Open screen reader settings",
        &["screen reader", "turn on screen reader", "narrator", "orca", "read aloud", "read screen", "voiceover", "blind"],
        win("ms-settings:easeofaccess-narrator", "Accessibility › Narrator"),
        eos("settings://universal-access", "Universal Access")),
    page("captions", "captions", "Caption settings",
        &["captions", "closed captions", "subtitles", "live captions", "caption style"],
        win("ms-settings:easeofaccess-closedcaptioning", "Accessibility › Captions"),
        None),
    page("pointer-size", "mouse-pointer-2", "Change mouse pointer size",
        &["mouse pointer", "pointer size", "cursor size", "bigger cursor", "large cursor", "cursor color", "pointer color",
          "mouse cursor", "reveal pointer", "find pointer"],
        win("ms-settings:easeofaccess-mousepointer", "Accessibility › Mouse pointer and touch"),
        eos("settings://input/pointer/pointing", "Mouse & Touchpad › Pointing")),
    page("sticky-keys", "arrow-big-up", "Open Sticky Keys settings",
        &["sticky keys", "turn on sticky keys", "stickykeys", "filter keys", "toggle keys", "slow keys", "bounce keys", "on-screen keyboard",
          "onscreen keyboard"],
        win("ms-settings:easeofaccess-keyboard", "Accessibility › Keyboard"),
        switch("Toggle Sticky Keys", "toggle:sticky-keys", "Universal Access",
            &[])),
    page("reduce-motion", "sparkles", "Reduce animations",
        &["animations", "animation", "reduce motion", "animation effects", "visual effects", "transparency",
          "transparency effects", "scrollbars", "always show scrollbars"],
        win("ms-settings:easeofaccess-visualeffects", "Accessibility › Visual effects"),
        switch("Toggle animations", "toggle:animations", "Desktop › Appearance",
            &["reduce animations", "reduce motion", "turn off animations"])),

    // Privacy and security
    page("privacy", "eye-off", "Privacy settings",
        &["privacy", "tracking", "history", "activity history", "recent files", "diagnostics", "telemetry", "advertising id"],
        win("ms-settings:privacy", "Privacy & security"),
        eos("settings://privacy", "Security & Privacy › Privacy")),
    page("location", "map-pin", "Location settings",
        &["location", "location services", "gps", "geolocation", "my location"],
        win("ms-settings:privacy-location", "Privacy & security › Location"),
        eos("settings://privacy/location", "Security & Privacy › Location Services")),
    page("camera-access", "webcam", "Camera permissions",
        &["camera access", "camera permission", "camera privacy", "webcam access", "apps using camera"],
        win("ms-settings:privacy-webcam", "Privacy & security › Camera"),
        None),
    page("microphone-access", "mic-vocal", "Microphone permissions",
        &["microphone access", "microphone permission", "microphone privacy", "mic access", "apps using microphone"],
        win("ms-settings:privacy-microphone", "Privacy & security › Microphone"),
        None),
    page("virus-protection", "shield-check", "Virus & threat protection",
        &["windows security", "defender", "windows defender", "antivirus", "anti virus", "virus scan", "scan for viruses",
          "malware", "security"],
        win("ms-settings:windowsdefender", "Privacy & security › Windows Security"),
        None),
    page("firewall", "brick-wall", "Firewall settings",
        &["firewall", "ufw", "block connections", "ports", "incoming connections"],
        win("ms-settings:windowsdefender", "Privacy & security › Windows Security"),
        eos("settings://security/firewall", "Security & Privacy › Firewall")),
    page("find-my-device", "locate-fixed", "Find my device",
        &["find my device", "find my pc", "lost laptop", "locate device", "track device"],
        win("ms-settings:findmydevice", "Privacy & security › Find my device"),
        None),
];

/// Words that only say "a settings page", so "display settings" finds
/// *Display settings* and "settings" alone is left to the "Open Settings"
/// quick action instead of matching every page.
const FILLER: &[&str] =
    &["settings", "setting", "preferences", "preference", "prefs", "options", "option", "config", "configuration", "configure"];

/// Shorter queries match too much ("c" is the start of every "Change …").
const MIN_QUERY_CHARS: usize = 2;
/// Misspellings are forgiven from this length: before it a query is more
/// likely an unfinished word ("sett" is not "set" with a typo).
const TYPO_MIN_CHARS: usize = 5;
/// Enough to cover "turn on …" without crowding out files.
const MAX_RESULTS: usize = 5;
/// How people ask for a switch: "turn off bluetooth", "enable night light".
const SWITCH_VERBS: &[&str] = &["turn on", "turn off", "switch on", "switch off", "enable", "disable"];
/// Enter on a search for "wifi" must not cut the connection: this switch is
/// found only by words that ask to change it, never by its name alone.
const WIFI_SWITCH: &str = "toggle:wifi";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    Windows,
    Elementary,
}

impl Desktop {
    pub fn current() -> Self {
        if cfg!(windows) {
            Desktop::Windows
        } else {
            Desktop::Elementary
        }
    }

    fn link(self, page: &'static SettingPage) -> Option<&'static Link> {
        match self {
            Desktop::Windows => page.windows.as_ref(),
            Desktop::Elementary => page.elementary.as_ref(),
        }
    }

    /// The settings app's name, the start of every page's path.
    fn app_name(self) -> &'static str {
        match self {
            Desktop::Windows => "Settings",
            Desktop::Elementary => "System Settings",
        }
    }
}

/// Folded text without filler words.
fn without_filler(text: &str) -> String {
    fold(text).split_whitespace().filter(|w| !FILLER.contains(w)).collect::<Vec<_>>().join(" ")
}

/// Whether the last word is a filler word still being typed ("display sett").
fn ends_with_unfinished_filler(text: &str) -> bool {
    text.rsplit(' ').next().is_some_and(|last| !last.is_empty() && FILLER.iter().any(|f| f.starts_with(last)))
}

/// Letters and digits only, so "darkmode" meets "dark mode" and "wifi" meets "Wi-Fi".
fn compact(folded: &str) -> String {
    folded.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// A page of this platform, with its text prepared for matching.
struct Entry {
    page: &'static SettingPage,
    link: &'static Link,
    title: String,
    keywords: Vec<String>,
    /// Title and keywords as [`compact`] text.
    compact: Vec<String>,
}

struct Match {
    score: u32,
    /// Matched as a word, prefix or substring rather than only with a typo.
    close: bool,
}

impl Entry {
    fn new(page: &'static SettingPage, link: &'static Link) -> Self {
        let title = without_filler(link.title.unwrap_or(page.title));
        let asked_for = link.title.and_then(|t| t.strip_prefix("Toggle ")).into_iter().flat_map(|name| {
            SWITCH_VERBS.iter().map(move |verb| without_filler(&format!("{verb} {name}")))
        });
        let keywords: Vec<String> = page
            .keywords
            .iter()
            .chain(link.keywords)
            .map(|k| without_filler(k))
            .chain(asked_for)
            .filter(|k| !k.is_empty())
            .collect();
        let compact = std::iter::once(&title).chain(&keywords).map(|t| compact(t)).filter(|t| !t.is_empty()).collect();
        Self { page, link, title, keywords, compact }
    }

    fn matches(&self, query: &Query, joined: Option<&Query>, typos: bool) -> Option<Match> {
        let by_title = !self.title.is_empty() && self.link.uri != WIFI_SWITCH;
        let title = by_title.then(|| query.score_contiguous(&self.title, typos)).flatten();
        let keyword = self.keywords.iter().filter_map(|k| query.score_contiguous(k, typos)).max();
        // a run-together query only counts when it is the whole word or its start
        let compact = joined.and_then(|joined| {
            self.compact.iter().filter_map(|k| joined.score_contiguous(k, false)).filter(|s| *s >= PREFIX_MATCH).max()
        });
        let best = title.max(keyword).max(compact)?;
        let score = title.max(keyword.map(|s| s * 9 / 10)).max(compact.map(|s| s * 9 / 10))?;
        Some(Match { score, close: best >= CONTIGUOUS_MATCH })
    }

    fn to_result(&self, desktop: Desktop, score: f32) -> SearchResult {
        SearchResult {
            id: format!("setting:{}", self.page.id),
            kind: ResultKind::Setting,
            title: self.link.title.unwrap_or(self.page.title).into(),
            subtitle: format!("{} › {}", desktop.app_name(), self.link.path),
            icon: ResultIcon::Glyph { name: self.page.icon },
            score,
            reveal: false,
            note: None,
            actions: Vec::new(),
        }
    }
}

pub struct SystemSettingsProvider {
    desktop: Desktop,
    entries: Vec<Entry>,
    /// The settings app's key in the app index while it can open its pages
    /// (elementary's System Settings is installed); `None` otherwise.
    app: RwLock<Option<String>>,
}

impl Default for SystemSettingsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemSettingsProvider {
    pub fn new() -> Self {
        Self::for_desktop(Desktop::current(), platform::system_settings::app_key())
    }

    pub fn for_desktop(desktop: Desktop, app: Option<String>) -> Self {
        let entries = PAGES.iter().filter_map(|p| desktop.link(p).map(|link| Entry::new(p, link))).collect();
        Self { desktop, entries, app: RwLock::new(app) }
    }

    pub fn refresh_availability(&self) {
        *self.app.write().unwrap_or_else(|e| e.into_inner()) = platform::system_settings::app_key();
    }

    fn is_available(&self) -> bool {
        self.app.read().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    fn entry(&self, local_id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.page.id == local_id)
    }
}

impl SearchProvider for SystemSettingsProvider {
    fn id(&self) -> &'static str {
        "setting"
    }

    fn enabled(&self, settings: &SpotlightSettings) -> bool {
        settings.search_settings
    }

    fn search(&self, ctx: &SearchContext<'_>, out: &mut Vec<SearchResult>) {
        if !self.is_available() {
            return;
        }
        let text = without_filler(ctx.raw);
        let length = text.chars().count();
        if length < MIN_QUERY_CHARS {
            return;
        }
        let query = Query::new(&text);
        let typos = length >= TYPO_MIN_CHARS;
        let unfinished = ends_with_unfinished_filler(&text);
        // "sett" is "settings" being typed, not "set time" run together
        let joined = (!unfinished).then(|| Query::new(&compact(&text)));
        // and "display sett" already finds Display settings, a little less surely than "display"
        let shorter = text.rsplit_once(' ').filter(|_| unfinished).map(|(rest, _)| Query::new(rest));
        let mut found: Vec<(Match, &Entry)> = self
            .entries
            .iter()
            .filter_map(|e| {
                let full = e.matches(&query, joined.as_ref(), typos);
                let shorter = shorter
                    .as_ref()
                    .and_then(|q| e.matches(q, None, typos))
                    .map(|m| Match { score: m.score * 9 / 10, close: m.close });
                let best = match (full, shorter) {
                    (Some(a), Some(b)) => Some(if b.score > a.score { b } else { a }),
                    (a, b) => a.or(b),
                }?;
                Some((best, e))
            })
            .collect();
        // a misspelling is only a guess: offer one only when nothing matched properly
        if found.iter().any(|(m, _)| m.close) {
            found.retain(|(m, _)| m.close);
        }
        found.sort_by_key(|(m, _)| std::cmp::Reverse(m.score));
        found.truncate(MAX_RESULTS.min(ctx.limit));
        if found.is_empty() {
            return;
        }
        out.extend(found.into_iter().map(|(m, e)| e.to_result(self.desktop, m.score as f32)));
    }

    fn execute(&self, local_id: &str, _mode: ExecuteMode) -> AppResult<ExecutedItem> {
        let entry = self
            .entry(local_id)
            .ok_or_else(|| AppError::NotFound(format!("Unknown settings page \"{local_id}\".")))?;
        if !self.is_available() {
            return Err(AppError::NotFound(format!("{} is not installed.", self.desktop.app_name())));
        }
        platform::system_settings::open(entry.link.uri).map_err(AppError::Launch)?;
        let result = entry.to_result(self.desktop, 0.0);
        Ok(ExecutedItem { kind: ResultKind::Setting, title: result.title, subtitle: result.subtitle })
    }

    fn resolve(&self, local_id: &str) -> Option<SearchResult> {
        self.entry(local_id).filter(|_| self.is_available()).map(|e| e.to_result(self.desktop, 0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const BOTH: [Desktop; 2] = [Desktop::Windows, Desktop::Elementary];
    const SETTINGS_APP: &str = "io.elementary.settings.desktop";

    /// With the settings app installed.
    fn provider(desktop: Desktop) -> SystemSettingsProvider {
        SystemSettingsProvider::for_desktop(desktop, Some(SETTINGS_APP.into()))
    }

    fn search_with(provider: &SystemSettingsProvider, raw: &str) -> Vec<SearchResult> {
        let query = Query::new(raw);
        let settings = SpotlightSettings::default();
        let ctx = SearchContext { raw: raw.trim(), query: &query, settings: &settings, limit: 24 };
        let mut out = Vec::new();
        provider.search(&ctx, &mut out);
        out
    }

    fn search(desktop: Desktop, raw: &str) -> Vec<SearchResult> {
        search_with(&provider(desktop), raw)
    }

    fn ids(desktop: Desktop, raw: &str) -> Vec<String> {
        search(desktop, raw).into_iter().map(|r| r.id).collect()
    }

    fn first(desktop: Desktop, raw: &str) -> Option<String> {
        ids(desktop, raw).into_iter().next()
    }

    #[test]
    fn catalog_is_well_formed() {
        let ids: HashSet<_> = PAGES.iter().map(|p| p.id).collect();
        assert_eq!(ids.len(), PAGES.len(), "ids are unique");
        for p in PAGES {
            assert!(p.windows.is_some() || p.elementary.is_some(), "{} opens nowhere", p.id);
            assert!(!without_filler(p.title).is_empty(), "{} has a searchable title", p.id);
            if let Some(l) = &p.windows {
                assert!(l.uri.starts_with("ms-settings:") && !l.path.is_empty(), "{}", p.id);
            }
            if let Some(l) = &p.elementary {
                assert!(l.uri.starts_with("settings://") || l.uri.starts_with("toggle:"), "{}", p.id);
                assert!(!l.path.is_empty(), "{}", p.id);
                assert_eq!(l.uri.starts_with("toggle:"), l.title.is_some_and(|t| t.starts_with("Toggle ")), "{}", p.id);
            }
            for k in p.keywords {
                assert_eq!(*k, k.to_lowercase(), "{}: keywords are lowercase", p.id);
            }
            // a Lucide name, which tests/spotlight-icons.test.js checks the frontend draws
            assert!(!p.icon.is_empty() && p.icon.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'), "{}", p.id);
        }
    }

    #[test]
    fn dark_mode_is_found_however_it_is_typed() {
        for desktop in BOTH {
            for q in ["dark mode", "darkmode", "Dark Mode", "DARKMODE", "turn on dark mode", "dark", "dark theme", "darkmode settings", "dark-mode"] {
                assert_eq!(first(desktop, q).as_deref(), Some("setting:dark-mode"), "{q:?} on {desktop:?}");
            }
        }
        let windows = &search(Desktop::Windows, "darkmode")[0];
        assert_eq!(windows.title, "Open dark mode settings");
        assert_eq!(windows.subtitle, "Settings › Personalization › Colors");
        assert!(windows.score >= 900.0, "strong enough to be the Top Hit: {}", windows.score);
        let elementary = &search(Desktop::Elementary, "darkmode")[0];
        assert_eq!(elementary.subtitle, "System Settings › Desktop › Appearance");
    }

    #[test]
    fn common_searches_find_their_page() {
        let cases = [
            ("wifi", "wifi"), ("wi-fi", "wifi"), ("wifi settings", "wifi"), ("bluetooth", "bluetooth"),
            ("bluetooth settings", "bluetooth"), ("display settings", "display"), ("resolution", "display"),
            ("night light", "night-light"), ("nightlight", "night-light"), ("wallpaper", "wallpaper"),
            ("background", "wallpaper"), ("sound", "sound"), ("speakers", "sound"), ("brightness", "brightness"),
            ("printer", "printers"), ("mouse", "mouse"), ("time zone", "date-time"), ("timezone", "date-time"),
            ("default browser", "default-apps"), ("startup apps", "startup-apps"), ("password", "password"),
            ("airplane mode", "airplane-mode"), ("vpn", "vpn"), ("notifications", "notifications"),
            ("do not disturb", "do-not-disturb"), ("dnd", "do-not-disturb"), ("updates", "updates"),
            ("keyboard layout", "keyboard-layout"), ("text size", "text-size"), ("battery", "power"),
            ("accent color", "accent-color"), ("blutooth", "bluetooth"),
        ];
        for desktop in BOTH {
            for (q, id) in cases {
                assert_eq!(first(desktop, q), Some(format!("setting:{id}")), "{q:?} on {desktop:?}");
            }
        }
        assert_eq!(first(Desktop::Windows, "light mode").as_deref(), Some("setting:light-mode"));
        assert_eq!(first(Desktop::Elementary, "light mode").as_deref(), Some("setting:dark-mode"));
        assert_eq!(first(Desktop::Windows, "uninstall").as_deref(), Some("setting:uninstall"));
        assert_eq!(first(Desktop::Windows, "taskbar").as_deref(), Some("setting:taskbar"));
        assert_eq!(first(Desktop::Elementary, "dock").as_deref(), Some("setting:dock"));
        assert_eq!(first(Desktop::Elementary, "hot corners").as_deref(), Some("setting:multitasking"));
    }

    #[test]
    fn on_off_commands_are_switches_on_elementary_only() {
        let elementary = provider(Desktop::Elementary);
        let windows = provider(Desktop::Windows);
        for (id, title) in [("dark-mode", "Toggle dark mode"), ("bluetooth", "Toggle Bluetooth"), ("night-light", "Toggle Night Light")] {
            assert_eq!(elementary.resolve(id).unwrap().title, title);
            assert!(windows.resolve(id).unwrap().title.starts_with("Open "), "{id}");
            assert!(windows.entry(id).unwrap().link.uri.starts_with("ms-settings:"), "{id}");
        }
        assert!(elementary.resolve("light-mode").is_none(), "one switch for dark and light");
        assert!(windows.resolve("light-mode").is_some());
        assert!(windows.resolve("wifi-switch").is_none());
        for q in ["turn off bluetooth", "turn on bluetooth", "disable bluetooth", "toggle bluetooth"] {
            assert_eq!(first(Desktop::Elementary, q).as_deref(), Some("setting:bluetooth"), "{q:?}");
        }
    }

    #[test]
    fn pages_that_only_open_settings_say_so() {
        // next to "Toggle Dark Mode", a page called "Turn on dark mode" reads as a second switch
        for desktop in BOTH {
            for page in PAGES {
                let Some(link) = desktop.link(page).filter(|l| !l.uri.starts_with("toggle:")) else { continue };
                let title = fold(link.title.unwrap_or(page.title));
                for verb in SWITCH_VERBS.iter().chain(&["toggle"]) {
                    assert!(!title.starts_with(verb), "{} on {desktop:?} opens a page but is titled {title:?}", page.id);
                }
            }
        }
        // and the old wording still finds them
        for (q, id) in [("turn on dark mode", "dark-mode"), ("turn on bluetooth", "bluetooth"), ("turn on night light", "night-light")] {
            assert_eq!(first(Desktop::Windows, q), Some(format!("setting:{id}")), "{q:?}");
        }
    }

    #[test]
    fn typing_wifi_never_puts_the_switch_first() {
        // Enter on the top result must not cut the connection
        for q in ["wifi", "wi-fi", "wifi settings", "wireless"] {
            assert_eq!(first(Desktop::Elementary, q).as_deref(), Some("setting:wifi"), "{q:?}");
        }
        for q in ["toggle wifi", "turn off wifi", "wifi off"] {
            assert_eq!(first(Desktop::Elementary, q).as_deref(), Some("setting:wifi-switch"), "{q:?}");
        }
    }

    #[test]
    fn pages_a_platform_lacks_are_not_offered() {
        assert!(!ids(Desktop::Elementary, "taskbar").contains(&"setting:taskbar".to_string()));
        assert!(!ids(Desktop::Windows, "dock").contains(&"setting:dock".to_string()));
        let provider = provider(Desktop::Elementary);
        assert!(provider.resolve("taskbar").is_none());
        assert!(provider.resolve("dark-mode").is_some());
    }

    #[test]
    fn filler_and_short_queries_find_nothing() {
        for desktop in BOTH {
            for q in ["settings", "setting", "options", "s", "c", "settings settings", "sett"] {
                assert!(ids(desktop, q).is_empty(), "{q:?} on {desktop:?}: {:?}", ids(desktop, q));
            }
        }
    }

    #[test]
    fn typing_settings_after_a_name_keeps_its_page() {
        for desktop in BOTH {
            for q in ["display s", "display se", "display sett", "display settin", "display settings"] {
                assert_eq!(first(desktop, q).as_deref(), Some("setting:display"), "{q:?} on {desktop:?}");
            }
            assert_eq!(first(desktop, "bluetooth sett").as_deref(), Some("setting:bluetooth"));
            // a word that only starts like "settings" still narrows the search
            let first = first(desktop, "turn on s").unwrap();
            assert!(["setting:sticky-keys", "setting:screen-reader"].contains(&first.as_str()), "{first}");
        }
    }

    #[test]
    fn results_are_capped_and_ranked() {
        for desktop in BOTH {
            let found = search(desktop, "turn on");
            assert_eq!(found.len(), MAX_RESULTS, "{desktop:?}");
            assert!(found.windows(2).all(|w| w[0].score >= w[1].score));
        }
    }

    #[test]
    fn typo_guesses_give_way_to_real_matches() {
        for desktop in BOTH {
            // "mouse" is one letter from "housekeeping"; real matches for it hide that guess
            let mouse = ids(desktop, "mouse");
            assert_eq!(mouse.first().map(String::as_str), Some("setting:mouse"));
            assert!(!mouse.contains(&"setting:auto-cleanup".to_string()), "{mouse:?}");
            assert!(ids(desktop, "zzzzz").is_empty());
        }
    }

    #[test]
    fn each_page_shows_its_own_glyph() {
        for desktop in BOTH {
            let dark = &search(desktop, "dark mode")[0];
            assert_eq!(dark.icon, ResultIcon::Glyph { name: "moon" }, "{desktop:?}");
            let wifi = &search(desktop, "wifi")[0];
            assert_eq!(wifi.icon, ResultIcon::Glyph { name: "wifi" }, "{desktop:?}");
            assert_eq!(provider(desktop).resolve("dark-mode").unwrap().icon, ResultIcon::Glyph { name: "moon" }, "also in Recent");
        }
        assert_eq!(provider(Desktop::Windows).resolve("bluetooth").unwrap().icon, ResultIcon::Glyph { name: "bluetooth" });
    }

    #[test]
    fn nothing_is_offered_without_the_settings_app() {
        let provider = SystemSettingsProvider::for_desktop(Desktop::Elementary, None);
        assert!(search_with(&provider, "dark mode").is_empty());
        assert!(provider.resolve("dark-mode").is_none());
        assert_eq!(provider.execute("dark-mode", ExecuteMode::Open).unwrap_err().code(), "not-found");
    }

    #[test]
    fn unknown_pages_are_rejected() {
        let provider = provider(Desktop::current());
        assert_eq!(provider.execute("format-disk", ExecuteMode::Open).unwrap_err().code(), "not-found");
    }

    #[test]
    fn only_settings_links_open() {
        assert!(platform::system_settings::open("https://example.com").is_err());
        assert!(platform::system_settings::open("file:///etc/passwd").is_err());
    }
}
