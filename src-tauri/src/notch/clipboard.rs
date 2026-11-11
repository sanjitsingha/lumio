//! Clipboard history for the notch: watches the Windows clipboard and
//! keeps the last copies (text, images, files), newest first. The history is
//! saved in Lummo's data folder (`history.json`, with copied images beside it
//! as PNGs), so it outlives restarts, and each copy is let go once it is older
//! than Settings → Notch allows (`NotchSettings`). Clicking a copy in the
//! notch puts it back on the clipboard; files and images can be dragged out of
//! it into other apps.
//!
//! Copies that a password manager marks as private (the formats Windows' own
//! clipboard history honours) are never read, so never saved.

use std::collections::hash_map::DefaultHasher;
use std::ffi::{c_void, OsString};
use std::hash::{Hash, Hasher};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Listener, Manager};
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    HBITMAP, HGDIOBJ,
};
use windows::Win32::System::Com::{IBindCtx, IDataObject};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable,
    OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{IDropSource, CF_BITMAP, CF_DIB, CF_HDROP, CF_UNICODETEXT, DROPEFFECT_COPY};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    DragQueryFileW, ILCreateFromPathW, ILFree, SHCreateShellItemArrayFromIDLists, SHDoDragDrop, BHID_DataObject, HDROP,
};

use super::NOTCH;
use crate::app_state::AppState;
use crate::events;
use crate::search::apps::AppIcon;
use crate::storage::json_store;

/// Kept in the history at most, however recent (`MAX_CLIPS` in `pages/Notch.jsx`).
const MAX_CLIPS: usize = 50;
const POLL: Duration = Duration::from_millis(250);
/// How often copies past their time are looked for; a settings change looks at once.
const PRUNE_EVERY: Duration = Duration::from_secs(30);
/// Characters of a copied text shown in the notch (all of it goes back on the clipboard).
const TEXT_PREVIEW: usize = 400;
/// Thumbnail bounds in pixels: twice the 62 css px items in `ClipItem.jsx`, for sharp HiDPI.
const THUMB_HEIGHT: u32 = 124;
const THUMB_MAX_WIDTH: u32 = 240;
/// Bigger images are skipped instead of read (about a 7000 x 6000 photo).
const MAX_PIXELS: u64 = 40_000_000;
/// Files of one copy listed in its tooltip; only the first gets a picture.
const FILES_LISTED: usize = 10;

/// Sent to the notch page: something was copied (new, or a repeat moved to
/// the front), which the page shows arriving.
const NOTCH_CLIP: &str = "notch-clip";
/// Sent to the notch page: a copy joined or moved to the front without being
/// copied just now (what was on the clipboard at start, or a copy clicked in
/// the notch going back on the clipboard), so without the arrival.
const NOTCH_CLIP_QUIET: &str = "notch-clip-quiet";
/// Sent to the notch page: the whole history, newest first.
const NOTCH_CLIPS: &str = "notch-clips";
/// From the notch page once it listens: asks for `NOTCH_CLIPS`.
const NOTCH_READY: &str = "notch-ready";
/// From the notch page, `{ id }`: put that copy back on the clipboard.
const NOTCH_COPY: &str = "notch-copy";
/// From the notch page, `{ id }`: the mouse is dragging that copy out.
const NOTCH_DRAG: &str = "notch-drag";

/// What the page shows of a copy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Content {
    Text { text: String },
    /// `src` is a PNG thumbnail as a `data:` URL; `width`/`height` are the copied image's.
    Image { src: String, width: u32, height: u32 },
    /// The first files of the copy (only the first with a picture), and how many there are in all.
    Files { files: Vec<FileClip>, count: usize },
}

impl Content {
    fn kind(&self) -> &'static str {
        match self {
            Content::Text { .. } => "text",
            Content::Image { .. } => "an image",
            Content::Files { .. } => "files",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileClip {
    name: String,
    path: String,
    /// Explorer's thumbnail (photos, videos, PDFs) as a PNG `data:` URL.
    thumb: Option<String>,
    /// The file's type icon as a PNG `data:` URL, for files without a thumbnail.
    icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    id: u64,
    #[serde(flatten)]
    content: Content,
}

/// What goes back on the clipboard, or out by drag: the whole copy, not its
/// preview. An image is its PNG in the store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Payload {
    Text(String),
    Image(PathBuf),
    Files(Vec<PathBuf>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    /// Of what was copied, so copying the same thing again moves it up instead of adding it twice.
    fingerprint: u64,
    /// When it was last copied (or clicked in the notch), in seconds since 1970.
    copied_at: u64,
    clip: Clip,
    payload: Payload,
}

/// Newest first.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct History {
    entries: Vec<Entry>,
    next_id: u64,
}

impl History {
    fn move_to_front(&mut self, pos: usize, now: u64) -> &Entry {
        let mut entry = self.entries.remove(pos);
        entry.copied_at = now;
        self.entries.insert(0, entry);
        &self.entries[0]
    }

