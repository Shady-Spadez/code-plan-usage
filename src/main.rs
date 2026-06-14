#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod browser_cookies;
mod log;
#[cfg(windows)]
mod tray;

use chrono::Utc;
use eframe::egui;
use egui::epaint::PathShape;
use egui::{Color32, Pos2, Shape, Stroke};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(windows)]
use eframe::glow::HasContext as _;
// ── Constants ────────────────────────────────────────────────────────────────

const DEFAULT_REFRESH_INTERVAL: Duration = Duration::from_secs(300); // 5 minutes
const HOVER_COOLDOWN: Duration = Duration::from_secs(30); // min interval between hover refreshes

// ── Theme ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
enum Theme {
    #[default]
    Dark,
    Light,
}

struct ThemeColors {
    bg_fill: Color32,
    text_secondary: Color32,
    circle_bg: Color32,
    widget_fg: Color32,
}

impl Theme {
    fn colors(&self) -> ThemeColors {
        match self {
            Theme::Dark => ThemeColors {
                bg_fill: Color32::from_rgba_premultiplied(30, 30, 35, 240),
                text_secondary: Color32::from_gray(150),
                circle_bg: Color32::from_gray(50),
                widget_fg: Color32::from_gray(220),
            },
            Theme::Light => ThemeColors {
                bg_fill: Color32::from_rgba_premultiplied(245, 245, 250, 240),
                text_secondary: Color32::from_gray(120),
                circle_bg: Color32::from_gray(210),
                widget_fg: Color32::from_gray(30),
            },
        }
    }
}

// ── Widget Size ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
enum WidgetSize {
    Small,
    #[default]
    Medium,
    Large,
}

struct SizeConfig {
    dimensions: egui::Vec2,
    circle_radius: f32,
    circle_center_dot: f32,
    stroke_width: f32,
    percent_font_size: f32,
    error_font_size: f32,
}

impl WidgetSize {
    fn config(&self) -> SizeConfig {
        match self {
            WidgetSize::Small => SizeConfig {
                dimensions: egui::vec2(180.0, 36.0),
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
                dimensions: egui::vec2(300.0, 60.0),
                circle_radius: 15.0,
                circle_center_dot: 4.0,
                stroke_width: 3.0,
                percent_font_size: 17.0,
                error_font_size: 12.0,
            },
        }
    }

    fn window_size(&self) -> egui::Vec2 {
        self.config().dimensions
    }
}

fn api_url(region: &str) -> String {
    format!(
        "https://console.volcengine.com/api/top/ark/{}/2024-01-01/GetCodingPlanUsage",
        region
    )
}

fn console_url(region: &str) -> String {
    format!(
        "https://console.volcengine.com/ark/region:ark+{}/openManagement?LLM=%7B%7D&advancedActiveKey=subscribe",
        region
    )
}

// ── Settings ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Settings {
    cookie: String,
    csrf_token: String,
    #[serde(default)]
    show_percentage: bool,
    #[serde(default)]
    window_x: Option<f32>,
    #[serde(default)]
    window_y: Option<f32>,
    #[serde(default)]
    auto_start: bool,
    #[serde(default = "default_region")]
    region: String,
    #[serde(default)]
    notification_threshold: f64,
    #[serde(default = "default_refresh_secs")]
    refresh_interval_secs: u64,
    #[serde(default)]
    theme: Theme,
    #[serde(default)]
    widget_size: WidgetSize,
}

fn default_region() -> String {
    "cn-beijing".to_string()
}

fn default_refresh_secs() -> u64 {
    300
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cookie: String::new(),
            csrf_token: String::new(),
            show_percentage: false,
            window_x: None,
            window_y: None,
            auto_start: false,
            region: default_region(),
            notification_threshold: 0.0,
            refresh_interval_secs: default_refresh_secs(),
            theme: Theme::Dark,
            widget_size: WidgetSize::Medium,
        }
    }
}

impl Settings {
    fn is_configured(&self) -> bool {
        !self.cookie.is_empty() && !self.csrf_token.is_empty()
    }

