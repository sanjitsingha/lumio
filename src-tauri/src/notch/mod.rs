//! The notch (Windows only): hidden at the top
//! centre of the primary screen until the mouse rests on the screen's top
//! edge there for a moment (`DWELL`); then a panel drops out of the edge, and
//! moving off it hides it again. The panel shows the clipboard history
//! (`clipboard`) and is as wide as its items need, up to `MAX_WIDTH`.
//!
//! The window always has the widest panel's size (plus room for its shadow)
//! and lets clicks through while the panel is hidden, so it never blocks what
//! is under it. The cursor is polled here instead of hovered in the page,
//! because a click-through window gets no mouse events.
//!
//! Being always on top does not keep it there: another always-on-top window
//! (a fullscreen game or video player) can rise over it, and Windows itself
//! can sink it below ordinary windows while it keeps its always-on-top flag.
//! Once covered, its webview also stops drawing until it learns it can be
//! seen again. So as it is about to open, and while it is open, a covered
//! notch is lifted back on top and its webview told (`lift`).

mod clipboard;

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Listener, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindow, GetWindowRect, IsWindowVisible, SetWindowPos, CHILDID_SELF, EVENT_OBJECT_SHOW, GW_HWNDPREV,
    HWND_TOPMOST, OBJID_WINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
};

use crate::app_state::AppState;

pub const NOTCH: &str = "notch";
/// Sent to the notch page: `true` expands the panel, `false` collapses it.
const NOTCH_EXPANDED: &str = "notch-expanded";
/// From the notch page: the expanded panel's width in CSS px, as its items change.
const NOTCH_WIDTH: &str = "notch-width";
/// From the notch page: whether it has opened by itself to show a copy arriving.
const NOTCH_SHOWCASE: &str = "notch-showcase";

/// Logical size of the widest expanded panel (`MAX_WIDTH` and `EXPANDED_HEIGHT`
/// in `pages/Notch.jsx`).
const MAX_WIDTH: f64 = 640.0;
const HEIGHT: f64 = 96.0;
/// Transparent margin beside and below the panel, in which its shadow (and an
/// opening spring's slight overshoot) fades out (`SHADOW_MARGIN` in `pages/Notch.jsx`).
const SHADOW_MARGIN: f64 = 40.0;
/// Logical width of the invisible spot on the screen's top edge, centred, that
/// opens the notch: wide, so it is easy to find without seeing it.
const STRIP_WIDTH: f64 = 600.0;
/// How far below the screen's top edge (physical px) still counts as touching it.
const EDGE: f64 = 2.0;
/// How long the mouse has to rest on that spot before the notch opens, so
/// passing over it on the way somewhere else does not.
const DWELL: Duration = Duration::from_millis(300);
const POLL: Duration = Duration::from_millis(16);
/// How often the open notch makes sure nothing has risen over it (`lift`).
const LIFT_EVERY: Duration = Duration::from_millis(250);

