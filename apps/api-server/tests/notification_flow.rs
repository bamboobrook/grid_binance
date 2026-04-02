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

#[tokio::test]
async fn bind_telegram_and_dispatch_runtime_membership_alerts() {
    let app = app();
    let session_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let bind_code = create_bind_code(&app, &session_token, "trader@example.com", None).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let bind_code_body = response_json(bind_code).await;
    let code = bind_code_body["code"]
        .as_str()
        .expect("bind code")
        .to_string();
    assert_eq!(bind_code_body["email"], "trader@example.com");

    let bound = bind_telegram(&app, &session_token, &code, "chat-9001").await;
    assert_eq!(bound.status(), StatusCode::OK);
    let bound_body = response_json(bound).await;
    assert_eq!(bound_body["email"], "trader@example.com");
    assert_eq!(bound_body["chat_id"], "chat-9001");

    let deposit = dispatch_notification(
        &app,
        &session_token,
        "trader@example.com",
        "DepositConfirmed",
        "Deposit received",
        "USDT top-up matched successfully.",
    )
    .await;
    assert_eq!(deposit.status(), StatusCode::OK);
    let deposit_body = response_json(deposit).await;
    assert_eq!(deposit_body["event"]["kind"], "DepositConfirmed");
    assert_eq!(deposit_body["telegram_delivered"], true);
    assert_eq!(deposit_body["in_app_delivered"], true);
    assert_eq!(deposit_body["show_expiry_popup"], false);

    let membership = dispatch_notification(
        &app,
        &session_token,
        "trader@example.com",
        "MembershipExpiring",
        "Membership ending soon",
        "Renew within 3 days to avoid strategy interruption.",
    )
    .await;
    assert_eq!(membership.status(), StatusCode::OK);
    let membership_body = response_json(membership).await;
    assert_eq!(membership_body["event"]["kind"], "MembershipExpiring");
    assert_eq!(membership_body["telegram_delivered"], true);
    assert_eq!(membership_body["show_expiry_popup"], true);

    let runtime = dispatch_notification(
        &app,
        &session_token,
        "trader@example.com",
        "RuntimeError",
        "Runtime failure",
        "Grid runtime lost exchange connectivity.",
    )
    .await;
    assert_eq!(runtime.status(), StatusCode::OK);
    let runtime_body = response_json(runtime).await;
    assert_eq!(runtime_body["event"]["kind"], "RuntimeError");
    assert_eq!(runtime_body["telegram_delivered"], true);
    assert_eq!(runtime_body["show_expiry_popup"], false);

    let inbox = list_notifications(&app, &session_token, "trader@example.com").await;
    assert_eq!(inbox.status(), StatusCode::OK);
    let inbox_body = response_json(inbox).await;
    let items = inbox_body["items"].as_array().expect("notification items");
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["event"]["kind"], "DepositConfirmed");
    assert_eq!(items[1]["event"]["kind"], "MembershipExpiring");
    assert_eq!(items[1]["show_expiry_popup"], true);
    assert_eq!(items[2]["event"]["kind"], "RuntimeError");
}

