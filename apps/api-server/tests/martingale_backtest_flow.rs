mod support;

use api_server::{AppState, app, app_with_state};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use shared_db::{NewBacktestCandidateRecord, SharedDb};
use support::register_and_login;
use tower::ServiceExt;

#[tokio::test]
async fn user_can_create_martingale_backtest_task() {
    let app = app();
    let token = register_and_login(&app, "backtest-create@example.com", "pass1234").await;

    let response = authed_json(
        &app,
        "POST",
        "/backtest/tasks",
        &token,
        json!({
            "strategy_type": "martingale_grid",
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "market": "usd_m_futures",
            "timeframe": "1h",
            "start_date": "2024-01-01",
            "end_date": "2024-02-01",
            "search": { "mode": "random", "samples": 10 }
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response_json(response).await;
    assert_eq!(body["status"], "queued");
    assert_eq!(body["strategy_type"], "martingale_grid");
    assert!(
        body["task_id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("bt_")
    );
}

#[tokio::test]
async fn quota_rejects_too_many_symbols() {
    let app = app();
    let token = register_and_login(&app, "backtest-quota@example.com", "pass1234").await;

    let response = authed_json(
        &app,
        "POST",
        "/backtest/tasks",
        &token,
        json!({
            "strategy_type": "martingale_grid",
            "symbols": ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "ADAUSDT", "XRPUSDT"],
            "market": "usd_m_futures",
            "timeframe": "1h",
            "start_date": "2024-01-01",
            "end_date": "2024-02-01"
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response_json(response).await;
    assert!(body["error"].as_str().unwrap_or_default().contains("quota"));
}

#[tokio::test]
async fn task_pause_resume_cancel_transitions_status() {
    let app = app();
    let token = register_and_login(&app, "backtest-actions@example.com", "pass1234").await;

    let created = authed_json(
        &app,
        "POST",
        "/backtest/tasks",
        &token,
        json!({
            "strategy_type": "martingale_grid",
            "symbols": ["BTCUSDT"],
            "market": "spot",
            "timeframe": "1h",
            "start_date": "2024-01-01",
            "end_date": "2024-01-10"
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let task_id = response_json(created).await["task_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let pause_queued = authed_empty(
        &app,
        "POST",
        &format!("/backtest/tasks/{task_id}/pause"),
        &token,
    )
    .await;
    assert_eq!(pause_queued.status(), StatusCode::CONFLICT);

    let resume_queued = authed_empty(
        &app,
        "POST",
        &format!("/backtest/tasks/{task_id}/resume"),
        &token,
    )
    .await;
    assert_eq!(resume_queued.status(), StatusCode::CONFLICT);

    let cancelled = authed_empty(
        &app,
        "POST",
        &format!("/backtest/tasks/{task_id}/cancel"),
        &token,
    )
    .await;
    assert_eq!(cancelled.status(), StatusCode::OK);
    assert_eq!(response_json(cancelled).await["status"], "cancelled");
}

#[tokio::test]
async fn user_can_archive_and_delete_cancelled_backtest_task() {
    let app = app();
    let token = register_and_login(&app, "backtest-manage@example.com", "pass1234").await;
    let task_id = create_task_with_portfolio(&app, &token, futures_portfolio_config(3)).await;

    let archived = authed_empty(
        &app,
        "POST",
        &format!("/backtest/tasks/{task_id}/archive"),
        &token,
    )
    .await;
    assert_eq!(archived.status(), StatusCode::OK);
    assert_eq!(response_json(archived).await["summary"]["archived"], true);

    let delete_active = authed_empty(
        &app,
        "DELETE",
        &format!("/backtest/tasks/{task_id}"),
        &token,
    )
    .await;
    assert_eq!(delete_active.status(), StatusCode::CONFLICT);

    let cancelled = authed_empty(
        &app,
        "POST",
        &format!("/backtest/tasks/{task_id}/cancel"),
        &token,
    )
    .await;
    assert_eq!(cancelled.status(), StatusCode::OK);

    let deleted = authed_empty(
        &app,
        "DELETE",
        &format!("/backtest/tasks/{task_id}"),
        &token,
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::OK);
    let body = response_json(deleted).await;
    assert_eq!(body["task_id"], task_id);
    assert_eq!(body["deleted"], true);

    let missing = authed_empty(&app, "GET", &format!("/backtest/tasks/{task_id}"), &token).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn task_creation_does_not_publish_unverified_placeholder_candidates() {
    let app = app();
    let token = register_and_login(&app, "backtest-no-placeholder@example.com", "pass1234").await;

    let task_id = create_task_with_portfolio(&app, &token, futures_portfolio_config(3)).await;
    let candidates = authed_empty(
        &app,
        "GET",
        &format!("/backtest/tasks/{task_id}/candidates"),
        &token,
    )
    .await;

    assert_eq!(candidates.status(), StatusCode::OK);
    assert_eq!(response_json(candidates).await, json!([]));
}

#[tokio::test]
async fn publish_intent_returns_risk_summary() {
    let db = SharedDb::ephemeral().expect("db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let token = register_and_login(&app, "backtest-publish@example.com", "pass1234").await;

    let task_id = create_task_with_portfolio(&app, &token, futures_portfolio_config(3)).await;
    let candidate_id = save_ready_candidate(&db, &task_id, futures_portfolio_config(3));

    let intent = authed_json(
        &app,
        "POST",
        &format!("/backtest/candidates/{candidate_id}/publish-intent"),
        &token,
        json!({}),
    )
    .await;

    assert_eq!(intent.status(), StatusCode::OK);
    let body = response_json(intent).await;
    assert_eq!(body["status"], "pending_confirmation");
    assert_eq!(body["risk_summary"]["strategy_count"], 1);
    assert_eq!(body["risk_summary"]["symbols"], json!(["BTCUSDT"]));
}

#[tokio::test]
async fn martingale_dynamic_publish_preserves_rules_and_reports_live_readiness() {
    let db = SharedDb::ephemeral().expect("db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let token = register_and_login(&app, "backtest-dynamic-publish@example.com", "pass1234").await;

    let task_id = create_task_with_portfolio(&app, &token, futures_portfolio_config(3)).await;
    let candidate_id = save_ready_candidate(&db, &task_id, futures_portfolio_config(3));
    db.backtest_repo()
        .transition_task(&task_id, "succeeded")
        .expect("succeeded");

    let dynamic_allocation_rules = json!({
        "btc_filter": true,
        "funding_rate_used": false,
        "timeframes": ["4h", "1d"],
        "existing_position_policy": "tiered_pause_cancel_force_exit"
    });
    let response = authed_json(
        &app,
        "POST",
        "/backtest/portfolios/publish",
        &token,
        json!({
            "name": "Dynamic BTC basket",
            "task_id": task_id,
            "market": "usd_m_futures",
            "direction": "long_short",
            "risk_profile": "dynamic_long_short",
            "total_weight_pct": 100,
            "dynamic_allocation_rules": dynamic_allocation_rules,
            "items": [{
                "candidate_id": candidate_id,
                "symbol": "BTCUSDT",
                "weight_pct": 100,
                "leverage": 3,
                "enabled": true,
                "parameter_snapshot": { "direction_mode": "long_and_short" }
            }]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response_json(response).await;
    assert_eq!(body["dynamic_allocation_rules"], dynamic_allocation_rules);
    assert_eq!(
        body["risk_summary"]["dynamic_allocation_rules"],
        dynamic_allocation_rules
    );
    assert!(
        body["live_ready"].as_bool() == Some(true)
            || body["live_readiness_blockers"]
                .as_array()
                .is_some_and(|blockers| !blockers.is_empty()
                    && blockers
                        .iter()
                        .all(|blocker| blocker.as_str().is_some_and(|text| !text.is_empty()))),
        "response must expose either live_ready=true or human-readable blockers: {body:?}"
    );
}

#[tokio::test]
async fn martingale_dynamic_publish_without_rules_blocks_confirm_start() {
    let db = SharedDb::ephemeral().expect("db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let token = register_and_login(
        &app,
        "backtest-dynamic-missing-rules@example.com",
        "pass1234",
    )
    .await;

    let task_id = create_task_with_portfolio(&app, &token, futures_portfolio_config(3)).await;
    let candidate_id = save_ready_candidate(&db, &task_id, futures_portfolio_config(3));
    db.backtest_repo()
        .transition_task(&task_id, "succeeded")
        .expect("succeeded");

    let response = authed_json(
        &app,
        "POST",
        "/backtest/portfolios/publish",
        &token,
        json!({
            "name": "Dynamic missing rules basket",
            "task_id": task_id,
            "market": "usd_m_futures",
            "direction": "long_short",
            "risk_profile": "dynamic_long_short",
            "total_weight_pct": 100,
            "items": [{
                "candidate_id": candidate_id,
                "symbol": "BTCUSDT",
                "weight_pct": 100,
                "leverage": 3,
                "enabled": true,
                "parameter_snapshot": { "direction_mode": "long_and_short" }
            }]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response_json(response).await;
    assert_eq!(body["live_ready"], false);
    assert!(
        body["live_readiness_blockers"]
            .as_array()
            .is_some_and(|blockers| blockers.iter().any(|blocker| blocker
                .as_str()
                .is_some_and(|text| text.contains("dynamic allocation rules are required"))))
    );
    let portfolio_id = body["portfolio_id"].as_str().unwrap().to_owned();

    let started = authed_empty(
        &app,
        "POST",
        &format!("/backtest/portfolios/{portfolio_id}/confirm-start"),
        &token,
    )
    .await;
    assert_eq!(started.status(), StatusCode::CONFLICT);
    assert!(
        response_json(started).await["error"]
            .as_str()
            .unwrap_or_default()
            .contains("not live-ready")
    );
}

#[tokio::test]
async fn martingale_dynamic_publish_with_invalid_rules_type_blocks_confirm_start() {
    let db = SharedDb::ephemeral().expect("db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let token = register_and_login(
        &app,
        "backtest-dynamic-invalid-rules@example.com",
        "pass1234",
    )
    .await;

    let task_id = create_task_with_portfolio(&app, &token, futures_portfolio_config(3)).await;
    let candidate_id = save_ready_candidate(&db, &task_id, futures_portfolio_config(3));
    db.backtest_repo()
        .transition_task(&task_id, "succeeded")
        .expect("succeeded");

    let response = authed_json(
        &app,
        "POST",
        "/backtest/portfolios/publish",
        &token,
        json!({
            "name": "Dynamic invalid rules basket",
            "task_id": task_id,
            "market": "usd_m_futures",
            "direction": "long_short",
            "risk_profile": "dynamic_long_short",
            "total_weight_pct": 100,
            "dynamic_allocation_rules": ["not", "an", "object"],
            "items": [{
                "candidate_id": candidate_id,
                "symbol": "BTCUSDT",
                "weight_pct": 100,
                "leverage": 3,
                "enabled": true,
                "parameter_snapshot": { "direction_mode": "long_and_short" }
            }]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response_json(response).await;
    assert_eq!(body["live_ready"], false);
    assert!(
        body["live_readiness_blockers"]
            .as_array()
            .is_some_and(|blockers| blockers.iter().any(|blocker| blocker
                .as_str()
                .is_some_and(|text| text.contains("must be a JSON object"))))
    );
}

#[tokio::test]
async fn non_dynamic_publish_intent_can_confirm_start_without_dynamic_rules() {
    let db = SharedDb::ephemeral().expect("db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let token =
        register_and_login(&app, "backtest-non-dynamic-confirm@example.com", "pass1234").await;

    let task_id = create_task_with_portfolio(&app, &token, futures_portfolio_config(3)).await;
    let candidate_id = save_ready_candidate(&db, &task_id, futures_portfolio_config(3));

    let intent = authed_json(
        &app,
        "POST",
        &format!("/backtest/candidates/{candidate_id}/publish-intent"),
        &token,
        json!({}),
    )
    .await;
    assert_eq!(intent.status(), StatusCode::OK);
    let portfolio_id = response_json(intent).await["portfolio_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let started = authed_empty(
        &app,
        "POST",
        &format!("/backtest/portfolios/{portfolio_id}/confirm-start"),
        &token,
    )
    .await;
    assert_eq!(started.status(), StatusCode::OK);
    assert_eq!(response_json(started).await["status"], "running");
}

#[tokio::test]
async fn publish_rejects_same_symbol_leverage_conflict() {
    let db = SharedDb::ephemeral().expect("db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let token = register_and_login(&app, "backtest-conflict@example.com", "pass1234").await;

    let first_task = create_task_with_portfolio(&app, &token, futures_portfolio_config(3)).await;
    let first_candidate = save_ready_candidate(&db, &first_task, futures_portfolio_config(3));
    let intent = authed_json(
        &app,
        "POST",
        &format!("/backtest/candidates/{first_candidate}/publish-intent"),
        &token,
        json!({}),
    )
    .await;
    assert_eq!(intent.status(), StatusCode::OK);
    let portfolio_id = response_json(intent).await["portfolio_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let confirmed = authed_empty(
        &app,
        "POST",
        &format!("/backtest/portfolios/{portfolio_id}/confirm-start"),
        &token,
    )
    .await;
    assert_eq!(confirmed.status(), StatusCode::OK);

    let second_task = create_task_with_portfolio(&app, &token, futures_portfolio_config(5)).await;
    let second_candidate = save_ready_candidate(&db, &second_task, futures_portfolio_config(5));
    let conflict = authed_json(
        &app,
        "POST",
        &format!("/backtest/candidates/{second_candidate}/publish-intent"),
        &token,
        json!({}),
    )
    .await;

    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let body = response_json(conflict).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("leverage conflict")
    );
}

#[tokio::test]
async fn admin_quota_upsert_persists_and_enforces_task_creation() {
    let app = app();
    let admin_token = register_admin_and_login(&app, "admin@example.com").await;
    let user_token = register_and_login(&app, "quota-user@example.com", "pass1234").await;

    let upsert = authed_json(
        &app,
        "PUT",
        "/admin/backtest/quotas/quota-user@example.com",
        &admin_token,
        json!({ "max_symbols": 1 }),
    )
    .await;
    assert_eq!(upsert.status(), StatusCode::OK);
    assert_eq!(response_json(upsert).await["policy"]["max_symbols"], 1);

    let fetched = authed_empty(
        &app,
        "GET",
        "/admin/backtest/quotas/quota-user@example.com",
        &admin_token,
    )
    .await;
    assert_eq!(fetched.status(), StatusCode::OK);
    assert_eq!(response_json(fetched).await["policy"]["max_symbols"], 1);

    let rejected = authed_json(
        &app,
        "POST",
        "/backtest/tasks",
        &user_token,
        json!({
            "strategy_type": "martingale_grid",
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "market": "spot",
            "timeframe": "1h",
            "start_date": "2024-01-01",
            "end_date": "2024-01-10"
        }),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
    assert!(
        response_json(rejected).await["error"]
            .as_str()
            .unwrap_or_default()
            .contains("quota")
    );
}

#[tokio::test]
async fn quota_uses_portfolio_strategy_symbols_when_symbols_is_empty() {
    let app = app();
    let admin_token = register_admin_and_login(&app, "admin@example.com").await;
    let user_token = register_and_login(&app, "portfolio-quota@example.com", "pass1234").await;

    let upsert = authed_json(
        &app,
        "PUT",
        "/admin/backtest/quotas/portfolio-quota@example.com",
        &admin_token,
        json!({ "max_symbols": 1 }),
    )
    .await;
    assert_eq!(upsert.status(), StatusCode::OK);

    let rejected = authed_json(
        &app,
        "POST",
        "/backtest/tasks",
        &user_token,
        json!({
            "strategy_type": "martingale_grid",
            "symbols": [],
            "market": "usd_m_futures",
            "timeframe": "1h",
            "start_date": "2024-01-01",
            "end_date": "2024-01-10",
            "portfolio_config": futures_portfolio_config_with_symbols(3, &["BTCUSDT", "ETHUSDT"])
        }),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
    assert!(
        response_json(rejected).await["error"]
            .as_str()
            .unwrap_or_default()
            .contains("quota")
    );
}

#[tokio::test]
async fn martingale_task_rejects_mismatched_symbols_and_portfolio_config() {
    let app = app();
    let token = register_and_login(&app, "symbol-mismatch@example.com", "pass1234").await;

    let response = authed_json(
        &app,
        "POST",
        "/backtest/tasks",
        &token,
        json!({
            "strategy_type": "martingale_grid",
            "symbols": ["BTCUSDT"],
            "market": "usd_m_futures",
            "timeframe": "1h",
            "start_date": "2024-01-01",
            "end_date": "2024-01-10",
            "portfolio_config": futures_portfolio_config_with_symbols(3, &["ETHUSDT"])
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        response_json(response).await["error"]
            .as_str()
            .unwrap_or_default()
            .contains("symbols do not match")
    );
}

#[tokio::test]
async fn confirm_start_rechecks_conflicts_after_paused_portfolio() {
    let db = SharedDb::ephemeral().expect("db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let token = register_and_login(&app, "backtest-resume-conflict@example.com", "pass1234").await;

    let first_task = create_task_with_portfolio(&app, &token, futures_portfolio_config(3)).await;
    let first_candidate = save_ready_candidate(&db, &first_task, futures_portfolio_config(3));
    let first_intent = authed_json(
        &app,
        "POST",
        &format!("/backtest/candidates/{first_candidate}/publish-intent"),
        &token,
        json!({}),
    )
    .await;
    assert_eq!(first_intent.status(), StatusCode::OK);
    let first_portfolio_id = response_json(first_intent).await["portfolio_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let first_started = authed_empty(
        &app,
        "POST",
        &format!("/backtest/portfolios/{first_portfolio_id}/confirm-start"),
        &token,
    )
    .await;
    assert_eq!(first_started.status(), StatusCode::OK);
    let paused = authed_empty(
        &app,
        "POST",
        &format!("/martingale-portfolios/{first_portfolio_id}/pause"),
        &token,
    )
    .await;
    assert_eq!(paused.status(), StatusCode::OK);

    let second_task = create_task_with_portfolio(&app, &token, futures_portfolio_config(5)).await;
    let second_candidate = save_ready_candidate(&db, &second_task, futures_portfolio_config(5));
    let second_intent = authed_json(
        &app,
        "POST",
        &format!("/backtest/candidates/{second_candidate}/publish-intent"),
        &token,
        json!({}),
    )
    .await;
    assert_eq!(second_intent.status(), StatusCode::OK);
    let second_portfolio_id = response_json(second_intent).await["portfolio_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let second_started = authed_empty(
        &app,
        "POST",
        &format!("/backtest/portfolios/{second_portfolio_id}/confirm-start"),
        &token,
    )
    .await;
    assert_eq!(second_started.status(), StatusCode::CONFLICT);
    assert!(
        response_json(second_started).await["error"]
            .as_str()
            .unwrap_or_default()
            .contains("leverage conflict")
    );
}

#[tokio::test]
async fn conflicting_pending_portfolios_allow_only_one_confirm() {
    let db = SharedDb::ephemeral().expect("db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let token = register_and_login(&app, "backtest-confirm-race@example.com", "pass1234").await;

    let first_task = create_task_with_portfolio(&app, &token, futures_portfolio_config(3)).await;
    let first_candidate = save_ready_candidate(&db, &first_task, futures_portfolio_config(3));
    let first_intent = authed_json(
        &app,
        "POST",
        &format!("/backtest/candidates/{first_candidate}/publish-intent"),
        &token,
        json!({}),
    )
    .await;
    assert_eq!(first_intent.status(), StatusCode::OK);
    let first_portfolio_id = response_json(first_intent).await["portfolio_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let second_task = create_task_with_portfolio(&app, &token, futures_portfolio_config(5)).await;
    let second_candidate = save_ready_candidate(&db, &second_task, futures_portfolio_config(5));
    let second_intent = authed_json(
        &app,
        "POST",
        &format!("/backtest/candidates/{second_candidate}/publish-intent"),
        &token,
        json!({}),
    )
    .await;
    assert_eq!(second_intent.status(), StatusCode::OK);
    let second_portfolio_id = response_json(second_intent).await["portfolio_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let first_started = authed_empty(
        &app,
        "POST",
        &format!("/backtest/portfolios/{first_portfolio_id}/confirm-start"),
        &token,
    )
    .await;
    assert_eq!(first_started.status(), StatusCode::OK);

    let second_started = authed_empty(
        &app,
        "POST",
        &format!("/backtest/portfolios/{second_portfolio_id}/confirm-start"),
        &token,
    )
    .await;
    assert_eq!(second_started.status(), StatusCode::CONFLICT);
    assert!(
        response_json(second_started).await["error"]
            .as_str()
            .unwrap_or_default()
            .contains("leverage conflict")
    );
}

async fn create_task_with_portfolio(
    app: &axum::Router,
    token: &str,
    portfolio_config: Value,
) -> String {
    let response = authed_json(
        app,
        "POST",
        "/backtest/tasks",
        token,
        json!({
            "strategy_type": "martingale_grid",
            "symbols": ["BTCUSDT"],
            "market": "usd_m_futures",
            "timeframe": "1h",
            "start_date": "2024-01-01",
            "end_date": "2024-01-10",
            "portfolio_config": portfolio_config
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["task_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn save_ready_candidate(db: &SharedDb, task_id: &str, portfolio_config: Value) -> String {
    db.backtest_repo()
        .save_candidate(NewBacktestCandidateRecord {
            task_id: task_id.to_owned(),
            status: "ready".to_owned(),
            rank: 1,
            config: json!({ "portfolio_config": portfolio_config }),
            summary: json!({
                "score": 1,
                "source": "worker_verified",
                "refinement": "trade_level",
                "risk_summary_ready": true
            }),
        })
        .expect("save candidate")
        .candidate_id
}

fn futures_portfolio_config_with_symbols(leverage: u32, symbols: &[&str]) -> Value {
    let strategies = symbols
        .iter()
        .map(|symbol| {
            json!({
                "strategy_id": format!("{}-long", symbol.to_lowercase()),
                "symbol": symbol,
                "market": "usd_m_futures",
                "direction": "long",
                "direction_mode": "long_and_short",
                "margin_mode": "isolated",
                "leverage": leverage,
                "spacing": { "fixed_percent": { "step_bps": 100 } },
                "sizing": { "multiplier": { "first_order_quote": "10", "multiplier": "2", "max_legs": 3 } },
                "take_profit": { "percent": { "bps": 120 } },
                "stop_loss": null,
                "indicators": [],
                "entry_triggers": ["immediate"],
                "risk_limits": { "max_strategy_budget_quote": "100" }
            })
        })
        .collect::<Vec<_>>();
    json!({
        "direction_mode": "long_and_short",
        "risk_limits": { "max_global_budget_quote": "1000" },
        "strategies": strategies
    })
}

fn futures_portfolio_config(leverage: u32) -> Value {
    json!({
        "direction_mode": "long_and_short",
        "risk_limits": { "max_global_budget_quote": "1000" },
        "strategies": [{
            "strategy_id": "btc-long",
            "symbol": "BTCUSDT",
            "market": "usd_m_futures",
            "direction": "long",
            "direction_mode": "long_and_short",
            "margin_mode": "isolated",
            "leverage": leverage,
            "spacing": { "fixed_percent": { "step_bps": 100 } },
            "sizing": { "multiplier": { "first_order_quote": "10", "multiplier": "2", "max_legs": 3 } },
            "take_profit": { "percent": { "bps": 120 } },
            "stop_loss": null,
            "indicators": [],
            "entry_triggers": ["immediate"],
            "risk_limits": { "max_strategy_budget_quote": "100" }
        }]
    })
}

async fn authed_json(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: &str,
    body: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn authed_empty(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: &str,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn register_admin_and_login(app: &axum::Router, email: &str) -> String {
    support::register_and_verify(app, email, "pass1234").await;
    let enabled = authed_or_public_json(
        app,
        "POST",
        "/auth/admin-bootstrap",
        None,
        json!({ "email": email, "password": "pass1234" }),
    )
    .await;
    assert_eq!(enabled.status(), StatusCode::OK);
    let totp_code = response_json(enabled).await["code"]
        .as_str()
        .expect("totp code")
        .to_owned();
    let login = authed_or_public_json(
        app,
        "POST",
        "/auth/login",
        None,
        json!({ "email": email, "password": "pass1234", "totp_code": totp_code }),
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    response_json(login).await["session_token"]
        .as_str()
        .expect("session token")
        .to_owned()
}

async fn authed_or_public_json(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Value,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
