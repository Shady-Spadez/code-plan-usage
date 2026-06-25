use eframe::egui;
use egui::{Color32, Stroke};
use std::time::{Duration, Instant};

#[cfg(windows)]
use eframe::glow::HasContext as _;

use coding_plan_widget_shared::{
    debug_log,
    DEFAULT_REFRESH_INTERVAL, HOVER_COOLDOWN, RESET_REFRESH_COOLDOWN,
    show_usage_notification, apply_auto_start,
};
use coding_plan_widget_shared::theme::{percent_color, widget_config, widget_window_size};
use coding_plan_widget_shared::widgets::{
    DragState, clamp_home_to_work_area, draw_widget_circle,
    compute_tooltip_placement,
    query_framebuffer_alpha, render_common_general_tab,
};

#[cfg(windows)]
use coding_plan_widget_shared::tray;

#[cfg(windows)]
use crate::webview_login;

use crate::api::{fetch_usage, DailyActivity};
use crate::settings::CoconutSettings;

// ── Constants ────────────────────────────────────────────────────────────────

const DRAG_THRESHOLD: f32 = 4.0;

// ── Tooltip row ──────────────────────────────────────────────────────────────

pub(crate) struct TooltipRow {
    label: String,
    value: String,
    color: Color32,
}

// ── Coconut State ────────────────────────────────────────────────────────────

#[cfg(windows)]
pub struct SettingsViewportState {
    pub settings: std::rc::Rc<std::cell::RefCell<CoconutSettings>>,
    pub notification_sent: std::rc::Rc<std::cell::Cell<bool>>,
    pub tab_index: std::rc::Rc<std::cell::Cell<usize>>,
    pub original_settings: std::rc::Rc<std::cell::RefCell<CoconutSettings>>,
    pub confirm_dialog: std::rc::Rc<std::cell::Cell<bool>>,
    pub webview_receiver: std::rc::Rc<std::cell::RefCell<Option<std::sync::mpsc::Receiver<Option<String>>>>>,
}

pub struct CoconutApp {
    pub settings: CoconutSettings,
    pub activity: Option<DailyActivity>,
    pub error: Option<String>,
    pub last_refresh: Instant,
    pub was_hovered: bool,
    pub drag: DragState,
    pub show_settings: bool,
    pub notification_sent: bool,
    pub animated_percent: f64,
    #[cfg(windows)]
    pub color_key_applied: bool,
    pub refresh_in_progress: bool,
    pub pending_result: Option<std::sync::mpsc::Receiver<Result<DailyActivity, String>>>,
    pub tooltip_expanded: bool,
    pub tooltip_home: Option<egui::Pos2>,
    pub tooltip_last_pos: Option<egui::Pos2>,
    pub tooltip_last_size: Option<egui::Vec2>,
    #[cfg(windows)]
    pub settings_viewport_data: Option<SettingsViewportState>,
    pub arc_points_cache: Vec<egui::Pos2>,
    pub cached_tooltip_rows: Vec<TooltipRow>,
    pub silent_reauth_in_progress: bool,
    pub silent_reauth_receiver: Option<std::sync::mpsc::Receiver<Option<String>>>,
    pub retry_after_reauth: bool,
    pub last_silent_reauth: Option<Instant>,
    pub logo_texture: Option<egui::TextureHandle>,
}

impl CoconutApp {
    pub fn with_settings(settings: CoconutSettings) -> Self {
        debug_log!("CoconutApp initialized: theme={:?}, auto_start={}, refresh_interval={}s, spend_limit=${}",
            settings.theme, settings.auto_start, settings.refresh_interval_secs, settings.spend_limit);
        Self {
            settings,
            activity: None,
            error: None,
            last_refresh: Instant::now() - DEFAULT_REFRESH_INTERVAL,
            was_hovered: false,
            drag: DragState::new(),
            show_settings: false,
            notification_sent: false,
            animated_percent: 0.0,
            #[cfg(windows)]
            color_key_applied: false,
            refresh_in_progress: false,
            pending_result: None,
            tooltip_expanded: false,
            tooltip_home: None,
            tooltip_last_pos: None,
            tooltip_last_size: None,
            #[cfg(windows)]
            settings_viewport_data: None,
            arc_points_cache: Vec::with_capacity(65),
            cached_tooltip_rows: Vec::new(),
            silent_reauth_in_progress: false,
            silent_reauth_receiver: None,
            retry_after_reauth: false,
            last_silent_reauth: None,
            logo_texture: None,
        }
    }

    fn needs_periodic_refresh(&self) -> bool {
        self.last_refresh.elapsed()
            >= Duration::from_secs(self.settings.refresh_interval_secs.max(30))
    }

