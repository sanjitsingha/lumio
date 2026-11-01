//! Caps Lock indicator: detection (platform monitor) -> state machine ->
//! animated overlay next to the pointer (or the compositor OSD where the
//! pointer position is unavailable, i.e. Wayland).

pub mod geometry;
pub mod render;
pub mod state;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::AppHandle;

use crate::platform;
use crate::storage::settings::{AppearanceMode, CapsLockSettings, WaylandFallback};
use geometry::Rect;
use render::{Frame, Style};
use state::{Behavior, Command, IndicatorState};

const FRAME: Duration = Duration::from_millis(16);
const IDLE_POLL: Duration = Duration::from_millis(100);
const MOVE_WINDOW: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Floating indicator anchored to the mouse pointer.
    Cursor,
    /// Compositor on-screen display (fixed position; Wayland fallback).
    Osd,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub available: bool,
    pub method: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapsLockSupport {
    pub mode: Mode,
    pub detection: Capability,
    pub cursor: Capability,
    pub overlay: Capability,
    pub osd_available: bool,
    pub summary: String,
    pub alternatives: Vec<String>,
    pub monitoring: bool,
    pub monitor_error: Option<String>,
    pub caps_lock_on: Option<bool>,
}

enum Msg {
    Caps(bool),
    Settings(CapsLockSettings),
    Preview(bool),
    Shutdown,
}

pub struct CapsLockController {
    tx: Sender<Msg>,
    support: Mutex<CapsLockSupport>,
    monitor_started: AtomicBool,
    monitor_active: Arc<AtomicBool>,
    caps_state: Arc<Mutex<Option<bool>>>,
}

fn capability(result: Result<&'static str, String>, ok_detail: &str) -> Capability {
    match result {
        Ok(method) => Capability { available: true, method: method.into(), detail: ok_detail.into() },
        Err(detail) => Capability { available: false, method: "Unavailable".into(), detail },
    }
}

pub fn probe_support() -> CapsLockSupport {
    let d = platform::capslock::support();
    let detection = Capability { available: d.available, method: d.method.into(), detail: d.detail };
    let cursor = capability(platform::cursor::support(), "The pointer position is read only while the indicator is visible.");
    let overlay = capability(platform::overlay::support(), "Transparent, click-through and never takes keyboard focus.");
    let osd_available = platform::overlay::osd_available();
    let (mode, summary, alternatives) = if detection.available && cursor.available && overlay.available {
        let summary = if cursor.method.contains("XWayland") {
            "The indicator appears next to the mouse pointer. Wayland hides the pointer from apps, so while the \
             indicator is visible Lumio locates it through XWayland with an invisible, click-through layer."
        } else {
            "The indicator appears next to the mouse pointer."
        };
        (Mode::Cursor, summary.to_string(), Vec::new())
    } else if detection.available && osd_available {
        (
            Mode::Osd,
            "This Wayland session does not let apps read the pointer position, so the indicator cannot follow the \
             mouse. Lumio uses the system on-screen display instead."
                .to_string(),
            vec![
                "Log in with the “Classic Session” (X11) from the login screen's session menu to get the \
                 cursor-anchored indicator."
                    .to_string(),
            ],
        )
    } else if !detection.available {
        (Mode::Unavailable, detection.detail.clone(), vec!["Use a keyboard that has a Caps Lock LED, or an X11 session.".to_string()])
    } else {
        (
            Mode::Unavailable,
            format!("{} {}", cursor.detail, overlay.detail),
            vec!["Log in with an X11 session to use the cursor indicator.".to_string()],
        )
    };
    CapsLockSupport {
        mode,
        detection,
        cursor,
        overlay,
        osd_available,
        summary,
        alternatives,
        monitoring: false,
        monitor_error: None,
        caps_lock_on: None,
    }
}

