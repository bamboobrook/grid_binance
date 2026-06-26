mod routes {
    pub mod admin_address_pools;
    pub mod admin_audit;
    pub mod admin_backtest;
    pub mod admin_deposits;
    pub mod admin_memberships;
    pub mod admin_strategies;
    pub mod admin_sweeps;
    pub mod admin_system;
    pub mod admin_templates;
    pub mod admin_users;
    pub mod analytics;
    pub mod auth;
    pub mod auth_guard;
    pub mod backtest;
    pub mod billing;
    pub mod exchange;
    pub mod exports;
    pub mod live_statistics;
    pub mod martingale_portfolios;
    pub mod membership;
    pub mod orders;
    pub mod profile;
    pub mod security;
    pub mod strategies;
    pub mod telegram;
}

mod services {
    pub mod analytics_service;
    pub mod auth_service;
    pub mod backtest_service;
    pub mod exchange_service;
    pub mod live_statistics_service;
    pub mod martingale_exchange_preconfigure_service;
    pub mod martingale_publish_service;
    pub mod membership_service;
    pub mod strategy_service;
    pub mod telegram_service;
}

use axum::{extract::FromRef, Router};
pub use services::backtest_service::normalize_martingale_auto_search_config;
use services::{
    analytics_service::AnalyticsService,
    auth_service::{AuthConfigError, AuthService},
    backtest_service::BacktestService,
    exchange_service::ExchangeService,
    live_statistics_service::LiveStatisticsService,
    martingale_publish_service::MartingalePublishService,
    membership_service::MembershipService,
    strategy_service::StrategyService,
    telegram_service::TelegramService,
};
use shared_db::{SharedDb, SharedDbError};

#[derive(Clone)]
pub struct AppState {
    analytics: AnalyticsService,
    backtest: BacktestService,
    auth: AuthService,
    db: SharedDb,
    exchange: ExchangeService,
    live_statistics: LiveStatisticsService,
    membership: MembershipService,
    martingale_publish: MartingalePublishService,
    strategy: StrategyService,
    telegram: TelegramService,
}

impl AppState {
    pub fn ephemeral() -> Result<Self, SharedDbError> {
        Self::from_shared_db(SharedDb::ephemeral()?)
    }

    pub fn persistent(
        database_url: impl AsRef<str>,
        redis_url: impl AsRef<str>,
    ) -> Result<Self, SharedDbError> {
        let db = SharedDb::connect(database_url, redis_url)?;
        let martingale_publish = MartingalePublishService::new(db.clone());
        Ok(Self {
            analytics: AnalyticsService::new(db.clone()),
            auth: AuthService::new_strict(db.clone())
                .map_err(|error| SharedDbError::new(error.to_string()))?,
            backtest: BacktestService::new(db.clone(), martingale_publish.clone()),
            db: db.clone(),
            exchange: ExchangeService::new_strict(db.clone())?,
            live_statistics: LiveStatisticsService::new(db.clone(), 600),
            membership: MembershipService::new(db.clone()),
            martingale_publish,
            strategy: StrategyService::new(db.clone()),
            telegram: TelegramService::new_strict(db)?,
        })
    }

    pub fn from_shared_db(db: SharedDb) -> Result<Self, SharedDbError> {
        let martingale_publish = MartingalePublishService::new(db.clone());
        Ok(Self {
            analytics: AnalyticsService::new(db.clone()),
            auth: AuthService::new_capture(db.clone()),
            backtest: BacktestService::new(db.clone(), martingale_publish.clone()),
            db: db.clone(),
            exchange: ExchangeService::new(db.clone()),
            live_statistics: LiveStatisticsService::new(db.clone(), 600),
            membership: MembershipService::new(db.clone()),
            martingale_publish,
            strategy: StrategyService::new(db.clone()),
            telegram: TelegramService::new(db),
        })
    }
}

impl FromRef<AppState> for SharedDb {
    fn from_ref(input: &AppState) -> Self {
        input.db.clone()
    }
}

impl FromRef<AppState> for AuthService {
    fn from_ref(input: &AppState) -> Self {
        input.auth.clone()
    }
}

impl FromRef<AppState> for AnalyticsService {
    fn from_ref(input: &AppState) -> Self {
        input.analytics.clone()
    }
}

impl FromRef<AppState> for BacktestService {
    fn from_ref(input: &AppState) -> Self {
        input.backtest.clone()
    }
}

impl FromRef<AppState> for MembershipService {
    fn from_ref(input: &AppState) -> Self {
        input.membership.clone()
    }
}

impl FromRef<AppState> for MartingalePublishService {
    fn from_ref(input: &AppState) -> Self {
        input.martingale_publish.clone()
    }
}

impl FromRef<AppState> for LiveStatisticsService {
    fn from_ref(input: &AppState) -> Self {
        input.live_statistics.clone()
    }
}

impl FromRef<AppState> for ExchangeService {
    fn from_ref(input: &AppState) -> Self {
        input.exchange.clone()
    }
}

impl FromRef<AppState> for StrategyService {
    fn from_ref(input: &AppState) -> Self {
        input.strategy.clone()
    }
}

impl FromRef<AppState> for TelegramService {
    fn from_ref(input: &AppState) -> Self {
        input.telegram.clone()
    }
}

pub fn app() -> Router {
    app_with_state(AppState::ephemeral().expect("test app state should initialize"))
}

