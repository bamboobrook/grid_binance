//! Small-capital-native martingale search.
//!
//! Read-only exploratory tool for finding executable martingale candidates under
//! <= 5000 USDT margin budgets. It deliberately avoids DB writes and live state.

use std::{collections::BTreeMap, env, fs, path::PathBuf, time::Instant};

use backtest_engine::{
    market_data::MarketDataSource,
    martingale::{
        budget_replay::{evaluate_gate, on_budget_metrics, RiskProfile},
        kline_engine::{run_kline_screening_with_funding, FundingRatePoint},
    },
    sqlite_market_data::{load_funding_rates_readonly, SqliteMarketDataSource},
};
use rust_decimal::Decimal;
use serde::Serialize;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleEntryTrigger,
    MartingaleIndicatorConfig, MartingaleMarginMode, MartingaleMarketKind,
    MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel,
    MartingaleStopLossModel, MartingaleStrategyConfig, MartingaleTakeProfitModel,
};

#[derive(Debug, Clone)]
struct Args {
    budgets: Vec<f64>,
    symbols: Vec<String>,
    direction_modes: Vec<SearchDirectionMode>,
    entry_filters: Vec<EntryFilter>,
    start_ms: i64,
    end_ms: i64,
    market_data_path: PathBuf,
    funding_data_path: PathBuf,
    output_path: PathBuf,
    top_n: usize,
    max_candidates: Option<usize>,
    max_params_per_symbol_budget: Option<usize>,
    grid: GridKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GridKind {
    Tiny,
    Small,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchDirectionMode {
    LongOnly,
    ShortOnly,
    LongAndShort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryFilter {
    None,
    Trend,
    TrendRsi,
    RsiExtreme,
    RsiModerate,
    BollingerExtreme,
    BollingerModerate,
    RsiBollingerExtreme,
    RsiBollingerModerate,
}

#[derive(Debug, Clone)]
struct Param {
    first_order_quote: f64,
    leverage: u32,
    multiplier: f64,
    max_legs: u32,
    step_bps: u32,
    take_profit_bps: u32,
    cooldown_seconds: u64,
    adx_min: Option<u32>,
    stop_loss_bps: u32,
    entry_filter: EntryFilter,
}

#[derive(Debug, Clone, Serialize)]
struct CandidateRow {
    budget: f64,
    symbol: String,
    direction_mode: String,
    entry_filter: String,
    new_cycle_drawdown_pause_pct: f64,
    new_cycle_atr_pause_pct: f64,
    safety_skip_adx_threshold: f64,
    first_order_quote: f64,
    leverage: u32,
    multiplier: f64,
    max_legs: u32,
    step_bps: u32,
    take_profit_bps: u32,
    cooldown_seconds: u64,
    adx_min: Option<u32>,
    stop_loss_bps: u32,
    planned_margin_quote: f64,
    annualized_return_pct: f64,
    max_drawdown_pct: f64,
    total_return_pct: f64,
    min_equity_quote: f64,
    principal_breached: bool,
    max_capital_used_quote: f64,
    trade_count: u64,
    stop_count: u64,
    total_fee_quote: f64,
    total_slippage_quote: f64,
    total_funding_quote: f64,
    conservative_pass: bool,
    balanced_pass: bool,
    aggressive_pass: bool,
}

#[derive(Debug, Serialize)]
struct SearchReport {
    generated_at: String,
    range: TimeRange,
    risk_guards: RiskGuardSnapshot,
    budgets: Vec<f64>,
    symbols: Vec<String>,
    candidate_count: usize,
    elapsed_seconds: f64,
    best_by_budget: BTreeMap<String, Vec<CandidateRow>>,
    frontier_by_budget: BTreeMap<String, FrontierSet>,
    pass_candidates: Vec<CandidateRow>,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct RiskGuardSnapshot {
    new_cycle_drawdown_pause_pct: f64,
    new_cycle_atr_pause_pct: f64,
    safety_skip_adx_threshold: f64,
}

#[derive(Debug, Serialize)]
struct FrontierSet {
    highest_annualized: Vec<CandidateRow>,
    best_under_dd10: Vec<CandidateRow>,
    best_under_dd20: Vec<CandidateRow>,
    best_under_dd30: Vec<CandidateRow>,
    lowest_dd_over_ann50: Vec<CandidateRow>,
    lowest_dd_over_ann90: Vec<CandidateRow>,
    lowest_dd_over_ann110: Vec<CandidateRow>,
}

#[derive(Debug, Serialize)]
struct TimeRange {
    start_ms: i64,
    end_ms: i64,
}

fn main() -> Result<(), String> {
    let args = Args::parse()?;
    let risk_guards = RiskGuardSnapshot::from_env();
    let started = Instant::now();
    let market = SqliteMarketDataSource::open_readonly(&args.market_data_path)?;
    let symbols = if args.symbols.is_empty() {
        market.recommended_liquid_symbols(args.start_ms, args.end_ms - 7 * 86_400_000, 12)?
    } else {
        args.symbols.clone()
    };
    eprintln!("symbols={symbols:?}");

    let mut bars_by_symbol = BTreeMap::new();
    for symbol in &symbols {
        let bars = market.load_klines(symbol, args.start_ms, args.end_ms, "1m")?;
        eprintln!("loaded {symbol}: {} bars", bars.len());
        if !bars.is_empty() {
            bars_by_symbol.insert(symbol.clone(), bars);
        }
    }
    let active_symbols = bars_by_symbol.keys().cloned().collect::<Vec<_>>();
    let funding = load_funding_rates_readonly(
        &args.funding_data_path,
        &active_symbols,
        args.start_ms,
        args.end_ms,
    )?;
    let funding_by_symbol = group_funding_by_symbol(&funding);

    let params = parameter_grid(args.grid, &args.entry_filters);
    eprintln!(
        "param_grid={} candidates_per_symbol_budget={}",
        args.grid.as_str(),
        params.len()
    );
    let mut rows = Vec::new();
    let mut evaluated = 0_usize;
    let sampled_params = sampled_params(&params, args.max_params_per_symbol_budget);
    let mut stop_all = false;
    'budgets: for budget in &args.budgets {
        for symbol in &active_symbols {
            if stop_all {
                break 'budgets;
            }
            let Some(bars) = bars_by_symbol.get(symbol) else {
                continue;
            };
            let empty = Vec::new();
            let symbol_funding = funding_by_symbol.get(symbol).unwrap_or(&empty);
            for mode in &args.direction_modes {
                let budget_params = budget_scaled_params(&sampled_params, *budget, *mode);
                for param in &budget_params {
                    if !param_fits_budget(*budget, param, *mode) {
                        continue;
                    }
                    let portfolio = build_portfolio(symbol, *budget, param, *mode)?;
                    let result = run_kline_screening_with_funding(
                        portfolio,
                        bars,
                        symbol_funding.as_slice(),
                    )?;
                    let initial = result
                        .equity_curve
                        .first()
                        .map(|p| p.equity_quote)
                        .unwrap_or(0.0);
                    let cum_pnl = result
                        .equity_curve
                        .iter()
                        .map(|p| p.equity_quote - initial)
                        .collect::<Vec<_>>();
                    let days = result
                        .equity_curve
                        .last()
                        .zip(result.equity_curve.first())
                        .map(|(l, f)| ((l.timestamp_ms - f.timestamp_ms) as f64) / 86_400_000.0)
                        .unwrap_or(0.0);
                    let on_budget = on_budget_metrics(*budget, &cum_pnl, days);
                    let conservative = evaluate_gate(
                        RiskProfile::Conservative,
                        on_budget.annualized_return_pct,
                        on_budget.max_drawdown_pct,
                        on_budget.principal_breached,
                        result.metrics.max_capital_used_quote,
                        *budget,
                    );
                    let balanced = evaluate_gate(
                        RiskProfile::Balanced,
                        on_budget.annualized_return_pct,
                        on_budget.max_drawdown_pct,
                        on_budget.principal_breached,
                        result.metrics.max_capital_used_quote,
                        *budget,
                    );
                    let aggressive = evaluate_gate(
                        RiskProfile::Aggressive,
                        on_budget.annualized_return_pct,
                        on_budget.max_drawdown_pct,
                        on_budget.principal_breached,
                        result.metrics.max_capital_used_quote,
                        *budget,
                    );
                    rows.push(CandidateRow {
                        budget: *budget,
                        symbol: symbol.clone(),
                        direction_mode: mode.as_str().to_string(),
                        entry_filter: param.entry_filter.as_key().to_string(),
                        new_cycle_drawdown_pause_pct: risk_guards.new_cycle_drawdown_pause_pct,
                        new_cycle_atr_pause_pct: risk_guards.new_cycle_atr_pause_pct,
                        safety_skip_adx_threshold: risk_guards.safety_skip_adx_threshold,
                        first_order_quote: param.first_order_quote,
                        leverage: param.leverage,
                        multiplier: param.multiplier,
                        max_legs: param.max_legs,
                        step_bps: param.step_bps,
                        take_profit_bps: param.take_profit_bps,
                        cooldown_seconds: param.cooldown_seconds,
                        adx_min: param.adx_min,
                        stop_loss_bps: param.stop_loss_bps,
                        planned_margin_quote: planned_margin(param) * mode.strategy_factor(),
                        annualized_return_pct: on_budget.annualized_return_pct,
                        max_drawdown_pct: on_budget.max_drawdown_pct,
                        total_return_pct: on_budget.total_return_pct,
                        min_equity_quote: on_budget.min_equity_quote,
                        principal_breached: on_budget.principal_breached,
                        max_capital_used_quote: result.metrics.max_capital_used_quote,
                        trade_count: result.metrics.trade_count,
                        stop_count: result.metrics.stop_count,
                        total_fee_quote: result.metrics.total_fee_quote.unwrap_or(0.0),
                        total_slippage_quote: result.metrics.total_slippage_quote.unwrap_or(0.0),
                        total_funding_quote: result.metrics.total_funding_quote.unwrap_or(0.0),
                        conservative_pass: conservative.passed,
                        balanced_pass: balanced.passed,
                        aggressive_pass: aggressive.passed,
                    });
                    evaluated += 1;
                    if evaluated % 250 == 0 {
                        eprintln!(
                            "evaluated={evaluated} rows={} elapsed={:.1}s",
                            rows.len(),
                            started.elapsed().as_secs_f64()
                        );
                    }
                    if let Some(max) = args.max_candidates {
                        if evaluated >= max {
                            eprintln!("max-candidates reached; finishing partial search");
                            stop_all = true;
                            break;
                        }
                    }
                }
                if stop_all {
                    break;
                }
            }
        }
    }

    rows.sort_by(|a, b| rank_score(b).total_cmp(&rank_score(a)));
    let mut best_by_budget = BTreeMap::new();
    let mut frontier_by_budget = BTreeMap::new();
    for budget in &args.budgets {
        let mut subset = rows
            .iter()
            .filter(|row| (row.budget - *budget).abs() < 1e-9)
            .cloned()
            .collect::<Vec<_>>();
        subset.sort_by(|a, b| rank_score(b).total_cmp(&rank_score(a)));
        subset.truncate(args.top_n);
        best_by_budget.insert(format!("{budget:.0}"), subset);

        let full_subset = rows
            .iter()
            .filter(|row| (row.budget - *budget).abs() < 1e-9)
            .cloned()
            .collect::<Vec<_>>();
        frontier_by_budget.insert(
            format!("{budget:.0}"),
            build_frontier(&full_subset, args.top_n),
        );
    }
    let mut pass_candidates = rows
        .iter()
        .filter(|row| row.conservative_pass || row.balanced_pass || row.aggressive_pass)
        .cloned()
        .collect::<Vec<_>>();
    pass_candidates.sort_by(|a, b| rank_score(b).total_cmp(&rank_score(a)));
    pass_candidates.truncate(200);

    let report = SearchReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        range: TimeRange {
            start_ms: args.start_ms,
            end_ms: args.end_ms,
        },
        risk_guards,
        budgets: args.budgets,
        symbols: active_symbols,
        candidate_count: rows.len(),
        elapsed_seconds: started.elapsed().as_secs_f64(),
        best_by_budget,
        frontier_by_budget,
        pass_candidates,
    };
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    fs::write(&args.output_path, text)
        .map_err(|e| format!("write {}: {e}", args.output_path.display()))?;
    eprintln!(
        "wrote {} rows={} passes={}",
        args.output_path.display(),
        rows.len(),
        report.pass_candidates.len()
    );
    Ok(())
}

impl RiskGuardSnapshot {
    fn from_env() -> Self {
        Self {
            new_cycle_drawdown_pause_pct: read_env_f64("MARTINGALE_BT_NEW_CYCLE_DD_PAUSE_PCT", 6.0),
            new_cycle_atr_pause_pct: read_env_f64("MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT", 2.0),
            safety_skip_adx_threshold: read_env_f64("MARTINGALE_BT_SAFETY_SKIP_ADX", 45.0),
        }
    }
}

fn read_env_f64(name: &str, default: f64) -> f64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(default)
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut budgets = vec![500.0, 1000.0, 1500.0, 2000.0, 3000.0, 5000.0];
        let mut symbols = Vec::new();
        let mut direction_modes = vec![SearchDirectionMode::LongAndShort];
        let mut entry_filters = vec![EntryFilter::None];
        let mut start_ms = 1_672_531_200_000_i64;
        let mut end_ms = 1_780_271_999_999_i64;
        let mut market_data_path = PathBuf::from("data/market_data_full.db");
        let mut funding_data_path = PathBuf::from("data/funding_rates.db");
        let mut output_path =
            PathBuf::from("docs/superpowers/reports/2026-06-27-small-capital-native-search.json");
        let mut top_n = 50_usize;
        let mut max_candidates = None;
        let mut max_params_per_symbol_budget = None;
        let mut grid = GridKind::Small;
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for argument {arg}"))?;
            match arg.as_str() {
                "--budgets" => budgets = parse_f64_list(&value)?,
                "--symbols" => symbols = parse_symbol_list(&value),
                "--direction-modes" => direction_modes = parse_direction_modes(&value)?,
                "--entry-filters" => entry_filters = parse_entry_filters(&value)?,
                "--start-ms" => start_ms = value.parse().map_err(|e| format!("start-ms: {e}"))?,
                "--end-ms" => end_ms = value.parse().map_err(|e| format!("end-ms: {e}"))?,
                "--market-data" => market_data_path = PathBuf::from(value),
                "--funding-data" => funding_data_path = PathBuf::from(value),
                "--output" => output_path = PathBuf::from(value),
                "--top-n" => top_n = value.parse().map_err(|e| format!("top-n: {e}"))?,
                "--max-candidates" => {
                    max_candidates =
                        Some(value.parse().map_err(|e| format!("max-candidates: {e}"))?)
                }
                "--max-params-per-symbol-budget" => {
                    max_params_per_symbol_budget = Some(
                        value
                            .parse()
                            .map_err(|e| format!("max-params-per-symbol-budget: {e}"))?,
                    )
                }
                "--grid" => grid = GridKind::parse(&value)?,
                _ => return Err(format!("unknown argument {arg}")),
            }
        }
        Ok(Self {
            budgets,
            symbols,
            direction_modes,
            entry_filters,
            start_ms,
            end_ms,
            market_data_path,
            funding_data_path,
            output_path,
            top_n,
            max_candidates,
            max_params_per_symbol_budget,
            grid,
        })
    }
}

