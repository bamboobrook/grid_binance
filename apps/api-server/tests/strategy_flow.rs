use api_server::{app, app_with_state, AppState};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use shared_db::{
    ExchangeWalletSnapshotRecord, MembershipRecord, SharedDb, UserExchangeAccountRecord,
    UserExchangeSymbolRecord,
};
use tower::ServiceExt;

mod support;

use support::register_and_login;

#[tokio::test]
async fn create_strategy_returns_explicit_strategy_type_and_runtime_phase() {
    let app = app();
    let user_token = register_and_login(&app, "strategy-kind@example.com", "pass1234").await;

    let created = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "Typed draft",
            "symbol": "BTCUSDT",
            "market": "Spot",
            "mode": "SpotClassic",
            "generation": "Custom",
            "levels": [grid_level("100.00", "0.0100", 120, None)],
            "overall_take_profit_bps": 500,
            "overall_stop_loss_bps": 200,
            "post_trigger_action": "Stop"
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    let created_body = response_json(created).await;
    assert_eq!(created_body["strategy_type"], "ordinary_grid");
    assert_eq!(created_body["runtime_phase"], "draft");
    assert_eq!(
        created_body["draft_revision"]["reference_price_source"],
        "manual"
    );
}

#[tokio::test]
async fn strategy_create_and_update_accept_payloads_without_client_readiness_flags() {
    let app = app();
    let user_token = register_and_login(&app, "payload@example.com", "pass1234").await;

    let created = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "Payload draft",
            "symbol": "BTCUSDT",
            "market": "Spot",
            "mode": "SpotClassic",
            "generation": "Custom",
            "levels": [grid_level("100.00", "0.0100", 120, None)],
            "overall_take_profit_bps": 500,
            "overall_stop_loss_bps": 200,
            "post_trigger_action": "Stop"
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = response_json(created).await;
    let strategy_id = created_body["id"]
        .as_str()
        .expect("strategy id")
        .to_string();

    let updated = update_strategy(
        &app,
        &user_token,
        &strategy_id,
        json!({
            "name": "Payload updated",
            "symbol": "BTCUSDT",
            "market": "Spot",
            "mode": "SpotClassic",
            "generation": "Arithmetic",
            "levels": [
                grid_level("95.00", "0.0100", 120, None),
                grid_level("100.00", "0.0150", 150, None)
            ],
            "overall_take_profit_bps": 700,
            "overall_stop_loss_bps": 250,
            "post_trigger_action": "Rebuild"
        }),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let updated_body = response_json(updated).await;
    assert_eq!(updated_body["draft_revision"]["generation"], "Arithmetic");
    assert_eq!(
        updated_body["draft_revision"]["post_trigger_action"],
        "Rebuild"
    );
}

#[tokio::test]
async fn ordinary_grid_rejects_bilateral_configuration_fields() {
    let app = app();
    let user_token = register_and_login(&app, "ordinary-validation@example.com", "pass1234").await;

    let response = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "Bad Ordinary",
            "symbol": "BTCUSDT",
            "market": "Spot",
            "mode": "SpotClassic",
            "generation": "Custom",
            "levels": [grid_level("100.00", "0.0100", 120, None)],
            "strategy_type": "ordinary_grid",
            "reference_price": "100.00",
            "levels_per_side": 3,
            "spacing_mode": "geometric",
            "grid_spacing_bps": 100,
            "overall_take_profit_bps": 500,
            "overall_stop_loss_bps": 200,
            "post_trigger_action": "Stop"
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(response_text(response)
        .await
        .contains("ordinary grid does not accept bilateral fields"));
}

#[tokio::test]
async fn classic_bilateral_create_and_list_round_trips_strategy_type() {
    let app = app();
    let user_token = register_and_login(&app, "classic-roundtrip@example.com", "pass1234").await;

    let created = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "Classic Persisted",
            "symbol": "ETHUSDT",
            "market": "FuturesUsdM",
            "mode": "FuturesNeutral",
            "generation": "Custom",
            "amount_mode": "Quote",
            "futures_margin_mode": "Isolated",
            "leverage": 5,
            "strategy_type": "classic_bilateral_grid",
            "reference_price_source": "manual",
            "reference_price": "100.00",
            "levels_per_side": 2,
            "spacing_mode": "fixed_step",
            "grid_spacing_bps": 100,
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    )
    .await;

    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = response_json(created).await;
    assert_eq!(created_body["strategy_type"], "classic_bilateral_grid");
    assert_eq!(
        created_body["draft_revision"]["strategy_type"],
        "classic_bilateral_grid"
    );

    let listed = list_strategies(&app, &user_token).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    let strategy = find_strategy(
        &listed_body,
        created_body["id"].as_str().expect("strategy id"),
    );
    assert_eq!(strategy["strategy_type"], "classic_bilateral_grid");
    assert_eq!(
        strategy["draft_revision"]["strategy_type"],
        "classic_bilateral_grid"
    );
}

