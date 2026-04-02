use api_server::{app_with_state, AppState};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use chrono::{TimeZone, Utc};
use serde_json::Value;
use shared_chain::assignment::AddressAssignment;
use shared_db::{BillingOrderRecord, SharedDb, StoredStrategy};
use shared_domain::strategy::{
    GridGeneration, PostTriggerAction, Strategy, StrategyMarket, StrategyMode, StrategyRevision,
    StrategyRuntime, StrategyRuntimeFill, StrategyRuntimeOrder, StrategyStatus,
};
use tower::ServiceExt;

mod support;

use support::register_and_login;

#[tokio::test]
async fn compute_strategy_and_account_snapshots_from_persisted_orders_fills_and_payments() {
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
    assert_eq!(analytics_body["fills"][1]["funding"], "0");
    assert_eq!(analytics_body["strategies"].as_array().expect("strategies").len(), 2);
    assert_eq!(analytics_body["strategies"][0]["strategy_id"], "strategy-alpha");
    assert_eq!(analytics_body["strategies"][0]["realized_pnl"], "0");
    assert_eq!(analytics_body["strategies"][0]["fees_paid"], "1.5");
    assert_eq!(analytics_body["strategies"][0]["funding_total"], "0");
    assert_eq!(analytics_body["user"]["user_id"], "trader@example.com");
    assert_eq!(analytics_body["user"]["realized_pnl"], "15");
    assert_eq!(analytics_body["user"]["fees_paid"], "2.25");
    assert_eq!(analytics_body["user"]["funding_total"], "0");
    assert_eq!(analytics_body["user"]["net_pnl"], "12.75");
    assert_eq!(analytics_body["costs"]["fees_paid"], "2.25");
    assert_eq!(analytics_body["costs"]["funding_total"], "0");

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
        order_lines[1],
        "alpha-order-1,strategy-alpha,BTCUSDT,Sell,Limit,100,1,Filled"
    );
    assert_eq!(
        order_lines[3],
        "beta-order-1,strategy-beta,ETHUSDT,Sell,Limit,50,3,Filled"
    );

    let fills_csv = export_csv(&app, &session_token, "/exports/fills.csv").await;
    let fill_lines: Vec<&str> = fills_csv.trim().lines().collect();
    assert_eq!(
        fill_lines[0],
        "fill_id,strategy_id,order_id,symbol,price,quantity,realized_pnl,fee_amount,fee_asset,fill_type"
    );
    assert_eq!(
        fill_lines[1],
        "alpha-fill-1,strategy-alpha,alpha-order-1,BTCUSDT,110,1,10,1,USDT,GridTakeProfit"
    );
    assert_eq!(
        fill_lines[3],
        "beta-fill-1,strategy-beta,beta-order-1,ETHUSDT,55,3,15,0.75,USDT,GridTakeProfit"
    );

    let strategy_stats_csv = export_csv(&app, &session_token, "/exports/strategy-stats.csv").await;
    let strategy_lines: Vec<&str> = strategy_stats_csv.trim().lines().collect();
    assert_eq!(
        strategy_lines[0],
        "strategy_id,user_id,symbol,realized_pnl,unrealized_pnl,fees_paid,funding_total,net_pnl"
    );
    assert_eq!(
        strategy_lines[1],
        "strategy-alpha,trader@example.com,BTCUSDT,0,0,1.5,0,-1.5"
    );
    assert_eq!(
        strategy_lines[2],
        "strategy-beta,trader@example.com,ETHUSDT,15,0,0.75,0,14.25"
    );

    let payments_csv = export_csv(&app, &session_token, "/exports/payments.csv").await;
    let payment_lines: Vec<&str> = payments_csv.trim().lines().collect();
    assert_eq!(
        payment_lines[0],
        "order_id,email,chain,asset,plan_code,amount,status,address,requested_at,paid_at,tx_hash"
    );
    assert_eq!(
        payment_lines[1],
        "501,trader@example.com,ETH,USDT,monthly,20.00000000,paid,eth-addr-7,2026-03-01T00:00:00+00:00,2026-03-01T00:05:00+00:00,tx-501"
    );
    assert_eq!(
        payment_lines[2],
        "502,trader@example.com,BSC,USDC,quarterly,54.00000000,pending,bsc-addr-3,2026-03-02T00:00:00+00:00,,"
    );
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
            vec![runtime_order("beta-order-1", "Sell", "Limit", Some("50"), "3", "Filled")],
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
            "foreign-strategy",
            "other@example.com",
            "Foreign SOL",
            "SOLUSDT",
            vec![runtime_order("foreign-order-1", "Sell", "Limit", Some("20"), "4", "Filled")],
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

    db.insert_billing_order(&BillingOrderRecord {
        order_id: 503,
        email: "other@example.com".to_string(),
        chain: "SOL".to_string(),
        asset: "USDT".to_string(),
        plan_code: "monthly".to_string(),
        amount: "20.00000000".to_string(),
        requested_at: Utc.with_ymd_and_hms(2026, 3, 3, 0, 0, 0).unwrap(),
        assignment: None,
        paid_at: Some(Utc.with_ymd_and_hms(2026, 3, 3, 0, 10, 0).unwrap()),
        tx_hash: Some("tx-503".to_string()),
        status: "paid".to_string(),
        enqueued_at: None,
    })
    .expect("insert foreign order");
}

fn stored_strategy(
    strategy_id: &str,
    owner_email: &str,
    name: &str,
    symbol: &str,
    orders: Vec<StrategyRuntimeOrder>,
    fills: Vec<StrategyRuntimeFill>,
) -> Strategy {
    let revision = StrategyRevision {
        revision_id: format!("{strategy_id}-rev-1"),
        version: 1,
        generation: GridGeneration::Custom,
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
        status: StrategyStatus::Stopped,
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
        market: StrategyMarket::Spot,
        mode: StrategyMode::SpotClassic,
        draft_revision: revision.clone(),
        active_revision: Some(revision),
        runtime: StrategyRuntime {
            positions: Vec::new(),
            orders,
            fills,
            events: Vec::new(),
            last_preflight: None,
        },
        archived_at: None,
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

fn decimal(value: &str) -> rust_decimal::Decimal {
    value.parse().expect("valid decimal")
}
