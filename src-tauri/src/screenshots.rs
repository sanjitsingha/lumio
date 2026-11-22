//! Spotlight's Quick files: the newest screenshots in `Pictures/Screenshots`.
//! Only image files directly in that folder can be listed, shown or opened, so
//! a file name from the webview never reaches anything else on disk.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use tauri::http::{header, Response, StatusCode};
use tauri::{AppHandle, Manager};

use crate::search::hex_key;

/// How many of the newest screenshots Spotlight lists.
const MAX_LISTED: usize = 30;
/// Thumbnails are twice the size they are drawn at (`QuickPanel.jsx`), for sharp HiDPI.
#[cfg(target_os = "linux")]
const THUMB_WIDTH: i32 = 384;
#[cfg(target_os = "linux")]
const THUMB_HEIGHT: i32 = 240;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub name: String,
    /// The name in hex, for the thumbnail's URL.
    pub key: String,
    /// Milliseconds since the epoch.
    pub modified: u64,
}

fn folder(app: &AppHandle) -> Option<PathBuf> {
    app.path().picture_dir().ok().map(|pictures| pictures.join("Screenshots"))
}

pub(crate) fn is_image(name: &str) -> bool {
    let extension = Path::new(name).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    matches!(extension.as_deref(), Some("png" | "jpg" | "jpeg" | "webp"))
}

/// The newest image files directly in `dir`. Links are skipped: they could point anywhere.
fn newest_in(dir: &Path, limit: usize) -> Vec<Screenshot> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut shots: Vec<Screenshot> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !is_image(&name) || !entry.file_type().ok()?.is_file() {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
            Some(Screenshot { key: hex_key(&name), name, modified: modified.as_millis() as u64 })
        })
        .collect();
    shots.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| b.name.cmp(&a.name)));
    shots.truncate(limit);
    shots
}

/// `name` inside `dir`, if it is a bare file name of an image that is a real file there.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn resolve(dir: &Path, name: &str) -> Option<PathBuf> {
    if !is_image(name) || Path::new(name).file_name()?.to_str()? != name {
        return None;
    }
    let path = dir.join(name);
    std::fs::symlink_metadata(&path).ok()?.is_file().then_some(path)
}

pub fn list(app: &AppHandle) -> Vec<Screenshot> {
    folder(app).map(|dir| newest_in(&dir, MAX_LISTED)).unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn path_of(app: &AppHandle, name: &str) -> Option<PathBuf> {
    resolve(&folder(app)?, name)
}

#[cfg(target_os = "linux")]
fn thumbnail(app: &AppHandle, key: &str) -> Option<Vec<u8>> {
    let name = crate::search::unhex_key(key)?;
    thumbnail_png(&path_of(app, &name)?)
}

/// A small PNG of an image file.
#[cfg(target_os = "linux")]
pub(crate) fn thumbnail_png(path: &Path) -> Option<Vec<u8>> {
    let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, THUMB_WIDTH, THUMB_HEIGHT, true).ok()?;
    pixbuf.save_to_bufferv("png", &[]).ok()
}

/// Quick files is Linux's stand-in for the Windows notch.
#[cfg(not(target_os = "linux"))]
fn thumbnail(_app: &AppHandle, _key: &str) -> Option<Vec<u8>> {
    None
}

/// Puts the full-size screenshot on the clipboard. GTK's clipboard and its
/// image belong to the main thread; Lummo keeps offering the image from there.
#[cfg(target_os = "linux")]
pub fn copy(app: &AppHandle, name: &str) -> Result<(), String> {
    let path = path_of(app, name).ok_or("That screenshot is no longer there.")?;
    // it is already listed here, so it should not join the clipboard history too
    crate::clips::expect_own_copy();
    let (done, result) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let copied = gtk::gdk_pixbuf::Pixbuf::from_file(&path).map(|image| {
            gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).set_image(&image);
        });
        let _ = done.send(copied.map_err(|e| e.message().to_string()));
    })
    .map_err(|e| e.to_string())?;
    result.recv().map_err(|e| e.to_string())?
}

#[cfg(not(target_os = "linux"))]
pub fn copy(_app: &AppHandle, _name: &str) -> Result<(), String> {
    Err("Quick files is only available on Linux.".into())
}

/// `lummo-icon://localhost/screenshot/<hex name>`. The page adds the file's
/// modified time to the URL, so a cached thumbnail is never out of date.
pub fn respond(app: &AppHandle, key: &str) -> Response<Vec<u8>> {
    image_response(thumbnail(app, key))
}

/// A thumbnail for the webview, or "not found".
pub(crate) fn image_response(png: Option<Vec<u8>>) -> Response<Vec<u8>> {
    match png {
        Some(png) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::CACHE_CONTROL, "max-age=86400")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(png),
        None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()),
    }
    .unwrap_or_else(|_| Response::new(Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::time::{Duration, SystemTime};

    fn touch(dir: &Path, name: &str, age_secs: u64) {
        let file = File::create(dir.join(name)).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(age_secs)).unwrap();
    }

    #[test]
    fn lists_only_images_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "old.png", 300);
        touch(dir.path(), "new.PNG", 10);
        touch(dir.path(), "middle.jpg", 100);
        touch(dir.path(), "notes.txt", 1);
        std::fs::create_dir(dir.path().join("folder.png")).unwrap();

        let names: Vec<_> = newest_in(dir.path(), 10).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["new.PNG", "middle.jpg", "old.png"]);
        assert_eq!(newest_in(dir.path(), 1).len(), 1);
    }

    #[test]
    fn a_missing_folder_is_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        assert!(newest_in(&dir.path().join("missing"), 10).is_empty());
    }

    #[test]
    fn resolves_only_images_directly_in_the_folder() {
        let parent = tempfile::tempdir().unwrap();
        let dir = parent.path().join("Screenshots");
        std::fs::create_dir(&dir).unwrap();
        touch(&dir, "shot.png", 1);
        touch(&dir, "notes.txt", 1);
        touch(parent.path(), "secret.png", 1);

        assert_eq!(resolve(&dir, "shot.png"), Some(dir.join("shot.png")));
        for name in ["notes.txt", "missing.png", "../secret.png", "sub/shot.png", "/etc/shot.png", ".."] {
            assert_eq!(resolve(&dir, name), None, "{name}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn links_are_neither_listed_nor_resolved() {
        let parent = tempfile::tempdir().unwrap();
        let dir = parent.path().join("Screenshots");
        std::fs::create_dir(&dir).unwrap();
        touch(parent.path(), "secret.png", 1);
        std::os::unix::fs::symlink(parent.path().join("secret.png"), dir.join("link.png")).unwrap();

        assert!(newest_in(&dir, 10).is_empty());
        assert_eq!(resolve(&dir, "link.png"), None);
    }
}
