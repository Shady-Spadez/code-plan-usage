use eframe::egui;
use egui::epaint::PathShape;
use egui::{Color32, Pos2, Shape, Stroke};
use std::time::{Duration, Instant};

#[cfg(windows)]
use eframe::glow::HasContext as _;

use crate::api::{fetch_usage, format_level_line, is_reset_expired, QuotaLevel};
use crate::debug_log;
use crate::settings::Settings;
use crate::theme::{percent_color, widget_config, widget_window_size, Theme};
use crate::{DEFAULT_REFRESH_INTERVAL, HOVER_COOLDOWN, RESET_REFRESH_COOLDOWN, show_usage_notification};

#[cfg(windows)]
use crate::apply_auto_start;
#[cfg(windows)]
use crate::tray;

// ── Widget State ─────────────────────────────────────────────────────────────

/// Minimum mouse displacement (in logical px) before a button-press on the
/// circle is treated as a drag instead of a click. Prevents the window from
/// resizing (tooltip hide) during a pure click, which broke `button_clicked`.
const DRAG_THRESHOLD: f32 = 4.0;

/// Clamp a widget home position so the whole widget (size `ws`) stays on its
/// monitor and off the taskbar. Horizontally the widget may touch the screen
/// edges (uses the full monitor rect, not the work area, so it isn't kept away
/// from the left/right edges); vertically it's clamped to the work area to
/// avoid the taskbar. Falls back to the input position when screen info is
/// unavailable.
fn clamp_home_to_work_area(home: egui::Pos2, ws: egui::Vec2, ppp: f32) -> egui::Pos2 {
    match crate::screen::screen_info_for_point(home, ppp) {
        Some(info) => {
            // X: full monitor (widget can touch left/right screen edges).
            let x = home
                .x
                .clamp(info.monitor.min.x, (info.monitor.max.x - ws.x).max(info.monitor.min.x));
            // Y: work area (keep off the taskbar).
            let y = home
                .y
                .clamp(info.work_area.min.y, (info.work_area.max.y - ws.y).max(info.work_area.min.y));
            egui::pos2(x, y)
        }
        None => home,
    }
}

