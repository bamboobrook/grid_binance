use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shared_db::{
    BacktestCandidateRecord, BacktestRepository, MartingalePortfolioRecord,
    NewMartingalePortfolioItemRecord, NewMartingalePortfolioRecord, SharedDb,
};
use shared_domain::{
    martingale::{MartingaleMarketKind, MartingalePortfolioConfig},
    strategy::Decimal,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub struct MartingalePublishService {
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
    #[serde(default)]
    pub direction_mode: Option<String>,
    #[serde(default)]
    pub dynamic_allocation_enabled: bool,
    pub total_weight_pct: Decimal,
    #[serde(default)]
    pub dynamic_allocation_rules: Option<Value>,
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
    pub dynamic_allocation_rules: Option<Value>,
    pub live_ready: bool,
    pub live_readiness_blockers: Vec<String>,
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

impl MartingalePublishService {
    pub fn new(db: SharedDb) -> Self {
        Self {
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
            direction_mode: None,
            dynamic_allocation_enabled: false,
            total_weight_pct: Decimal::new(100, 0),
            dynamic_allocation_rules: None,
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
        let live_readiness_blockers = live_readiness_blockers(&request, &candidates_by_id);
        let live_ready = live_readiness_blockers.is_empty();
        let mut risk_summary = portfolio_risk_summary(&request);
        if let Some(summary) = risk_summary.as_object_mut() {
            summary.insert("live_ready".to_owned(), json!(live_ready));
            summary.insert(
                "live_readiness_blockers".to_owned(),
                json!(live_readiness_blockers.clone()),
            );
            if let Some(rules) = request.dynamic_allocation_rules.clone() {
                summary.insert("dynamic_allocation_rules".to_owned(), rules);
            }
        }
        let config = json!({
            "kind": "martingale_batch_portfolio",
            "dynamic_allocation_rules": request.dynamic_allocation_rules.clone(),
            "live_ready": live_ready,
            "live_readiness_blockers": live_readiness_blockers.clone(),
        });
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
                    parameter_snapshot: item.parameter_snapshot.clone(),
                    metrics_snapshot: candidate.summary.clone(),
                }
            })
            .collect::<Vec<_>>();
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
                config,
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
    ) -> Result<LivePortfolio, PublishError> {
        let portfolio = self.get_portfolio(owner, portfolio_id)?;
        if portfolio.status != "pending_confirmation" && portfolio.status != "paused" {
            return Err(PublishError::conflict(
                "portfolio cannot be started from current status",
            ));
        }
        validate_live_ready_for_start(&portfolio)?;
        validate_running_futures_conflicts(
            &self.repo.list_martingale_portfolios(owner)?,
            &portfolio,
        )?;
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
                        "{} leverage conflict",
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
            dynamic_allocation_rules: record.config.get("dynamic_allocation_rules").cloned(),
            live_ready: record
                .config
                .get("live_ready")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            live_readiness_blockers: record
                .config
                .get("live_readiness_blockers")
                .and_then(Value::as_array)
                .map(|blockers| {
                    blockers
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

fn live_readiness_blockers(
    request: &PublishPortfolioRequest,
    candidates_by_id: &BTreeMap<String, BacktestCandidateRecord>,
) -> Vec<String> {
    let mut blockers = Vec::new();
    let Some(raw_rules) = request.dynamic_allocation_rules.as_ref() else {
        if requires_dynamic_allocation_rules(request, candidates_by_id) {
            blockers.push(
                "dynamic allocation rules are required before direct live publish".to_owned(),
            );
        }
        return blockers;
    };
    let Some(rules) = raw_rules.as_object() else {
        blockers.push("dynamic allocation rules must be a JSON object".to_owned());
        return blockers;
    };

    let requires_forced_exit = rules
        .get("existing_position_policy")
        .and_then(Value::as_str)
        .is_some_and(|policy| policy.contains("force_exit"));
    let mut has_long_and_short =
        request.direction == "long_short" || request.direction == "long_and_short";
    let mut has_futures_prerequisites =
        request.market == "usd_m_futures" || request.market == "futures";
    let mut forced_exit_marked = rules
        .get("forced_exit_supported")
        .and_then(Value::as_bool)
        .is_some()
        || rules
            .get("forced_exit_blocked_reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| !reason.trim().is_empty());

    for item in &request.items {
        if let Some(candidate) = candidates_by_id.get(&item.candidate_id) {
            let config = candidate
                .config
                .get("portfolio_config")
                .unwrap_or(&candidate.config);
            has_long_and_short |= config
                .get("direction_mode")
                .and_then(Value::as_str)
                .is_some_and(|mode| mode == "long_and_short");
            has_futures_prerequisites |= config
                .get("strategies")
                .and_then(Value::as_array)
                .is_some_and(|strategies| {
                    strategies.iter().any(|strategy| {
                        strategy
                            .get("market")
                            .and_then(Value::as_str)
                            .is_some_and(|market| market == "usd_m_futures" || market == "futures")
                            && strategy
                                .get("margin_mode")
                                .and_then(Value::as_str)
                                .is_some_and(|mode| !mode.is_empty())
                            && strategy.get("leverage").is_some()
                    })
                });
            forced_exit_marked |= candidate
                .summary
                .get("forced_exit_supported")
                .and_then(Value::as_bool)
                .is_some()
                || candidate
                    .summary
                    .get("forced_exit_blocked_reason")
                    .and_then(Value::as_str)
                    .is_some_and(|reason| !reason.trim().is_empty());
        }
        has_long_and_short |= item
            .parameter_snapshot
            .get("direction_mode")
            .and_then(Value::as_str)
            .is_some_and(|mode| mode == "long_and_short");
    }

    if !has_long_and_short {
        blockers
            .push("dynamic long/short package requires direction mode long_and_short".to_owned());
    }
    if !has_futures_prerequisites {
        blockers.push("futures hedge and margin prerequisites are not represented".to_owned());
    }
    if requires_forced_exit && !forced_exit_marked {
        blockers.push(
            "forced exit capability is not explicitly marked supported or blocked".to_owned(),
        );
    }
    blockers
}

fn requires_dynamic_allocation_rules(
    request: &PublishPortfolioRequest,
    candidates_by_id: &BTreeMap<String, BacktestCandidateRecord>,
) -> bool {
    request.dynamic_allocation_enabled
        || is_long_and_short(request.direction.as_str())
        || request
            .direction_mode
            .as_deref()
            .is_some_and(is_long_and_short)
        || request.items.iter().any(|item| {
            value_has_long_and_short_direction(&item.parameter_snapshot)
                || value_has_dynamic_allocation_marker(&item.parameter_snapshot)
                || candidates_by_id
                    .get(&item.candidate_id)
                    .is_some_and(candidate_has_dynamic_allocation_intent)
        })
}

fn candidate_has_dynamic_allocation_intent(candidate: &BacktestCandidateRecord) -> bool {
    let config = candidate
        .config
        .get("portfolio_config")
        .unwrap_or(&candidate.config);
    value_has_long_and_short_direction(config)
        || value_has_dynamic_allocation_marker(config)
        || value_has_dynamic_allocation_marker(&candidate.summary)
}

fn value_has_dynamic_allocation_marker(value: &Value) -> bool {
    value
        .get("dynamic_allocation_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || value.get("dynamic_allocation_rules").is_some()
        || value
            .get("dynamic_allocation_summary")
            .is_some_and(|summary| !summary.is_null())
        || value
            .get("summary")
            .is_some_and(value_has_dynamic_allocation_marker)
        || value
            .get("strategies")
            .and_then(Value::as_array)
            .is_some_and(|strategies| strategies.iter().any(value_has_dynamic_allocation_marker))
}

fn value_has_long_and_short_direction(value: &Value) -> bool {
    value
        .get("direction_mode")
        .and_then(Value::as_str)
        .is_some_and(is_long_and_short)
        || value
            .get("strategies")
            .and_then(Value::as_array)
            .is_some_and(|strategies| strategies.iter().any(value_has_long_and_short_direction))
}

fn is_long_and_short(value: &str) -> bool {
    matches!(value, "long_short" | "long_and_short")
}

fn validate_live_ready_for_start(
    portfolio: &MartingalePortfolioRecord,
) -> Result<(), PublishError> {
    let blockers = portfolio_live_readiness_blockers(portfolio);
    if !blockers.is_empty() {
        return Err(PublishError::conflict(format!(
            "portfolio is not live-ready: {}",
            blockers.join("; ")
        )));
    }
    let live_ready = portfolio
        .config
        .get("live_ready")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    if !live_ready {
        return Err(PublishError::conflict("portfolio is not live-ready"));
    }
    Ok(())
}

fn portfolio_live_readiness_blockers(portfolio: &MartingalePortfolioRecord) -> Vec<String> {
    portfolio
        .config
        .get("live_readiness_blockers")
        .and_then(Value::as_array)
        .map(|blockers| {
            blockers
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
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
    json!({
        "strategy_count": request.items.len(),
        "enabled_strategy_count": request.items.iter().filter(|item| item.enabled).count(),
        "symbols": symbols,
        "max_leverage": max_leverage,
        "total_weight_pct": request.total_weight_pct,
    })
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
            && (portfolio.status == "running" || portfolio.status == "paused")
    }) {
        if existing.market != "usd_m_futures" && existing.market != "futures" {
            continue;
        }
        for item in &existing.items {
            if incoming.contains(&item.symbol.to_uppercase()) {
                return Err(PublishError::conflict(format!(
                    "{} leverage conflict",
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
            direction_mode: None,
            dynamic_allocation_enabled: false,
            total_weight_pct: Decimal::new(100, 0),
            dynamic_allocation_rules: None,
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
    fn dynamic_rules_requirement_ignores_risk_profile_text() {
        let candidates_by_id = BTreeMap::new();
        let mut request = publish_request("bt_task", "candidate-1", "candidate-2");
        request.risk_profile = "anything".to_owned();
        request.direction = "long".to_owned();
        request.direction_mode = None;
        request.dynamic_allocation_enabled = false;
        request.items[0].parameter_snapshot = json!({
            "direction_mode": "long_and_short",
            "dynamic_allocation_enabled": true
        });

        assert!(requires_dynamic_allocation_rules(&request, &candidates_by_id));

        request.risk_profile = "dynamic marketing copy only".to_owned();
        request.items[0].parameter_snapshot = json!({ "direction_mode": "long_only" });

        assert!(!requires_dynamic_allocation_rules(&request, &candidates_by_id));
    }
}
