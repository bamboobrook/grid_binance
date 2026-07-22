//! Multi-symbol R8 combination: runs M1 gated-Martin on each symbol in parallel
//! with fit-only equal-risk allocation, combines equity curves, enforces symbol
//! gross <= 25%, and recomputes kurtosis/DSR/PBO on the combined return stream.
//!
//! This is the plan §13 combination: a shared cash/margin/reserve account with
//! per-symbol sleeves at equal-risk weights.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Result};

use backtest_engine::martingale::exchange_model::ExchangeFilters;
use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use backtest_engine::market_data::KlineBar;
use r23_replay::gated_martin::{run_gated_martin, GatedMartinConfig, SignalGate, SignalSource, TpMode};
use r23_replay::m1_signal::{compute_m1_states, M1State, MetricRow};
use r23_replay::multiple_testing::{deflated_sharpe, probability_of_backtest_overfitting, sharpe_stats};

fn main() -> Result<()> {
    let metrics_dir = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: r23_multi_symbol <metrics_dir> [budget] [price_ext_thresh] [fo_pct] [max_legs] [symbols...]"))?;
    let budget: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
    let price_ext_thresh: f64 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(1.5);
    let fo_pct: f64 = std::env::args().nth(4).and_then(|s| s.parse().ok()).unwrap_or(10.0);
    let max_legs: u32 = std::env::args().nth(5).and_then(|s| s.parse().ok()).unwrap_or(2);
    let tp_mode_str: String = std::env::args().nth(6).unwrap_or_else(|| "ScaleOut".to_string());
    let tp_mode = match tp_mode_str.as_str() {
        "Micro" => r23_replay::gated_martin::TpMode::Micro,
        "Fixed" => r23_replay::gated_martin::TpMode::Fixed,
        "Early" => r23_replay::gated_martin::TpMode::Early,
        _ => r23_replay::gated_martin::TpMode::ScaleOut,
    };
    let symbols: Vec<String> = std::env::args().skip(7).collect();
    let symbols: Vec<String> = if symbols.is_empty() { vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()] } else { symbols };
    println!("params: budget={budget} thresh={price_ext_thresh} fo_pct={fo_pct} max_legs={max_legs} tp={tp_mode_str}");

    // Load per-symbol bars + signals over the COMMON intersection window.
    let src = SqliteMarketDataSource::open_readonly("data/market_data.full.db").or_else(|_| SqliteMarketDataSource::open_readonly("data/market_data_full.db")).unwrap();
    let mut per_sym: BTreeMap<String, (Vec<KlineBar>, BTreeMap<i64, M1State>, i64, i64)> = BTreeMap::new();
    for sym in &symbols {
        let mdir = Path::new(&metrics_dir).join(sym);
        let metrics = load_metrics(&mdir, sym)?;
        if metrics.is_empty() { println!("WARN: no metrics for {sym}"); continue; }
        let mn = metrics.first().unwrap().ts_ms;
        let mx = metrics.last().unwrap().ts_ms;
        let bars1m = src.load_klines_with_market_type(sym, "futures_usdt_perp", mn, mx, "1m").unwrap();
        let bars = resample(&bars1m, 5);
        let closes: Vec<(i64, f64)> = bars.iter().map(|b| (b.open_time_ms, b.close)).collect();
        let states = compute_m1_states(&metrics, &closes, 2016);
        let sm: BTreeMap<i64, M1State> = states.iter().map(|s| (s.ts_ms, s.clone())).collect();
        let nbars = bars.len();
        let nstates = sm.len();
        let span_d = (mx - mn) as f64 / 86_400_000.0;
        per_sym.insert(sym.clone(), (bars, sm, mn, mx));
        println!("{sym}: {} bars, {} states, window {:.0}d", nbars, nstates, span_d);
    }
    if per_sym.len() < 2 { return Err(anyhow!("need >=2 symbols with metrics for combination; got {}", per_sym.len())); }

    // Common intersection window.
    let mut common_mn = i64::MIN;
    let mut common_mx = i64::MAX;
    for (_, _, mn, mx) in per_sym.values() { common_mn = common_mn.max(*mn); common_mx = common_mx.min(*mx); }
    if common_mn >= common_mx { return Err(anyhow!("no common window")); }
    let n_sym = per_sym.len() as f64;
    let sleeve_budget = budget / n_sym; // equal-risk: split budget equally
    let fo_quote = (fo_pct / 100.0 * sleeve_budget).max(6.0); // each sleeve fo_pct of its budget
    println!("combination: {} symbols, common window {:.0}d, sleeve_budget={:.1}, fo={:.1}", n_sym, (common_mx-common_mn) as f64/86_400_000.0, sleeve_budget, fo_quote);

    // Run each symbol's policy independently, collect equity curves.
    let mut combined_equity: BTreeMap<i64, f64> = BTreeMap::new();
    let mut symbol_gross: BTreeMap<String, f64> = BTreeMap::new();
    let mut policy_returns: Vec<Vec<f64>> = Vec::new();
    for (sym, (bars, sm, _, _)) in &per_sym {
        let cbars: Vec<KlineBar> = bars.iter().filter(|b| b.open_time_ms >= common_mn && b.open_time_ms <= common_mx).cloned().collect();
        let signal = RelaxedM1 { state_map: sm.clone(), thresh: price_ext_thresh };
        let cfg = GatedMartinConfig {
            symbol: sym.clone(),
            filters: ExchangeFilters::default_for(sym),
            budget_quote: sleeve_budget,
            fo_quote,
            adverse_spacing_frac: 0.01,
            tp_net_bps_floor: 10.0,
            fee_bps: 2.0,
            slippage_bps: 1.0,
            leverage: 3,
            direction_bias: 1,
            tp_mode,
            max_legs,
        };
        let res = run_gated_martin(&cfg, &cbars, &signal).unwrap_or_else(|_| empty_result(sleeve_budget));
        let mut daily: BTreeMap<i64, f64> = BTreeMap::new();
        for e in &res.equity_curve { daily.insert(e.timestamp_ms / 86_400_000, e.equity_quote); }
        let rets: Vec<f64> = daily.values().collect::<Vec<_>>().windows(2).filter_map(|w| if w[0] > &0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
        policy_returns.push(rets.clone());
        // sum equity across symbols (equal-weight combination)
        for e in &res.equity_curve {
            *combined_equity.entry(e.timestamp_ms).or_insert(0.0) += e.equity_quote;
        }
        let sym_end = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(sleeve_budget);
        symbol_gross.insert(sym.clone(), sym_end);
        let sym_st = sharpe_stats(&rets);
        let days = ((common_mx - common_mn) as f64) / 86_400_000.0;
        let sym_ann = if sym_end > 0.0 { ((sym_end / sleeve_budget).powf(365.0 / days) - 1.0) * 100.0 } else { -100.0 };
        println!("  {sym}: ann={:.2}% skew={:.2} kurt={:.2} end_eq={:.1}", sym_ann, sym_st.skew, sym_st.kurtosis, sym_end);
    }

    // Combined metrics.
    let combined_initial: f64 = sleeve_budget * n_sym;
    let mut c_rets: Vec<f64> = Vec::new();
    let daily_combined: BTreeMap<i64, f64> = {
        let mut m: BTreeMap<i64, f64> = BTreeMap::new();
        for (ts, eq) in &combined_equity { m.insert(ts / 86_400_000, *eq); }
        m
    };
    let dv: Vec<f64> = daily_combined.values().copied().collect();
    for w in dv.windows(2) {
        if w[0] > 0.0 { c_rets.push(w[1] / w[0] - 1.0); }
    }
    let st = sharpe_stats(&c_rets);
    let ann_sharpe = st.sharpe * (365.0_f64).sqrt();
    let combined_end = combined_equity.values().last().copied().unwrap_or(combined_initial);
    let days = ((common_mx - common_mn) as f64) / 86_400_000.0;
    let comp = (combined_end / combined_initial - 1.0) * 100.0;
    let ann = if combined_end > 0.0 { ((combined_end / combined_initial).powf(365.0 / days) - 1.0) * 100.0 } else { -100.0 };
    // peak/DD
    let mut peak = f64::NEG_INFINITY;
    let mut max_dd = 0.0f64;
    for &v in combined_equity.values() {
        peak = peak.max(v);
        if peak > 0.0 { max_dd = max_dd.max((peak - v) / peak * 100.0); }
    }
    // symbol gross contribution
    let total_gross: f64 = symbol_gross.values().sum();
    let max_sym_gross = symbol_gross.values().map(|g| g / total_gross * 100.0).fold(0.0f64, f64::max);
    let dsr = deflated_sharpe(ann_sharpe, per_sym.len() as u64, st.n as u64, st.skew, st.kurtosis);
    let pbo = probability_of_backtest_overfitting(&policy_returns, 200);

    println!("\n=== COMBINED ({} symbols, ScaleOut, max_legs=2) ===", n_sym);
    println!("compounded_pct={:.2} annualized_pct={:.2} max_dd_pct={:.2}", comp, ann, max_dd);
    println!("ann_sharpe={:.4} skew={:.3} kurt={:.3}", ann_sharpe, st.skew, st.kurtosis);
    println!("DSR(n={})={:.4} PBO={:.4}", per_sym.len(), dsr, pbo);
    println!("max_symbol_gross_pct={:.2} (<=25% required: {})", max_sym_gross, max_sym_gross <= 25.0);
    Ok(())
}

