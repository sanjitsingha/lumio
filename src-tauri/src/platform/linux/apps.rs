//! Application discovery from XDG desktop entries.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use gtk::gio;
use gtk::gio::prelude::*;

use super::desktop_entry::{self, DesktopEntry};
use super::icon_theme::{icon_base_dirs, IconResolver};
use crate::search::apps::{AppIcon, DiscoveredApp};

pub fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("/usr/local/share"))
}

/// `$XDG_DATA_DIRS`, plus Flatpak/Snap export folders in case the session
/// did not add them.
pub fn data_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into())
        .split(':')
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .collect();
    let mut extra = vec![PathBuf::from("/var/lib/flatpak/exports/share"), PathBuf::from("/var/lib/snapd/desktop")];
    extra.insert(0, data_home().join("flatpak/exports/share"));
    for dir in extra {
        if dir.is_dir() && !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}

/// Application folders in precedence order (user first).
pub fn application_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![data_home().join("applications")];
    dirs.extend(data_dirs().into_iter().map(|d| d.join("applications")));
    let mut seen = HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

pub fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn icon_theme_name() -> String {
    let from_gsettings = gio::SettingsSchemaSource::default()
        .and_then(|src| src.lookup("org.gnome.desktop.interface", true))
        .filter(|schema| schema.has_key("icon-theme"))
        .map(|schema| gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None).string("icon-theme").to_string());
    from_gsettings.filter(|s| !s.is_empty()).unwrap_or_else(|| "hicolor".into())
}

fn collect_desktop_files(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>, depth: usize) {
    let Ok(reader) = std::fs::read_dir(dir) else { return };
    for item in reader.flatten() {
        let path = item.path();
        let name = item.file_name().to_string_lossy().into_owned();
        if path.is_dir() && depth < 3 {
            collect_desktop_files(&path, &format!("{prefix}{name}-"), out, depth + 1);
        } else if name.ends_with(".desktop") {
            out.push((format!("{prefix}{name}"), path));
        }
    }
}

/// Name of the program an `Exec` line runs (e.g. `code` for VS Code), used
/// as an extra search keyword.
pub fn exec_program(exec: &str) -> Option<String> {
    let mut skip_next = false;
    for token in tokenize(exec) {
        let token = token.as_str();
        if skip_next {
            skip_next = false;
            continue;
        }
        if token == "env" || token.contains('=') || token.starts_with('%') || token.starts_with('-') {
            continue;
        }
        if token.ends_with("/flatpak") || token == "flatpak" || token == "run" {
            continue;
        }
        if token == "--command" {
            skip_next = true;
            continue;
        }
        let base = token.rsplit('/').next().unwrap_or(token);
        // flatpak app ids like org.gnome.Calculator -> calculator
        let base = if base.matches('.').count() >= 2 { base.rsplit('.').next().unwrap_or(base) } else { base };
        return (!base.is_empty()).then(|| base.to_lowercase());
    }
    None
}

/// Splits an `Exec` value into arguments, honouring double quotes.
fn tokenize(exec: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => quoted = !quoted,
            '\\' if quoted => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn to_app(id: String, path: &Path, entry: DesktopEntry, icons: &IconResolver) -> DiscoveredApp {
    // a generic name that only repeats the app name ("Terminal") says nothing; prefer the comment
    let generic = entry.generic_name.clone().filter(|g| !g.eq_ignore_ascii_case(&entry.name));
    let description = generic.or(entry.comment.clone()).unwrap_or_default();
    let mut keywords = entry.keywords.clone();
    if let Some(program) = entry.exec.as_deref().and_then(exec_program) {
        if !entry.name.to_lowercase().contains(&program) {
            keywords.push(program);
        }
    }
    if let (Some(generic), Some(comment)) = (&entry.generic_name, &entry.comment) {
        if generic != comment {
            keywords.push(comment.clone());
        }
    }
    let icon = entry
        .icon
        .as_deref()
        .and_then(|name| icons.lookup(name, 64, 1))
        .map(AppIcon::File)
        .unwrap_or(AppIcon::None);
    DiscoveredApp { key: id, name: entry.name, description, keywords, icon, target: path.to_string_lossy().into_owned() }
}

pub fn discover() -> Vec<DiscoveredApp> {
    let locales = desktop_entry::current_locales();
    let desktops = current_desktops();
    let icons = IconResolver::new(&icon_theme_name(), &icon_base_dirs());
    let mut seen = HashSet::new();
    let mut apps = Vec::new();
    for dir in application_dirs() {
        let mut files = Vec::new();
        collect_desktop_files(&dir, "", &mut files, 0);
        for (id, path) in files {
            // the first occurrence wins, even when it hides the entry (Hidden=true masks system files)
            if !seen.insert(id.clone()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Some(entry) = desktop_entry::parse(&text, &locales) else { continue };
            if entry.is_visible(&desktops) {
                apps.push(to_app(id, &path, entry, &icons));
            }
        }
    }
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_program_extracts_the_binary() {
        assert_eq!(exec_program("/usr/share/code/code --unity-launch %F").as_deref(), Some("code"));
        assert_eq!(exec_program("env GDK_BACKEND=x11 firefox %u").as_deref(), Some("firefox"));
        assert_eq!(
            exec_program("/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=gnome-calculator org.gnome.Calculator").as_deref(),
            Some("calculator")
        );
        assert_eq!(exec_program("\"/opt/My App/my-app\" %U").as_deref(), Some("my-app"));
        assert_eq!(exec_program("%U"), None);
        assert_eq!(tokenize(r#"a "b c" "d\"e""#), vec!["a", "b c", "d\"e"]);
    }

    #[test]
    fn discovers_installed_apps_on_this_machine() {
        // Smoke test against the real system: every Linux desktop has some apps.
        let apps = discover();
        assert!(!apps.is_empty(), "no desktop entries found in {:?}", application_dirs());
        assert!(apps.iter().all(|a| !a.name.is_empty() && a.target.ends_with(".desktop")));
    }
}
