//! Clipboard history on Windows, shown in Spotlight's Quick files (`clips.rs`
//! is Linux's, with the same functions). The clipboard's sequence number is
//! polled, which does not open the clipboard, and a copy is read when it moves.
//!
//! The history is saved in Lummo's data folder, with copied images beside it as
//! PNGs, and each copy is let go once it is older than the retention setting.
//! The folder keeps the name it had under the notch, so the history carries
//! over. Copies a password manager marks as private are never read, so never
//! saved.

use std::collections::hash_map::DefaultHasher;
use std::ffi::{c_void, OsString};
use std::hash::{Hash, Hasher};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::http::Response;
use tauri::{AppHandle, Manager};
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{GlobalFree, GENERIC_READ, HANDLE, HGLOBAL, HWND};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    HBITMAP, HGDIOBJ,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppRGBA, IWICBitmapSource, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom,
    WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable,
    OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{CF_BITMAP, CF_DIB, CF_HDROP, CF_UNICODETEXT, DROPEFFECT_COPY};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

use crate::app_state::AppState;
use crate::screenshots;
use crate::storage::json_store;
use crate::window_manager::SPOTLIGHT;

/// Kept in the history at most, however recent.
const MAX_CLIPS: usize = 50;
const POLL: Duration = Duration::from_millis(250);
const PRUNE_EVERY: Duration = Duration::from_secs(30);
/// Characters of a text sent to the page (all of it goes back on the clipboard).
const TEXT_PREVIEW: usize = 180;
/// Bigger images are skipped instead of read (about a 7000 x 6000 photo).
const MAX_PIXELS: u64 = 40_000_000;

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

/// The whole copy: what goes back on the clipboard. An image is its PNG in the store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Payload {
    Text(String),
    Image(PathBuf),
    Files(Vec<PathBuf>),
}

/// Histories saved by the notch also hold previews here, which are ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ClipId {
    id: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    /// Of what was copied, so copying the same thing again moves it up.
    fingerprint: u64,
    /// When it was last copied, in seconds since 1970.
    copied_at: u64,
    clip: ClipId,
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
            Payload::Text(text) => {
                let kind = if is_link(text) { "link" } else { "text" };
                (kind, Some(text.chars().take(TEXT_PREVIEW).collect()), None, 1)
            }
            Payload::Image(_) => ("image", None, None, 1),
            Payload::Files(paths) => {
                let name = paths.first().map(|p| {
                    p.file_name().map_or_else(|| p.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned())
                });
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
        ClipItem { id: self.clip.id, kind, at: self.copied_at.saturating_mul(1000), text, name, count }
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
        if let Some(pos) = self.entries.iter().position(|e| e.clip.id == id) {
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

    /// Lets go of copies older than `max_age` seconds; returns them.
    fn prune(&mut self, now: u64, max_age: u64) -> Vec<Entry> {
        let (keep, old) =
            std::mem::take(&mut self.entries).into_iter().partition(|e| now.saturating_sub(e.copied_at) <= max_age);
        self.entries = keep;
        old
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Debug)]
struct Store {
    dir: PathBuf,
}

impl Store {
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
                history.entries.retain(|e| !matches!(&e.payload, Payload::Image(image) if !image.exists()));
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

    fn save_image(&self, id: u64, png: &[u8]) -> Option<PathBuf> {
        let path = self.image_path(id);
        match std::fs::create_dir_all(&self.dir).and_then(|()| json_store::write_atomic(&path, png)) {
            Ok(()) => Some(path),
            Err(e) => {
                log::warn!("could not save a copied image: {e}");
                None
            }
        }
    }

    /// Deletes the saved images of copies let go, and nothing outside this folder.
    fn forget(&self, gone: &[Entry]) {
        for entry in gone {
            if let Payload::Image(image) = &entry.payload {
                if image.parent() == Some(self.dir.as_path()) {
                    let _ = std::fs::remove_file(image);
                }
            }
        }
    }
}

/// What was read off the clipboard, on its way to the history.
enum Raw {
    Text(String),
    Png(Vec<u8>),
    /// Width, height, straight RGBA.
    Pixels(u32, u32, Vec<u8>),
    Files(Vec<PathBuf>),
}

impl Raw {
    fn kind(&self) -> &'static str {
        match self {
            Raw::Text(_) => "text",
            Raw::Png(_) | Raw::Pixels(..) => "an image",
            Raw::Files(_) => "files",
        }
    }

    fn fingerprint(&self) -> u64 {
        let mut h = DefaultHasher::new();
        match self {
            Raw::Text(text) => (0u8, text).hash(&mut h),
            Raw::Png(bytes) => (1u8, bytes).hash(&mut h),
            Raw::Pixels(width, height, rgba) => (2u8, width, height, rgba).hash(&mut h),
            Raw::Files(paths) => (3u8, paths).hash(&mut h),
        }
        h.finish()
    }

    fn into_payload(self, id: u64, store: &Store) -> Option<Payload> {
        Some(match self {
            Raw::Text(text) => Payload::Text(text),
            Raw::Png(bytes) => {
                // must be readable, as it has to be decoded to go back on the clipboard
                decode_png(&bytes)?;
                Payload::Image(store.save_image(id, &bytes)?)
            }
            Raw::Pixels(width, height, rgba) => {
                let png = encode_png(width, height, &rgba)?;
                Payload::Image(store.save_image(id, &png)?)
            }
            Raw::Files(paths) => Payload::Files(paths),
        })
    }
}

