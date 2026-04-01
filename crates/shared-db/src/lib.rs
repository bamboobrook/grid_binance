use std::{
    error::Error,
    fmt::{Display, Formatter},
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use shared_chain::assignment::AddressAssignment;
use shared_domain::{
    membership::MembershipStatus,
    strategy::{Strategy, StrategyStatus, StrategyTemplate},
};

const INITIAL_CORE_MIGRATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../db/migrations/0001_initial_core.sql"
));

#[derive(Clone)]
pub struct SharedDb {
    inner: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone)]
pub struct SharedDbError {
    message: String,
}

#[derive(Debug, Clone)]
pub struct AuthUserRecord {
    pub user_id: u64,
    pub email: String,
    pub password_hash: String,
    pub email_verified: bool,
    pub verification_code: Option<String>,
    pub reset_code: Option<String>,
    pub totp_secret: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BillingOrderRecord {
    pub order_id: u64,
    pub email: String,
    pub chain: String,
    pub plan_code: String,
    pub amount: String,
    pub requested_at: DateTime<Utc>,
    pub assignment: AddressAssignment,
    pub paid_at: Option<DateTime<Utc>>,
    pub tx_hash: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MembershipRecord {
    pub activated_at: Option<DateTime<Utc>>,
    pub active_until: Option<DateTime<Utc>>,
    pub grace_until: Option<DateTime<Utc>>,
    pub override_status: Option<MembershipStatus>,
}

#[derive(Debug, Clone)]
pub struct StoredStrategy {
    pub sequence_id: u64,
    pub strategy: Strategy,
}

#[derive(Debug, Clone)]
pub struct StoredStrategyTemplate {
    pub sequence_id: u64,
    pub template: StrategyTemplate,
}

impl SharedDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SharedDbError> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }

