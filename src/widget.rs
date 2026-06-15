use eframe::egui;
use egui::epaint::PathShape;
use egui::{Color32, Pos2, Shape, Stroke};
use std::time::{Duration, Instant};

#[cfg(windows)]
use eframe::glow::HasContext as _;

use crate::api::{console_url, fetch_usage, format_level_line, QuotaLevel};
use crate::debug_log;
use crate::settings::Settings;
use crate::theme::{percent_color, Theme, WidgetSize};
use crate::{DEFAULT_REFRESH_INTERVAL, HOVER_COOLDOWN, open_url, show_usage_notification};

#[cfg(windows)]
use crate::apply_auto_start;
#[cfg(windows)]
use crate::tray;

// ── Widget State ─────────────────────────────────────────────────────────────

/// Shared state for the settings viewport (Rc<RefCell<>> so it persists across frames).
#[cfg(windows)]
pub struct SettingsViewportState {
    pub settings: std::rc::Rc<std::cell::RefCell<Settings>>,
    pub notification_sent: std::rc::Rc<std::cell::Cell<bool>>,
    pub tab_index: std::rc::Rc<std::cell::Cell<usize>>,
    pub original_settings: std::rc::Rc<std::cell::RefCell<Settings>>,
    pub confirm_dialog: std::rc::Rc<std::cell::Cell<bool>>,
}

pub struct WidgetApp {
    pub settings: Settings,
    pub usage: Option<Vec<QuotaLevel>>,
    pub error: Option<String>,
    pub last_refresh: Instant,
    pub was_hovered: bool,
    pub is_dragging: bool,
    pub show_settings: bool,
    pub notification_sent: bool,
    pub animated_percent: f64,
    pub last_widget_size: WidgetSize,
    #[cfg(windows)]
    pub color_key_applied: bool,
    #[cfg(debug_assertions)]
    pub frame_count: u64,
    /// Offset from window origin to mouse at drag start (for lerp-based tracking).
    pub drag_offset: Option<egui::Vec2>,
    /// True while a background refresh thread is running.
    pub refresh_in_progress: bool,
    /// Receiver for the background thread's result (oneshot channel).
    pub pending_result: Option<std::sync::mpsc::Receiver<Result<Vec<QuotaLevel>, String>>>,
    /// Whether the hover tooltip is currently visible (window expanded).
    pub tooltip_expanded: bool,
    /// Shared state for the settings viewport.
    #[cfg(windows)]
    pub settings_viewport_data: Option<SettingsViewportState>,
    /// Pre-allocated buffer for arc segment points (reused each frame).
    pub arc_points_cache: Vec<egui::Pos2>,
    /// Pre-computed tooltip lines (updated on each successful refresh).
    pub cached_level_lines: Vec<(String, Color32)>,
}

impl WidgetApp {
    pub fn with_settings(settings: Settings) -> Self {
        let widget_size = settings.widget_size;
        debug_log!("WidgetApp initialized: theme={:?}, size={:?}, auto_start={}, region={}, refresh_interval={}s, notification_threshold={}%",
            settings.theme, settings.widget_size, settings.auto_start, settings.region, settings.refresh_interval_secs, settings.notification_threshold);
        Self {
            settings,
            usage: None,
            error: None,
            last_refresh: Instant::now() - DEFAULT_REFRESH_INTERVAL, // force refresh on start
            was_hovered: false,
            is_dragging: false,
            show_settings: false,
            notification_sent: false,
            animated_percent: 0.0,
            last_widget_size: widget_size,
            #[cfg(windows)]
            color_key_applied: false,
            #[cfg(debug_assertions)]
            frame_count: 0,
            drag_offset: None,
            refresh_in_progress: false,
            pending_result: None,
            tooltip_expanded: false,
            #[cfg(windows)]
            settings_viewport_data: None,
            arc_points_cache: Vec::with_capacity(65),
            cached_level_lines: Vec::new(),
        }
    }

    fn needs_periodic_refresh(&self) -> bool {
        self.last_refresh.elapsed()
            >= Duration::from_secs(self.settings.refresh_interval_secs.max(30))
    }

