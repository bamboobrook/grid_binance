//! Portfolio-level budget-capped replay.
//!
//! Runs the joint martingale kline simulation (`run_kline_screening_with_funding`)
//! for a full portfolio under a `max_global_budget_quote` margin cap, then reports
//! annualized return and max drawdown REBASED TO THE BUDGET principal — because the
//! backtest engine's stock metrics use the uncapped planned-margin capital as the
//! denominator, which is meaningless when a budget cap blocks most legs.
//!
//! Reuses the same wiring as `reprice_martingale_candidates` (sqlite kline + funding
//! loaders, joint sim). Read-only: loads market data, runs the sim, prints a JSON
//! summary. No DB writes, no orders.

use std::{collections::BTreeSet, env, fs, path::PathBuf};

use backtest_engine::{
    market_data::MarketDataSource,
    martingale::kline_engine::run_kline_screening_with_funding,
    sqlite_market_data::{load_funding_rates_readonly, SqliteMarketDataSource},
};
use serde_json::Value;
use shared_domain::martingale::MartingalePortfolioConfig;

struct Args {
    config_path: PathBuf,
    budget: rust_decimal::Decimal,
    start_ms: i64,
    end_ms: i64,
    market_data_path: PathBuf,
    funding_data_path: PathBuf,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut config_path = None;
        let mut budget = None;
        let mut start_ms = None;
        let mut end_ms = None;
        let mut market_data_path = None;
        let mut funding_data_path = None;
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for argument {arg}"))?;
            match arg.as_str() {
                "--config" => config_path = Some(PathBuf::from(value)),
                "--budget" => budget = Some(value.parse::<rust_decimal::Decimal>().map_err(|e| format!("budget: {e}"))?),
                "--start-ms" => start_ms = Some(value.parse::<i64>().map_err(|e| format!("start-ms: {e}"))?),
                "--end-ms" => end_ms = Some(value.parse::<i64>().map_err(|e| format!("end-ms: {e}"))?),
                "--market-data" => market_data_path = Some(PathBuf::from(value)),
                "--funding-data" => funding_data_path = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {arg}")),
            }
        }
        Ok(Self {
            config_path: required_path(config_path, "--config")?,
            budget: budget.ok_or_else(|| "--budget is required".to_string())?,
            start_ms: start_ms.ok_or_else(|| "--start-ms is required".to_string())?,
            end_ms: end_ms.ok_or_else(|| "--end-ms is required".to_string())?,
            market_data_path: required_path(market_data_path, "--market-data")?,
            funding_data_path: required_path(funding_data_path, "--funding-data")?,
        })
    }
}

fn required_path(value: Option<PathBuf>, name: &str) -> Result<PathBuf, String> {
    let path = value.ok_or_else(|| format!("{name} is required"))?;
    if !std::path::Path::new(&path).exists() {
        return Err(format!("{name} does not exist: {}", path.display()));
    }
    Ok(path)
}

fn parse_portfolio_config(value: &Value) -> Result<MartingalePortfolioConfig, String> {
    let portfolio_value = value
        .get("portfolio_config")
        .cloned()
        .unwrap_or_else(|| value.clone());
    serde_json::from_value(portfolio_value).map_err(|err| format!("parse portfolio config: {err}"))
}

