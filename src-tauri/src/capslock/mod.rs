//! Caps Lock indicator: detection (platform monitor) -> state machine ->
//! animated overlay at the text cursor of the focused field or next to the
//! pointer (or the compositor OSD where neither position is available).

pub mod geometry;
pub mod render;
pub mod state;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::AppHandle;

use crate::platform;
use crate::storage::settings::{AppearanceMode, CapsLockSettings, IndicatorPosition};
use geometry::Rect;
use render::{Frame, Style};
use state::{Behavior, Command, IndicatorState};

const FRAME: Duration = Duration::from_millis(16);
/// How often the text cursor is re-read while the indicator follows it.
const CARET_POLL: Duration = Duration::from_millis(50);
/// After a text field gets focus, how long to wait for its cursor position
/// before looking for it through accessibility instead.
const FOCUS_CURSOR_WAIT: Duration = Duration::from_millis(250);
/// How soon to try again after the input method could not be watched.
const IME_RETRY: Duration = Duration::from_secs(10);
/// Input contexts remembered (one per text field or app).
const MAX_IME_CONTEXTS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Floating indicator at the text cursor or the mouse pointer.
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
    pub caret: Capability,
    pub cursor: Capability,
    pub overlay: Capability,
    pub osd_available: bool,
    pub summary: String,
    pub alternatives: Vec<String>,
    pub monitoring: bool,
    pub monitor_error: Option<String>,
    pub caps_lock_on: Option<bool>,
}

/// What the input method (IBus) reports about text fields: the cursor is the
/// position apps give it so that candidate windows appear next to the text.
#[derive(Debug, Clone, PartialEq)]
// Windows reads its system caret directly and watches no input method
#[cfg_attr(windows, allow(dead_code))]
pub enum InputMethodEvent {
    /// A text field (input context) got or lost keyboard focus.
    Focus { context: String, focused: bool },
    /// Its text cursor, relative to its window or (`relative == false`) on screen.
    Cursor { context: String, rect: Rect, relative: bool },
}