    fn load() -> Self {
        // First, try to load existing settings from file to preserve
        // position, theme, size, and other user preferences.
        let mut settings = Self::load_from_file().unwrap_or_default();

        // If credentials are missing, try to extract from browser or cookie file.
        // Merge only the credential fields — don't overwrite other settings.
        if !settings.is_configured() {
            if let Some(creds) = browser_cookies::try_extract_credentials() {
                debug_log!("Settings::load: extracted credentials from browser");
                settings.cookie = creds.cookie;
                settings.csrf_token = creds.csrf_token;
                settings.save();
            } else if let Some(cookie_settings) = Self::try_load_from_cookie_file() {
                debug_log!("Settings::load: loaded credentials from cookie file");
                settings.cookie = cookie_settings.cookie;
                settings.csrf_token = cookie_settings.csrf_token;
                settings.save();
            }
        } else {
            debug_log!("Settings::load: using existing credentials from settings file");
        }

        settings
    }

    fn load_from_file() -> Option<Self> {
        let path = settings_path();
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn try_load_from_cookie_file() -> Option<Self> {
        let path = settings_path()
            .parent()?
            .join("console.volcengine.com_cookies.txt");
        let content = std::fs::read_to_string(&path).ok()?;

        let mut cookie_pairs: Vec<String> = Vec::new();
        let mut csrf_token = String::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 7 {
                let name = parts[5];
                let value = parts[6];
                if name == "csrfToken" && csrf_token.is_empty() {
                    csrf_token = value.to_string();
                }
                cookie_pairs.push(format!("{}={}", name, value));
            }
        }

        if cookie_pairs.is_empty() {
            return None;
        }

        Some(Self {
            cookie: cookie_pairs.join("; "),
            csrf_token,
            ..Self::default()
        })
    }

    fn save(&self) {
        let path = settings_path();
        debug_log!("Settings: saving to {:?}", path);
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }
}

fn settings_path() -> PathBuf {
    std::env::current_exe()
        .expect("无法获取当前可执行文件路径")
        .parent()
        .expect("无法获取可执行文件父目录")
        .join("coding_plan_settings.json")
}

// ── API Types ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
struct ApiResponse {
    #[serde(rename = "Result")]
    result: UsageResult,
}

#[derive(Debug, Deserialize, Clone)]
struct UsageResult {
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "QuotaUsage")]
    quota_usage: Vec<QuotaLevel>,
}

#[derive(Debug, Deserialize, Clone)]
struct QuotaLevel {
    #[serde(rename = "Level")]
    level: String,
    #[serde(rename = "Percent")]
    percent: f64,
    #[serde(rename = "ResetTimestamp")]
    reset_timestamp: i64,
}

// ── Widget State ─────────────────────────────────────────────────────────────

/// Shared result type for background refresh threads.
type RefreshResult = Arc<Mutex<Option<Result<Vec<QuotaLevel>, String>>>>;

struct WidgetApp {
    settings: Settings,
    usage: Option<Vec<QuotaLevel>>,
    error: Option<String>,
    last_refresh: Instant,
    was_hovered: bool,
    is_dragging: bool,
    show_settings: bool,
    notification_sent: bool,
    animated_percent: f64,
    last_widget_size: WidgetSize,
    #[cfg(windows)]
    color_key_applied: bool,
    frame_count: u64,
    /// Offset from window origin to mouse at drag start (for lerp-based tracking).
    drag_offset: Option<egui::Vec2>,
    /// True while a background refresh thread is running.
    refresh_in_progress: bool,
    /// Shared handle to the background thread's result.
    pending_result: Option<RefreshResult>,
    /// Whether the hover tooltip is currently visible (window expanded).
    tooltip_expanded: bool,
    /// Shared state for the settings viewport (Rc<RefCell<>> so it persists across frames).
    #[cfg(windows)]
    settings_viewport_data: Option<(
        std::rc::Rc<std::cell::RefCell<Settings>>,
        std::rc::Rc<std::cell::Cell<bool>>,
    )>,
}

impl WidgetApp {
    fn with_settings(settings: Settings) -> Self {
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
            frame_count: 0,
            drag_offset: None,
            refresh_in_progress: false,
            pending_result: None,
            tooltip_expanded: false,
            #[cfg(windows)]
            settings_viewport_data: None,
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

        let result_arc: RefreshResult = Arc::new(Mutex::new(None));
        self.pending_result = Some(result_arc.clone());

        std::thread::spawn(move || {
            debug_log!("Background thread: calling fetch_usage...");
            let result = fetch_usage(&cookie, &csrf_token, &region);
            debug_log!(
                "Background thread: fetch_usage completed (success={})",
                result.is_ok()
            );
            if let Ok(mut guard) = result_arc.lock() {
                *guard = Some(result);
            }
        });
    }