/// The expanded panel's current logical width, as the page reports it.
static PANEL_WIDTH: AtomicU32 = AtomicU32::new(MAX_WIDTH as u32);
/// The page has opened the panel by itself to show a copy arriving (`NOTCH_SHOWCASE`).
static SHOWCASE: AtomicBool = AtomicBool::new(false);

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let Some(monitor) = app.primary_monitor()? else {
        log::warn!("no primary monitor; the notch is not created");
        return Ok(());
    };
    let scale = monitor.scale_factor();
    let (screen_x, top) = (monitor.position().x as f64, monitor.position().y as f64);
    let centre = screen_x + monitor.size().width as f64 / 2.0;
    let (window_width, window_height) = (MAX_WIDTH + 2.0 * SHADOW_MARGIN, HEIGHT + SHADOW_MARGIN);
    let left = centre - window_width * scale / 2.0;
    let height = HEIGHT * scale;

    // The notch never takes focus from the app being used: focused(false) makes
    // `show` use SW_SHOWNOACTIVATE, and focusable(false) (WS_EX_NOACTIVATE)
    // keeps clicks on the carousel from activating it.
    let window = WebviewWindowBuilder::new(app, NOTCH, WebviewUrl::App("index.html".into()))
        .title("Lummo Notch")
        .inner_size(window_width, window_height)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .visible(false)
        .build()?;
    window.set_position(PhysicalPosition::new(left.round() as i32, top as i32))?;
    window.set_ignore_cursor_events(true)?;
    window.show()?;
    let hwnd = window.hwnd()?;
    // beside Lummo's other data, which the file index never looks in
    clipboard::watch(app, hwnd, app.state::<AppState>().paths.data_dir.join("notch clipboard"));
    // HWND is a raw pointer; it crosses threads as a number
    let hwnd = hwnd.0 as isize;

    app.listen(NOTCH_WIDTH, |event| {
        if let Ok(width) = serde_json::from_str::<f64>(event.payload()) {
            PANEL_WIDTH.store(width.clamp(1.0, MAX_WIDTH).round() as u32, Ordering::Relaxed);
        }
    });
    app.listen(NOTCH_SHOWCASE, |event| {
        SHOWCASE.store(event.payload() == "true", Ordering::Relaxed);
    });

    let app = app.clone();
    let strip_half = STRIP_WIDTH * scale / 2.0;
    std::thread::Builder::new().name("notch".into()).spawn(move || {
        let mut expanded = false;
        // since when the mouse has been resting on the spot that opens it
        let mut resting_since: Option<Instant> = None;
        // The spot opens the notch only after the mouse has been off it since
        // it last closed: the spot is wider than a short panel, and leaving
        // such a panel sideways along the top edge must not open it again.
        let mut armed = true;
        // when it was last made sure nothing covers it
        let mut lifted: Option<Instant> = None;
        loop {
            std::thread::sleep(POLL);
            let Ok(cursor) = app.cursor_position() else { continue };
            // the panel is centred in the window and may be narrower than it
            let panel_half = f64::from(PANEL_WIDTH.load(Ordering::Relaxed)) * scale / 2.0;
            let on_panel = (cursor.x - centre).abs() < panel_half && cursor.y >= top && cursor.y < top + height;
            let on_spot = cursor.y <= top + EDGE && (cursor.x - centre).abs() <= strip_half;
            if !on_spot {
                armed = true;
            }
            let showcase = SHOWCASE.load(Ordering::Relaxed);
            let was_resting = resting_since.is_some();
            let next = if expanded {
                on_panel
            } else if showcase && on_panel {
                // the page opened it to show a copy: reaching for it keeps it open, at once
                true
            } else if on_spot && armed {
                resting_since.get_or_insert_with(Instant::now).elapsed() >= DWELL
            } else {
                // leaving the spot starts the wait over
                resting_since = None;
                false
            };
            // Lifted as the mouse starts resting on the spot, so it is on top
            // and drawing before it opens, and again and again while open (for
            // the mouse or to show a copy). Only then, so it never rises over
            // a fullscreen game or video unasked.
            let starts_resting = !was_resting && resting_since.is_some();
            if starts_resting || ((next || showcase) && lifted.is_none_or(|at| at.elapsed() >= LIFT_EVERY)) {
                lift(hwnd);
                lifted = Some(Instant::now());
            }
            if next != expanded {
                expanded = next;
                resting_since = None;
                armed = expanded || !on_spot;
                // while expanded the panel is solid: clicks, drags and the wheel stay on it
                let _ = window.set_ignore_cursor_events(!expanded);
                let _ = app.emit_to(NOTCH, NOTCH_EXPANDED, expanded);
            }
        }
    })?;
    Ok(())
}

/// If a window is over the notch window (`hwnd`, as a number), puts the notch
/// back on top of every window, always-on-top ones included, without
/// activating it.
fn lift(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut c_void);
    if !covered(hwnd) {
        return;
    }
    let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER;
    unsafe {
        // waits for the window's thread, so it is on top before the webview looks
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
        // WebView2 stops drawing a window it found covered, and looks again
        // when a window is shown, hidden, moved or activated, but not when one
        // is restacked: say this one has been shown.
        NotifyWinEvent(EVENT_OBJECT_SHOW, hwnd, OBJID_WINDOW.0, CHILDID_SELF as i32);
    }
}

/// Whether a window the user can see (shown and not cloaked) is stacked over
/// the notch window and overlaps it.
fn covered(hwnd: HWND) -> bool {
    let rect = |window: HWND| {
        let mut rect = RECT::default();
        unsafe { GetWindowRect(window, &mut rect) }.ok().map(|()| rect)
    };
    let Some(notch) = rect(hwnd) else { return false };
    let mut above = unsafe { GetWindow(hwnd, GW_HWNDPREV) };
    while let Ok(window) = above {
        // stays 0 (not cloaked) if it cannot be read
        let mut cloaked = 0u32;
        let _ = unsafe { DwmGetWindowAttribute(window, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast(), 4) };
        let seen = unsafe { IsWindowVisible(window) }.as_bool() && cloaked == 0;
        let overlaps =
            |r: RECT| r.left < notch.right && notch.left < r.right && r.top < notch.bottom && notch.top < r.bottom;
        if seen && rect(window).is_some_and(overlaps) {
            return true;
        }
        above = unsafe { GetWindow(window, GW_HWNDPREV) };
    }
    false
}
