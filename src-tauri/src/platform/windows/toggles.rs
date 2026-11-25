//! What the System commands change, changed the way Windows' own switches
//! change it: `SystemParametersInfo`, the current user's registry with the
//! broadcast that tells Explorer, the audio endpoints, the radios and the
//! power service. Nothing here opens a window.

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant, SystemTime};

use windows::core::{s, w, GUID, PCWSTR};
use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};
use windows::Win32::Foundation::{ERROR_SUCCESS, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eCapture, eConsole, eRender, EDataFlow, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{
    CoCreateInstance, CoIncrementMTAUsage, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};
use windows::Win32::System::Power::{
    GetSystemPowerStatus, PowerEnumerate, PowerGetActiveScheme, PowerSetActiveScheme, SetThreadExecutionState, ACCESS_SCHEME,
    ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SYSTEM_POWER_STATUS,
};
use windows::Win32::System::Registry::HKEY_CURRENT_USER;
use windows::Win32::System::Threading::CREATE_NO_WINDOW;
use windows::Win32::UI::Accessibility::{FILTERKEYS, SKF_STICKYKEYSON, STICKYKEYS, STICKYKEYS_FLAGS, TOGGLEKEYS};
use windows::Win32::UI::Shell::{
    SHAddToRecentDocs, SHAppBarMessage, SHChangeNotify, SHGetSetSettings, ABM_GETSTATE, ABM_SETSTATE, ABS_AUTOHIDE,
    APPBARDATA, SHARD_PIDL, SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHELLSTATEA, SSF_AUTOCHECKSELECT, SSF_MASK,
    SSF_SHOWALLOBJECTS, SSF_SHOWEXTENSIONS, SSF_SHOWSUPERHIDDEN,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::registry;

/// Saved for the next sign-in, and announced to running apps.
const SAVE: SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS = SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(SPIF_UPDATEINIFILE.0 | SPIF_SENDCHANGE.0);
const READ: SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS = SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0);
/// How long Spotlight gets to hand focus back to the window a command acts on.
const FOCUS_RETURNS: Duration = Duration::from_millis(1500);
/// Temporary files this recent may still be in use.
const TEMP_FILE_AGE: Duration = Duration::from_secs(24 * 60 * 60);

fn message(e: windows::core::Error) -> String {
    e.message()
}

/// Runs a Windows program without a console window and waits for it.
pub fn quiet(program: &str, arguments: &[&str]) -> Result<(), String> {
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

/// Keeps COM's multithreaded apartment open until Lummo exits. Without this, a
/// [`with_com`] thread is usually the only one in it, so finishing that thread
/// closes the apartment and Windows unloads the WinRT DLLs it used
/// (Windows.Devices.Radios, WlanRadioManager). The `windows` crate still holds
/// their activation factories, and the radio stack still has callbacks due: the
/// next Toggle Wi-Fi or Toggle Bluetooth, or a late callback, then crashed Lummo.
fn keep_com_open() {
    static OPEN: Once = Once::new();
    OPEN.call_once(|| {
        // the cookie is never handed back: the apartment lasts as long as the process
        if let Err(e) = unsafe { CoIncrementMTAUsage() } {
            log::warn!("could not keep COM open: {e}");
        }
    });
}

/// Runs `f` on a thread of its own with COM set up, as the audio, radio and other WinRT APIs need.
pub(super) fn with_com<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    keep_com_open();
    std::thread::spawn(move || {
        let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        let result = f();
        if initialized {
            unsafe { CoUninitialize() };
        }
        result
    })
    .join()
    .map_err(|_| "the system call failed".to_string())?
}

/// Tells running apps and Explorer that a setting changed (`WM_SETTINGCHANGE`).
fn announce(area: PCWSTR) {
    unsafe {
        SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, WPARAM(0), LPARAM(area.as_ptr() as isize), SMTO_ABORTIFHUNG, 200, None);
    }
}

// ---- Registry switches

/// Who has to hear about a registry change before it shows.
#[derive(Clone, Copy, Debug)]
pub enum Refresh {
    /// Read when next needed.
    Nothing,
    /// Colors and light or dark.
    Theme,
    /// The taskbar and Explorer.
    Shell,
}

impl Refresh {
    fn announce(self) {
        match self {
            Refresh::Nothing => {}
            Refresh::Theme => announce(w!("ImmersiveColorSet")),
            Refresh::Shell => {
                announce(w!("TraySettings"));
                announce(PCWSTR::null());
            }
        }
    }
}

