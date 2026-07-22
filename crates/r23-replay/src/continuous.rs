//! Continuous prequential replay driver.
//!
//! Routes every replay through the production-conservative sync cycle engine
//! (conservative gates wired in at FO/SO emit sites) and records running +
//! terminal registry rows via the launcher.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use backtest_engine::martingale::kline_engine::{set_fee_bps_override, set_slippage_bps_override};
use backtest_engine::martingale::metrics::MartingaleBacktestResult;
use backtest_engine::martingale::sync_cycle_engine::{
    run_synchronized_cycle_replay, SynchronizedFit,
};
use shared_domain::martingale::SynchronizedCycleConfig;
use backtest_engine::market_data::KlineBar;
use backtest_engine::martingale::kline_engine::FundingRatePoint;
use r23_registry::launcher::{LaunchRequest, Launcher, TraceHashes as RegTraceHashes};
use r23_registry::registry::TerminalStatus;

use crate::account::ContinuousAccount;
use crate::summary::{extract_trace_hashes, parse_sync_summary};

/// One block's inputs: its train-fit pairs and its test-window bars.
#[derive(Debug, Clone)]
pub struct BlockReplayInput {
    /// 1..=12.
    pub block_index: u32,
    /// Pairs fitted on `[fit_start, test_start_of_this_block - purge]`. The
    /// engine consumes these; it never re-fits (anti-overfit contract).
    pub train_fits: Vec<SynchronizedFit>,
    /// Bars in this block's test window, sorted by open_time_ms ascending.
    pub test_bars: Vec<KlineBar>,
}

/// How cycle/equity continuity is preserved across the 12 blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayMode {
    /// Merge all 12 blocks' test bars into one stream and call the engine ONCE.
    /// Full inter-block cycle + equity continuity. Production-faithful path.
    ContinuousMerged,
    /// Call the engine once per block, carrying settled equity forward as the
    /// next block's budget. Cycle state resets between blocks but the account
    /// balance is continuous. Used by directed tests and per-block refit.
    BlockWiseCarry,
}

#[derive(Debug, Clone)]
pub struct ContinuousReplayConfig {
    pub mode: ReplayMode,
    pub martingale_cfg: SynchronizedCycleConfig,
    pub budget_quote: f64,
    pub funding_rates: Vec<FundingRatePoint>,
    /// Optional fee/slippage overrides (production defaults if None).
    pub fee_bps_override: Option<f64>,
    pub slippage_bps_override: Option<f64>,
}

/// The outcome of one continuous replay.
#[derive(Debug, Clone)]
pub struct ReplayOutcome {
    pub merged_result: Option<MartingaleBacktestResult>,
    pub per_block_results: Vec<MartingaleBacktestResult>,
    pub account: ContinuousAccount,
    pub trace_hashes: RegTraceHashes,
    pub engine_error: Option<String>,
    pub breach: bool,
}

impl ReplayOutcome {
    pub fn is_ok(&self) -> bool {
        self.engine_error.is_none() && !self.breach
    }
}

/// Run a continuous 12-block replay. Returns the outcome. This function does
/// NOT write registry rows — call [`record_outcome`] afterwards (or use
/// [`run_and_record`] for the combined flow).
pub fn run_continuous_replay(cfg: &ContinuousReplayConfig, blocks: &[BlockReplayInput]) -> ReplayOutcome {
    if let Some(f) = cfg.fee_bps_override {
        set_fee_bps_override(f);
    }
    if let Some(s) = cfg.slippage_bps_override {
        set_slippage_bps_override(s);
    }

    let mut account = ContinuousAccount::new(cfg.budget_quote);

    match cfg.mode {
        ReplayMode::ContinuousMerged => {
            // Merge all blocks' test bars into one sorted stream. We must NOT
            // dedup by open_time_ms: at each timestamp there are N legs (one bar
            // per leg), and the engine groups bars by timestamp. Deduping would
            // silently drop all but one leg.
            let mut merged: Vec<KlineBar> = Vec::new();
            for b in blocks {
                merged.extend_from_slice(&b.test_bars);
            }
            merged.sort_by(|a, b| {
                a.open_time_ms
                    .cmp(&b.open_time_ms)
                    .then_with(|| a.symbol.cmp(&b.symbol))
            });

            // Use the union of all train_fits; the engine consumes them once and
            // never re-fits. (For the merged path the strictest, earliest fit
            // window governs — the caller is responsible for only supplying
            // fits computed on data <= each block's test_start - purge.)
            let mut all_fits: Vec<SynchronizedFit> = Vec::new();
            for b in blocks {
                for f in &b.train_fits {
                    if !all_fits.iter().any(|x| x.group_id == f.group_id) {
                        all_fits.push(f.clone());
                    }
                }
            }

            let res = run_synchronized_cycle_replay(
                &cfg.martingale_cfg,
                &all_fits,
                &merged,
                &cfg.funding_rates,
                cfg.budget_quote,
            );
            match res {
                Ok(result) => {
                    let (trace_hashes, breach) = build_traces_and_breach(&result);
                    if breach {
                        account.any_equity_nonpositive = true;
                    }
                    account.current_equity_quote = result
                        .equity_curve
                        .last()
                        .map(|e| e.equity_quote)
                        .unwrap_or(cfg.budget_quote);
                    ReplayOutcome {
                        merged_result: Some(result),
                        per_block_results: vec![],
                        account,
                        trace_hashes,
                        engine_error: None,
                        breach,
                    }
                }
                Err(e) => ReplayOutcome {
                    merged_result: None,
                    per_block_results: vec![],
                    account,
                    trace_hashes: empty_traces(),
                    engine_error: Some(e),
                    breach: false,
                },
            }
        }
        ReplayMode::BlockWiseCarry => {
            let mut per_block = Vec::new();
            let mut engine_error: Option<String> = None;
            let mut breach = false;
            for b in blocks {
                let budget = account.next_block_budget();
                let starting = account.current_equity_quote;
                let res = run_synchronized_cycle_replay(
                    &cfg.martingale_cfg,
                    &b.train_fits,
                    &b.test_bars,
                    &cfg.funding_rates,
                    budget,
                );
                match res {
                    Ok(result) => {
                        let (th, bch) = build_traces_and_breach(&result);
                        if bch {
                            breach = true;
                        }
                        let ending = result
                            .equity_curve
                            .last()
                            .map(|e| e.equity_quote)
                            .unwrap_or(budget);
                        let equity_nonpos = result
                            .equity_curve
                            .iter()
                            .any(|e| e.equity_quote <= 0.0);
                        let liq = parse_sync_summary(&result)
                            .ok()
                            .and_then(|s| s.liquidation_count)
                            .unwrap_or(0);
                        account.settle_block(
                            b.block_index,
                            starting,
                            ending,
                            0.0,
                            0.0,
                            0.0,
                            equity_nonpos,
                            liq,
                        );
                        per_block.push(result);
                        // carry the last block's trace hashes for the registry row
                        let _ = th;
                    }
                    Err(e) => {
                        engine_error = Some(e);
                        break;
                    }
                }
            }
            // The registry terminal uses the last block's traces (or empty).
            let trace_hashes = per_block
                .last()
                .map(|r| build_traces_and_breach(r).0)
                .unwrap_or_else(empty_traces);
            ReplayOutcome {
                merged_result: None,
                per_block_results: per_block,
                account,
                trace_hashes,
                engine_error,
                breach,
            }
        }
    }
}

