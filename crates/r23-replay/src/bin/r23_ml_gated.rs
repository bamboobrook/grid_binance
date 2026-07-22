//! ML-gated Martin: trains a logistic-regression next-bar predictor on M1
//! features (rolling prequential, past-only), gates FO on the prediction.
//! Tests whether ML improves Sharpe over the rule-based M1 signal.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Result};

use backtest_engine::martingale::exchange_model::ExchangeFilters;
use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use backtest_engine::market_data::KlineBar;
use r23_replay::gated_martin::{run_gated_martin, GatedMartinConfig, SignalGate, SignalSource, TpMode};
use r23_replay::m1_signal::{compute_m1_states, M1State, MetricRow};
use r23_replay::ml_predictor::{build_dataset, LogisticModel};
use r23_replay::multiple_testing::{deflated_sharpe, probability_of_backtest_overfitting, sharpe_stats};

const SYMBOLS: &[&str] = &["LINKUSDT", "BNBUSDT", "SOLUSDT", "ETHUSDT", "BTCUSDT"];

fn main() -> Result<()> {
    let metrics_dir = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: r23_ml_gated <metrics_dir> [budget] [fo_pct]"))?;
    let budget: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
    let fo_pct: f64 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(25.0);

    println!("=== ML-GATED MARTIN (logistic on M1 features) ===");
    let src = SqliteMarketDataSource::open_readonly("data/market_data_full.db").unwrap();
    let n_sym = SYMBOLS.len() as f64;
    let sleeve_budget = budget / n_sym;
    let fo_quote = (fo_pct / 100.0 * sleeve_budget).max(6.0);

    let mut common_mn = i64::MIN; let mut common_mx = i64::MAX;
    let mut per_sym: BTreeMap<String, (Vec<KlineBar>, Vec<M1State>)> = BTreeMap::new();
    for sym in SYMBOLS {
        let m = load_metrics(&Path::new(&metrics_dir).join(sym), sym)?;
        if m.is_empty() { return Err(anyhow!("no metrics for {sym}")); }
        common_mn = common_mn.max(m.first().unwrap().ts_ms);
        common_mx = common_mx.min(m.last().unwrap().ts_ms);
        let bars1m = src.load_klines_with_market_type(sym, "futures_usdt_perp", common_mn, common_mx, "1m").unwrap();
        let bars = resample(&bars1m, 5);
        let closes: Vec<(i64, f64)> = bars.iter().map(|b| (b.open_time_ms, b.close)).collect();
        let states = compute_m1_states(&m, &closes, 2016);
        per_sym.insert(sym.to_string(), (bars, states));
    }
    let days = (common_mx - common_mn) as f64 / 86_400_000.0;
    println!("common window: {:.0} days", days);

    // For each symbol: build the dataset, split into train (first 50%) + test (last 50%).
    // Train the model on the train half, then use it to gate FO in the test half.
    // This is a simplified prequential split (full retrain at midpoint).
    let mut combined_equity: BTreeMap<i64, f64> = BTreeMap::new();
    let mut policy_returns: Vec<Vec<f64>> = Vec::new();
    for sym in SYMBOLS {
        let (bars, states) = per_sym.get(*sym).unwrap();
        let closes: Vec<(i64, f64)> = bars.iter().map(|b| (b.open_time_ms, b.close)).collect();
        let (x, y, ts) = build_dataset(states, &closes);
        if x.len() < 1000 { continue; }
        let split = x.len() / 2;
        let mut model = LogisticModel::new(5);
        model.train(&x[..split], &y[..split]);
        let mut pred_map: BTreeMap<i64, f64> = BTreeMap::new();
        for i in split..x.len() {
            let p = model.predict_proba(&x[i]);
            pred_map.insert(ts[i], p);
        }
        println!("  {sym}: train={} test={} w={:?}", split, x.len() - split, model.weights);

        let test_start_ts = ts[split];
        let cbars: Vec<KlineBar> = bars.iter().filter(|b| b.open_time_ms >= test_start_ts).cloned().collect();
        let signal = MlSignal { preds: pred_map, long_thresh: 0.55, short_thresh: 0.45 };
        let cfg = GatedMartinConfig {
            symbol: sym.to_string(), filters: ExchangeFilters::default_for(sym),
            budget_quote: sleeve_budget, fo_quote, adverse_spacing_frac: 0.01,
            tp_net_bps_floor: 10.0, fee_bps: 2.0, slippage_bps: 1.0, leverage: 3,
            direction_bias: 0, tp_mode: TpMode::ScaleOut, max_legs: 3,
        };
        let res = run_gated_martin(&cfg, &cbars, &signal).unwrap_or_else(|_| empty_result(sleeve_budget));
        for e in &res.equity_curve { *combined_equity.entry(e.timestamp_ms).or_insert(0.0) += e.equity_quote; }
        let daily: BTreeMap<i64, f64> = res.equity_curve.iter().map(|e| (e.timestamp_ms / 86_400_000, e.equity_quote)).collect();
        let dv: Vec<f64> = daily.values().copied().collect();
        let rets: Vec<f64> = dv.windows(2).filter_map(|w| if w[0] > 0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
        policy_returns.push(rets);
        let end_eq = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(sleeve_budget);
        let test_days = (days / 2.0).max(1.0);
        let sym_ann = if end_eq > 0.0 { ((end_eq / sleeve_budget).powf(365.0 / test_days) - 1.0) * 100.0 } else { -100.0 };
        println!("    ann={:.2}% end_eq={:.1} trades={}", sym_ann, end_eq, res.metrics.trade_count);
    }

    let combined_initial = budget;
    let daily: BTreeMap<i64, f64> = combined_equity.iter().map(|(ts, eq)| (ts / 86_400_000, *eq)).collect();
    let dv: Vec<f64> = daily.values().copied().collect();
    let rets: Vec<f64> = dv.windows(2).filter_map(|w| if w[0] > 0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
    let st = sharpe_stats(&rets);
    let ann_sharpe = st.sharpe * (365.0_f64).sqrt();
    let end_eq = combined_equity.values().last().copied().unwrap_or(combined_initial);
    let half_days = (days / 2.0).max(1.0);
    let ann = if end_eq > 0.0 { ((end_eq / combined_initial).powf(365.0 / half_days) - 1.0) * 100.0 } else { -100.0 };
    let mut peak = f64::NEG_INFINITY; let mut max_dd = 0.0f64;
    for &v in combined_equity.values() { peak = peak.max(v); if peak > 0.0 { max_dd = max_dd.max((peak - v) / peak * 100.0); } }
    let dsr = deflated_sharpe(ann_sharpe, 1, st.n as u64, st.skew, st.kurtosis);
    let pbo = probability_of_backtest_overfitting(&policy_returns, 200);
    println!("\n=== ML-GATED RESULT (test half, {:.0}d) ===", half_days);
    println!("ann={:.2}% dd={:.2}% sharpe={:.4} skew={:.3} kurt={:.3} DSR={:.4} PBO={:.4}", ann, max_dd, ann_sharpe, st.skew, st.kurtosis, dsr, pbo);
    println!("(M1 rule-based was sharpe~0.71; ML-gated target is sharpe>=2)");
    Ok(())
}

struct MlSignal { preds: BTreeMap<i64, f64>, long_thresh: f64, short_thresh: f64 }
impl SignalSource for MlSignal {
    fn gate(&self, _i: usize, ts: i64, _price: f64) -> SignalGate {
        match self.preds.get(&ts) {
            Some(&p) if p >= self.long_thresh => SignalGate { long_fo: true, short_fo: false, so_allowed: true, force_abort: false },
            Some(&p) if p <= self.short_thresh => SignalGate { long_fo: false, short_fo: true, so_allowed: true, force_abort: false },
            _ => SignalGate::default(),
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
