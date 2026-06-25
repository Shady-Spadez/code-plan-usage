use eframe::egui;

/// Encapsulates drag-related state and logic shared by all widget binaries.
///
/// The owning `WidgetApp` / `CoconutApp` embeds this struct and delegates
/// drag-engage, drag-disengage, and mouse-screen computation to it.
pub struct DragState {
    pub is_dragging: bool,
    /// Offset from widget home to mouse at drag start.
    pub anchor: Option<egui::Vec2>,
    /// Mouse position captured when the primary button was first pressed.
    pub press_pos: Option<egui::Pos2>,
    /// Last known mouse position in screen coordinates, carried across frames.
    pub last_mouse_screen: Option<egui::Pos2>,
}

impl Default for DragState {
    fn default() -> Self {
        Self::new()
    }
}

impl DragState {
    pub fn new() -> Self {
        Self {
            is_dragging: false,
            anchor: None,
            press_pos: None,
            last_mouse_screen: None,
        }
    }

    /// Compute the current mouse screen position, handling the case where the
    /// window was moved without a real PointerMoved event.
    pub fn compute_mouse_screen(
        &mut self,
        cur_pos: Option<egui::Pos2>,
        pointer_pos: Option<egui::Pos2>,
        has_pointer_event: bool,
        has_pointer: bool,
    ) -> Option<egui::Pos2> {
        if !has_pointer {
            self.last_mouse_screen = None;
            None
        } else {
            match (cur_pos, pointer_pos) {
                (Some(c), Some(pp)) if has_pointer_event => {
                    let computed = c + pp.to_vec2();
                    self.last_mouse_screen = Some(computed);
                    Some(computed)
                }
                (Some(c), Some(pp)) => {
                    Some(self.last_mouse_screen.unwrap_or_else(|| c + pp.to_vec2()))
                }
                _ => self.last_mouse_screen,
            }
        }
    }

    /// Returns true if the mouse has moved beyond `threshold` px from the press
    /// origin, or if dragging is already engaged.
    pub fn should_drag(&self, ms: egui::Pos2, threshold: f32) -> bool {
        self.is_dragging
            || self
                .press_pos
                .is_some_and(|p| p.distance(ms) > threshold)
    }

    /// Engage dragging: set `is_dragging` and capture the anchor offset.
    pub fn engage(&mut self, ms: egui::Pos2, home: egui::Pos2) {
        self.is_dragging = true;
        self.anchor = Some(ms - home);
    }