fn build_traces_and_breach(result: &MartingaleBacktestResult) -> (RegTraceHashes, bool) {
    let summary = parse_sync_summary(result).ok();
    let traces = match &summary {
        Some(s) => {
            let t = extract_trace_hashes(s);
            RegTraceHashes {
                event: t.event,
                trade: t.trade,
                order: t.order,
                equity: t.equity,
                funding: t.funding,
                rejection: t.rejection,
                signal: t.signal,
                margin: t.margin,
            }
        }
        None => empty_traces(),
    };
    let breach = summary.as_ref().and_then(|s| s.breach).unwrap_or(false)
        || result.equity_curve.iter().any(|e| e.equity_quote <= 0.0);
    (traces, breach)
}

fn empty_traces() -> RegTraceHashes {
    let h = r23_registry::hash::sha256_str("");
    RegTraceHashes {
        event: h.clone(),
        trade: h.clone(),
        order: h.clone(),
        equity: h.clone(),
        funding: h.clone(),
        rejection: h.clone(),
        signal: h.clone(),
        margin: h,
    }
}

/// Record a completed outcome to the registry via the launcher. Writes a Running
/// row, then a Terminal row (Complete if ok, FailedExecution otherwise with a
/// paired failure row). Returns the terminal status used.
pub fn record_outcome(
    launcher: &Launcher,
    req: &LaunchRequest,
    outcome: &ReplayOutcome,
) -> Result<TerminalStatus> {
    launcher.launch(req).map_err(|e| anyhow!("launch failed: {e}"))?;
    if outcome.is_ok() {
        launcher
            .record_terminal(
                &req.experiment_id,
                &req.phase,
                TerminalStatus::Complete,
                None,
                outcome.trace_hashes.clone(),
            )
            .map_err(|e| anyhow!("record terminal complete failed: {e}"))?;
        Ok(TerminalStatus::Complete)
    } else {
        let reason = outcome
            .engine_error
            .clone()
            .unwrap_or_else(|| "execution breach (liquidation or equity<=0)".to_string());
        let failure = r23_registry::FailureRow {
            failure_id: format!("R23-R1-{}", req.experiment_id),
            experiment_id: req.experiment_id.clone(),
            phase: req.phase.clone(),
            status: "failed_execution".to_string(),
            reason,
            never_repeat: "any liquidation or equity<=0 immediately terminates the shared account; never retry the same policy".to_string(),
            recorded_at_utc: now_utc(),
        };
        launcher
            .record_terminal(
                &req.experiment_id,
                &req.phase,
                TerminalStatus::FailedExecution,
                Some(failure),
                outcome.trace_hashes.clone(),
            )
            .map_err(|e| anyhow!("record terminal failure failed: {e}"))?;
        Ok(TerminalStatus::FailedExecution)
    }
}

/// Combined flow: run the replay and record the outcome in one call.
pub fn run_and_record(
    launcher: &Launcher,
    req: &LaunchRequest,
    cfg: &ContinuousReplayConfig,
    blocks: &[BlockReplayInput],
) -> Result<(ReplayOutcome, TerminalStatus)> {
    let outcome = run_continuous_replay(cfg, blocks);
    let status = record_outcome(launcher, req, &outcome)?;
    Ok((outcome, status))
}

fn now_utc() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
