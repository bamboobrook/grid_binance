use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Row, Transaction};

use shared_domain::strategy::{
    GridGeneration, PostTriggerAction, PreflightReport, Strategy, StrategyMarket,
    StrategyMode, StrategyRevision, StrategyRuntime, StrategyRuntimeEvent, StrategyRuntimeFill,
    StrategyRuntimeOrder, StrategyRuntimePosition,
};

use crate::{parse_strategy_status, strategy_status_to_str, SharedDbError, StoredStrategy};

#[derive(Debug, Clone, PartialEq)]
pub struct StrategyRevisionRecord {
    pub strategy_id: String,
    pub revision_kind: String,
    pub config: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrategyEventRecord {
    pub strategy_id: String,
    pub event_type: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct StrategyRepository {
    pool: PgPool,
}

impl StrategyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_strategies(&self, owner_email: &str) -> Result<Vec<Strategy>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT id,
                    owner_email,
                    name,
                    symbol,
                    budget,
                    grid_spacing_bps,
                    status,
                    source_template_id,
                    membership_ready,
                    exchange_ready,
                    permissions_ready,
                    withdrawals_disabled,
                    hedge_mode_ready,
                    symbol_ready,
                    filters_ready,
                    margin_ready,
                    conflict_ready,
                    balance_ready,
                    market,
                    mode,
                    archived_at
             FROM strategies
             WHERE lower(owner_email) = lower($1)
             ORDER BY sequence_id ASC",
        )
        .bind(owner_email)
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(strategy_from_row(&self.pool, row).await?);
        }
        Ok(items)
    }

    pub async fn find_strategy(
        &self,
        owner_email: &str,
        strategy_id: &str,
    ) -> Result<Option<Strategy>, SharedDbError> {
        let row = sqlx::query(
            "SELECT id,
                    owner_email,
                    name,
                    symbol,
                    budget,
                    grid_spacing_bps,
                    status,
                    source_template_id,
                    membership_ready,
                    exchange_ready,
                    permissions_ready,
                    withdrawals_disabled,
                    hedge_mode_ready,
                    symbol_ready,
                    filters_ready,
                    margin_ready,
                    conflict_ready,
                    balance_ready,
                    market,
                    mode,
                    archived_at
             FROM strategies
             WHERE lower(owner_email) = lower($1) AND id = $2",
        )
        .bind(owner_email)
        .bind(strategy_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        match row {
            Some(row) => Ok(Some(strategy_from_row(&self.pool, row).await?)),
            None => Ok(None),
        }
    }

    pub async fn insert_strategy(&self, strategy: &StoredStrategy) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        sqlx::query(
            "INSERT INTO strategies (
                id,
                sequence_id,
                owner_email,
                name,
                symbol,
                budget,
                grid_spacing_bps,
                status,
                source_template_id,
                membership_ready,
                exchange_ready,
                permissions_ready,
                withdrawals_disabled,
                hedge_mode_ready,
                symbol_ready,
                filters_ready,
                margin_ready,
                conflict_ready,
                balance_ready,
                market,
                mode,
                archived_at,
                created_at,
                updated_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, now(), now())",
        )
        .bind(&strategy.strategy.id)
        .bind(strategy.sequence_id as i64)
        .bind(&strategy.strategy.owner_email)
        .bind(&strategy.strategy.name)
        .bind(&strategy.strategy.symbol)
        .bind(&strategy.strategy.budget)
        .bind(strategy.strategy.grid_spacing_bps as i32)
        .bind(strategy_status_to_str(&strategy.strategy.status))
        .bind(&strategy.strategy.source_template_id)
        .bind(strategy.strategy.membership_ready)
        .bind(strategy.strategy.exchange_ready)
        .bind(strategy.strategy.permissions_ready)
        .bind(strategy.strategy.withdrawals_disabled)
        .bind(strategy.strategy.hedge_mode_ready)
        .bind(strategy.strategy.symbol_ready)
        .bind(strategy.strategy.filters_ready)
        .bind(strategy.strategy.margin_ready)
        .bind(strategy.strategy.conflict_ready)
        .bind(strategy.strategy.balance_ready)
        .bind(strategy_market_to_str(strategy.strategy.market))
        .bind(strategy_mode_to_str(strategy.strategy.mode))
        .bind(strategy.strategy.archived_at)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        replace_revisions_in(&mut transaction, &strategy.strategy).await?;
        replace_runtime_in(&mut transaction, &strategy.strategy).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn update_strategy(&self, strategy: &Strategy) -> Result<usize, SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        let updated = sqlx::query(
            "UPDATE strategies
             SET name = $3,
                 symbol = $4,
                 budget = $5,
                 grid_spacing_bps = $6,
                 status = $7,
                 source_template_id = $8,
                 membership_ready = $9,
                 exchange_ready = $10,
                 permissions_ready = $11,
                 withdrawals_disabled = $12,
                 hedge_mode_ready = $13,
                 symbol_ready = $14,
                 filters_ready = $15,
                 margin_ready = $16,
                 conflict_ready = $17,
                 balance_ready = $18,
                 market = $19,
                 mode = $20,
                 archived_at = $21,
                 updated_at = now()
             WHERE id = $1 AND lower(owner_email) = lower($2)",
        )
        .bind(&strategy.id)
        .bind(&strategy.owner_email)
        .bind(&strategy.name)
        .bind(&strategy.symbol)
        .bind(&strategy.budget)
        .bind(strategy.grid_spacing_bps as i32)
        .bind(strategy_status_to_str(&strategy.status))
        .bind(&strategy.source_template_id)
        .bind(strategy.membership_ready)
        .bind(strategy.exchange_ready)
        .bind(strategy.permissions_ready)
        .bind(strategy.withdrawals_disabled)
        .bind(strategy.hedge_mode_ready)
        .bind(strategy.symbol_ready)
        .bind(strategy.filters_ready)
        .bind(strategy.margin_ready)
        .bind(strategy.conflict_ready)
        .bind(strategy.balance_ready)
        .bind(strategy_market_to_str(strategy.market))
        .bind(strategy_mode_to_str(strategy.mode))
        .bind(strategy.archived_at)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        replace_revisions_in(&mut transaction, strategy).await?;
        replace_runtime_in(&mut transaction, strategy).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(updated.rows_affected() as usize)
    }

    pub async fn delete_strategy(
        &self,
        owner_email: &str,
        strategy_id: &str,
    ) -> Result<usize, SharedDbError> {
        let archived = sqlx::query(
            "UPDATE strategies
             SET status = 'Archived',
                 archived_at = now(),
                 updated_at = now()
             WHERE lower(owner_email) = lower($1)
               AND id = $2
               AND status <> 'Archived'",
        )
        .bind(owner_email)
        .bind(strategy_id)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(archived.rows_affected() as usize)
    }

    pub async fn insert_revision(&self, record: &StrategyRevisionRecord) -> Result<(), SharedDbError> {
        sqlx::query(
            "INSERT INTO strategy_revisions (strategy_id, revision_kind, config, created_at)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(&record.strategy_id)
        .bind(&record.revision_kind)
        .bind(&record.config)
        .bind(record.created_at)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn insert_event(&self, record: &StrategyEventRecord) -> Result<(), SharedDbError> {
        sqlx::query(
            "INSERT INTO strategy_events (strategy_id, event_type, payload, created_at)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(&record.strategy_id)
        .bind(&record.event_type)
        .bind(&record.payload)
        .bind(record.created_at)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }
}

async fn strategy_from_row(pool: &PgPool, row: sqlx::postgres::PgRow) -> Result<Strategy, SharedDbError> {
    let id: String = row.try_get("id").map_err(SharedDbError::from)?;
    let status: String = row.try_get("status").map_err(SharedDbError::from)?;
    let market: String = row.try_get("market").map_err(SharedDbError::from)?;
    let mode: String = row.try_get("mode").map_err(SharedDbError::from)?;
    let draft_revision = load_revision(pool, &id, "draft")
        .await?
        .unwrap_or_else(default_revision);
    let active_revision = load_revision(pool, &id, "active").await?;
    let runtime = load_runtime(pool, &id, parse_strategy_market(&market)?, parse_strategy_mode(&mode)?).await?;

    Ok(Strategy {
        id,
        owner_email: row.try_get("owner_email").map_err(SharedDbError::from)?,
        name: row.try_get("name").map_err(SharedDbError::from)?,
        symbol: row.try_get("symbol").map_err(SharedDbError::from)?,
        budget: row.try_get("budget").map_err(SharedDbError::from)?,
        grid_spacing_bps: row.try_get::<i32, _>("grid_spacing_bps").map_err(SharedDbError::from)? as u32,
        status: parse_strategy_status(&status)?,
        source_template_id: row.try_get("source_template_id").map_err(SharedDbError::from)?,
        membership_ready: row.try_get("membership_ready").map_err(SharedDbError::from)?,
        exchange_ready: row.try_get("exchange_ready").map_err(SharedDbError::from)?,
        permissions_ready: row.try_get("permissions_ready").map_err(SharedDbError::from)?,
        withdrawals_disabled: row.try_get("withdrawals_disabled").map_err(SharedDbError::from)?,
        hedge_mode_ready: row.try_get("hedge_mode_ready").map_err(SharedDbError::from)?,
        symbol_ready: row.try_get("symbol_ready").map_err(SharedDbError::from)?,
        filters_ready: row.try_get("filters_ready").map_err(SharedDbError::from)?,
        margin_ready: row.try_get("margin_ready").map_err(SharedDbError::from)?,
        conflict_ready: row.try_get("conflict_ready").map_err(SharedDbError::from)?,
        balance_ready: row.try_get("balance_ready").map_err(SharedDbError::from)?,
        market: parse_strategy_market(&market)?,
        mode: parse_strategy_mode(&mode)?,
        draft_revision,
        active_revision,
        runtime,
        archived_at: row.try_get("archived_at").map_err(SharedDbError::from)?,
    })
}

async fn load_revision(
    pool: &PgPool,
    strategy_id: &str,
    revision_kind: &str,
) -> Result<Option<StrategyRevision>, SharedDbError> {
    let row = sqlx::query(
        "SELECT config
         FROM strategy_revisions
         WHERE strategy_id = $1 AND revision_kind = $2
         ORDER BY revision_id DESC
         LIMIT 1",
    )
    .bind(strategy_id)
    .bind(revision_kind)
    .fetch_optional(pool)
    .await
    .map_err(SharedDbError::from)?;

    row.map(|row| {
        let config: Value = row.try_get("config").map_err(SharedDbError::from)?;
        serde_json::from_value(config).map_err(|error| SharedDbError::new(error.to_string()))
    })
    .transpose()
}

async fn replace_revisions_in(
    transaction: &mut Transaction<'_, Postgres>,
    strategy: &Strategy,
) -> Result<(), SharedDbError> {
    sqlx::query("DELETE FROM strategy_grid_levels WHERE strategy_id = $1")
        .bind(&strategy.id)
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    sqlx::query("DELETE FROM strategy_revisions WHERE strategy_id = $1")
        .bind(&strategy.id)
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;

    let draft_revision_id = insert_revision_with_levels_in(
        transaction,
        &strategy.id,
        "draft",
        &strategy.draft_revision,
    )
    .await?;
    if let Some(active) = &strategy.active_revision {
        let _ = insert_revision_with_levels_in(transaction, &strategy.id, "active", active).await?;
    }

    if draft_revision_id == 0 {
        return Err(SharedDbError::new("failed to persist draft revision"));
    }

    Ok(())
}

async fn insert_revision_with_levels_in(
    transaction: &mut Transaction<'_, Postgres>,
    strategy_id: &str,
    revision_kind: &str,
    revision: &StrategyRevision,
) -> Result<i64, SharedDbError> {
    let row = sqlx::query(
        "INSERT INTO strategy_revisions (strategy_id, revision_kind, config, created_at)
         VALUES ($1, $2, $3, now())
         RETURNING revision_id",
    )
    .bind(strategy_id)
    .bind(revision_kind)
    .bind(serde_json::to_value(revision).map_err(|error| SharedDbError::new(error.to_string()))?)
    .fetch_one(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    let revision_id: i64 = row.try_get("revision_id").map_err(SharedDbError::from)?;

    for level in &revision.levels {
        sqlx::query(
            "INSERT INTO strategy_grid_levels (
                strategy_id,
                revision_id,
                level_index,
                entry_price,
                quantity,
                take_profit_bps,
                take_profit_price,
                trailing_bps,
                created_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now())",
        )
        .bind(strategy_id)
        .bind(revision_id)
        .bind(level.level_index as i32)
        .bind(level.entry_price.to_string())
        .bind(level.quantity.to_string())
        .bind(level.take_profit_bps as i32)
        .bind(take_profit_price(level))
        .bind(level.trailing_bps.map(|value| value as i32))
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    }

    Ok(revision_id)
}

async fn replace_runtime_in(
    transaction: &mut Transaction<'_, Postgres>,
    strategy: &Strategy,
) -> Result<(), SharedDbError> {
    sqlx::query("DELETE FROM strategy_runtime_positions WHERE strategy_id = $1")
        .bind(&strategy.id)
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    sqlx::query("DELETE FROM strategy_fills WHERE strategy_id = $1")
        .bind(&strategy.id)
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    sqlx::query("DELETE FROM strategy_orders WHERE strategy_id = $1")
        .bind(&strategy.id)
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    sqlx::query("DELETE FROM strategy_events WHERE strategy_id = $1")
        .bind(&strategy.id)
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;

    for position in &strategy.runtime.positions {
        sqlx::query(
            "INSERT INTO strategy_runtime_positions (
                strategy_id,
                market_type,
                direction,
                exposure_side,
                quantity,
                average_entry_price,
                updated_at
             ) VALUES ($1, $2, $3, $4, $5, $6, now())",
        )
        .bind(&strategy.id)
        .bind(strategy_market_to_str(position.market))
        .bind(strategy_mode_to_str(position.mode))
        .bind(exposure_side_for_mode(position.mode))
        .bind(position.quantity.to_string())
        .bind(position.average_entry_price.to_string())
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    }

    for order in &strategy.runtime.orders {
        sqlx::query(
            "INSERT INTO strategy_orders (
                order_id,
                strategy_id,
                exchange_order_id,
                side,
                order_type,
                price,
                quantity,
                status,
                created_at,
                updated_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now(), now())",
        )
        .bind(&order.order_id)
        .bind(&strategy.id)
        .bind(order.level_index.map(|index| format!("grid:{index}")))
        .bind(&order.side)
        .bind(&order.order_type)
        .bind(order.price.map(|value| value.to_string()))
        .bind(order.quantity.to_string())
        .bind(&order.status)
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    }

    for fill in &strategy.runtime.fills {
        sqlx::query(
            "INSERT INTO strategy_fills (
                fill_id,
                strategy_id,
                order_id,
                price,
                quantity,
                fee_amount,
                fee_asset,
                realized_pnl,
                filled_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now())",
        )
        .bind(&fill.fill_id)
        .bind(&strategy.id)
        .bind(&fill.order_id)
        .bind(fill.price.to_string())
        .bind(fill.quantity.to_string())
        .bind(fill.fee_amount.map(|value| value.to_string()))
        .bind(&fill.fee_asset)
        .bind(fill.realized_pnl.map(|value| value.to_string()))
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    }

    for event in &strategy.runtime.events {
        sqlx::query(
            "INSERT INTO strategy_events (strategy_id, event_type, payload, created_at)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(&strategy.id)
        .bind(&event.event_type)
        .bind(json!({
            "detail": event.detail,
            "price": event.price.map(|value| value.to_string()),
        }))
        .bind(event.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    }

    sqlx::query(
        "INSERT INTO strategy_events (strategy_id, event_type, payload, created_at)
         VALUES ($1, $2, $3, now())",
    )
    .bind(&strategy.id)
    .bind("runtime_snapshot")
    .bind(
        serde_json::to_value(&strategy.runtime)
            .map_err(|error| SharedDbError::new(error.to_string()))?,
    )
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;

    Ok(())
}

async fn load_runtime(
    pool: &PgPool,
    strategy_id: &str,
    _market: StrategyMarket,
    _mode: StrategyMode,
) -> Result<StrategyRuntime, SharedDbError> {
    let row = sqlx::query(
        "SELECT payload
         FROM strategy_events
         WHERE strategy_id = $1 AND event_type = 'runtime_snapshot'
         ORDER BY event_id DESC
         LIMIT 1",
    )
    .bind(strategy_id)
    .fetch_optional(pool)
    .await
    .map_err(SharedDbError::from)?;

    match row {
        Some(row) => {
            let payload: Value = row.try_get("payload").map_err(SharedDbError::from)?;
            serde_json::from_value(payload).map_err(|error| SharedDbError::new(error.to_string()))
        }
        None => Ok(StrategyRuntime {
            positions: Vec::<StrategyRuntimePosition>::new(),
            orders: Vec::<StrategyRuntimeOrder>::new(),
            fills: Vec::<StrategyRuntimeFill>::new(),
            events: Vec::<StrategyRuntimeEvent>::new(),
            last_preflight: None::<PreflightReport>,
        }),
    }
}

fn default_revision() -> StrategyRevision {
    StrategyRevision {
        revision_id: "draft-revision-0".to_string(),
        version: 0,
        generation: GridGeneration::Custom,
        levels: Vec::new(),
        overall_take_profit_bps: None,
        overall_stop_loss_bps: None,
        post_trigger_action: PostTriggerAction::Stop,
    }
}

fn parse_strategy_market(value: &str) -> Result<StrategyMarket, SharedDbError> {
    match value {
        "Spot" => Ok(StrategyMarket::Spot),
        "FuturesUsdM" => Ok(StrategyMarket::FuturesUsdM),
        "FuturesCoinM" => Ok(StrategyMarket::FuturesCoinM),
        _ => Err(SharedDbError::new(format!("unknown strategy market: {value}"))),
    }
}

fn strategy_market_to_str(value: StrategyMarket) -> &'static str {
    match value {
        StrategyMarket::Spot => "Spot",
        StrategyMarket::FuturesUsdM => "FuturesUsdM",
        StrategyMarket::FuturesCoinM => "FuturesCoinM",
    }
}

