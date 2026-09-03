use std::sync::{Arc, RwLock};
use chrono::Utc;
use ed25519_dalek::{SigningKey, VerifyingKey};
use crate::capability::Capability;
use crate::plan::Plan;
use crate::secrets;
use crate::token::{EntitlementToken, TokenClaims, TokenError};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Allowed,
    Denied { reason: DenialReason },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DenialReason {
    NoValidToken,
    OfflineGraceExpired,
    RequiresPlan { required: Plan, current: Plan },
    ServerDenied,
}
#[derive(Clone)]
pub struct KeyRing {
    pub production: Option<VerifyingKey>,
    pub development: Option<VerifyingKey>,
}
impl KeyRing {
    pub fn embedded() -> Self {
        let production = if secrets::has_production_key() {
            VerifyingKey::from_bytes(&secrets::PRODUCTION_VERIFY_KEY).ok()
        } else {
            None
        };
        let development = if cfg!(debug_assertions) {
            Some(SigningKey::from_bytes(&secrets::DEV_SIGNING_KEY).verifying_key())
        } else {
            None
        };
        Self { production, development }
    }
    fn accepted(&self) -> Vec<VerifyingKey> {
        let mut keys = Vec::new();
        if let Some(k) = self.production {
            keys.push(k);
        }
        if cfg!(debug_assertions) {
            if let Some(k) = self.development {
                keys.push(k);
            }
        }
        keys
    }
}
#[derive(Clone, Debug)]
struct GateState {
    token: Option<VerifiedToken>,
}
#[derive(Clone, Debug)]
struct VerifiedToken {
    claims: TokenClaims,
    verified_at: i64,
}
fn usable(token: &VerifiedToken, now: i64) -> bool {
    !token.claims.expired() || now.saturating_sub(token.verified_at) <= OFFLINE_GRACE_SECS
}
pub const OFFLINE_GRACE_SECS: i64 = 2 * 24 * 3600;
pub struct EntitlementGate {
    state: Arc<RwLock<GateState>>,
    keys: KeyRing,
}
impl EntitlementGate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Arc::new(RwLock::new(GateState { token: None })),
            keys: KeyRing::embedded(),
        })
    }
    pub fn plan(&self) -> Plan {
        let state = self.state.read().unwrap();
        let now = Utc::now().timestamp();
        state
            .token
            .as_ref()
            .filter(|t| usable(t, now))
            .map(|t| t.claims.plan)
            .unwrap_or(Plan::Free)
    }
    pub fn has_valid_token(&self) -> bool {
        let state = self.state.read().unwrap();
        let now = Utc::now().timestamp();
        state.token.as_ref().is_some_and(|t| usable(t, now))
    }
    pub fn token_expires_at(&self) -> Option<i64> {
        self.state.read().unwrap().token.as_ref().map(|t| t.claims.expires_at)
    }
    pub fn check(&self, cap: Capability) -> Decision {
        let state = self.state.read().unwrap();
        let Some(token) = &state.token else {
            return Decision::Denied { reason: DenialReason::NoValidToken };
        };
        let now = Utc::now().timestamp();
        if token.claims.expired() && now.saturating_sub(token.verified_at) > OFFLINE_GRACE_SECS {
            return Decision::Denied { reason: DenialReason::OfflineGraceExpired };
        }
        if token.claims.denials.contains(&cap) {
            return Decision::Denied { reason: DenialReason::ServerDenied };
        }
        if token.claims.extra_grants.contains(&cap) {
            return Decision::Allowed;
        }
        let required = cap.required_plan();
        if token.claims.plan.satisfies(required) {
            Decision::Allowed
        } else {
            Decision::Denied { reason: DenialReason::RequiresPlan { required, current: token.claims.plan } }
        }
    }
    pub fn is_locked(&self, cap: Capability) -> bool {
        !matches!(self.check(cap), Decision::Allowed)
    }
    pub fn install_token(&self, token: &EntitlementToken) -> Result<(), TokenError> {
        self.install_token_with_cache_flag(token, false)
    }
    pub fn install_cached_token(&self, token: &EntitlementToken) -> Result<(), TokenError> {
        self.install_token_with_cache_flag(token, true)
    }
    fn install_token_with_cache_flag(
        &self,
        token: &EntitlementToken,
        from_cache: bool,
    ) -> Result<(), TokenError> {
        token.verify(&self.keys.accepted())?;
        if token.claims.expired() && !from_cache {
            return Err(TokenError::Expired);
        }
        let device_ok = token.claims.device_id.is_empty()
            || token.claims.device_id == secrets::device_id();
        if !device_ok {
            return Err(TokenError::DeviceMismatch);
        }
        let now = Utc::now().timestamp();
        let verified_at = if token.claims.expired() { token.claims.issued_at.min(now) } else { now };
        let mut state = self.state.write().unwrap();
        state.token = Some(VerifiedToken {
            claims: token.claims.clone(),
            verified_at,
        });
        Ok(())
    }
    pub fn clear_token(&self) {
        self.state.write().unwrap().token = None;
    }
    pub fn refresh_heartbeat(&self, token: &EntitlementToken) -> Result<(), TokenError> {
        self.install_token(token)
    }
    pub fn mint_dev_token(plan: Plan, ttl_secs: i64) -> Result<EntitlementToken, TokenError> {
        if !cfg!(debug_assertions) {
            return Err(TokenError::NoKey);
        }
        let sk = SigningKey::from_bytes(&secrets::DEV_SIGNING_KEY);
        let now = Utc::now().timestamp();
        let claims = TokenClaims {
            plan,
            expires_at: now + ttl_secs,
            issued_at: now,
            device_id: secrets::device_id(),
            extra_grants: vec![],
            denials: vec![],
        };
        Ok(EntitlementToken::sign(claims, &sk))
    }
}
impl Default for EntitlementGate {
    fn default() -> Self {
        Self {
            state: Arc::new(RwLock::new(GateState { token: None })),
            keys: KeyRing::embedded(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_cached_token_usable_within_grace() {
        let gate = EntitlementGate::default();
        let token = EntitlementGate::mint_dev_token(Plan::Pro, -3600).unwrap();
        gate.install_cached_token(&token).unwrap();
        assert_eq!(gate.check(Capability::MapsHighResolution), Decision::Allowed);
        assert_eq!(gate.plan(), Plan::Pro);
        assert!(gate.has_valid_token());
    }
    #[test]
    fn expired_token_rejected_from_live_install() {
        let gate = EntitlementGate::default();
        let token = EntitlementGate::mint_dev_token(Plan::Pro, -3600).unwrap();
        assert!(matches!(gate.install_token(&token), Err(TokenError::Expired)));
        assert_eq!(gate.plan(), Plan::Free);
    }
    #[test]
    fn grace_window_counts_from_issuance_not_cache_load() {
        let gate = EntitlementGate::default();
        let now = Utc::now().timestamp();
        let claims = TokenClaims {
            plan: Plan::Pro,
            expires_at: now - 3600,
            issued_at: now - OFFLINE_GRACE_SECS - 10,
            device_id: secrets::device_id(),
            extra_grants: vec![],
            denials: vec![],
        };
        let sk = SigningKey::from_bytes(&secrets::DEV_SIGNING_KEY);
        let token = EntitlementToken::sign(claims, &sk);
        gate.install_cached_token(&token).unwrap();
        assert_eq!(
            gate.check(Capability::MapsHighResolution),
            Decision::Denied { reason: DenialReason::OfflineGraceExpired }
        );
        assert_eq!(gate.plan(), Plan::Free);
        assert!(!gate.has_valid_token());
    }
}
