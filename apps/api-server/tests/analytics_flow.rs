use api_server::{app_with_state, AppState};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
use shared_chain::assignment::AddressAssignment;
use shared_db::{
    AccountProfitSnapshotRecord, BillingOrderRecord, ExchangeTradeHistoryRecord,
    ExchangeWalletSnapshotRecord, SharedDb, StoredStrategy, StrategyProfitSnapshotRecord,
};
use shared_domain::strategy::{
    GridGeneration, GridLevel, PostTriggerAction, ReferencePriceSource, RuntimeControls, Strategy,
    StrategyAmountMode, StrategyMarket, StrategyMode, StrategyRevision, StrategyRuntime,
    StrategyRuntimeFill, StrategyRuntimeOrder, StrategyRuntimePhase, StrategyRuntimePosition,
    StrategyStatus, StrategyType,
};
use tower::ServiceExt;

mod support;

use support::register_and_login;

#[tokio::test]
async fn analytics_falls_back_to_trade_history_fees_when_account_snapshots_report_zero() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    db.insert_strategy(&StoredStrategy {
        sequence_id: 1,
        strategy: stored_strategy(
            "fee-fallback",
            "trader@example.com",
            "Fee Fallback",
            "BTCUSDT",
            StrategyStatus::Stopped,
            vec![],
            vec![],
            vec![],
        ),
    })
    .expect("strategy");
    db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
        user_email: "trader@example.com".to_string(),
        exchange: "binance".to_string(),
        realized_pnl: "0".to_string(),
        unrealized_pnl: "0".to_string(),
        fees: "0".to_string(),
        funding: None,
        captured_at: Utc.with_ymd_and_hms(2026, 3, 5, 0, 0, 0).unwrap(),
    })
    .expect("account snapshot");
    db.insert_exchange_trade_history(&exchange_trade(
        "fee-trade-1",
        "trader@example.com",
        "BTCUSDT",
        "Buy",
        "1",
        "100",
        Some("0.75"),
        Some("USDT"),
    ))
    .expect("trade history");

    let app = app_with_state(AppState::from_shared_db(db).expect("app state"));
    let session_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let analytics = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .expect("analytics request"),
        )
        .await
        .expect("analytics response");

    assert_eq!(analytics.status(), StatusCode::OK);
    let body = response_json(analytics).await;
    assert_eq!(body["user"]["fees_paid"], "0.75");
    assert_eq!(body["costs"]["fees_paid"], "0.75");
}

