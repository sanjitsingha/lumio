//! Where to put the indicator relative to the pointer, in physical pixels.

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

    #[test]
    fn scales_offsets_for_hidpi_and_respects_monitor_origin() {
        let second = Rect { x: 1920, y: -200, width: 2560, height: 1440 };
        let p = place_body((3000, 0), 80, second, IndicatorPosition::Near, 2.0);
        assert_eq!(p, (3032, 40));
        assert!(!overlaps_pointer(p, 80, (3000, 0), 2.0));
    }
}