#[tokio::test]
async fn futures_classic_bilateral_preflight_fails_without_hedge_mode() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let email = "hedge-check@example.com";
    let user_token = register_and_login(&app, email, "pass1234").await;

    seed_active_membership(&db, email);
    seed_exchange_context(
        &db,
        email,
        &["usdm"],
        true,
        false,
        &[symbol_record(email, "usdm", "ETHUSDT")],
    );

    let created = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "ETH Bilateral",
            "symbol": "ETHUSDT",
            "market": "FuturesUsdM",
            "mode": "FuturesNeutral",
            "generation": "Custom",
            "amount_mode": "Quote",
            "futures_margin_mode": "Isolated",
            "leverage": 5,
            "strategy_type": "classic_bilateral_grid",
            "reference_price_source": "manual",
            "reference_price": "100.00",
            "levels_per_side": 3,
            "spacing_mode": "fixed_step",
            "grid_spacing_bps": 100,
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
        .to_owned();

    let preflight = preflight_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(preflight.status(), StatusCode::OK);
    let body = response_json(preflight).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["failures"][0]["step"], "hedge_mode");
}

#[tokio::test]
async fn strategy_revisions_runtime_contracts_and_soft_archive_follow_frozen_lifecycle() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let email = "trader@example.com";
    let user_token = register_and_login(&app, email, "pass1234").await;
    seed_active_membership(&db, email);

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
    let strategy_id = created_body["id"]
        .as_str()
        .expect("strategy id")
        .to_string();
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
    assert_eq!(
        created_body["draft_revision"]["post_trigger_action"],
        "Rebuild"
    );
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
        created_body["runtime"]["orders"]
            .as_array()
            .expect("orders")
            .len(),
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
        preflight_failed_body["steps"]
            .as_array()
            .expect("steps")
            .len(),
        11
    );
    assert_eq!(
        preflight_failed_body["steps"][0]["step"],
        "membership_status"
    );
    assert_eq!(
        preflight_failed_body["steps"][1]["step"],
        "exchange_connection"
    );
    assert_eq!(
        preflight_failed_body["steps"][2]["step"],
        "exchange_permissions"
    );
    assert_eq!(preflight_failed_body["steps"][0]["status"], "Passed");
    assert_eq!(preflight_failed_body["steps"][1]["status"], "Failed");
    assert_eq!(preflight_failed_body["steps"][2]["status"], "Skipped");

    seed_exchange_context(
        &db,
        email,
        &["spot"],
        true,
        true,
        &[symbol_record(email, "spot", "BTCUSDT")],
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
    let started_orders = started_body["runtime"]["orders"]
        .as_array()
        .expect("orders");
    assert_eq!(started_orders.len(), 2);
    assert_eq!(started_orders[0]["side"], "Buy");
    assert_eq!(started_orders[1]["side"], "Sell");
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
    assert_eq!(orders_body["orders"].as_array().expect("orders").len(), 2);
    assert_eq!(
        orders_body["positions"]
            .as_array()
            .expect("positions")
            .len(),
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
        2
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
        stopped_body["runtime"]["fills"]
            .as_array()
            .expect("fills")
            .len(),
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
    assert!(listed_after_delete_body["items"]
        .as_array()
        .expect("items")
        .iter()
        .all(|item| item["id"] != strategy_id));
}

#[tokio::test]
async fn draft_lifecycle_actions_report_clear_failures_and_allow_delete() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db).expect("state"));
    let email = "draft-lifecycle@example.com";
    let user_token = register_and_login(&app, email, "pass1234").await;

    let created = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "Draft lifecycle",
            "symbol": "BTCUSDT",
            "market": "Spot",
            "mode": "SpotClassic",
            "generation": "Custom",
            "levels": [grid_level("100.00", "0.0100", 120, None)],
            "overall_take_profit_bps": 500,
            "overall_stop_loss_bps": 200,
            "post_trigger_action": "Stop"
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let strategy_id = response_json(created).await["id"]
        .as_str()
        .expect("strategy id")
        .to_string();

    let paused = pause_strategies(&app, &user_token, &[&strategy_id]).await;
    assert_eq!(paused.status(), StatusCode::OK);
    let paused_body = response_json(paused).await;
    assert_eq!(paused_body["paused"], 0);
    assert_eq!(paused_body["failures"][0]["strategy_id"], strategy_id);
    assert_eq!(
        paused_body["failures"][0]["error"],
        "Strategy has not started yet; only running strategies can be paused."
    );

    let stopped = stop_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(stopped.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(stopped).await["error"],
        "Stop is only available for running or paused strategies; this draft has never been started."
    );

    let deleted = delete_strategies(&app, &user_token, &[&strategy_id]).await;
    assert_eq!(deleted.status(), StatusCode::OK);
    let deleted_body = response_json(deleted).await;
    assert_eq!(deleted_body["deleted"], 1);
    assert_eq!(
        deleted_body["failures"].as_array().expect("failures").len(),
        0
    );
}

