use std::sync::OnceLock;
use tbx_entitlements::secrets;
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
static DOWNLOAD_URL: OnceLock<String> = OnceLock::new();
#[derive(Debug, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub version: String,
    pub url: String,
}
pub fn check_latest() -> Result<ReleaseInfo, String> {
    let base = secrets::server_url();
    let v = ureq::get(&format!("{base}/app/version"))
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|e| e.to_string())?
        .into_json::<serde_json::Value>()
        .map_err(|e| e.to_string())?;
    let version = v["version"].as_str().ok_or("malformed version response")?.to_string();
    let url = v["url"].as_str().unwrap_or_default().to_string();
    Ok(ReleaseInfo { version, url })
}
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> [u32; 3] {
        let mut out = [0u32; 3];
        for (i, part) in s.split('.').take(3).enumerate() {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            out[i] = digits.parse().unwrap_or(0);
        }
        out
    };
    parse(latest) > parse(current)
}
pub fn remember_download_url(url: String) {
    let _ = DOWNLOAD_URL.set(url);
}
pub fn download_url() -> Option<&'static String> {
    DOWNLOAD_URL.get()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_comparison() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("0.1", "0.1.0"));
        assert!(is_newer("0.2.0-beta1", "0.1.5"));
    }
    #[test]
    fn current_version_parses() {
        let v = parse_count(APP_VERSION);
        assert!(v > 0, "Cargo version must carry a number");
    }
    fn parse_count(s: &str) -> usize {
        s.split('.').filter(|p| !p.is_empty()).count()
    }
}
