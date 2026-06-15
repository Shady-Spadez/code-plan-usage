use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::debug_log;
use crate::theme::{Theme, WidgetSize};

// ── Settings ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Settings {
    pub cookie: String,
    pub csrf_token: String,
    #[serde(default)]
    pub show_percentage: bool,
    #[serde(default)]
    pub window_x: Option<f32>,
    #[serde(default)]
    pub window_y: Option<f32>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "default_region")]
    pub region: String,
    #[serde(default)]
    pub notification_threshold: f64,
    #[serde(default = "default_refresh_secs")]
    pub refresh_interval_secs: u64,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub widget_size: WidgetSize,
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
    pub fn is_configured(&self) -> bool {
        !self.cookie.is_empty() && !self.csrf_token.is_empty()
    }

    pub fn load() -> Self {
        // First, try to load existing settings from file to preserve
        // position, theme, size, and other user preferences.
        let mut settings = Self::load_from_file().unwrap_or_default();

        // If credentials are missing, try to extract from browser or cookie file.
        // Merge only the credential fields — don't overwrite other settings.
        if !settings.is_configured() {
            if let Some(creds) = crate::browser_cookies::try_extract_credentials() {
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

    pub fn save(&self) {
        let path = settings_path();
        debug_log!("Settings: saving to {:?}", path);
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, &json) {
                    debug_log!("Settings: FAILED to write file: {}", e);
                }
            }
            Err(e) => {
                debug_log!("Settings: FAILED to serialize: {}", e);
            }
        }
    }
}

pub fn settings_path() -> PathBuf {
    std::env::current_exe()
        .expect("无法获取当前可执行文件路径")
        .parent()
        .expect("无法获取可执行文件父目录")
        .join("coding_plan_settings.json")
}
