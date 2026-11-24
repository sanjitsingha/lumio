//! The System commands of Spotlight and the Shortcut page on Windows. Many
//! change the setting themselves, as Windows' own switch does (`toggles`);
//! the others open the tool, folder or panel of Windows that has it.
//!
//! To add one, add an entry to its section of [`SECTIONS`] (the section is
//! its category on the Shortcut page); its icon must be in `SYMBOLS`
//! (`ResultIcon.jsx`). Anything that can lose work, or leave the mouse or
//! keyboard hard to use, asks first ([`Run::Ask`]). A command this PC
//! cannot run (no battery, no Bluetooth, a program its edition lacks) is not
//! offered ([`Needs`]).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, E_UNEXPECTED, HANDLE, HWND, LPARAM, LUID, WPARAM};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, SE_SHUTDOWN_NAME,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard};
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::System::Registry::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use windows::Win32::System::Shutdown::LockWorkStation;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::{SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND};
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, PostMessageW, HWND_BROADCAST, IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO,
    SC_MONITORPOWER, SPI_GETACTIVEWINDOWTRACKING, SPI_GETCLIENTAREAANIMATION, SPI_GETDRAGFULLWINDOWS, SPI_GETKEYBOARDCUES,
    SPI_GETKEYBOARDDELAY, SPI_GETKEYBOARDSPEED, SPI_GETMOUSESONAR, SPI_GETMOUSESPEED, SPI_GETMOUSEVANISH,
    SPI_GETSNAPTODEFBUTTON, SPI_GETWHEELSCROLLLINES, SPI_GETWINARRANGING, SPI_SETACTIVEWINDOWTRACKING,
    SPI_SETCLIENTAREAANIMATION, SPI_SETDOUBLECLICKTIME, SPI_SETDRAGFULLWINDOWS, SPI_SETKEYBOARDCUES, SPI_SETKEYBOARDDELAY,
    SPI_SETKEYBOARDSPEED, SPI_SETMOUSESONAR, SPI_SETMOUSESPEED, SPI_SETMOUSEVANISH, SPI_SETSNAPTODEFBUTTON,
    SPI_SETWHEELSCROLLLINES, SPI_SETWINARRANGING, SYSTEM_PARAMETERS_INFO_ACTION, WM_SYSCOMMAND,
};

use super::launch::{shell_open, shell_run};
use super::registry;
use super::toggles::{self, quiet, Flag, RadioType, Refresh, ShellFlag, SpiFlag, SpiNumber, BALANCED};
use crate::search::actions::QuickAction;

/// How long Spotlight gets to close before keys are pressed or the PC is put to sleep.
const SPOTLIGHT_GONE: Duration = Duration::from_millis(180);
/// A hotkey's own keys would combine with the ones sent; wait this long for their release.
const KEYS_UP_TIMEOUT: Duration = Duration::from_secs(1);

const EXPLORER: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer";
const ADVANCED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const SEARCH: &str = r"Software\Microsoft\Windows\CurrentVersion\Search";
const GAME_BAR: &str = r"Software\Microsoft\GameBar";
const COLOR_FILTERS: &str = r"Software\Microsoft\ColorFiltering";

const HIGH_PERFORMANCE: u128 = 0x8c5e7fda_e8bf_4a96_9a85_a6e23a8c635c;
const POWER_SAVER: u128 = 0xa1841308_3541_4fab_bc81_f71556f20b4a;
const ULTIMATE_PERFORMANCE: u128 = 0xe9a42b02_d5df_448d_aa00_03f14749eb61;
/// Overlays of the Balanced plan, as Settings' Power mode sets them.
const BEST_EFFICIENCY: u128 = 0x961cc777_2547_4f9d_8174_7d86181b8a7a;
const BEST_PERFORMANCE: u128 = 0xded574b5_45a0_4f42_8737_46345c09c238;

const APPS_DARK: Flag = reg(PERSONALIZE, "AppsUseLightTheme", 0, 1, false, Refresh::Theme);
const WINDOWS_DARK: Flag = reg(PERSONALIZE, "SystemUsesLightTheme", 0, 1, false, Refresh::Theme);

const MOUSE_SPEED: SpiNumber = SpiNumber { get: SPI_GETMOUSESPEED, set: SPI_SETMOUSESPEED, in_pointer: true, min: 1, max: 20 };
const KEY_SPEED: SpiNumber = SpiNumber { get: SPI_GETKEYBOARDSPEED, set: SPI_SETKEYBOARDSPEED, in_pointer: false, min: 0, max: 31 };
const KEY_DELAY: SpiNumber = SpiNumber { get: SPI_GETKEYBOARDDELAY, set: SPI_SETKEYBOARDDELAY, in_pointer: false, min: 0, max: 3 };
const SCROLL_LINES: SpiNumber =
    SpiNumber { get: SPI_GETWHEELSCROLLLINES, set: SPI_SETWHEELSCROLLLINES, in_pointer: false, min: 1, max: 100 };

