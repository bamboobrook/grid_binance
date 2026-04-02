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
async fn strategy_revisions_runtime_contracts_and_soft_archive_follow_frozen_lifecycle() {
    let app = app();
    let user_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let created = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "BTC draft",
            "BTCUSDT",
            "Custom",
            &[
                grid_level("101.00", "0.0100", 160, Some(80)),
                grid_level("108.00", "0.0200", 220, None),
            ],
            true,
            false,
            true,
            Some(800),
            Some(350),
            "Rebuild",
        ),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = response_json(created).await;
    let strategy_id = created_body["id"].as_str().expect("strategy id").to_string();
    assert_eq!(created_body["status"], "Draft");
    assert_eq!(created_body["draft_revision"]["generation"], "Custom");
    assert_eq!(
        created_body["draft_revision"]["levels"]
            .as_array()
            .expect("levels")
            .len(),
        2
    );
    assert_eq!(created_body["draft_revision"]["version"], 1);
    assert!(created_body["active_revision"].is_null());
    assert_eq!(created_body["draft_revision"]["post_trigger_action"], "Rebuild");
    assert_eq!(
        created_body["draft_revision"]["overall_take_profit_bps"],
        json!(800)
    );
    assert_eq!(
        created_body["draft_revision"]["overall_stop_loss_bps"],
        json!(350)
    );
    assert_eq!(
        created_body["draft_revision"]["levels"][0]["trailing_bps"],
        json!(80)
    );
    assert_eq!(
        created_body["runtime"]["orders"].as_array().expect("orders").len(),
        0
    );
    assert_eq!(
        created_body["runtime"]["positions"]
            .as_array()
            .expect("positions")
            .len(),
        0
    );

    let preflight_failed = preflight_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(preflight_failed.status(), StatusCode::OK);
    let preflight_failed_body = response_json(preflight_failed).await;
    assert_eq!(preflight_failed_body["ok"], false);
    assert_eq!(
        preflight_failed_body["failures"][0]["step"],
        "exchange_connection"
    );
    assert_eq!(
        preflight_failed_body["steps"].as_array().expect("steps").len(),
        11
    );
    assert_eq!(preflight_failed_body["steps"][0]["step"], "membership_status");
    assert_eq!(
        preflight_failed_body["steps"][1]["step"],
        "exchange_connection"
    );
    assert_eq!(
        preflight_failed_body["steps"][2]["step"],
        "exchange_permissions"
    );
    assert_eq!(
        preflight_failed_body["steps"][0]["status"],
        "Passed"
    );
    assert_eq!(
        preflight_failed_body["steps"][1]["status"],
        "Failed"
    );
    assert_eq!(
        preflight_failed_body["steps"][2]["status"],
        "Skipped"
    );

    let saved = update_strategy(
        &app,
        &user_token,
        &strategy_id,
        strategy_payload(
            "BTC live draft",
            "BTCUSDT",
            "Arithmetic",
            &[
                grid_level("95.00", "0.0100", 150, Some(50)),
                grid_level("100.00", "0.0150", 180, None),
                grid_level("105.00", "0.0200", 220, None),
            ],
            true,
            true,
            true,
            Some(600),
            Some(300),
            "Stop",
        ),
    )
    .await;
    assert_eq!(saved.status(), StatusCode::OK);
    let saved_body = response_json(saved).await;
    assert_eq!(saved_body["status"], "Draft");
    assert_eq!(saved_body["draft_revision"]["version"], 2);
    assert_eq!(saved_body["draft_revision"]["generation"], "Arithmetic");
    assert_eq!(
        saved_body["draft_revision"]["levels"]
            .as_array()
            .expect("levels")
            .len(),
        3
    );

    let started = start_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(started.status(), StatusCode::OK);
    let started_body = response_json(started).await;
    assert_eq!(started_body["status"], "Running");
    assert_eq!(started_body["active_revision"]["version"], 2);
    assert_eq!(started_body["preflight"]["ok"], true);
    assert_eq!(
        started_body["preflight"]["steps"]
            .as_array()
            .expect("steps")
            .iter()
            .filter(|step| step["status"] == "Passed")
            .count(),
        9
    );
    assert_eq!(
        started_body["preflight"]["steps"]
            .as_array()
            .expect("steps")
            .iter()
            .filter(|step| step["status"] == "Skipped")
            .count(),
        2
    );
    assert_eq!(
        started_body["runtime"]["orders"]
            .as_array()
            .expect("orders")
            .len(),
        3
    );
    assert_eq!(
        started_body["runtime"]["positions"]
            .as_array()
            .expect("positions")
            .len(),
        0
    );
    assert_eq!(
        started_body["runtime"]["events"][0]["event_type"],
        "strategy_started"
    );

    let orders = list_strategy_orders(&app, &user_token, &strategy_id).await;
    assert_eq!(orders.status(), StatusCode::OK);
    let orders_body = response_json(orders).await;
    assert_eq!(
        orders_body["orders"].as_array().expect("orders").len(),
        3
    );
    assert_eq!(
        orders_body["positions"].as_array().expect("positions").len(),
        0
    );

    let paused = pause_strategies(&app, &user_token, &[&strategy_id]).await;
    assert_eq!(paused.status(), StatusCode::OK);
    assert_eq!(response_json(paused).await["paused"], 1);

    let paused_list = list_strategies(&app, &user_token).await;
    assert_eq!(paused_list.status(), StatusCode::OK);
    let paused_list_body = response_json(paused_list).await;
    let paused_strategy = find_strategy(&paused_list_body, &strategy_id);
    assert_eq!(paused_strategy["status"], "Paused");
    assert_eq!(
        paused_strategy["runtime"]["positions"]
            .as_array()
            .expect("positions")
            .len(),
        0
    );
    assert_eq!(
        paused_strategy["runtime"]["orders"]
            .as_array()
            .expect("orders")
            .iter()
            .filter(|order| order["status"] == "Canceled")
            .count(),
        3
    );

    let edited_while_paused = update_strategy(
        &app,
        &user_token,
        &strategy_id,
        strategy_payload(
            "BTC resumed draft",
            "BTCUSDT",
            "Geometric",
            &[
                grid_level("90.00", "0.0100", 120, None),
                grid_level("100.00", "0.0100", 140, None),
                grid_level("111.00", "0.0100", 160, None),
                grid_level("123.21", "0.0100", 200, None),
            ],
            true,
            true,
            true,
            Some(700),
            Some(250),
            "Rebuild",
        ),
    )
    .await;
    assert_eq!(edited_while_paused.status(), StatusCode::OK);
    let edited_body = response_json(edited_while_paused).await;
    assert_eq!(edited_body["status"], "Paused");
    assert_eq!(edited_body["draft_revision"]["version"], 3);
    assert_eq!(edited_body["active_revision"]["version"], 2);
    assert_eq!(edited_body["draft_revision"]["generation"], "Geometric");

    let resumed = resume_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(resumed.status(), StatusCode::OK);
    let resumed_body = response_json(resumed).await;
    assert_eq!(resumed_body["status"], "Running");
    assert_eq!(resumed_body["active_revision"]["version"], 3);
    assert_eq!(resumed_body["active_revision"]["generation"], "Geometric");
    assert_eq!(
        resumed_body["runtime"]["orders"]
            .as_array()
            .expect("orders")
            .len(),
        4
    );
    assert_eq!(
        resumed_body["runtime"]["positions"]
            .as_array()
            .expect("positions")
            .len(),
        0
    );

    let stopped = stop_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(stopped.status(), StatusCode::OK);
    let stopped_body = response_json(stopped).await;
    assert_eq!(stopped_body["status"], "Stopped");
    assert_eq!(
        stopped_body["runtime"]["positions"]
            .as_array()
            .expect("positions")
            .len(),
        0
    );
    assert_eq!(
        stopped_body["runtime"]["fills"].as_array().expect("fills").len(),
        0
    );
    assert_eq!(
        stopped_body["runtime"]["events"]
            .as_array()
            .expect("events")
            .last()
            .expect("last event")["event_type"],
        "strategy_stopped"
    );

    let deleted = delete_strategies(&app, &user_token, &[&strategy_id]).await;
    assert_eq!(deleted.status(), StatusCode::OK);
    assert_eq!(response_json(deleted).await["deleted"], 1);

    let listed_after_delete = list_strategies(&app, &user_token).await;
    assert_eq!(listed_after_delete.status(), StatusCode::OK);
    let listed_after_delete_body = response_json(listed_after_delete).await;
    let archived = find_strategy(&listed_after_delete_body, &strategy_id);
    assert_eq!(archived["status"], "Archived");
    assert!(archived["archived_at"].is_string());
}