#[tokio::test]
async fn compute_strategy_and_account_snapshots_from_persisted_trading_and_exchange_data() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    seed_analytics_data(&db);
    let app = app_with_state(AppState::from_shared_db(db).expect("app state"));
    let session_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let analytics = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .expect("analytics request"),
        )
        .await
        .expect("analytics response");

    assert_eq!(analytics.status(), StatusCode::OK);
    let analytics_body = response_json(analytics).await;
    assert_eq!(analytics_body["fills"].as_array().expect("fills").len(), 3);
    assert_eq!(analytics_body["fills"][0]["realized_pnl"], "10");
    assert_eq!(analytics_body["fills"][1]["fee"], "0.5");

    let strategies = analytics_body["strategies"].as_array().expect("strategies");
    assert_eq!(strategies.len(), 4);
    assert_eq!(strategies[0]["strategy_id"], "strategy-alpha");
    assert_eq!(strategies[0]["current_state"], "Stopped");
    assert_eq!(strategies[0]["fill_count"], 2);
    assert_eq!(strategies[0]["order_count"], 2);
    assert_eq!(strategies[0]["cost_basis"], "0");
    assert_eq!(strategies[0]["position_quantity"], "0");
    assert_eq!(strategies[0]["average_entry_price"], "0");

    assert_eq!(strategies[1]["strategy_id"], "strategy-beta");
    assert_eq!(strategies[1]["unrealized_pnl"], "2.5");
    assert_eq!(strategies[1]["current_state"], "Stopped");
    assert_eq!(strategies[1]["fill_count"], 1);
    assert_eq!(strategies[1]["order_count"], 1);

    assert_eq!(strategies[2]["strategy_id"], "strategy-gamma");
    assert_eq!(strategies[2]["current_state"], "Running");
    assert_eq!(strategies[2]["fill_count"], 0);
    assert_eq!(strategies[2]["order_count"], 1);
    assert_eq!(strategies[2]["cost_basis"], "33");
    assert_eq!(strategies[2]["position_quantity"], "1.5");
    assert_eq!(strategies[2]["average_entry_price"], "22");
    assert_eq!(strategies[2]["unrealized_pnl"], "4.2");
    assert_eq!(strategies[2]["funding_total"], "-0.4");
    assert_eq!(strategies[2]["net_pnl"], "3.8");

    assert_eq!(strategies[3]["strategy_id"], "strategy-delta");
    assert_eq!(strategies[3]["fill_count"], 0);
    assert_eq!(strategies[3]["position_quantity"], "0");
    assert_eq!(strategies[3]["funding_total"], "0.15");

    assert_eq!(analytics_body["user"]["user_id"], "trader@example.com");
    assert_eq!(analytics_body["user"]["realized_pnl"], "15");
    assert_eq!(analytics_body["user"]["unrealized_pnl"], "6.7");
    assert_eq!(analytics_body["user"]["fees_paid"], "2.25");
    assert_eq!(analytics_body["user"]["funding_total"], "-0.3");
    assert_eq!(analytics_body["user"]["net_pnl"], "19.15");
    assert_eq!(analytics_body["user"]["wallet_asset_count"], 3);
    assert_eq!(analytics_body["user"]["exchange_trade_count"], 4);

    let exchange_trades = analytics_body["exchange_trades"]
        .as_array()
        .expect("exchange trades");
    assert_eq!(exchange_trades.len(), 4);
    assert_eq!(exchange_trades[0]["trade_id"], "trade-1");
    assert_eq!(exchange_trades[0]["symbol"], "BTCUSDT");
    assert_eq!(exchange_trades[3]["trade_id"], "trade-4");
    assert_eq!(exchange_trades[3]["fee_amount"], "0.25");

    assert_eq!(analytics_body["costs"]["fees_paid"], "2.25");
    assert_eq!(analytics_body["costs"]["funding_total"], "-0.3");

    let wallet_snapshots = analytics_body["wallets"].as_array().expect("wallets");
    assert_eq!(wallet_snapshots.len(), 1);
    assert_eq!(wallet_snapshots[0]["wallet_type"], "spot");
    assert_eq!(wallet_snapshots[0]["balances"]["BTC"], "0.01");

    let account_snapshots = analytics_body["account_snapshots"]
        .as_array()
        .expect("account snapshots");
    assert_eq!(account_snapshots.len(), 4);
    assert_eq!(account_snapshots[0]["exchange"], "binance");
    assert_eq!(account_snapshots[0]["funding_total"], "-0.5");
    assert_eq!(account_snapshots[1]["exchange"], "binance");
    assert_eq!(account_snapshots[1]["funding_total"], "-1.25");
    assert_eq!(account_snapshots[2]["exchange"], "binance-futures");
    assert_eq!(account_snapshots[2]["funding_total"], "-0.1");
    assert_eq!(account_snapshots[3]["exchange"], "binance-futures");
    assert_eq!(account_snapshots[3]["funding_total"], "-0.25");

    let strategy_snapshots = analytics_body["strategy_snapshots"]
        .as_array()
        .expect("strategy snapshots");
    assert_eq!(strategy_snapshots.len(), 4);
    assert_eq!(strategy_snapshots[2]["strategy_id"], "strategy-gamma");
    assert_eq!(strategy_snapshots[2]["unrealized_pnl"], "4.2");
    assert_eq!(strategy_snapshots[2]["funding_total"], "-0.4");
    assert_eq!(strategy_snapshots[3]["strategy_id"], "strategy-delta");
    assert_eq!(strategy_snapshots[3]["funding_total"], "0.15");

    assert!(
        analytics_body["fills"]
            .as_array()
            .expect("fills array")
            .iter()
            .all(|fill| fill["strategy_id"] != "foreign-strategy"),
        "analytics should filter foreign user fills"
    );

    let orders_csv = export_csv(&app, &session_token, "/exports/orders.csv").await;
    let order_lines: Vec<&str> = orders_csv.trim().lines().collect();
    assert_eq!(
        order_lines[0],
        "order_id,strategy_id,symbol,side,order_type,price,quantity,status"
    );
    assert_eq!(
        order_lines[4],
        "gamma-order-1,strategy-gamma,SOLUSDT,Buy,Limit,22,1.5,Working"
    );

    let fills_csv = export_csv(&app, &session_token, "/exports/fills.csv").await;
    let fill_lines: Vec<&str> = fills_csv.trim().lines().collect();
    assert_eq!(fill_lines[0], "fill_id,strategy_id,order_id,symbol,price,quantity,realized_pnl,fee_amount,fee_asset,fill_type");
    assert_eq!(fill_lines.len(), 4);

    let strategy_stats_csv = export_csv(&app, &session_token, "/exports/strategy-stats.csv").await;
    let strategy_lines: Vec<&str> = strategy_stats_csv.trim().lines().collect();
    assert_eq!(
        strategy_lines[0],
        "strategy_id,user_id,symbol,current_state,fill_count,order_count,cost_basis,position_quantity,average_entry_price,realized_pnl,unrealized_pnl,fees_paid,funding_total,net_pnl"
    );
    assert_eq!(
        strategy_lines[3],
        "strategy-gamma,trader@example.com,SOLUSDT,Running,0,1,33,1.5,22,0,4.2,0,-0.4,3.8"
    );

    let payments_csv = export_csv(&app, &session_token, "/exports/payments.csv").await;
    let payment_lines: Vec<&str> = payments_csv.trim().lines().collect();
    assert_eq!(
        payment_lines[0],
        "order_id,email,chain,asset,plan_code,amount,status,address,requested_at,paid_at,tx_hash"
    );
    assert_eq!(payment_lines[2], "502,trader@example.com,BSC,USDC,quarterly,54.00000000,pending,bsc-addr-3,2026-03-02T00:00:00+00:00,,");
}

