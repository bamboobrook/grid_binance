use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use shared_db::{AuditLogRecord, SharedDb, SystemConfigRecord};

use crate::{
    routes::auth_guard::{require_admin_session, require_super_admin_session},
    services::auth_service::{AuthError, AuthService},
    AppState,
};

const ETH_CONFIRMATIONS_KEY: &str = "confirmations.eth";
const BSC_CONFIRMATIONS_KEY: &str = "confirmations.bsc";
const SOL_CONFIRMATIONS_KEY: &str = "confirmations.sol";

#[derive(Debug, Serialize)]
pub struct AdminSystemResponse {
    pub eth_confirmations: u32,
    pub bsc_confirmations: u32,
    pub sol_confirmations: u32,
}

#[derive(Debug, Deserialize)]
pub struct AdminSystemUpdateRequest {
    pub eth_confirmations: u32,
    pub bsc_confirmations: u32,
    pub sol_confirmations: u32,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/system", get(read_system).post(update_system))
}

async fn read_system(
    State(auth): State<AuthService>,
    State(db): State<SharedDb>,
    headers: HeaderMap,
) -> Result<Json<AdminSystemResponse>, AuthError> {
    require_admin_session(&auth, &headers)?;
    Ok(Json(AdminSystemResponse {
        eth_confirmations: read_confirmation(&db, ETH_CONFIRMATIONS_KEY)?,
        bsc_confirmations: read_confirmation(&db, BSC_CONFIRMATIONS_KEY)?,
        sol_confirmations: read_confirmation(&db, SOL_CONFIRMATIONS_KEY)?,
    }))
}

async fn update_system(
    State(auth): State<AuthService>,
    State(db): State<SharedDb>,
    headers: HeaderMap,
    Json(request): Json<AdminSystemUpdateRequest>,
) -> Result<Json<AdminSystemResponse>, AuthError> {
    let session = require_super_admin_session(&auth, &headers)?;
    let before_eth = read_confirmation(&db, ETH_CONFIRMATIONS_KEY)?;
    let before_bsc = read_confirmation(&db, BSC_CONFIRMATIONS_KEY)?;
    let before_sol = read_confirmation(&db, SOL_CONFIRMATIONS_KEY)?;
    let updated_at = Utc::now();
    let records = vec![
        SystemConfigRecord {
            config_key: ETH_CONFIRMATIONS_KEY.to_owned(),
            config_value: json!({ "value": request.eth_confirmations }),
            updated_at,
        },
        SystemConfigRecord {
            config_key: BSC_CONFIRMATIONS_KEY.to_owned(),
            config_value: json!({ "value": request.bsc_confirmations }),
            updated_at,
        },
        SystemConfigRecord {
            config_key: SOL_CONFIRMATIONS_KEY.to_owned(),
            config_value: json!({ "value": request.sol_confirmations }),
            updated_at,
        },
    ];
    let audit = AuditLogRecord {
        actor_email: session.email,
        action: "system.confirmations_updated".to_owned(),
        target_type: "system_config".to_owned(),
        target_id: "chain_confirmations".to_owned(),
        payload: json!({
            "eth": request.eth_confirmations,
            "bsc": request.bsc_confirmations,
            "sol": request.sol_confirmations,
            "session_role": session.admin_role.map(|role| role.as_str()),
            "session_sid": session.sid,
            "before_summary": confirmation_summary(before_eth, before_bsc, before_sol),
            "after_summary": confirmation_summary(request.eth_confirmations, request.bsc_confirmations, request.sol_confirmations),
        }),
        created_at: updated_at,
    };
    db.upsert_system_configs_with_audit(&records, &audit)
        .map_err(AuthError::storage)?;
    Ok(Json(AdminSystemResponse {
        eth_confirmations: request.eth_confirmations,
        bsc_confirmations: request.bsc_confirmations,
        sol_confirmations: request.sol_confirmations,
    }))
}

fn read_confirmation(db: &SharedDb, key: &str) -> Result<u32, AuthError> {
    let record = db
        .get_system_config(key)
        .map_err(|error| AuthError::storage(error))?;
    Ok(record
        .and_then(|item| {
            item.config_value
                .get("value")
                .and_then(|value: &serde_json::Value| value.as_u64())
                .map(|value| value as u32)
        })
        .unwrap_or(12))
}

fn confirmation_summary(eth: u32, bsc: u32, sol: u32) -> String {
    format!("ETH {} | BSC {} | SOL {}", eth, bsc, sol)
}
