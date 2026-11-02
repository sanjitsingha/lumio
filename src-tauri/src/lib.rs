//! Lumio: a keyboard-first productivity utility (Spotlight-style search and a
//! cursor-anchored Caps Lock indicator) for Linux and Windows.

mod app_state;
mod capslock;
mod cli;
mod commands;
#[cfg(feature = "dev-snapshot")]
mod dev;
mod error;
mod events;
#[cfg(test)]
mod integration_tests;
mod platform;
mod search;
mod shortcuts;
mod storage;
mod tray;
mod updater;
mod window_manager;

use std::sync::{Arc, RwLock};
use std::time::Instant;

use tauri::{AppHandle, Manager, RunEvent};
use tauri_plugin_global_shortcut::ShortcutState;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

use app_state::AppState;
use capslock::CapsLockController;
use search::actions::ActionsProvider;
use search::apps::AppIndex;
use search::engine::SearchEngine;
use search::files::FileIndex;
use search::web::WebSearchProvider;
use shortcuts::accelerator::Accelerator;
use shortcuts::actions::ShortcutAction;
use shortcuts::backend::{ActivationSink, ShortcutBackend};
use shortcuts::plugin_backend::{PluginBackend, PluginShortcutMap};
use shortcuts::ShortcutManager;
use storage::history::History;
use storage::StoragePaths;

/// Whether the OS-level hotkey plugin can work in this session. On Linux
/// Wayland it would only see keys typed into X11 windows, so it is not used.
fn plugin_shortcuts_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        platform::linux::session::session_type() == platform::linux::session::SessionType::X11
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

fn run_shortcut_action(app: &AppHandle, accel: Accelerator, timestamp: Option<u32>) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let Some(slot) = state.shortcuts.resolve_activation(accel) else { return };
    log::debug!("shortcut {accel} -> {}", slot.id);
    match slot.action {
        ShortcutAction::ToggleSpotlight => window_manager::toggle_spotlight(app, timestamp),
        ShortcutAction::OpenSettings => window_manager::open_settings(app, None),
        ShortcutAction::ToggleCapsLockIndicator => {
            let enabled = !state.settings().caps_lock.enabled;
            if let Err(e) = app_state::update_settings(app, &serde_json::json!({ "capsLock": { "enabled": enabled } })) {
                log::error!("could not toggle the Caps Lock indicator: {e}");
            }
        }
    }
}

fn choose_shortcut_backend(app: &AppHandle, plugin_map: PluginShortcutMap, plugin_loaded: bool) -> (Option<Arc<dyn ShortcutBackend>>, Option<String>) {
    let handle = app.clone();
    let sink: ActivationSink = Arc::new(move |accel, timestamp| run_shortcut_action(&handle, accel, timestamp));
    #[cfg(target_os = "linux")]
    {
        if platform::linux::session::is_pantheon() {
            match shortcuts::gala_backend::GalaBackend::connect(sink.clone()) {
                Ok(backend) => return (Some(Arc::new(backend)), None),
                Err(e) => log::warn!("Gala shortcut service unavailable: {e}"),
            }
        }
    }
    let _ = &sink;
    if plugin_loaded {
        return (Some(Arc::new(PluginBackend::new(app.clone(), plugin_map))), None);
    }
    (
        None,
        Some(
            "This Wayland desktop does not let applications register global shortcuts. Add a custom keyboard \
             shortcut in your system settings that runs the command “lumio --toggle”."
                .into(),
        ),
    )
}