#[tokio::test]
async fn analytics_strategy_summaries_endpoint_returns_lightweight_items() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    seed_analytics_data(&db);
    let app = app_with_state(AppState::from_shared_db(db).expect("app state"));
    let session_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics/strategies")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .expect("analytics request"),
        )
        .await
        .expect("analytics response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    let items = body.as_array().expect("strategy summaries array");
    assert_eq!(items.len(), 4);
    assert_eq!(items[0]["strategy_id"], "strategy-alpha");
    assert_eq!(items[2]["strategy_id"], "strategy-gamma");
    assert_eq!(items[2]["net_pnl"], "3.8");
    assert!(items[0].get("exchange_trades").is_none());
}

#[tokio::test]
async fn analytics_projects_short_fill_entry_price_from_short_direction() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let mut strategy = stored_strategy(
        "strategy-short",
        "trader@example.com",
        "Short BTC",
        "BTCUSDT",
        StrategyStatus::Stopped,
        vec![],
        vec![runtime_order(
            "short-close-1",
            "Buy",
            "Limit",
            Some("90"),
            "2",
            "Filled",
        )],
        vec![runtime_fill(
            "short-fill-1",
            Some("short-close-1"),
            "GridTakeProfit",
            "90",
            "2",
            Some("20"),
            Some("0.4"),
            Some("USDT"),
        )],
    );
    strategy.market = StrategyMarket::FuturesUsdM;
    strategy.mode = StrategyMode::FuturesShort;

    db.insert_strategy(&StoredStrategy {
        sequence_id: 1,
        strategy,
    })
    .expect("short strategy");

    let app = app_with_state(AppState::from_shared_db(db).expect("app state"));
    let session_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let analytics = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .expect("analytics request"),
        )
        .await
        .expect("analytics response");

    assert_eq!(analytics.status(), StatusCode::OK);
    let analytics_body = response_json(analytics).await;
    let fill = find_fill(&analytics_body, "strategy-short");

    assert_eq!(fill["entry_price"], "100");
    assert_eq!(fill["exit_price"], "90");
    assert_eq!(fill["realized_pnl"], "20");
}

#[tokio::test]
async fn analytics_preserves_hedged_position_breakdown_in_strategy_summary() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let mut strategy = stored_strategy(
        "strategy-hedged",
        "trader@example.com",
        "Hedged SOL",
        "SOLUSDT",
        StrategyStatus::Running,
        vec![
            StrategyRuntimePosition {
                market: StrategyMarket::FuturesUsdM,
                mode: StrategyMode::FuturesLong,
                quantity: decimal("1.5"),
                average_entry_price: decimal("100"),
            },
            StrategyRuntimePosition {
                market: StrategyMarket::FuturesUsdM,
                mode: StrategyMode::FuturesShort,
                quantity: decimal("0.75"),
                average_entry_price: decimal("120"),
            },
        ],
        vec![],
        vec![],
    );
    strategy.market = StrategyMarket::FuturesUsdM;
    strategy.mode = StrategyMode::FuturesNeutral;

    db.insert_strategy(&StoredStrategy {
        sequence_id: 1,
        strategy,
    })
    .expect("hedged strategy");

    let app = app_with_state(AppState::from_shared_db(db).expect("app state"));
    let session_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let analytics = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .expect("analytics request"),
        )
        .await
        .expect("analytics response");

    assert_eq!(analytics.status(), StatusCode::OK);
    let analytics_body = response_json(analytics).await;
    let strategy = find_strategy(&analytics_body, "strategy-hedged");

    assert_eq!(strategy["position_quantity"], "0");
    assert_eq!(strategy["average_entry_price"], "0");
    assert_eq!(strategy["long_position_quantity"], "1.5");
    assert_eq!(strategy["long_average_entry_price"], "100");
    assert_eq!(strategy["short_position_quantity"], "0.75");
    assert_eq!(strategy["short_average_entry_price"], "120");
}

