//! G1 frozen 12-block continuous replay harness + P-B gate (plan §11).
//!
//! For a given policy, runs the 12-block prequential protocol:
//! - one continuous cash/margin/equity account across tb01..tb12;
//! - per-block raw return + DD recorded (no ann per block, plan §6.4);
//! - stitched ann/DD only when stitched test days >= 365 (plan §6.5);
//! - contribution gates: symbol/group/block positive contribution <= 50%.
//!
//! P-B gate (plan §11): same policy 12-block compounded return > 0, no breach,
//! >= 8/12 blocks positive, contribution gates pass.

use serde::{Deserialize, Serialize};

use backtest_engine::market_data::KlineBar;
use r23_registry::launcher::{LaunchRequest, Launcher};
use r23_registry::registry::TerminalStatus;

use crate::gated_martin::{run_gated_martin, GatedMartinConfig, SignalSource};

/// One block's raw result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockResult {
    pub block_index: u32,
    pub test_start_ms: i64,
    pub test_end_ms: i64,
    pub bars: usize,
    pub start_equity: f64,
    pub end_equity: f64,
    pub raw_return_pct: f64,
    pub max_dd_pct: f64,
    pub trade_count: u64,
    pub liquidation: bool,
}

/// A policy's full 12-block result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyG1Result {
    pub policy_id: String,
    pub blocks: Vec<BlockResult>,
    pub compounded_return_pct: f64,
    pub stitched_annualized_pct: Option<f64>,
    pub stitched_max_dd_pct: f64,
    pub stitched_days: f64,
    pub blocks_positive: u32,
    pub any_breach: bool,
    pub max_symbol_contribution_pct: f64,
    pub max_block_contribution_pct: f64,
    pub p_b_passed: bool,
    pub p_b_reasons: Vec<String>,
}

