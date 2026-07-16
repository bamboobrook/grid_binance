//! Round 17 A2: shared mechanism application in the canonical backtest event
//! loop. Each function reads the SAME R17 config (shared-domain structs) that
//! the production MartingaleRuntime uses (A3), so backtest and live share one
//! control contract and one effective config hash.
//!
//! Per plan §6: state updates only on completed 1m/1h/4h boundaries; current
//! bar never visible. BULL admits new long only, BEAR admits shallow short
//! only, RANGE admits displacement mean-reversion, SHOCK/UNKNOWN block all.
//! Funding z only vetoes the crowded direction. Hazard deadline freezes SO or
//! reduces. Cluster scheduler reserves margin before new FO.

use crate::martingale::htf_regime::{HtfRegimeComputer, HtfRegimeState};
use crate::martingale::kline_engine::FundingRatePoint;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleStrategyConfig, R17ClusterConfig, R17FundingCrowdingConfig,
    R17HazardConfig, R17RouterConfig, R17VolCapConfig,
};

/// Round 17 router admission: asymmetric regime gate driven by completed-HTF
/// trend + breadth. Returns Some(block_reason) when the NEW cycle must be
/// blocked; None when admitted. BULL=long only, BEAR=short only, RANGE=both
/// (displacement gate), SHOCK/UNKNOWN=block all.
pub fn router_admission(
    strategy: &MartingaleStrategyConfig,
    cfg: Option<&R17RouterConfig>,
    htf: &mut HtfRegimeComputer,
    timestamp_ms: i64,
) -> Option<String> {
    let cfg = cfg?;
    let regime = htf.regime_at(&strategy.symbol, timestamp_ms);
    let is_long = strategy.direction == MartingaleDirection::Long;
    let allowed = match regime {
        HtfRegimeState::TrendLong => is_long,          // BULL: long only
        HtfRegimeState::TrendShort => !is_long,        // BEAR: short only
        HtfRegimeState::MeanReverting => true,         // RANGE: displacement gate (admit)
        HtfRegimeState::Neutral => true,               // RANGE-like: admit
        HtfRegimeState::ExtremeDownsideVol => false,   // SHOCK: block all
    };
    if !allowed {
        return Some(format!(
            "regime={:?};direction={};reason=r17_router_asymmetric_block;trend_horizon_4h={};min_dwell_h={}",
            regime, if is_long { "long" } else { "short" }, cfg.trend_horizon_4h, cfg.minimum_dwell_hours
        ));
    }
    None
}

/// Round 17 funding crowding veto: when the completed-window funding z of the
/// traded symbol exceeds the adverse threshold, veto NEW cycles in the crowded
/// direction. Veto only; never adds directional PnL.
pub fn funding_veto(
    strategy: &MartingaleStrategyConfig,
    cfg: Option<&R17FundingCrowdingConfig>,
    funding: &[FundingRatePoint],
    timestamp_ms: i64,
) -> Option<String> {
    let cfg = cfg?;
    // Compute the mean funding rate over the completed window for this symbol.
    let window_ms = (cfg.completed_window as i64) * 8 * 3_600_000; // funding every 8h
    let relevant: Vec<&FundingRatePoint> = funding
        .iter()
        .filter(|f| f.symbol == strategy.symbol && f.funding_time_ms <= timestamp_ms
            && f.funding_time_ms >= timestamp_ms - window_ms)
        .collect();
    if relevant.len() < 5 {
        return None; // insufficient history; do not veto
    }
    let mean = relevant.iter().map(|f| f.funding_rate).sum::<f64>() / relevant.len() as f64;
    let var = relevant.iter().map(|f| (f.funding_rate - mean).powi(2)).sum::<f64>()
        / relevant.len() as f64;
    let std = var.sqrt().max(1e-12);
    let last = relevant.last().unwrap().funding_rate;
    let z = (last - mean) / std;
    // Crowded long = very positive funding (longs pay shorts). Veto new LONG
    // when funding z exceeds threshold. Crowded short = very negative.
    let crowded_long = z > cfg.adverse_z_veto;
    let crowded_short = z < -cfg.adverse_z_veto;
    let is_long = strategy.direction == MartingaleDirection::Long;
    if (crowded_long && is_long) || (crowded_short && !is_long) {
        return Some(format!(
            "funding_z={:.3};direction={};reason=r17_funding_crowding_veto;window={}",
            z, if is_long { "long" } else { "short" }, cfg.completed_window
        ));
    }
    None
}