struct ClipState {
    history: Mutex<History>,
    store: Store,
    /// The Spotlight window, as a number: it owns what Lummo puts on the clipboard.
    owner: isize,
    /// The clipboard's sequence number when last read. Held while reading or
    /// writing, so Lummo's own writes are never taken for new copies.
    seen: Mutex<u32>,
}

impl ClipState {
    fn lock(&self) -> MutexGuard<'_, History> {
        self.history.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn payload(&self, id: u64) -> Option<Payload> {
        self.lock().entries.iter().find(|e| e.clip.id == id).map(|e| e.payload.clone())
    }

    /// What has been copied since the last look, if anything.
    fn read_new(&self) -> Option<Raw> {
        let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        // the number changes with every copy, and reading it does not open the clipboard
        let sequence = unsafe { GetClipboardSequenceNumber() };
        if sequence == *seen {
            return None;
        }
        *seen = sequence;
        read()
    }

    /// Writes to the clipboard without the watcher taking it for a new copy.
    fn put(&self, formats: &[(u32, Vec<u8>)]) -> Result<(), String> {
        let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        write(formats, HWND(self.owner as *mut c_void))?;
        *seen = unsafe { GetClipboardSequenceNumber() };
        Ok(())
    }

    /// Adds a copy (or moves a repeat up) and saves the history.
    fn record(&self, raw: Raw) -> bool {
        let fingerprint = raw.fingerprint();
        let now = unix_now();
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
        let gone = history.insert(Entry { fingerprint, copied_at: now, clip: ClipId { id }, payload });
        self.store.forget(&gone);
        self.store.save(&history);
        true
    }

    fn prune(&self, max_age: u64) {
        let mut history = self.lock();
        let gone = history.prune(unix_now(), max_age);
        if !gone.is_empty() {
            self.store.forget(&gone);
            self.store.save(&history);
        }
    }
}

/// How long a copy stays, in seconds.
fn max_age(app: &AppHandle) -> u64 {
    app.state::<AppState>().settings().notch.keep_duration().as_secs()
}

/// Loads the history and starts watching the clipboard. Spotlight's window must exist.
pub fn start(app: &AppHandle) {
    let owner = match app.get_webview_window(SPOTLIGHT).map(|window| window.hwnd()) {
        Some(Ok(hwnd)) => hwnd.0 as isize,
        _ => {
            log::error!("clipboard history is off: no Spotlight window to own the clipboard");
            return;
        }
    };
    // beside Lummo's other data, which the file index never looks in
    let store = Store { dir: app.state::<AppState>().paths.data_dir.join("notch clipboard") };
    let history = store.load();
    let state = ClipState { history: Mutex::new(history), store, owner, seen: Mutex::new(0) };
    state.prune(max_age(app));
    app.manage(state);

    let worker = app.clone();
    let spawned = std::thread::Builder::new().name("lummo-clipboard".into()).spawn(move || watch(&worker));
    if let Err(e) = spawned {
        log::error!("clipboard history is off: {e}");
    }
}