#[tokio::test]
async fn first_level_market_fill_is_counted_in_level_statistics() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let mut strategy = stored_strategy(
        "strategy-ordinary-levels",
        "trader@example.com",
        "Ordinary BTC",
        "BTCUSDT",
        StrategyStatus::Running,
        vec![],
        vec![
            runtime_order_with_level(
                "ordinary-level-1-entry",
                1,
                "Buy",
                "Limit",
                Some("68000"),
                "0.02",
                "Filled",
            ),
            runtime_order_with_level(
                "ordinary-level-1-tp",
                1,
                "Sell",
                "Limit",
                Some("69000"),
                "0.02",
                "Filled",
            ),
        ],
        vec![
            runtime_fill_with_level(
                "ordinary-level-0-entry",
                Some(0),
                None,
                "Entry",
                "70000",
                "0.001",
                None,
                Some("35.00"),
                Some("USDT"),
            ),
            runtime_fill_with_level(
                "ordinary-level-1-entry",
                Some(1),
                Some("ordinary-level-1-entry"),
                "Entry",
                "68000",
                "0.02",
                None,
                Some("1.00"),
                Some("USDT"),
            ),
            runtime_fill_with_level(
                "ordinary-level-1-exit",
                Some(1),
                Some("ordinary-level-1-tp"),
                "Exit",
                "69000",
                "0.02",
                Some("20.00"),
                Some("2.00"),
                Some("USDT"),
            ),
        ],
    );
    let levels = vec![
        grid_level(0, "70000", "0.001", 200, None),
        grid_level(1, "68000", "0.02", 147, None),
    ];
    strategy.draft_revision.levels = levels.clone();
    strategy
        .active_revision
        .as_mut()
        .expect("active revision")
        .levels = levels;

    db.insert_strategy(&StoredStrategy {
        sequence_id: 1,
        strategy,
    })
    .expect("ordinary strategy");

    let app = app_with_state(AppState::from_shared_db(db).expect("app state"));
    let session_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let analytics = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .expect("analytics request"),
        )
        .await
        .expect("analytics response");

    assert_eq!(analytics.status(), StatusCode::OK);
    let analytics_body = response_json(analytics).await;
    let fills: Vec<&Value> = analytics_body["fills"]
        .as_array()
        .expect("fills array")
        .iter()
        .filter(|fill| fill["strategy_id"] == "strategy-ordinary-levels")
        .collect();

    assert_eq!(
        fills.len(),
        2,
        "ordinary grid fills should project one row per filled level"
    );
    assert_eq!(fills[0]["level_index"], 0);
    assert_eq!(fills[0]["entry_price"], "70000");
    assert_eq!(fills[0]["exit_price"], "70000");
    assert_eq!(fills[0]["quantity"], "0.001");
    assert_eq!(fills[0]["fee"], "35.00");
    assert_eq!(fills[0]["realized_pnl"], "0");

    assert_eq!(fills[1]["level_index"], 1);
    assert_eq!(fills[1]["entry_price"], "68000");
    assert_eq!(fills[1]["exit_price"], "69000");
    assert_eq!(fills[1]["quantity"], "0.02");
    assert_eq!(fills[1]["fee"], "3.00");
    assert_eq!(fills[1]["realized_pnl"], "20.00");

    let strategy = find_strategy(&analytics_body, "strategy-ordinary-levels");
    assert_eq!(strategy["fill_count"], 3);
    assert_eq!(strategy["realized_pnl"], "20");
    assert_eq!(strategy["fees_paid"], "38");

    let strategy_stats_csv = export_csv(&app, &session_token, "/exports/strategy-stats.csv").await;
    let strategy_line = strategy_stats_csv
        .trim()
        .lines()
        .find(|line| line.starts_with("strategy-ordinary-levels,"))
        .expect("ordinary strategy stats line");
    assert_eq!(
        strategy_line,
        "strategy-ordinary-levels,trader@example.com,BTCUSDT,Running,3,2,0,0,0,20,0,38,0,-18"
    );
}

