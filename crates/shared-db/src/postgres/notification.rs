use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};

use crate::SharedDbError;

#[derive(Debug, Clone, PartialEq)]
pub struct NotificationLogRecord {
    pub user_email: String,
    pub channel: String,
    pub template_key: Option<String>,
    pub title: String,
    pub body: String,
    pub status: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct NotificationRepository {
    pool: PgPool,
}

impl NotificationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_notification(&self, record: &NotificationLogRecord) -> Result<(), SharedDbError> {
        sqlx::query(
            "INSERT INTO notification_logs (
                user_email,
                channel,
                template_key,
                title,
                body,
                status,
                payload,
                created_at,
                delivered_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(&record.user_email)
        .bind(&record.channel)
        .bind(&record.template_key)
        .bind(&record.title)
        .bind(&record.body)
        .bind(&record.status)
        .bind(&record.payload)
        .bind(record.created_at)
        .bind(record.delivered_at)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn list_recent_for_user(
        &self,
        user_email: &str,
        limit: i64,
    ) -> Result<Vec<NotificationLogRecord>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT user_email, channel, template_key, title, body, status, payload, created_at, delivered_at
             FROM notification_logs
             WHERE lower(user_email) = lower($1)
             ORDER BY created_at DESC
             LIMIT $2",
        )
        .bind(user_email)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter()
            .map(|row| {
                Ok(NotificationLogRecord {
                    user_email: row.try_get("user_email").map_err(SharedDbError::from)?,
                    channel: row.try_get("channel").map_err(SharedDbError::from)?,
                    template_key: row.try_get("template_key").map_err(SharedDbError::from)?,
                    title: row.try_get("title").map_err(SharedDbError::from)?,
                    body: row.try_get("body").map_err(SharedDbError::from)?,
                    status: row.try_get("status").map_err(SharedDbError::from)?,
                    payload: row.try_get("payload").map_err(SharedDbError::from)?,
                    created_at: row.try_get("created_at").map_err(SharedDbError::from)?,
                    delivered_at: row.try_get("delivered_at").map_err(SharedDbError::from)?,
                })
            })
            .collect()
    }
}
