use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
use serde::Serialize;
use shared_db::SharedDb;

use crate::{
    routes::auth_guard::require_admin_session,
    services::auth_service::AuthService,
    AppState,
};

#[derive(Debug, Serialize)]
pub struct AdminAuditItem {
    pub actor_email: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub payload: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct AdminAuditResponse {
    pub items: Vec<AdminAuditItem>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/audit", get(list_audit))
}

async fn list_audit(
    State(auth): State<AuthService>,
    State(db): State<SharedDb>,
    headers: HeaderMap,
) -> Result<Json<AdminAuditResponse>, crate::services::auth_service::AuthError> {
    require_admin_session(&auth, &headers)?;
    let items = db
        .list_audit_logs()
        .map_err(|error| crate::services::auth_service::AuthError::storage(error))?
        .into_iter()
        .map(|entry| AdminAuditItem {
            actor_email: entry.actor_email,
            action: entry.action,
            target_type: entry.target_type,
            target_id: entry.target_id,
            payload: entry.payload,
            created_at: entry.created_at,
        })
        .collect();
    Ok(Json(AdminAuditResponse { items }))
}
