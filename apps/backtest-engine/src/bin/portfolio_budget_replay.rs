//! Portfolio-level budget-capped replay (runtime-parity).
//!
//! Runs the joint martingale kline simulation (`run_kline_screening_with_funding`)
//! for a full portfolio under a `max_global_budget_quote` margin cap AND the
//! per-strategy `max_strategy_budget_quote` caps the live runtime applies, then
//! reports annualized return and max drawdown REBASED TO THE BUDGET principal —
//! because the backtest engine's stock metrics use the uncapped planned-margin
//! capital as the denominator, which is meaningless when a budget cap blocks
//! most legs.
//!
//! The pure logic lives in `backtest_engine::martingale::budget_replay`; this
//! binary is a thin wrapper: parse args, load data, run the sim, call the pure
//! functions, print JSON. Read-only: loads market data, runs the sim, prints a
//! JSON summary. No DB writes, no orders.

use std::{collections::BTreeSet, env, fs, path::PathBuf};

use backtest_engine::{
    market_data::MarketDataSource,
    martingale::{
        budget_replay::{
            build_per_strategy_diagnostics, classify_rejections, evaluate_gate,
            minimum_capital_view, on_budget_metrics, prepare_replay_config, RiskProfile,
        },
        capital::{extract_portfolio_weight_factors, project_portfolio_capital},
        kline_engine::run_kline_screening_with_funding,
    },
    sqlite_market_data::{load_funding_rates_readonly, SqliteMarketDataSource},
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::Value;
use shared_domain::martingale::MartingalePortfolioConfig;

struct Args {
    config_path: PathBuf,
    budget: Decimal,
    start_ms: i64,
    end_ms: i64,
    market_data_path: PathBuf,
    funding_data_path: PathBuf,
    profile: Option<String>,
    portfolio_id: Option<String>,
    exchange_min_notional: f64,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut config_path = None;
        let mut budget = None;
        let mut start_ms = None;
        let mut end_ms = None;
        let mut market_data_path = None;
        let mut funding_data_path = None;
        let mut profile = None;
        let mut portfolio_id = None;
        let mut exchange_min_notional: f64 = DEFAULT_EXCHANGE_MIN_NOTIONAL_IF_UNSET;
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for argument {arg}"))?;
            match arg.as_str() {
                "--config" => config_path = Some(PathBuf::from(value)),
                "--budget" => budget = Some(value.parse::<Decimal>().map_err(|e| format!("budget: {e}"))?),
                "--start-ms" => start_ms = Some(value.parse::<i64>().map_err(|e| format!("start-ms: {e}"))?),
                "--end-ms" => end_ms = Some(value.parse::<i64>().map_err(|e| format!("end-ms: {e}"))?),
                "--market-data" => market_data_path = Some(PathBuf::from(value)),
                "--funding-data" => funding_data_path = Some(PathBuf::from(value)),
                "--profile" => profile = Some(value),
                "--portfolio-id" => portfolio_id = Some(value),
                "--exchange-min-notional" => {
                    exchange_min_notional = value
                        .parse::<f64>()
                        .map_err(|e| format!("exchange-min-notional: {e}"))?;
                }
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
            profile,
            portfolio_id,
            exchange_min_notional,
        })
    }
}

/// Default exchange min notional surfaced in the JSON when the user does not pass
/// `--exchange-min-notional` (5.0 USDT, Binance USD-M futures' effective floor).
const DEFAULT_EXCHANGE_MIN_NOTIONAL_IF_UNSET: f64 = 5.0;

fn required_path(value: Option<PathBuf>, name: &str) -> Result<PathBuf, String> {
    let path = value.ok_or_else(|| format!("{name} is required"))?;
    if !std::path::Path::new(&path).exists() {
        return Err(format!("{name} does not exist: {}", path.display()));
    }
    Ok(path)
}

/// Extract the raw `portfolio_config` JSON value (the sub-object if the file is
/// a search-result wrapper, else the file root). This is what the canonical
/// weight-factor extractor reads.
fn raw_portfolio_config_value(root: &Value) -> Value {
    root.get("portfolio_config")
        .cloned()
        .unwrap_or_else(|| root.clone())
}