#[tokio::test]
async fn draft_strategy_can_be_deleted_without_starting() {
    let app = app();
    let user_token = register_and_login(&app, "draft-delete@example.com", "pass1234").await;

    let created = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "draft delete",
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
    assert_eq!(created.status(), StatusCode::CREATED);
    let strategy_id = response_json(created).await["id"]
        .as_str()
        .expect("strategy id")
        .to_string();

    let deleted = delete_strategies(&app, &user_token, &[&strategy_id]).await;
    assert_eq!(deleted.status(), StatusCode::OK);
    assert_eq!(response_json(deleted).await["deleted"], 1);

    let listed_after_delete = list_strategies(&app, &user_token).await;
    assert_eq!(listed_after_delete.status(), StatusCode::OK);
    let listed_body = response_json(listed_after_delete).await;
    assert!(listed_body["items"]
        .as_array()
        .expect("items")
        .iter()
        .all(|item| item["id"] != strategy_id));
}

#[tokio::test]
async fn running_strategy_delete_reports_blocker() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let email = "running-delete@example.com";
    let user_token = register_and_login(&app, email, "pass1234").await;
    seed_active_membership(&db, email);
    seed_exchange_context(
        &db,
        email,
        &["spot"],
        true,
        true,
        &[symbol_record(email, "spot", "BTCUSDT")],
    );

    let created = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "running delete",
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
    assert_eq!(created.status(), StatusCode::CREATED);
    let strategy_id = response_json(created).await["id"]
        .as_str()
        .expect("strategy id")
        .to_string();

    assert_eq!(
        start_strategy(&app, &user_token, &strategy_id)
            .await
            .status(),
        StatusCode::OK
    );

    let deleted = delete_strategies(&app, &user_token, &[&strategy_id]).await;
    assert_eq!(deleted.status(), StatusCode::OK);
    let deleted_body = response_json(deleted).await;
    assert_eq!(deleted_body["deleted"], 0);
    assert_eq!(
        deleted_body["failures"][0]["error"],
        "Strategy must be stopped before it can be deleted."
    );
}
#[tokio::test]
async fn batch_start_starts_draft_stopped_and_paused_strategies() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let email = "batchstart@example.com";
    let user_token = register_and_login(&app, email, "pass1234").await;
    seed_active_membership(&db, email);
    seed_exchange_context(
        &db,
        email,
        &["spot"],
        true,
        true,
        &[
            symbol_record(email, "spot", "BTCUSDT"),
            symbol_record(email, "spot", "ETHUSDT"),
            symbol_record(email, "spot", "SOLUSDT"),
        ],
    );

    let draft = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "draft-a",
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
    assert_eq!(draft.status(), StatusCode::CREATED);
    let draft_id = response_json(draft).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let paused = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "paused-a",
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
    assert_eq!(paused.status(), StatusCode::CREATED);
    let paused_id = response_json(paused).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        start_strategy(&app, &user_token, &paused_id).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        pause_strategies(&app, &user_token, &[&paused_id])
            .await
            .status(),
        StatusCode::OK
    );

    let stopped = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "stopped-a",
            "SOLUSDT",
            "Custom",
            &[grid_level("50.00", "0.0100", 120, None)],
            true,
            true,
            true,
            None,
            None,
            "Stop",
        ),
    )
    .await;
    assert_eq!(stopped.status(), StatusCode::CREATED);
    let stopped_id = response_json(stopped).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        start_strategy(&app, &user_token, &stopped_id)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        stop_strategy(&app, &user_token, &stopped_id).await.status(),
        StatusCode::OK
    );

    let started = start_strategies(&app, &user_token, &[&draft_id, &paused_id, &stopped_id]).await;
    assert_eq!(started.status(), StatusCode::OK);
    assert_eq!(response_json(started).await["started"], 3);

    let listed = list_strategies(&app, &user_token).await;
    let body = response_json(listed).await;
    assert_eq!(find_strategy(&body, &draft_id)["status"], "Running");
    assert_eq!(find_strategy(&body, &paused_id)["status"], "Running");
    assert_eq!(find_strategy(&body, &stopped_id)["status"], "Running");
}