/// A DWORD of the current user that is `on` or `off`; `missing_on` is what Windows assumes without it.
#[derive(Clone, Copy, Debug)]
pub struct Flag {
    pub key: &'static str,
    pub name: &'static str,
    pub on: u32,
    pub off: u32,
    pub missing_on: bool,
    pub refresh: Refresh,
}

impl Flag {
    pub fn is_on(&self) -> bool {
        registry::read_dword(HKEY_CURRENT_USER, self.key, self.name).map_or(self.missing_on, |v| v == self.on)
    }

    pub fn set(&self, on: bool) -> Result<(), String> {
        registry::write_dword(HKEY_CURRENT_USER, self.key, self.name, if on { self.on } else { self.off })?;
        self.refresh.announce();
        Ok(())
    }

    pub fn flip(&self) -> Result<(), String> {
        self.set(!self.is_on())
    }
}

pub fn set_dword(key: &str, name: &str, value: u32, refresh: Refresh) -> Result<(), String> {
    registry::write_dword(HKEY_CURRENT_USER, key, name, value)?;
    refresh.announce();
    Ok(())
}

const CONSENT: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";

/// "Let apps use the camera" and the like: `capability` is `webcam`, `microphone` or `location`.
pub fn flip_app_access(capability: &str) -> Result<(), String> {
    let key = format!(r"{CONSENT}\{capability}");
    let allowed = registry::read_string(HKEY_CURRENT_USER, &key, "Value").is_none_or(|v| v == "Allow");
    registry::write_string(HKEY_CURRENT_USER, &key, "Value", if allowed { "Deny" } else { "Allow" })
}

// ---- SystemParametersInfo

/// A BOOL of `SystemParametersInfo`. Some take the new value in `pvParam`, the others in `uiParam`.
#[derive(Clone, Copy, Debug)]
pub struct SpiFlag {
    pub get: SYSTEM_PARAMETERS_INFO_ACTION,
    pub set: SYSTEM_PARAMETERS_INFO_ACTION,
    pub in_pointer: bool,
}

/// A number of `SystemParametersInfo`, kept between `min` and `max`.
#[derive(Clone, Copy, Debug)]
pub struct SpiNumber {
    pub get: SYSTEM_PARAMETERS_INFO_ACTION,
    pub set: SYSTEM_PARAMETERS_INFO_ACTION,
    pub in_pointer: bool,
    pub min: u32,
    pub max: u32,
}

fn spi_get(action: SYSTEM_PARAMETERS_INFO_ACTION) -> Result<u32, String> {
    let mut value: u32 = 0;
    unsafe { SystemParametersInfoW(action, 0, Some((&mut value as *mut u32).cast()), READ) }.map_err(message)?;
    Ok(value)
}

fn spi_set(action: SYSTEM_PARAMETERS_INFO_ACTION, value: u32, in_pointer: bool) -> Result<(), String> {
    let (ui, pv) = if in_pointer { (0, Some(value as usize as *mut core::ffi::c_void)) } else { (value, None) };
    unsafe { SystemParametersInfoW(action, ui, pv, SAVE) }.map_err(message)
}

impl SpiFlag {
    pub fn is_on(&self) -> Result<bool, String> {
        spi_get(self.get).map(|v| v != 0)
    }

    pub fn flip(&self) -> Result<(), String> {
        spi_set(self.set, u32::from(!self.is_on()?), self.in_pointer)
    }
}

impl SpiNumber {
    pub fn get(&self) -> Result<u32, String> {
        spi_get(self.get)
    }

    pub fn set(&self, value: u32) -> Result<(), String> {
        spi_set(self.set, value.clamp(self.min, self.max), self.in_pointer)
    }

    pub fn step(&self, by: i32) -> Result<(), String> {
        let next = self.get()?.saturating_add_signed(by).clamp(self.min, self.max);
        self.set(next)
    }
}

/// A value `SystemParametersInfo` only sets (there is no getter for the double-click time).
pub fn spi_set_only(action: SYSTEM_PARAMETERS_INFO_ACTION, value: u32) -> Result<(), String> {
    spi_set(action, value, false)
}

/// A structure of `SystemParametersInfo`; each one used here starts with its size.
trait Sized32: Default {
    fn with_size() -> Self;
}