    /// Check if a background refresh has completed and apply the result.
    /// Returns true if a result was applied.
    fn check_refresh_result(&mut self) -> bool {
        if !self.refresh_in_progress {
            return false;
        }
        if let Some(ref arc) = self.pending_result {
            // Take the result out of the mutex in a separate scope so the
            // MutexGuard is dropped before we modify self.pending_result.
            let maybe_result = {
                if let Ok(mut guard) = arc.lock() {
                    guard.take()
                } else {
                    None
                }
            };
            if let Some(result) = maybe_result {
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
        let speed = 0.12;
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
                    self.settings_viewport_data = Some((settings_rc, notif_rc));
                }

                if let Some((ref settings_rc, ref notif_rc)) = self.settings_viewport_data {
                    let viewport_id = egui::ViewportId::from_hash_of("settings");
                    let builder = egui::ViewportBuilder::default()
                        .with_title("⚙ 设置")
                        .with_inner_size([640.0, 720.0])
                        .with_resizable(false);

                    let s = settings_rc.clone();
                    let n = notif_rc.clone();
                    let should_close = std::rc::Rc::new(std::cell::Cell::new(false));
                    let sc = should_close.clone();

                    ctx.show_viewport_immediate(viewport_id, builder, move |ctx, _class| {
                        // Handle OS close button
                        if ctx.input(|i| i.viewport().close_requested()) {
                            sc.set(true);
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

                        let mut show = true;
                        let mut notif_val = n.get();
                        WidgetApp::render_settings_viewport(
                            ctx,
                            &mut s.borrow_mut(),
                            &mut show,
                            &mut notif_val,
                        );
                        n.set(notif_val);

                        if !show {
                            sc.set(true);
                        }

                        ctx.request_repaint();
                    });

                    if should_close.get() {
                        debug_log!("Settings: closed");
                        self.settings = settings_rc.borrow().clone();
                        self.notification_sent = notif_rc.get();
                        self.settings.save();
                        apply_auto_start(self.settings.auto_start);
                        self.show_settings = false;
                        self.settings_viewport_data = None;
                        self.start_refresh();
                    }
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
            lines as f32 * 18.0 + 16.0
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
            let points: Vec<Pos2> = (0..=segments)
                .map(|i| {
                    let angle = start_angle + sweep * i as f32 / segments as f32;
                    center + radius * egui::vec2(angle.cos(), angle.sin())
                })
                .collect();
            painter.add(Shape::Path(PathShape {
                points,
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
            if let Some(ref usage) = self.usage {
                let tooltip_y = widget_size.y + 4.0;
                let tooltip_bg = egui::Rect::from_min_size(
                    egui::pos2(4.0, tooltip_y),
                    egui::vec2(widget_size.x - 8.0, tooltip_height - 8.0),
                );
                painter.rect_filled(tooltip_bg, 6.0, colors.bg_fill);

                let font_id = egui::FontId::proportional(12.0);
                let mut y = tooltip_y + 8.0;
                for level in usage {
                    let (label, countdown) = format_level_line(level);
                    let color = percent_color(level.percent);
                    let text = if label.is_empty() {
                        format!("当前会话  {:.1}%  {}", level.percent, countdown)
                    } else {
                        format!("{}  {:.1}%  {}", label, level.percent, countdown)
                    };
                    let galley =
                        ctx.fonts(|f| f.layout(text, font_id.clone(), color, f32::INFINITY));
                    painter.galley(egui::pos2(12.0, y), galley, color);
                    y += 18.0;
                }
            }
        }

        if button_clicked && hovered {
            let url = console_url(&self.settings.region);
            open_url(&url);
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

    fn render_settings_viewport(
        ctx: &egui::Context,
        settings: &mut Settings,
        show: &mut bool,
        notif: &mut bool,
    ) {
        let mut needs_save = false;

        let accent = Color32::from_rgb(0x00, 0x78, 0xD4);
        let card_bg = Color32::from_rgb(0x2D, 0x2D, 0x32);
        let text_muted = Color32::from_rgb(0x99, 0x99, 0x9F);

        egui::CentralPanel::default().show(ctx, |ui| {
            // Header
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("⚙ 设置")
                    .color(Color32::WHITE)
                    .font(egui::FontId::proportional(18.0))
                    .strong(),
            );
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

                        // ── 凭证 ──
                        section(ui, "🔑 凭证", &mut |ui| {
                            ui.label(
                                egui::RichText::new("Cookie")
                                    .color(text_muted)
                                    .font(egui::FontId::proportional(12.0)),
                            );
                            let cookie_before = settings.cookie.clone();
                            ui.add_sized(
                                egui::vec2(ui.available_width(), 80.0),
                                egui::TextEdit::multiline(&mut settings.cookie)
                                    .hint_text("粘贴浏览器 Cookie..."),
                            );
                            if settings.cookie != cookie_before {
                                needs_save = true;
                            }
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("CSRF Token")
                                    .color(text_muted)
                                    .font(egui::FontId::proportional(12.0)),
                            );
                            let csrf_before = settings.csrf_token.clone();
                            ui.add_sized(
                                egui::vec2(ui.available_width(), 20.0),
                                egui::TextEdit::singleline(&mut settings.csrf_token)
                                    .hint_text("粘贴 CSRF Token..."),
                            );
                            if settings.csrf_token != csrf_before {
                                needs_save = true;
                            }
                        });

                        // ── 区域 ──
                        section(ui, "🌐 区域", &mut |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("区域代码")
                                        .color(text_muted)
                                        .font(egui::FontId::proportional(12.0)),
                                );
                                let region_before = settings.region.clone();
                                ui.add_sized(
                                    egui::vec2(160.0, 20.0),
                                    egui::TextEdit::singleline(&mut settings.region),
                                );
                                if settings.region != region_before {
                                    needs_save = true;
                                }
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
                                    needs_save = true;
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
                                    needs_save = true;
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
                                    needs_save = true;
                                }
                                if ui.selectable_label(!is_dark, "☀️ 亮色").clicked() {
                                    settings.theme = Theme::Light;
                                    needs_save = true;
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
                                    needs_save = true;
                                }
                                if ui
                                    .selectable_label(current == WidgetSize::Medium, "中")
                                    .clicked()
                                {
                                    settings.widget_size = WidgetSize::Medium;
                                    needs_save = true;
                                }
                                if ui
                                    .selectable_label(current == WidgetSize::Large, "大")
                                    .clicked()
                                {
                                    settings.widget_size = WidgetSize::Large;
                                    needs_save = true;
                                }
                            });
                        });

                        // ── 其他 ──
                        section(ui, "⚙ 其他", &mut |ui| {
                            if ui
                                .checkbox(&mut settings.auto_start, "开机自启")
                                .changed()
                            {
                                needs_save = true;
                            }
                            if ui
                                .checkbox(
                                    &mut settings.show_percentage,
                                    "显示百分比数字",
                                )
                                .changed()
                            {
                                needs_save = true;
                            }
                        });

                        // ── 保存按钮 ──
                        ui.add_space(4.0);
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
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
                                    *show = false;
                                }
                            },
                        );
                    });
            });

        // Save on every change
        if needs_save {
            debug_log!("Settings: auto-saving changes");
            settings.save();
            #[cfg(windows)]
            apply_auto_start(settings.auto_start);
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn percent_color(percent: f64) -> Color32 {
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

fn format_level_line(level: &QuotaLevel) -> (&'static str, String) {
    let now = Utc::now().timestamp();
    let reset_timestamp = if level.reset_timestamp > 1_000_000_000_000 {
        level.reset_timestamp / 1000
    } else {
        level.reset_timestamp
    };
    let remaining = (reset_timestamp - now).max(0) as u64;

    let days = remaining / 86400;
    let hours = (remaining % 86400) / 3600;
    let minutes = (remaining % 3600) / 60;

    let countdown = if days > 0 {
        format!("{}天{:02}时{:02}分钟后刷新", days, hours, minutes)
    } else {
        format!("{:02}时{:02}分钟后刷新", hours, minutes)
    };

    match level.level.as_str() {
        "session" => ("", countdown),
        "weekly" => ("近1周", format!("({})", countdown)),
        "monthly" => ("近1月", format!("({})", countdown)),
        other => {
            debug_log!("Unknown quota level: {}", other);
            ("未知", countdown)
        }
    }
}

// ── API Fetch ────────────────────────────────────────────────────────────────

fn fetch_usage(cookie: &str, csrf_token: &str, region: &str) -> Result<Vec<QuotaLevel>, String> {
    const MAX_RETRIES: u32 = 3;
    let mut last_error = String::new();
    let url = api_url(region);
    let referer = console_url(region);

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let wait_secs = 1u64 << (attempt - 1);
            debug_log!(
                "fetch_usage retry {}/{}, waiting {}s...",
                attempt,
                MAX_RETRIES,
                wait_secs
            );
            std::thread::sleep(Duration::from_secs(wait_secs));
        }

        let response = match ureq::post(&url)
            .timeout(Duration::from_secs(10))
            .set("Content-Type", "application/json")
            .set("Cookie", cookie)
            .set("X-Csrf-Token", csrf_token)
            .set("Origin", "https://console.volcengine.com")
            .set("Referer", &referer)
            .send_string("{}")
        {
            Ok(resp) => resp,
            Err(e) => {
                let err_msg = format!("请求失败: {}", e);
                debug_log!("fetch_usage attempt {} failed: {}", attempt, err_msg);
                last_error = err_msg;
                continue;
            }
        };

        if response.status() != 200 {
            return Err(format!("HTTP {}", response.status()));
        }

        let api_response: ApiResponse = response
            .into_json()
            .map_err(|e| format!("解析失败: {}", e))?;

        if api_response.result.status != "Running" {
            return Err(format!("状态: {}", api_response.result.status));
        }

        return Ok(api_response.result.quota_usage);
    }

    Err(last_error)
}

