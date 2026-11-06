//! Opening apps, files and URLs through the Windows shell.

use std::os::windows::process::CommandExt;
use std::path::Path;

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows::Win32::UI::Shell::{ApplicationActivationManager, IApplicationActivationManager, ShellExecuteW, AO_NONE};

use super::jump_list::Launch;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// ShellExecute needs COM on the calling thread (STA, no OLE1 DDE).
fn ensure_com() {
    // S_FALSE / RPC_E_CHANGED_MODE just mean COM is already set up on this thread
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
}

pub fn shell_open(target: &str, directory: Option<&Path>) -> Result<(), String> {
    shell_execute(w!("open"), target, "", directory)
}

/// `ShellExecuteW` with a verb (`open`, or `runas` to start elevated) and arguments.
fn shell_execute(verb: PCWSTR, target: &str, arguments: &str, directory: Option<&Path>) -> Result<(), String> {
    ensure_com();
    let target = HSTRING::from(target);
    let arguments = (!arguments.is_empty()).then(|| HSTRING::from(arguments));
    let dir = directory.map(|d| HSTRING::from(d.as_os_str()));
    let result = unsafe {
        ShellExecuteW(
            None,
            verb,
            &target,
            arguments.as_ref().map(|a| PCWSTR(a.as_ptr())).unwrap_or(PCWSTR::null()),
            dir.as_ref().map(|d| PCWSTR(d.as_ptr())).unwrap_or(PCWSTR::null()),
            SW_SHOWNORMAL,
        )
    };
    // values <= 32 are error codes
    let code = result.0 as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(match code {
            2 | 3 => "the item could not be found".into(),
            5 => "access was denied".into(),
            31 => "no application is associated with this file type".into(),
            other => format!("the shell refused to open it (code {other})"),
        })
    }
}

/// `target` is the AppsFolder parsing name (AUMID or path), which covers
/// both classic desktop apps and Store apps.
pub fn launch_app(target: &str) -> Result<(), String> {
    shell_open(&format!("shell:AppsFolder\\{target}"), None)
}

/// Runs one of the app's jump-list tasks, read again from its jump list so
/// only a task the app still offers can run (see `jump_list`).
pub fn launch_app_action(target: &str, action: &str) -> Result<(), String> {
    ensure_com();
    let task = super::jump_list::tasks(target)
        .into_iter()
        .find(|t| t.id == action)
        .ok_or("the application no longer offers this action")?;
    log::info!("jump-list task “{}” of {target}: {:?}", task.title, task.launch);
    match &task.launch {
        Launch::Program { path, arguments, directory, run_as_admin } => {
            let verb = if *run_as_admin { w!("runas") } else { w!("open") };
            let directory = (!directory.is_empty()).then(|| Path::new(directory));
            shell_execute(verb, path, arguments, directory)
        }
        Launch::StoreApp { app_id, arguments } => activate_store_app(app_id, arguments),
    }
}

/// Starts a Store app with launch arguments, as its taskbar jump list does.
fn activate_store_app(app_id: &str, arguments: &str) -> Result<(), String> {
    unsafe {
        let manager: IApplicationActivationManager =
            CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER).map_err(|e| e.message())?;
        manager.ActivateApplication(&HSTRING::from(app_id), &HSTRING::from(arguments), AO_NONE).map_err(|e| e.message())?;
    }
    Ok(())
}

pub fn open_path(path: &Path) -> Result<(), String> {
    shell_open(&path.to_string_lossy(), None)
}

pub fn open_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("only https links can be opened".into());
    }
    shell_open(url, None)
}

pub fn reveal_path(path: &Path) -> Result<(), String> {
    // Windows paths cannot contain quotes, so quoting here is unambiguous.
    std::process::Command::new("explorer.exe")
        .raw_arg(format!("/select,\"{}\"", path.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

const EXECUTABLE_EXTENSIONS: &[&str] = &[
    "exe", "msi", "msix", "appx", "bat", "cmd", "com", "ps1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "scr", "lnk",
    "url", "appref-ms", "reg", "hta", "cpl", "jar", "pif", "application",
];

/// Files that would run code when opened; they are revealed instead.
pub fn is_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| EXECUTABLE_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(ext)))
}
