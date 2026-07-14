//! Round 14 P2.3: HTF regime search using BatchReplay.
//!
//! Preloads data once, then runs multiple configs with HTF gate on/off.
//! Much faster than CLI subprocess for each config.

use std::env;
use std::fs;
use std::time::Instant;

use backtest_engine::martingale::batch_replay::BatchReplay;
use backtest_engine::martingale::budget_replay::{on_budget_metrics, prepare_replay_config};
use backtest_engine::martingale::kline_engine::run_kline_screening_with_funding;
use rust_decimal::Decimal;
use serde_json::Value;

const DEV_START: i64 = 1_672_531_200_000;
const DEV_END: i64 = 1_780_271_999_999;

const SEGMENTS: &[(i64, i64)] = &[
    (1_672_531_200_000, 1_688_169_599_999), // H1-2023
    (1_688_169_600_000, 1_704_067_199_999), // H2-2023
    (1_704_067_200_000, 1_735_689_599_999), // 2024
    (1_735_689_600_000, 1_767_225_599_999), // 2025
    (1_767_225_600_000, 1_780_271_999_999), // 2026-YTD
];

const BUDGETS: &[f64] = &[1000.0, 2000.0, 3000.0, 4000.0, 4999.0];

struct MetricsResult {
    ann: f64,
    dd: f64,
    trades: u64,
    blocked: u64,
    max_capital: f64,
}