#[tokio::test]
async fn legacy_ordinary_fills_recover_level_identity_from_order_links_and_first_fill_fallback() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let mut strategy = stored_strategy(
        "strategy-ordinary-legacy",
        "trader@example.com",
        "Legacy Ordinary BTC",
        "BTCUSDT",
        StrategyStatus::Running,
        vec![],
        vec![
            runtime_order_with_level(
                "legacy-level-1-entry",
                1,
                "Buy",
                "Limit",
                Some("68000"),
                "0.02",
                "Filled",
            ),
            runtime_order_with_level(
                "legacy-level-1-tp",
                1,
                "Sell",
                "Limit",
                Some("69000"),
                "0.02",
                "Filled",
            ),
        ],
        vec![
            runtime_fill(
                "legacy-level-0-entry",
                None,
                "Entry",
                "70000",
                "0.001",
                None,
                Some("35.00"),
                Some("USDT"),
            ),
            runtime_fill(
                "legacy-level-1-entry",
                Some("legacy-level-1-entry"),
                "Entry",
                "68000",
                "0.02",
                None,
                Some("1.00"),
                Some("USDT"),
            ),
            runtime_fill(
                "legacy-level-1-exit",
                Some("legacy-level-1-tp"),
                "Exit",
                "69000",
                "0.02",
                Some("20.00"),
                Some("2.00"),
                Some("USDT"),
            ),
        ],
    );
    let levels = vec![
        grid_level(0, "70000", "0.001", 200, None),
        grid_level(1, "68000", "0.02", 147, None),
    ];
    strategy.draft_revision.levels = levels.clone();
    strategy
        .active_revision
        .as_mut()
        .expect("active revision")
        .levels = levels;

    db.insert_strategy(&StoredStrategy {
        sequence_id: 1,
        strategy,
    })
    .expect("legacy ordinary strategy");

    let app = app_with_state(AppState::from_shared_db(db).expect("app state"));
    let session_token = register_and_login(&app, "trader@example.com", "pass1234").await;

    let analytics = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .expect("analytics request"),
        )
        .await
        .expect("analytics response");

    assert_eq!(analytics.status(), StatusCode::OK);
    let analytics_body = response_json(analytics).await;
    let fills: Vec<&Value> = analytics_body["fills"]
        .as_array()
        .expect("fills array")
        .iter()
        .filter(|fill| fill["strategy_id"] == "strategy-ordinary-legacy")
        .collect();

    assert_eq!(fills.len(), 2);
    assert_eq!(fills[0]["level_index"], 0);
    assert_eq!(fills[0]["entry_price"], "70000");
    assert_eq!(fills[1]["level_index"], 1);
    assert_eq!(fills[1]["entry_price"], "68000");
    assert_eq!(fills[1]["exit_price"], "69000");
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = response.into_body();
    let bytes = to_bytes(body, usize::MAX).await.expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}

async fn response_text(response: axum::response::Response) -> String {
    let body = response.into_body();
    let bytes = to_bytes(body, usize::MAX).await.expect("body bytes");
    String::from_utf8(bytes.to_vec()).expect("utf8 text")
}

async fn export_csv(app: &axum::Router, session_token: &str, uri: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .expect("export request"),
        )
        .await
        .expect("export response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/csv; charset=utf-8")
    );

    response_text(response).await
}