/// What is on the clipboard now joins the history first.
fn watch(app: &AppHandle) {
    let state = app.state::<ClipState>();
    let mut pruned_at = Instant::now();
    log::info!("watching the clipboard");
    loop {
        if let Some(raw) = state.read_new() {
            // the kind only: what was copied stays out of the log
            let kind = raw.kind();
            if state.record(raw) {
                log::debug!("clipboard copy is {kind}");
            }
        }
        if pruned_at.elapsed() >= PRUNE_EVERY {
            pruned_at = Instant::now();
            state.prune(max_age(app));
        }
        std::thread::sleep(POLL);
    }
}

/// The history for the page, newest first.
pub fn list(app: &AppHandle) -> Vec<ClipItem> {
    app.try_state::<ClipState>().map(|state| state.lock().entries.iter().map(Entry::item).collect()).unwrap_or_default()
}

/// Puts a copy back on the clipboard and moves it to the front.
pub fn copy(app: &AppHandle, id: u64) -> Result<(), String> {
    let state = app.try_state::<ClipState>().ok_or("Clipboard history is off.")?;
    let payload = state.payload(id).ok_or("That copy is no longer there.")?;
    state.put(&formats(&payload)?)?;
    let mut history = state.lock();
    history.bump_id(id, unix_now());
    state.store.save(&history);
    Ok(())
}

/// Puts an image file on the clipboard, without it joining the history.
pub fn copy_image_file(app: &AppHandle, path: &Path) -> Result<(), String> {
    let state = app.try_state::<ClipState>().ok_or("The clipboard is not available.")?;
    let (width, height, rgba) = decode_image(path, None).ok_or("That image could not be read.")?;
    let png = encode_png(width, height, &rgba).ok_or("That image could not be copied.")?;
    state.put(&image_formats(png, width, height, &rgba))
}

/// `lummo-icon://localhost/clip/<id>`: only what is in the history can be asked for.
pub fn respond(app: &AppHandle, id: &str) -> Response<Vec<u8>> {
    screenshots::image_response(thumbnail(app, id))
}

fn thumbnail(app: &AppHandle, id: &str) -> Option<Vec<u8>> {
    let state = app.try_state::<ClipState>()?;
    match state.payload(id.parse().ok()?)? {
        Payload::Text(_) => None,
        Payload::Image(path) => screenshots::thumbnail_png(&path),
        Payload::Files(paths) => {
            let first = paths.into_iter().next()?;
            if screenshots::is_image(first.file_name()?.to_str()?) {
                screenshots::thumbnail_png(&first)
            } else {
                // Explorer's picture of it (a video's frame)
                let (width, height, rgba) = crate::platform::apps::file_thumbnail(&first, screenshots::THUMB_WIDTH)?;
                encode_png(width, height, &rgba)
            }
        }
    }
}

/// Open clipboard; closed again on drop.
struct OpenClipboardGuard;

impl OpenClipboardGuard {
    /// The app that just copied may still hold the clipboard open for a moment.
    fn open(owner: Option<HWND>) -> Option<Self> {
        for _ in 0..10 {
            if unsafe { OpenClipboard(owner) }.is_ok() {
                return Some(Self);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        log::debug!("the clipboard stayed busy");
        None
    }
}

impl Drop for OpenClipboardGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

// --- reading -----------------------------------------------------------------

/// Files come first (Explorer), then text, then images: an app that offers
/// both text and a picture of it (Excel cells) was copying the text.
fn read() -> Option<Raw> {
    let _open = OpenClipboardGuard::open(None)?;
    if is_private() {
        return None;
    }
    if let Some(paths) = read_files() {
        return Some(Raw::Files(paths));
    }
    if let Some(text) = read_text() {
        return Some(Raw::Text(text));
    }
    if let Some(png) = offered(w!("PNG")).and_then(global_bytes) {
        return Some(Raw::Png(png));
    }
    read_bitmap().map(|(width, height, rgba)| Raw::Pixels(width, height, rgba))
}

fn register(name: PCWSTR) -> u32 {
    unsafe { RegisterClipboardFormatW(name) }
}

/// A registered clipboard format, if something on the clipboard offers it.
fn offered(name: PCWSTR) -> Option<u32> {
    let id = register(name);
    (id != 0 && available(id)).then_some(id)
}

fn available(format: u32) -> bool {
    unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

/// Password managers mark their copies with these formats.
fn is_private() -> bool {
    if offered(w!("ExcludeClipboardContentFromMonitorProcessing")).is_some() || offered(w!("Clipboard Viewer Ignore")).is_some() {
        return true;
    }
    // a DWORD 0 here keeps the copy out of Windows' clipboard history
    offered(w!("CanIncludeInClipboardHistory"))
        .and_then(global_bytes)
        .is_some_and(|bytes| bytes.get(..4).is_some_and(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) == 0))
}

/// A copy of a memory-backed clipboard format's bytes.
fn global_bytes(format: u32) -> Option<Vec<u8>> {
    let handle = unsafe { GetClipboardData(format) }.ok()?;
    let memory = HGLOBAL(handle.0);
    let ptr = unsafe { GlobalLock(memory) } as *const u8;
    if ptr.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, GlobalSize(memory)) }.to_vec();
    let _ = unsafe { GlobalUnlock(memory) };
    Some(bytes)
}

