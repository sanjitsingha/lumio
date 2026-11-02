//! Spotlight and Settings window lifecycle.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window,
    WindowEvent,
};

use crate::app_state::AppState;
use crate::events;
use crate::storage::settings::AppearanceMode;

pub const SPOTLIGHT: &str = "spotlight";
pub const SETTINGS: &str = "settings";

/// Logical width of the Spotlight window: 680 px panel + transparent margin for its shadow.
pub const SPOTLIGHT_WIDTH: f64 = 752.0;
/// Transparent margin around the panel in CSS pixels (`MARGIN` in `pages/Spotlight.jsx`);
/// the panel's shadow (`--lm-shadow`) must fade out within it.
const SPOTLIGHT_MARGIN: f64 = 36.0;
#[cfg(not(target_os = "linux"))]
const SPOTLIGHT_MIN_HEIGHT: f64 = 60.0;
/// On Linux the window keeps this fixed size: GTK cannot resize a mapped,
/// non-resizable window on Wayland, so instead the input region is shrunk to
/// the panel and the transparent rest of the window lets clicks through.
/// The results list's height cap in `ResultList.jsx` assumes this height.
const SPOTLIGHT_MAX_HEIGHT: f64 = 632.0;
/// Focus changes right after showing are window-manager noise, not the user leaving.
const BLUR_GRACE: Duration = Duration::from_millis(300);
/// How long the page fades the panel out (on `SPOTLIGHT_HIDING`) before the window hides.
const FADE_OUT: Duration = Duration::from_millis(120);

static SHOWN_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Spotlight is open: shown and not closing (the window stays visible while it fades out).
static OPEN: AtomicBool = AtomicBool::new(false);
/// Bumped by every show and hide, so a pending hide can tell it was overtaken.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Last panel rectangle (x, y, width, height) in window pixels (Linux input region).
#[cfg(target_os = "linux")]
static PANEL_RECT: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);

pub fn create_spotlight(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(SPOTLIGHT) {
        return Ok(w);
    }
    let window = WebviewWindowBuilder::new(app, SPOTLIGHT, WebviewUrl::App("index.html".into()))
        .title("Lummo Spotlight")
        .inner_size(SPOTLIGHT_WIDTH, if cfg!(target_os = "linux") { SPOTLIGHT_MAX_HEIGHT } else { 120.0 })
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .visible(false)
        .focused(true)
        .build()?;
    Ok(window)
}

/// Open, and not fading out: toggling now closes it.
pub fn is_spotlight_open() -> bool {
    OPEN.load(Ordering::SeqCst)
}

/// `timestamp` is the input event that triggered the request, if known; on
/// Wayland it is what lets the compositor hand keyboard focus to Spotlight.
pub fn toggle_spotlight(app: &AppHandle, timestamp: Option<u32>) {
    if is_spotlight_open() {
        hide_spotlight(app);
    } else {
        show_spotlight(app, timestamp);
    }
}

pub fn show_spotlight(app: &AppHandle, timestamp: Option<u32>) {
    let state = app.state::<AppState>();
    if !state.settings().spotlight.enabled {
        log::info!("Spotlight is disabled in settings");
        return;
    }
    let window = match create_spotlight(app) {
        Ok(w) => w,
        Err(e) => {
            log::error!("could not create the Spotlight window: {e}");
            return;
        }
    };
    if state.engine.apps.is_stale(Duration::from_secs(15 * 60)) {
        let engine = state.engine.clone();
        std::thread::spawn(move || engine.apps.refresh());
    }
    if state.platform.can_position_windows {
        center_on_active_monitor(app, &window);
    }
    *SHOWN_AT.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    OPEN.store(true, Ordering::SeqCst);
    // cancels a fade-out that is still running
    GENERATION.fetch_add(1, Ordering::SeqCst);
    if let Err(e) = window.show() {
        log::error!("could not show Spotlight: {e}");
        return;
    }
    focus_window(app, &window, timestamp);
    #[cfg(target_os = "linux")]
    apply_input_region(app, &window);
    log::debug!("spotlight shown (input timestamp {timestamp:?})");
    let _ = window.emit(events::SPOTLIGHT_SHOWN, ());
}

