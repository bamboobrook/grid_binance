use sqlx::{migrate::Migrator, PgPool};

use crate::SharedDbError;

static MIGRATOR: Migrator = sqlx::migrate!("../../db/migrations");

const REQUIRED_MIGRATIONS: [&str; 18] = [
    "0001_initial_core.sql",
    "0002_identity_security.sql",
    "0003_membership_billing.sql",
    "0004_trading.sql",
    "0005_admin_and_notifications.sql",
    "0006_membership_billing_runtime_hardening.sql",
    "0007_strategy_runtime_hardening.sql",
    "0008_strategy_runtime_mode_alignment.sql",
    "0009_strategy_snapshot_funding.sql",
    "0010_sweep_lifecycle_columns.sql",
    "0011_strategy_template_futures_fields.sql",
    "0012_strategy_engine_rewrite.sql",
    "0013_strategy_type_and_reference_source_template_support.sql",
    "0014_strategy_template_reference_price_support.sql",
    "0015_strategy_tags_and_notes.sql",
    "0016_notification_preferences.sql",
    "0017_martingale_backtest_portfolios.sql",
    "0018_martingale_batch_portfolio_publish.sql",
];

pub async fn run(pool: &PgPool) -> Result<(), SharedDbError> {
    MIGRATOR.run(pool).await.map_err(SharedDbError::from)
}

pub fn required_migrations() -> &'static [&'static str; 18] {
    &REQUIRED_MIGRATIONS
}