macro_rules! sized32 {
    ($($name:ident),*) => {$(
        impl Sized32 for $name {
            fn with_size() -> Self {
                Self { cbSize: std::mem::size_of::<Self>() as u32, ..Default::default() }
            }
        }
    )*};
}
sized32!(STICKYKEYS, FILTERKEYS, TOGGLEKEYS);

fn spi_read<T: Sized32>(get: SYSTEM_PARAMETERS_INFO_ACTION) -> Result<T, String> {
    let mut value = T::with_size();
    let size = std::mem::size_of::<T>() as u32;
    unsafe { SystemParametersInfoW(get, size, Some((&mut value as *mut T).cast()), READ) }.map_err(message)?;
    Ok(value)
}

fn spi_write<T: Sized32>(set: SYSTEM_PARAMETERS_INFO_ACTION, mut value: T) -> Result<(), String> {
    let size = std::mem::size_of::<T>() as u32;
    unsafe { SystemParametersInfoW(set, size, Some((&mut value as *mut T).cast()), SAVE) }.map_err(message)
}

pub fn flip_sticky_keys() -> Result<(), String> {
    let mut keys: STICKYKEYS = spi_read(SPI_GETSTICKYKEYS)?;
    keys.dwFlags = STICKYKEYS_FLAGS(keys.dwFlags.0 ^ SKF_STICKYKEYSON.0);
    spi_write(SPI_SETSTICKYKEYS, keys)
}

pub fn filter_keys_on() -> Result<bool, String> {
    spi_read::<FILTERKEYS>(SPI_GETFILTERKEYS).map(|k| k.dwFlags & FKF_FILTERKEYSON != 0)
}

pub fn flip_filter_keys() -> Result<(), String> {
    let mut keys: FILTERKEYS = spi_read(SPI_GETFILTERKEYS)?;
    keys.dwFlags ^= FKF_FILTERKEYSON;
    spi_write(SPI_SETFILTERKEYS, keys)
}

pub fn flip_toggle_keys() -> Result<(), String> {
    let mut keys: TOGGLEKEYS = spi_read(SPI_GETTOGGLEKEYS)?;
    keys.dwFlags ^= TKF_TOGGLEKEYSON;
    spi_write(SPI_SETTOGGLEKEYS, keys)
}

/// "Enhance pointer precision": the third of the mouse's thresholds and acceleration.
pub fn flip_pointer_precision() -> Result<(), String> {
    let mut mouse = [0i32; 3];
    unsafe { SystemParametersInfoW(SPI_GETMOUSE, 0, Some(mouse.as_mut_ptr().cast()), READ) }.map_err(message)?;
    // Windows' own values for on and off
    let mut next = if mouse[2] != 0 { [0, 0, 0] } else { [6, 10, 1] };
    unsafe { SystemParametersInfoW(SPI_SETMOUSE, 0, Some(next.as_mut_ptr().cast()), SAVE) }.map_err(message)
}

pub fn flip_mouse_buttons() -> Result<(), String> {
    let swapped = unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0;
    spi_set(SPI_SETMOUSEBUTTONSWAP, u32::from(!swapped), false)
}

/// Seven is what the Mouse control panel sets.
pub fn flip_pointer_trails() -> Result<(), String> {
    let on = spi_get(SPI_GETMOUSETRAILS)? > 1;
    spi_set(SPI_SETMOUSETRAILS, if on { 0 } else { 7 }, false)
}

// ---- Explorer

/// Settings of File Explorer that `SHGetSetSettings` changes, which tells its open windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellFlag {
    HiddenFiles,
    Extensions,
    ProtectedFiles,
    CheckBoxes,
}

impl ShellFlag {
    /// Its mask, and its bit in the first or second bit field of `SHELLSTATE`.
    fn place(self) -> (SSF_MASK, bool, u32) {
        match self {
            ShellFlag::HiddenFiles => (SSF_SHOWALLOBJECTS, true, 0),
            ShellFlag::Extensions => (SSF_SHOWEXTENSIONS, true, 1),
            ShellFlag::ProtectedFiles => (SSF_SHOWSUPERHIDDEN, true, 15),
            ShellFlag::CheckBoxes => (SSF_AUTOCHECKSELECT, false, 3),
        }
    }

    pub fn is_on(self) -> bool {
        let (mask, first, bit) = self.place();
        let mut state = SHELLSTATEA::default();
        unsafe { SHGetSetSettings(Some(&mut state), mask, false) };
        let field = if first { state._bitfield1 } else { state._bitfield2 };
        field & (1 << bit) != 0
    }

