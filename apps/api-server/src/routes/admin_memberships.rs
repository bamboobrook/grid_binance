use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
use shared_domain::membership::MembershipSnapshot;

use crate::{
    routes::auth_guard::require_admin_session,
    services::auth_service::AuthService,
    services::membership_service::{
        MembershipError, MembershipOverrideRequest, MembershipService,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/memberships/override", post(override_membership))
}

async fn override_membership(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
    Json(request): Json<MembershipOverrideRequest>,
) -> Result<Json<MembershipSnapshot>, MembershipError> {
    let session = require_admin_session(&auth, &headers).map_err(MembershipError::from)?;
    Ok(Json(service.override_membership(&session.email, request)?))
}
