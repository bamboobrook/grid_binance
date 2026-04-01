use axum::{extract::State, http::HeaderMap, routing::{get, post}, Json, Router};
use shared_domain::membership::MembershipSnapshot;

use crate::{
    routes::auth_guard::require_admin_session,
    services::auth_service::AuthService,
    services::membership_service::{
        ManualMembershipRequest, MembershipError, MembershipOverrideRequest,
        MembershipPlanConfigListResponse, MembershipPlanConfigResponse, MembershipService,
        UpsertMembershipPlanRequest,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/memberships/override", post(override_membership))
        .route("/admin/memberships/manage", post(manage_membership))
        .route("/admin/memberships/plans", get(list_plans).post(upsert_plan))
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

async fn manage_membership(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
    Json(request): Json<ManualMembershipRequest>,
) -> Result<Json<MembershipSnapshot>, MembershipError> {
    let session = require_admin_session(&auth, &headers).map_err(MembershipError::from)?;
    Ok(Json(service.manage_membership(&session.email, request)?))
}

async fn list_plans(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
) -> Result<Json<MembershipPlanConfigListResponse>, MembershipError> {
    require_admin_session(&auth, &headers).map_err(MembershipError::from)?;
    Ok(Json(service.list_plan_configs()?))
}

async fn upsert_plan(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
    Json(request): Json<UpsertMembershipPlanRequest>,
) -> Result<Json<MembershipPlanConfigResponse>, MembershipError> {
    let session = require_admin_session(&auth, &headers).map_err(MembershipError::from)?;
    Ok(Json(service.upsert_plan_config(&session.email, request)?))
}
