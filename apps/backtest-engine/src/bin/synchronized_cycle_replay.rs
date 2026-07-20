//! Round 18 R3.1: synchronized residual Martingale cycle replay binary.
//!
//! Same data-loading pattern as `portfolio_budget_replay` (read-only SQLite,
//! merged sorted 1m bars, funding coverage check) but runs the NEW
//! `run_synchronized_cycle_replay` engine for M1 pair / M2 basket residual
//! cycles (plan §7/§8). Backtest and live share the SAME effective config +
//! completed-bar boundary + cycle state + order/rejection hash (plan §1 hard
//! gate 6).
//!
//! Config JSON shape (the file the engine + CLI both consume):
//! ```json
//! {
//!   "synchronized_cycle": { ...SynchronizedCycleConfig fields... },
//!   "fits": [ { "group_id": ..., "legs": [...], "leg_direction_signs": [...],
//!               "betas": [...], "mus": [...], "residual_sigma": ...,
//!               "half_life_h": ..., "fit_sha256": ... }, ... ],
//!   "budget_quote": <number, optional; else from --budget>
//! }
//! ```
//! `fits` are produced by train-only cointegration/residual fit (R4) and are
//! FROZEN at validation commit time; the engine never re-fits.

use std::{fs, path::PathBuf};

use backtest_engine::{
    market_data::MarketDataSource,
    martingale::sync_cycle_engine::{run_synchronized_cycle_replay, SynchronizedFit},
    sqlite_market_data::{load_funding_rates_readonly, SqliteMarketDataSource},
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use shared_domain::martingale::SynchronizedCycleConfig;

#[derive(Debug, Deserialize)]
struct SyncReplayConfig {
    synchronized_cycle: SynchronizedCycleConfig,
    fits: Vec<SynchronizedFit>,
    #[serde(default)]
    budget_quote: Option<f64>,
}

struct Args {
    config_path: PathBuf,
    budget: Decimal,
    start_ms: i64,
    end_ms: i64,
    market_data_path: PathBuf,
    funding_data_path: PathBuf,
    fee_override_bps: f64,
    slippage_override_bps: f64,
}

fn parse_args() -> Result<Args, String> {
    let mut config_path: Option<PathBuf> = None;
    let mut budget = Decimal::ZERO;
    let mut start_ms: i64 = 0;
    let mut end_ms: i64 = 0;
    let mut market_data_path: Option<PathBuf> = None;
    let mut funding_data_path: Option<PathBuf> = None;
    let mut fee_override_bps = 0.0f64;
    let mut slippage_override_bps = 0.0f64;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => config_path = Some(PathBuf::from(args.next().ok_or("--config <path>")?)),
            "--budget" => {
                let s = args.next().ok_or("--budget <decimal>")?;
                budget = s.parse::<Decimal>().map_err(|e| format!("budget: {e}"))?;
            }
            "--start-ms" => {
                let s = args.next().ok_or("--start-ms <i64>")?;
                start_ms = s.parse().map_err(|e| format!("start-ms: {e}"))?;
            }
            "--end-ms" => {
                let s = args.next().ok_or("--end-ms <i64>")?;
                end_ms = s.parse().map_err(|e| format!("end-ms: {e}"))?;
            }
            "--market-data" => {
                market_data_path = Some(PathBuf::from(args.next().ok_or("--market-data <path>")?))
            }
            "--funding-data" => {
                funding_data_path = Some(PathBuf::from(args.next().ok_or("--funding-data <path>")?))
            }
            "--fee-override-bps" => {
                let s = args.next().ok_or("--fee-override-bps <f64>")?;
                fee_override_bps = s.parse().map_err(|e| format!("fee-override-bps: {e}"))?;
            }
            "--slippage-override-bps" => {
                let s = args.next().ok_or("--slippage-override-bps <f64>")?;
                slippage_override_bps = s.parse().map_err(|e| format!("slippage-override-bps: {e}"))?;
            }
            _ => return Err(format!("unknown arg: {arg}")),
        }
    }
    Ok(Args {
        config_path: config_path.ok_or("--config <path> required")?,
        budget,
        start_ms,
        end_ms,
        market_data_path: market_data_path.ok_or("--market-data <path> required")?,
        funding_data_path: funding_data_path.ok_or("--funding-data <path> required")?,
        fee_override_bps,
        slippage_override_bps,
    })
}

