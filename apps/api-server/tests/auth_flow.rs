use api_server::{app, app_with_state, AppState};
use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use serde_json::{json, Value};
use shared_db::SharedDb;
use shared_auth::session_token::{verify_session_token, SessionClaims};
use tower::ServiceExt;

const DEFAULT_SESSION_TOKEN_SECRET: &str = "grid-binance-dev-session-secret";

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
    let session_token = response_json(login).await["session_token"]
        .as_str()
        .expect("session token")
        .to_owned();
    assert_eq!(
        verify_claims(&session_token),
        SessionClaims {
            email: "alice@example.com".to_string(),
            is_admin: false,
            sid: 2,
        }
    );

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

    let session_token = login_and_get_token(&app, "alice@example.com", "newpass123", None).await;

    let enable_totp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/security/totp/enable")
                .header("authorization", format!("Bearer {session_token}"))
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
    let session_token = response_json(login_with_totp).await["session_token"]
        .as_str()
        .expect("session token")
        .to_owned();
    assert_eq!(verify_claims(&session_token).email, "alice@example.com");
}

#[tokio::test]
async fn unauthenticated_user_cannot_enable_totp() {
    let app = app();
    let verification_code = register_and_verify(&app, "unauth@example.com", "pass1234").await;
    assert!(!verification_code.is_empty());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/security/totp/enable")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "unauth@example.com",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_requires_valid_totp_code_after_totp_is_enabled() {
    let app = app();
    let _verification_code = register_and_verify(&app, "totp@example.com", "pass1234").await;
    let session_token = login_and_get_token(&app, "totp@example.com", "pass1234", None).await;
    let _totp_code = enable_totp(&app, "totp@example.com", &session_token).await["code"]
        .as_str()
        .expect("totp code")
        .to_owned();

    let missing_code = app
        .clone()
        .oneshot(login_request("totp@example.com", "pass1234", None))
        .await
        .unwrap();
    assert_eq!(missing_code.status(), StatusCode::UNAUTHORIZED);

    let wrong_code = app
        .oneshot(login_request(
            "totp@example.com",
            "pass1234",
            Some("000000"),
        ))
        .await
        .unwrap();
    assert_eq!(wrong_code.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn verification_reset_and_totp_state_survive_router_rebuilds() {
    let db = SharedDb::ephemeral().expect("ephemeral db");

    let register = app_with_shared_db(&db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "durable@example.com",
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

    let verify = app_with_shared_db(&db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/verify-email")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "durable@example.com",
                        "code": verification_code,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(verify.status(), StatusCode::OK);

    let reset_request = app_with_shared_db(&db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/password-reset/request")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "email": "durable@example.com" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reset_request.status(), StatusCode::OK);
    let reset_code = response_json(reset_request).await["reset_code"]
        .as_str()
        .expect("reset code")
        .to_owned();

    let reset_confirm = app_with_shared_db(&db)
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/password-reset/confirm")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "durable@example.com",
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

    let session_token = login_and_get_token(
        &app_with_shared_db(&db),
        "durable@example.com",
        "newpass123",
        None,
    )
    .await;
    let enabled = enable_totp(
        &app_with_shared_db(&db),
        "durable@example.com",
        &session_token,
    )
    .await;
    let totp_code = enabled["code"].as_str().expect("totp code").to_owned();

    let login = app_with_shared_db(&db)
        .oneshot(login_request(
            "durable@example.com",
            "newpass123",
            Some(&totp_code),
        ))
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);

    let audit_logs = db.list_audit_logs().expect("list audit logs");
    assert!(audit_logs
        .iter()
        .any(|entry| entry.action == "auth.password_reset_requested"
            && entry.actor_email == "durable@example.com"));
}

#[tokio::test]
async fn admin_access_requires_totp_backed_session() {
    let app = app();
    let _verification_code = register_and_verify(&app, "admin@example.com", "pass1234").await;

    let session_token = login_and_get_token(&app, "admin@example.com", "pass1234", None).await;
    let admin_list = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/templates")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_list.status(), StatusCode::FORBIDDEN);

    let totp = enable_totp(&app, "admin@example.com", &session_token).await;
    let totp_code = totp["code"].as_str().expect("totp code");
    let admin_session = login_and_get_token(&app, "admin@example.com", "pass1234", Some(totp_code)).await;

    let admin_list = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/templates")
                .header("authorization", format!("Bearer {admin_session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_list.status(), StatusCode::OK);
}

#[tokio::test]
async fn password_reset_rejects_empty_password_and_invalidates_old_password() {
    let app = app();
    let _verification_code = register_and_verify(&app, "reset@example.com", "pass1234").await;
    let old_session = login_and_get_token(&app, "reset@example.com", "pass1234", None).await;
    let reset_code = request_password_reset(&app, "reset@example.com").await;

    let empty_password = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/password-reset/confirm")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "reset@example.com",
                        "code": reset_code,
                        "new_password": "",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(empty_password.status(), StatusCode::BAD_REQUEST);

    let reset_code = request_password_reset(&app, "reset@example.com").await;
    let reset_confirm = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/password-reset/confirm")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "reset@example.com",
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

    let revoked_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/profile")
                .header("authorization", format!("Bearer {old_session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoked_session.status(), StatusCode::UNAUTHORIZED);

    let old_password_login = app
        .clone()
        .oneshot(login_request("reset@example.com", "pass1234", None))
        .await
        .unwrap();
    assert_eq!(old_password_login.status(), StatusCode::UNAUTHORIZED);

    let new_password_login = app
        .oneshot(login_request("reset@example.com", "newpass123", None))
        .await
        .unwrap();
    assert_eq!(new_password_login.status(), StatusCode::OK);
}

async fn register_and_verify(app: &axum::Router, email: &str, password: &str) -> String {
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
                        "password": password,
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

    verification_code
}

async fn login_and_get_token(
    app: &axum::Router,
    email: &str,
    password: &str,
    totp_code: Option<&str>,
) -> String {
    let response = app
        .clone()
        .oneshot(login_request(email, password, totp_code))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await["session_token"]
        .as_str()
        .expect("session token")
        .to_owned()
}

async fn request_password_reset(app: &axum::Router, email: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/password-reset/request")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "email": email }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await["reset_code"]
        .as_str()
        .expect("reset code")
        .to_owned()
}

async fn enable_totp(app: &axum::Router, email: &str, session_token: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/security/totp/enable")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "email": email }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await
}

fn app_with_shared_db(db: &SharedDb) -> axum::Router {
    app_with_state(AppState::from_shared_db(db.clone()).expect("app state"))
}

fn login_request(email: &str, password: &str, totp_code: Option<&str>) -> Request<Body> {
    let body = match totp_code {
        Some(totp_code) => json!({
            "email": email,
            "password": password,
            "totp_code": totp_code,
        }),
        None => json!({
            "email": email,
            "password": password,
        }),
    };

    Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = response.into_body();
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn verify_claims(session_token: &str) -> SessionClaims {
    verify_session_token(DEFAULT_SESSION_TOKEN_SECRET, session_token).expect("valid session token")
}