fn handle_launch(app: &AppHandle, launch: cli::Launch, secondary: bool) {
    match launch {
        cli::Launch::Toggle => window_manager::toggle_spotlight(app, None),
        cli::Launch::Settings => window_manager::open_settings(app, None),
        cli::Launch::Quit => app.exit(0),
        cli::Launch::Dev => {}
        // a second autostart launch while running: nothing to do
        cli::Launch::Autostart | cli::Launch::Updated if secondary => {}
        cli::Launch::Updated => {
            let version = app.package_info().version.to_string();
            log::info!("restarted after updating to {version}");
            updater::notify(app, "Lumio was updated", &format!("You're now running version {version}."));
        }
        cli::Launch::Autostart => {
            let start_minimized = app.state::<AppState>().settings().general.start_minimized;
            if !start_minimized {
                window_manager::open_settings(app, None);
            }
        }
        // opening Lumio from the app menu brings up Settings (there is no tray on stock elementary OS)
        cli::Launch::Manual => window_manager::open_settings(app, None),
    }
}

fn setup(app: &mut tauri::App, launch: cli::Launch, plugin_map: PluginShortcutMap, plugin_loaded: bool) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    if launch.requires_running_instance() {
        // e.g. `lumio --quit` with nothing running: never become the main instance
        handle.exit(0);
        return Ok(());
    }
    let paths = StoragePaths::resolve(&handle)?;
    if let Err(e) = paths.ensure_dirs() {
        log::error!("could not create application folders: {e}");
    }
    let loaded = storage::settings::load_settings(&paths.settings_file());
    let settings = loaded.settings;
    log::info!(
        "{} {} starting ({:?}); first run: {}",
        handle.package_info().name,
        handle.package_info().version,
        launch,
        loaded.first_run
    );

    let history = Arc::new(History::load(paths.history_file()));
    let apps = Arc::new(AppIndex::new());
    let files = FileIndex::new(paths.file_index_cache(), app_state::dirs_home());
    let settings_handle = handle.clone();
    let actions = Arc::new(ActionsProvider::new(Arc::new(move || window_manager::open_settings(&settings_handle, None))));
    let browser_handle = handle.clone();
    let web = Arc::new(WebSearchProvider::new(
        Arc::new(move || browser_handle.try_state::<AppState>().map(|s| s.settings().spotlight.web_browser).unwrap_or_default()),
        apps.clone(),
    ));
    let engine = Arc::new(SearchEngine::new(apps, files, actions, web, history));

    let (backend, unavailable) = choose_shortcut_backend(&handle, plugin_map, plugin_loaded);
    if let Some(b) = &backend {
        log::info!("global shortcuts via {}", b.label());
    } else {
        log::warn!("global shortcuts unavailable");
    }
    let shortcuts = Arc::new(ShortcutManager::new(backend, unavailable));
    let capslock = CapsLockController::start(handle.clone(), settings.caps_lock.clone());
    let platform = platform::info();
    log::info!(
        "platform: {} (version {}) session={:?} desktop={:?} tray={}",
        platform.os_name,
        platform.os_version,
        platform.session_type,
        platform.desktop,
        platform.tray_supported
    );

    app.manage(tray::TrayState::default());
    app.manage(AppState {
        paths,
        settings: RwLock::new(settings.clone()),
        notices: std::sync::Mutex::new(loaded.notices),
        engine,
        shortcuts,
        capslock,
        updates: Arc::new(updater::UpdateService::new(handle.package_info().version.to_string())),
        platform,
        started: Instant::now(),
    });

    #[cfg(target_os = "linux")]
    {
        use tauri::Emitter;
        let appearance_handle = handle.clone();
        platform::appearance::watch(move || {
            let _ = appearance_handle.emit(events::APPEARANCE_CHANGED, platform::appearance::read());
        });
    }

    if let Err(e) = window_manager::create_spotlight(&handle) {
        log::error!("could not create the Spotlight window: {e}");
    }
    tray::sync(&handle, &settings);
    app_state::start_background_services(&handle);
    updater::start_background_checks(&handle);

    let launch = if loaded.first_run && launch == cli::Launch::Autostart { cli::Launch::Manual } else { launch };
    handle_launch(&handle, launch, false);
    Ok(())
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{}", cli::HELP);
        return;
    }
    let launch = cli::parse(&args);
    if let Some(pid) = cli::after_update_pid(&args) {
        // let the old version exit first, or single-instance would hand this launch to it
        updater::wait_for_previous_instance(pid);
    }
    let plugin_map: PluginShortcutMap = Arc::default();
    let plugin_loaded = plugin_shortcuts_supported();

    let log_plugin = tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: Some("lumio".into()) }),
        ])
        .level(log_level())
        .level_for("tao", log::LevelFilter::Warn)
        .level_for("notify", log::LevelFilter::Warn)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .max_file_size(2_000_000)
        .rotation_strategy(RotationStrategy::KeepOne)
        .build();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            #[cfg(feature = "dev-snapshot")]
            if dev::handle(app, &argv) {
                return;
            }
            handle_launch(app, cli::parse(&argv), true);
        }))
        .plugin(log_plugin)
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init());

    if plugin_loaded {
        let map = plugin_map.clone();
        builder = builder.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let accel = map.lock().unwrap_or_else(|e| e.into_inner()).get(&shortcut.id()).copied();
                    if let Some(accel) = accel {
                        run_shortcut_action(app, accel, None);
                    }
                })
                .build(),
        );
    }

    let app = builder
        .register_asynchronous_uri_scheme_protocol(search::icons::SCHEME, |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = request.uri().path().to_string();
            std::thread::spawn(move || {
                let index = app.state::<AppState>().engine.apps.clone();
                responder.respond(search::icons::respond(&index, &path));
            });
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::reset_settings,
            commands::search::search,
            commands::search::execute_result,
            commands::search::get_recent_items,
            commands::search::clear_search_history,
            commands::search::get_index_status,
            commands::search::rebuild_index,
            commands::window::hide_spotlight,
            commands::window::set_spotlight_height,
            commands::window::open_settings_window,
            commands::shortcuts::get_shortcut_status,
            commands::shortcuts::begin_shortcut_recording,
            commands::shortcuts::end_shortcut_recording,
            commands::shortcuts::set_shortcut,
            commands::shortcuts::reset_shortcuts,
            commands::shortcuts::validate_shortcut,
            commands::capslock::get_capslock_support,
            commands::capslock::preview_capslock_indicator,
            commands::system::get_platform_info,
            commands::system::get_system_appearance,
            commands::system::get_browsers,
            commands::system::get_diagnostics,
            commands::system::read_log_tail,
            commands::system::open_log_folder,
            commands::system::clear_cache,
            commands::system::open_repository,
            commands::system::quit_app,
            commands::updates::get_update_status,
            commands::updates::check_for_updates,
            commands::updates::download_update,
            commands::updates::install_update,
            commands::updates::open_release_page,
        ])
        .on_window_event(window_manager::on_window_event)
        .setup(move |app| setup(app, launch, plugin_map, plugin_loaded))
        .build(tauri::generate_context!());

    let app = match app {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Lumio failed to start: {e}");
            std::process::exit(1);
        }
    };

    app.run(|app, event| match event {
        RunEvent::ExitRequested { code: None, api, .. } => {
            // last window closed: stay resident when running in the background
            if app.try_state::<AppState>().is_some_and(|s| s.settings().general.run_in_background) {
                api.prevent_exit();
            }
        }
        RunEvent::Exit => {
            if let Some(state) = app.try_state::<AppState>() {
                log::info!("shutting down: releasing shortcuts and stopping background work");
                state.shortcuts.shutdown();
                state.capslock.shutdown();
                state.engine.files.stop_worker();
            }
        }
        _ => {}
    });
}

/// `LUMIO_LOG=debug` (or `trace`, `warn`, …) overrides the default level:
/// debug in development builds, info in releases.
fn log_level() -> log::LevelFilter {
    std::env::var("LUMIO_LOG")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(if cfg!(debug_assertions) { log::LevelFilter::Debug } else { log::LevelFilter::Info })
}