    pub fn set(self, on: bool) {
        let (mask, first, bit) = self.place();
        let mut state = SHELLSTATEA::default();
        unsafe { SHGetSetSettings(Some(&mut state), mask, false) };
        let field = if first { state._bitfield1 } else { state._bitfield2 };
        let field = if on { field | (1 << bit) } else { field & !(1 << bit) };
        if first {
            state._bitfield1 = field;
        } else {
            state._bitfield2 = field;
        }
        unsafe {
            SHGetSetSettings(Some(&mut state), mask, true);
            // open folders draw themselves again
            SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
        }
    }
}

fn taskbar() -> Result<HWND, String> {
    unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) }.map_err(|_| "the taskbar is not running".to_string())
}

pub fn flip_taskbar_autohide() -> Result<(), String> {
    let mut data = APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, hWnd: taskbar()?, ..Default::default() };
    let state = unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) } as u32;
    data.lParam = LPARAM((state ^ ABS_AUTOHIDE) as isize);
    unsafe { SHAppBarMessage(ABM_SETSTATE, &mut data) };
    Ok(())
}

/// The list view that shows the desktop's icons: in Progman, or in a WorkerW behind it.
fn desktop_view() -> Option<HWND> {
    unsafe extern "system" fn find(window: HWND, found: LPARAM) -> windows::core::BOOL {
        let view = unsafe { FindWindowExW(Some(window), None, w!("SHELLDLL_DefView"), PCWSTR::null()) };
        match view {
            Ok(view) => {
                unsafe { *(found.0 as *mut Option<HWND>) = Some(view) };
                false.into()
            }
            Err(_) => true.into(),
        }
    }
    let progman = unsafe { FindWindowW(w!("Progman"), PCWSTR::null()) }.ok()?;
    if let Ok(view) = unsafe { FindWindowExW(Some(progman), None, w!("SHELLDLL_DefView"), PCWSTR::null()) } {
        return Some(view);
    }
    let mut found: Option<HWND> = None;
    let _ = unsafe { EnumWindows(Some(find), LPARAM(&mut found as *mut Option<HWND> as isize)) };
    found
}

/// What the desktop's View › Show desktop icons sends.
pub fn flip_desktop_icons() -> Result<(), String> {
    const TOGGLE_ICONS: usize = 0x7402;
    let view = desktop_view().ok_or("the desktop is not running")?;
    unsafe { SendMessageW(view, WM_COMMAND, Some(WPARAM(TOGGLE_ICONS)), Some(LPARAM(0))) };
    Ok(())
}

pub fn restart_explorer() -> Result<(), String> {
    quiet("taskkill.exe", &["/f", "/im", "explorer.exe"])?;
    std::thread::spawn(|| {
        // Windows starts the shell again by itself; this is for when it does not
        std::thread::sleep(Duration::from_secs(4));
        if taskbar().is_err() {
            if let Err(e) = super::launch::shell_open("explorer.exe", None) {
                log::warn!("could not start Explorer again: {e}");
            }
        }
    });
    Ok(())
}

/// What Explorer lists under Recent, and Start under Recommended.
pub fn clear_recent_files() {
    unsafe { SHAddToRecentDocs(SHARD_PIDL.0 as u32, None) };
}

/// Files in the temporary folder untouched for a day, and the folders this empties.
/// What is in use cannot be deleted and stays.
pub fn delete_old_temp_files() -> Result<(), String> {
    let temp = std::env::temp_dir();
    if !temp.is_dir() {
        return Err("the temporary folder is missing".into());
    }
    delete_older_than(&temp, TEMP_FILE_AGE, SystemTime::now());
    Ok(())
}

/// Deletes the files in `dir` older than `age`, and the folders that leaves empty.
/// Returns whether `dir` is empty now. Links are deleted, never followed.
fn delete_older_than(dir: &Path, age: Duration, now: SystemTime) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    let mut emptied = true;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            emptied = false;
            continue;
        };
        let deleted = if kind.is_dir() && !kind.is_symlink() {
            delete_older_than(&path, age, now) && std::fs::remove_dir(&path).is_ok()
        } else {
            let modified = entry.metadata().and_then(|m| m.modified());
            let old = modified.is_ok_and(|t| now.duration_since(t).unwrap_or_default() >= age);
            old && std::fs::remove_file(&path).is_ok()
        };
        emptied &= deleted;
    }
    emptied
}

