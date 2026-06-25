use eframe::egui;
use egui::epaint::PathShape;
use egui::{Color32, Pos2, Shape, Stroke};

/// Draw an animated arc (percentage ring) on the given painter.
///
/// Does NOT draw the background circle or logo/text — only the arc PathShape.
/// Uses `arc_points_cache` for allocation reuse.
pub fn draw_widget_circle(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    stroke_width: f32,
    percent: f64,
    color: Color32,
    arc_points_cache: &mut Vec<egui::Pos2>,
) {
    if percent <= 0.0 {
        return;
    }
    let start_angle = -std::f32::consts::FRAC_PI_2;
    let sweep = (percent as f32 / 100.0) * 2.0 * std::f32::consts::PI;
    let segments = 64;
    arc_points_cache.clear();
    let points: &[Pos2] = {
        arc_points_cache.extend((0..=segments).map(|i| {
            let angle = start_angle + sweep * i as f32 / segments as f32;
            center + radius * egui::vec2(angle.cos(), angle.sin())
        }));
        arc_points_cache
    };
    painter.add(Shape::Path(PathShape {
        points: points.to_vec(),
        closed: false,
        fill: Color32::TRANSPARENT,
        stroke: Stroke::new(stroke_width, color).into(),
    }));
}
