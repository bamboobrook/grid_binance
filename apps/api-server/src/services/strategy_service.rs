use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use shared_db::{SharedDb, StoredStrategy, StoredStrategyTemplate};
use shared_domain::strategy::{
    PreflightFailure, PreflightReport, Strategy, StrategyStatus, StrategyTemplate,
};

use crate::services::auth_service::AuthError;

#[derive(Clone)]
pub struct StrategyService {
    db: SharedDb,
}

#[derive(Debug, Deserialize)]
pub struct SaveStrategyRequest {
    pub name: String,
    pub symbol: String,
    pub budget: String,
    pub grid_spacing_bps: u32,
    pub membership_ready: bool,
    pub exchange_ready: bool,
    pub symbol_ready: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub symbol: String,
    pub budget: String,
    pub grid_spacing_bps: u32,
    pub membership_ready: bool,
    pub exchange_ready: bool,
    pub symbol_ready: bool,
}

#[derive(Debug, Deserialize)]
pub struct ApplyTemplateRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchStrategyRequest {
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct StrategyListResponse {
    pub items: Vec<Strategy>,
}

#[derive(Debug, Serialize)]
pub struct TemplateListResponse {
    pub items: Vec<StrategyTemplate>,
}

#[derive(Debug, Serialize)]
pub struct StartStrategyResponse {
    #[serde(flatten)]
    pub strategy: Strategy,
    pub preflight: PreflightReport,
}

#[derive(Debug, Serialize)]
pub struct BatchPauseResponse {
    pub paused: usize,
}

#[derive(Debug, Serialize)]
pub struct BatchDeleteResponse {
    pub deleted: usize,
}

#[derive(Debug, Serialize)]
pub struct StopAllResponse {
    pub stopped: usize,
}

impl Default for StrategyService {
    fn default() -> Self {
        Self::new(SharedDb::in_memory().expect("in-memory strategy db should initialize"))
    }
}

impl StrategyService {
    pub fn new(db: SharedDb) -> Self {
        Self { db }
    }

    pub fn list_strategies(&self) -> StrategyListResponse {
        StrategyListResponse {
            items: self.db.list_strategies().unwrap_or_else(|_| Vec::new()),
        }
    }

    pub fn create_strategy(&self, request: SaveStrategyRequest) -> Result<Strategy, StrategyError> {
        validate_strategy_request(&request)?;

        let sequence_id = self
            .db
            .next_sequence("strategy")
            .map_err(StrategyError::storage)?;
        let strategy = Strategy {
            id: format!("strategy-{sequence_id}"),
            name: request.name,
            symbol: request.symbol,
            budget: request.budget,
            grid_spacing_bps: request.grid_spacing_bps,
            status: StrategyStatus::Draft,
            source_template_id: None,
            membership_ready: request.membership_ready,
            exchange_ready: request.exchange_ready,
            symbol_ready: request.symbol_ready,
        };
        self.db
            .insert_strategy(&StoredStrategy {
                sequence_id,
                strategy: strategy.clone(),
            })
            .map_err(StrategyError::storage)?;
        Ok(strategy)
    }

    pub fn update_strategy(
        &self,
        strategy_id: &str,
        request: SaveStrategyRequest,
    ) -> Result<Strategy, StrategyError> {
        validate_strategy_request(&request)?;

        let mut strategy = self
            .db
            .find_strategy(strategy_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("strategy not found"))?;

        if strategy.status != StrategyStatus::Draft {
            return Err(StrategyError::conflict(
                "only draft strategies can be edited",
            ));
        }

        strategy.name = request.name;
        strategy.symbol = request.symbol;
        strategy.budget = request.budget;
        strategy.grid_spacing_bps = request.grid_spacing_bps;
        strategy.membership_ready = request.membership_ready;
        strategy.exchange_ready = request.exchange_ready;
        strategy.symbol_ready = request.symbol_ready;

        self.db
            .update_strategy(&strategy)
            .map_err(StrategyError::storage)?;
        Ok(strategy)
    }

    pub fn preflight_strategy(&self, strategy_id: &str) -> Result<PreflightReport, StrategyError> {
        let strategy = self
            .db
            .find_strategy(strategy_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("strategy not found"))?;
        Ok(run_preflight(&strategy))
    }

    pub fn start_strategy(
        &self,
        strategy_id: &str,
    ) -> Result<StartStrategyResponse, StrategyError> {
        let mut strategy = self
            .db
            .find_strategy(strategy_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("strategy not found"))?;
        let preflight = run_preflight(&strategy);

        if !preflight.ok {
            return Err(StrategyError::preflight_failed(preflight));
        }

        strategy.status = StrategyStatus::Running;
        self.db
            .update_strategy(&strategy)
            .map_err(StrategyError::storage)?;

        Ok(StartStrategyResponse {
            strategy,
            preflight,
        })
    }

