use api_server::{app, app_with_state, AppState};
use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use serde_json::{json, Value};
use shared_db::SharedDb;
use tower::ServiceExt;

#[tokio::test]
async fn authenticated_user_can_read_profile_and_change_password() {
    let app = app();
    let _verification_code = register_and_verify(&app, "profile@example.com", "pass1234").await;
    let session_token = login_and_get_token(&app, "profile@example.com", "pass1234", None).await;

    let profile = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/profile")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(profile.status(), StatusCode::OK);
    let profile_body = response_json(profile).await;
    assert_eq!(profile_body["email"], "profile@example.com");
    assert_eq!(profile_body["email_verified"], true);
    assert_eq!(profile_body["totp_enabled"], false);
    assert_eq!(profile_body["admin_totp_required"], false);
    assert_eq!(profile_body["admin_access_granted"], false);

    let password_change = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/profile/password/change")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "current_password": "pass1234",
                        "new_password": "newpass123",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(password_change.status(), StatusCode::OK);
    assert_eq!(
        response_json(password_change).await["password_changed"],
        true
    );

    let revoked_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/profile")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoked_session.status(), StatusCode::UNAUTHORIZED);

    let old_password = app
        .clone()
        .oneshot(login_request("profile@example.com", "pass1234", None))
        .await
        .unwrap();
    assert_eq!(old_password.status(), StatusCode::UNAUTHORIZED);

    let new_password = app
        .oneshot(login_request("profile@example.com", "newpass123", None))
        .await
        .unwrap();
    assert_eq!(new_password.status(), StatusCode::OK);
}

