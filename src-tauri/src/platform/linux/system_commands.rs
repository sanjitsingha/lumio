//! The System commands of Spotlight and the Shortcut page on elementary OS.
//! Each one does the thing itself rather than opening a settings page: it
//! writes what the system's own switch writes, or calls the service behind it.
//!
//! To add one, add an entry to [`COMMANDS`]; its icon must be in `SYMBOLS`
//! (`ResultIcon.jsx`). Anything that can lose work, or leave you unable to
//! use the keyboard or pointer, asks first ([`Run::Ask`], [`Run::AskFlip`]).
//! A command this computer cannot run (no battery, no keyboard backlight, a
//! setting this version lacks) is not offered ([`Needs`]).

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib::{self, ToVariant};

use super::toggles::{flip_key, get, get_bool, required, set, settings, show, system_bus};
use super::gala;
use crate::search::actions::QuickAction;

/// How long Spotlight gets to close and hand focus back before a command
/// that acts on the focused window, the screen or the session runs.
const SPOTLIGHT_GONE: Duration = Duration::from_millis(300);
const CALL_TIMEOUT_MS: i32 = 5000;
/// What this computer has is read at most this often.
const CAPABILITIES_FOR: Duration = Duration::from_secs(10);

const INTERFACE: &str = "org.gnome.desktop.interface";
const SOUND: &str = "org.gnome.desktop.sound";
const WM_PREFS: &str = "org.gnome.desktop.wm.preferences";
const MUTTER: &str = "org.gnome.mutter";
/// The group of the commands that snap, move and center the window in use.
pub const WINDOW_MANAGER: &str = "Window Manager";
const MUTTER_KEYS: &str = "org.gnome.mutter.keybindings";
const WM_KEYS: &str = "org.gnome.desktop.wm.keybindings";
const KEYBOARD: &str = "org.gnome.desktop.peripherals.keyboard";
const MOUSE: &str = "org.gnome.desktop.peripherals.mouse";
const TOUCHPAD: &str = "org.gnome.desktop.peripherals.touchpad";
const A11Y_KEYS: &str = "org.gnome.desktop.a11y.keyboard";
const A11Y_APPS: &str = "org.gnome.desktop.a11y.applications";
const PRIVACY: &str = "org.gnome.desktop.privacy";
const SESSION: &str = "org.gnome.desktop.session";
const BACKGROUND: &str = "org.gnome.desktop.background";
const SCREENSAVER: &str = "org.gnome.desktop.screensaver";
const DATETIME: &str = "org.gnome.desktop.datetime";
const INPUT_SOURCES: &str = "org.gnome.desktop.input-sources";
const LOCATION: &str = "org.gnome.system.location";
const PROXY: &str = "org.gnome.system.proxy";
const POWER: &str = "org.gnome.settings-daemon.plugins.power";
const COLOR: &str = "org.gnome.settings-daemon.plugins.color";
const GALA: &str = "io.elementary.desktop.wm.behavior";
const GALA_A11Y: &str = "io.elementary.desktop.wm.accessibility";
const DOCK: &str = "io.elementary.dock";
const PANEL: &str = "io.elementary.desktop.wingpanel";
const CLOCK: &str = "io.elementary.desktop.wingpanel.datetime";
const BATTERY_PANEL: &str = "io.elementary.desktop.wingpanel.power";
const PANEL_KEYS: &str = "io.elementary.wingpanel.keyboard";
const QUICK_SETTINGS: &str = "io.elementary.desktop.quick-settings";
const DIM: &str = "io.elementary.desktop.background";
const LOCK_PREFS: &str = "io.elementary.desktop.screensaver";
const SCHEME: &str = "io.elementary.settings-daemon.prefers-color-scheme";
const HOUSEKEEPING: &str = "io.elementary.settings-daemon.housekeeping";
const SYSTEM_UPDATE: &str = "io.elementary.settings-daemon.system-update";
const APPCENTER: &str = "io.elementary.appcenter.settings";
const NOTIFICATIONS: &str = "io.elementary.notifications";
const BLUETOOTH_PREFS: &str = "io.elementary.desktop.bluetooth";
const FILES: &str = "io.elementary.files.preferences";

const SPEAKERS: &str = "@DEFAULT_AUDIO_SINK@";
const MICROPHONE: &str = "@DEFAULT_AUDIO_SOURCE@";

const SESSION_MANAGER: &str = "org.gnome.SessionManager";
const SESSION_MANAGER_PATH: &str = "/org/gnome/SessionManager";
const LOGIN: &str = "org.freedesktop.login1";
const LOGIN_PATH: &str = "/org/freedesktop/login1";
const LOGIN_MANAGER: &str = "org.freedesktop.login1.Manager";
const GSD_POWER: &str = "org.gnome.SettingsDaemon.Power";
const GSD_POWER_PATH: &str = "/org/gnome/SettingsDaemon/Power";
const GSD_SCREEN: &str = "org.gnome.SettingsDaemon.Power.Screen";
const GSD_KEYBOARD: &str = "org.gnome.SettingsDaemon.Power.Keyboard";
const GSD_COLOR: &str = "org.gnome.SettingsDaemon.Color";
const GSD_COLOR_PATH: &str = "/org/gnome/SettingsDaemon/Color";
const NETWORK_MANAGER: &str = "org.freedesktop.NetworkManager";
const NETWORK_MANAGER_PATH: &str = "/org/freedesktop/NetworkManager";
const POWER_PROFILES: &str = "org.freedesktop.UPower.PowerProfiles";
const POWER_PROFILES_PATH: &str = "/org/freedesktop/UPower/PowerProfiles";
const BLUEZ: &str = "org.bluez";
const BLUEZ_ADAPTER: &str = "org.bluez.Adapter1";
const BLUEZ_DEVICE: &str = "org.bluez.Device1";
const TIMEDATE: &str = "org.freedesktop.timedate1";
const TIMEDATE_PATH: &str = "/org/freedesktop/timedate1";
const ACCOUNTS: &str = "org.freedesktop.Accounts";
const PANTHEON_ACCOUNT: &str = "io.elementary.pantheon.AccountsService";
const GALA_BUS: &str = "org.pantheon.gala";
const REMOTE_DESKTOP: &str = "org.gnome.Mutter.RemoteDesktop";
const IDLE_MONITOR: &str = "org.gnome.Mutter.IdleMonitor";
const GALA_DESKTOP_PATH: &str = "/org/pantheon/gala/DesktopInterface";
const GALA_DESKTOP: &str = "org.pantheon.gala.DesktopIntegration";
const WINGPANEL: &str = "org.elementary.wingpanel";
const WINGPANEL_PATH: &str = "/org/elementary/wingpanel";
const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const MPRIS_PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SCREENSHOT_TOOL: &str = "io.elementary.screenshot.desktop";

/// Shown on the desktop's on-screen display: how full its bar is, if at all.
type Done = Result<Option<f64>, String>;