/// Shared state for the settings viewport (Rc<RefCell<>> so it persists across frames).
#[cfg(windows)]
pub struct SettingsViewportState {
    pub settings: std::rc::Rc<std::cell::RefCell<Settings>>,
    pub notification_sent: std::rc::Rc<std::cell::Cell<bool>>,
    pub tab_index: std::rc::Rc<std::cell::Cell<usize>>,
    pub original_settings: std::rc::Rc<std::cell::RefCell<Settings>>,
    pub confirm_dialog: std::rc::Rc<std::cell::Cell<bool>>,
    pub webview_receiver: std::rc::Rc<std::cell::RefCell<Option<std::sync::mpsc::Receiver<Option<crate::webview_login::BrowserCredentials>>>>>,
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
    #[cfg(windows)]
    pub color_key_applied: bool,
    #[cfg(debug_assertions)]
    pub frame_count: u64,
    /// Drag anchor: offset from the widget's screen position (home) to the mouse,
    /// captured at drag start. The widget is dragged by tracking `mouse - anchor`.
    pub drag_anchor: Option<egui::Vec2>,
    /// Mouse position (screen coords) captured when the primary button was
    /// pressed while hovering the circle. Dragging only engages after the mouse
    /// moves more than `DRAG_THRESHOLD` px from this point, so a pure click
    /// (press+release without moving) doesn't shrink the window / hide the
    /// tooltip mid-click (which used to break `button_clicked`).
    pub press_pos: Option<egui::Pos2>,
    /// Last known mouse position in screen coordinates, carried across frames.
    /// When the window is moved by tooltip geometry (no real mouse movement),
    /// winit does NOT emit a PointerMoved event, so the egui pointer stays stale
    /// (relative to the old window position). We recompute mouse_screen from this
    /// to keep hover/drag stable across window moves.
    pub last_mouse_screen: Option<egui::Pos2>,
    /// True while a background refresh thread is running.
    pub refresh_in_progress: bool,
    /// Receiver for the background thread's result (oneshot channel).
    pub pending_result: Option<std::sync::mpsc::Receiver<Result<Vec<QuotaLevel>, String>>>,
    /// Whether the hover tooltip is currently visible (window expanded).
    pub tooltip_expanded: bool,
    /// Widget home (screen) position captured while the tooltip is shown, so the
    /// widget stays put even though the OS window moves/resizes for top placement.
    pub tooltip_home: Option<egui::Pos2>,
    /// The last OuterPosition commanded to the OS window (to skip redundant sends).
    pub tooltip_last_pos: Option<egui::Pos2>,
    /// Last InnerSize sent for the tooltip window, to avoid redundant viewport cmds.
    pub tooltip_last_size: Option<egui::Vec2>,
    /// Shared state for the settings viewport.
    #[cfg(windows)]
    pub settings_viewport_data: Option<SettingsViewportState>,
    /// Pre-allocated buffer for arc segment points (reused each frame).
    pub arc_points_cache: Vec<egui::Pos2>,
    /// Pre-computed tooltip lines (updated on each successful refresh).
    pub cached_level_lines: Vec<(String, Color32)>,
    /// True while a silent WebView2 credential extraction is running.
    pub silent_reauth_in_progress: bool,
    /// Receiver for the silent reauth thread's result.
    pub silent_reauth_receiver: Option<std::sync::mpsc::Receiver<Option<crate::webview_login::BrowserCredentials>>>,
    /// True when the current refresh is a retry after silent reauth. If this
    /// retry also fails, we show "Cookie 过期" instead of looping.
    pub retry_after_reauth: bool,
    /// Timestamp of the last silent reauth attempt (for cooldown).
    pub last_silent_reauth: Option<Instant>,
    /// Lazily-loaded faint logo texture drawn on top of the circle background.
    pub logo_texture: Option<egui::TextureHandle>,
}

