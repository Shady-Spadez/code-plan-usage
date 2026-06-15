#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod browser_cookies;
mod log;
mod theme;
mod api;
mod settings;
mod widget;
#[cfg(windows)]
mod tray;

use eframe::egui;
use std::time::Duration;

use crate::settings::Settings;
use crate::widget::WidgetApp;

// ── Constants ────────────────────────────────────────────────────────────────

const DEFAULT_REFRESH_INTERVAL: Duration = Duration::from_secs(300); // 5 minutes
const HOVER_COOLDOWN: Duration = Duration::from_secs(30); // min interval between hover refreshes

// ── Notifications ────────────────────────────────────────────────────────────

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

// ── URL Opener ───────────────────────────────────────────────────────────────

fn open_url(url: &str) {
    debug_log!("Opening URL: {}", url);
    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

        let url_wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            ShellExecuteW(
                None,
                PCWSTR::null(),
                PCWSTR::from_raw(url_wide.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOW,
            );
        }
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

// ── Auto-start ───────────────────────────────────────────────────────────────

#[cfg(windows)]
fn apply_auto_start(enabled: bool) {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{
        RegCreateKeyExW, RegSetValueExW, RegDeleteValueW, RegCloseKey,
        HKEY_CURRENT_USER, REG_SZ, REG_OPTION_NON_VOLATILE, KEY_WRITE, KEY_SET_VALUE,
    };

    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let exe_path_str = exe_path.to_string_lossy();

    let sub_key = HSTRING::from(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
    );
    let value_name = HSTRING::from("CodingPlanWidget");

    unsafe {
        if enabled {
            debug_log!("Enabling auto-start via registry");
            let mut hkey = std::mem::zeroed();
            if RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &sub_key,
                0,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut hkey,
                None,
            ).is_ok() {
                let data = HSTRING::from(exe_path_str.as_ref());
                let bytes = data.as_wide();
                let _ = RegSetValueExW(
                    hkey,
                    &value_name,
                    0,
                    REG_SZ,
                    Some(std::slice::from_raw_parts(
                        bytes.as_ptr() as *const u8,
                        bytes.len() * 2 + 2, // include null terminator
                    )),
                );
                let _ = RegCloseKey(hkey);
            }
        } else {
            debug_log!("Disabling auto-start via registry");
            let mut hkey = std::mem::zeroed();
            if RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &sub_key,
                0,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut hkey,
                None,
            ).is_ok() {
                let _ = RegDeleteValueW(hkey, &value_name);
                let _ = RegCloseKey(hkey);
            }
        }
    }
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() -> eframe::Result {
    log::init_logger();
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

// ── CJK Font ─────────────────────────────────────────────────────────────────

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
    use crate::api::{ApiResponse, QuotaLevel, format_level_line};
    use crate::theme::percent_color;
    use chrono::Utc;
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