    /// Returns true when the billing cycle's end_date has already passed, meaning
    /// the quota should have been reset and a fresh fetch is worthwhile. A
    /// cooldown (`RESET_REFRESH_COOLDOWN`) prevents tight loops when the API
    /// keeps returning an already-past end date.
    fn needs_reset_refresh(&self) -> bool {
        if self.refresh_in_progress {
            return false;
        }
        if self.last_refresh.elapsed() < RESET_REFRESH_COOLDOWN {
            return false;
        }
        self.activity
            .as_ref()
            .is_some_and(|a| crate::api::is_end_date_passed(&a.end_date))
    }

    fn start_refresh(&mut self) {
        if self.refresh_in_progress {
            return;
        }
        if self.silent_reauth_in_progress {
            return;
        }
        if !self.settings.is_configured() {
            self.error = Some("未配置 Token".to_string());
            return;
        }

        self.error = None;
        self.refresh_in_progress = true;
        debug_log!("Coconut: start_refresh spawning background thread...");

        let auth_token = self.settings.authorization_token.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.pending_result = Some(rx);

        std::thread::spawn(move || {
            debug_log!("Coconut bg thread: calling fetch_usage...");
            let result = fetch_usage(&auth_token);
            debug_log!("Coconut bg thread: fetch_usage done (ok={})", result.is_ok());
            let _ = tx.send(result);
        });
    }

    fn check_refresh_result(&mut self) -> bool {
        if !self.refresh_in_progress { return false; }
        if let Some(ref rx) = self.pending_result {
            if let Ok(result) = rx.try_recv() {
                self.refresh_in_progress = false;
                self.pending_result = None;
                self.apply_refresh_result(result);
                return true;
            }
        }
        false
    }

    fn apply_refresh_result(&mut self, result: Result<DailyActivity, String>) {
        match result {
            Ok(activity) => {
                debug_log!("Coconut API SUCCESS: spend=${:.4}, tokens={}",
                    activity.metadata.total_spend, activity.metadata.total_tokens);

                let threshold = self.settings.notification_threshold;
                let pct = self.get_spend_percent(&activity);
                if threshold > 0.0 && pct >= threshold && !self.notification_sent {
                    self.notification_sent = true;
                    debug_log!("Coconut notification: {:.1}% >= {:.0}%", pct, threshold);
                    show_usage_notification("Coconut Plan Widget", pct, threshold);
                } else if pct < threshold {
                    self.notification_sent = false;
                }

                self.build_tooltip_rows(&activity);
                self.activity = Some(activity);
                self.error = None;
                self.retry_after_reauth = false;
            }
            Err(e) => {
                debug_log!("Coconut API FAILED: {}", e);
                self.activity = None;
                self.cached_tooltip_rows.clear();

                if self.retry_after_reauth {
                    self.retry_after_reauth = false;
                    if is_likely_auth_error(&e) {
                        self.error = Some("Token 过期，请重新登录".to_string());
                    } else {
                        self.error = Some(e);
                    }
                } else if is_likely_auth_error(&e)
                    && !self.silent_reauth_in_progress
                    && self.can_start_silent_reauth()
                {
                    self.error = Some("正在重新获取 Token...".to_string());
                    self.start_silent_reauth();
                } else {
                    self.error = Some(e);
                }
            }
        }
        self.last_refresh = Instant::now();
    }

    fn get_spend_percent(&self, activity: &DailyActivity) -> f64 {
        let limit = self.settings.spend_limit.max(0.01);
        (activity.metadata.total_spend / limit * 100.0).min(100.0)
    }

    fn build_tooltip_rows(&mut self, activity: &DailyActivity) {
        let meta = &activity.metadata;
        let limit = self.settings.spend_limit;
        let spend_pct = (meta.total_spend / limit.max(0.01) * 100.0).min(100.0);
        let success_rate = if meta.total_api_requests > 0 {
            meta.total_successful_requests as f64 / meta.total_api_requests as f64 * 100.0
        } else {
            100.0
        };

        self.cached_tooltip_rows = vec![
            TooltipRow {
                label: "本月消费".into(),
                value: format!("${:.2}", meta.total_spend),
                color: percent_color(spend_pct),
            },
            TooltipRow {
                label: "当日消耗占比".into(),
                value: format!("{:.1}% (限额 ${:.0})", spend_pct, limit),
                color: percent_color(spend_pct),
            },
            TooltipRow {
                label: "Token 总量".into(),
                value: format_number(meta.total_tokens),
                color: Color32::from_rgb(200, 200, 210),
            },
            TooltipRow {
                label: "API 请求数".into(),
                value: format!("{}", meta.total_api_requests),
                color: Color32::from_rgb(200, 200, 210),
            },
            TooltipRow {
                label: "Prompt Tokens".into(),
                value: format_number(meta.total_prompt_tokens),
                color: Color32::from_rgb(180, 190, 210),
            },
            TooltipRow {
                label: "Completion Tokens".into(),
                value: format_number(meta.total_completion_tokens),
                color: Color32::from_rgb(180, 190, 210),
            },
            TooltipRow {
                label: "缓存命中 Tokens".into(),
                value: format_number(meta.total_cache_read_input_tokens),
                color: Color32::from_rgb(160, 200, 160),
            },
            TooltipRow {
                label: "成功率".into(),
                value: format!("{:.1}%", success_rate),
                color: if success_rate >= 99.0 {
                    Color32::from_rgb(76, 175, 80)
                } else if success_rate >= 95.0 {
                    Color32::from_rgb(255, 193, 7)
                } else {
                    Color32::from_rgb(244, 67, 54)
                },
            },
        ];
    }