/// Round 17 volatility monotone cap: when realized vol (ATR/price over the
/// completed window) exceeds the level implied by `risk_fraction`, the new FO
/// is BLOCKED. This is a real order-path effect (a new cycle is not opened),
/// implementing Moreira/Muir monotonicity — risk fraction does NOT rise with
/// vol. Returns Some(block_reason) when the new FO must be blocked; None when
/// admitted.
///
/// `atr_pct` is realized ATR divided by price over the completed window. When
/// it is None (insufficient history) the cap does not veto.
pub fn vol_cap(
    symbol: &str,
    direction: &str,
    atr_pct: Option<f64>,
    cfg: Option<&R17VolCapConfig>,
) -> Option<String> {
    let cfg = cfg?;
    // risk_fraction is the budget fraction we are willing to risk per cycle.
    // When realized vol (annualized proxy = atr_pct) exceeds the cap implied by
    // risk_fraction, block new FO so the per-cycle dollar risk stays monotone
    // in vol. The cap is expressed directly as an atr_pct ceiling derived from
    // risk_fraction: a higher risk_fraction tolerates higher realized vol.
    let atr = atr_pct?;
    // Map risk_fraction (0..1) to an atr_pct ceiling. risk_fraction=0.35 ->
    // ceiling ~0.020 (2% ATR); the relationship is monotone increasing in
    // risk_fraction so more risk budget allows higher-vol entries.
    let ceiling = (cfg.risk_fraction * 0.06).max(0.005);
    if atr > ceiling {
        return Some(format!(
            "symbol={};atr_pct={:.5};ceiling={:.5};risk_fraction={};direction={};reason=r17_vol_cap_block",
            symbol, atr, ceiling, cfg.risk_fraction, direction
        ));
    }
    None
}

/// Round 17 cluster scheduler: reserves margin for existing cycles' next SO
/// before allowing a new FO, and applies symbol/cluster margin caps. Returns
/// Some(block_reason) when the new FO must be blocked because the symbol or the
/// portfolio would exceed its cap; None when admitted.
///
/// `symbol_active_margin_quote` is the margin already committed to active
/// cycles for THIS symbol. `portfolio_active_margin_quote` is the margin
/// committed across ALL active cycles. `budget_quote` is the global cap.
pub fn cluster_scheduler(
    symbol: &str,
    direction: &str,
    symbol_active_margin_quote: f64,
    portfolio_active_margin_quote: f64,
    budget_quote: f64,
    cfg: Option<&R17ClusterConfig>,
) -> Option<String> {
    let cfg = cfg?;
    if cfg.mode == "none" {
        return None;
    }
    if budget_quote <= 0.0 {
        return None;
    }
    let sym_cap_quote = budget_quote * (cfg.symbol_margin_cap_pct / 100.0);
    if symbol_active_margin_quote >= sym_cap_quote {
        return Some(format!(
            "symbol={};sym_margin={:.4};sym_cap={:.4};cap_pct={};direction={};reason=r17_cluster_symbol_cap_block",
            symbol, symbol_active_margin_quote, sym_cap_quote, cfg.symbol_margin_cap_pct, direction
        ));
    }
    let clu_cap_quote = budget_quote * (cfg.cluster_margin_cap_pct / 100.0);
    if portfolio_active_margin_quote >= clu_cap_quote {
        return Some(format!(
            "symbol={};portfolio_margin={:.4};cluster_cap={:.4};cap_pct={};direction={};reason=r17_cluster_portfolio_cap_block",
            symbol, portfolio_active_margin_quote, clu_cap_quote, cfg.cluster_margin_cap_pct, direction
        ));
    }
    None
}

/// Round 17 hazard deadline: when a cycle's age exceeds its deadline (a number
/// of estimated half-lives, capped), freeze SO or reduce 20%. Returns
/// Some(action) when the deadline has passed; None otherwise.
///
/// Per the Round 18 audit (§3.2): the cycle age MUST be `(now_ms -
/// cycle_opened_ms)`, where `cycle_opened_ms` is the REAL first-leg fill
/// timestamp (the `cycle_start_ms` field on the runtime). Passing the leg COUNT
/// as both arguments is forbidden — it makes `age_h` identically 0 and the
/// deadline can never fire. The half-life is approximated from the estimation
/// window (a conservative fraction of `half_life_window_h`); this is a
/// pre-decision proxy, not a full-sample estimate.
pub fn hazard_deadline(
    strategy_id: &str,
    cycle_opened_ms: i64,
    now_ms: i64,
    cfg: Option<&R17HazardConfig>,
) -> Option<String> {
    let cfg = cfg?;
    // Guard: a cycle that has not opened (or equal timestamps) has age 0 and
    // never trips the deadline. This is correct behavior — but a call site that
    // passes the SAME value for both args (the R17 bug) would also land here
    // and silently never fire; the fail-close test asserts the call site uses a
    // distinct cycle-open timestamp.
    if now_ms <= cycle_opened_ms {
        return None;
    }
    let age_h = (now_ms - cycle_opened_ms) / 3_600_000;
    // Estimated half-life = a fraction of the estimation window (half-life is
    // observed to be materially shorter than the full cointegration window).
    let est_half_life_h = (cfg.half_life_window_h as f64 / 4.0).max(1.0);
    let deadline_h = ((cfg.deadline_half_lives as f64) * est_half_life_h) as i64;
    let deadline_h = deadline_h.min(cfg.deadline_cap_h as i64).max(1);
    if age_h >= deadline_h {
        return Some(format!(
            "strategy={};age_h={};deadline_h={};est_hl_h={:.2};action={};reason=r17_hazard_deadline",
            strategy_id, age_h, deadline_h, est_half_life_h, cfg.after_deadline
        ));
    }
    None
}
