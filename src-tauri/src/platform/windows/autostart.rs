//! Launch at sign-in: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//! for the installer's copy; the package's startup task for the Microsoft
//! Store's, which cannot use the Run key.

use windows::core::HSTRING;
use windows::ApplicationModel::{StartupTask, StartupTaskState};
use windows::Win32::System::Registry::HKEY_CURRENT_USER;

use super::{registry, toggles};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// `TaskId` of the startup task in `src-tauri/msix/AppxManifest.xml`.
const STARTUP_TASK: &str = "LummoStartup";

fn command() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(format!("\"{}\" --autostart", exe.display()))
}

/// The Run value is named after the app id so renaming the product keeps it unique.
pub fn set_enabled(app_id: &str, _name: &str, enabled: bool) -> Result<(), String> {
    if super::is_packaged() {
        return set_startup_task(enabled);
    }
    if enabled {
        registry::write_string(HKEY_CURRENT_USER, RUN_KEY, app_id, &command()?)?;
        log::info!("registered for launch at sign-in");
    } else {
        registry::delete_value(HKEY_CURRENT_USER, RUN_KEY, app_id)?;
        log::info!("removed launch at sign-in");
    }
    Ok(())
}

/// Task Manager's Startup apps shows the same switch, and what is turned off
/// there only the user can turn back on.
fn set_startup_task(enabled: bool) -> Result<(), String> {
    toggles::with_com(move || {
        let message = |e: windows::core::Error| e.message();
        let task = StartupTask::GetAsync(&HSTRING::from(STARTUP_TASK)).and_then(|op| op.join()).map_err(message)?;
        if !enabled {
            task.Disable().map_err(message)?;
            log::info!("startup task off");
            return Ok(());
        }
        match task.RequestEnableAsync().and_then(|op| op.join()).map_err(message)? {
            StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy => {
                log::info!("startup task on");
                Ok(())
            }
            StartupTaskState::DisabledByUser => {
                Err("Lummo is turned off in Task Manager › Startup apps; turn it on there.".into())
            }
            StartupTaskState::DisabledByPolicy => Err("Your organization does not allow Lummo to open at sign-in.".into()),
            _ => Err("Windows did not turn on opening at sign-in.".into()),
        }
    })
}
