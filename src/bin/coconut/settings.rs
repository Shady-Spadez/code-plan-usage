use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use coding_plan_widget_shared::{debug_log, log};
use coding_plan_widget_shared::theme::Theme;

// ── Settings ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CoconutSettings {
    /// JWT Bearer token for Authorization header
    pub authorization_token: String,
    #[serde(default)]
    pub window_x: Option<f32>,
    #[serde(default)]
    pub window_y: Option<f32>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub notification_threshold: f64,
    #[serde(default = "default_refresh_secs")]
    pub refresh_interval_secs: u64,
    #[serde(default)]
    pub theme: Theme,
    /// Monthly spend limit (default $10.00)
    #[serde(default = "default_spend_limit")]
    pub spend_limit: f64,
}

fn default_refresh_secs() -> u64 {
    300
}

fn default_spend_limit() -> f64 {
    10.0
}

impl Default for CoconutSettings {
    fn default() -> Self {
        Self {
            authorization_token: String::new(),
            window_x: None,
            window_y: None,
            auto_start: false,
            notification_threshold: 0.0,
            refresh_interval_secs: default_refresh_secs(),
            theme: Theme::Dark,
            spend_limit: default_spend_limit(),
        }
    }
}

impl CoconutSettings {
    pub fn is_configured(&self) -> bool {
        !self.authorization_token.is_empty()
    }

    pub fn load() -> Self {
        let path = settings_path();
        let settings = if !path.exists() {
            let defaults = Self::default();
            defaults.save_to_path(&path);
            defaults
        } else {
            Self::load_from_path(&path).unwrap_or_default()
        };
        if !settings.is_configured() {
            debug_log!("CoconutSettings::load: no authorization token configured");
        } else {
            debug_log!("CoconutSettings::load: authorization token found");
        }
        settings
    }

    fn load_from_path(path: &Path) -> Option<Self> {
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self) {
        self.save_to_path(&settings_path());
    }

    fn save_to_path(&self, path: &Path) {
        debug_log!("CoconutSettings: saving to {:?}", path);
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, &json) {
                    debug_log!("CoconutSettings: FAILED to write file: {}", e);
                }
            }
            Err(e) => {
                debug_log!("CoconutSettings: FAILED to serialize: {}", e);
            }
        }
    }
}

pub fn settings_path() -> PathBuf {
    log::exe_dir().join("coconut_settings.json")
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings_not_configured() {
        let s = CoconutSettings::default();
        assert!(!s.is_configured());
        assert!(s.authorization_token.is_empty());
        assert_eq!(s.spend_limit, 10.0);
    }

    #[test]
    fn test_configured_with_token() {
        let s = CoconutSettings {
            authorization_token: "Bearer test123".to_string(),
            ..Default::default()
        };
        assert!(s.is_configured());
    }

    #[test]
    fn test_json_roundtrip() {
        let s = CoconutSettings {
            authorization_token: "Bearer test".to_string(),
            window_x: Some(100.0),
            window_y: Some(200.0),
            auto_start: true,
            notification_threshold: 80.0,
            refresh_interval_secs: 600,
            theme: Theme::Light,
            spend_limit: 20.0,
        };
        let json = serde_json::to_string(&s).unwrap();
        let parsed: CoconutSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, s);
    }

    #[test]
    fn test_load_from_path_nonexistent_file() {
        let tmp = std::env::temp_dir().join("cpw_test_nonexistent.json");
        let _ = std::fs::remove_file(&tmp);
        assert!(CoconutSettings::load_from_path(&tmp).is_none());
    }

    #[test]
    fn test_save_to_path_and_load_roundtrip() {
        let tmp = std::env::temp_dir().join("cpw_test_roundtrip.json");
        let _ = std::fs::remove_file(&tmp);
        let original = CoconutSettings {
            authorization_token: "Bearer abc".to_string(),
            window_x: Some(42.0),
            window_y: Some(99.0),
            auto_start: true,
            notification_threshold: 75.0,
            refresh_interval_secs: 120,
            theme: Theme::Light,
            spend_limit: 15.0,
        };
        original.save_to_path(&tmp);
        let loaded = CoconutSettings::load_from_path(&tmp).unwrap();
        assert_eq!(loaded, original);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_save_defaults_to_path_and_load_roundtrip() {
        let tmp = std::env::temp_dir().join("cpw_test_first_launch.json");
        let _ = std::fs::remove_file(&tmp);

        assert!(!tmp.exists());
        let defaults = CoconutSettings::default();
        defaults.save_to_path(&tmp);
        assert!(tmp.exists());

        let loaded = CoconutSettings::load_from_path(&tmp).unwrap();
        assert_eq!(loaded, CoconutSettings::default());
        let _ = std::fs::remove_file(&tmp);
    }
}
