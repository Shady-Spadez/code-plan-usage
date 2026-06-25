use chrono::{NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use std::time::Duration;

use coding_plan_widget_shared::debug_log;

// ── API URL ──────────────────────────────────────────────────────────────────

const API_URL: &str = "https://dash.coconut.is/api/users/checkHasLlmKey";

// ── API Types ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct ApiResponse {
    pub result: CheckResult,
    pub status: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CheckResult {
    #[serde(rename = "hasKey")]
    #[allow(dead_code)]
    pub has_key: bool,
    #[serde(rename = "llmKey")]
    #[allow(dead_code)]
    pub llm_key: String,
    #[serde(rename = "dailyActivity")]
    pub daily_activity: DailyActivity,
    #[serde(rename = "key_id")]
    #[allow(dead_code)]
    pub key_id: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DailyActivity {
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
    #[allow(dead_code)]
    pub results: Vec<DailyResult>,
    pub metadata: ActivityMetadata,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct DailyResult {
    pub date: String,
    pub metrics: ActivityMetrics,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ActivityMetrics {
    pub spend: f64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cache_read_input_tokens: u64,
    #[allow(dead_code)]
    pub cache_creation_input_tokens: u64,
    pub total_tokens: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub api_requests: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ActivityMetadata {
    pub total_spend: f64,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_tokens: u64,
    pub total_api_requests: u64,
    pub total_successful_requests: u64,
    #[allow(dead_code)]
    pub total_failed_requests: u64,
    pub total_cache_read_input_tokens: u64,
    #[allow(dead_code)]
    pub total_cache_creation_input_tokens: u64,
}

// ── API Fetch ────────────────────────────────────────────────────────────────

/// Fetch usage data from the Coconut API.
/// Returns the DailyActivity which includes metadata for monthly totals.
pub fn fetch_usage(auth_token: &str) -> Result<DailyActivity, String> {
    const MAX_RETRIES: u32 = 3;
    let mut last_error = String::new();

    let auth_header = if auth_token.starts_with("Bearer ") {
        auth_token.to_string()
    } else {
        format!("Bearer {}", auth_token)
    };

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let base_ms = (1u64 << (attempt - 1)) * 1000; // 1000, 2000, 4000 ms
            let jitter = (base_ms / 4) as i64;
            let rand_offset = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as i64 % (jitter * 2 + 1))
                - jitter;
            let wait_ms = (base_ms as i64 + rand_offset).max(100) as u64;
            debug_log!(
                "fetch_usage retry {}/{}, waiting {}ms...",
                attempt, MAX_RETRIES, wait_ms
            );
            std::thread::sleep(Duration::from_millis(wait_ms));
        }

        let response = match ureq::get(API_URL)
            .timeout(Duration::from_secs(10))
            .set("Authorization", &auth_header)
            .set("Accept", "application/json")
            .call()
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

        if api_response.status != "ok" {
            return Err(format!("状态: {}", api_response.status));
        }

        return Ok(api_response.result.daily_activity);
    }

    Err(last_error)
}

/// Returns true when `end_date` (format "YYYY-MM-DD") represents a date whose
/// 23:59:59 UTC timestamp has already passed — i.e. the billing cycle has ended
/// and a fresh fetch should include the new cycle's data.
pub fn is_end_date_passed(end_date: &str) -> bool {
    let expiry = match NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(23, 59, 59))
    {
        Some(ndt) => Utc.from_utc_datetime(&ndt).timestamp(),
        None => {
            debug_log!("is_end_date_passed: failed to parse end_date '{}'", end_date);
            return false;
        }
    };
    expiry <= Utc::now().timestamp()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_response() {
        let json = include_str!("../../../response.txt");
        let api_response: ApiResponse = serde_json::from_str(json).expect("should parse");
        assert_eq!(api_response.status, "ok");
        assert!(api_response.result.has_key);
        assert_eq!(api_response.result.daily_activity.start_date, "2026-06-01");
        assert_eq!(api_response.result.daily_activity.end_date, "2026-06-30");

        let meta = &api_response.result.daily_activity.metadata;
        assert!(meta.total_spend > 0.0);
        assert!(meta.total_tokens > 0);
        assert!(meta.total_api_requests > 0);
    }

    #[test]
    fn test_metadata_values() {
        let json = include_str!("../../../response.txt");
        let api_response: ApiResponse = serde_json::from_str(json).expect("should parse");
        let meta = &api_response.result.daily_activity.metadata;

        assert_eq!(meta.total_spend, 0.6087960158);
        assert_eq!(meta.total_prompt_tokens, 43752057);
        assert_eq!(meta.total_completion_tokens, 239167);
        assert_eq!(meta.total_tokens, 43991224);
        assert_eq!(meta.total_api_requests, 660);
        assert_eq!(meta.total_successful_requests, 657);
        assert_eq!(meta.total_failed_requests, 3);
        assert_eq!(meta.total_cache_read_input_tokens, 42846336);
    }

    #[test]
    fn test_is_end_date_passed_past_date() {
        assert!(is_end_date_passed("2020-01-01"));
    }

    #[test]
    fn test_is_end_date_passed_future_date() {
        assert!(!is_end_date_passed("2099-12-31"));
    }

    #[test]
    fn test_is_end_date_passed_invalid_format() {
        assert!(!is_end_date_passed("not-a-date"));
    }

    #[test]
    fn test_is_end_date_passed_empty() {
        assert!(!is_end_date_passed(""));
    }
}
