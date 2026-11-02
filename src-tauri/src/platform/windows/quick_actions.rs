//! Quick actions on Windows.

use std::path::PathBuf;

use super::browser::default_executable as default_browser;
use super::launch::shell_open;

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
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