    /// Moves an earlier copy with this fingerprint to the front, as if copied `now`.
    fn bump(&mut self, fingerprint: u64, now: u64) -> Option<Clip> {
        let pos = self.entries.iter().position(|e| e.fingerprint == fingerprint)?;
        Some(self.move_to_front(pos, now).clip.clone())
    }

    fn bump_id(&mut self, id: u64, now: u64) -> Option<Clip> {
        let pos = self.entries.iter().position(|e| e.clip.id == id)?;
        Some(self.move_to_front(pos, now).clip.clone())
    }

    fn payload(&self, id: u64) -> Option<Payload> {
        self.entries.iter().find(|e| e.clip.id == id).map(|e| e.payload.clone())
    }

    /// The id the next copy gets (its image is saved under it before it joins).
    fn reserve_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Puts a new copy first; returns the oldest ones it pushes past `MAX_CLIPS`.
    fn insert(&mut self, entry: Entry) -> Vec<Entry> {
        self.entries.insert(0, entry);
        self.entries.split_off(self.entries.len().min(MAX_CLIPS))
    }

    /// Lets go of copies last copied more than `max_age` seconds before `now`; returns them.
    fn prune(&mut self, now: u64, max_age: u64) -> Vec<Entry> {
        let (keep, old) = std::mem::take(&mut self.entries).into_iter().partition(|e| now.saturating_sub(e.copied_at) <= max_age);
        self.entries = keep;
        old
    }

    fn snapshot(&self) -> Vec<Clip> {
        self.entries.iter().map(|e| e.clip.clone()).collect()
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Where the history lives between runs: `history.json`, and the copied
/// images beside it as `Copied image <id>.png` (the name a dragged-out image
/// arrives with).
#[derive(Debug, Clone)]
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

    /// The saved history, without copies whose image has gone missing. An
    /// unreadable file is moved aside and the history starts empty.
    fn load(&self) -> History {
        let path = self.history_path();
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return History::default(),
            Err(e) => {
                log::warn!("could not read the notch's clipboard history: {e}");
                return History::default();
            }
        };
        match serde_json::from_slice::<History>(&bytes) {
            Ok(mut history) => {
                history.entries.retain(|e| !matches!(&e.payload, Payload::Image(image) if !image.exists()));
                history
            }
            Err(e) => {
                log::warn!("the notch's clipboard history was unreadable ({e}); starting afresh");
                let _ = json_store::quarantine(&path, "corrupt");
                History::default()
            }
        }
    }

    fn save(&self, history: &History) {
        if let Err(e) = json_store::write_json_atomic(&self.history_path(), history) {
            log::warn!("could not save the notch's clipboard history: {e}");
        }
    }

    fn save_image(&self, id: u64, png: &[u8]) -> Option<PathBuf> {
        let path = self.image_path(id);
        match std::fs::create_dir_all(&self.dir).and_then(|()| std::fs::write(&path, png)) {
            Ok(()) => Some(path),
            Err(e) => {
                log::warn!("could not save a copied image: {e}");
                None
            }
        }
    }

    /// Deletes what the store kept for copies that left the history.
    fn forget(&self, gone: &[Entry]) {
        for entry in gone {
            if let Payload::Image(image) = &entry.payload {
                let _ = std::fs::remove_file(image);
            }
        }
    }
}

/// What was read off the clipboard, before previews are made.
enum Raw {
    Text(String),
    Png(Vec<u8>),
    /// Width, height, straight RGBA.
    Pixels(u32, u32, Vec<u8>),
    Files(Vec<PathBuf>),
}

impl Raw {
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

