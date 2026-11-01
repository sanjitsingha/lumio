//! Spotlight and Settings window lifecycle.

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
pub const SPOTLIGHT_WIDTH: f64 = 720.0;
/// Transparent margin around the panel in CSS pixels (`MARGIN` in `pages/Spotlight.jsx`).
const SPOTLIGHT_MARGIN: f64 = 20.0;
#[cfg(not(target_os = "linux"))]
const SPOTLIGHT_MIN_HEIGHT: f64 = 60.0;
/// On Linux the window keeps this fixed size: GTK cannot resize a mapped,
/// non-resizable window on Wayland, so instead the input region is shrunk to
/// the panel and the transparent rest of the window lets clicks through.
const SPOTLIGHT_MAX_HEIGHT: f64 = 600.0;
/// Focus changes right after showing are window-manager noise, not the user leaving.
const BLUR_GRACE: Duration = Duration::from_millis(300);

static SHOWN_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Last panel rectangle (x, y, width, height) in window pixels (Linux input region).
#[cfg(target_os = "linux")]
static PANEL_RECT: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);

pub fn create_spotlight(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(SPOTLIGHT) {
        return Ok(w);
    }
    let window = WebviewWindowBuilder::new(app, SPOTLIGHT, WebviewUrl::App("index.html".into()))
        .title("Lumio Spotlight")
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

pub fn is_spotlight_visible(app: &AppHandle) -> bool {
    app.get_webview_window(SPOTLIGHT).and_then(|w| w.is_visible().ok()).unwrap_or(false)
}

/// `timestamp` is the input event that triggered the request, if known; on
/// Wayland it is what lets the compositor hand keyboard focus to Spotlight.
pub fn toggle_spotlight(app: &AppHandle, timestamp: Option<u32>) {
    if is_spotlight_visible(app) {
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

pub fn hide_spotlight(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SPOTLIGHT) {
        if window.is_visible().unwrap_or(false) {
            log::debug!("spotlight hidden");
            let _ = window.hide();
            let _ = window.emit(events::SPOTLIGHT_HIDDEN, ());
        }
    }
}

/// Fits the window to its content. `css_height` (panel + margins) and
/// `css_width` are measured in the page, whose CSS pixels differ from window
/// pixels when the desktop's text scaling zooms the webview.
pub fn set_spotlight_height(app: &AppHandle, css_height: f64, css_width: f64) {
    let Some(window) = app.get_webview_window(SPOTLIGHT) else { return };
    let zoom = if css_width > 0.0 { SPOTLIGHT_WIDTH / css_width } else { 1.0 };
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
                .title("Lumio Settings")
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
            let recently_shown = SHOWN_AT.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed() < BLUR_GRACE);
            if !focused && !recently_shown {
                hide_spotlight(app);
            }
        }
        (SETTINGS, WindowEvent::CloseRequested { api, .. }) => {
            let general = app.state::<AppState>().settings().general;
            if general.run_in_background && general.close_to_background {
                log::info!("Settings closed; Lumio keeps running in the background");
            } else {
                api.prevent_close();
                log::info!("Settings closed with background mode off; quitting");
                app.exit(0);
            }
        }
        _ => {}
    }
}
