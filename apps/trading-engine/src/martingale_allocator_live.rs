//! Round 11 Task P1: allocator parsing and rolling-metric helpers.
//!
//! These functions parse allocator config/state from portfolio JSON, compute
//! completed rolling metrics from persisted observations, and serialize state
//! back to JSON for persistence in risk_summary.
//!
//! The initial executor-creation path in `main.rs` uses these to:
//! 1. Parse the allocator config and current state (risk_summary priority).
//! 2. Compute rolling metrics from observations <= rebalance boundary.
//! 3. Call `runtime.rebalance_allocator(...)` when `now_ms >= next_rebalance_ms`.
//! 4. Persist the updated state to `risk_summary["allocator_state"]`.
//!
//! This module is not sufficient for production dynamic allocation. The
//! already-started executor path bypasses the initial gate, and no production
//! writer currently supplies shadow observations for inactive sleeves.

use std::collections::HashMap;

use backtest_engine::martingale::allocator_replay::{
    AllocatorConfig, AllocatorState, RollingMetrics,
};
use serde_json::Value;

/// Parse an `AllocatorConfig` from a JSON value (portfolio_config.allocator_config).
pub fn parse_allocator_config(value: &Value) -> Option<AllocatorConfig> {
    serde_json::from_value(value.clone()).ok()
}

/// Parse the strategy_id → sleeve_id map from a JSON object.
pub fn parse_strategy_to_sleeve_id(value: &Value) -> HashMap<String, String> {
    value
        .as_object()
        .into_iter()
        .flat_map(|obj| obj.iter())
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
        .collect()
}