#[cfg(windows)]
fn show_usage_notification(percent: f64, threshold: f64) {
    let title = "Coding Plan Widget";
    let message = format!("用量已达 {:.1}%，超过阈值 {:.0}%", percent, threshold);
    let ps = format!(
        r#"Add-Type -AssemblyName System.Windows.Forms; $n=New-Object System.Windows.Forms.NotifyIcon; $n.Icon=[System.Drawing.Icon]::ExtractAssociatedIcon((Get-Command powershell).Path); $n.Visible=$true; $n.ShowBalloonTip(8000,'{}','{}','Warning'); Start-Sleep -Seconds 9; $n.Visible=$false; $n.Dispose()"#,
        title.replace('\'', "''"),
        message.replace('\'', "''")
    );
    debug_log!("Showing notification: {}", message);
    let _ = std::process::Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Command", &ps])
        .spawn();
}

#[cfg(not(windows))]
fn show_usage_notification(_percent: f64, _threshold: f64) {}

fn open_url(url: &str) {
    debug_log!("Opening URL: {}", url);
    #[cfg(target_os = "windows")]
    {
        // Use rundll32 url.dll which is more reliable than cmd /c start
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

// ── Main ─────────────────────────────────────────────────────────────────────

#[cfg(windows)]
fn apply_auto_start(enabled: bool) {
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let exe_path_str = exe_path.to_string_lossy();

    if enabled {
        debug_log!("Enabling auto-start via registry");
        let _ = std::process::Command::new("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                "CodingPlanWidget",
                "/t",
                "REG_SZ",
                "/d",
                &exe_path_str,
                "/f",
            ])
            .output();
    } else {
        debug_log!("Disabling auto-start via registry");
        let _ = std::process::Command::new("reg")
            .args([
                "delete",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                "CodingPlanWidget",
                "/f",
            ])
            .output();
    }
}

fn main() -> eframe::Result {
    debug_log!("=== Coding Plan Widget starting ===");
    #[cfg(windows)]
    tray::tray::init_tray();

    let settings = Settings::load();
    debug_log!("Settings loaded: configured={}", settings.is_configured());

    #[cfg(windows)]
    apply_auto_start(settings.auto_start);

    let initial_size = settings.widget_size.window_size();

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

#[cfg(windows)]
fn setup_cjk_font(ctx: &egui::Context) {
    let font_paths = [
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];

    for path in &font_paths {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "cjk".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            fonts
                .families
                .get_mut(&egui::FontFamily::Proportional)
                .unwrap()
                .insert(0, "cjk".to_owned());
            ctx.set_fonts(fonts);
            debug_log!("Loaded CJK font: {}", path);
            return;
        }
    }
    debug_log!("No CJK font found, Chinese text may show as boxes");
}

#[cfg(not(windows))]
fn setup_cjk_font(_ctx: &egui::Context) {}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_format_level_line_session_label_empty() {
        let now = Utc::now().timestamp();
        let level = QuotaLevel {
            level: "session".to_string(),
            percent: 42.0,
            reset_timestamp: now + 3600,
        };
        let (label, _countdown) = format_level_line(&level);
        assert_eq!(label, "");
    }

    #[test]
    fn test_format_level_line_weekly_label() {
        let now = Utc::now().timestamp();
        let level = QuotaLevel {
            level: "weekly".to_string(),
            percent: 42.0,
            reset_timestamp: now + 3600,
        };
        let (label, _countdown) = format_level_line(&level);
        assert_eq!(label, "近1周");
    }

    #[test]
    fn test_format_level_line_monthly_label() {
        let now = Utc::now().timestamp();
        let level = QuotaLevel {
            level: "monthly".to_string(),
            percent: 42.0,
            reset_timestamp: now + 3600,
        };
        let (label, _countdown) = format_level_line(&level);
        assert_eq!(label, "近1月");
    }

    #[test]
    fn test_format_level_line_countdown_format() {
        let now = Utc::now().timestamp();
        let reset = now + 86400 + 7200 + 180;
        let level = QuotaLevel {
            level: "session".to_string(),
            percent: 42.0,
            reset_timestamp: reset,
        };
        let (_label, countdown) = format_level_line(&level);
        assert!(countdown.contains("1天"));
        assert!(countdown.contains("02时"));
        assert!(countdown.contains("03分"));
    }

    #[test]
    fn test_format_level_line_future_timestamp() {
        let now = Utc::now().timestamp();
        let reset = now + 3600;
        let level = QuotaLevel {
            level: "session".to_string(),
            percent: 42.0,
            reset_timestamp: reset,
        };
        let (_label, countdown) = format_level_line(&level);
        assert!(!countdown.is_empty());
    }

    #[test]
    fn test_format_level_line_millisecond_timestamp() {
        let now = Utc::now().timestamp();
        let reset_ms = (now + 3600) * 1000;
        let level = QuotaLevel {
            level: "session".to_string(),
            percent: 42.0,
            reset_timestamp: reset_ms,
        };
        let (_label, countdown) = format_level_line(&level);
        assert!(countdown.contains("01时00分"));
    }

    #[test]
    fn test_format_level_line_expired_timestamp() {
        let now = Utc::now().timestamp();
        let level = QuotaLevel {
            level: "session".to_string(),
            percent: 42.0,
            reset_timestamp: now - 3600,
        };
        let (_label, countdown) = format_level_line(&level);
        assert!(countdown.contains("00时00分"));
    }

    #[test]
    fn test_api_response_deserialization() {
        let json = r#"{
            "Result": {
                "Status": "Running",
                "QuotaUsage": [
                    {
                        "Level": "session",
                        "Percent": 42.5,
                        "ResetTimestamp": 1700000000
                    },
                    {
                        "Level": "weekly",
                        "Percent": 65.0,
                        "ResetTimestamp": 1700086400
                    },
                    {
                        "Level": "monthly",
                        "Percent": 88.3,
                        "ResetTimestamp": 1704067200
                    }
                ]
            }
        }"#;

        let api_response: ApiResponse = serde_json::from_str(json).expect("should parse");
        assert_eq!(api_response.result.status, "Running");
        assert_eq!(api_response.result.quota_usage.len(), 3);

        let session = &api_response.result.quota_usage[0];
        assert_eq!(session.level, "session");
        assert_eq!(session.percent, 42.5);
        assert_eq!(session.reset_timestamp, 1700000000);

        let weekly = &api_response.result.quota_usage[1];
        assert_eq!(weekly.level, "weekly");
        assert_eq!(weekly.percent, 65.0);

        let monthly = &api_response.result.quota_usage[2];
        assert_eq!(monthly.level, "monthly");
        assert_eq!(monthly.percent, 88.3);
    }
}
