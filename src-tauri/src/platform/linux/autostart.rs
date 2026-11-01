//! Launch at login via an XDG autostart entry
//! (`$XDG_CONFIG_HOME/autostart/<id>.desktop`).

use std::path::PathBuf;

use crate::storage::json_store;

fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

pub fn entry_path(app_id: &str) -> Option<PathBuf> {
    config_home().map(|c| c.join("autostart").join(format!("{app_id}.desktop")))
}

/// Quotes an argument for a desktop entry `Exec` key (quoting rules, then the
/// string-escaping rules applied on top of them).
pub fn quote_exec_arg(arg: &str) -> String {
    const RESERVED: &[char] = &[' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(', ')', '`'];
    if !arg.contains(RESERVED) {
        return arg.to_string();
    }
    let mut quoted = String::from("\"");
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' => {
                quoted.push('\\');
                quoted.push(c);
            }
            // a literal backslash is "\\" after quoting, "\\\\" after string escaping
            '\\' => quoted.push_str("\\\\\\\\"),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

fn executable() -> Result<String, String> {
    // inside an AppImage the stable path is the image itself
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        return Ok(appimage.to_string_lossy().into_owned());
    }
    std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).map_err(|e| e.to_string())
}

pub fn render_entry(name: &str, exec: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName={name}\nComment=Spotlight-style search and Caps Lock indicator\n\
         Exec={} --autostart\nIcon=lumio\nTerminal=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n\
         X-GNOME-Autostart-Delay=2\n",
        quote_exec_arg(exec)
    )
}

pub fn is_enabled(app_id: &str) -> bool {
    entry_path(app_id)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|text| !text.lines().any(|l| l.trim() == "Hidden=true" || l.trim() == "X-GNOME-Autostart-enabled=false"))
}

pub fn set_enabled(app_id: &str, name: &str, enabled: bool) -> Result<(), String> {
    let path = entry_path(app_id).ok_or("could not determine the autostart folder")?;
    if enabled {
        let exec = executable()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
        }
        json_store::write_atomic(&path, render_entry(name, &exec).as_bytes())
            .map_err(|e| format!("could not write {}: {e}", path.display()))?;
        log::info!("autostart entry written to {}", path.display());
    } else {
        match std::fs::remove_file(&path) {
            Ok(()) => log::info!("autostart entry removed"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("could not remove {}: {e}", path.display())),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_per_desktop_entry_spec() {
        assert_eq!(quote_exec_arg("/usr/bin/lumio"), "/usr/bin/lumio");
        assert_eq!(quote_exec_arg("/opt/My Apps/lumio"), "\"/opt/My Apps/lumio\"");
        assert_eq!(quote_exec_arg("/tmp/a$b"), "\"/tmp/a\\$b\"");
        assert_eq!(quote_exec_arg(r"/tmp/a\b"), "\"/tmp/a\\\\\\\\b\"");
    }

    #[test]
    fn entry_contains_autostart_flag() {
        let text = render_entry("Lumio", "/opt/Lumio App/lumio");
        assert!(text.contains("Exec=\"/opt/Lumio App/lumio\" --autostart"));
        assert!(text.starts_with("[Desktop Entry]"));
    }
}
