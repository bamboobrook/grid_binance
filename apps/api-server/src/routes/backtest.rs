use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use backtest_engine::{BacktestConfig, BacktestEngine};
use chrono::NaiveDate;
use serde::Deserialize;

use crate::{
    routes::auth_guard::require_user_session,
    services::{
        auth_service::{AuthError, AuthService},
        backtest_service::{
            BacktestError as TaskBacktestError, BacktestService, CreateBacktestTaskRequest,
        },
        martingale_publish_service::{
            LivePortfolio, MartingalePublishService, PublishError, PublishIntentResponse,
        },
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/backtest/run", post(run_backtest))
        .route("/backtest/tasks", post(create_task).get(list_tasks))
        .route("/backtest/tasks/{id}", get(get_task))
        .route("/backtest/tasks/{id}/pause", post(pause_task))
        .route("/backtest/tasks/{id}/resume", post(resume_task))
        .route("/backtest/tasks/{id}/cancel", post(cancel_task))
        .route("/backtest/tasks/{id}/candidates", get(list_candidates))
        .route("/backtest/candidates/{id}", get(get_candidate))
        .route(
            "/backtest/candidates/{id}/publish-intent",
            post(create_publish_intent),
        )
        .route(
            "/backtest/portfolios/{id}/confirm-start",
            post(confirm_start_portfolio),
        )
}

async fn create_task(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
    Json(request): Json<CreateBacktestTaskRequest>,
) -> Result<(axum::http::StatusCode, Json<shared_db::BacktestTaskRecord>), TaskBacktestError> {
    let session = require_user_session(&auth, &headers).map_err(TaskBacktestError::from)?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(service.create_task(&session.email, request)?),
    ))
}

async fn list_tasks(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
) -> Result<Json<Vec<shared_db::BacktestTaskRecord>>, TaskBacktestError> {
    let session = require_user_session(&auth, &headers).map_err(TaskBacktestError::from)?;
    Ok(Json(service.list_tasks(&session.email)?))
}

