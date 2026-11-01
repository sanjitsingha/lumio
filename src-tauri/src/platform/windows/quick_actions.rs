//! Quick actions on Windows.

use std::path::PathBuf;

use windows::core::{w, PWSTR};
use windows::Win32::UI::Shell::{AssocQueryStringW, ASSOCF_IS_PROTOCOL, ASSOCSTR_EXECUTABLE};

use super::launch::shell_open;

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

/// Executable registered for the `https` protocol (the default browser).
fn default_browser() -> Option<String> {
    let mut len: u32 = 0;
    unsafe {
        let _ = AssocQueryStringW(ASSOCF_IS_PROTOCOL, ASSOCSTR_EXECUTABLE, w!("https"), None, None, &mut len);
    }
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u16; len as usize];
    let hr = unsafe {
        AssocQueryStringW(ASSOCF_IS_PROTOCOL, ASSOCSTR_EXECUTABLE, w!("https"), None, Some(PWSTR(buf.as_mut_ptr())), &mut len)
    };
    if hr.is_err() {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end])).filter(|s| !s.is_empty())
}

/// Windows Terminal is an app-execution alias in WindowsApps when installed.
fn windows_terminal() -> Option<PathBuf> {
    let alias = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(r"Microsoft\WindowsApps\wt.exe");
    alias.exists().then_some(alias)
}

pub fn is_available(id: &str) -> bool {
    match id {
        "system-settings" | "calculator" | "file-manager" | "terminal" => true,
        "browser" => default_browser().is_some(),
        _ => false,
    }
}

pub fn run(id: &str) -> Result<(), String> {
    match id {
        "system-settings" => shell_open("ms-settings:", None),
        "calculator" => shell_open("calculator:", None),
        "file-manager" => {
            let home = home().ok_or("the user profile folder is unknown")?;
            shell_open(&home.to_string_lossy(), None)
        }
        "browser" => shell_open(&default_browser().ok_or("no default browser is set")?, None),
        "terminal" => match windows_terminal() {
            Some(wt) => shell_open(&wt.to_string_lossy(), home().as_deref()),
            None => shell_open("powershell.exe", home().as_deref()),
        },
        other => Err(format!("unknown action {other}")),
    }
}
