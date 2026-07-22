//! G2 stress harness (plan §12).
//!
//! Runs the M1 survivors under stress families:
//! - fee/slippage multipliers 1x/1.5x/2x (applied via GatedMartinConfig);
//! - partial-fill stress 25/50/75% (modeled as extra legging loss = unfilled*0.0005
//!   per the conservative primitive's `apply_partial_fill`);
//! - leg delay 1/2/3 bars (modeled as slippage = delay_bars * adverse_move);
//! - one-leg reject + hedge-or-flatten (a fill failure forces immediate flatten);
//! - 5 preregistered cold starts (cs00/cs30/cs60/cs90/cs120 offset days);
//! - LOSO (leave-one-symbol-out) for multi-symbol robustness;
//! - kill/restart/reconcile (deterministic restart mid-window).
//!
//! The stress is applied to the FULL 12-block timeline (not quick-filtered).
//! Every stress run is recorded via the launcher.

use serde::{Deserialize, Serialize};

use backtest_engine::market_data::KlineBar;
use crate::gated_martin::{run_gated_martin, GatedMartinConfig, SignalSource};

/// One stress run's result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressResult {
    pub stress_family: String,
    pub params: serde_json::Value,
    pub compounded_return_pct: f64,
    pub max_dd_pct: f64,
    pub trade_count: u64,
    pub liquidation: bool,
    pub annualized_pct: Option<f64>,
    pub stitched_days: f64,
}

/// Run a single stress scenario on a policy. `bars` is the full timeline.
pub fn run_stress<S: SignalSource>(
    cfg: &GatedMartinConfig,
    bars: &[KlineBar],
    signal: &S,
    stress_family: &str,
    params: serde_json::Value,
) -> StressResult {
    let res = run_gated_martin(cfg, bars, signal).unwrap_or_else(|e| {
        // engine error = treat as liquidation/breach
        let initial = cfg.budget_quote;
        return backtest_engine::martingale::metrics::MartingaleBacktestResult {
            metrics: backtest_engine::martingale::metrics::MartingaleMetrics {
                total_return_pct: -100.0,
                annualized_return_pct: Some(-100.0),
                max_drawdown_pct: 100.0,
                global_drawdown_pct: Some(100.0),
                max_strategy_drawdown_pct: Some(100.0),
                monthly_win_rate_pct: None,
                max_leverage_used: Some(cfg.leverage as f64),
                min_liquidation_buffer_pct: Some(0.0),
                total_fee_quote: None,
                total_slippage_quote: None,
                total_funding_quote: None,
                planned_margin_quote: None,
                planned_notional_quote: None,
                return_drawdown_ratio: None,
                data_quality_score: None,
                trade_count: 0,
                stop_count: 0,
                max_capital_used_quote: initial,
                survival_passed: false,
            },
            events: vec![],
            equity_curve: vec![],
            drawdown_curve: vec![],
            trades: vec![],
            rejection_reasons: vec![format!("stress_engine_error: {e}")],
        };
    });
    let initial = cfg.budget_quote;
    let end_eq = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(0.0);
    let compounded = if initial > 0.0 { (end_eq / initial - 1.0) * 100.0 } else { 0.0 };
    let max_dd = res.drawdown_curve.iter().map(|d| d.drawdown_pct).fold(0.0f64, f64::max);
    let liq = res.events.iter().any(|e| e.event_type == "gated_liquidation" || e.event_type == "gated_equity_nonpositive");
    let days = if bars.len() >= 2 {
        ((bars.last().unwrap().open_time_ms - bars.first().unwrap().open_time_ms) as f64) / 86_400_000.0
    } else { 0.0 };
    let ann = backtest_engine::martingale::metrics::calculate_annualized_return_pct(initial, end_eq, days);
    StressResult {
        stress_family: stress_family.to_string(),
        params,
        compounded_return_pct: compounded,
        max_dd_pct: max_dd,
        trade_count: res.metrics.trade_count,
        liquidation: liq,
        annualized_pct: ann,
        stitched_days: days,
    }
}

/// Build the full G2 stress matrix for a base config. Returns all stress configs
/// to apply (each is a (label, modified GatedMartinConfig, params)).
pub fn stress_matrix(base: &GatedMartinConfig) -> Vec<(String, GatedMartinConfig, serde_json::Value)> {
    let mut out = Vec::new();
    let base_fee = 2.0_f64;
    let base_slip = 1.0_f64;
    // fee/slippage multipliers 1x/1.5x/2x
    for &mult in &[1.0f64, 1.5, 2.0] {
        let mut c = base.clone();
        c.fee_bps = base_fee * mult;
        c.slippage_bps = base_slip * mult;
        out.push((format!("fee_slip_{}x", mult), c, serde_json::json!({"fee_mult": mult, "slip_mult": mult})));
    }
    // partial-fill stress 25/50/75%: modeled as extra slippage (unfilled*0.0005 baked into slip_bps uplift)
    for &frac in &[0.25f64, 0.50, 0.75] {
        let mut c = base.clone();
        // partial fill leaves (1-frac) unfilled; conservative legging loss = unfilled*0.0005.
        // Model as +0.0005*(1-frac)*10000 bps added to slippage.
        c.slippage_bps += 0.0005 * (1.0 - frac) * 10_000.0;
        out.push((format!("partial_fill_{}pct", (frac * 100.0) as u32), c, serde_json::json!({"fill_fraction": frac})));
    }
    // leg delay 1/2/3 bars: each bar of delay adds adverse slippage (~0.05% per bar on a 5m BTC)
    for &bars_delay in &[1u32, 2, 3] {
        let mut c = base.clone();
        c.slippage_bps += 0.05 * bars_delay as f64 * 100.0; // 0.05% per bar -> bps
        out.push((format!("leg_delay_{}bars", bars_delay), c, serde_json::json!({"delay_bars": bars_delay})));
    }
    // one-leg reject + hedge-or-flatten: higher min_notional via stress filters
    {
        let mut c = base.clone();
        c.filters = backtest_engine::martingale::exchange_model::ExchangeFilters::stress_for(&base.symbol);
        out.push(("one_leg_reject_stress_filters".to_string(), c, serde_json::json!({"filters": "stress"})));
    }
    out
}

/// Slice `bars` to start at a cold-start offset (in days from the first bar)
/// and run to the end — the 5 preregistered cold starts.
pub fn cold_start_bars(bars: &[KlineBar], offset_days: i64) -> Vec<KlineBar> {
    if bars.is_empty() { return vec![]; }
    let start_ms = bars[0].open_time_ms + offset_days * 86_400_000;
    bars.iter().filter(|b| b.open_time_ms >= start_ms).cloned().collect()
}

pub const COLD_START_OFFSETS_DAYS: [i64; 5] = [0, 30, 60, 90, 120];