async fn get_task(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<shared_db::BacktestTaskRecord>, TaskBacktestError> {
    let session = require_user_session(&auth, &headers).map_err(TaskBacktestError::from)?;
    Ok(Json(service.get_task(&session.email, &id)?))
}

async fn pause_task(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<shared_db::BacktestTaskRecord>, TaskBacktestError> {
    let session = require_user_session(&auth, &headers).map_err(TaskBacktestError::from)?;
    Ok(Json(service.pause_task(&session.email, &id)?))
}

async fn resume_task(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<shared_db::BacktestTaskRecord>, TaskBacktestError> {
    let session = require_user_session(&auth, &headers).map_err(TaskBacktestError::from)?;
    Ok(Json(service.resume_task(&session.email, &id)?))
}

async fn cancel_task(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<shared_db::BacktestTaskRecord>, TaskBacktestError> {
    let session = require_user_session(&auth, &headers).map_err(TaskBacktestError::from)?;
    Ok(Json(service.cancel_task(&session.email, &id)?))
}

async fn list_candidates(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Vec<shared_db::BacktestCandidateRecord>>, TaskBacktestError> {
    let session = require_user_session(&auth, &headers).map_err(TaskBacktestError::from)?;
    Ok(Json(service.list_candidates(&session.email, &id)?))
}

async fn get_candidate(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<shared_db::BacktestCandidateRecord>, TaskBacktestError> {
    let session = require_user_session(&auth, &headers).map_err(TaskBacktestError::from)?;
    Ok(Json(service.get_candidate(&session.email, &id)?))
}

async fn create_publish_intent(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<PublishIntentResponse>, TaskBacktestError> {
    let session = require_user_session(&auth, &headers).map_err(TaskBacktestError::from)?;
    Ok(Json(service.create_publish_intent(&session.email, &id)?))
}

async fn confirm_start_portfolio(
    State(auth): State<AuthService>,
    State(service): State<MartingalePublishService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<LivePortfolio>, PublishError> {
    let session = require_user_session(&auth, &headers)
        .map_err(|_error| PublishError::bad_request("unauthorized"))?;
    Ok(Json(service.confirm_start_portfolio(&session.email, &id)?))
}

#[derive(Debug, Deserialize)]
struct BacktestRequest {
    symbol: String,
    strategy_type: String,
    lower_price: f64,
    upper_price: f64,
    grid_count: u32,
    equal_mode: Option<String>,
    investment: f64,
    start_date: String,
    end_date: String,
    market: Option<String>,
    interval: Option<String>,
    klines: Option<Vec<backtest_engine::KlineRecord>>,
}

#[derive(Debug)]
enum BacktestError {
    Auth(AuthError),
    KlineFetch(String),
    Engine(String),
}

impl IntoResponse for BacktestError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Auth(e) => e.into_response(),
            Self::KlineFetch(msg) | Self::Engine(msg) => (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response(),
        }
    }
}

use axum::response::IntoResponse;

async fn run_backtest(
    State(auth): State<AuthService>,
    headers: HeaderMap,
    Json(body): Json<BacktestRequest>,
) -> Result<Json<backtest_engine::BacktestResult>, BacktestError> {
    let _claims = require_user_session(&auth, &headers).map_err(BacktestError::Auth)?;

    let config = BacktestConfig {
        symbol: body.symbol.clone(),
        strategy_type: body.strategy_type.clone(),
        lower_price: body.lower_price,
        upper_price: body.upper_price,
        grid_count: body.grid_count,
        equal_mode: body.equal_mode.unwrap_or_else(|| "arithmetic".into()),
        investment: body.investment,
        start_date: body.start_date.clone(),
        end_date: body.end_date.clone(),
    };

    let start_ms = NaiveDate::parse_from_str(&body.start_date, "%Y-%m-%d")
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis())
        .unwrap_or(0);
    let end_ms = NaiveDate::parse_from_str(&body.end_date, "%Y-%m-%d")
        .map(|d| {
            d.and_hms_opt(23, 59, 59)
                .unwrap()
                .and_utc()
                .timestamp_millis()
        })
        .unwrap_or(0);

    let market = body.market.as_deref().unwrap_or("spot");
    let interval = body.interval.as_deref().unwrap_or("1h");

    let klines = match body.klines {
        Some(klines) => klines,
        None => fetch_klines_from_binance(market, &body.symbol, interval, start_ms, end_ms)
            .map_err(BacktestError::KlineFetch)?,
    };

    let result =
        BacktestEngine::run(config, klines).map_err(|e| BacktestError::Engine(e.to_string()))?;

    Ok(Json(result))
}

fn fetch_klines_from_binance(
    market: &str,
    symbol: &str,
    interval: &str,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<backtest_engine::KlineRecord>, String> {
    let (api_base, klines_path) = match market {
        "usd-m" => ("https://fapi.binance.com", "/fapi/v1/klines"),
        "coin-m" => ("https://dapi.binance.com", "/dapi/v1/klines"),
        _ => ("https://api.binance.com", "/api/v3/klines"),
    };

    let agent = ureq::AgentBuilder::new()
        .timeout_read(std::time::Duration::from_secs(30))
        .build();

    let mut all_klines: Vec<backtest_engine::KlineRecord> = Vec::new();
    let mut current_start = start_ms;
    let limit = 1000u32;

    while current_start < end_ms {
        let response = agent
            .get(&format!("{api_base}{klines_path}"))
            .query("symbol", symbol)
            .query("interval", interval)
            .query("startTime", &current_start.to_string())
            .query("endTime", &end_ms.to_string())
            .query("limit", &limit.to_string())
            .call()
            .map_err(|e| format!("binance klines request failed: {e}"))?;

        let raw: Vec<Vec<serde_json::Value>> = response
            .into_json()
            .map_err(|e| format!("binance klines parse failed: {e}"))?;

        if raw.is_empty() {
            break;
        }

        for candle in &raw {
            if candle.len() < 6 {
                continue;
            }
            let open_time = candle[0].as_i64().unwrap_or(0);
            let open = candle[1]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let high = candle[2]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let low = candle[3]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let close = candle[4]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let volume = candle[5]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            all_klines.push(backtest_engine::KlineRecord {
                time: open_time.to_string(),
                open,
                high,
                low,
                close,
                volume,
            });

            current_start = open_time + 1;
        }

        if raw.len() < limit as usize {
            break;
        }
    }

    Ok(all_klines)
}