impl SearchDirectionMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "long_only" => Ok(Self::LongOnly),
            "short_only" => Ok(Self::ShortOnly),
            "long_and_short" => Ok(Self::LongAndShort),
            _ => Err("direction mode must be long_only|short_only|long_and_short".to_string()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::LongOnly => "long_only",
            Self::ShortOnly => "short_only",
            Self::LongAndShort => "long_and_short",
        }
    }

    fn strategy_factor(self) -> f64 {
        match self {
            Self::LongOnly | Self::ShortOnly => 1.0,
            Self::LongAndShort => 2.0,
        }
    }
}

impl EntryFilter {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "none" => Ok(Self::None),
            "trend" => Ok(Self::Trend),
            "trend_rsi" => Ok(Self::TrendRsi),
            "rsi_extreme" => Ok(Self::RsiExtreme),
            "rsi_moderate" => Ok(Self::RsiModerate),
            "bb_extreme" => Ok(Self::BollingerExtreme),
            "bb_moderate" => Ok(Self::BollingerModerate),
            "rsi_bb_extreme" => Ok(Self::RsiBollingerExtreme),
            "rsi_bb_moderate" => Ok(Self::RsiBollingerModerate),
            _ => Err(
                "entry filter must be none|trend|trend_rsi|rsi_extreme|rsi_moderate|bb_extreme|bb_moderate|rsi_bb_extreme|rsi_bb_moderate"
                    .to_string(),
            ),
        }
    }

    fn as_key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Trend => "trend",
            Self::TrendRsi => "trend_rsi",
            Self::RsiExtreme => "rsi_extreme",
            Self::RsiModerate => "rsi_moderate",
            Self::BollingerExtreme => "bb_extreme",
            Self::BollingerModerate => "bb_moderate",
            Self::RsiBollingerExtreme => "rsi_bb_extreme",
            Self::RsiBollingerModerate => "rsi_bb_moderate",
        }
    }
}

