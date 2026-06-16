use eframe::egui;
use egui::Color32;
use serde::{Deserialize, Serialize};

// ── Theme ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

pub struct ThemeColors {
    pub bg_fill: Color32,
    pub circle_bg: Color32,
    pub widget_fg: Color32,
}

impl Theme {
    pub fn colors(&self) -> ThemeColors {
        match self {
            Theme::Dark => ThemeColors {
                bg_fill: Color32::from_rgba_premultiplied(30, 30, 35, 240),
                circle_bg: Color32::from_gray(50),
                widget_fg: Color32::from_gray(220),
            },
            Theme::Light => ThemeColors {
                bg_fill: Color32::from_rgba_premultiplied(245, 245, 250, 240),
                circle_bg: Color32::from_gray(210),
                widget_fg: Color32::from_gray(30),
            },
        }
    }
}

// ── Widget Size ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
pub enum WidgetSize {
    Small,
    #[default]
    Medium,
    Large,
}

pub struct SizeConfig {
    pub dimensions: egui::Vec2,
    pub circle_radius: f32,
    pub circle_center_dot: f32,
    pub stroke_width: f32,
    pub percent_font_size: f32,
    pub error_font_size: f32,
}

impl WidgetSize {
    pub fn config(&self) -> SizeConfig {
        match self {
            WidgetSize::Small => SizeConfig {
                dimensions: egui::vec2(240.0, 36.0),
                circle_radius: 9.0,
                circle_center_dot: 2.0,
                stroke_width: 2.0,
                percent_font_size: 11.0,
                error_font_size: 8.0,
            },
            WidgetSize::Medium => SizeConfig {
                dimensions: egui::vec2(240.0, 48.0),
                circle_radius: 12.0,
                circle_center_dot: 3.0,
                stroke_width: 2.5,
                percent_font_size: 14.0,
                error_font_size: 10.0,
            },
            WidgetSize::Large => SizeConfig {
                dimensions: egui::vec2(240.0, 60.0),
                circle_radius: 15.0,
                circle_center_dot: 4.0,
                stroke_width: 3.0,
                percent_font_size: 17.0,
                error_font_size: 12.0,
            },
        }
    }

    pub fn window_size(&self) -> egui::Vec2 {
        self.config().dimensions
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

pub fn percent_color(percent: f64) -> Color32 {
    if percent < 50.0 {
        Color32::from_rgb(76, 175, 80)
    } else if percent < 80.0 {
        Color32::from_rgb(255, 193, 7)
    } else if percent < 95.0 {
        Color32::from_rgb(255, 152, 0)
    } else {
        Color32::from_rgb(244, 67, 54)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Color32;

    #[test]
    fn test_percent_color_green() {
        assert_eq!(percent_color(0.0), Color32::from_rgb(76, 175, 80));
        assert_eq!(percent_color(49.9), Color32::from_rgb(76, 175, 80));
    }

    #[test]
    fn test_percent_color_yellow() {
        assert_eq!(percent_color(50.0), Color32::from_rgb(255, 193, 7));
        assert_eq!(percent_color(79.9), Color32::from_rgb(255, 193, 7));
    }

    #[test]
    fn test_percent_color_orange() {
        assert_eq!(percent_color(80.0), Color32::from_rgb(255, 152, 0));
        assert_eq!(percent_color(94.9), Color32::from_rgb(255, 152, 0));
    }

    #[test]
    fn test_percent_color_red() {
        assert_eq!(percent_color(95.0), Color32::from_rgb(244, 67, 54));
        assert_eq!(percent_color(100.0), Color32::from_rgb(244, 67, 54));
    }
}
