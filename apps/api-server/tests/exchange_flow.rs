use api_server::{app, app_with_state, AppState};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use shared_db::{MembershipRecord, SharedDb};
use shared_domain::strategy::{StrategyMarket, StrategyMode, StrategyRuntimePosition, StrategyStatus};
use std::sync::{Mutex, OnceLock};
use tower::ServiceExt;

#[tokio::test]
async fn save_credentials_persists_masked_account_health_and_three_market_symbol_metadata() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("app state"));
    let session_token = register_and_login(&app, "exchange-save@example.com").await;

    let response = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["account"]["exchange"], "binance");
    assert_eq!(body["account"]["api_key_masked"], "demo****1234");
    assert_eq!(body["account"]["connection_status"], "healthy");
    assert_eq!(body["account"]["sync_status"], "success");
    assert_eq!(body["account"]["validation"]["can_read_spot"], true);
    assert_eq!(body["account"]["validation"]["can_read_usdm"], true);
    assert_eq!(body["account"]["validation"]["can_read_coinm"], true);
    assert_eq!(body["account"]["validation"]["api_connectivity_ok"], true);
    assert_eq!(body["account"]["validation"]["timestamp_in_sync"], true);
    assert_eq!(body["account"]["validation"]["hedge_mode_ok"], true);
    assert_eq!(body["account"]["validation"]["permissions_ok"], true);
    assert_eq!(body["account"]["validation"]["market_access_ok"], true);
    assert_eq!(
        body["account"]["selected_markets"],
        json!(["spot", "usdm", "coinm"])
    );
    assert_eq!(body["account"]["symbol_counts"]["spot"], 2);
    assert_eq!(body["account"]["symbol_counts"]["usdm"], 2);
    assert_eq!(body["account"]["symbol_counts"]["coinm"], 2);
    assert_eq!(body["synced_symbols"], 6);
    assert!(body["account"]["last_checked_at"].is_string());
    assert!(body["account"]["last_synced_at"].is_string());
}

#[tokio::test]
async fn test_credentials_uses_current_input_without_persisting_exchange_account() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("app state"));
    let session_token = register_and_login(&app, "exchange-test-only@example.com").await;

    let response = test_credentials(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["persisted"], false);
    assert_eq!(body["account"]["api_key_masked"], "demo****1234");
    assert_eq!(body["account"]["connection_status"], "healthy");
    assert_eq!(body["account"]["validation"]["can_read_spot"], true);
    assert_eq!(body["account"]["validation"]["can_read_usdm"], true);
    assert_eq!(body["account"]["validation"]["can_read_coinm"], true);
    assert_eq!(body["synced_symbols"], 6);

    let read = read_account(&app, Some(&session_token)).await;
    assert_eq!(read.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn one_user_only_has_one_binance_account_and_updates_replace_the_masked_read_model() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("app state"));
    let session_token = register_and_login(&app, "exchange-single-account@example.com").await;

    let first = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let second = save_credentials(
        &app,
        Some(&session_token),
        "next-key-5678",
        "demo-secret",
        true,
    )
    .await;

    assert_eq!(second.status(), StatusCode::OK);
    let second_body = response_json(second).await;
    assert_eq!(second_body["account"]["api_key_masked"], "next****5678");
    assert_eq!(second_body["synced_symbols"], 6);

    let read = read_account(&app, Some(&session_token)).await;
    assert_eq!(read.status(), StatusCode::OK);
    let read_body = response_json(read).await;
    assert_eq!(read_body["account"]["api_key_masked"], "next****5678");
    assert_eq!(read_body["account"]["symbol_counts"]["spot"], 2);
    assert_eq!(read_body["account"]["symbol_counts"]["usdm"], 2);
    assert_eq!(read_body["account"]["symbol_counts"]["coinm"], 2);
}

#[tokio::test]
async fn read_account_survives_partial_persistence_and_preserves_saved_state() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("app state"));
    let session_token = register_and_login(&app, "exchange-partial@example.com").await;

    db.upsert_exchange_credentials(&shared_db::UserExchangeCredentialRecord {
        user_email: "exchange-partial@example.com".to_string(),
        exchange: "binance".to_string(),
        api_key_masked: "demo****1234".to_string(),
        encrypted_secret: "ciphertext".to_string(),
    })
    .expect("seed credentials");

    let read = read_account(&app, Some(&session_token)).await;
    assert_eq!(read.status(), StatusCode::OK);
    let body = response_json(read).await;
    assert_eq!(body["account"]["api_key_masked"], "demo****1234");
    assert_eq!(body["account"]["binding_state"], "partial");
    assert_eq!(body["account"]["connection_status"], "untested");
}

