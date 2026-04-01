use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};

use crate::SharedDbError;

#[derive(Debug, Clone, PartialEq)]
pub struct UserExchangeAccountRecord {
    pub user_email: String,
    pub exchange: String,
    pub account_label: String,
    pub market_scope: String,
    pub is_active: bool,
    pub checked_at: Option<DateTime<Utc>>,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserExchangeCredentialRecord {
    pub user_email: String,
    pub exchange: String,
    pub api_key_masked: String,
    pub encrypted_secret: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserExchangeSymbolRecord {
    pub user_email: String,
    pub exchange: String,
    pub market: String,
    pub symbol: String,
    pub status: String,
    pub base_asset: String,
    pub quote_asset: String,
    pub price_precision: i32,
    pub quantity_precision: i32,
    pub min_quantity: String,
    pub min_notional: String,
    pub keywords: Vec<String>,
    pub metadata: Value,
    pub synced_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct ExchangeRepository {
    pool: PgPool,
}

impl ExchangeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert_account(
        &self,
        record: &UserExchangeAccountRecord,
    ) -> Result<(), SharedDbError> {
        sqlx::query(
            "INSERT INTO user_exchange_accounts (
                user_email,
                exchange,
                account_label,
                market_scope,
                is_active,
                checked_at,
                metadata,
                created_at,
                updated_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, now(), now())
             ON CONFLICT (user_email, exchange) DO UPDATE
             SET account_label = excluded.account_label,
                 market_scope = excluded.market_scope,
                 is_active = excluded.is_active,
                 checked_at = excluded.checked_at,
                 metadata = excluded.metadata,
                 updated_at = now()",
        )
        .bind(&record.user_email)
        .bind(&record.exchange)
        .bind(&record.account_label)
        .bind(&record.market_scope)
        .bind(record.is_active)
        .bind(record.checked_at)
        .bind(&record.metadata)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn upsert_credentials(
        &self,
        record: &UserExchangeCredentialRecord,
    ) -> Result<(), SharedDbError> {
        sqlx::query(
            "INSERT INTO user_exchange_credentials (
                user_email,
                exchange,
                api_key_masked,
                encrypted_secret,
                created_at,
                updated_at
             ) VALUES ($1, $2, $3, $4, now(), now())
             ON CONFLICT (user_email, exchange) DO UPDATE
             SET api_key_masked = excluded.api_key_masked,
                 encrypted_secret = excluded.encrypted_secret,
                 updated_at = now()",
        )
        .bind(&record.user_email)
        .bind(&record.exchange)
        .bind(&record.api_key_masked)
        .bind(&record.encrypted_secret)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn find_account(
        &self,
        user_email: &str,
        exchange: &str,
    ) -> Result<Option<UserExchangeAccountRecord>, SharedDbError> {
        let row = sqlx::query(
            "SELECT user_email, exchange, account_label, market_scope, is_active, checked_at, metadata
             FROM user_exchange_accounts
             WHERE lower(user_email) = lower($1) AND exchange = $2",
        )
        .bind(user_email)
        .bind(exchange)
        .fetch_optional(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        row.map(map_account_row).transpose()
    }

    pub async fn find_credentials(
        &self,
        user_email: &str,
        exchange: &str,
    ) -> Result<Option<UserExchangeCredentialRecord>, SharedDbError> {
        let row = sqlx::query(
            "SELECT user_email, exchange, api_key_masked, encrypted_secret
             FROM user_exchange_credentials
             WHERE lower(user_email) = lower($1) AND exchange = $2",
        )
        .bind(user_email)
        .bind(exchange)
        .fetch_optional(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        row.map(map_credential_row).transpose()
    }

    pub async fn replace_symbols(
        &self,
        user_email: &str,
        exchange: &str,
        records: &[UserExchangeSymbolRecord],
    ) -> Result<usize, SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        sqlx::query(
            "DELETE FROM user_exchange_symbol_metadata
             WHERE lower(user_email) = lower($1) AND exchange = $2",
        )
        .bind(user_email)
        .bind(exchange)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        for record in records {
            sqlx::query(
                "INSERT INTO user_exchange_symbol_metadata (
                    user_email,
                    exchange,
                    market,
                    symbol,
                    status,
                    base_asset,
                    quote_asset,
                    price_precision,
                    quantity_precision,
                    min_quantity,
                    min_notional,
                    keywords,
                    metadata,
                    synced_at,
                    created_at,
                    updated_at
                 ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, now(), now()
                 )",
            )
            .bind(&record.user_email)
            .bind(&record.exchange)
            .bind(&record.market)
            .bind(&record.symbol)
            .bind(&record.status)
            .bind(&record.base_asset)
            .bind(&record.quote_asset)
            .bind(record.price_precision)
            .bind(record.quantity_precision)
            .bind(&record.min_quantity)
            .bind(&record.min_notional)
            .bind(serde_json::to_value(&record.keywords).map_err(|error| {
                SharedDbError::new(format!("failed to serialize symbol keywords: {error}"))
            })?)
            .bind(&record.metadata)
            .bind(record.synced_at)
            .execute(&mut *transaction)
            .await
            .map_err(SharedDbError::from)?;
        }

        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(records.len())
    }

    pub async fn list_symbols(
        &self,
        user_email: &str,
        exchange: &str,
    ) -> Result<Vec<UserExchangeSymbolRecord>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT
                user_email,
                exchange,
                market,
                symbol,
                status,
                base_asset,
                quote_asset,
                price_precision,
                quantity_precision,
                min_quantity,
                min_notional,
                keywords,
                metadata,
                synced_at
             FROM user_exchange_symbol_metadata
             WHERE lower(user_email) = lower($1) AND exchange = $2
             ORDER BY market, symbol",
        )
        .bind(user_email)
        .bind(exchange)
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter().map(map_symbol_row).collect()
    }

