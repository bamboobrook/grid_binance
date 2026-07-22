//! TP-mode sweep to reduce return kurtosis and raise DSR.
//!
//! Runs M1 BTC gated-Martin under each TP mode (Fixed/Early/ScaleOut/TimeDecay)
//! and reports kurtosis, skew, Sharpe, DSR for each. The goal is to find the TP
//! config with the smoothest return profile (lowest kurtosis) that keeps the
//! edge, so DSR > 0.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Result};

use backtest_engine::martingale::exchange_model::ExchangeFilters;
use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use backtest_engine::market_data::KlineBar;
use r23_replay::gated_martin::{run_gated_martin, GatedMartinConfig, SignalGate, SignalSource, TpMode};
use r23_replay::m1_signal::{compute_m1_states, M1State, MetricRow};
use r23_replay::multiple_testing::{deflated_sharpe, sharpe_stats};

fn main() -> Result<()> {
    let metrics_dir = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: r23_tp_sweep <metrics_dir> [symbol] [budget]"))?;
    let symbol = std::env::args().nth(2).unwrap_or_else(|| "BTCUSDT".to_string());
    let budget: f64 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(1000.0);

    let mdir = Path::new(&metrics_dir).join(&symbol);
    let metrics = load_metrics(&mdir, &symbol)?;
    if metrics.is_empty() { return Err(anyhow!("no metrics for {symbol}")); }
    let src = SqliteMarketDataSource::open_readonly("data/market_data_full.db").unwrap();
    let mn = metrics.first().unwrap().ts_ms;
    let mx = metrics.last().unwrap().ts_ms;
    let bars1m = src.load_klines_with_market_type(&symbol, "futures_usdt_perp", mn, mx, "1m").unwrap();
    let bars = resample(&bars1m, 5);
    let closes: Vec<(i64, f64)> = bars.iter().map(|b| (b.open_time_ms, b.close)).collect();
    let states = compute_m1_states(&metrics, &closes, 2016);
    let sm: BTreeMap<i64, M1State> = states.iter().map(|s| (s.ts_ms, s.clone())).collect();
    println!("{symbol}: {} bars, {} states, {:.0} days", bars.len(), sm.len(),
        ((mx - mn) as f64) / 86_400_000.0);

    let signal = RelaxedM1 { state_map: sm };

    let tp_modes: Vec<(&str, TpMode)> = vec![
        ("Fixed", TpMode::Fixed),
        ("Early_half_floor", TpMode::Early),
        ("ScaleOut", TpMode::ScaleOut),
        ("TimeDecay_500", TpMode::TimeDecay { decay_bars: 500 }),
        ("TimeDecay_200", TpMode::TimeDecay { decay_bars: 200 }),
        ("TimeDecay_100", TpMode::TimeDecay { decay_bars: 100 }),
    ];

    println!("\nmode,max_legs,ann_sharpe,skew,kurt,dsr(n_trials),compounded_pct,annualized_pct,max_dd_pct,trades");
    let mut n_trials = 0u64;
    for (label, mode) in &tp_modes {
        for &ml in &[1u32, 2, 3, 4] {
            let cfg = GatedMartinConfig {
                symbol: symbol.clone(),
                filters: ExchangeFilters::default_for(&symbol),
                budget_quote: budget,
                fo_quote: (0.10 * budget).max(6.0),
                adverse_spacing_frac: 0.01,
                tp_net_bps_floor: 10.0,
                fee_bps: 2.0,
                slippage_bps: 1.0,
                leverage: 3,
                direction_bias: 1,
                tp_mode: *mode,
                max_legs: ml,
            };
            let res = run_gated_martin(&cfg, &bars, &signal).unwrap_or_else(|_| empty_result(budget));
            let mut daily: BTreeMap<i64, f64> = BTreeMap::new();
            for e in &res.equity_curve { daily.insert(e.timestamp_ms / 86_400_000, e.equity_quote); }
            let rets: Vec<f64> = daily.values().collect::<Vec<_>>().windows(2).filter_map(|w| if w[0] > &0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
            let st = sharpe_stats(&rets);
            let ann_sharpe = st.sharpe * (365.0_f64).sqrt();
            n_trials += 1;
            let dsr = deflated_sharpe(ann_sharpe, n_trials, st.n as u64, st.skew, st.kurtosis);
            let ee = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(budget);
            let comp = (ee / budget - 1.0) * 100.0;
            let days = ((mx - mn) as f64) / 86_400_000.0;
            let ann = if ee > 0.0 { ((ee / budget).powf(365.0 / days) - 1.0) * 100.0 } else { -100.0 };
            let dd = res.drawdown_curve.iter().map(|d| d.drawdown_pct).fold(0.0f64, f64::max);
            println!("{label},{ml},{:.4},{:.3},{:.3},{:.4},{:.2},{:.2},{:.2},{}", ann_sharpe, st.skew, st.kurtosis, dsr, comp, ann, dd, res.metrics.trade_count);
        }
    }
    Ok(())
}

#[derive(Clone)]
struct RelaxedM1 { state_map: BTreeMap<i64, M1State> }
impl SignalSource for RelaxedM1 {
    fn gate(&self, _i: usize, ts: i64, _price: f64) -> SignalGate {
        match self.state_map.get(&ts) {
            Some(s) => SignalGate { long_fo: s.price_ext <= -1.5 && s.oi_change >= 0.0, short_fo: s.price_ext >= 1.5 && s.oi_change >= 0.0, so_allowed: true, force_abort: false },
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
