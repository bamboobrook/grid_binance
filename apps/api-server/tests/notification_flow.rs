use api_server::{app, app_with_state, AppState};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use shared_db::{
    ExchangeWalletSnapshotRecord, MembershipRecord, SharedDb, UserExchangeAccountRecord,
    UserExchangeSymbolRecord,
};
use std::{
    collections::VecDeque,
    env,
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex, OnceLock},
    thread,
};
use tower::ServiceExt;

mod support;

use support::register_and_login;

const BOT_BIND_SECRET: &str = "grid-binance-dev-telegram-bot-bind-secret";
const WRONG_BOT_BIND_SECRET: &str = "wrong-bot-secret";

#[tokio::test]
async fn bot_side_binding_and_expanded_notification_payloads_are_logged_durably() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("TELEGRAM_BOT_TOKEN");
    std::env::remove_var("TELEGRAM_API_BASE_URL");
    let app = app();
    let session_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let bind_code = create_bind_code(&app, &session_token, "trader@example.com", None).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let bind_code_body = response_json(bind_code).await;
    let code = bind_code_body["code"]
        .as_str()
        .expect("bind code")
        .to_string();

    let bound =
        bot_bind_telegram(&app, &code, "tg-user-9001", "chat-9001", Some("gridtrader")).await;
    assert_eq!(bound.status(), StatusCode::OK);
    let bound_body = response_json(bound).await;
    assert_eq!(bound_body["email"], "trader@example.com");
    assert_eq!(bound_body["chat_id"], "chat-9001");
    assert_eq!(bound_body["telegram_user_id"], "tg-user-9001");
    assert_eq!(bound_body["username"], "gridtrader");

    let api_invalidated = dispatch_notification(
        &app,
        &session_token,
        "trader@example.com",
        "ApiCredentialsInvalidated",
        "API credentials invalid",
        "Binance rejected the API key during validation.",
        json!({
            "exchange": "binance",
            "reason": "timestamp drift",
        }),
    )
    .await;
    assert_eq!(api_invalidated.status(), StatusCode::OK);
    let api_invalidated_body = response_json(api_invalidated).await;
    assert_eq!(
        api_invalidated_body["event"]["kind"],
        "ApiCredentialsInvalidated"
    );
    assert_eq!(
        api_invalidated_body["event"]["payload"]["exchange"],
        "binance"
    );
    assert_eq!(api_invalidated_body["telegram_delivered"], true);

    let fill_notice = dispatch_notification(
        &app,
        &session_token,
        "trader@example.com",
        "GridFillExecuted",
        "Grid fill executed",
        "BTCUSDT grid filled at 110.",
        json!({
            "strategy_id": "strategy-alpha",
            "symbol": "BTCUSDT",
            "fill_id": "fill-1",
            "price": "110",
            "quantity": "1",
        }),
    )
    .await;
    assert_eq!(fill_notice.status(), StatusCode::OK);
    let fill_notice_body = response_json(fill_notice).await;
    assert_eq!(fill_notice_body["event"]["kind"], "GridFillExecuted");
    assert_eq!(fill_notice_body["event"]["payload"]["fill_id"], "fill-1");

    let pnl_notice = dispatch_notification(
        &app,
        &session_token,
        "trader@example.com",
        "FillProfitReported",
        "Fill profit update",
        "Grid fill realized +9 USDT net PnL.",
        json!({
            "strategy_id": "strategy-alpha",
            "fill_id": "fill-1",
            "realized_pnl": "10",
            "net_pnl": "9",
        }),
    )
    .await;
    assert_eq!(pnl_notice.status(), StatusCode::OK);
    let pnl_notice_body = response_json(pnl_notice).await;
    assert_eq!(pnl_notice_body["event"]["kind"], "FillProfitReported");
    assert_eq!(pnl_notice_body["event"]["payload"]["net_pnl"], "9");

    let tp_notice = dispatch_notification(
        &app,
        &session_token,
        "trader@example.com",
        "OverallTakeProfitTriggered",
        "Overall take profit reached",
        "Strategy closed after hitting the configured TP.",
        json!({
            "strategy_id": "strategy-alpha",
            "trigger_price": "112",
        }),
    )
    .await;
    assert_eq!(tp_notice.status(), StatusCode::OK);
    let tp_notice_body = response_json(tp_notice).await;
    assert_eq!(
        tp_notice_body["event"]["kind"],
        "OverallTakeProfitTriggered"
    );
    assert_eq!(tp_notice_body["event"]["payload"]["trigger_price"], "112");

    let sl_notice = dispatch_notification(
        &app,
        &session_token,
        "trader@example.com",
        "OverallStopLossTriggered",
        "Overall stop loss reached",
        "Strategy closed after hitting the configured SL.",
        json!({
            "strategy_id": "strategy-beta",
            "trigger_price": "95",
        }),
    )
    .await;
    assert_eq!(sl_notice.status(), StatusCode::OK);
    let sl_notice_body = response_json(sl_notice).await;
    assert_eq!(sl_notice_body["event"]["kind"], "OverallStopLossTriggered");
    assert_eq!(
        sl_notice_body["event"]["payload"]["strategy_id"],
        "strategy-beta"
    );

    let membership = dispatch_notification(
        &app,
        &session_token,
        "trader@example.com",
        "MembershipExpiring",
        "Membership ending soon",
        "Renew within 3 days to avoid strategy interruption.",
        json!({ "grace_hours": 48 }),
    )
    .await;
    assert_eq!(membership.status(), StatusCode::OK);
    let membership_body = response_json(membership).await;
    assert_eq!(membership_body["event"]["kind"], "MembershipExpiring");
    assert_eq!(membership_body["show_expiry_popup"], true);

    let runtime = dispatch_notification(
        &app,
        &session_token,
        "trader@example.com",
        "RuntimeError",
        "Runtime failure",
        "Grid runtime lost exchange connectivity.",
        json!({
            "strategy_id": "strategy-beta",
            "reason": "exchange connectivity lost",
        }),
    )
    .await;
    assert_eq!(runtime.status(), StatusCode::OK);
    let runtime_body = response_json(runtime).await;
    assert_eq!(runtime_body["event"]["kind"], "RuntimeError");
    assert_eq!(
        runtime_body["event"]["payload"]["reason"],
        "exchange connectivity lost"
    );

    let inbox = list_notifications(&app, &session_token, "trader@example.com").await;
    assert_eq!(inbox.status(), StatusCode::OK);
    let inbox_body = response_json(inbox).await;
    let items = inbox_body["items"].as_array().expect("notification items");
    assert_eq!(items.len(), 7);
    assert!(items[0]["created_at"].is_string());
    assert!(items[0]["event"]["message"].is_string());
    assert_eq!(items[0]["event"]["kind"], "ApiCredentialsInvalidated");
    assert_eq!(items[1]["event"]["kind"], "GridFillExecuted");
    assert_eq!(items[2]["event"]["kind"], "FillProfitReported");
    assert_eq!(items[3]["event"]["kind"], "OverallTakeProfitTriggered");
    assert_eq!(items[4]["event"]["kind"], "OverallStopLossTriggered");
    assert_eq!(items[5]["event"]["kind"], "MembershipExpiring");
    assert_eq!(items[6]["event"]["kind"], "RuntimeError");
}

