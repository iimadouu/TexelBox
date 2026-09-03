use tbx_entitlements::secrets;
use tbx_entitlements::token::EntitlementToken;
mod cache_hmac;
pub use cache_hmac::{load_cache, save_cache, clear_cache, touch_heartbeat, CachedLicense};
pub use cache_hmac::is_trial_expired;
pub use cache_hmac::HEARTBEAT_INTERVAL_SECS;
fn server_error(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(status, resp) => {
            let msg = resp
                .into_json::<serde_json::Value>()
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str().map(String::from)))
                .unwrap_or_else(|| format!("HTTP status {status}"));
            format!("HTTP {status}: {msg}")
        }
        _ => e.to_string(),
    }
}
pub fn activate(email: &str, password: &str, license_key: &str) -> Result<EntitlementToken, String> {
    if cfg!(not(debug_assertions)) && tbx_entitlements::integrity::debugger_attached() {
        return Err("activation unavailable (integrity check)".to_string());
    }
    let base = secrets::server_url();
    let login = ureq::post(&format!("{base}/auth/login"))
        .send_json(serde_json::json!({ "email": email, "password": password }))
        .map_err(server_error)?
        .into_json::<serde_json::Value>()
        .map_err(|e| e.to_string())?;
    let session_token = login["session_token"]
        .as_str()
        .ok_or_else(|| "malformed login response".to_string())?
        .to_string();
    let validate = ureq::post(&format!("{base}/license/validate"))
        .send_json(serde_json::json!({
            "license_key": license_key,
            "device_id": secrets::device_id(),
            "session_token": session_token,
        }))
        .map_err(server_error)?
        .into_json::<serde_json::Value>()
        .map_err(|e| e.to_string())?;
    let wire = validate["token"]
        .as_str()
        .ok_or_else(|| "malformed validate response".to_string())?
        .to_string();
    let token =
        EntitlementToken::parse(&wire).map_err(|e| format!("server issued an invalid token: {e}"))?;
    save_cache(&CachedLicense {
        email: email.to_string(),
        license_key: license_key.to_string(),
        session_token,
        token_wire: wire,
        trial_expires_at: None,
    })
    .map_err(|e| format!("activation succeeded but the license cache could not be saved: {e}"))?;
    Ok(token)
}
pub fn activate_trial() -> Result<EntitlementToken, String> {
    if cfg!(not(debug_assertions)) && tbx_entitlements::integrity::debugger_attached() {
        return Err("trial activation unavailable (integrity check)".to_string());
    }
    let base = secrets::server_url();
    let resp = ureq::post(&format!("{base}/auth/trial"))
        .send(std::io::empty())
        .map_err(server_error)?
        .into_json::<serde_json::Value>()
        .map_err(|e| e.to_string())?;
    let session_token = resp["session_token"]
        .as_str()
        .ok_or_else(|| "malformed trial response: no session_token".to_string())?
        .to_string();
    let wire = resp["token"]
        .as_str()
        .ok_or_else(|| "malformed trial response: no token".to_string())?
        .to_string();
    let email = resp["email"]
        .as_str()
        .unwrap_or("trial@texelbox.internal")
        .to_string();
    let license_key = resp["license_key"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let trial_expires_at = resp["trial_expires_at"].as_i64();
    let token = EntitlementToken::parse(&wire)
        .map_err(|e| format!("server issued an invalid trial token: {e}"))?;
    save_cache(&CachedLicense {
        email,
        license_key,
        session_token,
        token_wire: wire,
        trial_expires_at,
    })
    .map_err(|e| format!("trial activation succeeded but the license cache could not be saved: {e}"))?;
    Ok(token)
}
pub enum HeartbeatOutcome {
    Refreshed(EntitlementToken),
    Revoked,
    NoLicense,
    Error(String),
}
pub fn heartbeat() -> HeartbeatOutcome {
    let Some(cache) = load_cache() else {
        return HeartbeatOutcome::NoLicense;
    };
    let request_key = cache.license_key.clone();
    let request_session = cache.session_token.clone();
    let base = secrets::server_url();
    let resp = ureq::post(&format!("{base}/license/heartbeat")).send_json(serde_json::json!({
        "license_key": cache.license_key,
        "device_id": secrets::device_id(),
        "session_token": cache.session_token,
        "token": cache.token_wire,
    }));
    let resp = match resp {
        Ok(r) => r,
        Err(e) => return HeartbeatOutcome::Error(server_error(e)),
    };
    let v = match resp.into_json::<serde_json::Value>() {
        Ok(v) => v,
        Err(e) => return HeartbeatOutcome::Error(e.to_string()),
    };
    let still_current = load_cache()
        .is_some_and(|c| c.license_key == request_key && c.session_token == request_session);
    if v["revoked"].as_bool() == Some(true) {
        if still_current {
            clear_cache();
            return HeartbeatOutcome::Revoked;
        }
        return HeartbeatOutcome::Error(
            "heartbeat revoke ignored — license changed while the request was in flight".to_string(),
        );
    }
    let Some(wire) = v["token"].as_str() else {
        return HeartbeatOutcome::Error("malformed heartbeat response".to_string());
    };
    let token = match EntitlementToken::parse(wire) {
        Ok(t) => t,
        Err(e) => return HeartbeatOutcome::Error(format!("invalid token: {e}")),
    };
    if !still_current {
        return HeartbeatOutcome::Error(
            "heartbeat refresh ignored — license changed while the request was in flight".to_string(),
        );
    }
    let mut cache = cache;
    cache.token_wire = wire.to_string();
    if let Some(session_token) = v["session_token"].as_str() {
        if !session_token.is_empty() {
            cache.session_token = session_token.to_string();
        }
    }
    if token.claims.plan == tbx_entitlements::Plan::Trial {
        cache.trial_expires_at = Some(token.claims.expires_at);
    }
    if let Err(e) = save_cache(&cache) {
        return HeartbeatOutcome::Error(format!("token refreshed but cache save failed: {e}"));
    }
    if let Err(e) = touch_heartbeat() {
        return HeartbeatOutcome::Error(format!("heartbeat timestamp update failed: {e}"));
    }
    HeartbeatOutcome::Refreshed(token)
}
