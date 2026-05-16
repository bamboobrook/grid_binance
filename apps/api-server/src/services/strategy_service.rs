use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared_db::{
    MembershipRecord, NotificationLogRecord, SharedDb, StoredStrategy, StoredStrategyTemplate,
    UserExchangeAccountRecord,
};
use shared_events::{NotificationEvent, NotificationKind, NotificationRecord};
use std::{collections::BTreeMap, env, sync::OnceLock, time::Duration as StdDuration};
use trading_engine::strategy_runtime::StrategyRuntimeEngine;

use shared_domain::{
    membership::MembershipStatus,
    strategy::{
        set_strategy_template_reference_price, strategy_template_reference_price,
        FuturesMarginMode, GridGeneration, GridLevel, PostTriggerAction, PreflightFailure,
        PreflightReport, PreflightStepResult, PreflightStepStatus, ReferencePriceSource,
        RuntimeControls, Strategy, StrategyAmountMode, StrategyMarket, StrategyMode,
        StrategyRevision, StrategyRuntime, StrategyRuntimeEvent, StrategyRuntimeFill,
        StrategyRuntimeOrder, StrategyRuntimePhase, StrategyRuntimePosition, StrategyStatus,
        StrategyTemplate, StrategyType,
    },
};

use crate::services::auth_service::{AdminRole, AuthError};

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

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub struct SaveStrategyRequest {
    pub name: String,
    pub symbol: String,
    pub market: StrategyMarket,
    pub mode: StrategyMode,
    #[serde(default)]
    pub strategy_type: Option<StrategyType>,
    pub generation: GridGeneration,
    #[serde(default)]
    pub amount_mode: StrategyAmountMode,
    #[serde(default)]
    pub futures_margin_mode: Option<FuturesMarginMode>,
    #[serde(default)]
    pub leverage: Option<u32>,
    #[serde(default)]
    pub levels: Vec<SaveGridLevelRequest>,
    #[serde(default)]
    pub membership_ready: bool,
    #[serde(default)]
    pub exchange_ready: bool,
    #[serde(default)]
    pub permissions_ready: bool,
    #[serde(default)]
    pub withdrawals_disabled: bool,
    #[serde(default)]
    pub hedge_mode_ready: bool,
    #[serde(default)]
    pub symbol_ready: bool,
    #[serde(default)]
    pub filters_ready: bool,
    #[serde(default)]
    pub margin_ready: bool,
    #[serde(default)]
    pub conflict_ready: bool,
    #[serde(default)]
    pub balance_ready: bool,
    pub overall_take_profit_bps: Option<u32>,
    pub overall_stop_loss_bps: Option<u32>,
    #[serde(default)]
    pub reference_price_source: ReferencePriceSource,
    pub post_trigger_action: PostTriggerAction,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

impl SaveStrategyRequest {
    pub fn resolved_strategy_type(&self) -> StrategyType {
        self.strategy_type.unwrap_or(StrategyType::OrdinaryGrid)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BilateralSpacingMode {
    FixedStep,
    Geometric,
}

#[derive(Debug, Clone, Copy)]
struct BilateralRequestHints {
    strategy_type: StrategyType,
    levels_per_side: Option<u32>,
    spacing_mode: Option<BilateralSpacingMode>,
    grid_spacing_bps: Option<u32>,
    reference_price: Option<Decimal>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CreateTemplateRequest {
    #[serde(flatten)]
    pub strategy: SaveStrategyRequest,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateTemplateRequest {
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
pub struct BatchStrategyRuntimeResponse {
    pub items: Vec<StrategyRuntimeResponse>,
}

#[derive(Debug, Serialize)]
pub struct BatchStartFailure {
    pub strategy_id: String,
    pub error: String,
    pub preflight: Option<PreflightReport>,
}

#[derive(Debug, Serialize)]
pub struct BatchStartResponse {
    pub started: usize,
    pub items: Vec<StartStrategyResponse>,
    pub failures: Vec<BatchStartFailure>,
}

#[derive(Debug, Serialize)]
pub struct BatchPauseFailure {
    pub strategy_id: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct BatchPauseResponse {
    pub paused: usize,
    pub failures: Vec<BatchPauseFailure>,
}

#[derive(Debug, Serialize)]
pub struct BatchDeleteFailure {
    pub strategy_id: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct BatchDeleteResponse {
    pub deleted: usize,
    pub failures: Vec<BatchDeleteFailure>,
}

#[derive(Debug, Serialize)]
pub struct StopAllResponse {
    pub stopped: usize,
}

fn persist_strategy_notification(
    db: &SharedDb,
    email: &str,
    kind: NotificationKind,
    title: &str,
    message: &str,
    payload: BTreeMap<String, String>,
) -> Result<(), StrategyError> {
    let binding = db
        .find_telegram_binding(email)
        .map_err(StrategyError::storage)?;
    let telegram_delivered = binding.is_some();
    let record = NotificationRecord {
        event: NotificationEvent {
            email: email.to_owned(),
            kind: kind.clone(),
            title: title.to_owned(),
            message: message.to_owned(),
            payload,
        },
        telegram_delivered,
        in_app_delivered: true,
        show_expiry_popup: matches!(kind, NotificationKind::MembershipExpiring),
    };
    let now = Utc::now();
    let payload = serde_json::to_value(&record).map_err(|_| {
        StrategyError::storage(shared_db::SharedDbError::new(
            "notification serialization failed",
        ))
    })?;
    db.insert_notification_log(&NotificationLogRecord {
        user_email: email.to_owned(),
        channel: "in_app".to_owned(),
        template_key: Some(format!("{:?}", kind)),
        title: title.to_owned(),
        body: message.to_owned(),
        status: "delivered".to_owned(),
        payload: payload.clone(),
        created_at: now,
        delivered_at: Some(now),
    })
    .map_err(StrategyError::storage)?;
    if let Some(binding) = binding {
        if let Some(token) = telegram_bot_token() {
            let delivered =
                send_telegram_message(&token, &binding.telegram_chat_id, title, message).is_ok();
            db.insert_notification_log(&NotificationLogRecord {
                user_email: email.to_owned(),
                channel: "telegram".to_owned(),
                template_key: Some(format!("{:?}", kind)),
                title: title.to_owned(),
                body: message.to_owned(),
                status: if delivered { "delivered" } else { "failed" }.to_owned(),
                payload,
                created_at: now,
                delivered_at: delivered.then_some(now),
            })
            .map_err(StrategyError::storage)?;
        }
    }
    Ok(())
}

fn telegram_bot_token() -> Option<String> {
    env::var("TELEGRAM_BOT_TOKEN")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn telegram_api_base_url() -> String {
    env::var("TELEGRAM_API_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "https://api.telegram.org".to_string())
}

fn telegram_http_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout(StdDuration::from_secs(5))
            .build()
    })
}

fn send_telegram_message(
    bot_token: &str,
    chat_id: &str,
    title: &str,
    body: &str,
) -> Result<(), shared_db::SharedDbError> {
    telegram_http_agent()
        .post(&format!(
            "{}/bot{}/sendMessage",
            telegram_api_base_url(),
            bot_token
        ))
        .send_json(ureq::json!({
            "chat_id": chat_id,
            "text": format!("{}\n{}", title, body),
        }))
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    Ok(())
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
                .unwrap_or_else(|_| Vec::new())
                .into_iter()
                .filter(|strategy| strategy.status != StrategyStatus::Archived)
                .collect(),
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

    pub fn batch_get_strategy_runtimes(
        &self,
        owner_email: &str,
        strategy_ids: &[String],
    ) -> Result<BatchStrategyRuntimeResponse, StrategyError> {
        let items = strategy_ids
            .iter()
            .filter_map(|id| self.get_strategy_runtime(owner_email, id).ok())
            .collect();
        Ok(BatchStrategyRuntimeResponse { items })
    }

    pub fn list_strategies_paginated(
        &self,
        owner_email: &str,
        page: u32,
        per_page: u32,
    ) -> StrategyListResponse {
        let all = self.list_strategies(owner_email);
        let start = ((page - 1) * per_page) as usize;
        let per_page = per_page as usize;
        let items: Vec<Strategy> = all.items.into_iter().skip(start).take(per_page).collect();
        StrategyListResponse { items }
    }

    pub fn create_strategy(
        &self,
        owner_email: &str,
        request: SaveStrategyRequest,
    ) -> Result<Strategy, StrategyError> {
        let has_explicit_reference = parse_decimal_extra(&request, "reference_price")?.is_some();
        let request = normalize_strategy_request(request)?;
        validate_strategy_request(&request)?;

        let sequence_id = self
            .db
            .next_sequence("strategy")
            .map_err(StrategyError::storage)?;
        let mut strategy = build_strategy(
            sequence_id,
            owner_email,
            request,
            None,
            default_runtime(),
            None,
            None,
        )?;
        self.normalize_strategy_levels(&mut strategy)?;
        if !has_explicit_reference {
            if let Some(derived) = derived_reference_price_from_grid_levels(
                strategy.strategy_type,
                strategy.mode,
                &strategy.draft_revision.levels,
            ) {
                strategy.draft_revision.reference_price = Some(derived);
            }
        }
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
        let has_explicit_reference = parse_decimal_extra(&request, "reference_price")?.is_some();
        let request = normalize_strategy_request(request)?;
        validate_strategy_request(&request)?;

        let mut strategy = self
            .db
            .find_strategy(owner_email, strategy_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("strategy not found"))?;

        if matches!(
            strategy.status,
            StrategyStatus::Running
                | StrategyStatus::Archived
                | StrategyStatus::Completed
                | StrategyStatus::Stopping
        ) {
            return Err(StrategyError::conflict(
                "strategy must be paused or stopped before editing",
            ));
        }

        let reference_price = reference_price_for_request(&request)?;
        let resolved_strategy_type = request.resolved_strategy_type();
        strategy.name = request.name;
        strategy.symbol = request.symbol;
        strategy.market = request.market;
        strategy.mode = request.mode;
        strategy.strategy_type = resolved_strategy_type;
        strategy.budget = summarize_budget(&request.levels)?;
        strategy.grid_spacing_bps = summarize_spacing_bps(&request.levels)?;
        strategy.membership_ready = false;
        strategy.exchange_ready = false;
        strategy.permissions_ready = false;
        strategy.withdrawals_disabled = false;
        strategy.hedge_mode_ready = false;
        strategy.symbol_ready = false;
        strategy.filters_ready = false;
        strategy.margin_ready = false;
        strategy.conflict_ready = matches!(strategy.market, StrategyMarket::Spot);
        strategy.balance_ready = false;
        strategy.draft_revision = build_revision(
            &strategy.id,
            strategy.draft_revision.version + 1,
            resolved_strategy_type,
            request.generation,
            request.amount_mode,
            request.futures_margin_mode,
            request.leverage,
            &request.levels,
            request.overall_take_profit_bps,
            request.overall_stop_loss_bps,
            request.reference_price_source,
            reference_price,
            request.post_trigger_action,
        )?;
        self.normalize_strategy_levels(&mut strategy)?;
        if !has_explicit_reference {
            if let Some(derived) = derived_reference_price_from_grid_levels(
                strategy.strategy_type,
                strategy.mode,
                &strategy.draft_revision.levels,
            ) {
                strategy.draft_revision.reference_price = Some(derived);
            }
        }

        self.db
            .update_strategy(&strategy)
            .map_err(StrategyError::storage)?;
        Ok(strategy)
    }

    pub fn clone_strategy(
        &self,
        owner_email: &str,
        strategy_id: &str,
    ) -> Result<Strategy, StrategyError> {
        let source = self
            .db
            .find_strategy(owner_email, strategy_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("strategy not found"))?;
        let rev = &source.draft_revision;
        let request = SaveStrategyRequest {
            name: format!("{} (Copy)", source.name),
            symbol: source.symbol.clone(),
            market: source.market,
            mode: source.mode,
            strategy_type: Some(source.strategy_type),
            generation: rev.generation,
            levels: rev
                .levels
                .iter()
                .map(|l| SaveGridLevelRequest {
                    entry_price: l.entry_price.to_string(),
                    quantity: l.quantity.to_string(),
                    take_profit_bps: l.take_profit_bps,
                    trailing_bps: l.trailing_bps,
                })
                .collect(),
            amount_mode: rev.amount_mode,
            futures_margin_mode: rev.futures_margin_mode,
            leverage: rev.leverage,
            membership_ready: source.membership_ready,
            exchange_ready: source.exchange_ready,
            permissions_ready: source.permissions_ready,
            withdrawals_disabled: source.withdrawals_disabled,
            hedge_mode_ready: source.hedge_mode_ready,
            symbol_ready: source.symbol_ready,
            filters_ready: source.filters_ready,
            margin_ready: source.margin_ready,
            conflict_ready: source.conflict_ready,
            balance_ready: source.balance_ready,
            overall_take_profit_bps: rev.overall_take_profit_bps,
            overall_stop_loss_bps: rev.overall_stop_loss_bps,
            reference_price_source: rev.reference_price_source,
            post_trigger_action: rev.post_trigger_action,
            tags: source.tags.clone(),
            notes: source.notes.clone(),
            extra: BTreeMap::new(),
        };
        self.create_strategy(owner_email, request)
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
        self.live_preflight_report(&strategy)
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

        if !matches!(
            strategy.status,
            StrategyStatus::Draft | StrategyStatus::Stopped
        ) {
            return Err(StrategyError::conflict(
                "strategy cannot be started from this state",
            ));
        }

        let preflight = self.live_preflight_report(&strategy)?;
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
        let mut payload = BTreeMap::new();
        payload.insert("strategy_id".to_string(), strategy.id.clone());
        payload.insert("symbol".to_string(), strategy.symbol.clone());
        persist_strategy_notification(
            &self.db,
            &strategy.owner_email,
            NotificationKind::StrategyStarted,
            "Strategy started",
            &format!("{} started on {}.", strategy.name, strategy.symbol),
            payload,
        )?;

        Ok(StartStrategyResponse {
            strategy,
            preflight,
        })
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

        let preflight = self.live_preflight_report(&strategy)?;
        if !preflight.ok {
            return Err(StrategyError::preflight_failed(preflight));
        }

        let positions = strategy.runtime.positions.clone();
        let draft_differs_from_active = strategy
            .active_revision
            .as_ref()
            .map(|active| active.revision_id != strategy.draft_revision.revision_id)
            .unwrap_or(false);
        if !positions.is_empty() && draft_differs_from_active {
            return Err(StrategyError::conflict(
                "resume requires reconciling paused positions before applying a revised grid",
            ));
        }
        strategy.active_revision = Some(strategy.draft_revision.clone());
        strategy.runtime = resume_runtime(&strategy)?;
        strategy.runtime.last_preflight = Some(preflight.clone());
        strategy.status = StrategyStatus::Running;

        self.db
            .update_strategy(&strategy)
            .map_err(StrategyError::storage)?;

        Ok(StartStrategyResponse {
            strategy,
            preflight,
        })
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

    pub fn start_strategies(
        &self,
        owner_email: &str,
        request: BatchStrategyRequest,
    ) -> Result<BatchStartResponse, StrategyError> {
        if request.ids.is_empty() {
            return Err(StrategyError::bad_request("ids are required"));
        }

        let mut items = Vec::new();
        let mut failures = Vec::new();

        for strategy_id in request.ids {
            let strategy = self
                .db
                .find_strategy(owner_email, &strategy_id)
                .map_err(StrategyError::storage)?;
            let Some(strategy) = strategy else {
                failures.push(BatchStartFailure {
                    strategy_id,
                    error: "strategy not found".to_string(),
                    preflight: None,
                });
                continue;
            };

            let outcome = match strategy.status {
                StrategyStatus::Draft | StrategyStatus::Stopped => {
                    self.start_strategy(owner_email, &strategy.id)
                }
                StrategyStatus::Paused => self.resume_strategy(owner_email, &strategy.id),
                _ => Err(StrategyError::conflict(
                    "strategy cannot be started from this state",
                )),
            };

            match outcome {
                Ok(started) => items.push(started),
                Err(error) => failures.push(error.into_batch_start_failure(strategy.id.clone())),
            }
        }

        Ok(BatchStartResponse {
            started: items.len(),
            items,
            failures,
        })
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
        let mut failures = Vec::new();

        for strategy_id in request.ids {
            let Some(mut strategy) = self
                .db
                .find_strategy(owner_email, &strategy_id)
                .map_err(StrategyError::storage)?
            else {
                failures.push(BatchPauseFailure {
                    strategy_id,
                    error: "strategy not found".to_string(),
                });
                continue;
            };

            if strategy.status != StrategyStatus::Running {
                failures.push(BatchPauseFailure {
                    strategy_id: strategy.id.clone(),
                    error: pause_failure_reason(strategy.status).to_string(),
                });
                continue;
            }

            strategy.status = StrategyStatus::Paused;
            cancel_working_orders(&mut strategy.runtime.orders);
            push_runtime_event(
                &mut strategy.runtime,
                "strategy_paused",
                "strategy paused",
                None,
            );
            self.db
                .update_strategy(&strategy)
                .map_err(StrategyError::storage)?;
            let mut payload = BTreeMap::new();
            payload.insert("strategy_id".to_string(), strategy.id.clone());
            payload.insert("symbol".to_string(), strategy.symbol.clone());
            persist_strategy_notification(
                &self.db,
                &strategy.owner_email,
                NotificationKind::StrategyPaused,
                "Strategy paused",
                &format!("{} paused on {}.", strategy.name, strategy.symbol),
                payload,
            )?;
            paused += 1;
        }

        Ok(BatchPauseResponse { paused, failures })
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
        let mut failures = Vec::new();

        for strategy_id in request.ids {
            let Some(mut strategy) = self
                .db
                .find_strategy(owner_email, &strategy_id)
                .map_err(StrategyError::storage)?
            else {
                failures.push(BatchDeleteFailure {
                    strategy_id,
                    error: "strategy not found".to_string(),
                });
                continue;
            };

            if let Some(error) = delete_failure_reason(&strategy) {
                failures.push(BatchDeleteFailure {
                    strategy_id: strategy.id.clone(),
                    error,
                });
                continue;
            }

            strategy.status = StrategyStatus::Archived;
            strategy.archived_at = Some(Utc::now());
            push_runtime_event(
                &mut strategy.runtime,
                "strategy_archived",
                "strategy archived",
                None,
            );
            self.db
                .update_strategy(&strategy)
                .map_err(StrategyError::storage)?;
            deleted += 1;
        }

        Ok(BatchDeleteResponse { deleted, failures })
    }

    pub fn stop_all(&self, owner_email: &str) -> StopAllResponse {
        let mut stopped = 0;

        if let Ok(strategies) = self.db.list_strategies(owner_email) {
            for mut strategy in strategies {
                if matches!(
                    strategy.status,
                    StrategyStatus::Running | StrategyStatus::Paused
                ) && stop_strategy_runtime(&mut strategy).is_ok()
                    && self.db.update_strategy(&strategy).is_ok()
                {
                    stopped += 1;
                }
            }
        }

        StopAllResponse { stopped }
    }

    pub fn list_templates(&self) -> Result<TemplateListResponse, StrategyError> {
        Ok(TemplateListResponse {
            items: self.db.list_templates().map_err(StrategyError::storage)?,
        })
    }

    pub fn create_template(
        &self,
        actor_email: &str,
        admin_role: Option<AdminRole>,
        session_sid: u64,
        request: CreateTemplateRequest,
    ) -> Result<StrategyTemplate, StrategyError> {
        let has_explicit_reference =
            parse_decimal_extra(&request.strategy, "reference_price")?.is_some();
        let strategy = normalize_strategy_request(request.strategy)?;
        validate_strategy_request(&strategy)?;

        let sequence_id = self
            .db
            .next_sequence("strategy_template")
            .map_err(StrategyError::storage)?;
        let template_id = format!("template-{sequence_id}");
        let mut template = build_template(&template_id, strategy.clone())?;

        let mut temp_strategy = Strategy {
            id: template_id.clone(),
            owner_email: actor_email.to_string(),
            name: template.name.clone(),
            symbol: template.symbol.clone(),
            budget: template.budget.clone(),
            grid_spacing_bps: template.grid_spacing_bps,
            status: StrategyStatus::Draft,
            source_template_id: None,
            membership_ready: false,
            exchange_ready: false,
            permissions_ready: false,
            withdrawals_disabled: false,
            hedge_mode_ready: false,
            symbol_ready: false,
            filters_ready: false,
            margin_ready: false,
            conflict_ready: matches!(template.market, StrategyMarket::Spot),
            balance_ready: false,
            strategy_type: template.strategy_type,
            market: template.market,
            mode: template.mode,
            runtime_phase: StrategyRuntimePhase::Draft,
            runtime_controls: RuntimeControls::default(),
            draft_revision: StrategyRevision {
                revision_id: format!("{template_id}-revision-1"),
                version: 1,
                strategy_type: template.strategy_type,
                generation: template.generation,
                levels: template.levels.clone(),
                amount_mode: template.amount_mode,
                futures_margin_mode: template.futures_margin_mode,
                leverage: template.leverage,
                reference_price_source: template.reference_price_source,
                reference_price: strategy_template_reference_price(&template),
                overall_take_profit_bps: template.overall_take_profit_bps,
                overall_stop_loss_bps: template.overall_stop_loss_bps,
                post_trigger_action: template.post_trigger_action,
            },
            tags: Vec::new(),
            notes: String::new(),
            active_revision: None,
            runtime: default_runtime(),
            archived_at: None,
        };
        self.normalize_strategy_levels(&mut temp_strategy)?;
        template.levels = temp_strategy.draft_revision.levels.clone();
        template.budget = summarize_budget_from_grid_levels(&template.levels);
        template.grid_spacing_bps = summarize_spacing_bps_from_grid_levels(&template.levels);
        if !has_explicit_reference {
            if let Some(derived) = derived_reference_price_from_grid_levels(
                template.strategy_type,
                template.mode,
                &template.levels,
            ) {
                set_strategy_template_reference_price(&template.id, Some(derived));
            }
        }

        let stored_template = StoredStrategyTemplate {
            sequence_id,
            template: template.clone(),
            reference_price: strategy_template_reference_price(&template)
                .map(|value| value.to_string()),
        };
        let audit = build_template_audit(
            actor_email,
            admin_role,
            session_sid,
            "strategy.template_created",
            None,
            &template,
        );
        self.db
            .insert_template_with_audit(&stored_template, &audit)
            .map_err(StrategyError::storage)?;
        Ok(template)
    }

    pub fn update_template(
        &self,
        actor_email: &str,
        admin_role: Option<AdminRole>,
        session_sid: u64,
        template_id: &str,
        request: UpdateTemplateRequest,
    ) -> Result<StrategyTemplate, StrategyError> {
        let has_explicit_reference =
            parse_decimal_extra(&request.strategy, "reference_price")?.is_some();
        let strategy = normalize_strategy_request(request.strategy)?;
        validate_strategy_request(&strategy)?;

        let existing = self
            .db
            .find_template(template_id)
            .map_err(StrategyError::storage)?
            .ok_or_else(|| StrategyError::not_found("template not found"))?;
        let mut template = build_template(template_id, strategy.clone())?;

        let mut temp_strategy = Strategy {
            id: template_id.to_string(),
            owner_email: actor_email.to_string(),
            name: template.name.clone(),
            symbol: template.symbol.clone(),
            budget: template.budget.clone(),
            grid_spacing_bps: template.grid_spacing_bps,
            status: StrategyStatus::Draft,
            source_template_id: None,
            membership_ready: false,
            exchange_ready: false,
            permissions_ready: false,
            withdrawals_disabled: false,
            hedge_mode_ready: false,
            symbol_ready: false,
            filters_ready: false,
            margin_ready: false,
            conflict_ready: matches!(template.market, StrategyMarket::Spot),
            balance_ready: false,
            strategy_type: template.strategy_type,
            market: template.market,
            mode: template.mode,
            runtime_phase: StrategyRuntimePhase::Draft,
            runtime_controls: RuntimeControls::default(),
            draft_revision: StrategyRevision {
                revision_id: format!("{template_id}-revision-1"),
                version: 1,
                strategy_type: template.strategy_type,
                generation: template.generation,
                levels: template.levels.clone(),
                amount_mode: template.amount_mode,
                futures_margin_mode: template.futures_margin_mode,
                leverage: template.leverage,
                reference_price_source: template.reference_price_source,
                reference_price: strategy_template_reference_price(&template),
                overall_take_profit_bps: template.overall_take_profit_bps,
                overall_stop_loss_bps: template.overall_stop_loss_bps,
                post_trigger_action: template.post_trigger_action,
            },
            tags: Vec::new(),
            notes: String::new(),
            active_revision: None,
            runtime: default_runtime(),
            archived_at: None,
        };
        self.normalize_strategy_levels(&mut temp_strategy)?;
        template.levels = temp_strategy.draft_revision.levels.clone();
        template.budget = summarize_budget_from_grid_levels(&template.levels);
        template.grid_spacing_bps = summarize_spacing_bps_from_grid_levels(&template.levels);
        if !has_explicit_reference {
            if let Some(derived) = derived_reference_price_from_grid_levels(
                template.strategy_type,
                template.mode,
                &template.levels,
            ) {
                set_strategy_template_reference_price(&template.id, Some(derived));
            }
        }

        let stored_template = StoredStrategyTemplate {
            sequence_id: existing
                .id
                .trim_start_matches("template-")
                .parse::<u64>()
                .unwrap_or_default(),
            template: template.clone(),
            reference_price: strategy_template_reference_price(&template)
                .map(|value| value.to_string()),
        };
        let audit = build_template_audit(
            actor_email,
            admin_role,
            session_sid,
            "strategy.template_updated",
            Some(&existing),
            &template,
        );
        let updated = self
            .db
            .update_template_with_audit(&stored_template, &audit)
            .map_err(StrategyError::storage)?;
        if updated == 0 {
            return Err(StrategyError::not_found("template not found"));
        }
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

        let strategy = build_strategy(
            sequence_id,
            owner_email,
            template_to_save_request(&template, request.name),
            Some(template.id.clone()),
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

fn build_template(
    template_id: &str,
    request: SaveStrategyRequest,
) -> Result<StrategyTemplate, StrategyError> {
    let reference_price = reference_price_for_request(&request)?;
    let revision = build_revision(
        template_id,
        1,
        request.resolved_strategy_type(),
        request.generation,
        request.amount_mode,
        request.futures_margin_mode,
        request.leverage,
        &request.levels,
        request.overall_take_profit_bps,
        request.overall_stop_loss_bps,
        request.reference_price_source,
        reference_price,
        request.post_trigger_action,
    )?;

    let template = StrategyTemplate {
        id: template_id.to_owned(),
        name: request.name,
        symbol: request.symbol,
        market: request.market,
        mode: request.mode,
        strategy_type: revision.strategy_type,
        generation: revision.generation,
        levels: revision.levels.clone(),
        amount_mode: revision.amount_mode,
        futures_margin_mode: revision.futures_margin_mode,
        leverage: revision.leverage,
        budget: summarize_budget(&request.levels)?,
        grid_spacing_bps: summarize_spacing_bps(&request.levels)?,
        membership_ready: false,
        exchange_ready: false,
        permissions_ready: false,
        withdrawals_disabled: false,
        hedge_mode_ready: false,
        symbol_ready: false,
        filters_ready: false,
        margin_ready: false,
        conflict_ready: matches!(request.market, StrategyMarket::Spot),
        balance_ready: false,
        overall_take_profit_bps: revision.overall_take_profit_bps,
        overall_stop_loss_bps: revision.overall_stop_loss_bps,
        reference_price_source: revision.reference_price_source,
        post_trigger_action: revision.post_trigger_action,
    };
    set_strategy_template_reference_price(&template.id, revision.reference_price);
    Ok(template)
}

fn build_template_audit(
    actor_email: &str,
    admin_role: Option<AdminRole>,
    session_sid: u64,
    action: &str,
    before: Option<&StrategyTemplate>,
    after: &StrategyTemplate,
) -> shared_db::AuditLogRecord {
    shared_db::AuditLogRecord {
        actor_email: actor_email.to_owned(),
        action: action.to_owned(),
        target_type: "strategy_template".to_owned(),
        target_id: after.id.clone(),
        payload: json!({
            "template_name": after.name,
            "symbol": after.symbol,
            "market": after.market,
            "mode": after.mode,
            "generation": after.generation,
            "level_count": after.levels.len(),
            "budget": after.budget,
            "grid_spacing_bps": after.grid_spacing_bps,
            "session_role": admin_role.map(|role| role.as_str()),
            "session_sid": session_sid,
            "before_summary": before.map(template_summary).unwrap_or_else(|| "template absent".to_owned()),
            "after_summary": template_summary(after),
        }),
        created_at: Utc::now(),
    }
}

fn template_summary(template: &StrategyTemplate) -> String {
    format!(
        "{} {} {:?} {:?} {} levels",
        template.name,
        template.symbol,
        template.market,
        template.generation,
        template.levels.len()
    )
}

fn template_to_save_request(template: &StrategyTemplate, name: String) -> SaveStrategyRequest {
    let mut extra = BTreeMap::new();
    if let Some(reference_price) = strategy_template_reference_price(template) {
        extra.insert(
            "reference_price".to_string(),
            Value::String(reference_price.to_string()),
        );
    }

    SaveStrategyRequest {
        name,
        symbol: template.symbol.clone(),
        market: template.market,
        mode: template.mode,
        strategy_type: Some(template.strategy_type),
        generation: template.generation,
        levels: template
            .levels
            .iter()
            .map(|level| SaveGridLevelRequest {
                entry_price: level.entry_price.to_string(),
                quantity: level.quantity.to_string(),
                take_profit_bps: level.take_profit_bps,
                trailing_bps: level.trailing_bps,
            })
            .collect(),
        amount_mode: template.amount_mode,
        futures_margin_mode: template.futures_margin_mode,
        leverage: template.leverage,
        membership_ready: false,
        exchange_ready: false,
        permissions_ready: false,
        withdrawals_disabled: false,
        hedge_mode_ready: false,
        symbol_ready: false,
        filters_ready: false,
        margin_ready: false,
        conflict_ready: matches!(template.market, StrategyMarket::Spot),
        balance_ready: false,
        overall_take_profit_bps: template.overall_take_profit_bps,
        overall_stop_loss_bps: template.overall_stop_loss_bps,
        reference_price_source: template.reference_price_source,
        post_trigger_action: template.post_trigger_action,
        tags: Vec::new(),
        notes: String::new(),
        extra,
    }
}

fn normalize_strategy_request(
    mut request: SaveStrategyRequest,
) -> Result<SaveStrategyRequest, StrategyError> {
    match request.strategy_type {
        None => {
            request.strategy_type = Some(
                if matches!(
                    request.mode,
                    StrategyMode::SpotClassic | StrategyMode::FuturesNeutral
                ) {
                    StrategyType::ClassicBilateralGrid
                } else {
                    StrategyType::OrdinaryGrid
                },
            );
        }
        Some(StrategyType::OrdinaryGrid)
            if matches!(
                request.mode,
                StrategyMode::SpotClassic | StrategyMode::FuturesNeutral
            ) =>
        {
            return Err(StrategyError::bad_request(
                "ordinary_grid is incompatible with SpotClassic/FuturesNeutral mode; use classic_bilateral_grid or omit strategy_type",
            ));
        }
        _ => {}
    }
    let hints = bilateral_request_hints(&request)?;
    match hints.strategy_type {
        StrategyType::OrdinaryGrid => {
            if request_has_bilateral_fields(hints) {
                return Err(StrategyError::bad_request(
                    "ordinary grid does not accept bilateral fields",
                ));
            }
        }
        StrategyType::ClassicBilateralGrid => {
            apply_classic_bilateral_defaults(&mut request, hints)?;
        }
        StrategyType::MartingaleGrid => {}
    }
    Ok(request)
}

fn bilateral_request_hints(
    request: &SaveStrategyRequest,
) -> Result<BilateralRequestHints, StrategyError> {
    Ok(BilateralRequestHints {
        strategy_type: request.resolved_strategy_type(),
        levels_per_side: parse_u32_extra(request, "levels_per_side")?,
        spacing_mode: parse_spacing_mode(request)?,
        grid_spacing_bps: parse_u32_extra(request, "grid_spacing_bps")?,
        reference_price: parse_decimal_extra(request, "reference_price")?,
    })
}

fn parse_spacing_mode(
    request: &SaveStrategyRequest,
) -> Result<Option<BilateralSpacingMode>, StrategyError> {
    match extra_string(request, "spacing_mode") {
        None => Ok(None),
        Some("fixed_step") => Ok(Some(BilateralSpacingMode::FixedStep)),
        Some("geometric") => Ok(Some(BilateralSpacingMode::Geometric)),
        Some(other) => Err(StrategyError::bad_request(&format!(
            "unsupported spacing_mode: {other}",
        ))),
    }
}

fn parse_u32_extra(
    request: &SaveStrategyRequest,
    field: &str,
) -> Result<Option<u32>, StrategyError> {
    let Some(value) = request.extra.get(field) else {
        return Ok(None);
    };
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| {
                StrategyError::bad_request(&format!("{field} must be a positive integer"))
            }),
        Value::String(raw) => raw.trim().parse::<u32>().map(Some).map_err(|_| {
            StrategyError::bad_request(&format!("{field} must be a positive integer"))
        }),
        _ => Err(StrategyError::bad_request(&format!(
            "{field} must be a positive integer",
        ))),
    }
}

fn parse_decimal_extra(
    request: &SaveStrategyRequest,
    field: &str,
) -> Result<Option<Decimal>, StrategyError> {
    let Some(raw) = extra_string(request, field) else {
        return Ok(None);
    };
    parse_decimal(raw, field).map(Some)
}

fn extra_string<'a>(request: &'a SaveStrategyRequest, field: &str) -> Option<&'a str> {
    request
        .extra
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn request_has_bilateral_fields(hints: BilateralRequestHints) -> bool {
    hints.levels_per_side.is_some()
        || hints.spacing_mode.is_some()
        || hints.grid_spacing_bps.is_some()
}

fn apply_classic_bilateral_defaults(
    request: &mut SaveStrategyRequest,
    hints: BilateralRequestHints,
) -> Result<(), StrategyError> {
    if let Some(spacing_mode) = hints.spacing_mode {
        request.generation = match spacing_mode {
            BilateralSpacingMode::FixedStep => GridGeneration::Arithmetic,
            BilateralSpacingMode::Geometric => GridGeneration::Geometric,
        };
    }

    if !request.levels.is_empty() {
        return Ok(());
    }

    let levels_per_side = hints.levels_per_side.ok_or_else(|| {
        StrategyError::bad_request(
            "classic bilateral grid requires levels_per_side when levels are omitted",
        )
    })?;
    if levels_per_side == 0 {
        return Err(StrategyError::bad_request(
            "classic bilateral grid requires levels_per_side greater than 0",
        ));
    }

    let reference_price = hints
        .reference_price
        .unwrap_or_else(|| Decimal::from(100u32));
    let spacing_bps = hints.grid_spacing_bps.unwrap_or(100);
    if spacing_bps == 0 {
        return Err(StrategyError::bad_request(
            "grid_spacing_bps must be greater than 0",
        ));
    }

    let template_level = request
        .levels
        .first()
        .cloned()
        .unwrap_or(SaveGridLevelRequest {
            entry_price: reference_price.normalize().to_string(),
            quantity: "1".to_string(),
            take_profit_bps: 100,
            trailing_bps: None,
        });
    let spacing_mode = hints
        .spacing_mode
        .unwrap_or(BilateralSpacingMode::FixedStep);
    request.levels = build_classic_bilateral_levels(
        reference_price,
        levels_per_side,
        spacing_bps,
        spacing_mode,
        &template_level,
    )?;
    Ok(())
}

fn build_classic_bilateral_levels(
    reference_price: Decimal,
    levels_per_side: u32,
    spacing_bps: u32,
    spacing_mode: BilateralSpacingMode,
    template_level: &SaveGridLevelRequest,
) -> Result<Vec<SaveGridLevelRequest>, StrategyError> {
    if reference_price <= Decimal::ZERO {
        return Err(StrategyError::bad_request(
            "reference_price must be positive for classic bilateral grid",
        ));
    }

    let step_ratio = Decimal::from(spacing_bps) / Decimal::from(10_000u32);
    let mut levels = Vec::with_capacity((levels_per_side * 2) as usize);

    for offset in (1..=levels_per_side).rev() {
        let entry_price =
            bilateral_level_price(reference_price, step_ratio, spacing_mode, offset, false)?;
        levels.push(SaveGridLevelRequest {
            entry_price: entry_price.normalize().to_string(),
            quantity: template_level.quantity.clone(),
            take_profit_bps: template_level.take_profit_bps,
            trailing_bps: template_level.trailing_bps,
        });
    }

    for offset in 1..=levels_per_side {
        let entry_price =
            bilateral_level_price(reference_price, step_ratio, spacing_mode, offset, true)?;
        levels.push(SaveGridLevelRequest {
            entry_price: entry_price.normalize().to_string(),
            quantity: template_level.quantity.clone(),
            take_profit_bps: template_level.take_profit_bps,
            trailing_bps: template_level.trailing_bps,
        });
    }

    Ok(levels)
}

fn bilateral_level_price(
    reference_price: Decimal,
    step_ratio: Decimal,
    spacing_mode: BilateralSpacingMode,
    offset: u32,
    upper_side: bool,
) -> Result<Decimal, StrategyError> {
    let price = match spacing_mode {
        BilateralSpacingMode::FixedStep => {
            let multiplier = Decimal::ONE + step_ratio * Decimal::from(offset);
            if upper_side {
                reference_price * multiplier
            } else {
                reference_price * (Decimal::ONE - step_ratio * Decimal::from(offset))
            }
        }
        BilateralSpacingMode::Geometric => {
            let growth = decimal_power(Decimal::ONE + step_ratio, offset);
            if upper_side {
                reference_price * growth
            } else {
                reference_price / growth
            }
        }
    };

    if price <= Decimal::ZERO {
        return Err(StrategyError::bad_request(
            "classic bilateral grid generated a non-positive level",
        ));
    }

    Ok(price)
}

fn decimal_power(base: Decimal, exponent: u32) -> Decimal {
    let mut value = Decimal::ONE;
    for _ in 0..exponent {
        value *= base;
    }
    value
}

fn validate_strategy_request(request: &SaveStrategyRequest) -> Result<(), StrategyError> {
    if request.symbol.trim().is_empty() {
        return Err(StrategyError::bad_request("symbol is required"));
    }
    if request.levels.is_empty() {
        return Err(StrategyError::bad_request("levels are required"));
    }
    if matches!(
        request.resolved_strategy_type(),
        StrategyType::ClassicBilateralGrid
    ) && request.levels.len() < 2
    {
        return Err(StrategyError::bad_request(
            "classic bilateral grid requires at least 2 levels",
        ));
    }
    if !strategy_mode_matches_market(request.market, request.mode) {
        return Err(StrategyError::bad_request(
            "market and mode are incompatible",
        ));
    }
    if !strategy_type_matches_mode(request.resolved_strategy_type(), request.mode) {
        return Err(StrategyError::bad_request(
            "strategy type and mode are incompatible",
        ));
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

    if !levels_follow_execution_order(&parsed, request.resolved_strategy_type(), request.mode) {
        return Err(StrategyError::bad_request(expected_levels_order_message(
            request.resolved_strategy_type(),
            request.mode,
        )));
    }

    if matches!(request.market, StrategyMarket::Spot) {
        if request.futures_margin_mode.is_some() || request.leverage.is_some() {
            return Err(StrategyError::bad_request(
                "spot strategies cannot set futures margin mode or leverage",
            ));
        }
    } else {
        if request.futures_margin_mode.is_none() {
            return Err(StrategyError::bad_request(
                "futures strategies require futures_margin_mode",
            ));
        }
        if request.leverage.unwrap_or(0) == 0 {
            return Err(StrategyError::bad_request(
                "futures strategies require leverage greater than 0",
            ));
        }
    }

    Ok(())
}

fn levels_follow_execution_order(
    levels: &[Decimal],
    strategy_type: StrategyType,
    mode: StrategyMode,
) -> bool {
    match expected_level_direction(strategy_type, mode) {
        LevelDirection::Ascending => levels.windows(2).all(|pair| pair[0] < pair[1]),
        LevelDirection::Descending => levels.windows(2).all(|pair| pair[0] > pair[1]),
    }
}

fn grid_levels_follow_execution_order(
    levels: &[GridLevel],
    strategy_type: StrategyType,
    mode: StrategyMode,
) -> bool {
    match expected_level_direction(strategy_type, mode) {
        LevelDirection::Ascending => levels
            .windows(2)
            .all(|pair| pair[0].entry_price < pair[1].entry_price),
        LevelDirection::Descending => levels
            .windows(2)
            .all(|pair| pair[0].entry_price > pair[1].entry_price),
    }
}

fn expected_levels_order_message(strategy_type: StrategyType, mode: StrategyMode) -> &'static str {
    match expected_level_direction(strategy_type, mode) {
        LevelDirection::Ascending => "levels must be strictly increasing by entry_price",
        LevelDirection::Descending => "levels must be strictly decreasing by entry_price",
    }
}

#[derive(Clone, Copy)]
enum LevelDirection {
    Ascending,
    Descending,
}

fn expected_level_direction(strategy_type: StrategyType, mode: StrategyMode) -> LevelDirection {
    if matches!(strategy_type, StrategyType::ClassicBilateralGrid) {
        return LevelDirection::Ascending;
    }
    match mode {
        StrategyMode::SpotClassic
        | StrategyMode::SpotSellOnly
        | StrategyMode::FuturesNeutral
        | StrategyMode::FuturesShort => LevelDirection::Ascending,
        StrategyMode::SpotBuyOnly | StrategyMode::FuturesLong => LevelDirection::Descending,
    }
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
    let resolved_strategy_type = request.resolved_strategy_type();
    let reference_price = reference_price_for_request(&request)?;
    let draft_revision = build_revision(
        &format!("strategy-{sequence_id}"),
        1,
        resolved_strategy_type,
        request.generation,
        request.amount_mode,
        request.futures_margin_mode,
        request.leverage,
        &request.levels,
        request.overall_take_profit_bps,
        request.overall_stop_loss_bps,
        request.reference_price_source,
        reference_price,
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
        membership_ready: false,
        exchange_ready: false,
        permissions_ready: false,
        withdrawals_disabled: false,
        hedge_mode_ready: false,
        symbol_ready: false,
        filters_ready: false,
        margin_ready: false,
        conflict_ready: matches!(request.market, StrategyMarket::Spot),
        balance_ready: false,
        strategy_type: resolved_strategy_type,
        market: request.market,
        mode: request.mode,
        runtime_phase: StrategyRuntimePhase::Draft,
        runtime_controls: RuntimeControls::default(),
        draft_revision,
        active_revision,
        runtime,
        tags: request.tags,
        notes: request.notes,
        archived_at,
    })
}

fn build_revision(
    strategy_id: &str,
    version: u32,
    strategy_type: StrategyType,
    generation: GridGeneration,
    amount_mode: StrategyAmountMode,
    futures_margin_mode: Option<FuturesMarginMode>,
    leverage: Option<u32>,
    levels: &[SaveGridLevelRequest],
    overall_take_profit_bps: Option<u32>,
    overall_stop_loss_bps: Option<u32>,
    reference_price_source: ReferencePriceSource,
    reference_price: Option<Decimal>,
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
        strategy_type,
        generation,
        levels,
        amount_mode,
        futures_margin_mode,
        leverage,
        reference_price_source,
        reference_price,
        overall_take_profit_bps,
        overall_stop_loss_bps,
        post_trigger_action,
    })
}

fn derived_reference_price_from_grid_levels(
    strategy_type: StrategyType,
    mode: StrategyMode,
    levels: &[GridLevel],
) -> Option<Decimal> {
    if levels.is_empty() {
        return None;
    }
    let prices: Vec<Decimal> = levels.iter().map(|l| l.entry_price).collect();
    Some(if uses_center_reference(strategy_type, mode) {
        midpoint_reference_price(&prices)
    } else {
        prices[0]
    })
}

fn reference_price_for_request(
    request: &SaveStrategyRequest,
) -> Result<Option<Decimal>, StrategyError> {
    Ok(
        parse_decimal_extra(request, "reference_price")?.or(reference_price_from_levels(
            request.resolved_strategy_type(),
            request.mode,
            &request.levels,
        )?),
    )
}

fn reference_price_from_levels(
    strategy_type: StrategyType,
    mode: StrategyMode,
    levels: &[SaveGridLevelRequest],
) -> Result<Option<Decimal>, StrategyError> {
    if levels.is_empty() {
        return Ok(None);
    }

    let prices = levels
        .iter()
        .map(|level| parse_decimal(&level.entry_price, "entry_price"))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(if uses_center_reference(strategy_type, mode) {
        midpoint_reference_price(&prices)
    } else {
        prices[0]
    }))
}

fn uses_center_reference(strategy_type: StrategyType, mode: StrategyMode) -> bool {
    matches!(strategy_type, StrategyType::ClassicBilateralGrid)
        || matches!(
            mode,
            StrategyMode::SpotClassic | StrategyMode::FuturesNeutral
        )
}

fn midpoint_reference_price(prices: &[Decimal]) -> Decimal {
    match prices {
        [] => Decimal::ZERO,
        [single] => *single,
        _ => {
            (prices.first().copied().unwrap_or(Decimal::ZERO)
                + prices.last().copied().unwrap_or(Decimal::ZERO))
                / Decimal::from(2u32)
        }
    }
}

fn parse_decimal(value: &str, field: &str) -> Result<Decimal, StrategyError> {
    value
        .parse::<Decimal>()
        .map_err(|_| StrategyError::bad_request(&format!("{field} must be a valid decimal string")))
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
    if first <= Decimal::ZERO || second <= Decimal::ZERO || second == first {
        return Ok(1);
    }

    let spacing = ((second - first).abs() / first) * Decimal::from(10_000u32);
    let spacing = spacing.round().to_string().parse::<u32>().unwrap_or(1);
    Ok(spacing.max(1))
}

#[derive(Debug, Clone, Default)]
struct ServerPreflightState {
    membership_ready: Option<bool>,
    exchange_ready: Option<bool>,
    permissions_ready: Option<bool>,
    withdrawals_disabled: Option<bool>,
    hedge_mode_ready: Option<bool>,
    symbol_ready: Option<bool>,
    filters_ready: Option<bool>,
    margin_ready: Option<bool>,
    conflict_ready: Option<bool>,
    balance_ready: Option<bool>,
}

impl StrategyService {
    fn live_preflight_report(&self, strategy: &Strategy) -> Result<PreflightReport, StrategyError> {
        let state = self.server_preflight_state(strategy)?;
        Ok(run_preflight_with_state(strategy, Some(&state)))
    }

    fn server_preflight_state(
        &self,
        strategy: &Strategy,
    ) -> Result<ServerPreflightState, StrategyError> {
        let membership_ready = self
            .db
            .find_membership_record(&strategy.owner_email)
            .map_err(StrategyError::storage)?
            .map(|record| membership_allows_runtime(&record, Utc::now()))
            .unwrap_or(false);

        let exchange_account = self
            .db
            .find_exchange_account(&strategy.owner_email, "binance")
            .map_err(StrategyError::storage)?;
        let exchange_ready = exchange_account
            .as_ref()
            .map(|record| exchange_account_supports_strategy(record, strategy.market))
            .unwrap_or(false);
        let permissions_ready = exchange_account
            .as_ref()
            .and_then(|record| metadata_bool(&record.metadata, &["validation", "permissions_ok"]))
            .unwrap_or(false);
        let hedge_mode_ready = exchange_account
            .as_ref()
            .and_then(|record| metadata_bool(&record.metadata, &["validation", "hedge_mode_ok"]))
            .unwrap_or(false);
        let withdrawals_disabled = exchange_account
            .as_ref()
            .and_then(|record| {
                metadata_bool(&record.metadata, &["validation", "withdrawals_disabled"]).or_else(
                    || metadata_bool(&record.metadata, &["validation", "withdraw_disabled"]),
                )
            })
            .unwrap_or(false);
        let symbol_ready = if exchange_account.is_some() {
            self.symbol_exists_for_strategy(strategy)?
        } else {
            false
        };
        let filters_ready = if exchange_account.is_some() {
            self.filters_satisfied(strategy)?
        } else {
            false
        };
        let margin_ready = if matches!(strategy.market, StrategyMarket::Spot) {
            true
        } else {
            hedge_mode_ready && self.margin_requirements_satisfied(strategy)?
        };
        let conflict_ready = if futures_conflict_required(strategy.market) {
            self.futures_conflict_free(strategy)?
        } else {
            true
        };
        let balance_ready = if exchange_account.is_some() {
            self.balance_available(strategy)?
        } else {
            false
        };

        Ok(ServerPreflightState {
            membership_ready: Some(membership_ready),
            exchange_ready: Some(exchange_ready),
            permissions_ready: Some(permissions_ready),
            withdrawals_disabled: Some(withdrawals_disabled),
            hedge_mode_ready: Some(hedge_mode_ready),
            symbol_ready: Some(symbol_ready),
            filters_ready: Some(filters_ready),
            margin_ready: Some(margin_ready),
            conflict_ready: Some(conflict_ready),
            balance_ready: Some(balance_ready),
        })
    }

    fn symbol_exists_for_strategy(&self, strategy: &Strategy) -> Result<bool, StrategyError> {
        let market_key = strategy_market_key(strategy.market);
        let symbols = self
            .db
            .list_exchange_symbols(&strategy.owner_email, "binance")
            .map_err(StrategyError::storage)?;
        Ok(symbols.into_iter().any(|record| {
            record.market == market_key
                && record.symbol.eq_ignore_ascii_case(&strategy.symbol)
                && record.status == "TRADING"
        }))
    }

    fn futures_conflict_free(&self, strategy: &Strategy) -> Result<bool, StrategyError> {
        let current_mask = futures_direction_mask(strategy.mode);
        if current_mask == 0 {
            return Ok(true);
        }

        let has_conflict = self
            .db
            .list_strategies(&strategy.owner_email)
            .map_err(StrategyError::storage)?
            .into_iter()
            .filter(|candidate| candidate.id != strategy.id)
            .filter(|candidate| candidate.market == strategy.market)
            .filter(|candidate| candidate.symbol.eq_ignore_ascii_case(&strategy.symbol))
            .filter(|candidate| candidate.status == StrategyStatus::Running)
            .any(|candidate| futures_direction_mask(candidate.mode) & current_mask != 0);

        Ok(!has_conflict)
    }

    fn symbol_record_for_strategy(
        &self,
        strategy: &Strategy,
    ) -> Result<Option<shared_db::UserExchangeSymbolRecord>, StrategyError> {
        let market_key = strategy_market_key(strategy.market);
        Ok(self
            .db
            .list_exchange_symbols(&strategy.owner_email, "binance")
            .map_err(StrategyError::storage)?
            .into_iter()
            .find(|record| {
                record.market == market_key
                    && record.symbol.eq_ignore_ascii_case(&strategy.symbol)
                    && record.status == "TRADING"
            }))
    }

    fn margin_requirements_satisfied(&self, strategy: &Strategy) -> Result<bool, StrategyError> {
        if matches!(strategy.market, StrategyMarket::Spot) {
            return Ok(true);
        }
        let Some(symbol) = self.symbol_record_for_strategy(strategy)? else {
            return Ok(false);
        };
        let supports_isolated = metadata_bool(
            &symbol.metadata,
            &["market_requirements", "supports_isolated_margin"],
        )
        .unwrap_or(true);
        let supports_cross = metadata_bool(
            &symbol.metadata,
            &["market_requirements", "supports_cross_margin"],
        )
        .unwrap_or(true);
        let leverage_brackets = metadata_u32_list(
            &symbol.metadata,
            &["market_requirements", "leverage_brackets"],
        );
        let margin_mode_ok = match strategy.draft_revision.futures_margin_mode {
            Some(FuturesMarginMode::Isolated) => supports_isolated,
            Some(FuturesMarginMode::Cross) => supports_cross,
            None => false,
        };
        let leverage_ok = strategy.draft_revision.leverage.is_some_and(|value| {
            value > 0 && (leverage_brackets.is_empty() || leverage_brackets.contains(&value))
        });
        Ok(margin_mode_ok && leverage_ok)
    }

    fn filters_satisfied(&self, strategy: &Strategy) -> Result<bool, StrategyError> {
        let Some(symbol) = self.symbol_record_for_strategy(strategy)? else {
            return Ok(false);
        };

        let min_quantity = parse_strategy_decimal(&symbol.min_quantity).unwrap_or(Decimal::ZERO);
        let min_notional = parse_strategy_decimal(&symbol.min_notional).unwrap_or(Decimal::ZERO);
        let levels = &strategy.draft_revision.levels;
        Ok(levels.iter().all(|level| {
            level.quantity >= min_quantity && (level.entry_price * level.quantity) >= min_notional
        }))
    }

    fn normalize_strategy_levels(&self, strategy: &mut Strategy) -> Result<(), StrategyError> {
        let Some(symbol) = self.symbol_record_for_strategy(strategy)? else {
            return Ok(());
        };
        let price_tick = metadata_string(&symbol.metadata, &["filters", "price_tick_size"])
            .as_deref()
            .and_then(parse_strategy_decimal);
        let quantity_step = metadata_string(&symbol.metadata, &["filters", "quantity_step_size"])
            .as_deref()
            .and_then(parse_strategy_decimal);
        let fallback_price_step = precision_step(symbol.price_precision);
        let fallback_quantity_step = precision_step(symbol.quantity_precision);

        for level in &mut strategy.draft_revision.levels {
            level.entry_price =
                normalize_to_step(level.entry_price, price_tick.or(fallback_price_step));
            level.quantity =
                normalize_to_step(level.quantity, quantity_step.or(fallback_quantity_step));
        }

        if !grid_levels_follow_execution_order(
            &strategy.draft_revision.levels,
            strategy.strategy_type,
            strategy.mode,
        ) {
            return Err(StrategyError::bad_request(
                "levels collapse after exchange filter normalization; widen grid spacing or reduce precision",
            ));
        }

        strategy.budget = summarize_budget_from_grid_levels(&strategy.draft_revision.levels);
        strategy.grid_spacing_bps =
            summarize_spacing_bps_from_grid_levels(&strategy.draft_revision.levels);
        Ok(())
    }

    fn balance_available(&self, strategy: &Strategy) -> Result<bool, StrategyError> {
        let Some(symbol) = self.symbol_record_for_strategy(strategy)? else {
            return Ok(false);
        };
        let snapshots = self
            .db
            .list_exchange_wallet_snapshots(&strategy.owner_email)
            .map_err(StrategyError::storage)?;
        let Some(snapshot) = snapshots.iter().rev().find(|snapshot| {
            snapshot.exchange == wallet_exchange_key(strategy.market)
                || snapshot.wallet_type == wallet_type_key(strategy.market)
        }) else {
            return Ok(false);
        };

        let reference_price = effective_reference_price(&strategy.draft_revision, strategy.mode);
        let quote_needed = strategy
            .draft_revision
            .levels
            .iter()
            .filter(|level| {
                matches!(
                    initial_entry_side(
                        strategy.mode,
                        level.level_index,
                        level.entry_price,
                        reference_price
                    ),
                    Some("Buy")
                )
            })
            .fold(Decimal::ZERO, |acc, level| {
                acc + (level.entry_price * level.quantity)
            });
        let base_needed = strategy
            .draft_revision
            .levels
            .iter()
            .filter(|level| {
                matches!(
                    initial_entry_side(
                        strategy.mode,
                        level.level_index,
                        level.entry_price,
                        reference_price
                    ),
                    Some("Sell")
                )
            })
            .fold(Decimal::ZERO, |acc, level| acc + level.quantity);

        if matches!(strategy.market, StrategyMarket::Spot) {
            return Ok(
                wallet_balance(snapshot, &symbol.quote_asset) >= quote_needed
                    && wallet_balance(snapshot, &symbol.base_asset) >= base_needed,
            );
        }

        let leverage = Decimal::from(strategy.draft_revision.leverage.unwrap_or(1));
        let collateral_needed = if matches!(strategy.market, StrategyMarket::FuturesCoinM) {
            if leverage.is_zero() {
                Decimal::ZERO
            } else {
                base_needed / leverage
            }
        } else if leverage.is_zero() {
            Decimal::ZERO
        } else {
            quote_needed / leverage
        };
        let collateral_asset = if matches!(strategy.market, StrategyMarket::FuturesCoinM) {
            symbol.base_asset.clone()
        } else {
            symbol.quote_asset.clone()
        };

        Ok(wallet_balance(snapshot, &collateral_asset) >= collateral_needed)
    }
}

fn precision_step(precision: i32) -> Option<Decimal> {
    let precision = u32::try_from(precision.max(0)).ok()?;
    Some(Decimal::ONE / Decimal::from(10u64.pow(precision)))
}

fn normalize_to_step(value: Decimal, step: Option<Decimal>) -> Decimal {
    let Some(step) = step.filter(|step| *step > Decimal::ZERO) else {
        return value.normalize();
    };
    ((value / step).floor() * step).normalize()
}

fn summarize_budget_from_grid_levels(levels: &[GridLevel]) -> String {
    levels
        .iter()
        .fold(Decimal::ZERO, |acc, level| acc + level.quantity)
        .normalize()
        .to_string()
}

fn summarize_spacing_bps_from_grid_levels(levels: &[GridLevel]) -> u32 {
    if levels.len() < 2 {
        return 1;
    }

    let first = levels[0].entry_price;
    let second = levels[1].entry_price;
    if first <= Decimal::ZERO || second <= Decimal::ZERO || second == first {
        return 1;
    }

    let spacing = ((second - first).abs() / first) * Decimal::from(10_000u32);
    spacing
        .round()
        .to_string()
        .parse::<u32>()
        .unwrap_or(1)
        .max(1)
}

fn parse_strategy_decimal(value: &str) -> Option<Decimal> {
    value.parse::<Decimal>().ok()
}

fn wallet_balance(snapshot: &shared_db::ExchangeWalletSnapshotRecord, asset: &str) -> Decimal {
    snapshot
        .balances
        .get(asset)
        .or_else(|| snapshot.balances.get(&asset.to_ascii_uppercase()))
        .and_then(|value| value.as_str())
        .and_then(parse_strategy_decimal)
        .unwrap_or(Decimal::ZERO)
}

#[cfg_attr(not(test), allow(dead_code))]
fn run_preflight(strategy: &Strategy) -> PreflightReport {
    run_preflight_with_state(strategy, None)
}

fn run_preflight_with_state(
    strategy: &Strategy,
    server_state: Option<&ServerPreflightState>,
) -> PreflightReport {
    let mut steps = Vec::new();
    let mut failures = Vec::new();

    let balance_reason = if matches!(strategy.market, StrategyMarket::Spot)
        && matches!(
            strategy.mode,
            StrategyMode::SpotClassic | StrategyMode::SpotSellOnly
        ) {
        "insufficient quote or base inventory for the configured spot grid"
    } else {
        "insufficient available balance or collateral"
    };
    let balance_guidance = if matches!(strategy.market, StrategyMarket::Spot)
        && matches!(strategy.mode, StrategyMode::SpotClassic)
    {
        "spot classic needs quote balance for buy grids and base asset inventory for sell grids; add funds or reduce the two-way ladder before starting"
    } else if matches!(strategy.market, StrategyMarket::Spot)
        && matches!(strategy.mode, StrategyMode::SpotSellOnly)
    {
        "spot sell-only needs enough base asset inventory to place the configured sell grids"
    } else {
        "add funds or reduce the configured grid size before starting"
    };

    let checks = [
        (
            "membership_status",
            true,
            server_state
                .and_then(|state| state.membership_ready)
                .unwrap_or(strategy.membership_ready),
            "membership is not active",
            "renew or reactivate membership before starting",
        ),
        (
            "exchange_connection",
            true,
            server_state
                .and_then(|state| state.exchange_ready)
                .unwrap_or(strategy.exchange_ready),
            "exchange credentials are not ready",
            "verify API key, secret, and required Binance permissions",
        ),
        (
            "exchange_permissions",
            true,
            server_state
                .and_then(|state| state.permissions_ready)
                .unwrap_or(strategy.permissions_ready),
            "required Binance trading permissions are missing",
            "enable the permissions required for this market before starting",
        ),
        (
            "withdrawal_permission_disabled",
            true,
            server_state
                .and_then(|state| state.withdrawals_disabled)
                .unwrap_or(strategy.withdrawals_disabled),
            "withdraw permission must be disabled",
            "turn off withdrawal permission on the Binance API key",
        ),
        (
            "hedge_mode",
            requires_futures_checks(strategy.market),
            server_state
                .and_then(|state| state.hedge_mode_ready)
                .unwrap_or(strategy.hedge_mode_ready),
            "hedge mode is required for futures strategies",
            "enable hedge mode on Binance before starting this futures strategy",
        ),
        (
            "symbol_support",
            true,
            server_state
                .and_then(|state| state.symbol_ready)
                .unwrap_or(strategy.symbol_ready),
            "symbol is not available",
            "choose a tradable symbol from synced Binance metadata",
        ),
        (
            "filters_and_notional",
            true,
            server_state
                .and_then(|state| state.filters_ready)
                .unwrap_or(strategy.filters_ready),
            "grid quantities or notionals do not satisfy exchange filters",
            "adjust the grid quantities to satisfy Binance min quantity and min notional filters",
        ),
        (
            "margin_or_leverage",
            requires_futures_checks(strategy.market),
            server_state
                .and_then(|state| state.margin_ready)
                .unwrap_or(strategy.margin_ready),
            "margin type or leverage settings are invalid",
            "fix margin mode or leverage before starting this futures strategy",
        ),
        (
            "strategy_conflicts",
            true,
            server_state
                .and_then(|state| state.conflict_ready)
                .unwrap_or(strategy.conflict_ready),
            "strategy conflicts with an existing active strategy",
            "stop or change the conflicting strategy before starting this one",
        ),
        (
            "balance_or_collateral",
            true,
            server_state
                .and_then(|state| state.balance_ready)
                .unwrap_or(strategy.balance_ready),
            balance_reason,
            balance_guidance,
        ),
        (
            "trailing_take_profit",
            true,
            strategy.draft_revision.levels.iter().all(|level| {
                level.trailing_bps.unwrap_or(level.take_profit_bps) <= level.take_profit_bps
            }),
            "trailing take profit exceeds the configured grid take profit range",
            "reduce trailing_bps so it does not exceed take_profit_bps",
        ),
    ];

    let mut blocked = false;
    for (step, applicable, ok, reason, guidance) in checks {
        if blocked {
            steps.push(PreflightStepResult {
                step: step.to_string(),
                status: PreflightStepStatus::Skipped,
                reason: None,
                guidance: None,
            });
            continue;
        }

        if !applicable {
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

fn membership_allows_runtime(record: &MembershipRecord, at: DateTime<Utc>) -> bool {
    match record.override_status {
        Some(MembershipStatus::Frozen) | Some(MembershipStatus::Revoked) => false,
        _ => {
            record.active_until.is_some_and(|until| at <= until)
                || record.grace_until.is_some_and(|until| at <= until)
        }
    }
}

fn exchange_account_supports_strategy(
    record: &UserExchangeAccountRecord,
    market: StrategyMarket,
) -> bool {
    if !record.is_active {
        return false;
    }

    let connection_status = metadata_string(&record.metadata, &["connection_status"]);
    let connectivity_ok = metadata_bool(&record.metadata, &["validation", "api_connectivity_ok"])
        .or_else(|| {
            connection_status
                .as_deref()
                .map(|status| matches!(status, "healthy" | "connected"))
        })
        .unwrap_or(true);
    let timestamp_in_sync =
        metadata_bool(&record.metadata, &["validation", "timestamp_in_sync"]).unwrap_or(true);
    let market_access_ok = match market {
        StrategyMarket::Spot => metadata_bool(&record.metadata, &["validation", "can_read_spot"]),
        StrategyMarket::FuturesUsdM => {
            metadata_bool(&record.metadata, &["validation", "can_read_usdm"])
        }
        StrategyMarket::FuturesCoinM => {
            metadata_bool(&record.metadata, &["validation", "can_read_coinm"])
        }
    }
    .or_else(|| metadata_bool(&record.metadata, &["validation", "market_access_ok"]))
    .unwrap_or(true);

    connectivity_ok && timestamp_in_sync && market_access_ok
}

fn metadata_bool(metadata: &Value, path: &[&str]) -> Option<bool> {
    metadata_at_path(metadata, path).and_then(|value| value.as_bool())
}

fn metadata_string(metadata: &Value, path: &[&str]) -> Option<String> {
    metadata_at_path(metadata, path)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

fn metadata_u32_list(metadata: &Value, path: &[&str]) -> Vec<u32> {
    metadata_at_path(metadata, path)
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_u64())
                .filter_map(|item| u32::try_from(item).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn metadata_at_path<'a>(metadata: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = metadata;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn strategy_market_key(market: StrategyMarket) -> &'static str {
    match market {
        StrategyMarket::Spot => "spot",
        StrategyMarket::FuturesUsdM => "usdm",
        StrategyMarket::FuturesCoinM => "coinm",
    }
}

fn futures_conflict_required(market: StrategyMarket) -> bool {
    !matches!(market, StrategyMarket::Spot)
}

fn futures_direction_mask(mode: StrategyMode) -> u8 {
    match mode {
        StrategyMode::SpotClassic | StrategyMode::SpotBuyOnly | StrategyMode::SpotSellOnly => 0,
        StrategyMode::FuturesLong => 0b01,
        StrategyMode::FuturesShort => 0b10,
        StrategyMode::FuturesNeutral => 0b11,
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
    let reference_price = runtime_reference_price(strategy, positions_override.as_deref())
        .unwrap_or_else(|| effective_reference_price(&active, strategy.mode));
    let positions = positions_override.unwrap_or_default();

    let mut runtime = StrategyRuntime {
        positions,
        orders: active
            .levels
            .iter()
            .filter_map(|level| {
                let side = initial_entry_side(
                    strategy.mode,
                    level.level_index,
                    level.entry_price,
                    reference_price,
                )?;
                Some(StrategyRuntimeOrder {
                    order_id: format!("{}-order-{}", strategy.id, level.level_index),
                    exchange_order_id: None,
                    level_index: Some(level.level_index),
                    side: side.to_string(),
                    order_type: "Limit".to_string(),
                    price: Some(level.entry_price),
                    quantity: level.quantity,
                    status: "Working".to_string(),
                })
            })
            .collect(),
        fills: Vec::new(),
        events: Vec::new(),
        last_preflight: None,
    };
    push_runtime_event(
        &mut runtime,
        event_type,
        event_type.replace('_', " ").as_str(),
        None,
    );
    runtime
}

fn resume_runtime(strategy: &Strategy) -> Result<StrategyRuntime, StrategyError> {
    let revision = strategy
        .active_revision
        .clone()
        .unwrap_or_else(|| strategy.draft_revision.clone());
    let mut engine = StrategyRuntimeEngine::from_runtime_snapshot(
        &strategy.id,
        strategy.market,
        strategy.mode,
        revision,
        strategy.runtime.clone(),
        false,
    )
    .map_err(|error| StrategyError::bad_request(&error.to_string()))?;
    engine
        .resume()
        .map_err(|error| StrategyError::bad_request(&error.to_string()))?;
    Ok(engine.snapshot().clone())
}

fn requires_futures_checks(market: StrategyMarket) -> bool {
    !matches!(market, StrategyMarket::Spot)
}

fn strategy_mode_matches_market(market: StrategyMarket, mode: StrategyMode) -> bool {
    match market {
        StrategyMarket::Spot => matches!(
            mode,
            StrategyMode::SpotClassic | StrategyMode::SpotBuyOnly | StrategyMode::SpotSellOnly
        ),
        StrategyMarket::FuturesUsdM | StrategyMarket::FuturesCoinM => matches!(
            mode,
            StrategyMode::FuturesLong | StrategyMode::FuturesShort | StrategyMode::FuturesNeutral
        ),
    }
}

fn strategy_type_matches_mode(strategy_type: StrategyType, mode: StrategyMode) -> bool {
    match strategy_type {
        StrategyType::OrdinaryGrid => matches!(
            mode,
            StrategyMode::SpotBuyOnly
                | StrategyMode::SpotSellOnly
                | StrategyMode::FuturesLong
                | StrategyMode::FuturesShort
        ),
        StrategyType::ClassicBilateralGrid => {
            matches!(
                mode,
                StrategyMode::SpotClassic | StrategyMode::FuturesNeutral
            )
        }
        StrategyType::MartingaleGrid => matches!(
            mode,
            StrategyMode::SpotBuyOnly
                | StrategyMode::SpotSellOnly
                | StrategyMode::FuturesLong
                | StrategyMode::FuturesShort
        ),
    }
}

fn wallet_exchange_key(market: StrategyMarket) -> &'static str {
    match market {
        StrategyMarket::Spot => "binance",
        StrategyMarket::FuturesUsdM => "binance-usdm",
        StrategyMarket::FuturesCoinM => "binance-coinm",
    }
}

fn wallet_type_key(market: StrategyMarket) -> &'static str {
    match market {
        StrategyMarket::Spot => "spot",
        StrategyMarket::FuturesUsdM => "usdm",
        StrategyMarket::FuturesCoinM => "coinm",
    }
}

fn initial_entry_side(
    mode: StrategyMode,
    level_index: u32,
    level_price: Decimal,
    reference_price: Decimal,
) -> Option<&'static str> {
    match mode {
        StrategyMode::SpotClassic => {
            if level_price < reference_price {
                Some("Buy")
            } else if level_price > reference_price {
                Some("Sell")
            } else {
                None
            }
        }
        StrategyMode::FuturesNeutral => {
            if level_price < reference_price {
                Some("Buy")
            } else if level_price > reference_price {
                Some("Sell")
            } else if level_index % 2 == 0 {
                Some("Buy")
            } else {
                Some("Sell")
            }
        }
        StrategyMode::SpotBuyOnly | StrategyMode::FuturesLong => {
            if level_price <= reference_price {
                Some("Buy")
            } else {
                None
            }
        }
        StrategyMode::SpotSellOnly | StrategyMode::FuturesShort => {
            if level_price >= reference_price {
                Some("Sell")
            } else {
                None
            }
        }
    }
}

fn effective_reference_price(revision: &StrategyRevision, mode: StrategyMode) -> Decimal {
    revision.reference_price.unwrap_or_else(|| {
        if uses_center_reference(revision.strategy_type, mode) {
            midpoint_reference_price(
                &revision
                    .levels
                    .iter()
                    .map(|level| level.entry_price)
                    .collect::<Vec<_>>(),
            )
        } else {
            revision
                .levels
                .first()
                .map(|level| level.entry_price)
                .unwrap_or(Decimal::ZERO)
        }
    })
}

fn runtime_reference_price(
    strategy: &Strategy,
    positions_override: Option<&[StrategyRuntimePosition]>,
) -> Option<Decimal> {
    if let Some(price) = strategy
        .runtime
        .events
        .iter()
        .rev()
        .find_map(|event| event.price)
    {
        return Some(price);
    }

    let positions = positions_override.unwrap_or(&strategy.runtime.positions);
    let total_quantity = positions
        .iter()
        .fold(Decimal::ZERO, |acc, position| acc + position.quantity);
    if total_quantity > Decimal::ZERO {
        let weighted_price = positions.iter().fold(Decimal::ZERO, |acc, position| {
            acc + (position.average_entry_price * position.quantity)
        });
        return Some(weighted_price / total_quantity);
    }

    strategy
        .runtime
        .fills
        .iter()
        .rev()
        .find_map(|fill| (fill.price > Decimal::ZERO).then_some(fill.price))
}

fn cancel_working_orders(orders: &mut [StrategyRuntimeOrder]) {
    for order in orders.iter_mut() {
        if matches!(order.status.as_str(), "Working" | "Placed") {
            order.status = "Canceled".to_string();
        }
    }
}

fn delete_failure_reason(strategy: &Strategy) -> Option<String> {
    if strategy.status == StrategyStatus::Archived {
        return Some("Strategy has already been deleted.".to_string());
    }
    if matches!(
        strategy.status,
        StrategyStatus::Running | StrategyStatus::Stopping
    ) {
        return Some("Strategy must be stopped before it can be deleted.".to_string());
    }
    if !strategy.runtime.positions.is_empty() {
        return Some("Strategy cannot be deleted while it still holds open positions.".to_string());
    }
    if strategy
        .runtime
        .orders
        .iter()
        .any(|order| matches!(order.status.as_str(), "Working" | "Placed"))
    {
        return Some(
            "Strategy cannot be deleted while there are working orders; stop it first.".to_string(),
        );
    }
    None
}

fn pause_failure_reason(status: StrategyStatus) -> &'static str {
    match status {
        StrategyStatus::Draft => {
            "Strategy has not started yet; only running strategies can be paused."
        }
        StrategyStatus::Stopped => "Strategy is already stopped; pause is only available while running.",
        StrategyStatus::Paused => "Strategy is already paused.",
        StrategyStatus::ErrorPaused => {
            "Strategy is paused because of an error; resolve the runtime issue before pausing again."
        }
        StrategyStatus::Completed => "Strategy has completed and cannot be paused.",
        StrategyStatus::Stopping => {
            "Strategy is stopping; wait for the stop to finish before triggering pause."
        }
        StrategyStatus::Archived => "Strategy has already been deleted.",
        _ => "Strategy is not running and cannot be paused right now.",
    }
}

fn stop_strategy_runtime(strategy: &mut Strategy) -> Result<(), StrategyError> {
    match strategy.status {
        StrategyStatus::Running | StrategyStatus::Paused => {
            cancel_working_orders(&mut strategy.runtime.orders);
            if strategy.runtime.positions.is_empty() {
                strategy.status = StrategyStatus::Stopped;
                push_runtime_event(
                    &mut strategy.runtime,
                    "strategy_stopped",
                    "strategy stopped",
                    None,
                );
            } else {
                strategy.status = StrategyStatus::Stopping;
                push_runtime_event(
                    &mut strategy.runtime,
                    "strategy_stop_requested",
                    "strategy stop requested; waiting for exchange close reconciliation",
                    None,
                );
            }
            Ok(())
        }
        StrategyStatus::Draft => Err(StrategyError::conflict(
            "Stop is only available for running or paused strategies; this draft has never been started.",
        )),
        status => Err(StrategyError::conflict(format!(
            "Stop is only available for running or paused strategies (current status: {}).",
            strategy_status_label(status)
        ))),
    }
}

fn strategy_status_label(status: StrategyStatus) -> &'static str {
    match status {
        StrategyStatus::Draft => "Draft",
        StrategyStatus::Running => "Running",
        StrategyStatus::Paused => "Paused",
        StrategyStatus::ErrorPaused => "ErrorPaused",
        StrategyStatus::Completed => "Completed",
        StrategyStatus::Stopping => "Stopping",
        StrategyStatus::Stopped => "Stopped",
        StrategyStatus::Archived => "Archived",
    }
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
    fn into_batch_start_failure(self, strategy_id: String) -> BatchStartFailure {
        let preflight = self
            .extra
            .as_ref()
            .and_then(|extra| extra.get("preflight"))
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok());
        BatchStartFailure {
            strategy_id,
            error: self.message,
            preflight,
        }
    }

    fn bad_request(message: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.to_string(),
            extra: None,
        }
    }

    pub fn not_found(message: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.to_string(),
            extra: None,
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
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
        rebuild_runtime, run_preflight, strategy_template_reference_price,
        template_to_save_request, ApplyTemplateRequest, CreateTemplateRequest,
        SaveGridLevelRequest, SaveStrategyRequest, StrategyError, StrategyService,
        UpdateTemplateRequest,
    };
    use crate::services::auth_service::AdminRole;
    use axum::http::StatusCode;
    use chrono::Utc;
    use serde_json::{json, Value};
    use shared_db::{MembershipRecord, SharedDb, UserExchangeAccountRecord};
    use shared_domain::strategy::{
        FuturesMarginMode, GridGeneration, GridLevel, PostTriggerAction, PreflightStepStatus,
        ReferencePriceSource, RuntimeControls, Strategy, StrategyAmountMode, StrategyMarket,
        StrategyMode, StrategyRevision, StrategyRuntime, StrategyRuntimeFill, StrategyRuntimePhase,
        StrategyRuntimePosition, StrategyStatus, StrategyType,
    };
    use std::{
        collections::BTreeMap,
        fs,
        net::TcpListener,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn martingale_grid_strategy_type_matches_supported_modes() {
        assert!(super::strategy_type_matches_mode(
            StrategyType::MartingaleGrid,
            StrategyMode::SpotBuyOnly
        ));
        assert!(super::strategy_type_matches_mode(
            StrategyType::MartingaleGrid,
            StrategyMode::FuturesLong
        ));
        assert!(super::strategy_type_matches_mode(
            StrategyType::MartingaleGrid,
            StrategyMode::FuturesShort
        ));
        assert!(!super::strategy_type_matches_mode(
            StrategyType::MartingaleGrid,
            StrategyMode::SpotClassic
        ));
    }

    #[test]
    fn create_template_fails_when_audit_write_fails() {
        let harness = PersistentRuntimeHarness::start("strategy-audit");
        let db = SharedDb::connect(harness.database_url(), harness.redis_url()).expect("db");
        let service = StrategyService::new(db.clone());

        harness.break_audit_table();

        let result = service.create_template(
            "super-admin@example.com",
            Some(AdminRole::SuperAdmin),
            7,
            CreateTemplateRequest {
                strategy: template_request("Template A"),
            },
        );

        match result {
            Err(StrategyError {
                status, message, ..
            }) => {
                assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
                assert_eq!(message, "internal storage error");
            }
            Ok(_) => panic!("template create should fail when audit write fails"),
        }

        assert!(
            db.list_templates().expect("templates").is_empty(),
            "failed template create must not persist"
        );
    }

    #[test]
    fn list_templates_fails_when_storage_read_fails() {
        let harness = PersistentRuntimeHarness::start("strategy-template-read");
        let db = SharedDb::connect(harness.database_url(), harness.redis_url()).expect("db");
        let service = StrategyService::new(db.clone());

        service
            .create_template(
                "super-admin@example.com",
                Some(AdminRole::SuperAdmin),
                8,
                CreateTemplateRequest {
                    strategy: template_request("Template B"),
                },
            )
            .expect("template created");

        harness.break_templates_table();

        match service.list_templates() {
            Err(StrategyError {
                status, message, ..
            }) => {
                assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
                assert_eq!(message, "internal storage error");
            }
            Ok(_) => panic!("template list should fail when template storage is unavailable"),
        }
    }

    #[test]
    fn apply_template_preserves_classic_bilateral_strategy_type() {
        let db = SharedDb::ephemeral().expect("db");
        let service = StrategyService::new(db);
        let mut request = template_request("Classic Template");
        request.strategy_type = Some(StrategyType::ClassicBilateralGrid);
        request.mode = StrategyMode::SpotClassic;
        request.levels = vec![
            SaveGridLevelRequest {
                entry_price: "95".to_string(),
                quantity: "1".to_string(),
                take_profit_bps: 100,
                trailing_bps: None,
            },
            SaveGridLevelRequest {
                entry_price: "105".to_string(),
                quantity: "1".to_string(),
                take_profit_bps: 100,
                trailing_bps: None,
            },
        ];
        request.reference_price_source = ReferencePriceSource::Market;
        request
            .extra
            .insert("levels_per_side".to_string(), json!(2));
        request
            .extra
            .insert("spacing_mode".to_string(), json!("fixed_step"));
        request
            .extra
            .insert("grid_spacing_bps".to_string(), json!(100));
        request
            .extra
            .insert("reference_price".to_string(), json!("100"));

        let template = service
            .create_template(
                "super-admin@example.com",
                Some(AdminRole::SuperAdmin),
                9,
                CreateTemplateRequest { strategy: request },
            )
            .expect("template created");
        let listed = service.list_templates().expect("list templates");
        let listed_template = listed
            .items
            .into_iter()
            .find(|item| item.id == template.id)
            .expect("listed template");
        assert_eq!(
            listed_template.strategy_type,
            StrategyType::ClassicBilateralGrid
        );
        assert_eq!(
            listed_template.reference_price_source,
            ReferencePriceSource::Market
        );

        let updated = service
            .update_template(
                "super-admin@example.com",
                Some(AdminRole::SuperAdmin),
                10,
                &listed_template.id,
                UpdateTemplateRequest {
                    strategy: template_to_save_request(
                        &listed_template,
                        listed_template.name.clone(),
                    ),
                },
            )
            .expect("update template");
        assert_eq!(updated.strategy_type, StrategyType::ClassicBilateralGrid);
        assert_eq!(updated.reference_price_source, ReferencePriceSource::Market);
        assert_eq!(
            strategy_template_reference_price(&updated),
            Some("100".parse().expect("decimal"))
        );

        let applied = service
            .apply_template(
                "trader@example.com",
                &updated.id,
                ApplyTemplateRequest {
                    name: "Applied Classic".to_string(),
                },
            )
            .expect("apply template");

        assert_eq!(applied.strategy_type, StrategyType::ClassicBilateralGrid);
        assert_eq!(
            applied.draft_revision.strategy_type,
            StrategyType::ClassicBilateralGrid
        );
        assert_eq!(
            applied.draft_revision.reference_price_source,
            ReferencePriceSource::Market
        );
    }

    fn template_request(name: &str) -> SaveStrategyRequest {
        SaveStrategyRequest {
            name: name.to_string(),
            symbol: "BTCUSDT".to_string(),
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotBuyOnly,
            strategy_type: Some(StrategyType::OrdinaryGrid),
            generation: GridGeneration::Custom,
            levels: vec![
                SaveGridLevelRequest {
                    entry_price: "105".to_string(),
                    quantity: "1".to_string(),
                    take_profit_bps: 100,
                    trailing_bps: None,
                },
                SaveGridLevelRequest {
                    entry_price: "95".to_string(),
                    quantity: "1".to_string(),
                    take_profit_bps: 100,
                    trailing_bps: None,
                },
            ],
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            membership_ready: true,
            exchange_ready: true,
            permissions_ready: true,
            withdrawals_disabled: true,
            hedge_mode_ready: true,
            symbol_ready: true,
            filters_ready: true,
            margin_ready: true,
            conflict_ready: true,
            balance_ready: true,
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            reference_price_source: ReferencePriceSource::Manual,
            post_trigger_action: PostTriggerAction::Stop,
            tags: Vec::new(),
            notes: String::new(),
            extra: BTreeMap::new(),
        }
    }

    struct PersistentRuntimeHarness {
        project_name: String,
        override_file: PathBuf,
        postgres_port: u16,
        redis_port: u16,
    }

    impl PersistentRuntimeHarness {
        fn start(prefix: &str) -> Self {
            let workspace_root = workspace_root();
            let postgres_port = pick_unused_port();
            let redis_port = pick_unused_port();
            let project_name = format!(
                "{prefix}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("unix time")
                    .as_nanos()
            );
            let override_file = std::env::temp_dir().join(format!("{project_name}.yml"));
            fs::write(
                &override_file,
                format!(
                    "services:
  postgres:
    ports:
      - \"{postgres_port}:5432\"

  redis:
    ports:
      - \"{redis_port}:6379\"
"
                ),
            )
            .expect("write compose override");
            run_command(
                Command::new("docker")
                    .arg("compose")
                    .arg("-p")
                    .arg(&project_name)
                    .arg("--env-file")
                    .arg(workspace_root.join(".env.example"))
                    .arg("-f")
                    .arg(workspace_root.join("deploy/docker/docker-compose.yml"))
                    .arg("-f")
                    .arg(&override_file)
                    .arg("up")
                    .arg("-d")
                    .arg("--wait")
                    .arg("postgres")
                    .arg("redis"),
                "start persistent runtime",
            );

            Self {
                project_name,
                override_file,
                postgres_port,
                redis_port,
            }
        }

        fn database_url(&self) -> String {
            format!(
                "postgres://postgres:postgres@127.0.0.1:{}/grid_binance",
                self.postgres_port
            )
        }

        fn redis_url(&self) -> String {
            format!("redis://127.0.0.1:{}/0", self.redis_port)
        }

        fn break_audit_table(&self) {
            run_command(
                Command::new("docker")
                    .arg("exec")
                    .arg(format!("{}-postgres-1", self.project_name))
                    .arg("psql")
                    .arg("-U")
                    .arg("postgres")
                    .arg("-d")
                    .arg("grid_binance")
                    .arg("-c")
                    .arg("ALTER TABLE audit_logs RENAME TO audit_logs_disabled"),
                "break audit table",
            );
        }

        fn break_templates_table(&self) {
            run_command(
                Command::new("docker")
                    .arg("exec")
                    .arg(format!("{}-postgres-1", self.project_name))
                    .arg("psql")
                    .arg("-U")
                    .arg("postgres")
                    .arg("-d")
                    .arg("grid_binance")
                    .arg("-c")
                    .arg("ALTER TABLE strategy_templates RENAME TO strategy_templates_disabled"),
                "break strategy templates table",
            );
        }
    }

    impl Drop for PersistentRuntimeHarness {
        fn drop(&mut self) {
            let workspace_root = workspace_root();
            let _ = Command::new("docker")
                .arg("compose")
                .arg("-p")
                .arg(&self.project_name)
                .arg("--env-file")
                .arg(workspace_root.join(".env.example"))
                .arg("-f")
                .arg(workspace_root.join("deploy/docker/docker-compose.yml"))
                .arg("-f")
                .arg(&self.override_file)
                .arg("down")
                .arg("-v")
                .status();
            let _ = fs::remove_file(&self.override_file);
        }
    }

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root")
    }

    fn pick_unused_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .expect("bind random port")
            .local_addr()
            .expect("local addr")
            .port()
    }

    fn run_command(command: &mut Command, context: &str) {
        let output = command.output().expect(context);
        assert!(
            output.status.success(),
            "{context} failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

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
                    mode: StrategyMode::SpotBuyOnly,
                    strategy_type: Some(StrategyType::OrdinaryGrid),
                    generation: GridGeneration::Custom,
                    levels: vec![SaveGridLevelRequest {
                        entry_price: "100".to_string(),
                        quantity: "1".to_string(),
                        take_profit_bps: 100,
                        trailing_bps: None,
                    }],
                    amount_mode: StrategyAmountMode::Quote,
                    futures_margin_mode: None,
                    leverage: None,
                    membership_ready: true,
                    exchange_ready: false,
                    permissions_ready: true,
                    withdrawals_disabled: true,
                    hedge_mode_ready: true,
                    symbol_ready: true,
                    filters_ready: true,
                    margin_ready: true,
                    conflict_ready: true,
                    balance_ready: true,
                    overall_take_profit_bps: None,
                    overall_stop_loss_bps: None,
                    reference_price_source: ReferencePriceSource::Manual,
                    post_trigger_action: PostTriggerAction::Stop,
                    tags: Vec::new(),
                    notes: String::new(),
                    extra: BTreeMap::new(),
                },
            )
            .expect("strategy");

        let mut strategy = strategy;
        strategy.membership_ready = true;
        let report = run_preflight(&strategy);

        assert!(!report.ok);
        assert_eq!(report.steps[0].step, "membership_status");
        assert_eq!(report.steps[1].step, "exchange_connection");
        assert_eq!(report.failures[0].step, "exchange_connection");
    }

    #[test]
    fn server_preflight_does_not_trust_persisted_client_ready_flags_when_server_state_is_missing() {
        let db = SharedDb::ephemeral().expect("db");
        let service = StrategyService::new(db);

        let strategy = service
            .create_strategy(
                "trader@example.com",
                SaveStrategyRequest {
                    name: "spot".to_string(),
                    symbol: "BTCUSDT".to_string(),
                    market: StrategyMarket::Spot,
                    mode: StrategyMode::SpotBuyOnly,
                    strategy_type: Some(StrategyType::OrdinaryGrid),
                    generation: GridGeneration::Custom,
                    levels: vec![SaveGridLevelRequest {
                        entry_price: "100".to_string(),
                        quantity: "1".to_string(),
                        take_profit_bps: 100,
                        trailing_bps: None,
                    }],
                    amount_mode: StrategyAmountMode::Quote,
                    futures_margin_mode: None,
                    leverage: None,
                    membership_ready: true,
                    exchange_ready: true,
                    permissions_ready: true,
                    withdrawals_disabled: true,
                    hedge_mode_ready: true,
                    symbol_ready: true,
                    filters_ready: true,
                    margin_ready: true,
                    conflict_ready: true,
                    balance_ready: true,
                    overall_take_profit_bps: None,
                    overall_stop_loss_bps: None,
                    reference_price_source: ReferencePriceSource::Manual,
                    post_trigger_action: PostTriggerAction::Stop,
                    tags: Vec::new(),
                    notes: String::new(),
                    extra: BTreeMap::new(),
                },
            )
            .expect("strategy");

        let report = service
            .preflight_strategy("trader@example.com", &strategy.id)
            .expect("preflight");

        assert!(!report.ok);
        assert_eq!(
            report
                .steps
                .iter()
                .find(|step| step.step == "membership_status")
                .expect("membership step")
                .status,
            PreflightStepStatus::Failed
        );
        assert_eq!(
            report
                .steps
                .iter()
                .find(|step| step.step == "exchange_connection")
                .expect("exchange step")
                .status,
            PreflightStepStatus::Skipped
        );
        assert_eq!(report.failures[0].step, "membership_status");
    }

    #[test]
    fn server_preflight_overrides_false_client_flags_when_server_state_is_satisfied() {
        let db = SharedDb::ephemeral().expect("db");
        let service = StrategyService::new(db.clone());
        let now = Utc::now();
        db.upsert_membership_record(
            "trader@example.com",
            &MembershipRecord {
                activated_at: Some(now),
                active_until: Some(now + chrono::Duration::days(30)),
                grace_until: Some(now + chrono::Duration::days(32)),
                override_status: None,
            },
        )
        .expect("membership");
        db.upsert_exchange_account(&UserExchangeAccountRecord {
            user_email: "trader@example.com".to_string(),
            exchange: "binance".to_string(),
            account_label: "Binance".to_string(),
            market_scope: "spot".to_string(),
            is_active: true,
            checked_at: Some(now),
            metadata: serde_json::json!({
                "connection_status": "connected",
                "validation": {
                    "api_connectivity_ok": true,
                    "timestamp_in_sync": true,
                    "permissions_ok": true,
                    "hedge_mode_ok": true,
                    "withdrawals_disabled": true,
                    "can_read_spot": true,
                    "market_access_ok": true
                }
            }),
        })
        .expect("account");
        db.replace_exchange_symbols(
            "trader@example.com",
            "binance",
            &[shared_db::UserExchangeSymbolRecord {
                user_email: "trader@example.com".to_string(),
                exchange: "binance".to_string(),
                market: "spot".to_string(),
                symbol: "BTCUSDT".to_string(),
                status: "TRADING".to_string(),
                base_asset: "BTC".to_string(),
                quote_asset: "USDT".to_string(),
                price_precision: 2,
                quantity_precision: 4,
                min_quantity: "0.001".to_string(),
                min_notional: "5".to_string(),
                keywords: vec!["btcusdt".to_string()],
                metadata: serde_json::json!({}),
                synced_at: now,
            }],
        )
        .expect("symbols");
        db.insert_exchange_wallet_snapshot(&shared_db::ExchangeWalletSnapshotRecord {
            user_email: "trader@example.com".to_string(),
            exchange: "binance".to_string(),
            wallet_type: "spot".to_string(),
            balances: serde_json::json!({ "USDT": "1000" }),
            captured_at: now,
        })
        .expect("wallet");

        let strategy = service
            .create_strategy(
                "trader@example.com",
                SaveStrategyRequest {
                    name: "spot".to_string(),
                    symbol: "BTCUSDT".to_string(),
                    market: StrategyMarket::Spot,
                    mode: StrategyMode::SpotBuyOnly,
                    strategy_type: Some(StrategyType::OrdinaryGrid),
                    generation: GridGeneration::Custom,
                    levels: vec![SaveGridLevelRequest {
                        entry_price: "100".to_string(),
                        quantity: "1".to_string(),
                        take_profit_bps: 100,
                        trailing_bps: None,
                    }],
                    amount_mode: StrategyAmountMode::Quote,
                    futures_margin_mode: None,
                    leverage: None,
                    membership_ready: false,
                    exchange_ready: false,
                    permissions_ready: false,
                    withdrawals_disabled: false,
                    hedge_mode_ready: false,
                    symbol_ready: false,
                    filters_ready: false,
                    margin_ready: false,
                    conflict_ready: false,
                    balance_ready: false,
                    overall_take_profit_bps: None,
                    overall_stop_loss_bps: None,
                    reference_price_source: ReferencePriceSource::Manual,
                    post_trigger_action: PostTriggerAction::Stop,
                    tags: Vec::new(),
                    notes: String::new(),
                    extra: BTreeMap::new(),
                },
            )
            .expect("strategy");

        let report = service
            .preflight_strategy("trader@example.com", &strategy.id)
            .expect("preflight");
        assert!(report.ok);
        assert_eq!(
            report
                .steps
                .iter()
                .find(|step| step.step == "filters_and_notional")
                .unwrap()
                .status,
            PreflightStepStatus::Passed
        );
        assert_eq!(
            report
                .steps
                .iter()
                .find(|step| step.step == "balance_or_collateral")
                .unwrap()
                .status,
            PreflightStepStatus::Passed
        );
        assert_eq!(
            report
                .steps
                .iter()
                .find(|step| step.step == "strategy_conflicts")
                .unwrap()
                .status,
            PreflightStepStatus::Passed
        );
    }

    #[test]
    fn futures_preflight_includes_permissions_hedge_margin_and_balance_steps() {
        let service = StrategyService::new(SharedDb::ephemeral().expect("db"));
        let strategy = service
            .create_strategy(
                "trader@example.com",
                SaveStrategyRequest {
                    name: "futures".to_string(),
                    symbol: "BTCUSDT".to_string(),
                    market: StrategyMarket::FuturesUsdM,
                    mode: StrategyMode::FuturesLong,
                    strategy_type: Some(StrategyType::OrdinaryGrid),
                    generation: GridGeneration::Custom,
                    levels: vec![SaveGridLevelRequest {
                        entry_price: "100".to_string(),
                        quantity: "1".to_string(),
                        take_profit_bps: 100,
                        trailing_bps: None,
                    }],
                    amount_mode: StrategyAmountMode::Quote,
                    futures_margin_mode: Some(FuturesMarginMode::Cross),
                    leverage: Some(5),
                    membership_ready: true,
                    exchange_ready: true,
                    permissions_ready: false,
                    withdrawals_disabled: true,
                    hedge_mode_ready: false,
                    symbol_ready: true,
                    filters_ready: false,
                    margin_ready: false,
                    conflict_ready: false,
                    balance_ready: false,
                    overall_take_profit_bps: None,
                    overall_stop_loss_bps: None,
                    reference_price_source: ReferencePriceSource::Manual,
                    post_trigger_action: PostTriggerAction::Stop,
                    tags: Vec::new(),
                    notes: String::new(),
                    extra: BTreeMap::new(),
                },
            )
            .expect("strategy");

        let mut strategy = strategy;
        strategy.membership_ready = true;
        strategy.exchange_ready = true;
        strategy.permissions_ready = false;
        strategy.withdrawals_disabled = true;
        strategy.hedge_mode_ready = false;
        strategy.symbol_ready = true;
        strategy.filters_ready = false;
        strategy.margin_ready = false;
        strategy.conflict_ready = false;
        strategy.balance_ready = false;
        let report = run_preflight(&strategy);

        assert_eq!(report.steps.len(), 11);
        assert_eq!(report.steps[2].step, "exchange_permissions");
        assert_eq!(report.steps[3].step, "withdrawal_permission_disabled");
        assert_eq!(report.steps[4].step, "hedge_mode");
        assert_eq!(report.steps[7].step, "margin_or_leverage");
        assert_eq!(report.steps[8].step, "strategy_conflicts");
        assert_eq!(report.steps[9].step, "balance_or_collateral");
        assert_eq!(report.failures[0].step, "exchange_permissions");
    }

    #[test]
    fn spot_classic_rebuild_splits_orders_around_reference_price() {
        let revision = StrategyRevision {
            revision_id: "rev-classic".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            levels: vec![
                GridLevel {
                    level_index: 0,
                    entry_price: "95".parse().expect("decimal"),
                    quantity: "1".parse().expect("decimal"),
                    take_profit_bps: 100,
                    trailing_bps: None,
                },
                GridLevel {
                    level_index: 1,
                    entry_price: "105".parse().expect("decimal"),
                    quantity: "1".parse().expect("decimal"),
                    take_profit_bps: 100,
                    trailing_bps: None,
                },
            ],
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        };
        let strategy = Strategy {
            id: "strategy-classic".to_string(),
            owner_email: "trader@example.com".to_string(),
            name: "classic".to_string(),
            symbol: "BTCUSDT".to_string(),
            budget: "200".to_string(),
            grid_spacing_bps: 100,
            status: StrategyStatus::Draft,
            source_template_id: None,
            membership_ready: true,
            exchange_ready: true,
            permissions_ready: true,
            withdrawals_disabled: true,
            hedge_mode_ready: true,
            symbol_ready: true,
            filters_ready: true,
            margin_ready: true,
            conflict_ready: true,
            balance_ready: true,
            strategy_type: StrategyType::OrdinaryGrid,
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            runtime_phase: StrategyRuntimePhase::Draft,
            runtime_controls: RuntimeControls::default(),
            draft_revision: revision.clone(),
            active_revision: Some(revision),
            runtime: StrategyRuntime::default(),
            tags: Vec::new(),
            notes: String::new(),
            archived_at: None,
        };

        let runtime = rebuild_runtime(&strategy, None, "strategy_started");

        assert_eq!(runtime.orders.len(), 2);
        assert_eq!(runtime.orders[0].side, "Buy");
        assert_eq!(runtime.orders[1].side, "Sell");
    }

    #[test]
    fn futures_neutral_rebuild_uses_dual_sided_orders() {
        let revision = StrategyRevision {
            revision_id: "rev-neutral".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            levels: vec![
                GridLevel {
                    level_index: 0,
                    entry_price: "100".parse().expect("decimal"),
                    quantity: "1".parse().expect("decimal"),
                    take_profit_bps: 100,
                    trailing_bps: None,
                },
                GridLevel {
                    level_index: 1,
                    entry_price: "101".parse().expect("decimal"),
                    quantity: "1".parse().expect("decimal"),
                    take_profit_bps: 100,
                    trailing_bps: None,
                },
            ],
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: Some(FuturesMarginMode::Cross),
            leverage: Some(5),
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        };
        let strategy = Strategy {
            id: "strategy-neutral".to_string(),
            owner_email: "trader@example.com".to_string(),
            name: "neutral".to_string(),
            symbol: "BTCUSDT".to_string(),
            budget: "2".to_string(),
            grid_spacing_bps: 100,
            status: StrategyStatus::Draft,
            source_template_id: None,
            membership_ready: true,
            exchange_ready: true,
            permissions_ready: true,
            withdrawals_disabled: true,
            hedge_mode_ready: true,
            symbol_ready: true,
            filters_ready: true,
            margin_ready: true,
            conflict_ready: true,
            balance_ready: true,
            strategy_type: StrategyType::OrdinaryGrid,
            market: StrategyMarket::FuturesUsdM,
            mode: StrategyMode::FuturesNeutral,
            runtime_phase: StrategyRuntimePhase::Draft,
            runtime_controls: RuntimeControls::default(),
            draft_revision: revision.clone(),
            active_revision: Some(revision),
            runtime: StrategyRuntime::default(),
            tags: Vec::new(),
            notes: String::new(),
            archived_at: None,
        };

        let runtime = rebuild_runtime(&strategy, None, "strategy_started");

        assert_eq!(runtime.orders.len(), 2);
        assert_eq!(runtime.orders[0].side, "Buy");
        assert_eq!(runtime.orders[1].side, "Sell");
    }

    #[test]
    fn resume_rejects_paused_strategy_with_positions_after_revision_change() {
        let db = SharedDb::ephemeral().expect("db");
        let service = StrategyService::new(db.clone());
        let now = Utc::now();
        db.upsert_membership_record(
            "trader@example.com",
            &MembershipRecord {
                activated_at: Some(now),
                active_until: Some(now + chrono::Duration::days(30)),
                grace_until: Some(now + chrono::Duration::days(32)),
                override_status: None,
            },
        )
        .expect("membership");
        db.upsert_exchange_account(&UserExchangeAccountRecord {
            user_email: "trader@example.com".to_string(),
            exchange: "binance".to_string(),
            account_label: "Binance".to_string(),
            market_scope: "spot".to_string(),
            is_active: true,
            checked_at: Some(now),
            metadata: serde_json::json!({
                "connection_status": "connected",
                "validation": {
                    "api_connectivity_ok": true,
                    "timestamp_in_sync": true,
                    "permissions_ok": true,
                    "hedge_mode_ok": true,
                    "withdrawals_disabled": true,
                    "can_read_spot": true,
                    "market_access_ok": true
                }
            }),
        })
        .expect("account");
        db.replace_exchange_symbols(
            "trader@example.com",
            "binance",
            &[shared_db::UserExchangeSymbolRecord {
                user_email: "trader@example.com".to_string(),
                exchange: "binance".to_string(),
                market: "spot".to_string(),
                symbol: "BTCUSDT".to_string(),
                status: "TRADING".to_string(),
                base_asset: "BTC".to_string(),
                quote_asset: "USDT".to_string(),
                price_precision: 2,
                quantity_precision: 4,
                min_quantity: "0.001".to_string(),
                min_notional: "5".to_string(),
                keywords: vec!["btcusdt".to_string()],
                metadata: serde_json::json!({}),
                synced_at: now,
            }],
        )
        .expect("symbols");
        db.insert_exchange_wallet_snapshot(&shared_db::ExchangeWalletSnapshotRecord {
            user_email: "trader@example.com".to_string(),
            exchange: "binance".to_string(),
            wallet_type: "spot".to_string(),
            balances: serde_json::json!({ "USDT": "1000", "BTC": "5" }),
            captured_at: now,
        })
        .expect("wallet");

        let strategy = service
            .create_strategy(
                "trader@example.com",
                SaveStrategyRequest {
                    name: "paused".to_string(),
                    symbol: "BTCUSDT".to_string(),
                    market: StrategyMarket::Spot,
                    mode: StrategyMode::SpotBuyOnly,
                    strategy_type: Some(StrategyType::OrdinaryGrid),
                    generation: GridGeneration::Custom,
                    levels: vec![SaveGridLevelRequest {
                        entry_price: "100".to_string(),
                        quantity: "1".to_string(),
                        take_profit_bps: 100,
                        trailing_bps: None,
                    }],
                    amount_mode: StrategyAmountMode::Quote,
                    futures_margin_mode: None,
                    leverage: None,
                    membership_ready: true,
                    exchange_ready: true,
                    permissions_ready: true,
                    withdrawals_disabled: true,
                    hedge_mode_ready: true,
                    symbol_ready: true,
                    filters_ready: true,
                    margin_ready: true,
                    conflict_ready: true,
                    balance_ready: true,
                    overall_take_profit_bps: None,
                    overall_stop_loss_bps: None,
                    reference_price_source: ReferencePriceSource::Manual,
                    post_trigger_action: PostTriggerAction::Stop,
                    tags: Vec::new(),
                    notes: String::new(),
                    extra: BTreeMap::new(),
                },
            )
            .expect("strategy");

        let mut paused = strategy.clone();
        paused.status = StrategyStatus::Paused;
        paused.active_revision = Some(paused.draft_revision.clone());
        paused.runtime.positions = vec![StrategyRuntimePosition {
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            quantity: "1".parse().expect("decimal"),
            average_entry_price: "100".parse().expect("decimal"),
        }];
        db.update_strategy(&paused).expect("store paused strategy");

        let _edited = service
            .update_strategy(
                "trader@example.com",
                &strategy.id,
                SaveStrategyRequest {
                    name: "paused edited".to_string(),
                    symbol: "BTCUSDT".to_string(),
                    market: StrategyMarket::Spot,
                    mode: StrategyMode::SpotBuyOnly,
                    strategy_type: Some(StrategyType::OrdinaryGrid),
                    generation: GridGeneration::Custom,
                    levels: vec![SaveGridLevelRequest {
                        entry_price: "95".to_string(),
                        quantity: "1".to_string(),
                        take_profit_bps: 100,
                        trailing_bps: None,
                    }],
                    amount_mode: StrategyAmountMode::Quote,
                    futures_margin_mode: None,
                    leverage: None,
                    membership_ready: true,
                    exchange_ready: true,
                    permissions_ready: true,
                    withdrawals_disabled: true,
                    hedge_mode_ready: true,
                    symbol_ready: true,
                    filters_ready: true,
                    margin_ready: true,
                    conflict_ready: true,
                    balance_ready: true,
                    overall_take_profit_bps: None,
                    overall_stop_loss_bps: None,
                    reference_price_source: ReferencePriceSource::Manual,
                    post_trigger_action: PostTriggerAction::Stop,
                    tags: Vec::new(),
                    notes: String::new(),
                    extra: BTreeMap::new(),
                },
            )
            .expect("edit paused strategy");

        let resumed = service.resume_strategy("trader@example.com", &strategy.id);

        match resumed {
            Err(StrategyError { message, .. }) => {
                assert_eq!(
                    message,
                    "resume requires reconciling paused positions before applying a revised grid"
                );
            }
            Ok(_) => panic!("resume should have been rejected"),
        }
    }

    #[test]
    fn resume_preserves_fill_history_and_rebuilds_exit_order_for_open_position() {
        let db = SharedDb::ephemeral().expect("db");
        let service = StrategyService::new(db.clone());
        let now = Utc::now();
        db.upsert_membership_record(
            "trader@example.com",
            &MembershipRecord {
                activated_at: Some(now),
                active_until: Some(now + chrono::Duration::days(30)),
                grace_until: Some(now + chrono::Duration::days(32)),
                override_status: None,
            },
        )
        .expect("membership");
        db.upsert_exchange_account(&UserExchangeAccountRecord {
            user_email: "trader@example.com".to_string(),
            exchange: "binance".to_string(),
            account_label: "Binance".to_string(),
            market_scope: "spot".to_string(),
            is_active: true,
            checked_at: Some(now),
            metadata: serde_json::json!({
                "connection_status": "connected",
                "validation": {
                    "api_connectivity_ok": true,
                    "timestamp_in_sync": true,
                    "permissions_ok": true,
                    "hedge_mode_ok": true,
                    "withdrawals_disabled": true,
                    "can_read_spot": true,
                    "market_access_ok": true
                }
            }),
        })
        .expect("account");
        db.replace_exchange_symbols(
            "trader@example.com",
            "binance",
            &[shared_db::UserExchangeSymbolRecord {
                user_email: "trader@example.com".to_string(),
                exchange: "binance".to_string(),
                market: "spot".to_string(),
                symbol: "BTCUSDT".to_string(),
                status: "TRADING".to_string(),
                base_asset: "BTC".to_string(),
                quote_asset: "USDT".to_string(),
                price_precision: 2,
                quantity_precision: 4,
                min_quantity: "0.001".to_string(),
                min_notional: "5".to_string(),
                keywords: vec!["btcusdt".to_string()],
                metadata: serde_json::json!({}),
                synced_at: now,
            }],
        )
        .expect("symbols");
        db.insert_exchange_wallet_snapshot(&shared_db::ExchangeWalletSnapshotRecord {
            user_email: "trader@example.com".to_string(),
            exchange: "binance".to_string(),
            wallet_type: "spot".to_string(),
            balances: serde_json::json!({ "USDT": "1000", "BTC": "5" }),
            captured_at: now,
        })
        .expect("wallet");

        let strategy = service
            .create_strategy(
                "trader@example.com",
                SaveStrategyRequest {
                    name: "paused".to_string(),
                    symbol: "BTCUSDT".to_string(),
                    market: StrategyMarket::Spot,
                    mode: StrategyMode::SpotBuyOnly,
                    strategy_type: Some(StrategyType::OrdinaryGrid),
                    generation: GridGeneration::Custom,
                    levels: vec![SaveGridLevelRequest {
                        entry_price: "100".to_string(),
                        quantity: "1".to_string(),
                        take_profit_bps: 100,
                        trailing_bps: None,
                    }],
                    amount_mode: StrategyAmountMode::Quote,
                    futures_margin_mode: None,
                    leverage: None,
                    membership_ready: true,
                    exchange_ready: true,
                    permissions_ready: true,
                    withdrawals_disabled: true,
                    hedge_mode_ready: true,
                    symbol_ready: true,
                    filters_ready: true,
                    margin_ready: true,
                    conflict_ready: true,
                    balance_ready: true,
                    overall_take_profit_bps: None,
                    overall_stop_loss_bps: None,
                    reference_price_source: ReferencePriceSource::Manual,
                    post_trigger_action: PostTriggerAction::Stop,
                    tags: Vec::new(),
                    notes: String::new(),
                    extra: BTreeMap::new(),
                },
            )
            .expect("strategy");

        let mut paused = strategy.clone();
        paused.status = StrategyStatus::Paused;
        paused.active_revision = Some(paused.draft_revision.clone());
        paused.runtime.positions = vec![StrategyRuntimePosition {
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            quantity: "1".parse().expect("decimal"),
            average_entry_price: "100".parse().expect("decimal"),
        }];
        paused.runtime.orders.clear();
        paused.runtime.fills.push(StrategyRuntimeFill {
            fill_id: "fill-1".to_string(),
            order_id: Some(format!("{}-order-0", paused.id)),
            level_index: Some(0),
            fill_type: "Entry".to_string(),
            price: "100".parse().expect("decimal"),
            quantity: "1".parse().expect("decimal"),
            realized_pnl: None,
            fee_amount: Some("0.1".parse().expect("decimal")),
            fee_asset: Some("USDT".to_string()),
        });
        db.update_strategy(&paused).expect("store paused strategy");

        let resumed = service
            .resume_strategy("trader@example.com", &strategy.id)
            .expect("resume strategy")
            .strategy;

        assert_eq!(resumed.runtime.positions.len(), 1);
        assert_eq!(resumed.runtime.fills.len(), 1);
        assert!(
            resumed
                .runtime
                .orders
                .iter()
                .any(|order| order.order_id.ends_with("-tp-0")
                    || order.order_id.ends_with("-trail-0"))
        );
    }

    #[test]
    fn resume_uses_current_position_anchor_instead_of_grid_midpoint_for_pending_entries() {
        let db = SharedDb::ephemeral().expect("db");
        let service = StrategyService::new(db.clone());
        let now = Utc::now();
        db.upsert_membership_record(
            "trader@example.com",
            &MembershipRecord {
                activated_at: Some(now),
                active_until: Some(now + chrono::Duration::days(30)),
                grace_until: Some(now + chrono::Duration::days(32)),
                override_status: None,
            },
        )
        .expect("membership");
        db.upsert_exchange_account(&UserExchangeAccountRecord {
            user_email: "trader@example.com".to_string(),
            exchange: "binance".to_string(),
            account_label: "Binance".to_string(),
            market_scope: "spot".to_string(),
            is_active: true,
            checked_at: Some(now),
            metadata: serde_json::json!({
                "connection_status": "connected",
                "validation": {
                    "api_connectivity_ok": true,
                    "timestamp_in_sync": true,
                    "permissions_ok": true,
                    "hedge_mode_ok": true,
                    "withdrawals_disabled": true,
                    "can_read_spot": true,
                    "market_access_ok": true
                }
            }),
        })
        .expect("account");
        db.replace_exchange_symbols(
            "trader@example.com",
            "binance",
            &[shared_db::UserExchangeSymbolRecord {
                user_email: "trader@example.com".to_string(),
                exchange: "binance".to_string(),
                market: "spot".to_string(),
                symbol: "BTCUSDT".to_string(),
                status: "TRADING".to_string(),
                base_asset: "BTC".to_string(),
                quote_asset: "USDT".to_string(),
                price_precision: 2,
                quantity_precision: 4,
                min_quantity: "0.001".to_string(),
                min_notional: "5".to_string(),
                keywords: vec!["btcusdt".to_string()],
                metadata: serde_json::json!({}),
                synced_at: now,
            }],
        )
        .expect("symbols");
        db.insert_exchange_wallet_snapshot(&shared_db::ExchangeWalletSnapshotRecord {
            user_email: "trader@example.com".to_string(),
            exchange: "binance".to_string(),
            wallet_type: "spot".to_string(),
            balances: serde_json::json!({ "USDT": "1000", "BTC": "5" }),
            captured_at: now,
        })
        .expect("wallet");

        let strategy = service
            .create_strategy(
                "trader@example.com",
                SaveStrategyRequest {
                    name: "paused".to_string(),
                    symbol: "BTCUSDT".to_string(),
                    market: StrategyMarket::Spot,
                    mode: StrategyMode::SpotBuyOnly,
                    strategy_type: Some(StrategyType::OrdinaryGrid),
                    generation: GridGeneration::Custom,
                    levels: vec![
                        SaveGridLevelRequest {
                            entry_price: "110".to_string(),
                            quantity: "1".to_string(),
                            take_profit_bps: 100,
                            trailing_bps: None,
                        },
                        SaveGridLevelRequest {
                            entry_price: "100".to_string(),
                            quantity: "1".to_string(),
                            take_profit_bps: 100,
                            trailing_bps: None,
                        },
                    ],
                    amount_mode: StrategyAmountMode::Quote,
                    futures_margin_mode: None,
                    leverage: None,
                    membership_ready: true,
                    exchange_ready: true,
                    permissions_ready: true,
                    withdrawals_disabled: true,
                    hedge_mode_ready: true,
                    symbol_ready: true,
                    filters_ready: true,
                    margin_ready: true,
                    conflict_ready: true,
                    balance_ready: true,
                    overall_take_profit_bps: None,
                    overall_stop_loss_bps: None,
                    reference_price_source: ReferencePriceSource::Manual,
                    post_trigger_action: PostTriggerAction::Stop,
                    tags: Vec::new(),
                    notes: String::new(),
                    extra: BTreeMap::new(),
                },
            )
            .expect("strategy");

        let mut paused = strategy.clone();
        paused.status = StrategyStatus::Paused;
        paused.active_revision = Some(paused.draft_revision.clone());
        paused.runtime.positions = vec![StrategyRuntimePosition {
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotBuyOnly,
            quantity: "1".parse().expect("decimal"),
            average_entry_price: "140".parse().expect("decimal"),
        }];
        paused.runtime.orders.clear();
        paused.runtime.fills.push(StrategyRuntimeFill {
            fill_id: "fill-1".to_string(),
            order_id: Some(format!("{}-order-0", paused.id)),
            level_index: Some(0),
            fill_type: "Entry".to_string(),
            price: "140".parse().expect("decimal"),
            quantity: "1".parse().expect("decimal"),
            realized_pnl: None,
            fee_amount: Some("0.1".parse().expect("decimal")),
            fee_asset: Some("USDT".to_string()),
        });
        db.update_strategy(&paused).expect("store paused strategy");

        let resumed = service
            .resume_strategy("trader@example.com", &strategy.id)
            .expect("resume strategy")
            .strategy;

        assert!(resumed
            .runtime
            .orders
            .iter()
            .any(|order| order.order_id.ends_with("-order-1") && order.side == "Buy"));
        assert!(!resumed
            .runtime
            .orders
            .iter()
            .any(|order| order.order_id.ends_with("-order-1") && order.side == "Sell"));
    }

    #[test]
    fn create_strategy_quantizes_levels_to_exchange_filters_when_metadata_exists() {
        let db = SharedDb::ephemeral().expect("db");
        let service = StrategyService::new(db.clone());
        let now = Utc::now();
        db.replace_exchange_symbols(
            "trader@example.com",
            "binance",
            &[shared_db::UserExchangeSymbolRecord {
                user_email: "trader@example.com".to_string(),
                exchange: "binance".to_string(),
                market: "spot".to_string(),
                symbol: "BTCUSDT".to_string(),
                status: "TRADING".to_string(),
                base_asset: "BTC".to_string(),
                quote_asset: "USDT".to_string(),
                price_precision: 2,
                quantity_precision: 3,
                min_quantity: "0.001".to_string(),
                min_notional: "5".to_string(),
                keywords: vec!["btcusdt".to_string()],
                metadata: serde_json::json!({
                    "filters": {
                        "price_tick_size": "0.05",
                        "quantity_step_size": "0.001"
                    }
                }),
                synced_at: now,
            }],
        )
        .expect("symbols");

        let strategy = service
            .create_strategy(
                "trader@example.com",
                SaveStrategyRequest {
                    name: "quantized".to_string(),
                    symbol: "BTCUSDT".to_string(),
                    market: StrategyMarket::Spot,
                    mode: StrategyMode::SpotBuyOnly,
                    strategy_type: Some(StrategyType::OrdinaryGrid),
                    generation: GridGeneration::Custom,
                    levels: vec![SaveGridLevelRequest {
                        entry_price: "100.13".to_string(),
                        quantity: "0.1237".to_string(),
                        take_profit_bps: 100,
                        trailing_bps: None,
                    }],
                    amount_mode: StrategyAmountMode::Quote,
                    futures_margin_mode: None,
                    leverage: None,
                    membership_ready: true,
                    exchange_ready: true,
                    permissions_ready: true,
                    withdrawals_disabled: true,
                    hedge_mode_ready: true,
                    symbol_ready: true,
                    filters_ready: true,
                    margin_ready: true,
                    conflict_ready: true,
                    balance_ready: true,
                    overall_take_profit_bps: None,
                    overall_stop_loss_bps: None,
                    reference_price_source: ReferencePriceSource::Manual,
                    post_trigger_action: PostTriggerAction::Stop,
                    tags: Vec::new(),
                    notes: String::new(),
                    extra: BTreeMap::new(),
                },
            )
            .expect("strategy");

        assert_eq!(
            strategy.draft_revision.levels[0].entry_price,
            "100.10".parse().expect("decimal")
        );
        assert_eq!(
            strategy.draft_revision.levels[0].quantity,
            "0.123".parse().expect("decimal")
        );
        assert_eq!(
            strategy.draft_revision.reference_price,
            Some("100.10".parse().expect("decimal"))
        );

        let mut explicit_extra = BTreeMap::new();
        explicit_extra.insert(
            "reference_price".to_string(),
            Value::String("101.23".to_string()),
        );
        let strategy_with_explicit_reference = service
            .create_strategy(
                "trader@example.com",
                SaveStrategyRequest {
                    name: "explicit-ref".to_string(),
                    symbol: "BTCUSDT".to_string(),
                    market: StrategyMarket::Spot,
                    mode: StrategyMode::SpotBuyOnly,
                    strategy_type: Some(StrategyType::OrdinaryGrid),
                    generation: GridGeneration::Custom,
                    levels: vec![SaveGridLevelRequest {
                        entry_price: "100.13".to_string(),
                        quantity: "0.1237".to_string(),
                        take_profit_bps: 100,
                        trailing_bps: None,
                    }],
                    amount_mode: StrategyAmountMode::Quote,
                    futures_margin_mode: None,
                    leverage: None,
                    membership_ready: true,
                    exchange_ready: true,
                    permissions_ready: true,
                    withdrawals_disabled: true,
                    hedge_mode_ready: true,
                    symbol_ready: true,
                    filters_ready: true,
                    margin_ready: true,
                    conflict_ready: true,
                    balance_ready: true,
                    overall_take_profit_bps: None,
                    overall_stop_loss_bps: None,
                    reference_price_source: ReferencePriceSource::Manual,
                    post_trigger_action: PostTriggerAction::Stop,
                    tags: Vec::new(),
                    notes: String::new(),
                    extra: explicit_extra,
                },
            )
            .expect("strategy with explicit reference");
        assert_eq!(
            strategy_with_explicit_reference
                .draft_revision
                .reference_price,
            Some("101.23".parse().expect("decimal"))
        );
    }
}