        let connection = Connection::open(path)?;
        Self::from_connection(connection)
    }

    pub fn in_memory() -> Result<Self, SharedDbError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    pub fn bootstrap_label() -> &'static str {
        "sqlite"
    }

    pub fn next_sequence(&self, name: &str) -> Result<u64, SharedDbError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let next = next_sequence_value(&transaction, name)?;
        transaction.commit()?;
        Ok(next)
    }

    pub fn find_auth_user(&self, email: &str) -> Result<Option<AuthUserRecord>, SharedDbError> {
        let connection = self.lock_connection()?;
        connection
            .query_row(
                "SELECT user_id, email, password_hash, email_verified, verification_code, reset_code, totp_secret
                 FROM auth_users
                 WHERE email = ?1",
                params![email],
                |row| {
                    Ok(AuthUserRecord {
                        user_id: row.get::<_, i64>(0)? as u64,
                        email: row.get(1)?,
                        password_hash: row.get(2)?,
                        email_verified: row.get::<_, i64>(3)? != 0,
                        verification_code: row.get(4)?,
                        reset_code: row.get(5)?,
                        totp_secret: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(SharedDbError::from)
    }

    pub fn insert_auth_user(&self, record: &AuthUserRecord) -> Result<(), SharedDbError> {
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO auth_users (
                email, user_id, password_hash, email_verified, verification_code, reset_code, totp_secret
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.email,
                record.user_id as i64,
                record.password_hash,
                bool_to_i64(record.email_verified),
                record.verification_code,
                record.reset_code,
                record.totp_secret,
            ],
        )?;
        Ok(())
    }

    pub fn update_auth_email_verification(
        &self,
        email: &str,
        email_verified: bool,
        verification_code: Option<&str>,
    ) -> Result<usize, SharedDbError> {
        let connection = self.lock_connection()?;
        let updated = connection.execute(
            "UPDATE auth_users
             SET email_verified = ?2, verification_code = ?3
             WHERE email = ?1",
            params![email, bool_to_i64(email_verified), verification_code],
        )?;
        Ok(updated)
    }

    pub fn set_auth_reset_code(
        &self,
        email: &str,
        reset_code: Option<&str>,
    ) -> Result<usize, SharedDbError> {
        let connection = self.lock_connection()?;
        let updated = connection.execute(
            "UPDATE auth_users
             SET reset_code = ?2
             WHERE email = ?1",
            params![email, reset_code],
        )?;
        Ok(updated)
    }

    pub fn update_auth_password(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<usize, SharedDbError> {
        let connection = self.lock_connection()?;
        let updated = connection.execute(
            "UPDATE auth_users
             SET password_hash = ?2, reset_code = NULL
             WHERE email = ?1",
            params![email, password_hash],
        )?;
        Ok(updated)
    }

    pub fn set_auth_totp_secret(
        &self,
        email: &str,
        totp_secret: Option<&str>,
    ) -> Result<usize, SharedDbError> {
        let connection = self.lock_connection()?;
        let updated = connection.execute(
            "UPDATE auth_users
             SET totp_secret = ?2
             WHERE email = ?1",
            params![email, totp_secret],
        )?;
        Ok(updated)
    }

    pub fn insert_auth_session(
        &self,
        session_token: &str,
        email: &str,
        sid: u64,
    ) -> Result<(), SharedDbError> {
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO auth_sessions (session_token, email, sid)
             VALUES (?1, ?2, ?3)",
            params![session_token, email, sid as i64],
        )?;
        Ok(())
    }

    pub fn find_auth_session_email(
        &self,
        session_token: &str,
    ) -> Result<Option<String>, SharedDbError> {
        let connection = self.lock_connection()?;
        connection
            .query_row(
                "SELECT email
                 FROM auth_sessions
                 WHERE session_token = ?1",
                params![session_token],
                |row| row.get(0),
            )
            .optional()
            .map_err(SharedDbError::from)
    }

    pub fn list_billing_orders(&self) -> Result<Vec<BillingOrderRecord>, SharedDbError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT order_id, email, chain, plan_code, amount, requested_at, address, expires_at, paid_at, tx_hash
             FROM billing_orders
             ORDER BY order_id ASC",
        )?;
        let rows = statement.query_map([], |row| billing_order_from_row(row))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(SharedDbError::from)
    }

    pub fn insert_billing_order(&self, order: &BillingOrderRecord) -> Result<(), SharedDbError> {
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO billing_orders (
                order_id, email, chain, plan_code, amount, requested_at, address, expires_at, paid_at, tx_hash
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                order.order_id as i64,
                order.email,
                order.chain,
                order.plan_code,
                order.amount,
                order.requested_at.to_rfc3339(),
                order.assignment.address,
                order.assignment.expires_at.to_rfc3339(),
                option_datetime_to_string(order.paid_at),
                order.tx_hash,
            ],
        )?;
        Ok(())
    }

    pub fn record_seen_transfer(
        &self,
        tx_hash: &str,
        chain: &str,
        observed_at: DateTime<Utc>,
    ) -> Result<bool, SharedDbError> {
        let connection = self.lock_connection()?;
        let inserted = connection.execute(
            "INSERT OR IGNORE INTO seen_transfers (tx_hash, chain, observed_at)
             VALUES (?1, ?2, ?3)",
            params![tx_hash, chain, observed_at.to_rfc3339()],
        )?;
        Ok(inserted == 1)
    }

    pub fn find_membership_record(
        &self,
        email: &str,
    ) -> Result<Option<MembershipRecord>, SharedDbError> {
        let connection = self.lock_connection()?;
        connection
            .query_row(
                "SELECT activated_at, active_until, grace_until, override_status
                 FROM membership_records
                 WHERE email = ?1",
                params![email],
                |row| {
                    let activated_at_raw: Option<String> = row.get(0)?;
                    let active_until_raw: Option<String> = row.get(1)?;
                    let grace_until_raw: Option<String> = row.get(2)?;
                    let override_status_raw: Option<String> = row.get(3)?;
                    Ok(MembershipRecord {
                        activated_at: parse_optional_datetime(activated_at_raw)
                            .map_err(to_from_sql_error)?,
                        active_until: parse_optional_datetime(active_until_raw)
                            .map_err(to_from_sql_error)?,
                        grace_until: parse_optional_datetime(grace_until_raw)
                            .map_err(to_from_sql_error)?,
                        override_status: parse_optional_membership_status(override_status_raw)
                            .map_err(to_from_sql_error)?,
                    })
                },
            )
            .optional()
            .map_err(SharedDbError::from)
    }

    pub fn upsert_membership_record(
        &self,
        email: &str,
        record: &MembershipRecord,
    ) -> Result<(), SharedDbError> {
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO membership_records (
                email, activated_at, active_until, grace_until, override_status
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(email) DO UPDATE SET
                activated_at = excluded.activated_at,
                active_until = excluded.active_until,
                grace_until = excluded.grace_until,
                override_status = excluded.override_status",
            params![
                email,
                option_datetime_to_string(record.activated_at),
                option_datetime_to_string(record.active_until),
                option_datetime_to_string(record.grace_until),
                record
                    .override_status
                    .as_ref()
                    .map(membership_status_to_str),
            ],
        )?;
        Ok(())
    }

    pub fn update_membership_override(
        &self,
        email: &str,
        override_status: Option<&MembershipStatus>,
    ) -> Result<(), SharedDbError> {
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO membership_records (
                email, activated_at, active_until, grace_until, override_status
            )
            VALUES (?1, NULL, NULL, NULL, ?2)
            ON CONFLICT(email) DO UPDATE SET
                override_status = excluded.override_status",
            params![email, override_status.map(membership_status_to_str)],
        )?;
        Ok(())
    }

    pub fn apply_membership_payment(
        &self,
        order_id: u64,
        tx_hash: &str,
        paid_at: DateTime<Utc>,
        email: &str,
        active_until: DateTime<Utc>,
        grace_until: DateTime<Utc>,
    ) -> Result<(), SharedDbError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE billing_orders
             SET paid_at = ?2, tx_hash = ?3
             WHERE order_id = ?1",
            params![order_id as i64, paid_at.to_rfc3339(), tx_hash],
        )?;
        transaction.execute(
            "INSERT INTO membership_records (
                email, activated_at, active_until, grace_until, override_status
            ) VALUES (?1, ?2, ?3, ?4, NULL)
            ON CONFLICT(email) DO UPDATE SET
                activated_at = excluded.activated_at,
                active_until = excluded.active_until,
                grace_until = excluded.grace_until",
            params![
                email,
                paid_at.to_rfc3339(),
                active_until.to_rfc3339(),
                grace_until.to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn list_strategies(&self) -> Result<Vec<Strategy>, SharedDbError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, name, symbol, budget, grid_spacing_bps, status, source_template_id,
                    membership_ready, exchange_ready, symbol_ready
             FROM strategies
             ORDER BY sequence_id ASC",
        )?;
        let rows = statement.query_map([], |row| strategy_from_row(row))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(SharedDbError::from)
    }

    pub fn find_strategy(&self, strategy_id: &str) -> Result<Option<Strategy>, SharedDbError> {
        let connection = self.lock_connection()?;
        connection
            .query_row(
                "SELECT id, name, symbol, budget, grid_spacing_bps, status, source_template_id,
                        membership_ready, exchange_ready, symbol_ready
                 FROM strategies
                 WHERE id = ?1",
                params![strategy_id],
                |row| strategy_from_row(row),
            )
            .optional()
            .map_err(SharedDbError::from)
    }

    pub fn insert_strategy(&self, strategy: &StoredStrategy) -> Result<(), SharedDbError> {
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO strategies (
                id, sequence_id, name, symbol, budget, grid_spacing_bps, status, source_template_id,
                membership_ready, exchange_ready, symbol_ready
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                strategy.strategy.id,
                strategy.sequence_id as i64,
                strategy.strategy.name,
                strategy.strategy.symbol,
                strategy.strategy.budget,
                strategy.strategy.grid_spacing_bps as i64,
                strategy_status_to_str(&strategy.strategy.status),
                strategy.strategy.source_template_id,
                bool_to_i64(strategy.strategy.membership_ready),
                bool_to_i64(strategy.strategy.exchange_ready),
                bool_to_i64(strategy.strategy.symbol_ready),
            ],
        )?;
        Ok(())
    }

    pub fn update_strategy(&self, strategy: &Strategy) -> Result<usize, SharedDbError> {
        let connection = self.lock_connection()?;
        let updated = connection.execute(
            "UPDATE strategies
             SET name = ?2,
                 symbol = ?3,
                 budget = ?4,
                 grid_spacing_bps = ?5,
                 status = ?6,
                 source_template_id = ?7,
                 membership_ready = ?8,
                 exchange_ready = ?9,
                 symbol_ready = ?10
             WHERE id = ?1",
            params![
                strategy.id,
                strategy.name,
                strategy.symbol,
                strategy.budget,
                strategy.grid_spacing_bps as i64,
                strategy_status_to_str(&strategy.status),
                strategy.source_template_id,
                bool_to_i64(strategy.membership_ready),
                bool_to_i64(strategy.exchange_ready),
                bool_to_i64(strategy.symbol_ready),
            ],
        )?;
        Ok(updated)
    }

    pub fn delete_strategy(&self, strategy_id: &str) -> Result<usize, SharedDbError> {
        let connection = self.lock_connection()?;
        let deleted = connection.execute(
            "DELETE FROM strategies
             WHERE id = ?1",
            params![strategy_id],
        )?;
        Ok(deleted)
    }

    pub fn list_templates(&self) -> Result<Vec<StrategyTemplate>, SharedDbError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, name, symbol, budget, grid_spacing_bps, membership_ready, exchange_ready, symbol_ready
             FROM strategy_templates
             ORDER BY sequence_id ASC",
        )?;
        let rows = statement.query_map([], |row| strategy_template_from_row(row))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(SharedDbError::from)
    }

    pub fn find_template(
        &self,
        template_id: &str,
    ) -> Result<Option<StrategyTemplate>, SharedDbError> {
        let connection = self.lock_connection()?;
        connection
            .query_row(
                "SELECT id, name, symbol, budget, grid_spacing_bps, membership_ready, exchange_ready, symbol_ready
                 FROM strategy_templates
                 WHERE id = ?1",
                params![template_id],
                |row| strategy_template_from_row(row),
            )
            .optional()
            .map_err(SharedDbError::from)
    }

    pub fn insert_template(&self, template: &StoredStrategyTemplate) -> Result<(), SharedDbError> {
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO strategy_templates (
                id, sequence_id, name, symbol, budget, grid_spacing_bps,
                membership_ready, exchange_ready, symbol_ready
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                template.template.id,
                template.sequence_id as i64,
                template.template.name,
                template.template.symbol,
                template.template.budget,
                template.template.grid_spacing_bps as i64,
                bool_to_i64(template.template.membership_ready),
                bool_to_i64(template.template.exchange_ready),
                bool_to_i64(template.template.symbol_ready),
            ],
        )?;
        Ok(())
    }

    fn from_connection(connection: Connection) -> Result<Self, SharedDbError> {
        configure_connection(&connection)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(connection)),
        })
    }

    fn lock_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, SharedDbError> {
        self.inner
            .lock()
            .map_err(|_| SharedDbError::new("shared sqlite connection mutex poisoned"))
    }
}

