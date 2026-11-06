//! Resolves quick actions to the apps installed on this desktop.

use gtk::gio;
use gtk::gio::prelude::*;

use super::launch::launch_app_info;

const SETTINGS: &[&str] = &[
    "io.elementary.settings.desktop",
    "io.elementary.switchboard.desktop",
    "org.gnome.Settings.desktop",
    "gnome-control-center.desktop",
    "systemsettings.desktop",
    "org.kde.systemsettings.desktop",
    "xfce-settings-manager.desktop",
    "cinnamon-settings.desktop",
    "mate-control-center.desktop",
];

const CALCULATORS: &[&str] = &[
    "io.elementary.calculator.desktop",
    "org.gnome.Calculator.desktop",
    "gnome-calculator.desktop",
    "org.kde.kcalc.desktop",
    "kcalc.desktop",
    "qalculate-gtk.desktop",
    "galculator.desktop",
    "mate-calc.desktop",
];

const TERMINALS: &[&str] = &[
    "io.elementary.terminal.desktop",
    "org.gnome.Ptyxis.desktop",
    "org.gnome.Console.desktop",
    "org.gnome.Terminal.desktop",
    "org.kde.konsole.desktop",
    "xfce4-terminal.desktop",
    "com.mitchellh.ghostty.desktop",
    "kitty.desktop",
    "Alacritty.desktop",
    "org.wezfurlong.wezterm.desktop",
    "xterm.desktop",
];

fn first_installed(ids: &[&str]) -> Option<gio::AppInfo> {
    ids.iter().find_map(|id| gio::DesktopAppInfo::new(id)).map(|d| d.upcast())
}

fn any_terminal() -> Option<gio::AppInfo> {
    first_installed(TERMINALS).or_else(|| {
        gio::AppInfo::all().into_iter().find(|info| {
            info.downcast_ref::<gio::DesktopAppInfo>()
                .and_then(|d| d.categories())
                .is_some_and(|c| c.split(';').any(|cat| cat == "TerminalEmulator"))
        })
    })
}

fn resolve(id: &str) -> Option<(gio::AppInfo, Vec<gio::File>)> {
    match id {
        "system-settings" => first_installed(SETTINGS).map(|a| (a, vec![])),
        "calculator" => first_installed(CALCULATORS).map(|a| (a, vec![])),
        "terminal" => any_terminal().map(|a| (a, vec![])),
        "browser" => gio::AppInfo::default_for_uri_scheme("https").map(|a| (a, vec![])),
        "file-manager" => {
            let home = std::env::var_os("HOME").map(gio::File::for_path)?;
            gio::AppInfo::default_for_type("inode/directory", false).map(|a| (a, vec![home]))
        }
        _ => None,
    }
}

pub fn is_available(id: &str) -> bool {
    resolve(id).is_some()
}

/// The app an action opens, as its desktop-file id (its key in the app
/// index) and its name, which find its icon there.
pub fn app(id: &str) -> Option<(String, String)> {
    let (app, _) = resolve(id)?;
    Some((app.id()?.to_string(), app.name().to_string()))
}

pub fn run(id: &str) -> Result<(), String> {
    let (app, files) = resolve(id).ok_or_else(|| "No suitable application is installed.".to_string())?;
    log::info!("quick action {id} -> {}", app.name());
    launch_app_info(&app, &files)
}
