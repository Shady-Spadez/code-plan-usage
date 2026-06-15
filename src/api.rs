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
