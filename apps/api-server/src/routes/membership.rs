use axum::{extract::State, routing::post, Json, Router};
use shared_domain::membership::MembershipSnapshot;

use crate::{
    services::membership_service::{
        MembershipError, MembershipOverrideRequest, MembershipService, MembershipStatusRequest,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/membership/status", post(status))
        .route("/membership/admin/override", post(override_status))
}

async fn status(
    State(service): State<MembershipService>,
    Json(request): Json<MembershipStatusRequest>,
) -> Result<Json<MembershipSnapshot>, MembershipError> {
    Ok(Json(service.membership_status(request)?))
}

async fn override_status(
    State(service): State<MembershipService>,
    Json(request): Json<MembershipOverrideRequest>,
) -> Result<Json<MembershipSnapshot>, MembershipError> {
    Ok(Json(service.override_membership(request)?))
}
