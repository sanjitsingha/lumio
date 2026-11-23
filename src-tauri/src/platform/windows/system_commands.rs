//! The System commands of Spotlight and the Shortcut page on Windows: power,
//! sound, windows, folders and the tools that come with Windows.
//!
//! To add one, add an entry to [`COMMANDS`]; its icon must be in `SYMBOLS`
//! (`ResultIcon.jsx`). Anything that can lose work uses [`Run::Ask`]. A
//! command whose program or link this PC lacks is not offered.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, E_UNEXPECTED, HANDLE, LPARAM, LUID, WPARAM};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, SE_SHUTDOWN_NAME,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard};
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::System::Registry::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use windows::Win32::System::Shutdown::LockWorkStation;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken, CREATE_NO_WINDOW};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::{SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND};
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, PostMessageW, SendMessageTimeoutW, HWND_BROADCAST, IDYES, MB_DEFBUTTON2, MB_ICONWARNING,
    MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO, SC_MONITORPOWER, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE, WM_SYSCOMMAND,
};

use super::launch::{shell_open, shell_run};
use super::registry;
use crate::search::actions::QuickAction;

/// How long Spotlight gets to close before keys are pressed or the PC is put to sleep.
const SPOTLIGHT_GONE: Duration = Duration::from_millis(180);
/// A hotkey's own keys would combine with the ones sent; wait this long for their release.
const KEYS_UP_TIMEOUT: Duration = Duration::from_secs(1);