pub fn build_persistent_state(
    database_url: impl AsRef<str>,
    redis_url: impl AsRef<str>,
) -> Result<AppState, AppBuildError> {
    let db = SharedDb::connect(database_url, redis_url).map_err(AppBuildError::from)?;
    let auth = if std::env::var("APP_ENV").ok().as_deref() == Some("test") {
        AuthService::new_capture(db.clone())
    } else {
        AuthService::new_strict(db.clone()).map_err(AppBuildError::from)?
    };
    let martingale_publish = MartingalePublishService::new(db.clone());
    Ok(AppState {
        analytics: AnalyticsService::new(db.clone()),
        auth,
        backtest: BacktestService::new(db.clone(), martingale_publish.clone()),
        db: db.clone(),
        exchange: ExchangeService::new_strict(db.clone()).map_err(AppBuildError::from)?,
        live_statistics: LiveStatisticsService::new(db.clone(), 600),
        membership: MembershipService::new(db.clone()),
        martingale_publish,
        strategy: StrategyService::new(db.clone()),
        telegram: TelegramService::new_strict(db).map_err(AppBuildError::from)?,
    })
}

pub fn spawn_background_workers(state: &AppState) {
    state.telegram.spawn_bot_update_poller();
}

pub fn app_with_persistent_state(
    database_url: impl AsRef<str>,
    redis_url: impl AsRef<str>,
) -> Result<Router, AppBuildError> {
    Ok(app_with_state(build_persistent_state(
        database_url,
        redis_url,
    )?))
}

pub fn app_with_state(state: AppState) -> Router {
    Router::new()
        .merge(routes::admin_address_pools::router())
        .merge(routes::admin_backtest::router())
        .merge(routes::admin_audit::router())
        .merge(routes::admin_deposits::router())
        .merge(routes::admin_memberships::router())
        .merge(routes::admin_strategies::router())
        .merge(routes::admin_sweeps::router())
        .merge(routes::admin_system::router())
        .merge(routes::admin_templates::router())
        .merge(routes::admin_users::router())
        .merge(routes::analytics::router())
        .merge(routes::auth::router())
        .merge(routes::backtest::router())
        .merge(routes::billing::router())
        .merge(routes::exchange::router())
        .merge(routes::exports::router())
        .merge(routes::live_statistics::router())
        .merge(routes::membership::router())
        .merge(routes::martingale_portfolios::router())
        .merge(routes::orders::router())
        .merge(routes::profile::router())
        .merge(routes::security::router())
        .merge(routes::strategies::router())
        .merge(routes::telegram::router())
        .with_state(state)
}

#[derive(Debug)]
pub enum AppBuildError {
    Storage(SharedDbError),
    AuthConfig(AuthConfigError),
}

impl From<SharedDbError> for AppBuildError {
    fn from(value: SharedDbError) -> Self {
        Self::Storage(value)
    }
}

impl From<AuthConfigError> for AppBuildError {
    fn from(value: AuthConfigError) -> Self {
        Self::AuthConfig(value)
    }
}