    pub async fn list_active_accounts(
        &self,
        exchange: &str,
    ) -> Result<Vec<UserExchangeAccountRecord>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT user_email, exchange, account_label, market_scope, is_active, checked_at, metadata
             FROM user_exchange_accounts
             WHERE exchange = $1 AND is_active = TRUE
             ORDER BY lower(user_email)",
        )
        .bind(exchange)
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter().map(map_account_row).collect()
    }
}

fn map_account_row(row: sqlx::postgres::PgRow) -> Result<UserExchangeAccountRecord, SharedDbError> {
    Ok(UserExchangeAccountRecord {
        user_email: row.try_get("user_email").map_err(SharedDbError::from)?,
        exchange: row.try_get("exchange").map_err(SharedDbError::from)?,
        account_label: row.try_get("account_label").map_err(SharedDbError::from)?,
        market_scope: row.try_get("market_scope").map_err(SharedDbError::from)?,
        is_active: row.try_get("is_active").map_err(SharedDbError::from)?,
        checked_at: row.try_get("checked_at").map_err(SharedDbError::from)?,
        metadata: row.try_get("metadata").map_err(SharedDbError::from)?,
    })
}

fn map_credential_row(
    row: sqlx::postgres::PgRow,
) -> Result<UserExchangeCredentialRecord, SharedDbError> {
    Ok(UserExchangeCredentialRecord {
        user_email: row.try_get("user_email").map_err(SharedDbError::from)?,
        exchange: row.try_get("exchange").map_err(SharedDbError::from)?,
        api_key_masked: row.try_get("api_key_masked").map_err(SharedDbError::from)?,
        encrypted_secret: row
            .try_get("encrypted_secret")
            .map_err(SharedDbError::from)?,
    })
}

fn map_symbol_row(row: sqlx::postgres::PgRow) -> Result<UserExchangeSymbolRecord, SharedDbError> {
    let keywords_value: Value = row.try_get("keywords").map_err(SharedDbError::from)?;
    let keywords = serde_json::from_value(keywords_value).map_err(|error| {
        SharedDbError::new(format!(
            "failed to decode symbol keywords from database: {error}"
        ))
    })?;

    Ok(UserExchangeSymbolRecord {
        user_email: row.try_get("user_email").map_err(SharedDbError::from)?,
        exchange: row.try_get("exchange").map_err(SharedDbError::from)?,
        market: row.try_get("market").map_err(SharedDbError::from)?,
        symbol: row.try_get("symbol").map_err(SharedDbError::from)?,
        status: row.try_get("status").map_err(SharedDbError::from)?,
        base_asset: row.try_get("base_asset").map_err(SharedDbError::from)?,
        quote_asset: row.try_get("quote_asset").map_err(SharedDbError::from)?,
        price_precision: row
            .try_get("price_precision")
            .map_err(SharedDbError::from)?,
        quantity_precision: row
            .try_get("quantity_precision")
            .map_err(SharedDbError::from)?,
        min_quantity: row.try_get("min_quantity").map_err(SharedDbError::from)?,
        min_notional: row.try_get("min_notional").map_err(SharedDbError::from)?,
        keywords,
        metadata: row.try_get("metadata").map_err(SharedDbError::from)?,
        synced_at: row.try_get("synced_at").map_err(SharedDbError::from)?,
    })
}
