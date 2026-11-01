//! Pointer position for the Caps Lock indicator.
//!
//! * **X11**: `QueryPointer` on the root window.
//! * **Wayland**: native Wayland clients cannot see the pointer outside their
//!   own surfaces, but XWayland receives real pointer events for X11 windows.
//!   While the indicator is visible, Lumio maps a transparent, never-focused
//!   override-redirect window over the screen. Mutter then reports the actual
//!   pointer position to it. Its input region excludes a square "hole"
//!   around the pointer, so clicks at the pointer always reach the app below;
//!   when the pointer leaves the hole the window gets a motion event and the
//!   hole (and indicator) follow. The window is unmapped as soon as the
//!   indicator hides, so nothing is tracked otherwise.

use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt as _, EventMask, Rectangle, StackMode, Window};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use super::session::{self, SessionType};
use super::x11util::{err, rects_with_hole, ArgbContext};

/// Half the size of the click-through square around the pointer (X pixels).
const HOLE_RADIUS: i32 = 44;
/// How long to wait for XWayland's first real pointer report after mapping
/// (Mutter's re-pick typically arrives within 20-30 ms).
const FIRST_REPORT_TIMEOUT: Duration = Duration::from_millis(120);

pub fn support() -> Result<&'static str, String> {
    match session::session_type() {
        SessionType::X11 => Ok("X11 pointer query"),
        SessionType::Wayland => {
            let ctx = ArgbContext::connect()
                .map_err(|e| format!("Wayland hides the pointer from apps and XWayland is not available ({e})."))?;
            if !ctx.has_shape() {
                return Err("XWayland lacks the SHAPE extension needed for click-through tracking.".into());
            }
            Ok("XWayland pointer tracking")
        }
        SessionType::Unknown => Err("No graphical session was detected.".into()),
    }
}

/// Root coordinates are logical pixels under XWayland (scale 1 for X clients).
pub fn coordinates_are_logical() -> bool {
    session::is_wayland()
}

pub enum Tracker {
    X11 { conn: RustConnection, root: Window },
    XWayland(XWaylandTracker),
}

impl Tracker {
    pub fn new() -> Result<Self, String> {
        support()?;
        if session::is_wayland() {
            Ok(Tracker::XWayland(XWaylandTracker::new()?))
        } else {
            let (conn, screen) = x11rb::connect(None).map_err(err)?;
            let root = conn.setup().roots.get(screen).ok_or("X11 screen not found")?.root;
            Ok(Tracker::X11 { conn, root })
        }
    }

    /// Current pointer position in X root coordinates.
    pub fn position(&mut self) -> Option<(i32, i32)> {
        match self {
            Tracker::X11 { conn, root } => {
                let reply = conn.query_pointer(*root).ok()?.reply().ok()?;
                Some((reply.root_x as i32, reply.root_y as i32))
            }
            Tracker::XWayland(t) => t.position(),
        }
    }

    /// Stops observing the pointer (indicator hidden).
    pub fn release(&mut self) {
        if let Tracker::XWayland(t) = self {
            t.release();
        }
    }
}

pub struct XWaylandTracker {
    ctx: ArgbContext,
    window: Window,
    mapped: bool,
    last: Option<(i32, i32)>,
}

impl XWaylandTracker {
    fn new() -> Result<Self, String> {
        let ctx = ArgbContext::connect()?;
        let window = ctx.create_window(
            0,
            0,
            ctx.width,
            ctx.height,
            EventMask::POINTER_MOTION | EventMask::ENTER_WINDOW,
        )?;
        Ok(Self { ctx, window, mapped: false, last: None })
    }

    fn full_area(&self) -> Rectangle {
        Rectangle { x: 0, y: 0, width: self.ctx.width, height: self.ctx.height }
    }

    fn set_hole(&self, (x, y): (i32, i32)) {
        let rects = rects_with_hole(self.ctx.width as i32, self.ctx.height as i32, x, y, HOLE_RADIUS);
        if let Err(e) = self.ctx.set_input(self.window, &rects) {
            log::warn!("could not update the pointer tracker region: {e}");
        }
        let _ = self.ctx.conn.flush();
    }

    fn position(&mut self) -> Option<(i32, i32)> {
        if !self.mapped {
            return self.start();
        }
        let mut moved = None;
        while let Ok(Some(event)) = self.ctx.conn.poll_for_event() {
            match event {
                Event::MotionNotify(e) => moved = Some((e.root_x as i32, e.root_y as i32)),
                Event::EnterNotify(e) => moved = Some((e.root_x as i32, e.root_y as i32)),
                _ => {}
            }
        }
        if let Some(p) = moved {
            self.last = Some(p);
            self.set_hole(p);
        }
        self.last
    }

    /// Maps the tracker and waits for XWayland's first *motion* report. The
    /// enter event generated on map carries XWayland's stale internal
    /// position, so it is ignored.
    fn start(&mut self) -> Option<(i32, i32)> {
        let conn = &self.ctx.conn;
        if self.ctx.set_input(self.window, &[self.full_area()]).is_err() {
            return None;
        }
        let _ = conn.map_window(self.window);
        let _ = conn.configure_window(self.window, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE));
        let _ = conn.flush();
        self.mapped = true;
        let deadline = Instant::now() + FIRST_REPORT_TIMEOUT;
        let mut found = None;
        while Instant::now() < deadline {
            match self.ctx.conn.poll_for_event() {
                Ok(Some(Event::MotionNotify(e))) => {
                    found = Some((e.root_x as i32, e.root_y as i32));
                    break;
                }
                Ok(Some(_)) => {}
                Ok(None) => std::thread::sleep(Duration::from_millis(2)),
                Err(e) => {
                    log::warn!("XWayland connection error: {e}");
                    break;
                }
            }
        }
        // No motion means the pointer has not moved since XWayland last saw it,
        // so its internal position is current (a later motion still corrects it).
        let found = found.or_else(|| {
            let reply = self.ctx.conn.query_pointer(self.window).ok()?.reply().ok()?;
            reply.same_screen.then_some((reply.root_x as i32, reply.root_y as i32))
        });
        match found {
            Some(p) => {
                self.last = Some(p);
                self.set_hole(p);
                Some(p)
            }
            None => {
                log::info!("XWayland could not report the pointer position");
                self.release();
                None
            }
        }
    }

    fn release(&mut self) {
        if self.mapped {
            let _ = self.ctx.conn.unmap_window(self.window);
            let _ = self.ctx.conn.flush();
            self.mapped = false;
        }
        self.last = None;
    }
}
