use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionClaims {
    pub email: String,
    pub is_admin: bool,
    pub sid: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTokenError {
    InvalidFormat,
    InvalidPayload,
    InvalidSignature,
}

pub fn issue_session_token(
    secret: &str,
    claims: &SessionClaims,
) -> Result<String, SessionTokenError> {
    let payload =
        serde_json::to_vec(claims).map_err(|_| SessionTokenError::InvalidPayload)?;
    let encoded_payload = URL_SAFE_NO_PAD.encode(payload);
    let signed_value = format!("v1.{encoded_payload}");
    let signature = sign(secret, &signed_value)?;
    Ok(format!("{signed_value}.{signature}"))
}

pub fn verify_session_token(
    secret: &str,
    token: &str,
) -> Result<SessionClaims, SessionTokenError> {
    let mut parts = token.split('.');
    let Some(version) = parts.next() else {
        return Err(SessionTokenError::InvalidFormat);
    };
    let Some(payload) = parts.next() else {
        return Err(SessionTokenError::InvalidFormat);
    };
    let Some(signature) = parts.next() else {
        return Err(SessionTokenError::InvalidFormat);
    };

    if parts.next().is_some() || version != "v1" {
        return Err(SessionTokenError::InvalidFormat);
    }

    let signed_value = format!("{version}.{payload}");
    let expected_signature = sign(secret, &signed_value)?;
    if expected_signature != signature {
        return Err(SessionTokenError::InvalidSignature);
    }

    let decoded_payload = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| SessionTokenError::InvalidPayload)?;
    serde_json::from_slice(&decoded_payload).map_err(|_| SessionTokenError::InvalidPayload)
}

fn sign(secret: &str, value: &str) -> Result<String, SessionTokenError> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| SessionTokenError::InvalidSignature)?;
    mac.update(value.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}