#[tokio::test]
async fn strategy_start_and_pause_do_not_emit_telegram_logs_when_bound_and_configured() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("TELEGRAM_BOT_TOKEN");
    std::env::remove_var("TELEGRAM_API_BASE_URL");
    let db = SharedDb::ephemeral().expect("db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let session_token = register_and_login(&app, "auto-telegram@example.com", "pass1234").await;

    seed_strategy_start_prerequisites(&db, "auto-telegram@example.com", "BTCUSDT");

    let bind_code = create_bind_code(&app, &session_token, "auto-telegram@example.com", None).await;
    let code = response_json(bind_code).await["code"]
        .as_str()
        .unwrap()
        .to_string();
    let bound = bot_bind_telegram(&app, &code, "tg-auto", "chat-auto", Some("autobot")).await;
    assert_eq!(bound.status(), StatusCode::OK);

    let created = request(
        &app,
        Some(&session_token),
        "POST",
        "/strategies",
        json!({
            "name": "auto-start-telegram",
            "symbol": "BTCUSDT",
            "market": "Spot",
            "mode": "SpotBuyOnly",
            "generation": "Custom",
            "strategy_type": "ordinary_grid",
            "reference_price_source": "market",
            "levels": [{ "entry_price": "100.00", "quantity": "0.0100", "take_profit_bps": 120, "trailing_bps": null }],
            "membership_ready": true,
            "exchange_ready": true,
            "permissions_ready": true,
            "withdrawals_disabled": true,
            "hedge_mode_ready": true,
            "symbol_ready": true,
            "filters_ready": true,
            "margin_ready": true,
            "conflict_ready": true,
            "balance_ready": true,
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    ).await;
    let strategy_id = response_json(created).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let started = request(
        &app,
        Some(&session_token),
        "POST",
        &format!("/strategies/{strategy_id}/start"),
        Value::Null,
    )
    .await;
    assert_eq!(started.status(), StatusCode::OK);
    let paused = request(
        &app,
        Some(&session_token),
        "POST",
        "/strategies/batch/pause",
        json!({ "ids": [strategy_id] }),
    )
    .await;
    assert_eq!(paused.status(), StatusCode::OK);

    let logs = db
        .list_notification_logs("auto-telegram@example.com", 20)
        .expect("logs");
    assert!(!logs.iter().any(|record| record.channel == "telegram"
        && record.template_key.as_deref() == Some("StrategyStarted")));
    assert!(!logs.iter().any(|record| record.channel == "telegram"
        && record.template_key.as_deref() == Some("StrategyPaused")));
}

#[tokio::test]
async fn strategy_start_and_pause_emit_notifications_automatically() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("TELEGRAM_BOT_TOKEN");
    std::env::remove_var("TELEGRAM_API_BASE_URL");
    let db = SharedDb::ephemeral().expect("db");
    seed_strategy_start_prerequisites(&db, "auto-strategy@example.com", "BTCUSDT");
    let app = app_with_state(AppState::from_shared_db(db).expect("state"));
    let session_token = register_and_login(&app, "auto-strategy@example.com", "pass1234").await;

    let created = request(
        &app,
        Some(&session_token),
        "POST",
        "/strategies",
        json!({
            "name": "auto-start",
            "symbol": "BTCUSDT",
            "market": "Spot",
            "mode": "SpotBuyOnly",
            "generation": "Custom",
            "strategy_type": "ordinary_grid",
            "reference_price_source": "market",
            "levels": [{ "entry_price": "100.00", "quantity": "0.0100", "take_profit_bps": 120, "trailing_bps": null }],
            "membership_ready": true,
            "exchange_ready": true,
            "permissions_ready": true,
            "withdrawals_disabled": true,
            "hedge_mode_ready": true,
            "symbol_ready": true,
            "filters_ready": true,
            "margin_ready": true,
            "conflict_ready": true,
            "balance_ready": true,
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    ).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let strategy_id = response_json(created).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let started = request(
        &app,
        Some(&session_token),
        "POST",
        &format!("/strategies/{strategy_id}/start"),
        Value::Null,
    )
    .await;
    assert_eq!(started.status(), StatusCode::OK);
    let paused = request(
        &app,
        Some(&session_token),
        "POST",
        "/strategies/batch/pause",
        json!({ "ids": [strategy_id] }),
    )
    .await;
    assert_eq!(paused.status(), StatusCode::OK);

    let inbox = list_notifications(&app, &session_token, "auto-strategy@example.com").await;
    assert_eq!(inbox.status(), StatusCode::OK);
    let items = response_json(inbox).await["items"]
        .as_array()
        .expect("items")
        .clone();
    assert!(items
        .iter()
        .any(|item| item["event"]["kind"] == "StrategyStarted"));
    assert!(items
        .iter()
        .any(|item| item["event"]["kind"] == "StrategyPaused"));
}

#[tokio::test]
async fn telegram_bindings_and_inbox_items_survive_app_rebuilds_via_bot_binding_path() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("TELEGRAM_BOT_TOKEN");
    std::env::remove_var("TELEGRAM_API_BASE_URL");
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let first_app = app_with_state(AppState::from_shared_db(db.clone()).expect("first state"));
    let session_token = register_and_login(&first_app, "durable@example.com", "pass1234").await;

    let bind_code = create_bind_code(&first_app, &session_token, "durable@example.com", None).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let code = response_json(bind_code).await["code"]
        .as_str()
        .expect("bind code")
        .to_string();

    let bound = bot_bind_telegram(
        &first_app,
        &code,
        "tg-durable",
        "chat-durable",
        Some("durable_user"),
    )
    .await;
    assert_eq!(bound.status(), StatusCode::OK);

    let first_dispatch = dispatch_notification(
        &first_app,
        &session_token,
        "durable@example.com",
        "GridFillExecuted",
        "Grid fill executed",
        "ETHUSDT grid filled at 55.",
        json!({
            "strategy_id": "strategy-gamma",
            "fill_id": "durable-fill-1",
            "price": "55",
        }),
    )
    .await;
    assert_eq!(first_dispatch.status(), StatusCode::OK);
    assert_eq!(
        response_json(first_dispatch).await["telegram_delivered"],
        true
    );

    let rebuilt_app = app_with_state(AppState::from_shared_db(db).expect("rebuilt state"));

    let inbox = list_notifications(&rebuilt_app, &session_token, "durable@example.com").await;
    assert_eq!(inbox.status(), StatusCode::OK);
    let items = response_json(inbox).await["items"]
        .as_array()
        .expect("items")
        .clone();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["event"]["kind"], "GridFillExecuted");
    assert_eq!(items[0]["event"]["payload"]["fill_id"], "durable-fill-1");
    assert_eq!(items[0]["telegram_delivered"], true);

    let second_dispatch = dispatch_notification(
        &rebuilt_app,
        &session_token,
        "durable@example.com",
        "OverallStopLossTriggered",
        "Overall stop loss reached",
        "Strategy closed after hitting stop loss.",
        json!({
            "strategy_id": "strategy-gamma",
            "trigger_price": "48",
        }),
    )
    .await;
    assert_eq!(second_dispatch.status(), StatusCode::OK);
    let second_dispatch_body = response_json(second_dispatch).await;
    assert_eq!(second_dispatch_body["telegram_delivered"], true);
    assert_eq!(
        second_dispatch_body["event"]["payload"]["trigger_price"],
        "48"
    );
}

#[tokio::test]
async fn bot_bind_requires_internal_secret_and_direct_user_bind_is_forbidden() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("TELEGRAM_BOT_TOKEN");
    std::env::remove_var("TELEGRAM_API_BASE_URL");
    let app = app();
    let session_token = register_and_login(&app, "bot-auth@example.com", "pass1234").await;

    let bind_code = create_bind_code(&app, &session_token, "bot-auth@example.com", None).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let code = response_json(bind_code).await["code"]
        .as_str()
        .expect("bind code")
        .to_string();

    let forbidden_direct =
        direct_user_bind_telegram(&app, &session_token, &code, "chat-direct").await;
    assert_eq!(forbidden_direct.status(), StatusCode::FORBIDDEN);

    let missing_secret = bot_bind_telegram_with_secret(
        &app,
        &code,
        "tg-user-auth",
        "chat-auth",
        Some("botuser"),
        None,
    )
    .await;
    assert_eq!(missing_secret.status(), StatusCode::UNAUTHORIZED);

    let wrong_secret = bot_bind_telegram_with_secret(
        &app,
        &code,
        "tg-user-auth",
        "chat-auth",
        Some("botuser"),
        Some(WRONG_BOT_BIND_SECRET),
    )
    .await;
    assert_eq!(wrong_secret.status(), StatusCode::UNAUTHORIZED);

    let accepted = bot_bind_telegram_with_secret(
        &app,
        &code,
        "tg-user-auth",
        "chat-auth",
        Some("botuser"),
        Some(BOT_BIND_SECRET),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
}

#[tokio::test]
async fn previous_bind_code_is_rejected_after_regenerating_for_same_email() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("TELEGRAM_BOT_TOKEN");
    std::env::remove_var("TELEGRAM_API_BASE_URL");
    let app = app();
    let session_token = register_and_login(&app, "rotate@example.com", "pass1234").await;

    let first = create_bind_code(&app, &session_token, "rotate@example.com", None).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_body = response_json(first).await;
    let first_code = first_body["code"].as_str().expect("first code").to_string();

    let second = create_bind_code(&app, &session_token, "rotate@example.com", None).await;
    assert_eq!(second.status(), StatusCode::CREATED);
    let second_body = response_json(second).await;
    let second_code = second_body["code"]
        .as_str()
        .expect("second code")
        .to_string();
    assert_ne!(first_code, second_code);

    let rejected = bot_bind_telegram(&app, &first_code, "tg-old", "chat-old", None).await;
    assert_eq!(rejected.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response_json(rejected).await["error"],
        "bind code not found"
    );

    let accepted = bot_bind_telegram(
        &app,
        &second_code,
        "tg-new",
        "chat-new",
        Some("rotate_user"),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    let accepted_body = response_json(accepted).await;
    assert_eq!(accepted_body["email"], "rotate@example.com");
    assert_eq!(accepted_body["chat_id"], "chat-new");
}

#[tokio::test]
async fn expired_bind_code_is_rejected() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("TELEGRAM_BOT_TOKEN");
    std::env::remove_var("TELEGRAM_API_BASE_URL");
    let app = app();
    let session_token = register_and_login(&app, "expired-bind@example.com", "pass1234").await;

    let bind_code =
        create_bind_code(&app, &session_token, "expired-bind@example.com", Some(0)).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let code = response_json(bind_code).await["code"]
        .as_str()
        .expect("expired code")
        .to_string();

    let rejected = bot_bind_telegram(&app, &code, "tg-expired", "chat-expired", None).await;
    assert_eq!(rejected.status(), StatusCode::NOT_FOUND);
    assert_eq!(response_json(rejected).await["error"], "bind code expired");
}