#[tokio::test]
async fn preflight_prefers_server_side_symbol_and_conflict_truth_for_futures() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let email = "truthy@example.com";
    let user_token = register_and_login(&app, email, "pass1234").await;

    seed_active_membership(&db, email);
    seed_exchange_context(
        &db,
        email,
        &["spot", "usdm", "coinm"],
        true,
        true,
        &[
            symbol_record(email, "spot", "BTCUSDT"),
            symbol_record(email, "usdm", "BTCUSDT"),
            symbol_record(email, "coinm", "BTCUSD_PERP"),
        ],
    );
    db.insert_exchange_wallet_snapshot(&ExchangeWalletSnapshotRecord {
        user_email: email.to_string(),
        exchange: "binance-usdm".to_string(),
        wallet_type: "usdm".to_string(),
        balances: json!({ "USDT": "1000" }),
        captured_at: Utc::now(),
    })
    .expect("seed usdm wallet");

    let unsupported_symbol = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "missing-symbol",
            "symbol": "ADAUSDT",
            "market": "FuturesUsdM",
            "mode": "FuturesLong",
            "generation": "Custom",
            "amount_mode": "Quote",
            "futures_margin_mode": "Isolated",
            "leverage": 5,
            "levels": [{ "entry_price": "100.00", "quantity": "0.0100", "take_profit_bps": 120, "trailing_bps": null }],
            "membership_ready": true,
            "exchange_ready": true,
            "permissions_ready": true,
            "withdrawals_disabled": true,
            "hedge_mode_ready": true,
            "symbol_ready": true,
            "filters_ready": true,
            "margin_ready": true,
            "conflict_ready": true,
            "balance_ready": true,
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    ).await;
    assert_eq!(unsupported_symbol.status(), StatusCode::CREATED);
    let unsupported_id = response_json(unsupported_symbol).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let unsupported_preflight = preflight_strategy(&app, &user_token, &unsupported_id).await;
    let unsupported_body = response_json(unsupported_preflight).await;
    assert_eq!(unsupported_body["failures"][0]["step"], "symbol_support");

    let first_long = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "first-long",
            "symbol": "BTCUSDT",
            "market": "FuturesUsdM",
            "mode": "FuturesLong",
            "generation": "Custom",
            "amount_mode": "Quote",
            "futures_margin_mode": "Isolated",
            "leverage": 5,
            "levels": [{ "entry_price": "100.00", "quantity": "0.0100", "take_profit_bps": 120, "trailing_bps": null }],
            "membership_ready": true,
            "exchange_ready": true,
            "permissions_ready": true,
            "withdrawals_disabled": true,
            "hedge_mode_ready": true,
            "symbol_ready": true,
            "filters_ready": true,
            "margin_ready": true,
            "conflict_ready": true,
            "balance_ready": true,
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    ).await;
    assert_eq!(first_long.status(), StatusCode::CREATED);
    let first_long_id = response_json(first_long).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        start_strategy(&app, &user_token, &first_long_id)
            .await
            .status(),
        StatusCode::OK
    );

    let conflicting_long = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "second-long",
            "symbol": "BTCUSDT",
            "market": "FuturesUsdM",
            "mode": "FuturesLong",
            "generation": "Custom",
            "amount_mode": "Quote",
            "futures_margin_mode": "Isolated",
            "leverage": 5,
            "levels": [{ "entry_price": "101.00", "quantity": "0.0100", "take_profit_bps": 120, "trailing_bps": null }],
            "membership_ready": true,
            "exchange_ready": true,
            "permissions_ready": true,
            "withdrawals_disabled": true,
            "hedge_mode_ready": true,
            "symbol_ready": true,
            "filters_ready": true,
            "margin_ready": true,
            "conflict_ready": true,
            "balance_ready": true,
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    ).await;
    assert_eq!(conflicting_long.status(), StatusCode::CREATED);
    let conflicting_id = response_json(conflicting_long).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let conflicting_preflight = preflight_strategy(&app, &user_token, &conflicting_id).await;
    let conflicting_body = response_json(conflicting_preflight).await;
    assert_eq!(
        conflicting_body["failures"][0]["step"],
        "strategy_conflicts"
    );
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
    assert_eq!(listed_body["items"].as_array().expect("items").len(), 3);
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
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let email = "futures@example.com";
    let user_token = register_and_login(&app, email, "pass1234").await;
    seed_active_membership(&db, email);
    seed_exchange_context(
        &db,
        email,
        &["usdm"],
        false,
        false,
        &[symbol_record(email, "usdm", "BTCUSDT")],
    );

    let created = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "futures draft",
            "symbol": "BTCUSDT",
            "market": "FuturesUsdM",
            "mode": "FuturesLong",
            "generation": "Custom",
            "amount_mode": "Quote",
            "futures_margin_mode": "Isolated",
            "leverage": 5,
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
    assert_eq!(body["steps"].as_array().expect("steps").len(), 11);
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
async fn strategy_creation_rejects_market_and_mode_mismatch() {
    let app = app();
    let user_token = register_and_login(&app, "mode-check@example.com", "pass1234").await;

    let invalid_spot = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "bad spot",
            "symbol": "BTCUSDT",
            "market": "Spot",
            "mode": "FuturesLong",
            "generation": "Custom",
            "levels": [grid_level("100.00", "0.0100", 120, Option::<u32>::None)],
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    )
    .await;
    assert_eq!(invalid_spot.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(invalid_spot).await["error"],
        "market and mode are incompatible"
    );

    let invalid_futures = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "bad futures",
            "symbol": "BTCUSDT",
            "market": "FuturesUsdM",
            "mode": "SpotClassic",
            "generation": "Custom",
            "amount_mode": "Quote",
            "futures_margin_mode": "Isolated",
            "leverage": 5,
            "levels": [grid_level("100.00", "0.0100", 120, Option::<u32>::None)],
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    )
    .await;
    assert_eq!(invalid_futures.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(invalid_futures).await["error"],
        "market and mode are incompatible"
    );
}