#[tokio::test]
async fn strategy_validates_generation_modes_and_trailing_constraints() {
    let app = app();
    let user_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let arithmetic = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "arith",
            "BTCUSDT",
            "Arithmetic",
            &[
                grid_level("95.00", "0.0100", 100, None),
                grid_level("100.00", "0.0100", 100, None),
                grid_level("105.00", "0.0100", 100, None),
            ],
            true,
            true,
            true,
            None,
            None,
            "Stop",
        ),
    )
    .await;
    assert_eq!(arithmetic.status(), StatusCode::CREATED);

    let geometric = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "geo",
            "ETHUSDT",
            "Geometric",
            &[
                grid_level("100.00", "0.0100", 120, None),
                grid_level("125.00", "0.0100", 120, None),
                grid_level("156.25", "0.0100", 120, None),
            ],
            true,
            true,
            true,
            Some(500),
            None,
            "Rebuild",
        ),
    )
    .await;
    assert_eq!(geometric.status(), StatusCode::CREATED);

    let custom = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "custom",
            "SOLUSDT",
            "Custom",
            &[
                grid_level("80.10", "1.0000", 130, Some(90)),
                grid_level("84.20", "1.5000", 160, None),
            ],
            true,
            true,
            true,
            None,
            Some(200),
            "Stop",
        ),
    )
    .await;
    assert_eq!(custom.status(), StatusCode::CREATED);

    let listed = list_strategies(&app, &user_token).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    assert_eq!(
        listed_body["items"].as_array().expect("items").len(),
        3
    );
    assert!(listed_body["items"]
        .as_array()
        .expect("items")
        .iter()
        .any(|item| item["draft_revision"]["generation"] == "Arithmetic"));
    assert!(listed_body["items"]
        .as_array()
        .expect("items")
        .iter()
        .any(|item| item["draft_revision"]["generation"] == "Geometric"));
    assert!(listed_body["items"]
        .as_array()
        .expect("items")
        .iter()
        .any(|item| item["draft_revision"]["generation"] == "Custom"));

    let invalid_trailing = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "bad trailing",
            "BNBUSDT",
            "Custom",
            &[grid_level("600.00", "0.0100", 80, Some(120))],
            true,
            true,
            true,
            None,
            None,
            "Stop",
        ),
    )
    .await;
    assert_eq!(invalid_trailing.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(invalid_trailing).await["error"],
        "level 0 trailing_bps must be less than or equal to take_profit_bps"
    );
}