enum Run {
    /// A link, a `shell:` folder or a program of Windows, through the shell.
    Open(&'static str),
    /// A program of Windows with fixed arguments.
    Program(&'static str, &'static str),
    /// A key combination Windows itself answers to.
    Keys(&'static [VIRTUAL_KEY]),
    Native(fn() -> Result<(), String>),
    /// Can lose work, so it asks this question first.
    Ask(&'static str, fn() -> Result<(), String>),
}
use Run::{Ask, Keys, Native, Open, Program};

struct SystemCommand {
    action: QuickAction,
    run: Run,
}

const fn c(
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
    keywords: &'static [&'static str],
    icon: &'static str,
    run: Run,
) -> SystemCommand {
    SystemCommand { action: QuickAction { id, title, subtitle, keywords, icon }, run }
}

#[rustfmt::skip]
static COMMANDS: &[SystemCommand] = &[
    // Power and session
    c("lock-screen", "Lock Screen", "Lock this PC", &["lock", "lock pc", "lock computer"], "lock", Native(lock)),
    c("sleep", "Sleep", "Put this PC to sleep", &["sleep", "suspend", "standby"], "moon", Native(sleep)),
    c("hibernate", "Hibernate", "Save your session and power off", &["hibernate"], "moon-star", Native(hibernate)),
    c("shut-down", "Shut Down", "Turn off this PC", &["shutdown", "shut down", "power off", "turn off"], "power", Ask("Shut down this PC?", shut_down)),
    c("restart", "Restart", "Restart this PC", &["restart", "reboot"], "rotate-cw", Ask("Restart this PC?", restart)),
    c("log-out", "Log Out", "Sign out of Windows", &["logout", "log off", "logoff", "sign out"], "log-out", Ask("Log out of Windows?", log_out)),
    c("turn-off-display", "Turn Off Display", "Put the screen to sleep", &["screen off", "monitor off", "display off"], "monitor-off", Native(turn_off_display)),

    // Recycle Bin
    c("open-recycle-bin", "Open Recycle Bin", "Deleted files", &["trash", "recycle bin", "bin", "deleted files"], "trash", Open("shell:RecycleBinFolder")),
    c("empty-recycle-bin", "Empty Recycle Bin", "Permanently delete what is in it", &["empty trash", "empty bin", "clear recycle bin"], "trash-2", Ask("Permanently delete everything in the Recycle Bin?", empty_recycle_bin)),

    // Sound and media
    c("volume-up", "Volume Up", "Raise the sound", &["louder", "increase volume", "sound up"], "volume-2", Keys(&[VK_VOLUME_UP])),
    c("volume-down", "Volume Down", "Lower the sound", &["quieter", "decrease volume", "sound down"], "volume-1", Keys(&[VK_VOLUME_DOWN])),
    c("toggle-mute", "Toggle Mute", "Mute or unmute the sound", &["mute", "unmute", "silence"], "volume-x", Keys(&[VK_VOLUME_MUTE])),
    c("media-play-pause", "Play or Pause Media", "The song or video playing", &["play", "pause", "music", "resume"], "play", Keys(&[VK_MEDIA_PLAY_PAUSE])),
    c("media-next", "Next Track", "Skip to the next one", &["next song", "skip", "next media"], "skip-forward", Keys(&[VK_MEDIA_NEXT_TRACK])),
    c("media-previous", "Previous Track", "Back to the last one", &["previous song", "last song", "previous media"], "skip-back", Keys(&[VK_MEDIA_PREV_TRACK])),
    c("media-stop", "Stop Media", "Stop what is playing", &["stop music", "stop playing"], "square", Keys(&[VK_MEDIA_STOP])),
    c("sound-output", "Manage Sound Output Devices", "Speakers and headphones", &["speakers", "headphones", "output device", "playback"], "headphones", Program("control.exe", "mmsys.cpl,,0")),
    c("sound-input", "Manage Sound Input Devices", "Microphones", &["microphone", "mic", "input device", "recording devices"], "mic", Program("control.exe", "mmsys.cpl,,1")),
    c("volume-mixer-classic", "Volume Mixer", "The volume of each app", &["mixer", "app volume", "sndvol"], "sliders-horizontal", Open("sndvol.exe")),

    // Connections
    c("manage-wifi", "Manage Wi-Fi Connections", "Available networks", &["wifi", "wi-fi", "wireless", "networks"], "wifi", Open("ms-availablenetworks:")),
    c("manage-bluetooth", "Manage Bluetooth Devices", "Pair and connect", &["bluetooth", "pair", "devices"], "bluetooth", Open("ms-settings:bluetooth")),
    c("network-connections", "Network Connections", "Network adapters", &["adapters", "ethernet", "ncpa"], "network", Open("ncpa.cpl")),
    c("flush-dns", "Flush DNS Cache", "Forget saved website addresses", &["dns", "flushdns", "clear dns"], "refresh-cw", Native(flush_dns)),
    c("quick-settings", "Open Quick Settings", "Wi-Fi, Bluetooth and sound", &["action center", "quick settings"], "sliders-horizontal", Keys(&[VK_LWIN, VK_A])),
    c("notification-center", "Open Notification Center", "Notifications and calendar", &["notifications", "alerts", "calendar flyout"], "bell", Keys(&[VK_LWIN, VK_N])),
    c("cast-display", "Cast to a Display", "A wireless screen", &["cast", "miracast", "wireless display"], "cast", Keys(&[VK_LWIN, VK_K])),
    c("remote-desktop-connection", "Remote Desktop Connection", "Connect to another PC", &["rdp", "mstsc", "remote"], "screen-share", Open("mstsc.exe")),
    c("remote-assistance", "Windows Remote Assistance", "Get or give help", &["msra", "remote help"], "life-buoy", Open("msra.exe")),

    // Windows and desktops
    c("show-desktop", "Show Desktop", "Show or hide all windows", &["desktop", "hide windows", "peek"], "monitor", Keys(&[VK_LWIN, VK_D])),
    c("minimize-all", "Minimize All Windows", "Clear the screen", &["minimize", "hide all"], "minimize-2", Keys(&[VK_LWIN, VK_M])),
    c("task-view", "Task View", "All windows and desktops", &["windows", "alt tab", "overview", "mission control"], "layout-grid", Keys(&[VK_LWIN, VK_TAB])),
    c("new-virtual-desktop", "New Virtual Desktop", "Add a desktop", &["add desktop", "new desktop", "workspace"], "square-plus", Keys(&[VK_LWIN, VK_CONTROL, VK_D])),
    c("close-virtual-desktop", "Close Virtual Desktop", "Close this desktop", &["remove desktop", "close desktop"], "square-x", Keys(&[VK_LWIN, VK_CONTROL, VK_F4])),
    c("next-virtual-desktop", "Next Virtual Desktop", "Switch right", &["next desktop", "switch desktop"], "arrow-right", Keys(&[VK_LWIN, VK_CONTROL, VK_RIGHT])),
    c("previous-virtual-desktop", "Previous Virtual Desktop", "Switch left", &["previous desktop", "switch desktop"], "arrow-left", Keys(&[VK_LWIN, VK_CONTROL, VK_LEFT])),

    // Displays and appearance
    c("project-screen", "Project to a Screen", "Choose how displays are used", &["project", "second screen", "external display"], "projector", Keys(&[VK_LWIN, VK_P])),
    c("display-extend", "Extend Displays", "Use both screens", &["extend", "dual monitor"], "monitor-up", Program("DisplaySwitch.exe", "/extend")),
    c("display-duplicate", "Duplicate Displays", "Show the same on both", &["duplicate", "mirror", "clone display"], "copy", Program("DisplaySwitch.exe", "/clone")),
    c("display-second-only", "Second Screen Only", "Use only the external display", &["external only"], "monitor-check", Program("DisplaySwitch.exe", "/external")),
    c("display-pc-only", "PC Screen Only", "Use only this screen", &["internal only", "laptop screen"], "laptop", Program("DisplaySwitch.exe", "/internal")),
    c("toggle-dark-mode", "Toggle Dark Mode", "Switch Windows between light and dark", &["dark mode", "light mode", "theme", "auto dark mode"], "sun-moon", Native(toggle_dark_mode)),
    c("color-calibration", "Display Color Calibration", "Tune your screen's colors", &["dccw", "calibrate"], "palette", Open("dccw.exe")),
    c("cleartype", "ClearType Text Tuner", "Make text easier to read", &["cttune", "font smoothing"], "type", Open("cttune.exe")),

    // Capture and input
    c("take-screenshot", "Take a Screenshot", "Snip part of the screen", &["screenshot", "snip", "screen capture", "print screen"], "scissors", Keys(&[VK_LWIN, VK_SHIFT, VK_S])),
    c("clipboard-history", "Clipboard History", "What you copied", &["clipboard", "paste history"], "clipboard", Keys(&[VK_LWIN, VK_V])),
    c("clear-clipboard", "Clear Clipboard", "Empty what is copied now", &["empty clipboard"], "clipboard-x", Native(clear_clipboard)),
    c("emoji-picker", "Emoji Picker", "Emoji, symbols and GIFs", &["emoji", "symbols", "kaomoji", "gif"], "smile", Keys(&[VK_LWIN, VK_OEM_PERIOD])),
    c("voice-typing", "Voice Typing", "Dictate into any text field", &["dictation", "speech to text"], "audio-lines", Keys(&[VK_LWIN, VK_H])),
    c("run-dialog", "Run Dialog", "Open anything by its name", &["run", "win r"], "square-chevron-right", Keys(&[VK_LWIN, VK_R])),
    c("on-screen-keyboard", "On-Screen Keyboard", "Type with the mouse", &["osk", "virtual keyboard"], "keyboard", Open("osk.exe")),
    c("magnifier-app", "Magnifier", "Zoom into the screen", &["zoom", "magnify"], "zoom-in", Open("magnify.exe")),
    c("narrator-app", "Narrator", "Read the screen aloud", &["screen reader", "read aloud"], "speech", Open("narrator.exe")),
    c("character-map", "Character Map", "Special characters", &["charmap", "symbols", "special characters"], "languages", Open("charmap.exe")),

    // Folders
    c("folder-home", "Open Home Folder", "Your user folder", &["home", "profile", "user folder"], "house", Open("shell:Profile")),
    c("folder-desktop", "Open Desktop Folder", "Files on your desktop", &["desktop folder"], "monitor", Open("shell:Desktop")),
    c("folder-documents", "Open Documents", "Documents folder", &["documents", "docs"], "file-text", Open("shell:Personal")),
    c("folder-downloads", "Open Downloads", "Downloads folder", &["downloads"], "download", Open("shell:Downloads")),
    c("folder-pictures", "Open Pictures", "Pictures folder", &["pictures", "photos", "images"], "image", Open("shell:My Pictures")),
    c("folder-screenshots", "Open Screenshots", "Saved screenshots", &["screenshots folder"], "camera", Open("shell:Screenshots")),
    c("folder-camera-roll", "Open Camera Roll", "Photos from the camera", &["camera roll"], "images", Open("shell:Camera Roll")),
    c("folder-music", "Open Music", "Music folder", &["music", "songs"], "music", Open("shell:My Music")),
    c("folder-videos", "Open Videos", "Videos folder", &["videos", "movies"], "video", Open("shell:My Video")),
    c("folder-onedrive", "Open OneDrive", "OneDrive folder", &["onedrive", "cloud"], "cloud", Open("shell:OneDrive")),
    c("folder-this-pc", "Open This PC", "Drives and devices", &["this pc", "my computer", "drives"], "hard-drive", Open("shell:MyComputerFolder")),
    c("folder-network", "Open Network", "Computers on your network", &["network folder", "shared computers"], "network", Open("shell:NetworkPlacesFolder")),
    c("folder-recent", "Open Recent Files", "What you opened lately", &["recent", "recent files"], "history", Open("shell:Recent")),
    c("folder-appdata", "Open AppData", "Roaming app data", &["appdata", "roaming"], "folder-cog", Open("shell:AppData")),
    c("folder-local-appdata", "Open Local AppData", "Local app data", &["local appdata"], "folder-cog", Open("shell:Local AppData")),
    c("folder-temp", "Open Temp Folder", "Temporary files", &["temp", "tmp", "temporary files"], "folder-clock", Native(open_temp)),
    c("folder-startup", "Open Startup Folder", "Apps started at sign-in", &["startup folder", "autostart"], "rocket", Open("shell:Startup")),
    c("folder-fonts", "Open Fonts Folder", "Installed fonts", &["fonts folder"], "type", Open("shell:Fonts")),
    c("folder-program-files", "Open Program Files", "Installed programs", &["program files"], "package", Open("shell:ProgramFiles")),
    c("folder-system32", "Open System32", "Windows system files", &["system32", "windows folder"], "folder-cog", Open("shell:System")),

    // Monitoring and diagnostics
    c("task-manager", "Task Manager", "Running apps and performance", &["taskmgr", "processes", "end task"], "activity", Open("taskmgr.exe")),
    c("resource-monitor", "Resource Monitor", "CPU, memory, disk and network", &["resmon"], "gauge", Open("resmon.exe")),
    c("performance-monitor", "Performance Monitor", "Detailed counters", &["perfmon"], "chart-line", Open("perfmon.msc")),
    c("event-viewer", "Event Viewer", "System and app logs", &["eventvwr", "logs", "errors"], "scroll-text", Open("eventvwr.msc")),
    c("system-information", "System Information", "Hardware and software details", &["msinfo", "msinfo32", "specs"], "info", Open("msinfo32.exe")),
    c("directx-diagnostic", "DirectX Diagnostic Tool", "Graphics and sound details", &["dxdiag", "directx"], "cpu", Open("dxdiag.exe")),
    c("memory-diagnostic", "Windows Memory Diagnostic", "Check the RAM for errors", &["mdsched", "ram test"], "memory-stick", Open("mdsched.exe")),
    c("reliability-monitor", "Reliability Monitor", "Crashes and problems over time", &["reliability", "crash history"], "chart-line", Program("perfmon.exe", "/rel")),

    // Administration
    c("computer-management", "Computer Management", "Disks, services and users", &["compmgmt"], "server", Open("compmgmt.msc")),
    c("device-manager", "Device Manager", "Hardware and drivers", &["devmgmt", "drivers", "hardware"], "cpu", Open("devmgmt.msc")),
    c("disk-management", "Disk Management", "Partitions and volumes", &["diskmgmt", "partition", "format drive"], "hard-drive", Open("diskmgmt.msc")),
    c("disk-cleanup", "Disk Cleanup", "Free up disk space", &["cleanmgr", "clean disk"], "brush-cleaning", Open("cleanmgr.exe")),
    c("optimize-drives", "Defragment and Optimize Drives", "Keep drives fast", &["defrag", "dfrgui", "trim"], "hard-drive", Open("dfrgui.exe")),
    c("services", "Services", "Background services", &["services.msc"], "cog", Open("services.msc")),
    c("task-scheduler", "Task Scheduler", "Scheduled tasks", &["taskschd", "cron", "schedule"], "calendar-clock", Open("taskschd.msc")),
    c("registry-editor", "Registry Editor", "Windows registry", &["regedit", "registry"], "database", Open("regedit.exe")),
    c("system-configuration", "System Configuration", "Boot and startup options", &["msconfig", "boot options"], "wrench", Open("msconfig.exe")),
    c("windows-features", "Turn Windows Features On or Off", "Optional components", &["optionalfeatures", "hyper-v", "wsl"], "puzzle", Open("optionalfeatures.exe")),
    c("group-policy", "Group Policy Editor", "Local policies", &["gpedit", "policy"], "file-cog", Open("gpedit.msc")),
    c("security-policy", "Local Security Policy", "Security options", &["secpol"], "shield", Open("secpol.msc")),
    c("users-and-groups", "Local Users and Groups", "Accounts on this PC", &["lusrmgr"], "users", Open("lusrmgr.msc")),
    c("user-accounts", "User Accounts", "Manage who signs in", &["netplwiz", "auto login"], "user-cog", Open("netplwiz.exe")),
    c("certificates", "Manage User Certificates", "Your certificates", &["certmgr", "certificate"], "badge-check", Open("certmgr.msc")),
    c("shared-folders", "Shared Folders", "What this PC shares", &["fsmgmt", "shares"], "folder-cog", Open("fsmgmt.msc")),
    c("print-management", "Print Management", "Printers and drivers", &["printmanagement"], "printer", Open("printmanagement.msc")),
    c("component-services", "Component Services", "COM and DCOM", &["comexp", "dcom"], "boxes", Open("comexp.msc")),
    c("odbc-sources", "ODBC Data Sources", "Database connections", &["odbc", "odbcad32"], "database", Open("odbcad32.exe")),
    c("windows-firewall", "Windows Firewall Advanced", "Inbound and outbound rules", &["wf", "firewall rules"], "brick-wall", Open("wf.msc")),
    c("windows-security", "Windows Security", "Virus and threat protection", &["defender", "antivirus"], "shield-check", Open("windowsdefender:")),
    c("system-restore", "System Restore", "Go back to a restore point", &["rstrui", "restore point"], "history", Open("rstrui.exe")),
    c("recovery-drive", "Create a Recovery Drive", "A USB drive to repair this PC", &["recoverydrive", "usb recovery"], "usb", Open("RecoveryDrive.exe")),

    // Control Panel
    c("control-panel", "Control Panel", "Classic Windows settings", &["control"], "sliders-horizontal", Open("control.exe")),
    c("programs-and-features", "Programs and Features", "Uninstall programs", &["appwiz", "uninstall", "add or remove programs"], "package-x", Open("appwiz.cpl")),
    c("power-options", "Power Options", "Power plans", &["powercfg", "power plan", "battery"], "battery-charging", Open("powercfg.cpl")),
    c("system-properties", "System Properties", "Computer name and hardware", &["sysdm"], "laptop", Open("sysdm.cpl")),
    c("advanced-system-settings", "Advanced System Settings", "Performance and startup", &["system properties advanced"], "settings-2", Open("SystemPropertiesAdvanced.exe")),
    c("performance-options", "Performance Options", "Visual effects and virtual memory", &["visual effects", "virtual memory", "page file"], "gauge", Open("SystemPropertiesPerformance.exe")),
    c("system-protection", "System Protection", "Restore points", &["restore settings"], "shield", Open("SystemPropertiesProtection.exe")),
    c("remote-settings", "Remote Settings", "Allow remote connections", &["system properties remote"], "screen-share", Open("SystemPropertiesRemote.exe")),
    c("computer-name", "Change Computer Name", "Name and workgroup", &["rename pc", "workgroup", "domain"], "pencil", Open("SystemPropertiesComputerName.exe")),
    c("environment-variables", "Environment Variables", "PATH and other variables", &["env", "path", "system variables"], "braces", Program("rundll32.exe", "sysdm.cpl,EditEnvironmentVariables")),
    c("internet-options", "Internet Options", "Proxy and connection settings", &["inetcpl", "proxy"], "globe", Open("inetcpl.cpl")),
    c("mouse-properties", "Mouse Properties", "Buttons and pointers", &["main.cpl", "pointer"], "mouse", Open("main.cpl")),
    c("keyboard-properties", "Keyboard Properties", "Repeat rate", &["repeat delay"], "keyboard", Program("control.exe", "keyboard")),
    c("sound-control-panel", "Sound Control Panel", "All sound devices", &["mmsys", "sounds"], "volume-2", Open("mmsys.cpl")),
    c("date-and-time", "Date and Time Properties", "Clock and time zone", &["timedate", "clock"], "clock", Open("timedate.cpl")),
    c("region-properties", "Region Properties", "Date and number formats", &["intl", "locale"], "languages", Open("intl.cpl")),
    c("folder-options", "File Explorer Options", "Hidden files and extensions", &["folder options", "show hidden files", "file extensions"], "folder-cog", Program("control.exe", "folders")),
    c("fonts-control-panel", "Fonts", "Installed fonts", &["font"], "type", Program("control.exe", "fonts")),
    c("devices-and-printers", "Devices and Printers", "Connected devices", &["printers"], "printer", Program("control.exe", "printers")),
    c("game-controllers", "Game Controllers", "Gamepads and joysticks", &["joy", "gamepad", "joystick"], "gamepad-2", Open("joy.cpl")),
    c("indexing-options", "Indexing Options", "Windows Search index", &["search index"], "search", Program("control.exe", "srchadmin.dll")),
    c("credential-manager", "Credential Manager", "Saved passwords", &["passwords", "credentials"], "key-round", Program("control.exe", "/name Microsoft.CredentialManager")),
    c("file-history", "File History", "Back up your files", &["backup"], "archive-restore", Program("control.exe", "/name Microsoft.FileHistory")),
    c("backup-and-restore", "Backup and Restore", "Windows 7 style backups", &["backup", "system image"], "archive", Program("control.exe", "/name Microsoft.BackupAndRestore")),
    c("sync-center", "Sync Center", "Offline files", &["offline files"], "refresh-cw", Program("control.exe", "/name Microsoft.SyncCenter")),

    // Apps of Windows
    c("command-prompt", "Command Prompt", "cmd", &["cmd", "dos"], "terminal", Open("cmd.exe")),
    c("windows-powershell", "Windows PowerShell", "powershell", &["powershell", "ps"], "square-terminal", Open("powershell.exe")),
    c("notepad", "Notepad", "Plain text editor", &["text editor", "txt"], "notebook-pen", Open("notepad.exe")),
    c("paint", "Paint", "Draw and edit pictures", &["mspaint", "draw"], "paintbrush", Open("ms-paint:")),
    c("snipping-tool", "Snipping Tool", "Screenshots and recordings", &["snip", "screen recording"], "scissors", Open("ms-screenclip:")),
    c("clock", "Clock", "Alarms, timers and focus", &["alarm", "timer", "stopwatch"], "alarm-clock", Open("ms-clock:")),
    c("camera-app", "Camera", "Photos and video", &["webcam"], "camera", Open("microsoft.windows.camera:")),
    c("photos", "Photos", "Your pictures", &["gallery"], "image", Open("ms-photos:")),
    c("microsoft-store", "Microsoft Store", "Get apps", &["store", "install apps"], "store", Open("ms-windows-store:")),
    c("phone-link", "Phone Link", "Your phone on this PC", &["phone", "android"], "smartphone", Open("ms-phone:")),
    c("weather", "Weather", "The forecast", &["forecast"], "cloud-sun", Open("bingweather:")),
    c("get-help", "Get Help", "Microsoft support", &["support", "help"], "circle-help", Open("ms-contact-support:")),
    c("feedback-hub", "Feedback Hub", "Report a Windows problem", &["feedback"], "message-square", Open("feedback-hub:")),
];

pub fn all() -> impl Iterator<Item = &'static QuickAction> {
    COMMANDS.iter().map(|c| &c.action)
}

fn find(id: &str) -> Option<&'static SystemCommand> {
    COMMANDS.iter().find(|c| c.action.id == id)
}