fn main() -> Result<(), String> {
    let args = Args::parse()?;
    let text = fs::read_to_string(&args.config_path)
        .map_err(|err| format!("read {}: {err}", args.config_path.display()))?;
    let root: Value = serde_json::from_str(&text).map_err(|err| format!("parse config json: {err}"))?;
    let mut portfolio = parse_portfolio_config(&root)?;

    // Inject the global margin cap.
    portfolio.risk_limits.max_global_budget_quote = Some(args.budget);
    let budget_f = rust_decimal::prelude::ToPrimitive::to_f64(&args.budget).unwrap_or(0.0);

    let symbols = portfolio
        .strategies
        .iter()
        .map(|s| s.symbol.trim().to_uppercase())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    eprintln!(
        "replay: {} strategies, {} symbols, budget={}, range {}..{} ({} days)",
        portfolio.strategies.len(),
        symbols.len(),
        args.budget,
        args.start_ms,
        args.end_ms,
        (args.end_ms - args.start_ms) / 86_400_000
    );

    let market = SqliteMarketDataSource::open_readonly(&args.market_data_path)?;
    let mut bars = Vec::new();
    for symbol in &symbols {
        let loaded = market.load_klines(symbol, args.start_ms, args.end_ms, "1m")?;
        eprintln!("  loaded {symbol}: {} bars", loaded.len());
        bars.extend(loaded);
    }
    bars.sort_by(|l, r| l.open_time_ms.cmp(&r.open_time_ms).then_with(|| l.symbol.cmp(&r.symbol)));
    let funding = load_funding_rates_readonly(&args.funding_data_path, &symbols, args.start_ms, args.end_ms)?;
    eprintln!("  total bars: {}, funding points: {}", bars.len(), funding.len());

    let result = run_kline_screening_with_funding(portfolio, &bars, &funding)?;
    let m = &result.metrics;

    // Equity curve is on the uncapped planned-margin base. Rebase cumulative PnL
    // onto the budget principal for the live-budget view.
    let initial = result
        .equity_curve
        .first()
        .map(|p| p.equity_quote)
        .unwrap_or(0.0);
    let budget = budget_f.max(1e-9);
    let cum_pnl = |eq: f64| eq - initial;
    let days = result
        .equity_curve
        .last()
        .zip(result.equity_curve.first())
        .map(|(l, f)| ((l.timestamp_ms - f.timestamp_ms) as f64) / 86_400_000.0)
        .unwrap_or(0.0);

    // On-budget equity series: budget + cumulative_pnl(t). Peak-to-trough DD.
    let mut budget_peak = budget; // equity starts at budget (cum_pnl=0 at t0)
    let mut max_dd_budget_pct = 0.0_f64;
    for point in &result.equity_curve {
        let eq = budget + cum_pnl(point.equity_quote);
        if eq > budget_peak {
            budget_peak = eq;
        }
        if budget_peak > 0.0 {
            let dd = (budget_peak - eq) / budget_peak * 100.0;
            if dd > max_dd_budget_pct {
                max_dd_budget_pct = dd;
            }
        }
    }
    let final_eq = result.equity_curve.last().map(|p| p.equity_quote).unwrap_or(initial);
    let total_return_budget = cum_pnl(final_eq) / budget; // fraction
    let ann_budget = if days > 0.0 && total_return_budget > -1.0 {
        ((1.0 + total_return_budget).powf(365.0 / days) - 1.0) * 100.0
    } else {
        f64::NEG_INFINITY
    };

    let max_capital_used = m.max_capital_used_quote;
    let on_max_capital_return = if max_capital_used > 0.0 {
        Some(cum_pnl(final_eq) / max_capital_used)
    } else {
        None
    };

    let budget_blocked_legs = result
        .rejection_reasons
        .iter()
        .filter(|r| r.contains("budget exceeded"))
        .count();

    let summary = serde_json::json!({
        "portfolio_id": "mp_margin_v2_lp_conservative_20260626",
        "budget_quote": budget_f,
        "days": days,
        "strategy_count": symbols.len().max(1), // placeholder; real count logged
        "symbols": symbols,
        "on_budget": {
            "total_return_pct": total_return_budget * 100.0,
            "annualized_return_pct": ann_budget,
            "max_drawdown_pct": max_dd_budget_pct,
        },
        "on_max_capital_used": {
            "max_capital_used_quote": max_capital_used,
            "total_return_fraction": on_max_capital_return,
        },
        "sim_stock_metrics_on_uncapped_planned_margin_base": {
            "total_return_pct": m.total_return_pct,
            "annualized_return_pct": m.annualized_return_pct,
            "max_drawdown_pct": m.max_drawdown_pct,
            "planned_margin_quote": m.planned_margin_quote,
        },
        "trade_count": m.trade_count,
        "stop_count": m.stop_count,
        "total_fee_quote": m.total_fee_quote,
        "total_slippage_quote": m.total_slippage_quote,
        "total_funding_quote": m.total_funding_quote,
        "budget_blocked_legs": budget_blocked_legs,
        "total_rejection_reasons": result.rejection_reasons.len(),
        "gate_pass": (ann_budget > 50.0 && max_dd_budget_pct <= 10.0),
    });
    println!("{}", serde_json::to_string_pretty(&summary).map_err(|e| format!("serialize: {e}"))?);
    Ok(())
}