#[derive(Clone, Copy)]
enum Run {
    // These open something, or do something once.
    /// A link, a `shell:` folder or a program of Windows, through the shell.
    Open(&'static str),
    /// A program of Windows with fixed arguments.
    Program(&'static str, &'static str),
    /// A key combination Windows itself answers to.
    Keys(&'static [VIRTUAL_KEY]),
    Native(fn() -> Result<(), String>),
    /// Can lose work, so it asks this question first.
    Ask(&'static str, fn() -> Result<(), String>),

    // These change something the moment they run (see `changes_a_setting`).
    /// Keys that turn something on or off.
    Press(&'static [VIRTUAL_KEY]),
    Switch(fn() -> Result<(), String>),
    Reg(Flag),
    /// Sets a DWORD of the current user: key, value, data.
    Dword(&'static str, &'static str, u32, Refresh),
    Spi(SpiFlag),
    Step(SpiNumber, i32),
    Number(SpiNumber, u32),
    SetOnly(SYSTEM_PARAMETERS_INFO_ACTION, u32),
    Shell(ShellFlag),
    Volume(f32),
    Microphone(f32),
    Radio(RadioType),
    /// A `powercfg /change` timeout, in minutes (0 is never).
    Timeout(&'static str, u32),
    /// What closing the lid does, plugged in or on battery.
    Lid(bool, u32),
    Plan(u128),
    Mode(u128),
    /// Acts on the window in front once Spotlight has given focus back.
    Window(fn(HWND) -> Result<(), String>),
}
use Run::{
    Ask, Dword, Keys, Lid, Microphone, Mode, Native, Number, Open, Plan, Press, Program, Radio, Reg, SetOnly, Shell, Spi,
    Step, Switch, Timeout, Volume, Window,
};

impl Run {
    /// Enter runs Spotlight's top result, so these are listed only when one
    /// of their names is typed (`search::actions::named`), never for a topic.
    fn changes_a_setting(&self) -> bool {
        !matches!(self, Open(_) | Program(..) | Keys(_) | Native(_) | Ask(..))
    }
}

/// What a command needs of this PC, beyond its program or link.
#[derive(Clone, Copy)]
enum Needs {
    Battery,
    Windows11,
    Microphone,
    Hibernate,
    OneDrive,
    ColorFilterShortcut,
    /// A program or a link Windows knows.
    Link(&'static str),
}

struct SystemCommand {
    action: QuickAction,
    needs: Option<Needs>,
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
    SystemCommand { action: QuickAction { id, title, subtitle, keywords, icon }, needs: None, run }
}

const fn n(
    needs: Needs,
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
    keywords: &'static [&'static str],
    icon: &'static str,
    run: Run,
) -> SystemCommand {
    SystemCommand { action: QuickAction { id, title, subtitle, keywords, icon }, needs: Some(needs), run }
}

const fn reg(key: &'static str, name: &'static str, on: u32, off: u32, missing_on: bool, refresh: Refresh) -> Flag {
    Flag { key, name, on, off, missing_on, refresh }
}

const fn spi(get: SYSTEM_PARAMETERS_INFO_ACTION, set: SYSTEM_PARAMETERS_INFO_ACTION, in_pointer: bool) -> Run {
    Spi(SpiFlag { get, set, in_pointer })
}

/// Each section is a category of the Shortcut page.
#[rustfmt::skip]
static SECTIONS: &[(&str, &[SystemCommand])] = &[
    ("Session", &[
        c("lock-screen", "Lock Screen", "Lock this PC", &["lock", "lock pc", "lock computer"], "lock", Native(lock)),
        c("sleep", "Sleep", "Put this PC to sleep", &["sleep", "suspend", "standby"], "moon", Native(sleep)),
        n(Needs::Hibernate, "hibernate", "Hibernate", "Save your session and power off", &["hibernate"], "moon-star", Native(hibernate)),
        c("shut-down", "Shut Down", "Turn off this PC", &["shutdown", "shut down", "power off", "turn off"], "power", Ask("Shut down this PC?", shut_down)),
        c("restart", "Restart", "Restart this PC", &["restart", "reboot"], "rotate-cw", Ask("Restart this PC?", restart)),
        c("log-out", "Log Out", "Sign out of Windows", &["logout", "log off", "logoff", "sign out"], "log-out", Ask("Log out of Windows?", log_out)),
        n(Needs::Link("tsdiscon.exe"), "switch-user", "Switch User", "To the sign-in screen; your apps keep running", &["switch user", "change user", "other user", "switch account"], "users", Native(switch_user)),
    ]),

    ("Power", &[
        c("turn-off-display", "Turn Off Display", "Put the screen to sleep", &["screen off", "monitor off", "display off"], "monitor-off", Native(turn_off_display)),
        c("keep-awake", "Toggle Keep Awake", "Stop this PC from sleeping or dimming", &["keep awake", "toggle keep awake", "caffeine", "stay awake", "no sleep", "prevent sleep", "amphetamine"], "coffee", Switch(toggles::flip_keep_awake)),
        c("power-mode-best-performance", "Power Mode Best Performance", "Windows' Power mode", &["power mode best performance", "best performance", "best performance mode"], "gauge", Mode(BEST_PERFORMANCE)),
        c("power-mode-balanced", "Power Mode Balanced", "Windows' Power mode", &["power mode balanced", "balanced power mode"], "gauge", Mode(0)),
        c("power-mode-best-efficiency", "Power Mode Best Power Efficiency", "Windows' Power mode", &["power mode best power efficiency", "best power efficiency", "power efficiency mode"], "gauge", Mode(BEST_EFFICIENCY)),
        c("power-plan-balanced", "Power Plan Balanced", "Classic power plan", &["power plan balanced", "balanced power plan", "balanced plan"], "battery-charging", Plan(BALANCED)),
        c("power-plan-high-performance", "Power Plan High Performance", "Classic power plan", &["power plan high performance", "high performance power plan", "high performance plan"], "battery-charging", Plan(HIGH_PERFORMANCE)),
        c("power-plan-power-saver", "Power Plan Power Saver", "Classic power plan", &["power plan power saver", "power saver power plan", "power saver plan"], "battery-charging", Plan(POWER_SAVER)),
        c("power-plan-ultimate", "Power Plan Ultimate Performance", "Classic power plan", &["power plan ultimate performance", "ultimate performance", "ultimate performance plan"], "battery-charging", Plan(ULTIMATE_PERFORMANCE)),
        c("screen-off-plugged-5", "Turn Off Screen After 5 Minutes (Plugged In)", "Screen timeout", &["turn off screen after 5 minutes plugged in", "screen timeout 5 minutes plugged in"], "monitor-off", Timeout("monitor-timeout-ac", 5)),
        c("screen-off-plugged-10", "Turn Off Screen After 10 Minutes (Plugged In)", "Screen timeout", &["turn off screen after 10 minutes plugged in", "screen timeout 10 minutes plugged in"], "monitor-off", Timeout("monitor-timeout-ac", 10)),
        c("screen-off-plugged-15", "Turn Off Screen After 15 Minutes (Plugged In)", "Screen timeout", &["turn off screen after 15 minutes plugged in", "screen timeout 15 minutes plugged in"], "monitor-off", Timeout("monitor-timeout-ac", 15)),
        c("screen-off-plugged-30", "Turn Off Screen After 30 Minutes (Plugged In)", "Screen timeout", &["turn off screen after 30 minutes plugged in", "screen timeout 30 minutes plugged in"], "monitor-off", Timeout("monitor-timeout-ac", 30)),
        c("screen-off-plugged-never", "Never Turn Off Screen (Plugged In)", "Screen timeout", &["never turn off screen plugged in", "screen timeout never plugged in"], "monitor", Timeout("monitor-timeout-ac", 0)),
        n(Needs::Battery, "screen-off-battery-5", "Turn Off Screen After 5 Minutes (On Battery)", "Screen timeout", &["turn off screen after 5 minutes on battery", "screen timeout 5 minutes on battery"], "monitor-off", Timeout("monitor-timeout-dc", 5)),
        n(Needs::Battery, "screen-off-battery-10", "Turn Off Screen After 10 Minutes (On Battery)", "Screen timeout", &["turn off screen after 10 minutes on battery", "screen timeout 10 minutes on battery"], "monitor-off", Timeout("monitor-timeout-dc", 10)),
        n(Needs::Battery, "screen-off-battery-15", "Turn Off Screen After 15 Minutes (On Battery)", "Screen timeout", &["turn off screen after 15 minutes on battery", "screen timeout 15 minutes on battery"], "monitor-off", Timeout("monitor-timeout-dc", 15)),
        n(Needs::Battery, "screen-off-battery-30", "Turn Off Screen After 30 Minutes (On Battery)", "Screen timeout", &["turn off screen after 30 minutes on battery", "screen timeout 30 minutes on battery"], "monitor-off", Timeout("monitor-timeout-dc", 30)),
        n(Needs::Battery, "screen-off-battery-never", "Never Turn Off Screen (On Battery)", "Screen timeout", &["never turn off screen on battery", "screen timeout never on battery"], "monitor", Timeout("monitor-timeout-dc", 0)),
        c("sleep-plugged-10", "Sleep After 10 Minutes (Plugged In)", "Sleep timeout", &["sleep after 10 minutes plugged in"], "timer", Timeout("standby-timeout-ac", 10)),
        c("sleep-plugged-15", "Sleep After 15 Minutes (Plugged In)", "Sleep timeout", &["sleep after 15 minutes plugged in"], "timer", Timeout("standby-timeout-ac", 15)),
        c("sleep-plugged-30", "Sleep After 30 Minutes (Plugged In)", "Sleep timeout", &["sleep after 30 minutes plugged in"], "timer", Timeout("standby-timeout-ac", 30)),
        c("sleep-plugged-60", "Sleep After 1 Hour (Plugged In)", "Sleep timeout", &["sleep after 1 hour plugged in", "sleep after 60 minutes plugged in"], "timer", Timeout("standby-timeout-ac", 60)),
        c("sleep-plugged-never", "Never Sleep (Plugged In)", "Sleep timeout", &["never sleep plugged in", "sleep never plugged in"], "coffee", Timeout("standby-timeout-ac", 0)),
        n(Needs::Battery, "sleep-battery-10", "Sleep After 10 Minutes (On Battery)", "Sleep timeout", &["sleep after 10 minutes on battery"], "timer", Timeout("standby-timeout-dc", 10)),
        n(Needs::Battery, "sleep-battery-15", "Sleep After 15 Minutes (On Battery)", "Sleep timeout", &["sleep after 15 minutes on battery"], "timer", Timeout("standby-timeout-dc", 15)),
        n(Needs::Battery, "sleep-battery-30", "Sleep After 30 Minutes (On Battery)", "Sleep timeout", &["sleep after 30 minutes on battery"], "timer", Timeout("standby-timeout-dc", 30)),
        n(Needs::Battery, "sleep-battery-60", "Sleep After 1 Hour (On Battery)", "Sleep timeout", &["sleep after 1 hour on battery", "sleep after 60 minutes on battery"], "timer", Timeout("standby-timeout-dc", 60)),
        n(Needs::Battery, "sleep-battery-never", "Never Sleep (On Battery)", "Sleep timeout", &["never sleep on battery", "sleep never on battery"], "coffee", Timeout("standby-timeout-dc", 0)),
        n(Needs::Battery, "lid-nothing-plugged", "Do Nothing When Lid Closes (Plugged In)", "Closing the lid", &["do nothing when lid closes plugged in", "lid do nothing plugged in", "clamshell mode"], "laptop", Lid(true, 0)),
        n(Needs::Battery, "lid-sleep-plugged", "Sleep When Lid Closes (Plugged In)", "Closing the lid", &["sleep when lid closes plugged in", "lid sleep plugged in"], "laptop", Lid(true, 1)),
        n(Needs::Battery, "lid-nothing-battery", "Do Nothing When Lid Closes (On Battery)", "Closing the lid", &["do nothing when lid closes on battery", "lid do nothing on battery"], "laptop", Lid(false, 0)),
        n(Needs::Battery, "lid-sleep-battery", "Sleep When Lid Closes (On Battery)", "Closing the lid", &["sleep when lid closes on battery", "lid sleep on battery"], "laptop", Lid(false, 1)),
        n(Needs::Battery, "battery-report", "Battery Report", "Battery health and use, in your browser", &["battery report", "battery health", "batteryreport"], "battery", Native(toggles::battery_report)),
        c("power-options", "Power Options", "Power plans", &["powercfg", "power plan", "battery"], "battery-charging", Open("powercfg.cpl")),
        c("power-buttons", "Choose What the Power Buttons Do", "Power button, sleep button and lid", &["power button", "sleep button", "fast startup", "lid action"], "power", Program("control.exe", "/name Microsoft.PowerOptions /page pageGlobalSettings")),
        c("mobility-center", "Windows Mobility Center", "Laptop settings at a glance", &["mblctr", "mobility center"], "laptop", Open("mblctr.exe")),
    ]),

    ("Sound", &[
        c("volume-up", "Volume Up", "Raise the sound", &["louder", "increase volume", "sound up"], "volume-2", Keys(&[VK_VOLUME_UP])),
        c("volume-down", "Volume Down", "Lower the sound", &["quieter", "decrease volume", "sound down"], "volume-1", Keys(&[VK_VOLUME_DOWN])),
        c("toggle-mute", "Toggle Mute", "Mute or unmute the sound", &["mute", "unmute", "silence"], "volume-x", Keys(&[VK_VOLUME_MUTE])),
        c("volume-10", "Set Volume to 10%", "Speakers or headphones", &["volume 10", "volume 10%", "set volume to 10"], "volume-1", Volume(0.10)),
        c("volume-25", "Set Volume to 25%", "Speakers or headphones", &["volume 25", "volume 25%", "set volume to 25"], "volume-1", Volume(0.25)),
        c("volume-50", "Set Volume to 50%", "Speakers or headphones", &["volume 50", "volume 50%", "set volume to 50", "half volume"], "volume-2", Volume(0.50)),
        c("volume-75", "Set Volume to 75%", "Speakers or headphones", &["volume 75", "volume 75%", "set volume to 75"], "volume-2", Volume(0.75)),
        c("volume-100", "Set Volume to 100%", "Speakers or headphones", &["volume 100", "volume 100%", "set volume to 100", "full volume", "max volume"], "volume-2", Volume(1.0)),
        n(Needs::Microphone, "mic-mute", "Toggle Microphone Mute", "Mute or unmute the microphone", &["toggle microphone mute", "mute microphone", "unmute microphone", "mute mic", "unmute mic"], "mic-off", Switch(toggles::flip_microphone_mute)),
        n(Needs::Microphone, "mic-up", "Microphone Volume Up", "By a tenth", &["microphone volume up", "mic volume up", "louder microphone"], "mic", Switch(microphone_up)),
        n(Needs::Microphone, "mic-down", "Microphone Volume Down", "By a tenth", &["microphone volume down", "mic volume down", "quieter microphone"], "mic", Switch(microphone_down)),
        c("mic-25", "Set Microphone to 25%", "Microphone volume", &["microphone 25", "microphone 25%", "mic 25", "set microphone to 25"], "mic", Microphone(0.25)),
        c("mic-50", "Set Microphone to 50%", "Microphone volume", &["microphone 50", "microphone 50%", "mic 50", "set microphone to 50"], "mic", Microphone(0.50)),
        c("mic-75", "Set Microphone to 75%", "Microphone volume", &["microphone 75", "microphone 75%", "mic 75", "set microphone to 75"], "mic", Microphone(0.75)),
        c("mic-100", "Set Microphone to 100%", "Microphone volume", &["microphone 100", "microphone 100%", "mic 100", "set microphone to 100"], "mic", Microphone(1.0)),
        n(Needs::Windows11, "sound-output-picker", "Choose Sound Output", "Speakers or headphones, in Quick Settings", &["choose sound output", "switch speakers", "switch headphones", "change audio output"], "headphones", Keys(&[VK_LWIN, VK_CONTROL, VK_V])),
        c("sound-output", "Manage Sound Output Devices", "Speakers and headphones", &["speakers", "headphones", "output device", "playback"], "headphones", Program("control.exe", "mmsys.cpl,,0")),
        c("sound-input", "Manage Sound Input Devices", "Microphones", &["microphone", "mic", "input device", "recording devices"], "mic", Program("control.exe", "mmsys.cpl,,1")),
        c("volume-mixer-classic", "Volume Mixer", "The volume of each app", &["mixer", "app volume", "sndvol"], "sliders-horizontal", Open("sndvol.exe")),
        c("system-sounds", "Change System Sounds", "Sound scheme and alerts", &["sound scheme", "system sounds", "startup sound", "notification sound"], "bell", Program("control.exe", "mmsys.cpl,,2")),
        c("sound-control-panel", "Sound Control Panel", "All sound devices", &["mmsys", "sounds"], "volume-2", Open("mmsys.cpl")),
    ]),

    ("Media", &[
        c("media-play-pause", "Play or Pause Media", "The song or video playing", &["play", "pause", "music", "resume"], "play", Keys(&[VK_MEDIA_PLAY_PAUSE])),
        c("media-next", "Next Track", "Skip to the next one", &["next song", "skip", "next media"], "skip-forward", Keys(&[VK_MEDIA_NEXT_TRACK])),
        c("media-previous", "Previous Track", "Back to the last one", &["previous song", "last song", "previous media"], "skip-back", Keys(&[VK_MEDIA_PREV_TRACK])),
        c("media-stop", "Stop Media", "Stop what is playing", &["stop music", "stop playing"], "square", Keys(&[VK_MEDIA_STOP])),
    ]),

    ("Display", &[
        c("project-screen", "Project to a Screen", "Choose how displays are used", &["project", "second screen", "external display"], "projector", Keys(&[VK_LWIN, VK_P])),
        c("display-extend", "Extend Displays", "Use both screens", &["extend", "dual monitor"], "monitor-up", Program("DisplaySwitch.exe", "/extend")),
        c("display-duplicate", "Duplicate Displays", "Show the same on both", &["duplicate", "mirror", "clone display"], "copy", Program("DisplaySwitch.exe", "/clone")),
        c("display-second-only", "Second Screen Only", "Use only the external display", &["external only"], "monitor-check", Program("DisplaySwitch.exe", "/external")),
        c("display-pc-only", "PC Screen Only", "Use only this screen", &["internal only", "laptop screen"], "laptop", Program("DisplaySwitch.exe", "/internal")),
        n(Needs::Windows11, "toggle-hdr", "Toggle HDR", "On displays that support it", &["toggle hdr", "hdr on", "hdr off", "turn on hdr", "turn off hdr"], "sun", Press(&[VK_LWIN, VK_MENU, VK_B])),
        c("cast-display", "Cast to a Display", "A wireless screen", &["cast", "miracast", "wireless display"], "cast", Keys(&[VK_LWIN, VK_K])),
        c("color-calibration", "Display Color Calibration", "Tune your screen's colors", &["dccw", "calibrate"], "palette", Open("dccw.exe")),
        c("color-management", "Color Management", "Display color profiles", &["colorcpl", "icc profile", "color profile"], "palette", Open("colorcpl.exe")),
        c("cleartype", "ClearType Text Tuner", "Make text easier to read", &["cttune", "font smoothing"], "type", Open("cttune.exe")),
    ]),

    ("Appearance", &[
        c("toggle-dark-mode", "Toggle Dark Mode", "Switch Windows between light and dark", &["toggle dark mode", "dark mode", "light mode", "auto dark mode"], "sun-moon", Switch(toggle_dark_mode)),
        c("dark-mode-on", "Use Dark Mode", "For apps and for Windows", &["use dark mode", "dark mode on", "turn on dark mode", "enable dark mode"], "moon", Switch(use_dark_mode)),
        c("light-mode-on", "Use Light Mode", "For apps and for Windows", &["use light mode", "light mode on", "turn on light mode", "enable light mode"], "sun", Switch(use_light_mode)),
        c("dark-mode-apps", "Toggle Dark Mode for Apps", "Apps only, not the taskbar and Start", &["toggle dark mode for apps", "dark mode for apps", "app dark mode"], "sun-moon", Reg(APPS_DARK)),
        c("dark-mode-windows", "Toggle Dark Mode for Windows", "The taskbar and Start only", &["toggle dark mode for windows", "dark mode for windows", "dark taskbar"], "sun-moon", Reg(WINDOWS_DARK)),
        c("transparency", "Toggle Transparency Effects", "See-through taskbar and windows", &["toggle transparency effects", "toggle transparency", "transparency on", "transparency off", "transparency effects"], "layers", Reg(reg(PERSONALIZE, "EnableTransparency", 1, 0, true, Refresh::Theme))),
        c("accent-taskbar", "Toggle Accent Color on Start and Taskbar", "With dark mode for Windows", &["toggle accent color on start and taskbar", "accent color on taskbar", "colored taskbar"], "palette", Reg(reg(PERSONALIZE, "ColorPrevalence", 1, 0, false, Refresh::Theme))),
        c("desktop-icons", "Toggle Desktop Icons", "Show or hide the icons on the desktop", &["toggle desktop icons", "show desktop icons", "hide desktop icons", "desktop icons on", "desktop icons off"], "layout-grid", Switch(toggles::flip_desktop_icons)),
        c("desktop-icon-settings", "Desktop Icon Settings", "This PC, Recycle Bin and others on the desktop", &["desktop icon settings", "show this pc on desktop", "recycle bin icon"], "monitor", Program("rundll32.exe", "shell32.dll,Control_RunDLL desk.cpl,,0")),
        c("screen-saver", "Screen Saver Settings", "Pick a screen saver", &["screen saver", "screensaver"], "monitor", Program("rundll32.exe", "shell32.dll,Control_RunDLL desk.cpl,,1")),
        c("fonts-control-panel", "Fonts", "Installed fonts", &["font"], "type", Program("control.exe", "fonts")),
    ]),

    ("Taskbar & Start", &[
        c("open-start", "Open Start Menu", "Pinned and all apps", &["start menu", "open start"], "layout-grid", Keys(&[VK_LWIN])),
        c("open-search", "Open Windows Search", "Apps, files and the web", &["windows search", "open search", "search windows"], "search", Keys(&[VK_LWIN, VK_S])),
        c("quick-link-menu", "Open Quick Link Menu", "The menu of Start's right-click", &["quick link menu", "win x", "power user menu"], "square-chevron-right", Keys(&[VK_LWIN, VK_X])),
        c("run-dialog", "Run Dialog", "Open anything by its name", &["run", "win r"], "square-chevron-right", Keys(&[VK_LWIN, VK_R])),
        n(Needs::Windows11, "widgets", "Open Widgets", "News, weather and more", &["widgets", "news"], "panel-top", Keys(&[VK_LWIN, VK_W])),
        c("taskbar-autohide", "Toggle Taskbar Auto-Hide", "Hidden until the pointer reaches it", &["toggle taskbar auto-hide", "auto hide taskbar", "autohide taskbar", "hide taskbar", "show taskbar"], "panel-bottom", Switch(toggles::flip_taskbar_autohide)),
        n(Needs::Windows11, "taskbar-left", "Align Taskbar Left", "Start and apps at the left", &["align taskbar left", "taskbar left", "left taskbar", "start button left"], "panel-bottom", Dword(ADVANCED, "TaskbarAl", 0, Refresh::Shell)),
        n(Needs::Windows11, "taskbar-center", "Center Taskbar", "Start and apps in the middle", &["center taskbar", "align taskbar center", "taskbar center", "centered taskbar"], "panel-bottom", Dword(ADVANCED, "TaskbarAl", 1, Refresh::Shell)),
        c("taskbar-task-view", "Toggle Task View Button", "On the taskbar", &["toggle task view button", "task view button", "hide task view button"], "layout-grid", Reg(reg(ADVANCED, "ShowTaskViewButton", 1, 0, true, Refresh::Shell))),
        c("taskbar-search-hidden", "Hide Search on Taskbar", "Taskbar search", &["hide search on taskbar", "hide taskbar search", "remove search box"], "search", Dword(SEARCH, "SearchboxTaskbarMode", 0, Refresh::Shell)),
        c("taskbar-search-icon", "Search Icon on Taskbar", "Taskbar search", &["search icon on taskbar", "taskbar search icon", "search icon only"], "search", Dword(SEARCH, "SearchboxTaskbarMode", 1, Refresh::Shell)),
        c("taskbar-search-box", "Search Box on Taskbar", "Taskbar search", &["search box on taskbar", "taskbar search box", "show search box"], "search", Dword(SEARCH, "SearchboxTaskbarMode", 2, Refresh::Shell)),
    ]),

    ("Windows", &[
        c("show-desktop", "Show Desktop", "Show or hide all windows", &["desktop", "hide windows", "peek"], "monitor", Keys(&[VK_LWIN, VK_D])),
        c("minimize-all", "Minimize All Windows", "Clear the screen", &["minimize", "hide all"], "minimize-2", Keys(&[VK_LWIN, VK_M])),
        c("restore-minimized", "Restore Minimized Windows", "What Minimize All hid", &["restore minimized windows", "restore all windows", "unminimize"], "maximize-2", Keys(&[VK_LWIN, VK_SHIFT, VK_M])),
        c("task-view", "Task View", "All windows and desktops", &["windows", "alt tab", "overview", "mission control"], "layout-grid", Keys(&[VK_LWIN, VK_TAB])),
        c("window-maximize", "Maximize Window", "Or restore it when maximized", &["maximize window", "maximise window", "maximize"], "maximize-2", Window(toggles::flip_maximized)),
        c("window-minimize", "Minimize Window", "The window in front", &["minimize window", "minimise window"], "minimize-2", Window(toggles::minimize)),
        c("window-restore", "Restore Window", "Its size before maximizing", &["restore window", "unmaximize"], "copy", Window(toggles::restore)),
        c("window-close", "Close Window", "As its close button does", &["close window", "close this window"], "x", Window(toggles::close)),
        c("window-always-on-top", "Toggle Always on Top", "Keep the window above the others", &["toggle always on top", "always on top", "pin window", "keep on top", "stay on top"], "pin", Window(toggles::flip_always_on_top)),
        c("window-center", "Center Window", "In the middle of its display", &["center window", "centre window"], "scan", Window(toggles::center)),
        c("window-snap-left", "Snap Window Left", "To the left half of the screen", &["snap window left", "snap left", "window left half"], "arrow-left", Window(snap_left)),
        c("window-snap-right", "Snap Window Right", "To the right half of the screen", &["snap window right", "snap right", "window right half"], "arrow-right", Window(snap_right)),
        c("window-next-display", "Move Window to Next Display", "The display at the right", &["move window to next display", "move window to other monitor", "next monitor"], "monitor-up", Window(next_display)),
        c("window-previous-display", "Move Window to Previous Display", "The display at the left", &["move window to previous display", "previous monitor"], "monitor-up", Window(previous_display)),
        n(Needs::Windows11, "snap-layouts", "Open Snap Layouts", "Arrange the window in a layout", &["open snap layouts", "snap layouts", "snap layout"], "grid-2x2", Window(snap_layouts)),
    ]),

    ("Window Behavior", &[
        c("snap-windows", "Toggle Snap Windows", "Snap windows to the screen's edges", &["toggle snap windows", "snap windows on", "snap windows off", "aero snap"], "grid-2x2", spi(SPI_GETWINARRANGING, SPI_SETWINARRANGING, false)),
        c("focus-follows-mouse", "Toggle Focus Follows Mouse", "Activate a window by pointing at it", &["toggle focus follows mouse", "focus follows mouse", "x-mouse", "xmouse"], "mouse-pointer-2", spi(SPI_GETACTIVEWINDOWTRACKING, SPI_SETACTIVEWINDOWTRACKING, true)),
        c("drag-full-windows", "Toggle Window Contents While Dragging", "Or only an outline", &["toggle window contents while dragging", "show window contents while dragging", "drag full windows"], "copy", spi(SPI_GETDRAGFULLWINDOWS, SPI_SETDRAGFULLWINDOWS, false)),
    ]),

    ("Virtual Desktops", &[
        c("new-virtual-desktop", "New Virtual Desktop", "Add a desktop", &["add desktop", "new desktop", "workspace"], "square-plus", Keys(&[VK_LWIN, VK_CONTROL, VK_D])),
        c("close-virtual-desktop", "Close Virtual Desktop", "Its windows move to the next one", &["remove desktop", "close desktop"], "square-x", Keys(&[VK_LWIN, VK_CONTROL, VK_F4])),
        c("next-virtual-desktop", "Next Virtual Desktop", "Switch right", &["next desktop", "switch desktop"], "arrow-right", Keys(&[VK_LWIN, VK_CONTROL, VK_RIGHT])),
        c("previous-virtual-desktop", "Previous Virtual Desktop", "Switch left", &["previous desktop", "switch desktop"], "arrow-left", Keys(&[VK_LWIN, VK_CONTROL, VK_LEFT])),
    ]),

    ("Network", &[
        c("wifi-toggle", "Toggle Wi-Fi", "Turn Wi-Fi on or off", &["toggle wifi", "toggle wi-fi", "wifi on", "wifi off", "wi-fi on", "wi-fi off", "turn on wifi", "turn off wifi", "enable wifi", "disable wifi"], "wifi", Radio(RadioType::WiFi)),
        c("manage-wifi", "Manage Wi-Fi Connections", "Available networks", &["wifi", "wi-fi", "wireless", "networks"], "wifi", Open("ms-availablenetworks:")),
        c("network-connections", "Network Connections", "Network adapters", &["adapters", "ethernet", "ncpa"], "network", Open("ncpa.cpl")),
        c("network-sharing-center", "Network and Sharing Center", "Connections and sharing", &["network and sharing center", "sharing center"], "network", Program("control.exe", "/name Microsoft.NetworkAndSharingCenter")),
        c("advanced-sharing", "Advanced Sharing Settings", "Network discovery and file sharing", &["network discovery", "file sharing", "public folder sharing"], "network", Program("control.exe", "/name Microsoft.NetworkAndSharingCenter /page Advanced")),
        c("flush-dns", "Flush DNS Cache", "Forget saved website addresses", &["dns", "flushdns", "clear dns"], "refresh-cw", Native(flush_dns)),
        c("quick-settings", "Open Quick Settings", "Wi-Fi, Bluetooth and sound", &["action center", "quick settings"], "sliders-horizontal", Keys(&[VK_LWIN, VK_A])),
        c("internet-options", "Internet Options", "Proxy and connection settings", &["inetcpl", "proxy"], "globe", Open("inetcpl.cpl")),
        c("remote-desktop-connection", "Remote Desktop Connection", "Connect to another PC", &["rdp", "mstsc", "remote"], "screen-share", Open("mstsc.exe")),
        c("remote-assistance", "Windows Remote Assistance", "Get or give help", &["msra", "remote help"], "life-buoy", Open("msra.exe")),
        c("quick-assist", "Quick Assist", "Help someone, or get help, over the internet", &["quick assist", "screen sharing help"], "life-buoy", Open("ms-quick-assist:")),
    ]),

    ("Bluetooth", &[
        c("bluetooth-toggle", "Toggle Bluetooth", "Turn Bluetooth on or off", &["toggle bluetooth", "bluetooth on", "bluetooth off", "turn on bluetooth", "turn off bluetooth", "enable bluetooth", "disable bluetooth"], "bluetooth", Radio(RadioType::Bluetooth)),
        c("manage-bluetooth", "Manage Bluetooth Devices", "Pair and connect", &["bluetooth", "pair", "devices"], "bluetooth", Open("ms-settings:bluetooth")),
        c("bluetooth-files", "Send or Receive Files via Bluetooth", "Bluetooth file transfer", &["fsquirt", "bluetooth file transfer", "send file bluetooth", "receive file bluetooth"], "bluetooth", Open("fsquirt.exe")),
    ]),

    ("Notifications", &[
        c("notification-center", "Open Notification Center", "Notifications and calendar", &["notifications", "alerts", "calendar flyout"], "bell", Keys(&[VK_LWIN, VK_N])),
    ]),

    ("Keyboard", &[
        c("emoji-picker", "Emoji Picker", "Emoji, symbols and GIFs", &["emoji", "symbols", "kaomoji", "gif"], "smile", Keys(&[VK_LWIN, VK_OEM_PERIOD])),
        c("voice-typing", "Voice Typing", "Dictate into any text field", &["dictation", "speech to text"], "audio-lines", Keys(&[VK_LWIN, VK_H])),
        c("next-keyboard-layout", "Next Keyboard Layout", "Switch the input language", &["next keyboard layout", "switch keyboard layout", "next input language", "change keyboard language"], "languages", Keys(&[VK_LWIN, VK_SPACE])),
        c("key-repeat-slow", "Key Repeat Speed Slow", "How fast a held key repeats", &["key repeat speed slow", "slow key repeat"], "keyboard", Number(KEY_SPEED, 8)),
        c("key-repeat-medium", "Key Repeat Speed Medium", "How fast a held key repeats", &["key repeat speed medium", "medium key repeat"], "keyboard", Number(KEY_SPEED, 20)),
        c("key-repeat-fast", "Key Repeat Speed Fast", "How fast a held key repeats", &["key repeat speed fast", "fast key repeat"], "keyboard", Number(KEY_SPEED, 31)),
        c("key-delay-short", "Key Repeat Delay Short", "How soon a held key repeats", &["key repeat delay short", "short repeat delay"], "keyboard", Number(KEY_DELAY, 0)),
        c("key-delay-medium", "Key Repeat Delay Medium", "How soon a held key repeats", &["key repeat delay medium", "medium repeat delay"], "keyboard", Number(KEY_DELAY, 1)),
        c("key-delay-long", "Key Repeat Delay Long", "How soon a held key repeats", &["key repeat delay long", "long repeat delay"], "keyboard", Number(KEY_DELAY, 3)),
        c("access-key-underlines", "Toggle Access Key Underlines", "Always underline the letters of menu shortcuts", &["toggle access key underlines", "underline access keys", "keyboard cues"], "type", spi(SPI_GETKEYBOARDCUES, SPI_SETKEYBOARDCUES, true)),
        c("character-map", "Character Map", "Special characters", &["charmap", "symbols", "special characters"], "languages", Open("charmap.exe")),
        c("keyboard-properties", "Keyboard Properties", "Repeat rate", &["repeat delay"], "keyboard", Program("control.exe", "keyboard")),
    ]),

    ("Mouse", &[
        c("mouse-faster", "Mouse Speed Up", "Pointer speed", &["mouse speed up", "faster mouse", "faster pointer", "increase mouse speed", "pointer speed up"], "mouse", Step(MOUSE_SPEED, 2)),
        c("mouse-slower", "Mouse Speed Down", "Pointer speed", &["mouse speed down", "slower mouse", "slower pointer", "decrease mouse speed", "pointer speed down"], "mouse", Step(MOUSE_SPEED, -2)),
        c("mouse-speed-default", "Mouse Speed Default", "Pointer speed", &["mouse speed default", "default mouse speed", "reset mouse speed"], "mouse", Number(MOUSE_SPEED, 10)),
        c("pointer-precision", "Toggle Enhance Pointer Precision", "Mouse acceleration", &["toggle enhance pointer precision", "enhance pointer precision", "toggle mouse acceleration", "mouse acceleration on", "mouse acceleration off"], "mouse-pointer-2", Switch(toggles::flip_pointer_precision)),
        c("mouse-left-handed", "Toggle Left-Handed Mouse", "Swap the primary and secondary buttons", &["toggle left-handed mouse", "left handed mouse", "swap mouse buttons"], "mouse", Ask("Swap the left and right mouse buttons?", toggles::flip_mouse_buttons)),
        c("scroll-lines-1", "Scroll 1 Line at a Time", "Mouse wheel", &["scroll 1 line at a time", "scroll one line"], "mouse", Number(SCROLL_LINES, 1)),
        c("scroll-lines-3", "Scroll 3 Lines at a Time", "Mouse wheel, as Windows comes", &["scroll 3 lines at a time", "default scroll speed"], "mouse", Number(SCROLL_LINES, 3)),
        c("scroll-lines-5", "Scroll 5 Lines at a Time", "Mouse wheel", &["scroll 5 lines at a time"], "mouse", Number(SCROLL_LINES, 5)),
        c("scroll-lines-10", "Scroll 10 Lines at a Time", "Mouse wheel", &["scroll 10 lines at a time"], "mouse", Number(SCROLL_LINES, 10)),
        c("hide-pointer-typing", "Toggle Hide Pointer While Typing", "Mouse pointer", &["toggle hide pointer while typing", "hide pointer while typing", "hide cursor while typing"], "mouse-pointer-2", spi(SPI_GETMOUSEVANISH, SPI_SETMOUSEVANISH, true)),
        c("locate-pointer", "Toggle Locate Pointer With Ctrl", "Ctrl shows where the pointer is", &["toggle locate pointer with ctrl", "locate pointer", "find pointer", "mouse sonar"], "locate", spi(SPI_GETMOUSESONAR, SPI_SETMOUSESONAR, true)),
        c("pointer-trails", "Toggle Pointer Trails", "Mouse pointer", &["toggle pointer trails", "pointer trails", "mouse trails"], "mouse-pointer-2", Switch(toggles::flip_pointer_trails)),
        c("snap-to-default-button", "Toggle Snap Pointer to Default Button", "In dialog boxes", &["toggle snap pointer to default button", "snap to default button"], "mouse-pointer-click", spi(SPI_GETSNAPTODEFBUTTON, SPI_SETSNAPTODEFBUTTON, false)),
        c("double-click-fast", "Double-Click Speed Fast", "Mouse buttons", &["double-click speed fast", "double click speed fast", "fast double click"], "mouse-pointer-click", SetOnly(SPI_SETDOUBLECLICKTIME, 300)),
        c("double-click-medium", "Double-Click Speed Medium", "Mouse buttons, as Windows comes", &["double-click speed medium", "double click speed medium"], "mouse-pointer-click", SetOnly(SPI_SETDOUBLECLICKTIME, 500)),
        c("double-click-slow", "Double-Click Speed Slow", "Mouse buttons", &["double-click speed slow", "double click speed slow", "slow double click"], "mouse-pointer-click", SetOnly(SPI_SETDOUBLECLICKTIME, 700)),
        c("mouse-properties", "Mouse Properties", "Buttons and pointers", &["main.cpl", "pointer"], "mouse", Open("main.cpl")),
        c("mouse-pointers", "Change Mouse Pointers", "Pointer scheme and shadow", &["mouse pointers", "cursor scheme", "pointer scheme", "cursors"], "mouse-pointer-2", Program("control.exe", "main.cpl,,1")),
    ]),

    ("Accessibility", &[
        c("sticky-keys", "Toggle Sticky Keys", "Press shortcut keys one at a time", &["toggle sticky keys", "sticky keys", "stickykeys"], "keyboard", Switch(toggles::flip_sticky_keys)),
        c("filter-keys", "Toggle Filter Keys", "Ignore brief or repeated keystrokes", &["toggle filter keys", "filter keys", "slow keys", "bounce keys"], "keyboard", Switch(flip_filter_keys)),
        c("toggle-keys", "Toggle Lock Key Sounds", "A tone for Caps, Num and Scroll Lock", &["toggle lock key sounds", "toggle keys", "lock key sounds", "caps lock sound"], "volume-2", Switch(toggles::flip_toggle_keys)),
        c("animations", "Toggle Animation Effects", "Animations in Windows", &["toggle animation effects", "toggle animations", "animations on", "animations off", "turn off animations", "reduce motion"], "zap", spi(SPI_GETCLIENTAREAANIMATION, SPI_SETCLIENTAREAANIMATION, true)),
        c("magnifier-zoom-in", "Magnifier Zoom In", "Starts Magnifier", &["magnifier zoom in", "zoom in", "zoom screen in"], "zoom-in", Press(&[VK_LWIN, VK_OEM_PLUS])),
        c("magnifier-zoom-out", "Magnifier Zoom Out", "Magnifier", &["magnifier zoom out", "zoom out", "zoom screen out"], "zoom-out", Press(&[VK_LWIN, VK_OEM_MINUS])),
        c("magnifier-close", "Close Magnifier", "Magnifier", &["close magnifier", "stop magnifier", "turn off magnifier"], "x", Press(&[VK_LWIN, VK_ESCAPE])),
        c("narrator-toggle", "Toggle Narrator", "The screen reader", &["toggle narrator", "narrator on", "narrator off", "start narrator", "stop narrator"], "speech", Press(&[VK_LWIN, VK_CONTROL, VK_RETURN])),
        n(Needs::Windows11, "live-captions", "Toggle Live Captions", "Captions for any audio", &["toggle live captions", "live captions", "captions on", "captions off"], "message-square", Press(&[VK_LWIN, VK_CONTROL, VK_L])),
        n(Needs::ColorFilterShortcut, "color-filters", "Toggle Color Filters", "Grayscale and color-blind filters", &["toggle color filters", "color filters on", "color filters off", "grayscale on", "grayscale off"], "contrast", Press(&[VK_LWIN, VK_CONTROL, VK_C])),
        c("on-screen-keyboard", "On-Screen Keyboard", "Type with the mouse", &["osk", "virtual keyboard"], "keyboard", Open("osk.exe")),
        c("magnifier-app", "Magnifier", "Zoom into the screen", &["magnify"], "zoom-in", Open("magnify.exe")),
        c("narrator-app", "Narrator", "Read the screen aloud", &["screen reader", "read aloud"], "speech", Open("narrator.exe")),
    ]),

    ("Privacy & Security", &[
        c("camera-access", "Toggle Camera for Apps", "Let apps use the camera", &["toggle camera for apps", "block camera", "allow camera", "camera off for apps", "camera on for apps"], "camera", Switch(flip_camera_access)),
        c("microphone-access", "Toggle Microphone for Apps", "Let apps use the microphone", &["toggle microphone for apps", "block microphone", "allow microphone", "microphone off for apps", "microphone on for apps"], "mic", Switch(flip_microphone_access)),
        c("location", "Toggle Location Services", "Let apps know where this PC is", &["toggle location services", "location on", "location off", "turn on location", "turn off location"], "locate", Switch(flip_location_access)),
        c("remember-recent", "Toggle Remember Recent Files", "In Start, File Explorer and jump lists", &["toggle remember recent files", "remember recent files", "track recent files"], "history", Reg(reg(ADVANCED, "Start_TrackDocs", 1, 0, true, Refresh::Shell))),
        c("clear-recent-files", "Clear Recent Files", "What Start and File Explorer list as recent", &["clear recent files", "clear recent", "delete recent files"], "history", Ask("Clear the list of recent files? The files themselves stay.", clear_recent_files)),
        c("windows-security", "Windows Security", "Virus and threat protection", &["defender", "antivirus"], "shield-check", Open("windowsdefender:")),
        c("virus-protection", "Virus & Threat Protection", "Scan for viruses", &["virus scan", "quick scan", "scan for viruses", "malware scan"], "shield-check", Open("windowsdefender://threat")),
        c("firewall-protection", "Firewall & Network Protection", "Windows Security", &["firewall protection", "network protection", "firewall status"], "brick-wall", Open("windowsdefender://network")),
        c("app-browser-control", "App & Browser Control", "SmartScreen and exploit protection", &["smartscreen", "exploit protection", "app and browser control"], "shield", Open("windowsdefender://appbrowser")),
        c("device-security", "Device Security", "Core isolation and the security chip", &["core isolation", "memory integrity", "device security"], "shield", Open("windowsdefender://devicesecurity")),
        c("windows-firewall", "Windows Firewall Advanced", "Inbound and outbound rules", &["wf", "firewall rules"], "brick-wall", Open("wf.msc")),
        c("firewall-control-panel", "Windows Defender Firewall", "Allow an app through the firewall", &["firewall.cpl", "allow app through firewall"], "brick-wall", Open("firewall.cpl")),
        c("credential-manager", "Credential Manager", "Saved passwords", &["passwords", "credentials"], "key-round", Program("control.exe", "/name Microsoft.CredentialManager")),
        c("certificates", "Manage User Certificates", "Your certificates", &["certmgr", "certificate"], "badge-check", Open("certmgr.msc")),
        c("security-policy", "Local Security Policy", "Security options", &["secpol"], "shield", Open("secpol.msc")),
        c("tpm", "TPM Management", "The security chip", &["tpm", "trusted platform module"], "cpu", Open("tpm.msc")),
    ]),

    ("Clipboard", &[
        c("clipboard-history", "Clipboard History", "What you copied", &["clipboard", "paste history"], "clipboard", Keys(&[VK_LWIN, VK_V])),
        c("clear-clipboard", "Clear Clipboard", "Empty what is copied now", &["empty clipboard"], "clipboard-x", Native(clear_clipboard)),
    ]),

    ("Screenshots", &[
        c("take-screenshot", "Take a Screenshot", "Snip part of the screen", &["screenshot", "snip", "screen capture", "print screen"], "scissors", Keys(&[VK_LWIN, VK_SHIFT, VK_S])),
        c("screenshot-full", "Screenshot Full Screen", "Saved to the Screenshots folder", &["screenshot full screen", "full screenshot", "save screenshot"], "camera", Keys(&[VK_LWIN, VK_SNAPSHOT])),
        c("screenshot-window", "Screenshot Window", "The window in front, copied", &["screenshot window", "window screenshot", "capture window"], "app-window", Window(screenshot_window)),
        n(Needs::Windows11, "record-screen", "Record Screen", "With Snipping Tool", &["record screen", "screen recording", "screen recorder"], "video", Keys(&[VK_LWIN, VK_SHIFT, VK_R])),
        c("folder-screenshots", "Open Screenshots", "Saved screenshots", &["screenshots folder"], "camera", Open("shell:Screenshots")),
        c("snipping-tool", "Snipping Tool", "Screenshots and recordings", &["snip", "screen recording"], "scissors", Open("ms-screenclip:")),
    ]),

    ("Gaming", &[
        n(Needs::Link("ms-gamebar:"), "game-bar", "Open Game Bar", "Xbox Game Bar", &["game bar", "xbox game bar", "gamebar"], "gamepad-2", Keys(&[VK_LWIN, VK_G])),
        n(Needs::Link("ms-gamebar:"), "record-that", "Record What Just Happened", "The last moments of a game", &["record what just happened", "record that", "clip that"], "video", Keys(&[VK_LWIN, VK_MENU, VK_G])),
        n(Needs::Link("ms-gamebar:"), "game-recording", "Start or Stop Game Recording", "With Game Bar", &["start game recording", "stop game recording", "record game"], "video", Keys(&[VK_LWIN, VK_MENU, VK_R])),
        n(Needs::Link("ms-gamebar:"), "game-screenshot", "Game Screenshot", "With Game Bar", &["game screenshot", "screenshot game"], "camera", Keys(&[VK_LWIN, VK_MENU, VK_SNAPSHOT])),
        c("game-mode", "Toggle Game Mode", "Windows puts the game first", &["toggle game mode", "game mode on", "game mode off"], "gamepad-2", Reg(reg(GAME_BAR, "AutoGameModeEnabled", 1, 0, true, Refresh::Nothing))),
        c("game-controllers", "Game Controllers", "Gamepads and joysticks", &["joy", "gamepad", "joystick"], "gamepad-2", Open("joy.cpl")),
    ]),

    ("Files", &[
        c("hidden-files", "Toggle Hidden Files", "In File Explorer", &["toggle hidden files", "show hidden files", "hide hidden files", "hidden files on", "hidden files off"], "folder-cog", Shell(ShellFlag::HiddenFiles)),
        c("file-extensions", "Toggle File Name Extensions", "In File Explorer", &["toggle file name extensions", "toggle file extensions", "show file extensions", "hide file extensions"], "file-text", Shell(ShellFlag::Extensions)),
        c("protected-files", "Toggle Protected System Files", "In File Explorer", &["toggle protected system files", "show protected system files", "hide protected system files"], "shield", Switch(flip_protected_files)),
        c("item-check-boxes", "Toggle Item Check Boxes", "Select files with check boxes", &["toggle item check boxes", "item check boxes", "file check boxes"], "square", Shell(ShellFlag::CheckBoxes)),
        c("compact-view", "Toggle Compact View", "Less space between files", &["toggle compact view", "compact view", "compact mode"], "layout-grid", Reg(reg(ADVANCED, "UseCompactMode", 1, 0, false, Refresh::Shell))),
        c("home-recent-files", "Toggle Recent Files in Home", "File Explorer's Home", &["toggle recent files in home", "recent files in home", "recent files in quick access"], "history", Reg(reg(EXPLORER, "ShowRecent", 1, 0, true, Refresh::Shell))),
        c("home-frequent-folders", "Toggle Frequent Folders in Home", "File Explorer's Home", &["toggle frequent folders in home", "frequent folders in home", "frequent folders in quick access"], "folder-clock", Reg(reg(EXPLORER, "ShowFrequent", 1, 0, true, Refresh::Shell))),
        c("explorer-opens-this-pc", "File Explorer Opens to This PC", "Where new windows start", &["file explorer opens to this pc", "open explorer to this pc"], "hard-drive", Dword(ADVANCED, "LaunchTo", 1, Refresh::Nothing)),
        c("explorer-opens-home", "File Explorer Opens to Home", "Where new windows start", &["file explorer opens to home", "open explorer to home", "open explorer to quick access"], "house", Dword(ADVANCED, "LaunchTo", 2, Refresh::Nothing)),
        c("restart-explorer", "Restart File Explorer", "When the taskbar or Explorer is stuck", &["restart file explorer", "restart explorer", "restart taskbar"], "rotate-cw", Ask("Restart File Explorer? Folder windows close and the taskbar goes away for a moment.", toggles::restart_explorer)),
        c("folder-options", "File Explorer Options", "Hidden files and extensions", &["folder options", "file extensions"], "folder-cog", Program("control.exe", "folders")),
        c("indexing-options", "Indexing Options", "Windows Search index", &["search index"], "search", Program("control.exe", "srchadmin.dll")),
        c("file-history", "File History", "Back up your files", &["backup"], "archive-restore", Program("control.exe", "/name Microsoft.FileHistory")),
        c("backup-and-restore", "Backup and Restore", "Windows 7 style backups", &["backup", "system image"], "archive", Program("control.exe", "/name Microsoft.BackupAndRestore")),
        c("sync-center", "Sync Center", "Offline files", &["offline files"], "refresh-cw", Program("control.exe", "/name Microsoft.SyncCenter")),
    ]),

    ("Folders", &[
        c("folder-home", "Open Home Folder", "Your user folder", &["home", "profile", "user folder"], "house", Open("shell:Profile")),
        c("folder-desktop", "Open Desktop Folder", "Files on your desktop", &["desktop folder"], "monitor", Open("shell:Desktop")),
        c("folder-documents", "Open Documents", "Documents folder", &["documents", "docs"], "file-text", Open("shell:Personal")),
        c("folder-downloads", "Open Downloads", "Downloads folder", &["downloads"], "download", Open("shell:Downloads")),
        c("folder-pictures", "Open Pictures", "Pictures folder", &["pictures", "photos", "images"], "image", Open("shell:My Pictures")),
        c("folder-camera-roll", "Open Camera Roll", "Photos from the camera", &["camera roll"], "images", Open("shell:Camera Roll")),
        c("folder-music", "Open Music", "Music folder", &["music", "songs"], "music", Open("shell:My Music")),
        c("folder-videos", "Open Videos", "Videos folder", &["videos", "movies"], "video", Open("shell:My Video")),
        n(Needs::OneDrive, "folder-onedrive", "Open OneDrive", "OneDrive folder", &["onedrive", "cloud"], "cloud", Open("shell:OneDrive")),
        c("folder-public", "Open Public Folder", "Shared by everyone on this PC", &["public folder"], "users", Open("shell:Public")),
        c("folder-templates", "Open Templates", "Templates for new files", &["templates"], "file-text", Open("shell:Templates")),
        c("folder-saved-games", "Open Saved Games", "Where games keep their saves", &["saved games", "game saves"], "gamepad-2", Open("shell:SavedGames")),
        c("folder-libraries", "Open Libraries", "Documents, Music, Pictures and Videos", &["libraries"], "archive", Open("shell:Libraries")),
        c("folder-this-pc", "Open This PC", "Drives and devices", &["this pc", "my computer", "drives"], "hard-drive", Open("shell:MyComputerFolder")),
        c("folder-network", "Open Network", "Computers on your network", &["network folder", "shared computers"], "network", Open("shell:NetworkPlacesFolder")),
        c("folder-recent", "Open Recent Files", "What you opened lately", &["recent", "recent files"], "history", Open("shell:Recent")),
        c("folder-appdata", "Open AppData", "Roaming app data", &["appdata", "roaming"], "folder-cog", Open("shell:AppData")),
        c("folder-local-appdata", "Open Local AppData", "Local app data", &["local appdata"], "folder-cog", Open("shell:Local AppData")),
        c("folder-temp", "Open Temp Folder", "Temporary files", &["temp", "tmp", "temporary files"], "folder-clock", Native(open_temp)),
        c("folder-startup", "Open Startup Folder", "Apps started at sign-in", &["startup folder", "autostart"], "rocket", Open("shell:Startup")),
        c("folder-start-menu", "Open Start Menu Folder", "The shortcuts Start lists", &["start menu folder", "programs folder", "start menu shortcuts"], "layout-grid", Open("shell:Programs")),
        c("folder-send-to", "Open Send To Folder", "The Send to menu", &["send to", "sendto"], "folder-cog", Open("shell:SendTo")),
        c("folder-fonts", "Open Fonts Folder", "Installed fonts", &["fonts folder"], "type", Open("shell:Fonts")),
        c("folder-windows-tools", "Open Windows Tools", "Administrative tools", &["windows tools", "administrative tools", "admin tools"], "wrench", Open("shell:Common Administrative Tools")),
        c("folder-all-tasks", "Open All Control Panel Tasks", "Every Control Panel task in one list", &["god mode", "all tasks", "all control panel items"], "sliders-horizontal", Open("shell:::{ED7BA470-8E54-465E-825C-99712043E01C}")),
        c("folder-printers", "Open Printers Folder", "Printers and faxes", &["printers folder"], "printer", Open("shell:PrintersFolder")),
        c("folder-program-files", "Open Program Files", "Installed programs", &["program files"], "package", Open("shell:ProgramFiles")),
        c("folder-program-files-x86", "Open Program Files (x86)", "32-bit programs", &["program files x86"], "package", Open("shell:ProgramFilesX86")),
        c("folder-windows", "Open Windows Folder", "Windows itself", &["windows folder", "windir"], "folder-cog", Open("shell:Windows")),
        c("folder-system32", "Open System32", "Windows system files", &["system32"], "folder-cog", Open("shell:System")),
        c("folder-hosts", "Open Hosts File Folder", "drivers › etc", &["hosts", "hosts file", "etc folder"], "file-cog", Native(open_hosts_folder)),
        c("folder-wallpapers", "Open Windows Wallpapers", "The pictures Windows comes with", &["windows wallpapers", "wallpapers folder", "default wallpapers"], "image", Native(open_wallpapers)),
    ]),

    ("Cleanup", &[
        c("open-recycle-bin", "Open Recycle Bin", "Deleted files", &["trash", "recycle bin", "bin", "deleted files"], "trash", Open("shell:RecycleBinFolder")),
        c("empty-recycle-bin", "Empty Recycle Bin", "Permanently delete what is in it", &["empty trash", "empty bin", "clear recycle bin"], "trash-2", Ask("Permanently delete everything in the Recycle Bin?", empty_recycle_bin)),
        c("delete-temp-files", "Delete Old Temporary Files", "Untouched for a day; files in use stay", &["delete old temporary files", "delete temporary files", "delete temp files", "clear temp", "clean temp"], "brush-cleaning", Ask("Delete the temporary files apps have not touched for a day?", toggles::delete_old_temp_files)),
        c("disk-cleanup", "Disk Cleanup", "Free up disk space", &["cleanmgr", "clean disk"], "brush-cleaning", Open("cleanmgr.exe")),
    ]),

    ("Updates", &[
        c("check-updates", "Check for Windows Updates", "Starts a check in Windows Update", &["check for updates", "update windows", "windows update check"], "refresh-cw", Open("ms-settings:windowsupdate-action")),
        c("store-updates", "Microsoft Store Updates", "Update your Store apps", &["store updates", "update store apps"], "store", Open("ms-windows-store://downloadsandupdates")),
        n(Needs::Link("winget.exe"), "winget-list", "List App Updates", "With winget, in a terminal", &["list app updates", "winget upgrade", "outdated apps", "winget"], "terminal", Program("cmd.exe", "/k winget upgrade")),
        n(Needs::Link("winget.exe"), "winget-upgrade-all", "Update All Apps", "With winget, in a terminal", &["update all apps", "upgrade all apps", "winget upgrade all"], "download", Ask("Update every app winget can? A terminal shows the progress, and some installers ask for permission.", winget_upgrade_all)),
    ]),

    ("Date & Time", &[
        n(Needs::Windows11, "show-calendar", "Show Calendar", "The date and calendar flyout", &["show calendar", "calendar", "date flyout"], "calendar-days", Keys(&[VK_LWIN, VK_MENU, VK_D])),
        n(Needs::Windows11, "clock-seconds", "Toggle Seconds in Clock", "On the taskbar", &["toggle seconds in clock", "seconds in clock", "show seconds in clock"], "clock", Reg(reg(ADVANCED, "ShowSecondsInSystemClock", 1, 0, false, Refresh::Shell))),
        c("date-and-time", "Date and Time Properties", "Clock and time zone", &["timedate", "clock"], "clock", Open("timedate.cpl")),
        c("additional-clocks", "Additional Clocks", "Clocks for other time zones", &["additional clocks", "world clock", "second clock"], "clock", Program("control.exe", "timedate.cpl,,1")),
    ]),

    ("Language & Region", &[
        c("region-properties", "Region Properties", "Date and number formats", &["intl", "locale"], "languages", Open("intl.cpl")),
    ]),

    ("Devices", &[
        c("devices-and-printers", "Devices and Printers", "Connected devices", &["printers"], "printer", Program("control.exe", "printers")),
        c("print-management", "Print Management", "Printers and drivers", &["printmanagement"], "printer", Open("printmanagement.msc")),
    ]),

    ("Accounts", &[
        c("user-accounts", "User Accounts", "Manage who signs in", &["netplwiz", "auto login"], "user-cog", Open("netplwiz.exe")),
        c("users-and-groups", "Local Users and Groups", "Accounts on this PC", &["lusrmgr"], "users", Open("lusrmgr.msc")),
    ]),

    ("Applications", &[
        c("programs-and-features", "Programs and Features", "Uninstall programs", &["appwiz", "uninstall", "add or remove programs"], "package-x", Open("appwiz.cpl")),
    ]),

    ("System", &[
        c("control-panel", "Control Panel", "Classic Windows settings", &["control"], "sliders-horizontal", Open("control.exe")),
        c("windows-version", "About Windows", "Version and build", &["winver", "windows version", "build number"], "info", Open("winver.exe")),
        c("system-properties", "System Properties", "Computer name and hardware", &["sysdm"], "laptop", Open("sysdm.cpl")),
        c("advanced-system-settings", "Advanced System Settings", "Performance and startup", &["system properties advanced"], "settings-2", Open("SystemPropertiesAdvanced.exe")),
        c("performance-options", "Performance Options", "Visual effects and virtual memory", &["visual effects", "virtual memory", "page file"], "gauge", Open("SystemPropertiesPerformance.exe")),
        c("system-protection", "System Protection", "Restore points", &["restore settings"], "shield", Open("SystemPropertiesProtection.exe")),
        c("remote-settings", "Remote Settings", "Allow remote connections", &["system properties remote"], "screen-share", Open("SystemPropertiesRemote.exe")),
        c("computer-name", "Change Computer Name", "Name and workgroup", &["rename pc", "workgroup", "domain"], "pencil", Open("SystemPropertiesComputerName.exe")),
        c("environment-variables", "Environment Variables", "PATH and other variables", &["env", "path", "system variables"], "braces", Program("rundll32.exe", "sysdm.cpl,EditEnvironmentVariables")),
        c("windows-features", "Turn Windows Features On or Off", "Optional components", &["optionalfeatures", "hyper-v", "wsl"], "puzzle", Open("optionalfeatures.exe")),
        c("system-restore", "System Restore", "Go back to a restore point", &["rstrui", "restore point"], "history", Open("rstrui.exe")),
        c("recovery-drive", "Create a Recovery Drive", "A USB drive to repair this PC", &["recoverydrive", "usb recovery"], "usb", Open("RecoveryDrive.exe")),
    ]),

    ("System Tools", &[
        c("task-manager", "Task Manager", "Running apps and performance", &["taskmgr", "processes", "end task"], "activity", Open("taskmgr.exe")),
        c("resource-monitor", "Resource Monitor", "CPU, memory, disk and network", &["resmon"], "gauge", Open("resmon.exe")),
        c("performance-monitor", "Performance Monitor", "Detailed counters", &["perfmon"], "chart-line", Open("perfmon.msc")),
        c("reliability-monitor", "Reliability Monitor", "Crashes and problems over time", &["reliability", "crash history"], "chart-line", Program("perfmon.exe", "/rel")),
        c("event-viewer", "Event Viewer", "System and app logs", &["eventvwr", "logs", "errors"], "scroll-text", Open("eventvwr.msc")),
        c("system-information", "System Information", "Hardware and software details", &["msinfo", "msinfo32", "specs"], "info", Open("msinfo32.exe")),
        c("directx-diagnostic", "DirectX Diagnostic Tool", "Graphics and sound details", &["dxdiag", "directx"], "cpu", Open("dxdiag.exe")),
        c("memory-diagnostic", "Windows Memory Diagnostic", "Check the RAM for errors", &["mdsched", "ram test"], "memory-stick", Open("mdsched.exe")),
        c("computer-management", "Computer Management", "Disks, services and users", &["compmgmt"], "server", Open("compmgmt.msc")),
        c("device-manager", "Device Manager", "Hardware and drivers", &["devmgmt", "drivers", "hardware"], "cpu", Open("devmgmt.msc")),
        c("disk-management", "Disk Management", "Partitions and volumes", &["diskmgmt", "partition", "format drive"], "hard-drive", Open("diskmgmt.msc")),
        c("optimize-drives", "Defragment and Optimize Drives", "Keep drives fast", &["defrag", "dfrgui", "trim"], "hard-drive", Open("dfrgui.exe")),
        c("services", "Services", "Background services", &["services.msc"], "cog", Open("services.msc")),
        c("task-scheduler", "Task Scheduler", "Scheduled tasks", &["taskschd", "cron", "schedule"], "calendar-clock", Open("taskschd.msc")),
        c("registry-editor", "Registry Editor", "Windows registry", &["regedit", "registry"], "database", Open("regedit.exe")),
        c("system-configuration", "System Configuration", "Boot and startup options", &["msconfig", "boot options"], "wrench", Open("msconfig.exe")),
        c("group-policy", "Group Policy Editor", "Local policies", &["gpedit", "policy"], "file-cog", Open("gpedit.msc")),
        c("shared-folders", "Shared Folders", "What this PC shares", &["fsmgmt", "shares"], "folder-cog", Open("fsmgmt.msc")),
        c("component-services", "Component Services", "COM and DCOM", &["comexp", "dcom"], "boxes", Open("comexp.msc")),
        c("odbc-sources", "ODBC Data Sources", "Database connections", &["odbc", "odbcad32"], "database", Open("odbcad32.exe")),
    ]),

    ("Built-in Apps", &[
        c("command-prompt", "Command Prompt", "cmd", &["cmd", "dos"], "terminal", Open("cmd.exe")),
        c("windows-powershell", "Windows PowerShell", "powershell", &["powershell", "ps"], "square-terminal", Open("powershell.exe")),
        c("notepad", "Notepad", "Plain text editor", &["text editor", "txt"], "notebook-pen", Open("notepad.exe")),
        c("paint", "Paint", "Draw and edit pictures", &["mspaint", "draw"], "paintbrush", Open("ms-paint:")),
        c("clock", "Clock", "Alarms, timers and focus", &["alarm", "timer", "stopwatch"], "alarm-clock", Open("ms-clock:")),
        c("camera-app", "Camera", "Photos and video", &["webcam"], "camera", Open("microsoft.windows.camera:")),
        c("photos", "Photos", "Your pictures", &["gallery"], "image", Open("ms-photos:")),
        c("media-player", "Media Player", "Music and videos", &["media player", "music player", "video player"], "play", Open("mswindowsmusic:")),
        c("microsoft-store", "Microsoft Store", "Get apps", &["store", "install apps"], "store", Open("ms-windows-store:")),
        c("phone-link", "Phone Link", "Your phone on this PC", &["phone", "android"], "smartphone", Open("ms-phone:")),
        c("weather", "Weather", "The forecast", &["forecast"], "cloud-sun", Open("bingweather:")),
        c("get-help", "Get Help", "Microsoft support", &["support", "help"], "circle-help", Open("ms-contact-support:")),
        c("feedback-hub", "Feedback Hub", "Report a Windows problem", &["feedback"], "message-square", Open("feedback-hub:")),
    ]),
];

fn commands() -> impl Iterator<Item = &'static SystemCommand> {
    SECTIONS.iter().flat_map(|(_, commands)| commands.iter())
}

pub fn all() -> impl Iterator<Item = &'static QuickAction> {
    commands().map(|c| &c.action)
}

fn find(id: &str) -> Option<&'static SystemCommand> {
    commands().find(|c| c.action.id == id)
}

/// Its section of [`SECTIONS`], which the Shortcut page filters by.
pub fn category(id: &str) -> Option<&'static str> {
    SECTIONS.iter().find(|(_, commands)| commands.iter().any(|c| c.action.id == id)).map(|(name, _)| *name)
}

/// Whether Spotlight lists it only when one of its names is typed.
pub fn needs_its_name(id: &str) -> bool {
    find(id).is_some_and(|c| c.run.changes_a_setting())
}

/// Where Windows keeps its programs, and Store apps their command names.
fn program_dirs() -> Vec<PathBuf> {
    let root = windows_folder();
    let mut dirs = vec![root.join("System32"), root.join(r"System32\WindowsPowerShell\v1.0"), root.clone()];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join(r"Microsoft\WindowsApps"));
    }
    dirs
}