/// Where Windows keeps its programs, and Store apps their command names.
fn program_dirs() -> Vec<PathBuf> {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let mut dirs = vec![root.join("System32"), root.join(r"System32\WindowsPowerShell\v1.0"), root.clone()];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join(r"Microsoft\WindowsApps"));
    }
    dirs
}

/// Whether Windows knows this link, or has this program.
fn target_exists(target: &str) -> bool {
    if target.starts_with("shell:") {
        return true;
    }
    if let Some((scheme, _)) = target.split_once(':') {
        return registry::read_string(HKEY_CLASSES_ROOT, scheme, "URL Protocol").is_some();
    }
    program_dirs().iter().any(|dir| dir.join(target).exists())
}

pub fn is_available(id: &str) -> bool {
    let Some(command) = find(id) else { return false };
    match command.run {
        // its folder only opens once OneDrive is set up, whatever the environment says
        Open(_) if id == "folder-onedrive" => {
            registry::read_string(HKEY_CURRENT_USER, r"Software\Microsoft\OneDrive", "UserFolder").is_some()
        }
        Open(target) | Program(target, _) => target_exists(target),
        // turned off with `powercfg /hibernate off` on many PCs
        Native(_) if id == "hibernate" => {
            registry::read_dword(HKEY_LOCAL_MACHINE, r"SYSTEM\CurrentControlSet\Control\Power", "HibernateEnabled").unwrap_or(1) != 0
        }
        _ => true,
    }
}