fn read_text() -> Option<String> {
    let bytes = global_bytes(u32::from(CF_UNICODETEXT.0))?;
    let units: Vec<u16> = bytes.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).take_while(|&u| u != 0).collect();
    let text = String::from_utf16_lossy(&units);
    (!text.trim().is_empty()).then_some(text)
}

fn read_files() -> Option<Vec<PathBuf>> {
    if !available(u32::from(CF_HDROP.0)) {
        return None;
    }
    let hdrop = HDROP(unsafe { GetClipboardData(u32::from(CF_HDROP.0)) }.ok()?.0);
    let count = unsafe { DragQueryFileW(hdrop, u32::MAX, None) };
    let paths: Vec<PathBuf> = (0..count)
        .filter_map(|i| {
            let len = unsafe { DragQueryFileW(hdrop, i, None) } as usize;
            let mut buf = vec![0u16; len + 1];
            let copied = unsafe { DragQueryFileW(hdrop, i, Some(buf.as_mut_slice())) } as usize;
            (copied > 0).then(|| PathBuf::from(OsString::from_wide(&buf[..copied])))
        })
        .collect();
    (!paths.is_empty()).then_some(paths)
}

/// Any bitmap format, which Windows converts to `CF_BITMAP` on request. Read
/// as opaque: what such bitmaps carry in their alpha byte is unreliable.
fn read_bitmap() -> Option<(u32, u32, Vec<u8>)> {
    if !available(u32::from(CF_BITMAP.0)) {
        return None;
    }
    // owned by the clipboard: not deleted here
    let bitmap = HBITMAP(unsafe { GetClipboardData(u32::from(CF_BITMAP.0)) }.ok()?.0);
    let mut info = BITMAP::default();
    let got = unsafe { GetObjectW(HGDIOBJ(bitmap.0), std::mem::size_of::<BITMAP>() as i32, Some((&mut info as *mut BITMAP).cast())) };
    if got == 0 || info.bmWidth <= 0 || info.bmHeight == 0 {
        return None;
    }
    let (width, height) = (info.bmWidth as u32, info.bmHeight.unsigned_abs());
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        log::debug!("copied image is {width} x {height}; too big to keep");
        return None;
    }
    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32), // top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0u8; width as usize * height as usize * 4];
    let dc = unsafe { CreateCompatibleDC(None) };
    let lines = unsafe { GetDIBits(dc, bitmap, 0, height, Some(bgra.as_mut_ptr().cast()), &mut header, DIB_RGB_COLORS) };
    unsafe {
        let _ = DeleteDC(dc);
    }
    if lines == 0 {
        return None;
    }
    let rgba = bgra.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], 255]).collect();
    Some((width, height, rgba))
}

// --- writing -----------------------------------------------------------------

/// PNG keeps transparency for apps that take it; everything else pastes the bitmap.
fn image_formats(png: Vec<u8>, width: u32, height: u32, rgba: &[u8]) -> Vec<(u32, Vec<u8>)> {
    vec![(register(w!("PNG")), png), (u32::from(CF_DIB.0), dib(width, height, rgba))]
}

/// A copy in the formats apps paste from, prepared before the clipboard is opened
/// so it is held only briefly.
fn formats(payload: &Payload) -> Result<Vec<(u32, Vec<u8>)>, String> {
    Ok(match payload {
        Payload::Text(text) => vec![(u32::from(CF_UNICODETEXT.0), utf16_nul(text))],
        Payload::Image(image) => {
            let png = std::fs::read(image).map_err(|_| "That image is no longer there.")?;
            let (width, height, rgba) = decode_png(&png).ok_or("That image could not be read.")?;
            image_formats(png, width, height, &rgba)
        }
        Payload::Files(paths) => {
            let paths: Vec<PathBuf> = paths.iter().filter(|p| p.exists()).cloned().collect();
            if paths.is_empty() {
                return Err("Those files are no longer there.".into());
            }
            vec![
                (u32::from(CF_HDROP.0), drop_files(&paths)),
                // pasting in Explorer copies the files instead of moving them
                (register(w!("Preferred DropEffect")), DROPEFFECT_COPY.0.to_le_bytes().to_vec()),
            ]
        }
    })
}