#[tokio::test]
async fn read_account_accepts_legacy_validation_metadata_with_withdrawal_disabled_field() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("app state"));
    let session_token = register_and_login(&app, "exchange-legacy@example.com").await;

    db.upsert_exchange_account(&shared_db::UserExchangeAccountRecord {
        user_email: "exchange-legacy@example.com".to_string(),
        exchange: "binance".to_string(),
        account_label: "Binance".to_string(),
        market_scope: "spot,usdm,coinm".to_string(),
        is_active: true,
        checked_at: Some(chrono::Utc::now()),
        metadata: json!({
            "api_key_masked": "demo****1234",
            "connection_status": "healthy",
            "sync_status": "success",
            "last_synced_at": chrono::Utc::now().to_rfc3339(),
            "expected_hedge_mode": true,
            "selected_markets": ["spot", "usdm", "coinm"],
            "validation": {
                "api_connectivity_ok": true,
                "timestamp_in_sync": true,
                "can_read_spot": true,
                "can_read_usdm": true,
                "can_read_coinm": true,
                "hedge_mode_ok": true,
                "permissions_ok": true,
                "withdrawal_disabled": true,
                "market_access_ok": true
            },
            "symbol_counts": {
                "spot": 2,
                "usdm": 2,
                "coinm": 2
            }
        }),
    })
    .expect("seed account");

    db.upsert_exchange_credentials(&shared_db::UserExchangeCredentialRecord {
        user_email: "exchange-legacy@example.com".to_string(),
        exchange: "binance".to_string(),
        api_key_masked: "demo****1234".to_string(),
        encrypted_secret: "ciphertext".to_string(),
    })
    .expect("seed credentials");

    let read = read_account(&app, Some(&session_token)).await;
    assert_eq!(read.status(), StatusCode::OK);
    let body = response_json(read).await;
    assert_eq!(body["account"]["binding_state"], "saved");
    assert_eq!(body["account"]["validation"]["withdrawals_disabled"], true);
}