#[tokio::test]
async fn futures_preflight_reports_permissions_hedge_margin_and_balance_surface() {
    let app = app();
    let user_token = register_and_login(&app, "futures@example.com", "pass1234").await;

    let created = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "futures draft",
            "symbol": "BTCUSDT",
            "market": "FuturesUsdM",
            "mode": "FuturesLong",
            "generation": "Custom",
            "levels": [
                {
                    "entry_price": "100.00",
                    "quantity": "0.0100",
                    "take_profit_bps": 120,
                    "trailing_bps": null
                }
            ],
            "membership_ready": true,
            "exchange_ready": true,
            "symbol_ready": true,
            "permissions_ready": false,
            "withdrawals_disabled": true,
            "hedge_mode_ready": false,
            "filters_ready": false,
            "margin_ready": false,
            "conflict_ready": false,
            "balance_ready": false,
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let strategy_id = response_json(created).await["id"]
        .as_str()
        .expect("strategy id")
        .to_string();

    let preflight = preflight_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(preflight.status(), StatusCode::OK);
    let body = response_json(preflight).await;
    assert_eq!(body["ok"], false);
    assert_eq!(
        body["steps"].as_array().expect("steps").len(),
        11
    );
    assert_eq!(body["steps"][0]["step"], "membership_status");
    assert_eq!(body["steps"][2]["step"], "exchange_permissions");
    assert_eq!(body["steps"][3]["step"], "withdrawal_permission_disabled");
    assert_eq!(body["steps"][4]["step"], "hedge_mode");
    assert_eq!(body["steps"][7]["step"], "margin_or_leverage");
    assert_eq!(body["steps"][8]["step"], "strategy_conflicts");
    assert_eq!(body["steps"][9]["step"], "balance_or_collateral");
    assert_eq!(body["failures"][0]["step"], "exchange_permissions");
    assert_eq!(body["steps"][2]["status"], "Failed");
    assert_eq!(body["steps"][3]["status"], "Skipped");
}

