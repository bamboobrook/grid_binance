use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};

use shared_domain::strategy::{Strategy, StrategyStatus};

use crate::{
    parse_strategy_status, strategy_status_to_str, SharedDbError, StoredStrategy,
};

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
                    symbol_ready
             FROM strategies
             WHERE lower(owner_email) = lower($1)
             ORDER BY sequence_id ASC",
        )
        .bind(owner_email)
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter().map(strategy_from_row).collect()
    }

    pub async fn find_strategy(
        &self,
        owner_email: &str,
        strategy_id: &str,
    ) -> Result<Option<Strategy>, SharedDbError> {
        sqlx::query(
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
                    symbol_ready
             FROM strategies
             WHERE lower(owner_email) = lower($1) AND id = $2",
        )
        .bind(owner_email)
        .bind(strategy_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SharedDbError::from)?
        .map(strategy_from_row)
        .transpose()
    }

    pub async fn insert_strategy(&self, strategy: &StoredStrategy) -> Result<(), SharedDbError> {
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
                symbol_ready,
                created_at,
                updated_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, now(), now())",
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
        .bind(strategy.strategy.symbol_ready)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn update_strategy(&self, strategy: &Strategy) -> Result<usize, SharedDbError> {
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
                 symbol_ready = $11,
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
        .bind(strategy.symbol_ready)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(updated.rows_affected() as usize)
    }

    pub async fn delete_strategy(
        &self,
        owner_email: &str,
        strategy_id: &str,
    ) -> Result<usize, SharedDbError> {
        let deleted = sqlx::query(
            "DELETE FROM strategies
             WHERE lower(owner_email) = lower($1) AND id = $2",
        )
        .bind(owner_email)
        .bind(strategy_id)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(deleted.rows_affected() as usize)
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

fn strategy_from_row(row: sqlx::postgres::PgRow) -> Result<Strategy, SharedDbError> {
    let status: String = row.try_get("status").map_err(SharedDbError::from)?;

    Ok(Strategy {
        id: row.try_get("id").map_err(SharedDbError::from)?,
        owner_email: row.try_get("owner_email").map_err(SharedDbError::from)?,
        name: row.try_get("name").map_err(SharedDbError::from)?,
        symbol: row.try_get("symbol").map_err(SharedDbError::from)?,
        budget: row.try_get("budget").map_err(SharedDbError::from)?,
        grid_spacing_bps: row.try_get::<i32, _>("grid_spacing_bps").map_err(SharedDbError::from)? as u32,
        status: parse_strategy_status(&status)?,
        source_template_id: row.try_get("source_template_id").map_err(SharedDbError::from)?,
        membership_ready: row.try_get("membership_ready").map_err(SharedDbError::from)?,
        exchange_ready: row.try_get("exchange_ready").map_err(SharedDbError::from)?,
        symbol_ready: row.try_get("symbol_ready").map_err(SharedDbError::from)?,
    })
}

#[allow(dead_code)]
fn _status_for_compile(_: StrategyStatus) {}