fn windows_folder() -> PathBuf {
    std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
}

/// A link (`ms-settings:…`, `windowsdefender://…`) rather than a program or a `shell:` folder.
fn is_link(target: &str) -> bool {
    !target.starts_with("shell:") && target.split_once(':').is_some_and(|(scheme, _)| scheme.len() > 1)
}

/// Whether Windows knows this link, or has this program.
fn target_exists(target: &str) -> bool {
    if target.starts_with("shell:") {
        return true;
    }
    if is_link(target) {
        let scheme = target.split_once(':').map_or(target, |(scheme, _)| scheme);
        return registry::read_string(HKEY_CLASSES_ROOT, scheme, "URL Protocol").is_some();
    }
    program_dirs().iter().any(|dir| dir.join(target).exists())
}

fn has(needs: Needs) -> bool {
    match needs {
        Needs::Battery => toggles::has_battery(),
        Needs::Windows11 => super::is_windows_11(),
        Needs::Microphone => toggles::has_microphone(),
        // turned off with `powercfg /hibernate off` on many PCs
        Needs::Hibernate => {
            registry::read_dword(HKEY_LOCAL_MACHINE, r"SYSTEM\CurrentControlSet\Control\Power", "HibernateEnabled").unwrap_or(1) != 0
        }
        // its folder only opens once OneDrive is set up, whatever the environment says
        Needs::OneDrive => registry::read_string(HKEY_CURRENT_USER, r"Software\Microsoft\OneDrive", "UserFolder").is_some(),
        // Win+Ctrl+C does nothing until Settings turns the shortcut on
        Needs::ColorFilterShortcut => registry::read_dword(HKEY_CURRENT_USER, COLOR_FILTERS, "HotkeyEnabled") == Some(1),
        Needs::Link(target) => target_exists(target),
    }
}

