//! System tray icon and menu.

use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::app_state::{self, AppState};
use crate::events;
use crate::storage::settings::Settings;
use crate::window_manager;

const TRAY_ID: &str = "lummo";

#[derive(Default)]
pub struct TrayState {
    caps_item: Mutex<Option<CheckMenuItem<Wry>>>,
}

pub fn sync(app: &AppHandle, settings: &Settings) {
    let exists = app.tray_by_id(TRAY_ID).is_some();
    // Without a StatusNotifier host (stock elementary OS) the icon would be
    // invisible, and libappindicator logs GTK assertions while creating it.
    #[cfg(target_os = "linux")]
    if !exists && !crate::platform::linux::tray_host_available() {
        log::info!("no system tray host is running; the tray icon is not created");
        return;
    }
    if !exists {
        if let Err(e) = create(app, settings.caps_lock.enabled) {
            log::error!("could not create the tray icon: {e}");
            let _ = app.emit(events::NOTICE, format!("The tray icon could not be created: {e}"));
        }
    }
}

fn create(app: &AppHandle, caps_enabled: bool) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open-spotlight", "Open Spotlight", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let caps = CheckMenuItem::with_id(app, "toggle-caps", "Caps Lock Indicator", true, caps_enabled, None::<&str>)?;
    let updates = MenuItem::with_id(app, "check-updates", "Check for Updates…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Lummo", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open, &settings, &PredefinedMenuItem::separator(app)?, &caps, &updates, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Lummo")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                window_manager::toggle_spotlight(tray.app_handle(), None);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    if let Some(state) = app.try_state::<TrayState>() {
        *state.caps_item.lock().unwrap_or_else(|e| e.into_inner()) = Some(caps);
    }
    Ok(())
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "open-spotlight" => window_manager::show_spotlight(app, None),
        "settings" => window_manager::open_settings(app, None),
        "toggle-caps" => {
            let enabled = !app.state::<AppState>().settings().caps_lock.enabled;
            if let Err(e) = app_state::update_settings(app, &serde_json::json!({ "capsLock": { "enabled": enabled } })) {
                log::error!("could not toggle the Caps Lock indicator: {e}");
            }
        }
        "check-updates" => {
            window_manager::open_settings(app, Some("about"));
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = app.state::<AppState>().updates.check(&app).await {
                    log::warn!("update check from the tray failed: {e}");
                }
            });
        }
        "quit" => app.exit(0),
        other => log::debug!("unhandled tray menu item {other}"),
    }
}

pub fn set_caps_checked(app: &AppHandle, checked: bool) {
    if let Some(state) = app.try_state::<TrayState>() {
        if let Some(item) = state.caps_item.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let _ = item.set_checked(checked);
        }
    }
}