    /// Start a background refresh if one isn't already in progress.
    /// The network request runs on a separate thread so the UI never blocks.
    fn start_refresh(&mut self) {
        if self.refresh_in_progress {
            debug_log!("start_refresh: already in progress, skipping");
            return;
        }
        if !self.settings.is_configured() {
            self.error = Some("未配置凭证".to_string());
            debug_log!("start_refresh: not configured");
            return;
        }

        self.error = None;
        self.refresh_in_progress = true;
        debug_log!("start_refresh: spawning background thread...");

        let cookie = self.settings.cookie.clone();
        let csrf_token = self.settings.csrf_token.clone();
        let region = self.settings.region.clone();

        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.pending_result = Some(rx);

        std::thread::spawn(move || {
            debug_log!("Background thread: calling fetch_usage...");
            let result = fetch_usage(&cookie, &csrf_token, &region);
            debug_log!(
                "Background thread: fetch_usage completed (success={})",
                result.is_ok()
            );
            let _ = tx.send(result);
        });
    }

    /// Check if a background refresh has completed and apply the result.
    /// Returns true if a result was applied.
    fn check_refresh_result(&mut self) -> bool {
        if !self.refresh_in_progress {
            return false;
        }
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

    /// Apply a completed refresh result on the main thread.
    fn apply_refresh_result(&mut self, result: Result<Vec<QuotaLevel>, String>) {
        match result {
            Ok(usage) => {
                debug_log!("API call SUCCESS, got {} levels", usage.len());
                for level in &usage {
                    debug_log!(
                        "  {}: {:.1}% (reset at {})",
                        level.level,
                        level.percent,
                        level.reset_timestamp
                    );
                }

                // Check notification threshold
                let threshold = self.settings.notification_threshold;
                if threshold > 0.0 {
                    if let Some(monthly_pct) = usage
                        .iter()
                        .find(|q| q.level == "monthly")
                        .map(|q| q.percent)
                    {
                        if monthly_pct >= threshold && !self.notification_sent {
                            self.notification_sent = true;
                            debug_log!(
                                "Notification triggered: {:.1}% >= threshold {:.0}%",
                                monthly_pct,
                                threshold
                            );
                            show_usage_notification(monthly_pct, threshold);
                        } else if monthly_pct < threshold {
                            if self.notification_sent {
                                debug_log!(
                                    "Notification reset: {:.1}% < threshold {:.0}%",
                                    monthly_pct,
                                    threshold
                                );
                            }
                            self.notification_sent = false;
                        }
                    }
                }

                self.cached_level_lines = usage.iter().map(|level| {
                    let (label, countdown) = format_level_line(level);
                    let color = percent_color(level.percent);
                    let text = if label.is_empty() {
                        format!("当前会话  {:.1}%  {}", level.percent, countdown)
                    } else {
                        format!("{}  {:.1}%  {}", label, level.percent, countdown)
                    };
                    (text, color)
                }).collect();

                self.usage = Some(usage);
                self.error = None;
            }
            Err(e) => {
                debug_log!("API call FAILED: {}", e);
                self.error = Some(e);
            }
        }

        self.last_refresh = Instant::now();
    }

    fn get_monthly_percent(&self) -> Option<f64> {
        self.usage
            .as_ref()
            .and_then(|u| u.iter().find(|q| q.level == "monthly"))
            .map(|q| q.percent)
    }
}

// ── eframe App ───────────────────────────────────────────────────────────────

impl eframe::App for WidgetApp {
    /// Override clear color to always use the widget's own background.
    /// When `show_viewport_immediate` runs the child viewport's UI on the
    /// parent's egui context, the child's `set_visuals()` overwrites the
    /// parent's visuals. This causes the rendering loop to clear the widget
    /// window with the child's opaque panel fill instead of the widget's
    /// semi-transparent background, producing a black frame.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // The widget window is transparent; the semi-transparent background
        // comes from egui's CentralPanel (panel_fill), not from the clear.
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        #[cfg(windows)]
        if let Some(gl) = frame.gl() {
            unsafe {
                gl.clear_color(0.0, 0.0, 0.0, 0.0);
                gl.clear(eframe::glow::COLOR_BUFFER_BIT);
                let err = gl.get_error();
                if err != 0 {
                    debug_log!("OpenGL error after clear: {}", err);
                }
            }
        }

