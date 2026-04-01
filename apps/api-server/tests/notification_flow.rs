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

    let bind_code = create_bind_code(&app, "trader@example.com").await;
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

async fn create_bind_code(app: &axum::Router, email: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/telegram/bind-codes")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "email": email }).to_string()))
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