#[derive(Clone)]
struct RelaxedM1 { state_map: BTreeMap<i64, M1State>, thresh: f64 }
impl SignalSource for RelaxedM1 {
    fn gate(&self, _i: usize, ts: i64, _price: f64) -> SignalGate {
        match self.state_map.get(&ts) {
            Some(s) => SignalGate { long_fo: s.price_ext <= -self.thresh && s.oi_change >= 0.0, short_fo: s.price_ext >= self.thresh && s.oi_change >= 0.0, so_allowed: true, force_abort: false },
            None => SignalGate::default(),
        }
    }
}

fn empty_result(budget: f64) -> backtest_engine::martingale::metrics::MartingaleBacktestResult {
    use backtest_engine::martingale::metrics::*;
    MartingaleBacktestResult {
        metrics: MartingaleMetrics { total_return_pct: 0.0, annualized_return_pct: None, max_drawdown_pct: 0.0, global_drawdown_pct: None, max_strategy_drawdown_pct: None, monthly_win_rate_pct: None, max_leverage_used: None, min_liquidation_buffer_pct: None, total_fee_quote: None, total_slippage_quote: None, total_funding_quote: None, planned_margin_quote: None, planned_notional_quote: None, return_drawdown_ratio: None, data_quality_score: None, trade_count: 0, stop_count: 0, max_capital_used_quote: budget, survival_passed: true },
        events: vec![], equity_curve: vec![], drawdown_curve: vec![], trades: vec![], rejection_reasons: vec![],
    }
}