        #[cfg(windows)]
        if !self.color_key_applied {
            if let Some(gl) = frame.gl() {
                unsafe {
                    let alpha_bits = gl.get_framebuffer_attachment_parameter_i32(
                        eframe::glow::FRAMEBUFFER,
                        eframe::glow::COLOR_ATTACHMENT0,
                        eframe::glow::FRAMEBUFFER_ATTACHMENT_ALPHA_SIZE,
                    );
                    debug_log!("Framebuffer alpha bits: {}", alpha_bits);
                }
            }
            self.color_key_applied = true;
        }

        #[cfg(debug_assertions)]
        {
            self.frame_count += 1;
            if self.frame_count % 60 == 0 {
                debug_log!("update frame #{}", self.frame_count);
            }
        }

        if self.check_refresh_result() {
            ctx.request_repaint();
        }

        if self.refresh_in_progress {
            ctx.request_repaint();
        }

        let target = self.get_monthly_percent().unwrap_or(0.0);
        let dt = ctx.input(|i| i.unstable_dt).min(0.1) as f64; // 限制最大 dt 防止卡顿后跳变
        // 使用指数衰减: speed = 1 - exp(-lambda * dt)
        // lambda = 8.0 约等于原来 0.12 在 60fps 下的表现
        let lambda: f64 = 8.0;
        let speed = 1.0 - (-lambda * dt).exp();
        self.animated_percent += (target - self.animated_percent) * speed;
        if (self.animated_percent - target).abs() < 0.05 {
            self.animated_percent = target;
        }