    // ── Silent reauth ────────────────────────────────────────────────────────

    const SILENT_REAUTH_COOLDOWN: Duration = Duration::from_secs(300);

    fn can_start_silent_reauth(&self) -> bool {
        match self.last_silent_reauth {
            Some(t) => t.elapsed() >= Self::SILENT_REAUTH_COOLDOWN,
            None => true,
        }
    }

    #[cfg(windows)]
    fn start_silent_reauth(&mut self) {
        debug_log!("Coconut: starting silent reauth...");
        self.silent_reauth_in_progress = true;
        self.last_silent_reauth = Some(Instant::now());
        let rx = webview_login::try_silent_extract_token();
        self.silent_reauth_receiver = Some(rx);
    }

    #[cfg(not(windows))]
    fn start_silent_reauth(&mut self) {
        self.error = Some("Token 过期，请重新登录".to_string());
    }

    #[cfg(windows)]
    fn check_silent_reauth_result(&mut self) -> bool {
        if !self.silent_reauth_in_progress { return false; }
        if let Some(ref rx) = self.silent_reauth_receiver {
            if let Ok(result) = rx.try_recv() {
                self.silent_reauth_in_progress = false;
                self.silent_reauth_receiver = None;
                if let Some(token) = result {
                    debug_log!("Coconut silent reauth: token obtained, retrying fetch");
                    self.settings.authorization_token = token;
                    self.settings.save();
                    self.retry_after_reauth = true;
                    self.start_refresh();
                } else {
                    debug_log!("Coconut silent reauth: no token obtained");
                    self.error = Some("Token 过期，请重新登录".to_string());
                }
                return true;
            }
        }
        false
    }

    #[cfg(not(windows))]
    fn check_silent_reauth_result(&mut self) -> bool { false }
}