#[tokio::test]
async fn unbound_email_keeps_telegram_delivery_disabled_for_api_invalidation() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("TELEGRAM_BOT_TOKEN");
    std::env::remove_var("TELEGRAM_API_BASE_URL");
    let app = app();
    let session_token = register_and_login(&app, "unbound@example.com", "pass1234").await;

    let runtime = dispatch_notification(
        &app,
        &session_token,
        "unbound@example.com",
        "ApiCredentialsInvalidated",
        "API invalidated",
        "Binance rejected the API key.",
        json!({"exchange": "binance"}),
    )
    .await;
    assert_eq!(runtime.status(), StatusCode::OK);
    let runtime_body = response_json(runtime).await;
    assert_eq!(runtime_body["event"]["kind"], "ApiCredentialsInvalidated");
    assert_eq!(runtime_body["telegram_delivered"], false);
    assert_eq!(runtime_body["in_app_delivered"], true);
}

#[tokio::test]
async fn dispatch_notification_sends_real_telegram_message_when_bot_is_configured() {
    let _guard = env_lock().lock().unwrap();
    let _bot_token = set_env("TELEGRAM_BOT_TOKEN", "bot-test-token");
    let server = spawn_test_server(vec![TestRoute {
        path_prefix: "/botbot-test-token/sendMessage",
        status_line: "HTTP/1.1 200 OK",
        body: r#"{"ok":true,"result":{"message_id":1}}"#,
    }]);
    let _api_base = set_env("TELEGRAM_API_BASE_URL", &server.base_url);
    let db = SharedDb::ephemeral().expect("db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let session_token = register_and_login(&app, "telegram-live@example.com", "pass1234").await;

    let bind_code = create_bind_code(&app, &session_token, "telegram-live@example.com", None).await;
    let code = response_json(bind_code).await["code"]
        .as_str()
        .unwrap()
        .to_string();
    let bound = bot_bind_telegram(&app, &code, "tg-live", "chat-live", Some("gridlive")).await;
    assert_eq!(bound.status(), StatusCode::OK);

    let dispatched = dispatch_notification(
        &app,
        &session_token,
        "telegram-live@example.com",
        "GridFillExecuted",
        "Grid fill executed",
        "BTCUSDT grid filled at 110.",
        json!({"fill_id": "fill-live-1"}),
    )
    .await;
    assert_eq!(dispatched.status(), StatusCode::OK);
    let body = response_json(dispatched).await;
    assert_eq!(body["telegram_delivered"], true);

    let logs = db
        .list_notification_logs("telegram-live@example.com", 10)
        .expect("logs");
    assert!(logs
        .iter()
        .any(|record| record.channel == "telegram" && record.status == "delivered"));
}

#[tokio::test]
async fn invalid_ttl_returns_bad_request_instead_of_panicking() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("TELEGRAM_BOT_TOKEN");
    std::env::remove_var("TELEGRAM_API_BASE_URL");
    let app = app();
    let session_token = register_and_login(&app, "invalid-ttl@example.com", "pass1234").await;

    let response = create_bind_code(
        &app,
        &session_token,
        "invalid-ttl@example.com",
        Some(i64::MAX),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await["error"],
        "ttl_seconds must be between 0 and 86400"
    );
}