/// The app index knows many of these by name, which gives them their real icon.
pub fn title(id: &str) -> Option<&'static str> {
    find(id).map(|c| c.action.title)
}

pub fn run(id: &str) -> Result<(), String> {
    let command = find(id).ok_or_else(|| format!("unknown action {id}"))?;
    match command.run {
        Open(target) => shell_open(target, None),
        Program(program, arguments) => shell_run(program, arguments),
        Keys(keys) => {
            after_keys_up(move || press(keys));
            Ok(())
        }
        Native(action) => action(),
        Ask(question, action) if ask(question) => action(),
        Ask(..) => Ok(()),
    }
}

fn ask(question: &str) -> bool {
    let style = MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2 | MB_TOPMOST | MB_SETFOREGROUND;
    unsafe { MessageBoxW(None, &HSTRING::from(question), w!("Lummo"), style) == IDYES }
}

/// Runs `action` on its own thread once the keys of a hotkey are released and Spotlight has closed.
fn after_keys_up(action: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        let held = || [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN].iter().any(|k| unsafe { GetAsyncKeyState(k.0 as i32) } < 0);
        let started = Instant::now();
        while held() && started.elapsed() < KEYS_UP_TIMEOUT {
            std::thread::sleep(Duration::from_millis(15));
        }
        std::thread::sleep(SPOTLIGHT_GONE);
        action();
    });
}

