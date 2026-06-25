#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "coconut/api.rs"]
mod api;
#[path = "coconut/settings.rs"]
mod settings;
#[cfg(windows)]
#[path = "coconut/webview_login.rs"]
mod webview_login;
#[path = "coconut/widget.rs"]
mod widget;

use eframe::egui;

use coding_plan_widget_shared::{
    apply_auto_start, debug_log, setup_cjk_font,
    theme, tray,
};

use crate::settings::CoconutSettings;
use crate::widget::CoconutApp;

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() -> eframe::Result {
    coding_plan_widget_shared::log::init_logger();
    debug_log!("=== Coconut Plan Widget starting ===");
    #[cfg(windows)]
    tray::init_tray("Coconut Plan Widget", "Coconut Plan Widget");

    let settings = CoconutSettings::load();
    debug_log!("Coconut settings loaded: configured={}", settings.is_configured());

    #[cfg(windows)]
    apply_auto_start("CoconutPlanWidget", settings.auto_start);

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
        "Coconut Plan Widget",
        options,
        Box::new(move |cc| {
            setup_cjk_font(&cc.egui_ctx);
            Ok(Box::new(CoconutApp::with_settings(settings)))
        }),
    )
}