fn main() -> Result<(), String> {
    let args = parse_args()?;

    if args.fee_override_bps > 0.0 {
        backtest_engine::martingale::kline_engine::set_fee_bps_override(args.fee_override_bps);
    }
    if args.slippage_override_bps > 0.0 {
        backtest_engine::martingale::kline_engine::set_slippage_bps_override(args.slippage_override_bps);
    }

    let text = fs::read_to_string(&args.config_path)
        .map_err(|err| format!("read {}: {err}", args.config_path.display()))?;
    let cfg: SyncReplayConfig =
        serde_json::from_str(&text).map_err(|err| format!("parse config json: {err}"))?;

    let budget_f = cfg.budget_quote.unwrap_or_else(|| {
        ToPrimitive::to_f64(&args.budget).unwrap_or(4999.0)
    });

    // Build the union of all leg symbols across all groups (for data loading).
    // Round 21 R4.3 (plan §6.2): B1S needs spot AND perp as DISTINCT series
    // for the same symbol. The legacy loader deduped [BTCUSDT, BTCUSDT] to 1
    // series — that was the R20 audit's B1 blocking finding. Now when a fit
    // carries leg_markets, we load each (symbol, market_type) pair separately
    // and encode the market_type into the bar's symbol string as
    // "{SYMBOL}::{market_type}" so the engine's latest_close map keeps both.
    let family = cfg.synchronized_cycle.family.clone();
    let is_b1s = family == "B1S";
    let mut load_keys: std::collections::BTreeSet<(String, String)> =
        std::collections::BTreeSet::new();
    for fit in &cfg.fits {
        if !fit.leg_markets.is_empty() && fit.leg_markets.len() == fit.legs.len() {
            // Use the explicit market identity per leg.
            for m in &fit.leg_markets {
                load_keys.insert((m.market_type.clone(), m.symbol.trim().to_uppercase()));
            }
        } else {
            // Legacy: load as perp (the engine's default market_type).
            for leg in &fit.legs {
                load_keys.insert(("futures_usdt_perp".to_string(),
                                  leg.trim().to_uppercase()));
            }
        }
    }
    // For M2 baskets, the factor symbol (BTCUSDT) must also be loaded even if
    // it is not itself a traded leg (it feeds residual computation).
    if cfg.synchronized_cycle.factor.as_deref() == Some("BTC") {
        load_keys.insert(("futures_usdt_perp".to_string(), "BTCUSDT".to_string()));
    }
    let load_keys: Vec<(String, String)> = load_keys.into_iter().collect();
    let symbols: Vec<String> = load_keys.iter().map(|(_, s)| s.clone()).collect();
    eprintln!(
        "sync_replay: family={}, {} groups, {} (symbol,market_type) load keys, budget={}, range {}..{} ({} days)",
        cfg.synchronized_cycle.family,
        cfg.fits.len(),
        load_keys.len(),
        budget_f,
        args.start_ms,
        args.end_ms,
        (args.end_ms - args.start_ms) / 86_400_000
    );

    // Load market data (read-only). For B1S / explicit leg_markets, encode the
    // market_type into the bar symbol so the engine can distinguish spot vs perp.
    let market = SqliteMarketDataSource::open_readonly(&args.market_data_path)?;
    let mut bars = Vec::new();
    for (market_type, symbol) in &load_keys {
        let loaded = market.load_klines_with_market_type(
            symbol, market_type, args.start_ms, args.end_ms, "1m")?;
        eprintln!("  loaded {symbol} ({market_type}): {} bars", loaded.len());
        // Encode market_type into symbol for B1S so latest_close keeps both
        if is_b1s || !load_keys.iter().all(|(mt, _)| mt == "futures_usdt_perp") {
            for mut b in loaded {
                b.symbol = format!("{symbol}::{market_type}");
                bars.push(b);
            }
        } else {
            bars.extend(loaded);
        }
    }
    bars.sort_by(|l, r| {
        l.open_time_ms
            .cmp(&r.open_time_ms)
            .then_with(|| l.symbol.cmp(&r.symbol))
    });
    let funding = load_funding_rates_readonly(&args.funding_data_path, &symbols, args.start_ms, args.end_ms)?;
    eprintln!("  total bars: {}, funding points: {}", bars.len(), funding.len());

    // Run the synchronized-cycle engine.
    let result = run_synchronized_cycle_replay(
        &cfg.synchronized_cycle,
        &cfg.fits,
        &bars,
        &funding,
        budget_f,
    )?;

    // Emit a JSON summary including the SYNC_SUMMARY stashed in rejection_reasons.
    let sync_summary = result
        .rejection_reasons
        .iter()
        .find_map(|r| r.strip_prefix("SYNC_SUMMARY:").and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()))
        .unwrap_or_else(|| serde_json::json!({}));

    // Resolved config sha256 (for fingerprint dedup).
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    let resolved_config_sha256 = format!("{:x}", h.finalize());

    let summary = serde_json::json!({
        "family": cfg.synchronized_cycle.family,
        "budget_quote": budget_f,
        "start_ms": args.start_ms,
        "end_ms": args.end_ms,
        "days": (args.end_ms - args.start_ms) / 86_400_000,
        "symbols": symbols,
        "resolved_config_sha256": resolved_config_sha256,
        "metrics": {
            "total_return_pct": result.metrics.total_return_pct,
            "annualized_return_pct": result.metrics.annualized_return_pct,
            "max_drawdown_pct": result.metrics.max_drawdown_pct,
            "trade_count": result.metrics.trade_count,
            "total_fee_quote": result.metrics.total_fee_quote,
            "total_slippage_quote": result.metrics.total_slippage_quote,
            "total_funding_quote": result.metrics.total_funding_quote,
            "min_equity_quote": result.equity_curve.iter().map(|p| p.equity_quote).fold(budget_f, f64::min),
            "breach": result.equity_curve.iter().map(|p| p.equity_quote).fold(false, |a, e| a || (e <= 0.0)),
        },
        "event_count": result.events.len(),
        "sync_summary": sync_summary,
    });
    println!("{}", serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?);
    Ok(())
}
