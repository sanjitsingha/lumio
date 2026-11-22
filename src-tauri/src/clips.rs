//! Clipboard history on Linux, shown in Spotlight's Quick files (the Windows
//! notch has its own, `notch/clipboard.rs`). Wayland hides the clipboard from
//! apps without focus, so copies are watched through XWayland, where the
//! compositor mirrors it. What goes back on the clipboard goes through the
//! app's own display.
//!
//! The history is saved in a private folder in Lummo's data folder, with copied
//! images beside it as PNGs, and each copy is let go once it is older than the
//! notch's retention setting. Copies a password manager marks as secret are
//! never read, so never saved.

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gtk::gdk;
use gtk::gdk_pixbuf::{Colorspace, Pixbuf};
use gtk::gio::{self, prelude::*};
use gtk::glib;
use serde::{Deserialize, Serialize};
use tauri::http::Response;
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::screenshots;
use crate::storage::json_store;

/// Kept in the history at most, however recent.
const MAX_CLIPS: usize = 50;
/// Longer texts are skipped.
const MAX_TEXT: usize = 1_000_000;
/// Bigger images are skipped (about a 7000 x 6000 photo).
const MAX_PIXELS: u64 = 40_000_000;
/// Characters of a text sent to the page (all of it goes back on the clipboard).
const TEXT_PREVIEW: usize = 180;
/// Apps often claim the clipboard several times for one copy.
const SETTLE: Duration = Duration::from_millis(80);
/// How long a change of the clipboard counts as Lummo's own re-copy.
const OWN_COPY: Duration = Duration::from_millis(600);
const PRUNE_EVERY: Duration = Duration::from_secs(30);
/// Marks a copy as secret (KeePassXC, KDE and others).
const SECRET_FORMAT: &str = "x-kde-passwordManagerHint";

/// Until when a clipboard change is Lummo putting a copy back.
static OWN_COPY_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

/// What the watcher should read of the formats a copy offers.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Offered {
    Files,
    /// With whether an image is offered too.
    Text { image: bool },
    Image,
}

fn is_text_format(format: &str) -> bool {
    matches!(format, "UTF8_STRING" | "STRING" | "TEXT" | "COMPOUND_TEXT") || format.starts_with("text/plain")
}

fn offered<S: AsRef<str>>(formats: &[S], with_files: bool) -> Option<Offered> {
    let has = |test: &dyn Fn(&str) -> bool| formats.iter().any(|f| test(f.as_ref()));
    if has(&|f| f == SECRET_FORMAT) {
        return None;
    }
    let image = has(&|f| f.starts_with("image/"));
    if with_files && has(&|f| f == "text/uri-list") {
        Some(Offered::Files)
    } else if has(&is_text_format) {
        Some(Offered::Text { image })
    } else if image {
        Some(Offered::Image)
    } else {
        None
    }
}

/// One web address and nothing else.
fn is_link(text: &str) -> bool {
    let text = text.trim();
    let address = text.strip_prefix("https://").or_else(|| text.strip_prefix("http://"));
    address.is_some_and(|a| !a.is_empty()) && !text.chars().any(char::is_whitespace)
}

fn is_video(name: &str) -> bool {
    let extension = Path::new(name).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    matches!(extension.as_deref(), Some("mp4" | "mkv" | "webm" | "mov" | "avi" | "m4v" | "mpg" | "mpeg" | "wmv" | "flv"))
}

/// A copy as read from the clipboard, on its way to the history.
enum Raw {
    Text(String),
    Files(Vec<PathBuf>),
    Image { pixels: glib::Bytes, width: i32, height: i32, stride: i32, alpha: bool },
}

