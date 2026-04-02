use api_server::{app, app_with_state, AppState};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use shared_db::SharedDb;
use tower::ServiceExt;

mod support;

use support::register_and_login;

const BOT_BIND_SECRET: &str = "grid-binance-dev-telegram-bot-bind-secret";
const WRONG_BOT_BIND_SECRET: &str = "wrong-bot-secret";

#[tokio::test]
async fn bot_side_binding_and_expanded_notification_payloads_are_logged_durably() {
    let app = app();
    let session_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let bind_code = create_bind_code(&app, &session_token, "trader@example.com", None).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let bind_code_body = response_json(bind_code).await;
    let code = bind_code_body["code"].as_str().expect("bind code").to_string();

    let bound = bot_bind_telegram(&app, &code, "tg-user-9001", "chat-9001", Some("gridtrader")).await;
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
    assert_eq!(api_invalidated_body["event"]["kind"], "ApiCredentialsInvalidated");
    assert_eq!(api_invalidated_body["event"]["payload"]["exchange"], "binance");
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
    assert_eq!(tp_notice_body["event"]["kind"], "OverallTakeProfitTriggered");
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
    assert_eq!(sl_notice_body["event"]["payload"]["strategy_id"], "strategy-beta");

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
    assert_eq!(runtime_body["event"]["payload"]["reason"], "exchange connectivity lost");

    let inbox = list_notifications(&app, &session_token, "trader@example.com").await;
    assert_eq!(inbox.status(), StatusCode::OK);
    let inbox_body = response_json(inbox).await;
    let items = inbox_body["items"].as_array().expect("notification items");
    assert_eq!(items.len(), 7);
    assert_eq!(items[0]["event"]["kind"], "ApiCredentialsInvalidated");
    assert_eq!(items[1]["event"]["kind"], "GridFillExecuted");
    assert_eq!(items[2]["event"]["kind"], "FillProfitReported");
    assert_eq!(items[3]["event"]["kind"], "OverallTakeProfitTriggered");
    assert_eq!(items[4]["event"]["kind"], "OverallStopLossTriggered");
    assert_eq!(items[5]["event"]["kind"], "MembershipExpiring");
    assert_eq!(items[6]["event"]["kind"], "RuntimeError");
}

#[tokio::test]
async fn telegram_bindings_and_inbox_items_survive_app_rebuilds_via_bot_binding_path() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let first_app = app_with_state(AppState::from_shared_db(db.clone()).expect("first state"));
    let session_token = register_and_login(&first_app, "durable@example.com", "pass1234").await;

    let bind_code = create_bind_code(&first_app, &session_token, "durable@example.com", None).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let code = response_json(bind_code).await["code"]
        .as_str()
        .expect("bind code")
        .to_string();

    let bound = bot_bind_telegram(&first_app, &code, "tg-durable", "chat-durable", Some("durable_user")).await;
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
    assert_eq!(response_json(first_dispatch).await["telegram_delivered"], true);

    let rebuilt_app = app_with_state(AppState::from_shared_db(db).expect("rebuilt state"));

    let inbox = list_notifications(&rebuilt_app, &session_token, "durable@example.com").await;
    assert_eq!(inbox.status(), StatusCode::OK);
    let items = response_json(inbox).await["items"].as_array().expect("items").clone();
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
    assert_eq!(second_dispatch_body["event"]["payload"]["trigger_price"], "48");
}

#[tokio::test]
async fn bot_bind_requires_internal_secret_and_direct_user_bind_is_forbidden() {
    let app = app();
    let session_token = register_and_login(&app, "bot-auth@example.com", "pass1234").await;

    let bind_code = create_bind_code(&app, &session_token, "bot-auth@example.com", None).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let code = response_json(bind_code).await["code"].as_str().expect("bind code").to_string();

    let forbidden_direct = direct_user_bind_telegram(&app, &session_token, &code, "chat-direct").await;
    assert_eq!(forbidden_direct.status(), StatusCode::FORBIDDEN);

    let missing_secret = bot_bind_telegram_with_secret(&app, &code, "tg-user-auth", "chat-auth", Some("botuser"), None).await;
    assert_eq!(missing_secret.status(), StatusCode::UNAUTHORIZED);

    let wrong_secret = bot_bind_telegram_with_secret(&app, &code, "tg-user-auth", "chat-auth", Some("botuser"), Some(WRONG_BOT_BIND_SECRET)).await;
    assert_eq!(wrong_secret.status(), StatusCode::UNAUTHORIZED);

    let accepted = bot_bind_telegram_with_secret(&app, &code, "tg-user-auth", "chat-auth", Some("botuser"), Some(BOT_BIND_SECRET)).await;
    assert_eq!(accepted.status(), StatusCode::OK);
}

#[tokio::test]
async fn previous_bind_code_is_rejected_after_regenerating_for_same_email() {
    let app = app();
    let session_token = register_and_login(&app, "rotate@example.com", "pass1234").await;

    let first = create_bind_code(&app, &session_token, "rotate@example.com", None).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_body = response_json(first).await;
    let first_code = first_body["code"].as_str().expect("first code").to_string();

    let second = create_bind_code(&app, &session_token, "rotate@example.com", None).await;
    assert_eq!(second.status(), StatusCode::CREATED);
    let second_body = response_json(second).await;
    let second_code = second_body["code"].as_str().expect("second code").to_string();
    assert_ne!(first_code, second_code);

    let rejected = bot_bind_telegram(&app, &first_code, "tg-old", "chat-old", None).await;
    assert_eq!(rejected.status(), StatusCode::NOT_FOUND);
    assert_eq!(response_json(rejected).await["error"], "bind code not found");

    let accepted = bot_bind_telegram(&app, &second_code, "tg-new", "chat-new", Some("rotate_user")).await;
    assert_eq!(accepted.status(), StatusCode::OK);
    let accepted_body = response_json(accepted).await;
    assert_eq!(accepted_body["email"], "rotate@example.com");
    assert_eq!(accepted_body["chat_id"], "chat-new");
}

#[tokio::test]
async fn expired_bind_code_is_rejected() {
    let app = app();
    let session_token = register_and_login(&app, "expired-bind@example.com", "pass1234").await;

    let bind_code = create_bind_code(&app, &session_token, "expired-bind@example.com", Some(0)).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let code = response_json(bind_code).await["code"].as_str().expect("expired code").to_string();

    let rejected = bot_bind_telegram(&app, &code, "tg-expired", "chat-expired", None).await;
    assert_eq!(rejected.status(), StatusCode::NOT_FOUND);
    assert_eq!(response_json(rejected).await["error"], "bind code expired");
}

#[tokio::test]
async fn unbound_email_keeps_telegram_delivery_disabled_for_api_invalidation() {
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
async fn invalid_ttl_returns_bad_request_instead_of_panicking() {
    let app = app();
    let session_token = register_and_login(&app, "invalid-ttl@example.com", "pass1234").await;

    let response = create_bind_code(&app, &session_token, "invalid-ttl@example.com", Some(i64::MAX)).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response_json(response).await["error"], "ttl_seconds must be between 0 and 86400");
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
    bot_bind_telegram_with_secret(app, code, telegram_user_id, chat_id, username, Some(BOT_BIND_SECRET)).await
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

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("response body");
    serde_json::from_slice(&bytes).expect("valid json")
}