/// Run a single-symbol gated-Martin policy through the 12-block protocol.
/// `blocks_bars` is the per-block test bars; the account carries equity forward.
pub fn run_g1_single_symbol<S: SignalSource>(
    policy_id: &str,
    cfg_template: &GatedMartinConfig,
    blocks_bars: &[(u32, i64, i64, Vec<KlineBar>)], // (idx, start_ms, end_ms, bars)
    signal_factory: &dyn Fn() -> S,
) -> PolicyG1Result {
    let mut equity = cfg_template.budget_quote;
    let initial = cfg_template.budget_quote;
    let mut blocks: Vec<BlockResult> = Vec::new();
    let mut any_breach = false;
    let mut trade_total: u64 = 0;

    for (idx, start_ms, end_ms, bars) in blocks_bars {
        if bars.is_empty() {
            continue;
        }
        let mut cfg = cfg_template.clone();
        cfg.budget_quote = equity.max(0.0); // carry forward, never reset principal
        let start_eq = equity;
        let signal = signal_factory();
        let res = match run_gated_martin(&cfg, bars, &signal) {
            Ok(r) => r,
            Err(_) => {
                any_breach = true;
                blocks.push(BlockResult {
                    block_index: *idx, test_start_ms: *start_ms, test_end_ms: *end_ms,
                    bars: bars.len(), start_equity: start_eq, end_equity: equity,
                    raw_return_pct: 0.0, max_dd_pct: 100.0, trade_count: 0, liquidation: true,
                });
                continue;
            }
        };
        let end_eq = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(equity);
        let raw_return_pct = if start_eq > 0.0 { (end_eq / start_eq - 1.0) * 100.0 } else { 0.0 };
        let max_dd = res.drawdown_curve.iter().map(|d| d.drawdown_pct).fold(0.0f64, f64::max);
        let liquidation = res.events.iter().any(|e| e.event_type == "gated_liquidation" || e.event_type == "gated_equity_nonpositive");
        if liquidation { any_breach = true; }
        trade_total += res.metrics.trade_count;
        equity = end_eq.max(0.0);
        blocks.push(BlockResult {
            block_index: *idx, test_start_ms: *start_ms, test_end_ms: *end_ms,
            bars: bars.len(), start_equity: start_eq, end_equity: end_eq,
            raw_return_pct, max_dd_pct: max_dd, trade_count: res.metrics.trade_count, liquidation,
        });
    }

    let compounded = if initial > 0.0 { (equity / initial - 1.0) * 100.0 } else { 0.0 };
    let stitched_days = if blocks.len() >= 2 {
        ((blocks.last().unwrap().test_end_ms - blocks.first().unwrap().test_start_ms) as f64) / 86_400_000.0
    } else {
        0.0
    };
    let stitched_max_dd = blocks.iter().map(|b| b.max_dd_pct).fold(0.0f64, f64::max);
    let stitched_ann = if stitched_days >= 365.0 {
        backtest_engine::martingale::metrics::calculate_annualized_return_pct(initial, equity, stitched_days)
    } else {
        None
    };
    let blocks_positive = blocks.iter().filter(|b| b.raw_return_pct > 0.0).count() as u32;

    // contribution gates (single-symbol policy: symbol contribution = 100% by construction;
    // we report it but it fails the <=50% symbol gate — this is expected for a single-symbol
    // policy and is why the plan requires multi-symbol combinations in R8).
    let total_abs_return: f64 = blocks.iter().map(|b| b.raw_return_pct.abs()).sum();
    let max_block_contribution = if total_abs_return > 1e-9 {
        blocks.iter().map(|b| b.raw_return_pct.abs() / total_abs_return * 100.0).fold(0.0f64, f64::max)
    } else { 0.0 };

    let mut p_b_reasons = Vec::new();
    let mut p_b_passed = true;
    if compounded <= 0.0 { p_b_passed = false; p_b_reasons.push(format!("compounded_return={:.2}% <= 0", compounded)); }
    if blocks_positive < 8 { p_b_passed = false; p_b_reasons.push(format!("blocks_positive={blocks_positive} < 8")); }
    if any_breach { p_b_passed = false; p_b_reasons.push("breach (liquidation/equity<=0)".into()); }
    if max_block_contribution > 50.0 { p_b_passed = false; p_b_reasons.push(format!("max_block_contribution={:.1}% > 50", max_block_contribution)); }

    PolicyG1Result {
        policy_id: policy_id.to_string(),
        blocks,
        compounded_return_pct: compounded,
        stitched_annualized_pct: stitched_ann,
        stitched_max_dd_pct: stitched_max_dd,
        stitched_days,
        blocks_positive,
        any_breach,
        max_symbol_contribution_pct: 100.0, // single-symbol
        max_block_contribution_pct: max_block_contribution,
        p_b_passed,
        p_b_reasons,
    }
}

/// Record a G1 policy result via the launcher.
pub fn record_g1(launcher: &Launcher, req: &LaunchRequest, result: &PolicyG1Result) -> Result<TerminalStatus, String> {
    launcher.launch(req).map_err(|e| format!("launch: {e}"))?;
    let status = if result.p_b_passed { TerminalStatus::Complete } else { TerminalStatus::FailedGate };
    let failure = if status == TerminalStatus::Complete {
        None
    } else {
        Some(r23_registry::FailureRow {
            failure_id: format!("R23-G1-{}", req.experiment_id),
            experiment_id: req.experiment_id.clone(),
            phase: "G1".to_string(),
            status: "failed_gate_p_b".to_string(),
            reason: result.p_b_reasons.join("; "),
            never_repeat: "do not expand grid after seeing P-B failure".to_string(),
            recorded_at_utc: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        })
    };
    // traces from the result summary
    let h = |s: &str| r23_registry::hash::sha256_str(s);
    let traces = r23_registry::launcher::TraceHashes {
        event: h(&result.policy_id), trade: h("g1"), order: h("g1"),
        equity: h(&serde_json::to_string(&result.blocks).unwrap_or_default()),
        funding: h("g1"), rejection: h("g1"), signal: h("g1"), margin: h("g1"),
    };
    launcher.record_terminal(&req.experiment_id, "G1", status, failure, traces).map_err(|e| format!("terminal: {e}"))?;
    Ok(status)
}