#[tokio::test]
async fn futures_preflight_uses_market_scoped_wallet_and_margin_rules() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let email = "wallet-scope@example.com";
    let user_token = register_and_login(&app, email, "pass1234").await;
    seed_active_membership(&db, email);
    seed_exchange_context(
        &db,
        email,
        &["usdm"],
        true,
        true,
        &[symbol_record(email, "usdm", "BTCUSDT")],
    );
    let now = Utc::now();
    db.insert_exchange_wallet_snapshot(&ExchangeWalletSnapshotRecord {
        user_email: email.to_string(),
        exchange: "binance".to_string(),
        wallet_type: "spot".to_string(),
        balances: json!({ "USDT": "0" }),
        captured_at: now + Duration::seconds(1),
    })
    .expect("generic wallet snapshot");
    db.insert_exchange_wallet_snapshot(&ExchangeWalletSnapshotRecord {
        user_email: email.to_string(),
        exchange: "binance-usdm".to_string(),
        wallet_type: "usdm".to_string(),
        balances: json!({ "USDT": "10" }),
        captured_at: now + Duration::seconds(2),
    })
    .expect("futures wallet snapshot");

    let created = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "futures scoped wallet",
            "symbol": "BTCUSDT",
            "market": "FuturesUsdM",
            "mode": "FuturesLong",
            "generation": "Custom",
            "amount_mode": "Quote",
            "futures_margin_mode": "Isolated",
            "leverage": 7,
            "levels": [{
                "entry_price": "100.00",
                "quantity": "0.0100",
                "take_profit_bps": 120,
                "trailing_bps": null
            }],
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

    let invalid_margin = preflight_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(invalid_margin.status(), StatusCode::OK);
    let invalid_body = response_json(invalid_margin).await;
    assert_eq!(invalid_body["ok"], false);
    assert_eq!(invalid_body["steps"][7]["step"], "margin_or_leverage");
    assert_eq!(invalid_body["steps"][7]["status"], "Failed");
    assert_eq!(invalid_body["failures"][0]["step"], "margin_or_leverage");

    let updated = update_strategy(
        &app,
        &user_token,
        &strategy_id,
        json!({
            "name": "futures scoped wallet",
            "symbol": "BTCUSDT",
            "market": "FuturesUsdM",
            "mode": "FuturesLong",
            "generation": "Custom",
            "amount_mode": "Quote",
            "futures_margin_mode": "Isolated",
            "leverage": 5,
            "levels": [{
                "entry_price": "100.00",
                "quantity": "0.0100",
                "take_profit_bps": 120,
                "trailing_bps": null
            }],
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);

    let valid_preflight = preflight_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(valid_preflight.status(), StatusCode::OK);
    let valid_body = response_json(valid_preflight).await;
    assert_eq!(valid_body["ok"], true);
    assert_eq!(valid_body["steps"][7]["status"], "Passed");
    assert_eq!(valid_body["steps"][9]["status"], "Passed");
}

#[tokio::test]
async fn futures_neutral_strategy_start_exposes_dual_sided_orders() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let email = "neutral@example.com";
    let user_token = register_and_login(&app, email, "pass1234").await;
    seed_active_membership(&db, email);
    seed_exchange_context(
        &db,
        email,
        &["usdm"],
        true,
        true,
        &[symbol_record(email, "usdm", "BTCUSDT")],
    );

    let created = create_strategy(
        &app,
        &user_token,
        json!({
            "name": "neutral",
            "symbol": "BTCUSDT",
            "market": "FuturesUsdM",
            "mode": "FuturesNeutral",
            "generation": "Custom",
            "amount_mode": "Quote",
            "futures_margin_mode": "Isolated",
            "leverage": 5,
            "levels": [
                { "entry_price": "100.00", "quantity": "0.0100", "take_profit_bps": 120, "trailing_bps": null },
                { "entry_price": "101.00", "quantity": "0.0100", "take_profit_bps": 120, "trailing_bps": null }
            ],
            "membership_ready": true,
            "exchange_ready": true,
            "permissions_ready": true,
            "withdrawals_disabled": true,
            "hedge_mode_ready": true,
            "symbol_ready": true,
            "filters_ready": true,
            "margin_ready": true,
            "conflict_ready": true,
            "balance_ready": true,
            "overall_take_profit_bps": null,
            "overall_stop_loss_bps": null,
            "post_trigger_action": "Stop"
        }),
    ).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let strategy_id = response_json(created).await["id"]
        .as_str()
        .expect("strategy id")
        .to_string();

    let started = start_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(started.status(), StatusCode::OK);
    let body = response_json(started).await;
    assert_eq!(body["runtime"]["orders"][0]["side"], "Buy");
    assert_eq!(body["runtime"]["orders"][1]["side"], "Sell");
}

