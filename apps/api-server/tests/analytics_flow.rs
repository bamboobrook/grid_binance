use api_server::app;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn compute_strategy_and_account_profit_fee_and_cost_views() {
    let app = app();

    let analytics = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics")
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
    assert_eq!(analytics_body["fills"][1]["funding"], "-0.2");
    assert_eq!(
        analytics_body["strategies"]
            .as_array()
            .expect("strategies")
            .len(),
        2
    );
    assert_eq!(analytics_body["strategies"][0]["strategy_id"], "strategy-1");
    assert_eq!(analytics_body["strategies"][0]["realized_pnl"], "0");
    assert_eq!(analytics_body["strategies"][0]["fees_paid"], "1.5");
    assert_eq!(analytics_body["strategies"][0]["funding_total"], "-0.2");
    assert_eq!(analytics_body["user"]["user_id"], "user-1");
    assert_eq!(analytics_body["user"]["realized_pnl"], "15");
    assert_eq!(analytics_body["user"]["fees_paid"], "2.25");
    assert_eq!(analytics_body["user"]["funding_total"], "-0.1");
    assert_eq!(analytics_body["user"]["net_pnl"], "12.65");
    assert_eq!(analytics_body["costs"]["fees_paid"], "2.25");
    assert_eq!(analytics_body["costs"]["funding_total"], "-0.1");

    let export = app
        .oneshot(
            Request::builder()
                .uri("/exports/analytics.csv")
                .body(Body::empty())
                .expect("export request"),
        )
        .await
        .expect("export response");

    assert_eq!(export.status(), StatusCode::OK);
    assert_eq!(
        export
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/csv; charset=utf-8")
    );

    let export_body = response_text(export).await;
    let lines: Vec<&str> = export_body.trim().lines().collect();
    assert_eq!(
        lines[0],
        "strategy_id,user_id,symbol,quantity,entry_price,exit_price,realized_pnl,fee,funding,net_pnl"
    );
    assert_eq!(lines[1], "strategy-1,user-1,BTCUSDT,1,100,110,10,1,0,9");
    assert_eq!(
        lines[2],
        "strategy-1,user-1,BTCUSDT,2,110,105,-10,0.5,-0.2,-10.7"
    );
    assert_eq!(
        lines[3],
        "strategy-2,user-1,ETHUSDT,3,50,55,15,0.75,0.1,14.35"
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