/// GTK's `present_with_time` forwards the triggering input's timestamp to the
/// compositor (gtk_surface1.present on Wayland, _NET_ACTIVE_WINDOW on X11),
/// which satisfies focus-stealing prevention.
#[cfg(target_os = "linux")]
fn focus_window(app: &AppHandle, window: &WebviewWindow, timestamp: Option<u32>) {
    let Some(ts) = timestamp.filter(|t| *t != 0) else {
        if let Err(e) = window.set_focus() {
            log::warn!("could not focus Spotlight: {e}");
        }
        return;
    };
    let window = window.clone();
    let _ = app.run_on_main_thread(move || {
        use gtk::prelude::GtkWindowExt;
        match window.gtk_window() {
            Ok(gtk_window) => gtk_window.present_with_time(ts),
            Err(e) => log::warn!("could not focus Spotlight: {e}"),
        }
    });
}

#[cfg(not(target_os = "linux"))]
fn focus_window(_app: &AppHandle, window: &WebviewWindow, _timestamp: Option<u32>) {
    if let Err(e) = window.set_focus() {
        log::warn!("could not focus Spotlight: {e}");
    }
}

/// Closes Spotlight: the page fades the panel out (`SPOTLIGHT_HIDING`), then
/// the window hides, unless Spotlight was opened again in the meantime.
pub fn hide_spotlight(app: &AppHandle) {
    if !OPEN.swap(false, Ordering::SeqCst) {
        return;
    }
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let Some(window) = app.get_webview_window(SPOTLIGHT) else { return };
    log::debug!("spotlight closing");
    let _ = window.emit(events::SPOTLIGHT_HIDING, ());
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(FADE_OUT);
        // decided on the main thread, where showing happens too, so a show cannot slip in between
        let _ = app.run_on_main_thread(move || {
            if GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            log::debug!("spotlight hidden");
            let _ = window.hide();
            let _ = window.emit(events::SPOTLIGHT_HIDDEN, ());
        });
    });
}

/// Fits the window to its content. `css_height` (panel + margins) and
/// `css_width` are measured in the page, whose CSS pixels differ from window
/// pixels when the desktop's text scaling zooms the webview.
pub fn set_spotlight_height(app: &AppHandle, css_height: f64, css_width: f64) {
    let Some(window) = app.get_webview_window(SPOTLIGHT) else { return };
    let zoom = if css_width > 0.0 { SPOTLIGHT_WIDTH / css_width } else { 1.0 };
    log::debug!("spotlight content {css_height} css px tall at width {css_width} (zoom {zoom:.3})");
    #[cfg(target_os = "linux")]
    {
        let margin = (SPOTLIGHT_MARGIN * zoom).round() as i32;
        let height = (css_height * zoom).round().min(SPOTLIGHT_MAX_HEIGHT) as i32;
        let rect = (margin, margin, SPOTLIGHT_WIDTH as i32 - 2 * margin, (height - 2 * margin).max(1));
        *PANEL_RECT.lock().unwrap_or_else(|e| e.into_inner()) = Some(rect);
        apply_input_region(app, &window);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = SPOTLIGHT_MARGIN;
        let height = (css_height * zoom).round().clamp(SPOTLIGHT_MIN_HEIGHT, SPOTLIGHT_MAX_HEIGHT);
        if let Err(e) = window.set_size(tauri::LogicalSize::new(SPOTLIGHT_WIDTH, height)) {
            log::warn!("could not resize Spotlight: {e}");
        }
    }
}

/// Restricts pointer input to the panel; the transparent margin and the
/// unused space below the panel pass clicks to the windows underneath.
#[cfg(target_os = "linux")]
fn apply_input_region(app: &AppHandle, window: &WebviewWindow) {
    let Some((x, y, w, h)) = *PANEL_RECT.lock().unwrap_or_else(|e| e.into_inner()) else { return };
    let window = window.clone();
    let _ = app.run_on_main_thread(move || {
        use gtk::prelude::WidgetExt;
        let Ok(gtk_window) = window.gtk_window() else { return };
        // the GDK window exists once the toplevel has been realized (first show)
        if let Some(gdk_window) = gtk_window.window() {
            let region = gtk::cairo::Region::create_rectangle(&gtk::cairo::RectangleInt::new(x, y, w, h));
            gdk_window.input_shape_combine_region(&region, 0, 0);
        }
    });
}

