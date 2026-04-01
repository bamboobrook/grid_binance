use api_server::app;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn create_save_pause_edit_and_start_strategy() {
    let app = app();

    let template = create_template(
        &app,
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

    let preflight_failed = preflight_strategy(&app, strategy_id).await;
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

    let started = start_strategy(&app, strategy_id).await;
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

    let paused = pause_strategies(&app, &[strategy_id]).await;
    assert_eq!(paused.status(), StatusCode::OK);
    let paused_body = response_json(paused).await;
    assert_eq!(paused_body["paused"], 1);

    let listed = list_strategies(&app).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    assert_eq!(listed_body["items"][0]["status"], "Paused");
}

#[tokio::test]
async fn batch_pause_delete_and_stop_all_strategies() {
    let app = app();

    let alpha = create_strategy(
        &app,
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

    let alpha_started = start_strategy(&app, &alpha_id).await;
    assert_eq!(alpha_started.status(), StatusCode::OK);
    let beta_started = start_strategy(&app, &beta_id).await;
    assert_eq!(beta_started.status(), StatusCode::OK);

    let paused = pause_strategies(&app, &[&alpha_id, &beta_id]).await;
    assert_eq!(paused.status(), StatusCode::OK);
    let paused_body = response_json(paused).await;
    assert_eq!(paused_body["paused"], 2);

    let deleted = delete_strategies(&app, &[&beta_id, &gamma_id]).await;
    assert_eq!(deleted.status(), StatusCode::OK);
    let deleted_body = response_json(deleted).await;
    assert_eq!(deleted_body["deleted"], 2);

    let delta = create_strategy(
        &app,
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

    let delta_started = start_strategy(&app, &delta_id).await;
    assert_eq!(delta_started.status(), StatusCode::OK);

    let stopped = stop_all_strategies(&app).await;
    assert_eq!(stopped.status(), StatusCode::OK);
    let stopped_body = response_json(stopped).await;
    assert_eq!(stopped_body["stopped"], 2);

    let listed = list_strategies(&app).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    assert_eq!(listed_body["items"].as_array().expect("items").len(), 2);
    assert_eq!(find_status(&listed_body, &alpha_id), Some("Stopped"));
    assert_eq!(find_status(&listed_body, &delta_id), Some("Stopped"));
}

fn find_status<'a>(body: &'a Value, strategy_id: &str) -> Option<&'a str> {
    body["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| item["id"] == strategy_id)
        .and_then(|item| item["status"].as_str())
}

async fn create_template(app: &axum::Router, payload: Value) -> axum::response::Response {
    request(app, "POST", "/admin/templates", payload).await
}

async fn apply_template(
    app: &axum::Router,
    template_id: &str,
    payload: Value,
) -> axum::response::Response {
    request(
        app,
        "POST",
        &format!("/admin/templates/{template_id}/apply"),
        payload,
    )
    .await
}

async fn create_strategy(app: &axum::Router, payload: Value) -> axum::response::Response {
    request(app, "POST", "/strategies", payload).await
}

async fn update_strategy(
    app: &axum::Router,
    strategy_id: &str,
    payload: Value,
) -> axum::response::Response {
    request(app, "PUT", &format!("/strategies/{strategy_id}"), payload).await
}

async fn preflight_strategy(app: &axum::Router, strategy_id: &str) -> axum::response::Response {
    request(
        app,
        "POST",
        &format!("/strategies/{strategy_id}/preflight"),
        json!({}),
    )
    .await
}

async fn start_strategy(app: &axum::Router, strategy_id: &str) -> axum::response::Response {
    request(
        app,
        "POST",
        &format!("/strategies/{strategy_id}/start"),
        json!({}),
    )
    .await
}

async fn pause_strategies(app: &axum::Router, ids: &[&str]) -> axum::response::Response {
    request(
        app,
        "POST",
        "/strategies/batch/pause",
        json!({ "ids": ids }),
    )
    .await
}

async fn delete_strategies(app: &axum::Router, ids: &[&str]) -> axum::response::Response {
    request(
        app,
        "POST",
        "/strategies/batch/delete",
        json!({ "ids": ids }),
    )
    .await
}

async fn stop_all_strategies(app: &axum::Router) -> axum::response::Response {
    request(app, "POST", "/strategies/stop-all", json!({})).await
}

async fn list_strategies(app: &axum::Router) -> axum::response::Response {
    request(app, "GET", "/strategies", Value::Null).await
}

async fn request(
    app: &axum::Router,
    method: &str,
    uri: &str,
    payload: Value,
) -> axum::response::Response {
    let mut builder = Request::builder().method(method).uri(uri);
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
