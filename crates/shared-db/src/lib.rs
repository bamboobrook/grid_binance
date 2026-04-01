use std::{
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fmt::{Display, Formatter},
    future::Future,
    sync::{Arc, Mutex},
};

use ::redis::RedisError;
use chrono::{DateTime, Duration, Utc};
use crate::postgres::{
    admin::AdminRepository,
    billing::BillingRepository,
    exchange::ExchangeRepository,
    identity::IdentityRepository,
    strategy::StrategyRepository,
    PostgresConfig, PostgresStore,
};
use crate::redis::{RedisConfig, RedisStore};
use shared_domain::{
    membership::MembershipStatus,
    strategy::{Strategy, StrategyStatus, StrategyTemplate},
};

pub mod postgres;
pub mod redis;

pub use crate::postgres::billing::{BillingOrderRecord, MembershipRecord};
pub use crate::postgres::admin::AuditLogRecord;
pub use crate::postgres::identity::AuthUserRecord;

#[derive(Clone)]
pub struct SharedDb {
    backend: SharedDbBackend,
}

#[derive(Clone)]
enum SharedDbBackend {
    Runtime {
        postgres: PostgresStore,
        redis: RedisStore,
    },
    Ephemeral(Arc<Mutex<EphemeralState>>),
}

#[derive(Debug, Default)]
struct EphemeralState {
    sequences: HashMap<String, u64>,
    auth_users: HashMap<String, AuthUserRecord>,
    auth_sessions: HashMap<String, String>,
    audit_logs: Vec<AuditLogRecord>,
    billing_orders: BTreeMap<u64, BillingOrderRecord>,
    seen_transfers: HashSet<String>,
    membership_records: HashMap<String, MembershipRecord>,
    strategies: BTreeMap<u64, Strategy>,
    templates: BTreeMap<u64, StrategyTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedDbError {
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredStrategy {
    pub sequence_id: u64,
    pub strategy: Strategy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredStrategyTemplate {
    pub sequence_id: u64,
    pub template: StrategyTemplate,
}

impl SharedDb {
    pub fn connect(
        database_url: impl AsRef<str>,
        redis_url: impl AsRef<str>,
    ) -> Result<Self, SharedDbError> {
        let postgres = PostgresConfig::new(database_url.as_ref())?;
        let redis = RedisConfig::new(redis_url.as_ref())?;
        Self::from_configs(postgres, redis)
    }

    pub fn open(database_url: impl AsRef<str>) -> Result<Self, SharedDbError> {
        let redis_url = std::env::var("REDIS_URL")
            .map_err(|_| SharedDbError::new("REDIS_URL is required for SharedDb::open"))?;
        Self::connect(database_url, redis_url)
    }

    pub fn ephemeral() -> Result<Self, SharedDbError> {
        Ok(Self {
            backend: SharedDbBackend::Ephemeral(Arc::new(Mutex::new(EphemeralState::default()))),
        })
    }

    pub fn bootstrap_label() -> &'static str {
        "postgresql+redis"
    }

    pub fn postgres(&self) -> &PostgresStore {
        match &self.backend {
            SharedDbBackend::Runtime { postgres, .. } => postgres,
            SharedDbBackend::Ephemeral(_) => {
                panic!("postgres() is unavailable for the ephemeral test backend")
            }
        }
    }

    pub fn redis(&self) -> &RedisStore {
        match &self.backend {
            SharedDbBackend::Runtime { redis, .. } => redis,
            SharedDbBackend::Ephemeral(_) => {
                panic!("redis() is unavailable for the ephemeral test backend")
            }
        }
    }

    pub fn identity_repo(&self) -> IdentityRepository {
        IdentityRepository::new(self.postgres().pool().clone())
    }

    pub fn billing_repo(&self) -> BillingRepository {
        BillingRepository::new(self.postgres().pool().clone())
    }

    pub fn exchange_repo(&self) -> ExchangeRepository {
        ExchangeRepository::new(self.postgres().pool().clone())
    }

    pub fn strategy_repo(&self) -> StrategyRepository {
        StrategyRepository::new(self.postgres().pool().clone())
    }

    pub fn notification_repo(&self) -> postgres::notification::NotificationRepository {
        postgres::notification::NotificationRepository::new(self.postgres().pool().clone())
    }

    pub fn admin_repo(&self) -> AdminRepository {
        AdminRepository::new(self.postgres().pool().clone())
    }

    pub fn next_sequence(&self, name: &str) -> Result<u64, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { postgres, .. } => {
                let postgres = postgres.clone();
                let name = name.to_owned();
                Self::block_on(async move {
                    postgres::transaction::next_sequence(postgres.pool(), &name).await
                })
            }
            SharedDbBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let next = state.sequences.entry(name.to_owned()).or_insert(0);
                *next += 1;
                Ok(*next)
            }
        }
    }

    pub fn find_auth_user(&self, email: &str) -> Result<Option<AuthUserRecord>, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.identity_repo();
                let email = email.to_owned();
                Self::block_on(async move { repo.find_auth_user(&email).await })
            }
            SharedDbBackend::Ephemeral(state) => Ok(lock_ephemeral(state)?
                .auth_users
                .get(&email.to_lowercase())
                .cloned()),
        }
    }

    pub fn insert_auth_user(&self, record: &AuthUserRecord) -> Result<(), SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.identity_repo();
                let record = record.clone();
                Self::block_on(async move { repo.insert_auth_user(&record).await })
            }
            SharedDbBackend::Ephemeral(state) => {
                lock_ephemeral(state)?
                    .auth_users
                    .insert(record.email.to_lowercase(), record.clone());
                Ok(())
            }
        }
    }

    pub fn update_auth_email_verification(
        &self,
        email: &str,
        email_verified: bool,
        verification_code: Option<&str>,
    ) -> Result<usize, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.identity_repo();
                let email = email.to_owned();
                let verification_code = verification_code.map(str::to_owned);
                Self::block_on(async move {
                    repo.update_auth_email_verification(
                        &email,
                        email_verified,
                        verification_code.as_deref(),
                    )
                    .await
                })
            }
            SharedDbBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let Some(user) = state.auth_users.get_mut(&email.to_lowercase()) else {
                    return Ok(0);
                };
                user.email_verified = email_verified;
                user.verification_code = verification_code.map(str::to_owned);
                Ok(1)
            }
        }
    }

    pub fn set_auth_reset_code(
        &self,
        email: &str,
        reset_code: Option<&str>,
    ) -> Result<usize, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.identity_repo();
                let email = email.to_owned();
                let reset_code = reset_code.map(str::to_owned);
                Self::block_on(async move {
                    repo.set_auth_reset_code(&email, reset_code.as_deref()).await
                })
            }
            SharedDbBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let Some(user) = state.auth_users.get_mut(&email.to_lowercase()) else {
                    return Ok(0);
                };
                user.reset_code = reset_code.map(str::to_owned);
                Ok(1)
            }
        }
    }

    pub fn update_auth_password(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<usize, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.identity_repo();
                let email = email.to_owned();
                let password_hash = password_hash.to_owned();
                Self::block_on(async move { repo.update_auth_password(&email, &password_hash).await })
            }
            SharedDbBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let Some(user) = state.auth_users.get_mut(&email.to_lowercase()) else {
                    return Ok(0);
                };
                user.password_hash = password_hash.to_owned();
                user.reset_code = None;
                Ok(1)
            }
        }
    }

    pub fn set_auth_totp_secret(
        &self,
        email: &str,
        totp_secret: Option<&str>,
    ) -> Result<usize, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.identity_repo();
                let email = email.to_owned();
                let totp_secret = totp_secret.map(str::to_owned);
                Self::block_on(async move {
                    repo.set_auth_totp_secret(&email, totp_secret.as_deref()).await
                })
            }
            SharedDbBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let Some(user) = state.auth_users.get_mut(&email.to_lowercase()) else {
                    return Ok(0);
                };
                user.totp_secret = totp_secret.map(str::to_owned);
                Ok(1)
            }
        }
    }

    pub fn insert_auth_session(
        &self,
        session_token: &str,
        email: &str,
        sid: u64,
    ) -> Result<(), SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.identity_repo();
                let session_token = session_token.to_owned();
                let email = email.to_owned();
                Self::block_on(async move { repo.insert_auth_session(&session_token, &email, sid).await })
            }
            SharedDbBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let normalized = email.to_lowercase();
                if !state.auth_users.contains_key(&normalized) {
                    return Err(SharedDbError::new("cannot create session for missing user"));
                }
                state
                    .auth_sessions
                    .insert(session_token.to_owned(), normalized);
                Ok(())
            }
        }
    }

    pub fn find_auth_session_email(
        &self,
        session_token: &str,
    ) -> Result<Option<String>, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.identity_repo();
                let session_token = session_token.to_owned();
                Self::block_on(async move { repo.find_auth_session_email(&session_token).await })
            }
            SharedDbBackend::Ephemeral(state) => Ok(lock_ephemeral(state)?
                .auth_sessions
                .get(session_token)
                .cloned()),
        }
    }

    pub fn insert_audit_log(&self, record: &AuditLogRecord) -> Result<(), SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.admin_repo();
                let record = record.clone();
                Self::block_on(async move { repo.insert_audit_log(&record).await })
            }
            SharedDbBackend::Ephemeral(state) => {
                lock_ephemeral(state)?.audit_logs.push(record.clone());
                Ok(())
            }
        }
    }

    pub fn list_billing_orders(&self) -> Result<Vec<BillingOrderRecord>, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.billing_repo();
                Self::block_on(async move { repo.list_orders().await })
            }
            SharedDbBackend::Ephemeral(state) => Ok(lock_ephemeral(state)?
                .billing_orders
                .values()
                .cloned()
                .collect()),
        }
    }

    pub fn insert_billing_order(&self, order: &BillingOrderRecord) -> Result<(), SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.billing_repo();
                let order = order.clone();
                Self::block_on(async move { repo.insert_order(&order).await })
            }
            SharedDbBackend::Ephemeral(state) => {
                lock_ephemeral(state)?
                    .billing_orders
                    .insert(order.order_id, order.clone());
                Ok(())
            }
        }
    }

    pub fn record_seen_transfer(
        &self,
        tx_hash: &str,
        chain: &str,
        observed_at: DateTime<Utc>,
    ) -> Result<bool, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.billing_repo();
                let tx_hash = tx_hash.to_owned();
                let chain = chain.to_owned();
                Self::block_on(async move { repo.record_seen_transfer(&tx_hash, &chain, observed_at).await })
            }
            SharedDbBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                Ok(state.seen_transfers.insert(format!("{chain}:{tx_hash}")))
            }
        }
    }

    pub fn find_membership_record(
        &self,
        email: &str,
    ) -> Result<Option<MembershipRecord>, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.billing_repo();
                let email = email.to_owned();
                Self::block_on(async move { repo.find_membership_record(&email).await })
            }
            SharedDbBackend::Ephemeral(state) => Ok(lock_ephemeral(state)?
                .membership_records
                .get(&email.to_lowercase())
                .cloned()),
        }
    }

    pub fn upsert_membership_record(
        &self,
        email: &str,
        record: &MembershipRecord,
    ) -> Result<(), SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.billing_repo();
                let email = email.to_owned();
                let record = record.clone();
                Self::block_on(async move { repo.upsert_membership_record(&email, &record).await })
            }
            SharedDbBackend::Ephemeral(state) => {
                lock_ephemeral(state)?
                    .membership_records
                    .insert(email.to_lowercase(), record.clone());
                Ok(())
            }
        }
    }

    pub fn update_membership_override(
        &self,
        email: &str,
        override_status: Option<&MembershipStatus>,
    ) -> Result<(), SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.billing_repo();
                let email = email.to_owned();
                let override_status = override_status.cloned();
                Self::block_on(async move {
                    repo.update_membership_override(&email, override_status.as_ref()).await
                })
            }
            SharedDbBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let entry = state
                    .membership_records
                    .entry(email.to_lowercase())
                    .or_default();
                entry.override_status = override_status.cloned();
                Ok(())
            }
        }
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
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.billing_repo();
                let tx_hash = tx_hash.to_owned();
                let email = email.to_owned();
                Self::block_on(async move {
                    repo.apply_payment(order_id, &tx_hash, paid_at, &email, active_until, grace_until)
                        .await
                })
            }
            SharedDbBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                if let Some(order) = state.billing_orders.get_mut(&order_id) {
                    order.paid_at = Some(paid_at);
                    order.tx_hash = Some(tx_hash.to_owned());
                }
                state.membership_records.insert(
                    email.to_lowercase(),
                    MembershipRecord {
                        activated_at: Some(paid_at),
                        active_until: Some(active_until),
                        grace_until: Some(grace_until),
                        override_status: None,
                    },
                );
                Ok(())
            }
        }
    }

    pub fn list_strategies(&self, owner_email: &str) -> Result<Vec<Strategy>, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.strategy_repo();
                let owner_email = owner_email.to_owned();
                Self::block_on(async move { repo.list_strategies(&owner_email).await })
            }
            SharedDbBackend::Ephemeral(state) => Ok(lock_ephemeral(state)?
                .strategies
                .values()
                .filter(|strategy| strategy.owner_email == owner_email)
                .cloned()
                .collect()),
        }
    }

    pub fn find_strategy(
        &self,
        owner_email: &str,
        strategy_id: &str,
    ) -> Result<Option<Strategy>, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.strategy_repo();
                let owner_email = owner_email.to_owned();
                let strategy_id = strategy_id.to_owned();
                Self::block_on(async move { repo.find_strategy(&owner_email, &strategy_id).await })
            }
            SharedDbBackend::Ephemeral(state) => Ok(lock_ephemeral(state)?
                .strategies
                .values()
                .find(|strategy| strategy.owner_email == owner_email && strategy.id == strategy_id)
                .cloned()),
        }
    }

    pub fn insert_strategy(&self, strategy: &StoredStrategy) -> Result<(), SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.strategy_repo();
                let strategy = strategy.clone();
                Self::block_on(async move { repo.insert_strategy(&strategy).await })
            }
            SharedDbBackend::Ephemeral(state) => {
                lock_ephemeral(state)?
                    .strategies
                    .insert(strategy.sequence_id, strategy.strategy.clone());
                Ok(())
            }
        }
    }

    pub fn update_strategy(&self, strategy: &Strategy) -> Result<usize, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.strategy_repo();
                let strategy = strategy.clone();
                Self::block_on(async move { repo.update_strategy(&strategy).await })
            }
            SharedDbBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let Some((_, stored)) = state
                    .strategies
                    .iter_mut()
                    .find(|(_, stored)| stored.id == strategy.id && stored.owner_email == strategy.owner_email)
                else {
                    return Ok(0);
                };
                *stored = strategy.clone();
                Ok(1)
            }
        }
    }

    pub fn delete_strategy(
        &self,
        owner_email: &str,
        strategy_id: &str,
    ) -> Result<usize, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.strategy_repo();
                let owner_email = owner_email.to_owned();
                let strategy_id = strategy_id.to_owned();
                Self::block_on(async move { repo.delete_strategy(&owner_email, &strategy_id).await })
            }
            SharedDbBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let before = state.strategies.len();
                state
                    .strategies
                    .retain(|_, strategy| !(strategy.owner_email == owner_email && strategy.id == strategy_id));
                Ok(before.saturating_sub(state.strategies.len()))
            }
        }
    }

    pub fn list_templates(&self) -> Result<Vec<StrategyTemplate>, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.admin_repo();
                Self::block_on(async move { repo.list_templates().await })
            }
            SharedDbBackend::Ephemeral(state) => Ok(lock_ephemeral(state)?
                .templates
                .values()
                .cloned()
                .collect()),
        }
    }

    pub fn find_template(
        &self,
        template_id: &str,
    ) -> Result<Option<StrategyTemplate>, SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.admin_repo();
                let template_id = template_id.to_owned();
                Self::block_on(async move { repo.find_template(&template_id).await })
            }
            SharedDbBackend::Ephemeral(state) => Ok(lock_ephemeral(state)?
                .templates
                .values()
                .find(|template| template.id == template_id)
                .cloned()),
        }
    }

    pub fn insert_template(&self, template: &StoredStrategyTemplate) -> Result<(), SharedDbError> {
        match &self.backend {
            SharedDbBackend::Runtime { .. } => {
                let repo = self.admin_repo();
                let template = template.clone();
                Self::block_on(async move { repo.insert_template(&template).await })
            }
            SharedDbBackend::Ephemeral(state) => {
                lock_ephemeral(state)?
                    .templates
                    .insert(template.sequence_id, template.template.clone());
                Ok(())
            }
        }
    }

    fn from_configs(
        postgres: PostgresConfig,
        redis: RedisConfig,
    ) -> Result<Self, SharedDbError> {
        Self::block_on(async move {
            let postgres = PostgresStore::connect(postgres).await?;
            let redis = RedisStore::connect(redis).await?;
            Ok(Self {
                backend: SharedDbBackend::Runtime { postgres, redis },
            })
        })
    }

    fn block_on<F, T>(future: F) -> Result<T, SharedDbError>
    where
        F: Future<Output = Result<T, SharedDbError>> + Send + 'static,
        T: Send + 'static,
    {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
                return Ok(tokio::task::block_in_place(move || handle.block_on(future))?);
            }
        }

        blocking_runtime()?
            .lock()
            .map_err(|_| SharedDbError::new("shared-db blocking runtime mutex poisoned"))?
            .block_on(future)
    }
}

