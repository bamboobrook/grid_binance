use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use shared_db::SharedDb;

use crate::{
    routes::auth_guard::{require_admin_session, require_super_admin_session},
    services::auth_service::{AuthError, AuthService},
    services::membership_service::{
        CreateSweepJobRequest, MembershipError, MembershipService, SweepJobResponse,
    },
    AppState,
};

#[derive(Debug, Serialize)]
pub struct AdminSweepJobItem {
    pub asset: String,
    pub chain: String,
    pub requested_by: String,
    pub status: String,
    pub sweep_job_id: u64,
    pub transfer_count: usize,
    pub treasury_address: String,
}

#[derive(Debug, Serialize)]
pub struct AdminSweepJobListResponse {
    pub jobs: Vec<AdminSweepJobItem>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/sweeps", get(list_sweeps).post(create_sweep_job))
}

async fn create_sweep_job(
    State(auth): State<AuthService>,
    State(service): State<MembershipService>,
    headers: HeaderMap,
    Json(request): Json<CreateSweepJobRequest>,
) -> Result<(StatusCode, Json<SweepJobResponse>), MembershipError> {
    let session = require_super_admin_session(&auth, &headers).map_err(MembershipError::from)?;
    Ok((
        StatusCode::CREATED,
        Json(service.create_sweep_job(&session.email, session.admin_role, session.sid, request)?),
    ))
}

async fn list_sweeps(
    State(auth): State<AuthService>,
    State(db): State<SharedDb>,
    headers: HeaderMap,
) -> Result<Json<AdminSweepJobListResponse>, AuthError> {
    require_admin_session(&auth, &headers)?;
    let jobs = db
        .list_sweep_jobs()
        .map_err(AuthError::storage)?
        .into_iter()
        .map(|job| AdminSweepJobItem {
            asset: job.asset,
            chain: job.chain,
            requested_by: job.requested_by,
            status: job.status,
            sweep_job_id: job.sweep_job_id,
            transfer_count: job.transfers.len(),
            treasury_address: job
                .transfers
                .first()
                .map(|transfer| transfer.to_address.clone())
                .unwrap_or_else(|| "-".to_owned()),
        })
        .collect();
    Ok(Json(AdminSweepJobListResponse { jobs }))
}