fn parse_strategy_mode(value: &str) -> Result<StrategyMode, SharedDbError> {
    match value {
        "SpotClassic" => Ok(StrategyMode::SpotClassic),
        "SpotBuyOnly" => Ok(StrategyMode::SpotBuyOnly),
        "SpotSellOnly" => Ok(StrategyMode::SpotSellOnly),
        "FuturesLong" => Ok(StrategyMode::FuturesLong),
        "FuturesShort" => Ok(StrategyMode::FuturesShort),
        "FuturesNeutral" => Ok(StrategyMode::FuturesNeutral),
        _ => Err(SharedDbError::new(format!("unknown strategy mode: {value}"))),
    }
}

fn strategy_mode_to_str(value: StrategyMode) -> &'static str {
    match value {
        StrategyMode::SpotClassic => "SpotClassic",
        StrategyMode::SpotBuyOnly => "SpotBuyOnly",
        StrategyMode::SpotSellOnly => "SpotSellOnly",
        StrategyMode::FuturesLong => "FuturesLong",
        StrategyMode::FuturesShort => "FuturesShort",
        StrategyMode::FuturesNeutral => "FuturesNeutral",
    }
}

fn exposure_side_for_mode(value: StrategyMode) -> &'static str {
    match value {
        StrategyMode::SpotSellOnly | StrategyMode::FuturesShort => "Sell",
        _ => "Buy",
    }
}

fn take_profit_price(level: &shared_domain::strategy::GridLevel) -> String {
    let entry = level.entry_price.to_string().parse::<f64>().unwrap_or_default();
    let factor = 1.0 + (level.take_profit_bps as f64 / 10_000.0);
    format!("{:.8}", entry * factor)
}