fn lock_ephemeral(
    state: &Arc<Mutex<EphemeralState>>,
) -> Result<std::sync::MutexGuard<'_, EphemeralState>, SharedDbError> {
    state
        .lock()
        .map_err(|_| SharedDbError::new("ephemeral shared-db mutex poisoned"))
}

fn blocking_runtime() -> Result<&'static Mutex<tokio::runtime::Runtime>, SharedDbError> {
    static RUNTIME: std::sync::OnceLock<Result<Mutex<tokio::runtime::Runtime>, SharedDbError>> =
        std::sync::OnceLock::new();
    RUNTIME
        .get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map(Mutex::new)
            .map_err(SharedDbError::from)
        })
        .as_ref()
        .map_err(Clone::clone)
}

impl Display for SharedDbError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for SharedDbError {}

impl SharedDbError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<RedisError> for SharedDbError {
    fn from(value: RedisError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<sqlx::Error> for SharedDbError {
    fn from(value: sqlx::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl From<sqlx::migrate::MigrateError> for SharedDbError {
    fn from(value: sqlx::migrate::MigrateError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<std::io::Error> for SharedDbError {
    fn from(value: std::io::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl From<std::env::VarError> for SharedDbError {
    fn from(value: std::env::VarError) -> Self {
        Self::new(value.to_string())
    }
}

pub(crate) fn parse_strategy_status(value: &str) -> Result<StrategyStatus, SharedDbError> {
    match value {
        "Draft" => Ok(StrategyStatus::Draft),
        "Running" => Ok(StrategyStatus::Running),
        "Paused" => Ok(StrategyStatus::Paused),
        "Stopped" => Ok(StrategyStatus::Stopped),
        "Error" => Ok(StrategyStatus::Error),
        _ => Err(SharedDbError::new(format!("unknown strategy status: {value}"))),
    }
}

pub(crate) fn strategy_status_to_str(value: &StrategyStatus) -> &'static str {
    match value {
        StrategyStatus::Draft => "Draft",
        StrategyStatus::Running => "Running",
        StrategyStatus::Paused => "Paused",
        StrategyStatus::Stopped => "Stopped",
        StrategyStatus::Error => "Error",
    }
}

pub(crate) fn parse_membership_status(value: &str) -> Result<MembershipStatus, SharedDbError> {
    match value {
        "Pending" => Ok(MembershipStatus::Pending),
        "Active" => Ok(MembershipStatus::Active),
        "Grace" => Ok(MembershipStatus::Grace),
        "Expired" => Ok(MembershipStatus::Expired),
        "Frozen" => Ok(MembershipStatus::Frozen),
        "Revoked" => Ok(MembershipStatus::Revoked),
        _ => Err(SharedDbError::new(format!("unknown membership status: {value}"))),
    }
}

pub(crate) fn membership_status_to_str(value: &MembershipStatus) -> &'static str {
    match value {
        MembershipStatus::Pending => "Pending",
        MembershipStatus::Active => "Active",
        MembershipStatus::Grace => "Grace",
        MembershipStatus::Expired => "Expired",
        MembershipStatus::Frozen => "Frozen",
        MembershipStatus::Revoked => "Revoked",
    }
}

pub(crate) fn default_token_expiry() -> DateTime<Utc> {
    Utc::now() + Duration::hours(24)
}

#[cfg(test)]
mod tests {
    use super::{postgres, redis, SharedDb};

    #[test]
    fn bootstrap_label_reflects_postgres_and_redis_runtime() {
        assert_eq!(SharedDb::bootstrap_label(), "postgresql+redis");
    }

    #[test]
    fn migration_manifest_lists_all_foundation_files() {
        assert_eq!(
            *postgres::migrations::required_migrations(),
            [
                "0001_initial_core.sql",
                "0002_identity_security.sql",
                "0003_membership_billing.sql",
                "0004_trading.sql",
                "0005_admin_and_notifications.sql",
            ]
        );
    }

    #[test]
    fn config_parsers_require_runtime_urls() {
        assert!(postgres::PostgresConfig::new("postgres://grid:secret@localhost/grid").is_ok());
        assert!(redis::RedisConfig::new("redis://127.0.0.1:6379/0").is_ok());
        assert!(postgres::PostgresConfig::new("not-a-postgres-url").is_err());
        assert!(redis::RedisConfig::new("not-a-redis-url").is_err());
    }
}