/// Build an `AllocatorState` from persisted JSON, with a fallback sleeve.
/// Priority: persisted active_sleeve_id > fallback_sleeve.
pub fn allocator_state_from_json(
    value: Option<&Value>,
    fallback_sleeve: &str,
    now_ms: i64,
) -> AllocatorState {
    let active = value
        .and_then(|v| v.get("active_sleeve_id"))
        .and_then(Value::as_str)
        .unwrap_or(fallback_sleeve)
        .to_owned();
    let next_rebalance_ms = value
        .and_then(|v| v.get("next_rebalance_ms"))
        .and_then(Value::as_i64)
        .unwrap_or(now_ms);
    let last_completed_interval_ms = value
        .and_then(|v| v.get("last_completed_interval_ms"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let mut state = AllocatorState::new(active, next_rebalance_ms);
    state.last_completed_interval_ms = last_completed_interval_ms;
    state
}

/// Serialize an `AllocatorState` to JSON for persistence.
pub fn state_to_json(state: &AllocatorState) -> Value {
    serde_json::json!({
        "active_sleeve_id": state.active_sleeve_id,
        "next_rebalance_ms": state.next_rebalance_ms,
        "last_completed_interval_ms": state.last_completed_interval_ms,
        "rolling_metrics_by_sleeve": state.rolling_metrics_by_sleeve
    })
}

/// A single persisted allocator observation (equity snapshot for a sleeve).
#[derive(Debug, Clone)]
pub struct AllocatorObservation {
    pub timestamp_ms: i64,
    pub sleeve_id: String,
    pub equity_quote: f64,
}

/// Parse observations from risk_summary.allocator_observations array.
/// Each observation: {"timestamp_ms": ..., "sleeve_id": "...", "equity_quote": ...}
pub fn parse_observations(value: Option<&Value>) -> Vec<AllocatorObservation> {
    let arr = match value.and_then(Value::as_array) {
        Some(a) => a,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|o| {
            let ts = o.get("timestamp_ms")?.as_i64()?;
            let sleeve = o.get("sleeve_id")?.as_str()?;
            let eq = o.get("equity_quote")?.as_f64()?;
            Some(AllocatorObservation {
                timestamp_ms: ts,
                sleeve_id: sleeve.to_string(),
                equity_quote: eq,
            })
        })
        .collect()
}

/// Compute completed rolling metrics per sleeve using ONLY observations with
/// `timestamp_ms <= rebalance_boundary_ms`. Returns a map of sleeve_id →
/// RollingMetrics suitable for `AllocatorState::rebalance`.
///
/// The rolling window is `[rebalance_boundary_ms - lookback_ms, rebalance_boundary_ms]`.
/// For each sleeve with observations in this window:
/// - rolling_return_quote = last_equity - first_equity (in the window)
/// - rolling_max_dd_pct = max peak-to-trough drawdown in the window
/// - score = cfg.score_function.evaluate(return, dd)
pub fn completed_allocator_metrics(
    observations: &[AllocatorObservation],
    rebalance_boundary_ms: i64,
    cfg: &AllocatorConfig,
    budget: f64,
) -> HashMap<String, RollingMetrics> {
    let lookback_ms = (cfg.lookback_days as i64) * 86_400_000;
    let window_start = rebalance_boundary_ms - lookback_ms;
    // Group observations by sleeve, filtered to window
    let mut by_sleeve: HashMap<String, Vec<(i64, f64)>> = HashMap::new();
    for obs in observations {
        if obs.timestamp_ms <= rebalance_boundary_ms && obs.timestamp_ms >= window_start {
            by_sleeve
                .entry(obs.sleeve_id.clone())
                .or_default()
                .push((obs.timestamp_ms, obs.equity_quote));
        }
    }
    let mut out = HashMap::new();
    for (sleeve, mut pts) in by_sleeve {
        if pts.is_empty() {
            continue;
        }
        pts.sort_by_key(|(t, _)| *t);
        let first_eq = pts.first().map(|(_, e)| *e).unwrap_or(budget);
        let last_eq = pts.last().map(|(_, e)| *e).unwrap_or(budget);
        let rolling_return = last_eq - first_eq;
        // Max DD in window
        let mut peak = pts.first().map(|(_, e)| *e).unwrap_or(budget);
        let mut max_dd = 0.0_f64;
        for (_, eq) in &pts {
            if *eq > peak {
                peak = *eq;
            }
            let dd = if peak > 0.0 {
                ((peak - eq) / peak) * 100.0
            } else {
                0.0
            };
            if dd > max_dd {
                max_dd = dd;
            }
        }
        let score = cfg.score_function.evaluate(rolling_return, max_dd);
        out.insert(
            sleeve,
            RollingMetrics {
                rolling_return_quote: rolling_return,
                rolling_max_dd_pct: max_dd,
                score,
            },
        );
    }
    out
}

/// Read allocator state with priority: risk_summary > config > fallback.
/// Returns (state, allocator_config_if_present).
pub fn read_allocator_state_with_priority(
    portfolio: &shared_db::MartingalePortfolioRecord,
    fallback_sleeve: &str,
    now_ms: i64,
) -> Option<(
    AllocatorState,
    AllocatorConfig,
    HashMap<String, String>,
)> {
    let portfolio_config = portfolio.config.get("portfolio_config")?;
    let allocator_cfg_value = portfolio_config.get("allocator_config")?;
    let cfg = parse_allocator_config(allocator_cfg_value)?;
    let strategy_to_sleeve_raw = portfolio_config.get("strategy_to_sleeve_id")?;
    let strategy_to_sleeve_id = parse_strategy_to_sleeve_id(strategy_to_sleeve_raw);
    if strategy_to_sleeve_id.is_empty() {
        return None;
    }
    // Priority: risk_summary.allocator_state > config.allocator_state > fallback
    let risk_summary_state = portfolio.risk_summary.get("allocator_state");
    let config_state = portfolio_config.get("allocator_state");
    let state_value = risk_summary_state.or(config_state);
    let state = allocator_state_from_json(state_value, fallback_sleeve, now_ms);
    Some((state, cfg, strategy_to_sleeve_id))
}
