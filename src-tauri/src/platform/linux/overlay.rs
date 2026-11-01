//! Caps Lock indicator window on Linux: an X11 override-redirect window with
//! a 32-bit visual (per-pixel alpha) and an empty input region. It works on
//! X11 sessions and, through XWayland, on Wayland sessions: the window
//! manager never focuses, decorates or lists it, it stays above normal
//! windows, and every click passes through it.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ConfigureWindowAux, ConnectionExt as _, CreateGCAux, EventMask, Gcontext, ImageFormat, StackMode, Window,
};

use super::x11util::{err, ArgbContext};
use super::{gala, session};
use crate::capslock::render::Frame;

/// Stay under the X11 maximum request size (without BIG-REQUESTS).
const MAX_PUT_IMAGE_BYTES: usize = 200 * 1024;

pub fn support() -> Result<&'static str, String> {
    let ctx = ArgbContext::connect().map_err(|e| {
        if session::is_wayland() {
            format!("XWayland is not available ({e}).")
        } else {
            e
        }
    })?;
    if !ctx.compositing() {
        return Err("A compositing window manager is required for a transparent overlay.".into());
    }
    if !ctx.has_shape() {
        return Err("The X server lacks the SHAPE extension needed for a click-through overlay.".into());
    }
    Ok(if session::is_wayland() { "XWayland overlay window" } else { "X11 overlay window" })
}

/// Gala's on-screen display, used if the pointer cannot be located.
pub fn osd_available() -> bool {
    gala::session_bus().is_ok_and(|c| gala::gala_available(&c))
}

pub fn show_osd(on: bool) -> Result<(), String> {
    let conn = gala::session_bus()?;
    let icon = if on { "caps-lock-symbolic" } else { "input-keyboard-symbolic" };
    gala::show_osd(&conn, icon, if on { "Caps Lock On" } else { "Caps Lock Off" })
}

pub struct Overlay {
    ctx: ArgbContext,
    window: Window,
    gc: Gcontext,
    frame: Option<Frame>,
    size: (u16, u16),
    visible: bool,
}

impl Overlay {
    pub fn new() -> Result<Self, String> {
        let ctx = ArgbContext::connect()?;
        let window = ctx.create_window(0, 0, 1, 1, EventMask::NO_EVENT)?;
        ctx.set_input(window, &[])?;
        let gc = ctx.conn.generate_id().map_err(err)?;
        ctx.conn.create_gc(gc, window, &CreateGCAux::new()).map_err(err)?;
        ctx.conn.flush().map_err(err)?;
        Ok(Self { ctx, window, gc, frame: None, size: (0, 0), visible: false })
    }

    /// Shows the indicator with its canvas top-left at `(x, y)` (X root pixels).
    /// `frame: None` reuses the previous artwork (moves and fades).
    pub fn present(&mut self, frame: Option<&Frame>, x: i32, y: i32, opacity: f32) {
        if let Some(f) = frame {
            self.frame = Some(f.clone());
        }
        let Some(f) = self.frame.as_ref() else { return };
        let (w, h) = (f.width as u16, f.height as u16);
        let conn = &self.ctx.conn;
        let mut aux = ConfigureWindowAux::new().x(x).y(y).stack_mode(StackMode::ABOVE);
        if (w, h) != self.size {
            aux = aux.width(w as u32).height(h as u32);
            self.size = (w, h);
        }
        let _ = conn.configure_window(self.window, &aux);
        if !self.visible {
            let _ = conn.map_window(self.window);
            self.visible = true;
        }
        self.draw(opacity);
        let _ = conn.flush();
    }

    /// Uploads the frame as premultiplied BGRA with `opacity` applied, in
    /// row bands that fit into a single X request.
    fn draw(&self, opacity: f32) {
        let Some(f) = self.frame.as_ref() else { return };
        let a = (opacity.clamp(0.0, 1.0) * 256.0) as u32;
        let mut pixels = Vec::with_capacity(f.rgba.len());
        for p in f.rgba.chunks_exact(4) {
            let s = |c: u8| ((c as u32 * a) >> 8) as u8;
            pixels.extend_from_slice(&[s(p[2]), s(p[1]), s(p[0]), s(p[3])]);
        }
        let row = f.width as usize * 4;
        let rows_per_band = (MAX_PUT_IMAGE_BYTES / row).max(1);
        for (band, chunk) in pixels.chunks(rows_per_band * row).enumerate() {
            let rows = (chunk.len() / row) as u16;
            let y = (band * rows_per_band) as i16;
            if let Err(e) =
                self.ctx.conn.put_image(ImageFormat::Z_PIXMAP, self.window, self.gc, f.width as u16, rows, 0, y, 0, 32, chunk)
            {
                log::warn!("could not draw the Caps Lock indicator: {e}");
                return;
            }
        }
    }

    pub fn hide(&mut self) {
        if self.visible {
            let _ = self.ctx.conn.unmap_window(self.window);
            let _ = self.ctx.conn.flush();
            self.visible = false;
        }
    }
}