// ---- Sound

fn endpoint(flow: EDataFlow) -> Result<IAudioEndpointVolume, String> {
    unsafe {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(message)?;
        let device = devices.GetDefaultAudioEndpoint(flow, eConsole).map_err(|_| {
            if flow == eCapture { "no microphone is connected" } else { "no speakers or headphones are connected" }.to_string()
        })?;
        device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).map_err(message)
    }
}

fn has_endpoint(flow: EDataFlow) -> bool {
    with_com(move || endpoint(flow).map(|_| ())).is_ok()
}

/// Sets the speakers or the microphone to `level` (0 to 1), unmuted.
fn set_level(flow: EDataFlow, level: f32) -> Result<(), String> {
    with_com(move || unsafe {
        let volume = endpoint(flow)?;
        volume.SetMute(false, std::ptr::null()).map_err(message)?;
        volume.SetMasterVolumeLevelScalar(level.clamp(0.0, 1.0), std::ptr::null()).map_err(message)
    })
}

pub fn set_volume(level: f32) -> Result<(), String> {
    set_level(eRender, level)
}

pub fn set_microphone(level: f32) -> Result<(), String> {
    set_level(eCapture, level)
}

pub fn step_microphone(by: f32) -> Result<(), String> {
    with_com(move || unsafe {
        let volume = endpoint(eCapture)?;
        let now = volume.GetMasterVolumeLevelScalar().map_err(message)?;
        volume.SetMasterVolumeLevelScalar((now + by).clamp(0.0, 1.0), std::ptr::null()).map_err(message)
    })
}

pub fn flip_microphone_mute() -> Result<(), String> {
    with_com(|| unsafe {
        let volume = endpoint(eCapture)?;
        let muted = volume.GetMute().map_err(message)?.as_bool();
        volume.SetMute(!muted, std::ptr::null()).map_err(message)
    })
}

// ---- Radios

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioType {
    WiFi,
    Bluetooth,
}

impl RadioType {
    fn kind(self) -> RadioKind {
        match self {
            RadioType::WiFi => RadioKind::WiFi,
            RadioType::Bluetooth => RadioKind::Bluetooth,
        }
    }
}

fn find_radio(radio: RadioType) -> Result<Option<Radio>, String> {
    let radios = Radio::GetRadiosAsync().and_then(|op| op.join()).map_err(message)?;
    Ok(radios.into_iter().find(|r| r.Kind().is_ok_and(|k| k == radio.kind())))
}

/// The radios this PC has, read at most once in a while.
fn radios() -> Vec<RadioType> {
    static KNOWN: Mutex<Option<(Instant, Vec<RadioType>)>> = Mutex::new(None);
    const FOR: Duration = Duration::from_secs(10);
    let mut known = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, radios)) = known.as_ref().filter(|(at, _)| at.elapsed() < FOR) {
        return radios.clone();
    }
    let found = with_com(|| {
        Ok([RadioType::WiFi, RadioType::Bluetooth]
            .into_iter()
            .filter(|&r| find_radio(r).is_ok_and(|found| found.is_some()))
            .collect::<Vec<_>>())
    })
    .unwrap_or_default();
    *known = Some((Instant::now(), found.clone()));
    found
}

pub fn has_radio(radio: RadioType) -> bool {
    radios().contains(&radio)
}

pub fn flip_radio(radio: RadioType) -> Result<(), String> {
    with_com(move || {
        let access = Radio::RequestAccessAsync().and_then(|op| op.join()).map_err(message)?;
        if access != RadioAccessStatus::Allowed {
            return Err("Windows does not let apps turn radios on and off (Privacy & security › Radios)".into());
        }
        let found = find_radio(radio)?.ok_or("this PC has no such radio")?;
        let next = if found.State().map_err(message)? == RadioState::On { RadioState::Off } else { RadioState::On };
        match found.SetStateAsync(next).and_then(|op| op.join()).map_err(message)? {
            RadioAccessStatus::Allowed => Ok(()),
            _ => Err("Windows refused to change the radio".into()),
        }
    })
}

// ---- Power

pub fn has_battery() -> bool {
    const NO_BATTERY: u8 = 128;
    const UNKNOWN: u8 = 255;
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) }.is_ok() && status.BatteryFlag != NO_BATTERY && status.BatteryFlag != UNKNOWN
}

pub fn has_microphone() -> bool {
    has_endpoint(eCapture)
}