fn write(formats: &[(u32, Vec<u8>)], owner: HWND) -> Result<(), String> {
    // with an owner window, or EmptyClipboard leaves the clipboard ownerless and SetClipboardData fails
    let _open = OpenClipboardGuard::open(Some(owner)).ok_or("The clipboard is busy.")?;
    unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
    for (format, bytes) in formats.iter().filter(|(format, _)| *format != 0) {
        set_global(*format, bytes)?;
    }
    Ok(())
}

/// Hands a copy of `bytes` to the clipboard, which then owns the memory.
fn set_global(format: u32, bytes: &[u8]) -> Result<(), String> {
    let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }.map_err(|e| e.to_string())?;
    let ptr = unsafe { GlobalLock(memory) } as *mut u8;
    if ptr.is_null() {
        let _ = unsafe { GlobalFree(Some(memory)) };
        return Err("out of memory".into());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        let _ = GlobalUnlock(memory);
    }
    if let Err(e) = unsafe { SetClipboardData(format, Some(HANDLE(memory.0))) } {
        let _ = unsafe { GlobalFree(Some(memory)) };
        return Err(e.to_string());
    }
    Ok(())
}

fn utf16_nul(text: &str) -> Vec<u8> {
    text.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
}

/// A `CF_DIB`: a 40-byte `BITMAPINFOHEADER`, then 32-bit BGRA rows bottom-up.
fn dib(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(40 + rgba.len());
    out.extend(40u32.to_le_bytes()); // biSize
    out.extend((width as i32).to_le_bytes());
    out.extend((height as i32).to_le_bytes()); // positive: bottom-up
    out.extend(1u16.to_le_bytes()); // biPlanes
    out.extend(32u16.to_le_bytes()); // biBitCount
    out.extend(BI_RGB.0.to_le_bytes());
    out.extend((rgba.len() as u32).to_le_bytes()); // biSizeImage
    out.extend([0u8; 16]); // resolution, palette sizes
    let row = width as usize * 4;
    for y in (0..height as usize).rev() {
        for p in rgba[y * row..(y + 1) * row].chunks_exact(4) {
            out.extend([p[2], p[1], p[0], p[3]]);
        }
    }
    out
}

/// A `CF_HDROP`: a 20-byte `DROPFILES` header, then NUL-terminated UTF-16
/// paths and a final NUL.
fn drop_files(paths: &[PathBuf]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(20u32.to_le_bytes()); // pFiles: where the list starts
    out.extend([0u8; 8]); // pt
    out.extend(0i32.to_le_bytes()); // fNC
    out.extend(1i32.to_le_bytes()); // fWide
    for path in paths {
        out.extend(path.as_os_str().encode_wide().chain([0]).flat_map(u16::to_le_bytes));
    }
    out.extend(0u16.to_le_bytes());
    out
}

// --- images ------------------------------------------------------------------

/// An image file (PNG, JPEG, WebP) as straight RGBA, through Windows' own
/// codecs, scaled down to fit `fit` if given.
pub(crate) fn decode_image(path: &Path, fit: Option<(u32, u32)>) -> Option<(u32, u32, Vec<u8>)> {
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let pixels = wic_pixels(path, fit);
    if initialized {
        unsafe { CoUninitialize() };
    }
    match pixels {
        Ok(pixels) => pixels,
        Err(e) => {
            log::debug!("could not decode an image: {e}");
            None
        }
    }
}