    pub fn pause_strategies(
        &self,
        request: BatchStrategyRequest,
    ) -> Result<BatchPauseResponse, StrategyError> {
        if request.ids.is_empty() {
            return Err(StrategyError::bad_request("ids are required"));
        }

        let mut paused = 0;
        for mut strategy in self.db.list_strategies().map_err(StrategyError::storage)? {
            if request.ids.iter().any(|id| id == &strategy.id)
                && strategy.status == StrategyStatus::Running
            {
                strategy.status = StrategyStatus::Paused;
                self.db
                    .update_strategy(&strategy)
                    .map_err(StrategyError::storage)?;
                paused += 1;
            }
        }

        Ok(BatchPauseResponse { paused })
    }

    pub fn delete_strategies(
        &self,
        request: BatchStrategyRequest,
    ) -> Result<BatchDeleteResponse, StrategyError> {
        if request.ids.is_empty() {
            return Err(StrategyError::bad_request("ids are required"));
        }

        let mut deleted = 0;
        for strategy in self.db.list_strategies().map_err(StrategyError::storage)? {
            if request.ids.iter().any(|id| id == &strategy.id)
                && strategy.status != StrategyStatus::Running
            {
                deleted += self
                    .db
                    .delete_strategy(&strategy.id)
                    .map_err(StrategyError::storage)?;
            }
        }

        Ok(BatchDeleteResponse { deleted })
    }

    pub fn stop_all(&self) -> StopAllResponse {
        let mut stopped = 0;

        if let Ok(strategies) = self.db.list_strategies() {
            for mut strategy in strategies {
                if matches!(
                    strategy.status,
                    StrategyStatus::Running | StrategyStatus::Paused
                ) {
                    strategy.status = StrategyStatus::Stopped;
                    if self.db.update_strategy(&strategy).is_ok() {
                        stopped += 1;
                    }
                }
            }
        }

        StopAllResponse { stopped }
    }

    pub fn list_templates(&self) -> TemplateListResponse {
        TemplateListResponse {
            items: self.db.list_templates().unwrap_or_else(|_| Vec::new()),
        }
    }

    pub fn create_template(
        &self,
        request: CreateTemplateRequest,
    ) -> Result<StrategyTemplate, StrategyError> {
        validate_template_request(&request)?;

        let sequence_id = self
            .db
            .next_sequence("strategy_template")
            .map_err(StrategyError::storage)?;
        let template = StrategyTemplate {
            id: format!("template-{sequence_id}"),
            name: request.name,
            symbol: request.symbol,
            budget: request.budget,
            grid_spacing_bps: request.grid_spacing_bps,
            membership_ready: request.membership_ready,
            exchange_ready: request.exchange_ready,
            symbol_ready: request.symbol_ready,
        };
        self.db
            .insert_template(&StoredStrategyTemplate {
                sequence_id,
                template: template.clone(),
            })
            .map_err(StrategyError::storage)?;
        Ok(template)
    }

