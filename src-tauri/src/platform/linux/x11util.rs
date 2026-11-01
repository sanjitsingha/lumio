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
    pub width: u16,
    pub height: u16,
}

pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl ArgbContext {
    /// Connects and finds a 32-bit (alpha-capable) TrueColor visual.
    pub fn connect() -> Result<Self, String> {
        let (conn, screen) = x11rb::connect(None).map_err(|e| format!("cannot reach the X server ({e})"))?;
        let s = conn.setup().roots.get(screen).ok_or("X11 screen not found")?;
        let (root, width, height) = (s.root, s.width_in_pixels, s.height_in_pixels);
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
        Ok(Self { conn, screen, root, visual, colormap, width, height })
    }

    /// An override-redirect (unmanaged: never focused, decorated or listed)
    /// transparent window.
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
        self.set_window_type(win, b"_NET_WM_WINDOW_TYPE_NOTIFICATION")?;
        Ok(win)
    }

    fn set_window_type(&self, win: Window, kind: &[u8]) -> Result<(), String> {
        let prop = self.conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE").map_err(err)?.reply().map_err(err)?.atom;
        let value = self.conn.intern_atom(false, kind).map_err(err)?.reply().map_err(err)?.atom;
        self.conn.change_property32(PropMode::REPLACE, win, prop, AtomEnum::ATOM, &[value]).map_err(err)?;
        Ok(())
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

/// Input region covering the whole `w`x`h` area except a square of radius
/// `r` around `(cx, cy)`.
pub fn rects_with_hole(w: i32, h: i32, cx: i32, cy: i32, r: i32) -> Vec<Rectangle> {
    let (x0, y0) = ((cx - r).clamp(0, w), (cy - r).clamp(0, h));
    let (x1, y1) = ((cx + r).clamp(0, w), (cy + r).clamp(0, h));
    let rect = |x: i32, y: i32, rw: i32, rh: i32| Rectangle { x: x as i16, y: y as i16, width: rw.max(0) as u16, height: rh.max(0) as u16 };
    [rect(0, 0, w, y0), rect(0, y1, w, h - y1), rect(0, y0, x0, y1 - y0), rect(x1, y0, w - x1, y1 - y0)]
        .into_iter()
        .filter(|r| r.width > 0 && r.height > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn covers(rects: &[Rectangle], x: i32, y: i32) -> bool {
        rects.iter().any(|r| x >= r.x as i32 && y >= r.y as i32 && x < r.x as i32 + r.width as i32 && y < r.y as i32 + r.height as i32)
    }

    #[test]
    fn hole_leaves_pointer_area_click_through() {
        let rects = rects_with_hole(1920, 1080, 500, 400, 40);
        assert!(!covers(&rects, 500, 400), "pointer hot spot passes clicks through");
        assert!(!covers(&rects, 539, 439));
        assert!(covers(&rects, 541, 400) && covers(&rects, 500, 441) && covers(&rects, 0, 0) && covers(&rects, 1919, 1079));
        let area: i64 = rects.iter().map(|r| r.width as i64 * r.height as i64).sum();
        assert_eq!(area, 1920 * 1080 - 80 * 80, "no overlaps, no gaps");
    }

    #[test]
    fn hole_is_clamped_at_screen_edges() {
        let rects = rects_with_hole(1920, 1080, 5, 5, 40);
        assert!(!covers(&rects, 0, 0));
        let area: i64 = rects.iter().map(|r| r.width as i64 * r.height as i64).sum();
        assert_eq!(area, 1920 * 1080 - 45 * 45);
    }
}