#[tokio::test]
async fn credential_updates_require_running_strategies_to_be_paused_first() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("app state"));
    let session_token = register_and_login(&app, "exchange-pause-first@example.com").await;
    let now = chrono::Utc::now();
    db.upsert_membership_record(
        "exchange-pause-first@example.com",
        &MembershipRecord {
            activated_at: Some(now),
            active_until: Some(now + chrono::Duration::days(30)),
            grace_until: Some(now + chrono::Duration::days(32)),
            override_status: None,
        },
    )
    .expect("membership");

    let first = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    db.insert_exchange_wallet_snapshot(&shared_db::ExchangeWalletSnapshotRecord {
        user_email: "exchange-pause-first@example.com".to_string(),
        exchange: "binance".to_string(),
        wallet_type: "spot".to_string(),
        balances: json!({ "USDT": "1000" }),
        captured_at: chrono::Utc::now(),
    })
    .expect("wallet snapshot");

    let strategy = create_strategy(
        &app,
        &session_token,
        json!({
            "name": "needs pause",
            "symbol": "BTCUSDT",
            "market": "Spot",
            "mode": "SpotClassic",
            "generation": "Custom",
            "levels": [{
                "entry_price": "100.00",
                "quantity": "0.1000",
                "take_profit_bps": 100,
                "trailing_bps": null
            }],
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
            "permissions_ready": true,
            "withdrawals_disabled": true,
            "hedge_mode_ready": true,
            "filters_ready": true,
            "margin_ready": true,
            "conflict_ready": true,
            "balance_ready": true,
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    )
    .await;
    assert_eq!(strategy.status(), StatusCode::CREATED);
    let strategy_id = response_json(strategy).await["id"]
        .as_str()
        .expect("strategy id")
        .to_owned();

    let started = start_strategy(&app, &session_token, &strategy_id).await;
    assert_eq!(started.status(), StatusCode::OK);

    let blocked = save_credentials(
        &app,
        Some(&session_token),
        "next-key-5678",
        "demo-secret",
        true,
    )
    .await;
    assert_eq!(blocked.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(blocked).await["error"],
        "pause running strategies before updating exchange credentials"
    );

    let read = read_account(&app, Some(&session_token)).await;
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(
        response_json(read).await["account"]["api_key_masked"],
        "demo****1234"
    );

    let paused = pause_strategies(&app, &session_token, &[&strategy_id]).await;
    assert_eq!(paused.status(), StatusCode::OK);
    assert_eq!(response_json(paused).await["paused"], 1);

    let retried = save_credentials(
        &app,
        Some(&session_token),
        "next-key-5678",
        "demo-secret",
        true,
    )
    .await;
    assert_eq!(retried.status(), StatusCode::OK);
    assert_eq!(
        response_json(retried).await["account"]["api_key_masked"],
        "next****5678"
    );
}

#[tokio::test]
async fn credential_updates_are_blocked_when_paused_strategy_still_has_positions() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("app state"));
    let session_token = register_and_login(&app, "exchange-paused-open@example.com").await;
    let now = chrono::Utc::now();
    db.upsert_membership_record(
        "exchange-paused-open@example.com",
        &MembershipRecord {
            activated_at: Some(now),
            active_until: Some(now + chrono::Duration::days(30)),
            grace_until: Some(now + chrono::Duration::days(32)),
            override_status: None,
        },
    )
    .expect("membership");

    let first = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    db.insert_exchange_wallet_snapshot(&shared_db::ExchangeWalletSnapshotRecord {
        user_email: "exchange-paused-open@example.com".to_string(),
        exchange: "binance".to_string(),
        wallet_type: "spot".to_string(),
        balances: json!({ "USDT": "1000" }),
        captured_at: chrono::Utc::now(),
    })
    .expect("wallet snapshot");

    let strategy = create_strategy(
        &app,
        &session_token,
        json!({
            "name": "paused with holdings",
            "symbol": "BTCUSDT",
            "market": "Spot",
            "mode": "SpotClassic",
            "generation": "Custom",
            "levels": [{
                "entry_price": "100.00",
                "quantity": "0.1000",
                "take_profit_bps": 100,
                "trailing_bps": null
            }],
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
            "permissions_ready": true,
            "withdrawals_disabled": true,
            "hedge_mode_ready": true,
            "filters_ready": true,
            "margin_ready": true,
            "conflict_ready": true,
            "balance_ready": true,
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    )
    .await;
    assert_eq!(strategy.status(), StatusCode::CREATED);
    let strategy_id = response_json(strategy).await["id"]
        .as_str()
        .expect("strategy id")
        .to_owned();

    let paused = pause_strategies(&app, &session_token, &[&strategy_id]).await;
    assert_eq!(paused.status(), StatusCode::OK);

    let mut stored = db
        .find_strategy("exchange-paused-open@example.com", &strategy_id)
        .expect("find strategy")
        .expect("strategy");
    stored.status = StrategyStatus::Paused;
    stored.runtime.positions = vec![StrategyRuntimePosition {
        market: StrategyMarket::Spot,
        mode: StrategyMode::SpotClassic,
        quantity: Decimal::new(1, 1),
        average_entry_price: Decimal::new(100, 0),
    }];
    db.update_strategy(&stored).expect("update strategy");

    let blocked = save_credentials(
        &app,
        Some(&session_token),
        "next-key-5678",
        "demo-secret",
        true,
    )
    .await;
    assert_eq!(blocked.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(blocked).await["error"],
        "fully stop strategies and close remaining positions before updating exchange credentials"
    );
}

#[tokio::test]
async fn fuzzy_search_uses_persisted_symbol_metadata_after_service_rebuild() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let first_app = app_with_state(AppState::from_shared_db(db.clone()).expect("first app"));
    let session_token = register_and_login(&first_app, "exchange-search@example.com").await;
    let sync = save_credentials(
        &first_app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;
    assert_eq!(sync.status(), StatusCode::OK);

    let rebuilt_app = app_with_state(AppState::from_shared_db(db).expect("rebuilt app"));

    let account = read_account(&rebuilt_app, Some(&session_token)).await;
    assert_eq!(account.status(), StatusCode::OK);
    let account_body = response_json(account).await;
    assert_eq!(account_body["account"]["api_key_masked"], "demo****1234");
    assert_eq!(account_body["account"]["connection_status"], "healthy");

    let response = search_symbols(&rebuilt_app, Some(&session_token), "btc coin delivery").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["items"].as_array().expect("items").len(), 1);
    assert_eq!(body["items"][0]["symbol"], "BTCUSD_PERP");
    assert_eq!(body["items"][0]["market"], "coinm");
}

