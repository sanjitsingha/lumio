//! Shared X11 helpers for the Caps Lock overlay and pointer tracker. They talk
//! to the X server directly (x11rb), which is a real X server on X11 sessions
//! and XWayland on Wayland sessions.

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    AtomEnum, ClipOrdering, ColormapAlloc, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, Rectangle,
    VisualClass, Visualid, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

pub struct ArgbContext {
    pub conn: RustConnection,
    pub screen: usize,
    pub root: Window,
    pub visual: Visualid,
    pub colormap: u32,
}

pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl ArgbContext {
    /// Connects and finds a 32-bit (alpha-capable) TrueColor visual.
    pub fn connect() -> Result<Self, String> {
        let (conn, screen) = x11rb::connect(None).map_err(|e| format!("cannot reach the X server ({e})"))?;
        let s = conn.setup().roots.get(screen).ok_or("X11 screen not found")?;
        let root = s.root;
        let visual = s
            .allowed_depths
            .iter()
            .filter(|d| d.depth == 32)
            .flat_map(|d| d.visuals.iter())
            .find(|v| v.class == VisualClass::TRUE_COLOR)
            .map(|v| v.visual_id)
            .ok_or("the X server has no 32-bit visual for transparent windows")?;
        let colormap = conn.generate_id().map_err(err)?;
        conn.create_colormap(ColormapAlloc::NONE, colormap, root, visual).map_err(err)?;
        Ok(Self { conn, screen, root, visual, colormap })
    }

    /// An override-redirect (unmanaged: never focused, decorated or listed)
    /// transparent window.
    ///
    /// It deliberately has no `_NET_WM_WINDOW_TYPE`: Gala moves windows typed
    /// `NOTIFICATION` into its notification stack, which animates the first
    /// one and leaves every later map invisible.
    pub fn create_window(&self, x: i16, y: i16, w: u16, h: u16, events: EventMask) -> Result<Window, String> {
        let win = self.conn.generate_id().map_err(err)?;
        self.conn
            .create_window(
                32,
                win,
                self.root,
                x,
                y,
                w.max(1),
                h.max(1),
                0,
                WindowClass::INPUT_OUTPUT,
                self.visual,
                &CreateWindowAux::new()
                    .override_redirect(1)
                    .background_pixel(0)
                    .border_pixel(0)
                    .colormap(self.colormap)
                    .event_mask(events),
            )
            .map_err(err)?;
        self.conn
            .change_property8(PropMode::REPLACE, win, AtomEnum::WM_CLASS, AtomEnum::STRING, b"lumio\0Lumio\0")
            .map_err(err)?;
        Ok(win)
    }

    /// Sets where the window accepts pointer input; an empty list makes it
    /// fully click-through.
    pub fn set_input(&self, win: Window, rects: &[Rectangle]) -> Result<(), String> {
        self.conn
            .shape_rectangles(shape::SO::SET, shape::SK::INPUT, ClipOrdering::UNSORTED, win, 0, 0, rects)
            .map_err(err)?;
        Ok(())
    }

    /// True when a compositing manager runs (needed for per-pixel alpha).
    pub fn compositing(&self) -> bool {
        let name = format!("_NET_WM_CM_S{}", self.screen);
        self.conn
            .intern_atom(false, name.as_bytes())
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|a| self.conn.get_selection_owner(a.atom).ok()?.reply().ok())
            .is_some_and(|r| r.owner != 0)
    }

    pub fn has_shape(&self) -> bool {
        self.conn.extension_information(shape::X11_EXTENSION_NAME).ok().flatten().is_some()
    }
}