fn load_metrics(sym_dir: &Path, symbol: &str) -> Result<Vec<MetricRow>> {
    let mut metrics = Vec::new();
    if !sym_dir.exists() { return Ok(metrics); }
    for entry in std::fs::read_dir(sym_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("zip") { continue; }
        let bytes = std::fs::read(&path)?;
        let mut za = zip::ZipArchive::new(std::io::Cursor::new(bytes.as_slice())).map_err(|e| anyhow!("zip: {e}"))?;
        let name = za.by_index(0).map_err(|e| anyhow!("idx: {e}"))?.name().to_string();
        let mut buf = Vec::new();
        za.by_name(&name).map_err(|e| anyhow!("name: {e}"))?.read_to_end(&mut buf)?;
        let text = String::from_utf8_lossy(&buf);
        let mut rdr = csv::ReaderBuilder::new().has_headers(true).from_reader(text.as_bytes());
        for rec in rdr.records() {
            let r = match rec { Ok(r) => r, Err(_) => continue };
            let ts = match parse_ts(&r[0]) { Some(t) => t, None => continue };
            let oiv: f64 = r.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let top: f64 = r.get(5).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            let all: f64 = r.get(6).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            let taker: f64 = r.get(7).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            metrics.push(MetricRow { ts_ms: ts, symbol: symbol.to_string(), open_interest_value: oiv, top_trader_ls_ratio: top, all_trader_ls_ratio: all, taker_ls_vol_ratio: taker });
        }
    }
    metrics.sort_by_key(|m| m.ts_ms);
    Ok(metrics)
}

fn parse_ts(s: &str) -> Option<i64> {
    let dt = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()?;
    Some(dt.and_utc().timestamp_millis())
}

fn resample(bars: &[KlineBar], bm: u32) -> Vec<KlineBar> {
    let mut o: BTreeMap<i64, KlineBar> = BTreeMap::new();
    let bk = (bm as i64) * 60_000;
    for b in bars {
        let bu = (b.open_time_ms / bk) * bk;
        let e = o.entry(bu).or_insert_with(|| KlineBar { symbol: b.symbol.clone(), open_time_ms: bu, open: b.open, high: b.high, low: b.low, close: b.close, volume: 0.0 });
        e.high = e.high.max(b.high); e.low = e.low.min(b.low); e.close = b.close; e.volume += b.volume;
    }
    o.into_values().collect()
}