fn seed_analytics_data(db: &SharedDb) {
    db.insert_strategy(&StoredStrategy {
        sequence_id: 1,
        strategy: stored_strategy(
            "strategy-alpha",
            "trader@example.com",
            "Alpha BTC",
            "BTCUSDT",
            StrategyStatus::Stopped,
            vec![],
            vec![
                runtime_order("alpha-order-1", "Sell", "Limit", Some("100"), "1", "Filled"),
                runtime_order("alpha-order-2", "Sell", "Limit", Some("110"), "2", "Filled"),
            ],
            vec![
                runtime_fill(
                    "alpha-fill-1",
                    Some("alpha-order-1"),
                    "GridTakeProfit",
                    "110",
                    "1",
                    Some("10"),
                    Some("1"),
                    Some("USDT"),
                ),
                runtime_fill(
                    "alpha-fill-2",
                    Some("alpha-order-2"),
                    "GridTakeProfit",
                    "105",
                    "2",
                    Some("-10"),
                    Some("0.5"),
                    Some("USDT"),
                ),
            ],
        ),
    })
    .expect("insert alpha strategy");

    db.insert_strategy(&StoredStrategy {
        sequence_id: 2,
        strategy: stored_strategy(
            "strategy-beta",
            "trader@example.com",
            "Beta ETH",
            "ETHUSDT",
            StrategyStatus::Stopped,
            vec![],
            vec![runtime_order(
                "beta-order-1",
                "Sell",
                "Limit",
                Some("50"),
                "3",
                "Filled",
            )],
            vec![runtime_fill(
                "beta-fill-1",
                Some("beta-order-1"),
                "GridTakeProfit",
                "55",
                "3",
                Some("15"),
                Some("0.75"),
                Some("USDT"),
            )],
        ),
    })
    .expect("insert beta strategy");

    db.insert_strategy(&StoredStrategy {
        sequence_id: 3,
        strategy: stored_strategy(
            "strategy-gamma",
            "trader@example.com",
            "Gamma SOL",
            "SOLUSDT",
            StrategyStatus::Running,
            vec![runtime_position("1.5", "22")],
            vec![runtime_order(
                "gamma-order-1",
                "Buy",
                "Limit",
                Some("22"),
                "1.5",
                "Working",
            )],
            vec![],
        ),
    })
    .expect("insert gamma strategy");

    db.insert_strategy(&StoredStrategy {
        sequence_id: 4,
        strategy: stored_strategy(
            "strategy-delta",
            "trader@example.com",
            "Delta Flat",
            "BNBUSDT",
            StrategyStatus::Paused,
            vec![],
            vec![],
            vec![],
        ),
    })
    .expect("insert delta strategy");

    db.insert_strategy(&StoredStrategy {
        sequence_id: 5,
        strategy: stored_strategy(
            "foreign-strategy",
            "other@example.com",
            "Foreign SOL",
            "SOLUSDT",
            StrategyStatus::Stopped,
            vec![],
            vec![runtime_order(
                "foreign-order-1",
                "Sell",
                "Limit",
                Some("20"),
                "4",
                "Filled",
            )],
            vec![runtime_fill(
                "foreign-fill-1",
                Some("foreign-order-1"),
                "GridTakeProfit",
                "25",
                "4",
                Some("20"),
                Some("0.25"),
                Some("USDT"),
            )],
        ),
    })
    .expect("insert foreign strategy");

    db.insert_strategy_profit_snapshot(&StrategyProfitSnapshotRecord {
        strategy_id: "strategy-alpha".to_string(),
        realized_pnl: "0".to_string(),
        unrealized_pnl: "0".to_string(),
        fees: "1.5".to_string(),
        funding: Some("0".to_string()),
        captured_at: Utc.with_ymd_and_hms(2026, 3, 4, 0, 0, 0).unwrap(),
    })
    .expect("alpha snapshot");

    db.insert_strategy_profit_snapshot(&StrategyProfitSnapshotRecord {
        strategy_id: "strategy-beta".to_string(),
        realized_pnl: "15".to_string(),
        unrealized_pnl: "2.5".to_string(),
        fees: "0.75".to_string(),
        funding: Some("-0.05".to_string()),
        captured_at: Utc.with_ymd_and_hms(2026, 3, 4, 0, 5, 0).unwrap(),
    })
    .expect("beta snapshot");

    db.insert_strategy_profit_snapshot(&StrategyProfitSnapshotRecord {
        strategy_id: "strategy-gamma".to_string(),
        realized_pnl: "0".to_string(),
        unrealized_pnl: "4.2".to_string(),
        fees: "0".to_string(),
        funding: Some("-0.4".to_string()),
        captured_at: Utc.with_ymd_and_hms(2026, 3, 4, 0, 10, 0).unwrap(),
    })
    .expect("gamma snapshot");

    db.insert_strategy_profit_snapshot(&StrategyProfitSnapshotRecord {
        strategy_id: "strategy-delta".to_string(),
        realized_pnl: "0".to_string(),
        unrealized_pnl: "0".to_string(),
        fees: "0".to_string(),
        funding: Some("0.15".to_string()),
        captured_at: Utc.with_ymd_and_hms(2026, 3, 4, 0, 15, 0).unwrap(),
    })
    .expect("delta snapshot");

    db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
        user_email: "trader@example.com".to_string(),
        exchange: "binance".to_string(),
        realized_pnl: "10".to_string(),
        unrealized_pnl: "4.5".to_string(),
        fees: "1.5".to_string(),
        funding: Some("-0.5".to_string()),
        captured_at: Utc.with_ymd_and_hms(2026, 3, 4, 0, 30, 0).unwrap(),
    })
    .expect("older account snapshot");

    db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
        user_email: "trader@example.com".to_string(),
        exchange: "binance".to_string(),
        realized_pnl: "15".to_string(),
        unrealized_pnl: "6.7".to_string(),
        fees: "2.25".to_string(),
        funding: Some("-1.25".to_string()),
        captured_at: Utc.with_ymd_and_hms(2026, 3, 4, 1, 0, 0).unwrap(),
    })
    .expect("account snapshot");

    db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
        user_email: "trader@example.com".to_string(),
        exchange: "binance-futures".to_string(),
        realized_pnl: "2".to_string(),
        unrealized_pnl: "0.8".to_string(),
        fees: "0.2".to_string(),
        funding: Some("-0.1".to_string()),
        captured_at: Utc.with_ymd_and_hms(2026, 3, 4, 0, 45, 0).unwrap(),
    })
    .expect("older futures account snapshot");

    db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
        user_email: "trader@example.com".to_string(),
        exchange: "binance-futures".to_string(),
        realized_pnl: "3".to_string(),
        unrealized_pnl: "1".to_string(),
        fees: "0.5".to_string(),
        funding: Some("-0.25".to_string()),
        captured_at: Utc.with_ymd_and_hms(2026, 3, 4, 1, 5, 0).unwrap(),
    })
    .expect("futures account snapshot");

    db.insert_exchange_wallet_snapshot(&ExchangeWalletSnapshotRecord {
        user_email: "trader@example.com".to_string(),
        exchange: "binance".to_string(),
        wallet_type: "spot".to_string(),
        balances: json!({
            "USDT": "120.5",
            "BTC": "0.01",
            "ETH": "0.50",
        }),
        captured_at: Utc.with_ymd_and_hms(2026, 3, 4, 1, 5, 0).unwrap(),
    })
    .expect("wallet snapshot");

    for trade in [
        exchange_trade(
            "trade-1",
            "trader@example.com",
            "BTCUSDT",
            "Buy",
            "1",
            "100",
            Some("0.5"),
            Some("USDT"),
        ),
        exchange_trade(
            "trade-2",
            "trader@example.com",
            "BTCUSDT",
            "Sell",
            "1",
            "110",
            Some("0.5"),
            Some("USDT"),
        ),
        exchange_trade(
            "trade-3",
            "trader@example.com",
            "ETHUSDT",
            "Buy",
            "3",
            "50",
            Some("0.25"),
            Some("USDT"),
        ),
        exchange_trade(
            "trade-4",
            "trader@example.com",
            "ETHUSDT",
            "Sell",
            "3",
            "55",
            Some("0.25"),
            Some("USDT"),
        ),
        exchange_trade(
            "trade-foreign",
            "other@example.com",
            "SOLUSDT",
            "Sell",
            "4",
            "25",
            Some("0.25"),
            Some("USDT"),
        ),
    ] {
        db.insert_exchange_trade_history(&trade)
            .expect("trade history");
    }

    db.insert_billing_order(&BillingOrderRecord {
        order_id: 501,
        email: "trader@example.com".to_string(),
        chain: "ETH".to_string(),
        asset: "USDT".to_string(),
        plan_code: "monthly".to_string(),
        amount: "20.00000000".to_string(),
        requested_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
        assignment: Some(AddressAssignment {
            chain: "ETH".to_string(),
            address: "eth-addr-7".to_string(),
            expires_at: Utc.with_ymd_and_hms(2026, 3, 1, 1, 0, 0).unwrap(),
        }),
        paid_at: Some(Utc.with_ymd_and_hms(2026, 3, 1, 0, 5, 0).unwrap()),
        tx_hash: Some("tx-501".to_string()),
        status: "paid".to_string(),
        enqueued_at: None,
    })
    .expect("insert paid order");

    db.insert_billing_order(&BillingOrderRecord {
        order_id: 502,
        email: "trader@example.com".to_string(),
        chain: "BSC".to_string(),
        asset: "USDC".to_string(),
        plan_code: "quarterly".to_string(),
        amount: "54.00000000".to_string(),
        requested_at: Utc.with_ymd_and_hms(2026, 3, 2, 0, 0, 0).unwrap(),
        assignment: Some(AddressAssignment {
            chain: "BSC".to_string(),
            address: "bsc-addr-3".to_string(),
            expires_at: Utc.with_ymd_and_hms(2026, 3, 2, 1, 0, 0).unwrap(),
        }),
        paid_at: None,
        tx_hash: None,
        status: "pending".to_string(),
        enqueued_at: None,
    })
    .expect("insert pending order");
}