impl Raw {
    fn kind(&self) -> &'static str {
        match self {
            Raw::Text(_) => "text",
            Raw::Files(_) => "files",
            Raw::Image { .. } => "an image",
        }
    }

    fn fingerprint(&self) -> u64 {
        let mut hash = DefaultHasher::new();
        match self {
            Raw::Text(text) => (0u8, text).hash(&mut hash),
            Raw::Files(paths) => (1u8, paths).hash(&mut hash),
            Raw::Image { pixels, width, height, stride, alpha } => {
                (2u8, width, height).hash(&mut hash);
                // rows may be padded, and the padding is not the picture
                let row = *width as usize * if *alpha { 4 } else { 3 };
                for line in pixels.chunks(*stride as usize) {
                    line[..row.min(line.len())].hash(&mut hash);
                }
            }
        }
        hash.finish()
    }

    fn into_payload(self, id: u64, store: &Store) -> Option<Payload> {
        Some(match self {
            Raw::Text(text) => Payload::Text { text },
            Raw::Files(paths) => Payload::Files { paths },
            Raw::Image { pixels, width, height, stride, alpha } => {
                let image = Pixbuf::from_bytes(&pixels, Colorspace::Rgb, alpha, 8, width, height, stride);
                let png = image.save_to_bufferv("png", &[]).ok()?;
                let path = store.image_path(id);
                if let Err(e) = json_store::write_atomic(&path, &png) {
                    log::warn!("could not save a copied image: {e}");
                    return None;
                }
                Payload::Image { path }
            }
        })
    }
}

/// The whole copy: what goes back on the clipboard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Payload {
    Text { text: String },
    Image { path: PathBuf },
    Files { paths: Vec<PathBuf> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    id: u64,
    /// Of what was copied, so copying the same thing again moves it up.
    fingerprint: u64,
    /// When it was last copied, in milliseconds since 1970.
    copied_at: u64,
    payload: Payload,
}

/// What the page shows of a copy.
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipItem {
    id: u64,
    /// `text`, `link`, `image`, `video` or `file`.
    kind: &'static str,
    /// Milliseconds since 1970.
    at: u64,
    text: Option<String>,
    /// The first file's name.
    name: Option<String>,
    /// How many files the copy holds.
    count: usize,
}

impl Entry {
    fn item(&self) -> ClipItem {
        let (kind, text, name, count) = match &self.payload {
            Payload::Text { text } => {
                let kind = if is_link(text) { "link" } else { "text" };
                (kind, Some(text.chars().take(TEXT_PREVIEW).collect()), None, 1)
            }
            Payload::Image { .. } => ("image", None, None, 1),
            Payload::Files { paths } => {
                let name = paths.first().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
                let first = name.as_deref().unwrap_or_default();
                let kind = if screenshots::is_image(first) {
                    "image"
                } else if is_video(first) {
                    "video"
                } else {
                    "file"
                };
                (kind, None, name, paths.len())
            }
        };
        ClipItem { id: self.id, kind, at: self.copied_at, text, name, count }
    }
}

/// Newest first.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct History {
    entries: Vec<Entry>,
    next_id: u64,
}

impl History {
    fn move_to_front(&mut self, pos: usize, now: u64) {
        let mut entry = self.entries.remove(pos);
        entry.copied_at = now;
        self.entries.insert(0, entry);
    }

    fn bump(&mut self, fingerprint: u64, now: u64) -> bool {
        let pos = self.entries.iter().position(|e| e.fingerprint == fingerprint);
        pos.map(|pos| self.move_to_front(pos, now)).is_some()
    }

