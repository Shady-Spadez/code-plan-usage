use eframe::egui;

/// Clamp a widget home position so the whole widget (size `ws`) stays on its
/// monitor and off the taskbar. Horizontally the widget may touch the screen
/// edges (uses the full monitor rect, not the work area, so it isn't kept away
/// from the left/right edges); vertically it's clamped to the work area to
/// avoid the taskbar. Falls back to the input position when screen info is
/// unavailable.
pub fn clamp_home_to_work_area(home: egui::Pos2, ws: egui::Vec2, ppp: f32) -> egui::Pos2 {
    match crate::screen::screen_info_for_point(home, ppp) {
        Some(info) => {
            let x = home
                .x
                .clamp(info.monitor.min.x, (info.monitor.max.x - ws.x).max(info.monitor.min.x));
            let y = home
                .y
                .clamp(info.work_area.min.y, (info.work_area.max.y - ws.y).max(info.work_area.min.y));
            egui::pos2(x, y)
        }
        None => home,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    #[test]
    fn test_clamp_home_invalid_ppp_returns_home() {
        let home = pos2(100.0, 200.0);
        let ws = vec2(60.0, 60.0);
        // ppp <= 0 forces screen_info_for_point to return None
        assert_eq!(clamp_home_to_work_area(home, ws, 0.0), home);
        assert_eq!(clamp_home_to_work_area(home, ws, -1.0), home);
    }

    #[test]
    fn test_clamp_home_nan_ppp_returns_home() {
        let home = pos2(100.0, 200.0);
        let ws = vec2(60.0, 60.0);
        assert_eq!(clamp_home_to_work_area(home, ws, f32::NAN), home);
    }

    #[test]
    fn test_clamp_home_negative_coords_returns_unchanged() {
        let home = pos2(-1000.0, -1000.0);
        let ws = vec2(60.0, 60.0);
        let result = clamp_home_to_work_area(home, ws, 0.0);
        assert_eq!(result, home);
    }

    #[test]
    fn test_clamp_home_zero_coords() {
        let home = pos2(0.0, 0.0);
        let ws = vec2(60.0, 60.0);
        let result = clamp_home_to_work_area(home, ws, 0.0);
        assert_eq!(result, home);
    }
}
