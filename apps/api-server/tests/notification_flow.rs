use api_server::app;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn bind_telegram_and_dispatch_runtime_membership_alerts() {
    let app = app();

    let bind_code = create_bind_code(&app, "trader@example.com", None).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let bind_code_body = response_json(bind_code).await;
    let code = bind_code_body["code"]
        .as_str()
        .expect("bind code")
        .to_string();
    assert_eq!(bind_code_body["email"], "trader@example.com");

    let bound = bind_telegram(&app, &code, "chat-9001").await;
    assert_eq!(bound.status(), StatusCode::OK);
    let bound_body = response_json(bound).await;
    assert_eq!(bound_body["email"], "trader@example.com");
    assert_eq!(bound_body["chat_id"], "chat-9001");

    let deposit = dispatch_notification(
        &app,
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

    let inbox = list_notifications(&app, "trader@example.com").await;
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
async fn previous_bind_code_is_rejected_after_regenerating_for_same_email() {
    let app = app();

    let first = create_bind_code(&app, "rotate@example.com", None).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_body = response_json(first).await;
    let first_code = first_body["code"].as_str().expect("first code").to_string();

    let second = create_bind_code(&app, "rotate@example.com", None).await;
    assert_eq!(second.status(), StatusCode::CREATED);
    let second_body = response_json(second).await;
    let second_code = second_body["code"]
        .as_str()
        .expect("second code")
        .to_string();
    assert_ne!(first_code, second_code);

    let rejected = bind_telegram(&app, &first_code, "chat-old").await;
    assert_eq!(rejected.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response_json(rejected).await["error"],
        "bind code not found"
    );

    let accepted = bind_telegram(&app, &second_code, "chat-new").await;
    assert_eq!(accepted.status(), StatusCode::OK);
    let accepted_body = response_json(accepted).await;
    assert_eq!(accepted_body["email"], "rotate@example.com");
    assert_eq!(accepted_body["chat_id"], "chat-new");
}

#[tokio::test]
async fn expired_bind_code_is_rejected() {
    let app = app();

    let bind_code = create_bind_code(&app, "expired-bind@example.com", Some(0)).await;
    assert_eq!(bind_code.status(), StatusCode::CREATED);
    let bind_code_body = response_json(bind_code).await;
    let code = bind_code_body["code"]
        .as_str()
        .expect("expired code")
        .to_string();

    let rejected = bind_telegram(&app, &code, "chat-expired").await;
    assert_eq!(rejected.status(), StatusCode::NOT_FOUND);
    assert_eq!(response_json(rejected).await["error"], "bind code expired");
}

#[tokio::test]
async fn unbound_email_keeps_telegram_delivery_disabled() {
    let app = app();

    let runtime = dispatch_notification(
        &app,
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

async fn create_bind_code(
    app: &axum::Router,
    email: &str,
    ttl_seconds: Option<i64>,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/telegram/bind-codes")
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

async fn bind_telegram(app: &axum::Router, code: &str, chat_id: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/telegram/bind")
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

async fn list_notifications(app: &axum::Router, email: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/notifications?email={email}"))
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
