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

#[derive(Debug, Clone, PartialEq)]
pub struct NotificationPreferencesRecord {
    pub take_profit: bool,
    pub stop_loss: bool,
    pub error: bool,
    pub daily_report: bool,
}

#[derive(Clone)]
pub struct NotificationRepository {
    pool: PgPool,
}

impl NotificationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_notification(
        &self,
        record: &NotificationLogRecord,
    ) -> Result<(), SharedDbError> {
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

    pub async fn read_notification_preferences(
        &self,
        user_email: &str,
    ) -> Result<NotificationPreferencesRecord, SharedDbError> {
        let row = sqlx::query(
            "SELECT np.take_profit, np.stop_loss, np.error, np.daily_report
             FROM notification_preferences np
             JOIN users u ON u.user_id = np.user_id
             WHERE lower(u.email) = lower($1)",
        )
        .bind(user_email)
        .fetch_optional(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        match row {
            Some(row) => Ok(NotificationPreferencesRecord {
                take_profit: row.try_get("take_profit").map_err(SharedDbError::from)?,
                stop_loss: row.try_get("stop_loss").map_err(SharedDbError::from)?,
                error: row.try_get("error").map_err(SharedDbError::from)?,
                daily_report: row.try_get("daily_report").map_err(SharedDbError::from)?,
            }),
            None => Ok(NotificationPreferencesRecord {
                take_profit: true,
                stop_loss: false,
                error: true,
                daily_report: false,
            }),
        }
    }

    pub async fn upsert_notification_preferences(
        &self,
        user_email: &str,
        prefs: &NotificationPreferencesRecord,
    ) -> Result<NotificationPreferencesRecord, SharedDbError> {
        let row = sqlx::query(
            "INSERT INTO notification_preferences (user_id, take_profit, stop_loss, error, daily_report)
             SELECT u.user_id, $2, $3, $4, $5
             FROM users u
             WHERE lower(u.email) = lower($1)
             ON CONFLICT (user_id)
             DO UPDATE SET take_profit = $2, stop_loss = $3, error = $4, daily_report = $5
             RETURNING take_profit, stop_loss, error, daily_report",
        )
        .bind(user_email)
        .bind(prefs.take_profit)
        .bind(prefs.stop_loss)
        .bind(prefs.error)
        .bind(prefs.daily_report)
        .fetch_one(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        Ok(NotificationPreferencesRecord {
            take_profit: row.try_get("take_profit").map_err(SharedDbError::from)?,
            stop_loss: row.try_get("stop_loss").map_err(SharedDbError::from)?,
            error: row.try_get("error").map_err(SharedDbError::from)?,
            daily_report: row.try_get("daily_report").map_err(SharedDbError::from)?,
        })
    }
}
