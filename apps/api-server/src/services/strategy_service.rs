use std::sync::{Arc, Mutex};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use shared_domain::strategy::{
    PreflightFailure, PreflightReport, Strategy, StrategyStatus, StrategyTemplate,
};

#[derive(Clone, Default)]
pub struct StrategyService {
    inner: Arc<Mutex<StrategyState>>,
}

#[derive(Default)]
struct StrategyState {
    next_strategy_id: usize,
    next_template_id: usize,
    strategies: Vec<Strategy>,
    templates: Vec<StrategyTemplate>,
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

impl StrategyService {
    pub fn list_strategies(&self) -> StrategyListResponse {
        let inner = self.inner.lock().expect("strategy state poisoned");
        StrategyListResponse {
            items: inner.strategies.clone(),
        }
    }

    pub fn create_strategy(&self, request: SaveStrategyRequest) -> Result<Strategy, StrategyError> {
        validate_strategy_request(&request)?;

        let mut inner = self.inner.lock().expect("strategy state poisoned");
        inner.next_strategy_id += 1;
        let strategy = Strategy {
            id: format!("strategy-{}", inner.next_strategy_id),
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
        inner.strategies.push(strategy.clone());
        Ok(strategy)
    }

    pub fn update_strategy(
        &self,
        strategy_id: &str,
        request: SaveStrategyRequest,
    ) -> Result<Strategy, StrategyError> {
        validate_strategy_request(&request)?;

        let mut inner = self.inner.lock().expect("strategy state poisoned");
        let strategy = find_strategy_mut(&mut inner.strategies, strategy_id)?;

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

        Ok(strategy.clone())
    }

    pub fn preflight_strategy(&self, strategy_id: &str) -> Result<PreflightReport, StrategyError> {
        let inner = self.inner.lock().expect("strategy state poisoned");
        let strategy = find_strategy(&inner.strategies, strategy_id)?;
        Ok(run_preflight(strategy))
    }

    pub fn start_strategy(
        &self,
        strategy_id: &str,
    ) -> Result<StartStrategyResponse, StrategyError> {
        let mut inner = self.inner.lock().expect("strategy state poisoned");
        let strategy = find_strategy_mut(&mut inner.strategies, strategy_id)?;
        let preflight = run_preflight(strategy);

        if !preflight.ok {
            strategy.status = StrategyStatus::Error;
            return Err(StrategyError::conflict("preflight failed"));
        }

        strategy.status = StrategyStatus::Running;
        Ok(StartStrategyResponse {
            strategy: strategy.clone(),
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

        let mut inner = self.inner.lock().expect("strategy state poisoned");
        let mut paused = 0;

        for strategy in &mut inner.strategies {
            if request.ids.iter().any(|id| id == &strategy.id)
                && strategy.status == StrategyStatus::Running
            {
                strategy.status = StrategyStatus::Paused;
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

        let mut inner = self.inner.lock().expect("strategy state poisoned");
        let before = inner.strategies.len();
        inner.strategies.retain(|strategy| {
            !(request.ids.iter().any(|id| id == &strategy.id)
                && strategy.status != StrategyStatus::Running)
        });

        Ok(BatchDeleteResponse {
            deleted: before - inner.strategies.len(),
        })
    }

    pub fn stop_all(&self) -> StopAllResponse {
        let mut inner = self.inner.lock().expect("strategy state poisoned");
        let mut stopped = 0;

        for strategy in &mut inner.strategies {
            if matches!(
                strategy.status,
                StrategyStatus::Running | StrategyStatus::Paused
            ) {
                strategy.status = StrategyStatus::Stopped;
                stopped += 1;
            }
        }

        StopAllResponse { stopped }
    }

    pub fn list_templates(&self) -> TemplateListResponse {
        let inner = self.inner.lock().expect("strategy state poisoned");
        TemplateListResponse {
            items: inner.templates.clone(),
        }
    }

    pub fn create_template(
        &self,
        request: CreateTemplateRequest,
    ) -> Result<StrategyTemplate, StrategyError> {
        validate_template_request(&request)?;

        let mut inner = self.inner.lock().expect("strategy state poisoned");
        inner.next_template_id += 1;
        let template = StrategyTemplate {
            id: format!("template-{}", inner.next_template_id),
            name: request.name,
            symbol: request.symbol,
            budget: request.budget,
            grid_spacing_bps: request.grid_spacing_bps,
            membership_ready: request.membership_ready,
            exchange_ready: request.exchange_ready,
            symbol_ready: request.symbol_ready,
        };
        inner.templates.push(template.clone());
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

        let mut inner = self.inner.lock().expect("strategy state poisoned");
        let template = inner
            .templates
            .iter()
            .find(|template| template.id == template_id)
            .cloned()
            .ok_or_else(|| StrategyError::not_found("template not found"))?;

        inner.next_strategy_id += 1;
        let strategy = Strategy {
            id: format!("strategy-{}", inner.next_strategy_id),
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
        inner.strategies.push(strategy.clone());
        Ok(strategy)
    }
}

fn find_strategy<'a>(
    strategies: &'a [Strategy],
    strategy_id: &str,
) -> Result<&'a Strategy, StrategyError> {
    strategies
        .iter()
        .find(|strategy| strategy.id == strategy_id)
        .ok_or_else(|| StrategyError::not_found("strategy not found"))
}

fn find_strategy_mut<'a>(
    strategies: &'a mut [Strategy],
    strategy_id: &str,
) -> Result<&'a mut Strategy, StrategyError> {
    strategies
        .iter_mut()
        .find(|strategy| strategy.id == strategy_id)
        .ok_or_else(|| StrategyError::not_found("strategy not found"))
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
}

impl StrategyError {
    fn bad_request(message: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.to_string(),
        }
    }

    fn not_found(message: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.to_string(),
        }
    }

    fn conflict(message: &str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.to_string(),
        }
    }
}

impl IntoResponse for StrategyError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}