#[tokio::test]
async fn strategy_owner_isolation_blocks_cross_user_reads_and_mutations() {
    let app = app();
    let alice_token = register_and_login(&app, "alice@example.com", "pass1234").await;
    let bob_token = register_and_login(&app, "bob@example.com", "pass1234").await;

    let alice_strategy = create_strategy(
        &app,
        &alice_token,
        strategy_payload(
            "alice draft",
            "BTCUSDT",
            "Custom",
            &[grid_level("100.00", "0.0100", 120, None)],
            true,
            true,
            true,
            None,
            None,
            "Stop",
        ),
    )
    .await;
    assert_eq!(alice_strategy.status(), StatusCode::CREATED);
    let alice_strategy_id = response_json(alice_strategy).await["id"]
        .as_str()
        .expect("alice strategy id")
        .to_string();

    let foreign_preflight = preflight_strategy(&app, &bob_token, &alice_strategy_id).await;
    assert_eq!(foreign_preflight.status(), StatusCode::NOT_FOUND);

    let foreign_update = update_strategy(
        &app,
        &bob_token,
        &alice_strategy_id,
        strategy_payload(
            "stolen",
            "BTCUSDT",
            "Custom",
            &[grid_level("100.00", "1.0000", 100, None)],
            true,
            true,
            true,
            None,
            None,
            "Stop",
        ),
    )
    .await;
    assert_eq!(foreign_update.status(), StatusCode::NOT_FOUND);

    let foreign_start = start_strategy(&app, &bob_token, &alice_strategy_id).await;
    assert_eq!(foreign_start.status(), StatusCode::NOT_FOUND);

    let foreign_resume = resume_strategy(&app, &bob_token, &alice_strategy_id).await;
    assert_eq!(foreign_resume.status(), StatusCode::NOT_FOUND);

    let foreign_stop = stop_strategy(&app, &bob_token, &alice_strategy_id).await;
    assert_eq!(foreign_stop.status(), StatusCode::NOT_FOUND);

    let foreign_orders = list_strategy_orders(&app, &bob_token, &alice_strategy_id).await;
    assert_eq!(foreign_orders.status(), StatusCode::NOT_FOUND);

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
        find_strategy(&alice_list_after_body, &alice_strategy_id)["status"],
        "Draft"
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
        strategy_payload(
            "alice running",
            "BTCUSDT",
            "Custom",
            &[grid_level("100.00", "0.0100", 120, None)],
            true,
            true,
            true,
            None,
            None,
            "Stop",
        ),
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
        strategy_payload(
            "bob running",
            "ETHUSDT",
            "Custom",
            &[grid_level("200.00", "0.0100", 120, None)],
            true,
            true,
            true,
            None,
            None,
            "Stop",
        ),
    )
    .await;
    assert_eq!(bob_strategy.status(), StatusCode::CREATED);
    let bob_strategy_id = response_json(bob_strategy).await["id"]
        .as_str()
        .expect("bob strategy id")
        .to_string();

    assert_eq!(
        start_strategy(&app, &alice_token, &alice_strategy_id)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        start_strategy(&app, &bob_token, &bob_strategy_id)
            .await
            .status(),
        StatusCode::OK
    );

    let stopped = stop_all_strategies(&app, &alice_token).await;
    assert_eq!(stopped.status(), StatusCode::OK);
    assert_eq!(response_json(stopped).await["stopped"], 1);

    let alice_list = list_strategies(&app, &alice_token).await;
    assert_eq!(alice_list.status(), StatusCode::OK);
    let alice_list_body = response_json(alice_list).await;
    assert_eq!(
        find_strategy(&alice_list_body, &alice_strategy_id)["status"],
        "Stopped"
    );

    let bob_list = list_strategies(&app, &bob_token).await;
    assert_eq!(bob_list.status(), StatusCode::OK);
    let bob_list_body = response_json(bob_list).await;
    assert_eq!(
        find_strategy(&bob_list_body, &bob_strategy_id)["status"],
        "Running"
    );
}

