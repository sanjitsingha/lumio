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
/// How often the focused field is checked for selected text, which hides
/// its text cursor and so the indicator.
const SELECTION_POLL: Duration = Duration::from_millis(100);
/// After a text field gets focus, how long to wait for its cursor position
/// before looking for it through accessibility instead. Windows reports a
/// field only once its caret exists, so there it looks right away.
const FOCUS_CURSOR_WAIT: Duration = Duration::from_millis(if cfg!(windows) { 0 } else { 250 });
/// How long the text cursor may be gone (its field lost focus) before the
/// indicator hides; moving straight to another field of the window keeps
/// it. Another window coming to the front hides it at once.
const FOCUS_LOST_GRACE: Duration = Duration::from_millis(300);
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
// Windows reports focus only and reads cursor positions from the system caret
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
        (Some(ime), Some((caret, field))) => Some(if field.is_some_and(|f| in_field(f, ime)) { ime } else { caret }),
        (ime, accessible) => ime.or(accessible.map(|(caret, _)| caret)),
    }
}

/// Whether the text cursor `caret` lies in `field`, give or take a few
/// pixels (input method rectangles can stick out a little).
fn in_field(field: Rect, caret: Rect) -> bool {
    let m = 8;
    caret.x >= field.x - m
        && caret.x <= field.x + field.width + m
        && caret.y >= field.y - m
        && caret.y <= field.y + field.height + m
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// How long the indicator takes to glide to a new place: 0.4 × the animation
/// duration (Normal, 170 ms: about 0.25 s to settle). `0` jumps.
fn glide_time(animation_ms: u64) -> f32 {
    animation_ms as f32 * 0.4 / 1000.0
}

/// How long the appear pop takes to settle: twice the fade, so the bounce
/// plays out after the indicator is already fully visible.
fn pop_time(animation_ms: u64) -> f32 {
    animation_ms as f32 * 2.0 / 1000.0
}

/// Body scale the appear pop starts from.
const POP_FROM: f32 = 0.8;

/// Body scale `t` seconds after appearing: an underdamped spring from
/// `POP_FROM` that overshoots to about 104%, dips once below 100% and
/// settles exactly at full size after `settle` seconds.
fn spring_pop(t: f32, settle: f32) -> f32 {
    const DAMPING: f32 = 0.45;
    if settle <= 0.0 || t >= settle {
        return 1.0;
    }
    // the envelope decays to 1% of the start offset by `settle`
    let omega = 100f32.ln() / (DAMPING * settle);
    let damped = omega * (1.0 - DAMPING * DAMPING).sqrt();
    let t = t.max(0.0);
    let envelope = (-DAMPING * omega * t).exp();
    1.0 - (1.0 - POP_FROM) * envelope * ((damped * t).cos() + DAMPING * omega / damped * (damped * t).sin())
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
    dark: bool,
    from: f32,
    to: f32,
    start: Instant,
    progress: f32,
    /// Body scale while popping in; kept as it is while fading out.
    pop: f32,
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

    fn popping(&self) -> bool {
        self.to > 0.0 && self.pop != 1.0
    }

    /// Leaves where it is drawn and appears afresh at `body_pos`: fades in
    /// and pops there.
    fn pop_in(&mut self, now: Instant, animate: bool) {
        self.pos = (self.body_pos.0 as f32, self.body_pos.1 as f32);
        self.velocity = (0.0, 0.0);
        self.from = if animate { 0.0 } else { 1.0 };
        self.to = 1.0;
        self.start = now;
        self.progress = self.from;
        self.pop = if animate { POP_FROM } else { 1.0 };
        self.frame = None;
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
    /// The text cursor went away: hide the indicator at this deadline unless
    /// one turns up again.
    blur_at: Option<Instant>,
    /// A text field got focus since the indicator was last placed: when it
    /// moves there, it pops in at the new field rather than gliding over.
    refocused: bool,
    /// The field the indicator stands in, where accessibility knows it: a
    /// cursor outside it is in another field.
    field: Option<Rect>,
    /// The indicator stands at the input method's cursor: it reports every
    /// move, so accessibility is not polled while following.
    follow_ime: bool,
    /// The focused field has text selected: the indicator waits for its text
    /// cursor to come back.
    selection: bool,
    selection_checked: Instant,
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
            blur_at: None,
            refocused: false,
            field: None,
            follow_ime: false,
            selection: false,
            selection_checked: Instant::now(),
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
                Err(RecvTimeoutError::Timeout) => match (self.blur_at, self.focus_show_at) {
                    (Some(at), _) if now >= at => {
                        self.blur_at = None;
                        let command = self.state.on_blur();
                        if command == Command::Hide {
                            log::debug!("no text field has focus; hiding the indicator");
                        }
                        command
                    }
                    // the focused field never reported a cursor to the input
                    // method: show anyway, accessibility may know it
                    (_, Some(at)) if now >= at => {
                        self.focus_show_at = None;
                        self.state.on_focus(now, &behavior(&self.settings))
                    }
                    _ => self.state.on_tick(now),
                },
            };
            let command = match command {
                Command::None => self.check_selection(now),
                command => command,
            };
            match command {
                Command::Show => self.show(),
                Command::Hide => self.begin_hide(now),
                Command::None => {}
            }
            self.step(now);
        }
    }

    fn next_wake(&self) -> Option<Duration> {
        let now = Instant::now();
        let deadline = [self.state.deadline(), self.focus_show_at, self.blur_at]
            .into_iter()
            .flatten()
            .min()
            .map(|d| d.saturating_duration_since(now));
        let animating = self.visual.as_ref().is_some_and(|v| v.progress != v.to || v.popping() || v.gliding());
        let follow = self.visual.is_some() && self.settings.follow_cursor && self.mode == Mode::Cursor;
        let frame = if animating {
            Some(FRAME)
        } else if follow {
            Some(CARET_POLL)
        } else if self.visual.is_some() || self.selection {
            Some(SELECTION_POLL)
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
                self.blur_at = None;
                self.refocused = true;
                self.selection = false;
                // with Caps Lock already on, show the indicator in the new field
                // (the state decides whether it should)
                self.focus_show_at = Some(now + FOCUS_CURSOR_WAIT);
                Command::None
            }
            // also when no focus was seen yet: the indicator may have found
            // the field itself when Caps Lock was turned on
            InputMethodEvent::Focus { context, focused: false } => {
                if self.ime_focus.as_ref().is_none_or(|c| *c == context) {
                    self.ime_focus = None;
                    self.focus_show_at = None;
                    self.selection = false;
                    self.cursor_gone(now);
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

    fn show(&mut self) {
        match self.mode {
            Mode::Unavailable => {}
            Mode::Osd => match platform::overlay::show_osd() {
                Ok(()) => crate::analytics::count("caps_lock_osd_shown"),
                Err(e) => log::warn!("could not show the Caps Lock OSD: {e}"),
            },
            Mode::Cursor => self.show_cursor_overlay(),
        }
    }

    fn show_cursor_overlay(&mut self) {
        if self.overlay.is_none() {
            match platform::overlay::Overlay::new() {
                Ok(o) => self.overlay = Some(o),
                Err(e) => {
                    log::error!("could not create the Caps Lock overlay: {e}");
                    return;
                }
            }
        }
        let refocused = std::mem::take(&mut self.refocused);
        // already visible (its selection collapsed again): if the text cursor
        // cannot be found again, keep the indicator where it is
        let visible = self.visual.as_ref().is_some_and(|v| v.to > 0.0);
        let found = self.find_anchor();
        let switched = self.window_switched();
        // finding the text cursor can take a few hundred milliseconds in
        // Chrome: the indicator appears (and pops) once it is found
        let now = Instant::now();
        let anchor = match found {
            Some(a) => {
                self.blur_at = None;
                a
            }
            None => match self.anchor.filter(|_| visible) {
                Some(a) => a,
                None => {
                    crate::analytics::count("caps_lock_no_cursor");
                    return;
                }
            },
        };
        // selected text hides the text cursor: wait for it to come back
        self.selection = self.caret.as_mut().is_some_and(|c| c.has_selection());
        if self.selection {
            log::debug!("text is selected; the indicator waits for the text cursor");
            self.selection_checked = now;
            self.begin_hide(now);
            return;
        }
        crate::analytics::count("caps_lock_shown");
        let dark = self.dark();
        let monitor = self.monitor_for(anchor.point());
        let body = (self.settings.size as f64 * monitor.scale).round() as i32;
        let body_pos = anchor.place(body, &monitor, self.settings.position);
        log::debug!("caps lock indicator at {body_pos:?} for {anchor:?}");
        let new_field = (refocused || switched) && self.anchor != Some(anchor);
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
                v.dark = dark;
                v.frame = None;
                v.monitor = monitor;
                v.body_pos = body_pos;
                if new_field {
                    v.pop_in(now, animate);
                }
            }
            _ => {
                self.visual = Some(Visual {
                    dark,
                    from: if animate { 0.0 } else { 1.0 },
                    to: 1.0,
                    start: now,
                    progress: if animate { 0.0 } else { 1.0 },
                    pop: if animate { POP_FROM } else { 1.0 },
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
        self.field = accessible.and_then(|(_, field)| field);
        self.follow_ime = ime.is_some() && caret == ime;
        if caret.is_none() {
            log::debug!("no text cursor found; not showing the indicator");
        }
        caret.map(Anchor)
    }

    /// Whether input method positions came from another window since the
    /// indicator was last placed.
    fn window_switched(&mut self) -> bool {
        self.caret.as_mut().is_some_and(|c| c.take_window_switch())
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
                // accessibility placed the indicator; its field cannot vouch
                // for input method positions, which disagreed with it
                if !self.follow_ime {
                    self.follow_ime = true;
                    self.field = None;
                }
                return Some(Anchor(caret)).filter(|a| Some(*a) != self.anchor);
            }
        }
        // where accessibility placed the indicator (the input method's cursor
        // was missing or outside the focused field), accessibility follows it
        if self.follow_ime && self.ime_focus.as_ref().is_some_and(|c| self.ime_cursors.contains_key(c)) {
            return None;
        }
        if now.duration_since(self.last_caret_poll) < CARET_POLL {
            return None;
        }
        self.last_caret_poll = now;
        // a field that lost focus keeps the indicator where it was until it hides
        let refreshed = self.caret.as_mut()?.refresh();
        match refreshed {
            Some(caret) => {
                self.blur_at = None;
                Some(Anchor(caret)).filter(|a| Some(*a) != self.anchor)
            }
            None => {
                if self.visual.as_ref().is_some_and(|v| v.to > 0.0) {
                    self.cursor_gone(now);
                }
                None
            }
        }
    }

    /// The text cursor went away: the indicator hides after a moment, in
    /// case focus moves straight to another field, but at once when another
    /// window came to the front. It belongs to the window you type in, never
    /// to one behind it.
    fn cursor_gone(&mut self, now: Instant) {
        let switched = self.caret.as_ref().is_some_and(|c| c.window_changed());
        if switched {
            log::debug!("another window came to the front; hiding the indicator");
        }
        let at = if switched { now } else { now + FOCUS_LOST_GRACE };
        self.blur_at = Some(self.blur_at.map_or(at, |b| b.min(at)));
    }

    /// Selected text hides the field's text cursor, and the indicator with
    /// it: checked while the indicator shows, and while it waits for the
    /// selection to collapse into a text cursor again.
    fn check_selection(&mut self, now: Instant) -> Command {
        let showing = self.visual.as_ref().is_some_and(|v| v.to > 0.0);
        if !(showing || self.selection) || now.duration_since(self.selection_checked) < SELECTION_POLL {
            return Command::None;
        }
        self.selection_checked = now;
        let waiting = self.selection && self.state.caps_lock() == Some(true);
        let Some(caret) = self.caret.as_mut().filter(|_| showing || waiting) else {
            self.selection = false;
            return Command::None;
        };
        if showing {
            if caret.has_selection() {
                log::debug!("text was selected; hiding the indicator until the text cursor is back");
                self.selection = true;
                return Command::Hide;
            }
            return Command::None;
        }
        // the field lost focus: its focus report takes over
        if caret.refresh().is_none() {
            self.selection = false;
            return Command::None;
        }
        if caret.has_selection() {
            return Command::None;
        }
        log::debug!("the selection collapsed into a text cursor");
        self.selection = false;
        self.state.on_cursor_back(now, &behavior(&self.settings))
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
        self.blur_at = None;
        self.refocused = false;
        self.field = None;
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
        let settle = pop_time(self.settings.animation.duration_ms());
        let animate = self.settings.animation.duration_ms() > 0;

        let moved_to = if follow && self.visual.is_some() { self.poll_anchor(now) } else { None };
        // placing the cursor of another window can take a moment
        let now = if moved_to.is_some() { Instant::now() } else { now };
        if let Some(a) = moved_to {
            self.anchor = Some(a);
        }
        // the cursor moved to another text field: one that got focus, one in
        // another window, or one outside the field it was in. It pops in
        // there; gliding is for the cursor moving while you type.
        let new_field = match moved_to {
            Some(Anchor(caret)) if self.visual.as_ref().is_some_and(|v| v.to > 0.0) => {
                let refocused = std::mem::take(&mut self.refocused);
                let switched = self.window_switched();
                let left = self.follow_ime && self.field.is_some_and(|f| !in_field(f, caret));
                refocused || switched || left
            }
            _ => false,
        };
        if new_field {
            self.field = None;
        }
        let new_monitor = moved_to.and_then(|a| {
            let (x, y) = a.point();
            let current = self.visual.as_ref()?.monitor;
            (!current.bounds.contains(x, y)).then(|| self.monitor_for((x, y)))
        });

        let (progress, changed, finished) = {
            let Some(v) = self.visual.as_mut() else { return };
            if let Some(m) = new_monitor {
                v.monitor = m;
                v.frame = None; // scale may differ between monitors
            }
            if let Some(a) = moved_to {
                let body = (size as f64 * v.monitor.scale).round() as i32;
                v.body_pos = a.place(body, &v.monitor, position);
                log::debug!("caps lock indicator follows to {:?} for {a:?}", v.body_pos);
                if new_field {
                    v.pop_in(now, animate);
                }
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
            let elapsed = now.duration_since(v.start).as_secs_f32();
            let t = (elapsed / duration.as_secs_f32()).clamp(0.0, 1.0);
            let progress = if v.from == v.to { v.to } else { v.from + (v.to - v.from) * ease_out_cubic(t) };
            let mut changed = (progress - v.progress).abs() > f32::EPSILON;
            v.progress = progress;
            // the spring only runs while appearing; fading out keeps the size it has
            if v.popping() {
                let pop = spring_pop(elapsed, settle);
                changed |= pop != v.pop;
                v.pop = pop;
            }
            (progress, changed, v.to == 0.0 && t >= 1.0)
        };
        if finished {
            self.hide_now();
            return;
        }
        let Self { visual, overlay, .. } = self;
        let Some(v) = visual.as_mut() else { return };
        let rerender = v.frame.is_none() || (changed && v.to > 0.0);
        if rerender {
            let style = Style { size, scale: v.monitor.scale, dark: v.dark, radius: corner_radius };
            v.frame = render::render(style, v.pop);
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

    #[test]
    fn pop_springs_past_full_size_once_and_settles() {
        let settle = pop_time(AnimationSpeed::Normal.duration_ms());
        let path: Vec<f32> = (0..=30).map(|i| spring_pop(i as f32 / 60.0, settle)).collect();
        assert_eq!(path[0], POP_FROM);
        let peak = path.iter().cloned().fold(0.0, f32::max);
        assert!((1.02..=1.06).contains(&peak), "a slight overshoot, peaked at {peak}");
        let peak_frame = path.iter().position(|p| *p == peak).unwrap();
        assert!((4..=10).contains(&peak_frame), "peaks soon after the fade, at frame {peak_frame}");
        assert!(path[peak_frame..].iter().all(|p| *p > 0.98), "dips back only a little: {path:?}");
        assert_eq!(*path.last().unwrap(), 1.0, "settles exactly");
    }

    #[test]
    fn animations_off_means_no_pop() {
        assert_eq!(spring_pop(0.0, pop_time(AnimationSpeed::Off.duration_ms())), 1.0);
    }
}