async fn create_bind_code(
    app: &axum::Router,
    session_token: &str,
    email: &str,
    ttl_seconds: Option<i64>,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/telegram/bind-codes")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "ttl_seconds": ttl_seconds,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn direct_user_bind_telegram(
    app: &axum::Router,
    session_token: &str,
    code: &str,
    chat_id: &str,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/telegram/bind")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": code,
                        "chat_id": chat_id,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn bot_bind_telegram(
    app: &axum::Router,
    code: &str,
    telegram_user_id: &str,
    chat_id: &str,
    username: Option<&str>,
) -> axum::response::Response {
    bot_bind_telegram_with_secret(
        app,
        code,
        telegram_user_id,
        chat_id,
        username,
        Some(BOT_BIND_SECRET),
    )
    .await
}

async fn bot_bind_telegram_with_secret(
    app: &axum::Router,
    code: &str,
    telegram_user_id: &str,
    chat_id: &str,
    username: Option<&str>,
    bot_secret: Option<&str>,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/telegram/bot/bind")
        .header("content-type", "application/json");
    if let Some(bot_secret) = bot_secret {
        builder = builder.header("x-telegram-bot-secret", bot_secret);
    }
    app.clone()
        .oneshot(
            builder
                .body(Body::from(
                    json!({
                        "code": code,
                        "telegram_user_id": telegram_user_id,
                        "chat_id": chat_id,
                        "username": username,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn dispatch_notification(
    app: &axum::Router,
    session_token: &str,
    email: &str,
    kind: &str,
    title: &str,
    message: &str,
    payload: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/notifications/dispatch")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "kind": kind,
                        "title": title,
                        "message": message,
                        "payload": payload,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn list_notifications(
    app: &axum::Router,
    session_token: &str,
    email: &str,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/notifications?email={email}"))
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn request(
    app: &axum::Router,
    session_token: Option<&str>,
    method: &str,
    uri: &str,
    payload: Value,
) -> axum::response::Response {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(session_token) = session_token {
        builder = builder.header("authorization", format!("Bearer {session_token}"));
    }
    if payload != Value::Null {
        builder = builder.header("content-type", "application/json");
    }
    let body = if payload == Value::Null {
        Body::empty()
    } else {
        Body::from(payload.to_string())
    };
    app.clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap()
}

#[derive(Clone)]
struct TestRoute {
    path_prefix: &'static str,
    status_line: &'static str,
    body: &'static str,
}

struct TestServer {
    base_url: String,
    join_handle: Option<thread::JoinHandle<()>>,
}

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => env::set_var(self.key, value),
            None => env::remove_var(self.key),
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(handle) = self.join_handle.take() {
            handle
                .join()
                .expect("telegram test server thread should exit cleanly");
        }
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn set_env(key: &'static str, value: impl Into<String>) -> EnvGuard {
    let previous = env::var(key).ok();
    env::set_var(key, value.into());
    EnvGuard { key, previous }
}

fn spawn_test_server(routes: Vec<TestRoute>) -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local telegram test server");
    let address = listener.local_addr().expect("telegram test server address");
    let queue = Arc::new(Mutex::new(VecDeque::from(routes)));
    let queue_for_thread = queue.clone();
    let join_handle = thread::spawn(move || {
        while let Some(route) = queue_for_thread
            .lock()
            .expect("telegram route queue poisoned")
            .pop_front()
        {
            let (mut stream, _) = listener.accept().expect("accept telegram test request");
            let mut buffer = [0u8; 4096];
            let read = stream
                .read(&mut buffer)
                .expect("read telegram test request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .expect("telegram request path");
            assert!(
                path.starts_with(route.path_prefix),
                "expected path prefix {} but received {}",
                route.path_prefix,
                path
            );
            let response = format!(
                "{}
content-type: application/json
content-length: {}
connection: close

{}",
                route.status_line,
                route.body.len(),
                route.body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write telegram test response");
        }
    });
    TestServer {
        base_url: format!("http://{}", address),
        join_handle: Some(join_handle),
    }
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("valid json")
}

fn seed_strategy_start_prerequisites(db: &SharedDb, email: &str, symbol: &str) {
    let now = Utc::now();
    db.upsert_membership_record(
        email,
        &MembershipRecord {
            activated_at: Some(now),
            active_until: Some(now + Duration::days(30)),
            grace_until: Some(now + Duration::days(32)),
            override_status: None,
        },
    )
    .expect("seed membership");

    let symbol_record = UserExchangeSymbolRecord {
        user_email: email.to_string(),
        exchange: "binance".to_string(),
        market: "spot".to_string(),
        symbol: symbol.to_string(),
        status: "TRADING".to_string(),
        base_asset: symbol.trim_end_matches("USDT").to_string(),
        quote_asset: "USDT".to_string(),
        price_precision: 2,
        quantity_precision: 4,
        min_quantity: "0.001".to_string(),
        min_notional: "0.5".to_string(),
        keywords: vec![symbol.to_lowercase(), "spot".to_string()],
        metadata: json!({
            "symbol": symbol,
            "market": "spot",
            "status": "TRADING",
            "base_asset": symbol.trim_end_matches("USDT"),
            "quote_asset": "USDT",
            "price_precision": 2,
            "quantity_precision": 4,
            "filters": {
                "price_tick_size": "0.01",
                "quantity_step_size": "0.001",
                "min_quantity": "0.001",
                "min_notional": "0.5",
                "contract_size": null
            },
            "market_requirements": {
                "supports_isolated_margin": true,
                "supports_cross_margin": true,
                "hedge_mode_required": false,
                "requires_futures_permissions": false,
                "leverage_brackets": [1, 5, 10]
            },
            "keywords": [symbol.to_lowercase(), "spot"]
        }),
        synced_at: now,
    };

    db.upsert_exchange_account(&UserExchangeAccountRecord {
        user_email: email.to_string(),
        exchange: "binance".to_string(),
        account_label: "Binance".to_string(),
        market_scope: "spot".to_string(),
        is_active: true,
        checked_at: Some(now),
        metadata: json!({
            "connection_status": "connected",
            "sync_status": "success",
            "last_synced_at": now.to_rfc3339(),
            "expected_hedge_mode": false,
            "selected_markets": ["spot"],
            "validation": {
                "api_connectivity_ok": true,
                "timestamp_in_sync": true,
                "can_read_spot": true,
                "can_read_usdm": false,
                "can_read_coinm": false,
                "hedge_mode_ok": true,
                "permissions_ok": true,
                "withdrawals_disabled": true,
                "market_access_ok": true
            }
        }),
    })
    .expect("seed exchange account");
    db.replace_exchange_symbols(email, "binance", &[symbol_record])
        .expect("seed symbols");
    db.insert_exchange_wallet_snapshot(&ExchangeWalletSnapshotRecord {
        user_email: email.to_string(),
        exchange: "binance".to_string(),
        wallet_type: "spot".to_string(),
        balances: json!({ "USDT": "1000" }),
        captured_at: now,
    })
    .expect("seed wallet snapshot");
}