pub fn has_speakers() -> bool {
    has_endpoint(eRender)
}

pub const BALANCED: u128 = 0x381b4222_f694_41f0_9685_ff5bb260df2e;

/// The plans Power Options lists (`powercfg /list`); hidden templates such as Ultimate Performance are not among them.
fn power_plans() -> Vec<u128> {
    let mut plans = Vec::new();
    for index in 0.. {
        let mut guid = GUID::zeroed();
        let mut size = std::mem::size_of::<GUID>() as u32;
        let buffer = Some((&mut guid as *mut GUID).cast::<u8>());
        if unsafe { PowerEnumerate(None, None, None, ACCESS_SCHEME, index, buffer, &mut size) } != ERROR_SUCCESS {
            break;
        }
        plans.push(guid.to_u128());
    }
    plans
}

pub fn has_power_plan(plan: u128) -> bool {
    power_plans().contains(&plan)
}

pub fn set_power_plan(plan: u128) -> Result<(), String> {
    let guid = GUID::from_u128(plan);
    match unsafe { PowerSetActiveScheme(None, Some(&guid)) } {
        ERROR_SUCCESS => Ok(()),
        error => Err(format!("Windows refused the power plan ({})", error.0)),
    }
}

fn active_power_plan() -> Option<u128> {
    let mut guid: *mut GUID = std::ptr::null_mut();
    if unsafe { PowerGetActiveScheme(None, &mut guid) } != ERROR_SUCCESS || guid.is_null() {
        return None;
    }
    let plan = unsafe { *guid }.to_u128();
    unsafe { windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(guid.cast()))) };
    Some(plan)
}

type SetOverlay = unsafe extern "system" fn(*const GUID) -> u32;

/// `PowerSetActiveOverlayScheme`, what Settings' Power mode calls; powrprof exports it without a header.
fn set_overlay() -> Option<SetOverlay> {
    let library = unsafe { LoadLibraryExW(w!("powrprof.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }.ok()?;
    let function = unsafe { GetProcAddress(library, s!("PowerSetActiveOverlayScheme")) }?;
    // SAFETY: the export's signature is `DWORD (const GUID *)`
    Some(unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, SetOverlay>(function) })
}

pub fn has_power_modes() -> bool {
    set_overlay().is_some() && has_power_plan(BALANCED)
}

/// Power modes refine the Balanced plan, so that plan is chosen first.
pub fn set_power_mode(overlay: u128) -> Result<(), String> {
    let set = set_overlay().ok_or("this version of Windows has no power modes")?;
    if active_power_plan() != Some(BALANCED) {
        set_power_plan(BALANCED)?;
    }
    let guid = GUID::from_u128(overlay);
    match unsafe { set(&guid) } {
        0 => Ok(()),
        error => Err(format!("Windows refused the power mode ({error})")),
    }
}

/// The screen and sleep timeouts of the plan in use, in minutes (0 is never).
pub fn set_timeout(setting: &str, minutes: u32) -> Result<(), String> {
    quiet("powercfg.exe", &["/change", setting, &minutes.to_string()])
}

/// What closing the lid does: 0 nothing, 1 sleep.
pub fn set_lid_action(plugged_in: bool, action: u32) -> Result<(), String> {
    let index = if plugged_in { "/setacvalueindex" } else { "/setdcvalueindex" };
    quiet("powercfg.exe", &[index, "SCHEME_CURRENT", "SUB_BUTTONS", "LIDACTION", &action.to_string()])?;
    quiet("powercfg.exe", &["/setactive", "SCHEME_CURRENT"])
}

/// Kept on by a thread that holds the request until it is told to let go.
static KEEP_AWAKE: Mutex<Option<Sender<()>>> = Mutex::new(None);

pub fn flip_keep_awake() -> Result<(), String> {
    let mut awake = KEEP_AWAKE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(stop) = awake.take() {
        let _ = stop.send(());
        log::info!("keep awake off");
        return Ok(());
    }
    let (stop, stopped) = mpsc::channel::<()>();
    std::thread::Builder::new()
        .name("keep-awake".into())
        .spawn(move || {
            unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED) };
            let _ = stopped.recv();
            unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
        })
        .map_err(|e| e.to_string())?;
    *awake = Some(stop);
    log::info!("keep awake on");
    Ok(())
}

pub fn battery_report() -> Result<(), String> {
    let path = std::env::temp_dir().join("battery-report.html");
    let path = path.to_string_lossy().into_owned();
    quiet("powercfg.exe", &["/batteryreport", "/output", &path])?;
    super::launch::shell_open(&path, None)
}

