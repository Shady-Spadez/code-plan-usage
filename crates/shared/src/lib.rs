//! Shared library for Coding Plan Widget family.
//!
//! Provides platform-neutral modules (log, screen, theme) and Windows-specific
//! helpers (tray, notifications, auto-start, CJK font) used by all binary targets.

pub mod log;
pub mod screen;
pub mod theme;

#[cfg(windows)]
pub mod tray;

pub mod widgets;

use eframe::egui;
use std::time::Duration;

// ── Constants ────────────────────────────────────────────────────────────────

pub const DEFAULT_REFRESH_INTERVAL: Duration = Duration::from_secs(300); // 5 minutes
pub const HOVER_COOLDOWN: Duration = Duration::from_secs(30); // min interval between hover refreshes
pub const RESET_REFRESH_COOLDOWN: Duration = Duration::from_secs(60); // min interval for reset-triggered refreshes

// ── Notification ─────────────────────────────────────────────────────────────

#[cfg(windows)]
pub fn show_usage_notification(app_title: &str, percent: f64, threshold: f64) {
    let title = app_title;
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
pub fn show_usage_notification(_app_title: &str, _percent: f64, _threshold: f64) {}

// ── Auto-start ───────────────────────────────────────────────────────────────

#[cfg(windows)]
pub fn apply_auto_start(reg_name: &str, enabled: bool) {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY_CURRENT_USER,
        KEY_SET_VALUE, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let exe_path_str = exe_path.to_string_lossy();

    let sub_key = HSTRING::from(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let value_name = HSTRING::from(reg_name);

    unsafe {
        if enabled {
            debug_log!("Enabling auto-start via registry for {}", reg_name);
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
            )
            .is_ok()
            {
                let data = HSTRING::from(exe_path_str.as_ref());
                let bytes = data.as_wide();
                let _ = RegSetValueExW(
                    hkey,
                    &value_name,
                    0,
                    REG_SZ,
                    Some(std::slice::from_raw_parts(
                        bytes.as_ptr() as *const u8,
                        bytes.len() * 2 + 2,
                    )),
                );
                let _ = RegCloseKey(hkey);
            }
        } else {
            debug_log!("Disabling auto-start via registry for {}", reg_name);
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
            )
            .is_ok()
            {
                let _ = RegDeleteValueW(hkey, &value_name);
                let _ = RegCloseKey(hkey);
            }
        }
    }
}

#[cfg(not(windows))]
pub fn apply_auto_start(_reg_name: &str, _enabled: bool) {}

// ── CJK Font ─────────────────────────────────────────────────────────────────

#[cfg(windows)]
pub fn setup_cjk_font(ctx: &egui::Context) {
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
pub fn setup_cjk_font(_ctx: &egui::Context) {}