    /// Disengage dragging and clear press tracking.
    /// Returns true if we were actually dragging before disengaging.
    pub fn disengage(&mut self) -> bool {
        let was = self.is_dragging;
        self.is_dragging = false;
        self.anchor = None;
        self.press_pos = None;
        was
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    // ── should_drag ────────────────────────────────────────────────────────

    #[test]
    fn test_should_drag_already_dragging() {
        let mut state = DragState::new();
        state.is_dragging = true;
        assert!(state.should_drag(pos2(0.0, 0.0), 4.0));
    }

    #[test]
    fn test_should_drag_threshold_exceeded() {
        let mut state = DragState::new();
        state.press_pos = Some(pos2(0.0, 0.0));
        assert!(state.should_drag(pos2(5.0, 0.0), 4.0));
    }

    #[test]
    fn test_should_drag_threshold_not_exceeded() {
        let mut state = DragState::new();
        state.press_pos = Some(pos2(0.0, 0.0));
        assert!(!state.should_drag(pos2(3.9, 0.0), 4.0));
    }

    #[test]
    fn test_should_drag_exact_threshold() {
        let mut state = DragState::new();
        state.press_pos = Some(pos2(0.0, 0.0));
        // distance == threshold -> not exceeded (strict > check)
        assert!(!state.should_drag(pos2(4.0, 0.0), 4.0));
    }

    #[test]
    fn test_should_drag_no_press_pos() {
        let state = DragState::new();
        assert!(!state.should_drag(pos2(100.0, 0.0), 4.0));
    }

    #[test]
    fn test_should_drag_diagonal_distance() {
        let mut state = DragState::new();
        state.press_pos = Some(pos2(0.0, 0.0));
        // 3-4-5 triangle: distance = 5.0
        assert!(state.should_drag(pos2(3.0, 4.0), 4.0));
    }

    // ── compute_mouse_screen ───────────────────────────────────────────────

    #[test]
    fn test_compute_mouse_screen_no_pointer() {
        let mut state = DragState::new();
        state.last_mouse_screen = Some(pos2(10.0, 20.0));
        let result = state.compute_mouse_screen(
            Some(pos2(100.0, 100.0)),
            Some(pos2(5.0, 5.0)),
            true,
            false,
        );
        assert_eq!(result, None);
        assert_eq!(state.last_mouse_screen, None);
    }

    #[test]
    fn test_compute_mouse_screen_with_pointer_event() {
        let mut state = DragState::new();
        let result = state.compute_mouse_screen(
            Some(pos2(100.0, 200.0)),
            Some(pos2(30.0, 40.0)),
            true,
            true,
        );
        assert_eq!(result, Some(pos2(130.0, 240.0)));
        assert_eq!(state.last_mouse_screen, Some(pos2(130.0, 240.0)));
    }

    #[test]
    fn test_compute_mouse_screen_stale_pointer_uses_cached() {
        let mut state = DragState::new();
        state.last_mouse_screen = Some(pos2(99.0, 88.0));
        let result = state.compute_mouse_screen(
            Some(pos2(0.0, 0.0)),
            Some(pos2(1.0, 1.0)),
            false,
            true,
        );
        assert_eq!(result, Some(pos2(99.0, 88.0)));
    }

    #[test]
    fn test_compute_mouse_screen_stale_pointer_falls_back() {
        let mut state = DragState::new();
        let result = state.compute_mouse_screen(
            Some(pos2(100.0, 200.0)),
            Some(pos2(30.0, 40.0)),
            false,
            true,
        );
        assert_eq!(result, Some(pos2(130.0, 240.0)));
    }

    #[test]
    fn test_compute_mouse_screen_no_positions_uses_cached() {
        let mut state = DragState::new();
        state.last_mouse_screen = Some(pos2(77.0, 66.0));
        let result = state.compute_mouse_screen(None, None, true, true);
        assert_eq!(result, Some(pos2(77.0, 66.0)));
    }

    #[test]
    fn test_compute_mouse_screen_no_positions_no_cache() {
        let mut state = DragState::new();
        let result = state.compute_mouse_screen(None, None, true, true);
        assert_eq!(result, None);
    }

    // ── engage / disengage ─────────────────────────────────────────────────

    #[test]
    fn test_engage_sets_state() {
        let mut state = DragState::new();
        state.engage(pos2(50.0, 60.0), pos2(10.0, 20.0));
        assert!(state.is_dragging);
        assert_eq!(state.anchor, Some(egui::vec2(40.0, 40.0)));
    }

    #[test]
    fn test_disengage_returns_false_when_not_dragging() {
        let mut state = DragState::new();
        state.press_pos = Some(pos2(5.0, 5.0));
        let was = state.disengage();
        assert!(!was);
        assert!(!state.is_dragging);
        assert_eq!(state.anchor, None);
        assert_eq!(state.press_pos, None);
    }

    #[test]
    fn test_disengage_returns_true_when_dragging() {
        let mut state = DragState::new();
        state.is_dragging = true;
        state.anchor = Some(egui::vec2(10.0, 10.0));
        state.press_pos = Some(pos2(5.0, 5.0));
        let was = state.disengage();
        assert!(was);
        assert!(!state.is_dragging);
        assert_eq!(state.anchor, None);
        assert_eq!(state.press_pos, None);
    }

    // ── Default ────────────────────────────────────────────────────────────

    #[test]
    fn test_default_matches_new() {
        let d = DragState::default();
        let n = DragState::new();
        assert!(!d.is_dragging);
        assert_eq!(d.is_dragging, n.is_dragging);
        assert_eq!(d.anchor, n.anchor);
        assert_eq!(d.press_pos, n.press_pos);
        assert_eq!(d.last_mouse_screen, n.last_mouse_screen);
    }
}
