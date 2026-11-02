//! Launching apps and opening files through GIO, which implements the
//! desktop-entry spec (field codes, `Terminal=true`, D-Bus activation) and
//! the user's default-application choices.

use std::path::Path;

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib::{self, ToVariant};

/// Variables injected by an AppImage runtime that must not leak into apps we launch.
const APPIMAGE_VARS: &[&str] = &[
    "APPDIR", "APPIMAGE", "ARGV0", "OWD", "LD_LIBRARY_PATH", "GDK_PIXBUF_MODULE_FILE", "GIO_MODULE_DIR",
    "GSETTINGS_SCHEMA_DIR", "GTK_PATH", "GTK_EXE_PREFIX", "GTK_DATA_PREFIX", "PYTHONHOME", "PYTHONPATH", "PERLLIB",
    "QT_PLUGIN_PATH",
];

fn launch_context() -> gio::AppLaunchContext {
    let ctx = gio::AppLaunchContext::new();
    if std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some() {
        for var in APPIMAGE_VARS {
            ctx.unsetenv(var);
        }
    }
    ctx.unsetenv("LUMMO_DEV_SNAPSHOT_DIR");
    ctx
}

/// `target` is the path of the application's `.desktop` file.
pub fn launch_app(target: &str) -> Result<(), String> {
    let info = gio::DesktopAppInfo::from_filename(target).ok_or("the application entry can no longer be read")?;
    info.launch(&[], Some(&launch_context())).map_err(|e| e.message().to_string())
}

/// Runs one of the entry's `[Desktop Action …]` groups (GIO handles `Exec`
/// field codes and D-Bus activation the same way as for the app itself).
pub fn launch_app_action(target: &str, action: &str) -> Result<(), String> {
    let info = gio::DesktopAppInfo::from_filename(target).ok_or("the application entry can no longer be read")?;
    if !info.list_actions().iter().any(|a| a.as_str() == action) {
        return Err("the application no longer offers this action".into());
    }
    info.launch_action(action, Some(&launch_context()));
    Ok(())
}

/// Opens `uri` with a specific app (e.g. the browser chosen for web searches).
pub fn launch_uri_with(info: &impl IsA<gio::AppInfo>, uri: &str) -> Result<(), String> {
    info.launch_uris(&[uri], Some(&launch_context())).map_err(|e| e.message().to_string())
}

pub fn launch_app_info(info: &gio::AppInfo, files: &[gio::File]) -> Result<(), String> {
    info.launch(files, Some(&launch_context())).map_err(|e| e.message().to_string())
}

/// Opens a file with its default application, or a folder in the file manager.
pub fn open_path(path: &Path) -> Result<(), String> {
    let uri = gio::File::for_path(path).uri();
    gio::AppInfo::launch_default_for_uri(&uri, Some(&launch_context())).map_err(|e| e.message().to_string())
}

/// Opens an `https://` URL in the default browser.
pub fn open_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("only https links can be opened".into());
    }
    gio::AppInfo::launch_default_for_uri(url, Some(&launch_context())).map_err(|e| e.message().to_string())
}

/// Shows the item selected in the file manager (`org.freedesktop.FileManager1`),
/// falling back to opening the containing folder.
pub fn reveal_path(path: &Path) -> Result<(), String> {
    let uri = gio::File::for_path(path).uri().to_string();
    let shown = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).and_then(|conn| {
        conn.call_sync(
            Some("org.freedesktop.FileManager1"),
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1",
            "ShowItems",
            Some(&(vec![uri], "").to_variant()),
            None::<&glib::VariantTy>,
            gio::DBusCallFlags::NONE,
            3000,
            gio::Cancellable::NONE,
        )
    });
    match shown {
        Ok(_) => Ok(()),
        Err(e) => {
            log::info!("FileManager1.ShowItems unavailable ({e}); opening the parent folder");
            open_path(path.parent().unwrap_or(path))
        }
    }
}

const EXECUTABLE_EXTENSIONS: &[&str] = &["desktop", "appimage", "run", "sh", "bash", "bin", "out", "x86_64", "exe", "msi", "jar", "deb", "rpm"];

/// Files that would *run code* when opened. Such results are revealed in the
/// file manager instead of being opened, so a single Enter can never start
/// an untrusted program.
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if path.is_dir() {
        return false;
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => EXECUTABLE_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(ext)),
        // no extension: an ELF binary or script with the execute bit
        None => path.metadata().is_ok_and(|m| m.permissions().mode() & 0o111 != 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn executable_detection() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("tool");
        std::fs::write(&script, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let doc = dir.path().join("report.pdf");
        std::fs::write(&doc, b"%PDF").unwrap();
        // documents on FAT/NTFS mounts often carry +x; they must still open normally
        std::fs::set_permissions(&doc, std::fs::Permissions::from_mode(0o777)).unwrap();
        let installer = dir.path().join("Setup.AppImage");
        std::fs::write(&installer, b"x").unwrap();
        assert!(is_executable(&script));
        assert!(!is_executable(&doc));
        assert!(is_executable(&installer));
        assert!(!is_executable(dir.path()));
    }

    #[test]
    fn only_https_urls_open() {
        assert!(open_url("file:///etc/passwd").is_err());
        assert!(open_url("javascript:alert(1)").is_err());
    }
}
