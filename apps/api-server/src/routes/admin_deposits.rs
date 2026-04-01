use axum::{
    extract::{Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};

use crate::{
    routes::auth_guard::require_admin_session,
    services::auth_service::AuthService,
    services::membership_service::{
        AdminDepositsQuery, AdminDepositsResponse, MembershipError, MembershipService,
        ProcessAbnormalDepositRequest, ProcessAbnormalDepositResponse,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/deposits", get(list_deposits))
        .route("/admin/deposits/process", post(process_deposit))
}

async fn list_deposits(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
    Query(query): Query<AdminDepositsQuery>,
) -> Result<Json<AdminDepositsResponse>, MembershipError> {
    require_admin_session(&auth, &headers).map_err(MembershipError::from)?;
    Ok(Json(service.admin_list_deposits(
        query.at.unwrap_or_else(chrono::Utc::now),
    )?))
}

async fn process_deposit(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
    Json(request): Json<ProcessAbnormalDepositRequest>,
) -> Result<Json<ProcessAbnormalDepositResponse>, MembershipError> {
    let session = require_admin_session(&auth, &headers).map_err(MembershipError::from)?;
    Ok(Json(service.process_abnormal_deposit(&session.email, request)?))
}