/// Horizontally centered, with the search field a little above the middle
/// (results grow downward) on the monitor that has the pointer.
fn center_on_active_monitor(app: &AppHandle, window: &WebviewWindow) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let width = (SPOTLIGHT_WIDTH * scale).round() as i32;
    let x = area.position.x + (area.size.width as i32 - width) / 2;
    let y = area.position.y + (area.size.height as f64 * 0.22).round() as i32;
    if let Err(e) = window.set_position(PhysicalPosition::new(x, y)) {
        log::warn!("could not position Spotlight: {e}");
    }
}

pub fn open_settings(app: &AppHandle, pane: Option<&str>) {
    hide_spotlight(app);
    let window = match app.get_webview_window(SETTINGS) {
        Some(w) => w,
        None => {
            let dark = match app.state::<AppState>().settings().appearance.theme {
                AppearanceMode::Dark => true,
                AppearanceMode::Light => false,
                AppearanceMode::System => crate::platform::appearance::read().dark.unwrap_or(false),
            };
            let background = if dark { tauri::window::Color(23, 23, 25, 255) } else { tauri::window::Color(245, 245, 247, 255) };
            let url = match pane {
                Some(p) => format!("index.html#{p}"),
                None => "index.html".to_string(),
            };
            let built = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App(url.into()))
                .title("Lummo Settings")
                .inner_size(880.0, 620.0)
                .min_inner_size(760.0, 520.0)
                .center()
                .theme(Some(if dark { Theme::Dark } else { Theme::Light }))
                .background_color(background)
                .visible(true)
                .build();
            match built {
                Ok(w) => w,
                Err(e) => {
                    log::error!("could not create the Settings window: {e}");
                    return;
                }
            }
        }
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    if let Some(pane) = pane {
        let _ = window.emit(events::NAVIGATE, pane);
    }
}

/// After Spotlight loses focus: `None` to hide it now, or how long to wait
/// before checking again (during [`BLUR_GRACE`] after it was shown).
fn blur_recheck(since_shown: Option<Duration>) -> Option<Duration> {
    since_shown.filter(|t| *t < BLUR_GRACE).map(|t| BLUR_GRACE - t)
}

/// Global window event handler (installed with `Builder::on_window_event`).
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let app = window.app_handle();
    match (window.label(), event) {
        (SPOTLIGHT, WindowEvent::CloseRequested { api, .. }) => {
            api.prevent_close();
            hide_spotlight(app);
        }
        (SPOTLIGHT, WindowEvent::Focused(focused)) => {
            log::debug!("spotlight focus changed: {focused}");
            if *focused {
                return;
            }
            let shown_at = *SHOWN_AT.lock().unwrap_or_else(|e| e.into_inner());
            match blur_recheck(shown_at.map(|t| t.elapsed())) {
                None => hide_spotlight(app),
                Some(wait) => {
                    // maybe noise while appearing, maybe a quick click outside: if focus
                    // has not come back once the grace period is over, the user left
                    let app = app.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(wait);
                        let same_showing = *SHOWN_AT.lock().unwrap_or_else(|e| e.into_inner()) == shown_at;
                        let Some(window) = app.get_webview_window(SPOTLIGHT) else { return };
                        if same_showing && window.is_visible().unwrap_or(false) && !window.is_focused().unwrap_or(true) {
                            log::debug!("spotlight lost focus while appearing and did not get it back");
                            hide_spotlight(&app);
                        }
                    });
                }
            }
        }
        (SETTINGS, WindowEvent::CloseRequested { .. }) => {
            log::info!("Settings closed; Lummo keeps running in the background");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_right_after_showing_is_checked_again_once_the_grace_is_over() {
        assert_eq!(blur_recheck(Some(Duration::from_millis(100))), Some(Duration::from_millis(200)));
        assert_eq!(blur_recheck(Some(BLUR_GRACE)), None);
        assert_eq!(blur_recheck(Some(Duration::from_secs(5))), None);
        assert_eq!(blur_recheck(None), None);
    }
}