fn is_likely_auth_error(err: &str) -> bool {
    err.starts_with("HTTP 401") || err.starts_with("HTTP 403") || err.starts_with("解析失败")
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

// ── eframe App ───────────────────────────────────────────────────────────────

impl eframe::App for CoconutApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        #[cfg(windows)]
        if let Some(gl) = frame.gl() {
            unsafe {
                gl.clear_color(0.0, 0.0, 0.0, 0.0);
                gl.clear(eframe::glow::COLOR_BUFFER_BIT);
            }
        }

        #[cfg(windows)]
        if !self.color_key_applied {
            if let Some(gl) = frame.gl() {
                let _alpha_bits = query_framebuffer_alpha(gl);
                debug_log!("Coconut framebuffer alpha bits: {}", _alpha_bits);
            }
            self.color_key_applied = true;
        }

        if self.check_refresh_result() {
            ctx.request_repaint();
        }

        if self.check_silent_reauth_result() {
            ctx.request_repaint();
        }

        if self.refresh_in_progress || self.silent_reauth_in_progress {
            ctx.request_repaint();
        }

        let target = self.activity.as_ref()
            .map(|a| self.get_spend_percent(a))
            .unwrap_or(0.0);
        let dt = ctx.input(|i| i.unstable_dt).min(0.1) as f64;
        let lambda: f64 = 8.0;
        let speed = 1.0 - (-lambda * dt).exp();
        self.animated_percent += (target - self.animated_percent) * speed;
        if (self.animated_percent - target).abs() < 0.05 {
            self.animated_percent = target;
        }

        // ── Settings viewport ──
        #[cfg(windows)]
        {
            if tray::SETTINGS_REQUESTED.swap(false, std::sync::atomic::Ordering::SeqCst)
                && !self.show_settings {
                self.show_settings = true;
            }

            if self.show_settings {
                if self.settings_viewport_data.is_none() {
                    let settings_rc = std::rc::Rc::new(std::cell::RefCell::new(self.settings.clone()));
                    let notif_rc = std::rc::Rc::new(std::cell::Cell::new(self.notification_sent));
                    let tab_rc = std::rc::Rc::new(std::cell::Cell::new(0usize));
                    let orig_rc = std::rc::Rc::new(std::cell::RefCell::new(self.settings.clone()));
                    let confirm_rc = std::rc::Rc::new(std::cell::Cell::new(false));
                    let webview_rx = std::rc::Rc::new(std::cell::RefCell::new(None));
                    self.settings_viewport_data = Some(SettingsViewportState {
                        settings: settings_rc,
                        notification_sent: notif_rc,
                        tab_index: tab_rc,
                        original_settings: orig_rc,
                        confirm_dialog: confirm_rc,
                        webview_receiver: webview_rx,
                    });
                }

                if let Some(ref state) = self.settings_viewport_data {
                    let viewport_id = egui::ViewportId::from_hash_of("coconut_settings");
                    let builder = egui::ViewportBuilder::default()
                        .with_title("Coconut 设置")
                        .with_inner_size([640.0, 720.0])
                        .with_resizable(false);

                    let s = state.settings.clone();
                    let n = state.notification_sent.clone();
                    let t = state.tab_index.clone();
                    let o = state.original_settings.clone();
                    let cf = state.confirm_dialog.clone();
                    let should_close = std::rc::Rc::new(std::cell::Cell::new(false));
                    let sc = should_close.clone();
                    let saved = std::rc::Rc::new(std::cell::Cell::new(false));
                    let sv = saved.clone();

                    ctx.show_viewport_immediate(viewport_id, builder, move |ctx, _class| {
                        if ctx.input(|i| i.viewport().close_requested()) {
                            if cf.get() {
                                sc.set(true);
                            } else {
                                let current = s.borrow();
                                let original = o.borrow();
                                if *current != *original {
                                    cf.set(true);
                                } else {
                                    sc.set(true);
                                }
                            }
                        }

                        let mut visuals = egui::Visuals::dark();
                        visuals.panel_fill = Color32::from_rgb(0x1E, 0x1E, 0x22);
                        visuals.window_fill = Color32::from_rgb(0x25, 0x25, 0x28);
                        visuals.faint_bg_color = Color32::from_rgb(0x2D, 0x2D, 0x32);
                        visuals.extreme_bg_color = Color32::from_rgb(0x18, 0x18, 0x1C);
                        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(0x33, 0x33, 0x38);
                        visuals.widgets.inactive.bg_fill = Color32::from_rgb(0x3A, 0x3A, 0x40);
                        visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x45, 0x45, 0x4C);
                        visuals.widgets.active.bg_fill = Color32::from_rgb(0x50, 0x50, 0x58);
                        visuals.widgets.open.bg_fill = Color32::from_rgb(0x3A, 0x3A, 0x40);
                        visuals.selection.bg_fill = Color32::from_rgb(0x00, 0x78, 0xD4);
                        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
                        visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
                        ctx.set_visuals(visuals);

                        if cf.get() {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                ui.add_space(80.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new("确定要关闭吗？").color(Color32::WHITE)
                                        .font(egui::FontId::proportional(16.0)).strong());
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new("有未保存的更改")
                                        .color(Color32::from_rgb(0x99, 0x99, 0x9F))
                                        .font(egui::FontId::proportional(13.0)));
                                    ui.add_space(24.0);
                                    ui.horizontal(|ui| {
                                        let spacing = 12.0;
                                        let btn_w = (ui.available_width() - spacing) / 2.0;
                                        let save_btn = egui::Button::new(
                                            egui::RichText::new("  保存并退出  ").color(Color32::WHITE)
                                                .font(egui::FontId::proportional(14.0)),
                                        ).fill(Color32::from_rgb(0x00, 0x78, 0xD4))
                                            .corner_radius(egui::CornerRadius::same(6))
                                            .min_size(egui::vec2(btn_w, 36.0));
                                        if ui.add(save_btn).clicked() {
                                            s.borrow().save();
                                            apply_auto_start("CoconutPlanWidget", s.borrow().auto_start);
                                            sv.set(true);
                                            sc.set(true);
                                        }
                                        ui.add_space(12.0);
                                        let discard_btn = egui::Button::new(
                                            egui::RichText::new("  不保存  ").color(Color32::from_rgb(0x99, 0x99, 0x9F))
                                                .font(egui::FontId::proportional(14.0)),
                                        ).fill(Color32::from_rgb(0x3A, 0x3A, 0x40))
                                            .corner_radius(egui::CornerRadius::same(6))
                                            .min_size(egui::vec2(btn_w, 36.0));
                                        if ui.add(discard_btn).clicked() {
                                            sv.set(false);
                                            sc.set(true);
                                        }
                                    });
                                });
                            });
                        } else {
                            let mut notif_val = n.get();
                            let mut tab_val = t.get();
                            let mut saved_val = sv.get();
                            let mut close_req = false;
                            Self::render_settings_viewport(
                                ctx, &mut s.borrow_mut(),
                                &mut notif_val, &mut tab_val, &mut saved_val, &mut close_req,
                                &state.webview_receiver,
                            );
                            n.set(notif_val);
                            t.set(tab_val);
                            sv.set(saved_val);
                            if close_req { sc.set(true); }
                        }
                        ctx.request_repaint();
                    });

                    // Poll webview login result
                    {
                        let mut rx_opt = state.webview_receiver.borrow_mut();
                        if let Some(ref rx) = *rx_opt {
                            if let Ok(result) = rx.try_recv() {
                                if let Some(token) = result {
                                    debug_log!("WebView2: token received, updating settings");
                                    let mut s = state.settings.borrow_mut();
                                    s.authorization_token = token;
                                    s.save();
                                    saved.set(true);
                                    should_close.set(true);
                                }
                                *rx_opt = None;
                            }
                        }
                    }

                    if should_close.get() {
                        if saved.get() {
                            self.settings = state.settings.borrow().clone();
                            self.settings.save();
                            apply_auto_start("CoconutPlanWidget", self.settings.auto_start);
                        } else {
                            self.settings = state.original_settings.borrow().clone();
                        }
                        self.notification_sent = state.notification_sent.get();
                        self.show_settings = false;
                        self.settings_viewport_data = None;
                        self.start_refresh();
                    }
                }
            }

            #[cfg(windows)]
            if self.show_settings {
                if let Some(ref state) = self.settings_viewport_data {
                    let s = state.settings.borrow();
                    self.settings.theme = s.theme;
                }
            }

            match tray::check_events() {
                Some(tray::TrayCommand::Refresh) => {
                    debug_log!("Coconut tray: refresh requested");
                    self.start_refresh();
                }
                None => {}
            }
        }

        // ── Theme ──
        let colors = self.settings.theme.colors();
        let mut visuals = ctx.style().visuals.clone();
        visuals.panel_fill = colors.bg_fill;
        visuals.window_fill = Color32::TRANSPARENT;
        visuals.faint_bg_color = Color32::TRANSPARENT;
        visuals.extreme_bg_color = Color32::TRANSPARENT;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, colors.widget_fg);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, colors.widget_fg);
        ctx.set_visuals(visuals);

        if self.needs_periodic_refresh() {
            debug_log!(
                "Periodic refresh triggered (elapsed: {}s)",
                self.last_refresh.elapsed().as_secs()
            );
            self.start_refresh();
        } else if self.needs_reset_refresh() {
            debug_log!(
                "Reset refresh triggered (end_date passed, elapsed: {}s)",
                self.last_refresh.elapsed().as_secs()
            );
            self.start_refresh();
        }

        if (self.animated_percent - target).abs() > 0.05 {
            ctx.request_repaint();
        } else if !self.refresh_in_progress {
            ctx.request_repaint_after(Duration::from_secs(1));
        }

        // ── Pointer / hover ──
        let (pointer_pos, button_down, button_clicked, viewport_rect, has_pointer_event, has_pointer) =
            ctx.input(|i| {
                (i.pointer.interact_pos(), i.pointer.button_down(egui::PointerButton::Primary),
                 i.pointer.button_clicked(egui::PointerButton::Primary), i.viewport().outer_rect,
                 i.events.iter().any(|e| matches!(e, egui::Event::PointerMoved(_))), i.pointer.has_pointer())
            });

        let widget_size = widget_window_size();
        let cfg = widget_config();
        let radius = cfg.circle_radius;
        let circle_x = 2.0 + radius + cfg.stroke_width / 2.0;

        let cur_pos = viewport_rect.map(|r| r.min);
        let mut home = self.tooltip_home.or(cur_pos);

        let mouse_screen = self.drag.compute_mouse_screen(
            cur_pos, pointer_pos, has_pointer_event, has_pointer,
        );

        let circle_screen_center = home.map(|h| egui::pos2(h.x + circle_x, h.y + widget_size.y / 2.0));
        let circle_hovered = match (mouse_screen, circle_screen_center) {
            (Some(ms), Some(cc)) => ms.distance(cc) <= radius,
            _ => false,
        };

        let show_tooltip = circle_hovered && !self.drag.is_dragging && self.activity.is_some();

        // ── Tooltip sizing ──
        const TIP_PAD_X: f32 = 10.0;
        const TIP_PAD_Y: f32 = 8.0;
        const TIP_LINE_H: f32 = 16.0;
        const TIP_GAP: f32 = 4.0;
        const TIP_HEADER_H: f32 = 20.0;

        let (tooltip_w, tooltip_h) = if show_tooltip {
            let font_id = egui::FontId::proportional(11.0);
            let label_w = ctx.fonts(|f| {
                let mut max_w = 0.0f32;
                for row in &self.cached_tooltip_rows {
                    let w = f.layout(row.label.clone(), font_id.clone(), Color32::WHITE, f32::INFINITY).size().x;
                    max_w = max_w.max(w);
                }
                max_w
            });
            let value_w = ctx.fonts(|f| {
                let mut max_w = 0.0f32;
                for row in &self.cached_tooltip_rows {
                    let w = f.layout(row.value.clone(), font_id.clone(), Color32::WHITE, f32::INFINITY).size().x;
                    max_w = max_w.max(w);
                }
                max_w
            });
            let col_gap = 12.0;
            let w = (label_w + col_gap + value_w + TIP_PAD_X * 2.0).max(280.0);
            let rows = self.cached_tooltip_rows.len();
            let h = TIP_HEADER_H + rows as f32 * TIP_LINE_H + TIP_PAD_Y * 2.0;
            (w, h)
        } else {
            (0.0, 0.0)
        };

        // ── Placement ──
        let placement = if show_tooltip {
            compute_tooltip_placement(
                home,
                widget_size,
                egui::vec2(tooltip_w, tooltip_h),
                ctx.pixels_per_point(),
                TIP_GAP,
            )
        } else {
            compute_tooltip_placement(
                None, widget_size, egui::vec2(0.0, 0.0), ctx.pixels_per_point(), TIP_GAP,
            )
        };
        let tip_top = placement.top;
        let tip_right = placement.right;
        let work_area = placement.work_area;

        // ── Drag ──
        if button_down && (circle_hovered || self.drag.is_dragging) {
            if let (Some(ms), Some(h)) = (mouse_screen, home) {
                if self.drag.press_pos.is_none() { self.drag.press_pos = Some(ms); }
                if self.drag.should_drag(ms, DRAG_THRESHOLD) {
                    if !self.drag.is_dragging {
                        self.drag.engage(ms, h);
                    }
                    if let Some(anchor) = self.drag.anchor {
                        let clamped = clamp_home_to_work_area(ms - anchor, widget_size, ctx.pixels_per_point());
                        home = Some(clamped);
                        self.tooltip_home = Some(clamped);
                    }
                }
            }
        }

        let full_h = widget_size.y + TIP_GAP + tooltip_h;
        let tip_pref_x = home.map(|h| if tip_right { h.x + widget_size.x - tooltip_w } else { h.x }).unwrap_or(0.0);
        let tip_screen_x = if show_tooltip {
            match work_area {
                Some(a) if tooltip_w <= (a.max.x - a.min.x).max(0.0) => tip_pref_x.clamp(a.min.x, a.max.x - tooltip_w),
                _ => tip_pref_x,
            }
        } else { tip_pref_x };

        let (win_left, win_right) = if show_tooltip {
            let h = home.unwrap_or(egui::Pos2::ZERO);
            (h.x.min(tip_screen_x), (h.x + widget_size.x).max(tip_screen_x + tooltip_w))
        } else {
            let h = home.unwrap_or(egui::Pos2::ZERO);
            (h.x, h.x + widget_size.x)
        };
        let win_w = (win_right - win_left).max(0.0);
        let desired_size = if show_tooltip { egui::vec2(win_w, full_h) } else { widget_size };
        let desired_pos = if self.drag.is_dragging {
            home
        } else if show_tooltip {
            home.map(|h| {
                let win_y = if tip_top { h.y - TIP_GAP - tooltip_h } else { h.y };
                egui::pos2(win_left, win_y)
            })
        } else { home };

        if self.tooltip_last_size != Some(desired_size) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(desired_size));
            self.tooltip_last_size = Some(desired_size);
        }
        if let Some(dp) = desired_pos {
            if self.tooltip_last_pos.is_none_or(|lp| lp.distance(dp) > 0.5) {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(dp));
                self.tooltip_last_pos = Some(dp);
            }
        }

        if self.drag.is_dragging {
        } else if show_tooltip {
            if self.tooltip_home.is_none() { self.tooltip_home = cur_pos; }
        } else if let (Some(c), Some(h)) = (cur_pos, self.tooltip_home) {
            if c.distance(h) < 1.0 { self.tooltip_home = None; }
        }
        self.tooltip_expanded = show_tooltip;

        let cur = cur_pos.unwrap_or(egui::Pos2::ZERO);
        let home_pos = home.unwrap_or(egui::Pos2::ZERO);
        let widget_local_x = home_pos.x - cur.x;
        let widget_local_y = home_pos.y - cur.y;
        let center = egui::pos2(widget_local_x + circle_x, widget_local_y + widget_size.y / 2.0);

        let tip_local = if show_tooltip {
            let tip_screen_y = if tip_top { home_pos.y - TIP_GAP - tooltip_h } else { home_pos.y + widget_size.y + TIP_GAP };
            Some(egui::pos2(tip_screen_x - cur.x, tip_screen_y - cur.y))
        } else { None };

        let layer_id = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("coconut_widget"));
        let painter = ctx.layer_painter(layer_id);

        // Circle background
        painter.circle_filled(center, radius, colors.circle_bg);

        // Logo watermark (coconut.is)
        if self.logo_texture.is_none() {
            let bytes = include_bytes!("../../../assets/logo_coconut.png");
            if let Ok(img) = image::load_from_memory(bytes) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                self.logo_texture = Some(ctx.load_texture("coconut-logo", color_image, egui::TextureOptions::LINEAR));
            } else {
                // Fallback: try loading volc logo or skip
                let bytes = include_bytes!("../../../assets/logo.png");
                if let Ok(img) = image::load_from_memory(bytes) {
                    let rgba = img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                    self.logo_texture = Some(ctx.load_texture("coconut-logo", color_image, egui::TextureOptions::LINEAR));
                } else {
                    debug_log!("Coconut: failed to decode logo");
                }
            }
        }
        if let Some(ref tex) = self.logo_texture {
            let logo_side = radius * 1.3;
            let logo_rect = egui::Rect::from_center_size(center, egui::vec2(logo_side, logo_side));
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            let tint = Color32::from_rgba_unmultiplied(colors.widget_fg.r(), colors.widget_fg.g(), colors.widget_fg.b(), 40);
            painter.image(tex.id(), logo_rect, uv, tint);
        }

        // Arc
        let percent = self.animated_percent;
        draw_widget_circle(
            &painter,
            center,
            radius,
            cfg.stroke_width,
            percent,
            percent_color(percent),
            &mut self.arc_points_cache,
        );

        // Error text
        if let Some(ref err) = self.error {
            let font_id = egui::FontId::proportional(cfg.error_font_size);
            let galley = ctx.fonts(|f| f.layout(err.clone(), font_id, Color32::from_rgb(244, 67, 54), f32::INFINITY));
            let text_pos = egui::pos2(center.x + radius + 6.0, widget_local_y + (widget_size.y - galley.size().y) / 2.0);
            painter.galley(text_pos, galley, Color32::from_rgb(244, 67, 54));
        } else {
            let pct = self.activity.as_ref().map(|a| self.get_spend_percent(a)).unwrap_or(0.0);
            let color = percent_color(pct);
            let text = format!("{:.1}%", pct);
            let font_id = egui::FontId::proportional(cfg.percent_font_size);
            let galley = ctx.fonts(|f| f.layout(text, font_id, color, f32::INFINITY));
            let text_pos = egui::pos2(center.x - galley.size().x / 2.0, center.y - galley.size().y / 2.0);
            painter.galley(text_pos, galley, color);
        }

        // Tooltip table
        if let Some(tl) = tip_local {
            if self.activity.is_some() && tooltip_w > 0.0 {
                let tooltip_bg = egui::Rect::from_min_size(tl, egui::vec2(tooltip_w, tooltip_h));
                painter.rect_filled(tooltip_bg, 6.0, colors.bg_fill);

                // Header
                let header_id = egui::FontId::proportional(12.0);
                let header_text = if let Some(ref act) = self.activity {
                    format!("本月使用统计（{} ~ {}）", act.start_date, act.end_date)
                } else { String::new() };
                let header_galley = ctx.fonts(|f| f.layout(header_text, header_id.clone(), Color32::from_rgb(0xBB, 0xBB, 0xCC), f32::INFINITY));
                painter.galley(egui::pos2(tl.x + TIP_PAD_X, tl.y + TIP_PAD_Y), header_galley, Color32::from_rgb(0xBB, 0xBB, 0xCC));

                // Separator line
                let sep_y = tl.y + TIP_PAD_Y + TIP_HEADER_H - 2.0;
                painter.line_segment(
                    [egui::pos2(tl.x + TIP_PAD_X, sep_y), egui::pos2(tl.x + tooltip_w - TIP_PAD_X, sep_y)],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 30)),
                );

                // Data rows
                let font_id = egui::FontId::proportional(11.0);
                let col_label_x = tl.x + TIP_PAD_X;
                let col_value_x = tl.x + tooltip_w - TIP_PAD_X;
                let mut y = tl.y + TIP_PAD_Y + TIP_HEADER_H + 2.0;
                for row in &self.cached_tooltip_rows {
                    // Label (left-aligned)
                    let label_galley = ctx.fonts(|f| f.layout(row.label.clone(), font_id.clone(), Color32::from_rgb(0x99, 0x99, 0x9F), f32::INFINITY));
                    painter.galley(egui::pos2(col_label_x, y), label_galley, Color32::from_rgb(0x99, 0x99, 0x9F));

                    // Value (right-aligned)
                    let val_galley = ctx.fonts(|f| f.layout(row.value.clone(), font_id.clone(), row.color, f32::INFINITY));
                    let val_x = col_value_x - val_galley.size().x;
                    painter.galley(egui::pos2(val_x, y), val_galley, row.color);
                    y += TIP_LINE_H;
                }
            }
        }

        if button_clicked && circle_hovered {
            self.show_settings = true;
        }

        if !button_down && (self.drag.is_dragging || self.drag.press_pos.is_some()) {
            let was_dragging = self.drag.disengage();
            if was_dragging {
                if let Some(h) = home {
                    let changed = self.settings.window_x.is_none_or(|x| (x - h.x).abs() > 0.5)
                        || self.settings.window_y.is_none_or(|y| (y - h.y).abs() > 0.5);
                    if changed {
                        self.settings.window_x = Some(h.x);
                        self.settings.window_y = Some(h.y);
                        self.settings.save();
                    }
                }
            }
        }

        if circle_hovered && !self.was_hovered
            && self.last_refresh.elapsed() >= HOVER_COOLDOWN {
            self.start_refresh();
        }
        self.was_hovered = circle_hovered;
    }
}

