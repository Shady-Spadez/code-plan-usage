use eframe::egui;
use egui::Color32;
use crate::theme::Theme;

/// Render the "通用" (general) settings tab contents.
///
/// Provides refresh interval, notification threshold, theme selection, and
/// auto-start toggle. Uses the `section` card-style layout common to both
/// Volcengine and Coconut settings panels.
pub fn render_common_general_tab(
    ui: &mut egui::Ui,
    refresh_interval_secs: &mut u64,
    notification_threshold: &mut f64,
    theme: &mut Theme,
    auto_start: &mut bool,
    card_bg: Color32,
    text_muted: Color32,
) {
    let section_gap = 12.0;

    let section = |ui: &mut egui::Ui, title: &str, body: &mut dyn FnMut(&mut egui::Ui)| {
        egui::Frame::NONE
            .fill(card_bg)
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(title)
                        .color(Color32::WHITE)
                        .font(egui::FontId::proportional(14.0))
                        .strong(),
                );
                ui.add_space(8.0);
                body(ui);
            });
        ui.add_space(section_gap);
    };

    // ── Refresh ──
    section(ui, "🔄 刷新", &mut |ui| {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("刷新间隔")
                    .color(text_muted)
                    .font(egui::FontId::proportional(12.0)),
            );
            let mut secs = *refresh_interval_secs;
            if secs < 30 {
                secs = 30;
            }
            if ui
                .add(
                    egui::DragValue::new(&mut secs)
                        .range(30..=3600)
                        .suffix(" 秒"),
                )
                .changed()
            {
                *refresh_interval_secs = secs;
            }
        });
    });

    // ── Notification ──
    section(ui, "🔔 通知", &mut |ui| {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("用量阈值")
                    .color(text_muted)
                    .font(egui::FontId::proportional(12.0)),
            );
            let mut threshold = *notification_threshold;
            if ui
                .add(
                    egui::DragValue::new(&mut threshold)
                        .range(0.0..=100.0)
                        .speed(1.0)
                        .suffix(" %"),
                )
                .changed()
            {
                *notification_threshold = threshold;
            }
        });
        if *notification_threshold <= 0.0 {
            ui.label(
                egui::RichText::new("设为 0 可禁用通知")
                    .color(text_muted)
                    .font(egui::FontId::proportional(11.0)),
            );
        }
    });

    // ── Appearance ──
    section(ui, "🎨 外观", &mut |ui| {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("主题")
                    .color(text_muted)
                    .font(egui::FontId::proportional(12.0)),
            );
            let is_dark = matches!(*theme, Theme::Dark);
            if ui.selectable_label(is_dark, "🌙 暗色").clicked() {
                *theme = Theme::Dark;
            }
            if ui.selectable_label(!is_dark, "☀️ 亮色").clicked() {
                *theme = Theme::Light;
            }
        });
    });

    // ── Other ──
    section(ui, "⚙ 其他", &mut |ui| {
        if ui.checkbox(auto_start, "开机自启").changed() {}
    });
}