#[tokio::test]
async fn hedge_mode_validation_flags_mismatch_between_expectation_and_account_state() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("first app"));
    let session_token = register_and_login(&app, "exchange-hedge@example.com").await;

    let response = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-oneway-1234",
        "demo-secret",
        true,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["account"]["validation"]["can_read_spot"], true);
    assert_eq!(body["account"]["validation"]["can_read_usdm"], true);
    assert_eq!(body["account"]["validation"]["can_read_coinm"], true);
    assert_eq!(body["account"]["validation"]["api_connectivity_ok"], true);
    assert_eq!(body["account"]["validation"]["timestamp_in_sync"], true);
    assert_eq!(body["account"]["validation"]["hedge_mode_ok"], false);
    assert_eq!(body["account"]["connection_status"], "degraded");
    assert_eq!(body["account"]["sync_status"], "success");

    let rebuilt = app_with_state(AppState::from_shared_db(db).expect("rebuilt app"));
    let read = read_account(&rebuilt, Some(&session_token)).await;
    assert_eq!(read.status(), StatusCode::OK);
    let read_body = response_json(read).await;
    assert_eq!(read_body["account"]["validation"]["hedge_mode_ok"], false);
    assert_eq!(read_body["account"]["connection_status"], "degraded");
}

#[tokio::test]
async fn degraded_credentials_emit_api_invalidation_notification_without_manual_dispatch() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("app state"));
    let session_token = register_and_login(&app, "exchange-invalidated@example.com").await;

    let response = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-oneway-1234",
        "demo-secret",
        true,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let notifications = db
        .list_notification_logs("exchange-invalidated@example.com", 10)
        .expect("notification logs");
    let invalidation = notifications
        .iter()
        .find(|record| {
            record.template_key.as_deref() == Some("ApiCredentialsInvalidated")
                && record.channel == "in_app"
        })
        .expect("api invalidation notification");
    assert_eq!(invalidation.title, "API credentials invalid");
    assert_eq!(
        invalidation.payload["event"]["kind"],
        "ApiCredentialsInvalidated"
    );
    assert_eq!(
        invalidation.payload["event"]["payload"]["exchange"],
        "binance"
    );
    assert_eq!(
        invalidation.payload["event"]["payload"]["reason"],
        "hedge_mode"
    );
}

#[tokio::test]
async fn selected_markets_drive_timestamp_and_market_reachability_validation() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("app state"));
    let session_token = register_and_login(&app, "exchange-market-scope@example.com").await;

    let response = save_credentials_for_markets(
        &app,
        Some(&session_token),
        "demo-key-nocoinm-1234",
        "demo-secret-skew",
        true,
        &["spot", "coinm"],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(
        body["account"]["selected_markets"],
        json!(["spot", "coinm"])
    );
    assert_eq!(body["account"]["validation"]["api_connectivity_ok"], true);
    assert_eq!(body["account"]["validation"]["timestamp_in_sync"], false);
    assert_eq!(body["account"]["validation"]["can_read_spot"], true);
    assert_eq!(body["account"]["validation"]["can_read_usdm"], false);
    assert_eq!(body["account"]["validation"]["can_read_coinm"], false);
    assert_eq!(body["account"]["validation"]["market_access_ok"], false);
    assert_eq!(body["account"]["connection_status"], "degraded");
    assert_eq!(body["synced_symbols"], 2);
    assert_eq!(body["account"]["symbol_counts"]["spot"], 2);
    assert_eq!(body["account"]["symbol_counts"]["usdm"], 0);
    assert_eq!(body["account"]["symbol_counts"]["coinm"], 0);

    let rebuilt = app_with_state(AppState::from_shared_db(db).expect("rebuilt app"));
    let search = search_symbols(&rebuilt, Some(&session_token), "delivery").await;
    assert_eq!(search.status(), StatusCode::OK);
    assert_eq!(
        response_json(search).await["items"]
            .as_array()
            .expect("items")
            .len(),
        0
    );
}