impl Display for SharedDbError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for SharedDbError {}

impl SharedDbError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<rusqlite::Error> for SharedDbError {
    fn from(value: rusqlite::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl From<std::io::Error> for SharedDbError {
    fn from(value: std::io::Error) -> Self {
        Self::new(value.to_string())
    }
}

fn configure_connection(connection: &Connection) -> Result<(), SharedDbError> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;",
    )?;
    connection.execute_batch(INITIAL_CORE_MIGRATION)?;
    Ok(())
}

fn next_sequence_value(transaction: &Transaction<'_>, name: &str) -> Result<u64, SharedDbError> {
    transaction.execute(
        "INSERT OR IGNORE INTO shared_sequences (name, value)
         VALUES (?1, 0)",
        params![name],
    )?;
    let current = transaction.query_row(
        "SELECT value
         FROM shared_sequences
         WHERE name = ?1",
        params![name],
        |row| row.get::<_, i64>(0),
    )?;
    let next = current + 1;
    transaction.execute(
        "UPDATE shared_sequences
         SET value = ?2
         WHERE name = ?1",
        params![name, next],
    )?;
    Ok(next as u64)
}

fn billing_order_from_row(row: &rusqlite::Row<'_>) -> Result<BillingOrderRecord, rusqlite::Error> {
    let chain: String = row.get(2)?;
    let address: String = row.get(6)?;
    let requested_at_raw: String = row.get(5)?;
    let expires_at_raw: String = row.get(7)?;
    let paid_at_raw: Option<String> = row.get(8)?;
    let expires_at = parse_datetime(&expires_at_raw).map_err(to_from_sql_error)?;
    let requested_at = parse_datetime(&requested_at_raw).map_err(to_from_sql_error)?;
    let paid_at = parse_optional_datetime(paid_at_raw).map_err(to_from_sql_error)?;

    Ok(BillingOrderRecord {
        order_id: row.get::<_, i64>(0)? as u64,
        email: row.get(1)?,
        chain: chain.clone(),
        plan_code: row.get(3)?,
        amount: row.get(4)?,
        requested_at,
        assignment: AddressAssignment {
            chain,
            address,
            expires_at,
        },
        paid_at,
        tx_hash: row.get(9)?,
    })
}

