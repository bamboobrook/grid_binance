use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use shared_db::{SharedDb, StoredStrategy, StoredStrategyTemplate};
use shared_domain::strategy::{
    GridGeneration, GridLevel, PostTriggerAction, PreflightFailure, PreflightReport,
    PreflightStepResult, PreflightStepStatus, Strategy, StrategyMarket, StrategyMode, StrategyRevision,
    StrategyRuntime, StrategyRuntimeEvent, StrategyRuntimeFill, StrategyRuntimeOrder,
    StrategyRuntimePosition, StrategyStatus, StrategyTemplate,
};

use crate::services::auth_service::AuthError;

#[derive(Clone)]
pub struct StrategyService {
    db: SharedDb,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SaveGridLevelRequest {
    pub entry_price: String,
    pub quantity: String,
    pub take_profit_bps: u32,
    pub trailing_bps: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SaveStrategyRequest {
    pub name: String,
    pub symbol: String,
    pub market: StrategyMarket,
    pub mode: StrategyMode,
    pub generation: GridGeneration,
    pub levels: Vec<SaveGridLevelRequest>,
    pub membership_ready: bool,
    pub exchange_ready: bool,
    pub symbol_ready: bool,
    pub overall_take_profit_bps: Option<u32>,
    pub overall_stop_loss_bps: Option<u32>,
    pub post_trigger_action: PostTriggerAction,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CreateTemplateRequest {
    #[serde(flatten)]
    pub strategy: SaveStrategyRequest,
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
pub struct StrategyRuntimeResponse {
    pub strategy_id: String,
    pub orders: Vec<StrategyRuntimeOrder>,
    pub fills: Vec<StrategyRuntimeFill>,
    pub positions: Vec<StrategyRuntimePosition>,
    pub events: Vec<StrategyRuntimeEvent>,
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
        Self::new(SharedDb::ephemeral().expect("ephemeral strategy db should initialize"))
    }
}

impl StrategyService {
    pub fn new(db: SharedDb) -> Self {
        Self { db }
    }

    pub fn list_strategies(&self, owner_email: &str) -> StrategyListResponse {
        StrategyListResponse {
            items: self
                .db
                .list_strategies(owner_email)
                .unwrap_or_else(|_| Vec::new()),
        }
    }

    pub fn get_strategy_runtime(
        &self,
        owner_email: &str,
        strategy_id: &str,
    ) -> Result<StrategyRuntimeResponse, StrategyError> {
        let strategy = self
            .db
            .find_strategy(owner_email, strategy_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("strategy not found"))?;

        Ok(StrategyRuntimeResponse {
            strategy_id: strategy.id,
            orders: strategy.runtime.orders,
            fills: strategy.runtime.fills,
            positions: strategy.runtime.positions,
            events: strategy.runtime.events,
        })
    }

    pub fn create_strategy(
        &self,
        owner_email: &str,
        request: SaveStrategyRequest,
    ) -> Result<Strategy, StrategyError> {
        validate_strategy_request(&request)?;

        let sequence_id = self
            .db
            .next_sequence("strategy")
            .map_err(StrategyError::storage)?;
        let strategy = build_strategy(
            sequence_id,
            owner_email,
            request,
            None,
            default_runtime(),
            None,
            None,
        )?;
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
        owner_email: &str,
        strategy_id: &str,
        request: SaveStrategyRequest,
    ) -> Result<Strategy, StrategyError> {
        validate_strategy_request(&request)?;

        let mut strategy = self
            .db
            .find_strategy(owner_email, strategy_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("strategy not found"))?;

        if matches!(
            strategy.status,
            StrategyStatus::Running | StrategyStatus::Archived | StrategyStatus::Completed
        ) {
            return Err(StrategyError::conflict(
                "strategy must be paused or stopped before editing",
            ));
        }

        strategy.name = request.name;
        strategy.symbol = request.symbol;
        strategy.market = request.market;
        strategy.mode = request.mode;
        strategy.budget = summarize_budget(&request.levels)?;
        strategy.grid_spacing_bps = summarize_spacing_bps(&request.levels)?;
        strategy.membership_ready = request.membership_ready;
        strategy.exchange_ready = request.exchange_ready;
        strategy.symbol_ready = request.symbol_ready;
        strategy.draft_revision = build_revision(
            &strategy.id,
            strategy.draft_revision.version + 1,
            request.generation,
            &request.levels,
            request.overall_take_profit_bps,
            request.overall_stop_loss_bps,
            request.post_trigger_action,
        )?;

        self.db
            .update_strategy(&strategy)
            .map_err(StrategyError::storage)?;
        Ok(strategy)
    }

    pub fn preflight_strategy(
        &self,
        owner_email: &str,
        strategy_id: &str,
    ) -> Result<PreflightReport, StrategyError> {
        let strategy = self
            .db
            .find_strategy(owner_email, strategy_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("strategy not found"))?;
        Ok(run_preflight(&strategy))
    }

    pub fn start_strategy(
        &self,
        owner_email: &str,
        strategy_id: &str,
    ) -> Result<StartStrategyResponse, StrategyError> {
        let mut strategy = self
            .db
            .find_strategy(owner_email, strategy_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("strategy not found"))?;

        if !matches!(strategy.status, StrategyStatus::Draft | StrategyStatus::Stopped) {
            return Err(StrategyError::conflict("strategy cannot be started from this state"));
        }

        let preflight = run_preflight(&strategy);
        if !preflight.ok {
            return Err(StrategyError::preflight_failed(preflight));
        }

        strategy.active_revision = Some(strategy.draft_revision.clone());
        strategy.runtime = rebuild_runtime(&strategy, None, "strategy_started");
        strategy.runtime.last_preflight = Some(preflight.clone());
        strategy.status = StrategyStatus::Running;

        self.db
            .update_strategy(&strategy)
            .map_err(StrategyError::storage)?;

        Ok(StartStrategyResponse { strategy, preflight })
    }

    pub fn resume_strategy(
        &self,
        owner_email: &str,
        strategy_id: &str,
    ) -> Result<StartStrategyResponse, StrategyError> {
        let mut strategy = self
            .db
            .find_strategy(owner_email, strategy_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("strategy not found"))?;

        if strategy.status != StrategyStatus::Paused {
            return Err(StrategyError::conflict("only paused strategies can resume"));
        }

        let preflight = run_preflight(&strategy);
        if !preflight.ok {
            return Err(StrategyError::preflight_failed(preflight));
        }

        let positions = strategy.runtime.positions.clone();
        strategy.active_revision = Some(strategy.draft_revision.clone());
        strategy.runtime = rebuild_runtime(&strategy, Some(positions), "strategy_resumed");
        strategy.runtime.last_preflight = Some(preflight.clone());
        strategy.status = StrategyStatus::Running;

        self.db
            .update_strategy(&strategy)
            .map_err(StrategyError::storage)?;

        Ok(StartStrategyResponse { strategy, preflight })
    }

    pub fn stop_strategy(
        &self,
        owner_email: &str,
        strategy_id: &str,
    ) -> Result<Strategy, StrategyError> {
        let mut strategy = self
            .db
            .find_strategy(owner_email, strategy_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("strategy not found"))?;

        stop_strategy_runtime(&mut strategy)?;
        self.db
            .update_strategy(&strategy)
            .map_err(StrategyError::storage)?;
        Ok(strategy)
    }

    pub fn pause_strategies(
        &self,
        owner_email: &str,
        request: BatchStrategyRequest,
    ) -> Result<BatchPauseResponse, StrategyError> {
        if request.ids.is_empty() {
            return Err(StrategyError::bad_request("ids are required"));
        }

        let mut paused = 0;
        for mut strategy in self
            .db
            .list_strategies(owner_email)
            .map_err(StrategyError::storage)?
        {
            if request.ids.iter().any(|id| id == &strategy.id)
                && strategy.status == StrategyStatus::Running
            {
                strategy.status = StrategyStatus::Paused;
                cancel_working_orders(&mut strategy.runtime.orders);
                push_runtime_event(&mut strategy.runtime, "strategy_paused", "strategy paused", None);
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
        owner_email: &str,
        request: BatchStrategyRequest,
    ) -> Result<BatchDeleteResponse, StrategyError> {
        if request.ids.is_empty() {
            return Err(StrategyError::bad_request("ids are required"));
        }

        let mut deleted = 0;
        for mut strategy in self
            .db
            .list_strategies(owner_email)
            .map_err(StrategyError::storage)?
        {
            if request.ids.iter().any(|id| id == &strategy.id)
                && can_soft_archive(&strategy)
                && strategy.status != StrategyStatus::Archived
            {
                strategy.status = StrategyStatus::Archived;
                strategy.archived_at = Some(Utc::now());
                push_runtime_event(&mut strategy.runtime, "strategy_archived", "strategy archived", None);
                self.db
                    .update_strategy(&strategy)
                    .map_err(StrategyError::storage)?;
                deleted += 1;
            }
        }

        Ok(BatchDeleteResponse { deleted })
    }

    pub fn stop_all(&self, owner_email: &str) -> StopAllResponse {
        let mut stopped = 0;

        if let Ok(strategies) = self.db.list_strategies(owner_email) {
            for mut strategy in strategies {
                if matches!(strategy.status, StrategyStatus::Running | StrategyStatus::Paused)
                    && stop_strategy_runtime(&mut strategy).is_ok()
                    && self.db.update_strategy(&strategy).is_ok()
                {
                    stopped += 1;
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
        validate_strategy_request(&request.strategy)?;

        let sequence_id = self
            .db
            .next_sequence("strategy_template")
            .map_err(StrategyError::storage)?;
        let template = StrategyTemplate {
            id: format!("template-{sequence_id}"),
            name: request.strategy.name,
            symbol: request.strategy.symbol,
            budget: summarize_budget(&request.strategy.levels)?,
            grid_spacing_bps: summarize_spacing_bps(&request.strategy.levels)?,
            membership_ready: request.strategy.membership_ready,
            exchange_ready: request.strategy.exchange_ready,
            symbol_ready: request.strategy.symbol_ready,
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
        owner_email: &str,
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

        let request = SaveStrategyRequest {
            name: request.name,
            symbol: template.symbol.clone(),
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            generation: GridGeneration::Custom,
            levels: vec![SaveGridLevelRequest {
                entry_price: "100".to_string(),
                quantity: "1".to_string(),
                take_profit_bps: 100,
                trailing_bps: None,
            }],
            membership_ready: template.membership_ready,
            exchange_ready: template.exchange_ready,
            symbol_ready: template.symbol_ready,
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        };

        let strategy = build_strategy(
            sequence_id,
            owner_email,
            request,
            Some(template.id),
            default_runtime(),
            None,
            None,
        )?;
        self.db
            .insert_strategy(&StoredStrategy {
                sequence_id,
                strategy: strategy.clone(),
            })
            .map_err(StrategyError::storage)?;
        Ok(strategy)
    }
}

fn validate_strategy_request(request: &SaveStrategyRequest) -> Result<(), StrategyError> {
    if request.name.trim().is_empty() {
        return Err(StrategyError::bad_request("name is required"));
    }
    if request.symbol.trim().is_empty() {
        return Err(StrategyError::bad_request("symbol is required"));
    }
    if request.levels.is_empty() {
        return Err(StrategyError::bad_request("levels are required"));
    }

    let mut parsed = Vec::with_capacity(request.levels.len());
    for (index, level) in request.levels.iter().enumerate() {
        let entry_price = parse_decimal(&level.entry_price, "entry_price")?;
        let quantity = parse_decimal(&level.quantity, "quantity")?;
        if entry_price <= Decimal::ZERO {
            return Err(StrategyError::bad_request(&format!(
                "level {index} entry_price must be positive"
            )));
        }
        if quantity <= Decimal::ZERO {
            return Err(StrategyError::bad_request(&format!(
                "level {index} quantity must be positive"
            )));
        }
        if let Some(trailing_bps) = level.trailing_bps {
            if trailing_bps > level.take_profit_bps {
                return Err(StrategyError::bad_request(&format!(
                    "level {index} trailing_bps must be less than or equal to take_profit_bps"
                )));
            }
        }
        parsed.push(entry_price);
    }

    if parsed.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(StrategyError::bad_request(
            "levels must be strictly increasing by entry_price",
        ));
    }

    Ok(())
}

fn build_strategy(
    sequence_id: u64,
    owner_email: &str,
    request: SaveStrategyRequest,
    source_template_id: Option<String>,
    runtime: StrategyRuntime,
    active_revision: Option<StrategyRevision>,
    archived_at: Option<chrono::DateTime<Utc>>,
) -> Result<Strategy, StrategyError> {
    let draft_revision = build_revision(
        &format!("strategy-{sequence_id}"),
        1,
        request.generation,
        &request.levels,
        request.overall_take_profit_bps,
        request.overall_stop_loss_bps,
        request.post_trigger_action,
    )?;

    Ok(Strategy {
        id: format!("strategy-{sequence_id}"),
        owner_email: owner_email.to_string(),
        name: request.name,
        symbol: request.symbol,
        budget: summarize_budget(&request.levels)?,
        grid_spacing_bps: summarize_spacing_bps(&request.levels)?,
        status: StrategyStatus::Draft,
        source_template_id,
        membership_ready: request.membership_ready,
        exchange_ready: request.exchange_ready,
        symbol_ready: request.symbol_ready,
        market: request.market,
        mode: request.mode,
        draft_revision,
        active_revision,
        runtime,
        archived_at,
    })
}

fn build_revision(
    strategy_id: &str,
    version: u32,
    generation: GridGeneration,
    levels: &[SaveGridLevelRequest],
    overall_take_profit_bps: Option<u32>,
    overall_stop_loss_bps: Option<u32>,
    post_trigger_action: PostTriggerAction,
) -> Result<StrategyRevision, StrategyError> {
    let levels = levels
        .iter()
        .enumerate()
        .map(|(index, level)| {
            Ok(GridLevel {
                level_index: index as u32,
                entry_price: parse_decimal(&level.entry_price, "entry_price")?,
                quantity: parse_decimal(&level.quantity, "quantity")?,
                take_profit_bps: level.take_profit_bps,
                trailing_bps: level.trailing_bps,
            })
        })
        .collect::<Result<Vec<_>, StrategyError>>()?;

    Ok(StrategyRevision {
        revision_id: format!("{strategy_id}-revision-{version}"),
        version,
        generation,
        levels,
        overall_take_profit_bps,
        overall_stop_loss_bps,
        post_trigger_action,
    })
}

fn parse_decimal(value: &str, field: &str) -> Result<Decimal, StrategyError> {
    value.parse::<Decimal>().map_err(|_| {
        StrategyError::bad_request(&format!("{field} must be a valid decimal string"))
    })
}

fn summarize_budget(levels: &[SaveGridLevelRequest]) -> Result<String, StrategyError> {
    let total = levels.iter().try_fold(Decimal::ZERO, |acc, level| {
        Ok::<_, StrategyError>(acc + parse_decimal(&level.quantity, "quantity")?)
    })?;
    Ok(total.normalize().to_string())
}

fn summarize_spacing_bps(levels: &[SaveGridLevelRequest]) -> Result<u32, StrategyError> {
    if levels.len() < 2 {
        return Ok(1);
    }

    let first = parse_decimal(&levels[0].entry_price, "entry_price")?;
    let second = parse_decimal(&levels[1].entry_price, "entry_price")?;
    if first <= Decimal::ZERO || second <= first {
        return Ok(1);
    }

    let spacing = ((second - first) / first) * Decimal::from(10_000u32);
    let spacing = spacing.round().to_string().parse::<u32>().unwrap_or(1);
    Ok(spacing.max(1))
}

fn run_preflight(strategy: &Strategy) -> PreflightReport {
    let mut steps = Vec::new();
    let mut failures = Vec::new();

    let checks = [
        (
            "membership_status",
            strategy.membership_ready,
            "membership is not active",
            "renew or reactivate membership before starting",
        ),
        (
            "exchange_connection",
            strategy.exchange_ready,
            "exchange credentials are not ready",
            "verify API key, secret, and required Binance permissions",
        ),
        (
            "symbol_support",
            strategy.symbol_ready,
            "symbol is not available",
            "choose a tradable symbol from synced Binance metadata",
        ),
        (
            "grid_configuration",
            !strategy.draft_revision.levels.is_empty(),
            "grid levels are missing",
            "save at least one grid level before starting",
        ),
        (
            "trailing_take_profit",
            strategy
                .draft_revision
                .levels
                .iter()
                .all(|level| level.trailing_bps.unwrap_or(level.take_profit_bps) <= level.take_profit_bps),
            "trailing take profit exceeds the configured grid take profit range",
            "reduce trailing_bps so it does not exceed take_profit_bps",
        ),
    ];

    let mut blocked = false;
    for (step, ok, reason, guidance) in checks {
        if blocked {
            steps.push(PreflightStepResult {
                step: step.to_string(),
                status: PreflightStepStatus::Skipped,
                reason: None,
                guidance: None,
            });
            continue;
        }

        if ok {
            steps.push(PreflightStepResult {
                step: step.to_string(),
                status: PreflightStepStatus::Passed,
                reason: None,
                guidance: None,
            });
        } else {
            let failure = PreflightFailure {
                step: step.to_string(),
                reason: reason.to_string(),
                guidance: Some(guidance.to_string()),
            };
            steps.push(PreflightStepResult {
                step: step.to_string(),
                status: PreflightStepStatus::Failed,
                reason: Some(reason.to_string()),
                guidance: Some(guidance.to_string()),
            });
            failures.push(failure);
            blocked = true;
        }
    }

    PreflightReport {
        ok: failures.is_empty(),
        steps,
        failures,
    }
}

fn default_runtime() -> StrategyRuntime {
    StrategyRuntime::default()
}

fn rebuild_runtime(
    strategy: &Strategy,
    positions_override: Option<Vec<StrategyRuntimePosition>>,
    event_type: &str,
) -> StrategyRuntime {
    let active = strategy
        .active_revision
        .clone()
        .unwrap_or_else(|| strategy.draft_revision.clone());
    let positions = positions_override.unwrap_or_else(|| seed_positions(strategy.market, strategy.mode, &active));

    let mut runtime = StrategyRuntime {
        positions,
        orders: active
            .levels
            .iter()
            .map(|level| StrategyRuntimeOrder {
                order_id: format!("{}-order-{}", strategy.id, level.level_index),
                level_index: Some(level.level_index),
                side: side_for_mode(strategy.mode).to_string(),
                order_type: "Limit".to_string(),
                price: Some(level.entry_price),
                quantity: level.quantity,
                status: "Working".to_string(),
            })
            .collect(),
        fills: Vec::new(),
        events: Vec::new(),
        last_preflight: None,
    };
    push_runtime_event(&mut runtime, event_type, event_type.replace('_', " ").as_str(), None);
    runtime
}

fn seed_positions(
    market: StrategyMarket,
    mode: StrategyMode,
    revision: &StrategyRevision,
) -> Vec<StrategyRuntimePosition> {
    revision
        .levels
        .first()
        .map(|level| StrategyRuntimePosition {
            market,
            mode,
            quantity: level.quantity,
            average_entry_price: level.entry_price,
        })
        .into_iter()
        .collect()
}

fn side_for_mode(mode: StrategyMode) -> &'static str {
    match mode {
        StrategyMode::SpotClassic | StrategyMode::SpotBuyOnly | StrategyMode::FuturesLong => "Buy",
        StrategyMode::SpotSellOnly | StrategyMode::FuturesShort | StrategyMode::FuturesNeutral => "Sell",
    }
}

fn cancel_working_orders(orders: &mut [StrategyRuntimeOrder]) {
    for order in orders.iter_mut() {
        if order.status == "Working" {
            order.status = "Canceled".to_string();
        }
    }
}

fn can_soft_archive(strategy: &Strategy) -> bool {
    !strategy.runtime.orders.iter().any(|order| order.status == "Working")
        && strategy.runtime.positions.is_empty()
        && !matches!(strategy.status, StrategyStatus::Running)
}

fn stop_strategy_runtime(strategy: &mut Strategy) -> Result<(), StrategyError> {
    if !matches!(strategy.status, StrategyStatus::Running | StrategyStatus::Paused) {
        return Err(StrategyError::conflict("only running or paused strategies can stop"));
    }

    cancel_working_orders(&mut strategy.runtime.orders);
    let mut closing_fills = strategy
        .runtime
        .positions
        .iter()
        .enumerate()
        .map(|(index, position)| StrategyRuntimeFill {
            fill_id: format!("{}-close-fill-{index}", strategy.id),
            order_id: None,
            level_index: None,
            fill_type: "MarketClose".to_string(),
            price: position.average_entry_price,
            quantity: position.quantity,
            realized_pnl: Some(Decimal::ZERO),
            fee_amount: None,
            fee_asset: None,
        })
        .collect::<Vec<_>>();
    strategy.runtime.fills.append(&mut closing_fills);
    strategy.runtime.positions.clear();
    strategy.status = StrategyStatus::Stopped;
    push_runtime_event(&mut strategy.runtime, "strategy_stopped", "strategy stopped", None);
    Ok(())
}

fn push_runtime_event(
    runtime: &mut StrategyRuntime,
    event_type: &str,
    detail: &str,
    price: Option<Decimal>,
) {
    runtime.events.push(StrategyRuntimeEvent {
        event_type: event_type.to_string(),
        detail: detail.to_string(),
        price,
        created_at: Utc::now(),
    });
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
    use super::{run_preflight, SaveGridLevelRequest, SaveStrategyRequest, StrategyService};
    use shared_db::SharedDb;
    use shared_domain::strategy::{GridGeneration, PostTriggerAction, StrategyMarket, StrategyMode};

    #[test]
    fn preflight_reports_fail_fast_steps() {
        let service = StrategyService::new(SharedDb::ephemeral().expect("db"));
        let strategy = service
            .create_strategy(
                "trader@example.com",
                SaveStrategyRequest {
                    name: "draft".to_string(),
                    symbol: "BTCUSDT".to_string(),
                    market: StrategyMarket::Spot,
                    mode: StrategyMode::SpotClassic,
                    generation: GridGeneration::Custom,
                    levels: vec![SaveGridLevelRequest {
                        entry_price: "100".to_string(),
                        quantity: "1".to_string(),
                        take_profit_bps: 100,
                        trailing_bps: None,
                    }],
                    membership_ready: true,
                    exchange_ready: false,
                    symbol_ready: true,
                    overall_take_profit_bps: None,
                    overall_stop_loss_bps: None,
                    post_trigger_action: PostTriggerAction::Stop,
                },
            )
            .expect("strategy");

        let report = run_preflight(&strategy);

        assert!(!report.ok);
        assert_eq!(report.steps[0].step, "membership_status");
        assert_eq!(report.steps[1].step, "exchange_connection");
        assert_eq!(report.failures[0].step, "exchange_connection");
    }
}
