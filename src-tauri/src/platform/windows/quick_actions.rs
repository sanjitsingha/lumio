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

/// The app an action opens, as its AppsFolder key and its name (the default
/// browser is only known by name), which find its icon in the app index.
pub fn app(id: &str) -> Option<(String, String)> {
    let app = |key: &str, name: &str| Some((key.to_string(), name.to_string()));
    match id {
        "system-settings" => super::system_settings::app_key().map(|key| (key, "Settings".into())),
        "calculator" => app("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", "Calculator"),
        "file-manager" => app("Microsoft.Windows.Explorer", "File Explorer"),
        "browser" => super::browser::resolve("").map(|b| (b.id, b.name)),
        "terminal" if windows_terminal().is_some() => app("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App", "Terminal"),
        // {1AC14E77-…} is the System32 folder
        "terminal" => app(r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe", "Windows PowerShell"),
        _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::apps::AppIcon;

    #[test]
    #[ignore = "reads the installed apps"]
    fn every_action_opens_an_indexed_app_with_an_icon() {
        let apps = crate::platform::apps::discover();
        for id in ["system-settings", "calculator", "file-manager", "browser", "terminal"] {
            let (key, name) = app(id).unwrap_or_else(|| panic!("{id} names no app"));
            let found = apps.iter().find(|a| (!key.is_empty() && a.key == key) || a.name.eq_ignore_ascii_case(&name));
            let found = found.unwrap_or_else(|| panic!("{id}: {key} / {name} is not in the app index"));
            assert!(matches!(&found.icon, AppIcon::Png(png) if png.starts_with(b"\x89PNG")), "{id}: no shell icon");
        }
    }
}
