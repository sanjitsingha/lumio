//! Where to put the indicator relative to the pointer or the text cursor, in
//! physical pixels.

use crate::storage::settings::IndicatorPosition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
    fn right(&self) -> i32 {
        self.x + self.width
    }
    fn bottom(&self) -> i32 {
        self.y + self.height
    }
}

/// Returns the top-left of the indicator *body* so that it never covers the
/// pointer's hot spot and arrow (roughly 12x20 logical px below-right of it),
/// flipping sides near screen edges and staying inside `work_area`.
pub fn place_body(cursor: (i32, i32), body: i32, work_area: Rect, position: IndicatorPosition, scale: f64) -> (i32, i32) {
    let s = |v: f64| (v * scale).round() as i32;
    let (cx, cy) = cursor;
    let (mut x, mut y) = match position {
        IndicatorPosition::Near => (cx + s(16.0), cy + s(20.0)),
        IndicatorPosition::Below => (cx - body / 2, cy + s(28.0)),
        IndicatorPosition::Above => (cx - body / 2, cy - s(12.0) - body),
    };
    match position {
        IndicatorPosition::Near => {
            if x + body > work_area.right() {
                x = cx - s(12.0) - body;
            }
            if y + body > work_area.bottom() {
                y = cy - s(10.0) - body;
            }
        }
        IndicatorPosition::Below => {
            if y + body > work_area.bottom() {
                y = cy - s(12.0) - body;
            }
        }
        IndicatorPosition::Above => {
            if y < work_area.y {
                y = cy + s(28.0);
            }
        }
    }
    x = x.clamp(work_area.x, (work_area.right() - body).max(work_area.x));
    y = y.clamp(work_area.y, (work_area.bottom() - body).max(work_area.y));
    (x, y)
}

/// Returns the top-left of the indicator body next to a text cursor
/// (`caret`: its rectangle, typically 1-2 px wide and one line high). The
/// indicator never covers the caret's line: it sits below it (like macOS),
/// or above it when there is no room below.
pub fn place_at_caret(caret: Rect, body: i32, work_area: Rect, position: IndicatorPosition, scale: f64) -> (i32, i32) {
    let s = |v: f64| (v * scale).round() as i32;
    let gap = s(6.0);
    let below = caret.bottom() + gap;
    let above = caret.y - gap - body;
    let x = match position {
        // just right of where the caret is, so it points at it
        IndicatorPosition::Near => caret.x - s(4.0),
        IndicatorPosition::Below | IndicatorPosition::Above => caret.x - body / 2,
    };
    let y = match position {
        IndicatorPosition::Above if above >= work_area.y => above,
        IndicatorPosition::Above => below,
        _ if below + body <= work_area.bottom() => below,
        _ => above,
    };
    let x = x.clamp(work_area.x, (work_area.right() - body).max(work_area.x));
    let y = y.clamp(work_area.y, (work_area.bottom() - body).max(work_area.y));
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect { x: 0, y: 0, width: 1920, height: 1080 };

    fn overlaps_pointer(body_pos: (i32, i32), body: i32, cursor: (i32, i32), scale: f64) -> bool {
        // pointer arrow footprint
        let arrow = Rect { x: cursor.0, y: cursor.1, width: (12.0 * scale) as i32, height: (20.0 * scale) as i32 };
        let b = Rect { x: body_pos.0, y: body_pos.1, width: body, height: body };
        b.x < arrow.right() && arrow.x < b.right() && b.y < arrow.bottom() && arrow.y < b.bottom()
    }

    #[test]
    fn near_sits_below_right_of_pointer() {
        let pos = place_body((500, 400), 40, SCREEN, IndicatorPosition::Near, 1.0);
        assert_eq!(pos, (516, 420));
        assert!(!overlaps_pointer(pos, 40, (500, 400), 1.0));
    }

    #[test]
    fn below_and_above_are_centered() {
        assert_eq!(place_body((500, 400), 40, SCREEN, IndicatorPosition::Below, 1.0), (480, 428));
        assert_eq!(place_body((500, 400), 40, SCREEN, IndicatorPosition::Above, 1.0), (480, 348));
    }

    #[test]
    fn flips_at_screen_edges_without_covering_the_pointer() {
        for cursor in [(1915, 500), (500, 1075), (1915, 1075), (2, 2), (960, 3)] {
            for pos in [IndicatorPosition::Near, IndicatorPosition::Below, IndicatorPosition::Above] {
                let p = place_body(cursor, 40, SCREEN, pos, 1.0);
                assert!(SCREEN.contains(p.0, p.1) && SCREEN.contains(p.0 + 39, p.1 + 39), "{cursor:?} {pos:?} -> {p:?}");
                if pos == IndicatorPosition::Near {
                    assert!(!overlaps_pointer(p, 40, cursor, 1.0), "{cursor:?} -> {p:?}");
                }
            }
        }
    }

    fn caret(x: i32, y: i32) -> Rect {
        Rect { x, y, width: 1, height: 18 }
    }

    fn covers_line(body_pos: (i32, i32), body: i32, caret: Rect) -> bool {
        body_pos.1 < caret.bottom() && caret.y < body_pos.1 + body
    }

    #[test]
    fn caret_indicator_sits_below_the_line() {
        let c = caret(500, 400);
        assert_eq!(place_at_caret(c, 40, SCREEN, IndicatorPosition::Near, 1.0), (496, 424));
        assert_eq!(place_at_caret(c, 40, SCREEN, IndicatorPosition::Below, 1.0), (480, 424));
        assert_eq!(place_at_caret(c, 40, SCREEN, IndicatorPosition::Above, 1.0), (480, 354));
    }

    #[test]
    fn caret_indicator_flips_instead_of_covering_the_line() {
        for c in [caret(500, 1060), caret(1919, 1050), caret(0, 0), caret(960, 2), caret(5, 500)] {
            for pos in [IndicatorPosition::Near, IndicatorPosition::Below, IndicatorPosition::Above] {
                let p = place_at_caret(c, 40, SCREEN, pos, 1.0);
                assert!(SCREEN.contains(p.0, p.1) && SCREEN.contains(p.0 + 39, p.1 + 39), "{c:?} {pos:?} -> {p:?}");
                assert!(!covers_line(p, 40, c), "{c:?} {pos:?} -> {p:?}");
            }
        }
    }

    #[test]
    fn caret_gap_scales_with_the_monitor() {
        let second = Rect { x: 1920, y: 0, width: 2560, height: 1440 };
        let c = Rect { x: 2400, y: 600, width: 2, height: 36 };
        assert_eq!(place_at_caret(c, 80, second, IndicatorPosition::Near, 2.0), (2392, 648));
    }

    #[test]
    fn scales_offsets_for_hidpi_and_respects_monitor_origin() {
        let second = Rect { x: 1920, y: -200, width: 2560, height: 1440 };
        let p = place_body((3000, 0), 80, second, IndicatorPosition::Near, 2.0);
        assert_eq!(p, (3032, 40));
        assert!(!overlaps_pointer(p, 80, (3000, 0), 2.0));
    }
}