fn stored_strategy(
    strategy_id: &str,
    owner_email: &str,
    name: &str,
    symbol: &str,
    status: StrategyStatus,
    positions: Vec<StrategyRuntimePosition>,
    orders: Vec<StrategyRuntimeOrder>,
    fills: Vec<StrategyRuntimeFill>,
) -> Strategy {
    let revision = StrategyRevision {
        revision_id: format!("{strategy_id}-rev-1"),
        version: 1,
        strategy_type: StrategyType::OrdinaryGrid,
        generation: GridGeneration::Custom,
        amount_mode: StrategyAmountMode::Quote,
        futures_margin_mode: None,
        leverage: None,
        reference_price: Some(decimal("70000")),
        reference_price_source: ReferencePriceSource::Manual,
        levels: Vec::new(),
        overall_take_profit_bps: None,
        overall_stop_loss_bps: None,
        post_trigger_action: PostTriggerAction::Stop,
    };

    Strategy {
        id: strategy_id.to_string(),
        owner_email: owner_email.to_string(),
        name: name.to_string(),
        symbol: symbol.to_string(),
        budget: "300".to_string(),
        grid_spacing_bps: 100,
        status,
        source_template_id: None,
        membership_ready: true,
        exchange_ready: true,
        permissions_ready: true,
        withdrawals_disabled: true,
        hedge_mode_ready: true,
        symbol_ready: true,
        filters_ready: true,
        margin_ready: true,
        conflict_ready: true,
        balance_ready: true,
        strategy_type: StrategyType::OrdinaryGrid,
        market: StrategyMarket::Spot,
        mode: StrategyMode::SpotClassic,
        runtime_phase: StrategyRuntimePhase::Draft,
        runtime_controls: RuntimeControls::default(),
        draft_revision: revision.clone(),
        active_revision: Some(revision),
        runtime: StrategyRuntime {
            positions,
            orders,
            fills,
            events: Vec::new(),
            last_preflight: None,
        },
        tags: Vec::new(),
        notes: String::new(),
        archived_at: None,
    }
}

