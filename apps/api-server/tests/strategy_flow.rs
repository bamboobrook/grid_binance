use api_server::app;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

mod support;

use support::register_and_login;

#[tokio::test]
async fn create_save_pause_edit_and_start_strategy() {
    let app = app();
    let user_token = register_and_login(&app, "trader@example.com", "pass1234").await;
    let admin_token = register_and_login(&app, "admin@example.com", "pass1234").await;

    let template = create_template(
        &app,
        Some(&admin_token),
        json!({
            "name": "Momentum Starter",
            "symbol": "BTCUSDT",
            "budget": "150.00",
            "grid_spacing_bps": 60,
            "membership_ready": true,
            "exchange_ready": false,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(template.status(), StatusCode::CREATED);
    let template_body = response_json(template).await;
    let template_id = template_body["id"].as_str().expect("template id");

    let applied = apply_template(
        &app,
        Some(&user_token),
        template_id,
        json!({
            "name": "BTC copied draft",
        }),
    )
    .await;
    assert_eq!(applied.status(), StatusCode::CREATED);
    let applied_body = response_json(applied).await;
    let strategy_id = applied_body["id"].as_str().expect("strategy id");
    assert_eq!(applied_body["status"], "Draft");
    assert_eq!(applied_body["symbol"], "BTCUSDT");
    assert_eq!(applied_body["budget"], "150.00");
    assert_eq!(applied_body["source_template_id"], template_id);

    let preflight_failed = preflight_strategy(&app, &user_token, strategy_id).await;
    assert_eq!(preflight_failed.status(), StatusCode::OK);
    let preflight_failed_body = response_json(preflight_failed).await;
    assert_eq!(preflight_failed_body["ok"], false);
    assert_eq!(
        preflight_failed_body["failures"]
            .as_array()
            .expect("failures")
            .len(),
        1
    );
    assert_eq!(
        preflight_failed_body["failures"][0]["step"],
        "exchange_connection"
    );

    let saved = update_strategy(
        &app,
        &user_token,
        strategy_id,
        json!({
            "name": "BTC live draft",
            "symbol": "BTCUSDT",
            "budget": "180.00",
            "grid_spacing_bps": 45,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(saved.status(), StatusCode::OK);
    let saved_body = response_json(saved).await;
    assert_eq!(saved_body["name"], "BTC live draft");
    assert_eq!(saved_body["budget"], "180.00");
    assert_eq!(saved_body["grid_spacing_bps"], 45);

    let started = start_strategy(&app, &user_token, strategy_id).await;
    assert_eq!(started.status(), StatusCode::OK);
    let started_body = response_json(started).await;
    assert_eq!(started_body["status"], "Running");
    assert_eq!(started_body["preflight"]["ok"], true);
    assert_eq!(
        started_body["preflight"]["failures"]
            .as_array()
            .expect("failures")
            .len(),
        0
    );

    let edit_rejected = update_strategy(
        &app,
        &user_token,
        strategy_id,
        json!({
            "name": "should fail",
            "symbol": "BTCUSDT",
            "budget": "180.00",
            "grid_spacing_bps": 45,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(edit_rejected.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(edit_rejected).await["error"],
        "only draft strategies can be edited"
    );

    let paused = pause_strategies(&app, &user_token, &[strategy_id]).await;
    assert_eq!(paused.status(), StatusCode::OK);
    let paused_body = response_json(paused).await;
    assert_eq!(paused_body["paused"], 1);

    let listed = list_strategies(&app, &user_token).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    assert_eq!(listed_body["items"][0]["status"], "Paused");
}

#[tokio::test]
async fn batch_pause_delete_and_stop_all_strategies() {
    let app = app();
    let user_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let alpha = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "alpha",
            "symbol": "BTCUSDT",
            "budget": "100.00",
            "grid_spacing_bps": 50,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(alpha.status(), StatusCode::CREATED);
    let alpha_id = response_json(alpha).await["id"]
        .as_str()
        .expect("alpha id")
        .to_string();

    let beta = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "beta",
            "symbol": "ETHUSDT",
            "budget": "90.00",
            "grid_spacing_bps": 40,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(beta.status(), StatusCode::CREATED);
    let beta_id = response_json(beta).await["id"]
        .as_str()
        .expect("beta id")
        .to_string();

    let gamma = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "gamma",
            "symbol": "SOLUSDT",
            "budget": "80.00",
            "grid_spacing_bps": 30,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(gamma.status(), StatusCode::CREATED);
    let gamma_id = response_json(gamma).await["id"]
        .as_str()
        .expect("gamma id")
        .to_string();

    let alpha_started = start_strategy(&app, &user_token, &alpha_id).await;
    assert_eq!(alpha_started.status(), StatusCode::OK);
    let beta_started = start_strategy(&app, &user_token, &beta_id).await;
    assert_eq!(beta_started.status(), StatusCode::OK);

    let paused = pause_strategies(&app, &user_token, &[&alpha_id, &beta_id]).await;
    assert_eq!(paused.status(), StatusCode::OK);
    let paused_body = response_json(paused).await;
    assert_eq!(paused_body["paused"], 2);

    let deleted = delete_strategies(&app, &user_token, &[&beta_id, &gamma_id]).await;
    assert_eq!(deleted.status(), StatusCode::OK);
    let deleted_body = response_json(deleted).await;
    assert_eq!(deleted_body["deleted"], 2);

    let delta = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "delta",
            "symbol": "BNBUSDT",
            "budget": "120.00",
            "grid_spacing_bps": 25,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(delta.status(), StatusCode::CREATED);
    let delta_id = response_json(delta).await["id"]
        .as_str()
        .expect("delta id")
        .to_string();

    let delta_started = start_strategy(&app, &user_token, &delta_id).await;
    assert_eq!(delta_started.status(), StatusCode::OK);

    let stopped = stop_all_strategies(&app, &user_token).await;
    assert_eq!(stopped.status(), StatusCode::OK);
    let stopped_body = response_json(stopped).await;
    assert_eq!(stopped_body["stopped"], 2);

    let listed = list_strategies(&app, &user_token).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    assert_eq!(listed_body["items"].as_array().expect("items").len(), 2);
    assert_eq!(find_status(&listed_body, &alpha_id), Some("Stopped"));
    assert_eq!(find_status(&listed_body, &delta_id), Some("Stopped"));
}

#[tokio::test]
async fn failed_start_keeps_draft_editable_until_preflight_is_fixed() {
    let app = app();
    let user_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let draft = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "recoverable",
            "symbol": "BTCUSDT",
            "budget": "100.00",
            "grid_spacing_bps": 50,
            "membership_ready": true,
            "exchange_ready": false,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(draft.status(), StatusCode::CREATED);
    let strategy_id = response_json(draft).await["id"]
        .as_str()
        .expect("strategy id")
        .to_string();

    let failed_start = start_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(failed_start.status(), StatusCode::CONFLICT);
    let failed_start_body = response_json(failed_start).await;
    assert_eq!(failed_start_body["error"], "preflight failed");
    assert_eq!(failed_start_body["preflight"]["ok"], false);
    assert_eq!(
        failed_start_body["preflight"]["failures"][0]["step"],
        "exchange_connection"
    );

    let repaired = update_strategy(
        &app,
        &user_token,
        &strategy_id,
        json!({
            "name": "recoverable fixed",
            "symbol": "BTCUSDT",
            "budget": "120.00",
            "grid_spacing_bps": 45,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(repaired.status(), StatusCode::OK);
    let repaired_body = response_json(repaired).await;
    assert_eq!(repaired_body["status"], "Draft");
    assert_eq!(repaired_body["name"], "recoverable fixed");

    let restarted = start_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(restarted.status(), StatusCode::OK);
    let restarted_body = response_json(restarted).await;
    assert_eq!(restarted_body["status"], "Running");
    assert_eq!(restarted_body["preflight"]["ok"], true);
}

#[tokio::test]
async fn start_strategy_rejects_unknown_strategy_id() {
    let app = app();
    let user_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let response = start_strategy(&app, &user_token, "strategy-missing").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response_json(response).await["error"], "strategy not found");
}

#[tokio::test]
async fn strategy_owner_isolation_blocks_cross_user_reads_and_mutations() {
    let app = app();
    let alice_token = register_and_login(&app, "alice@example.com", "pass1234").await;
    let bob_token = register_and_login(&app, "bob@example.com", "pass1234").await;

    let alice_strategy = create_strategy(
        &app,
        &alice_token,
        json!({
            "name": "alice draft",
            "symbol": "BTCUSDT",
            "budget": "100.00",
            "grid_spacing_bps": 50,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(alice_strategy.status(), StatusCode::CREATED);
    let alice_strategy_id = response_json(alice_strategy).await["id"]
        .as_str()
        .expect("alice strategy id")
        .to_string();

    let bob_strategy = create_strategy(
        &app,
        &bob_token,
        json!({
            "name": "bob draft",
            "symbol": "ETHUSDT",
            "budget": "120.00",
            "grid_spacing_bps": 45,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(bob_strategy.status(), StatusCode::CREATED);
    let bob_strategy_id = response_json(bob_strategy).await["id"]
        .as_str()
        .expect("bob strategy id")
        .to_string();

    let alice_list = list_strategies(&app, &alice_token).await;
    assert_eq!(alice_list.status(), StatusCode::OK);
    let alice_list_body = response_json(alice_list).await;
    assert_eq!(alice_list_body["items"].as_array().expect("items").len(), 1);
    assert_eq!(alice_list_body["items"][0]["id"], alice_strategy_id);

    let bob_list = list_strategies(&app, &bob_token).await;
    assert_eq!(bob_list.status(), StatusCode::OK);
    let bob_list_body = response_json(bob_list).await;
    assert_eq!(bob_list_body["items"].as_array().expect("items").len(), 1);
    assert_eq!(bob_list_body["items"][0]["id"], bob_strategy_id);

    let foreign_preflight = preflight_strategy(&app, &bob_token, &alice_strategy_id).await;
    assert_eq!(foreign_preflight.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response_json(foreign_preflight).await["error"],
        "strategy not found"
    );

    let foreign_update = update_strategy(
        &app,
        &bob_token,
        &alice_strategy_id,
        json!({
            "name": "stolen",
            "symbol": "BTCUSDT",
            "budget": "999.00",
            "grid_spacing_bps": 10,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(foreign_update.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response_json(foreign_update).await["error"],
        "strategy not found"
    );

    let foreign_start = start_strategy(&app, &bob_token, &alice_strategy_id).await;
    assert_eq!(foreign_start.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response_json(foreign_start).await["error"],
        "strategy not found"
    );

    let foreign_pause = pause_strategies(&app, &bob_token, &[&alice_strategy_id]).await;
    assert_eq!(foreign_pause.status(), StatusCode::OK);
    assert_eq!(response_json(foreign_pause).await["paused"], 0);

    let foreign_delete = delete_strategies(&app, &bob_token, &[&alice_strategy_id]).await;
    assert_eq!(foreign_delete.status(), StatusCode::OK);
    assert_eq!(response_json(foreign_delete).await["deleted"], 0);

    let alice_list_after = list_strategies(&app, &alice_token).await;
    assert_eq!(alice_list_after.status(), StatusCode::OK);
    let alice_list_after_body = response_json(alice_list_after).await;
    assert_eq!(
        alice_list_after_body["items"]
            .as_array()
            .expect("items")
            .len(),
        1
    );
    assert_eq!(
        find_status(&alice_list_after_body, &alice_strategy_id),
        Some("Draft")
    );
}

#[tokio::test]
async fn stop_all_only_stops_strategies_owned_by_current_user() {
    let app = app();
    let alice_token = register_and_login(&app, "alice@example.com", "pass1234").await;
    let bob_token = register_and_login(&app, "bob@example.com", "pass1234").await;

    let alice_strategy = create_strategy(
        &app,
        &alice_token,
        json!({
            "name": "alice running",
            "symbol": "BTCUSDT",
            "budget": "100.00",
            "grid_spacing_bps": 50,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(alice_strategy.status(), StatusCode::CREATED);
    let alice_strategy_id = response_json(alice_strategy).await["id"]
        .as_str()
        .expect("alice strategy id")
        .to_string();

    let bob_strategy = create_strategy(
        &app,
        &bob_token,
        json!({
            "name": "bob running",
            "symbol": "ETHUSDT",
            "budget": "120.00",
            "grid_spacing_bps": 45,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;
    assert_eq!(bob_strategy.status(), StatusCode::CREATED);
    let bob_strategy_id = response_json(bob_strategy).await["id"]
        .as_str()
        .expect("bob strategy id")
        .to_string();

    let alice_started = start_strategy(&app, &alice_token, &alice_strategy_id).await;
    assert_eq!(alice_started.status(), StatusCode::OK);
    let bob_started = start_strategy(&app, &bob_token, &bob_strategy_id).await;
    assert_eq!(bob_started.status(), StatusCode::OK);

    let stopped = stop_all_strategies(&app, &alice_token).await;
    assert_eq!(stopped.status(), StatusCode::OK);
    assert_eq!(response_json(stopped).await["stopped"], 1);

    let alice_list = list_strategies(&app, &alice_token).await;
    assert_eq!(alice_list.status(), StatusCode::OK);
    let alice_list_body = response_json(alice_list).await;
    assert_eq!(
        find_status(&alice_list_body, &alice_strategy_id),
        Some("Stopped")
    );

    let bob_list = list_strategies(&app, &bob_token).await;
    assert_eq!(bob_list.status(), StatusCode::OK);
    let bob_list_body = response_json(bob_list).await;
    assert_eq!(
        find_status(&bob_list_body, &bob_strategy_id),
        Some("Running")
    );
}

#[tokio::test]
async fn regular_user_cannot_create_admin_template() {
    let app = app();
    let user_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let response = create_template(
        &app,
        Some(&user_token),
        json!({
            "name": "Forbidden Template",
            "symbol": "BTCUSDT",
            "budget": "100.00",
            "grid_spacing_bps": 50,
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

fn find_status<'a>(body: &'a Value, strategy_id: &str) -> Option<&'a str> {
    body["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| item["id"] == strategy_id)
        .and_then(|item| item["status"].as_str())
}

async fn create_template(
    app: &axum::Router,
    session_token: Option<&str>,
    payload: Value,
) -> axum::response::Response {
    request(app, session_token, "POST", "/admin/templates", payload).await
}

async fn apply_template(
    app: &axum::Router,
    session_token: Option<&str>,
    template_id: &str,
    payload: Value,
) -> axum::response::Response {
    request(
        app,
        session_token,
        "POST",
        &format!("/admin/templates/{template_id}/apply"),
        payload,
    )
    .await
}

async fn create_strategy(
    app: &axum::Router,
    session_token: &str,
    payload: Value,
) -> axum::response::Response {
    request(app, Some(session_token), "POST", "/strategies", payload).await
}

async fn update_strategy(
    app: &axum::Router,
    session_token: &str,
    strategy_id: &str,
    payload: Value,
) -> axum::response::Response {
    request(
        app,
        Some(session_token),
        "PUT",
        &format!("/strategies/{strategy_id}"),
        payload,
    )
    .await
}

async fn preflight_strategy(
    app: &axum::Router,
    session_token: &str,
    strategy_id: &str,
) -> axum::response::Response {
    request(
        app,
        Some(session_token),
        "POST",
        &format!("/strategies/{strategy_id}/preflight"),
        json!({}),
    )
    .await
}

async fn start_strategy(
    app: &axum::Router,
    session_token: &str,
    strategy_id: &str,
) -> axum::response::Response {
    request(
        app,
        Some(session_token),
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
    request(
        app,
        Some(session_token),
        "POST",
        "/strategies/batch/pause",
        json!({ "ids": ids }),
    )
    .await
}

async fn delete_strategies(
    app: &axum::Router,
    session_token: &str,
    ids: &[&str],
) -> axum::response::Response {
    request(
        app,
        Some(session_token),
        "POST",
        "/strategies/batch/delete",
        json!({ "ids": ids }),
    )
    .await
}

async fn stop_all_strategies(app: &axum::Router, session_token: &str) -> axum::response::Response {
    request(
        app,
        Some(session_token),
        "POST",
        "/strategies/stop-all",
        json!({}),
    )
    .await
}

async fn list_strategies(app: &axum::Router, session_token: &str) -> axum::response::Response {
    request(app, Some(session_token), "GET", "/strategies", Value::Null).await
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

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("valid json")
}
