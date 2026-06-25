#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod settings;
#[cfg(windows)]
mod webview_login;
mod widget;

use eframe::egui;

use coding_plan_widget_shared::{
    debug_log, theme,
    apply_auto_start, setup_cjk_font,
    log, tray,
};

use crate::settings::Settings;
use crate::widget::WidgetApp;

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() -> eframe::Result {
    log::init_logger();
    debug_log!("=== Coding Plan Widget starting ===");
    #[cfg(windows)]
    tray::init_tray("Coding Plan Widget", "Coding Plan Widget");

    let settings = Settings::load();
    debug_log!("Settings loaded: configured={}", settings.is_configured());

    #[cfg(windows)]
    apply_auto_start("CodingPlanWidget", settings.auto_start);

    let initial_size = theme::widget_window_size();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(initial_size)
        .with_decorations(false)
        .with_always_on_top()
        .with_transparent(true)
        .with_resizable(false)
        .with_taskbar(false);

    if let (Some(x), Some(y)) = (settings.window_x, settings.window_y) {
        viewport = viewport.with_position(egui::Pos2::new(x, y));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Coding Plan Widget",
        options,
        Box::new(move |cc| {
            setup_cjk_font(&cc.egui_ctx);
            Ok(Box::new(WidgetApp::with_settings(settings)))
        }),
    )
}
