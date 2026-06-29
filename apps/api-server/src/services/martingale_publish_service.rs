use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared_db::{
    BacktestCandidateRecord, BacktestRepository, MartingalePortfolioRecord,
    NewMartingalePortfolioItemRecord, NewMartingalePortfolioRecord, SharedDb,
};
use shared_domain::{
    martingale::{MartingaleMarketKind, MartingalePortfolioConfig},
    strategy::Decimal,
};
use std::collections::{BTreeMap, BTreeSet};

use crate::services::martingale_exchange_preconfigure_service::{
    binance_client_for_owner, check_live_state_blockers, target_exchange_settings_from_portfolio,
};

use backtest_engine::martingale::capital::{project_portfolio_capital, PortfolioCapitalProjection};

#[derive(Clone)]
pub struct MartingalePublishService {
    db: SharedDb,
    repo: BacktestRepository,
}

pub type LivePortfolio = MartingalePortfolioRecord;

#[derive(Debug, Clone, Deserialize)]
pub struct PublishPortfolioRequest {
    pub name: String,
    pub task_id: String,
    pub market: String,
    pub direction: String,
    pub risk_profile: String,
    pub total_weight_pct: Decimal,
    pub items: Vec<PublishPortfolioItemRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PublishPortfolioItemRequest {
    pub candidate_id: String,
    pub symbol: String,
    pub weight_pct: Decimal,
    pub leverage: i32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub parameter_snapshot: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishPortfolioResponse {
    pub portfolio_id: String,
    pub status: String,
    pub source_task_id: String,
    pub instances: Vec<PublishedStrategyInstance>,
    pub items: Vec<PublishedStrategyInstance>,
    pub risk_summary: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishedStrategyInstance {
    pub strategy_instance_id: String,
    pub candidate_id: String,
    pub symbol: String,
    pub weight_pct: Decimal,
    pub leverage: i32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishIntentResponse {
    pub portfolio_id: String,
    pub status: String,
    pub candidate_id: String,
    pub risk_summary: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConfirmStartPortfolioRequest {
    pub max_global_budget_quote: Option<Decimal>,
}

impl MartingalePublishService {
    pub fn new(db: SharedDb) -> Self {
        Self {
            db: db.clone(),
            repo: db.backtest_repo(),
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
        // Surface the effective pause-guard thresholds (config value or engine
        // default) so users can see e.g. "this portfolio pauses new cycles at 6%
        // drawdown". Read from the typed portfolio risk_limits.
        let limits = &config.risk_limits;
        let risk_guard_thresholds = json!({
            "new_cycle_drawdown_pause_pct": limits.new_cycle_drawdown_pause_pct.unwrap_or(6.0),
            "new_cycle_atr_pause_pct": limits.new_cycle_atr_pause_pct.unwrap_or(2.0),
            "safety_skip_adx_threshold": limits.safety_skip_adx_threshold.unwrap_or(45.0),
        });
        // Cross-symbol indicator dependencies (e.g. a SOL strategy referencing
        // `BTCUSDT.ema(50)`). These symbols must be subscribed for live market
        // data even though they are never traded; surfaced so operators know
        // why a portfolio needs BTC/ETH feeds.
        let market_data_dependencies =
            backtest_engine::martingale::indicator_runtime::extract_symbol_dependencies(&config);
        Ok(json!({
            "strategy_count": config.strategies.len(),
            "symbols": symbols,
            "max_leverage": max_leverage,
            "requires_futures": config.strategies.iter().any(|s| s.market == MartingaleMarketKind::UsdMFutures),
            "risk_guard_thresholds": risk_guard_thresholds,
            "market_data_dependencies": market_data_dependencies,
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
        let symbol = config
            .strategies
            .first()
            .map(|strategy| strategy.symbol.clone())
            .or_else(|| {
                candidate
                    .config
                    .get("symbol")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "UNKNOWN".to_owned());
        let leverage = config
            .strategies
            .first()
            .and_then(|strategy| strategy.leverage)
            .unwrap_or(1) as i32;
        let (market, direction) = candidate_publish_metadata(&config);
        let request = PublishPortfolioRequest {
            name: format!("{} publish intent", symbol),
            task_id: candidate.task_id.clone(),
            market,
            direction,
            risk_profile: "single_candidate".to_owned(),
            total_weight_pct: Decimal::new(100, 0),
            items: vec![PublishPortfolioItemRequest {
                candidate_id: candidate.candidate_id.clone(),
                symbol,
                weight_pct: Decimal::new(100, 0),
                leverage,
                enabled: true,
                parameter_snapshot: candidate.config.clone(),
            }],
        };
        let response = self.publish_portfolio(owner, request, vec![candidate.clone()])?;
        Ok(PublishIntentResponse {
            portfolio_id: response.portfolio_id,
            status: response.status,
            candidate_id: candidate.candidate_id.clone(),
            risk_summary,
        })
    }

    pub fn publish_portfolio(
        &self,
        owner: &str,
        request: PublishPortfolioRequest,
        candidates: Vec<BacktestCandidateRecord>,
    ) -> Result<PublishPortfolioResponse, PublishError> {
        validate_publish_request(&request)?;
        let candidates_by_id = candidates
            .into_iter()
            .map(|candidate| (candidate.candidate_id.clone(), candidate))
            .collect::<BTreeMap<_, _>>();
        for item in &request.items {
            let candidate = candidates_by_id
                .get(&item.candidate_id)
                .ok_or_else(|| PublishError::bad_request("candidate not found"))?;
            if candidate.task_id != request.task_id {
                return Err(PublishError::bad_request(
                    "candidate does not belong to task",
                ));
            }
        }

        let portfolio_id = format!("mp_{}", uuid_simple());
        let risk_summary = portfolio_risk_summary(&request);
        let item_records = request
            .items
            .iter()
            .map(|item| {
                let candidate = candidates_by_id
                    .get(&item.candidate_id)
                    .expect("candidate was validated");
                NewMartingalePortfolioItemRecord {
                    strategy_instance_id: format!("msi_{}", uuid_simple()),
                    candidate_id: item.candidate_id.clone(),
                    symbol: item.symbol.clone(),
                    weight_pct: item.weight_pct,
                    leverage: item.leverage,
                    enabled: item.enabled,
                    status: "pending_confirmation".to_owned(),
                    parameter_snapshot: publish_parameter_snapshot(item, candidate),
                    metrics_snapshot: candidate.summary.clone(),
                }
            })
            .collect::<Vec<_>>();
        let live_config = live_portfolio_config_snapshot(&request, &item_records);
        let record = self.repo.create_martingale_portfolio(
            NewMartingalePortfolioRecord {
                portfolio_id,
                owner: owner.to_owned(),
                name: request.name,
                status: "pending_confirmation".to_owned(),
                source_task_id: request.task_id,
                market: request.market,
                direction: request.direction,
                risk_profile: request.risk_profile,
                total_weight_pct: request.total_weight_pct,
                config: live_config,
                risk_summary: risk_summary.clone(),
            },
            item_records,
        )?;
        Ok(PublishPortfolioResponse::from(record))
    }

    pub fn confirm_start_portfolio(
        &self,
        owner: &str,
        portfolio_id: &str,
        request: ConfirmStartPortfolioRequest,
    ) -> Result<LivePortfolio, PublishError> {
        let portfolio = self.get_portfolio(owner, portfolio_id)?;
        if portfolio.status != "pending_confirmation" && portfolio.status != "paused" {
            return Err(PublishError::conflict(
                "portfolio cannot be started from current status",
            ));
        }

        // --- Readiness gate: exchange preconfigure must be ready and fresh ---
        let preconfigure = portfolio
            .risk_summary
            .get("exchange_preconfigure")
            .ok_or_else(|| {
                PublishError::conflict(
                    "exchange preconfigure is required before starting; run exchange preconfigure first",
                )
            })?;
        let preconfigure_status = preconfigure
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if preconfigure_status != "ready" {
            return Err(PublishError::conflict(format!(
                "exchange preconfigure status is '{}'; re-run exchange preconfigure to confirm readiness",
                preconfigure_status
            )));
        }
        // Check TTL: preconfigure must be within 10 minutes
        const PRECONFIGURE_TTL_SECS: i64 = 600;
        if let Some(checked_at) = preconfigure.get("checked_at").and_then(|v| v.as_str()) {
            if let Ok(checked_dt) = chrono::DateTime::parse_from_rfc3339(checked_at) {
                let age_secs = (Utc::now() - checked_dt.with_timezone(&Utc)).num_seconds();
                if age_secs > PRECONFIGURE_TTL_SECS {
                    return Err(PublishError::conflict(
                        "exchange preconfigure snapshot is stale; re-run exchange preconfigure",
                    ));
                }
            }
        }

        // --- Strategy count check ---
        let enabled_count = portfolio.items.iter().filter(|item| item.enabled).count();
        if enabled_count == 0 {
            return Err(PublishError::conflict(
                "portfolio has no enabled strategy instances",
            ));
        }

        // --- Config validation ---
        let mut config_value = portfolio.config.clone();
        let portfolio_config_value = config_value
            .get("portfolio_config")
            .cloned()
            .ok_or_else(|| PublishError::conflict("portfolio_config is missing"))?;
        let mut portfolio_config: shared_domain::martingale::MartingalePortfolioConfig =
            serde_json::from_value(portfolio_config_value.clone()).map_err(|error| {
                PublishError::conflict(format!("invalid portfolio config: {error}"))
            })?;

        let requires_futures = portfolio_config
            .strategies
            .iter()
            .any(|strategy| strategy.market == MartingaleMarketKind::UsdMFutures);
        let live_budget = request
            .max_global_budget_quote
            .or(portfolio_config.risk_limits.max_global_budget_quote)
            .filter(|value| *value > Decimal::ZERO);
        if requires_futures && live_budget.is_none() {
            return Err(PublishError::conflict(
                "portfolio live capital is required before starting",
            ));
        }
        if let Some(budget) = live_budget {
            portfolio_config.risk_limits.max_global_budget_quote = Some(budget);
            set_live_budget_in_config(&mut config_value, budget)?;
        }
        portfolio_config.validate().map_err(|error| {
            PublishError::conflict(format!("config validation failed: {error}"))
        })?;

        let target_settings = target_exchange_settings_from_portfolio(&portfolio)
            .map_err(|error| PublishError::conflict(error.to_string()))?;
        if live_exchange_state_check_enabled() && !target_settings.symbols.is_empty() {
            let client = binance_client_for_owner(&self.db, owner)
                .map_err(|error| PublishError::conflict(error.to_string()))?;
            let blockers = check_live_state_blockers(&client, &target_settings)
                .map_err(|error| PublishError::conflict(error.to_string()))?;
            if !blockers.is_empty() {
                return Err(PublishError::conflict(format!(
                    "live account has open orders or positions; clear them before starting: {}",
                    blockers.join("; ")
                )));
            }
        }

        validate_running_futures_conflicts(
            &self.repo.list_martingale_portfolios(owner)?,
            &portfolio,
        )?;

        // --- Capital preflight: project the budget-capped margin the runtime
        // will actually allow (capping each strategy's leg walk at the global
        // margin pool), then gate on whether every strategy can place its first
        // leg and whether the available USDT covers margin + fee buffer. The
        // full uncapped geometric-series margin is recorded as a diagnostic
        // only — it is NOT the gate (the runtime never places those legs).
        const ENTRY_FEE_BPS: f64 = 4.5;
        const FEE_BUFFER_PCT: f64 = 5.0;
        let global_margin_cap =
            rust_decimal::prelude::ToPrimitive::to_f64(&live_budget.unwrap_or_default())
                .unwrap_or(0.0);
        let weights = extract_portfolio_weight_factors(&config_value);
        let projection = project_portfolio_capital(
            &portfolio_config.strategies,
            &weights,
            global_margin_cap,
            0.0, // exchange_min_notional: diagnostic here (enforced at placement)
            ENTRY_FEE_BPS,
            FEE_BUFFER_PCT,
        )
        .map_err(|e| PublishError::conflict(format!("capital projection failed: {e}")))?;

        // Keep the available-USDT probe (gated on the live-state check, which is
        // disabled in tests, so available_usdt stays None there). It feeds both
        // the preflight gate and the diagnostic JSON.
        let mut available_usdt: Option<f64> = None;
        if live_exchange_state_check_enabled() && !target_settings.symbols.is_empty() {
            if let Ok(client) = binance_client_for_owner(&self.db, owner) {
                if let Ok(balances) = client.read_usdm_account_v3_balance() {
                    if let Some(usdt) = balances
                        .iter()
                        .find(|balance| balance.asset.eq_ignore_ascii_case("USDT"))
                    {
                        if let Ok(avail) = usdt.available_balance.parse::<f64>() {
                            available_usdt = Some(avail);
                        }
                    }
                }
            }
        }
        if let Some(reason) = preflight_rejection_reason(&projection, available_usdt) {
            return Err(PublishError::conflict(reason));
        }

        // --- Record start intent ---
        let mut risk_summary = portfolio.risk_summary.clone();
        if !risk_summary.is_object() {
            risk_summary = serde_json::json!({});
        }
        if let serde_json::Value::Object(map) = &mut risk_summary {
            let per_strategy: Vec<Value> = projection
                .strategies
                .iter()
                .map(|s| {
                    let skipped_legs: Vec<Value> = s
                        .legs
                        .iter()
                        .filter(|leg| !leg.accepted)
                        .map(|leg| {
                            json!({
                                "leg_index": leg.leg_index,
                                "reason": leg.skip_reason.clone().unwrap_or_default(),
                            })
                        })
                        .collect();
                    json!({
                        "strategy_id": s.strategy_id,
                        "full_series_margin_quote": s.full_series_margin_quote,
                        "full_series_notional_quote": s.full_series_notional_quote,
                        "budget_capped_margin_quote": s.budget_capped_margin_quote,
                        "budget_capped_notional_quote": s.budget_capped_notional_quote,
                        "first_leg_margin_quote": s.first_leg_margin_quote,
                        "first_leg_notional_quote": s.first_leg_notional_quote,
                        "first_leg_accepted": s.first_leg_accepted,
                        "strategy_margin_cap_quote": s.strategy_margin_cap_quote,
                        "skipped_legs": skipped_legs,
                    })
                })
                .collect();
            map.insert(
                "live_start_preflight".to_owned(),
                serde_json::json!({
                    "checked_at": Utc::now().to_rfc3339(),
                    "capital_model": "margin_budget_cap",
                    "max_global_budget_quote": live_budget.map(|value| value.to_string()),
                    "full_series_projected_margin_quote": projection.full_series_margin_quote,
                    "full_series_projected_notional_quote": projection.full_series_notional_quote,
                    "budget_capped_projected_margin_quote": projection.budget_capped_margin_quote,
                    "budget_capped_projected_notional_quote": projection.budget_capped_notional_quote,
                    "first_leg_margin_quote": projection.first_leg_margin_quote,
                    "first_leg_notional_quote": projection.first_leg_notional_quote,
                    "projected_fee_quote": projection.projected_fee_quote,
                    "required_with_buffer_quote": projection.required_with_buffer_quote,
                    "available_usdt": available_usdt,
                    "all_strategies_can_start": projection.all_strategies_can_start,
                    "per_strategy": per_strategy,
                    "status": "passed",
                }),
            );
            map.insert(
                "live_start".to_owned(),
                serde_json::json!({
                    "confirmed_at": Utc::now().to_rfc3339(),
                    "executor_state": "pending_pickup",
                    "strategy_count": enabled_count,
                    "max_global_budget_quote": live_budget.map(|value| value.to_string()),
                }),
            );
        }
        self.repo
            .update_martingale_portfolio_config_and_risk_summary(
                owner,
                portfolio_id,
                config_value,
                risk_summary,
            )?
            .ok_or_else(|| PublishError::not_found("portfolio not found"))?;

        self.repo
            .set_martingale_portfolio_status(owner, portfolio_id, "running")?
            .ok_or_else(|| PublishError::not_found("portfolio not found"))
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
        Ok(self.repo.list_martingale_portfolios(owner)?)
    }

    pub fn get_portfolio(
        &self,
        owner: &str,
        portfolio_id: &str,
    ) -> Result<LivePortfolio, PublishError> {
        self.repo
            .get_martingale_portfolio(owner, portfolio_id)?
            .ok_or_else(|| PublishError::not_found("portfolio not found"))
    }

    pub fn validate_futures_symbol_compatibility(
        &self,
        owner: &str,
        config: &MartingalePortfolioConfig,
    ) -> Result<(), PublishError> {
        let incoming = futures_symbols_from_config(config);
        if incoming.is_empty() {
            return Ok(());
        }
        for existing in self.repo.list_martingale_portfolios(owner)? {
            if existing.status != "running" {
                continue;
            }
            for item in existing.items {
                if incoming.contains(&item.symbol.to_uppercase()) {
                    return Err(PublishError::conflict(format!(
                        "{} futures symbol conflict",
                        item.symbol
                    )));
                }
            }
        }
        Ok(())
    }

    fn set_portfolio_status(
        &self,
        owner: &str,
        portfolio_id: &str,
        status: &str,
    ) -> Result<LivePortfolio, PublishError> {
        self.repo
            .set_martingale_portfolio_status(owner, portfolio_id, status)?
            .ok_or_else(|| PublishError::not_found("portfolio not found"))
    }
}

impl From<MartingalePortfolioRecord> for PublishPortfolioResponse {
    fn from(record: MartingalePortfolioRecord) -> Self {
        let instances = record
            .items
            .into_iter()
            .map(|item| PublishedStrategyInstance {
                strategy_instance_id: item.strategy_instance_id,
                candidate_id: item.candidate_id,
                symbol: item.symbol,
                weight_pct: item.weight_pct,
                leverage: item.leverage,
                status: item.status,
            })
            .collect::<Vec<_>>();
        Self {
            portfolio_id: record.portfolio_id,
            status: record.status,
            source_task_id: record.source_task_id,
            items: instances.clone(),
            instances,
            risk_summary: record.risk_summary,
        }
    }
}

fn validate_publish_request(request: &PublishPortfolioRequest) -> Result<(), PublishError> {
    if request.items.is_empty() {
        return Err(PublishError::bad_request("items must not be empty"));
    }
    if request.total_weight_pct != Decimal::new(100, 0) {
        return Err(PublishError::bad_request("total_weight_pct must be 100"));
    }
    let mut enabled_weight = Decimal::new(0, 0);
    for item in &request.items {
        if item.weight_pct <= Decimal::new(0, 0) {
            return Err(PublishError::bad_request("item weight must be positive"));
        }
        if !(1..=125).contains(&item.leverage) {
            return Err(PublishError::bad_request(
                "leverage must be between 1 and 125",
            ));
        }
        if item.enabled {
            enabled_weight += item.weight_pct;
        }
    }
    if enabled_weight != Decimal::new(100, 0) {
        return Err(PublishError::bad_request(
            "enabled item weights must sum to 100",
        ));
    }
    Ok(())
}

fn portfolio_risk_summary(request: &PublishPortfolioRequest) -> Value {
    let symbols = request
        .items
        .iter()
        .map(|item| item.symbol.clone())
        .collect::<Vec<_>>();
    let max_leverage = request
        .items
        .iter()
        .map(|item| item.leverage)
        .max()
        .unwrap_or(1);
    // Surface the effective pause-guard thresholds (config value or engine
    // default) so users can see e.g. "this portfolio pauses new cycles at 6%
    // drawdown". Mirrors `risk_summary_for_candidate`.
    let risk_guard_thresholds = request
        .items
        .iter()
        .find(|item| item.enabled)
        .and_then(|item| {
            item.parameter_snapshot
                .get("portfolio_config")
                .and_then(|config| config.get("risk_limits"))
        })
        .map(effective_risk_guard_thresholds)
        .unwrap_or_else(default_risk_guard_thresholds);
    // Cross-symbol indicator dependencies (e.g. BTCUSDT referenced by a SOL
    // strategy's entry expression). Extracted by deserializing the first
    // enabled item's portfolio_config; if the snapshot is malformed we default
    // to no dependencies rather than fail the publish.
    let market_data_dependencies = request
        .items
        .iter()
        .find(|item| item.enabled)
        .and_then(|item| {
            item.parameter_snapshot
                .get("portfolio_config")
                .cloned()
                .or_else(|| Some(item.parameter_snapshot.clone()))
        })
        .and_then(|config_value| {
            serde_json::from_value::<MartingalePortfolioConfig>(config_value).ok()
        })
        .map(|config| {
            backtest_engine::martingale::indicator_runtime::extract_symbol_dependencies(&config)
        })
        .unwrap_or_default();
    json!({
        "strategy_count": request.items.len(),
        "enabled_strategy_count": request.items.iter().filter(|item| item.enabled).count(),
        "symbols": symbols,
        "max_leverage": max_leverage,
        "total_weight_pct": request.total_weight_pct,
        "risk_guard_thresholds": risk_guard_thresholds,
        "market_data_dependencies": market_data_dependencies,
    })
}

/// Build a JSON view of the effective pause-guard thresholds from a portfolio's
/// `risk_limits` object. Values absent from the config fall back to the engine
/// defaults, matching `backtest_engine::martingale::kline_engine` and
/// `trading_engine::martingale_runtime`. Surfaced in `risk_summary` so the
/// effective guard behavior is visible without reading the config blob.
fn effective_risk_guard_thresholds(risk_limits: &Value) -> Value {
    let read_f64 = |field: &str| -> Option<f64> {
        risk_limits
            .get(field)
            .and_then(|value| value.as_f64())
            .filter(|value| value.is_finite() && *value >= 0.0)
    };
    json!({
        "new_cycle_drawdown_pause_pct": read_f64("new_cycle_drawdown_pause_pct").unwrap_or(6.0),
        "new_cycle_atr_pause_pct": read_f64("new_cycle_atr_pause_pct").unwrap_or(2.0),
        "safety_skip_adx_threshold": read_f64("safety_skip_adx_threshold").unwrap_or(45.0),
    })
}

/// Engine-default guard thresholds, used when a portfolio carries no
/// `risk_limits` at all (e.g. legacy candidates).
fn default_risk_guard_thresholds() -> Value {
    json!({
        "new_cycle_drawdown_pause_pct": 6.0,
        "new_cycle_atr_pause_pct": 2.0,
        "safety_skip_adx_threshold": 45.0,
    })
}

fn publish_parameter_snapshot(
    item: &PublishPortfolioItemRequest,
    candidate: &BacktestCandidateRecord,
) -> Value {
    match &item.parameter_snapshot {
        Value::Null => candidate.config.clone(),
        Value::Object(map) if map.is_empty() => candidate.config.clone(),
        _ => item.parameter_snapshot.clone(),
    }
}

fn live_portfolio_config_snapshot(
    request: &PublishPortfolioRequest,
    items: &[NewMartingalePortfolioItemRecord],
) -> Value {
    let strategies = items
        .iter()
        .filter(|item| item.enabled)
        .flat_map(|item| {
            let inner_strategies = item
                .parameter_snapshot
                .get("portfolio_config")
                .and_then(|config| config.get("strategies"))
                .and_then(Value::as_array);
            // One candidate may expand into multiple internal strategies (e.g.
            // long + short). Divide the item's portfolio weight equally among
            // them so a single 100%-weighted long/short item does not reserve
            // 200% of the capital.
            let internal_count = inner_strategies
                .map(|strategies| strategies.len())
                .filter(|count| *count > 0)
                .unwrap_or(1);
            let per_strategy_weight = item.weight_pct / Decimal::from(internal_count as u64);
            inner_strategies
                .into_iter()
                .flatten()
                .map(move |strategy| {
                    let mut strategy = strategy.clone();
                    if let Value::Object(ref mut map) = strategy {
                        map.insert(
                            "portfolio_weight_pct".to_owned(),
                            Value::String(per_strategy_weight.to_string()),
                        );
                        map.insert(
                            "strategy_instance_id".to_owned(),
                            Value::String(item.strategy_instance_id.clone()),
                        );
                    }
                    strategy
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    // Carry forward the portfolio-level risk_limits from the first enabled
    // candidate's parameter_snapshot, so parity-structured fields (the
    // new_cycle_*_pause_pct / safety_skip_adx_threshold thresholds) survive
    // publish. The live budget is merged into this object later by
    // `set_live_budget_in_config` (it inserts rather than replaces). If a
    // candidate carries no risk_limits, fall back to an empty object as before.
    let portfolio_risk_limits = items
        .iter()
        .find(|item| item.enabled)
        .and_then(|item| {
            item.parameter_snapshot
                .get("portfolio_config")
                .and_then(|config| config.get("risk_limits"))
                .cloned()
        })
        .unwrap_or_else(|| json!({}));

    json!({
        "kind": "martingale_batch_portfolio",
        "market": request.market,
        "direction": request.direction,
        "risk_profile": request.risk_profile,
        "total_weight_pct": request.total_weight_pct,
        "portfolio_config": {
            "direction_mode": if request.direction == "long_short" { "long_and_short" } else { request.direction.as_str() },
            "strategies": strategies,
            "risk_limits": portfolio_risk_limits,
        },
        "execution": {
            "requires_connected_strategy_executor": true,
            "source": "backtest_candidate_parameter_snapshot",
        }
    })
}

/// Extract per-strategy weight factors (weight_pct/100) keyed by strategy_id,
/// from the raw portfolio config JSON (the same source the runtime reads).
/// Strategies missing a weight get no entry (project_portfolio_capital falls
/// back to equal split).
fn extract_portfolio_weight_factors(
    config_value: &serde_json::Value,
) -> std::collections::HashMap<String, f64> {
    use std::collections::HashMap;
    let mut weights: HashMap<String, f64> = HashMap::new();
    let Some(strategies) = config_value
        .get("portfolio_config")
        .and_then(|pc| pc.get("strategies"))
        .and_then(|s| s.as_array())
    else {
        return weights;
    };
    for strategy in strategies {
        let Some(strategy_id) = strategy.get("strategy_id").and_then(|v| v.as_str()) else {
            continue;
        };
        // `portfolio_weight_pct` is injected as a JSON string at publish time.
        let weight = strategy
            .get("portfolio_weight_pct")
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .or_else(|| v.as_f64())
            })
            .unwrap_or(0.0);
        if weight > 0.0 {
            weights.insert(strategy_id.to_owned(), weight / 100.0);
        }
    }
    weights
}

/// Pure preflight gate. Returns Some(reason) to reject (409), None to pass.
/// Reject when:
///   - global margin cap <= 0 (defensive),
///   - NOT all_strategies_can_start (some strategy's first leg cannot fit the
///     global margin pool — reason must list which strategy_ids and why),
///   - available_usdt is Some and < required_with_buffer_quote.
fn preflight_rejection_reason(
    projection: &PortfolioCapitalProjection,
    available_usdt: Option<f64>,
) -> Option<String> {
    if projection.global_margin_cap_quote <= 0.0 {
        return Some("max_global_budget_quote must be positive".to_owned());
    }
    if !projection.all_strategies_can_start {
        let blocked: Vec<&str> = projection
            .strategies
            .iter()
            .filter(|s| !s.first_leg_accepted)
            .map(|s| s.strategy_id.as_str())
            .collect();
        return Some(format!(
            "strategy first leg cannot fit global margin cap {:.4}; blocked strategies: {}; first-leg margin sum exceeds cap",
            projection.global_margin_cap_quote,
            blocked.join(", ")
        ));
    }
    if let Some(usdt) = available_usdt {
        if usdt < projection.required_with_buffer_quote {
            return Some(format!(
                "required capital with buffer {:.4} exceeds available USDT {:.4}",
                projection.required_with_buffer_quote, usdt
            ));
        }
    }
    None
}

fn candidate_publish_metadata(config: &MartingalePortfolioConfig) -> (String, String) {
    let market = if config
        .strategies
        .iter()
        .any(|strategy| strategy.market == MartingaleMarketKind::UsdMFutures)
    {
        "usd_m_futures"
    } else {
        "spot"
    };
    let has_long = config.strategies.iter().any(|strategy| {
        matches!(
            strategy.direction,
            shared_domain::martingale::MartingaleDirection::Long
        )
    });
    let has_short = config.strategies.iter().any(|strategy| {
        matches!(
            strategy.direction,
            shared_domain::martingale::MartingaleDirection::Short
        )
    });
    let direction = match (has_long, has_short) {
        (true, true) => "long_short",
        (false, true) => "short",
        _ => "long",
    };
    (market.to_owned(), direction.to_owned())
}

fn validate_running_futures_conflicts(
    portfolios: &[MartingalePortfolioRecord],
    starting: &MartingalePortfolioRecord,
) -> Result<(), PublishError> {
    let incoming = starting
        .items
        .iter()
        .map(|item| item.symbol.to_uppercase())
        .collect::<BTreeSet<_>>();
    if incoming.is_empty() || starting.market != "usd_m_futures" && starting.market != "futures" {
        return Ok(());
    }
    for existing in portfolios.iter().filter(|portfolio| {
        portfolio.portfolio_id != starting.portfolio_id
            && matches!(portfolio.status.as_str(), "running" | "paused")
    }) {
        if existing.market != "usd_m_futures" && existing.market != "futures" {
            continue;
        }
        for item in &existing.items {
            if incoming.contains(&item.symbol.to_uppercase()) {
                return Err(PublishError::conflict(format!(
                    "{} futures symbol conflict",
                    item.symbol
                )));
            }
        }
    }
    Ok(())
}

fn futures_symbols_from_config(config: &MartingalePortfolioConfig) -> BTreeSet<String> {
    config
        .strategies
        .iter()
        .filter(|strategy| strategy.market == MartingaleMarketKind::UsdMFutures)
        .map(|strategy| strategy.symbol.to_uppercase())
        .collect()
}

fn set_live_budget_in_config(config: &mut Value, budget: Decimal) -> Result<(), PublishError> {
    let Some(portfolio_config) = config
        .get_mut("portfolio_config")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Err(PublishError::conflict("portfolio_config is missing"));
    };
    let risk_limits = portfolio_config
        .entry("risk_limits".to_owned())
        .or_insert_with(|| json!({}));
    if !risk_limits.is_object() {
        *risk_limits = json!({});
    }
    let Some(risk_limits_map) = risk_limits.as_object_mut() else {
        return Err(PublishError::conflict("portfolio risk_limits is invalid"));
    };
    risk_limits_map.insert(
        "max_global_budget_quote".to_owned(),
        Value::String(budget.to_string()),
    );
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

fn default_enabled() -> bool {
    true
}

fn live_exchange_state_check_enabled() -> bool {
    std::env::var("BINANCE_LIVE_MODE")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn uuid_simple() -> String {
    let mut bytes = [0_u8; 16];
    getrandom::getrandom(&mut bytes).expect("random id generation should succeed");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Debug)]
pub struct PublishError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
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
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }
}

impl From<shared_db::SharedDbError> for PublishError {
    fn from(error: shared_db::SharedDbError) -> Self {
        Self::internal(error.to_string())
    }
}

impl IntoResponse for PublishError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_db::{NewBacktestCandidateRecord, NewBacktestTaskRecord};
    use shared_domain::strategy::Decimal;

    fn publish_request(
        task_id: &str,
        first_candidate_id: &str,
        second_candidate_id: &str,
    ) -> PublishPortfolioRequest {
        PublishPortfolioRequest {
            name: "BTC basket".to_owned(),
            task_id: task_id.to_owned(),
            market: "futures".to_owned(),
            direction: "long".to_owned(),
            risk_profile: "balanced".to_owned(),
            total_weight_pct: Decimal::new(100, 0),
            items: vec![
                PublishPortfolioItemRequest {
                    candidate_id: first_candidate_id.to_owned(),
                    symbol: "BTCUSDT".to_owned(),
                    weight_pct: Decimal::new(50, 0),
                    leverage: 3,
                    enabled: true,
                    parameter_snapshot: json!({ "spacing": "0.01" }),
                },
                PublishPortfolioItemRequest {
                    candidate_id: second_candidate_id.to_owned(),
                    symbol: "BTCUSDT".to_owned(),
                    weight_pct: Decimal::new(50, 0),
                    leverage: 5,
                    enabled: true,
                    parameter_snapshot: json!({ "spacing": "0.02" }),
                },
            ],
        }
    }

    fn ready_candidate(task_id: &str, rank: i32) -> NewBacktestCandidateRecord {
        NewBacktestCandidateRecord {
            task_id: task_id.to_owned(),
            status: "ready".to_owned(),
            rank,
            config: json!({ "symbol": "BTCUSDT", "rank": rank }),
            summary: json!({ "score": rank }),
        }
    }

    fn long_short_candidate(task_id: &str) -> NewBacktestCandidateRecord {
        NewBacktestCandidateRecord {
            task_id: task_id.to_owned(),
            status: "ready".to_owned(),
            rank: 1,
            config: json!({
                "portfolio_config": {
                    "direction_mode": "long_and_short",
                    "risk_limits": {},
                    "strategies": [
                        {
                            "strategy_id": "BTCUSDT-long-v1",
                            "symbol": "BTCUSDT",
                            "market": "usd_m_futures",
                            "direction": "long",
                            "direction_mode": "long_and_short",
                            "margin_mode": "isolated",
                            "leverage": 4,
                            "spacing": { "fixed_percent": { "step_bps": 90 } },
                            "sizing": {
                                "budget_scaled": {
                                    "first_order_quote": "10",
                                    "multiplier": "1.8",
                                    "max_legs": 7,
                                    "max_budget_quote": "150"
                                }
                            },
                            "take_profit": { "percent": { "bps": 80 } },
                            "stop_loss": { "strategy_drawdown_pct": { "pct_bps": 1800 } },
                            "indicators": [],
                            "entry_triggers": ["immediate"],
                            "risk_limits": {}
                        },
                        {
                            "strategy_id": "BTCUSDT-short-v1",
                            "symbol": "BTCUSDT",
                            "market": "usd_m_futures",
                            "direction": "short",
                            "direction_mode": "long_and_short",
                            "margin_mode": "isolated",
                            "leverage": 4,
                            "spacing": { "fixed_percent": { "step_bps": 130 } },
                            "sizing": {
                                "budget_scaled": {
                                    "first_order_quote": "8",
                                    "multiplier": "1.6",
                                    "max_legs": 6,
                                    "max_budget_quote": "120"
                                }
                            },
                            "take_profit": { "percent": { "bps": 95 } },
                            "stop_loss": { "strategy_drawdown_pct": { "pct_bps": 1400 } },
                            "indicators": [],
                            "entry_triggers": ["immediate"],
                            "risk_limits": {}
                        }
                    ]
                }
            }),
            summary: json!({ "annualized_return_pct": 68.9, "max_drawdown_pct": 19.8 }),
        }
    }

    fn task_record(owner: &str) -> NewBacktestTaskRecord {
        NewBacktestTaskRecord {
            owner: owner.to_owned(),
            strategy_type: "martingale_grid".to_owned(),
            config: json!({ "symbol": "BTCUSDT", "timeframe": "1h" }),
            summary: json!({}),
        }
    }

    #[test]
    fn publishes_two_btc_items_with_distinct_strategy_instances() {
        let db = SharedDb::ephemeral().expect("db");
        let repo = db.backtest_repo();
        let task = repo
            .create_task(task_record("user@example.com"))
            .expect("task");
        repo.transition_task(&task.task_id, "succeeded")
            .expect("succeeded");
        let first = repo
            .save_candidate(ready_candidate(&task.task_id, 1))
            .expect("first");
        let second = repo
            .save_candidate(ready_candidate(&task.task_id, 2))
            .expect("second");
        let service = MartingalePublishService::new(db.clone());

        let response = service
            .publish_portfolio(
                "user@example.com",
                publish_request(&task.task_id, &first.candidate_id, &second.candidate_id),
                vec![first, second],
            )
            .expect("published");

        assert_eq!(response.status, "pending_confirmation");
        assert_eq!(response.source_task_id, task.task_id);
        assert_eq!(response.items.len(), 2);
        assert_ne!(
            response.items[0].strategy_instance_id,
            response.items[1].strategy_instance_id
        );
        assert!(response.items.iter().all(|item| item.symbol == "BTCUSDT"));
        assert_eq!(
            service
                .get_portfolio("user@example.com", &response.portfolio_id)
                .unwrap()
                .items
                .len(),
            2
        );
    }

    #[test]
    fn rejects_enabled_weight_sum_below_one_hundred() {
        let db = SharedDb::ephemeral().expect("db");
        let repo = db.backtest_repo();
        let task = repo
            .create_task(task_record("user@example.com"))
            .expect("task");
        let first = repo
            .save_candidate(ready_candidate(&task.task_id, 1))
            .expect("first");
        let second = repo
            .save_candidate(ready_candidate(&task.task_id, 2))
            .expect("second");
        let mut request = publish_request(&task.task_id, &first.candidate_id, &second.candidate_id);
        request.items[1].weight_pct = Decimal::new(40, 0);
        let service = MartingalePublishService::new(db);

        let error = service
            .publish_portfolio("user@example.com", request, vec![first, second])
            .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "enabled item weights must sum to 100");
    }

    #[test]
    fn rejects_candidate_from_other_task_or_owner() {
        let db = SharedDb::ephemeral().expect("db");
        let repo = db.backtest_repo();
        let owner_task = repo
            .create_task(task_record("user@example.com"))
            .expect("owner task");
        let other_task = repo
            .create_task(task_record("other@example.com"))
            .expect("other task");
        let first = repo
            .save_candidate(ready_candidate(&owner_task.task_id, 1))
            .expect("first");
        let other = repo
            .save_candidate(ready_candidate(&other_task.task_id, 2))
            .expect("other");
        let request = publish_request(
            &owner_task.task_id,
            &first.candidate_id,
            &other.candidate_id,
        );
        let service = MartingalePublishService::new(db);

        let error = service
            .publish_portfolio("user@example.com", request, vec![first, other])
            .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "candidate does not belong to task");
    }

    #[test]
    fn publish_falls_back_to_complete_candidate_config_when_snapshot_is_empty() {
        let db = SharedDb::ephemeral().expect("db");
        let repo = db.backtest_repo();
        let task = repo
            .create_task(task_record("user@example.com"))
            .expect("task");
        repo.transition_task(&task.task_id, "succeeded")
            .expect("succeeded");
        let candidate = repo
            .save_candidate(long_short_candidate(&task.task_id))
            .expect("candidate");
        let mut request = PublishPortfolioRequest {
            name: "BTC long short sandbox".to_owned(),
            task_id: task.task_id.clone(),
            market: "usd_m_futures".to_owned(),
            direction: "long_short".to_owned(),
            risk_profile: "aggressive".to_owned(),
            total_weight_pct: Decimal::new(100, 0),
            items: vec![PublishPortfolioItemRequest {
                candidate_id: candidate.candidate_id.clone(),
                symbol: "BTCUSDT".to_owned(),
                weight_pct: Decimal::new(100, 0),
                leverage: 4,
                enabled: true,
                parameter_snapshot: json!({}),
            }],
        };
        let service = MartingalePublishService::new(db.clone());

        let response = service
            .publish_portfolio("user@example.com", request.clone(), vec![candidate.clone()])
            .expect("published with empty snapshot fallback");
        let portfolio = service
            .get_portfolio("user@example.com", &response.portfolio_id)
            .expect("portfolio");
        let snapshot = &portfolio.items[0].parameter_snapshot;
        assert_eq!(snapshot, &candidate.config);
        assert_eq!(
            snapshot["portfolio_config"]["direction_mode"],
            "long_and_short"
        );
        assert_eq!(
            snapshot["portfolio_config"]["strategies"][0]["direction"],
            "long"
        );
        assert_eq!(
            snapshot["portfolio_config"]["strategies"][1]["direction"],
            "short"
        );
        assert_eq!(
            snapshot["portfolio_config"]["strategies"][0]["market"],
            "usd_m_futures"
        );
        assert!(snapshot["portfolio_config"]["strategies"][0]["stop_loss"].is_object());
        assert!(snapshot["portfolio_config"]["strategies"][1]["spacing"].is_object());

        request.items[0].parameter_snapshot = Value::Null;
        let null_response = service
            .publish_portfolio("user@example.com", request, vec![candidate.clone()])
            .expect("published with null snapshot fallback");
        let null_portfolio = service
            .get_portfolio("user@example.com", &null_response.portfolio_id)
            .expect("null portfolio");
        assert_eq!(null_portfolio.items[0].parameter_snapshot, candidate.config);
    }

    #[test]
    fn confirm_start_portfolio_preserves_parameter_snapshot() {
        let db = SharedDb::ephemeral().expect("db");
        let repo = db.backtest_repo();
        let task = repo
            .create_task(task_record("user@example.com"))
            .expect("task");
        repo.transition_task(&task.task_id, "succeeded")
            .expect("succeeded");
        let candidate = repo
            .save_candidate(long_short_candidate(&task.task_id))
            .expect("candidate");
        let service = MartingalePublishService::new(db.clone());
        let response = service
            .publish_portfolio(
                "user@example.com",
                PublishPortfolioRequest {
                    name: "BTC long short live".to_owned(),
                    task_id: task.task_id.clone(),
                    market: "usd_m_futures".to_owned(),
                    direction: "long_short".to_owned(),
                    risk_profile: "balanced".to_owned(),
                    total_weight_pct: Decimal::new(100, 0),
                    items: vec![PublishPortfolioItemRequest {
                        candidate_id: candidate.candidate_id.clone(),
                        symbol: "BTCUSDT".to_owned(),
                        weight_pct: Decimal::new(100, 0),
                        leverage: 4,
                        enabled: true,
                        parameter_snapshot: candidate.config.clone(),
                    }],
                },
                vec![candidate.clone()],
            )
            .expect("published");

        // Seed exchange_preconfigure readiness required by the start gate.
        repo.update_martingale_portfolio_risk_summary(
            "user@example.com",
            &response.portfolio_id,
            serde_json::json!({
                "exchange_preconfigure": {
                    "status": "ready",
                    "checked_at": chrono::Utc::now().to_rfc3339(),
                    "applied": true,
                    "hedge_mode": {"status": "ready", "target": true, "current": true, "message": "ok"},
                    "symbols": [{"symbol": "BTCUSDT", "status": "ready", "message": "ok"}],
                    "warnings": [],
                    "blocked_symbols": [],
                    "open_order_count": 0,
                    "nonzero_position_count": 0,
                }
            }),
        )
        .expect("seed exchange_preconfigure");

        let running = service
            .confirm_start_portfolio(
                "user@example.com",
                &response.portfolio_id,
                ConfirmStartPortfolioRequest {
                    max_global_budget_quote: Some(Decimal::new(2000, 0)),
                },
            )
            .expect("running");

        assert_eq!(running.status, "running");
        assert_eq!(running.items.len(), 1);
        assert_eq!(running.items[0].parameter_snapshot, candidate.config);
        assert_eq!(
            running.items[0].parameter_snapshot["portfolio_config"]["strategies"][1]["direction"],
            "short"
        );
        assert_eq!(
            running.config["portfolio_config"]["direction_mode"],
            "long_and_short"
        );
        assert_eq!(
            running.config["portfolio_config"]["strategies"][0]["leverage"],
            4
        );
        assert_eq!(
            running.config["portfolio_config"]["strategies"][1]["direction"],
            "short"
        );
        assert!(running.config["portfolio_config"]["strategies"][1]["stop_loss"].is_object());
        assert!(
            running.config["portfolio_config"]["strategies"][0]["portfolio_weight_pct"].is_string()
        );
        assert_eq!(
            running.config["portfolio_config"]["risk_limits"]["max_global_budget_quote"],
            "2000"
        );
    }

    #[test]
    fn published_config_carries_candidate_risk_guard_thresholds() {
        // P0.1d: parity-structured pause thresholds in the candidate's portfolio
        // risk_limits must survive publish (not be reset to {}), so they reach
        // the live trading-engine. The live budget is merged in afterward.
        let db = SharedDb::ephemeral().expect("db");
        let repo = db.backtest_repo();
        let task = repo
            .create_task(task_record("user@example.com"))
            .expect("task");
        repo.transition_task(&task.task_id, "succeeded")
            .expect("succeeded");
        let mut candidate = long_short_candidate(&task.task_id);
        // Inject structured guard thresholds into the candidate's portfolio risk_limits.
        candidate.config["portfolio_config"]["risk_limits"] = json!({
            "new_cycle_drawdown_pause_pct": 8.0,
            "new_cycle_atr_pause_pct": 1.5,
            "safety_skip_adx_threshold": 50.0,
        });
        let candidate = repo.save_candidate(candidate).expect("candidate");
        let service = MartingalePublishService::new(db.clone());
        let response = service
            .publish_portfolio(
                "user@example.com",
                PublishPortfolioRequest {
                    name: "BTC thresholds".to_owned(),
                    task_id: task.task_id.clone(),
                    market: "usd_m_futures".to_owned(),
                    direction: "long_short".to_owned(),
                    risk_profile: "balanced".to_owned(),
                    total_weight_pct: Decimal::new(100, 0),
                    items: vec![PublishPortfolioItemRequest {
                        candidate_id: candidate.candidate_id.clone(),
                        symbol: "BTCUSDT".to_owned(),
                        weight_pct: Decimal::new(100, 0),
                        leverage: 4,
                        enabled: true,
                        parameter_snapshot: candidate.config.clone(),
                    }],
                },
                vec![candidate.clone()],
            )
            .expect("published");

        // Read the stored portfolio back: its config must carry the structured
        // thresholds (not reset to {}), and risk_summary must surface them.
        let stored = service
            .get_portfolio("user@example.com", &response.portfolio_id)
            .expect("stored portfolio");

        let limits = &stored.config["portfolio_config"]["risk_limits"];
        assert_eq!(limits["new_cycle_drawdown_pause_pct"], 8.0);
        assert_eq!(limits["new_cycle_atr_pause_pct"], 1.5);
        assert_eq!(limits["safety_skip_adx_threshold"], 50.0);

        let guard = &stored.risk_summary["risk_guard_thresholds"];
        assert_eq!(guard["new_cycle_drawdown_pause_pct"], 8.0);
        assert_eq!(guard["new_cycle_atr_pause_pct"], 1.5);
        assert_eq!(guard["safety_skip_adx_threshold"], 50.0);
    }

    #[test]
    fn risk_summary_falls_back_to_default_guard_thresholds_when_unset() {
        // P0.1d: a candidate without structured thresholds still reports the
        // engine defaults in risk_summary, so the UI shows consistent guard info.
        let db = SharedDb::ephemeral().expect("db");
        let repo = db.backtest_repo();
        let task = repo
            .create_task(task_record("user@example.com"))
            .expect("task");
        repo.transition_task(&task.task_id, "succeeded")
            .expect("succeeded");
        let candidate = repo
            .save_candidate(long_short_candidate(&task.task_id))
            .expect("candidate");
        let service = MartingalePublishService::new(db.clone());
        let response = service
            .publish_portfolio(
                "user@example.com",
                PublishPortfolioRequest {
                    name: "BTC defaults".to_owned(),
                    task_id: task.task_id.clone(),
                    market: "usd_m_futures".to_owned(),
                    direction: "long_short".to_owned(),
                    risk_profile: "balanced".to_owned(),
                    total_weight_pct: Decimal::new(100, 0),
                    items: vec![PublishPortfolioItemRequest {
                        candidate_id: candidate.candidate_id.clone(),
                        symbol: "BTCUSDT".to_owned(),
                        weight_pct: Decimal::new(100, 0),
                        leverage: 4,
                        enabled: true,
                        parameter_snapshot: candidate.config.clone(),
                    }],
                },
                vec![candidate.clone()],
            )
            .expect("published");

        let stored = service
            .get_portfolio("user@example.com", &response.portfolio_id)
            .expect("stored portfolio");

        let guard = &stored.risk_summary["risk_guard_thresholds"];
        assert_eq!(guard["new_cycle_drawdown_pause_pct"], 6.0);
        assert_eq!(guard["new_cycle_atr_pause_pct"], 2.0);
        assert_eq!(guard["safety_skip_adx_threshold"], 45.0);
    }

    #[test]
    fn risk_summary_surfaces_market_data_dependencies_for_cross_symbol_expression() {
        // P0.2d: a strategy whose entry trigger references BTCUSDT must list
        // BTCUSDT in market_data_dependencies so operators know the portfolio
        // needs BTC market-data feeds even though BTC is never traded.
        let db = SharedDb::ephemeral().expect("db");
        let repo = db.backtest_repo();
        let task = repo
            .create_task(task_record("user@example.com"))
            .expect("task");
        repo.transition_task(&task.task_id, "succeeded")
            .expect("succeeded");
        let mut candidate = long_short_candidate(&task.task_id);
        // Give the long leg a cross-symbol entry trigger referencing BTCUSDT.
        // (The candidate already trades BTCUSDT, so add a second referenced
        // symbol ETHUSDT to ensure a genuine non-traded dependency surfaces.)
        candidate.config["portfolio_config"]["strategies"][0]["entry_triggers"] = json!([
            { "indicator_expression": { "expression": "close > ETHUSDT.ema(50)" } }
        ]);
        let candidate = repo.save_candidate(candidate).expect("candidate");
        let service = MartingalePublishService::new(db.clone());
        let response = service
            .publish_portfolio(
                "user@example.com",
                PublishPortfolioRequest {
                    name: "BTC cross-symbol".to_owned(),
                    task_id: task.task_id.clone(),
                    market: "usd_m_futures".to_owned(),
                    direction: "long_short".to_owned(),
                    risk_profile: "balanced".to_owned(),
                    total_weight_pct: Decimal::new(100, 0),
                    items: vec![PublishPortfolioItemRequest {
                        candidate_id: candidate.candidate_id.clone(),
                        symbol: "BTCUSDT".to_owned(),
                        weight_pct: Decimal::new(100, 0),
                        leverage: 4,
                        enabled: true,
                        parameter_snapshot: candidate.config.clone(),
                    }],
                },
                vec![candidate.clone()],
            )
            .expect("published");

        let stored = service
            .get_portfolio("user@example.com", &response.portfolio_id)
            .expect("stored portfolio");

        let deps = stored.risk_summary["market_data_dependencies"]
            .as_array()
            .expect("market_data_dependencies should be an array");
        let dep_symbols: Vec<&str> = deps.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            dep_symbols.contains(&"ETHUSDT"),
            "ETHUSDT (referenced, non-traded) must be a dependency, got: {dep_symbols:?}"
        );
        // BTCUSDT is traded, so it must NOT appear as a dependency.
        assert!(
            !dep_symbols.contains(&"BTCUSDT"),
            "BTCUSDT (traded) must not be a dependency, got: {dep_symbols:?}"
        );
    }

    #[test]
    fn publish_long_short_weight_does_not_double_capital() {
        // One candidate with 100% weight that expands into long + short internal
        // strategies must split the weight (50/50), not assign 100% to each
        // (which would reserve 200% of the capital).
        let db = SharedDb::ephemeral().expect("db");
        let repo = db.backtest_repo();
        let task = repo
            .create_task(task_record("user@example.com"))
            .expect("task");
        repo.transition_task(&task.task_id, "succeeded")
            .expect("succeeded");
        let candidate = repo
            .save_candidate(long_short_candidate(&task.task_id))
            .expect("candidate");
        let service = MartingalePublishService::new(db.clone());
        let response = service
            .publish_portfolio(
                "user@example.com",
                PublishPortfolioRequest {
                    name: "weight split live".to_owned(),
                    task_id: task.task_id.clone(),
                    market: "usd_m_futures".to_owned(),
                    direction: "long_short".to_owned(),
                    risk_profile: "balanced".to_owned(),
                    total_weight_pct: Decimal::new(100, 0),
                    items: vec![PublishPortfolioItemRequest {
                        candidate_id: candidate.candidate_id.clone(),
                        symbol: "BTCUSDT".to_owned(),
                        weight_pct: Decimal::new(100, 0),
                        leverage: 4,
                        enabled: true,
                        parameter_snapshot: candidate.config.clone(),
                    }],
                },
                vec![candidate.clone()],
            )
            .expect("published");

        let portfolio = service
            .get_portfolio("user@example.com", &response.portfolio_id)
            .expect("portfolio");
        let strategies = portfolio.config["portfolio_config"]["strategies"]
            .as_array()
            .expect("strategies");
        assert_eq!(strategies.len(), 2, "long + short internal strategies");
        let weights: Vec<String> = strategies
            .iter()
            .map(|s| s["portfolio_weight_pct"].as_str().unwrap_or("").to_owned())
            .collect();
        // 100% / 2 internal strategies = 50% each (not 100% each).
        assert_eq!(weights, vec!["50".to_owned(), "50".to_owned()]);
    }

    // --- Budget-capped preflight pure tests ---------------------------------
    //
    // These exercise the pure gate (project_portfolio_capital ->
    // preflight_rejection_reason) without touching the DB. Single Multiplier
    // strategy: first_order_quote=250, multiplier=2, max_legs=4, leverage=5,
    // weight factor 1.0. First-leg notional=250 -> first-leg margin=250/5=50.

    fn multiplier_strategy_config() -> shared_domain::martingale::MartingalePortfolioConfig {
        use shared_domain::martingale::{
            MartingaleDirection, MartingaleDirectionMode, MartingaleMarginMode,
            MartingaleMarketKind, MartingalePortfolioConfig, MartingaleRiskLimits,
            MartingaleSizingModel, MartingaleSpacingModel, MartingaleStrategyConfig,
            MartingaleTakeProfitModel,
        };
        let strategy = MartingaleStrategyConfig {
            strategy_id: "s1".to_owned(),
            symbol: "BTCUSDT".to_owned(),
            market: MartingaleMarketKind::UsdMFutures,
            direction: MartingaleDirection::Long,
            direction_mode: MartingaleDirectionMode::LongOnly,
            margin_mode: Some(MartingaleMarginMode::Isolated),
            leverage: Some(5),
            spacing: MartingaleSpacingModel::Multiplier {
                first_step_bps: 100,
                multiplier: Decimal::from(2),
            },
            sizing: MartingaleSizingModel::Multiplier {
                first_order_quote: Decimal::from(250),
                multiplier: Decimal::from(2),
                max_legs: 4,
            },
            take_profit: MartingaleTakeProfitModel::Percent { bps: 80 },
            stop_loss: None,
            indicators: vec![],
            entry_triggers: vec![],
            risk_limits: MartingaleRiskLimits::default(),
        };
        MartingalePortfolioConfig {
            direction_mode: MartingaleDirectionMode::LongOnly,
            strategies: vec![strategy],
            risk_limits: MartingaleRiskLimits {
                max_global_budget_quote: Some(Decimal::from(50)),
                ..Default::default()
            },
        }
    }

    fn multiplier_weights() -> std::collections::HashMap<String, f64> {
        let mut w = std::collections::HashMap::new();
        w.insert("s1".to_owned(), 1.0);
        w
    }

    #[test]
    fn confirm_start_accepts_multiplier_when_budget_capped_projection_fits_margin_cap() {
        let config = multiplier_strategy_config();
        let proj = backtest_engine::martingale::capital::project_portfolio_capital(
            &config.strategies,
            &multiplier_weights(),
            50.0, // global_margin_cap
            0.0,  // exchange_min_notional (diagnostic only here)
            4.5,  // ENTRY_FEE_BPS
            5.0,  // fee_buffer_pct (5% buffer)
        )
        .expect("projection");
        // First-leg margin = 250 / 5 = 50; cap is 50 -> first leg accepted.
        assert!((proj.first_leg_margin_quote - 50.0).abs() < 1e-6);
        assert!(proj.strategies[0].first_leg_accepted);
        assert!(proj.all_strategies_can_start);
        // budget_capped margin is just the first leg (leg1 margin 100, cum 150 > 50).
        assert!((proj.budget_capped_margin_quote - 50.0).abs() < 1e-6);
        assert!(preflight_rejection_reason(&proj, None).is_none());
    }

    #[test]
    fn confirm_start_does_not_treat_leveraged_notional_as_budget() {
        let config = multiplier_strategy_config();
        let proj = backtest_engine::martingale::capital::project_portfolio_capital(
            &config.strategies,
            &multiplier_weights(),
            50.0,
            0.0,
            4.5,
            5.0,
        )
        .expect("projection");
        // First-leg notional is 250 — greater than the budget 50. Notional is
        // position size, not margin; it must not gate the margin-budget preflight.
        assert!((proj.first_leg_notional_quote - 250.0).abs() < 1e-6);
        assert!(preflight_rejection_reason(&proj, None).is_none());
    }

    #[test]
    fn confirm_start_rejects_when_first_leg_margin_exceeds_budget() {
        let config = multiplier_strategy_config();
        let proj = backtest_engine::martingale::capital::project_portfolio_capital(
            &config.strategies,
            &multiplier_weights(),
            10.0, // global_margin_cap far below first-leg margin 50
            0.0,
            4.5,
            5.0,
        )
        .expect("projection");
        assert!(!proj.strategies[0].first_leg_accepted);
        assert!(!proj.all_strategies_can_start);
        let reason = preflight_rejection_reason(&proj, None).expect("must reject");
        assert!(
            reason.contains("first leg") || reason.contains("margin cap"),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn confirm_start_rejects_when_available_usdt_below_margin_plus_fee_buffer() {
        let config = multiplier_strategy_config();
        // Use the #1 projection (passes the margin gate): cap 50, first-leg
        // margin 50, accepted notional 250 (leg1 skipped).
        let proj = backtest_engine::martingale::capital::project_portfolio_capital(
            &config.strategies,
            &multiplier_weights(),
            50.0,
            0.0,
            4.5,
            5.0,
        )
        .expect("projection");
        // fee = 250 * 4.5 / 10000 = 0.1125; required = 50 * 1.05 + 0.1125 = 52.6125.
        assert!((proj.projected_fee_quote - 0.1125).abs() < 1e-6);
        assert!((proj.required_with_buffer_quote - 52.6125).abs() < 1e-6);
        let reason = preflight_rejection_reason(&proj, Some(40.0)).expect("must reject");
        assert!(
            reason.contains("available USDT"),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn confirm_start_records_full_series_and_budget_capped_projection() {
        let db = SharedDb::ephemeral().expect("db");
        let repo = db.backtest_repo();
        let task = repo
            .create_task(task_record("user@example.com"))
            .expect("task");
        repo.transition_task(&task.task_id, "succeeded")
            .expect("succeeded");
        let candidate = repo
            .save_candidate(long_short_candidate(&task.task_id))
            .expect("candidate");
        let service = MartingalePublishService::new(db.clone());
        let response = service
            .publish_portfolio(
                "user@example.com",
                PublishPortfolioRequest {
                    name: "preflight projection live".to_owned(),
                    task_id: task.task_id.clone(),
                    market: "usd_m_futures".to_owned(),
                    direction: "long_short".to_owned(),
                    risk_profile: "balanced".to_owned(),
                    total_weight_pct: Decimal::new(100, 0),
                    items: vec![PublishPortfolioItemRequest {
                        candidate_id: candidate.candidate_id.clone(),
                        symbol: "BTCUSDT".to_owned(),
                        weight_pct: Decimal::new(100, 0),
                        leverage: 4,
                        enabled: true,
                        parameter_snapshot: candidate.config.clone(),
                    }],
                },
                vec![candidate.clone()],
            )
            .expect("published");

        repo.update_martingale_portfolio_risk_summary(
            "user@example.com",
            &response.portfolio_id,
            serde_json::json!({
                "exchange_preconfigure": {
                    "status": "ready",
                    "checked_at": chrono::Utc::now().to_rfc3339(),
                    "applied": true,
                    "hedge_mode": {"status": "ready", "target": true, "current": true, "message": "ok"},
                    "symbols": [{"symbol": "BTCUSDT", "status": "ready", "message": "ok"}],
                    "warnings": [],
                    "blocked_symbols": [],
                    "open_order_count": 0,
                    "nonzero_position_count": 0,
                }
            }),
        )
        .expect("seed exchange_preconfigure");

        service
            .confirm_start_portfolio(
                "user@example.com",
                &response.portfolio_id,
                ConfirmStartPortfolioRequest {
                    max_global_budget_quote: Some(Decimal::new(2000, 0)),
                },
            )
            .expect("running");

        let portfolio = service
            .get_portfolio("user@example.com", &response.portfolio_id)
            .expect("portfolio");
        let preflight = &portfolio.risk_summary["live_start_preflight"];
        assert_eq!(preflight["capital_model"], "margin_budget_cap");
        let full_margin = preflight["full_series_projected_margin_quote"]
            .as_f64()
            .expect("full_series_projected_margin_quote present");
        assert!(full_margin > 0.0, "full-series margin must be recorded");
        let capped_margin = preflight["budget_capped_projected_margin_quote"]
            .as_f64()
            .expect("budget_capped_projected_margin_quote present");
        assert!(
            capped_margin <= 2000.0,
            "budget-capped margin must respect the cap: {capped_margin}"
        );
        assert_eq!(preflight["all_strategies_can_start"], true);
        let per_strategy = preflight["per_strategy"]
            .as_array()
            .expect("per_strategy array");
        assert!(!per_strategy.is_empty(), "per_strategy must be non-empty");
    }
}
