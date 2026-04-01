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

#[derive(Clone)]
pub struct ExchangeRepository {
    pool: PgPool,
}

impl ExchangeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert_account(&self, record: &UserExchangeAccountRecord) -> Result<(), SharedDbError> {
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

        row.map(|row| {
            Ok(UserExchangeAccountRecord {
                user_email: row.try_get("user_email").map_err(SharedDbError::from)?,
                exchange: row.try_get("exchange").map_err(SharedDbError::from)?,
                account_label: row.try_get("account_label").map_err(SharedDbError::from)?,
                market_scope: row.try_get("market_scope").map_err(SharedDbError::from)?,
                is_active: row.try_get("is_active").map_err(SharedDbError::from)?,
                checked_at: row.try_get("checked_at").map_err(SharedDbError::from)?,
                metadata: row.try_get("metadata").map_err(SharedDbError::from)?,
            })
        })
        .transpose()
    }
}