fn parse_portfolio_config(value: &Value) -> Result<MartingalePortfolioConfig, String> {
    let portfolio_value = raw_portfolio_config_value(value);
    serde_json::from_value(portfolio_value).map_err(|err| format!("parse portfolio config: {err}"))
}

fn main() -> Result<(), String> {
    let args = Args::parse()?;
    let text = fs::read_to_string(&args.config_path)
        .map_err(|err| format!("read {}: {err}", args.config_path.display()))?;
    let root: Value = serde_json::from_str(&text).map_err(|err| format!("parse config json: {err}"))?;
    let raw_portfolio_value = raw_portfolio_config_value(&root);
    let mut portfolio = parse_portfolio_config(&root)?;

    // ---- Config prep: runtime-parity fix (global cap + per-strategy weight caps). ----
    let prep = prepare_replay_config(&mut portfolio, &raw_portfolio_value, args.budget)?;
    let budget_f = ToPrimitive::to_f64(&args.budget).unwrap_or(0.0);
    let exchange_min_notional = args.exchange_min_notional;

    // ---- Risk profile (explicit arg, else detect from portfolio_id, else conservative). ----
    let profile = match args.profile.as_deref() {
        Some(text) => RiskProfile::parse(text)?,
        None => RiskProfile::detect_from_portfolio_id(
            args.portfolio_id.as_deref().unwrap_or(""),
        ),
    };
    let portfolio_id = args
        .portfolio_id
        .clone()
        .unwrap_or_else(|| format!("replay_{}", profile.as_str()));

    let symbols = portfolio
        .strategies
        .iter()
        .map(|s| s.symbol.trim().to_uppercase())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    eprintln!(
        "replay: {} strategies, {} symbols, budget={}, profile={}, range {}..{} ({} days)",
        portfolio.strategies.len(),
        symbols.len(),
        args.budget,
        profile.as_str(),
        args.start_ms,
        args.end_ms,
        (args.end_ms - args.start_ms) / 86_400_000
    );

    // ---- Load market data. ----
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

    // ---- Run the sim on the runtime-parity config. ----
    let result = run_kline_screening_with_funding(portfolio.clone(), &bars, &funding)?;
    let m = &result.metrics;

    // ---- On-budget metrics (rebased to budget principal, with min-equity hardening). ----
    let initial = result
        .equity_curve
        .first()
        .map(|p| p.equity_quote)
        .unwrap_or(0.0);
    let budget = budget_f.max(1e-9);
    let cum_pnl_series: Vec<f64> = result
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
    let on_budget = on_budget_metrics(budget, &cum_pnl_series, days);

    let final_eq = result.equity_curve.last().map(|p| p.equity_quote).unwrap_or(initial);
    let max_capital_used = m.max_capital_used_quote;
    let on_max_capital_return = if max_capital_used > 0.0 {
        Some((final_eq - initial) / max_capital_used)
    } else {
        None
    };

    // ---- Capital diagnostics via the canonical projection. ----
    let weights_dec = extract_portfolio_weight_factors(&raw_portfolio_value)?;
    let weights_f64: std::collections::HashMap<String, f64> = weights_dec
        .iter()
        .map(|(k, v)| (k.clone(), v.to_f64().unwrap_or(0.0)))
        .collect();
    let proj = project_portfolio_capital(
        &portfolio.strategies,
        &weights_f64,
        budget,
        exchange_min_notional,
        0.0,
        0.0,
    )?;
    let per_strategy = build_per_strategy_diagnostics(&portfolio, &weights_dec, &proj);

    // ---- Minimum-capital feasibility view. ----
    let min_cap = minimum_capital_view(&portfolio.strategies, &weights_dec, exchange_min_notional)?;

    // ---- Rejection breakdown. ----
    let breakdown = classify_rejections(&result.rejection_reasons);

    // ---- Gate. ----
    let gate = evaluate_gate(
        profile,
        on_budget.annualized_return_pct,
        on_budget.max_drawdown_pct,
        on_budget.principal_breached,
        max_capital_used,
        budget,
    );

    // ---- Assemble JSON summary. ----
    let per_strategy_json: Vec<Value> = per_strategy
        .iter()
        .map(|d| {
            serde_json::json!({
                "strategy_id": d.strategy.strategy_id,
                "symbol": d.strategy.symbol,
                "direction": format!("{:?}", d.strategy.direction),
                "weight_pct": d.weight_factor * 100.0,
                "first_leg_margin_quote": d.first_leg_margin_quote,
                "effective_cap_quote": d.effective_cap_quote,
                // Static best-effort projection; NOT a gate.
                "accepted_static_legs": d.accepted_static_legs,
            })
        })
        .collect();

    let summary = serde_json::json!({
        "portfolio_id": portfolio_id,
        "profile": profile.as_str(),
        "budget_quote": budget_f,
        "days": days,
        "strategy_count": portfolio.strategies.len(),
        "symbols": symbols,
        "runtime_weight_caps_applied": prep.runtime_weight_caps_applied,
        "global_margin_cap_quote": budget_f,
        "first_leg_margin_total_quote": proj.first_leg_margin_quote,
        "full_series_margin_quote": proj.full_series_margin_quote,
        "budget_capped_projected_margin_quote": proj.budget_capped_margin_quote,
        "max_capital_used_quote": max_capital_used,
        "on_budget": {
            "total_return_pct": on_budget.total_return_pct,
            "annualized_return_pct": on_budget.annualized_return_pct,
            "max_drawdown_pct": on_budget.max_drawdown_pct,
            "min_equity_quote": on_budget.min_equity_quote,
            "principal_breached": on_budget.principal_breached,
        },
        "rejection_breakdown": {
            "global": breakdown.global,
            "strategy": breakdown.strategy,
            "symbol": breakdown.symbol,
            "direction": breakdown.direction,
            "total": breakdown.total,
        },
        "per_strategy": per_strategy_json,
        "minimum_capital": {
            "exchange_min_notional": exchange_min_notional,
            "exchange_min_notional_is_default": (args.exchange_min_notional
                == DEFAULT_EXCHANGE_MIN_NOTIONAL_IF_UNSET),
            "natural_unscaled_planned_margin_quote": min_cap.natural_unscaled_planned_margin_quote,
            "lp_weighted_planned_margin_quote": min_cap.lp_weighted_planned_margin_quote,
            "min_exact_scaled_executable_principal_quote": min_cap.min_exact_scaled_executable_principal_quote,
            "min_exact_scaled_bottleneck_symbol": min_cap.min_exact_scaled_bottleneck_symbol,
            "min_exact_scaled_bottleneck_strategy_id": min_cap.min_exact_scaled_bottleneck_strategy_id,
            "min_exact_scaled_bottleneck_first_order_quote": min_cap.min_exact_scaled_bottleneck_first_order_quote,
            "scale_to_1000_min_first_order_quote": min_cap.scale_to_1000_min_first_order_quote,
            "scale_model_used_for_gate": min_cap.scale_model_used_for_gate,
        },
        "sim_stock_metrics_on_uncapped_planned_margin_base": {
            "total_return_pct": m.total_return_pct,
            "annualized_return_pct": m.annualized_return_pct,
            "max_drawdown_pct": m.max_drawdown_pct,
            "planned_margin_quote": m.planned_margin_quote,
        },
        "on_max_capital_used": {
            "max_capital_used_quote": max_capital_used,
            "total_return_fraction": on_max_capital_return,
        },
        "trade_count": m.trade_count,
        "stop_count": m.stop_count,
        "total_fee_quote": m.total_fee_quote,
        "total_slippage_quote": m.total_slippage_quote,
        "total_funding_quote": m.total_funding_quote,
        "budget_blocked_legs": breakdown.global + breakdown.symbol + breakdown.direction + breakdown.strategy,
        "total_rejection_reasons": result.rejection_reasons.len(),
        "gate": {
            "profile": gate.profile.as_str(),
            "annualized_threshold": gate.annualized_threshold,
            "drawdown_threshold": gate.drawdown_threshold,
            "passed": gate.passed,
        },
    });
    println!("{}", serde_json::to_string_pretty(&summary).map_err(|e| format!("serialize: {e}"))?);
    Ok(())
}
