use axum::{extract::State, http::HeaderMap, routing::{get, post}, Json, Router};
use chrono::Utc;
use serde::Serialize;
use shared_db::SharedDb;
use shared_domain::membership::{MembershipSnapshot, MembershipStatus};

use crate::{
    routes::auth_guard::require_admin_session,
    services::auth_service::{AuthError, AuthService},
    services::membership_service::{
        ManualMembershipRequest, MembershipError, MembershipOverrideRequest,
        MembershipPlanConfigListResponse, MembershipPlanConfigResponse, MembershipService,
        UpsertMembershipPlanRequest,
    },
    AppState,
};

#[derive(Debug, Serialize)]
pub struct AdminMembershipListResponse {
    pub items: Vec<MembershipSnapshot>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/memberships", get(list_memberships))
        .route("/admin/memberships/override", post(override_membership))
        .route("/admin/memberships/manage", post(manage_membership))
        .route("/admin/memberships/plans", get(list_plans).post(upsert_plan))
}

async fn list_memberships(
    State(auth): State<AuthService>,
    State(db): State<SharedDb>,
    headers: HeaderMap,
) -> Result<Json<AdminMembershipListResponse>, AuthError> {
    require_admin_session(&auth, &headers)?;
    let now = Utc::now();
    let items = db
        .list_membership_records()
        .map_err(AuthError::storage)?
        .into_iter()
        .map(|(email, record)| MembershipSnapshot {
            email,
            status: derive_status(&record, now),
            active_until: record.active_until,
            grace_until: record.grace_until,
            override_status: record.override_status,
        })
        .collect();
    Ok(Json(AdminMembershipListResponse { items }))
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

fn derive_status(record: &shared_db::MembershipRecord, now: chrono::DateTime<chrono::Utc>) -> MembershipStatus {
    if let Some(status) = record.override_status.clone() {
        return status;
    }
    if record.active_until.is_some_and(|active_until| now <= active_until) {
        return MembershipStatus::Active;
    }
    if record.grace_until.is_some_and(|grace_until| now <= grace_until) {
        return MembershipStatus::Grace;
    }
    if record.activated_at.is_some() {
        return MembershipStatus::Expired;
    }
    MembershipStatus::Pending
}
