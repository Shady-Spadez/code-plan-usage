use serde::Deserialize;
use chrono::Utc;
use std::time::Duration;

use crate::debug_log;

// ── API URL helpers ───────────────────────────────────────────────────────────

pub fn api_url(region: &str) -> String {
    format!(
        "https://console.volcengine.com/api/top/ark/{}/2024-01-01/GetCodingPlanUsage",
        region
    )
}

pub fn console_url(region: &str) -> String {
    format!(
        "https://console.volcengine.com/ark/region:ark+{}/openManagement?LLM=%7B%7D&advancedActiveKey=subscribe",
        region
    )
}

// ── API Types ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct ApiResponse {
    #[serde(rename = "Result")]
    pub result: UsageResult,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UsageResult {
    #[serde(rename = "Status")]
    pub status: String,
    #[serde(rename = "QuotaUsage")]
    pub quota_usage: Vec<QuotaLevel>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct QuotaLevel {
    #[serde(rename = "Level")]
    pub level: String,
    #[serde(rename = "Percent")]
    pub percent: f64,
    #[serde(rename = "ResetTimestamp")]
    pub reset_timestamp: i64,
}

// ── Formatting ────────────────────────────────────────────────────────────────

pub fn format_level_line(level: &QuotaLevel) -> (&'static str, String) {
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

pub fn fetch_usage(cookie: &str, csrf_token: &str, region: &str) -> Result<Vec<QuotaLevel>, String> {
    const MAX_RETRIES: u32 = 3;
    let mut last_error = String::new();
    let url = api_url(region);
    let referer = console_url(region);

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let base_ms = (1u64 << (attempt - 1)) * 1000; // 1000, 2000, 4000 ms
            let jitter = (base_ms / 4) as i64; // ±25%
            // Simple pseudo-random using time
            let rand_offset = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as i64 % (jitter * 2 + 1)) - jitter;
            let wait_ms = (base_ms as i64 + rand_offset).max(100) as u64;
            debug_log!(
                "fetch_usage retry {}/{}, waiting {}ms...",
                attempt, MAX_RETRIES, wait_ms
            );
            std::thread::sleep(Duration::from_millis(wait_ms));
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

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