fn runtime_position(quantity: &str, average_entry_price: &str) -> StrategyRuntimePosition {
    StrategyRuntimePosition {
        market: StrategyMarket::Spot,
        mode: StrategyMode::SpotClassic,
        quantity: decimal(quantity),
        average_entry_price: decimal(average_entry_price),
    }
}

fn runtime_order(
    order_id: &str,
    side: &str,
    order_type: &str,
    price: Option<&str>,
    quantity: &str,
    status: &str,
) -> StrategyRuntimeOrder {
    StrategyRuntimeOrder {
        order_id: order_id.to_string(),
        exchange_order_id: None,
        level_index: None,
        side: side.to_string(),
        order_type: order_type.to_string(),
        price: price.map(decimal),
        quantity: decimal(quantity),
        status: status.to_string(),
    }
}

fn runtime_fill(
    fill_id: &str,
    order_id: Option<&str>,
    fill_type: &str,
    price: &str,
    quantity: &str,
    realized_pnl: Option<&str>,
    fee_amount: Option<&str>,
    fee_asset: Option<&str>,
) -> StrategyRuntimeFill {
    StrategyRuntimeFill {
        fill_id: fill_id.to_string(),
        order_id: order_id.map(ToOwned::to_owned),
        level_index: None,
        fill_type: fill_type.to_string(),
        price: decimal(price),
        quantity: decimal(quantity),
        realized_pnl: realized_pnl.map(decimal),
        fee_amount: fee_amount.map(decimal),
        fee_asset: fee_asset.map(ToOwned::to_owned),
    }
}

fn runtime_order_with_level(
    order_id: &str,
    level_index: u32,
    side: &str,
    order_type: &str,
    price: Option<&str>,
    quantity: &str,
    status: &str,
) -> StrategyRuntimeOrder {
    let mut order = runtime_order(order_id, side, order_type, price, quantity, status);
    order.level_index = Some(level_index);
    order
}

fn runtime_fill_with_level(
    fill_id: &str,
    level_index: Option<u32>,
    order_id: Option<&str>,
    fill_type: &str,
    price: &str,
    quantity: &str,
    realized_pnl: Option<&str>,
    fee_amount: Option<&str>,
    fee_asset: Option<&str>,
) -> StrategyRuntimeFill {
    let mut fill = runtime_fill(
        fill_id,
        order_id,
        fill_type,
        price,
        quantity,
        realized_pnl,
        fee_amount,
        fee_asset,
    );
    fill.level_index = level_index;
    fill
}

fn grid_level(
    level_index: u32,
    entry_price: &str,
    quantity: &str,
    take_profit_bps: u32,
    trailing_bps: Option<u32>,
) -> GridLevel {
    GridLevel {
        level_index,
        entry_price: decimal(entry_price),
        quantity: decimal(quantity),
        take_profit_bps,
        trailing_bps,
    }
}

fn exchange_trade(
    trade_id: &str,
    user_email: &str,
    symbol: &str,
    side: &str,
    quantity: &str,
    price: &str,
    fee_amount: Option<&str>,
    fee_asset: Option<&str>,
) -> ExchangeTradeHistoryRecord {
    ExchangeTradeHistoryRecord {
        trade_id: trade_id.to_string(),
        user_email: user_email.to_string(),
        exchange: "binance".to_string(),
        symbol: symbol.to_string(),
        side: side.to_string(),
        quantity: quantity.to_string(),
        price: price.to_string(),
        fee_amount: fee_amount.map(ToOwned::to_owned),
        fee_asset: fee_asset.map(ToOwned::to_owned),
        traded_at: Utc.with_ymd_and_hms(2026, 3, 4, 2, 0, 0).unwrap(),
    }
}

fn decimal(value: &str) -> rust_decimal::Decimal {
    value.parse().expect("valid decimal")
}

fn find_fill<'a>(analytics_body: &'a Value, strategy_id: &str) -> &'a Value {
    analytics_body["fills"]
        .as_array()
        .expect("fills array")
        .iter()
        .find(|fill| fill["strategy_id"] == strategy_id)
        .expect("fill exists")
}

fn find_strategy<'a>(analytics_body: &'a Value, strategy_id: &str) -> &'a Value {
    analytics_body["strategies"]
        .as_array()
        .expect("strategies array")
        .iter()
        .find(|strategy| strategy["strategy_id"] == strategy_id)
        .expect("strategy exists")
}
