//! R23 multi-symbol grid sweep — tests combinations of symbols with the
//! newly-discovered good params (adverse=0.010, tp_bps=5, thresh=1.5) from the
//! single-symbol grid sweep, sweeping adverse × thresh × symbol-subset to find
//! a combination reaching Sharpe >= 2.0 / ann >= 50%.
//!
//! This is the critical test: if 5+ symbols each at Sharpe ~1.0 diversify, the
//! combined Sharpe could reach the target tier.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Result};

use backtest_engine::martingale::exchange_model::ExchangeFilters;
use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use backtest_engine::market_data::KlineBar;
use r23_replay::gated_martin::{run_gated_martin, GatedMartinConfig, SignalSource, TpMode};
use r23_replay::m1_signal::{compute_m1_states, M1State, MetricRow};
use r23_replay::multiple_testing::{deflated_sharpe, probability_of_backtest_overfitting, sharpe_stats};

struct RelaxedM1 {
    state_map: BTreeMap<i64, M1State>,
    thresh: f64,
}

impl SignalSource for RelaxedM1 {
    fn gate(&self, bar_idx: usize, ts_ms: i64, price: f64) -> r23_replay::gated_martin::SignalGate {
        let _ = bar_idx;
        let s = match self.state_map.range(..=ts_ms).next_back() {
            Some((_, v)) if (ts_ms - v.ts_ms) < 600_000 => v,
            _ => return r23_replay::gated_martin::SignalGate { long_fo: false, short_fo: false, so_allowed: false, force_abort: false },
        };
        let long_fo = s.price_ext <= -self.thresh;
        let short_fo = s.price_ext >= self.thresh;
        let exhaustion = s.top_crowd > 0.5 || s.all_crowd > 0.5;
        r23_replay::gated_martin::SignalGate { long_fo, short_fo, so_allowed: exhaustion, force_abort: false }
    }
}