impl std::fmt::Display for AppBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(error) => write!(f, "{error}"),
            Self::AuthConfig(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AppBuildError {}

#[cfg(test)]
mod tests {
    use super::{app_with_persistent_state, AppState};
    use crate::services::auth_service::{LoginRequest, RegisterUserRequest, VerifyEmailRequest};
    use std::sync::{Mutex, OnceLock};

    #[test]
    fn app_state_reuses_ephemeral_auth_data_across_service_rebuilds() {
        let db = shared_db::SharedDb::ephemeral().expect("ephemeral db");
        let first = AppState::from_shared_db(db.clone()).expect("open first app state");
        let registered = first
            .auth
            .register(RegisterUserRequest {
                email: "persisted@app.test".to_string(),
                password: "secret".to_string(),
            })
            .expect("register user");
        first
            .auth
            .verify_email(VerifyEmailRequest {
                email: "persisted@app.test".to_string(),
                code: registered.verification_code.expect("verification code"),
            })
            .expect("verify email");
        let session = first
            .auth
            .login(LoginRequest {
                email: "persisted@app.test".to_string(),
                password: "secret".to_string(),
                totp_code: None,
            })
            .expect("login");

        let reopened = AppState::from_shared_db(db).expect("reopen app state");
        let claims = reopened
            .auth
            .session_claims(&session.session_token)
            .expect("session still exists");

        assert_eq!(claims.email, "persisted@app.test");
    }

    #[test]
    fn persistent_router_requires_runtime_auth_env() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::remove_var("ADMIN_EMAILS");
        std::env::remove_var("SESSION_TOKEN_SECRET");

        let router = app_with_persistent_state(
            "postgres://grid:secret@localhost/grid",
            "redis://localhost:6379/0",
        );

        assert!(router.is_err());
    }

    #[test]
    fn persistent_router_requires_telegram_bot_secret_env() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("ADMIN_EMAILS", "admin@example.com");
        std::env::set_var("SESSION_TOKEN_SECRET", "grid-binance-dev-session-secret");
        std::env::remove_var("TELEGRAM_BOT_BIND_SECRET");

        let router = app_with_persistent_state(
            "postgres://grid:secret@localhost/grid",
            "redis://localhost:6379/0",
        );

        assert!(router.is_err());
    }

    #[test]
    fn persistent_router_rejects_invalid_database_url() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("ADMIN_EMAILS", "admin@example.com");
        std::env::set_var("SESSION_TOKEN_SECRET", "grid-binance-dev-session-secret");

        let router = app_with_persistent_state("not-a-postgres-url", "redis://localhost:6379/0");

        assert!(router.is_err());
    }

    #[test]
    fn persistent_router_rejects_invalid_redis_url() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("ADMIN_EMAILS", "admin@example.com");
        std::env::set_var("SESSION_TOKEN_SECRET", "grid-binance-dev-session-secret");

        let router =
            app_with_persistent_state("postgres://grid:secret@localhost/grid", "not-a-redis-url");

        assert!(router.is_err());
    }

    #[test]
    fn live_statistics_service_returns_structured_data_from_db_tables() {
        use crate::services::live_statistics_service::LiveStatisticsService;
        use shared_db::{
            AccountProfitSnapshotRecord, ExchangeTradeHistoryRecord, ExchangeWalletSnapshotRecord,
        };
        use shared_domain::strategy::{
            GridGeneration, GridLevel, PostTriggerAction, ReferencePriceSource, Strategy,
            StrategyAmountMode, StrategyMarket, StrategyMode, StrategyRevision, StrategyRuntime,
            StrategyRuntimeEvent, StrategyRuntimeOrder, StrategyRuntimePosition, StrategyStatus,
            StrategyType,
        };

        let db = shared_db::SharedDb::ephemeral().expect("ephemeral db");
        let email = "trader@db.test";
        let now = chrono::Utc::now();

        let revision = StrategyRevision {
            revision_id: "rev-1".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Arithmetic,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: rust_decimal::Decimal::new(49000, 0),
                quantity: rust_decimal::Decimal::new(2, 2),
                take_profit_bps: 200,
                trailing_bps: None,
            }],
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        };

        let ts_now = now.to_rfc3339();
        let strategy = Strategy {
            id: "s1".to_string(),
            owner_email: email.to_string(),
            name: "Live BTC".to_string(),
            symbol: "BTCUSDT".to_string(),
            budget: "10000".to_string(),
            grid_spacing_bps: 50,
            status: StrategyStatus::Running,
            source_template_id: None,
            membership_ready: true,
            exchange_ready: true,
            permissions_ready: true,
            withdrawals_disabled: false,
            hedge_mode_ready: true,
            symbol_ready: true,
            filters_ready: true,
            margin_ready: true,
            conflict_ready: true,
            balance_ready: true,
            strategy_type: StrategyType::OrdinaryGrid,
            market: StrategyMarket::FuturesUsdM,
            mode: StrategyMode::FuturesLong,
            runtime_phase: Default::default(),
            runtime_controls: Default::default(),
            draft_revision: revision,
            tags: vec![],
            notes: String::new(),
            active_revision: None,
            runtime: StrategyRuntime {
                positions: vec![StrategyRuntimePosition {
                    market: StrategyMarket::FuturesUsdM,
                    mode: StrategyMode::FuturesLong,
                    quantity: rust_decimal::Decimal::new(1, 1),
                    average_entry_price: rust_decimal::Decimal::new(500, 2),
                }],
                orders: vec![StrategyRuntimeOrder {
                    order_id: "order-1".to_string(),
                    exchange_order_id: None,
                    level_index: Some(0),
                    side: "Buy".to_string(),
                    order_type: "Limit".to_string(),
                    price: Some(rust_decimal::Decimal::new(500, 2)),
                    quantity: rust_decimal::Decimal::new(1, 1),
                    status: "Placed".to_string(),
                }],
                fills: vec![],
                events: vec![
                    StrategyRuntimeEvent {
                        event_type: "last_stream_event_at".to_string(),
                        detail: ts_now.clone(),
                        price: None,
                        created_at: now,
                    },
                    StrategyRuntimeEvent {
                        event_type: "last_rest_reconcile_at".to_string(),
                        detail: ts_now.clone(),
                        price: None,
                        created_at: now,
                    },
                ],
                last_preflight: None,
            },
            archived_at: None,
        };

        db.insert_strategy(&shared_db::StoredStrategy {
            sequence_id: 1,
            strategy,
        })
        .expect("insert strategy");

        let _ = db.insert_exchange_trade_history(&ExchangeTradeHistoryRecord {
            trade_id: "t1".to_string(),
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            symbol: "BTCUSDT".to_string(),
            side: "BUY".to_string(),
            quantity: "0.1".to_string(),
            price: "50000".to_string(),
            fee_amount: Some("2.50".to_string()),
            fee_asset: Some("USDT".to_string()),
            traded_at: now,
        });

        let _ = db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            realized_pnl: "150".to_string(),
            unrealized_pnl: "75".to_string(),
            fees: "4.00".to_string(),
            funding: Some("-3.50".to_string()),
            captured_at: now,
        });

        let mut balances = serde_json::Map::new();
        balances.insert("USDT".to_string(), serde_json::json!("9800.50"));
        let _ = db.insert_exchange_wallet_snapshot(&ExchangeWalletSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            wallet_type: "futures".to_string(),
            balances: serde_json::Value::Object(balances),
            captured_at: now,
        });

        let service = LiveStatisticsService::new(db.clone(), 600);
        let stats = service
            .compute_live_stats(email)
            .expect("compute live stats");

        assert_eq!(stats.open_order_count, 1, "open_order_count");
        assert_eq!(stats.position_count, 1, "position_count");
        assert!(
            stats.fees_paid.parse::<f64>().unwrap_or(0.0) > 0.0,
            "fees_paid {} should be > 0 (from structured data, not price*0.001)",
            stats.fees_paid,
        );
        assert!(
            stats.funding_total.parse::<f64>().unwrap_or(0.0) != 0.0,
            "funding_total should be non-zero: {}",
            stats.funding_total,
        );
        assert!(
            stats.wallet_balance.parse::<f64>().unwrap_or(0.0) > 0.0,
            "wallet_balance should be > 0 (9800.50 from wallet snapshot): {}",
            stats.wallet_balance,
        );
        assert!(
            stats.last_user_stream_event_at.is_some(),
            "last_user_stream_event_at"
        );
        assert!(
            stats.last_rest_reconcile_at.is_some(),
            "last_rest_reconcile_at"
        );
        assert!(
            !stats.stats_stale,
            "stats should be fresh with recent sync timestamps"
        );
        assert!(!stats.computed_at.is_empty(), "computed_at should be set");
    }

    #[test]
    fn portfolio_live_stats_returns_error_for_nonexistent_portfolio() {
        use crate::services::live_statistics_service::LiveStatisticsService;

        let db = shared_db::SharedDb::ephemeral().expect("ephemeral db");
        let email = "owner@test.com";
        let service = LiveStatisticsService::new(db, 600);

        let result = service.compute_portfolio_live_stats(email, "nonexistent-portfolio");
        assert!(result.is_err(), "should error for nonexistent portfolio");
        assert!(
            result.unwrap_err().to_string().contains("not found"),
            "error should indicate not found/not owned"
        );
    }

    #[test]
    fn portfolio_live_stats_filters_only_enabled_strategy_ids() {
        use crate::services::live_statistics_service::LiveStatisticsService;
        use shared_db::AccountProfitSnapshotRecord;
        use shared_domain::strategy::{
            GridGeneration, GridLevel, PostTriggerAction, ReferencePriceSource, Strategy,
            StrategyAmountMode, StrategyMarket, StrategyMode, StrategyRevision, StrategyRuntime,
            StrategyRuntimeEvent, StrategyRuntimeOrder, StrategyStatus, StrategyType,
        };

        let db = shared_db::SharedDb::ephemeral().expect("ephemeral db");
        let email = "filter@test.com";
        let now = chrono::Utc::now();

        let revision = StrategyRevision {
            revision_id: "rev-f".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Arithmetic,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: rust_decimal::Decimal::new(50000, 0),
                quantity: rust_decimal::Decimal::new(1, 2),
                take_profit_bps: 200,
                trailing_bps: None,
            }],
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        };

        let ts_now = now.to_rfc3339();

        let strat_a = Strategy {
            id: "strat-a".to_string(),
            owner_email: email.to_string(),
            name: "A".to_string(),
            symbol: "BTCUSDT".to_string(),
            budget: "5000".to_string(),
            grid_spacing_bps: 50,
            status: StrategyStatus::Running,
            source_template_id: None,
            membership_ready: true,
            exchange_ready: true,
            permissions_ready: true,
            withdrawals_disabled: false,
            hedge_mode_ready: true,
            symbol_ready: true,
            filters_ready: true,
            margin_ready: true,
            conflict_ready: true,
            balance_ready: true,
            strategy_type: StrategyType::OrdinaryGrid,
            market: StrategyMarket::FuturesUsdM,
            mode: StrategyMode::FuturesLong,
            runtime_phase: Default::default(),
            runtime_controls: Default::default(),
            draft_revision: revision.clone(),
            tags: vec![],
            notes: String::new(),
            active_revision: None,
            runtime: StrategyRuntime {
                positions: vec![],
                orders: vec![StrategyRuntimeOrder {
                    order_id: "a-1".to_string(),
                    exchange_order_id: None,
                    level_index: Some(0),
                    side: "Buy".to_string(),
                    order_type: "Limit".to_string(),
                    price: Some(rust_decimal::Decimal::new(500, 2)),
                    quantity: rust_decimal::Decimal::new(1, 1),
                    status: "Placed".to_string(),
                }],
                fills: vec![],
                events: vec![StrategyRuntimeEvent {
                    event_type: "last_stream_event_at".to_string(),
                    detail: ts_now.clone(),
                    price: None,
                    created_at: now,
                }],
                last_preflight: None,
            },
            archived_at: None,
        };

        let strat_b = Strategy {
            id: "strat-b".to_string(),
            owner_email: email.to_string(),
            name: "B".to_string(),
            symbol: "ETHUSDT".to_string(),
            budget: "5000".to_string(),
            grid_spacing_bps: 50,
            status: StrategyStatus::Running,
            source_template_id: None,
            membership_ready: true,
            exchange_ready: true,
            permissions_ready: true,
            withdrawals_disabled: false,
            hedge_mode_ready: true,
            symbol_ready: true,
            filters_ready: true,
            margin_ready: true,
            conflict_ready: true,
            balance_ready: true,
            strategy_type: StrategyType::OrdinaryGrid,
            market: StrategyMarket::FuturesUsdM,
            mode: StrategyMode::FuturesLong,
            runtime_phase: Default::default(),
            runtime_controls: Default::default(),
            draft_revision: revision.clone(),
            tags: vec![],
            notes: String::new(),
            active_revision: None,
            runtime: StrategyRuntime {
                positions: vec![],
                orders: vec![
                    StrategyRuntimeOrder {
                        order_id: "b-1".to_string(),
                        exchange_order_id: None,
                        level_index: Some(0),
                        side: "Buy".to_string(),
                        order_type: "Limit".to_string(),
                        price: Some(rust_decimal::Decimal::new(30, 2)),
                        quantity: rust_decimal::Decimal::new(1, 0),
                        status: "Placed".to_string(),
                    },
                    StrategyRuntimeOrder {
                        order_id: "b-2".to_string(),
                        exchange_order_id: None,
                        level_index: Some(1),
                        side: "Buy".to_string(),
                        order_type: "Limit".to_string(),
                        price: Some(rust_decimal::Decimal::new(29, 2)),
                        quantity: rust_decimal::Decimal::new(1, 0),
                        status: "Placed".to_string(),
                    },
                ],
                fills: vec![],
                events: vec![StrategyRuntimeEvent {
                    event_type: "last_stream_event_at".to_string(),
                    detail: ts_now.clone(),
                    price: None,
                    created_at: now,
                }],
                last_preflight: None,
            },
            archived_at: None,
        };

        db.insert_strategy(&shared_db::StoredStrategy {
            sequence_id: 1,
            strategy: strat_a,
        })
        .expect("insert A");
        db.insert_strategy(&shared_db::StoredStrategy {
            sequence_id: 2,
            strategy: strat_b,
        })
        .expect("insert B");

        let _ = db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            realized_pnl: "100".to_string(),
            unrealized_pnl: "50".to_string(),
            fees: "5".to_string(),
            funding: Some("-1".to_string()),
            captured_at: now,
        });

        let service = LiveStatisticsService::new(db.clone(), 600);

        let all_stats = service.compute_live_stats(email).expect("compute all");
        assert_eq!(
            all_stats.open_order_count, 3,
            "all strategies: 1+2 = 3 orders"
        );

        // Verify portfolio scoping via compute_live_statistics_from_db directly
        // (full portfolio flow requires postgres; core logic already tested in
        // statistics.rs: portfolio_scoped_stats_only_include_given_strategy_ids)
        let scoped = trading_engine::statistics::compute_live_statistics_from_db(
            &db,
            email,
            Some(&["strat-a".to_string()]),
            600,
        )
        .expect("scoped");
        assert_eq!(
            scoped.open_order_count, 1,
            "only strat-a (1 order), not strat-b (2 orders)"
        );

        let scoped_b = trading_engine::statistics::compute_live_statistics_from_db(
            &db,
            email,
            Some(&["strat-b".to_string()]),
            600,
        )
        .expect("scoped B");
        assert_eq!(scoped_b.open_order_count, 2, "only strat-b (2 orders)");
    }

    #[test]
    fn portfolio_live_stats_uses_config_strategy_id_not_msi_instance_id() {
        use shared_db::AccountProfitSnapshotRecord;
        use shared_domain::strategy::{
            GridGeneration, GridLevel, PostTriggerAction, ReferencePriceSource, Strategy,
            StrategyAmountMode, StrategyMarket, StrategyMode, StrategyRevision, StrategyRuntime,
            StrategyRuntimeEvent, StrategyRuntimeOrder, StrategyRuntimePosition, StrategyStatus,
            StrategyType,
        };

        let db = shared_db::SharedDb::ephemeral().expect("ephemeral db");
        let email = "realflow@test.com";
        let now = chrono::Utc::now();

        let revision = StrategyRevision {
            revision_id: "rev-rf".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Arithmetic,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: rust_decimal::Decimal::new(50000, 0),
                quantity: rust_decimal::Decimal::new(1, 2),
                take_profit_bps: 200,
                trailing_bps: None,
            }],
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        };

        let ts_now = now.to_rfc3339();

        let strat_long = Strategy {
            id: "btc-long".to_string(),
            owner_email: email.to_string(),
            name: "BTC Long".to_string(),
            symbol: "BTCUSDT".to_string(),
            budget: "5000".to_string(),
            grid_spacing_bps: 50,
            status: StrategyStatus::Running,
            source_template_id: None,
            membership_ready: true,
            exchange_ready: true,
            permissions_ready: true,
            withdrawals_disabled: false,
            hedge_mode_ready: true,
            symbol_ready: true,
            filters_ready: true,
            margin_ready: true,
            conflict_ready: true,
            balance_ready: true,
            strategy_type: StrategyType::OrdinaryGrid,
            market: StrategyMarket::FuturesUsdM,
            mode: StrategyMode::FuturesLong,
            runtime_phase: Default::default(),
            runtime_controls: Default::default(),
            draft_revision: revision.clone(),
            tags: vec![],
            notes: String::new(),
            active_revision: None,
            runtime: StrategyRuntime {
                positions: vec![StrategyRuntimePosition {
                    market: StrategyMarket::FuturesUsdM,
                    mode: StrategyMode::FuturesLong,
                    quantity: rust_decimal::Decimal::new(1, 1),
                    average_entry_price: rust_decimal::Decimal::new(500, 2),
                }],
                orders: vec![StrategyRuntimeOrder {
                    order_id: "btc-long-order-1".to_string(),
                    exchange_order_id: None,
                    level_index: Some(0),
                    side: "Buy".to_string(),
                    order_type: "Limit".to_string(),
                    price: Some(rust_decimal::Decimal::new(500, 2)),
                    quantity: rust_decimal::Decimal::new(1, 1),
                    status: "Placed".to_string(),
                }],
                fills: vec![],
                events: vec![StrategyRuntimeEvent {
                    event_type: "last_stream_event_at".to_string(),
                    detail: ts_now.clone(),
                    price: None,
                    created_at: now,
                }],
                last_preflight: None,
            },
            archived_at: None,
        };

        let strat_short = Strategy {
            id: "btc-short".to_string(),
            owner_email: email.to_string(),
            name: "BTC Short".to_string(),
            symbol: "BTCUSDT".to_string(),
            budget: "5000".to_string(),
            grid_spacing_bps: 50,
            status: StrategyStatus::Running,
            source_template_id: None,
            membership_ready: true,
            exchange_ready: true,
            permissions_ready: true,
            withdrawals_disabled: false,
            hedge_mode_ready: true,
            symbol_ready: true,
            filters_ready: true,
            margin_ready: true,
            conflict_ready: true,
            balance_ready: true,
            strategy_type: StrategyType::OrdinaryGrid,
            market: StrategyMarket::FuturesUsdM,
            mode: StrategyMode::FuturesShort,
            runtime_phase: Default::default(),
            runtime_controls: Default::default(),
            draft_revision: revision.clone(),
            tags: vec![],
            notes: String::new(),
            active_revision: None,
            runtime: StrategyRuntime {
                positions: vec![],
                orders: vec![
                    StrategyRuntimeOrder {
                        order_id: "btc-short-order-1".to_string(),
                        exchange_order_id: None,
                        level_index: Some(0),
                        side: "Sell".to_string(),
                        order_type: "Limit".to_string(),
                        price: Some(rust_decimal::Decimal::new(520, 2)),
                        quantity: rust_decimal::Decimal::new(1, 1),
                        status: "Placed".to_string(),
                    },
                    StrategyRuntimeOrder {
                        order_id: "btc-short-order-2".to_string(),
                        exchange_order_id: None,
                        level_index: Some(1),
                        side: "Sell".to_string(),
                        order_type: "Limit".to_string(),
                        price: Some(rust_decimal::Decimal::new(530, 2)),
                        quantity: rust_decimal::Decimal::new(1, 1),
                        status: "Placed".to_string(),
                    },
                ],
                fills: vec![],
                events: vec![StrategyRuntimeEvent {
                    event_type: "last_stream_event_at".to_string(),
                    detail: ts_now,
                    price: None,
                    created_at: now,
                }],
                last_preflight: None,
            },
            archived_at: None,
        };

        db.insert_strategy(&shared_db::StoredStrategy {
            sequence_id: 1,
            strategy: strat_long,
        })
        .expect("insert btc-long");
        db.insert_strategy(&shared_db::StoredStrategy {
            sequence_id: 2,
            strategy: strat_short,
        })
        .expect("insert btc-short");

        let _ = db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            realized_pnl: "200".to_string(),
            unrealized_pnl: "80".to_string(),
            fees: "8".to_string(),
            funding: Some("-3".to_string()),
            captured_at: now,
        });

        // Simulate: the live-stats filter uses strategy_id from config (btc-long/btc-short)
        let with_real_ids = trading_engine::statistics::compute_live_statistics_from_db(
            &db,
            email,
            Some(&["btc-long".to_string(), "btc-short".to_string()]),
            600,
        )
        .expect("real IDs");
        assert_eq!(
            with_real_ids.open_order_count, 3,
            "1 long + 2 short = 3 orders"
        );

        // Simulate: if we wrongly use msi_* values as filter, nothing matches
        let with_msi_ids = trading_engine::statistics::compute_live_statistics_from_db(
            &db,
            email,
            Some(&["msi_abc".to_string(), "msi_def".to_string()]),
            600,
        )
        .expect("msi IDs");
        assert_eq!(
            with_msi_ids.open_order_count, 0,
            "msi_* should not match any runtime strategy ID (btc-long/btc-short)"
        );

        assert_eq!(
            with_msi_ids.realized_pnl, "200",
            "account-level PnL still populated even when no strategies match (wallet data is account-scoped)"
        );
    }

    #[test]
    fn portfolio_live_stats_service_full_flow_with_msi_mapping_to_runtime_strategy_ids() {
        use crate::services::live_statistics_service::LiveStatisticsService;
        use shared_db::{
            AccountProfitSnapshotRecord, NewBacktestCandidateRecord, NewBacktestTaskRecord,
        };
        use shared_domain::strategy::{
            GridGeneration, GridLevel, PostTriggerAction, ReferencePriceSource, Strategy,
            StrategyAmountMode, StrategyMarket, StrategyMode, StrategyRevision, StrategyRuntime,
            StrategyRuntimeEvent, StrategyRuntimeOrder, StrategyRuntimePosition, StrategyStatus,
            StrategyType,
        };

        let db = shared_db::SharedDb::ephemeral().expect("ephemeral db");
        let email = "fullflow@test.com";
        let now = chrono::Utc::now();
        let repo = db.backtest_repo();

        let task = repo
            .create_task(NewBacktestTaskRecord {
                owner: email.to_string(),
                strategy_type: "martingale_grid".to_string(),
                config: serde_json::json!({"symbol": "BTCUSDT", "timeframe": "1h"}),
                summary: serde_json::json!({}),
            })
            .expect("create task");

        let candidate = repo
            .save_candidate(NewBacktestCandidateRecord {
                task_id: task.task_id.clone(),
                status: "ready".to_string(),
                rank: 1,
                config: serde_json::json!({
                    "portfolio_config": {
                        "strategies": [{
                            "strategy_id": "btc-long",
                            "symbol": "BTCUSDT",
                            "market": "usd_m_futures",
                            "direction": "long",
                            "direction_mode": "long_and_short",
                            "leverage": 4,
                            "margin_mode": "isolated"
                        }]
                    }
                }),
                summary: serde_json::json!({"score": 0.8}),
            })
            .expect("save candidate long");

        let candidate2 = repo
            .save_candidate(NewBacktestCandidateRecord {
                task_id: task.task_id.clone(),
                status: "ready".to_string(),
                rank: 2,
                config: serde_json::json!({
                    "portfolio_config": {
                        "strategies": [{
                            "strategy_id": "btc-short",
                            "symbol": "BTCUSDT",
                            "market": "usd_m_futures",
                            "direction": "short",
                            "direction_mode": "long_and_short",
                            "leverage": 4,
                            "margin_mode": "isolated"
                        }]
                    }
                }),
                summary: serde_json::json!({"score": 0.7}),
            })
            .expect("save candidate short");

        let msi_long = "msi_long_full".to_string();
        let msi_short = "msi_short_full".to_string();
        let portfolio_config = serde_json::json!({
            "kind": "martingale_batch_portfolio",
            "portfolio_config": {
                "direction_mode": "long_and_short",
                "strategies": [
                    {
                        "strategy_id": "btc-long",
                        "strategy_instance_id": msi_long,
                        "portfolio_weight_pct": "50",
                        "symbol": "BTCUSDT",
                        "market": "usd_m_futures",
                        "direction": "long",
                        "leverage": 4,
                        "margin_mode": "isolated"
                    },
                    {
                        "strategy_id": "btc-short",
                        "strategy_instance_id": msi_short,
                        "portfolio_weight_pct": "50",
                        "symbol": "BTCUSDT",
                        "market": "usd_m_futures",
                        "direction": "short",
                        "leverage": 4,
                        "margin_mode": "isolated"
                    }
                ]
            }
        });

        let portfolio = repo
            .create_martingale_portfolio(
                shared_db::NewMartingalePortfolioRecord {
                    portfolio_id: "pf-fullflow".to_string(),
                    owner: email.to_string(),
                    name: "FullFlow Portfolio".to_string(),
                    status: "running".to_string(),
                    source_task_id: task.task_id,
                    market: "usd_m_futures".to_string(),
                    direction: "long_short".to_string(),
                    risk_profile: "balanced".to_string(),
                    total_weight_pct: rust_decimal::Decimal::new(100, 0),
                    config: portfolio_config,
                    risk_summary: serde_json::json!({
                        "exchange_preconfigure": {"status": "ready", "checked_at": now.to_rfc3339()},
                        "live_executor_started": true
                    }),
                },
                vec![
                    shared_db::NewMartingalePortfolioItemRecord {
                        strategy_instance_id: msi_long,
                        candidate_id: candidate.candidate_id,
                        symbol: "BTCUSDT".to_string(),
                        weight_pct: rust_decimal::Decimal::new(50, 0),
                        leverage: 4,
                        enabled: true,
                        status: "running".to_string(),
                        parameter_snapshot: candidate.config.clone(),
                        metrics_snapshot: candidate.summary.clone(),
                    },
                    shared_db::NewMartingalePortfolioItemRecord {
                        strategy_instance_id: msi_short,
                        candidate_id: candidate2.candidate_id,
                        symbol: "BTCUSDT".to_string(),
                        weight_pct: rust_decimal::Decimal::new(50, 0),
                        leverage: 4,
                        enabled: true,
                        status: "running".to_string(),
                        parameter_snapshot: candidate2.config.clone(),
                        metrics_snapshot: candidate2.summary.clone(),
                    },
                ],
            )
            .expect("create portfolio");

        assert_eq!(portfolio.items.len(), 2);
        assert_eq!(portfolio.items[0].strategy_instance_id, "msi_long_full");
        assert_eq!(portfolio.items[1].strategy_instance_id, "msi_short_full");

        let revision = StrategyRevision {
            revision_id: "rev-ff".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Arithmetic,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: rust_decimal::Decimal::new(50000, 0),
                quantity: rust_decimal::Decimal::new(1, 2),
                take_profit_bps: 200,
                trailing_bps: None,
            }],
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        };

        let ts_now = now.to_rfc3339();
        db.insert_strategy(&shared_db::StoredStrategy {
            sequence_id: 1,
            strategy: Strategy {
                id: "btc-long".to_string(),
                owner_email: email.to_string(),
                name: "BTC Long".to_string(),
                symbol: "BTCUSDT".to_string(),
                budget: "5000".to_string(),
                grid_spacing_bps: 50,
                status: StrategyStatus::Running,
                source_template_id: None,
                membership_ready: true,
                exchange_ready: true,
                permissions_ready: true,
                withdrawals_disabled: false,
                hedge_mode_ready: true,
                symbol_ready: true,
                filters_ready: true,
                margin_ready: true,
                conflict_ready: true,
                balance_ready: true,
                strategy_type: StrategyType::OrdinaryGrid,
                market: StrategyMarket::FuturesUsdM,
                mode: StrategyMode::FuturesLong,
                runtime_phase: Default::default(),
                runtime_controls: Default::default(),
                draft_revision: revision.clone(),
                tags: vec![],
                notes: String::new(),
                active_revision: None,
                runtime: StrategyRuntime {
                    positions: vec![StrategyRuntimePosition {
                        market: StrategyMarket::FuturesUsdM,
                        mode: StrategyMode::FuturesLong,
                        quantity: rust_decimal::Decimal::new(1, 1),
                        average_entry_price: rust_decimal::Decimal::new(500, 2),
                    }],
                    orders: vec![StrategyRuntimeOrder {
                        order_id: "long-order-1".to_string(),
                        exchange_order_id: None,
                        level_index: Some(0),
                        side: "Buy".to_string(),
                        order_type: "Limit".to_string(),
                        price: Some(rust_decimal::Decimal::new(495, 2)),
                        quantity: rust_decimal::Decimal::new(1, 1),
                        status: "Placed".to_string(),
                    }],
                    fills: vec![],
                    events: vec![StrategyRuntimeEvent {
                        event_type: "last_stream_event_at".to_string(),
                        detail: ts_now.clone(),
                        price: None,
                        created_at: now,
                    }],
                    last_preflight: None,
                },
                archived_at: None,
            },
        })
        .expect("insert btc-long");

        db.insert_strategy(&shared_db::StoredStrategy {
            sequence_id: 2,
            strategy: Strategy {
                id: "btc-short".to_string(),
                owner_email: email.to_string(),
                name: "BTC Short".to_string(),
                symbol: "BTCUSDT".to_string(),
                budget: "5000".to_string(),
                grid_spacing_bps: 50,
                status: StrategyStatus::Running,
                source_template_id: None,
                membership_ready: true,
                exchange_ready: true,
                permissions_ready: true,
                withdrawals_disabled: false,
                hedge_mode_ready: true,
                symbol_ready: true,
                filters_ready: true,
                margin_ready: true,
                conflict_ready: true,
                balance_ready: true,
                strategy_type: StrategyType::OrdinaryGrid,
                market: StrategyMarket::FuturesUsdM,
                mode: StrategyMode::FuturesShort,
                runtime_phase: Default::default(),
                runtime_controls: Default::default(),
                draft_revision: revision,
                tags: vec![],
                notes: String::new(),
                active_revision: None,
                runtime: StrategyRuntime {
                    positions: vec![],
                    orders: vec![
                        StrategyRuntimeOrder {
                            order_id: "short-order-1".to_string(),
                            exchange_order_id: None,
                            level_index: Some(0),
                            side: "Sell".to_string(),
                            order_type: "Limit".to_string(),
                            price: Some(rust_decimal::Decimal::new(505, 2)),
                            quantity: rust_decimal::Decimal::new(1, 1),
                            status: "Placed".to_string(),
                        },
                        StrategyRuntimeOrder {
                            order_id: "short-order-2".to_string(),
                            exchange_order_id: None,
                            level_index: Some(1),
                            side: "Sell".to_string(),
                            order_type: "Limit".to_string(),
                            price: Some(rust_decimal::Decimal::new(510, 2)),
                            quantity: rust_decimal::Decimal::new(1, 1),
                            status: "Placed".to_string(),
                        },
                    ],
                    fills: vec![],
                    events: vec![StrategyRuntimeEvent {
                        event_type: "last_stream_event_at".to_string(),
                        detail: ts_now,
                        price: None,
                        created_at: now,
                    }],
                    last_preflight: None,
                },
                archived_at: None,
            },
        })
        .expect("insert btc-short");

        let _ = db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            realized_pnl: "200".to_string(),
            unrealized_pnl: "80".to_string(),
            fees: "8".to_string(),
            funding: Some("-3".to_string()),
            captured_at: now,
        });

        let service = LiveStatisticsService::new(db.clone(), 600);
        let stats = service
            .compute_portfolio_live_stats(email, "pf-fullflow")
            .expect("compute portfolio live stats");

        assert_eq!(
            stats.open_order_count, 3,
            "1 long + 2 short = 3 runtime orders"
        );
        assert_eq!(stats.realized_pnl, "200");
        assert_eq!(stats.unrealized_pnl, "80");
        assert_eq!(stats.fees_paid, "8");
        assert_eq!(stats.funding_total, "-3");
    }

    #[test]
    fn portfolio_live_stats_fallback_order_count_from_risk_summary_when_strategy_runtime_empty() {
        use crate::services::live_statistics_service::LiveStatisticsService;
        use shared_db::{
            AccountProfitSnapshotRecord, NewBacktestCandidateRecord, NewBacktestTaskRecord,
        };

        let db = shared_db::SharedDb::ephemeral().expect("ephemeral db");
        let email = "fallback@test.com";
        let now = chrono::Utc::now();
        let repo = db.backtest_repo();

        let task = repo
            .create_task(NewBacktestTaskRecord {
                owner: email.to_string(),
                strategy_type: "martingale_grid".to_string(),
                config: serde_json::json!({"symbol": "BTCUSDT"}),
                summary: serde_json::json!({}),
            })
            .expect("create task");

        let candidate = repo
            .save_candidate(NewBacktestCandidateRecord {
                task_id: task.task_id.clone(),
                status: "ready".to_string(),
                rank: 1,
                config: serde_json::json!({"portfolio_config": {"strategies": [{"strategy_id": "btc-long", "symbol": "BTCUSDT"}]}}),
                summary: serde_json::json!({"score": 0.8}),
            })
            .expect("save cand");

        // Portfolio with risk_summary.order_count but NO Strategy runtime records
        let _portfolio = repo
            .create_martingale_portfolio(
                shared_db::NewMartingalePortfolioRecord {
                    portfolio_id: "pf-fallback".to_string(),
                    owner: email.to_string(),
                    name: "Fallback Portfolio".to_string(),
                    status: "running".to_string(),
                    source_task_id: task.task_id,
                    market: "usd_m_futures".to_string(),
                    direction: "long".to_string(),
                    risk_profile: "balanced".to_string(),
                    total_weight_pct: rust_decimal::Decimal::new(100, 0),
                    config: serde_json::json!({
                        "portfolio_config": {
                            "strategies": [{
                                "strategy_id": "btc-long",
                                "symbol": "BTCUSDT",
                                "market": "usd_m_futures",
                                "direction": "long"
                            }]
                        }
                    }),
                    risk_summary: serde_json::json!({
                        "exchange_preconfigure": {"status": "ready"},
                        "live_executor_started": true,
                        "live_executor_state": "started",
                        "order_count": 42,
                        "strategy_count": 1
                    }),
                },
                vec![shared_db::NewMartingalePortfolioItemRecord {
                    strategy_instance_id: "msi_fallback_1".to_string(),
                    candidate_id: candidate.candidate_id,
                    symbol: "BTCUSDT".to_string(),
                    weight_pct: rust_decimal::Decimal::new(100, 0),
                    leverage: 4,
                    enabled: true,
                    status: "running".to_string(),
                    parameter_snapshot: candidate.config,
                    metrics_snapshot: candidate.summary,
                }],
            )
            .expect("create portfolio");

        // No Strategy runtime records inserted (simulating pre-backfill state)
        // But account snapshots exist
        let _ = db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            realized_pnl: "100".to_string(),
            unrealized_pnl: "50".to_string(),
            fees: "5".to_string(),
            funding: Some("-1".to_string()),
            captured_at: now,
        });

        let service = LiveStatisticsService::new(db.clone(), 600);
        let stats = service
            .compute_portfolio_live_stats(email, "pf-fallback")
            .expect("compute");

        assert_eq!(
            stats.open_order_count, 42,
            "fallback: risk_summary.order_count=42 when Strategy runtime is empty"
        );
        assert_eq!(stats.realized_pnl, "100");
        assert_eq!(stats.unrealized_pnl, "50");
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
