use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use crate::capability::Capability;
use crate::plan::Plan;
#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("malformed token")]
    Malformed,
    #[error("invalid claim payload: {0}")]
    BadClaims(String),
    #[error("signature verification failed")]
    BadSignature,
    #[error("token expired")]
    Expired,
    #[error("no verifying key available")]
    NoKey,
    #[error("token is for a different device")]
    DeviceMismatch,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub plan: Plan,
    pub expires_at: i64,
    pub issued_at: i64,
    pub device_id: String,
    #[serde(default)]
    pub extra_grants: Vec<Capability>,
    #[serde(default)]
    pub denials: Vec<Capability>,
}
impl TokenClaims {
    pub fn expired(&self) -> bool {
        Utc::now().timestamp() > self.expires_at
    }
}
#[derive(Clone)]
pub struct EntitlementToken {
    pub claims: TokenClaims,
    raw_claims: Vec<u8>,
    signature: Vec<u8>,
}
impl EntitlementToken {
    pub fn parse(wire: &str) -> Result<Self, TokenError> {
        let (claims_b64, sig_b64) = wire.split_once('.').ok_or(TokenError::Malformed)?;
        let raw_claims = URL_SAFE_NO_PAD
            .decode(claims_b64.trim())
            .map_err(|_| TokenError::Malformed)?;
        let signature = URL_SAFE_NO_PAD
            .decode(sig_b64.trim())
            .map_err(|_| TokenError::Malformed)?;
        let claims: TokenClaims = serde_json::from_slice(&raw_claims)
            .map_err(|e| TokenError::BadClaims(e.to_string()))?;
        Ok(Self { claims, raw_claims, signature })
    }
    pub fn wire(&self) -> String {
        format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(&self.raw_claims),
            URL_SAFE_NO_PAD.encode(&self.signature)
        )
    }
    pub fn verify(&self, keys: &[VerifyingKey]) -> Result<(), TokenError> {
        if keys.is_empty() {
            return Err(TokenError::NoKey);
        }
        let sig_bytes: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| TokenError::BadSignature)?;
        let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        let ok = keys.iter().any(|k| k.verify(&self.raw_claims, &sig).is_ok());
        if ok {
            Ok(())
        } else {
            Err(TokenError::BadSignature)
        }
    }
    pub fn sign(claims: TokenClaims, signing_key: &SigningKey) -> Self {
        let raw_claims = serde_json::to_vec(&claims).expect("claims serialize");
        let signature = signing_key.sign(&raw_claims).to_vec();
        Self { claims, raw_claims, signature }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_sign_parse_verify() {
        let sk = SigningKey::from_bytes(&[9u8; 32]);
        let vk = sk.verifying_key();
        let claims = TokenClaims {
            plan: Plan::Pro,
            expires_at: Utc::now().timestamp() + 3600,
            issued_at: Utc::now().timestamp(),
            device_id: "test-device".into(),
            extra_grants: vec![Capability::MapsAoMap],
            denials: vec![],
        };
        let token = EntitlementToken::sign(claims, &sk);
        let wire = token.wire();
        let parsed = EntitlementToken::parse(&wire).unwrap();
        parsed.verify(&[vk]).unwrap();
        assert_eq!(parsed.claims.plan, Plan::Pro);
    }
    #[test]
    fn cross_language_token_vector() {
        let sk = SigningKey::from_bytes(&[
            0x7E, 0x58, 0xC1, 0x3B, 0x94, 0x02, 0x6D, 0xF5, 0x1A, 0xB7, 0x4E, 0x83, 0xD9, 0x60,
            0x2F, 0xAA, 0x0B, 0x71, 0xE6, 0x5C, 0x98, 0x34, 0xAD, 0x17, 0xF0, 0x69, 0x25, 0xCC,
            0x48, 0xB3, 0x8E, 0x05,
        ]);
        let claims = TokenClaims {
            plan: Plan::Pro,
            expires_at: 2_000_000_000,
            issued_at: 1_000_000_000,
            device_id: "vector-device".into(),
            extra_grants: vec![Capability::MapsAoMap],
            denials: vec![],
        };
        let wire = EntitlementToken::sign(claims, &sk).wire();
        assert_eq!(
            wire,
            "eyJwbGFuIjoiUHJvIiwiZXhwaXJlc19hdCI6MjAwMDAwMDAwMCwiaXNzdWVkX2F0IjoxMDAwMDAwMDAwLCJkZXZpY2VfaWQiOiJ2ZWN0b3ItZGV2aWNlIiwiZXh0cmFfZ3JhbnRzIjpbIk1hcHNBb01hcCJdLCJkZW5pYWxzIjpbXX0.\
             hGC8iaHZnajAGlunHtsCgBZgfmD___UV65XtNVFoc8e1dkc25yVm2uzneN5XwGe_B7M6MQKPb4xGm2CKz2ZcCQ"
        );
        let parsed = EntitlementToken::parse(&wire).unwrap();
        assert!(parsed.verify(&[sk.verifying_key()]).is_ok());
    }
    #[test]
    fn forged_signature_rejected() {
        let sk = SigningKey::from_bytes(&[9u8; 32]);
        let claims = TokenClaims {
            plan: Plan::Pro,
            expires_at: Utc::now().timestamp() + 3600,
            issued_at: Utc::now().timestamp(),
            device_id: "d".into(),
            extra_grants: vec![],
            denials: vec![],
        };
        let token = EntitlementToken::sign(claims, &sk);
        let mut forged_claims = token.raw_claims.clone();
        forged_claims[0] ^= 0xFF;
        let forged = EntitlementToken {
            claims: TokenClaims {
                plan: Plan::Pro,
                expires_at: token.claims.expires_at,
                issued_at: token.claims.issued_at,
                device_id: "d".into(),
                extra_grants: vec![],
                denials: vec![],
            },
            raw_claims: forged_claims,
            signature: token.signature.clone(),
        };
        let vk = sk.verifying_key();
        assert!(forged.verify(&[vk]).is_err());
    }
}