fn run_with_on_budget(
    batch: &BatchReplay,
    config: &shared_domain::martingale::MartingalePortfolioConfig,
    budget: f64,
    start_ms: i64,
    end_ms: i64,
) -> Result<MetricsResult, String> {
    let result = batch.run_single_full(config, budget, start_ms, end_ms)?;
    let initial = result
        .equity_curve
        .first()
        .map(|p| p.equity_quote)
        .unwrap_or(0.0);
    let cum_pnl: Vec<f64> = result
        .equity_curve
        .iter()
        .map(|p| p.equity_quote - initial)
        .collect();
    let days = result
        .equity_curve
        .last()
        .zip(result.equity_curve.first())
        .map(|(l, f)| ((l.timestamp_ms - f.timestamp_ms) as f64) / 86_400_000.0)
        .unwrap_or(0.0);
    let ob = on_budget_metrics(budget, &cum_pnl, days);
    Ok(MetricsResult {
        ann: ob.annualized_return_pct,
        dd: ob.max_drawdown_pct,
        trades: result.metrics.trade_count,
        blocked: result.rejection_reasons.len() as u64,
        max_capital: result.metrics.max_capital_used_quote,
    })
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let base_config_path = args
        .get(1)
        .ok_or("usage: r14_htf_search <base_config.json> <label>")?;
    let label = args.get(2).cloned().unwrap_or_else(|| "search".to_string());
    let htf_on = args
        .get(3)
        .map(|s| s == "on")
        .unwrap_or(true);

    let raw = fs::read_to_string(base_config_path)
        .map_err(|e| format!("read config: {e}"))?;
    let raw_json: Value = serde_json::from_str(&raw).map_err(|e| format!("parse json: {e}"))?;
    let portfolio_config = raw_json
        .get("portfolio_config")
        .ok_or("missing portfolio_config")?
        .clone();

    // Deserialize into typed config
    let mut portfolio: shared_domain::martingale::MartingalePortfolioConfig =
        serde_json::from_value(portfolio_config.clone()).map_err(|e| format!("deserialize: {e}"))?;

    // Apply the same budget preparation as the CLI (weight caps, etc.)
    let budget_decimal = Decimal::new(4999, 0);
    prepare_replay_config(&mut portfolio, &portfolio_config, budget_decimal)?;

    // Collect symbols (traded + dependencies)
    use backtest_engine::martingale::indicator_runtime::extract_symbol_dependencies;
    let traded: Vec<String> = portfolio
        .strategies
        .iter()
        .map(|s| s.symbol.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let deps = extract_symbol_dependencies(&portfolio);
    let symbols: Vec<String> = traded
        .iter()
        .cloned()
        .chain(deps.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    // Apply HTF gate
    if htf_on {
        for strategy in &mut portfolio.strategies {
            strategy.risk_limits.htf_regime_gate_enabled = Some(true);
        }
    }

    // Apply XS selector gate if config is provided via env vars.
    // R14_XS_FAMILY=momentum|reversal
    // R14_XS_LOOKBACK=14
    // R14_XS_SKIP_RECENT=1
    // R14_XS_REBALANCE_DAYS=7
    // R14_XS_ACTIVE_LONG=3
    // R14_XS_ACTIVE_SHORT=3
    if let Ok(xs_family) = std::env::var("R14_XS_FAMILY") {
        let xs_config = shared_domain::martingale::MartingaleXsSelectorConfig {
            family: xs_family,
            lookback_periods: std::env::var("R14_XS_LOOKBACK")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(14),
            skip_recent_periods: std::env::var("R14_XS_SKIP_RECENT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
            rebalance_period_bars: std::env::var("R14_XS_REBALANCE_DAYS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .map(|d| d * 24 * 60)
                .unwrap_or(7 * 24 * 60),
            active_long_count: std::env::var("R14_XS_ACTIVE_LONG")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            active_short_count: std::env::var("R14_XS_ACTIVE_SHORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            symbol_cap: 0.25,
            cluster_cap: 0.35,
            min_active_symbols: 3,
        };
        eprintln!("XS selector: {:?}", xs_config);
        for strategy in &mut portfolio.strategies {
            strategy.risk_limits.xs_selector_gate_enabled = Some(true);
            strategy.risk_limits.xs_selector_config = Some(xs_config.clone());
        }
    }

    // Apply dual-state ladder if enabled via env var.
    // R14_DUAL_STATE=1 enables the ladder.
    // R14_DUAL_SO_SCALE=0.5 sets SO scale for adverse trend.
    // R14_DUAL_SPACING_MULT=1.5 sets spacing multiplier for adverse trend.
    // Note: dual-state ladder requires HTF regime to be active (htf_on=true).
    if std::env::var("R14_DUAL_STATE").ok().as_deref() == Some("1") {
        let so_scale = std::env::var("R14_DUAL_SO_SCALE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.5);
        let spacing_mult = std::env::var("R14_DUAL_SPACING_MULT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.5);
        eprintln!("Dual-state ladder: so_scale={}, spacing_mult={}", so_scale, spacing_mult);
        for strategy in &mut portfolio.strategies {
            strategy.risk_limits.dual_state_ladder_enabled = Some(true);
            strategy.risk_limits.dual_state_so_scale = Some(so_scale);
            strategy.risk_limits.dual_state_spacing_mult = Some(spacing_mult);
        }
    }

    // Apply P5 inventory scheduler if enabled.
    // R14_INV_SCHED=1 enables the scheduler.
    // R14_INV_PENALTY=0.5 sets inventory penalty factor.
    // R14_RISK_FLOOR=0.5 sets risk scale floor.
    if std::env::var("R14_INV_SCHED").ok().as_deref() == Some("1") {
        let inv_penalty = std::env::var("R14_INV_PENALTY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.5);
        let risk_floor = std::env::var("R14_RISK_FLOOR")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.5);
        eprintln!("Inventory scheduler: penalty={}, risk_floor={}", inv_penalty, risk_floor);
        for strategy in &mut portfolio.strategies {
            strategy.risk_limits.inventory_scheduler_enabled = Some(true);
            strategy.risk_limits.inventory_penalty = Some(inv_penalty);
            strategy.risk_limits.risk_scale_floor = Some(risk_floor);
        }
    }

    eprintln!("Label: {} HTF={} Symbols={:?}", label, htf_on, symbols);

    // Preload data
    let t0 = Instant::now();
    let mut batch = BatchReplay::new("data/market_data_full.db", "data/funding_rates_round12.db")?;
    let symbol_refs: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
    batch.preload_symbols(&symbol_refs, DEV_START, DEV_END)?;
    eprintln!("Preload: {:.1}s", t0.elapsed().as_secs_f64());

    let mut result = serde_json::json!({
        "label": label,
        "htf_on": htf_on,
        "symbols": symbols,
    });

    // Full window (on-budget metrics)
    let t0 = Instant::now();
    let full = run_with_on_budget(&batch, &portfolio, 4999.0, DEV_START, DEV_END)?;
    eprintln!(
        "Full: ann={:.2}% DD={:.2}% trades={} blocked={} [{:.1}s]",
        full.ann,
        full.dd,
        full.trades,
        full.blocked,
        t0.elapsed().as_secs_f64()
    );
    result["full"] = serde_json::json!({
        "ann": full.ann,
        "dd": full.dd,
        "trades": full.trades,
        "blocked": full.blocked,
        "max_capital": full.max_capital,
    });

    // Segments
    let mut seg_results = Vec::new();
    for (i, (start, end)) in SEGMENTS.iter().enumerate() {
        let seg = run_with_on_budget(&batch, &portfolio, 4999.0, *start, *end)?;
        eprintln!("  Seg {}: ann={:.2}% DD={:.2}%", i, seg.ann, seg.dd);
        seg_results.push(serde_json::json!({
            "ann": seg.ann,
            "dd": seg.dd,
            "positive": seg.ann > 0.0,
        }));
    }
    let pos_count = seg_results
        .iter()
        .filter(|s| s["positive"].as_bool().unwrap_or(false))
        .count();
    eprintln!("Positive segments: {}/5", pos_count);
    result["segments"] = serde_json::Value::Array(seg_results);
    result["positive_segments"] = serde_json::json!(pos_count);

    // Budgets
    let mut bud_results = Vec::new();
    for &budget in BUDGETS {
        let bud = run_with_on_budget(&batch, &portfolio, budget, DEV_START, DEV_END)?;
        eprintln!("  Budget {}U: ann={:.2}%", budget, bud.ann);
        bud_results.push(serde_json::json!({
            "budget": budget,
            "ann": bud.ann,
            "dd": bud.dd,
        }));
    }
    result["budgets"] = serde_json::Value::Array(bud_results);

    // Output JSON
    println!("{}", serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?);

    Ok(())
}