        // ── Settings viewport (separate popup window) ──
        #[cfg(windows)]
        {
            if tray::tray::SETTINGS_REQUESTED.swap(false, std::sync::atomic::Ordering::SeqCst) {
                debug_log!("Tray: settings requested");
                if !self.show_settings {
                    self.show_settings = true;
                }
            }

            if self.show_settings {
                // Initialize shared state on first frame
                if self.settings_viewport_data.is_none() {
                    let settings_rc = std::rc::Rc::new(std::cell::RefCell::new(
                        self.settings.clone(),
                    ));
                    let notif_rc =
                        std::rc::Rc::new(std::cell::Cell::new(self.notification_sent));
                    let tab_rc = std::rc::Rc::new(std::cell::Cell::new(0usize));
                    let orig_rc = std::rc::Rc::new(std::cell::RefCell::new(
                        self.settings.clone(),
                    ));
                    let confirm_rc = std::rc::Rc::new(std::cell::Cell::new(false));
                    self.settings_viewport_data = Some(SettingsViewportState {
                        settings: settings_rc,
                        notification_sent: notif_rc,
                        tab_index: tab_rc,
                        original_settings: orig_rc,
                        confirm_dialog: confirm_rc,
                    });
                }

                if let Some(ref state) = self.settings_viewport_data {
                    let viewport_id = egui::ViewportId::from_hash_of("settings");
                    let builder = egui::ViewportBuilder::default()
                        .with_title("⚙ 设置")
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
                        // Handle OS close button
                        if ctx.input(|i| i.viewport().close_requested()) {
                            let current = s.borrow();
                            let original = o.borrow();
                            if *current != *original {
                                cf.set(true);
                            } else {
                                sc.set(true);
                            }
                            return;
                        }

                        // Set dark theme
                        let mut visuals = egui::Visuals::dark();
                        visuals.panel_fill = Color32::from_rgb(0x1E, 0x1E, 0x22);
                        visuals.window_fill = Color32::from_rgb(0x25, 0x25, 0x28);
                        visuals.faint_bg_color = Color32::from_rgb(0x2D, 0x2D, 0x32);
                        visuals.extreme_bg_color = Color32::from_rgb(0x18, 0x18, 0x1C);
                        visuals.widgets.noninteractive.bg_fill =
                            Color32::from_rgb(0x33, 0x33, 0x38);
                        visuals.widgets.inactive.bg_fill =
                            Color32::from_rgb(0x3A, 0x3A, 0x40);
                        visuals.widgets.hovered.bg_fill =
                            Color32::from_rgb(0x45, 0x45, 0x4C);
                        visuals.widgets.active.bg_fill =
                            Color32::from_rgb(0x50, 0x50, 0x58);
                        visuals.widgets.open.bg_fill =
                            Color32::from_rgb(0x3A, 0x3A, 0x40);
                        visuals.selection.bg_fill = Color32::from_rgb(0x00, 0x78, 0xD4);
                        visuals.widgets.hovered.fg_stroke =
                            Stroke::new(1.0, Color32::WHITE);
                        visuals.widgets.active.fg_stroke =
                            Stroke::new(1.0, Color32::WHITE);
                        ctx.set_visuals(visuals);

                        if cf.get() {
                            // ── Confirmation dialog ──
                            egui::CentralPanel::default().show(ctx, |ui| {
                                ui.add_space(80.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        egui::RichText::new("确定要关闭吗？")
                                            .color(Color32::WHITE)
                                            .font(egui::FontId::proportional(16.0))
                                            .strong(),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new("有未保存的更改")
                                            .color(Color32::from_rgb(0x99, 0x99, 0x9F))
                                            .font(egui::FontId::proportional(13.0)),
                                    );
                                    ui.add_space(24.0);
                                    ui.horizontal(|ui| {
                                        let spacing = 12.0;
                                        let btn_w = (ui.available_width() - spacing) / 2.0;
                                        let save_btn = egui::Button::new(
                                            egui::RichText::new("  保存并退出  ")
                                                .color(Color32::WHITE)
                                                .font(egui::FontId::proportional(14.0)),
                                        )
                                        .fill(Color32::from_rgb(0x00, 0x78, 0xD4))
                                        .corner_radius(egui::CornerRadius::same(6))
                                        .min_size(egui::vec2(btn_w, 36.0));
                                        if ui.add(save_btn).clicked() {
                                            s.borrow().save();
                                            #[cfg(windows)]
                                            apply_auto_start(s.borrow().auto_start);
                                            sv.set(true);
                                            sc.set(true);
                                        }
                                        ui.add_space(12.0);
                                        let discard_btn = egui::Button::new(
                                            egui::RichText::new("  不保存  ")
                                                .color(Color32::from_rgb(0x99, 0x99, 0x9F))
                                                .font(egui::FontId::proportional(14.0)),
                                        )
                                        .fill(Color32::from_rgb(0x3A, 0x3A, 0x40))
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
                            let mut show = true;
                            let mut notif_val = n.get();
                            let mut tab_val = t.get();
                            let mut saved_val = sv.get();
                            WidgetApp::render_settings_viewport(
                                ctx,
                                &mut s.borrow_mut(),
                                &mut show,
                                &mut notif_val,
                                &mut tab_val,
                                &mut saved_val,
                            );
                            n.set(notif_val);
                            t.set(tab_val);
                            sv.set(saved_val);

                            if !show {
                                sc.set(true);
                            }
                        }

                        ctx.request_repaint();
                    });

                    if should_close.get() {
                        debug_log!("Settings: closed");
                        if saved.get() {
                            self.settings = state.settings.borrow().clone();
                            self.settings.save();
                            apply_auto_start(self.settings.auto_start);
                        } else {
                            // Revert to original settings
                            self.settings = state.original_settings.borrow().clone();
                        }
                        self.notification_sent = state.notification_sent.get();
                        self.show_settings = false;
                        self.settings_viewport_data = None;
                        self.start_refresh();
                    }
                }
            }

            // Sync appearance changes from settings viewport in real-time
            #[cfg(windows)]
            if self.show_settings {
                if let Some(ref state) = self.settings_viewport_data {
                    let s = state.settings.borrow();
                    self.settings.theme = s.theme;
                    self.settings.widget_size = s.widget_size;
                    self.settings.show_percentage = s.show_percentage;
                }
            }

            match tray::tray::check_events() {
                Some(tray::tray::TrayCommand::Refresh) => {
                    debug_log!("Tray: refresh requested");
                    self.start_refresh();
                }
                None => {}
            }
        }

        // Dynamic window sizing
        let current_size = self.settings.widget_size;
        if current_size != self.last_widget_size {
            debug_log!(
                "Widget size changed: {:?} -> {:?}",
                self.last_widget_size,
                current_size
            );
            self.last_widget_size = current_size;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(current_size.window_size()));
        }

        // Theme visuals for widget
        let colors = self.settings.theme.colors();
        let mut visuals = ctx.style().visuals.clone();
        visuals.panel_fill = colors.bg_fill;
        visuals.window_fill = Color32::from_rgb(0, 255, 255);
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
        }

        if (self.animated_percent - target).abs() > 0.05 {
            ctx.request_repaint();
        } else if !self.refresh_in_progress {
            ctx.request_repaint_after(Duration::from_secs(1));
        }