fn key_input(key: VIRTUAL_KEY, up: bool) -> INPUT {
    let mut flags = if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
    if [VK_LEFT, VK_RIGHT, VK_LWIN].contains(&key) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: key, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn press(keys: &[VIRTUAL_KEY]) {
    let inputs: Vec<INPUT> =
        keys.iter().map(|k| key_input(*k, false)).chain(keys.iter().rev().map(|k| key_input(*k, true))).collect();
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        log::warn!("only {sent} of {} key events were sent", inputs.len());
    }
}

/// Runs a Windows program without a console window and waits for it.
fn quiet(program: &str, arguments: &[&str]) -> Result<(), String> {
    let status = std::process::Command::new(program)
        .args(arguments)
        .creation_flags(CREATE_NO_WINDOW.0)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed ({status})"))
    }
}

fn lock() -> Result<(), String> {
    unsafe { LockWorkStation() }.map_err(|e| e.message())
}

/// `SetSuspendState` needs this privilege, which processes hold but have turned off.
fn enable_shutdown_privilege() -> Result<(), String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token).map_err(|e| e.message())?;
        let mut luid = LUID::default();
        let result = LookupPrivilegeValueW(PCWSTR::null(), SE_SHUTDOWN_NAME, &mut luid).and_then(|()| {
            let privileges = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
            };
            AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None)
        });
        let _ = CloseHandle(token);
        result.map_err(|e| e.message())
    }
}

