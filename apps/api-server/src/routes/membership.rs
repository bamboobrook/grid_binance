use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
use shared_domain::membership::MembershipSnapshot;

use crate::{
    routes::auth_guard::{require_session_email, require_user_session},
    services::auth_service::AuthService,
    services::membership_service::{MembershipError, MembershipService, MembershipStatusRequest},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/membership/status", post(status))
}

async fn status(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
    Json(request): Json<MembershipStatusRequest>,
) -> Result<Json<MembershipSnapshot>, MembershipError> {
    let session = require_user_session(&auth, &headers).map_err(MembershipError::from)?;
    require_session_email(&session, &request.email).map_err(MembershipError::from)?;
    Ok(Json(service.membership_status(request)?))
}
