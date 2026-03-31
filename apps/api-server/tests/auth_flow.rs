use api_server::app;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn register_verify_login_and_enable_totp() {
    let app = app();

    let register = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "alice@example.com",
                        "password": "pass1234",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register.status(), StatusCode::CREATED);
    let register_body = response_json(register).await;
    let verification_code = register_body["verification_code"]
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
                        "email": "alice@example.com",
                        "code": verification_code,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(verify.status(), StatusCode::OK);
    assert_eq!(response_json(verify).await["verified"], true);

    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "alice@example.com",
                        "password": "pass1234",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(login.status(), StatusCode::OK);
    assert!(response_json(login).await["session_token"]
        .as_str()
        .expect("session token")
        .starts_with("session-"));

    let reset_request = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/password-reset/request")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "alice@example.com",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(reset_request.status(), StatusCode::OK);
    let reset_code = response_json(reset_request).await["reset_code"]
        .as_str()
        .expect("reset code")
        .to_owned();

    let reset_confirm = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/password-reset/confirm")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "alice@example.com",
                        "code": reset_code,
                        "new_password": "newpass123",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(reset_confirm.status(), StatusCode::OK);
    assert_eq!(response_json(reset_confirm).await["password_reset"], true);

    let enable_totp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/security/totp/enable")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "alice@example.com",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(enable_totp.status(), StatusCode::OK);
    let enable_totp_body = response_json(enable_totp).await;
    let totp_code = enable_totp_body["code"]
        .as_str()
        .expect("totp code")
        .to_owned();
    assert!(enable_totp_body["secret"]
        .as_str()
        .expect("totp secret")
        .starts_with("totp-secret-"));

    let login_with_totp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "alice@example.com",
                        "password": "newpass123",
                        "totp_code": totp_code,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(login_with_totp.status(), StatusCode::OK);
    assert!(response_json(login_with_totp).await["session_token"]
        .as_str()
        .expect("session token")
        .starts_with("session-"));
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = response.into_body();
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
