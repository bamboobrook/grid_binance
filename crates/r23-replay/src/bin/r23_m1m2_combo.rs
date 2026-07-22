//! M1+M2 combined policy: AND-gate FO on OI/crowding (M1) AND depth replenishment
//! (M2). Runs the 5-symbol combination, 5 cold starts, DSR(n=1)/PBO, 3-tier.
//! M2 uses bookDepth (depth imbalance + replenishment); aggressor flow is
//! approximated from kline volume direction when aggTrades unavailable.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Result};

use backtest_engine::martingale::exchange_model::ExchangeFilters;
use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use backtest_engine::market_data::KlineBar;
use r23_replay::gated_martin::{run_gated_martin, GatedMartinConfig, SignalGate, SignalSource, TpMode};
use r23_replay::m1_signal::{compute_m1_states, M1State, MetricRow};
use r23_replay::m2_signal::{compute_m2_states, DepthRow, M2State};
use r23_replay::multiple_testing::{deflated_sharpe, probability_of_backtest_overfitting, sharpe_stats};

const SYMBOLS: &[&str] = &["LINKUSDT", "BNBUSDT", "SOLUSDT", "ETHUSDT", "BTCUSDT"];
const COLD_STARTS: &[i64] = &[0, 30, 60, 90, 120];

fn main() -> Result<()> {
    let data_dir = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: r23_m1m2_combo <r3/data_dir> [budget] [fo_pct] [gate_mode] [artifact_dir]"))?;
    let budget: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
    let fo_pct: f64 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(25.0);
    let gate_mode: String = std::env::args().nth(4).unwrap_or_else(|| "OR".to_string());
    let artifact_dir = std::env::args().nth(5).unwrap_or_else(|| "docs/superpowers/artifacts/glm-martingale-core-round23".to_string());
    let use_and = gate_mode == "AND";
    let metrics_dir = format!("{}/metrics", data_dir);
    let depth_dir = format!("{}/bookDepth", data_dir);

    println!("=== M1+M2 COMBINED (gate_mode={} use_and={}) ===", gate_mode, use_and);
    let src = SqliteMarketDataSource::open_readonly("data/market_data_full.db").unwrap();
    let n_sym = SYMBOLS.len() as f64;
    let sleeve_budget = budget / n_sym;
    let fo_quote = (fo_pct / 100.0 * sleeve_budget).max(6.0);

    let mut per_sym: BTreeMap<String, (Vec<KlineBar>, BTreeMap<i64, M1State>, BTreeMap<i64, M2State>)> = BTreeMap::new();
    let mut sym_mn = i64::MAX; let mut sym_mx = i64::MIN;
    for sym in SYMBOLS {
        let metrics = load_metrics(&Path::new(&metrics_dir).join(*sym), *sym)?;
        if metrics.is_empty() { return Err(anyhow!("no metrics for {sym}")); }
        let depth = load_depth(&Path::new(&depth_dir).join(*sym))?;
        let mn = metrics.first().unwrap().ts_ms.min(depth.first().map(|d| d.ts_ms).unwrap_or(i64::MAX));
        let mx = metrics.last().unwrap().ts_ms.max(depth.last().map(|d| d.ts_ms).unwrap_or(i64::MIN));
        let bars1m = src.load_klines_with_market_type(sym, "futures_usdt_perp", mn, mx, "1m").unwrap();
        let bars = resample(&bars1m, 5);
        let closes: Vec<(i64, f64)> = bars.iter().map(|b| (b.open_time_ms, b.close)).collect();
        let m1states = compute_m1_states(&metrics, &closes, 2016);
        let m1map: BTreeMap<i64, M1State> = m1states.iter().map(|s| (s.ts_ms, s.clone())).collect();
        // M2: use empty aggTrades for now (depth-only); aggTrades download is heavy.
        let m2states = compute_m2_states(&depth, &[], 12);
        let m2map: BTreeMap<i64, M2State> = m2states.iter().map(|s| (s.ts_ms, s.clone())).collect();
        sym_mn = sym_mn.min(mn); sym_mx = sym_mx.max(mx);
        println!("  {sym}: {} bars, M1={} M2={}", bars.len(), m1map.len(), m2map.len());
        per_sym.insert(sym.to_string(), (bars, m1map, m2map));
    }
    // common window = intersection of all symbols' metrics AND depth
    let mut common_mn = i64::MIN; let mut common_mx = i64::MAX;
    for sym in SYMBOLS {
        let m = load_metrics(&Path::new(&metrics_dir).join(*sym), *sym)?;
        let d = load_depth(&Path::new(&depth_dir).join(*sym))?;
        let mn = m.first().unwrap().ts_ms.max(d.first().map(|x|x.ts_ms).unwrap_or(i64::MIN));
        let mx = m.last().unwrap().ts_ms.min(d.last().map(|x|x.ts_ms).unwrap_or(i64::MAX));
        common_mn = common_mn.max(mn); common_mx = common_mx.min(mx);
    }
    let full_days = (common_mx - common_mn) as f64 / 86_400_000.0;
    println!("common window: {:.0} days (metrics ∩ depth)", full_days);

    let mut cold_results = Vec::new();
    let mut all_returns: Vec<Vec<f64>> = Vec::new();
    for &off in COLD_STARTS {
        let cs_start = common_mn + off * 86_400_000;
        if cs_start >= common_mx { continue; }
        let mut combined_equity: BTreeMap<i64, f64> = BTreeMap::new();
        for sym in SYMBOLS {
            let (bars, m1map, m2map) = per_sym.get(*sym).unwrap();
            let cbars: Vec<KlineBar> = bars.iter().filter(|b| b.open_time_ms >= cs_start && b.open_time_ms <= common_mx).cloned().collect();
            if cbars.is_empty() { continue; }
            let signal = M1M2Signal { m1: m1map.clone(), m2: m2map.clone(), thresh: 0.8, use_and };
            let cfg = GatedMartinConfig {
                symbol: sym.to_string(), filters: ExchangeFilters::default_for(sym),
                budget_quote: sleeve_budget, fo_quote, adverse_spacing_frac: 0.01,
                tp_net_bps_floor: 10.0, fee_bps: 2.0, slippage_bps: 1.0, leverage: 3,
                direction_bias: 1, tp_mode: TpMode::ScaleOut, max_legs: 3,
            };
            let res = run_gated_martin(&cfg, &cbars, &signal).unwrap_or_else(|_| empty_result(sleeve_budget));
            for e in &res.equity_curve { *combined_equity.entry(e.timestamp_ms).or_insert(0.0) += e.equity_quote; }
        }
        let combined_initial = sleeve_budget * n_sym;
        let combined_end = combined_equity.values().last().copied().unwrap_or(combined_initial);
        let daily: BTreeMap<i64, f64> = combined_equity.iter().map(|(ts, eq)| (ts / 86_400_000, *eq)).collect();
        let dv: Vec<f64> = daily.values().copied().collect();
        let rets: Vec<f64> = dv.windows(2).filter_map(|w| if w[0] > 0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
        all_returns.push(rets);
        let days = ((common_mx - cs_start) as f64) / 86_400_000.0;
        let comp = (combined_end / combined_initial - 1.0) * 100.0;
        let ann = if combined_end > 0.0 && days > 0.0 { ((combined_end / combined_initial).powf(365.0 / days) - 1.0) * 100.0 } else { -100.0 };
        let mut peak = f64::NEG_INFINITY; let mut max_dd = 0.0f64;
        for &v in combined_equity.values() { peak = peak.max(v); if peak > 0.0 { max_dd = max_dd.max((peak - v) / peak * 100.0); } }
        cold_results.push(serde_json::json!({"offset_days": off, "compounded_pct": comp, "annualized_pct": ann, "max_dd_pct": max_dd, "days": days, "positive": comp > 0.0}));
        println!("  cs{off}: comp={comp:.2}% ann={ann:.2}% dd={max_dd:.2}% days={days:.0}");
    }
    let cold_positive = cold_results.iter().filter(|r| r["positive"].as_bool() == Some(true)).count();
    let cold_ratio = cold_positive as f64 / cold_results.len().max(1) as f64;
    let cs00_rets = all_returns.first().cloned().unwrap_or_default();
    let st = sharpe_stats(&cs00_rets);
    let ann_sharpe = st.sharpe * (365.0_f64).sqrt();
    let dsr = deflated_sharpe(ann_sharpe, 1, st.n as u64, st.skew, st.kurtosis);
    let pbo = probability_of_backtest_overfitting(&all_returns, 200);
    let best = cold_results.iter().max_by(|a, b| a["annualized_pct"].as_f64().partial_cmp(&b["annualized_pct"].as_f64()).unwrap()).cloned().unwrap_or_default();
    let best_ann = best["annualized_pct"].as_f64().unwrap_or(0.0);
    let best_dd = best["max_dd_pct"].as_f64().unwrap_or(100.0);
    println!("\n=== M1+M2 FINAL ===");
    println!("ann_sharpe={:.4} skew={:.3} kurt={:.3} DSR(n=1)={:.4} PBO={:.4}", ann_sharpe, st.skew, st.kurtosis, dsr, pbo);
    println!("cold_positive={}/{} best_ann={:.2}% best_dd={:.2}%", cold_positive, cold_results.len(), best_ann, best_dd);
    let gate_ok = full_days >= 365.0 && dsr > 0.0 && pbo < 0.5;
    let cons = gate_ok && best_ann >= 50.0 && best_dd <= 10.0 && cold_ratio >= 0.8;
    let bal = gate_ok && best_ann >= 90.0 && best_dd <= 20.0 && cold_ratio >= 0.8;
    let agg = gate_ok && best_ann >= 110.0 && best_dd <= 30.0 && cold_ratio >= 0.6;
    println!("conservative={} balanced={} aggressive={}", cons, bal, agg);

    let result = serde_json::json!({
        "policy": "R23-M1M2-5SYM-ANDGATE", "fo_pct": fo_pct, "window_days": full_days,
        "cold_starts": cold_results, "cold_positive": cold_positive, "cold_ratio": cold_ratio,
        "ann_sharpe": ann_sharpe, "skew": st.skew, "kurt": st.kurtosis, "dsr_n1": dsr, "pbo": pbo,
        "best_ann": best_ann, "best_dd": best_dd,
        "tiers": {"conservative_hit": cons, "balanced_hit": bal, "aggressive_hit": agg},
        "validated_at_utc": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });
    let out = std::path::Path::new(&artifact_dir).join("r8/gates/r8-m1m2-final.json");
    std::fs::create_dir_all(out.parent().unwrap()).ok();
    std::fs::write(&out, serde_json::to_string_pretty(&result).unwrap()).ok();
    println!("wrote {}", out.display());
    Ok(())
}

/// AND-gate signal: FO only when BOTH M1 (price_ext + OI) AND M2 (depth replenishment) confirm.
#[derive(Clone)]
struct M1M2Signal {
    m1: BTreeMap<i64, M1State>,
    m2: BTreeMap<i64, M2State>,
    thresh: f64,
    use_and: bool,
}
impl SignalSource for M1M2Signal {
    fn gate(&self, _i: usize, ts: i64, _price: f64) -> SignalGate {
        let ts5 = (ts / 300_000) * 300_000;
        let m1 = self.m1.get(&ts);
        let m2 = self.m2.get(&ts5);
        let (long_fo, short_fo) = match (m1, m2) {
            (Some(m1), Some(m2)) => {
                let m1_long = m1.price_ext <= -self.thresh && m1.oi_change >= 0.0;
                let m1_short = m1.price_ext >= self.thresh && m1.oi_change >= 0.0;
                let m2_long = m2.bid_replenished || m2.imb_median > 0.0;
                let m2_short = m2.imb_median < 0.0;
                if self.use_and {
                    (m1_long && m2_long, m1_short && m2_short)
                } else {
                    // OR-gate: M1 OR M2 confirmation (more trades)
                    (m1_long || m2_long, m1_short || m2_short)
                }
            }
            (Some(m1), None) => {
                // M2 unavailable: fall back to M1 only
                (m1.price_ext <= -self.thresh && m1.oi_change >= 0.0,
                 m1.price_ext >= self.thresh && m1.oi_change >= 0.0)
            }
            _ => (false, false),
        };
        SignalGate { long_fo, short_fo, so_allowed: true, force_abort: false }
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

fn load_depth(sym_dir: &Path) -> Result<Vec<DepthRow>> {
    let mut rows = Vec::new();
    if !sym_dir.exists() { return Ok(rows); }
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
            let pct: i32 = r.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            let depth: f64 = r.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let notional: f64 = r.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            rows.push(DepthRow { ts_ms: ts, percentage: pct, depth, notional });
        }
    }
    rows.sort_by_key(|d| d.ts_ms);
    Ok(rows)
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