#[tokio::test]
async fn profile_reflects_totp_state_after_enable_and_disable() {
    let app = app();
    let _verification_code = register_and_verify(&app, "security@example.com", "pass1234").await;
    let session_token = login_and_get_token(&app, "security@example.com", "pass1234", None).await;

    let enabled = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/security/totp/enable")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "security@example.com",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(enabled.status(), StatusCode::OK);
    let totp_code = response_json(enabled).await["code"]
        .as_str()
        .expect("totp code")
        .to_owned();

    let profile = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/profile")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(profile.status(), StatusCode::OK);
    assert_eq!(response_json(profile).await["totp_enabled"], true);

    let disabled = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/security/totp/disable")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "security@example.com",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disabled.status(), StatusCode::OK);
    assert_eq!(response_json(disabled).await["disabled"], true);

    let revoked_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/profile")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoked_session.status(), StatusCode::UNAUTHORIZED);

    let without_totp = app
        .clone()
        .oneshot(login_request("security@example.com", "pass1234", None))
        .await
        .unwrap();
    assert_eq!(without_totp.status(), StatusCode::OK);

    let stale_totp = app
        .clone()
        .oneshot(login_request(
            "security@example.com",
            "pass1234",
            Some(&totp_code),
        ))
        .await
        .unwrap();
    assert_eq!(stale_totp.status(), StatusCode::OK);

    let revoked_profile = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/profile")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoked_profile.status(), StatusCode::UNAUTHORIZED);

    let refreshed_session =
        login_and_get_token(&app, "security@example.com", "pass1234", None).await;
    let profile = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/profile")
                .header("authorization", format!("Bearer {refreshed_session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(profile.status(), StatusCode::OK);
    assert_eq!(response_json(profile).await["totp_enabled"], false);
}

#[tokio::test]
async fn profile_admin_access_granted_tracks_current_bearer_session() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_shared_db(&db);

    let _verification_code = register_and_verify(&app, "admin@example.com", "pass1234").await;

    let blocked_login = app
        .clone()
        .oneshot(login_request("admin@example.com", "pass1234", None))
        .await
        .unwrap();
    assert_eq!(blocked_login.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(blocked_login).await["error"],
        "admin totp setup required"
    );

    let bootstrap = bootstrap_admin_totp(&app, "admin@example.com", "pass1234").await;
    let totp_code = bootstrap["code"].as_str().expect("totp code").to_owned();

    let admin_session =
        login_and_get_token(&app, "admin@example.com", "pass1234", Some(&totp_code)).await;
    let current_profile = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/profile")
                .header("authorization", format!("Bearer {admin_session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(current_profile.status(), StatusCode::OK);
    let current_body = response_json(current_profile).await;
    assert_eq!(current_body["admin_totp_required"], true);
    assert_eq!(current_body["totp_enabled"], true);
    assert_eq!(current_body["admin_access_granted"], true);
    assert_eq!(current_body["admin_role"], "operator_admin");
    assert_eq!(
        current_body["admin_permissions"]["can_manage_memberships"],
        false
    );
    assert_eq!(
        current_body["admin_permissions"]["can_manage_templates"],
        false
    );
}

#[tokio::test]
async fn profile_super_admin_access_grants_full_permissions() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_shared_db(&db);

    register_and_verify(&app, "super-admin@example.com", "pass1234").await;

    let blocked_login = app
        .clone()
        .oneshot(login_request("super-admin@example.com", "pass1234", None))
        .await
        .unwrap();
    assert_eq!(blocked_login.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(blocked_login).await["error"],
        "admin totp setup required"
    );

    let bootstrap = bootstrap_admin_totp(&app, "super-admin@example.com", "pass1234").await;
    let totp_code = bootstrap["code"].as_str().expect("totp code").to_owned();

    let admin_session = login_and_get_token(
        &app,
        "super-admin@example.com",
        "pass1234",
        Some(&totp_code),
    )
    .await;
    let current_profile = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/profile")
                .header("authorization", format!("Bearer {admin_session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(current_profile.status(), StatusCode::OK);
    let current_body = response_json(current_profile).await;
    assert_eq!(current_body["admin_access_granted"], true);
    assert_eq!(current_body["admin_role"], "super_admin");
    assert_eq!(
        current_body["admin_permissions"]["can_manage_memberships"],
        true
    );
    assert_eq!(
        current_body["admin_permissions"]["can_manage_templates"],
        true
    );
    assert_eq!(current_body["admin_permissions"]["can_manage_system"], true);
}

#[tokio::test]
async fn profile_requires_authenticated_session() {
    let response = app()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/profile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn password_change_rejects_wrong_current_password_and_preserves_session() {
    let app = app();
    let _verification_code = register_and_verify(&app, "wrong-pass@example.com", "pass1234").await;
    let session_token = login_and_get_token(&app, "wrong-pass@example.com", "pass1234", None).await;

    let password_change = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/profile/password/change")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "current_password": "badpass",
                        "new_password": "newpass123",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(password_change.status(), StatusCode::UNAUTHORIZED);

    let profile = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/profile")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(profile.status(), StatusCode::OK);
}

#[tokio::test]
async fn totp_disable_rejects_session_email_mismatch() {
    let app = app();
    let _ = register_and_verify(&app, "first@example.com", "pass1234").await;
    let _ = register_and_verify(&app, "second@example.com", "pass1234").await;
    let session_token = login_and_get_token(&app, "first@example.com", "pass1234", None).await;
    let _enabled = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/security/totp/enable")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "email": "first@example.com" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let disabled = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/security/totp/disable")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "email": "second@example.com" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disabled.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn configured_admin_totp_disable_attempt_keeps_profile_totp_enabled() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_shared_db(&db);

    let _verification_code = register_and_verify(&app, "admin@example.com", "pass1234").await;
    let bootstrap = bootstrap_admin_totp(&app, "admin@example.com", "pass1234").await;
    let totp_code = bootstrap["code"].as_str().expect("totp code").to_owned();
    let admin_session =
        login_and_get_token(&app, "admin@example.com", "pass1234", Some(&totp_code)).await;

    let disabled = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/security/totp/disable")
                .header("authorization", format!("Bearer {admin_session}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "email": "admin@example.com" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disabled.status(), StatusCode::FORBIDDEN);

    let profile = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/profile")
                .header("authorization", format!("Bearer {admin_session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(profile.status(), StatusCode::OK);

    let profile_body = response_json(profile).await;
    assert_eq!(profile_body["totp_enabled"], true);
    assert_eq!(profile_body["admin_totp_required"], true);
    assert_eq!(profile_body["admin_access_granted"], true);
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
    let register_body = response_json(register).await;

    if let Some(verification_code) = register_body["verification_code"].as_str() {
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

        verification_code.to_owned()
    } else {
        "auto-verified".to_owned()
    }
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

async fn bootstrap_admin_totp(app: &axum::Router, email: &str, password: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/admin-bootstrap")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "email": email, "password": password }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = response.into_body();
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn app_with_shared_db(db: &SharedDb) -> axum::Router {
    app_with_state(AppState::from_shared_db(db.clone()).expect("app state"))
}