    /// The preview and payload of copy `id`; an image is saved in `store`.
    fn into_entry(self, id: u64, store: &Store) -> Option<(Content, Payload)> {
        match self {
            Raw::Text(text) => {
                let preview = text.trim().chars().take(TEXT_PREVIEW).collect();
                Some((Content::Text { text: preview }, Payload::Text(text)))
            }
            Raw::Png(bytes) => {
                let (width, height, rgba) = decode_png(&bytes)?;
                let content = image_content(width, height, &rgba)?;
                Some((content, Payload::Image(store.save_image(id, &bytes)?)))
            }
            Raw::Pixels(width, height, rgba) => {
                let png = encode_png(width, height, &rgba, png::Compression::Fast)?;
                let content = image_content(width, height, &rgba)?;
                Some((content, Payload::Image(store.save_image(id, &png)?)))
            }
            Raw::Files(paths) => {
                let files = paths.iter().take(FILES_LISTED).enumerate().map(|(i, path)| file_clip(path, i == 0)).collect();
                Some((Content::Files { files, count: paths.len() }, Payload::Files(paths)))
            }
        }
    }
}

#[derive(Deserialize)]
struct ClipRef {
    id: u64,
}

fn clip_ref(payload: &str) -> Option<u64> {
    serde_json::from_str::<ClipRef>(payload).ok().map(|r| r.id)
}

/// Work for the watcher thread besides watching.
enum Command {
    /// Put this copy back on the clipboard (the thread then knows to skip it when reading).
    Copy(u64),
    /// Settings changed: let go of copies past their time now.
    Prune,
}

/// The longest a copy stays, from Settings → Notch, in seconds.
fn max_age(app: &AppHandle) -> u64 {
    app.state::<AppState>().settings().notch.keep_duration().as_secs()
}

/// Starts watching the clipboard, with the history saved in `dir`; the
/// clipboard's current content joins it first. `owner` (the notch window)
/// owns what the notch puts back on the clipboard and is where drags start.
pub fn watch(app: &AppHandle, owner: HWND, dir: PathBuf) {
    let store = Store { dir };
    let mut saved = store.load();
    let gone = saved.prune(unix_now(), max_age(app));
    if !gone.is_empty() {
        store.forget(&gone);
        store.save(&saved);
    }
    let history = Arc::new(Mutex::new(saved));
    // HWND is a raw pointer; it crosses threads as a number
    let owner = owner.0 as isize;

    let (handle, clips) = (app.clone(), history.clone());
    app.listen(NOTCH_READY, move |_| {
        let _ = handle.emit_to(NOTCH, NOTCH_CLIPS, lock(&clips).snapshot());
    });

    let (commands, command_requests) = mpsc::channel::<Command>();
    let copies = commands.clone();
    app.listen(NOTCH_COPY, move |event| {
        if let Some(id) = clip_ref(event.payload()) {
            let _ = copies.send(Command::Copy(id));
        }
    });
    app.listen(events::SETTINGS_CHANGED, move |_| {
        let _ = commands.send(Command::Prune);
    });

    let (handle, clips) = (app.clone(), history.clone());
    app.listen(NOTCH_DRAG, move |event| {
        let Some(payload) = clip_ref(event.payload()).and_then(|id| lock(&clips).payload(id).map(|p| (id, p))) else {
            return;
        };
        let handle = handle.clone();
        // off the webview's event callback: the drag runs a modal loop
        std::thread::spawn(move || {
            let Some(paths) = drag_paths(&payload.1) else { return };
            let _ = handle.run_on_main_thread(move || drag_files(&paths, HWND(owner as *mut c_void)));
        });
    });

    let app = app.clone();
    let spawned = std::thread::Builder::new().name("notch-clipboard".into()).spawn(move || {
        let mut seen = 0;
        // what is on the clipboard when Lummo starts is not a new copy
        let mut starting = true;
        let mut pruned_at = Instant::now();
        loop {
            let mut prune = pruned_at.elapsed() >= PRUNE_EVERY;
            match command_requests.recv_timeout(POLL) {
                Ok(Command::Copy(id)) => {
                    let payload = lock(&history).payload(id);
                    match payload.map(|p| write(&p, HWND(owner as *mut c_void))) {
                        Some(Ok(())) => {
                            // our own write is not a new copy, but the clicked one moves to the front
                            seen = unsafe { GetClipboardSequenceNumber() };
                            let mut h = lock(&history);
                            if let Some(clip) = h.bump_id(id, unix_now()) {
                                store.save(&h);
                                let _ = app.emit_to(NOTCH, NOTCH_CLIP_QUIET, clip);
                            }
                        }
                        Some(Err(e)) => log::warn!("could not put copy {id} back on the clipboard: {e}"),
                        None => {}
                    }
                }
                Ok(Command::Prune) => prune = true,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            if prune {
                pruned_at = Instant::now();
                let mut h = lock(&history);
                let gone = h.prune(unix_now(), max_age(&app));
                if !gone.is_empty() {
                    store.forget(&gone);
                    store.save(&h);
                    let _ = app.emit_to(NOTCH, NOTCH_CLIPS, h.snapshot());
                }
            }
            // the number changes with every copy, and reading it does not open the clipboard
            let sequence = unsafe { GetClipboardSequenceNumber() };
            if sequence != seen {
                seen = sequence;
                if let Some(clip) = read().and_then(|raw| record(&history, &store, raw)) {
                    // the kind only: what was copied stays out of the log
                    log::debug!("clipboard copy {} is {}", clip.id, clip.content.kind());
                    // sent either way: the page may have asked for the history before this was read
                    let _ = app.emit_to(NOTCH, if starting { NOTCH_CLIP_QUIET } else { NOTCH_CLIP }, clip);
                }
            }
            starting = false;
        }
    });
    if let Err(e) = spawned {
        log::error!("could not start the notch's clipboard watcher: {e}");
    }
}

fn lock(history: &Mutex<History>) -> MutexGuard<'_, History> {
    history.lock().unwrap_or_else(|e| e.into_inner())
}

/// Adds what was copied to the history (or moves a repeat up) and saves it.
fn record(history: &Mutex<History>, store: &Store, raw: Raw) -> Option<Clip> {
    let fingerprint = raw.fingerprint();
    let now = unix_now();
    {
        let mut h = lock(history);
        if let Some(clip) = h.bump(fingerprint, now) {
            store.save(&h);
            return Some(clip);
        }
    }
    // previews are made without holding the lock
    let id = lock(history).reserve_id();
    let (content, payload) = raw.into_entry(id, store)?;
    let clip = Clip { id, content };
    let mut h = lock(history);
    let gone = h.insert(Entry { fingerprint, copied_at: now, clip: clip.clone(), payload });
    store.forget(&gone);
    store.save(&h);
    Some(clip)
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
        log::debug!("copied image is {width} x {height}; too big for the notch");
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

/// Puts a copy back on the clipboard in the formats apps paste from.
fn write(payload: &Payload, owner: HWND) -> Result<(), String> {
    // everything is prepared before the clipboard is opened, so it is held only briefly
    let formats: Vec<(u32, Vec<u8>)> = match payload {
        Payload::Text(text) => vec![(u32::from(CF_UNICODETEXT.0), utf16_nul(text))],
        Payload::Image(image) => {
            let png = std::fs::read(image).map_err(|e| format!("the copied image is gone: {e}"))?;
            let (width, height, rgba) = decode_png(&png).ok_or("the image could not be read back")?;
            // PNG keeps transparency for apps that take it; everything else pastes the bitmap
            vec![(register(w!("PNG")), png), (u32::from(CF_DIB.0), dib(width, height, &rgba))]
        }
        Payload::Files(paths) => vec![
            (u32::from(CF_HDROP.0), drop_files(paths)),
            // pasting in Explorer copies the files instead of moving them
            (register(w!("Preferred DropEffect")), DROPEFFECT_COPY.0.to_le_bytes().to_vec()),
        ],
    };
    // with an owner window, or EmptyClipboard leaves the clipboard ownerless and SetClipboardData fails
    let _open = OpenClipboardGuard::open(Some(owner)).ok_or("the clipboard is busy")?;
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

// --- dragging ----------------------------------------------------------------

/// The files a drag carries: a copy's files, or its saved image. Text is not dragged.
fn drag_paths(payload: &Payload) -> Option<Vec<PathBuf>> {
    let paths = match payload {
        Payload::Files(paths) => paths.clone(),
        Payload::Image(image) => vec![image.clone()],
        Payload::Text(_) => return None,
    };
    let existing: Vec<PathBuf> = paths.into_iter().filter(|p| p.exists()).collect();
    (!existing.is_empty()).then_some(existing)
}

/// Drags files out with Explorer's own data object, so any app that accepts
/// dropped files takes them. Runs a modal loop until the drop; main thread only.
fn drag_files(paths: &[PathBuf], source: HWND) {
    let pidls: Vec<*mut ITEMIDLIST> =
        paths.iter().map(|p| unsafe { ILCreateFromPathW(&HSTRING::from(p.as_os_str())) }).filter(|p| !p.is_null()).collect();
    if pidls.is_empty() {
        return;
    }
    let list: Vec<*const ITEMIDLIST> = pidls.iter().map(|&p| p.cast_const()).collect();
    let dragged = (|| -> windows::core::Result<()> {
        let items = unsafe { SHCreateShellItemArrayFromIDLists(&list) }?;
        let data: IDataObject = unsafe { items.BindToHandler(None::<&IBindCtx>, &BHID_DataObject) }?;
        // copy only: a drop must never move the user's original files
        unsafe { SHDoDragDrop(Some(source), &data, None::<&IDropSource>, DROPEFFECT_COPY) }?;
        Ok(())
    })();
    for pidl in pidls {
        unsafe { ILFree(Some(pidl.cast_const())) };
    }
    if let Err(e) = dragged {
        log::warn!("could not drag the copy out: {e}");
    }
}

// --- previews ----------------------------------------------------------------

fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let (width, height) = (reader.info().width, reader.info().height);
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        log::debug!("copied image is {width} x {height}; too big for the notch");
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

fn image_content(width: u32, height: u32, rgba: &[u8]) -> Option<Content> {
    Some(Content::Image { src: thumbnail_url(width, height, rgba)?, width, height })
}

fn thumbnail_url(width: u32, height: u32, rgba: &[u8]) -> Option<String> {
    let (tw, th, thumb) = thumbnail(width, height, rgba);
    encode_png(tw, th, &thumb, png::Compression::Default).map(|png| png_data_url(&png))
}

/// Box-filtered downscale to fit `THUMB_MAX_WIDTH` x `THUMB_HEIGHT`; never enlarges.
fn thumbnail(width: u32, height: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let scale = (f64::from(THUMB_HEIGHT) / f64::from(height)).min(f64::from(THUMB_MAX_WIDTH) / f64::from(width)).min(1.0);
    let tw = ((f64::from(width) * scale).round() as u32).max(1);
    let th = ((f64::from(height) * scale).round() as u32).max(1);
    let (w, h) = (width as usize, height as usize);
    let span = |t: usize, out: usize, src: usize| {
        let start = t * src / out;
        (start, ((t + 1) * src / out).max(start + 1))
    };
    let mut out = Vec::with_capacity(tw as usize * th as usize * 4);
    for ty in 0..th as usize {
        let (y0, y1) = span(ty, th as usize, h);
        for tx in 0..tw as usize {
            let (x0, x1) = span(tx, tw as usize, w);
            let mut sum = [0u64; 4];
            for y in y0..y1 {
                for pixel in rgba[(y * w + x0) * 4..(y * w + x1) * 4].chunks_exact(4) {
                    for (s, &c) in sum.iter_mut().zip(pixel) {
                        *s += u64::from(c);
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u64;
            out.extend(sum.iter().map(|s| (s / n) as u8));
        }
    }
    (tw, th, out)
}

fn encode_png(width: u32, height: u32, rgba: &[u8], compression: png::Compression) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(compression);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}

fn png_data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

/// The first file of a copy (`with_picture`) gets Explorer's thumbnail, or its type's icon.
fn file_clip(path: &Path, with_picture: bool) -> FileClip {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string_lossy().into_owned());
    let thumb = (with_picture && !path.is_dir())
        .then(|| crate::platform::apps::file_thumbnail(path, 256))
        .flatten()
        .and_then(|(width, height, rgba)| thumbnail_url(width, height, &rgba));
    let icon = match (with_picture && thumb.is_none()).then(|| crate::platform::apps::file_icon(path)) {
        Some(AppIcon::Png(png)) => Some(png_data_url(&png)),
        _ => None,
    };
    FileClip { name, path: path.to_string_lossy().into_owned(), thumb, icon }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> Content {
        Content::Text { text: t.into() }
    }

    /// Adds a text copy made at `copied_at`; returns its clip and the copies it pushed out.
    fn insert_at(history: &mut History, fingerprint: u64, t: &str, copied_at: u64) -> (Clip, Vec<Entry>) {
        let id = history.reserve_id();
        let clip = Clip { id, content: text(t) };
        let gone = history.insert(Entry { fingerprint, copied_at, clip: clip.clone(), payload: Payload::Text(t.into()) });
        (clip, gone)
    }

    fn insert(history: &mut History, fingerprint: u64, t: &str) -> Clip {
        insert_at(history, fingerprint, t, 100).0
    }

    #[test]
    fn copying_again_moves_the_clip_up_and_keeps_its_id() {
        let mut history = History::default();
        let first = insert(&mut history, 1, "a");
        insert(&mut history, 2, "b");
        assert_eq!(history.bump(1, 500), Some(first.clone()));
        assert_eq!(history.snapshot()[0], first);
        assert_eq!(history.entries[0].copied_at, 500, "a repeat counts as copied just now");
        assert_eq!(history.bump(3, 500), None);
    }

    #[test]
    fn a_clicked_clip_moves_to_the_front_with_its_whole_payload() {
        let mut history = History::default();
        let first = insert(&mut history, 1, "a");
        insert(&mut history, 2, "b");
        assert_eq!(history.payload(first.id), Some(Payload::Text("a".into())));
        assert_eq!(history.bump_id(first.id, 500), Some(first.clone()));
        assert_eq!(history.snapshot()[0], first);
        assert_eq!(history.bump_id(99, 500), None);
    }

    #[test]
    fn history_keeps_the_newest() {
        let mut history = History::default();
        let mut pushed_out = 0;
        for i in 0..MAX_CLIPS as u64 + 5 {
            pushed_out += insert_at(&mut history, i, &i.to_string(), 100).1.len();
        }
        let clips = history.snapshot();
        assert_eq!(clips.len(), MAX_CLIPS);
        assert_eq!(pushed_out, 5);
        assert_eq!(clips[0].content, text(&(MAX_CLIPS + 4).to_string()));
    }

    #[test]
    fn copies_past_their_time_are_let_go() {
        let mut history = History::default();
        insert_at(&mut history, 1, "old", 1_000);
        let (recent, _) = insert_at(&mut history, 2, "recent", 9_000);
        let gone = history.prune(10_000, 3_600);
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].clip.content, text("old"));
        assert_eq!(history.snapshot(), vec![recent]);
        assert!(history.prune(10_000, 3_600).is_empty());
    }

    #[test]
    fn the_history_survives_a_restart_without_missing_images() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store { dir: dir.path().to_path_buf() };
        let mut history = History::default();
        let (kept, _) = insert_at(&mut history, 1, "hello", 100);
        let image_id = history.reserve_id();
        let image = store.save_image(image_id, b"png bytes").unwrap();
        let image_clip = Clip { id: image_id, content: Content::Image { src: "data:".into(), width: 1, height: 1 } };
        history.insert(Entry { fingerprint: 2, copied_at: 100, clip: image_clip.clone(), payload: Payload::Image(image.clone()) });
        store.save(&history);

        let loaded = store.load();
        assert_eq!(loaded.snapshot(), vec![image_clip.clone(), kept.clone()]);
        assert_eq!(loaded.next_id, history.next_id, "ids keep counting up after a restart");

        // a copy whose image was deleted is dropped; forgetting a copy deletes its image
        store.forget(&loaded.entries[..1]);
        assert!(!image.exists());
        assert_eq!(store.load().snapshot(), vec![kept]);
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
    fn thumbnails_fit_the_card_and_keep_the_aspect() {
        let rgba = vec![200u8; 1000 * 500 * 4];
        let (w, h, out) = thumbnail(1000, 500, &rgba);
        assert_eq!((w, h), (240, 120));
        assert_eq!(out.len(), (w * h * 4) as usize);
        assert!(out.iter().all(|&c| c == 200));
        // small images are not enlarged
        assert_eq!(thumbnail(10, 10, &[0; 400]).0, 10);
    }

    #[test]
    fn pngs_round_trip_through_the_decoder() {
        let rgba: Vec<u8> = (0..4 * 3 * 4).map(|i| i as u8).collect();
        let png = encode_png(4, 3, &rgba, png::Compression::Fast).unwrap();
        assert_eq!(decode_png(&png), Some((4, 3, rgba)));
        assert!(png_data_url(&png).starts_with("data:image/png;base64,iVBORw0KGgo"));
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

    #[test]
    fn clips_serialize_flat_for_the_page() {
        let clip = Clip { id: 7, content: text("hi") };
        assert_eq!(serde_json::to_value(&clip).unwrap(), serde_json::json!({ "id": 7, "kind": "text", "text": "hi" }));
        assert_eq!(clip_ref(r#"{"id":7}"#), Some(7));
        assert_eq!(clip_ref("7"), None);
    }
}
