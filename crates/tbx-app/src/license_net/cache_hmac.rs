use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tbx_entitlements::secrets;
const CACHE_FILE: &str = "license-cache.json";
#[derive(Clone, Serialize, Deserialize)]
pub struct CachedLicense {
    pub email: String,
    pub license_key: String,
    pub session_token: String,
    pub token_wire: String,
    pub trial_expires_at: Option<i64>,
}
pub const HEARTBEAT_INTERVAL_SECS: u64 = 6 * 3600;
const MAX_TIME_DEVIATION_SECS: i64 = 300;
static CACHE_LOCK: Mutex<()> = Mutex::new(());
#[derive(Clone, Serialize, Deserialize)]
struct SignedCache {
    data: CachedLicense,
    sig: String,
    last_heartbeat_at: i64,
    trial_started_at: Option<i64>,
}
impl SignedCache {
    fn compute_sig(data: &CachedLicense, last_hb: i64, trial_start: Option<i64>) -> String {
        use sha2::{Sha256, Digest};
        let key = Self::derive_key();
        let mut hasher = Sha256::new();
        hasher.update(&key);
        hasher.update(data.email.as_bytes());
        hasher.update(data.license_key.as_bytes());
        hasher.update(data.session_token.as_bytes());
        hasher.update(data.token_wire.as_bytes());
        if let Some(exp) = data.trial_expires_at {
            hasher.update(&exp.to_le_bytes());
        }
        hasher.update(&last_hb.to_le_bytes());
        if let Some(start) = trial_start {
            hasher.update(&start.to_le_bytes());
        }
        let result = hasher.finalize();
        result.iter().map(|b| format!("{:02x}", b)).collect()
    }
    fn derive_key() -> Vec<u8> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(b"texelbox-cache-v1");
        hasher.update(secrets::device_id().as_bytes());
        hasher.finalize().to_vec()
    }
    #[cfg(test)]
    fn new(data: CachedLicense, now: i64) -> Self {
        let trial_start = data.trial_expires_at.map(|_| now);
        let sig = Self::compute_sig(&data, now, trial_start);
        Self {
            data,
            sig,
            last_heartbeat_at: now,
            trial_started_at: trial_start,
        }
    }
    fn verify(&self, now: i64) -> bool {
        let expected_sig = Self::compute_sig(&self.data, self.last_heartbeat_at, self.trial_started_at);
        if !constant_time_eq(&self.sig, &expected_sig) {
            return false;
        }
        if now < self.last_heartbeat_at - MAX_TIME_DEVIATION_SECS {
            return false;
        }
        true
    }
    fn touch(&mut self, now: i64) {
        self.last_heartbeat_at = now;
        self.sig = Self::compute_sig(&self.data, self.last_heartbeat_at, self.trial_started_at);
    }
}
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (ca, cb) in a.bytes().zip(b.bytes()) {
        result |= ca ^ cb;
    }
    result == 0
}
fn cache_path() -> Option<std::path::PathBuf> {
    let dirs = directories::ProjectDirs::from("app", "TexelBox", "TexelBox")?;
    Some(dirs.config_dir().join(CACHE_FILE))
}
pub fn load_cache() -> Option<CachedLicense> {
    let path = cache_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    let signed: SignedCache = serde_json::from_str(&text).ok()?;
    let now = chrono::Utc::now().timestamp();
    if !signed.verify(now) {
        eprintln!("[tamper] cache verification failed — possible tampering detected");
        return None;
    }
    Some(signed.data)
}
pub fn save_cache(data: &CachedLicense) -> Result<(), String> {
    let _guard = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = cache_path().ok_or_else(|| "no OS config directory".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let now = chrono::Utc::now().timestamp();
    let trial_started = if let Some(existing) = load_signed_cache() {
        existing.trial_started_at
    } else {
        data.trial_expires_at.map(|_| now)
    };
    let mut signed = SignedCache {
        data: data.clone(),
        sig: String::new(),
        last_heartbeat_at: now,
        trial_started_at: trial_started,
    };
    signed.sig = SignedCache::compute_sig(&signed.data, now, trial_started);
    let text = serde_json::to_string_pretty(&signed).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}
fn load_signed_cache() -> Option<SignedCache> {
    let path = cache_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}
pub fn touch_heartbeat() -> Result<(), String> {
    let _guard = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = cache_path().ok_or_else(|| "no OS config directory".to_string())?;
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut signed: SignedCache = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    if !signed.verify(now) {
        return Err("cache verification failed".to_string());
    }
    signed.touch(now);
    let text = serde_json::to_string_pretty(&signed).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}
pub fn clear_cache() {
    let _guard = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(path) = cache_path() {
        let _ = std::fs::remove_file(path);
    }
}
pub fn trial_expires_at() -> Option<i64> {
    load_signed_cache().and_then(|s| s.data.trial_expires_at)
}
#[allow(dead_code)]
pub fn trial_started_at() -> Option<i64> {
    load_signed_cache().and_then(|s| s.trial_started_at)
}
pub fn is_trial_expired(server_remaining_secs: Option<i64>) -> bool {
    let now = chrono::Utc::now().timestamp();
    if let Some(expires_at) = trial_expires_at() {
        if now >= expires_at {
            return true;
        }
    }
    if let Some(remaining) = server_remaining_secs {
        if remaining <= 0 {
            return true;
        }
    }
    false
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_cache_signature_changes_with_data() {
        let data1 = CachedLicense {
            email: "test@example.com".to_string(),
            license_key: "TBX-TEST".to_string(),
            session_token: "sess1".to_string(),
            token_wire: "wire1".to_string(),
            trial_expires_at: None,
        };
        let data2 = CachedLicense {
            email: "test@example.com".to_string(),
            license_key: "TBX-TEST".to_string(),
            session_token: "sess2".to_string(),
            token_wire: "wire2".to_string(),
            trial_expires_at: Some(1234567890),
        };
        let cache1 = SignedCache::new(data1, 1000);
        let cache2 = SignedCache::new(data2, 1000);
        assert_ne!(cache1.sig, cache2.sig);
    }
    #[test]
    fn test_cache_verify_detects_tampering() {
        let data = CachedLicense {
            email: "test@example.com".to_string(),
            license_key: "TBX-TEST".to_string(),
            session_token: "sess1".to_string(),
            token_wire: "wire1".to_string(),
            trial_expires_at: Some(9999999999),
        };
        let mut cache = SignedCache::new(data, 1000);
        assert!(cache.verify(1000));
        cache.data.trial_expires_at = Some(9999999999999);
        assert!(!cache.verify(1000));
    }
    #[test]
    fn test_time_rollback_detected() {
        let data = CachedLicense {
            email: "test@example.com".to_string(),
            license_key: "TBX-TEST".to_string(),
            session_token: "sess1".to_string(),
            token_wire: "wire1".to_string(),
            trial_expires_at: None,
        };
        let cache = SignedCache::new(data, 100000);
        assert!(!cache.verify(100000 - MAX_TIME_DEVIATION_SECS - 1000));
    }
}