pub fn is_available(id: &str) -> bool {
    let Some(command) = find(id) else { return false };
    if !command.needs.is_none_or(has) {
        return false;
    }
    match command.run {
        Open(target) | Program(target, _) => target_exists(target),
        Volume(_) => toggles::has_speakers(),
        Microphone(_) => toggles::has_microphone(),
        Radio(radio) => toggles::has_radio(radio),
        Plan(plan) => toggles::has_power_plan(plan),
        Mode(_) => toggles::has_power_modes(),
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
        Keys(keys) | Press(keys) => {
            after_keys_up(move || press(keys));
            Ok(())
        }
        Native(action) | Switch(action) => action(),
        Ask(question, action) if ask(question) => action(),
        Ask(..) => Ok(()),
        Reg(flag) => flag.flip(),
        Dword(key, name, value, refresh) => toggles::set_dword(key, name, value, refresh),
        Spi(flag) => flag.flip(),
        Step(number, by) => number.step(by),
        Number(number, value) => number.set(value),
        SetOnly(action, value) => toggles::spi_set_only(action, value),
        Shell(flag) => {
            flag.set(!flag.is_on());
            Ok(())
        }
        Volume(level) => toggles::set_volume(level),
        Microphone(level) => toggles::set_microphone(level),
        Radio(radio) => toggles::flip_radio(radio),
        Timeout(setting, minutes) => toggles::set_timeout(setting, minutes),
        Lid(plugged_in, action) => toggles::set_lid_action(plugged_in, action),
        Plan(plan) => toggles::set_power_plan(plan),
        Mode(overlay) => toggles::set_power_mode(overlay),
        Window(action) => {
            after_keys_up(move || match toggles::window_in_front() {
                Some(window) => {
                    if let Err(e) = action(window) {
                        log::warn!("could not change the window: {e}");
                    }
                }
                None => log::info!("no window to act on"),
            });
            Ok(())
        }
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

/// Keys that sit apart from the numeric keypad, sent as such.
const EXTENDED: &[VIRTUAL_KEY] = &[VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN, VK_LWIN, VK_RWIN, VK_SNAPSHOT];

fn key_input(key: VIRTUAL_KEY, up: bool) -> INPUT {
    let mut flags = if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
    if EXTENDED.contains(&key) {
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

/// Disconnecting the session at the console shows the sign-in screen, with every app still running.
fn switch_user() -> Result<(), String> {
    after_keys_up(|| {
        if let Err(e) = quiet("tsdiscon.exe", &[]) {
            log::warn!("could not switch user: {e}");
        }
    });
    Ok(())
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

fn open_hosts_folder() -> Result<(), String> {
    shell_open(&windows_folder().join(r"System32\drivers\etc").to_string_lossy(), None)
}

fn open_wallpapers() -> Result<(), String> {
    shell_open(&windows_folder().join(r"Web\Wallpaper").to_string_lossy(), None)
}

fn set_dark_mode(dark: bool) -> Result<(), String> {
    APPS_DARK.set(dark)?;
    WINDOWS_DARK.set(dark)
}

/// By the apps' mode, so a split setting becomes one.
fn toggle_dark_mode() -> Result<(), String> {
    set_dark_mode(!APPS_DARK.is_on())
}

fn use_dark_mode() -> Result<(), String> {
    set_dark_mode(true)
}

fn use_light_mode() -> Result<(), String> {
    set_dark_mode(false)
}

fn microphone_up() -> Result<(), String> {
    toggles::step_microphone(0.1)
}

fn microphone_down() -> Result<(), String> {
    toggles::step_microphone(-0.1)
}

/// Filter Keys makes short presses go unnoticed, so turning it on asks first.
fn flip_filter_keys() -> Result<(), String> {
    if !toggles::filter_keys_on()? && !ask("Turn on Filter Keys? Brief and repeated keystrokes will be ignored.") {
        return Ok(());
    }
    toggles::flip_filter_keys()
}

fn flip_protected_files() -> Result<(), String> {
    let flag = ShellFlag::ProtectedFiles;
    let showing = flag.is_on();
    // the question File Explorer asks too
    if !showing && !ask("Show protected operating system files? Deleting or editing them can make Windows unusable.") {
        return Ok(());
    }
    flag.set(!showing);
    Ok(())
}

fn flip_camera_access() -> Result<(), String> {
    toggles::flip_app_access("webcam")
}

fn flip_microphone_access() -> Result<(), String> {
    toggles::flip_app_access("microphone")
}

fn flip_location_access() -> Result<(), String> {
    toggles::flip_app_access("location")
}

fn clear_recent_files() -> Result<(), String> {
    toggles::clear_recent_files();
    Ok(())
}

fn winget_upgrade_all() -> Result<(), String> {
    shell_run("cmd.exe", "/k winget upgrade --all")
}

fn snap_left(_: HWND) -> Result<(), String> {
    press(&[VK_LWIN, VK_LEFT]);
    Ok(())
}

fn snap_right(_: HWND) -> Result<(), String> {
    press(&[VK_LWIN, VK_RIGHT]);
    Ok(())
}

fn next_display(_: HWND) -> Result<(), String> {
    press(&[VK_LWIN, VK_SHIFT, VK_RIGHT]);
    Ok(())
}

fn previous_display(_: HWND) -> Result<(), String> {
    press(&[VK_LWIN, VK_SHIFT, VK_LEFT]);
    Ok(())
}

fn snap_layouts(_: HWND) -> Result<(), String> {
    press(&[VK_LWIN, VK_Z]);
    Ok(())
}

/// Alt+Print Screen copies the window in front.
fn screenshot_window(_: HWND) -> Result<(), String> {
    press(&[VK_MENU, VK_SNAPSHOT]);
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
        assert_eq!(ids.len(), commands().count(), "ids are unique");
        assert!(ACTIONS.iter().all(|a| !ids.contains(a.id)), "and differ from the built-in actions");
        let categories: HashSet<_> = SECTIONS.iter().map(|(name, _)| *name).collect();
        assert_eq!(categories.len(), SECTIONS.len(), "categories are unique");
        for (name, commands) in SECTIONS {
            assert!(!commands.is_empty(), "{name} has commands");
        }
        for command in commands() {
            let a = &command.action;
            assert!(!a.title.is_empty() && !a.subtitle.is_empty() && !a.keywords.is_empty(), "{}", a.id);
            assert!(a.keywords.iter().all(|k| *k == k.to_lowercase()), "{}: keywords are lowercase", a.id);
            assert!(category(a.id).is_some(), "{}", a.id);
        }
    }

    #[test]
    fn what_can_lose_work_asks_first() {
        for id in [
            "shut-down", "restart", "log-out", "empty-recycle-bin", "restart-explorer", "delete-temp-files",
            "clear-recent-files", "winget-upgrade-all", "mouse-left-handed",
        ] {
            assert!(matches!(find(id).unwrap().run, Ask(..)), "{id}");
        }
    }

    #[test]
    fn what_changes_a_setting_needs_its_name() {
        for id in ["wifi-toggle", "bluetooth-toggle", "keep-awake", "volume-50", "mic-mute", "hidden-files", "taskbar-left",
            "toggle-dark-mode", "window-close", "toggle-hdr", "mouse-faster", "power-plan-balanced", "screen-off-plugged-5"] {
            assert!(needs_its_name(id), "{id}");
        }
        for id in ["task-manager", "lock-screen", "emoji-picker", "folder-downloads", "shut-down", "volume-up"] {
            assert!(!needs_its_name(id), "{id}");
        }
    }

    #[test]
    fn levels_are_fractions() {
        for command in commands() {
            if let Volume(level) | Microphone(level) = command.run {
                assert!(level > 0.0 && level <= 1.0, "{}", command.action.id);
            }
        }
    }

    #[test]
    fn programs_are_plain_names_of_windows_programs() {
        for command in commands() {
            if let Open(target) | Program(target, _) = command.run {
                if target.starts_with("shell:") || is_link(target) {
                    continue;
                }
                assert!(!target.contains(['\\', '/', '"', ' ', ':']), "{target}");
            }
        }
    }

    #[test]
    fn what_windows_lacks_is_not_offered() {
        assert!(target_exists("cmd.exe") && target_exists("ms-settings:bluetooth") && target_exists("windowsdefender://threat"));
        assert!(!target_exists("no-such-program.exe") && !target_exists("no-such-link:"));
        assert!(!is_link("shell:Downloads") && !is_link("cmd.exe") && is_link("ms-settings:display"));
    }

    #[test]
    #[ignore = "reads what this PC has; runs nothing"]
    fn lists_what_this_pc_lacks() {
        let missing: Vec<_> = all().filter(|a| !is_available(a.id)).map(|a| a.id).collect();
        println!("{} of {} available, missing: {missing:?}", commands().count() - missing.len(), commands().count());
    }
}