        // ── Widget rendering ──
        let (pointer_pos, button_down, button_clicked, viewport_rect) = ctx.input(|i| {
            (
                i.pointer.interact_pos(),
                i.pointer.button_down(egui::PointerButton::Primary),
                i.pointer.button_clicked(egui::PointerButton::Primary),
                i.viewport().outer_rect,
            )
        });

        let widget_size = self.settings.widget_size.window_size();
        let cfg = self.settings.widget_size.config();
        let radius = cfg.circle_radius;
        let center = egui::pos2(2.0 + radius + cfg.stroke_width / 2.0, widget_size.y / 2.0);

        let circle_hovered = pointer_pos.map_or(false, |pos| pos.distance(center) <= radius);

        let show_tooltip =
            circle_hovered && !self.is_dragging && self.usage.as_ref().map_or(false, |u| !u.is_empty());

        let tooltip_height = if show_tooltip {
            let lines = self.usage.as_ref().map_or(0, |u| u.len());
            lines as f32 * 18.0 + 24.0
        } else {
            0.0
        };

        if show_tooltip != self.tooltip_expanded {
            self.tooltip_expanded = show_tooltip;
            let effective_h = widget_size.y + tooltip_height;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                widget_size.x,
                effective_h,
            )));
        }

        let hovered = circle_hovered;

        let layer_id = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("widget"));
        let painter = ctx.layer_painter(layer_id);

        let colors = self.settings.theme.colors();

        painter.circle_filled(center, radius, colors.circle_bg);

        let percent = self.animated_percent;
        if percent > 0.0 {
            let color = percent_color(percent);
            let start_angle = -std::f32::consts::FRAC_PI_2;
            let sweep = (percent as f32 / 100.0) * 2.0 * std::f32::consts::PI;
            let segments = 64;
            self.arc_points_cache.clear();
            let points: &[Pos2] = {
                self.arc_points_cache.extend((0..=segments).map(|i| {
                    let angle = start_angle + sweep * i as f32 / segments as f32;
                    center + radius * egui::vec2(angle.cos(), angle.sin())
                }));
                &self.arc_points_cache
            };
            painter.add(Shape::Path(PathShape {
                points: points.to_vec(),
                closed: false,
                fill: Color32::TRANSPARENT,
                stroke: Stroke::new(cfg.stroke_width, color).into(),
            }));
            painter.circle_filled(center, cfg.circle_center_dot, color);
        }

        if let Some(ref err) = self.error {
            let font_id = egui::FontId::proportional(cfg.error_font_size);
            let galley = ctx.fonts(|f| {
                f.layout(
                    err.clone(),
                    font_id,
                    Color32::from_rgb(244, 67, 54),
                    f32::INFINITY,
                )
            });
            let text_pos = egui::pos2(
                center.x + radius + 6.0,
                (widget_size.y - galley.size().y) / 2.0,
            );
            painter.galley(text_pos, galley, Color32::from_rgb(244, 67, 54));
        } else if self.settings.show_percentage {
            if let Some(pct) = self.get_monthly_percent() {
                let color = percent_color(pct);
                let text = format!("{:.1}%", pct);
                let font_id = egui::FontId::proportional(cfg.percent_font_size);
                let galley = ctx.fonts(|f| f.layout(text, font_id, color, f32::INFINITY));
                let text_pos = egui::pos2(
                    center.x + radius + 6.0,
                    (widget_size.y - galley.size().y) / 2.0,
                );
                painter.galley(text_pos, galley, color);
            }
        }

        if show_tooltip {
            if self.usage.is_some() {
                let tooltip_y = widget_size.y + 4.0;
                let tooltip_bg = egui::Rect::from_min_size(
                    egui::pos2(4.0, tooltip_y),
                    egui::vec2(widget_size.x - 8.0, tooltip_height - 8.0),
                );
                painter.rect_filled(tooltip_bg, 6.0, colors.bg_fill);

                let font_id = egui::FontId::proportional(12.0);
                let mut y = tooltip_y + 8.0;
                for (text, color) in &self.cached_level_lines {
                    let galley =
                        ctx.fonts(|f| f.layout(text.clone(), font_id.clone(), *color, f32::INFINITY));
                    painter.galley(egui::pos2(12.0, y), galley, *color);
                    y += 18.0;
                }
            }
        }

        if button_clicked && hovered {
            self.show_settings = true;
        }

        if button_down && (circle_hovered || self.is_dragging) {
            if let (Some(viewport_min), Some(current_pos)) =
                (viewport_rect.map(|r| r.min), pointer_pos)
            {
                let screen_mouse = viewport_min + current_pos.to_vec2();

                if !self.is_dragging {
                    self.is_dragging = true;
                    self.drag_offset = Some(screen_mouse - viewport_min);
                }

                if let Some(offset) = self.drag_offset {
                    let target = screen_mouse - offset;
                    let current = viewport_min;
                    let lerp_factor = 0.6;
                    let new_x = current.x + (target.x - current.x) * lerp_factor;
                    let new_y = current.y + (target.y - current.y) * lerp_factor;
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                        egui::Pos2::new(new_x, new_y),
                    ));
                }
            }
        }

        if self.is_dragging && !button_down {
            self.is_dragging = false;
            self.drag_offset = None;
            if let Some(rect) = viewport_rect {
                self.settings.window_x = Some(rect.min.x);
                self.settings.window_y = Some(rect.min.y);
                self.settings.save();
            }
        }

        if hovered && !self.was_hovered {
            if self.last_refresh.elapsed() >= HOVER_COOLDOWN {
                self.start_refresh();
            }
        }
        self.was_hovered = hovered;
    }
}