// ---- Windows

fn class_name(window: HWND) -> String {
    let mut name = [0u16; 64];
    let len = unsafe { GetClassNameW(window, &mut name) };
    String::from_utf16_lossy(&name[..len.max(0) as usize])
}

/// The window in front once Spotlight has given focus back: not Lummo's own, the desktop or the taskbar.
pub fn window_in_front() -> Option<HWND> {
    const SHELL: &[&str] = &["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];
    let own = std::process::id();
    let started = Instant::now();
    loop {
        let window = unsafe { GetForegroundWindow() };
        if !window.is_invalid() {
            let mut process = 0u32;
            unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
            if process != own {
                return (!SHELL.contains(&class_name(window).as_str())).then_some(window);
            }
        }
        if started.elapsed() > FOCUS_RETURNS {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn flip_maximized(window: HWND) -> Result<(), String> {
    let maximized = unsafe { IsZoomed(window) }.as_bool();
    let _ = unsafe { ShowWindow(window, if maximized { SW_RESTORE } else { SW_MAXIMIZE }) };
    Ok(())
}

pub fn minimize(window: HWND) -> Result<(), String> {
    let _ = unsafe { ShowWindow(window, SW_MINIMIZE) };
    Ok(())
}

pub fn restore(window: HWND) -> Result<(), String> {
    let _ = unsafe { ShowWindow(window, SW_RESTORE) };
    Ok(())
}

/// Asks the window to close, as its close button does: it can still ask to save.
pub fn close(window: HWND) -> Result<(), String> {
    unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) }.map_err(message)
}

pub fn flip_always_on_top(window: HWND) -> Result<(), String> {
    let on_top = unsafe { GetWindowLongW(window, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0 != 0;
    let place = if on_top { HWND_NOTOPMOST } else { HWND_TOPMOST };
    unsafe { SetWindowPos(window, Some(place), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) }.map_err(message)
}

/// In the middle of the work area of its display.
pub fn center(window: HWND) -> Result<(), String> {
    if unsafe { IsZoomed(window) }.as_bool() {
        let _ = unsafe { ShowWindow(window, SW_RESTORE) };
    }
    let mut frame = RECT::default();
    unsafe { GetWindowRect(window, &mut frame) }.map_err(message)?;
    let mut display = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if !unsafe { GetMonitorInfoW(MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST), &mut display) }.as_bool() {
        return Err("the window's display is unknown".into());
    }
    let work = display.rcWork;
    let (width, height) = (frame.right - frame.left, frame.bottom - frame.top);
    let x = work.left + (work.right - work.left - width).max(0) / 2;
    let y = work.top + (work.bottom - work.top - height).max(0) / 2;
    unsafe { SetWindowPos(window, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE) }.map_err(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_old_temporary_files_are_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("recent.txt");
        std::fs::write(&file, "x").unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("new.txt"), "x").unwrap();
        let empty = dir.path().join("empty");
        std::fs::create_dir(&empty).unwrap();
        let hour = Duration::from_secs(3600);

        assert!(!delete_older_than(dir.path(), hour, SystemTime::now()));
        assert!(file.exists() && nested.join("new.txt").exists(), "recent files stay");
        assert!(!empty.exists(), "an empty folder goes");

        assert!(delete_older_than(dir.path(), hour, SystemTime::now() + 2 * hour), "an hour later all of it is old");
        assert!(!file.exists() && !nested.exists());
    }

    #[test]
    #[ignore = "flips settings of this PC and puts them back"]
    fn switches_flip_and_flip_back() {
        let flags = [
            SpiFlag { get: SPI_GETMOUSESONAR, set: SPI_SETMOUSESONAR, in_pointer: true },
            SpiFlag { get: SPI_GETMOUSEVANISH, set: SPI_SETMOUSEVANISH, in_pointer: true },
            SpiFlag { get: SPI_GETCLIENTAREAANIMATION, set: SPI_SETCLIENTAREAANIMATION, in_pointer: true },
            SpiFlag { get: SPI_GETACTIVEWINDOWTRACKING, set: SPI_SETACTIVEWINDOWTRACKING, in_pointer: true },
            SpiFlag { get: SPI_GETKEYBOARDCUES, set: SPI_SETKEYBOARDCUES, in_pointer: true },
            SpiFlag { get: SPI_GETSNAPTODEFBUTTON, set: SPI_SETSNAPTODEFBUTTON, in_pointer: false },
            SpiFlag { get: SPI_GETDRAGFULLWINDOWS, set: SPI_SETDRAGFULLWINDOWS, in_pointer: false },
            SpiFlag { get: SPI_GETWINARRANGING, set: SPI_SETWINARRANGING, in_pointer: false },
        ];
        for flag in flags {
            let before = flag.is_on().unwrap();
            flag.flip().unwrap();
            assert_ne!(flag.is_on().unwrap(), before, "{:?} did not change", flag.set);
            flag.flip().unwrap();
            assert_eq!(flag.is_on().unwrap(), before, "{:?} did not change back", flag.set);
        }
        for flag in [ShellFlag::HiddenFiles, ShellFlag::Extensions, ShellFlag::CheckBoxes] {
            let before = flag.is_on();
            flag.set(!before);
            assert_ne!(flag.is_on(), before, "{flag:?} did not change");
            flag.set(before);
            assert_eq!(flag.is_on(), before, "{flag:?} did not change back");
        }
    }

    #[test]
    #[ignore = "flips the microphone, volume, taskbar and power mode of this PC and puts them back"]
    fn system_switches_flip_and_flip_back() {
        let muted = || with_com(|| unsafe { endpoint(eCapture)?.GetMute().map(|m| m.as_bool()).map_err(message) }).unwrap();
        let before = muted();
        flip_microphone_mute().unwrap();
        assert_ne!(muted(), before, "the microphone did not change");
        flip_microphone_mute().unwrap();
        assert_eq!(muted(), before);

        let level = || with_com(|| unsafe { endpoint(eRender)?.GetMasterVolumeLevelScalar().map_err(message) }).unwrap();
        let speaker_muted = with_com(|| unsafe { endpoint(eRender)?.GetMute().map(|m| m.as_bool()).map_err(message) }).unwrap();
        let volume = level();
        set_volume(volume).unwrap();
        assert!((level() - volume).abs() < 0.01, "the volume moved");
        if speaker_muted {
            with_com(|| unsafe { endpoint(eRender)?.SetMute(true, std::ptr::null()).map_err(message) }).unwrap();
        }

        let autohide = || {
            let mut data = APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, hWnd: taskbar().unwrap(), ..Default::default() };
            let state = unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) } as u32;
            state & ABS_AUTOHIDE != 0
        };
        let before = autohide();
        flip_taskbar_autohide().unwrap();
        assert_ne!(autohide(), before, "auto-hide did not change");
        flip_taskbar_autohide().unwrap();
        assert_eq!(autohide(), before);

        // the mode this PC is in already, so nothing changes
        const SCHEMES: &str = r"SYSTEM\CurrentControlSet\Control\Power\User\PowerSchemes";
        let current = registry::read_string(windows::Win32::System::Registry::HKEY_LOCAL_MACHINE, SCHEMES, "ActiveOverlayAcPowerScheme")
            .and_then(|g| u128::from_str_radix(&g.replace('-', ""), 16).ok())
            .unwrap_or(0);
        set_power_mode(current).unwrap();
    }

    #[test]
    #[ignore = "asks the radio stack, as Toggle Wi-Fi does first; turns nothing on or off"]
    fn radio_calls_survive_com_shutting_down_between_them() {
        // each with_com thread is the only one in COM's multithreaded apartment, so
        // finishing it used to tear the apartment down and unload the radio DLLs
        for _ in 0..5 {
            with_com(|| find_radio(RadioType::WiFi).map(|_| ())).unwrap();
            let access = with_com(|| Radio::RequestAccessAsync().and_then(|op| op.join()).map_err(message)).unwrap();
            assert_eq!(access, RadioAccessStatus::Allowed);
        }
    }

    #[test]
    #[ignore = "reads what this PC has; changes nothing"]
    fn reads_what_this_pc_has() {
        println!("wifi {} bluetooth {}", has_radio(RadioType::WiFi), has_radio(RadioType::Bluetooth));
        println!("microphone {} speakers {} battery {}", has_microphone(), has_speakers(), has_battery());
        println!("power modes {} active plan {:x?}", has_power_modes(), active_power_plan());
        println!("power plans {:x?}", power_plans());
        println!("filter keys {:?}", filter_keys_on());
        println!("desktop view {:?} taskbar {:?}", desktop_view(), taskbar());
    }
}