enum Msg {
    Caps(bool),
    InputMethod(InputMethodEvent),
    Settings(CapsLockSettings),
    /// The app's appearance, which the indicator follows.
    Theme(AppearanceMode),
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
    let caret = capability(
        platform::caret::support(),
        "Read from the focused text field only when Caps Lock changes and while the indicator is visible.",
    );
    let cursor = capability(platform::cursor::support(), "Used to find the monitor and its scale.");
    let overlay = capability(platform::overlay::support(), "Transparent, click-through and never takes keyboard focus.");
    let osd_available = platform::overlay::osd_available();
    // one short sentence for the Settings pane, only when something is off
    let (mode, summary) = if detection.available && overlay.available && caret.available {
        (Mode::Cursor, String::new())
    } else if detection.available && osd_available {
        (Mode::Osd, "On this computer the indicator appears as a system notice.".to_string())
    } else if !detection.available {
        (Mode::Unavailable, "Caps Lock can't be detected on this computer.".to_string())
    } else {
        (Mode::Unavailable, "The indicator can't be shown on this computer.".to_string())
    };
    let alternatives = Vec::new();
    CapsLockSupport {
        mode,
        detection,
        caret,
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
    pub fn start(app: AppHandle, settings: CapsLockSettings, theme: AppearanceMode) -> Arc<Self> {
        let support = probe_support();
        log::info!(
            "caps lock indicator mode {:?} (detection: {}, cursor: {}, overlay: {})",
            support.mode,
            support.detection.method,
            support.cursor.method,
            support.overlay.method
        );
        let (tx, rx) = mpsc::channel();
        let runner_tx = tx.clone();
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
            .name("lummo-capslock".into())
            .spawn(move || Runner::new(app, mode, initial, theme, caps_state, runner_tx).run(rx));
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

    pub fn set_theme(&self, theme: AppearanceMode) {
        let _ = self.tx.send(Msg::Theme(theme));
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
        show_on_focus: s.show_on_focus,
    }
}

/// The text cursor the indicator is placed next to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Anchor(Rect);

impl Anchor {
    fn point(self) -> (i32, i32) {
        (self.0.x, self.0.y)
    }

    /// Top-left of the indicator body.
    fn place(self, body: i32, monitor: &MonitorGeom, position: IndicatorPosition) -> (i32, i32) {
        geometry::place_at_caret(self.0, body, monitor.work, position, monitor.scale)
    }
}

/// Picks the text cursor from the input method (`ime`) and accessibility
/// (`accessible`: the caret and, if known, its field). The input method is
/// exact everywhere (also in fields whose characters accessibility cannot
/// place, like Chrome's address bar) but can be stale; when its position
/// lies outside the field accessibility reports as focused, accessibility's
/// fresh answer wins.
fn choose_caret(ime: Option<Rect>, accessible: Option<(Rect, Option<Rect>)>) -> Option<Rect> {
    match (ime, accessible) {
        (Some(ime), Some((caret, field))) => {
            let in_field = field.is_some_and(|f| {
                let m = 8;
                ime.x >= f.x - m && ime.x <= f.x + f.width + m && ime.y >= f.y - m && ime.y <= f.y + f.height + m
            });
            Some(if in_field { ime } else { caret })
        }
        (ime, accessible) => ime.or(accessible.map(|(caret, _)| caret)),
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// How long the indicator takes to glide to a new place: 0.4 × the animation
/// duration (Normal, 170 ms: about 0.25 s to settle). `0` jumps.
fn glide_time(animation_ms: u64) -> f32 {
    animation_ms as f32 * 0.4 / 1000.0
}

/// One frame of a critically damped spring toward `target`: it starts gently,
/// never overshoots, and keeps its momentum when the target moves on (each
/// keystroke moves the text cursor), so following the cursor looks continuous.
fn smooth_damp(current: f32, target: f32, velocity: &mut f32, smooth_time: f32, dt: f32) -> f32 {
    let omega = 2.0 / smooth_time;
    let x = omega * dt;
    let decay = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current - target;
    let temp = (*velocity + omega * change) * dt;
    *velocity = (*velocity - omega * temp) * decay;
    let next = target + (change + temp) * decay;
    // never past the target; done within half a pixel
    let overshot = (target > current) == (next > target);
    if overshot || ((next - target).abs() < 0.5 && velocity.abs() < 10.0) {
        *velocity = 0.0;
        return target;
    }
    next
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
    /// Where the body belongs (next to the anchor)…
    body_pos: (i32, i32),
    /// …and where it is drawn, gliding toward `body_pos`.
    pos: (f32, f32),
    velocity: (f32, f32),
    stepped: Instant,
}

impl Visual {
    fn gliding(&self) -> bool {
        self.pos != (self.body_pos.0 as f32, self.body_pos.1 as f32)
    }
}

struct Runner {
    app: AppHandle,
    mode: Mode,
    settings: CapsLockSettings,
    theme: AppearanceMode,
    state: IndicatorState,
    caps_state: Arc<Mutex<Option<bool>>>,
    overlay: Option<platform::overlay::Overlay>,
    caret: Option<platform::caret::CaretTracker>,
    caret_unavailable: bool,
    visual: Option<Visual>,
    anchor: Option<Anchor>,
    last_caret_poll: Instant,
    /// Input method events come back through the controller's own queue.
    tx: Sender<Msg>,
    ime: Option<platform::ime::ImeWatch>,
    ime_retry_at: Instant,
    ime_error: Option<String>,
    /// The input context that has keyboard focus, and the last cursor each
    /// context reported.
    ime_focus: Option<String>,
    ime_cursors: HashMap<String, (Rect, bool)>,
    /// The focused context reported a new cursor since it was last placed.
    ime_moved: bool,
    /// A text field got focus with Caps Lock on: show the indicator once its
    /// cursor is known, or at this deadline.
    focus_show_at: Option<Instant>,
}

impl Runner {
    fn new(
        app: AppHandle,
        mode: Mode,
        settings: CapsLockSettings,
        theme: AppearanceMode,
        caps_state: Arc<Mutex<Option<bool>>>,
        tx: Sender<Msg>,
    ) -> Self {
        Self {
            app,
            mode,
            settings,
            theme,
            state: IndicatorState::default(),
            caps_state,
            overlay: None,
            caret: None,
            caret_unavailable: false,
            visual: None,
            anchor: None,
            last_caret_poll: Instant::now(),
            tx,
            ime: None,
            ime_retry_at: Instant::now(),
            ime_error: None,
            ime_focus: None,
            ime_cursors: HashMap::new(),
            ime_moved: false,
            focus_show_at: None,
        }
    }

    fn run(mut self, rx: mpsc::Receiver<Msg>) {
        self.ensure_ime(Instant::now());
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
                    self.ensure_ime(now);
                    self.state.on_caps(on, now, &behavior(&self.settings))
                }
                Ok(Msg::InputMethod(event)) => self.on_input_method(event, now),
                Ok(Msg::Settings(s)) => {
                    let restyle = s.size != self.settings.size || s.corner_radius != self.settings.corner_radius;
                    self.settings = s;
                    self.ensure_ime(now);
                    if restyle {
                        if let Some(v) = self.visual.as_mut() {
                            v.frame = None;
                        }
                    }
                    self.state.on_settings(now, &behavior(&self.settings))
                }
                Ok(Msg::Theme(theme)) => {
                    self.theme = theme;
                    let dark = self.dark();
                    if let Some(v) = self.visual.as_mut() {
                        v.dark = dark;
                        v.frame = None;
                    }
                    Command::None
                }
                Ok(Msg::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                    self.hide_now();
                    break;
                }
                Err(RecvTimeoutError::Timeout) => match self.focus_show_at {
                    // the focused field never reported a cursor to the input
                    // method: show anyway, accessibility may know it
                    Some(at) if now >= at => {
                        self.focus_show_at = None;
                        self.state.on_focus(now, &behavior(&self.settings))
                    }
                    _ => self.state.on_tick(now),
                },
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
        let deadline = match (self.state.deadline(), self.focus_show_at) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
        .map(|d| d.saturating_duration_since(now));
        let animating = self.visual.as_ref().is_some_and(|v| v.progress != v.to || v.gliding());
        let follow = self.visual.is_some() && self.settings.follow_cursor && self.mode == Mode::Cursor;
        let frame = if animating {
            Some(FRAME)
        } else if follow {
            Some(CARET_POLL)
        } else {
            None
        };
        match (deadline, frame) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Watches the input method while the indicator follows the text cursor
    /// (and again after IBus restarts).
    fn ensure_ime(&mut self, now: Instant) {
        let wanted = self.settings.enabled && self.mode == Mode::Cursor;
        if !wanted {
            if self.ime.take().is_some() {
                self.ime_focus = None;
                self.ime_cursors.clear();
            }
            return;
        }
        if self.ime.as_ref().is_some_and(|w| w.is_alive()) || now < self.ime_retry_at {
            return;
        }
        let tx = self.tx.clone();
        match platform::ime::ImeWatch::start(move |event| {
            let _ = tx.send(Msg::InputMethod(event));
        }) {
            Ok(watch) => {
                self.ime = Some(watch);
                self.ime_error = None;
            }
            Err(e) => {
                self.ime = None;
                if self.ime_error.as_deref() != Some(e.as_str()) {
                    log::info!("text cursor positions from the input method are unavailable: {e}");
                    self.ime_error = Some(e);
                }
                self.ime_retry_at = now + IME_RETRY;
            }
        }
    }

    fn on_input_method(&mut self, event: InputMethodEvent, now: Instant) -> Command {
        match event {
            InputMethodEvent::Focus { context, focused: true } => {
                // a field's position from before it got focus can be stale
                // (reported while its page was still being laid out)
                self.ime_cursors.remove(&context);
                self.ime_focus = Some(context);
                self.ime_moved = true;
                // with Caps Lock already on, show the indicator in the new field
                if self.settings.show_on_focus {
                    self.focus_show_at = Some(now + FOCUS_CURSOR_WAIT);
                }
                Command::None
            }
            InputMethodEvent::Focus { context, focused: false } => {
                if self.ime_focus.as_ref() == Some(&context) {
                    self.ime_focus = None;
                }
                Command::None
            }
            InputMethodEvent::Cursor { context, rect, relative } => {
                if self.ime_cursors.len() >= MAX_IME_CONTEXTS && !self.ime_cursors.contains_key(&context) {
                    self.ime_cursors.retain(|c, _| self.ime_focus.as_ref() == Some(c));
                }
                let focused = self.ime_focus.as_ref() == Some(&context);
                self.ime_cursors.insert(context, (rect, relative));
                if !focused {
                    return Command::None;
                }
                self.ime_moved = true;
                if self.focus_show_at.take().is_some() {
                    return self.state.on_focus(now, &behavior(&self.settings));
                }
                Command::None
            }
        }
    }

    /// The focused field's cursor as last reported to the input method, on
    /// screen. `fresh` re-reads where its window is.
    fn ime_caret(&mut self, fresh: bool) -> Option<Rect> {
        let context = self.ime_focus.clone()?;
        let (rect, relative) = *self.ime_cursors.get(&context)?;
        self.ime_moved = false;
        let caret = self.caret_tracker()?.map_input_method(rect, relative, fresh);
        log::debug!("input method cursor {rect:?} (relative: {relative}) -> {caret:?}");
        caret
    }

    fn dark(&self) -> bool {
        match self.theme {
            AppearanceMode::Dark => true,
            AppearanceMode::Light => false,
            AppearanceMode::System => platform::appearance::read().dark.unwrap_or(false),
        }
    }

    fn show(&mut self, on: bool, now: Instant) {
        match self.mode {
            Mode::Unavailable => {}
            Mode::Osd => match platform::overlay::show_osd(on) {
                Ok(()) if on => crate::analytics::count("caps_lock_osd_shown"),
                Ok(()) => {}
                Err(e) => log::warn!("could not show the Caps Lock OSD: {e}"),
            },
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
        // already visible (e.g. on -> off flash): if the text cursor cannot be
        // found again, keep the indicator where it is
        let visible = self.visual.as_ref().is_some_and(|v| v.to > 0.0);
        let anchor = match self.find_anchor() {
            Some(a) => a,
            None => match self.anchor.filter(|_| visible) {
                Some(a) => a,
                None => {
                    if on {
                        crate::analytics::count("caps_lock_no_cursor");
                    }
                    return;
                }
            },
        };
        if on {
            crate::analytics::count("caps_lock_shown");
        }
        let dark = self.dark();
        let monitor = self.monitor_for(anchor.point());
        let body = (self.settings.size as f64 * monitor.scale).round() as i32;
        let body_pos = anchor.place(body, &monitor, self.settings.position);
        log::debug!("caps lock indicator at {body_pos:?} for {anchor:?}");
        self.anchor = Some(anchor);
        self.last_caret_poll = now;
        let animate = self.settings.animation.duration_ms() > 0;
        match self.visual.as_mut() {
            // already visible: swap the artwork, keep opacity, glide to the new place
            Some(v) if v.to > 0.0 => {
                // another monitor can have another scale: jump there rather than glide
                if v.monitor.bounds != monitor.bounds {
                    v.pos = (body_pos.0 as f32, body_pos.1 as f32);
                    v.velocity = (0.0, 0.0);
                }
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
                    pos: (body_pos.0 as f32, body_pos.1 as f32),
                    velocity: (0.0, 0.0),
                    stepped: now,
                });
            }
        }
    }

    /// The text cursor of the focused field: from the input method, which
    /// knows the cursor of every app that types through it, cross-checked by
    /// accessibility, which covers the others and catches stale reports.
    /// `None` where there is no text cursor: then nothing is shown.
    fn find_anchor(&mut self) -> Option<Anchor> {
        let ime = self.ime_caret(true);
        let accessible = self.caret_tracker().and_then(|c| c.locate().map(|caret| (caret, c.focused_field())));
        let caret = choose_caret(ime, accessible);
        if caret.is_none() {
            log::debug!("no text cursor found; not showing the indicator");
        }
        caret.map(Anchor)
    }

    fn caret_tracker(&mut self) -> Option<&mut platform::caret::CaretTracker> {
        if self.caret.is_none() && !self.caret_unavailable {
            match platform::caret::CaretTracker::new() {
                Ok(c) => self.caret = Some(c),
                Err(e) => {
                    log::info!("text cursor positions are unavailable: {e}");
                    self.caret_unavailable = true;
                }
            }
        }
        self.caret.as_mut()
    }

    /// The anchor's new position while following, if it moved.
    fn poll_anchor(&mut self, now: Instant) -> Option<Anchor> {
        self.anchor?;
        // the input method reports every move as it happens
        if self.ime_moved {
            if let Some(caret) = self.ime_caret(false) {
                return Some(Anchor(caret)).filter(|a| Some(*a) != self.anchor);
            }
        }
        if self.ime_focus.as_ref().is_some_and(|c| self.ime_cursors.contains_key(c)) {
            return None;
        }
        if now.duration_since(self.last_caret_poll) < CARET_POLL {
            return None;
        }
        self.last_caret_poll = now;
        // a field that lost focus keeps the indicator where it was
        self.caret.as_mut()?.refresh().map(Anchor).filter(|a| Some(*a) != self.anchor)
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
        self.anchor = None;
        if let Some(o) = self.overlay.as_mut() {
            o.hide();
        }
        if let Some(c) = self.caret.as_mut() {
            c.release();
        }
    }

    /// Advances animation and anchor following, then presents the frame.
    fn step(&mut self, now: Instant) {
        let duration = Duration::from_millis(self.settings.animation.duration_ms().max(1));
        let follow = self.settings.follow_cursor;
        let position = self.settings.position;
        let size = self.settings.size;
        let corner_radius = self.settings.corner_radius as f32 / 100.0;
        let opacity = self.settings.opacity;
        let glide = glide_time(self.settings.animation.duration_ms());

        let moved_to = if follow && self.visual.is_some() { self.poll_anchor(now) } else { None };
        if let Some(a) = moved_to {
            self.anchor = Some(a);
        }
        let new_monitor = moved_to.and_then(|a| {
            let (x, y) = a.point();
            let current = self.visual.as_ref()?.monitor;
            (!current.bounds.contains(x, y)).then(|| self.monitor_for((x, y)))
        });

        let (progress, progress_changed, finished) = {
            let Some(v) = self.visual.as_mut() else { return };
            if let Some(m) = new_monitor {
                v.monitor = m;
                v.frame = None; // scale may differ between monitors
            }
            if let Some(a) = moved_to {
                let body = (size as f64 * v.monitor.scale).round() as i32;
                v.body_pos = a.place(body, &v.monitor, position);
                log::debug!("caps lock indicator follows to {:?} for {a:?}", v.body_pos);
            }
            // after a pause (polling slows down when nothing moves) a glide starts with one frame's step
            let dt = now.duration_since(v.stepped).as_secs_f32().min(2.0 * FRAME.as_secs_f32());
            v.stepped = now;
            let target = (v.body_pos.0 as f32, v.body_pos.1 as f32);
            if new_monitor.is_some() || glide <= 0.0 {
                v.pos = target;
                v.velocity = (0.0, 0.0);
            } else if dt > 0.0 {
                v.pos.0 = smooth_damp(v.pos.0, target.0, &mut v.velocity.0, glide, dt);
                v.pos.1 = smooth_damp(v.pos.1, target.1, &mut v.velocity.1, glide, dt);
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
            let style = Style { size, scale: v.monitor.scale, dark: v.dark, on: v.on, radius: corner_radius };
            // the "pop" only applies while growing in; fading out keeps full size
            let pop = if v.to > 0.0 { progress } else { 1.0 };
            v.frame = render::render(style, pop);
        }
        let Some(frame) = v.frame.as_ref() else { return };
        let margin = frame.margin as i32;
        let (x, y) = (v.pos.0.round() as i32 - margin, v.pos.1.round() as i32 - margin);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::settings::AnimationSpeed;

    /// Glides from `from` to `to` at 60 frames per second.
    fn glide(from: f32, to: f32, smooth_time: f32, frames: usize) -> Vec<f32> {
        let (mut pos, mut velocity) = (from, 0.0);
        (0..frames)
            .map(|_| {
                pos = smooth_damp(pos, to, &mut velocity, smooth_time, 1.0 / 60.0);
                pos
            })
            .collect()
    }

    #[test]
    fn glide_starts_gently_never_overshoots_and_settles_exactly() {
        let path = glide(0.0, 100.0, glide_time(170), 40);
        assert!(path.windows(2).all(|w| w[1] >= w[0]), "moves one way only: {path:?}");
        assert!(path.iter().all(|p| *p <= 100.0), "never passes the target: {path:?}");
        assert!(path[0] < 15.0, "first frame moves only a little: {path:?}");
        let settled = path.iter().position(|p| *p == 100.0).expect("settles on the target");
        assert!((10..=20).contains(&settled), "settles in about a quarter second, took {settled} frames");
        // leftward too
        assert_eq!(*glide(100.0, 40.0, glide_time(170), 40).last().unwrap(), 40.0);
    }

    #[test]
    fn glide_keeps_its_momentum_when_the_target_moves_on() {
        let (mut pos, mut velocity) = (0.0, 0.0);
        for _ in 0..3 {
            pos = smooth_damp(pos, 10.0, &mut velocity, glide_time(170), 1.0 / 60.0);
        }
        assert!(velocity > 0.0);
        // the next keystroke moves the cursor further: no stop, no jump
        let next = smooth_damp(pos, 20.0, &mut velocity, glide_time(170), 1.0 / 60.0);
        assert!(next > pos && next < 20.0);
    }

    fn r(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect { x, y, width, height }
    }

    #[test]
    fn the_input_method_wins_inside_the_focused_field() {
        // Chrome's address bar: accessibility only knows the field's start
        let bar = r(184, 66, 1600, 24);
        assert_eq!(choose_caret(Some(r(1457, 71, 1, 15)), Some((r(188, 68, 1, 20), Some(bar)))), Some(r(1457, 71, 1, 15)));
    }

    #[test]
    fn a_stale_input_method_position_loses_to_accessibility() {
        let field = r(47, 150, 330, 22);
        assert_eq!(choose_caret(Some(r(571, 240, 1, 19)), Some((r(93, 152, 1, 18), Some(field)))), Some(r(93, 152, 1, 18)));
        // without a known field, the fresh accessibility answer wins too
        assert_eq!(choose_caret(Some(r(571, 240, 1, 19)), Some((r(93, 152, 1, 18), None))), Some(r(93, 152, 1, 18)));
    }

    #[test]
    fn either_source_alone_is_used() {
        // VS Code and Terminal report only to the input method
        assert_eq!(choose_caret(Some(r(771, 969, 1, 15)), None), Some(r(771, 969, 1, 15)));
        assert_eq!(choose_caret(None, Some((r(10, 20, 1, 18), None))), Some(r(10, 20, 1, 18)));
        assert_eq!(choose_caret(None, None), None);
    }

    #[test]
    fn animations_off_means_no_glide() {
        assert_eq!(glide_time(AnimationSpeed::Off.duration_ms()), 0.0);
        assert!(glide_time(AnimationSpeed::Slow.duration_ms()) > glide_time(AnimationSpeed::Fast.duration_ms()));
    }
}
