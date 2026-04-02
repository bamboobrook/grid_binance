use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::postgres::admin::{insert_audit_log_in, AuditLogRecord};
use crate::{default_token_expiry, SharedDbError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUserRecord {
    pub user_id: u64,
    pub email: String,
    pub password_hash: String,
    pub email_verified: bool,
    pub verification_code: Option<String>,
    pub reset_code: Option<String>,
    pub totp_secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUserDirectoryRecord {
    pub email: String,
    pub email_verified: bool,
    pub totp_enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdminUserRecord {
    pub email: String,
    pub role: String,
    pub totp_required: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TelegramBindingRecord {
    pub user_email: String,
    pub telegram_user_id: String,
    pub telegram_chat_id: String,
    pub bound_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct IdentityRepository {
    pool: PgPool,
}

impl IdentityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_auth_user(&self, email: &str) -> Result<Option<AuthUserRecord>, SharedDbError> {
        let row = sqlx::query(
            "SELECT u.user_id,
                    u.email,
                    u.password_hash,
                    (u.email_verified_at IS NOT NULL) AS email_verified,
                    evt.token AS verification_code,
                    prt.token AS reset_code,
                    utf.secret AS totp_secret
             FROM users u
             LEFT JOIN email_verification_tokens evt
               ON evt.user_id = u.user_id AND evt.consumed_at IS NULL
             LEFT JOIN password_reset_tokens prt
               ON prt.user_id = u.user_id AND prt.consumed_at IS NULL
             LEFT JOIN user_totp_factors utf
               ON utf.user_id = u.user_id AND utf.disabled_at IS NULL
             WHERE lower(u.email) = lower($1)",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        row.map(|row| {
            Ok(AuthUserRecord {
                user_id: row.try_get::<i64, _>("user_id").map_err(SharedDbError::from)? as u64,
                email: row.try_get("email").map_err(SharedDbError::from)?,
                password_hash: row.try_get("password_hash").map_err(SharedDbError::from)?,
                email_verified: row.try_get("email_verified").map_err(SharedDbError::from)?,
                verification_code: row.try_get("verification_code").map_err(SharedDbError::from)?,
                reset_code: row.try_get("reset_code").map_err(SharedDbError::from)?,
                totp_secret: row.try_get("totp_secret").map_err(SharedDbError::from)?,
            })
        })
        .transpose()
    }

    pub async fn list_auth_users(&self) -> Result<Vec<AuthUserDirectoryRecord>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT u.email,
                    (u.email_verified_at IS NOT NULL) AS email_verified,
                    (utf.secret IS NOT NULL) AS totp_enabled
             FROM users u
             LEFT JOIN user_totp_factors utf
               ON utf.user_id = u.user_id AND utf.disabled_at IS NULL
             ORDER BY lower(u.email) ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter()
            .map(|row| {
                Ok(AuthUserDirectoryRecord {
                    email: row.try_get("email").map_err(SharedDbError::from)?,
                    email_verified: row.try_get("email_verified").map_err(SharedDbError::from)?,
                    totp_enabled: row.try_get("totp_enabled").map_err(SharedDbError::from)?,
                })
            })
            .collect()
    }

    pub async fn insert_auth_user(&self, record: &AuthUserRecord) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        sqlx::query(
            "INSERT INTO users (user_id, email, password_hash, email_verified_at, created_at, updated_at)
             VALUES ($1, $2, $3, $4, now(), now())
             ON CONFLICT (email) DO UPDATE
             SET user_id = excluded.user_id,
                 password_hash = excluded.password_hash,
                 email_verified_at = excluded.email_verified_at,
                 updated_at = now()",
        )
        .bind(record.user_id as i64)
        .bind(&record.email)
        .bind(&record.password_hash)
        .bind(record.email_verified.then(Utc::now))
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        let user_id = record.user_id as i64;
        replace_email_verification_token(
            &mut transaction,
            user_id,
            record.verification_code.as_deref(),
        )
        .await?;
        replace_password_reset_token(&mut transaction, user_id, record.reset_code.as_deref()).await?;
        replace_totp_factor(&mut transaction, user_id, record.totp_secret.as_deref()).await?;
        transaction.commit().await.map_err(SharedDbError::from)
    }

    pub async fn update_auth_email_verification(
        &self,
        email: &str,
        email_verified: bool,
        verification_code: Option<&str>,
    ) -> Result<usize, SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        let user_id = lookup_user_id(&mut transaction, email).await?;
        let updated = sqlx::query(
            "UPDATE users
             SET email_verified_at = $2, updated_at = now()
             WHERE lower(email) = lower($1)",
        )
        .bind(email)
        .bind(email_verified.then(Utc::now))
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?
        .rows_affected() as usize;
        if let Some(user_id) = user_id {
            replace_email_verification_token(&mut transaction, user_id, verification_code).await?;
        }
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(updated)
    }

    pub async fn update_auth_email_verification_with_audit(
        &self,
        email: &str,
        email_verified: bool,
        verification_code: Option<&str>,
        audit: &AuditLogRecord,
    ) -> Result<usize, SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        let user_id = lookup_user_id(&mut transaction, email).await?;
        let updated = sqlx::query(
            "UPDATE users
             SET email_verified_at = $2, updated_at = now()
             WHERE lower(email) = lower($1)",
        )
        .bind(email)
        .bind(email_verified.then(Utc::now))
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?
        .rows_affected() as usize;
        if let Some(user_id) = user_id {
            replace_email_verification_token(&mut transaction, user_id, verification_code).await?;
            insert_audit_log_in(&mut transaction, audit).await?;
        }
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(updated)
    }

    pub async fn set_auth_reset_code(
        &self,
        email: &str,
        reset_code: Option<&str>,
    ) -> Result<usize, SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        let user_id = lookup_user_id(&mut transaction, email).await?;
        if let Some(user_id) = user_id {
            replace_password_reset_token(&mut transaction, user_id, reset_code).await?;
            transaction.commit().await.map_err(SharedDbError::from)?;
            Ok(1)
        } else {
            transaction.rollback().await.map_err(SharedDbError::from)?;
            Ok(0)
        }
    }

    pub async fn set_auth_reset_code_with_audit(
        &self,
        email: &str,
        reset_code: Option<&str>,
        audit: &AuditLogRecord,
    ) -> Result<usize, SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        let user_id = lookup_user_id(&mut transaction, email).await?;
        if let Some(user_id) = user_id {
            replace_password_reset_token(&mut transaction, user_id, reset_code).await?;
            insert_audit_log_in(&mut transaction, audit).await?;
            transaction.commit().await.map_err(SharedDbError::from)?;
            Ok(1)
        } else {
            transaction.rollback().await.map_err(SharedDbError::from)?;
            Ok(0)
        }
    }

    pub async fn update_auth_password(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<usize, SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        let user_id = lookup_user_id(&mut transaction, email).await?;
        let updated = sqlx::query(
            "UPDATE users
             SET password_hash = $2, updated_at = now()
             WHERE lower(email) = lower($1)",
        )
        .bind(email)
        .bind(password_hash)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?
        .rows_affected() as usize;
        if let Some(user_id) = user_id {
            replace_password_reset_token(&mut transaction, user_id, None).await?;
        }
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(updated)
    }

    pub async fn update_auth_password_with_audit(
        &self,
        email: &str,
        password_hash: &str,
        revoke_sessions: bool,
        audit: &AuditLogRecord,
    ) -> Result<usize, SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        let user_id = lookup_user_id(&mut transaction, email).await?;
        let updated = sqlx::query(
            "UPDATE users
             SET password_hash = $2, updated_at = now()
             WHERE lower(email) = lower($1)",
        )
        .bind(email)
        .bind(password_hash)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?
        .rows_affected() as usize;
        if let Some(user_id) = user_id {
            replace_password_reset_token(&mut transaction, user_id, None).await?;
            if revoke_sessions {
                revoke_auth_sessions(&mut transaction, user_id).await?;
            }
            insert_audit_log_in(&mut transaction, audit).await?;
        }
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(updated)
    }

    pub async fn set_auth_totp_secret(
        &self,
        email: &str,
        totp_secret: Option<&str>,
    ) -> Result<usize, SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        let user_id = lookup_user_id(&mut transaction, email).await?;
        if let Some(user_id) = user_id {
            replace_totp_factor(&mut transaction, user_id, totp_secret).await?;
            transaction.commit().await.map_err(SharedDbError::from)?;
            Ok(1)
        } else {
            transaction.rollback().await.map_err(SharedDbError::from)?;
            Ok(0)
        }
    }

    pub async fn set_auth_totp_secret_with_audit(
        &self,
        email: &str,
        totp_secret: Option<&str>,
        revoke_sessions: bool,
        audit: &AuditLogRecord,
    ) -> Result<usize, SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        let user_id = lookup_user_id(&mut transaction, email).await?;
        if let Some(user_id) = user_id {
            replace_totp_factor(&mut transaction, user_id, totp_secret).await?;
            if revoke_sessions {
                revoke_auth_sessions(&mut transaction, user_id).await?;
            }
            insert_audit_log_in(&mut transaction, audit).await?;
            transaction.commit().await.map_err(SharedDbError::from)?;
            Ok(1)
        } else {
            transaction.rollback().await.map_err(SharedDbError::from)?;
            Ok(0)
        }
    }

    pub async fn insert_auth_session(
        &self,
        session_token: &str,
        email: &str,
        sid: u64,
    ) -> Result<(), SharedDbError> {
        let inserted = sqlx::query(
            "INSERT INTO user_sessions (session_token, user_id, sid, created_at)
             SELECT $1, user_id, $2, now()
             FROM users
             WHERE lower(email) = lower($3)",
        )
        .bind(session_token)
        .bind(sid as i64)
        .bind(email)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        if inserted.rows_affected() == 0 {
            return Err(SharedDbError::new(format!("cannot create session for missing user {email}")));
        }
        Ok(())
    }

    pub async fn find_auth_session_email(
        &self,
        session_token: &str,
    ) -> Result<Option<String>, SharedDbError> {
        sqlx::query(
            "SELECT u.email
             FROM user_sessions s
             JOIN users u ON u.user_id = s.user_id
             WHERE s.session_token = $1 AND s.revoked_at IS NULL",
        )
        .bind(session_token)
        .fetch_optional(&self.pool)
        .await
        .map_err(SharedDbError::from)?
        .map(|row| row.try_get("email").map_err(SharedDbError::from))
        .transpose()
    }

    pub async fn upsert_admin_user(&self, email: &str, role: &str, totp_required: bool) -> Result<(), SharedDbError> {
        sqlx::query(
            "INSERT INTO admin_users (email, role, totp_required, created_at, updated_at)
             VALUES ($1, $2, $3, now(), now())
             ON CONFLICT (email) DO UPDATE
             SET role = excluded.role,
                 totp_required = excluded.totp_required,
                 updated_at = now()",
        )
        .bind(email)
        .bind(role)
        .bind(totp_required)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn find_telegram_binding(
        &self,
        email: &str,
    ) -> Result<Option<TelegramBindingRecord>, SharedDbError> {
        let row = sqlx::query(
            "SELECT u.email AS user_email, tb.telegram_user_id, tb.telegram_chat_id, tb.bound_at
             FROM telegram_bindings tb
             JOIN users u ON u.user_id = tb.user_id
             WHERE lower(u.email) = lower($1)",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        row.map(|row| {
            Ok(TelegramBindingRecord {
                user_email: row.try_get("user_email").map_err(SharedDbError::from)?,
                telegram_user_id: row.try_get("telegram_user_id").map_err(SharedDbError::from)?,
                telegram_chat_id: row.try_get("telegram_chat_id").map_err(SharedDbError::from)?,
                bound_at: row.try_get("bound_at").map_err(SharedDbError::from)?,
            })
        })
        .transpose()
    }

    pub async fn upsert_telegram_binding(
        &self,
        binding: &TelegramBindingRecord,
    ) -> Result<(), SharedDbError> {
        sqlx::query(
            "INSERT INTO telegram_bindings (
                user_id, telegram_user_id, telegram_chat_id, bound_at, updated_at
             )
             SELECT user_id, $2, $3, $4, now()
             FROM users
             WHERE lower(email) = lower($1)
             ON CONFLICT (user_id) DO UPDATE
             SET telegram_user_id = excluded.telegram_user_id,
                 telegram_chat_id = excluded.telegram_chat_id,
                 bound_at = excluded.bound_at,
                 updated_at = now()",
        )
        .bind(&binding.user_email)
        .bind(&binding.telegram_user_id)
        .bind(&binding.telegram_chat_id)
        .bind(binding.bound_at)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }
}

