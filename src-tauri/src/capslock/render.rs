//! Draws the Caps Lock indicator into a premultiplied RGBA bitmap with
//! tiny-skia, so both platforms show pixel-identical artwork at any scale.

use tiny_skia::{Color, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke, Transform};

/// A rendered indicator. `rgba` is premultiplied, row-major, `width * height * 4`.
#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Transparent shadow margin around the body, in physical pixels.
    pub margin: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Body edge length in logical pixels.
    pub size: u32,
    pub scale: f64,
    pub dark: bool,
    /// Corner radius as a fraction of the edge length (0.5 = a circle).
    pub radius: f32,
}

/// Shadow margin as a fraction of the body size.
const MARGIN: f32 = 0.30;

pub fn margin_px(size: u32, scale: f64) -> u32 {
    (size as f32 * MARGIN * scale as f32).ceil() as u32
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

fn paint(r: u8, g: u8, b: u8, a: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba8(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8));
    p.anti_alias = true;
    p
}

/// The ⇪ glyph: an upward arrow with a stem, in unit coordinates of the body.
fn arrow_path(x: f32, y: f32, s: f32) -> Option<Path> {
    let p = |u: f32, v: f32| (x + u * s, y + v * s);
    let mut pb = PathBuilder::new();
    let pts = [p(0.5, 0.17), p(0.79, 0.47), p(0.625, 0.47), p(0.625, 0.62), p(0.375, 0.62), p(0.375, 0.47), p(0.21, 0.47)];
    pb.move_to(pts[0].0, pts[0].1);
    for (px, py) in &pts[1..] {
        pb.line_to(*px, *py);
    }
    pb.close();
    pb.finish()
}

/// Renders the indicator. `pop` is the body's scale around its center for
/// the appear spring: 1.0 is full size, a little more while it overshoots
/// (the shadow margin leaves room for that).
pub fn render(style: Style, pop: f32) -> Option<Frame> {
    let scale = style.scale.max(1.0) as f32;
    let body = style.size as f32 * scale;
    let margin = margin_px(style.size, style.scale) as f32;
    let canvas = (body + 2.0 * margin).ceil() as u32;
    let mut pixmap = Pixmap::new(canvas, canvas)?;

    let s = body * pop.clamp(0.5, 1.1);
    let x = margin + (body - s) / 2.0;
    let y = margin + (body - s) / 2.0;
    let radius = s * style.radius.clamp(0.0, 0.5);

    // soft shadow: stacked, expanding translucent rounded rects
    let layers = 14;
    for i in (0..layers).rev() {
        let spread = (i as f32 + 1.0) / layers as f32 * margin * 0.8;
        let falloff = 1.0 - i as f32 / layers as f32;
        let alpha = if style.dark { 0.05 } else { 0.026 } * falloff * falloff;
        if let Some(path) = rounded_rect(x - spread, y - spread + s * 0.05, s + spread * 2.0, s + spread * 2.0, radius + spread) {
            pixmap.fill_path(&path, &paint(0, 0, 0, alpha), FillRule::Winding, Transform::identity(), None);
        }
    }

    let (fill, border, glyph) = if style.dark {
        ((40, 40, 44, 0.96), (255, 255, 255, 0.14), (245, 245, 247))
    } else {
        ((252, 252, 253, 0.97), (0, 0, 0, 0.09), (29, 29, 31))
    };
    let body_path = rounded_rect(x, y, s, s, radius)?;
    pixmap.fill_path(&body_path, &paint(fill.0, fill.1, fill.2, fill.3), FillRule::Winding, Transform::identity(), None);
    let hairline = Stroke { width: (scale * 1.0).max(1.0), ..Stroke::default() };
    pixmap.stroke_path(&body_path, &paint(border.0, border.1, border.2, border.3), &hairline, Transform::identity(), None);

    let arrow = arrow_path(x, y, s)?;
    let bar = rounded_rect(x + 0.35 * s, y + 0.705 * s, 0.30 * s, 0.075 * s, 0.03 * s)?;
    let glyph_paint = paint(glyph.0, glyph.1, glyph.2, 1.0);
    pixmap.fill_path(&arrow, &glyph_paint, FillRule::Winding, Transform::identity(), None);
    let round = Stroke { width: 0.035 * s, line_join: LineJoin::Round, line_cap: LineCap::Round, ..Stroke::default() };
    pixmap.stroke_path(&arrow, &glyph_paint, &round, Transform::identity(), None);
    // the "LED": green like a lit Caps Lock key
    let led = if style.dark { (48, 209, 88) } else { (40, 190, 80) };
    pixmap.fill_path(&bar, &paint(led.0, led.1, led.2, 1.0), FillRule::Winding, Transform::identity(), None);

    Some(Frame { width: canvas, height: canvas, rgba: pixmap.take(), margin: margin as u32 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(f: &Frame, x: u32, y: u32) -> u8 {
        f.rgba[((y * f.width + x) * 4 + 3) as usize]
    }

    #[test]
    fn frame_dimensions_follow_size_and_scale() {
        let f1 = render(Style { size: 40, scale: 1.0, dark: false, radius: 0.27 }, 1.0).unwrap();
        let f2 = render(Style { size: 40, scale: 2.0, dark: false, radius: 0.27 }, 1.0).unwrap();
        assert_eq!(f1.width, 40 + 2 * margin_px(40, 1.0));
        assert_eq!(f2.width, 80 + 2 * margin_px(40, 2.0));
        assert_eq!(f1.rgba.len(), (f1.width * f1.height * 4) as usize);
    }

    #[test]
    fn corners_are_transparent_and_body_is_opaque() {
        let f = render(Style { size: 48, scale: 1.0, dark: true, radius: 0.27 }, 1.0).unwrap();
        assert_eq!(alpha_at(&f, 0, 0), 0, "canvas corner");
        let c = f.width / 2;
        assert!(alpha_at(&f, c, f.margin + 3) > 200, "body near top edge");
    }

    #[test]
    fn data_is_premultiplied() {
        let f = render(Style { size: 40, scale: 1.0, dark: false, radius: 0.27 }, 0.5).unwrap();
        assert!(f.rgba.chunks(4).all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]));
    }

    /// Writes PNG previews for manual inspection:
    /// `LUMMO_RENDER_PREVIEW=/tmp cargo test render_previews -- --ignored`
    #[test]
    #[ignore]
    fn render_previews() {
        let Some(dir) = std::env::var_os("LUMMO_RENDER_PREVIEW") else { return };
        for dark in [false, true] {
            let f = render(Style { size: 64, scale: 2.0, dark, radius: 0.27 }, 1.0).unwrap();
            let pixmap = Pixmap::from_vec(f.rgba, tiny_skia::IntSize::from_wh(f.width, f.height).unwrap()).unwrap();
            let name = format!("capslock-{}.png", if dark { "dark" } else { "light" });
            pixmap.save_png(std::path::Path::new(&dir).join(name)).unwrap();
        }
    }
}
