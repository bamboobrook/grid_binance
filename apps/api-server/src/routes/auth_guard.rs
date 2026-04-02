use axum::http::{header::AUTHORIZATION, HeaderMap};

use crate::services::auth_service::{AdminRole, AuthError, AuthService};

#[derive(Debug, Clone)]
pub struct AuthenticatedSession {
    pub email: String,
    pub is_admin: bool,
    pub admin_role: Option<AdminRole>,
}

pub fn require_user_session(
    service: &AuthService,
    headers: &HeaderMap,
) -> Result<AuthenticatedSession, AuthError> {
    let session_token = bearer_token(headers)
        .ok_or_else(|| AuthError::unauthorized("valid session token required"))?;
    let claims = service.session_claims(session_token)?;
    let admin_role = if claims.is_admin {
        service.admin_role_for_email(&claims.email)
    } else {
        None
    };
    Ok(AuthenticatedSession {
        email: claims.email,
        is_admin: admin_role.is_some(),
        admin_role,
    })
}

pub fn require_admin_session(
    service: &AuthService,
    headers: &HeaderMap,
) -> Result<AuthenticatedSession, AuthError> {
    let session = require_user_session(service, headers)?;
    if !session.is_admin {
        return Err(AuthError::forbidden("admin access required"));
    }
    Ok(session)
}

pub fn require_super_admin_session(
    service: &AuthService,
    headers: &HeaderMap,
) -> Result<AuthenticatedSession, AuthError> {
    let session = require_admin_session(service, headers)?;
    if session.admin_role != Some(AdminRole::SuperAdmin) {
        return Err(AuthError::forbidden("super admin access required"));
    }
    Ok(session)
}

pub fn require_session_email(session: &AuthenticatedSession, email: &str) -> Result<(), AuthError> {
    if session.email != email.trim().to_lowercase() {
        return Err(AuthError::forbidden("session does not match user"));
    }
    Ok(())
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}