fn sleep() -> Result<(), String> {
    enable_shutdown_privilege()?;
    after_keys_up(|| {
        if !unsafe { SetSuspendState(false, false, false) } {
            log::warn!("Windows did not go to sleep: {}", windows::core::Error::from_thread().message());
        }
    });
    Ok(())
}

fn hibernate() -> Result<(), String> {
    after_keys_up(|| {
        if let Err(e) = quiet("shutdown.exe", &["/h"]) {
            log::warn!("Windows did not hibernate: {e}");
        }
    });
    Ok(())
}

// without `/f`, Windows still lets apps with unsaved work object
fn shut_down() -> Result<(), String> {
    quiet("shutdown.exe", &["/s", "/t", "0"])
}

fn restart() -> Result<(), String> {
    quiet("shutdown.exe", &["/r", "/t", "0"])
}

fn log_out() -> Result<(), String> {
    quiet("shutdown.exe", &["/l"])
}

fn turn_off_display() -> Result<(), String> {
    const OFF: isize = 2;
    after_keys_up(|| {
        let sent = unsafe { PostMessageW(Some(HWND_BROADCAST), WM_SYSCOMMAND, WPARAM(SC_MONITORPOWER as usize), LPARAM(OFF)) };
        if let Err(e) = sent {
            log::warn!("could not turn off the display: {}", e.message());
        }
    });
    Ok(())
}