fn wic_pixels(path: &Path, fit: Option<(u32, u32)>) -> windows::core::Result<Option<(u32, u32, Vec<u8>)>> {
    unsafe {
        let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let decoder = factory.CreateDecoderFromFilename(
            &HSTRING::from(path.as_os_str()),
            None,
            GENERIC_READ,
            WICDecodeMetadataCacheOnDemand,
        )?;
        let frame = decoder.GetFrame(0)?;
        let (mut width, mut height) = (0, 0);
        frame.GetSize(&mut width, &mut height)?;
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
            return Ok(None);
        }
        let mut source: IWICBitmapSource = windows::core::Interface::cast(&frame)?;
        if let Some((max_width, max_height)) = fit {
            let (fit_width, fit_height) = fitted(width, height, max_width, max_height);
            if (fit_width, fit_height) != (width, height) {
                let scaler = factory.CreateBitmapScaler()?;
                scaler.Initialize(&source, fit_width, fit_height, WICBitmapInterpolationModeFant)?;
                source = windows::core::Interface::cast(&scaler)?;
                (width, height) = (fit_width, fit_height);
            }
        }
        // converted last: the scaler can hand back a different channel order than it was given
        let converter = factory.CreateFormatConverter()?;
        converter.Initialize(&source, &GUID_WICPixelFormat32bppRGBA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
        let stride = width * 4;
        let mut rgba = vec![0u8; stride as usize * height as usize];
        converter.CopyPixels(std::ptr::null(), stride, &mut rgba)?;
        Ok(Some((width, height, rgba)))
    }
}

/// The size that fits within the bounds at the same shape; never enlarges.
fn fitted(width: u32, height: u32, max_width: u32, max_height: u32) -> (u32, u32) {
    let scale = (f64::from(max_width) / f64::from(width)).min(f64::from(max_height) / f64::from(height)).min(1.0);
    (((f64::from(width) * scale).round() as u32).max(1), ((f64::from(height) * scale).round() as u32).max(1))
}

fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let (width, height) = (reader.info().width, reader.info().height);
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        log::debug!("copied image is {width} x {height}; too big to keep");
        return None;
    }
    let mut buf = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buf).ok()?;
    buf.truncate(frame.buffer_size());
    let rgba = match frame.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        // normalize_to_color8 expands palettes
        png::ColorType::Indexed => return None,
    };
    Some((frame.width, frame.height, rgba))
}

