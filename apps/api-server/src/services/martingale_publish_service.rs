use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared_db::{BacktestCandidateRecord, SharedDb};
use shared_domain::martingale::{MartingaleMarketKind, MartingalePortfolioConfig};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct MartingalePublishService {
    state: Arc<Mutex<PublishState>>,
}

#[derive(Default)]
struct PublishState {
    next_portfolio_id: u64,
    portfolios: BTreeMap<String, LivePortfolio>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivePortfolio {
    pub portfolio_id: String,
    pub owner: String,
    pub status: String,
    pub candidate_id: String,
    pub config: MartingalePortfolioConfig,
    pub risk_summary: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishIntentResponse {
    pub portfolio_id: String,
    pub status: String,
    pub candidate_id: String,
    pub risk_summary: Value,
}

impl MartingalePublishService {
    pub fn new(_db: SharedDb) -> Self {
        Self {
            state: Arc::new(Mutex::new(PublishState::default())),
        }
    }

    pub fn risk_summary_for_candidate(
        &self,
        candidate: &BacktestCandidateRecord,
    ) -> Result<Value, PublishError> {
        let config = candidate_portfolio_config(candidate)?;
        let symbols = config
            .strategies
            .iter()
            .map(|strategy| strategy.symbol.clone())
            .collect::<Vec<_>>();
        let max_leverage = config
            .strategies
            .iter()
            .filter_map(|strategy| strategy.leverage)
            .max()
            .unwrap_or(1);
        Ok(json!({
            "strategy_count": config.strategies.len(),
            "symbols": symbols,
            "max_leverage": max_leverage,
            "requires_futures": config.strategies.iter().any(|s| s.market == MartingaleMarketKind::UsdMFutures),
        }))
    }

    pub fn create_pending_portfolio(
        &self,
        owner: &str,
        candidate: &BacktestCandidateRecord,
    ) -> Result<PublishIntentResponse, PublishError> {
        let config = candidate_portfolio_config(candidate)?;
        config.validate().map_err(PublishError::bad_request)?;
        self.validate_futures_symbol_compatibility(owner, &config)?;
        let risk_summary = self.risk_summary_for_candidate(candidate)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| PublishError::internal("publish state lock poisoned"))?;
        state.next_portfolio_id += 1;
        let portfolio_id = format!("mp_{}", state.next_portfolio_id);
        let portfolio = LivePortfolio {
            portfolio_id: portfolio_id.clone(),
            owner: owner.to_owned(),
            status: "pending_confirmation".to_owned(),
            candidate_id: candidate.candidate_id.clone(),
            config,
            risk_summary: risk_summary.clone(),
        };
        state.portfolios.insert(portfolio_id.clone(), portfolio);
        Ok(PublishIntentResponse {
            portfolio_id,
            status: "pending_confirmation".to_owned(),
            candidate_id: candidate.candidate_id.clone(),
            risk_summary,
        })
    }

    pub fn confirm_start_portfolio(
        &self,
        owner: &str,
        portfolio_id: &str,
    ) -> Result<LivePortfolio, PublishError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PublishError::internal("publish state lock poisoned"))?;
        let portfolio = state
            .portfolios
            .get(portfolio_id)
            .ok_or_else(|| PublishError::not_found("portfolio not found"))?;
        if portfolio.owner != owner {
            return Err(PublishError::not_found("portfolio not found"));
        }
        if portfolio.status != "pending_confirmation" && portfolio.status != "paused" {
            return Err(PublishError::conflict(
                "portfolio cannot be started from current status",
            ));
        }
        validate_futures_symbol_compatibility_in_state(
            &state,
            owner,
            &portfolio.config,
            Some(portfolio_id),
            &["running", "paused"],
        )?;
        let portfolio = state
            .portfolios
            .get_mut(portfolio_id)
            .ok_or_else(|| PublishError::not_found("portfolio not found"))?;
        portfolio.status = "running".to_owned();
        Ok(portfolio.clone())
    }

    pub fn pause_portfolio(
        &self,
        owner: &str,
        portfolio_id: &str,
    ) -> Result<LivePortfolio, PublishError> {
        self.set_portfolio_status(owner, portfolio_id, "paused")
    }

    pub fn stop_portfolio(
        &self,
        owner: &str,
        portfolio_id: &str,
    ) -> Result<LivePortfolio, PublishError> {
        self.set_portfolio_status(owner, portfolio_id, "stopped")
    }

    pub fn list_portfolios(&self, owner: &str) -> Result<Vec<LivePortfolio>, PublishError> {
        let state = self
            .state
            .lock()
            .map_err(|_| PublishError::internal("publish state lock poisoned"))?;
        Ok(state
            .portfolios
            .values()
            .filter(|p| p.owner == owner)
            .cloned()
            .collect())
    }

    pub fn get_portfolio(
        &self,
        owner: &str,
        portfolio_id: &str,
    ) -> Result<LivePortfolio, PublishError> {
        let state = self
            .state
            .lock()
            .map_err(|_| PublishError::internal("publish state lock poisoned"))?;
        state
            .portfolios
            .get(portfolio_id)
            .filter(|p| p.owner == owner)
            .cloned()
            .ok_or_else(|| PublishError::not_found("portfolio not found"))
    }

    pub fn validate_futures_symbol_compatibility(
        &self,
        owner: &str,
        config: &MartingalePortfolioConfig,
    ) -> Result<(), PublishError> {
        self.validate_futures_symbol_compatibility_excluding(owner, config, None, &["running"])
    }

    fn validate_futures_symbol_compatibility_excluding(
        &self,
        owner: &str,
        config: &MartingalePortfolioConfig,
        excluded_portfolio_id: Option<&str>,
        active_statuses: &[&str],
    ) -> Result<(), PublishError> {
        let state = self
            .state
            .lock()
            .map_err(|_| PublishError::internal("publish state lock poisoned"))?;
        validate_futures_symbol_compatibility_in_state(
            &state,
            owner,
            config,
            excluded_portfolio_id,
            active_statuses,
        )
    }

    fn set_portfolio_status(
        &self,
        owner: &str,
        portfolio_id: &str,
        status: &str,
    ) -> Result<LivePortfolio, PublishError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PublishError::internal("publish state lock poisoned"))?;
        let portfolio = state
            .portfolios
            .get_mut(portfolio_id)
            .ok_or_else(|| PublishError::not_found("portfolio not found"))?;
        if portfolio.owner != owner {
            return Err(PublishError::not_found("portfolio not found"));
        }
        portfolio.status = status.to_owned();
        Ok(portfolio.clone())
    }
}