    pub fn apply_template(
        &self,
        template_id: &str,
        request: ApplyTemplateRequest,
    ) -> Result<Strategy, StrategyError> {
        if request.name.trim().is_empty() {
            return Err(StrategyError::bad_request("name is required"));
        }

        let template = self
            .db
            .find_template(template_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("template not found"))?;
        let sequence_id = self
            .db
            .next_sequence("strategy")
            .map_err(StrategyError::storage)?;
        let strategy = Strategy {
            id: format!("strategy-{sequence_id}"),
            name: request.name,
            symbol: template.symbol,
            budget: template.budget,
            grid_spacing_bps: template.grid_spacing_bps,
            status: StrategyStatus::Draft,
            source_template_id: Some(template.id),
            membership_ready: template.membership_ready,
            exchange_ready: template.exchange_ready,
            symbol_ready: template.symbol_ready,
        };
        self.db
            .insert_strategy(&StoredStrategy {
                sequence_id,
                strategy: strategy.clone(),
            })
            .map_err(StrategyError::storage)?;
        Ok(strategy)
    }
}

fn run_preflight(strategy: &Strategy) -> PreflightReport {
    let mut failures = Vec::new();

    if !strategy.membership_ready {
        failures.push(PreflightFailure {
            step: "membership_status".to_string(),
            reason: "membership is not active".to_string(),
        });
    }

    if !strategy.exchange_ready {
        failures.push(PreflightFailure {
            step: "exchange_connection".to_string(),
            reason: "exchange credentials are not ready".to_string(),
        });
    }

    if !strategy.symbol_ready {
        failures.push(PreflightFailure {
            step: "symbol_support".to_string(),
            reason: "symbol is not available".to_string(),
        });
    }

    PreflightReport {
        ok: failures.is_empty(),
        failures,
    }
}

fn validate_strategy_request(request: &SaveStrategyRequest) -> Result<(), StrategyError> {
    if request.name.trim().is_empty() {
        return Err(StrategyError::bad_request("name is required"));
    }

    if request.symbol.trim().is_empty() {
        return Err(StrategyError::bad_request("symbol is required"));
    }

    if request.budget.trim().is_empty() {
        return Err(StrategyError::bad_request("budget is required"));
    }

    if request.grid_spacing_bps == 0 {
        return Err(StrategyError::bad_request(
            "grid_spacing_bps must be positive",
        ));
    }

    Ok(())
}

fn validate_template_request(request: &CreateTemplateRequest) -> Result<(), StrategyError> {
    validate_strategy_request(&SaveStrategyRequest {
        name: request.name.clone(),
        symbol: request.symbol.clone(),
        budget: request.budget.clone(),
        grid_spacing_bps: request.grid_spacing_bps,
        membership_ready: request.membership_ready,
        exchange_ready: request.exchange_ready,
        symbol_ready: request.symbol_ready,
    })
}

#[derive(Debug)]
pub struct StrategyError {
    status: StatusCode,
    message: String,
    extra: Option<serde_json::Value>,
}

impl StrategyError {
    fn bad_request(message: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.to_string(),
            extra: None,
        }
    }

    fn not_found(message: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.to_string(),
            extra: None,
        }
    }

    fn conflict(message: &str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.to_string(),
            extra: None,
        }
    }

    fn preflight_failed(preflight: PreflightReport) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: "preflight failed".to_string(),
            extra: Some(serde_json::json!({ "preflight": preflight })),
        }
    }

    fn storage(_error: shared_db::SharedDbError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal storage error".to_string(),
            extra: None,
        }
    }
}

impl IntoResponse for StrategyError {
    fn into_response(self) -> Response {
        let mut body = serde_json::json!({ "error": self.message });
        if let Some(extra) = self.extra {
            if let (Some(body_object), Some(extra_object)) =
                (body.as_object_mut(), extra.as_object())
            {
                for (key, value) in extra_object {
                    body_object.insert(key.clone(), value.clone());
                }
            }
        }

        (self.status, Json(body)).into_response()
    }
}

impl From<AuthError> for StrategyError {
    fn from(value: AuthError) -> Self {
        Self {
            status: value.status,
            message: value.message.to_string(),
            extra: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ApplyTemplateRequest, CreateTemplateRequest, SaveStrategyRequest, StrategyService,
    };
    use shared_db::SharedDb;
    use shared_domain::strategy::StrategyStatus;
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn strategy_and_template_state_survive_service_restart() {
        let db_path = temp_db_path("strategy");
        let db = SharedDb::open(&db_path).expect("open db");
        let service = StrategyService::new(db.clone());

        let template = service
            .create_template(CreateTemplateRequest {
                name: "Starter".to_string(),
                symbol: "BTCUSDT".to_string(),
                budget: "1000".to_string(),
                grid_spacing_bps: 50,
                membership_ready: true,
                exchange_ready: true,
                symbol_ready: true,
            })
            .expect("create template");

        service
            .create_strategy(SaveStrategyRequest {
                name: "Manual".to_string(),
                symbol: "ETHUSDT".to_string(),
                budget: "500".to_string(),
                grid_spacing_bps: 40,
                membership_ready: true,
                exchange_ready: true,
                symbol_ready: true,
            })
            .expect("create strategy");

        let reopened = StrategyService::new(SharedDb::open(&db_path).expect("reopen db"));
        let from_template = reopened
            .apply_template(
                &template.id,
                ApplyTemplateRequest {
                    name: "Applied".to_string(),
                },
            )
            .expect("apply template");
        reopened
            .start_strategy(&from_template.id)
            .expect("start strategy");

        let restarted = StrategyService::new(SharedDb::open(&db_path).expect("reopen db"));
        let strategies = restarted.list_strategies();
        let templates = restarted.list_templates();

        assert_eq!(templates.items.len(), 1);
        assert!(strategies
            .items
            .iter()
            .any(|strategy| strategy.id == from_template.id
                && strategy.status == StrategyStatus::Running));
    }

    fn temp_db_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic")
            .as_nanos();
        std::env::temp_dir().join(format!("grid-binance-{label}-{nonce}.sqlite3"))
    }
}