impl GridKind {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "tiny" => Ok(Self::Tiny),
            "small" => Ok(Self::Small),
            "full" => Ok(Self::Full),
            _ => Err("grid must be tiny|small|full".to_string()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Small => "small",
            Self::Full => "full",
        }
    }
}

fn parse_f64_list(text: &str) -> Result<Vec<f64>, String> {
    text.split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().parse::<f64>().map_err(|e| e.to_string()))
        .collect()
}

fn parse_symbol_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_direction_modes(text: &str) -> Result<Vec<SearchDirectionMode>, String> {
    let modes = text
        .split(',')
        .map(|s| SearchDirectionMode::parse(s.trim()))
        .collect::<Result<Vec<_>, _>>()?;
    if modes.is_empty() {
        Err("direction-modes cannot be empty".to_string())
    } else {
        Ok(modes)
    }
}

fn parse_entry_filters(text: &str) -> Result<Vec<EntryFilter>, String> {
    let filters = text
        .split(',')
        .map(|s| EntryFilter::parse(s.trim()))
        .collect::<Result<Vec<_>, _>>()?;
    if filters.is_empty() {
        Err("entry-filters cannot be empty".to_string())
    } else {
        Ok(filters)
    }
}

fn parameter_grid(kind: GridKind, entry_filters: &[EntryFilter]) -> Vec<Param> {
    let mut out = Vec::new();
    let first_orders: &[f64] = match kind {
        GridKind::Tiny => &[5.0, 10.0, 20.0],
        GridKind::Small => &[5.0, 10.0, 20.0, 40.0],
        GridKind::Full => &[5.0, 8.0, 10.0, 15.0, 20.0, 30.0, 40.0, 60.0, 80.0],
    };
    let leverages: &[u32] = match kind {
        GridKind::Tiny => &[5, 10],
        GridKind::Small => &[3, 5, 10],
        GridKind::Full => &[3, 5, 8, 10],
    };
    let multipliers: &[f64] = match kind {
        GridKind::Tiny => &[1.25, 1.6],
        GridKind::Small => &[1.2, 1.4, 1.7, 2.0],
        GridKind::Full => &[1.15, 1.25, 1.4, 1.6, 1.8, 2.0],
    };
    let max_legs: &[u32] = match kind {
        GridKind::Tiny => &[3, 5],
        GridKind::Small => &[2, 3, 5, 7],
        GridKind::Full => &[2, 3, 4, 5, 6, 7],
    };
    let steps: &[u32] = match kind {
        GridKind::Tiny => &[150, 300],
        GridKind::Small => &[100, 180, 300, 500],
        GridKind::Full => &[80, 120, 180, 250, 350, 500],
    };
    let tps: &[u32] = match kind {
        GridKind::Tiny => &[80, 150],
        GridKind::Small => &[50, 100, 180],
        GridKind::Full => &[50, 80, 120, 180, 250],
    };
    let cooldowns: &[u64] = match kind {
        GridKind::Tiny => &[10_800, 43_200],
        GridKind::Small => &[3_600, 21_600, 43_200],
        GridKind::Full => &[3_600, 10_800, 21_600, 43_200],
    };
    let adx_values: &[Option<u32>] = match kind {
        GridKind::Tiny => &[None, Some(20)],
        GridKind::Small => &[None, Some(15), Some(25)],
        GridKind::Full => &[None, Some(15), Some(20), Some(25)],
    };
    let stops: &[u32] = match kind {
        GridKind::Tiny => &[1200, 3000],
        GridKind::Small => &[800, 2000, 5000],
        GridKind::Full => &[800, 1200, 2000, 3000, 5000],
    };
    for &first_order_quote in first_orders {
        for &leverage in leverages {
            for &multiplier in multipliers {
                for &max_legs in max_legs {
                    for &step_bps in steps {
                        for &take_profit_bps in tps {
                            if take_profit_bps >= step_bps && max_legs > 2 && kind != GridKind::Tiny
                            {
                                continue;
                            }
                            for &cooldown_seconds in cooldowns {
                                for &adx_min in adx_values {
                                    for &stop_loss_bps in stops {
                                        for &entry_filter in entry_filters {
                                            out.push(Param {
                                                first_order_quote,
                                                leverage,
                                                multiplier,
                                                max_legs,
                                                step_bps,
                                                take_profit_bps,
                                                cooldown_seconds,
                                                adx_min,
                                                stop_loss_bps,
                                                entry_filter,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn sampled_params(params: &[Param], max: Option<usize>) -> Vec<Param> {
    let Some(max) = max else {
        return params.to_vec();
    };
    if max == 0 || params.is_empty() {
        return Vec::new();
    }
    if params.len() <= max {
        return params.to_vec();
    }
    if max == 1 {
        return vec![params[0].clone()];
    }
    let last = params.len() - 1;
    let denom = max - 1;
    let mut out = Vec::with_capacity(max);
    let mut seen = std::collections::BTreeSet::new();
    for i in 0..max {
        let idx = i * last / denom;
        if seen.insert(idx) {
            out.push(params[idx].clone());
        }
    }
    out
}

fn budget_scaled_params(params: &[Param], budget: f64, mode: SearchDirectionMode) -> Vec<Param> {
    let utilization_targets = [0.15_f64, 0.35, 0.60, 0.85, 0.95];
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for p in params {
        // Keep the explicit small first-order variant as a low-risk baseline.
        push_dedup(&mut out, &mut seen, p.clone());

        let denom = planned_margin_per_first_order(p) * mode.strategy_factor();
        if denom <= 0.0 || !denom.is_finite() {
            continue;
        }
        for util in utilization_targets {
            let first = (budget * util / denom).max(5.0);
            let first = (first * 100.0).round() / 100.0;
            let mut scaled = p.clone();
            scaled.first_order_quote = first;
            push_dedup(&mut out, &mut seen, scaled);
        }
    }
    out
}

fn push_dedup(out: &mut Vec<Param>, seen: &mut std::collections::BTreeSet<String>, p: Param) {
    let key = format!(
        "{:.2}|{}|{:.4}|{}|{}|{}|{}|{:?}|{}|{}",
        p.first_order_quote,
        p.leverage,
        p.multiplier,
        p.max_legs,
        p.step_bps,
        p.take_profit_bps,
        p.cooldown_seconds,
        p.adx_min,
        p.stop_loss_bps,
        p.entry_filter.as_key()
    );
    if seen.insert(key) {
        out.push(p);
    }
}

fn param_fits_budget(budget: f64, p: &Param, mode: SearchDirectionMode) -> bool {
    if p.first_order_quote < 5.0 {
        return false;
    }
    let margin = planned_margin(p) * mode.strategy_factor();
    margin <= budget * 0.95
}

fn planned_margin(p: &Param) -> f64 {
    planned_margin_per_first_order(p) * p.first_order_quote
}

fn planned_margin_per_first_order(p: &Param) -> f64 {
    let mut total = 0.0;
    let mut notional = 1.0;
    for _ in 0..p.max_legs {
        total += notional / p.leverage as f64;
        notional *= p.multiplier;
    }
    total
}

fn build_portfolio(
    symbol: &str,
    budget: f64,
    p: &Param,
    mode: SearchDirectionMode,
) -> Result<MartingalePortfolioConfig, String> {
    let mut strategies = Vec::new();
    let portfolio_mode = match mode {
        SearchDirectionMode::LongOnly => {
            strategies.push(strategy(symbol, MartingaleDirection::Long, mode, p)?);
            MartingaleDirectionMode::LongOnly
        }
        SearchDirectionMode::ShortOnly => {
            strategies.push(strategy(symbol, MartingaleDirection::Short, mode, p)?);
            MartingaleDirectionMode::ShortOnly
        }
        SearchDirectionMode::LongAndShort => {
            strategies.push(strategy(symbol, MartingaleDirection::Long, mode, p)?);
            strategies.push(strategy(symbol, MartingaleDirection::Short, mode, p)?);
            MartingaleDirectionMode::LongAndShort
        }
    };
    Ok(MartingalePortfolioConfig {
        direction_mode: portfolio_mode,
        strategies,
        risk_limits: MartingaleRiskLimits {
            max_global_budget_quote: Some(decimal(budget)?),
            ..MartingaleRiskLimits::default()
        },
    })
}

fn strategy(
    symbol: &str,
    direction: MartingaleDirection,
    mode: SearchDirectionMode,
    p: &Param,
) -> Result<MartingaleStrategyConfig, String> {
    let dir = match direction {
        MartingaleDirection::Long => "long",
        MartingaleDirection::Short => "short",
    };
    let mut entry_triggers = vec![MartingaleEntryTrigger::Cooldown {
        seconds: p.cooldown_seconds,
    }];
    if let Some(adx) = p.adx_min {
        entry_triggers.push(MartingaleEntryTrigger::IndicatorExpression {
            expression: format!("adx(14) > {adx}"),
        });
    }
    add_entry_filter_triggers(&mut entry_triggers, direction, p.entry_filter);
    Ok(MartingaleStrategyConfig {
        strategy_id: format!(
            "small-{symbol}-{dir}-foq{}-lev{}-m{:.2}-legs{}-step{}-tp{}-cd{}-adx{}-sl{}-filter{}",
            p.first_order_quote,
            p.leverage,
            p.multiplier,
            p.max_legs,
            p.step_bps,
            p.take_profit_bps,
            p.cooldown_seconds,
            p.adx_min.unwrap_or(0),
            p.stop_loss_bps,
            p.entry_filter.as_key()
        ),
        symbol: symbol.to_string(),
        market: MartingaleMarketKind::UsdMFutures,
        direction,
        direction_mode: match mode {
            SearchDirectionMode::LongOnly => MartingaleDirectionMode::LongOnly,
            SearchDirectionMode::ShortOnly => MartingaleDirectionMode::ShortOnly,
            SearchDirectionMode::LongAndShort => MartingaleDirectionMode::LongAndShort,
        },
        margin_mode: Some(MartingaleMarginMode::Isolated),
        leverage: Some(p.leverage),
        spacing: MartingaleSpacingModel::FixedPercent {
            step_bps: p.step_bps,
        },
        sizing: MartingaleSizingModel::Multiplier {
            first_order_quote: decimal(p.first_order_quote)?,
            multiplier: decimal(p.multiplier)?,
            max_legs: p.max_legs,
        },
        take_profit: MartingaleTakeProfitModel::Percent {
            bps: p.take_profit_bps,
        },
        stop_loss: Some(MartingaleStopLossModel::StrategyDrawdownPct {
            pct_bps: p.stop_loss_bps,
        }),
        indicators: vec![
            MartingaleIndicatorConfig::Atr { period: 21 },
            MartingaleIndicatorConfig::Adx { period: 14 },
        ],
        entry_triggers,
        risk_limits: MartingaleRiskLimits::default(),
    })
}

fn add_entry_filter_triggers(
    triggers: &mut Vec<MartingaleEntryTrigger>,
    direction: MartingaleDirection,
    filter: EntryFilter,
) {
    match filter {
        EntryFilter::None => {}
        EntryFilter::Trend | EntryFilter::TrendRsi => {
            let trend_expr = match direction {
                MartingaleDirection::Long => "close > ema(200)",
                MartingaleDirection::Short => "close < ema(200)",
            };
            triggers.push(MartingaleEntryTrigger::IndicatorExpression {
                expression: trend_expr.to_string(),
            });
            if filter == EntryFilter::TrendRsi {
                let rsi_expr = match direction {
                    MartingaleDirection::Long => "rsi(14) < 65",
                    MartingaleDirection::Short => "rsi(14) > 35",
                };
                triggers.push(MartingaleEntryTrigger::IndicatorExpression {
                    expression: rsi_expr.to_string(),
                });
            }
        }
        EntryFilter::RsiExtreme => {
            triggers.push(MartingaleEntryTrigger::IndicatorExpression {
                expression: rsi_mean_reversion_expr(direction, 30, 70).to_string(),
            });
        }
        EntryFilter::RsiModerate => {
            triggers.push(MartingaleEntryTrigger::IndicatorExpression {
                expression: rsi_mean_reversion_expr(direction, 35, 65).to_string(),
            });
        }
        EntryFilter::BollingerExtreme => {
            triggers.push(MartingaleEntryTrigger::IndicatorExpression {
                expression: bollinger_mean_reversion_expr(direction, "2.5").to_string(),
            });
        }
        EntryFilter::BollingerModerate => {
            triggers.push(MartingaleEntryTrigger::IndicatorExpression {
                expression: bollinger_mean_reversion_expr(direction, "2").to_string(),
            });
        }
        EntryFilter::RsiBollingerExtreme => {
            triggers.push(MartingaleEntryTrigger::IndicatorExpression {
                expression: rsi_mean_reversion_expr(direction, 35, 65).to_string(),
            });
            triggers.push(MartingaleEntryTrigger::IndicatorExpression {
                expression: bollinger_mean_reversion_expr(direction, "2").to_string(),
            });
        }
        EntryFilter::RsiBollingerModerate => {
            triggers.push(MartingaleEntryTrigger::IndicatorExpression {
                expression: rsi_mean_reversion_expr(direction, 40, 60).to_string(),
            });
            triggers.push(MartingaleEntryTrigger::IndicatorExpression {
                expression: bollinger_mean_reversion_expr(direction, "1.5").to_string(),
            });
        }
    }
}

fn rsi_mean_reversion_expr(
    direction: MartingaleDirection,
    long_threshold: u32,
    short_threshold: u32,
) -> String {
    match direction {
        MartingaleDirection::Long => format!("rsi(14) < {long_threshold}"),
        MartingaleDirection::Short => format!("rsi(14) > {short_threshold}"),
    }
}

fn bollinger_mean_reversion_expr(direction: MartingaleDirection, stddev: &str) -> String {
    match direction {
        MartingaleDirection::Long => format!("close < bb_lower(20,{stddev})"),
        MartingaleDirection::Short => format!("close > bb_upper(20,{stddev})"),
    }
}

fn decimal(value: f64) -> Result<Decimal, String> {
    Decimal::try_from(value).map_err(|e| e.to_string())
}

fn group_funding_by_symbol(points: &[FundingRatePoint]) -> BTreeMap<String, Vec<FundingRatePoint>> {
    let mut out: BTreeMap<String, Vec<FundingRatePoint>> = BTreeMap::new();
    for point in points {
        out.entry(point.symbol.clone())
            .or_default()
            .push(point.clone());
    }
    out
}

fn rank_score(row: &CandidateRow) -> f64 {
    if row.principal_breached || !row.annualized_return_pct.is_finite() {
        return -1_000_000.0 + row.total_return_pct;
    }
    row.annualized_return_pct - row.max_drawdown_pct * 5.0 - (row.trade_count as f64 / 1000.0)
}

fn build_frontier(rows: &[CandidateRow], top_n: usize) -> FrontierSet {
    FrontierSet {
        highest_annualized: top_by(rows, top_n, |row| row.annualized_return_pct, None),
        best_under_dd10: top_by(
            rows,
            top_n,
            |row| row.annualized_return_pct,
            Some(|row: &CandidateRow| row.max_drawdown_pct <= 10.0),
        ),
        best_under_dd20: top_by(
            rows,
            top_n,
            |row| row.annualized_return_pct,
            Some(|row: &CandidateRow| row.max_drawdown_pct <= 20.0),
        ),
        best_under_dd30: top_by(
            rows,
            top_n,
            |row| row.annualized_return_pct,
            Some(|row: &CandidateRow| row.max_drawdown_pct <= 30.0),
        ),
        lowest_dd_over_ann50: low_by(
            rows,
            top_n,
            |row| row.max_drawdown_pct,
            Some(|row: &CandidateRow| row.annualized_return_pct > 50.0),
        ),
        lowest_dd_over_ann90: low_by(
            rows,
            top_n,
            |row| row.max_drawdown_pct,
            Some(|row: &CandidateRow| row.annualized_return_pct > 90.0),
        ),
        lowest_dd_over_ann110: low_by(
            rows,
            top_n,
            |row| row.max_drawdown_pct,
            Some(|row: &CandidateRow| row.annualized_return_pct > 110.0),
        ),
    }
}

fn top_by(
    rows: &[CandidateRow],
    top_n: usize,
    score: impl Fn(&CandidateRow) -> f64,
    pred: Option<fn(&CandidateRow) -> bool>,
) -> Vec<CandidateRow> {
    let mut out = rows
        .iter()
        .filter(|row| pred.map(|p| p(row)).unwrap_or(true))
        .filter(|row| !row.principal_breached && row.annualized_return_pct.is_finite())
        .cloned()
        .collect::<Vec<_>>();
    out.sort_by(|a, b| score(b).total_cmp(&score(a)));
    out.truncate(top_n);
    out
}

fn low_by(
    rows: &[CandidateRow],
    top_n: usize,
    score: impl Fn(&CandidateRow) -> f64,
    pred: Option<fn(&CandidateRow) -> bool>,
) -> Vec<CandidateRow> {
    let mut out = rows
        .iter()
        .filter(|row| pred.map(|p| p(row)).unwrap_or(true))
        .filter(|row| !row.principal_breached && row.annualized_return_pct.is_finite())
        .cloned()
        .collect::<Vec<_>>();
    out.sort_by(|a, b| score(a).total_cmp(&score(b)));
    out.truncate(top_n);
    out
}