#[tokio::test]
async fn preflight_prefers_server_derived_membership_exchange_and_symbol_truths() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let email = "server-truth@example.com";
    let user_token = register_and_login(&app, email, "pass1234").await;

    seed_active_membership(&db, email);
    seed_exchange_context(
        &db,
        email,
        &["spot"],
        true,
        true,
        &[symbol_record(email, "spot", "BTCUSDT")],
    );

    let created = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "server truth",
            "BTCUSDT",
            "Custom",
            &[grid_level("100.00", "0.0100", 120, None)],
            false,
            false,
            false,
            None,
            None,
            "Stop",
        ),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let strategy_id = response_json(created).await["id"]
        .as_str()
        .expect("strategy id")
        .to_string();

    let preflight = preflight_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(preflight.status(), StatusCode::OK);
    let preflight_body = response_json(preflight).await;
    assert_eq!(preflight_body["ok"], true);
    assert_eq!(preflight_body["steps"][0]["step"], "membership_status");
    assert_eq!(preflight_body["steps"][0]["status"], "Passed");
    assert_eq!(preflight_body["steps"][1]["step"], "exchange_connection");
    assert_eq!(preflight_body["steps"][1]["status"], "Passed");
    assert_eq!(preflight_body["steps"][5]["step"], "symbol_support");
    assert_eq!(preflight_body["steps"][5]["status"], "Passed");

    let started = start_strategy(&app, &user_token, &strategy_id).await;
    assert_eq!(started.status(), StatusCode::OK);
    assert_eq!(response_json(started).await["preflight"]["ok"], true);
}