impl WidgetApp {
    pub fn with_settings(settings: Settings) -> Self {
        debug_log!("WidgetApp initialized: theme={:?}, auto_start={}, region={}, refresh_interval={}s, notification_threshold={}%",
            settings.theme, settings.auto_start, settings.region, settings.refresh_interval_secs, settings.notification_threshold);
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
            #[cfg(windows)]
            color_key_applied: false,
            #[cfg(debug_assertions)]
            frame_count: 0,
            drag_anchor: None,
            press_pos: None,
            last_mouse_screen: None,
            refresh_in_progress: false,
            pending_result: None,
            tooltip_expanded: false,
            tooltip_home: None,
            tooltip_last_pos: None,
            tooltip_last_size: None,
            #[cfg(windows)]
            settings_viewport_data: None,
            arc_points_cache: Vec::with_capacity(65),
            cached_level_lines: Vec::new(),
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

    /// Returns true when a quota's reset timestamp has already passed, meaning
    /// the quota should have been reset and a fresh fetch is worthwhile. A
    /// cooldown (`RESET_REFRESH_COOLDOWN`) prevents tight loops when the API
    /// keeps returning an already-past reset time.
    fn needs_reset_refresh(&self) -> bool {
        if self.refresh_in_progress {
            return false;
        }
        if self.last_refresh.elapsed() < RESET_REFRESH_COOLDOWN {
            return false;
        }
        self.usage
            .as_ref()
            .is_some_and(|usage| usage.iter().any(is_reset_expired))
    }

    /// Start a background refresh if one isn't already in progress.
    /// The network request runs on a separate thread so the UI never blocks.
    fn start_refresh(&mut self) {
        if self.refresh_in_progress {
            debug_log!("start_refresh: already in progress, skipping");
            return;
        }
        if self.silent_reauth_in_progress {
            debug_log!("start_refresh: silent reauth in progress, skipping");
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
                self.retry_after_reauth = false;
            }
            Err(e) => {
                debug_log!("API call FAILED: {}", e);

                // Clear old progress — stale data shouldn't be shown after a failure
                self.usage = None;
                self.cached_level_lines.clear();

                if self.retry_after_reauth {
                    // Retry after silent reauth also failed
                    self.retry_after_reauth = false;
                    if is_likely_cookie_error(&e) {
                        // Cookie error again — cookies are truly expired
                        self.error = Some("Cookie 过期，请重新登录".to_string());
                    } else {
                        // Different error (e.g. network) — show the actual error
                        self.error = Some(e);
                    }
                } else if is_likely_cookie_error(&e)
                    && !self.silent_reauth_in_progress
                    && self.can_start_silent_reauth()
                {
                    // Cookie-related error: silently try to refresh credentials from WebView2
                    debug_log!("apply_refresh_result: starting silent reauth for cookie error");
                    self.error = Some("正在重新获取凭证...".to_string());
                    self.start_silent_reauth();
                } else {
                    self.error = Some(e);
                }
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

    /// Cooldown for silent reauth attempts (matches default refresh interval).
    const SILENT_REAUTH_COOLDOWN: Duration = Duration::from_secs(300);

    /// Returns true if enough time has passed since the last silent reauth attempt.
    fn can_start_silent_reauth(&self) -> bool {
        match self.last_silent_reauth {
            Some(t) => t.elapsed() >= Self::SILENT_REAUTH_COOLDOWN,
            None => true,
        }
    }

    /// Start a silent WebView2 credential extraction in the background.
    fn start_silent_reauth(&mut self) {
        debug_log!("Starting silent reauth...");
        self.silent_reauth_in_progress = true;
        self.last_silent_reauth = Some(Instant::now());
        let rx = crate::webview_login::try_silent_extract_credentials();
        self.silent_reauth_receiver = Some(rx);
    }

    /// Check if the silent reauth has completed and apply the result.
    /// Returns true if a result was applied.
    fn check_silent_reauth_result(&mut self) -> bool {
        if !self.silent_reauth_in_progress {
            return false;
        }
        if let Some(ref rx) = self.silent_reauth_receiver {
            if let Ok(result) = rx.try_recv() {
                self.silent_reauth_in_progress = false;
                self.silent_reauth_receiver = None;

                if let Some(creds) = result {
                    debug_log!("Silent reauth: credentials obtained, retrying fetch");
                    self.settings.cookie = creds.cookie;
                    self.settings.csrf_token = creds.csrf_token;
                    self.settings.save();
                    self.retry_after_reauth = true;
                    self.start_refresh();
                } else {
                    debug_log!("Silent reauth: no credentials obtained");
                    self.error = Some("Cookie 过期，请重新登录".to_string());
                }
                return true;
            }
        }
        false
    }
}

/// Check whether an error string is likely caused by cookie expiration
/// (HTTP 401/403 or JSON parse failure — the latter happens when the API
/// redirects to an HTML login page).
fn is_likely_cookie_error(err: &str) -> bool {
    err.starts_with("HTTP 401") || err.starts_with("HTTP 403") || err.starts_with("解析失败")
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

        if self.check_silent_reauth_result() {
            ctx.request_repaint();
        }

        if self.refresh_in_progress || self.silent_reauth_in_progress {
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
                    let webview_receiver_rc = std::rc::Rc::new(std::cell::RefCell::new(None));
                    self.settings_viewport_data = Some(SettingsViewportState {
                        settings: settings_rc,
                        notification_sent: notif_rc,
                        tab_index: tab_rc,
                        original_settings: orig_rc,
                        confirm_dialog: confirm_rc,
                        webview_receiver: webview_receiver_rc,
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
                            if cf.get() {
                                // Already showing confirmation dialog - force close
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
                            let mut notif_val = n.get();
                            let mut tab_val = t.get();
                            let mut saved_val = sv.get();
                            let mut close_req = false;
                            WidgetApp::render_settings_viewport(
                                ctx,
                                &mut s.borrow_mut(),
                                &mut notif_val,
                                &mut tab_val,
                                &mut saved_val,
                                &mut close_req,
                                &state.webview_receiver,
                            );
                            n.set(notif_val);
                            t.set(tab_val);
                            sv.set(saved_val);
                            if close_req {
                                sc.set(true);
                            }
                        }

                        ctx.request_repaint();
                    });

                    // Poll webview login result
                    {
                        let mut rx_opt = state.webview_receiver.borrow_mut();
                        if let Some(ref rx) = *rx_opt {
                            if let Ok(result) = rx.try_recv() {
                                if let Some(creds) = result {
                                    debug_log!("WebView2: credentials received, updating settings");
                                    let mut s = state.settings.borrow_mut();
                                    s.cookie = creds.cookie;
                                    s.csrf_token = creds.csrf_token;
                                    s.save();
                                    saved.set(true);
                                    should_close.set(true);
                                } else {
                                    debug_log!("WebView2: login window closed without credentials");
                                }
                                *rx_opt = None;
                            }
                        }
                    }

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


        // Theme visuals for widget
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
                "Reset refresh triggered (quota reset time reached, elapsed: {}s)",
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
        let (pointer_pos, button_down, button_clicked, viewport_rect, has_pointer_event, has_pointer) =
            ctx.input(|i| {
                (
                    i.pointer.interact_pos(),
                    i.pointer.button_down(egui::PointerButton::Primary),
                    i.pointer.button_clicked(egui::PointerButton::Primary),
                    i.viewport().outer_rect,
                    i.events
                        .iter()
                        .any(|e| matches!(e, egui::Event::PointerMoved(_))),
                    i.pointer.has_pointer(),
                )
            });

        let widget_size = widget_window_size();
        let cfg = widget_config();
        let radius = cfg.circle_radius;
        let circle_x = 2.0 + radius + cfg.stroke_width / 2.0;

        // The widget's stable screen position ("home"). While the tooltip is
        // shown or the widget is being dragged we track this independently of the
        // OS window position (which moves around for top placement / drag), so the
        // widget stays put and dragging is decoupled from window-command lag.
        let cur_pos = viewport_rect.map(|r| r.min);
        let mut home = self.tooltip_home.or(cur_pos);

        // Mouse position in screen coordinates.
        // ⚠ When the window is moved by our own OuterPosition command, winit does
        // NOT emit a PointerMoved event (the mouse didn't physically move), so
        // `pointer_pos` stays stale (relative to the OLD window position). Using
        // `cur_pos + pointer` would give a wrong screen pos and break hover/drag
        // → this caused the top-placement freeze (hover oscillation 845↔919).
        // Fix: only trust `cur_pos + pointer` when a real PointerMoved event
        // arrived this frame; otherwise carry the last known screen mouse pos.
        //
        // ⚠ When the mouse leaves the window, egui fires `Event::PointerGone`
        // which clears `latest_pos` (and thus `has_pointer()`) immediately.
        // `interact_pos`, however, is sticky for one frame after PointerGone
        // (egui only clears it next frame), so relying on `pointer_pos` alone
        // would leave `circle_hovered` true on the exact frame the mouse exits
        // → the tooltip wouldn't hide on fast mouse-leaves. We therefore drop
        // the carried screen pos as soon as `has_pointer` is false.
        let mouse_screen = if !has_pointer {
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
                    // No real mouse event this frame: prefer the carried screen pos
                    // (it stays correct even when the window jumped). Fall back to the
                    // computed value only on the very first frame.
                    Some(self.last_mouse_screen.unwrap_or_else(|| c + pp.to_vec2()))
                }
                _ => self.last_mouse_screen,
            }
        };

        // Hover detection in screen coordinates.
        let circle_screen_center =
            home.map(|h| egui::pos2(h.x + circle_x, h.y + widget_size.y / 2.0));
        let circle_hovered = match (mouse_screen, circle_screen_center) {
            (Some(ms), Some(cc)) => ms.distance(cc) <= radius,
            _ => false,
        };

        // Note: do NOT hide the tooltip on `button_down`. Hiding it shrinks the
        // window (InnerSize command) during the click, which can make egui lose
        // the pointer release → button_clicked never fires → settings never
        // opens. Instead we keep the tooltip shown during the press; the click
        // is detected below and is_dragging handles actual drag motion.
        let show_tooltip = circle_hovered
            && !self.is_dragging
            && self.usage.as_ref().map_or(false, |u| !u.is_empty());

        // ── Tooltip sizing (natural content width, so left/right flip is visible) ──
        const TIP_PAD: f32 = 8.0; // inner padding (text inset)
        const TIP_LINE_H: f32 = 18.0; // line height
        const TIP_GAP: f32 = 4.0; // gap between widget and tooltip box
        const TIP_MIN_W: f32 = 40.0;

        let (tooltip_w, tooltip_h) = if show_tooltip {
            let font_id = egui::FontId::proportional(12.0);
            let max_text_w = self
                .cached_level_lines
                .iter()
                .map(|(text, color)| {
                    ctx.fonts(|f| {
                        f.layout(text.clone(), font_id.clone(), *color, f32::INFINITY)
                            .size()
                            .x
                    })
                })
                .fold(0.0_f32, f32::max);
            let w = (max_text_w + TIP_PAD * 2.0).max(TIP_MIN_W);
            let lines = self.usage.as_ref().map_or(0, |u| u.len());
            (w, lines as f32 * TIP_LINE_H + TIP_PAD * 2.0)
        } else {
            (0.0, 0.0)
        };

        // ── Placement: pick the first corner (in priority order) that fits ──
        // Priority: bottom-right → bottom-left → top-left → top-right.
        // Only computed while the tooltip is actually shown (avoids a per-frame
        // Win32 monitor query + log line when idle).
        //
        // tip_right semantics: true = tooltip's RIGHT edge aligns with the
        // widget's RIGHT edge (tooltip extends LEFT of the widget); false =
        // tooltip's LEFT edge aligns with the widget's LEFT edge (extends RIGHT).
        let (tip_top, tip_right, work_area) = if show_tooltip {
            let wa: Option<egui::Rect> = home.and_then(|h| {
                #[cfg(windows)]
                {
                    crate::screen::work_area_for_point(h, ctx.pixels_per_point())
                }
                #[cfg(not(windows))]
                {
                    None
                }
            });
            if let (Some(h), Some(area)) = (home, wa) {
                let tip_x_ofs = |right: bool| if right { widget_size.x - tooltip_w } else { 0.0 };
                let bottom_y = h.y + widget_size.y + TIP_GAP;
                let top_y = h.y - TIP_GAP - tooltip_h;
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
                let chosen = if fits(tip_rect(true, false)) {
                    (false, true)
                } else if fits(tip_rect(false, false)) {
                    (false, false)
                } else if fits(tip_rect(false, true)) {
                    (true, false)
                } else if fits(tip_rect(true, true)) {
                    (true, true)
                } else {
                    // Nothing fits cleanly (widget itself is off-screen, or the
                    // tooltip is wider than the work area). Pick the side with
                    // more room so clamping below keeps the tooltip on-screen.
                    let right_room = (area.max.x - (h.x + widget_size.x)).max(0.0);
                    let left_room = (h.x - area.min.x).max(0.0);
                    (false, left_room >= right_room)
                };
                (chosen.0, chosen.1, Some(area))
            } else {
                (false, true, None)
            }
        } else {
            (false, true, None)
        };

        // ── Drag: track the widget's screen position directly ──
        // The widget follows the mouse via home = mouse - anchor, independent of
        // the OS window's (lagging) position. This keeps dragging stable even
        // while tooltip geometry commands are in flight, and avoids the window
        // being tall/moved (which used to get OS-clamped near screen edges).
        // The widget is clamped to the monitor's work area so it can't be dragged
        // off-screen or onto the taskbar.
        //
        // Drag-vs-click: we capture the press position and only engage dragging
        // once the mouse moves beyond DRAG_THRESHOLD. A pure click (press+release
        // without moving) never sets is_dragging, so the tooltip stays shown and
        // the window doesn't resize mid-click (which used to break button_clicked
        // → settings wouldn't open).
        if button_down && (circle_hovered || self.is_dragging) {
            if let (Some(ms), Some(h)) = (mouse_screen, home) {
                // Capture press origin on the first frame the button is down.
                if self.press_pos.is_none() {
                    self.press_pos = Some(ms);
                }
                // Check if the mouse has moved beyond the drag threshold.
                let should_drag = self.is_dragging
                    || self
                        .press_pos
                        .map_or(false, |p| p.distance(ms) > DRAG_THRESHOLD);
                if should_drag {
                    if !self.is_dragging {
                        self.is_dragging = true;
                        // anchor = offset from widget home to mouse at drag start.
                        self.drag_anchor = Some(ms - h);
                    }
                    if let Some(anchor) = self.drag_anchor {
                        let new_home = ms - anchor;
                        // `home` is the widget's top-left corner and the widget
                        // itself only occupies `widget_size` (in egui points /
                        // logical px, the same space as the monitor rect from
                        // screen.rs and as `home`). Clamp with widget_size, NOT
                        // the actual OS window size: right after the tooltip was
                        // shown the window is still expanded to cover the tooltip
                        // (resize lag), so `outer_rect.size()` == widget_size +
                        // tooltip_width and clamping with it would subtract the
                        // tooltip width, leaving a gap between the widget and the
                        // right screen edge. The tooltip is clamped independently
                        // below (tip_screen_x), so it never needs to factor into
                        // the widget's own clamp.
                        let clamped = clamp_home_to_work_area(
                            new_home,
                            widget_size,
                            ctx.pixels_per_point(),
                        );
                        home = Some(clamped);
                        self.tooltip_home = Some(clamped);
                    }
                }
            }
        }

        // ── Desired window geometry ──
        // The tooltip's horizontal position is CLAMPED to the work area so it
        // stays fully on-screen even when the widget itself is at/past a screen
        // edge (the "nothing fits" fallback). The window is sized/positioned to
        // cover both the widget (at `home`) and the tooltip (at `tip_screen_x`).
        let full_h = widget_size.y + TIP_GAP + tooltip_h;

        // Preferred tooltip left edge (before clamping), in screen coords.
        let tip_pref_x = home
            .map(|h| if tip_right { h.x + widget_size.x - tooltip_w } else { h.x })
            .unwrap_or(0.0);
        // Clamp so the whole tooltip stays within the work area horizontally.
        let tip_screen_x = if show_tooltip {
            match work_area {
                Some(a) if tooltip_w <= (a.max.x - a.min.x).max(0.0) => {
                    tip_pref_x.clamp(a.min.x, a.max.x - tooltip_w)
                }
                _ => tip_pref_x,
            }
        } else {
            tip_pref_x
        };

        // Window spans [win_left, win_right] covering widget + tooltip.
        let (win_left, win_right) = if show_tooltip {
            let h = home.unwrap_or(egui::Pos2::ZERO);
            let wl = h.x.min(tip_screen_x);
            let wr = (h.x + widget_size.x).max(tip_screen_x + tooltip_w);
            (wl, wr)
        } else {
            let h = home.unwrap_or(egui::Pos2::ZERO);
            (h.x, h.x + widget_size.x)
        };
        let win_w = (win_right - win_left).max(0.0);
        let desired_size = if show_tooltip {
            egui::vec2(win_w, full_h)
        } else {
            widget_size
        };
        let desired_pos = if self.is_dragging {
            home
        } else if show_tooltip {
            home.map(|h| {
                let win_y = if tip_top {
                    h.y - TIP_GAP - tooltip_h
                } else {
                    h.y
                };
                egui::pos2(win_left, win_y)
            })
        } else {
            home
        };

        // Send InnerSize when it changes.
        if self.tooltip_last_size != Some(desired_size) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(desired_size));
            self.tooltip_last_size = Some(desired_size);
        }
        // Send OuterPosition when it changes (drag, top placement, restore).
        if let Some(dp) = desired_pos {
            if self.tooltip_last_pos.map_or(true, |lp| lp.distance(dp) > 0.5) {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(dp));
                self.tooltip_last_pos = Some(dp);
            }
        }

        // ── tooltip_home lifecycle ──
        if self.is_dragging {
            // updated above; kept as the widget's current screen position.
        } else if show_tooltip {
            if self.tooltip_home.is_none() {
                self.tooltip_home = cur_pos;
            }
        } else if let (Some(c), Some(h)) = (cur_pos, self.tooltip_home) {
            // Tooltip hidden & not dragging: clear home once the window has
            // settled back at home (handles the 1-frame restore lag without a
            // visual jump).
            if c.distance(h) < 1.0 {
                self.tooltip_home = None;
            }
        }
        self.tooltip_expanded = show_tooltip;

        // Drawing offsets are relative to the *current* (lagging) window position;
        // `widget_local_*` compensate so the widget appears at `home` on screen.
        let cur = cur_pos.unwrap_or(egui::Pos2::ZERO);
        let home_pos = home.unwrap_or(egui::Pos2::ZERO);
        // The widget's screen left = home.x; window's (commanded) left = win_left.
        // With lag, actual window left = cur.x, so widget local x = home.x - cur.x
        // (equivalently win_left + (home.x - win_left) - cur.x, and home.x - win_left
        // is constant, absorbed into the lag term).
        let widget_local_x = home_pos.x - cur.x;
        let widget_local_y = home_pos.y - cur.y;
        let center = egui::pos2(
            widget_local_x + circle_x,
            widget_local_y + widget_size.y / 2.0,
        );

        // Tooltip local position = its screen pos - current window pos.
        let tip_local = if show_tooltip {
            let tip_screen_y = if tip_top {
                home_pos.y - TIP_GAP - tooltip_h
            } else {
                home_pos.y + widget_size.y + TIP_GAP
            };
            Some(egui::pos2(tip_screen_x - cur.x, tip_screen_y - cur.y))
        } else {
            None
        };

        let hovered = circle_hovered;

        let layer_id = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("widget"));
        let painter = ctx.layer_painter(layer_id);

        let colors = self.settings.theme.colors();

        painter.circle_filled(center, radius, colors.circle_bg);

        // Faint platform logo watermark on the circle background.
        if self.logo_texture.is_none() {
            let bytes = include_bytes!("../assets/logo.png");
            if let Ok(img) = image::load_from_memory(bytes) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                self.logo_texture = Some(ctx.load_texture(
                    "volc-logo",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
            } else {
                debug_log!("Failed to decode embedded logo.png");
            }
        }
        if let Some(ref tex) = self.logo_texture {
            let logo_side = radius * 1.3;
            let logo_rect = egui::Rect::from_center_size(center, egui::vec2(logo_side, logo_side));
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            let tint = Color32::from_rgba_unmultiplied(
                colors.widget_fg.r(),
                colors.widget_fg.g(),
                colors.widget_fg.b(),
                40,
            );
            painter.image(tex.id(), logo_rect, uv, tint);
        }

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
                widget_local_y + (widget_size.y - galley.size().y) / 2.0,
            );
            painter.galley(text_pos, galley, Color32::from_rgb(244, 67, 54));
        } else {
            if let Some(pct) = self.get_monthly_percent() {
                let color = percent_color(pct);
                let text = format!("{:.1}%", pct);
                let font_id = egui::FontId::proportional(cfg.percent_font_size);
                let galley = ctx.fonts(|f| f.layout(text, font_id, color, f32::INFINITY));
                let text_pos = egui::pos2(
                    center.x - galley.size().x / 2.0,
                    center.y - galley.size().y / 2.0,
                );
                painter.galley(text_pos, galley, color);
            }
        }

        if let Some(tl) = tip_local {
            if self.usage.is_some() && tooltip_w > 0.0 {
                let tooltip_bg = egui::Rect::from_min_size(tl, egui::vec2(tooltip_w, tooltip_h));
                painter.rect_filled(tooltip_bg, 6.0, colors.bg_fill);

                let font_id = egui::FontId::proportional(12.0);
                let mut y = tl.y + TIP_PAD;
                for (text, color) in &self.cached_level_lines {
                    let galley =
                        ctx.fonts(|f| f.layout(text.clone(), font_id.clone(), *color, f32::INFINITY));
                    painter.galley(egui::pos2(tl.x + TIP_PAD, y), galley, *color);
                    y += TIP_LINE_H;
                }
            }
        }

        if button_clicked && hovered {
            self.show_settings = true;
        }

        // Clear press tracking when the button is released. If we were dragging,
        // persist the new position; for a pure click (never dragged) just clear
        // press_pos without a disk write.
        if !button_down && (self.is_dragging || self.press_pos.is_some()) {
            let was_dragging = self.is_dragging;
            self.is_dragging = false;
            self.drag_anchor = None;
            self.press_pos = None;
            if was_dragging {
                if let Some(h) = home {
                    let changed = self
                        .settings
                        .window_x
                        .map_or(true, |x| (x - h.x).abs() > 0.5)
                        || self
                            .settings
                            .window_y
                            .map_or(true, |y| (y - h.y).abs() > 0.5);
                    if changed {
                        self.settings.window_x = Some(h.x);
                        self.settings.window_y = Some(h.y);
                        self.settings.save();
                    }
                }
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
        notif: &mut bool,
        tab: &mut usize,
        saved: &mut bool,
        close_requested: &mut bool,
        webview_receiver: &std::rc::Rc<std::cell::RefCell<Option<std::sync::mpsc::Receiver<Option<crate::webview_login::BrowserCredentials>>>>>,
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
                                        if webview_receiver.borrow().is_none() {
                                            debug_log!("Settings: open console (WebView2 login) clicked");
                                            let rx = crate::webview_login::try_extract_credentials();
                                            webview_receiver.borrow_mut().replace(rx);
                                        }
                                    }
                                },
                            );

                            // ── 清理 Cookie ──
                            ui.add_space(4.0);
                            ui.with_layout(
                                egui::Layout::top_down_justified(egui::Align::Center),
                                |ui| {
                                    let btn = egui::Button::new(
                                        egui::RichText::new("🗑 清理 Cookie")
                                            .color(Color32::WHITE)
                                            .font(egui::FontId::proportional(14.0)),
                                    )
                                    .fill(Color32::from_rgb(0xD4, 0x3F, 0x3F))
                                    .corner_radius(egui::CornerRadius::same(6))
                                    .min_size(egui::vec2(160.0, 36.0));
                                    if ui.add(btn).clicked() {
                                        debug_log!("Settings: clear cookie clicked");
                                        settings.cookie.clear();
                                        settings.csrf_token.clear();
                                        settings.save();
                                        *saved = true;
                                        crate::webview_login::clear_webview_cookies();
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
                                    *close_requested = true;
                                }
                            },
                        );
                    });
            });
    }
}
