//! Panel geometry for the side-panel framework. `panel_geometry` returns the
//! BODY rectangle at full open — right-anchored against the window's right edge.
//! Its edges and `center` are the hook the #144 feather-puff animation consumes
//! to know where to burst feathers from.

use iced::Point;

/// The panel body's rectangle at full open, plus its center point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub center: Point,
}

/// Geometry of a right-anchored body `panel_w` wide in a `(win_w, win_h)` window.
/// Clamps degenerate inputs: width is never negative and never exceeds the
/// window; the center is always finite.
pub fn panel_geometry(window: (f32, f32), panel_w: f32) -> PanelRect {
    let win_w = window.0.max(0.0);
    let win_h = window.1.max(0.0);
    let w = panel_w.clamp(0.0, win_w);
    let x = win_w - w;
    PanelRect {
        x,
        y: 0.0,
        w,
        h: win_h,
        center: Point::new(x + w / 2.0, win_h / 2.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_window_right_anchors_body() {
        let r = panel_geometry((1280.0, 800.0), 400.0);
        assert_eq!(r.x, 880.0);
        assert_eq!(r.y, 0.0);
        assert_eq!(r.w, 400.0);
        assert_eq!(r.h, 800.0);
        assert_eq!(r.center, Point::new(1080.0, 400.0));
    }

    #[test]
    fn panel_wider_than_window_clamps() {
        let r = panel_geometry((300.0, 600.0), 400.0);
        assert_eq!(r.w, 300.0);
        assert_eq!(r.x, 0.0);
        assert_eq!(r.center, Point::new(150.0, 300.0));
    }

    #[test]
    fn degenerate_window_is_finite() {
        let r = panel_geometry((0.0, 0.0), 400.0);
        assert_eq!(r.w, 0.0);
        assert_eq!(r.x, 0.0);
        assert!(r.center.x.is_finite() && r.center.y.is_finite());
    }

    #[test]
    fn negative_window_is_guarded() {
        let r = panel_geometry((-50.0, -50.0), 400.0);
        assert_eq!(r.w, 0.0);
        assert_eq!(r.x, 0.0);
        assert!(r.center.x.is_finite() && r.center.y.is_finite());
    }
}
