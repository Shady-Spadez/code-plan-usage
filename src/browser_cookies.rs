use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::Aes256Gcm;
use base64::Engine;
use rusqlite::Connection;
use std::path::PathBuf;

use crate::debug_log;

pub struct BrowserCredentials {
    pub cookie: String,
    pub csrf_token: String,
}

/// Try to extract credentials from installed browsers (Chrome, Edge).
/// Returns None if no browser has valid cookies for console.volcengine.com.
pub fn try_extract_credentials() -> Option<BrowserCredentials> {
    let profiles = find_browser_profiles();
    debug_log!("Found {} browser profile(s)", profiles.len());
    for profile_dir in &profiles {
        debug_log!("Trying profile: {:?}", profile_dir);
        if let Some(creds) = extract_from_profile(profile_dir) {
            return Some(creds);
        }
    }
    debug_log!("No profile had valid cookies");
    None
}

fn find_browser_profiles() -> Vec<PathBuf> {
    let mut profiles = Vec::new();

    #[cfg(target_os = "windows")]
    {
        let local_app_data = match std::env::var("LOCALAPPDATA") {
            Ok(v) => v,
            Err(_) => {
                debug_log!("LOCALAPPDATA not set");
                return Vec::new();
            }
        };

        let chrome = PathBuf::from(&local_app_data)
            .join("Google")
            .join("Chrome")
            .join("User Data");
        if chrome.exists() {
            debug_log!("Chrome profile found: {:?}", chrome);
            profiles.push(chrome);
        } else {
            debug_log!("Chrome profile not found at {:?}", chrome);
        }

        let edge = PathBuf::from(&local_app_data)
            .join("Microsoft")
            .join("Edge")
            .join("User Data");
        if edge.exists() {
            debug_log!("Edge profile found: {:?}", edge);
            profiles.push(edge);
        } else {
            debug_log!("Edge profile not found at {:?}", edge);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs_next_home() {
            let chrome = home
                .join("Library")
                .join("Application Support")
                .join("Google")
                .join("Chrome");
            if chrome.exists() {
                debug_log!("Chrome profile found: {:?}", chrome);
                profiles.push(chrome);
            } else {
                debug_log!("Chrome profile not found at {:?}", chrome);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs_next_home() {
            let chrome = home.join(".config").join("google-chrome");
            if chrome.exists() {
                debug_log!("Chrome profile found: {:?}", chrome);
                profiles.push(chrome);
            } else {
                debug_log!("Chrome profile not found at {:?}", chrome);
            }
        }
    }

    profiles
}

/// Helper to get the user's home directory without adding a full dependency.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn dirs_next_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

fn extract_from_profile(user_data: &PathBuf) -> Option<BrowserCredentials> {
    debug_log!("Getting encryption key from {:?}", user_data);
    let key = match get_encryption_key(user_data) {
        Some(k) => {
            debug_log!("Encryption key obtained, length: {} bytes", k.len());
            k
        }
        None => {
            debug_log!("Failed to get encryption key");
            return None;
        }
    };

    // Try common profile directories
    let profile_names = ["Default", "Profile 1", "Profile 2", "Profile 3"];

    for profile_name in &profile_names {
        let cookies_db = user_data.join(profile_name).join("Network").join("Cookies");

        if cookies_db.exists() {
            debug_log!("Cookies DB found: {:?}", cookies_db);
            match extract_cookies_from_db(&cookies_db, &key) {
                Some(creds) => {
                    debug_log!(
                        "Extracted {} cookie pairs, csrf_token present: {}",
                        creds.cookie.split(';').count(),
                        !creds.csrf_token.is_empty()
                    );
                    return Some(creds);
                }
                None => {
                    debug_log!("No matching cookies in this DB");
                }
            }
        } else {
            debug_log!("No Cookies DB at {:?}", cookies_db);
        }
    }

    None
}

/// Read the AES encryption key from the browser's Local State file,
/// decrypting it via Windows DPAPI.
fn get_encryption_key(user_data: &PathBuf) -> Option<Vec<u8>> {
    let local_state_path = user_data.join("Local State");
    debug_log!("Reading Local State: {:?}", local_state_path);
    let content = std::fs::read_to_string(&local_state_path).ok()?;

    let state: serde_json::Value = serde_json::from_str(&content).ok()?;

    let encrypted_key_b64 = state.get("os_crypt")?.get("encrypted_key")?.as_str()?;
    debug_log!(
        "Found encrypted_key in Local State (len: {})",
        encrypted_key_b64.len()
    );

    let encrypted_key = base64::engine::general_purpose::STANDARD
        .decode(encrypted_key_b64)
        .ok()?;
    debug_log!("Decoded encrypted_key, {} bytes", encrypted_key.len());

    // Chrome/Edge prepend "DPAPI" (5 bytes) to the key
    if encrypted_key.len() <= 5 || &encrypted_key[..5] != b"DPAPI" {
        debug_log!("encrypted_key missing DPAPI prefix");
        return None;
    }

    debug_log!("Calling DPAPI decrypt on {} bytes", encrypted_key.len() - 5);
    let result = dpapi_decrypt(&encrypted_key[5..]);
    if result.is_some() {
        debug_log!("DPAPI decrypt succeeded");
    } else {
        debug_log!("DPAPI decrypt FAILED");
    }
    result
}

#[cfg(windows)]
fn dpapi_decrypt(data: &[u8]) -> Option<Vec<u8>> {
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    let blob_in = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };

    let mut blob_out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    unsafe {
        CryptUnprotectData(&blob_in, None, None, None, None, 0, &mut blob_out).ok()?;

        let result = std::slice::from_raw_parts(blob_out.pbData, blob_out.cbData as usize).to_vec();

        // Free the memory allocated by CryptUnprotectData
        let _ = windows::Win32::Foundation::LocalFree(windows::Win32::Foundation::HLOCAL(
            blob_out.pbData as *mut std::ffi::c_void,
        ));

        Some(result)
    }
}

#[cfg(not(windows))]
fn dpapi_decrypt(_data: &[u8]) -> Option<Vec<u8>> {
    None
}

/// Open the browser's Cookies SQLite database and extract cookies for
/// console.volcengine.com.
fn extract_cookies_from_db(db_path: &PathBuf, key: &[u8]) -> Option<BrowserCredentials> {
    // Copy to temp to avoid lock conflicts with a running browser
    let tmp_db = std::env::temp_dir().join(format!(
        "coding_plan_cookies_{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("系统时间异常")
            .as_millis()
    ));
    debug_log!("Copying Cookies DB to {:?}", tmp_db);

    // Retry up to 3 times if copy fails (browser may have a temporary lock)
    let mut copied = false;
    for attempt in 0..3 {
        if attempt > 0 {
            debug_log!("Retry copying Cookies DB, attempt {}", attempt + 1);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if std::fs::copy(db_path, &tmp_db).is_ok() {
            copied = true;
            break;
        }
    }
    if !copied {
        debug_log!("Failed to copy Cookies DB after 3 attempts");
        return None;
    }

    let conn = Connection::open(&tmp_db).ok()?;

    let mut stmt = conn
        .prepare("SELECT name, host_key, encrypted_value FROM cookies WHERE host_key LIKE '%volcengine%'")
        .ok()?;

    let mut cookie_pairs: Vec<String> = Vec::new();
    let mut csrf_token = String::new();

    let rows = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let host_key: String = row.get(1)?;
            let encrypted_value: Vec<u8> = row.get(2)?;
            Ok((name, host_key, encrypted_value))
        })
        .ok()?;

    let mut total = 0;
    let mut decrypted_count = 0;
    for row in rows {
        total += 1;
        if let Ok((name, host_key, encrypted_value)) = row {
            debug_log!(
                "Cookie '{}' (host: {}): encrypted {} bytes, prefix: {:?}",
                name,
                host_key,
                encrypted_value.len(),
                if encrypted_value.len() >= 3 {
                    String::from_utf8_lossy(&encrypted_value[..3.min(encrypted_value.len())])
                        .to_string()
                } else {
                    "N/A".to_string()
                }
            );
            if let Some(decrypted) = decrypt_cookie_value(&encrypted_value, key) {
                if decrypted.is_empty() {
                    debug_log!("Cookie '{}' decrypted but empty, skipping", name);
                    continue;
                }
                decrypted_count += 1;
                if name == "csrfToken" {
                    csrf_token = decrypted.clone();
                    debug_log!("Found csrfToken (len: {})", csrf_token.len());
                }
                cookie_pairs.push(format!("{}={}", name, decrypted));
            } else {
                debug_log!("FAILED to decrypt cookie '{}'", name);
            }
        }
    }

    debug_log!(
        "Total cookies for domain: {}, decrypted: {}",
        total,
        decrypted_count
    );

    // Clean up temp file
    let _ = std::fs::remove_file(&tmp_db);

    if cookie_pairs.is_empty() {
        debug_log!("No valid cookies extracted");
        return None;
    }

    let cookie = cookie_pairs.join("; ");
    debug_log!("Final cookie string length: {}", cookie.len());

    Some(BrowserCredentials { cookie, csrf_token })
}

/// Decrypt a single cookie value using AES-256-GCM.
/// Chrome/Edge use "v10" or "v11" prefix for encrypted values.
pub fn decrypt_cookie_value(encrypted: &[u8], key: &[u8]) -> Option<String> {
    // Format: prefix (3) + nonce (12) + ciphertext + tag (16 at end)
    if encrypted.len() < 3 + 12 + 16 {
        return None;
    }
    if &encrypted[..3] != b"v10" && &encrypted[..3] != b"v11" {
        return None;
    }

    let nonce = &encrypted[3..15];
    let ciphertext_with_tag = &encrypted[15..];

    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let nonce = aes_gcm::Nonce::from_slice(nonce);

    let plaintext = cipher.decrypt(nonce, ciphertext_with_tag).ok()?;
    String::from_utf8(plaintext).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decrypt_empty_input() {
        let result = decrypt_cookie_value(&[], &[0u8; 32]);
        assert!(result.is_none());
    }

    #[test]
    fn test_decrypt_v10_too_short() {
        // v10 prefix (3 bytes) but total length < 3 + 12 + 16 = 31
        let data = b"v10short";
        let result = decrypt_cookie_value(data, &[0u8; 32]);
        assert!(result.is_none());
    }

    #[test]
    fn test_decrypt_non_v10_prefix() {
        // Data without v10/v11 prefix should return None
        let data = b"plain_cookie_value";
        let result = decrypt_cookie_value(data, &[0u8; 32]);
        assert!(result.is_none());
    }
}
