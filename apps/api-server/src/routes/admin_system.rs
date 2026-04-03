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
    let eth_confirmations =
        validate_confirmation_value(request.eth_confirmations, ETH_CONFIRMATIONS_KEY)?;
    let bsc_confirmations =
        validate_confirmation_value(request.bsc_confirmations, BSC_CONFIRMATIONS_KEY)?;
    let sol_confirmations =
        validate_confirmation_value(request.sol_confirmations, SOL_CONFIRMATIONS_KEY)?;
    let updated_at = Utc::now();
    let records = vec![
        SystemConfigRecord {
            config_key: ETH_CONFIRMATIONS_KEY.to_owned(),
            config_value: json!({ "value": eth_confirmations }),
            updated_at,
        },
        SystemConfigRecord {
            config_key: BSC_CONFIRMATIONS_KEY.to_owned(),
            config_value: json!({ "value": bsc_confirmations }),
            updated_at,
        },
        SystemConfigRecord {
            config_key: SOL_CONFIRMATIONS_KEY.to_owned(),
            config_value: json!({ "value": sol_confirmations }),
            updated_at,
        },
    ];
    let audit = AuditLogRecord {
        actor_email: session.email,
        action: "system.confirmations_updated".to_owned(),
        target_type: "system_config".to_owned(),
        target_id: "chain_confirmations".to_owned(),
        payload: json!({
            "eth": eth_confirmations,
            "bsc": bsc_confirmations,
            "sol": sol_confirmations,
            "session_role": session.admin_role.map(|role| role.as_str()),
            "session_sid": session.sid,
            "before_summary": confirmation_summary(before_eth, before_bsc, before_sol),
            "after_summary": confirmation_summary(eth_confirmations, bsc_confirmations, sol_confirmations),
        }),
        created_at: updated_at,
    };
    db.upsert_system_configs_with_audit(&records, &audit)
        .map_err(AuthError::storage)?;
    Ok(Json(AdminSystemResponse {
        eth_confirmations,
        bsc_confirmations,
        sol_confirmations,
    }))
}

fn read_confirmation(db: &SharedDb, key: &str) -> Result<u32, AuthError> {
    let record = db.get_system_config(key).map_err(AuthError::storage)?;

    let Some(record) = record else {
        return Ok(12);
    };

    let value = record
        .config_value
        .get("value")
        .and_then(|value: &serde_json::Value| value.as_u64())
        .ok_or(AuthError {
            status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            message: "invalid stored confirmation value",
        })?;

    if value == 0 || value > u32::MAX as u64 {
        return Err(AuthError {
            status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            message: "invalid stored confirmation value",
        });
    }

    Ok(value as u32)
}

fn confirmation_summary(eth: u32, bsc: u32, sol: u32) -> String {
    format!("ETH {} | BSC {} | SOL {}", eth, bsc, sol)
}

fn validate_confirmation_value(value: u32, key: &str) -> Result<u32, AuthError> {
    if value == 0 {
        return Err(AuthError::bad_request(match key {
            ETH_CONFIRMATIONS_KEY => "eth_confirmations must be greater than 0",
            BSC_CONFIRMATIONS_KEY => "bsc_confirmations must be greater than 0",
            SOL_CONFIRMATIONS_KEY => "sol_confirmations must be greater than 0",
            _ => "confirmation value must be greater than 0",
        }));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{read_confirmation, validate_confirmation_value, ETH_CONFIRMATIONS_KEY};
    use crate::services::auth_service::AuthError;
    use axum::http::StatusCode;
    use chrono::Utc;
    use serde_json::json;
    use shared_db::{SharedDb, SystemConfigRecord};

    #[test]
    fn missing_confirmation_uses_default_value() {
        let db = SharedDb::ephemeral().expect("db");
        assert_eq!(
            read_confirmation(&db, ETH_CONFIRMATIONS_KEY).expect("default"),
            12
        );
    }

    #[test]
    fn stored_zero_confirmation_is_rejected() {
        let db = SharedDb::ephemeral().expect("db");
        db.upsert_system_config(&SystemConfigRecord {
            config_key: ETH_CONFIRMATIONS_KEY.to_owned(),
            config_value: json!({ "value": 0 }),
            updated_at: Utc::now(),
        })
        .expect("config stored");

        match read_confirmation(&db, ETH_CONFIRMATIONS_KEY) {
            Err(AuthError { status, message }) => {
                assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
                assert_eq!(message, "invalid stored confirmation value");
            }
            Ok(value) => panic!("expected invalid stored value to fail, got {value}"),
        }
    }

    #[test]
    fn write_validation_rejects_zero_confirmation() {
        match validate_confirmation_value(0, ETH_CONFIRMATIONS_KEY) {
            Err(AuthError { status, message }) => {
                assert_eq!(status, StatusCode::BAD_REQUEST);
                assert_eq!(message, "eth_confirmations must be greater than 0");
            }
            Ok(value) => panic!("expected zero confirmation to fail, got {value}"),
        }
    }
}