// ── Settings viewport rendering ──────────────────────────────────────────────

impl CoconutApp {
    pub fn render_settings_viewport(
        ctx: &egui::Context,
        settings: &mut CoconutSettings,
        notif: &mut bool,
        tab: &mut usize,
        saved: &mut bool,
        close_requested: &mut bool,
        webview_receiver: &std::rc::Rc<std::cell::RefCell<Option<std::sync::mpsc::Receiver<Option<String>>>>>,
    ) {
        let accent = Color32::from_rgb(0x00, 0x78, 0xD4);
        let card_bg = Color32::from_rgb(0x2D, 0x2D, 0x32);
        let text_muted = Color32::from_rgb(0x99, 0x99, 0x9F);

        egui::CentralPanel::default().show(ctx, |ui| {
            let tab_labels = ["通用", "Token"];
            let mut selected = *tab;
            ui.horizontal(|ui| {
                for (i, label) in tab_labels.iter().enumerate() {
                    let is_selected = selected == i;
                    let text = if is_selected { egui::RichText::new(*label).color(Color32::WHITE).strong() }
                        else { egui::RichText::new(*label).color(text_muted) };
                    if ui.selectable_label(is_selected, text).clicked() { selected = i; }
                    ui.add_space(4.0);
                }
            });
            if selected != *tab { *tab = selected; }
            ui.add_space(8.0);

            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                let section_gap = 12.0;
                let section = |ui: &mut egui::Ui, title: &str, body: &mut dyn FnMut(&mut egui::Ui)| {
                    egui::Frame::NONE.fill(card_bg).corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                            ui.label(egui::RichText::new(title).color(Color32::WHITE)
                                .font(egui::FontId::proportional(14.0)).strong());
                            ui.add_space(8.0);
                            body(ui);
                        });
                    ui.add_space(section_gap);
                };

                if *tab == 0 {
                    render_common_general_tab(
                        ui,
                        &mut settings.refresh_interval_secs,
                        &mut settings.notification_threshold,
                        &mut settings.theme,
                        &mut settings.auto_start,
                        card_bg,
                        text_muted,
                    );
                } else {
                    section(ui, "Authorization Token", &mut |ui| {
                        ui.label(egui::RichText::new("JWT Bearer Token").color(text_muted)
                            .font(egui::FontId::proportional(12.0)));
                        ui.add_sized(
                            egui::vec2(ui.available_width(), 80.0),
                            egui::TextEdit::multiline(&mut settings.authorization_token)
                                .hint_text("粘贴 Authorization Token (Bearer ...)"),
                        );
                    });
                    ui.add_space(4.0);
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                        let btn = egui::Button::new(egui::RichText::new("打开控制台").color(Color32::WHITE)
                            .font(egui::FontId::proportional(14.0)))
                            .fill(accent).corner_radius(egui::CornerRadius::same(6)).min_size(egui::vec2(160.0, 36.0));
                        if ui.add(btn).clicked()
                            && webview_receiver.borrow().is_none() {
                            debug_log!("Coconut settings: open WebView2 login clicked");
                            #[cfg(windows)]
                            {
                                let rx = webview_login::try_extract_token();
                                webview_receiver.borrow_mut().replace(rx);
                            }
                        }
                    });
                    ui.add_space(4.0);
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                        let btn = egui::Button::new(egui::RichText::new("清理 Token").color(Color32::WHITE)
                            .font(egui::FontId::proportional(14.0)))
                            .fill(Color32::from_rgb(0xD4, 0x3F, 0x3F)).corner_radius(egui::CornerRadius::same(6)).min_size(egui::vec2(160.0, 36.0));
                        if ui.add(btn).clicked() {
                            settings.authorization_token.clear();
                            settings.save();
                            *saved = true;
                        }
                    });
                }

                ui.add_space(4.0);
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                    let btn = egui::Button::new(egui::RichText::new("  保存并关闭  ").color(Color32::WHITE)
                        .font(egui::FontId::proportional(14.0)))
                        .fill(accent).corner_radius(egui::CornerRadius::same(6)).min_size(egui::vec2(140.0, 36.0));
                    if ui.add(btn).clicked() {
                        settings.save();
                        apply_auto_start("CoconutPlanWidget", settings.auto_start);
                        *notif = false;
                        *saved = true;
                        *close_requested = true;
                    }
                });
            });
        });
    }
}
