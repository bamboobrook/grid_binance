use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};

use crate::{
    routes::auth_guard::require_admin_session,
    services::auth_service::AuthService,
    services::membership_service::{
        CreateSweepJobRequest, MembershipError, MembershipService, SweepJobListResponse,
        SweepJobResponse,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/sweeps", get(list_sweeps).post(create_sweep_job))
}

async fn create_sweep_job(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
    Json(request): Json<CreateSweepJobRequest>,
) -> Result<(StatusCode, Json<SweepJobResponse>), MembershipError> {
    let session = require_admin_session(&auth, &headers).map_err(MembershipError::from)?;
    Ok((
        StatusCode::CREATED,
        Json(service.create_sweep_job(&session.email, request)?),
    ))
}

async fn list_sweeps(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
) -> Result<Json<SweepJobListResponse>, MembershipError> {
    require_admin_session(&auth, &headers).map_err(MembershipError::from)?;
    Ok(Json(service.list_sweep_jobs()?))
}
