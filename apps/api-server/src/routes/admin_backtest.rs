use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};

use crate::{
    routes::auth_guard::require_admin_session,
    services::{
        auth_service::AuthService,
        backtest_service::{
            BacktestError, BacktestService, QuotaPolicyResponse, UpsertQuotaRequest,
        },
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/admin/backtest/quotas/{owner}",
        get(get_quota).put(upsert_quota),
    )
}

async fn get_quota(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
    Path(owner): Path<String>,
) -> Result<Json<QuotaPolicyResponse>, BacktestError> {
    let _admin = require_admin_session(&auth, &headers)
        .map_err(|_error| BacktestError::forbidden("unauthorized"))?;
    Ok(Json(service.get_quota_policy(&owner)?))
}

async fn upsert_quota(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
    Path(owner): Path<String>,
    Json(request): Json<UpsertQuotaRequest>,
) -> Result<Json<QuotaPolicyResponse>, BacktestError> {
    let _admin = require_admin_session(&auth, &headers)
        .map_err(|_error| BacktestError::forbidden("unauthorized"))?;
    Ok(Json(service.upsert_quota_policy(&owner, request)?))
}