#[tokio::test]
async fn telegram_bindings_and_inbox_items_survive_app_rebuilds() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let first_app = app_with_state(AppState::from_shared_db(db.clone()).expect("first state"));
    let session_token = register_and_login(&first_app, "durable@example.com", "pass1234").await;

    let bind_code = create_bind_code(&first_app, &session_token, "durable@example.com", None).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let code = response_json(bind_code).await["code"]
        .as_str()
        .expect("bind code")
        .to_string();

    let bound = bind_telegram(&first_app, &session_token, &code, "chat-durable").await;
    assert_eq!(bound.status(), StatusCode::OK);

    let first_dispatch = dispatch_notification(
        &first_app,
        &session_token,
        "durable@example.com",
        "DepositConfirmed",
        "Deposit received",
        "USDT top-up matched successfully.",
    )
    .await;
    assert_eq!(first_dispatch.status(), StatusCode::OK);
    assert_eq!(response_json(first_dispatch).await["telegram_delivered"], true);

    let rebuilt_app = app_with_state(AppState::from_shared_db(db).expect("rebuilt state"));

    let inbox = list_notifications(&rebuilt_app, &session_token, "durable@example.com").await;
    assert_eq!(inbox.status(), StatusCode::OK);
    let inbox_body = response_json(inbox).await;
    let items = inbox_body["items"].as_array().expect("notification items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["event"]["kind"], "DepositConfirmed");
    assert_eq!(items[0]["telegram_delivered"], true);

    let second_dispatch = dispatch_notification(
        &rebuilt_app,
        &session_token,
        "durable@example.com",
        "MembershipExpiring",
        "Membership ending soon",
        "Renew within 3 days to avoid strategy interruption.",
    )
    .await;
    assert_eq!(second_dispatch.status(), StatusCode::OK);
    let second_dispatch_body = response_json(second_dispatch).await;
    assert_eq!(second_dispatch_body["telegram_delivered"], true);
    assert_eq!(second_dispatch_body["show_expiry_popup"], true);

    let inbox_after_rebuild = list_notifications(&rebuilt_app, &session_token, "durable@example.com").await;
    assert_eq!(inbox_after_rebuild.status(), StatusCode::OK);
    let items_after_rebuild = response_json(inbox_after_rebuild).await["items"]
        .as_array()
        .expect("notification items after rebuild")
        .clone();
    assert_eq!(items_after_rebuild.len(), 2);
    assert_eq!(items_after_rebuild[0]["event"]["kind"], "DepositConfirmed");
    assert_eq!(items_after_rebuild[1]["event"]["kind"], "MembershipExpiring");
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
    let second_code = second_body["code"]
        .as_str()
        .expect("second code")
        .to_string();
    assert_ne!(first_code, second_code);

    let rejected = bind_telegram(&app, &session_token, &first_code, "chat-old").await;
    assert_eq!(rejected.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response_json(rejected).await["error"],
        "bind code not found"
    );

    let accepted = bind_telegram(&app, &session_token, &second_code, "chat-new").await;
    assert_eq!(accepted.status(), StatusCode::OK);
    let accepted_body = response_json(accepted).await;
    assert_eq!(accepted_body["email"], "rotate@example.com");
    assert_eq!(accepted_body["chat_id"], "chat-new");
}

#[tokio::test]
async fn expired_bind_code_is_rejected() {
    let app = app();
    let session_token = register_and_login(&app, "expired-bind@example.com", "pass1234").await;

    let bind_code =
        create_bind_code(&app, &session_token, "expired-bind@example.com", Some(0)).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let bind_code_body = response_json(bind_code).await;
    let code = bind_code_body["code"]
        .as_str()
        .expect("expired code")
        .to_string();

    let rejected = bind_telegram(&app, &session_token, &code, "chat-expired").await;
    assert_eq!(rejected.status(), StatusCode::NOT_FOUND);
    assert_eq!(response_json(rejected).await["error"], "bind code expired");
}

#[tokio::test]
async fn unbound_email_keeps_telegram_delivery_disabled() {
    let app = app();
    let session_token = register_and_login(&app, "unbound@example.com", "pass1234").await;

    let runtime = dispatch_notification(
        &app,
        &session_token,
        "unbound@example.com",
        "RuntimeError",
        "Runtime failure",
        "Worker panic detected.",
    )
    .await;
    assert_eq!(runtime.status(), StatusCode::OK);
    let runtime_body = response_json(runtime).await;
    assert_eq!(runtime_body["event"]["kind"], "RuntimeError");
    assert_eq!(runtime_body["telegram_delivered"], false);
    assert_eq!(runtime_body["in_app_delivered"], true);
}

#[tokio::test]
async fn invalid_ttl_returns_bad_request_instead_of_panicking() {
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

async fn bind_telegram(
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

async fn dispatch_notification(
    app: &axum::Router,
    session_token: &str,
    email: &str,
    kind: &str,
    title: &str,
    message: &str,
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
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("valid json")
}