#[tokio::test]
async fn persisted_symbol_metadata_includes_filters_and_market_requirements() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("app state"));
    let session_token = register_and_login(&app, "exchange-rich-symbols@example.com").await;

    let sync = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;
    assert_eq!(sync.status(), StatusCode::OK);

    let rebuilt = app_with_state(AppState::from_shared_db(db).expect("rebuilt app"));
    let search = search_symbols(&rebuilt, Some(&session_token), "btc spot").await;
    assert_eq!(search.status(), StatusCode::OK);
    let body = response_json(search).await;
    assert_eq!(body["items"][0]["symbol"], "BTCUSDT");
    assert_eq!(body["items"][0]["filters"]["price_tick_size"], "0.01");
    assert_eq!(
        body["items"][0]["filters"]["quantity_step_size"],
        "0.000010"
    );
    assert_eq!(body["items"][0]["filters"]["min_notional"], "5");
    assert_eq!(
        body["items"][0]["market_requirements"]["hedge_mode_required"],
        false
    );
    assert_eq!(
        body["items"][0]["market_requirements"]["supports_isolated_margin"],
        false
    );
}

#[tokio::test]
async fn empty_api_credentials_are_rejected() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let app = app();
    let session_token = register_and_login(&app, "exchange-empty-creds@example.com").await;

    let response = save_credentials(&app, Some(&session_token), "", "", true).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await["error"],
        "api_key and api_secret are required"
    );
}

#[tokio::test]
async fn empty_symbol_query_is_rejected() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let app = app();
    let session_token = register_and_login(&app, "exchange-empty-query@example.com").await;
    let sync = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;
    assert_eq!(sync.status(), StatusCode::OK);

    let response = search_symbols(&app, Some(&session_token), "   ").await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response_json(response).await["error"], "query is required");
}

#[tokio::test]
async fn unauthenticated_requests_to_exchange_api_are_rejected() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let app = app();

    let save_response = save_credentials(&app, None, "demo-key", "demo-secret", true).await;
    assert_eq!(save_response.status(), StatusCode::UNAUTHORIZED);

    let read_response = read_account(&app, None).await;
    assert_eq!(read_response.status(), StatusCode::UNAUTHORIZED);

    let search_response = search_symbols(&app, None, "btc").await;
    assert_eq!(search_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn invalid_selected_markets_are_rejected() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::set_var(
        "EXCHANGE_CREDENTIALS_MASTER_KEY",
        "exchange-flow-test-master-key",
    );
    let app = app();
    let session_token = register_and_login(&app, "exchange-invalid-markets@example.com").await;

    let empty = save_credentials_for_markets(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
        &[],
    )
    .await;
    assert_eq!(empty.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(empty).await["error"],
        "selected_markets must include at least one of spot, usdm, coinm"
    );

    let invalid = save_credentials_raw_markets(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
        json!(["spot", "margin"]),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(invalid).await["error"],
        "selected_markets contains unsupported market"
    );
}

#[tokio::test]
async fn exchange_credentials_master_key_is_required() {
    let _guard = exchange_env_lock().lock().expect("env lock");
    std::env::remove_var("EXCHANGE_CREDENTIALS_MASTER_KEY");
    std::env::remove_var("SESSION_TOKEN_SECRET");
    let app = app();
    let session_token = register_and_login(&app, "exchange-missing-key@example.com").await;

    let response = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        response_json(response).await["error"],
        "EXCHANGE_CREDENTIALS_MASTER_KEY is required"
    );

    let read = read_account(&app, Some(&session_token)).await;
    assert_eq!(read.status(), StatusCode::NOT_FOUND);
}