#[tokio::test]
async fn regular_user_cannot_create_admin_template() {
    let app = app();
    let user_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let response = create_template(
        &app,
        Some(&user_token),
        strategy_payload(
            "Forbidden Template",
            "BTCUSDT",
            "Custom",
            &[grid_level("100.00", "0.0100", 100, None)],
            true,
            true,
            true,
            None,
            None,
            "Stop",
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

fn strategy_payload(
    name: &str,
    symbol: &str,
    generation: &str,
    levels: &[Value],
    membership_ready: bool,
    exchange_ready: bool,
    symbol_ready: bool,
    overall_take_profit_bps: Option<u32>,
    overall_stop_loss_bps: Option<u32>,
    post_trigger_action: &str,
) -> Value {
    json!({
        "name": name,
        "symbol": symbol,
        "market": "Spot",
        "mode": "SpotClassic",
        "generation": generation,
        "levels": levels,
        "membership_ready": membership_ready,
        "exchange_ready": exchange_ready,
        "symbol_ready": symbol_ready,
        "permissions_ready": true,
        "withdrawals_disabled": true,
        "hedge_mode_ready": true,
        "filters_ready": true,
        "margin_ready": true,
        "conflict_ready": true,
        "balance_ready": true,
        "overall_take_profit_bps": overall_take_profit_bps,
        "overall_stop_loss_bps": overall_stop_loss_bps,
        "post_trigger_action": post_trigger_action,
    })
}

fn grid_level(
    entry_price: &str,
    quantity: &str,
    take_profit_bps: u32,
    trailing_bps: Option<u32>,
) -> Value {
    json!({
        "entry_price": entry_price,
        "quantity": quantity,
        "take_profit_bps": take_profit_bps,
        "trailing_bps": trailing_bps,
    })
}

fn find_strategy<'a>(body: &'a Value, strategy_id: &str) -> &'a Value {
    body["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| item["id"] == strategy_id)
        .expect("strategy present")
}

async fn create_template(
    app: &axum::Router,
    session_token: Option<&str>,
    payload: Value,
) -> axum::response::Response {
    request(app, session_token, "POST", "/admin/templates", payload).await
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

async fn resume_strategy(
    app: &axum::Router,
    session_token: &str,
    strategy_id: &str,
) -> axum::response::Response {
    request(
        app,
        Some(session_token),
        "POST",
        &format!("/strategies/{strategy_id}/resume"),
        json!({}),
    )
    .await
}

async fn stop_strategy(
    app: &axum::Router,
    session_token: &str,
    strategy_id: &str,
) -> axum::response::Response {
    request(
        app,
        Some(session_token),
        "POST",
        &format!("/strategies/{strategy_id}/stop"),
        json!({}),
    )
    .await
}

async fn list_strategy_orders(
    app: &axum::Router,
    session_token: &str,
    strategy_id: &str,
) -> axum::response::Response {
    request(
        app,
        Some(session_token),
        "GET",
        &format!("/strategies/{strategy_id}/orders"),
        Value::Null,
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