#[tokio::test]
async fn batch_start_uses_server_preflight_truth_and_starts_multiple_strategies() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let email = "batch-start@example.com";
    let user_token = register_and_login(&app, email, "pass1234").await;

    seed_active_membership(&db, email);
    seed_exchange_context(
        &db,
        email,
        &["spot"],
        true,
        true,
        &[
            symbol_record(email, "spot", "BTCUSDT"),
            symbol_record(email, "spot", "ETHUSDT"),
        ],
    );

    let btc = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "btc",
            "BTCUSDT",
            "Custom",
            &[grid_level("100.00", "0.0100", 120, None)],
            false,
            false,
            false,
            None,
            None,
            "Stop",
        ),
    )
    .await;
    assert_eq!(btc.status(), StatusCode::CREATED);
    let btc_id = response_json(btc).await["id"]
        .as_str()
        .expect("btc id")
        .to_string();

    let eth = create_strategy(
        &app,
        &user_token,
        strategy_payload(
            "eth",
            "ETHUSDT",
            "Custom",
            &[grid_level("200.00", "0.0100", 120, None)],
            false,
            false,
            false,
            None,
            None,
            "Stop",
        ),
    )
    .await;
    assert_eq!(eth.status(), StatusCode::CREATED);
    let eth_id = response_json(eth).await["id"]
        .as_str()
        .expect("eth id")
        .to_string();

    let started = batch_start_strategies(&app, &user_token, &[&btc_id, &eth_id]).await;
    assert_eq!(started.status(), StatusCode::OK);
    let body = response_json(started).await;
    assert_eq!(body["started"], 2);
    assert_eq!(body["items"].as_array().expect("items").len(), 2);
    assert_eq!(body["failures"].as_array().expect("failures").len(), 0);

    let listed = list_strategies(&app, &user_token).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    assert_eq!(find_strategy(&listed_body, &btc_id)["status"], "Running");
    assert_eq!(find_strategy(&listed_body, &eth_id)["status"], "Running");
}

