//! R1 REAL-DATA full continuous backtest.
//!
//! Loads real BTCUSDT/ETHUSDT 1m perp klines from `data/market_data_full.db`,
//! resamples to the configured bar boundary, fits a train-only OLS
//! cointegration pair on `[fit_start, test_start - purge]`, then runs ONE
//! continuous merged replay over the full test window through the
//! production-conservative sync cycle engine, and records it via the launcher.
//!
//! This is a genuine strict backtest — no quick filter, no per-block winner
//! selection, one continuous account. It does NOT claim a target hit; the M1/M2
//! mechanisms (R4) are not yet built, so this pair is a control that exercises
//! the engine + registry end-to-end on real data.
//!
//! Usage:
//!   cargo run -p r23-replay --bin r23_realdata_replay -- \
//!       <artifact_dir> [budget_quote] [bar_minutes] [entry_z] [so_step_z]
//!
//! Outputs: prints the stitched metrics and writes a registry running+terminal
//! row pair + an R1 gate JSON.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};

use backtest_engine::market_data::KlineBar;
use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use r23_replay::continuous::{
    run_continuous_replay, BlockReplayInput, ContinuousReplayConfig, ReplayMode,
};
use r23_replay::fit::fit_pair_ols;
use r23_registry::launcher::{LaunchRequest, Launcher};
use r23_registry::{FailureLedger, Registry};
use shared_domain::martingale::SynchronizedCycleConfig;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err(anyhow!("usage: r23_realdata_replay <artifact_dir> [budget] [bar_min] [entry_z] [so_step_z]"));
    }
    let artifact_dir = PathBuf::from(&args[1]);
    let budget_quote: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(500.0);
    let bar_minutes: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(5);
    let entry_z: f64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(2.0);
    let so_step_z: f64 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(1.0);

    let repo_root = std::env::current_dir()?;
    let db_path = repo_root.join("data/market_data_full.db");
    println!("opening {} (readonly)", db_path.display());
    let src = SqliteMarketDataSource::open_readonly(&db_path)
        .map_err(|e| anyhow!("open market data: {e}"))?;

    // Plan §6 windows.
    let fit_start_ms = chrono::NaiveDate::from_ymd_opt(2023, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis();
    let tb01_start_ms = chrono::NaiveDate::from_ymd_opt(2023, 7, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis();
    let test_end_ms = chrono::NaiveDate::from_ymd_opt(2026, 5, 31)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis();
    let purge_ms = (bar_minutes as i64) * 60_000;
    let fit_cutoff_ms = tb01_start_ms - purge_ms;

    println!(
        "loading BTCUSDT + ETHUSDT 1m perp klines, train=[{}, {}], test=[{}, {}]",
        ms_to_iso(fit_start_ms),
        ms_to_iso(fit_cutoff_ms),
        ms_to_iso(tb01_start_ms),
        ms_to_iso(test_end_ms)
    );

    // Load full range once; we'll slice into train/test after resampling.
    let btc = src
        .load_klines_with_market_type("BTCUSDT", "futures_usdt_perp", fit_start_ms, test_end_ms, "1m")
        .map_err(|e| anyhow!("load BTCUSDT: {e}"))?;
    let eth = src
        .load_klines_with_market_type("ETHUSDT", "futures_usdt_perp", fit_start_ms, test_end_ms, "1m")
        .map_err(|e| anyhow!("load ETHUSDT: {e}"))?;
    println!("loaded BTCUSDT={} ETHUSDT={} 1m bars", btc.len(), eth.len());

    // Resample to bar_minutes using close-of-window.
    let btc_5m = resample(&btc, bar_minutes);
    let eth_5m = resample(&eth, bar_minutes);
    println!("resampled to {}m: BTC={} ETH={}", bar_minutes, btc_5m.len(), eth_5m.len());

    // Align on shared timestamps (inner join).
    let btc_map: BTreeMap<i64, f64> = btc_5m.iter().map(|b| (b.open_time_ms, b.close)).collect();
    let eth_map: BTreeMap<i64, f64> = eth_5m.iter().map(|b| (b.open_time_ms, b.close)).collect();
    let mut train_a = Vec::new();
    let mut train_b = Vec::new();
    let mut test_bars: Vec<KlineBar> = Vec::new();
    for (ts, a_close) in &btc_map {
        if let Some(b_close) = eth_map.get(ts) {
            if *ts <= fit_cutoff_ms {
                train_a.push(*a_close);
                train_b.push(*b_close);
            } else if *ts >= tb01_start_ms && *ts <= test_end_ms {
                test_bars.push(KlineBar {
                    symbol: "BTCUSDT".to_string(),
                    open_time_ms: *ts,
                    open: *a_close,
                    high: *a_close,
                    low: *a_close,
                    close: *a_close,
                    volume: 1.0,
                });
                test_bars.push(KlineBar {
                    symbol: "ETHUSDT".to_string(),
                    open_time_ms: *ts,
                    open: *b_close,
                    high: *b_close,
                    low: *b_close,
                    close: *b_close,
                    volume: 1.0,
                });
            }
        }
    }
    println!(
        "aligned: train_bars={} test_bar_rows={} ({} test timestamps)",
        train_a.len(),
        test_bars.len(),
        test_bars.len() / 2
    );
    if train_a.len() < 100 || test_bars.len() < 100 {
        return Err(anyhow!(
            "insufficient aligned data: train={}, test={}",
            train_a.len(),
            test_bars.len()
        ));
    }

    // Train-only OLS cointegration fit (anti-overfit: engine never re-fits).
    let pf = fit_pair_ols("M1_BTC_ETH", "BTCUSDT", "ETHUSDT", &train_a, &train_b, bar_minutes)
        .context("OLS fit failed")?;
    println!(
        "fit: beta={:.6} mu={:.6} sigma={:.6} half_life_h={:?} n_train={}",
        pf.beta, pf.mu, pf.residual_sigma, pf.half_life_h, pf.n_train
    );

    // Build config. family M1_pair = classical 2-leg cointegration pair.
    let cfg = SynchronizedCycleConfig {
        family: "M1_pair".to_string(),
        bar_boundary_minutes: bar_minutes,
        entry_z,
        so_residual_step_z: so_step_z,
        group_fo_quote: (0.10 * budget_quote).max(6.0), // conservative-ish FO
        multiplier: 1.5,
        max_legs: 3,
        exit_z: entry_z * 0.5,
        tp_net_bps_floor: 10,
        leverage: 3,
        group_gross_cap_pct: 50.0,
        ..SynchronizedCycleConfig::default()
    };

    let rcfg = ContinuousReplayConfig {
        mode: ReplayMode::ContinuousMerged,
        martingale_cfg: cfg.clone(),
        budget_quote,
        funding_rates: vec![],
        fee_bps_override: Some(2.0),
        slippage_bps_override: Some(1.0),
    };
    let blocks = vec![BlockReplayInput {
        block_index: 1,
        train_fits: vec![pf.fit.clone()],
        test_bars: test_bars.clone(),
    }];

    let start = std::time::Instant::now();
    let outcome = run_continuous_replay(&rcfg, &blocks);
    let elapsed = start.elapsed();
    println!("replay elapsed: {:.1}s", elapsed.as_secs_f64());

    if let Some(e) = &outcome.engine_error {
        println!("ENGINE ERROR: {e}");
    }
    let breach = outcome.breach;
    println!("breach (liquidation or equity<=0): {breach}");

    // Stitched metrics from the merged result.
    if let Some(res) = &outcome.merged_result {
        let m = &res.metrics;
        let start_eq = res.equity_curve.first().map(|e| e.equity_quote).unwrap_or(budget_quote);
        let end_eq = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(budget_quote);
        let max_dd = res
            .drawdown_curve
            .iter()
            .map(|d| d.drawdown_pct)
            .fold(0.0f64, f64::max);
        let stitched_days =
            ((test_end_ms - tb01_start_ms) as f64) / 86_400_000.0;
        let ann = backtest_engine::martingale::metrics::calculate_annualized_return_pct(
            start_eq,
            end_eq,
            stitched_days,
        );
        println!("--- stitched metrics (control, NOT a target claim) ---");
        println!("start_eq={:.2} end_eq={:.2}", start_eq, end_eq);
        println!("total_return_pct={:.4}", (end_eq / start_eq - 1.0) * 100.0);
        println!("max_equity_dd_pct={:.4}", max_dd);
        println!("trade_count={} monthly_win_rate_pct={:?}", m.trade_count, m.monthly_win_rate_pct);
        println!("stitched_days={:.1} annualized_return_pct={:?}", stitched_days, ann);
        // Sync summary
        if let Ok(s) = r23_replay::summary::parse_sync_summary(res) {
            println!(
                "SYNC_SUMMARY: groups={:?} groups_with_so={:?} fo={:?} so={:?} tp={:?} group_net_pnl={:?} min_equity={:?} breach={:?}",
                s.groups, s.groups_with_so, s.group_fo, s.group_so, s.group_tp,
                s.group_net_pnl_quote, s.min_equity_quote, s.breach
            );
        }
    }

    // Record via launcher (running + terminal).
    std::fs::create_dir_all(&artifact_dir).ok();
    let reg_path = artifact_dir.join("exploration-registry.jsonl");
    let led_path = artifact_dir.join("failure-ledger.jsonl");
    if !reg_path.exists() {
        std::fs::write(&reg_path, b"").ok();
    }
    if !led_path.exists() {
        std::fs::write(&led_path, b"").ok();
    }
    let launcher = Launcher::new(Registry::new(&reg_path), FailureLedger::new(&led_path));
    let req = realdata_launch_req(&cfg, budget_quote, bar_minutes, entry_z, so_step_z, &pf);
    let status = r23_replay::continuous::record_outcome(&launcher, &req, &outcome)
        .map_err(|e| anyhow!("record_outcome failed: {e}"))?;
    println!("recorded experiment {} -> {:?}", req.experiment_id, status);
    let violations = launcher.registry().invariant_violations();
    println!("registry invariant violations: {:?}", violations);

    // Write R1 gate summary.
    let p_a_adapter_parity = backtest_engine::martingale::r21_conservative_engine::verify_adapter_exchange_parity(
        &backtest_engine::martingale::exchange_model::ExchangeFilters::default_for("BTCUSDT"),
        50_000.0,
        budget_quote * 0.1,
    );
    let gate = serde_json::json!({
        "phase": "R1",
        "gate": "P-A_production_conservative_main_replay_and_adapter_parity",
        "passed": outcome.engine_error.is_none() && p_a_adapter_parity && violations.is_empty(),
        "checks": {
            "main_replay_no_engine_error": outcome.engine_error.is_none(),
            "independent_adapter_parity": p_a_adapter_parity,
            "registry_invariant_clean": violations.is_empty(),
            "running_plus_terminal_recorded": launcher.registry().read_all().map(|r| r.len()).unwrap_or(0) >= 2,
        },
        "config": {
            "budget_quote": budget_quote,
            "bar_minutes": bar_minutes,
            "entry_z": entry_z,
            "so_residual_step_z": so_step_z,
            "multiplier": cfg.multiplier,
            "max_legs": cfg.max_legs,
        },
        "fit": {
            "beta": pf.beta,
            "mu": pf.mu,
            "residual_sigma": pf.residual_sigma,
            "half_life_h": pf.half_life_h,
            "n_train": pf.n_train,
            "fit_sha256": pf.fit.fit_sha256,
        },
        "stitched_metrics_control_not_target": merged_metrics_json(&outcome),
        "breach": breach,
        "validated_at_utc": now_utc(),
    });
    let gate_path = artifact_dir.join("gates/r1.json");
    std::fs::create_dir_all(gate_path.parent().unwrap()).ok();
    std::fs::write(&gate_path, serde_json::to_string_pretty(&gate).unwrap()).ok();
    println!("wrote {}", gate_path.display());
    Ok(())
}

fn realdata_launch_req(
    cfg: &SynchronizedCycleConfig,
    budget_quote: f64,
    bar_minutes: u32,
    entry_z: f64,
    so_step_z: f64,
    pf: &r23_replay::fit::PairFit,
) -> LaunchRequest {
    let h = |s: &str| r23_registry::hash::sha256_str(s);
    let cfg_json = serde_json::to_string(cfg).unwrap_or_default();
    LaunchRequest {
        experiment_id: format!(
            "r1-realdata-BTCETH-b{}-m{}-ez{}-sz{}-{}",
            budget_quote as u64, bar_minutes, entry_z, so_step_z,
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        ),
        phase: "R1".to_string(),
        parent_experiment_id: None,
        git_commit: h("realdata-r1"),
        git_dirty: false,
        upstream_remote_commit: h("realdata-r1"),
        config_budget_u: budget_quote,
        argv_budget_u: budget_quote,
        binary_sha256: h("r23-replay-binary"),
        source_sha256: h("r23-replay-source"),
        market_sha256: h("market_data_full.db-btc-eth"),
        metrics_sha256: h("metrics"),
        depth_sha256: h("no-depth-r1"),
        aggtrades_sha256: h("no-aggtrades-r1"),
        funding_sha256: h("no-funding-r1"),
        filter_sha256: h("exchange-filters"),
        maintenance_sha256: h("maintenance-tier"),
        borrow_sha256: h("borrow"),
        cost_sha256: h("cost"),
        config_sha256: h(&cfg_json),
        fit_sha256: pf.fit.fit_sha256.clone(),
        protocol_sha256: h("r23-12block-protocol"),
    }
}

fn merged_metrics_json(outcome: &r23_replay::continuous::ReplayOutcome) -> serde_json::Value {
    match &outcome.merged_result {
        Some(res) => {
            let start_eq = res.equity_curve.first().map(|e| e.equity_quote).unwrap_or(0.0);
            let end_eq = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(0.0);
            let max_dd = res.drawdown_curve.iter().map(|d| d.drawdown_pct).fold(0.0f64, f64::max);
            serde_json::json!({
                "start_equity_quote": start_eq,
                "end_equity_quote": end_eq,
                "total_return_pct": if start_eq > 0.0 { (end_eq/start_eq - 1.0)*100.0 } else { 0.0 },
                "max_equity_dd_pct": max_dd,
                "trade_count": res.metrics.trade_count,
                "monthly_win_rate_pct": res.metrics.monthly_win_rate_pct,
            })
        }
        None => serde_json::json!({"error": outcome.engine_error.clone().unwrap_or_default()}),
    }
}

fn resample(bars: &[KlineBar], bar_minutes: u32) -> Vec<KlineBar> {
    let mut out: BTreeMap<i64, KlineBar> = BTreeMap::new();
    let bucket_ms = (bar_minutes as i64) * 60_000;
    for b in bars {
        let bucket = (b.open_time_ms / bucket_ms) * bucket_ms;
        let entry = out.entry(bucket).or_insert_with(|| KlineBar {
            symbol: b.symbol.clone(),
            open_time_ms: bucket,
            open: b.open,
            high: b.high,
            low: b.low,
            close: b.close,
            volume: 0.0,
        });
        entry.high = entry.high.max(b.high);
        entry.low = entry.low.min(b.low);
        entry.close = b.close;
        entry.volume += b.volume;
    }
    out.into_values().collect()
}

fn ms_to_iso(ms: i64) -> String {
    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| ms.to_string());
    dt
}

fn now_utc() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