fn strategy_from_row(row: &rusqlite::Row<'_>) -> Result<Strategy, rusqlite::Error> {
    let status_raw: String = row.get(5)?;
    let status = parse_strategy_status(&status_raw).map_err(to_from_sql_error)?;

    Ok(Strategy {
        id: row.get(0)?,
        name: row.get(1)?,
        symbol: row.get(2)?,
        budget: row.get(3)?,
        grid_spacing_bps: row.get::<_, i64>(4)? as u32,
        status,
        source_template_id: row.get(6)?,
        membership_ready: row.get::<_, i64>(7)? != 0,
        exchange_ready: row.get::<_, i64>(8)? != 0,
        symbol_ready: row.get::<_, i64>(9)? != 0,
    })
}

fn strategy_template_from_row(
    row: &rusqlite::Row<'_>,
) -> Result<StrategyTemplate, rusqlite::Error> {
    Ok(StrategyTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        symbol: row.get(2)?,
        budget: row.get(3)?,
        grid_spacing_bps: row.get::<_, i64>(4)? as u32,
        membership_ready: row.get::<_, i64>(5)? != 0,
        exchange_ready: row.get::<_, i64>(6)? != 0,
        symbol_ready: row.get::<_, i64>(7)? != 0,
    })
}

fn parse_datetime(value: &str) -> Result<DateTime<Utc>, SharedDbError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|error| SharedDbError::new(error.to_string()))
}

