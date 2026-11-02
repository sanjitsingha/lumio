//! Launch at sign-in via `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

use windows::Win32::System::Registry::HKEY_CURRENT_USER;

use super::registry;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

fn command() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(format!("\"{}\" --autostart", exe.display()))
}

/// The Run value is named after the app id so renaming the product keeps it unique.
pub fn set_enabled(app_id: &str, _name: &str, enabled: bool) -> Result<(), String> {
    if enabled {
        registry::write_string(HKEY_CURRENT_USER, RUN_KEY, app_id, &command()?)?;
        log::info!("registered for launch at sign-in");
    } else {
        registry::delete_value(HKEY_CURRENT_USER, RUN_KEY, app_id)?;
        log::info!("removed launch at sign-in");
    }
    Ok(())
}
