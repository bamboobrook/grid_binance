mod support;

use api_server::app;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use support::register_and_login;
use tower::ServiceExt;

#[tokio::test]
async fn legacy_backtest_run_still_works() {
    let app = app();
    let token = register_and_login(&app, "legacy-backtest@example.com", "pass1234").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/backtest/run")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "symbol": "BTCUSDT",
                        "strategy_type": "grid",
                        "lower_price": 100.0,
                        "upper_price": 110.0,
                        "grid_count": 5,
                        "equal_mode": "arithmetic",
                        "investment": 1000.0,
                        "start_date": "2024-01-01",
                        "end_date": "2024-01-02",
                        "klines": [
                            { "time": "2024-01-01T00:00:00Z", "open": 101.0, "high": 106.0, "low": 99.0, "close": 104.0, "volume": 1.0 },
                            { "time": "2024-01-01T01:00:00Z", "open": 104.0, "high": 109.0, "low": 102.0, "close": 108.0, "volume": 1.0 }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert!(body.get("total_pnl").is_some());
    assert!(body.get("trade_count").is_some());
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