async fn lookup_user_id(
    transaction: &mut Transaction<'_, Postgres>,
    email: &str,
) -> Result<Option<i64>, SharedDbError> {
    sqlx::query("SELECT user_id FROM users WHERE lower(email) = lower($1)")
        .bind(email)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?
        .map(|row| row.try_get("user_id").map_err(SharedDbError::from))
        .transpose()
}

async fn revoke_auth_sessions(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: i64,
) -> Result<(), SharedDbError> {
    sqlx::query(
        "UPDATE user_sessions
         SET revoked_at = now()
         WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(())
}

async fn replace_email_verification_token(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: i64,
    token: Option<&str>,
) -> Result<(), SharedDbError> {
    sqlx::query("DELETE FROM email_verification_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    if let Some(token) = token {
        sqlx::query(
            "INSERT INTO email_verification_tokens (user_id, token, expires_at, created_at)
             VALUES ($1, $2, $3, now())",
        )
        .bind(user_id)
        .bind(token)
        .bind(default_token_expiry())
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    }
    Ok(())
}

async fn replace_password_reset_token(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: i64,
    token: Option<&str>,
) -> Result<(), SharedDbError> {
    sqlx::query("DELETE FROM password_reset_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    if let Some(token) = token {
        sqlx::query(
            "INSERT INTO password_reset_tokens (user_id, token, expires_at, created_at)
             VALUES ($1, $2, $3, now())",
        )
        .bind(user_id)
        .bind(token)
        .bind(default_token_expiry())
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    }
    Ok(())
}

async fn replace_totp_factor(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: i64,
    secret: Option<&str>,
) -> Result<(), SharedDbError> {
    sqlx::query("DELETE FROM user_totp_factors WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    if let Some(secret) = secret {
        sqlx::query(
            "INSERT INTO user_totp_factors (user_id, secret, enabled_at, created_at)
             VALUES ($1, $2, now(), now())",
        )
        .bind(user_id)
        .bind(secret)
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    }
    Ok(())
}