#[tokio::test]
async fn futures_same_symbol_same_direction_conflict_is_checked_server_side() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let email = "futures-conflict@example.com";
    let user_token = register_and_login(&app, email, "pass1234").await;

    seed_active_membership(&db, email);
    seed_exchange_context(
        &db,
        email,
        &["usdm"],
        true,
        true,
        &[symbol_record(email, "usdm", "BTCUSDT")],
    );

    let first = create_strategy(
        &app,
        &user_token,
        futures_strategy_payload("leader", "BTCUSDT", "FuturesLong"),
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_id = response_json(first).await["id"]
        .as_str()
        .expect("first id")
        .to_string();
    assert_eq!(
        start_strategy(&app, &user_token, &first_id).await.status(),
        StatusCode::OK
    );

    let second = create_strategy(
        &app,
        &user_token,
        futures_strategy_payload("follower", "BTCUSDT", "FuturesLong"),
    )
    .await;
    assert_eq!(second.status(), StatusCode::CREATED);
    let second_id = response_json(second).await["id"]
        .as_str()
        .expect("second id")
        .to_string();

    let preflight = preflight_strategy(&app, &user_token, &second_id).await;
    assert_eq!(preflight.status(), StatusCode::OK);
    let preflight_body = response_json(preflight).await;
    assert_eq!(preflight_body["ok"], false);
    assert_eq!(preflight_body["failures"][0]["step"], "strategy_conflicts");
    assert_eq!(preflight_body["steps"][8]["status"], "Failed");

    let started = start_strategy(&app, &user_token, &second_id).await;
    assert_eq!(started.status(), StatusCode::CONFLICT);
    let started_body = response_json(started).await;
    assert_eq!(started_body["error"], "preflight failed");
    assert_eq!(
        started_body["preflight"]["failures"][0]["step"],
        "strategy_conflicts"
    );
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
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let alice_email = "alice@example.com";
    let bob_email = "bob@example.com";
    let alice_token = register_and_login(&app, alice_email, "pass1234").await;
    let bob_token = register_and_login(&app, bob_email, "pass1234").await;
    seed_active_membership(&db, alice_email);
    seed_active_membership(&db, bob_email);
    seed_exchange_context(
        &db,
        alice_email,
        &["spot"],
        true,
        true,
        &[symbol_record(alice_email, "spot", "BTCUSDT")],
    );
    seed_exchange_context(
        &db,
        bob_email,
        &["spot"],
        true,
        true,
        &[symbol_record(bob_email, "spot", "ETHUSDT")],
    );

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

async fn start_strategies(
    app: &axum::Router,
    session_token: &str,
    ids: &[&str],
) -> axum::response::Response {
    request(
        app,
        Some(session_token),
        "POST",
        "/strategies/batch/start",
        json!({ "ids": ids }),
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

async fn batch_start_strategies(
    app: &axum::Router,
    session_token: &str,
    ids: &[&str],
) -> axum::response::Response {
    request(
        app,
        Some(session_token),
        "POST",
        "/strategies/batch/start",
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

async fn response_text(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    String::from_utf8(bytes.to_vec()).expect("utf8 body")
}

fn seed_active_membership(db: &SharedDb, email: &str) {
    let now = Utc::now();
    db.upsert_membership_record(
        email,
        &MembershipRecord {
            activated_at: Some(now),
            active_until: Some(now + Duration::days(30)),
            grace_until: Some(now + Duration::days(32)),
            override_status: None,
        },
    )
    .expect("seed membership");
}

fn seed_exchange_context(
    db: &SharedDb,
    email: &str,
    selected_markets: &[&str],
    permissions_ok: bool,
    hedge_mode_ok: bool,
    symbols: &[UserExchangeSymbolRecord],
) {
    let now = Utc::now();
    db.upsert_exchange_account(&UserExchangeAccountRecord {
        user_email: email.to_string(),
        exchange: "binance".to_string(),
        account_label: "Binance".to_string(),
        market_scope: selected_markets.join(","),
        is_active: true,
        checked_at: Some(now),
        metadata: json!({
            "connection_status": "connected",
            "sync_status": "success",
            "last_synced_at": now.to_rfc3339(),
            "expected_hedge_mode": true,
            "selected_markets": selected_markets,
            "validation": {
                "api_connectivity_ok": true,
                "timestamp_in_sync": true,
                "can_read_spot": selected_markets.contains(&"spot"),
                "can_read_usdm": selected_markets.contains(&"usdm"),
                "can_read_coinm": selected_markets.contains(&"coinm"),
                "hedge_mode_ok": hedge_mode_ok,
                "permissions_ok": permissions_ok,
                "withdrawals_disabled": true,
                "market_access_ok": true
            },
            "symbol_counts": {
                "spot": symbols.iter().filter(|symbol| symbol.market == "spot").count(),
                "usdm": symbols.iter().filter(|symbol| symbol.market == "usdm").count(),
                "coinm": symbols.iter().filter(|symbol| symbol.market == "coinm").count()
            }
        }),
    })
    .expect("seed account");
    db.replace_exchange_symbols(email, "binance", symbols)
        .expect("seed symbols");
    let mut balances = json!({ "USDT": "1000", "USD": "1000" });
    if let Some(map) = balances.as_object_mut() {
        for symbol in symbols.iter().filter(|symbol| symbol.market == "spot") {
            map.entry(symbol.base_asset.clone())
                .or_insert_with(|| json!("100"));
        }
    }
    db.insert_exchange_wallet_snapshot(&ExchangeWalletSnapshotRecord {
        user_email: email.to_string(),
        exchange: "binance".to_string(),
        wallet_type: selected_markets
            .first()
            .copied()
            .unwrap_or("spot")
            .to_string(),
        balances,
        captured_at: now,
    })
    .expect("seed wallet");
}

fn symbol_record(email: &str, market: &str, symbol: &str) -> UserExchangeSymbolRecord {
    let now = Utc::now();
    UserExchangeSymbolRecord {
        user_email: email.to_string(),
        exchange: "binance".to_string(),
        market: market.to_string(),
        symbol: symbol.to_string(),
        status: "TRADING".to_string(),
        base_asset: symbol.trim_end_matches("USDT").to_string(),
        quote_asset: "USDT".to_string(),
        price_precision: 2,
        quantity_precision: 4,
        min_quantity: "0.001".to_string(),
        min_notional: "0.5".to_string(),
        keywords: vec![symbol.to_lowercase(), market.to_string()],
        metadata: json!({
            "symbol": symbol,
            "market": market,
            "status": "TRADING",
            "base_asset": symbol.trim_end_matches("USDT"),
            "quote_asset": "USDT",
            "price_precision": 2,
            "quantity_precision": 4,
            "filters": {
                "price_tick_size": "0.01",
                "quantity_step_size": "0.001",
                "min_quantity": "0.001",
                "min_notional": "0.5",
                "contract_size": null
            },
            "market_requirements": {
                "supports_isolated_margin": true,
                "supports_cross_margin": true,
                "hedge_mode_required": market != "spot",
                "requires_futures_permissions": market != "spot",
                "leverage_brackets": [1, 5, 10]
            },
            "keywords": [symbol.to_lowercase(), market]
        }),
        synced_at: now,
    }
}

fn futures_strategy_payload(name: &str, symbol: &str, mode: &str) -> Value {
    json!({
        "name": name,
        "symbol": symbol,
        "market": "FuturesUsdM",
        "mode": mode,
        "generation": "Custom",
        "amount_mode": "Quote",
        "futures_margin_mode": "Isolated",
        "leverage": 5,
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
        "permissions_ready": true,
        "withdrawals_disabled": true,
        "hedge_mode_ready": true,
        "symbol_ready": true,
        "filters_ready": true,
        "margin_ready": true,
        "conflict_ready": true,
        "balance_ready": true,
        "overall_take_profit_bps": null,
        "overall_stop_loss_bps": null,
        "post_trigger_action": "Stop"
    })
}