pub(crate) fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_entry(id: u64, copied_at: u64) -> Entry {
        Entry { fingerprint: id, copied_at, clip: ClipId { id }, payload: Payload::Text(format!("copy {id}")) }
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
            payload: Payload::Files(names.iter().map(|n| Path::new("C:\\Users\\me").join(n)).collect()),
            ..text_entry(1, 5)
        };
        assert_eq!(files(&["a.MP4"]).item().kind, "video");
        assert_eq!(files(&["a.png", "b.txt"]).item().kind, "image");
        let item = files(&["report.pdf", "b.txt"]).item();
        assert_eq!((item.kind, item.name.as_deref(), item.count), ("file", Some("report.pdf"), 2));
        assert_eq!(item.at, 5000, "the page counts in milliseconds");

        let long = Entry { payload: Payload::Text("é".repeat(1000)), ..text_entry(2, 5) };
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
        assert_eq!((history.entries[0].clip.id, history.entries[0].copied_at), (10, 1000));
        assert!(!history.bump(999, 1000));

        let gone = history.prune(1000, 10);
        assert_eq!(gone.len(), MAX_CLIPS - 1);
        assert_eq!(history.entries.len(), 1);
    }

    #[test]
    fn reads_a_history_saved_by_the_notch() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store { dir: dir.path().to_path_buf() };
        let image = store.image_path(2);
        std::fs::write(&image, b"png").unwrap();
        let saved = serde_json::json!({
            "entries": [
                { "fingerprint": 3, "copiedAt": 30,
                  "clip": { "id": 3, "kind": "files", "files": [{ "name": "a.txt", "path": "C:\\a.txt", "thumb": null, "icon": null }], "count": 1 },
                  "payload": { "files": ["C:\\a.txt"] } },
                { "fingerprint": 2, "copiedAt": 20,
                  "clip": { "id": 2, "kind": "image", "src": "data:image/png;base64,AA", "width": 1, "height": 1 },
                  "payload": { "image": image } },
                { "fingerprint": 1, "copiedAt": 10,
                  "clip": { "id": 1, "kind": "text", "text": "hello" },
                  "payload": { "text": "hello" } },
            ],
            "nextId": 3,
        });
        std::fs::write(store.history_path(), saved.to_string()).unwrap();

        let history = store.load();
        assert_eq!(history.next_id, 3);
        let kinds: Vec<_> = history.entries.iter().map(|e| (e.clip.id, e.item().kind)).collect();
        assert_eq!(kinds, [(3, "file"), (2, "image"), (1, "text")]);
        assert_eq!(history.entries[2].payload, Payload::Text("hello".into()));
    }

    #[test]
    fn the_history_survives_a_restart_without_missing_images() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store { dir: dir.path().to_path_buf() };
        let image = store.save_image(2, b"png bytes").unwrap();
        let mut history = History { next_id: 3, ..History::default() };
        history.insert(text_entry(1, 1));
        history.insert(Entry { payload: Payload::Image(image.clone()), ..text_entry(2, 2) });
        history.insert(Entry { payload: Payload::Image(store.image_path(3)), ..text_entry(3, 3) });
        store.save(&history);

        // the copy whose image is missing is dropped
        let loaded = store.load();
        let ids: Vec<u64> = loaded.entries.iter().map(|e| e.clip.id).collect();
        assert_eq!(ids, [2, 1]);
        assert_eq!(loaded.next_id, 3, "ids keep counting up after a restart");
    }

    #[test]
    fn an_unreadable_history_starts_afresh() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store { dir: dir.path().to_path_buf() };
        std::fs::write(store.history_path(), b"{ not json").unwrap();
        assert!(store.load().entries.is_empty());
        assert!(!store.history_path().exists(), "the unreadable file is moved aside, not overwritten");
    }

    #[test]
    fn forgetting_deletes_only_inside_the_folder() {
        let parent = tempfile::tempdir().unwrap();
        let dir = parent.path().join("clipboard");
        std::fs::create_dir(&dir).unwrap();
        let store = Store { dir };
        let inside = store.image_path(1);
        let outside = parent.path().join("keep.png");
        std::fs::write(&inside, b"png").unwrap();
        std::fs::write(&outside, b"png").unwrap();
        let entry = |id, path: &Path| Entry { payload: Payload::Image(path.into()), ..text_entry(id, 1) };

        store.forget(&[entry(1, &inside), entry(2, &outside)]);
        assert!(!inside.exists());
        assert!(outside.exists());
    }

    #[test]
    fn image_files_decode_and_scale_to_fit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shot.png");
        let rgba: Vec<u8> = [10, 20, 30, 255].repeat(800 * 400);
        std::fs::write(&path, encode_png(800, 400, &rgba).unwrap()).unwrap();

        let (width, height, full) = decode_image(&path, None).unwrap();
        assert_eq!((width, height), (800, 400));
        assert_eq!(full, rgba);

        let (width, height, small) = decode_image(&path, Some((384, 240))).unwrap();
        assert_eq!((width, height), (384, 192));
        assert_eq!(small.len(), 384 * 192 * 4);
        assert_eq!(&small[..4], &[10, 20, 30, 255]);

        assert!(decode_image(&dir.path().join("missing.png"), None).is_none());
    }

    #[test]
    fn small_images_are_not_enlarged() {
        assert_eq!(fitted(100, 50, 384, 240), (100, 50));
        assert_eq!(fitted(1000, 1000, 384, 240), (240, 240));
    }

    #[test]
    fn pngs_round_trip_through_the_decoder() {
        let rgba: Vec<u8> = (0..4 * 3 * 4).map(|i| i as u8).collect();
        let png = encode_png(4, 3, &rgba).unwrap();
        assert_eq!(decode_png(&png), Some((4, 3, rgba)));
    }

    #[test]
    fn dibs_are_bottom_up_bgra_after_a_40_byte_header() {
        // 1 x 2: red on top, blue below
        let out = dib(1, 2, &[255, 0, 0, 255, 0, 0, 255, 128]);
        assert_eq!(out.len(), 40 + 8);
        assert_eq!(&out[..4], &40u32.to_le_bytes());
        assert_eq!(&out[40..], &[255, 0, 0, 128, 0, 0, 255, 255]);
    }

    #[test]
    fn dropped_file_lists_are_wide_and_double_nul_terminated() {
        let out = drop_files(&[PathBuf::from("C:\\a"), PathBuf::from("C:\\bc")]);
        assert_eq!(&out[..4], &20u32.to_le_bytes());
        let units: Vec<u16> = out[20..].chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        assert_eq!(String::from_utf16(&units).unwrap(), "C:\\a\0C:\\bc\0\0");
        assert_eq!(utf16_nul("hi"), vec![b'h', 0, b'i', 0, 0, 0]);
    }
}
