use eframe::egui;

/// Result of tooltip placement calculation.
pub struct TooltipPlacement {
    /// true = tooltip above widget, false = below.
    pub top: bool,
    /// true = tooltip's RIGHT edge aligns with widget's RIGHT edge (extends LEFT),
    /// false = tooltip's LEFT edge aligns with widget's LEFT edge (extends RIGHT).
    pub right: bool,
    /// The work area used for fitting, or None if unavailable.
    pub work_area: Option<egui::Rect>,
}

/// Compute optimal tooltip placement relative to the widget, with 4-corner
/// priority: bottom-right → bottom-left → top-left → top-right.
///
/// `home` is the widget's top-left screen position, `widget_size` the widget
/// dimensions, `tooltip_size` the tooltip dimensions, `ppp` pixels-per-point,
/// and `gap` the spacing between widget and tooltip.
pub fn compute_tooltip_placement(
    home: Option<egui::Pos2>,
    widget_size: egui::Vec2,
    tooltip_size: egui::Vec2,
    ppp: f32,
    gap: f32,
) -> TooltipPlacement {
    let (tooltip_w, tooltip_h) = (tooltip_size.x, tooltip_size.y);

    let wa: Option<egui::Rect> = home.and_then(|h| {
        #[cfg(windows)]
        {
            crate::screen::work_area_for_point(h, ppp)
        }
        #[cfg(not(windows))]
        {
            None
        }
    });

    let (top, right) = if let (Some(h), Some(area)) = (home, wa) {
        let tip_x_ofs = |right: bool| if right { widget_size.x - tooltip_w } else { 0.0 };
        let bottom_y = h.y + widget_size.y + gap;
        let top_y = h.y - gap - tooltip_h;
        let tip_rect = |right: bool, top: bool| {
            egui::Rect::from_min_size(
                egui::pos2(h.x + tip_x_ofs(right), if top { top_y } else { bottom_y }),
                egui::vec2(tooltip_w, tooltip_h),
            )
        };
        let fits = |r: egui::Rect| {
            r.min.x >= area.min.x - 1.0
                && r.min.y >= area.min.y - 1.0
                && r.max.x <= area.max.x + 1.0
                && r.max.y <= area.max.y + 1.0
        };
        if fits(tip_rect(true, false)) {
            (false, true)
        } else if fits(tip_rect(false, false)) {
            (false, false)
        } else if fits(tip_rect(false, true)) {
            (true, false)
        } else if fits(tip_rect(true, true)) {
            (true, true)
        } else {
            let right_room = (area.max.x - (h.x + widget_size.x)).max(0.0);
            let left_room = (h.x - area.min.x).max(0.0);
            (false, left_room >= right_room)
        }
    } else {
        (false, true)
    };

    TooltipPlacement {
        top,
        right,
        work_area: wa,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    #[test]
    fn test_placement_no_home_returns_default() {
        let placement = compute_tooltip_placement(
            None,
            vec2(60.0, 60.0),
            vec2(200.0, 100.0),
            1.0,
            4.0,
        );
        assert!(!placement.top);
        assert!(placement.right);
        assert_eq!(placement.work_area, None);
    }

    #[test]
    fn test_placement_home_no_work_area_returns_default() {
        // When work_area_for_point returns None (non-Windows or invalid ppp),
        // placement falls back to (false, true).
        let placement = compute_tooltip_placement(
            Some(pos2(100.0, 100.0)),
            vec2(60.0, 60.0),
            vec2(200.0, 100.0),
            0.0,
            4.0,
        );
        assert!(!placement.top);
        assert!(placement.right);
        assert_eq!(placement.work_area, None);
    }

    #[test]
    fn test_placement_negative_ppp() {
        let placement = compute_tooltip_placement(
            Some(pos2(100.0, 100.0)),
            vec2(60.0, 60.0),
            vec2(200.0, 100.0),
            -1.0,
            4.0,
        );
        assert!(!placement.top);
        assert!(placement.right);
        assert_eq!(placement.work_area, None);
    }

    #[test]
    fn test_placement_zero_size_tooltip() {
        let placement = compute_tooltip_placement(
            Some(pos2(100.0, 100.0)),
            vec2(60.0, 60.0),
            vec2(0.0, 0.0),
            0.0,
            4.0,
        );
        assert!(!placement.top);
        assert!(placement.right);
        assert_eq!(placement.work_area, None);
    }

    #[test]
    fn test_placement_zero_gap() {
        let placement = compute_tooltip_placement(
            Some(pos2(100.0, 100.0)),
            vec2(60.0, 60.0),
            vec2(200.0, 100.0),
            0.0,
            0.0,
        );
        assert!(!placement.top);
        assert!(placement.right);
    }
}