// ── Rendering ────────────────────────────────────────────────────────────────

impl WidgetApp {

    pub fn render_settings_viewport(
        ctx: &egui::Context,
        settings: &mut Settings,
        show: &mut bool,
        notif: &mut bool,
        tab: &mut usize,
        saved: &mut bool,
    ) {
        let accent = Color32::from_rgb(0x00, 0x78, 0xD4);
        let card_bg = Color32::from_rgb(0x2D, 0x2D, 0x32);
        let text_muted = Color32::from_rgb(0x99, 0x99, 0x9F);

        egui::CentralPanel::default().show(ctx, |ui| {
            // ── Tab bar ──
            let tab_labels = ["📋 通用", "🍪 Cookie"];
            let mut selected = *tab;
            ui.horizontal(|ui| {
                for (i, label) in tab_labels.iter().enumerate() {
                    let is_selected = selected == i;
                    let text = if is_selected {
                        egui::RichText::new(*label).color(Color32::WHITE).strong()
                    } else {
                        egui::RichText::new(*label).color(text_muted)
                    };
                    let resp = ui.selectable_label(is_selected, text);
                    if resp.clicked() {
                        selected = i;
                    }
                    ui.add_space(4.0);
                }
            });
            if selected != *tab {
                *tab = selected;
            }
            ui.add_space(8.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                        let section_gap = 12.0;

                        // ── Helper: draw a card-style section ──
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

                        if *tab == 0 {
                            // ── 通用 ──

                            // ── 区域 ──
                            section(ui, "🌐 区域", &mut |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("区域代码")
                                            .color(text_muted)
                                            .font(egui::FontId::proportional(12.0)),
                                    );
                                    ui.add_sized(
                                        egui::vec2(160.0, 20.0),
                                        egui::TextEdit::singleline(&mut settings.region),
                                    );
                                    ui.label(
                                        egui::RichText::new("  例如: openai")
                                            .color(text_muted)
                                            .font(egui::FontId::proportional(11.0)),
                                    );
                                });
                            });

                            // ── 刷新 ──
                            section(ui, "🔄 刷新", &mut |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("刷新间隔")
                                            .color(text_muted)
                                            .font(egui::FontId::proportional(12.0)),
                                    );
                                    let mut secs = settings.refresh_interval_secs;
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
                                        settings.refresh_interval_secs = secs;
                                    }
                                });
                            });

                            // ── 通知 ──
                            section(ui, "🔔 通知", &mut |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("用量阈值")
                                            .color(text_muted)
                                            .font(egui::FontId::proportional(12.0)),
                                    );
                                    let mut threshold = settings.notification_threshold;
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut threshold)
                                                .range(0.0..=100.0)
                                                .speed(1.0)
                                                .suffix(" %"),
                                        )
                                        .changed()
                                    {
                                        settings.notification_threshold = threshold;
                                    }
                                });
                                if settings.notification_threshold <= 0.0 {
                                    ui.label(
                                        egui::RichText::new("设为 0 可禁用通知")
                                            .color(text_muted)
                                            .font(egui::FontId::proportional(11.0)),
                                    );
                                }
                            });

                            // ── 外观 ──
                            section(ui, "🎨 外观", &mut |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("主题")
                                            .color(text_muted)
                                            .font(egui::FontId::proportional(12.0)),
                                    );
                                    let is_dark = matches!(settings.theme, Theme::Dark);
                                    if ui.selectable_label(is_dark, "🌙 暗色").clicked() {
                                        settings.theme = Theme::Dark;
                                    }
                                    if ui.selectable_label(!is_dark, "☀️ 亮色").clicked() {
                                        settings.theme = Theme::Light;
                                    }
                                });
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("窗口大小")
                                            .color(text_muted)
                                            .font(egui::FontId::proportional(12.0)),
                                    );
                                    let current = settings.widget_size;
                                    if ui
                                        .selectable_label(current == WidgetSize::Small, "小")
                                        .clicked()
                                    {
                                        settings.widget_size = WidgetSize::Small;
                                    }
                                    if ui
                                        .selectable_label(current == WidgetSize::Medium, "中")
                                        .clicked()
                                    {
                                        settings.widget_size = WidgetSize::Medium;
                                    }
                                    if ui
                                        .selectable_label(current == WidgetSize::Large, "大")
                                        .clicked()
                                    {
                                        settings.widget_size = WidgetSize::Large;
                                    }
                                });
                                ui.add_space(6.0);
                                if ui
                                    .checkbox(
                                        &mut settings.show_percentage,
                                        "显示百分比数字",
                                    )
                                    .changed()
                                {
                                }
                            });

                            // ── 其他 ──
                            section(ui, "⚙ 其他", &mut |ui| {
                                if ui
                                    .checkbox(&mut settings.auto_start, "开机自启")
                                    .changed()
                                {
                                }
                            });
                        } else {
                            // ── Cookie ──
                            section(ui, "🔑 凭证", &mut |ui| {
                                ui.label(
                                    egui::RichText::new("Cookie")
                                        .color(text_muted)
                                        .font(egui::FontId::proportional(12.0)),
                                );
                                ui.add_sized(
                                    egui::vec2(ui.available_width(), 80.0),
                                    egui::TextEdit::multiline(&mut settings.cookie)
                                        .hint_text("粘贴浏览器 Cookie..."),
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("CSRF Token")
                                        .color(text_muted)
                                        .font(egui::FontId::proportional(12.0)),
                                );
                                ui.add_sized(
                                    egui::vec2(ui.available_width(), 20.0),
                                    egui::TextEdit::singleline(&mut settings.csrf_token)
                                        .hint_text("粘贴 CSRF Token..."),
                                );
                            });

                            // ── 打开控制台 ──
                            ui.add_space(4.0);
                            ui.with_layout(
                                egui::Layout::top_down_justified(egui::Align::Center),
                                |ui| {
                                    let btn = egui::Button::new(
                                        egui::RichText::new("🌐 打开控制台")
                                            .color(Color32::WHITE)
                                            .font(egui::FontId::proportional(14.0)),
                                    )
                                    .fill(accent)
                                    .corner_radius(egui::CornerRadius::same(6))
                                    .min_size(egui::vec2(160.0, 36.0));
                                    if ui.add(btn).clicked() {
                                        let url = console_url(&settings.region);
                                        open_url(&url);
                                    }
                                },
                            );
                        }

                        // ── 保存按钮 ──
                        ui.add_space(4.0);
                        ui.with_layout(
                            egui::Layout::top_down_justified(egui::Align::Center),
                            |ui| {
                                let btn = egui::Button::new(
                                    egui::RichText::new("  保存并关闭  ")
                                        .color(Color32::WHITE)
                                        .font(egui::FontId::proportional(14.0)),
                                )
                                .fill(accent)
                                .corner_radius(egui::CornerRadius::same(6))
                                .min_size(egui::vec2(140.0, 36.0));
                                if ui.add(btn).clicked() {
                                    debug_log!("Settings: save & close clicked");
                                    settings.save();
                                    #[cfg(windows)]
                                    apply_auto_start(settings.auto_start);
                                    *notif = false;
                                    *saved = true;
                                    *show = false;
                                }
                            },
                        );
                    });
            });
    }
}