fn validate_futures_symbol_compatibility_in_state(
    state: &PublishState,
    owner: &str,
    config: &MartingalePortfolioConfig,
    excluded_portfolio_id: Option<&str>,
    active_statuses: &[&str],
) -> Result<(), PublishError> {
    for existing in state.portfolios.values().filter(|p| {
        p.owner == owner
            && Some(p.portfolio_id.as_str()) != excluded_portfolio_id
            && active_statuses.contains(&p.status.as_str())
    }) {
        for active in &existing.config.strategies {
            if active.market != MartingaleMarketKind::UsdMFutures {
                continue;
            }
            for incoming in &config.strategies {
                if incoming.market != MartingaleMarketKind::UsdMFutures
                    || !active.symbol.eq_ignore_ascii_case(&incoming.symbol)
                {
                    continue;
                }
                if active.margin_mode != incoming.margin_mode {
                    return Err(PublishError::conflict(format!(
                        "{} margin_mode conflict",
                        incoming.symbol
                    )));
                }
                if active.leverage != incoming.leverage {
                    return Err(PublishError::conflict(format!(
                        "{} leverage conflict",
                        incoming.symbol
                    )));
                }
            }
        }
    }
    Ok(())
}

fn candidate_portfolio_config(
    candidate: &BacktestCandidateRecord,
) -> Result<MartingalePortfolioConfig, PublishError> {
    let value = candidate
        .config
        .get("portfolio_config")
        .cloned()
        .unwrap_or_else(|| candidate.config.clone());
    serde_json::from_value(value).map_err(|error| {
        PublishError::bad_request(format!("invalid martingale portfolio config: {error}"))
    })
}

#[derive(Debug)]
pub struct PublishError {
    status: StatusCode,
    message: String,
}

impl PublishError {
    pub fn into_parts(self) -> (StatusCode, String) {
        (self.status, self.message)
    }
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for PublishError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