fn parse_optional_datetime(value: Option<String>) -> Result<Option<DateTime<Utc>>, SharedDbError> {
    value.map(|value| parse_datetime(&value)).transpose()
}

fn parse_strategy_status(value: &str) -> Result<StrategyStatus, SharedDbError> {
    match value {
        "Draft" => Ok(StrategyStatus::Draft),
        "Running" => Ok(StrategyStatus::Running),
        "Paused" => Ok(StrategyStatus::Paused),
        "Stopped" => Ok(StrategyStatus::Stopped),
        "Error" => Ok(StrategyStatus::Error),
        _ => Err(SharedDbError::new(format!(
            "unknown strategy status: {value}"
        ))),
    }
}

fn parse_optional_membership_status(
    value: Option<String>,
) -> Result<Option<MembershipStatus>, SharedDbError> {
    value
        .map(|value| parse_membership_status(&value))
        .transpose()
}

fn parse_membership_status(value: &str) -> Result<MembershipStatus, SharedDbError> {
    match value {
        "Pending" => Ok(MembershipStatus::Pending),
        "Active" => Ok(MembershipStatus::Active),
        "Grace" => Ok(MembershipStatus::Grace),
        "Expired" => Ok(MembershipStatus::Expired),
        "Frozen" => Ok(MembershipStatus::Frozen),
        "Revoked" => Ok(MembershipStatus::Revoked),
        _ => Err(SharedDbError::new(format!(
            "unknown membership status: {value}"
        ))),
    }
}

fn option_datetime_to_string(value: Option<DateTime<Utc>>) -> Option<String> {
    value.map(|timestamp| timestamp.to_rfc3339())
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn strategy_status_to_str(value: &StrategyStatus) -> &'static str {
    match value {
        StrategyStatus::Draft => "Draft",
        StrategyStatus::Running => "Running",
        StrategyStatus::Paused => "Paused",
        StrategyStatus::Stopped => "Stopped",
        StrategyStatus::Error => "Error",
    }
}

fn membership_status_to_str(value: &MembershipStatus) -> &'static str {
    match value {
        MembershipStatus::Pending => "Pending",
        MembershipStatus::Active => "Active",
        MembershipStatus::Grace => "Grace",
        MembershipStatus::Expired => "Expired",
        MembershipStatus::Frozen => "Frozen",
        MembershipStatus::Revoked => "Revoked",
    }
}

fn to_from_sql_error(error: SharedDbError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::{AuthUserRecord, SharedDb};
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn opening_db_bootstraps_core_tables() {
        let db = SharedDb::in_memory().expect("open in-memory db");
        let user = db
            .find_auth_user("missing@example.com")
            .expect("query users table");

        assert!(user.is_none());
        assert_eq!(SharedDb::bootstrap_label(), "sqlite");
    }

    #[test]
    fn file_backed_db_persists_auth_records() {
        let path = temp_db_path("shared-db");
        let db = SharedDb::open(&path).expect("open file-backed db");
        db.insert_auth_user(&AuthUserRecord {
            user_id: 1,
            email: "persisted@example.com".to_string(),
            password_hash: "hash".to_string(),
            email_verified: false,
            verification_code: Some("123456".to_string()),
            reset_code: None,
            totp_secret: None,
        })
        .expect("insert user");

        let reopened = SharedDb::open(&path).expect("reopen db");
        let user = reopened
            .find_auth_user("persisted@example.com")
            .expect("read user")
            .expect("user exists");

        assert_eq!(user.user_id, 1);
        assert_eq!(user.verification_code.as_deref(), Some("123456"));
    }

    fn temp_db_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic")
            .as_nanos();
        std::env::temp_dir().join(format!("grid-binance-{label}-{nonce}.sqlite3"))
    }
}