async fn read_account(app: &axum::Router, session_token: Option<&str>) -> axum::response::Response {
    let mut request = Request::builder()
        .method("GET")
        .uri("/exchange/binance/account");
    if let Some(session_token) = session_token {
        request = request.header("authorization", format!("Bearer {session_token}"));
    }

    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn save_credentials(
    app: &axum::Router,
    session_token: Option<&str>,
    api_key: &str,
    api_secret: &str,
    expected_hedge_mode: bool,
) -> axum::response::Response {
    save_credentials_for_markets(
        app,
        session_token,
        api_key,
        api_secret,
        expected_hedge_mode,
        &["spot", "usdm", "coinm"],
    )
    .await
}

async fn test_credentials(
    app: &axum::Router,
    session_token: Option<&str>,
    api_key: &str,
    api_secret: &str,
    expected_hedge_mode: bool,
) -> axum::response::Response {
    test_credentials_for_markets(
        app,
        session_token,
        api_key,
        api_secret,
        expected_hedge_mode,
        &["spot", "usdm", "coinm"],
    )
    .await
}

async fn test_credentials_for_markets(
    app: &axum::Router,
    session_token: Option<&str>,
    api_key: &str,
    api_secret: &str,
    expected_hedge_mode: bool,
    selected_markets: &[&str],
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri("/exchange/binance/credentials/test")
        .header("content-type", "application/json");
    if let Some(session_token) = session_token {
        request = request.header("authorization", format!("Bearer {session_token}"));
    }

    app.clone()
        .oneshot(
            request
                .body(Body::from(
                    json!({
                        "api_key": api_key,
                        "api_secret": api_secret,
                        "expected_hedge_mode": expected_hedge_mode,
                        "selected_markets": selected_markets,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn save_credentials_for_markets(
    app: &axum::Router,
    session_token: Option<&str>,
    api_key: &str,
    api_secret: &str,
    expected_hedge_mode: bool,
    selected_markets: &[&str],
) -> axum::response::Response {
    save_credentials_raw_markets(
        app,
        session_token,
        api_key,
        api_secret,
        expected_hedge_mode,
        json!(selected_markets),
    )
    .await
}

async fn save_credentials_raw_markets(
    app: &axum::Router,
    session_token: Option<&str>,
    api_key: &str,
    api_secret: &str,
    expected_hedge_mode: bool,
    selected_markets: Value,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri("/exchange/binance/credentials")
        .header("content-type", "application/json");
    if let Some(session_token) = session_token {
        request = request.header("authorization", format!("Bearer {session_token}"));
    }

    app.clone()
        .oneshot(
            request
                .body(Body::from(
                    json!({
                        "api_key": api_key,
                        "api_secret": api_secret,
                        "expected_hedge_mode": expected_hedge_mode,
                        "selected_markets": selected_markets,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn create_strategy(
    app: &axum::Router,
    session_token: &str,
    payload: Value,
) -> axum::response::Response {
    strategy_request(app, session_token, "POST", "/strategies", payload).await
}

async fn start_strategy(
    app: &axum::Router,
    session_token: &str,
    strategy_id: &str,
) -> axum::response::Response {
    strategy_request(
        app,
        session_token,
        "POST",
        &format!("/strategies/{strategy_id}/start"),
        json!({}),
    )
    .await
}

async fn pause_strategies(
    app: &axum::Router,
    session_token: &str,
    ids: &[&str],
) -> axum::response::Response {
    strategy_request(
        app,
        session_token,
        "POST",
        "/strategies/batch/pause",
        json!({ "ids": ids }),
    )
    .await
}

async fn strategy_request(
    app: &axum::Router,
    session_token: &str,
    method: &str,
    uri: &str,
    payload: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn search_symbols(
    app: &axum::Router,
    session_token: Option<&str>,
    query: &str,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri("/exchange/binance/symbols/search")
        .header("content-type", "application/json");
    if let Some(session_token) = session_token {
        request = request.header("authorization", format!("Bearer {session_token}"));
    }

    app.clone()
        .oneshot(
            request
                .body(Body::from(
                    json!({
                        "query": query,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn register_and_login(app: &axum::Router, email: &str) -> String {
    let register = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "password": "pass1234",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(register.status(), StatusCode::CREATED);
    let verification_code = response_json(register).await["verification_code"]
        .as_str()
        .expect("verification code")
        .to_owned();

    let verify = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/verify-email")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "code": verification_code,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(verify.status(), StatusCode::OK);

    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "password": "pass1234",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);

    response_json(login).await["session_token"]
        .as_str()
        .expect("session token")
        .to_owned()
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("valid json")
}

fn exchange_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