impl CapsLockController {
    pub fn start(app: AppHandle, settings: CapsLockSettings) -> Arc<Self> {
        let support = probe_support();
        log::info!(
            "caps lock indicator mode {:?} (detection: {}, cursor: {}, overlay: {})",
            support.mode,
            support.detection.method,
            support.cursor.method,
            support.overlay.method
        );
        let (tx, rx) = mpsc::channel();
        let caps_state: Arc<Mutex<Option<bool>>> = Arc::default();
        let controller = Arc::new(Self {
            tx,
            support: Mutex::new(support.clone()),
            monitor_started: AtomicBool::new(false),
            monitor_active: Arc::new(AtomicBool::new(settings.enabled)),
            caps_state: caps_state.clone(),
        });
        let mode = support.mode;
        let initial = settings.clone();
        let spawned = std::thread::Builder::new()
            .name("lumio-capslock".into())
            .spawn(move || Runner::new(app, mode, initial, caps_state).run(rx));
        if let Err(e) = spawned {
            log::error!("could not start the Caps Lock indicator thread: {e}");
        }
        if settings.enabled {
            controller.ensure_monitor();
        }
        controller
    }

    fn ensure_monitor(&self) {
        let mode = self.support.lock().unwrap_or_else(|e| e.into_inner()).mode;
        if mode == Mode::Unavailable || self.monitor_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let tx = self.tx.clone();
        let result = platform::capslock::start_monitor(
            Arc::new(move |on| {
                let _ = tx.send(Msg::Caps(on));
            }),
            self.monitor_active.clone(),
        );
        let mut support = self.support.lock().unwrap_or_else(|e| e.into_inner());
        match result {
            Ok(method) => {
                support.monitoring = true;
                support.detection.method = method.into();
            }
            Err(e) => {
                log::error!("Caps Lock monitoring failed to start: {e}");
                support.monitoring = false;
                support.monitor_error = Some(e);
                self.monitor_started.store(false, Ordering::SeqCst);
            }
        }
    }

    pub fn update_settings(&self, settings: CapsLockSettings) {
        self.monitor_active.store(settings.enabled, Ordering::SeqCst);
        if settings.enabled {
            self.ensure_monitor();
        }
        let _ = self.tx.send(Msg::Settings(settings));
    }

    pub fn preview(&self, on: bool) {
        let _ = self.tx.send(Msg::Preview(on));
    }

    pub fn support(&self) -> CapsLockSupport {
        let mut s = self.support.lock().unwrap_or_else(|e| e.into_inner()).clone();
        s.caps_lock_on = *self.caps_state.lock().unwrap_or_else(|e| e.into_inner());
        s
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(Msg::Shutdown);
    }
}