fn empty_recycle_bin() -> Result<(), String> {
    match unsafe { SHEmptyRecycleBinW(None, PCWSTR::null(), SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND) } {
        // what Windows answers when it is already empty
        Err(e) if e.code() != E_UNEXPECTED => Err(e.message()),
        _ => Ok(()),
    }
}

fn flush_dns() -> Result<(), String> {
    quiet("ipconfig.exe", &["/flushdns"])
}

fn clear_clipboard() -> Result<(), String> {
    unsafe {
        OpenClipboard(None).map_err(|e| e.message())?;
        let emptied = EmptyClipboard();
        let _ = CloseClipboard();
        emptied.map_err(|e| e.message())
    }
}

fn open_temp() -> Result<(), String> {
    shell_open(&std::env::temp_dir().to_string_lossy(), None)
}

fn toggle_dark_mode() -> Result<(), String> {
    const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    let light = registry::read_dword(HKEY_CURRENT_USER, KEY, "AppsUseLightTheme").unwrap_or(1) != 0;
    let next = u32::from(!light);
    registry::write_dword(HKEY_CURRENT_USER, KEY, "AppsUseLightTheme", next)?;
    registry::write_dword(HKEY_CURRENT_USER, KEY, "SystemUsesLightTheme", next)?;
    // tells the taskbar and open apps
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(w!("ImmersiveColorSet").as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            200,
            None,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::actions::ACTIONS;
    use std::collections::HashSet;

    #[test]
    fn catalog_is_well_formed() {
        let ids: HashSet<_> = all().map(|a| a.id).collect();
        assert_eq!(ids.len(), COMMANDS.len(), "ids are unique");
        assert!(ACTIONS.iter().all(|a| !ids.contains(a.id)), "and differ from the built-in actions");
        for command in COMMANDS {
            let a = &command.action;
            assert!(!a.title.is_empty() && !a.subtitle.is_empty() && !a.keywords.is_empty(), "{}", a.id);
            assert!(a.keywords.iter().all(|k| *k == k.to_lowercase()), "{}: keywords are lowercase", a.id);
        }
    }

    #[test]
    fn what_can_lose_work_asks_first() {
        for id in ["shut-down", "restart", "log-out", "empty-recycle-bin"] {
            assert!(matches!(find(id).unwrap().run, Ask(..)), "{id}");
        }
    }

    #[test]
    fn programs_are_plain_names_of_windows_programs() {
        for command in COMMANDS {
            if let Open(target) | Program(target, _) = command.run {
                assert!(!target.contains(['\\', '/', '"', ' ']) || target.starts_with("shell:"), "{target}");
            }
        }
    }

    #[test]
    fn what_windows_lacks_is_not_offered() {
        assert!(target_exists("cmd.exe") && target_exists("ms-settings:bluetooth"));
        assert!(!target_exists("no-such-program.exe") && !target_exists("no-such-link:"));    }

    #[test]
    #[ignore = "reads what this PC has; runs nothing"]
    fn lists_what_this_pc_lacks() {
        let missing: Vec<_> = all().filter(|a| !is_available(a.id)).map(|a| a.id).collect();
        println!("{} of {} available, missing: {missing:?}", COMMANDS.len() - missing.len(), COMMANDS.len());
    }
}