fn parse_ts(s: &str) -> Option<i64> {
    let dt = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()?;
    Some(dt.and_utc().timestamp_millis())
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

fn resample(bars1m: &[KlineBar], bm: u32) -> Vec<KlineBar> {
    let mut o: BTreeMap<i64, KlineBar> = BTreeMap::new();
    let bk = (bm as i64) * 60_000;
    for b in bars1m {
        let bu = (b.open_time_ms / bk) * bk;
        let e = o.entry(bu).or_insert_with(|| KlineBar { symbol: b.symbol.clone(), open_time_ms: bu, open: b.open, high: b.high, low: b.low, close: b.close, volume: 0.0 });
        e.high = e.high.max(b.high); e.low = e.low.min(b.low); e.close = b.close; e.volume += b.volume;
    }
    o.into_values().collect()
}

#[derive(Debug, Clone)]
struct ComboResult {
    n_sym: usize,
    symbols: String,
    adverse: f64,
    tp_bps: f64,
    thresh: f64,
    fo_pct: f64,
    max_legs: u32,
    ann_pct: f64,
    dd_pct: f64,
    sharpe: f64,
    skew: f64,
    kurt: f64,
    dsr: f64,
    pbo: f64,
    max_sym_gross: f64,
    days: f64,
}

fn run_combo(
    symbols: &[String],
    per_sym: &BTreeMap<String, (Vec<KlineBar>, BTreeMap<i64, M1State>, i64, i64)>,
    budget: f64,
    adverse: f64,
    tp_bps: f64,
    thresh: f64,
    fo_pct: f64,
    max_legs: u32,
    common_mn: i64,
    common_mx: i64,
) -> Option<ComboResult> {
    let n_sym = symbols.len();
    let sleeve_budget = budget / n_sym as f64;
    let fo_quote = (fo_pct / 100.0 * sleeve_budget).max(6.0);
    let mut combined_equity: BTreeMap<i64, f64> = BTreeMap::new();
    let mut symbol_gross: BTreeMap<String, f64> = BTreeMap::new();
    let mut policy_returns: Vec<Vec<f64>> = Vec::new();
    for sym in symbols {
        let (bars, sm, _, _) = per_sym.get(sym)?;
        let cbars: Vec<KlineBar> = bars.iter().filter(|b| b.open_time_ms >= common_mn && b.open_time_ms <= common_mx).cloned().collect();
        let signal = RelaxedM1 { state_map: sm.clone(), thresh };
        let cfg = GatedMartinConfig {
            symbol: sym.clone(),
            filters: ExchangeFilters::default_for(sym),
            budget_quote: sleeve_budget,
            fo_quote,
            adverse_spacing_frac: adverse,
            tp_net_bps_floor: tp_bps,
            fee_bps: 2.0,
            slippage_bps: 1.0,
            leverage: 3,
            direction_bias: 1,
            tp_mode: TpMode::ScaleOut,
            max_legs,
        };
        let res = run_gated_martin(&cfg, &cbars, &signal).ok()?;
        let mut daily: BTreeMap<i64, f64> = BTreeMap::new();
        for e in &res.equity_curve { daily.insert(e.timestamp_ms / 86_400_000, e.equity_quote); }
        let rets: Vec<f64> = daily.values().collect::<Vec<_>>().windows(2).filter_map(|w| if w[0] > &0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
        policy_returns.push(rets);
        for e in &res.equity_curve {
            *combined_equity.entry(e.timestamp_ms).or_insert(0.0) += e.equity_quote;
        }
        let sym_end = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(sleeve_budget);
        symbol_gross.insert(sym.clone(), sym_end);
    }
    let combined_initial = sleeve_budget * n_sym as f64;
    let daily_combined: BTreeMap<i64, f64> = combined_equity.iter().map(|(ts, eq)| (ts / 86_400_000, *eq)).collect();
    let dv: Vec<f64> = daily_combined.values().copied().collect();
    let c_rets: Vec<f64> = dv.windows(2).filter_map(|w| if w[0] > 0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
    if c_rets.len() < 30 { return None; }
    let st = sharpe_stats(&c_rets);
    let ann_sharpe = st.sharpe * (365.0_f64).sqrt();
    let combined_end = *combined_equity.values().last().unwrap_or(&combined_initial);
    let days = ((common_mx - common_mn) as f64) / 86_400_000.0;
    let ann = if combined_end > 0.0 { ((combined_end / combined_initial).powf(365.0 / days) - 1.0) * 100.0 } else { -100.0 };
    let mut peak = f64::NEG_INFINITY;
    let mut max_dd = 0.0f64;
    for &v in combined_equity.values() {
        peak = peak.max(v);
        if peak > 0.0 { max_dd = max_dd.max((peak - v) / peak * 100.0); }
    }
    let total_gross: f64 = symbol_gross.values().sum();
    let max_sym_gross = symbol_gross.values().map(|g| g / total_gross * 100.0).fold(0.0f64, f64::max);
    let dsr = deflated_sharpe(ann_sharpe, 1, st.n as u64, st.skew, st.kurtosis);
    let pbo = probability_of_backtest_overfitting(&policy_returns, 100);
    Some(ComboResult {
        n_sym, symbols: symbols.join("+"),
        adverse, tp_bps, thresh, fo_pct, max_legs,
        ann_pct: ann, dd_pct: max_dd, sharpe: ann_sharpe,
        skew: st.skew, kurt: st.kurtosis, dsr, pbo,
        max_sym_gross, days,
    })
}

fn main() -> Result<()> {
    let metrics_dir = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: r23_multi_grid <metrics_dir> [budget]"))?;
    let budget: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
    let all_symbols = ["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "LINKUSDT", "XRPUSDT", "DOGEUSDT", "LTCUSDT"];

    let src = SqliteMarketDataSource::open_readonly("data/market_data.full.db")
        .or_else(|_| SqliteMarketDataSource::open_readonly("data/market_data_full.db"))
        .map_err(|e| anyhow!("{e}"))?;

    // Load all symbols once.
    let mut per_sym: BTreeMap<String, (Vec<KlineBar>, BTreeMap<i64, M1State>, i64, i64)> = BTreeMap::new();
    for sym in &all_symbols {
        let mdir = Path::new(&metrics_dir).join(sym);
        let metrics = load_metrics(&mdir, sym)?;
        if metrics.is_empty() { println!("WARN: no metrics for {sym}"); continue; }
        let mn = metrics.first().unwrap().ts_ms;
        let mx = metrics.last().unwrap().ts_ms;
        let bars1m = src.load_klines_with_market_type(sym, "futures_usdt_perp", mn, mx, "1m").map_err(|e| anyhow!("{e}"))?;
        let bars = resample(&bars1m, 5);
        let nbars = bars.len();
        let closes: Vec<(i64, f64)> = bars.iter().map(|b| (b.open_time_ms, b.close)).collect();
        let states = compute_m1_states(&metrics, &closes, 2016);
        let sm: BTreeMap<i64, M1State> = states.iter().map(|s| (s.ts_ms, s.clone())).collect();
        let nstates = sm.len();
        per_sym.insert(sym.to_string(), (bars, sm, mn, mx));
        println!("loaded {sym}: {} bars, {} states", nbars, nstates);
    }

    // Common window across ALL loaded symbols.
    let mut common_mn = i64::MIN;
    let mut common_mx = i64::MAX;
    for (_, _, mn, mx) in per_sym.values() { common_mn = common_mn.max(*mn); common_mx = common_mx.min(*mx); }
    println!("common window: {:.0}d", (common_mx - common_mn) as f64 / 86_400_000.0);

    let loaded: Vec<String> = per_sym.keys().cloned().collect();

    // Grid: adverse × thresh × tp_bps × fo_pct × max_legs × symbol-subsets
    let adverses = [0.005f64, 0.010, 0.020];
    let threshes = [1.0f64, 1.5, 2.0];
    let tp_bps_arr = [5.0f64, 10.0, 20.0];
    let fo_pcts = [35.0f64, 50.0];
    let max_legs_arr = [2u32, 3];

    // Symbol subsets to test (sizes 2, 3, 5, 8).
    let subsets: Vec<Vec<String>> = vec![
        vec!["BTCUSDT".into(), "ETHUSDT".into()],
        vec!["BTCUSDT".into(), "ETHUSDT".into(), "SOLUSDT".into()],
        vec!["BTCUSDT".into(), "ETHUSDT".into(), "SOLUSDT".into(), "BNBUSDT".into(), "LINKUSDT".into()],
        loaded.clone(),
    ];

    let total = adverses.len() * threshes.len() * tp_bps_arr.len() * fo_pcts.len() * max_legs_arr.len() * subsets.len();
    println!("Grid: {} combos", total);

    let mut results: Vec<ComboResult> = Vec::new();
    let mut done = 0usize;
    let mut best_sharpe = -100.0f64;

    for &adverse in &adverses {
        for &thresh in &threshes {
            for &tp_bps in &tp_bps_arr {
                for &fo_pct in &fo_pcts {
                    for &ml in &max_legs_arr {
                        for syms in &subsets {
                            if let Some(r) = run_combo(syms, &per_sym, budget, adverse, tp_bps, thresh, fo_pct, ml, common_mn, common_mx) {
                                if r.sharpe > best_sharpe { best_sharpe = r.sharpe; }
                                results.push(r);
                            }
                            done += 1;
                            if done % 20 == 0 {
                                println!("  ...{}/{} combos, best_sharpe={:.3}", done, total, best_sharpe);
                            }
                        }
                    }
                }
            }
        }
    }

    println!("\n=== MULTI-SYMBOL GRID COMPLETE: {} combos ===", results.len());
    println!("\n--- TOP 25 BY ann_sharpe ---");
    println!("rank,n_sym,adverse,tp_bps,thresh,fo,legs,ann_pct,dd_pct,sharpe,skew,kurt,dsr,pbo,max_sym_gross");
    results.sort_by(|a, b| b.sharpe.partial_cmp(&a.sharpe).unwrap_or(std::cmp::Ordering::Equal));
    for (i, r) in results.iter().take(25).enumerate() {
        println!("{},{},{:.0},{:.0},{:.1},{:.0},{},{:.2},{:.2},{:.3},{:.3},{:.3},{:.2},{:.3},{:.2}",
            i+1, r.n_sym, r.adverse, r.tp_bps, r.thresh, r.fo_pct, r.max_legs,
            r.ann_pct, r.dd_pct, r.sharpe, r.skew, r.kurt, r.dsr, r.pbo, r.max_sym_gross);
    }

    println!("\n--- TIER CHECK ---");
    let mut hits = 0;
    for r in &results {
        if r.ann_pct >= 50.0 && r.dd_pct <= 10.0 && r.sharpe >= 2.0 && r.pbo < 0.5 && r.max_sym_gross <= 25.0 {
            println!("CONSERVATIVE HIT: {} sym adv={:.3} bps={:.0} th={:.1} fo={:.0} legs={} -> ann={:.2}% dd={:.2}% sh={:.3} pbo={:.3} gross={:.2}",
                r.n_sym, r.adverse, r.tp_bps, r.thresh, r.fo_pct, r.max_legs, r.ann_pct, r.dd_pct, r.sharpe, r.pbo, r.max_sym_gross);
            hits += 1;
        }
        if r.ann_pct >= 90.0 && r.dd_pct <= 20.0 && r.sharpe >= 2.0 && r.pbo < 0.5 && r.max_sym_gross <= 25.0 {
            println!("BALANCED HIT: {} sym adv={:.3} bps={:.0} th={:.1} fo={:.0} legs={} -> ann={:.2}% dd={:.2}% sh={:.3} pbo={:.3} gross={:.2}",
                r.n_sym, r.adverse, r.tp_bps, r.thresh, r.fo_pct, r.max_legs, r.ann_pct, r.dd_pct, r.sharpe, r.pbo, r.max_sym_gross);
            hits += 1;
        }
        if r.ann_pct >= 110.0 && r.dd_pct <= 30.0 && r.sharpe >= 2.0 && r.pbo < 0.5 && r.max_sym_gross <= 25.0 {
            println!("AGGRESSIVE HIT: {} sym adv={:.3} bps={:.0} th={:.1} fo={:.0} legs={} -> ann={:.2}% dd={:.2}% sh={:.3} pbo={:.3} gross={:.2}",
                r.n_sym, r.adverse, r.tp_bps, r.thresh, r.fo_pct, r.max_legs, r.ann_pct, r.dd_pct, r.sharpe, r.pbo, r.max_sym_gross);
            hits += 1;
        }
    }
    if hits == 0 {
        let best = results.first();
        println!("No tier hit. Best: sh={:.3} ann={:.2}% dd={:.2}% pbo={:.3}",
            best.map(|r| r.sharpe).unwrap_or(0.0),
            best.map(|r| r.ann_pct).unwrap_or(0.0),
            best.map(|r| r.dd_pct).unwrap_or(0.0),
            best.map(|r| r.pbo).unwrap_or(0.0));
    }

    // Write artifact.
    let out_path = "docs/superpowers/artifacts/glm-martingale-core-round23/r8/gates/multi-grid-sweep.jsonl";
    if let Some(parent) = Path::new(out_path).parent() { std::fs::create_dir_all(parent)?; }
    let mut f = std::fs::File::create(out_path)?;
    use std::io::Write;
    for r in &results {
        let row = serde_json::json!({
            "n_sym": r.n_sym, "symbols": r.symbols,
            "adverse": r.adverse, "tp_bps": r.tp_bps, "thresh": r.thresh,
            "fo_pct": r.fo_pct, "max_legs": r.max_legs,
            "ann_pct": r.ann_pct, "dd_pct": r.dd_pct, "sharpe": r.sharpe,
            "skew": r.skew, "kurt": r.kurt, "dsr": r.dsr, "pbo": r.pbo,
            "max_sym_gross": r.max_sym_gross, "days": r.days,
        });
        writeln!(f, "{}", row)?;
    }
    println!("\nWrote {} combos to {}", results.len(), out_path);
    Ok(())
}