/// A value for a setting; numbers are written as the key's own type.
#[derive(Clone, Copy)]
enum Value {
    T(&'static str),
    V(f64),
    B(bool),
}
use Value::{B, T, V};

#[derive(Clone, Copy)]
enum Place {
    /// `None` is the home folder.
    Special(Option<glib::UserDirectory>),
    InHome(&'static str),
    InPictures(&'static str),
    Uri(&'static str),
}
use Place::{InHome, InPictures, Special, Uri};

#[derive(Clone, Copy)]
enum Run {
    /// A boolean setting.
    Flip(&'static str, &'static str),
    /// A boolean setting that means "off" when true (`disable-camera`).
    FlipOff(&'static str, &'static str),
    /// A boolean setting that asks this first before turning on: question, button.
    AskFlip(&'static str, &'static str, &'static str, &'static str),
    /// Between two values of a setting.
    Swap(&'static str, &'static str, &'static str, &'static str),
    Set(&'static str, &'static str, Value),
    /// Settings that must change together.
    Both(&'static [(&'static str, &'static str, Value)]),
    /// A number setting: by, from, to.
    Step(&'static str, &'static str, f64, f64, f64),
    /// One of Gala's actions (`ActionType` in its `lib/CommonEnums.vala`).
    Gala(i32),
    /// One of the window manager's own keyboard actions (snap, center),
    /// which only its shortcut can start: schema, key.
    Wm(&'static str, &'static str),
    GoTo(i32),
    MoveTo(i32),
    /// Opens or closes a menu of the panel.
    Menu(&'static str),
    Folder(Place),
    Volume(&'static str, &'static str),
    VolumeTo(&'static str, f64),
    Mute(&'static str),
    NextDevice(&'static str),
    Media(&'static str),
    Seek(i64),
    Screen(&'static str),
    BrightnessTo(i32),
    KeyboardLight(&'static str),
    Profile(&'static str),
    Accent(i32),
    Native(fn() -> Done),
    /// Runs once Spotlight has closed.
    Later(fn() -> Done),
    /// Can lose work, so it asks first: question, button.
    Ask(&'static str, &'static str, fn() -> Done),
}
use Run::{
    Accent, Ask, AskFlip, Both, BrightnessTo, Flip, FlipOff, Folder, Gala, GoTo, KeyboardLight, Later, Media, Menu, MoveTo, Mute,
    Native, NextDevice, Profile, Screen, Seek, Set, Step, Swap, Volume, VolumeTo, Wm,
};

/// What a command needs of this computer, beyond the setting it writes.
#[derive(Clone, Copy)]
enum Needs {
    Suspend,
    Hibernate,
    HybridSleep,
    SuspendThenHibernate,
    Firmware,
    Seat,
    Profile(&'static str),
    Battery,
    LightSensor,
    Sound,
    Brightness,
    KeyboardLight,
    Panel,
    Gala,
    /// Presses keys for the window manager's own actions.
    RemoteDesktop,
    Network,
    MobileData,
    Vpn,
    Bluetooth,
    SeveralLayouts,
    Touchpad,
    ScreenshotTool,
    Program(&'static str),
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

/// The subtitle of each command is its group on the Shortcut page.
#[rustfmt::skip]
static COMMANDS: &[SystemCommand] = &[

    // Session
    c("lock-screen", "Lock Screen", "Session", &["lock", "lock screen", "lock computer", "lock pc"], "lock", Later(lock)),
    n(Needs::Suspend, "sleep", "Sleep", "Session", &["sleep", "suspend", "standby", "go to sleep"], "moon", Later(suspend)),
    n(Needs::Hibernate, "hibernate", "Hibernate", "Session", &["hibernate"], "moon-star", Later(hibernate)),
    n(Needs::HybridSleep, "hybrid-sleep", "Hybrid Sleep", "Session", &["hybrid sleep"], "moon-star", Later(hybrid_sleep)),
    n(Needs::SuspendThenHibernate, "suspend-then-hibernate", "Sleep Then Hibernate", "Session", &["sleep then hibernate", "suspend then hibernate"], "moon-star", Later(suspend_then_hibernate)),
    c("shut-down", "Shut Down", "Session", &["shutdown", "shut down", "power off", "turn off computer"], "power", Ask("Shut down this computer?", "Shut Down", shut_down)),
    c("restart", "Restart", "Session", &["restart", "reboot", "restart computer"], "rotate-cw", Ask("Restart this computer?", "Restart", restart)),
    c("log-out", "Log Out", "Session", &["logout", "log out", "log off", "logoff", "sign out"], "log-out", Ask("Log out of this session?", "Log Out", log_out)),
    n(Needs::Firmware, "restart-firmware", "Restart to Firmware Setup", "Session", &["bios", "uefi", "firmware setup", "restart to bios"], "cpu", Ask("Restart into the firmware setup (BIOS/UEFI)?", "Restart", restart_to_firmware)),
    n(Needs::Seat, "switch-user", "Switch User", "Session", &["switch user", "change user", "other user"], "users", Later(switch_user)),

    // Power
    c("keep-awake", "Toggle Keep Awake", "Power", &["keep awake", "caffeine", "stay awake", "no sleep", "prevent sleep", "amphetamine"], "coffee", Native(flip_keep_awake)),
    n(Needs::Profile("performance"), "power-mode-performance", "Power Mode Performance", "Power", &["performance mode", "high performance", "full power", "power mode performance"], "gauge", Profile("performance")),
    n(Needs::Profile("balanced"), "power-mode-balanced", "Power Mode Balanced", "Power", &["balanced mode", "normal power", "power mode balanced"], "gauge", Profile("balanced")),
    n(Needs::Profile("power-saver"), "power-mode-power-saver", "Power Mode Power Saver", "Power", &["power saver", "power saving", "save power", "power mode power saver"], "gauge", Profile("power-saver")),
    c("idle-sleep-plugged", "Toggle Sleep When Idle (Plugged In)", "Power", &["sleep when idle plugged in", "auto sleep plugged in", "never sleep plugged in"], "moon", Swap(POWER, "sleep-inactive-ac-type", "suspend", "nothing")),
    n(Needs::Battery, "idle-sleep-battery", "Toggle Sleep When Idle (On Battery)", "Power", &["sleep when idle on battery", "auto sleep on battery", "never sleep on battery"], "moon", Swap(POWER, "sleep-inactive-battery-type", "suspend", "nothing")),
    c("sleep-plugged-5", "Sleep After 5 Minutes (Plugged In)", "Power", &["sleep after 5 minutes plugged in"], "timer", Set(POWER, "sleep-inactive-ac-timeout", V(300.0))),
    n(Needs::Battery, "sleep-battery-5", "Sleep After 5 Minutes (On Battery)", "Power", &["sleep after 5 minutes on battery"], "timer", Set(POWER, "sleep-inactive-battery-timeout", V(300.0))),
    c("sleep-plugged-10", "Sleep After 10 Minutes (Plugged In)", "Power", &["sleep after 10 minutes plugged in"], "timer", Set(POWER, "sleep-inactive-ac-timeout", V(600.0))),
    n(Needs::Battery, "sleep-battery-10", "Sleep After 10 Minutes (On Battery)", "Power", &["sleep after 10 minutes on battery"], "timer", Set(POWER, "sleep-inactive-battery-timeout", V(600.0))),
    c("sleep-plugged-15", "Sleep After 15 Minutes (Plugged In)", "Power", &["sleep after 15 minutes plugged in"], "timer", Set(POWER, "sleep-inactive-ac-timeout", V(900.0))),
    n(Needs::Battery, "sleep-battery-15", "Sleep After 15 Minutes (On Battery)", "Power", &["sleep after 15 minutes on battery"], "timer", Set(POWER, "sleep-inactive-battery-timeout", V(900.0))),
    c("sleep-plugged-30", "Sleep After 30 Minutes (Plugged In)", "Power", &["sleep after 30 minutes plugged in"], "timer", Set(POWER, "sleep-inactive-ac-timeout", V(1800.0))),
    n(Needs::Battery, "sleep-battery-30", "Sleep After 30 Minutes (On Battery)", "Power", &["sleep after 30 minutes on battery"], "timer", Set(POWER, "sleep-inactive-battery-timeout", V(1800.0))),
    c("sleep-plugged-60", "Sleep After 1 Hour (Plugged In)", "Power", &["sleep after 1 hour plugged in"], "timer", Set(POWER, "sleep-inactive-ac-timeout", V(3600.0))),
    n(Needs::Battery, "sleep-battery-60", "Sleep After 1 Hour (On Battery)", "Power", &["sleep after 1 hour on battery"], "timer", Set(POWER, "sleep-inactive-battery-timeout", V(3600.0))),
    c("screen-off-never", "Never Turn Screen Off", "Power", &["never turn screen off", "screen always on", "disable screen timeout"], "monitor", Set(SESSION, "idle-delay", V(0.0))),
    c("screen-off-1", "Turn Screen Off After 1 Minute", "Power", &["screen off after 1 minute", "screen timeout 1 minute"], "monitor-off", Set(SESSION, "idle-delay", V(60.0))),
    c("screen-off-2", "Turn Screen Off After 2 Minutes", "Power", &["screen off after 2 minutes", "screen timeout 2 minutes"], "monitor-off", Set(SESSION, "idle-delay", V(120.0))),
    c("screen-off-3", "Turn Screen Off After 3 Minutes", "Power", &["screen off after 3 minutes", "screen timeout 3 minutes"], "monitor-off", Set(SESSION, "idle-delay", V(180.0))),
    c("screen-off-5", "Turn Screen Off After 5 Minutes", "Power", &["screen off after 5 minutes", "screen timeout 5 minutes"], "monitor-off", Set(SESSION, "idle-delay", V(300.0))),
    c("screen-off-10", "Turn Screen Off After 10 Minutes", "Power", &["screen off after 10 minutes", "screen timeout 10 minutes"], "monitor-off", Set(SESSION, "idle-delay", V(600.0))),
    c("screen-off-15", "Turn Screen Off After 15 Minutes", "Power", &["screen off after 15 minutes", "screen timeout 15 minutes"], "monitor-off", Set(SESSION, "idle-delay", V(900.0))),
    c("idle-dim", "Toggle Dim Screen When Idle", "Power", &["dim screen when idle", "idle dim", "auto dim"], "sun-dim", Flip(POWER, "idle-dim")),
    n(Needs::LightSensor, "auto-brightness", "Toggle Automatic Brightness", "Power", &["automatic brightness", "auto brightness", "adaptive brightness"], "sun", Flip(POWER, "ambient-enabled")),
    n(Needs::Battery, "low-battery-saver", "Toggle Power Saver on Low Battery", "Power", &["power saver on low battery", "low battery saver"], "battery", Flip(POWER, "power-saver-profile-on-low-battery")),
    c("power-button-sleep", "Power Button Sleeps", "Power", &["power button sleep", "power button suspend"], "power", Set(POWER, "power-button-action", T("suspend"))),
    c("power-button-ask", "Power Button Asks", "Power", &["power button asks", "power button menu"], "power", Set(POWER, "power-button-action", T("interactive"))),
    c("power-button-nothing", "Power Button Does Nothing", "Power", &["power button nothing", "disable power button"], "power", Set(POWER, "power-button-action", T("nothing"))),

    // Sound
    n(Needs::Sound, "volume-up", "Volume Up", "Sound", &["volume up", "louder", "increase volume", "sound up"], "volume-2", Volume(SPEAKERS, "5%+")),
    n(Needs::Sound, "volume-down", "Volume Down", "Sound", &["volume down", "quieter", "decrease volume", "sound down"], "volume-1", Volume(SPEAKERS, "5%-")),
    n(Needs::Sound, "volume-up-fine", "Volume Up a Little", "Sound", &["volume up a little", "slightly louder"], "volume-2", Volume(SPEAKERS, "1%+")),
    n(Needs::Sound, "volume-down-fine", "Volume Down a Little", "Sound", &["volume down a little", "slightly quieter"], "volume-1", Volume(SPEAKERS, "1%-")),
    n(Needs::Sound, "toggle-mute", "Toggle Mute", "Sound", &["mute", "unmute", "silence", "toggle mute"], "volume-x", Mute(SPEAKERS)),
    n(Needs::Sound, "volume-10", "Set Volume to 10%", "Sound", &["volume 10", "set volume to 10", "volume 10%"], "volume-2", VolumeTo(SPEAKERS, 0.1)),
    n(Needs::Sound, "volume-25", "Set Volume to 25%", "Sound", &["volume 25", "set volume to 25", "volume 25%"], "volume-2", VolumeTo(SPEAKERS, 0.25)),
    n(Needs::Sound, "volume-50", "Set Volume to 50%", "Sound", &["volume 50", "set volume to 50", "volume 50%"], "volume-2", VolumeTo(SPEAKERS, 0.5)),
    n(Needs::Sound, "volume-75", "Set Volume to 75%", "Sound", &["volume 75", "set volume to 75", "volume 75%"], "volume-2", VolumeTo(SPEAKERS, 0.75)),
    n(Needs::Sound, "volume-100", "Set Volume to 100%", "Sound", &["volume 100", "set volume to 100", "volume 100%"], "volume-2", VolumeTo(SPEAKERS, 1.0)),
    n(Needs::Sound, "mic-mute", "Toggle Microphone Mute", "Sound", &["mic mute", "mute mic", "mute microphone", "unmute mic", "unmute microphone"], "mic-off", Mute(MICROPHONE)),
    n(Needs::Sound, "mic-up", "Microphone Volume Up", "Sound", &["mic up", "microphone up", "microphone volume up"], "mic", Volume(MICROPHONE, "5%+")),
    n(Needs::Sound, "mic-down", "Microphone Volume Down", "Sound", &["mic down", "microphone down", "microphone volume down"], "mic", Volume(MICROPHONE, "5%-")),
    n(Needs::Sound, "mic-25", "Set Microphone to 25%", "Sound", &["mic 25", "microphone 25", "set microphone to 25"], "mic", VolumeTo(MICROPHONE, 0.25)),
    n(Needs::Sound, "mic-50", "Set Microphone to 50%", "Sound", &["mic 50", "microphone 50", "set microphone to 50"], "mic", VolumeTo(MICROPHONE, 0.5)),
    n(Needs::Sound, "mic-75", "Set Microphone to 75%", "Sound", &["mic 75", "microphone 75", "set microphone to 75"], "mic", VolumeTo(MICROPHONE, 0.75)),
    n(Needs::Sound, "mic-100", "Set Microphone to 100%", "Sound", &["mic 100", "microphone 100", "set microphone to 100"], "mic", VolumeTo(MICROPHONE, 1.0)),
    n(Needs::Sound, "next-output", "Next Sound Output", "Sound", &["next output", "switch speakers", "switch headphones", "change output device", "next sound output"], "headphones", NextDevice("Sinks:")),
    n(Needs::Sound, "next-input", "Next Sound Input", "Sound", &["next input", "switch microphone", "change input device", "next sound input"], "mic", NextDevice("Sources:")),
    c("over-amplify", "Toggle Volume Above 100%", "Sound", &["volume above 100", "over amplify", "overamplification", "boost volume"], "audio-lines", Flip(SOUND, "allow-volume-above-100-percent")),
    c("event-sounds", "Toggle Event Sounds", "Sound", &["event sounds", "system sounds", "alert sounds"], "bell", Flip(SOUND, "event-sounds")),
    c("input-feedback-sounds", "Toggle Input Feedback Sounds", "Sound", &["input feedback sounds", "click sounds", "button sounds"], "audio-lines", Flip(SOUND, "input-feedback-sounds")),
    c("audible-bell", "Toggle Alert Sound", "Sound", &["alert sound", "audible bell", "system beep", "beep"], "bell-ring", Flip(WM_PREFS, "audible-bell")),
    c("visual-bell", "Toggle Screen Flash on Alerts", "Sound", &["screen flash", "visual bell", "flash on alert"], "zap", Flip(WM_PREFS, "visual-bell")),
    c("visual-bell-screen", "Alerts Flash the Whole Screen", "Sound", &["flash whole screen"], "zap", Set(WM_PREFS, "visual-bell-type", T("fullscreen-flash"))),
    c("visual-bell-window", "Alerts Flash the Window", "Sound", &["flash window"], "zap", Set(WM_PREFS, "visual-bell-type", T("frame-flash"))),

    // Media
    c("media-play-pause", "Play or Pause Media", "Media", &["play", "pause", "play pause", "resume music", "pause music"], "play", Media("PlayPause")),
    c("media-play", "Play Media", "Media", &["play media", "play music", "resume media"], "play", Media("Play")),
    c("media-pause", "Pause Media", "Media", &["pause media", "pause video"], "pause", Media("Pause")),
    c("media-stop", "Stop Media", "Media", &["stop media", "stop music", "stop playing"], "square", Media("Stop")),
    c("media-next", "Next Track", "Media", &["next track", "next song", "skip song", "skip track"], "skip-forward", Media("Next")),
    c("media-previous", "Previous Track", "Media", &["previous track", "previous song", "last song"], "skip-back", Media("Previous")),
    c("media-forward-10", "Skip Forward 10 Seconds", "Media", &["forward 10 seconds", "skip forward 10"], "fast-forward", Seek(10)),
    c("media-back-10", "Skip Back 10 Seconds", "Media", &["back 10 seconds", "skip back 10", "rewind 10"], "rewind", Seek(-10)),
    c("media-forward-30", "Skip Forward 30 Seconds", "Media", &["forward 30 seconds", "skip forward 30"], "fast-forward", Seek(30)),
    c("media-back-30", "Skip Back 30 Seconds", "Media", &["back 30 seconds", "skip back 30", "rewind 30"], "rewind", Seek(-30)),
    c("media-shuffle", "Toggle Shuffle", "Media", &["shuffle", "toggle shuffle"], "shuffle", Native(flip_shuffle)),
    c("media-repeat", "Cycle Repeat", "Media", &["repeat", "loop", "repeat song", "cycle repeat"], "repeat", Native(cycle_repeat)),
    c("media-show", "Show Media Player", "Media", &["show media player", "show player", "raise player"], "music", Later(raise_player)),

    // Display
    n(Needs::Brightness, "brightness-up", "Brightness Up", "Display", &["brightness up", "brighter", "increase brightness"], "sun", Screen("StepUp")),
    n(Needs::Brightness, "brightness-down", "Brightness Down", "Display", &["brightness down", "dimmer", "decrease brightness", "dim screen"], "sun-dim", Screen("StepDown")),
    n(Needs::Brightness, "brightness-10", "Set Brightness to 10%", "Display", &["brightness 10", "set brightness to 10", "brightness 10%"], "sun", BrightnessTo(10)),
    n(Needs::Brightness, "brightness-25", "Set Brightness to 25%", "Display", &["brightness 25", "set brightness to 25", "brightness 25%"], "sun", BrightnessTo(25)),
    n(Needs::Brightness, "brightness-50", "Set Brightness to 50%", "Display", &["brightness 50", "set brightness to 50", "brightness 50%"], "sun", BrightnessTo(50)),
    n(Needs::Brightness, "brightness-75", "Set Brightness to 75%", "Display", &["brightness 75", "set brightness to 75", "brightness 75%"], "sun", BrightnessTo(75)),
    n(Needs::Brightness, "brightness-100", "Set Brightness to 100%", "Display", &["brightness 100", "set brightness to 100", "brightness 100%"], "sun", BrightnessTo(100)),
    n(Needs::KeyboardLight, "keyboard-light-up", "Keyboard Backlight Up", "Display", &["keyboard backlight up", "keyboard light up"], "keyboard", KeyboardLight("StepUp")),
    n(Needs::KeyboardLight, "keyboard-light-down", "Keyboard Backlight Down", "Display", &["keyboard backlight down", "keyboard light down"], "keyboard", KeyboardLight("StepDown")),
    n(Needs::KeyboardLight, "keyboard-light-toggle", "Toggle Keyboard Backlight", "Display", &["keyboard backlight", "keyboard light", "toggle keyboard backlight"], "keyboard", KeyboardLight("Toggle")),

    // Night Light
    c("night-light-schedule", "Toggle Night Light Sunset to Sunrise", "Night Light", &["night light sunset to sunrise", "night light schedule", "night light automatic"], "moon-star", Flip(COLOR, "night-light-schedule-automatic")),
    c("night-light-pause", "Toggle Night Light Until Tomorrow", "Night Light", &["night light until tomorrow", "pause night light", "snooze night light"], "moon", Native(flip_night_light_pause)),
    c("night-light-warmer", "Night Light Warmer", "Night Light", &["night light warmer", "warmer screen"], "sun-dim", Step(COLOR, "night-light-temperature", -500.0, 1700.0, 6000.0)),
    c("night-light-cooler", "Night Light Cooler", "Night Light", &["night light cooler", "cooler screen"], "sun", Step(COLOR, "night-light-temperature", 500.0, 1700.0, 6000.0)),
    c("night-light-very-warm", "Night Light Very Warm", "Night Light", &["night light very warm"], "sun-dim", Set(COLOR, "night-light-temperature", V(2000.0))),
    c("night-light-warm", "Night Light Warm", "Night Light", &["night light warm"], "sun-dim", Set(COLOR, "night-light-temperature", V(2700.0))),
    c("night-light-medium", "Night Light Medium", "Night Light", &["night light medium"], "sun-dim", Set(COLOR, "night-light-temperature", V(3500.0))),
    c("night-light-soft", "Night Light Soft", "Night Light", &["night light soft"], "sun-dim", Set(COLOR, "night-light-temperature", V(4500.0))),

    // Color Filters
    c("color-filter-off", "Color Filter Off", "Color Filters", &["color filter off", "no color filter", "color filter off"], "contrast", Set(GALA_A11Y, "colorblindness-correction-filter", T("none"))),
    c("color-filter-protanopia", "Color Filter Protanopia", "Color Filters", &["protanopia", "red blind", "color filter protanopia"], "contrast", Set(GALA_A11Y, "colorblindness-correction-filter", T("protanopia"))),
    c("color-filter-protanopia-high", "Color Filter Protanopia High Contrast", "Color Filters", &["protanopia high contrast", "color filter protanopia high contrast"], "contrast", Set(GALA_A11Y, "colorblindness-correction-filter", T("protanopia-high-contrast"))),
    c("color-filter-deuteranopia", "Color Filter Deuteranopia", "Color Filters", &["deuteranopia", "green blind", "color filter deuteranopia"], "contrast", Set(GALA_A11Y, "colorblindness-correction-filter", T("deuteranopia"))),
    c("color-filter-deuteranopia-high", "Color Filter Deuteranopia High Contrast", "Color Filters", &["deuteranopia high contrast", "color filter deuteranopia high contrast"], "contrast", Set(GALA_A11Y, "colorblindness-correction-filter", T("deuteranopia-high-contrast"))),
    c("color-filter-tritanopia", "Color Filter Tritanopia", "Color Filters", &["tritanopia", "blue blind", "color filter tritanopia"], "contrast", Set(GALA_A11Y, "colorblindness-correction-filter", T("tritanopia"))),

    // Appearance
    c("dark-style", "Use Dark Style", "Appearance", &["use dark style", "dark style on", "set dark mode"], "moon", Set(SCHEME, "color-scheme", T("prefer-dark"))),
    c("light-style", "Use Light Style", "Appearance", &["use light style", "light style on", "set light mode"], "sun", Set(SCHEME, "color-scheme", T("no-preference"))),
    c("dark-schedule-off", "Dark Style Schedule Off", "Appearance", &["dark schedule off", "dark style schedule off"], "sun-moon", Set(SCHEME, "prefer-dark-schedule", T("disabled"))),
    c("dark-schedule-sunset", "Dark Style Sunset to Sunrise", "Appearance", &["dark style sunset to sunrise", "dark at sunset", "auto dark mode"], "sun-moon", Set(SCHEME, "prefer-dark-schedule", T("sunset-to-sunrise"))),
    c("dark-schedule-manual", "Dark Style on a Schedule", "Appearance", &["dark style on a schedule", "dark schedule manual"], "sun-moon", Set(SCHEME, "prefer-dark-schedule", T("manual"))),
    c("dim-wallpaper", "Toggle Dim Wallpaper in Dark Style", "Appearance", &["dim wallpaper", "dim wallpaper in dark style"], "image", Flip(DIM, "dim-wallpaper-in-dark-style")),
    c("overlay-scrollbars", "Toggle Overlay Scrollbars", "Appearance", &["overlay scrollbars", "hide scrollbars", "always show scrollbars"], "sliders-horizontal", Flip(INTERFACE, "overlay-scrolling")),
    c("cursor-blink", "Toggle Text Cursor Blink", "Appearance", &["cursor blink", "blinking cursor", "text cursor blink"], "type", Flip(INTERFACE, "cursor-blink")),
    c("primary-paste", "Toggle Middle-Click Paste", "Appearance", &["middle click paste", "primary paste"], "clipboard", Flip(INTERFACE, "gtk-enable-primary-paste")),

    // Accent Color
    c("accent-automatic", "Accent Color Automatic", "Accent Color", &["accent automatic", "accent color automatic", "accent wallpaper", "wallpaper accent"], "palette", Accent(0)),
    c("accent-strawberry", "Accent Color Strawberry", "Accent Color", &["accent strawberry", "accent color strawberry", "accent red", "red accent"], "palette", Accent(1)),
    c("accent-orange", "Accent Color Orange", "Accent Color", &["accent orange", "accent color orange", "accent orange", "orange accent"], "palette", Accent(2)),
    c("accent-banana", "Accent Color Banana", "Accent Color", &["accent banana", "accent color banana", "accent yellow", "yellow accent"], "palette", Accent(3)),
    c("accent-lime", "Accent Color Lime", "Accent Color", &["accent lime", "accent color lime", "accent green", "green accent"], "palette", Accent(4)),
    c("accent-mint", "Accent Color Mint", "Accent Color", &["accent mint", "accent color mint", "accent teal", "teal accent"], "palette", Accent(5)),
    c("accent-blueberry", "Accent Color Blueberry", "Accent Color", &["accent blueberry", "accent color blueberry", "accent blue", "blue accent"], "palette", Accent(6)),
    c("accent-grape", "Accent Color Grape", "Accent Color", &["accent grape", "accent color grape", "accent purple", "purple accent"], "palette", Accent(7)),
    c("accent-bubblegum", "Accent Color Bubblegum", "Accent Color", &["accent bubblegum", "accent color bubblegum", "accent pink", "pink accent"], "palette", Accent(8)),
    c("accent-cocoa", "Accent Color Cocoa", "Accent Color", &["accent cocoa", "accent color cocoa", "accent brown", "brown accent"], "palette", Accent(9)),
    c("accent-slate", "Accent Color Slate", "Accent Color", &["accent slate", "accent color slate", "accent gray", "gray accent"], "palette", Accent(10)),
    c("accent-latte", "Accent Color Latte", "Accent Color", &["accent latte", "accent color latte", "accent beige", "beige accent"], "palette", Accent(11)),

    // Wallpaper
    c("next-wallpaper", "Next Wallpaper", "Wallpaper", &["next wallpaper", "change wallpaper now", "shuffle wallpaper"], "images", Native(next_wallpaper)),
    c("wallpaper-zoom", "Wallpaper Zoom", "Wallpaper", &["wallpaper zoom"], "image", Set(BACKGROUND, "picture-options", T("zoom"))),
    c("wallpaper-fit", "Wallpaper Fit", "Wallpaper", &["wallpaper fit"], "image", Set(BACKGROUND, "picture-options", T("scaled"))),
    c("wallpaper-center", "Wallpaper Center", "Wallpaper", &["wallpaper center"], "image", Set(BACKGROUND, "picture-options", T("centered"))),
    c("wallpaper-stretch", "Wallpaper Stretch", "Wallpaper", &["wallpaper stretch"], "image", Set(BACKGROUND, "picture-options", T("stretched"))),
    c("wallpaper-span", "Wallpaper Span", "Wallpaper", &["wallpaper span"], "image", Set(BACKGROUND, "picture-options", T("spanned"))),
    c("wallpaper-tile", "Wallpaper Tile", "Wallpaper", &["wallpaper tile"], "image", Set(BACKGROUND, "picture-options", T("wallpaper"))),

    // Text
    c("text-bigger", "Make Text Bigger", "Text", &["make text bigger", "bigger text", "increase text size"], "a-large-small", Step(INTERFACE, "text-scaling-factor", 0.05, 0.75, 1.5)),
    c("text-smaller", "Make Text Smaller", "Text", &["make text smaller", "smaller text", "decrease text size"], "a-large-small", Step(INTERFACE, "text-scaling-factor", -0.05, 0.75, 1.5)),
    c("text-size-small", "Text Size Small", "Text", &["text size small", "small text size"], "type", Set(INTERFACE, "text-scaling-factor", V(0.85))),
    c("text-size-default", "Text Size Default", "Text", &["text size default", "default text size"], "type", Set(INTERFACE, "text-scaling-factor", V(1.0))),
    c("text-size-large", "Text Size Large", "Text", &["text size large", "large text size"], "type", Set(INTERFACE, "text-scaling-factor", V(1.25))),
    c("text-size-larger", "Text Size Larger", "Text", &["text size larger", "larger text size"], "type", Set(INTERFACE, "text-scaling-factor", V(1.5))),
    c("font-smoothing-none", "Font Smoothing None", "Text", &["font smoothing none", "antialiasing none"], "type", Set(INTERFACE, "font-antialiasing", T("none"))),
    c("font-smoothing-grayscale", "Font Smoothing Grayscale", "Text", &["font smoothing grayscale", "antialiasing grayscale"], "type", Set(INTERFACE, "font-antialiasing", T("grayscale"))),
    c("font-smoothing-rgba", "Font Smoothing Subpixel", "Text", &["font smoothing subpixel", "antialiasing subpixel"], "type", Set(INTERFACE, "font-antialiasing", T("rgba"))),
    c("font-hinting-none", "Font Hinting None", "Text", &["font hinting none", "hinting none"], "type", Set(INTERFACE, "font-hinting", T("none"))),
    c("font-hinting-slight", "Font Hinting Slight", "Text", &["font hinting slight", "hinting slight"], "type", Set(INTERFACE, "font-hinting", T("slight"))),
    c("font-hinting-medium", "Font Hinting Medium", "Text", &["font hinting medium", "hinting medium"], "type", Set(INTERFACE, "font-hinting", T("medium"))),
    c("font-hinting-full", "Font Hinting Full", "Text", &["font hinting full", "hinting full"], "type", Set(INTERFACE, "font-hinting", T("full"))),

    // Pointer
    c("pointer-size-small", "Pointer Size Small", "Pointer", &["pointer size small", "cursor size small", "small pointer", "small cursor"], "mouse-pointer-2", Set(INTERFACE, "cursor-size", V(24.0))),
    c("pointer-size-medium", "Pointer Size Medium", "Pointer", &["pointer size medium", "cursor size medium", "medium pointer", "medium cursor"], "mouse-pointer-2", Set(INTERFACE, "cursor-size", V(32.0))),
    c("pointer-size-large", "Pointer Size Large", "Pointer", &["pointer size large", "cursor size large", "large pointer", "large cursor"], "mouse-pointer-2", Set(INTERFACE, "cursor-size", V(48.0))),
    c("locate-pointer", "Toggle Locate Pointer With Ctrl", "Pointer", &["locate pointer", "find pointer", "find my cursor", "show pointer with ctrl"], "locate", Flip(INTERFACE, "locate-pointer")),

    // Dock
    c("dock-hide-never", "Never Hide Dock", "Dock", &["never hide dock", "dock hide never"], "panel-bottom", Set(DOCK, "autohide-mode", T("never"))),
    c("dock-hide-maximized", "Hide Dock for Maximized Windows", "Dock", &["hide dock for maximized windows", "dock hide maximized"], "panel-bottom", Set(DOCK, "autohide-mode", T("maximized-focus-window"))),
    c("dock-hide-overlap-focus", "Hide Dock When Focused Window Overlaps", "Dock", &["hide dock when focused window overlaps", "dock hide overlap focus"], "panel-bottom", Set(DOCK, "autohide-mode", T("overlapping-focus-window"))),
    c("dock-hide-overlap-any", "Hide Dock When Any Window Overlaps", "Dock", &["hide dock when any window overlaps", "dock hide overlap any"], "panel-bottom", Set(DOCK, "autohide-mode", T("overlapping-window"))),
    c("dock-hide-always", "Always Hide Dock", "Dock", &["always hide dock", "dock hide always"], "panel-bottom", Set(DOCK, "autohide-mode", T("always"))),
    c("dock-size-small", "Dock Icons Small", "Dock", &["dock icons small", "dock size small", "small dock"], "panel-bottom", Set(DOCK, "icon-size", V(32.0))),
    c("dock-size-default", "Dock Icons Default", "Dock", &["dock icons default", "dock size default", "default dock"], "panel-bottom", Set(DOCK, "icon-size", V(48.0))),
    c("dock-size-large", "Dock Icons Large", "Dock", &["dock icons large", "dock size large", "large dock"], "panel-bottom", Set(DOCK, "icon-size", V(64.0))),

    // Panel
    c("panel-translucency", "Toggle Panel Translucency", "Panel", &["panel translucency", "transparent panel", "solid panel"], "panel-top", Flip(PANEL, "use-transparency")),
    c("panel-scroll-workspaces", "Toggle Scroll on Panel to Switch Workspaces", "Panel", &["scroll on panel", "panel scroll workspaces"], "panel-top", Flip(PANEL, "scroll-to-switch-workspaces")),
    n(Needs::Battery, "battery-percentage", "Toggle Battery Percentage", "Panel", &["battery percentage", "show battery percent", "battery percent"], "battery", Flip(BATTERY_PANEL, "show-percentage")),
    c("panel-caps-lock", "Toggle Caps Lock in Panel", "Panel", &["caps lock in panel", "panel caps lock"], "keyboard", Flip(PANEL_KEYS, "capslock")),
    c("panel-num-lock", "Toggle Num Lock in Panel", "Panel", &["num lock in panel", "panel num lock"], "keyboard", Flip(PANEL_KEYS, "numlock")),
    c("panel-layout", "Toggle Keyboard Layout in Panel", "Panel", &["keyboard layout in panel", "always show layout"], "languages", Flip(PANEL_KEYS, "always-show-layout")),
    c("panel-a11y", "Toggle Accessibility in Quick Settings", "Panel", &["accessibility in quick settings"], "accessibility", Flip(QUICK_SETTINGS, "show-a11y")),

    // Panel Menus
    n(Needs::Panel, "menu-applications", "Open Applications Menu", "Panel Menus", &["applications menu", "app menu", "launcher", "start menu"], "layout-grid", Menu("app-launcher")),
    n(Needs::Panel, "menu-notifications", "Open Notifications", "Panel Menus", &["open notifications", "notification center", "notifications menu"], "bell", Menu("messages")),
    n(Needs::Panel, "menu-quick-settings", "Open Quick Settings", "Panel Menus", &["quick settings", "control center", "action center"], "sliders-horizontal", Menu("quick-settings")),
    n(Needs::Panel, "menu-calendar", "Open Calendar Menu", "Panel Menus", &["calendar menu", "date menu", "clock menu"], "calendar-days", Menu("datetime")),
    n(Needs::Panel, "menu-network", "Open Network Menu", "Panel Menus", &["network menu", "wifi menu"], "wifi", Menu("network")),
    n(Needs::Panel, "menu-sound", "Open Sound Menu", "Panel Menus", &["sound menu", "volume menu"], "volume-2", Menu("sound")),
    n(Needs::Panel, "menu-bluetooth", "Open Bluetooth Menu", "Panel Menus", &["bluetooth menu"], "bluetooth", Menu("bluetooth")),
    n(Needs::Panel, "menu-power", "Open Power Menu", "Panel Menus", &["power menu", "battery menu"], "battery-charging", Menu("power")),
    n(Needs::Panel, "menu-keyboard", "Open Keyboard Menu", "Panel Menus", &["keyboard menu", "layout menu"], "keyboard", Menu("keyboard")),
    n(Needs::Panel, "menu-night-light", "Open Night Light Menu", "Panel Menus", &["night light menu"], "moon", Menu("nightlight")),

    // Clock
    c("clock-12h", "Use 12-Hour Clock", "Clock", &["12 hour clock", "12h clock", "12-hour clock", "use 12 hour time"], "clock", Both(&[(INTERFACE, "clock-format", T("12h")), (CLOCK, "clock-format", T("12h"))])),
    c("clock-24h", "Use 24-Hour Clock", "Clock", &["24 hour clock", "24h clock", "24-hour clock", "use 24 hour time"], "clock", Both(&[(INTERFACE, "clock-format", T("24h")), (CLOCK, "clock-format", T("24h"))])),
    c("clock-seconds", "Toggle Seconds in Clock", "Clock", &["seconds in clock", "show seconds"], "clock", Flip(CLOCK, "clock-show-seconds")),
    c("clock-date", "Toggle Date in Clock", "Clock", &["date in clock", "show date"], "calendar-days", Flip(CLOCK, "clock-show-date")),
    c("clock-weekday", "Toggle Weekday in Clock", "Clock", &["weekday in clock", "show weekday", "day of week in clock"], "calendar-days", Flip(CLOCK, "clock-show-weekday")),
    c("clock-week-numbers", "Toggle Week Numbers", "Clock", &["week numbers", "show week numbers"], "calendar-days", Flip(CLOCK, "show-weeks")),

    // Window Manager: found by "window" (all of them, in this order) and
    // listed with a setting of their own; all act on the window in use.
    n(Needs::RemoteDesktop, "window-snap-left", "Snap Window Left", WINDOW_MANAGER, &["window manager", "window left", "window left half", "snap left", "snap window left", "tile left", "left half"], "panel-left", Wm(MUTTER_KEYS, "toggle-tiled-left")),
    n(Needs::RemoteDesktop, "window-snap-right", "Snap Window Right", WINDOW_MANAGER, &["window manager", "window right", "window right half", "snap right", "snap window right", "tile right", "right half"], "panel-right", Wm(MUTTER_KEYS, "toggle-tiled-right")),
    n(Needs::RemoteDesktop, "window-top-left", "Move Window to Top Left", WINDOW_MANAGER, &["window manager", "window top left", "top left", "top left corner", "snap top left"], "arrow-up-left", Wm(WM_KEYS, "move-to-corner-nw")),
    n(Needs::RemoteDesktop, "window-top-right", "Move Window to Top Right", WINDOW_MANAGER, &["window manager", "window top right", "top right", "top right corner", "snap top right"], "arrow-up-right", Wm(WM_KEYS, "move-to-corner-ne")),
    n(Needs::RemoteDesktop, "window-bottom-left", "Move Window to Bottom Left", WINDOW_MANAGER, &["window manager", "window bottom left", "bottom left", "bottom left corner", "snap bottom left"], "arrow-down-left", Wm(WM_KEYS, "move-to-corner-sw")),
    n(Needs::RemoteDesktop, "window-bottom-right", "Move Window to Bottom Right", WINDOW_MANAGER, &["window manager", "window bottom right", "bottom right", "bottom right corner", "snap bottom right"], "arrow-down-right", Wm(WM_KEYS, "move-to-corner-se")),
    n(Needs::RemoteDesktop, "window-top", "Move Window to Top", WINDOW_MANAGER, &["window manager", "window top", "window to top edge", "snap top"], "arrow-up", Wm(WM_KEYS, "move-to-side-n")),
    n(Needs::RemoteDesktop, "window-bottom", "Move Window to Bottom", WINDOW_MANAGER, &["window manager", "window bottom", "window to bottom edge", "snap bottom"], "arrow-down", Wm(WM_KEYS, "move-to-side-s")),
    n(Needs::RemoteDesktop, "window-left-edge", "Move Window to Left Edge", WINDOW_MANAGER, &["window manager", "window left edge", "move window to left edge"], "arrow-left", Wm(WM_KEYS, "move-to-side-w")),
    n(Needs::RemoteDesktop, "window-right-edge", "Move Window to Right Edge", WINDOW_MANAGER, &["window manager", "window right edge", "move window to right edge"], "arrow-right", Wm(WM_KEYS, "move-to-side-e")),
    n(Needs::RemoteDesktop, "window-center", "Center Window", WINDOW_MANAGER, &["window manager", "window center", "window centre", "window middle", "center window", "centre window"], "focus", Wm(WM_KEYS, "move-to-center")),
    n(Needs::Gala, "window-maximize", "Maximize Window", WINDOW_MANAGER, &["window manager", "window maximize", "window full screen", "maximize window", "unmaximize window", "maximize"], "maximize-2", Gala(2)),
    n(Needs::RemoteDesktop, "window-restore", "Restore Window", WINDOW_MANAGER, &["window manager", "window restore", "restore window", "restore window size", "unsnap window", "untile window"], "shrink", Wm(WM_KEYS, "unmaximize")),
    n(Needs::RemoteDesktop, "window-fill-height", "Fill Window Height", WINDOW_MANAGER, &["window manager", "window fill height", "window full height", "fill height", "maximize vertically"], "move-vertical", Wm(WM_KEYS, "maximize-vertically")),
    n(Needs::RemoteDesktop, "window-fill-width", "Fill Window Width", WINDOW_MANAGER, &["window manager", "window fill width", "window full width", "fill width", "maximize horizontally"], "move-horizontal", Wm(WM_KEYS, "maximize-horizontally")),
    n(Needs::Gala, "window-minimize", "Minimize Window", WINDOW_MANAGER, &["window manager", "window minimize", "window hide", "minimize window", "hide window", "minimize"], "minimize-2", Gala(3)),

    // Windows
    n(Needs::Gala, "window-multitasking", "Multitasking View", "Windows", &["multitasking view", "mission control", "task view", "overview"], "layout-grid", Gala(1)),
    n(Needs::Gala, "window-all-windows", "Show All Windows", "Windows", &["show all windows", "window overview", "expose"], "layers", Gala(7)),
    n(Needs::Gala, "window-close", "Close Window", "Windows", &["close window", "close this window"], "x", Gala(17)),
    n(Needs::Gala, "window-always-on-top", "Toggle Always on Top", "Windows", &["always on top", "pin window", "keep on top"], "pin", Gala(13)),
    n(Needs::Gala, "window-all-workspaces", "Toggle Window on All Workspaces", "Windows", &["window on all workspaces", "sticky window", "show on all workspaces"], "pin", Gala(14)),
    n(Needs::Gala, "window-move-left", "Move Window to Left Workspace", "Windows", &["move window left", "move window to left workspace"], "arrow-left", Gala(15)),
    n(Needs::Gala, "window-move-right", "Move Window to Right Workspace", "Windows", &["move window right", "move window to right workspace"], "arrow-right", Gala(16)),

    // Workspaces
    n(Needs::Gala, "workspace-previous", "Previous Workspace", "Workspaces", &["previous workspace", "workspace left", "desktop left"], "arrow-left", Gala(8)),
    n(Needs::Gala, "workspace-next", "Next Workspace", "Workspaces", &["next workspace", "workspace right", "desktop right"], "arrow-right", Gala(9)),
    n(Needs::Gala, "workspace-last", "Last Workspace", "Workspaces", &["last workspace", "new workspace"], "square-plus", Gala(10)),
    n(Needs::Gala, "workspace-1", "Go to Workspace 1", "Workspaces", &["workspace 1", "go to workspace 1", "desktop 1"], "grid-2x2", GoTo(0)),
    n(Needs::Gala, "workspace-2", "Go to Workspace 2", "Workspaces", &["workspace 2", "go to workspace 2", "desktop 2"], "grid-2x2", GoTo(1)),
    n(Needs::Gala, "workspace-3", "Go to Workspace 3", "Workspaces", &["workspace 3", "go to workspace 3", "desktop 3"], "grid-2x2", GoTo(2)),
    n(Needs::Gala, "workspace-4", "Go to Workspace 4", "Workspaces", &["workspace 4", "go to workspace 4", "desktop 4"], "grid-2x2", GoTo(3)),
    n(Needs::Gala, "workspace-5", "Go to Workspace 5", "Workspaces", &["workspace 5", "go to workspace 5", "desktop 5"], "grid-2x2", GoTo(4)),
    n(Needs::Gala, "workspace-6", "Go to Workspace 6", "Workspaces", &["workspace 6", "go to workspace 6", "desktop 6"], "grid-2x2", GoTo(5)),
    n(Needs::Gala, "workspace-7", "Go to Workspace 7", "Workspaces", &["workspace 7", "go to workspace 7", "desktop 7"], "grid-2x2", GoTo(6)),
    n(Needs::Gala, "workspace-8", "Go to Workspace 8", "Workspaces", &["workspace 8", "go to workspace 8", "desktop 8"], "grid-2x2", GoTo(7)),
    n(Needs::Gala, "workspace-9", "Go to Workspace 9", "Workspaces", &["workspace 9", "go to workspace 9", "desktop 9"], "grid-2x2", GoTo(8)),
    n(Needs::Gala, "window-to-workspace-1", "Move Window to Workspace 1", "Workspaces", &["move window to workspace 1", "send window to workspace 1"], "grid-2x2", MoveTo(0)),
    n(Needs::Gala, "window-to-workspace-2", "Move Window to Workspace 2", "Workspaces", &["move window to workspace 2", "send window to workspace 2"], "grid-2x2", MoveTo(1)),
    n(Needs::Gala, "window-to-workspace-3", "Move Window to Workspace 3", "Workspaces", &["move window to workspace 3", "send window to workspace 3"], "grid-2x2", MoveTo(2)),
    n(Needs::Gala, "window-to-workspace-4", "Move Window to Workspace 4", "Workspaces", &["move window to workspace 4", "send window to workspace 4"], "grid-2x2", MoveTo(3)),
    n(Needs::Gala, "window-to-workspace-5", "Move Window to Workspace 5", "Workspaces", &["move window to workspace 5", "send window to workspace 5"], "grid-2x2", MoveTo(4)),
    n(Needs::Gala, "window-to-workspace-6", "Move Window to Workspace 6", "Workspaces", &["move window to workspace 6", "send window to workspace 6"], "grid-2x2", MoveTo(5)),
    n(Needs::Gala, "window-to-workspace-7", "Move Window to Workspace 7", "Workspaces", &["move window to workspace 7", "send window to workspace 7"], "grid-2x2", MoveTo(6)),
    n(Needs::Gala, "window-to-workspace-8", "Move Window to Workspace 8", "Workspaces", &["move window to workspace 8", "send window to workspace 8"], "grid-2x2", MoveTo(7)),
    n(Needs::Gala, "window-to-workspace-9", "Move Window to Workspace 9", "Workspaces", &["move window to workspace 9", "send window to workspace 9"], "grid-2x2", MoveTo(8)),
    c("dynamic-workspaces", "Toggle Dynamic Workspaces", "Workspaces", &["dynamic workspaces", "automatic workspaces"], "layers", Flip(MUTTER, "dynamic-workspaces")),
    c("workspaces-primary", "Toggle Workspaces on Main Display Only", "Workspaces", &["workspaces on main display only", "workspaces on primary display"], "monitor", Flip(MUTTER, "workspaces-only-on-primary")),
    c("maximized-own-workspace", "Toggle Maximized Windows Get a Workspace", "Workspaces", &["maximized windows get a workspace", "move maximized to workspace"], "maximize-2", Flip(GALA, "move-maximized-workspace")),
    c("fullscreen-own-workspace", "Toggle Fullscreen Windows Get a Workspace", "Workspaces", &["fullscreen windows get a workspace", "move fullscreen to workspace"], "maximize-2", Flip(GALA, "move-fullscreened-workspace")),

    // Hot Corners
    c("hot-corner-topleft-off", "Top Left Hot Corner Off", "Hot Corners", &["top left hot corner off", "hot corner top left off"], "scan", Set(GALA, "hotcorner-topleft", T("none"))),
    c("hot-corner-topleft-multitasking", "Top Left Hot Corner Multitasking", "Hot Corners", &["top left hot corner multitasking", "hot corner top left multitasking"], "scan", Set(GALA, "hotcorner-topleft", T("show-workspace-view"))),
    c("hot-corner-topleft-windows", "Top Left Hot Corner All Windows", "Hot Corners", &["top left hot corner all windows", "hot corner top left all windows"], "scan", Set(GALA, "hotcorner-topleft", T("window-overview-all"))),
    c("hot-corner-topleft-apps", "Top Left Hot Corner Applications", "Hot Corners", &["top left hot corner applications", "hot corner top left applications"], "scan", Set(GALA, "hotcorner-topleft", T("open-launcher"))),
    c("hot-corner-topleft-maximize", "Top Left Hot Corner Maximize", "Hot Corners", &["top left hot corner maximize", "hot corner top left maximize"], "scan", Set(GALA, "hotcorner-topleft", T("maximize-current"))),
    c("hot-corner-topleft-minimize", "Top Left Hot Corner Minimize", "Hot Corners", &["top left hot corner minimize", "hot corner top left minimize"], "scan", Set(GALA, "hotcorner-topleft", T("minimize-current"))),
    c("hot-corner-topright-off", "Top Right Hot Corner Off", "Hot Corners", &["top right hot corner off", "hot corner top right off"], "scan", Set(GALA, "hotcorner-topright", T("none"))),
    c("hot-corner-topright-multitasking", "Top Right Hot Corner Multitasking", "Hot Corners", &["top right hot corner multitasking", "hot corner top right multitasking"], "scan", Set(GALA, "hotcorner-topright", T("show-workspace-view"))),
    c("hot-corner-topright-windows", "Top Right Hot Corner All Windows", "Hot Corners", &["top right hot corner all windows", "hot corner top right all windows"], "scan", Set(GALA, "hotcorner-topright", T("window-overview-all"))),
    c("hot-corner-topright-apps", "Top Right Hot Corner Applications", "Hot Corners", &["top right hot corner applications", "hot corner top right applications"], "scan", Set(GALA, "hotcorner-topright", T("open-launcher"))),
    c("hot-corner-topright-maximize", "Top Right Hot Corner Maximize", "Hot Corners", &["top right hot corner maximize", "hot corner top right maximize"], "scan", Set(GALA, "hotcorner-topright", T("maximize-current"))),
    c("hot-corner-topright-minimize", "Top Right Hot Corner Minimize", "Hot Corners", &["top right hot corner minimize", "hot corner top right minimize"], "scan", Set(GALA, "hotcorner-topright", T("minimize-current"))),
    c("hot-corner-bottomleft-off", "Bottom Left Hot Corner Off", "Hot Corners", &["bottom left hot corner off", "hot corner bottom left off"], "scan", Set(GALA, "hotcorner-bottomleft", T("none"))),
    c("hot-corner-bottomleft-multitasking", "Bottom Left Hot Corner Multitasking", "Hot Corners", &["bottom left hot corner multitasking", "hot corner bottom left multitasking"], "scan", Set(GALA, "hotcorner-bottomleft", T("show-workspace-view"))),
    c("hot-corner-bottomleft-windows", "Bottom Left Hot Corner All Windows", "Hot Corners", &["bottom left hot corner all windows", "hot corner bottom left all windows"], "scan", Set(GALA, "hotcorner-bottomleft", T("window-overview-all"))),
    c("hot-corner-bottomleft-apps", "Bottom Left Hot Corner Applications", "Hot Corners", &["bottom left hot corner applications", "hot corner bottom left applications"], "scan", Set(GALA, "hotcorner-bottomleft", T("open-launcher"))),
    c("hot-corner-bottomleft-maximize", "Bottom Left Hot Corner Maximize", "Hot Corners", &["bottom left hot corner maximize", "hot corner bottom left maximize"], "scan", Set(GALA, "hotcorner-bottomleft", T("maximize-current"))),
    c("hot-corner-bottomleft-minimize", "Bottom Left Hot Corner Minimize", "Hot Corners", &["bottom left hot corner minimize", "hot corner bottom left minimize"], "scan", Set(GALA, "hotcorner-bottomleft", T("minimize-current"))),
    c("hot-corner-bottomright-off", "Bottom Right Hot Corner Off", "Hot Corners", &["bottom right hot corner off", "hot corner bottom right off"], "scan", Set(GALA, "hotcorner-bottomright", T("none"))),
    c("hot-corner-bottomright-multitasking", "Bottom Right Hot Corner Multitasking", "Hot Corners", &["bottom right hot corner multitasking", "hot corner bottom right multitasking"], "scan", Set(GALA, "hotcorner-bottomright", T("show-workspace-view"))),
    c("hot-corner-bottomright-windows", "Bottom Right Hot Corner All Windows", "Hot Corners", &["bottom right hot corner all windows", "hot corner bottom right all windows"], "scan", Set(GALA, "hotcorner-bottomright", T("window-overview-all"))),
    c("hot-corner-bottomright-apps", "Bottom Right Hot Corner Applications", "Hot Corners", &["bottom right hot corner applications", "hot corner bottom right applications"], "scan", Set(GALA, "hotcorner-bottomright", T("open-launcher"))),
    c("hot-corner-bottomright-maximize", "Bottom Right Hot Corner Maximize", "Hot Corners", &["bottom right hot corner maximize", "hot corner bottom right maximize"], "scan", Set(GALA, "hotcorner-bottomright", T("maximize-current"))),
    c("hot-corner-bottomright-minimize", "Bottom Right Hot Corner Minimize", "Hot Corners", &["bottom right hot corner minimize", "hot corner bottom right minimize"], "scan", Set(GALA, "hotcorner-bottomright", T("minimize-current"))),
    c("hot-corners-fullscreen", "Toggle Hot Corners in Fullscreen", "Hot Corners", &["hot corners in fullscreen"], "scan", Flip(GALA, "enable-hotcorners-in-fullscreen")),

    // Window Behavior
    c("behavior-edge-tiling", "Toggle Edge Tiling", "Window Behavior", &["edge tiling", "toggle edge tiling"], "app-window", Flip(MUTTER, "edge-tiling")),
    c("behavior-center-new", "Toggle Center New Windows", "Window Behavior", &["center new windows", "toggle center new windows"], "app-window", Flip(MUTTER, "center-new-windows")),
    c("behavior-attach-dialogs", "Toggle Attach Dialogs to Windows", "Window Behavior", &["attach dialogs to windows", "toggle attach dialogs to windows"], "app-window", Flip(MUTTER, "attach-modal-dialogs")),
    c("behavior-auto-maximize", "Toggle Auto Maximize", "Window Behavior", &["auto maximize", "toggle auto maximize"], "app-window", Flip(MUTTER, "auto-maximize")),
    c("behavior-focus-on-rest", "Toggle Focus When Pointer Rests", "Window Behavior", &["focus when pointer rests", "toggle focus when pointer rests"], "app-window", Flip(MUTTER, "focus-change-on-pointer-rest")),
    c("behavior-raise-on-click", "Toggle Raise Window on Click", "Window Behavior", &["raise window on click", "toggle raise window on click"], "app-window", Flip(WM_PREFS, "raise-on-click")),
    c("behavior-auto-raise", "Toggle Auto Raise Windows", "Window Behavior", &["auto raise windows", "toggle auto raise windows"], "app-window", Flip(WM_PREFS, "auto-raise")),
    c("behavior-right-resize", "Toggle Resize With Right Button", "Window Behavior", &["resize with right button", "toggle resize with right button"], "app-window", Flip(WM_PREFS, "resize-with-right-button")),
    c("focus-mode-click", "Focus Windows on Click", "Window Behavior", &["focus windows on click", "focus mode click", "focus follows click"], "mouse-pointer-click", Set(WM_PREFS, "focus-mode", T("click"))),
    c("focus-mode-sloppy", "Focus Windows on Hover", "Window Behavior", &["focus windows on hover", "focus mode hover", "focus follows hover"], "mouse-pointer-click", Set(WM_PREFS, "focus-mode", T("sloppy"))),
    c("focus-mode-mouse", "Focus Windows on Mouse", "Window Behavior", &["focus windows on mouse", "focus mode mouse", "focus follows mouse"], "mouse-pointer-click", Set(WM_PREFS, "focus-mode", T("mouse"))),
    c("titlebar-double-toggle-maximize", "Titlebar Double-Click Maximizes", "Window Behavior", &["titlebar double click maximizes", "titlebar double-click maximizes"], "app-window", Set(WM_PREFS, "action-double-click-titlebar", T("toggle-maximize"))),
    c("titlebar-double-minimize", "Titlebar Double-Click Minimizes", "Window Behavior", &["titlebar double click minimizes", "titlebar double-click minimizes"], "app-window", Set(WM_PREFS, "action-double-click-titlebar", T("minimize"))),
    c("titlebar-double-none", "Titlebar Double-Click Does Nothing", "Window Behavior", &["titlebar double click does nothing", "titlebar double-click does nothing"], "app-window", Set(WM_PREFS, "action-double-click-titlebar", T("none"))),
    c("titlebar-double-lower", "Titlebar Double-Click Lowers", "Window Behavior", &["titlebar double click lowers", "titlebar double-click lowers"], "app-window", Set(WM_PREFS, "action-double-click-titlebar", T("lower"))),
    c("titlebar-middle-toggle-maximize", "Titlebar Middle-Click Maximizes", "Window Behavior", &["titlebar middle click maximizes", "titlebar middle-click maximizes"], "app-window", Set(WM_PREFS, "action-middle-click-titlebar", T("toggle-maximize"))),
    c("titlebar-middle-minimize", "Titlebar Middle-Click Minimizes", "Window Behavior", &["titlebar middle click minimizes", "titlebar middle-click minimizes"], "app-window", Set(WM_PREFS, "action-middle-click-titlebar", T("minimize"))),
    c("titlebar-middle-none", "Titlebar Middle-Click Does Nothing", "Window Behavior", &["titlebar middle click does nothing", "titlebar middle-click does nothing"], "app-window", Set(WM_PREFS, "action-middle-click-titlebar", T("none"))),
    c("titlebar-middle-lower", "Titlebar Middle-Click Lowers", "Window Behavior", &["titlebar middle click lowers", "titlebar middle-click lowers"], "app-window", Set(WM_PREFS, "action-middle-click-titlebar", T("lower"))),

    // Network
    n(Needs::Network, "networking", "Toggle Networking", "Network", &["toggle networking", "all networking", "networking on", "networking off"], "network", Native(flip_networking)),
    n(Needs::MobileData, "mobile-data", "Toggle Mobile Broadband", "Network", &["mobile broadband", "mobile data", "wwan", "cellular"], "smartphone", Native(flip_mobile_data)),
    n(Needs::Network, "reconnect-wifi", "Reconnect Wi-Fi", "Network", &["reconnect wifi", "reconnect wi-fi", "reset wifi"], "refresh-cw", Later(reconnect_wifi)),
    n(Needs::Vpn, "vpn", "Toggle VPN", "Network", &["toggle vpn", "vpn on", "vpn off", "connect vpn", "disconnect vpn"], "shield", Later(flip_vpn)),
    n(Needs::Program("resolvectl"), "flush-dns", "Flush DNS Cache", "Network", &["flush dns", "clear dns", "dns cache"], "refresh-cw", Native(flush_dns)),
    c("proxy-off", "Turn Off Proxy", "Network", &["turn off proxy", "no proxy", "proxy off"], "globe", Set(PROXY, "mode", T("none"))),
    c("proxy-auto", "Use Automatic Proxy", "Network", &["automatic proxy", "proxy auto"], "globe", Set(PROXY, "mode", T("auto"))),
    c("location", "Toggle Location Services", "Privacy", &["location services", "toggle location", "gps"], "locate", Flip(LOCATION, "enabled")),

    // Bluetooth
    n(Needs::Bluetooth, "bluetooth-discoverable", "Toggle Bluetooth Discoverable", "Bluetooth", &["bluetooth discoverable", "make discoverable", "visible to devices"], "bluetooth", Native(flip_discoverable)),
    n(Needs::Bluetooth, "bluetooth-disconnect", "Disconnect Bluetooth Devices", "Bluetooth", &["disconnect bluetooth", "disconnect headphones", "disconnect bluetooth devices"], "bluetooth-off", Native(disconnect_bluetooth)),
    n(Needs::Bluetooth, "bluetooth-sharing", "Toggle Bluetooth File Sharing", "Bluetooth", &["bluetooth file sharing", "bluetooth sharing", "receive files bluetooth"], "bluetooth", Flip(BLUETOOTH_PREFS, "sharing")),
    n(Needs::Bluetooth, "bluetooth-confirm", "Toggle Confirm Bluetooth Files", "Bluetooth", &["confirm bluetooth files", "ask before accepting files"], "bluetooth", Flip(BLUETOOTH_PREFS, "confirm-accept-files")),

    // Notifications
    c("dnd-on", "Turn On Do Not Disturb", "Notifications", &["turn on do not disturb", "dnd on", "silence notifications"], "bell-off", Set(NOTIFICATIONS, "do-not-disturb", B(true))),
    c("dnd-off", "Turn Off Do Not Disturb", "Notifications", &["turn off do not disturb", "dnd off", "allow notifications"], "bell", Set(NOTIFICATIONS, "do-not-disturb", B(false))),

    // Keyboard
    n(Needs::SeveralLayouts, "layout-next", "Next Keyboard Layout", "Keyboard", &["next keyboard layout", "next layout", "switch keyboard layout", "next language"], "languages", Native(next_layout)),
    n(Needs::SeveralLayouts, "layout-previous", "Previous Keyboard Layout", "Keyboard", &["previous keyboard layout", "previous layout", "previous language"], "languages", Native(previous_layout)),
    c("key-repeat", "Toggle Key Repeat", "Keyboard", &["key repeat", "repeat keys", "hold key to repeat"], "keyboard", Flip(KEYBOARD, "repeat")),
    c("key-repeat-slow", "Key Repeat Speed Slow", "Keyboard", &["key repeat speed slow", "slow key repeat"], "keyboard", Set(KEYBOARD, "repeat-interval", V(60.0))),
    c("key-repeat-medium", "Key Repeat Speed Medium", "Keyboard", &["key repeat speed medium", "medium key repeat"], "keyboard", Set(KEYBOARD, "repeat-interval", V(30.0))),
    c("key-repeat-fast", "Key Repeat Speed Fast", "Keyboard", &["key repeat speed fast", "fast key repeat"], "keyboard", Set(KEYBOARD, "repeat-interval", V(15.0))),
    c("key-delay-short", "Key Repeat Delay Short", "Keyboard", &["key repeat delay short", "short key delay"], "keyboard", Set(KEYBOARD, "delay", V(250.0))),
    c("key-delay-medium", "Key Repeat Delay Medium", "Keyboard", &["key repeat delay medium", "medium key delay"], "keyboard", Set(KEYBOARD, "delay", V(500.0))),
    c("key-delay-long", "Key Repeat Delay Long", "Keyboard", &["key repeat delay long", "long key delay"], "keyboard", Set(KEYBOARD, "delay", V(800.0))),
    c("remember-numlock", "Toggle Remember Num Lock", "Keyboard", &["remember num lock", "num lock on startup"], "keyboard", Flip(KEYBOARD, "remember-numlock-state")),

    // Mouse
    c("mouse-left-handed", "Toggle Left-Handed Mouse", "Mouse", &["left handed mouse", "left-handed mouse", "swap mouse buttons"], "mouse", AskFlip(MOUSE, "left-handed", "Swap the left and right mouse buttons?", "Swap Buttons")),
    c("mouse-natural-scroll", "Toggle Mouse Natural Scrolling", "Mouse", &["mouse natural scrolling", "mouse reverse scroll", "mouse natural scroll"], "mouse", Flip(MOUSE, "natural-scroll")),
    c("mouse-middle-click", "Toggle Mouse Middle-Click Emulation", "Mouse", &["mouse middle click emulation", "two button middle click"], "mouse", Flip(MOUSE, "middle-click-emulation")),
    c("mouse-faster", "Mouse Speed Up", "Mouse", &["mouse speed up", "faster mouse", "mouse faster"], "mouse-pointer-2", Step(MOUSE, "speed", 0.1, -1.0, 1.0)),
    c("mouse-slower", "Mouse Speed Down", "Mouse", &["mouse speed down", "slower mouse", "mouse slower"], "mouse-pointer-2", Step(MOUSE, "speed", -0.1, -1.0, 1.0)),
    c("mouse-speed-default", "Mouse Speed Default", "Mouse", &["mouse speed default", "reset mouse speed"], "mouse-pointer-2", Set(MOUSE, "speed", V(0.0))),
    c("mouse-accel-default", "Mouse Acceleration Default", "Mouse", &["mouse acceleration default", "pointer acceleration default"], "mouse", Set(MOUSE, "accel-profile", T("default"))),
    c("mouse-accel-flat", "Mouse Acceleration Flat", "Mouse", &["mouse acceleration flat", "pointer acceleration flat"], "mouse", Set(MOUSE, "accel-profile", T("flat"))),
    c("mouse-accel-adaptive", "Mouse Acceleration Adaptive", "Mouse", &["mouse acceleration adaptive", "pointer acceleration adaptive"], "mouse", Set(MOUSE, "accel-profile", T("adaptive"))),
    c("double-click-fast", "Double-Click Speed Fast", "Mouse", &["double click speed fast", "double-click speed fast"], "mouse-pointer-click", Set(MOUSE, "double-click", V(250.0))),
    c("double-click-medium", "Double-Click Speed Medium", "Mouse", &["double click speed medium", "double-click speed medium"], "mouse-pointer-click", Set(MOUSE, "double-click", V(400.0))),
    c("double-click-slow", "Double-Click Speed Slow", "Mouse", &["double click speed slow", "double-click speed slow"], "mouse-pointer-click", Set(MOUSE, "double-click", V(700.0))),

    // Touchpad
    n(Needs::Touchpad, "touchpad", "Toggle Touchpad", "Touchpad", &["toggle touchpad", "touchpad on", "touchpad off", "disable touchpad", "enable touchpad"], "touchpad", Later(flip_touchpad)),
    n(Needs::Touchpad, "touchpad-external-mouse", "Turn Off Touchpad With External Mouse", "Touchpad", &["touchpad off with mouse", "disable touchpad with external mouse", "turn off touchpad with external mouse"], "touchpad-off", Set(TOUCHPAD, "send-events", T("disabled-on-external-mouse"))),
    n(Needs::Touchpad, "touchpad-tap", "Toggle Touchpad Tap to Click", "Touchpad", &["touchpad tap to click", "toggle touchpad tap to click", "tap to click"], "touchpad", Flip(TOUCHPAD, "tap-to-click")),
    n(Needs::Touchpad, "touchpad-tap-drag", "Toggle Touchpad Tap and Drag", "Touchpad", &["touchpad tap and drag", "toggle touchpad tap and drag", "tap and drag"], "touchpad", Flip(TOUCHPAD, "tap-and-drag")),
    n(Needs::Touchpad, "touchpad-drag-lock", "Toggle Touchpad Drag Lock", "Touchpad", &["touchpad drag lock", "toggle touchpad drag lock", "drag lock"], "touchpad", Flip(TOUCHPAD, "tap-and-drag-lock")),
    n(Needs::Touchpad, "touchpad-natural", "Toggle Touchpad Natural Scrolling", "Touchpad", &["touchpad natural scrolling", "toggle touchpad natural scrolling", "natural scrolling"], "touchpad", Flip(TOUCHPAD, "natural-scroll")),
    n(Needs::Touchpad, "touchpad-two-finger", "Toggle Touchpad Two-Finger Scrolling", "Touchpad", &["touchpad two-finger scrolling", "toggle touchpad two-finger scrolling", "two-finger scrolling"], "touchpad", Flip(TOUCHPAD, "two-finger-scrolling-enabled")),
    n(Needs::Touchpad, "touchpad-edge", "Toggle Touchpad Edge Scrolling", "Touchpad", &["touchpad edge scrolling", "toggle touchpad edge scrolling", "edge scrolling"], "touchpad", Flip(TOUCHPAD, "edge-scrolling-enabled")),
    n(Needs::Touchpad, "touchpad-typing", "Toggle Touchpad Off While Typing", "Touchpad", &["touchpad off while typing", "toggle touchpad off while typing", "off while typing"], "touchpad", Flip(TOUCHPAD, "disable-while-typing")),
    n(Needs::Touchpad, "touchpad-middle", "Toggle Touchpad Middle-Click Emulation", "Touchpad", &["touchpad middle-click emulation", "toggle touchpad middle-click emulation", "middle-click emulation"], "touchpad", Flip(TOUCHPAD, "middle-click-emulation")),
    n(Needs::Touchpad, "touchpad-click-default", "Touchpad Right-Click Default", "Touchpad", &["touchpad right click default", "touchpad click method default"], "touchpad", Set(TOUCHPAD, "click-method", T("default"))),
    n(Needs::Touchpad, "touchpad-click-areas", "Touchpad Right-Click Corners", "Touchpad", &["touchpad right click corners", "touchpad click method corners"], "touchpad", Set(TOUCHPAD, "click-method", T("areas"))),
    n(Needs::Touchpad, "touchpad-click-fingers", "Touchpad Right-Click Fingers", "Touchpad", &["touchpad right click fingers", "touchpad click method fingers"], "touchpad", Set(TOUCHPAD, "click-method", T("fingers"))),
    n(Needs::Touchpad, "touchpad-click-none", "Touchpad Right-Click Off", "Touchpad", &["touchpad right click off", "touchpad click method off"], "touchpad", Set(TOUCHPAD, "click-method", T("none"))),
    n(Needs::Touchpad, "touchpad-faster", "Touchpad Speed Up", "Touchpad", &["touchpad speed up", "faster touchpad"], "touchpad", Step(TOUCHPAD, "speed", 0.1, -1.0, 1.0)),
    n(Needs::Touchpad, "touchpad-slower", "Touchpad Speed Down", "Touchpad", &["touchpad speed down", "slower touchpad"], "touchpad", Step(TOUCHPAD, "speed", -0.1, -1.0, 1.0)),
    n(Needs::Touchpad, "touchpad-speed-default", "Touchpad Speed Default", "Touchpad", &["touchpad speed default", "reset touchpad speed"], "touchpad", Set(TOUCHPAD, "speed", V(0.0))),
    n(Needs::Touchpad, "touchpad-accel-default", "Touchpad Acceleration Default", "Touchpad", &["touchpad acceleration default"], "touchpad", Set(TOUCHPAD, "accel-profile", T("default"))),
    n(Needs::Touchpad, "touchpad-accel-flat", "Touchpad Acceleration Flat", "Touchpad", &["touchpad acceleration flat"], "touchpad", Set(TOUCHPAD, "accel-profile", T("flat"))),
    n(Needs::Touchpad, "touchpad-accel-adaptive", "Touchpad Acceleration Adaptive", "Touchpad", &["touchpad acceleration adaptive"], "touchpad", Set(TOUCHPAD, "accel-profile", T("adaptive"))),
    n(Needs::Touchpad, "touchpad-tap-map-default", "Touchpad Button Map Default", "Touchpad", &["touchpad button map default"], "touchpad", Set(TOUCHPAD, "tap-button-map", T("default"))),
    n(Needs::Touchpad, "touchpad-tap-map-lrm", "Touchpad Button Map Left Right Middle", "Touchpad", &["touchpad button map left right middle"], "touchpad", Set(TOUCHPAD, "tap-button-map", T("lrm"))),
    n(Needs::Touchpad, "touchpad-tap-map-lmr", "Touchpad Button Map Left Middle Right", "Touchpad", &["touchpad button map left middle right"], "touchpad", Set(TOUCHPAD, "tap-button-map", T("lmr"))),

    // Accessibility
    c("onscreen-keyboard", "Toggle On-Screen Keyboard", "Accessibility", &["on screen keyboard", "on-screen keyboard", "virtual keyboard", "osk"], "keyboard", Flip(A11Y_APPS, "screen-keyboard-enabled")),
    c("slow-keys", "Toggle Slow Keys", "Accessibility", &["slow keys"], "accessibility", AskFlip(A11Y_KEYS, "slowkeys-enable", "Turn on Slow Keys? Each key must then be held down to type.", "Turn On")),
    c("bounce-keys", "Toggle Bounce Keys", "Accessibility", &["bounce keys"], "accessibility", AskFlip(A11Y_KEYS, "bouncekeys-enable", "Turn on Bounce Keys? Quickly repeated key presses will be ignored.", "Turn On")),
    c("mouse-keys", "Toggle Mouse Keys", "Accessibility", &["mouse keys", "numpad mouse"], "accessibility", AskFlip(A11Y_KEYS, "mousekeys-enable", "Turn on Mouse Keys? The number pad will move the pointer.", "Turn On")),
    c("toggle-keys", "Toggle Lock Key Sounds", "Accessibility", &["toggle keys", "lock key sounds", "caps lock sound"], "accessibility", Flip(A11Y_KEYS, "togglekeys-enable")),

    // Privacy
    c("remember-recent", "Toggle Remember Recent Files", "Privacy", &["remember recent files", "file history", "recent files history"], "history", Flip(PRIVACY, "remember-recent-files")),
    c("camera-access", "Toggle Camera for Apps", "Privacy", &["camera for apps", "block camera", "camera access", "disable camera"], "camera", FlipOff(PRIVACY, "disable-camera")),
    c("microphone-access", "Toggle Microphone for Apps", "Privacy", &["microphone for apps", "block microphone", "microphone access", "disable microphone"], "mic", FlipOff(PRIVACY, "disable-microphone")),
    c("lock-on-suspend", "Toggle Lock on Sleep", "Privacy", &["lock on sleep", "lock on suspend"], "lock", Flip(LOCK_PREFS, "lock-on-suspend")),
    c("auto-lock", "Toggle Automatic Screen Lock", "Privacy", &["automatic screen lock", "auto lock", "lock when idle"], "lock", Flip(SCREENSAVER, "lock-enabled")),
    c("clear-recent", "Clear Recent Files", "Privacy", &["clear recent files", "clear file history", "forget recent files"], "brush-cleaning", Ask("Clear the list of recent files?", "Clear", clear_recent_files)),

    // Cleanup
    n(Needs::Program("gio"), "empty-trash", "Empty Trash", "Cleanup", &["empty trash", "clear trash", "empty bin", "empty recycle bin"], "trash-2", Ask("Permanently delete everything in the Trash?", "Empty Trash", empty_trash)),
    c("auto-clean-trash", "Toggle Auto-Delete Old Trash", "Cleanup", &["auto delete old trash", "clean up trash automatically"], "brush-cleaning", Flip(HOUSEKEEPING, "cleanup-trash-folder")),
    c("auto-clean-temp", "Toggle Auto-Delete Old Temporary Files", "Cleanup", &["auto delete old temporary files", "clean up temporary files automatically"], "brush-cleaning", Flip(HOUSEKEEPING, "cleanup-temp-folder")),
    c("auto-clean-downloads", "Toggle Auto-Delete Old Downloads", "Cleanup", &["auto delete old downloads", "clean up downloads automatically"], "brush-cleaning", AskFlip(HOUSEKEEPING, "cleanup-downloads-folder", "Automatically delete old downloads?", "Delete Old Files")),
    c("auto-clean-screenshots", "Toggle Auto-Delete Old Screenshots", "Cleanup", &["auto delete old screenshots", "clean up screenshots automatically"], "brush-cleaning", AskFlip(HOUSEKEEPING, "cleanup-screenshots-folder", "Automatically delete old screenshots?", "Delete Old Files")),
    c("auto-clean-age-7", "Delete Old Files After 7 Days", "Cleanup", &["delete old files after 7 days", "cleanup after 7 days"], "calendar-clock", Set(HOUSEKEEPING, "old-files-age", V(7.0))),
    c("auto-clean-age-30", "Delete Old Files After 30 Days", "Cleanup", &["delete old files after 30 days", "cleanup after 30 days"], "calendar-clock", Set(HOUSEKEEPING, "old-files-age", V(30.0))),
    c("auto-clean-age-90", "Delete Old Files After 90 Days", "Cleanup", &["delete old files after 90 days", "cleanup after 90 days"], "calendar-clock", Set(HOUSEKEEPING, "old-files-age", V(90.0))),

    // Updates
    c("auto-system-updates", "Toggle Automatic System Updates", "Updates", &["automatic system updates", "auto updates system"], "download", Flip(SYSTEM_UPDATE, "automatic-updates")),
    c("auto-app-updates", "Toggle Automatic App Updates", "Updates", &["automatic app updates", "auto update apps", "appcenter updates"], "store", Flip(APPCENTER, "automatic-updates")),

    // Date & Time
    c("auto-timezone", "Toggle Automatic Time Zone", "Date & Time", &["automatic time zone", "auto timezone", "automatic timezone"], "globe", Flip(DATETIME, "automatic-timezone")),
    c("auto-time", "Toggle Automatic Date & Time", "Date & Time", &["automatic date and time", "network time", "ntp"], "clock", Native(flip_network_time)),

    // Files
    c("files-hidden", "Toggle Hidden Files", "Files", &["hidden files", "toggle hidden files", "files hidden files"], "file-cog", Flip(FILES, "show-hiddenfiles")),
    c("files-preview", "Toggle File Previews", "Files", &["file previews", "toggle file previews", "files file previews"], "file-cog", Flip(FILES, "show-file-preview")),
    c("files-folders-first", "Toggle Folders First", "Files", &["folders first", "toggle folders first", "files folders first"], "file-cog", Flip(FILES, "sort-directories-first")),
    c("files-single-click", "Toggle Single-Click to Select", "Files", &["single-click to select", "toggle single-click to select", "files single-click to select"], "file-cog", Flip(FILES, "singleclick-select")),
    c("files-restore-tabs", "Toggle Restore Tabs", "Files", &["restore tabs", "toggle restore tabs", "files restore tabs"], "file-cog", Flip(FILES, "restore-tabs")),
    c("files-thumbnails", "Toggle Local Thumbnails", "Files", &["local thumbnails", "toggle local thumbnails", "files local thumbnails"], "file-cog", Flip(FILES, "show-local-thumbnails")),
    c("files-remote-thumbnails", "Toggle Remote Thumbnails", "Files", &["remote thumbnails", "toggle remote thumbnails", "files remote thumbnails"], "file-cog", Flip(FILES, "show-remote-thumbnails")),
    c("files-view-grid", "Files View Grid", "Files", &["files view grid", "file manager grid view"], "folder-cog", Set(FILES, "default-viewmode", T("icon"))),
    c("files-view-list", "Files View List", "Files", &["files view list", "file manager list view"], "folder-cog", Set(FILES, "default-viewmode", T("list"))),
    c("files-view-columns", "Files View Columns", "Files", &["files view columns", "file manager columns view"], "folder-cog", Set(FILES, "default-viewmode", T("miller_columns"))),
    c("files-date-iso", "Files Date Format ISO", "Files", &["files date format iso"], "calendar-days", Set(FILES, "date-format", T("iso"))),
    c("files-date-locale", "Files Date Format Locale", "Files", &["files date format locale"], "calendar-days", Set(FILES, "date-format", T("locale"))),
    c("files-date-informal", "Files Date Format Informal", "Files", &["files date format informal"], "calendar-days", Set(FILES, "date-format", T("informal"))),
    c("files-date-compact", "Files Date Format Compact", "Files", &["files date format compact"], "calendar-days", Set(FILES, "date-format", T("compact"))),

    // Folders
    c("folder-home", "Open Home Folder", "Folders", &["home folder", "open home"], "house", Folder(Special(None))),
    c("folder-desktop", "Open Desktop Folder", "Folders", &["desktop folder"], "monitor", Folder(Special(Some(glib::UserDirectory::Desktop)))),
    c("folder-documents", "Open Documents", "Folders", &["documents", "documents folder"], "file-text", Folder(Special(Some(glib::UserDirectory::Documents)))),
    c("folder-downloads", "Open Downloads", "Folders", &["downloads", "downloads folder"], "download", Folder(Special(Some(glib::UserDirectory::Downloads)))),
    c("folder-music", "Open Music Folder", "Folders", &["music folder"], "music", Folder(Special(Some(glib::UserDirectory::Music)))),
    c("folder-pictures", "Open Pictures", "Folders", &["pictures", "pictures folder", "photos folder"], "image", Folder(Special(Some(glib::UserDirectory::Pictures)))),
    c("folder-videos", "Open Videos", "Folders", &["videos", "videos folder"], "video", Folder(Special(Some(glib::UserDirectory::Videos)))),
    c("folder-public", "Open Public Folder", "Folders", &["public folder"], "users", Folder(Special(Some(glib::UserDirectory::PublicShare)))),
    c("folder-templates", "Open Templates", "Folders", &["templates", "templates folder"], "file-text", Folder(Special(Some(glib::UserDirectory::Templates)))),
    c("folder-screenshots", "Open Screenshots Folder", "Folders", &["screenshots folder", "open screenshots"], "camera", Folder(InPictures("Screenshots"))),
    c("folder-trash", "Open Trash", "Folders", &["trash", "open trash", "recycle bin", "deleted files"], "trash", Folder(Uri("trash:///"))),
    c("folder-recent", "Open Recent Files", "Folders", &["recent files", "open recent"], "history", Folder(Uri("recent:///"))),
    c("folder-network", "Open Network Places", "Folders", &["network places", "network folder"], "network", Folder(Uri("network:///"))),
    c("folder-computer", "Open File System", "Folders", &["file system", "root folder", "computer"], "hard-drive", Folder(Uri("file:///"))),
    c("folder-config", "Open Config Folder", "Folders", &["config folder", "dot config"], "folder-cog", Folder(InHome(".config"))),
    c("folder-local-share", "Open App Data Folder", "Folders", &["app data folder", "local share"], "database", Folder(InHome(".local/share"))),
    c("folder-cache", "Open Cache Folder", "Folders", &["cache folder"], "archive", Folder(InHome(".cache"))),
    c("folder-applications", "Open Applications Folder", "Folders", &["applications folder"], "boxes", Folder(Uri("file:///usr/share/applications"))),
    c("folder-fonts", "Open Fonts Folder", "Folders", &["fonts folder"], "type", Folder(InHome(".local/share/fonts"))),
    c("folder-drives", "Open Mounted Drives", "Folders", &["mounted drives", "usb drives", "external drives"], "usb", Folder(Uri("file:///media"))),
    c("folder-temp", "Open Temporary Folder", "Folders", &["temp folder", "tmp folder", "temporary folder"], "folder-clock", Folder(Uri("file:///tmp"))),

    // Screenshots
    c("screenshot", "Take Screenshot", "Screenshots", &["take screenshot", "screenshot", "screen capture", "capture screen", "print screen"], "camera", Later(take_screenshot)),
    n(Needs::Gala, "screenshot-window", "Screenshot Window", "Screenshots", &["screenshot window", "capture window"], "camera", Gala(18)),
    n(Needs::ScreenshotTool, "screenshot-tool", "Open Screenshot Tool", "Screenshots", &["screenshot tool", "screenshot area", "capture area", "select area"], "scissors", Later(open_screenshot_tool)),
];

pub fn all() -> impl Iterator<Item = &'static QuickAction> {
    COMMANDS.iter().map(|c| &c.action)
}

fn find(id: &str) -> Option<&'static SystemCommand> {
    COMMANDS.iter().find(|c| c.action.id == id)
}

pub fn is_available(id: &str) -> bool {
    find(id).is_some_and(|command| {
        settings_exist(command.run) && command.needs.is_none_or(|needs| capabilities().has(needs))
    })
}

/// Runs the command. What waits for Spotlight to close, or for an answer,
/// carries on in the background and reports on the on-screen display.
pub fn run(id: &str) -> Result<(), String> {
    let command = find(id).ok_or_else(|| format!("Unknown command \"{id}\"."))?;
    log::info!("system command {id}");
    if matches!(command.run, Gala(_) | Wm(..) | GoTo(_) | MoveTo(_) | Menu(_) | Later(_) | Ask(..) | AskFlip(..)) {
        std::thread::spawn(move || {
            std::thread::sleep(SPOTLIGHT_GONE);
            let _ = finish(command, perform(command.run));
        });
        return Ok(());
    }
    finish(command, perform(command.run))
}

fn finish(command: &SystemCommand, done: Done) -> Result<(), String> {
    match done {
        Ok(Some(level)) => {
            show(osd_icon(command.action.icon), level.clamp(0.0, 1.0));
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(e) => {
            log::warn!("{} failed: {e}", command.action.id);
            show("dialog-error-symbolic", 0.0);
            Err(format!("{}: {e}", command.action.title))
        }
    }
}

fn perform(run: Run) -> Done {
    match run {
        Flip(schema, key) => shown(flip_key(schema, key)?),
        FlipOff(schema, key) => shown(!flip_key(schema, key)?),
        AskFlip(schema, key, question, button) => {
            let settings = required(schema, key)?;
            if !settings.boolean(key) && !ask(question, button) {
                return Ok(None);
            }
            shown(flip_key(schema, key)?)
        }
        Swap(schema, key, on, off) => {
            let settings = required(schema, key)?;
            let turn_on = settings.string(key) != on;
            write(schema, key, T(if turn_on { on } else { off }))?;
            shown(turn_on)
        }
        Set(schema, key, value) => {
            write(schema, key, value)?;
            Ok(Some(1.0))
        }
        Both(keys) => {
            for (schema, key, value) in keys {
                write(schema, key, *value)?;
            }
            Ok(Some(1.0))
        }
        Step(schema, key, by, from, to) => step(schema, key, by, from, to),
        Gala(action) => {
            call(&gala::session_bus()?, GALA_BUS, "/org/pantheon/gala", GALA_BUS, "PerformAction", Some((action,).to_variant()))?;
            Ok(None)
        }
        Wm(schema, key) => press_wm_shortcut(schema, key),
        GoTo(index) => go_to_workspace(index),
        MoveTo(index) => move_window_to_workspace(index),
        Menu(code) => toggle_menu(code),
        Folder(place) => open_place(place),
        Volume(target, change) => change_volume(target, change),
        VolumeTo(target, level) => set_volume(target, level),
        Mute(target) => flip_mute(target),
        NextDevice(section) => next_device(section),
        Media(method) => {
            let bus = gala::session_bus()?;
            let player = current_player(&bus)?;
            call(&bus, &player, MPRIS_PATH, MPRIS_PLAYER, method, None)?;
            Ok(None)
        }
        Seek(seconds) => {
            let bus = gala::session_bus()?;
            let player = current_player(&bus)?;
            call(&bus, &player, MPRIS_PATH, MPRIS_PLAYER, "Seek", Some((seconds * 1_000_000,).to_variant()))?;
            Ok(None)
        }
        Screen(method) => {
            let reply = call(&gala::session_bus()?, GSD_POWER, GSD_POWER_PATH, GSD_SCREEN, method, None)?;
            Ok(reply.child_value(0).get::<i32>().map(|p| f64::from(p) / 100.0))
        }
        BrightnessTo(percent) => {
            set(&gala::session_bus()?, GSD_POWER, GSD_POWER_PATH, GSD_SCREEN, "Brightness", percent.to_variant())?;
            Ok(Some(f64::from(percent) / 100.0))
        }
        KeyboardLight(method) => {
            let reply = call(&gala::session_bus()?, GSD_POWER, GSD_POWER_PATH, GSD_KEYBOARD, method, None)?;
            Ok(reply.child_value(0).get::<i32>().map(|p| f64::from(p) / 100.0))
        }
        Profile(profile) => {
            set(&system_bus()?, POWER_PROFILES, POWER_PROFILES_PATH, POWER_PROFILES, "ActiveProfile", profile.to_variant())?;
            Ok(Some(1.0))
        }
        Accent(color) => set_accent(color),
        Native(action) | Later(action) => action(),
        Ask(question, button, action) => {
            if ask(question, button) {
                action()
            } else {
                Ok(None)
            }
        }
    }
}

/// A full bar for on, an empty one for off.
fn shown(on: bool) -> Done {
    Ok(Some(if on { 1.0 } else { 0.0 }))
}

/// The desktop's symbolic icon closest to the command's own.
fn osd_icon(icon: &str) -> &'static str {
    match icon {
        "volume-2" | "volume-1" | "audio-lines" => "audio-volume-high-symbolic",
        "volume-x" => "audio-volume-muted-symbolic",
        "mic" => "audio-input-microphone-symbolic",
        "mic-off" => "microphone-sensitivity-muted-symbolic",
        "headphones" => "audio-headphones-symbolic",
        "sun" | "sun-dim" => "display-brightness-symbolic",
        "moon" | "moon-star" => "weather-clear-night-symbolic",
        "sun-moon" => "weather-clear-symbolic",
        "keyboard" | "languages" => "input-keyboard-symbolic",
        "mouse" | "mouse-pointer-2" | "mouse-pointer-click" | "locate" => "input-mouse-symbolic",
        "touchpad" | "touchpad-off" => "input-touchpad-symbolic",
        "bluetooth" => "bluetooth-active-symbolic",
        "bluetooth-off" => "bluetooth-disabled-symbolic",
        "wifi" | "network" => "network-wireless-symbolic",
        "bell" | "bell-ring" => "notification-symbolic",
        "bell-off" => "notification-disabled-symbolic",
        "battery" | "battery-charging" | "gauge" => "battery-symbolic",
        "camera" => "camera-photo-symbolic",
        "lock" => "system-lock-screen-symbolic",
        "monitor" | "monitor-off" => "video-display-symbolic",
        "clock" | "timer" | "calendar-days" | "calendar-clock" => "preferences-system-time-symbolic",
        "image" | "images" | "palette" | "contrast" => "applications-graphics-symbolic",
        "accessibility" => "preferences-desktop-accessibility-symbolic",
        "trash" | "trash-2" | "brush-cleaning" => "user-trash-symbolic",
        "type" | "a-large-small" => "preferences-desktop-font-symbolic",
        "shuffle" => "media-playlist-shuffle-symbolic",
        "repeat" => "media-playlist-repeat-symbolic",
        "coffee" => "preferences-system-time-symbolic",
        "globe" | "shield" | "refresh-cw" | "smartphone" => "network-wired-symbolic",
        _ => "preferences-system-symbolic",
    }
}

// Settings

fn settings_exist(run: Run) -> bool {
    let key = |schema: &str, key: &str| {
        let schema = gio::SettingsSchemaSource::default()?.lookup(schema, true)?;
        schema.has_key(key).then(|| schema.key(key))
    };
    let is_bool = |schema, name| key(schema, name).is_some_and(|k| k.value_type().as_str() == "b");
    let accepts = |schema, name, value| {
        key(schema, name).is_some_and(|k| as_variant(k.value_type().as_str(), value).is_some_and(|v| k.range_check(&v)))
    };
    match run {
        Flip(schema, name) | FlipOff(schema, name) | AskFlip(schema, name, ..) => is_bool(schema, name),
        Swap(schema, name, a, b) => accepts(schema, name, T(a)) && accepts(schema, name, T(b)),
        Set(schema, name, value) => accepts(schema, name, value),
        Both(keys) => keys.iter().all(|(schema, name, value)| accepts(schema, name, *value)),
        Step(schema, name, ..) => key(schema, name).is_some_and(|k| matches!(k.value_type().as_str(), "i" | "u" | "d")),
        Wm(schema, name) => key(schema, name).is_some_and(|k| k.value_type().as_str() == "as"),
        _ => true,
    }
}

fn as_variant(kind: &str, value: Value) -> Option<glib::Variant> {
    match (kind, value) {
        ("s", T(text)) => Some(text.to_variant()),
        ("b", B(on)) => Some(on.to_variant()),
        ("i", V(number)) => Some((number as i32).to_variant()),
        ("u", V(number)) => Some((number as u32).to_variant()),
        ("d", V(number)) => Some(number.to_variant()),
        _ => None,
    }
}

fn write(schema: &str, key: &str, value: Value) -> Result<(), String> {
    let settings = required(schema, key)?;
    let kind = settings.value(key).type_().as_str().to_string();
    let value = as_variant(&kind, value).ok_or_else(|| format!("{key} does not take that value"))?;
    settings.set_value(key, &value).map_err(|e| e.to_string())?;
    gio::Settings::sync();
    Ok(())
}

fn step(schema: &str, key: &str, by: f64, from: f64, to: f64) -> Done {
    let settings = required(schema, key)?;
    let value = settings.value(key);
    let current = value
        .get::<f64>()
        .or_else(|| value.get::<i32>().map(f64::from))
        .or_else(|| value.get::<u32>().map(f64::from))
        .ok_or_else(|| format!("{key} is not a number"))?;
    // in hundredths, or 0.1 + 0.2 slowly drifts away from the values people pick
    let next = (((current + by) * 100.0).round() / 100.0).clamp(from, to);
    write(schema, key, V(next))?;
    Ok(Some((next - from) / (to - from)))
}

// Asking first

static ASKING: AtomicBool = AtomicBool::new(false);

/// Whether the person confirmed. Cancel is the default answer, closing the
/// dialog counts as Cancel, and only one question is open at a time.
fn ask(question: &'static str, button: &'static str) -> bool {
    if ASKING.swap(true, Ordering::SeqCst) {
        return false;
    }
    let (tx, rx) = mpsc::channel();
    glib::MainContext::default().invoke(move || {
        use gtk::prelude::*;
        let dialog = gtk::MessageDialog::new(
            None::<&gtk::Window>,
            gtk::DialogFlags::empty(),
            gtk::MessageType::Warning,
            gtk::ButtonsType::None,
            question,
        );
        dialog.set_title("Lummo");
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        if let Some(confirm) = dialog.add_button(button, gtk::ResponseType::Accept).downcast_ref::<gtk::Button>() {
            confirm.style_context().add_class("destructive-action");
        }
        dialog.set_default_response(gtk::ResponseType::Cancel);
        dialog.set_position(gtk::WindowPosition::Center);
        dialog.set_keep_above(true);
        let answered = std::cell::Cell::new(false);
        dialog.connect_response(move |dialog, response| {
            if !answered.replace(true) {
                let _ = tx.send(response == gtk::ResponseType::Accept);
                dialog.close();
            }
        });
        dialog.present();
    });
    let answer = rx.recv().unwrap_or(false);
    ASKING.store(false, Ordering::SeqCst);
    answer
}

// D-Bus and programs

fn call(
    bus: &gio::DBusConnection,
    name: &str,
    path: &str,
    iface: &str,
    method: &str,
    args: Option<glib::Variant>,
) -> Result<glib::Variant, String> {
    bus.call_sync(
        Some(name),
        path,
        iface,
        method,
        args.as_ref(),
        None,
        gio::DBusCallFlags::ALLOW_INTERACTIVE_AUTHORIZATION,
        CALL_TIMEOUT_MS,
        gio::Cancellable::NONE,
    )
    .map_err(|e| e.to_string())
}

fn tool(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program).args(args).output().map_err(|e| format!("{program}: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(format!("{program}: {}", error.trim()))
    }
}

fn has_program(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
}

// What this computer has

#[derive(Default)]
struct Capabilities {
    suspend: bool,
    hibernate: bool,
    hybrid_sleep: bool,
    suspend_then_hibernate: bool,
    firmware: bool,
    seat: bool,
    profiles: Vec<String>,
    battery: bool,
    light_sensor: bool,
    sound: bool,
    brightness: bool,
    keyboard_light: bool,
    panel: bool,
    gala: bool,
    remote_desktop: bool,
    network: bool,
    mobile_data: bool,
    vpn: bool,
    bluetooth: bool,
    several_layouts: bool,
    touchpad: bool,
    screenshot_tool: bool,
}

impl Capabilities {
    fn has(&self, needs: Needs) -> bool {
        match needs {
            Needs::Suspend => self.suspend,
            Needs::Hibernate => self.hibernate,
            Needs::HybridSleep => self.hybrid_sleep,
            Needs::SuspendThenHibernate => self.suspend_then_hibernate,
            Needs::Firmware => self.firmware,
            Needs::Seat => self.seat,
            Needs::Profile(profile) => self.profiles.iter().any(|p| p == profile),
            Needs::Battery => self.battery,
            Needs::LightSensor => self.light_sensor,
            Needs::Sound => self.sound,
            Needs::Brightness => self.brightness,
            Needs::KeyboardLight => self.keyboard_light,
            Needs::Panel => self.panel,
            Needs::Gala => self.gala,
            Needs::RemoteDesktop => self.remote_desktop,
            Needs::Network => self.network,
            Needs::MobileData => self.mobile_data,
            Needs::Vpn => self.vpn,
            Needs::Bluetooth => self.bluetooth,
            Needs::SeveralLayouts => self.several_layouts,
            Needs::Touchpad => self.touchpad,
            Needs::ScreenshotTool => self.screenshot_tool,
            Needs::Program(name) => has_program(name),
        }
    }

    fn read() -> Self {
        let session = gala::session_bus().ok();
        let system = system_bus().ok();
        let on_session = |name: &str| session.as_ref().is_some_and(|bus| gala::name_has_owner(bus, name));
        let can = |method: &str| {
            system
                .as_ref()
                .and_then(|bus| call(bus, LOGIN, LOGIN_PATH, LOGIN_MANAGER, method, None).ok())
                .and_then(|reply| reply.child_value(0).get::<String>())
                .is_some_and(|answer| answer == "yes" || answer == "challenge")
        };
        let profiles = system
            .as_ref()
            .and_then(|bus| get(bus, POWER_PROFILES, POWER_PROFILES_PATH, POWER_PROFILES, "Profiles").ok())
            .map(|list| {
                (0..list.n_children())
                    .filter_map(|i| {
                        glib::VariantDict::new(Some(&list.child_value(i))).lookup_value("Profile", None)?.get::<String>()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let brightness = session
            .as_ref()
            .and_then(|bus| get(bus, GSD_POWER, GSD_POWER_PATH, GSD_SCREEN, "Brightness").ok())
            .and_then(|v| v.get::<i32>())
            .is_some_and(|percent| percent >= 0);
        let keyboard_light = session
            .as_ref()
            .and_then(|bus| get(bus, GSD_POWER, GSD_POWER_PATH, GSD_KEYBOARD, "Brightness").ok())
            .and_then(|v| v.get::<i32>())
            .is_some_and(|percent| percent >= 0);
        let network = system.as_ref().is_some_and(|bus| gala::name_has_owner(bus, NETWORK_MANAGER));
        let mobile_data = network
            && system
                .as_ref()
                .is_some_and(|bus| get_bool(bus, NETWORK_MANAGER, NETWORK_MANAGER_PATH, NETWORK_MANAGER, "WwanHardwareEnabled").unwrap_or(false))
            && tool("nmcli", &["-t", "-f", "TYPE", "device"]).is_ok_and(|devices| devices.lines().any(|t| t == "gsm" || t == "cdma"));
        let vpn = network
            && tool("nmcli", &["-t", "-f", "TYPE", "connection", "show"])
                .is_ok_and(|kinds| kinds.lines().any(is_vpn));
        let bluetooth = system.as_ref().is_some_and(|bus| !bluetooth_objects(bus, BLUEZ_ADAPTER).unwrap_or_default().is_empty());
        let several_layouts = settings(INPUT_SOURCES, "sources").is_some_and(|s| s.value("sources").n_children() > 1);
        let battery = std::fs::read_dir("/sys/class/power_supply").is_ok_and(|entries| {
            entries.flatten().any(|e| {
                std::fs::read_to_string(e.path().join("type")).is_ok_and(|kind| kind.trim() == "Battery")
                    && std::fs::read_to_string(e.path().join("scope")).map_or(true, |scope| scope.trim() != "Device")
            })
        });
        let touchpad = std::fs::read_to_string("/proc/bus/input/devices").is_ok_and(|devices| has_touchpad(&devices));
        let firmware = session
            .as_ref()
            .and_then(|bus| call(bus, SESSION_MANAGER, SESSION_MANAGER_PATH, SESSION_MANAGER, "CanRebootToFirmwareSetup", None).ok())
            .and_then(|reply| reply.child_value(0).get::<bool>())
            .unwrap_or(false);
        Self {
            suspend: can("CanSuspend"),
            hibernate: can("CanHibernate"),
            hybrid_sleep: can("CanHybridSleep"),
            suspend_then_hibernate: can("CanSuspendThenHibernate"),
            firmware,
            seat: std::env::var_os("XDG_SEAT_PATH").is_some(),
            profiles,
            battery,
            light_sensor: system.as_ref().is_some_and(|bus| gala::name_has_owner(bus, "net.hadess.SensorProxy")),
            sound: has_program("wpctl"),
            brightness,
            keyboard_light,
            panel: on_session(WINGPANEL),
            gala: on_session(GALA_BUS),
            remote_desktop: on_session(REMOTE_DESKTOP),
            network,
            mobile_data,
            vpn,
            bluetooth,
            several_layouts,
            touchpad,
            screenshot_tool: gio::DesktopAppInfo::new(SCREENSHOT_TOOL).is_some(),
        }
    }
}

fn capabilities() -> Arc<Capabilities> {
    static CACHE: Mutex<Option<(Instant, Arc<Capabilities>)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    match cache.as_ref() {
        Some((read_at, capabilities)) if read_at.elapsed() < CAPABILITIES_FOR => capabilities.clone(),
        _ => {
            let capabilities = Arc::new(Capabilities::read());
            *cache = Some((Instant::now(), capabilities.clone()));
            capabilities
        }
    }
}

fn has_touchpad(input_devices: &str) -> bool {
    input_devices.lines().any(|line| {
        let line = line.to_lowercase();
        line.starts_with("n: name=") && (line.contains("touchpad") || line.contains("trackpad"))
    })
}

fn is_vpn(kind: &str) -> bool {
    kind == "vpn" || kind == "wireguard"
}

// Session and power

fn session_call(method: &str, args: Option<glib::Variant>) -> Done {
    call(&gala::session_bus()?, SESSION_MANAGER, SESSION_MANAGER_PATH, SESSION_MANAGER, method, args)?;
    Ok(None)
}

fn login_call(method: &str) -> Done {
    call(&system_bus()?, LOGIN, LOGIN_PATH, LOGIN_MANAGER, method, Some((true,).to_variant()))?;
    Ok(None)
}

fn lock() -> Done {
    call(&gala::session_bus()?, "org.gnome.ScreenSaver", "/org/gnome/ScreenSaver", "org.gnome.ScreenSaver", "Lock", None)?;
    Ok(None)
}

fn suspend() -> Done {
    login_call("Suspend")
}

fn hibernate() -> Done {
    login_call("Hibernate")
}

fn hybrid_sleep() -> Done {
    login_call("HybridSleep")
}

fn suspend_then_hibernate() -> Done {
    login_call("SuspendThenHibernate")
}

// The session manager lets apps with unsaved work object before it ends the session.
fn shut_down() -> Done {
    session_call("RequestShutdown", None)
}

fn restart() -> Done {
    session_call("RequestReboot", None)
}

fn log_out() -> Done {
    // 1: no second confirmation
    session_call("Logout", Some((1u32,).to_variant()))
}

fn restart_to_firmware() -> Done {
    session_call("SetRebootToFirmwareSetup", Some((true,).to_variant()))?;
    session_call("RequestReboot", None)
}

fn switch_user() -> Done {
    let seat = std::env::var("XDG_SEAT_PATH").map_err(|_| "this session has no seat".to_string())?;
    call(&system_bus()?, "org.freedesktop.DisplayManager", &seat, "org.freedesktop.DisplayManager.Seat", "SwitchToGreeter", None)?;
    Ok(None)
}

/// While set, the session neither idles nor suspends. The session manager
/// drops it by itself if Lummo quits.
static KEEP_AWAKE: Mutex<Option<u32>> = Mutex::new(None);
const INHIBIT_SUSPEND_AND_IDLE: u32 = 4 | 8;

fn flip_keep_awake() -> Done {
    let bus = gala::session_bus()?;
    let mut cookie = KEEP_AWAKE.lock().unwrap_or_else(|e| e.into_inner());
    match cookie.take() {
        Some(held) => {
            call(&bus, SESSION_MANAGER, SESSION_MANAGER_PATH, SESSION_MANAGER, "Uninhibit", Some((held,).to_variant()))?;
            shown(false)
        }
        None => {
            let args = ("lummo", 0u32, "Keep Awake is on", INHIBIT_SUSPEND_AND_IDLE).to_variant();
            let reply = call(&bus, SESSION_MANAGER, SESSION_MANAGER_PATH, SESSION_MANAGER, "Inhibit", Some(args))?;
            *cookie = Some(reply.child_value(0).get::<u32>().ok_or("no inhibitor was returned")?);
            shown(true)
        }
    }
}

// Sound

fn parse_volume(output: &str) -> Option<(f64, bool)> {
    let mut words = output.split_whitespace();
    (words.next()? == "Volume:").then_some(())?;
    Some((words.next()?.parse().ok()?, output.contains("[MUTED]")))
}

fn read_volume(target: &str) -> Result<(f64, bool), String> {
    parse_volume(&tool("wpctl", &["get-volume", target])?).ok_or_else(|| "could not read the volume".to_string())
}

/// 100%, or the system's 150% where volume above 100% is allowed.
fn volume_limit() -> f64 {
    let allowed = settings(SOUND, "allow-volume-above-100-percent").is_some_and(|s| s.boolean("allow-volume-above-100-percent"));
    if allowed {
        1.5
    } else {
        1.0
    }
}

fn change_volume(target: &str, change: &str) -> Done {
    if change.ends_with('+') {
        tool("wpctl", &["set-mute", target, "0"])?;
    }
    let limit = volume_limit();
    tool("wpctl", &["set-volume", "-l", &limit.to_string(), target, change])?;
    let (volume, muted) = read_volume(target)?;
    Ok(Some(if muted { 0.0 } else { volume / limit }))
}

fn set_volume(target: &str, level: f64) -> Done {
    tool("wpctl", &["set-mute", target, "0"])?;
    tool("wpctl", &["set-volume", target, &level.to_string()])?;
    Ok(Some(level))
}

fn flip_mute(target: &str) -> Done {
    tool("wpctl", &["set-mute", target, "toggle"])?;
    let (volume, muted) = read_volume(target)?;
    Ok(Some(if muted { 0.0 } else { volume / volume_limit() }))
}

/// The ids in a section ("Sinks:") of `wpctl status`'s Audio part, and which is the default.
fn audio_devices(status: &str, section: &str) -> Vec<(u32, bool)> {
    let audio = status.split_once("\nAudio\n").map_or("", |(_, rest)| rest);
    let audio = audio.split("\nVideo\n").next().unwrap_or("");
    let clean = |line: &str| line.trim_start_matches(['│', '├', '└', '─', ' ']).trim_end().to_string();
    audio
        .lines()
        .map(clean)
        .skip_while(|line| line != section)
        .skip(1)
        .take_while(|line| !line.is_empty() && !line.ends_with(':'))
        .filter_map(|line| {
            let default = line.starts_with('*');
            let id = line.trim_start_matches(['*', ' ']).split('.').next()?.parse().ok()?;
            Some((id, default))
        })
        .collect()
}

/// The device after the default one, going round.
fn following(devices: &[(u32, bool)]) -> Option<u32> {
    if devices.len() < 2 {
        return None;
    }
    let current = devices.iter().position(|(_, default)| *default).unwrap_or(0);
    Some(devices[(current + 1) % devices.len()].0)
}

fn next_device(section: &str) -> Done {
    let devices = audio_devices(&tool("wpctl", &["status"])?, section);
    let next = following(&devices).ok_or("there is no other device to switch to")?;
    tool("wpctl", &["set-default", &next.to_string()])?;
    Ok(Some(1.0))
}

// Media

fn players(bus: &gio::DBusConnection) -> Result<Vec<String>, String> {
    let reply = call(bus, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "ListNames", None)?;
    let names = reply.child_value(0).get::<Vec<String>>().ok_or("the bus did not list its names")?;
    Ok(names.into_iter().filter(|name| name.starts_with(MPRIS_PREFIX)).collect())
}

/// The player that is playing, else the first one open.
fn current_player(bus: &gio::DBusConnection) -> Result<String, String> {
    let players = players(bus)?;
    let playing = players.iter().find(|player| {
        get(bus, player, MPRIS_PATH, MPRIS_PLAYER, "PlaybackStatus").ok().and_then(|v| v.get::<String>()).as_deref() == Some("Playing")
    });
    playing.or(players.first()).cloned().ok_or_else(|| "no media player is open".to_string())
}

fn flip_shuffle() -> Done {
    let bus = gala::session_bus()?;
    let player = current_player(&bus)?;
    let on = !get_bool(&bus, &player, MPRIS_PATH, MPRIS_PLAYER, "Shuffle")?;
    set(&bus, &player, MPRIS_PATH, MPRIS_PLAYER, "Shuffle", on.to_variant())?;
    shown(on)
}

fn next_loop_status(current: &str) -> &'static str {
    match current {
        "None" => "Playlist",
        "Playlist" => "Track",
        _ => "None",
    }
}

fn cycle_repeat() -> Done {
    let bus = gala::session_bus()?;
    let player = current_player(&bus)?;
    let current = get(&bus, &player, MPRIS_PATH, MPRIS_PLAYER, "LoopStatus")?.get::<String>().unwrap_or_default();
    let next = next_loop_status(&current);
    set(&bus, &player, MPRIS_PATH, MPRIS_PLAYER, "LoopStatus", next.to_variant())?;
    Ok(Some(match next {
        "None" => 0.0,
        "Playlist" => 0.5,
        _ => 1.0,
    }))
}

fn raise_player() -> Done {
    let bus = gala::session_bus()?;
    let player = current_player(&bus)?;
    call(&bus, &player, MPRIS_PATH, "org.mpris.MediaPlayer2", "Raise", None)?;
    Ok(None)
}

// Display

fn flip_night_light_pause() -> Done {
    let bus = gala::session_bus()?;
    let on = !get_bool(&bus, GSD_COLOR, GSD_COLOR_PATH, GSD_COLOR, "DisabledUntilTomorrow")?;
    set(&bus, GSD_COLOR, GSD_COLOR_PATH, GSD_COLOR, "DisabledUntilTomorrow", on.to_variant())?;
    shown(on)
}

/// The accent is the account's; the settings daemon applies it to the theme and the portal.
fn set_accent(color: i32) -> Done {
    let bus = system_bus()?;
    let user = glib::user_name().to_string_lossy().into_owned();
    let reply = call(&bus, ACCOUNTS, "/org/freedesktop/Accounts", ACCOUNTS, "FindUserByName", Some((user,).to_variant()))?;
    let path = reply.child_value(0);
    let path = path.str().ok_or("the account has no path")?;
    set(&bus, ACCOUNTS, path, PANTHEON_ACCOUNT, "PrefersAccentColor", color.to_variant())?;
    Ok(Some(1.0))
}

fn wallpapers() -> Vec<PathBuf> {
    let mut folders = vec![PathBuf::from("/usr/share/backgrounds")];
    folders.push(glib::user_data_dir().join("backgrounds"));
    let mut found: Vec<PathBuf> = folders
        .iter()
        .filter_map(|folder| std::fs::read_dir(folder).ok())
        .flat_map(|entries| entries.flatten().map(|e| e.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| ["jpg", "jpeg", "png", "webp"].contains(&e.to_ascii_lowercase().as_str()))
        })
        .collect();
    found.sort();
    found
}

fn next_wallpaper() -> Done {
    let background = required(BACKGROUND, "picture-uri")?;
    let uris: Vec<String> = wallpapers().iter().map(|path| gio::File::for_path(path).uri().to_string()).collect();
    if uris.is_empty() {
        return Err("no wallpapers were found".into());
    }
    let current = background.string("picture-uri");
    let next = uris.iter().position(|uri| uri.as_str() == current.as_str()).map_or(0, |i| (i + 1) % uris.len());
    background.set_string("picture-uri", &uris[next]).map_err(|e| e.to_string())?;
    if settings(BACKGROUND, "picture-uri-dark").is_some_and(|s| !s.string("picture-uri-dark").is_empty()) {
        background.set_string("picture-uri-dark", &uris[next]).map_err(|e| e.to_string())?;
    }
    gio::Settings::sync();
    Ok(Some(1.0))
}

// Windows, workspaces and the panel

fn workspace_exists(bus: &gio::DBusConnection, index: i32) -> Result<(), String> {
    let reply = call(bus, GALA_BUS, GALA_DESKTOP_PATH, GALA_DESKTOP, "GetNWorkspaces", None)?;
    let count = reply.child_value(0).get::<i32>().unwrap_or(0);
    if index < count {
        Ok(())
    } else {
        Err(format!("there is no workspace {}", index + 1))
    }
}

// The window manager's own window actions

/// Lent for a moment to a window action that has no shortcut of its own.
const BORROWED_SHORTCUT: &str = "<Shift><Control><Alt><Super>F12";
/// How long the window manager takes to pick up a changed shortcut, and to
/// handle keys that were pressed.
const SHORTCUT_SETTLES: Duration = Duration::from_millis(150);
/// A command started by its own shortcut waits at most this long for its keys
/// to be let go.
const KEYS_LET_GO_WITHIN: Duration = Duration::from_secs(2);
/// The keyboard counts as let go once nothing has happened on it for this long.
const KEYS_AT_REST: Duration = Duration::from_millis(180);

/// Snapping or centering a window is only offered by the window manager's own
/// shortcuts (Gala has no call for it), so this presses that shortcut through
/// Mutter's remote desktop service. An action without one gets
/// [`BORROWED_SHORTCUT`] for that moment, and its setting is put back after.
fn press_wm_shortcut(schema: &str, key: &str) -> Done {
    let settings = required(schema, key)?;
    let bus = gala::session_bus()?;
    wait_for_shortcut_keys(&bus);
    let bound: Vec<String> = settings.strv(key).iter().map(|a| a.to_string()).collect();
    if let Some(keys) = bound.iter().find_map(|a| keysyms(a)) {
        press_keys(&bus, &keys)?;
        return Ok(None);
    }
    let keys = keysyms(BORROWED_SHORTCUT).ok_or("the borrowed shortcut has an unknown key")?;
    let was_set = settings.user_value(key).is_some();
    let mut lent: Vec<&str> = bound.iter().map(String::as_str).collect();
    lent.push(BORROWED_SHORTCUT);
    settings.set_strv(key, lent.as_slice()).map_err(|e| e.to_string())?;
    gio::Settings::sync();
    std::thread::sleep(SHORTCUT_SETTLES);
    let pressed = press_keys(&bus, &keys);
    std::thread::sleep(SHORTCUT_SETTLES);
    let put_back = if was_set {
        settings.set_strv(key, bound.iter().map(String::as_str).collect::<Vec<_>>().as_slice()).map_err(|e| e.to_string())
    } else {
        settings.reset(key);
        Ok(())
    };
    gio::Settings::sync();
    pressed.and(put_back).map(|()| None)
}

/// A command run by its own shortcut starts while those keys are still down,
/// and a held Ctrl or Alt would make the pressed shortcut a different one. So
/// it waits until a key has been let go since and the keyboard has rested.
fn wait_for_shortcut_keys(bus: &gio::DBusConnection) {
    let Some(pressed) = gala::shortcut_pressed_at() else { return };
    while pressed.elapsed() < KEYS_LET_GO_WITHIN {
        let Ok(reply) = call(bus, IDLE_MONITOR, "/org/gnome/Mutter/IdleMonitor/Core", IDLE_MONITOR, "GetIdletime", None) else {
            return;
        };
        let Some(idle) = reply.child_value(0).get::<u64>().map(Duration::from_millis) else { return };
        // the last key event came after the shortcut's own press
        let since_then = idle + Duration::from_millis(30) < pressed.elapsed();
        if since_then && idle >= KEYS_AT_REST {
            return;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

/// Presses and lets go of keys as a virtual keyboard: modifiers first.
fn press_keys(bus: &gio::DBusConnection, keys: &[u32]) -> Result<(), String> {
    const SESSION: &str = "org.gnome.Mutter.RemoteDesktop.Session";
    let reply = call(bus, REMOTE_DESKTOP, "/org/gnome/Mutter/RemoteDesktop", REMOTE_DESKTOP, "CreateSession", None)?;
    let session = reply.child_value(0).str().ok_or("no remote desktop session")?.to_string();
    let result = call(bus, REMOTE_DESKTOP, &session, SESSION, "Start", None).and_then(|_| {
        let key = |keysym: u32, down: bool| {
            call(bus, REMOTE_DESKTOP, &session, SESSION, "NotifyKeyboardKeysym", Some((keysym, down).to_variant())).map(drop)
        };
        let pressed = keys.iter().try_for_each(|k| key(*k, true));
        // let go of every key, even after a failed press
        let released = keys.iter().rev().map(|k| key(*k, false)).collect::<Result<Vec<_>, _>>();
        pressed.and(released.map(drop))
    });
    let _ = call(bus, REMOTE_DESKTOP, &session, SESSION, "Stop", None);
    result
}

/// The keysyms of a shortcut as GSettings writes it ("<Control><Super>Left"),
/// modifiers first, when every part of it is known.
fn keysyms(accelerator: &str) -> Option<Vec<u32>> {
    let mut keys = Vec::new();
    let mut rest = accelerator.trim();
    while let Some(after) = rest.strip_prefix('<') {
        let (modifier, after) = after.split_once('>')?;
        let keysym = match modifier.to_ascii_lowercase().as_str() {
            "shift" => 0xffe1,
            "control" | "ctrl" | "primary" => 0xffe3,
            "alt" | "mod1" => 0xffe9,
            "super" | "mod4" => 0xffeb,
            _ => return None,
        };
        if !keys.contains(&keysym) {
            keys.push(keysym);
        }
        rest = after;
    }
    keys.push(key_named(rest)?);
    Some(keys)
}

fn key_named(name: &str) -> Option<u32> {
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return c.is_ascii_alphanumeric().then(|| u32::from(c.to_ascii_lowercase()));
    }
    if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()).filter(|n| (1..=12).contains(n)) {
        return Some(0xffbe + n - 1);
    }
    Some(match name {
        "Left" => 0xff51,
        "Up" => 0xff52,
        "Right" => 0xff53,
        "Down" => 0xff54,
        "Page_Up" => 0xff55,
        "Page_Down" => 0xff56,
        "Home" => 0xff50,
        "End" => 0xff57,
        "Insert" => 0xff63,
        "Delete" => 0xffff,
        "space" => 0x20,
        "Return" => 0xff0d,
        "Tab" => 0xff09,
        "Escape" => 0xff1b,
        "BackSpace" => 0xff08,
        _ => return None,
    })
}

fn go_to_workspace(index: i32) -> Done {
    let bus = gala::session_bus()?;
    workspace_exists(&bus, index)?;
    call(&bus, GALA_BUS, GALA_DESKTOP_PATH, GALA_DESKTOP, "ActivateWorkspace", Some((index,).to_variant()))?;
    Ok(None)
}

fn focused_window(bus: &gio::DBusConnection) -> Result<u64, String> {
    let reply = call(bus, GALA_BUS, GALA_DESKTOP_PATH, GALA_DESKTOP, "GetWindows", None)?;
    let windows = reply.child_value(0);
    (0..windows.n_children())
        .find_map(|i| {
            let window = windows.child_value(i);
            let props = glib::VariantDict::new(Some(&window.child_value(1)));
            props.lookup_value("has-focus", None)?.get::<bool>()?.then(|| window.child_value(0).get::<u64>())?
        })
        .ok_or_else(|| "no window has focus".to_string())
}

fn move_window_to_workspace(index: i32) -> Done {
    let bus = gala::session_bus()?;
    workspace_exists(&bus, index)?;
    let window = focused_window(&bus)?;
    call(&bus, GALA_BUS, GALA_DESKTOP_PATH, GALA_DESKTOP, "MoveWindowToWorkspace", Some((window, index).to_variant()))?;
    Ok(None)
}

fn toggle_menu(code: &str) -> Done {
    let platform: HashMap<String, glib::Variant> = HashMap::new();
    let parameter = glib::Variant::array_from_iter_with_type(glib::VariantTy::VARIANT, [glib::Variant::from_variant(&code.to_variant())]);
    let args = glib::Variant::tuple_from_iter(["toggle-indicator".to_variant(), parameter, platform.to_variant()]);
    call(&gala::session_bus()?, WINGPANEL, WINGPANEL_PATH, "org.gtk.Actions", "Activate", Some(args))?;
    Ok(None)
}

// Network and Bluetooth

fn flip_networking() -> Done {
    let bus = system_bus()?;
    let on = !get_bool(&bus, NETWORK_MANAGER, NETWORK_MANAGER_PATH, NETWORK_MANAGER, "NetworkingEnabled")?;
    call(&bus, NETWORK_MANAGER, NETWORK_MANAGER_PATH, NETWORK_MANAGER, "Enable", Some((on,).to_variant()))?;
    shown(on)
}

fn flip_mobile_data() -> Done {
    let bus = system_bus()?;
    let on = !get_bool(&bus, NETWORK_MANAGER, NETWORK_MANAGER_PATH, NETWORK_MANAGER, "WwanEnabled")?;
    set(&bus, NETWORK_MANAGER, NETWORK_MANAGER_PATH, NETWORK_MANAGER, "WwanEnabled", on.to_variant())?;
    shown(on)
}

/// Splits a line of `nmcli -t`, whose separator is escaped inside values.
fn nmcli_fields(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => fields.last_mut().expect("never empty").extend(chars.next()),
            ':' => fields.push(String::new()),
            _ => fields.last_mut().expect("never empty").push(ch),
        }
    }
    fields
}

fn reconnect_wifi() -> Done {
    let devices = tool("nmcli", &["-t", "-f", "DEVICE,TYPE", "device"])?;
    let device = devices
        .lines()
        .map(nmcli_fields)
        .find(|fields| fields.get(1).is_some_and(|kind| kind == "wifi"))
        .and_then(|fields| fields.into_iter().next())
        .ok_or("no Wi-Fi device was found")?;
    tool("nmcli", &["device", "connect", &device])?;
    Ok(Some(1.0))
}

fn flip_vpn() -> Done {
    let connections = tool("nmcli", &["-t", "-f", "UUID,TYPE,ACTIVE", "connection", "show"])?;
    let vpns: Vec<Vec<String>> =
        connections.lines().map(nmcli_fields).filter(|f| f.get(1).is_some_and(|kind| is_vpn(kind))).collect();
    if let Some(active) = vpns.iter().find(|f| f.get(2).is_some_and(|a| a == "yes")) {
        tool("nmcli", &["connection", "down", "uuid", &active[0]])?;
        return shown(false);
    }
    let first = vpns.first().ok_or("no VPN is set up")?;
    tool("nmcli", &["connection", "up", "uuid", &first[0]])?;
    shown(true)
}

fn flush_dns() -> Done {
    tool("resolvectl", &["flush-caches"])?;
    Ok(Some(1.0))
}

fn bluetooth_objects(bus: &gio::DBusConnection, interface: &str) -> Result<Vec<String>, String> {
    let reply = call(bus, BLUEZ, "/", "org.freedesktop.DBus.ObjectManager", "GetManagedObjects", None)?;
    let objects = reply.child_value(0);
    Ok((0..objects.n_children())
        .filter_map(|i| {
            let object = objects.child_value(i);
            let interfaces = object.child_value(1);
            (0..interfaces.n_children())
                .any(|j| interfaces.child_value(j).child_value(0).str() == Some(interface))
                .then(|| object.child_value(0).str().map(str::to_string))
                .flatten()
        })
        .collect())
}

fn flip_discoverable() -> Done {
    let bus = system_bus()?;
    let adapters = bluetooth_objects(&bus, BLUEZ_ADAPTER)?;
    let powered: Vec<_> = adapters.iter().filter(|a| get_bool(&bus, BLUEZ, a, BLUEZ_ADAPTER, "Powered").unwrap_or(false)).collect();
    if powered.is_empty() {
        return Err("Bluetooth is off".into());
    }
    let on = !powered.iter().any(|a| get_bool(&bus, BLUEZ, a, BLUEZ_ADAPTER, "Discoverable").unwrap_or(false));
    for adapter in powered {
        set(&bus, BLUEZ, adapter, BLUEZ_ADAPTER, "Discoverable", on.to_variant())?;
    }
    shown(on)
}

fn disconnect_bluetooth() -> Done {
    let bus = system_bus()?;
    for device in bluetooth_objects(&bus, BLUEZ_DEVICE)? {
        if get_bool(&bus, BLUEZ, &device, BLUEZ_DEVICE, "Connected").unwrap_or(false) {
            call(&bus, BLUEZ, &device, BLUEZ_DEVICE, "Disconnect", None)?;
        }
    }
    Ok(Some(0.0))
}

// Keyboard and touchpad

fn switch_layout(forward: bool) -> Done {
    let sources = required(INPUT_SOURCES, "sources")?;
    let count = sources.value("sources").n_children() as u32;
    if count < 2 {
        return Err("only one keyboard layout is set up".into());
    }
    let current = sources.uint("current").min(count - 1);
    let next = if forward { (current + 1) % count } else { (current + count - 1) % count };
    sources.set_uint("current", next).map_err(|e| e.to_string())?;
    gio::Settings::sync();
    Ok(Some(1.0))
}

fn next_layout() -> Done {
    switch_layout(true)
}

fn previous_layout() -> Done {
    switch_layout(false)
}

/// Turning it off can leave a laptop without a pointer, so that asks first.
fn flip_touchpad() -> Done {
    let touchpad = required(TOUCHPAD, "send-events")?;
    let turn_on = touchpad.string("send-events") != "enabled";
    if !turn_on && !ask("Turn off the touchpad? Without a mouse you would need the keyboard to turn it back on.", "Turn Off") {
        return Ok(None);
    }
    write(TOUCHPAD, "send-events", T(if turn_on { "enabled" } else { "disabled" }))?;
    shown(turn_on)
}

// Time

fn flip_network_time() -> Done {
    let bus = system_bus()?;
    let on = !get_bool(&bus, TIMEDATE, TIMEDATE_PATH, TIMEDATE, "NTP")?;
    call(&bus, TIMEDATE, TIMEDATE_PATH, TIMEDATE, "SetNTP", Some((on, true).to_variant()))?;
    shown(on)
}

// Files and folders

fn open_place(place: Place) -> Done {
    let uri = match place {
        Special(None) => gio::File::for_path(glib::home_dir()).uri().to_string(),
        Special(Some(folder)) => {
            let path = glib::user_special_dir(folder).ok_or("that folder is not set up")?;
            gio::File::for_path(path).uri().to_string()
        }
        InHome(relative) => {
            let path = glib::home_dir().join(relative);
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            gio::File::for_path(path).uri().to_string()
        }
        InPictures(relative) => {
            let pictures = glib::user_special_dir(glib::UserDirectory::Pictures).ok_or("there is no Pictures folder")?;
            let path = pictures.join(relative);
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            gio::File::for_path(path).uri().to_string()
        }
        Uri(uri) => uri.to_string(),
    };
    gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>).map_err(|e| e.to_string())?;
    Ok(None)
}

fn empty_trash() -> Done {
    tool("gio", &["trash", "--empty"])?;
    Ok(Some(0.0))
}

fn clear_recent_files() -> Done {
    let (tx, rx) = mpsc::channel();
    glib::MainContext::default().invoke(move || {
        use gtk::prelude::RecentManagerExt;
        let result = gtk::RecentManager::default()
            .ok_or_else(|| "the list of recent files is not available".to_string())
            .and_then(|recent| recent.purge_items().map_err(|e| e.to_string()));
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_secs(10)).map_err(|e| e.to_string())??;
    Ok(Some(0.0))
}

// Screenshots

/// Asks the portal for a picture of the screen (Gala lets only the portal
/// take one) and returns where it put it.
fn portal_screenshot() -> Result<PathBuf, String> {
    let bus = gala::session_bus()?;
    let sender = bus.unique_name().ok_or("not connected to the session")?;
    let sender = sender.trim_start_matches(':').replace('.', "_");
    let token = format!("lummo{}", glib::monotonic_time());
    let handle = format!("{PORTAL_PATH}/request/{sender}/{token}");
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| {
            let answer: Arc<Mutex<Option<Result<String, String>>>> = Arc::default();
            let received = answer.clone();
            #[allow(deprecated)]
            let subscription = bus.signal_subscribe(
                Some(PORTAL),
                Some("org.freedesktop.portal.Request"),
                Some("Response"),
                Some(&handle),
                None,
                gio::DBusSignalFlags::NO_MATCH_RULE,
                move |_, _, _, _, _, params| {
                    let code = params.child_value(0).get::<u32>().unwrap_or(2);
                    let uri = glib::VariantDict::new(Some(&params.child_value(1)))
                        .lookup_value("uri", None)
                        .and_then(|v| v.get::<String>());
                    *received.lock().unwrap_or_else(|e| e.into_inner()) = Some(match (code, uri) {
                        (0, Some(uri)) => Ok(uri),
                        _ => Err("the screenshot was refused".to_string()),
                    });
                },
            );
            let options = glib::VariantDict::new(None);
            options.insert_value("handle_token", &token.to_variant());
            options.insert_value("interactive", &false.to_variant());
            let args = glib::Variant::tuple_from_iter(["".to_variant(), options.end()]);
            let sent = call(&bus, PORTAL, PORTAL_PATH, "org.freedesktop.portal.Screenshot", "Screenshot", Some(args));
            let deadline = Instant::now() + Duration::from_secs(10);
            let result = loop {
                if let Err(e) = &sent {
                    break Err(e.clone());
                }
                if let Some(result) = answer.lock().unwrap_or_else(|e| e.into_inner()).take() {
                    break result;
                }
                if Instant::now() > deadline {
                    break Err("the screenshot took too long".to_string());
                }
                if !context.iteration(false) {
                    std::thread::sleep(Duration::from_millis(10));
                }
            };
            #[allow(deprecated)]
            bus.signal_unsubscribe(subscription);
            result
        })
        .map_err(|e| e.to_string())?
        .and_then(|uri| gio::File::for_uri(&uri).path().ok_or_else(|| "the screenshot is not a local file".to_string()))
}

fn take_screenshot() -> Done {
    let taken = portal_screenshot()?;
    let pictures = glib::user_special_dir(glib::UserDirectory::Pictures).unwrap_or_else(|| glib::home_dir().join("Pictures"));
    let folder = pictures.join("Screenshots");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let stamp = glib::DateTime::now_local().and_then(|now| now.format("%Y-%m-%d %H-%M-%S")).map_err(|e| e.to_string())?;
    let target = folder.join(format!("Screenshot from {stamp}.png"));
    if std::fs::rename(&taken, &target).is_err() {
        std::fs::copy(&taken, &target).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&taken);
    }
    log::info!("screenshot saved to {}", target.display());
    Ok(Some(1.0))
}

fn open_screenshot_tool() -> Done {
    let tool = gio::DesktopAppInfo::new(SCREENSHOT_TOOL).ok_or("the Screenshot app is not installed")?;
    tool.launch(&[], None::<&gio::AppLaunchContext>).map_err(|e| e.to_string())?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_turn_into_keys() {
        assert_eq!(keysyms("<Control><Super>Left"), Some(vec![0xffe3, 0xffeb, 0xff51]));
        assert_eq!(keysyms("<Primary><Shift>a"), Some(vec![0xffe3, 0xffe1, 0x61]));
        assert_eq!(keysyms("<Alt>F5"), Some(vec![0xffe9, 0xffc2]));
        assert_eq!(keysyms(BORROWED_SHORTCUT), Some(vec![0xffe1, 0xffe3, 0xffe9, 0xffeb, 0xffc9]));
        assert_eq!(keysyms("<Hyper>Left"), None);
        assert_eq!(keysyms("XF86RotateWindows"), None);
    }

    /// Acts on the focused window: `wm-live.py` opens one to act on.
    #[test]
    #[ignore]
    fn press_live_window_action() {
        let action = std::env::var("WM_ACTION").unwrap();
        let (schema, key) = action.split_once(' ').unwrap();
        press_wm_shortcut(if schema == "mutter" { MUTTER_KEYS } else { WM_KEYS }, key).unwrap();
    }

    #[test]
    fn window_manager_commands_are_all_found_by_window() {
        let ids: Vec<_> = COMMANDS.iter().filter(|c| c.action.subtitle == WINDOW_MANAGER).map(|c| c.action.id).collect();
        assert_eq!(ids.len(), 16);
        for c in COMMANDS.iter().filter(|c| c.action.subtitle == WINDOW_MANAGER) {
            assert!(c.action.title.contains("Window"), "{} is not found by \"window\"", c.action.id);
        }
    }
    use crate::search::actions::ACTIONS;
    use std::collections::HashSet;

    #[test]
    fn catalog_is_well_formed() {
        let ids: HashSet<_> = all().map(|a| a.id).collect();
        assert_eq!(ids.len(), COMMANDS.len(), "ids are unique");
        assert!(ACTIONS.iter().all(|a| !ids.contains(a.id)), "and differ from the built-in actions");
        let titles: HashSet<_> = all().map(|a| a.title).collect();
        assert_eq!(titles.len(), COMMANDS.len(), "titles are unique");
        for a in all() {
            assert!(!a.title.is_empty() && !a.subtitle.is_empty() && !a.keywords.is_empty(), "{}", a.id);
            assert!(a.keywords.iter().all(|k| *k == k.to_lowercase()), "{}: keywords are lowercase", a.id);
        }
    }

    #[test]
    fn every_icon_can_be_drawn() {
        let icons = include_str!("../../../../src/components/spotlight/ResultIcon.jsx");
        let symbols = &icons[icons.find("export const SYMBOLS").unwrap()..];
        for a in all() {
            let quoted = format!("\"{}\":", a.icon);
            let plain = format!("\n  {}:", a.icon);
            assert!(symbols.contains(&quoted) || symbols.contains(&plain), "{}: no symbol {}", a.id, a.icon);
        }
    }

    #[test]
    fn what_can_lose_work_or_lock_you_out_asks_first() {
        for id in ["shut-down", "restart", "log-out", "restart-firmware", "empty-trash", "clear-recent"] {
            assert!(matches!(find(id).unwrap().run, Ask(..)), "{id}");
        }
        for id in ["slow-keys", "bounce-keys", "mouse-keys", "mouse-left-handed", "auto-clean-downloads", "auto-clean-screenshots"] {
            assert!(matches!(find(id).unwrap().run, AskFlip(..)), "{id}");
        }
        assert!(matches!(find("touchpad").unwrap().run, Later(_)), "flip_touchpad asks, off the calling thread");
    }

    #[test]
    fn the_screen_reader_is_never_a_command() {
        // turning it on starts Orca, and Lummo's own caret tracking must not depend on it
        for command in COMMANDS {
            if let Flip(_, key) | Set(_, key, _) = command.run {
                assert_ne!(key, "screen-reader-enabled");
            }
        }
    }

    #[test]
    fn gala_action_numbers_are_the_ones_of_its_source() {
        for (id, action) in [("window-multitasking", 1), ("window-maximize", 2), ("window-minimize", 3), ("window-all-windows", 7),
            ("workspace-previous", 8), ("workspace-next", 9), ("workspace-last", 10), ("window-always-on-top", 13),
            ("window-all-workspaces", 14), ("window-move-left", 15), ("window-move-right", 16), ("window-close", 17), ("screenshot-window", 18)] {
            assert!(matches!(find(id).unwrap().run, Gala(n) if n == action), "{id}");
        }
    }

    #[test]
    fn reads_the_volume() {
        assert_eq!(parse_volume("Volume: 0.19\n"), Some((0.19, false)));
        assert_eq!(parse_volume("Volume: 0.40 [MUTED]\n"), Some((0.40, true)));
        assert_eq!(parse_volume("Translate ID failed"), None);
    }

    const STATUS: &str = "PipeWire 'pipewire-0' [1.0.5]\n └─ Clients:\n        33. WirePlumber\n\nAudio\n ├─ Devices:\n │      46. Built-in Audio                      [alsa]\n │  \n ├─ Sinks:\n │      48. HDMI                                [vol: 1.00]\n │  *   51. Built-in Audio Analog Stereo        [vol: 0.19]\n │      60. Headphones                          [vol: 0.50]\n │  \n ├─ Sources:\n │  *   49. Built-in Mic                        [vol: 1.00]\n │  \n └─ Streams:\n\nVideo\n ├─ Devices:\n ├─ Sinks:\n │      99. Not audio\n";

    #[test]
    fn finds_the_audio_devices_and_the_next_one() {
        let sinks = audio_devices(STATUS, "Sinks:");
        assert_eq!(sinks, [(48, false), (51, true), (60, false)]);
        assert_eq!(following(&sinks), Some(60));
        assert_eq!(following(&[(48, false), (60, true)]), Some(48), "goes round");
        let sources = audio_devices(STATUS, "Sources:");
        assert_eq!(sources, [(49, true)]);
        assert_eq!(following(&sources), None, "nothing to switch to");
    }

    #[test]
    fn repeat_cycles_through_its_states() {
        assert_eq!(next_loop_status("None"), "Playlist");
        assert_eq!(next_loop_status("Playlist"), "Track");
        assert_eq!(next_loop_status("Track"), "None");
    }

    #[test]
    fn splits_nmcli_lines() {
        assert_eq!(nmcli_fields("My\\:VPN:vpn:yes"), ["My:VPN", "vpn", "yes"]);
        assert_eq!(nmcli_fields("wlp2s0:wifi"), ["wlp2s0", "wifi"]);
    }

    #[test]
    fn recognises_a_touchpad() {
        assert!(has_touchpad("I: Bus=0018\nN: Name=\"DELL0A2B:00 06CB:CDD6 Touchpad\"\n"));
        assert!(!has_touchpad("N: Name=\"Logitech USB Mouse\"\nP: Phys=touchpad-like\n"));
    }

    /// Exercises every setting on this desktop's real schemas in a throwaway
    /// in-memory store, so nothing on the machine changes:
    /// `GSETTINGS_BACKEND=memory cargo test --lib every_setting -- --ignored --test-threads=1`
    #[test]
    #[ignore = "needs this desktop's schemas and GSETTINGS_BACKEND=memory"]
    fn every_setting_really_changes() {
        assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"), "must not touch the real settings");
        let (mut flips, mut sets, mut steps, mut missing) = (0, 0, 0, Vec::new());
        for command in COMMANDS {
            if !settings_exist(command.run) {
                missing.push(command.action.id);
                continue;
            }
            match command.run {
                Flip(schema, key) | FlipOff(schema, key) | AskFlip(schema, key, ..) => {
                    let before = required(schema, key).unwrap().boolean(key);
                    let on = flip_key(schema, key).unwrap();
                    assert_eq!(on, !before, "{}", command.action.id);
                    assert_eq!(required(schema, key).unwrap().boolean(key), on, "{}", command.action.id);
                    assert_eq!(flip_key(schema, key).unwrap(), before, "{}", command.action.id);
                    flips += 1;
                }
                Swap(schema, key, a, b) => {
                    write(schema, key, T(a)).unwrap();
                    assert_eq!(perform(command.run).unwrap(), Some(0.0), "{}", command.action.id);
                    assert_eq!(required(schema, key).unwrap().string(key), b, "{}", command.action.id);
                    assert_eq!(perform(command.run).unwrap(), Some(1.0), "{}", command.action.id);
                    assert_eq!(required(schema, key).unwrap().string(key), a, "{}", command.action.id);
                    sets += 1;
                }
                Set(schema, key, value) => {
                    perform(command.run).unwrap();
                    let kind = required(schema, key).unwrap().value(key).type_().as_str().to_string();
                    assert_eq!(required(schema, key).unwrap().value(key), as_variant(&kind, value).unwrap(), "{}", command.action.id);
                    sets += 1;
                }
                Both(keys) => {
                    perform(command.run).unwrap();
                    for (schema, key, value) in keys {
                        let kind = required(schema, key).unwrap().value(key).type_().as_str().to_string();
                        assert_eq!(required(schema, key).unwrap().value(key), as_variant(&kind, *value).unwrap(), "{}", command.action.id);
                    }
                    sets += 1;
                }
                Step(schema, key, by, from, to) => {
                    let middle = ((from + to) / 2.0 * 100.0).round() / 100.0;
                    write(schema, key, V(middle)).unwrap();
                    let level = perform(command.run).unwrap().unwrap();
                    assert!((0.0..=1.0).contains(&level), "{}", command.action.id);
                    let value = required(schema, key).unwrap().value(key);
                    let now = value.get::<f64>().or(value.get::<i32>().map(f64::from)).or(value.get::<u32>().map(f64::from)).unwrap();
                    assert!((now - (middle + by)).abs() < 1.0, "{}: {now}", command.action.id);
                    assert_ne!(now, middle, "{}", command.action.id);
                    steps += 1;
                }
                _ => {}
            }
        }
        println!("{flips} switches, {sets} choices and {steps} steps changed for real; missing here: {missing:?}");
    }

    #[test]
    #[ignore = "reads what this computer has; runs nothing"]
    fn lists_what_this_computer_lacks() {
        let missing: Vec<_> = all().filter(|a| !is_available(a.id)).map(|a| a.id).collect();
        println!("{} of {} available, not here: {missing:?}", COMMANDS.len() - missing.len(), COMMANDS.len());
    }
}