    fn bump_id(&mut self, id: u64, now: u64) {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            self.move_to_front(pos, now);
        }
    }

    fn reserve_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Puts a new copy first; returns the oldest ones it pushes out.
    fn insert(&mut self, entry: Entry) -> Vec<Entry> {
        self.entries.insert(0, entry);
        self.entries.split_off(self.entries.len().min(MAX_CLIPS))
    }

    /// Lets go of copies older than `max_age` milliseconds; returns them.
    fn prune(&mut self, now: u64, max_age: u64) -> Vec<Entry> {
        let (keep, old) =
            std::mem::take(&mut self.entries).into_iter().partition(|e| now.saturating_sub(e.copied_at) <= max_age);
        self.entries = keep;
        old
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[derive(Debug)]
struct Store {
    dir: PathBuf,
}

impl Store {
    /// The folder, readable by this user only: copies can be private.
    fn prepare(&self) -> std::io::Result<()> {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&self.dir)?;
        std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700))
    }

    fn history_path(&self) -> PathBuf {
        self.dir.join("history.json")
    }

    fn image_path(&self, id: u64) -> PathBuf {
        self.dir.join(format!("Copied image {id}.png"))
    }

    /// The saved history, without images that have gone missing. An unreadable
    /// file is moved aside and the history starts empty.
    fn load(&self) -> History {
        let path = self.history_path();
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("could not read the clipboard history: {e}");
                }
                return History::default();
            }
        };
        match serde_json::from_slice::<History>(&bytes) {
            Ok(mut history) => {
                history.entries.retain(|e| !matches!(&e.payload, Payload::Image { path } if !path.exists()));
                history
            }
            Err(e) => {
                log::warn!("the clipboard history was unreadable ({e}); starting afresh");
                let _ = json_store::quarantine(&path, "corrupt");
                History::default()
            }
        }
    }

    fn save(&self, history: &History) {
        if let Err(e) = json_store::write_json_atomic(&self.history_path(), history) {
            log::warn!("could not save the clipboard history: {e}");
        }
    }

    /// Deletes the saved images of copies let go, and nothing outside this folder.
    fn forget(&self, gone: &[Entry]) {
        for entry in gone {
            if let Payload::Image { path } = &entry.payload {
                if path.parent() == Some(self.dir.as_path()) {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
}

struct ClipState {
    history: Mutex<History>,
    store: Store,
}

impl ClipState {
    fn lock(&self) -> MutexGuard<'_, History> {
        self.history.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn payload(&self, id: u64) -> Option<Payload> {
        self.lock().entries.iter().find(|e| e.id == id).map(|e| e.payload.clone())
    }

    /// Adds a copy (or moves a repeat up) and saves the history.
    fn record(&self, raw: Raw) -> bool {
        let fingerprint = raw.fingerprint();
        let now = now_ms();
        {
            let mut history = self.lock();
            if history.bump(fingerprint, now) {
                self.store.save(&history);
                return true;
            }
        }
        // images are encoded without holding the lock
        let id = self.lock().reserve_id();
        let Some(payload) = raw.into_payload(id, &self.store) else { return false };
        let mut history = self.lock();
        let gone = history.insert(Entry { id, fingerprint, copied_at: now, payload });
        self.store.forget(&gone);
        self.store.save(&history);
        true
    }

    fn prune(&self, max_age: u64) {
        let mut history = self.lock();
        let gone = history.prune(now_ms(), max_age);
        if !gone.is_empty() {
            self.store.forget(&gone);
            self.store.save(&history);
        }
    }
}

fn max_age(app: &AppHandle) -> u64 {
    app.state::<AppState>().settings().notch.keep_duration().as_millis() as u64
}

/// Loads the history and starts watching the clipboard.
pub fn start(app: &AppHandle) {
    let store = Store { dir: app.state::<AppState>().paths.data_dir.join("clipboard") };
    if let Err(e) = store.prepare() {
        log::error!("clipboard history is off: could not prepare its folder: {e}");
        return;
    }
    let history = store.load();
    let state = ClipState { history: Mutex::new(history), store };
    state.prune(max_age(app));
    app.manage(state);

    let (captured, received) = mpsc::channel();
    let worker = app.clone();
    let spawned = std::thread::Builder::new().name("lummo-clipboard".into()).spawn(move || record_copies(&worker, received));
    if let Err(e) = spawned {
        log::error!("clipboard history is off: {e}");
        return;
    }
    if let Err(e) = app.run_on_main_thread(move || watch(captured)) {
        log::error!("clipboard history is off: {e}");
    }
}

fn record_copies(app: &AppHandle, copies: Receiver<Raw>) {
    let state = app.state::<ClipState>();
    let mut pruned_at = Instant::now();
    loop {
        match copies.recv_timeout(PRUNE_EVERY) {
            Ok(raw) => {
                // the kind only: what was copied stays out of the log
                let kind = raw.kind();
                if state.record(raw) {
                    log::debug!("clipboard copy is {kind}");
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if pruned_at.elapsed() >= PRUNE_EVERY {
            pruned_at = Instant::now();
            state.prune(max_age(app));
        }
    }
}

/// The X11 display the clipboard is watched through: the session's own on X11, XWayland's on Wayland.
fn x11_display() -> Option<gdk::Display> {
    let default = gdk::Display::default()?;
    if default.type_().name() == "GdkX11Display" {
        return Some(default);
    }
    let name = std::env::var("DISPLAY").ok().filter(|d| !d.is_empty())?;
    gdk::Display::open(&name)
}

/// Main thread. What is on the clipboard now joins the history first.
fn watch(captured: Sender<Raw>) {
    let Some(display) = x11_display() else {
        log::warn!("clipboard history is off: no X11 display to watch the clipboard through");
        return;
    };
    let clipboard = gtk::Clipboard::for_display(&display, &gdk::SELECTION_CLIPBOARD);
    read(&clipboard, captured.clone());

    let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
    let watched = clipboard.clone();
    clipboard.connect_local("owner-change", false, move |_| {
        if let Some(source) = pending.borrow_mut().take() {
            source.remove();
        }
        let (slot, clipboard, captured) = (pending.clone(), watched.clone(), captured.clone());
        let source = glib::timeout_add_local_once(SETTLE, move || {
            slot.borrow_mut().take();
            read(&clipboard, captured);
        });
        *pending.borrow_mut() = Some(source);
        None
    });
    log::info!("watching the clipboard");
}

fn is_own_copy() -> bool {
    OWN_COPY_UNTIL.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|until| Instant::now() < until)
}

/// The next clipboard change is Lummo putting a copy back, not a new copy.
pub fn expect_own_copy() {
    *OWN_COPY_UNTIL.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now() + OWN_COPY);
}

fn read(clipboard: &gtk::Clipboard, captured: Sender<Raw>) {
    if is_own_copy() {
        return;
    }
    clipboard.request_contents(&gdk::Atom::intern("TARGETS"), move |clipboard, data| {
        let formats: Vec<String> = data.targets().unwrap_or_default().iter().map(|a| a.name().to_string()).collect();
        match offered(&formats, true) {
            Some(Offered::Files) => read_files(clipboard, captured, formats),
            other => read_offered(clipboard, captured, other),
        }
    });
}

fn read_offered(clipboard: &gtk::Clipboard, captured: Sender<Raw>, what: Option<Offered>) {
    match what {
        Some(Offered::Text { image }) => read_text(clipboard, captured, image),
        Some(Offered::Image) => read_image(clipboard, captured),
        Some(Offered::Files) | None => {}
    }
}

/// `text/uri-list` also carries web links: those are read as what else is offered.
fn read_files(clipboard: &gtk::Clipboard, captured: Sender<Raw>, formats: Vec<String>) {
    clipboard.request_uris(move |clipboard, uris| {
        let local = !uris.is_empty() && uris.iter().all(|uri| uri.starts_with("file://"));
        let paths: Vec<PathBuf> = if local { uris.iter().filter_map(|uri| gio::File::for_uri(uri).path()).collect() } else { Vec::new() };
        if paths.is_empty() {
            read_offered(clipboard, captured, offered(&formats, false));
        } else {
            let _ = captured.send(Raw::Files(paths));
        }
    });
}

/// "Copy image" in a browser offers the picture and its address: the picture is what was copied.
fn read_text(clipboard: &gtk::Clipboard, captured: Sender<Raw>, image: bool) {
    clipboard.request_text(move |clipboard, text| {
        let Some(text) = text else { return };
        if image && is_link(text) {
            read_image(clipboard, captured);
        } else if !text.trim().is_empty() && text.len() <= MAX_TEXT {
            let _ = captured.send(Raw::Text(text.to_string()));
        }
    });
}

fn read_image(clipboard: &gtk::Clipboard, captured: Sender<Raw>) {
    clipboard.request_image(move |_, image| {
        let Some(image) = image else { return };
        let (width, height) = (image.width(), image.height());
        if width <= 0 || height <= 0 || width as u64 * height as u64 > MAX_PIXELS {
            return;
        }
        let pixels = image.read_pixel_bytes();
        let _ = captured.send(Raw::Image { pixels, width, height, stride: image.rowstride(), alpha: image.has_alpha() });
    });
}

/// The history for the page, newest first.
pub fn list(app: &AppHandle) -> Vec<ClipItem> {
    app.try_state::<ClipState>().map(|state| state.lock().entries.iter().map(Entry::item).collect()).unwrap_or_default()
}

/// Puts a copy back on the clipboard and moves it to the front.
pub fn copy(app: &AppHandle, id: u64) -> Result<(), String> {
    let state = app.try_state::<ClipState>().ok_or("Clipboard history is off.")?;
    let payload = state.payload(id).ok_or("That copy is no longer there.")?;
    expect_own_copy();
    let (done, result) = mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = done.send(put_on_clipboard(&payload));
    })
    .map_err(|e| e.to_string())?;
    result.recv().map_err(|e| e.to_string())??;
    let mut history = state.lock();
    history.bump_id(id, now_ms());
    state.store.save(&history);
    Ok(())
}

/// Main thread.
fn put_on_clipboard(payload: &Payload) -> Result<(), String> {
    let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
    match payload {
        Payload::Text { text } => clipboard.set_text(text),
        Payload::Image { path } => {
            let image = Pixbuf::from_file(path).map_err(|e| e.message().to_string())?;
            clipboard.set_image(&image);
        }
        Payload::Files { paths } => {
            let paths: Vec<PathBuf> = paths.iter().filter(|p| p.exists()).cloned().collect();
            if paths.is_empty() {
                return Err("Those files are no longer there.".into());
            }
            set_files(&clipboard, &paths)?;
        }
    }
    Ok(())
}

/// Files, the way file managers offer them, so they paste as files; a text field gets their paths.
fn set_files(clipboard: &gtk::Clipboard, paths: &[PathBuf]) -> Result<(), String> {
    const GNOME_FILES: u32 = 0;
    const URI_LIST: u32 = 1;
    const PLAIN: u32 = 2;
    let uris: Vec<String> = paths.iter().map(|p| gio::File::for_path(p).uri().to_string()).collect();
    let plain = paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>().join("\n");
    let flags = gtk::TargetFlags::empty();
    let targets = [
        gtk::TargetEntry::new("x-special/gnome-copied-files", flags, GNOME_FILES),
        gtk::TargetEntry::new("text/uri-list", flags, URI_LIST),
        gtk::TargetEntry::new("UTF8_STRING", flags, PLAIN),
        gtk::TargetEntry::new("text/plain;charset=utf-8", flags, PLAIN),
    ];
    let set = clipboard.set_with_data(&targets, move |_, selection, info| match info {
        GNOME_FILES => {
            let body = format!("copy\n{}", uris.join("\n"));
            selection.set(&selection.target(), 8, body.as_bytes());
        }
        URI_LIST => {
            let refs: Vec<&str> = uris.iter().map(String::as_str).collect();
            selection.set_uris(&refs);
        }
        _ => {
            selection.set_text(&plain);
        }
    });
    if set {
        Ok(())
    } else {
        Err("The files could not be copied.".into())
    }
}

/// A picture of the file the system already has a thumbnail for (videos, mostly).
fn cached_thumbnail(path: &Path) -> Option<PathBuf> {
    let info = gio::File::for_path(path)
        .query_info("thumbnail::path", gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
        .ok()?;
    info.attribute_byte_string("thumbnail::path").map(|p| PathBuf::from(p.as_str()))
}

fn thumbnail(app: &AppHandle, id: &str) -> Option<Vec<u8>> {
    let state = app.try_state::<ClipState>()?;
    let source = match state.payload(id.parse().ok()?)? {
        Payload::Text { .. } => return None,
        Payload::Image { path } => path,
        Payload::Files { paths } => {
            let first = paths.into_iter().next()?;
            if screenshots::is_image(first.file_name()?.to_str()?) {
                first
            } else {
                cached_thumbnail(&first)?
            }
        }
    };
    screenshots::thumbnail_png(&source)
}

/// `lummo-icon://localhost/clip/<id>`: only what is in the history can be asked for.
pub fn respond(app: &AppHandle, id: &str) -> Response<Vec<u8>> {
    screenshots::image_response(thumbnail(app, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_entry(id: u64, copied_at: u64) -> Entry {
        Entry { id, fingerprint: id, copied_at, payload: Payload::Text { text: format!("copy {id}") } }
    }

    #[test]
    fn reads_the_most_useful_format() {
        assert_eq!(offered(&["text/uri-list", "UTF8_STRING"], true), Some(Offered::Files));
        assert_eq!(offered(&["text/uri-list", "UTF8_STRING"], false), Some(Offered::Text { image: false }));
        assert_eq!(offered(&["UTF8_STRING", "image/png"], true), Some(Offered::Text { image: true }));
        assert_eq!(offered(&["text/html", "image/png"], true), Some(Offered::Image));
        assert_eq!(offered(&["TARGETS", "TIMESTAMP"], true), None);
    }

    #[test]
    fn secret_copies_are_never_read() {
        assert_eq!(offered(&["UTF8_STRING", SECRET_FORMAT], true), None);
    }

    #[test]
    fn tells_links_from_text() {
        assert!(is_link("https://example.com/a?b=c"));
        assert!(is_link("  http://example.com\n"));
        assert!(!is_link("see https://example.com"));
        assert!(!is_link("https://"));
        assert!(!is_link("example.com"));
    }

    #[test]
    fn items_say_what_kind_of_copy_they_are() {
        let files = |names: &[&str]| Entry {
            id: 1,
            fingerprint: 1,
            copied_at: 5,
            payload: Payload::Files { paths: names.iter().map(|n| Path::new("/home/me").join(n)).collect() },
        };
        assert_eq!(files(&["a.MP4"]).item().kind, "video");
        assert_eq!(files(&["a.png", "b.txt"]).item().kind, "image");
        let item = files(&["report.pdf", "b.txt"]).item();
        assert_eq!((item.kind, item.name.as_deref(), item.count), ("file", Some("report.pdf"), 2));

        let long = Entry { payload: Payload::Text { text: "é".repeat(1000) }, ..text_entry(2, 5) };
        assert_eq!(long.item().text.unwrap().chars().count(), TEXT_PREVIEW);
    }

    #[test]
    fn repeats_move_up_and_old_copies_go() {
        let mut history = History::default();
        for id in 1..=(MAX_CLIPS as u64 + 2) {
            let gone = history.insert(text_entry(id, id));
            assert_eq!(gone.len(), usize::from(id > MAX_CLIPS as u64));
        }
        assert_eq!(history.entries.len(), MAX_CLIPS);

        assert!(history.bump(10, 1000));
        assert_eq!((history.entries[0].id, history.entries[0].copied_at), (10, 1000));
        assert!(!history.bump(999, 1000));

        let gone = history.prune(1000, 10);
        assert_eq!(gone.len(), MAX_CLIPS - 1);
        assert_eq!(history.entries.len(), 1);
    }

    #[test]
    fn the_history_survives_a_restart_in_a_private_folder() {
        let parent = tempfile::tempdir().unwrap();
        let store = Store { dir: parent.path().join("clipboard") };
        store.prepare().unwrap();
        assert_eq!(std::fs::metadata(&store.dir).unwrap().permissions().mode() & 0o777, 0o700);

        let image = store.image_path(2);
        std::fs::write(&image, b"png").unwrap();
        let mut history = History::default();
        history.insert(text_entry(1, 1));
        history.insert(Entry { id: 2, fingerprint: 2, copied_at: 2, payload: Payload::Image { path: image.clone() } });
        history.insert(Entry {
            id: 3,
            fingerprint: 3,
            copied_at: 3,
            payload: Payload::Image { path: store.image_path(3) },
        });
        store.save(&history);

        // the copy whose image is missing is dropped
        let ids: Vec<u64> = store.load().entries.iter().map(|e| e.id).collect();
        assert_eq!(ids, [2, 1]);
    }

    #[test]
    fn forgetting_deletes_only_inside_the_folder() {
        let parent = tempfile::tempdir().unwrap();
        let store = Store { dir: parent.path().join("clipboard") };
        store.prepare().unwrap();
        let inside = store.image_path(1);
        let outside = parent.path().join("keep.png");
        std::fs::write(&inside, b"png").unwrap();
        std::fs::write(&outside, b"png").unwrap();
        let entry = |id, path: &Path| Entry { id, fingerprint: id, copied_at: 1, payload: Payload::Image { path: path.into() } };

        store.forget(&[entry(1, &inside), entry(2, &outside)]);
        assert!(!inside.exists());
        assert!(outside.exists());
    }
}
