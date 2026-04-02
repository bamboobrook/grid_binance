use sqlx::{migrate::Migrator, PgPool};

use crate::SharedDbError;

static MIGRATOR: Migrator = sqlx::migrate!("../../db/migrations");

const REQUIRED_MIGRATIONS: [&str; 7] = [
    "0001_initial_core.sql",
    "0002_identity_security.sql",
    "0003_membership_billing.sql",
    "0004_trading.sql",
    "0005_admin_and_notifications.sql",
    "0006_membership_billing_runtime_hardening.sql",
    "0007_strategy_runtime_hardening.sql",
];

pub async fn run(pool: &PgPool) -> Result<(), SharedDbError> {
    MIGRATOR.run(pool).await.map_err(SharedDbError::from)
}

pub fn required_migrations() -> &'static [&'static str; 7] {
    &REQUIRED_MIGRATIONS
}