fn behavior(s: &CapsLockSettings) -> Behavior {
    Behavior {
        enabled: s.enabled,
        duration: (s.duration_ms > 0).then(|| Duration::from_millis(s.duration_ms as u64)),
        show_when_disabled: s.show_when_disabled,
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

#[derive(Clone, Copy)]
struct MonitorGeom {
    bounds: Rect,
    work: Rect,
    scale: f64,
}

struct Visual {
    on: bool,
    dark: bool,
    from: f32,
    to: f32,
    start: Instant,
    progress: f32,
    frame: Option<Frame>,
    monitor: MonitorGeom,
    body_pos: (i32, i32),
}

struct Runner {
    app: AppHandle,
    mode: Mode,
    settings: CapsLockSettings,
    state: IndicatorState,
    caps_state: Arc<Mutex<Option<bool>>>,
    overlay: Option<platform::overlay::Overlay>,
    tracker: Option<platform::cursor::Tracker>,
    visual: Option<Visual>,
    last_cursor: Option<(i32, i32)>,
    last_move: Instant,
}

impl Runner {
    fn new(app: AppHandle, mode: Mode, settings: CapsLockSettings, caps_state: Arc<Mutex<Option<bool>>>) -> Self {
        Self {
            app,
            mode,
            settings,
            state: IndicatorState::default(),
            caps_state,
            overlay: None,
            tracker: None,
            visual: None,
            last_cursor: None,
            last_move: Instant::now(),
        }
    }

    fn run(mut self, rx: mpsc::Receiver<Msg>) {
        loop {
            let timeout = self.next_wake();
            let msg = match timeout {
                Some(t) => rx.recv_timeout(t),
                None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            let now = Instant::now();
            let command = match msg {
                Ok(Msg::Caps(on)) => {
                    *self.caps_state.lock().unwrap_or_else(|e| e.into_inner()) = Some(on);
                    log::debug!("caps lock {}", if on { "on" } else { "off" });
                    self.state.on_caps(on, now, &behavior(&self.settings))
                }
                Ok(Msg::Settings(s)) => {
                    let restyle = s.size != self.settings.size || s.appearance != self.settings.appearance;
                    self.settings = s;
                    if restyle {
                        if let Some(v) = self.visual.as_mut() {
                            v.frame = None;
                        }
                    }
                    self.state.on_settings(now, &behavior(&self.settings))
                }
                Ok(Msg::Preview(on)) => self.state.preview(on, now),
                Ok(Msg::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                    self.hide_now();
                    break;
                }
                Err(RecvTimeoutError::Timeout) => self.state.on_tick(now),
            };
            match command {
                Command::Show { on } => self.show(on, now),
                Command::Hide => self.begin_hide(now),
                Command::None => {}
            }
            self.step(now);
        }
    }

    fn next_wake(&self) -> Option<Duration> {
        let now = Instant::now();
        let deadline = self.state.deadline().map(|d| d.saturating_duration_since(now));
        let animating = self.visual.as_ref().is_some_and(|v| v.progress != v.to);
        let follow = self.visual.is_some() && self.settings.follow_cursor && self.mode == Mode::Cursor;
        let frame = if animating || (follow && self.last_move.elapsed() < MOVE_WINDOW) {
            Some(FRAME)
        } else if follow {
            Some(IDLE_POLL)
        } else {
            None
        };
        match (deadline, frame) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn dark(&self) -> bool {
        match self.settings.appearance {
            AppearanceMode::Dark => true,
            AppearanceMode::Light => false,
            AppearanceMode::System => platform::appearance::read().dark.unwrap_or(false),
        }
    }

    fn show(&mut self, on: bool, now: Instant) {
        match self.mode {
            Mode::Unavailable => {}
            Mode::Osd if self.settings.wayland_fallback == WaylandFallback::None => {}
            Mode::Osd => {
                if let Err(e) = platform::overlay::show_osd(on) {
                    log::warn!("could not show the Caps Lock OSD: {e}");
                }
            }
            Mode::Cursor => self.show_cursor_overlay(on, now),
        }
    }

    fn show_cursor_overlay(&mut self, on: bool, now: Instant) {
        if self.overlay.is_none() {
            match platform::overlay::Overlay::new() {
                Ok(o) => self.overlay = Some(o),
                Err(e) => {
                    log::error!("could not create the Caps Lock overlay: {e}");
                    return;
                }
            }
        }
        if self.tracker.is_none() {
            match platform::cursor::Tracker::new() {
                Ok(t) => self.tracker = Some(t),
                Err(e) => {
                    log::error!("pointer tracking unavailable: {e}");
                    return;
                }
            }
        }
        let Some(cursor) = self.tracker.as_mut().and_then(|t| t.position()) else {
            // e.g. a client holds a pointer grab: still tell the user, via the OSD
            log::warn!("could not read the pointer position; using the system OSD");
            if platform::overlay::osd_available() {
                let _ = platform::overlay::show_osd(on);
            }
            return;
        };
        if !self.settings.follow_cursor {
            if let Some(t) = self.tracker.as_mut() {
                t.release();
            }
        }
        let dark = self.dark();
        let monitor = self.monitor_for(cursor);
        let body = (self.settings.size as f64 * monitor.scale).round() as i32;
        let body_pos = geometry::place_body(cursor, body, monitor.work, self.settings.position, monitor.scale);
        log::debug!("caps lock indicator at {body_pos:?} for pointer {cursor:?}");
        self.last_cursor = Some(cursor);
        self.last_move = now;
        let animate = self.settings.animation.duration_ms() > 0;
        match self.visual.as_mut() {
            // already visible (e.g. on -> off flash): swap the artwork, keep opacity
            Some(v) if v.to > 0.0 => {
                v.on = on;
                v.dark = dark;
                v.frame = None;
                v.monitor = monitor;
                v.body_pos = body_pos;
            }
            _ => {
                self.visual = Some(Visual {
                    on,
                    dark,
                    from: if animate { 0.0 } else { 1.0 },
                    to: 1.0,
                    start: now,
                    progress: if animate { 0.0 } else { 1.0 },
                    frame: None,
                    monitor,
                    body_pos,
                });
            }
        }
    }

    fn begin_hide(&mut self, now: Instant) {
        let animate = self.settings.animation.duration_ms() > 0;
        match self.visual.as_mut() {
            Some(v) if animate => {
                v.from = v.progress;
                v.to = 0.0;
                v.start = now;
            }
            Some(_) => self.hide_now(),
            None => {}
        }
    }

    fn hide_now(&mut self) {
        self.visual = None;
        if let Some(o) = self.overlay.as_mut() {
            o.hide();
        }
        if let Some(t) = self.tracker.as_mut() {
            t.release();
        }
    }

    /// Advances animation and pointer following, then presents the frame.
    fn step(&mut self, now: Instant) {
        let duration = Duration::from_millis(self.settings.animation.duration_ms().max(1));
        let follow = self.settings.follow_cursor;
        let position = self.settings.position;
        let size = self.settings.size;
        let opacity = self.settings.opacity;

        let moved_to = if follow && self.visual.is_some() {
            self.tracker.as_mut().and_then(|t| t.position()).filter(|c| Some(*c) != self.last_cursor)
        } else {
            None
        };
        if let Some(c) = moved_to {
            self.last_cursor = Some(c);
            self.last_move = now;
        }
        let new_monitor = moved_to.and_then(|c| {
            let current = self.visual.as_ref()?.monitor;
            (!current.bounds.contains(c.0, c.1)).then(|| self.monitor_for(c))
        });

        let (progress, progress_changed, finished) = {
            let Some(v) = self.visual.as_mut() else { return };
            if let Some(m) = new_monitor {
                v.monitor = m;
                v.frame = None; // scale may differ between monitors
            }
            if let Some(c) = moved_to {
                let body = (size as f64 * v.monitor.scale).round() as i32;
                v.body_pos = geometry::place_body(c, body, v.monitor.work, position, v.monitor.scale);
            }
            let t = (now.duration_since(v.start).as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
            let progress = if v.from == v.to { v.to } else { v.from + (v.to - v.from) * ease_out_cubic(t) };
            let changed = (progress - v.progress).abs() > f32::EPSILON;
            v.progress = progress;
            (progress, changed, v.to == 0.0 && t >= 1.0)
        };
        if finished {
            self.hide_now();
            return;
        }
        let Self { visual, overlay, .. } = self;
        let Some(v) = visual.as_mut() else { return };
        let rerender = v.frame.is_none() || (progress_changed && v.to > 0.0);
        if rerender {
            let style = Style { size, scale: v.monitor.scale, dark: v.dark, on: v.on };
            // the "pop" only applies while growing in; fading out keeps full size
            let pop = if v.to > 0.0 { progress } else { 1.0 };
            v.frame = render::render(style, pop);
        }
        let Some(frame) = v.frame.as_ref() else { return };
        let margin = frame.margin as i32;
        let (x, y) = (v.body_pos.0 - margin, v.body_pos.1 - margin);
        if let Some(o) = overlay.as_mut() {
            o.present(rerender.then_some(frame), x, y, opacity * progress);
        }
    }

    fn monitor_for(&self, cursor: (i32, i32)) -> MonitorGeom {
        let logical = platform::cursor::coordinates_are_logical();
        let monitors: Vec<MonitorGeom> = self
            .app
            .available_monitors()
            .unwrap_or_default()
            .into_iter()
            .map(|m| {
                let (pos, size) = (m.position(), m.size());
                let bounds = Rect { x: pos.x, y: pos.y, width: size.width as i32, height: size.height as i32 };
                let wa = m.work_area();
                let work = Rect { x: wa.position.x, y: wa.position.y, width: wa.size.width as i32, height: wa.size.height as i32 };
                let work = if work.width > 0 && work.height > 0 { work } else { bounds };
                if logical {
                    // XWayland root coordinates are logical pixels and X clients render at 1x
                    let s = m.scale_factor().max(1.0);
                    let l = |r: Rect| Rect {
                        x: (r.x as f64 / s).round() as i32,
                        y: (r.y as f64 / s).round() as i32,
                        width: (r.width as f64 / s).round() as i32,
                        height: (r.height as f64 / s).round() as i32,
                    };
                    MonitorGeom { bounds: l(bounds), work: l(work), scale: 1.0 }
                } else {
                    MonitorGeom { bounds, work, scale: m.scale_factor() }
                }
            })
            .collect();
        monitors
            .iter()
            .find(|m| m.bounds.contains(cursor.0, cursor.1))
            .or(monitors.first())
            .copied()
            .unwrap_or(MonitorGeom {
                bounds: Rect { x: 0, y: 0, width: 1920, height: 1080 },
                work: Rect { x: 0, y: 0, width: 1920, height: 1080 },
                scale: 1.0,
            })
    }
}
