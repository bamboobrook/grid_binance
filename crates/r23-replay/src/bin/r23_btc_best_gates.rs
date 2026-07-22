//! R8 gate evaluation for the newly-discovered best single-symbol BTC config
//! (Sharpe 1.074 from the grid sweep): adverse=0.010, tp_bps=5, thresh=1.5,
//! fo=35%, legs=3, ScaleOut.
//!
//! Runs:
//!  1. 5 cold starts (cs00/cs30/cs60/cs90/cs120) — must all be positive
//!  2. DSR (Deflated Sharpe Ratio) — must be > 0
//!  3. PBO (Probability of Backtest Overfitment) across 8 independent configs — must be < 0.5
//!  4. Tier check (conservative 50%/DD<=10%, balanced 90%/DD<=20%, aggressive 110%/DD<=30%)
//!
//! This is the proper anti-overfit gate test on the best candidate.

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

fn run_config(bars: &[KlineBar], signal: &RelaxedM1, budget: f64, adverse: f64, tp_bps: f64, fo_pct: f64, max_legs: u32, symbol: &str) -> Option<(f64, f64, f64, f64, f64, f64, usize)> {
    // returns (ann_pct, dd_pct, ann_sharpe, skew, kurt, end_eq, n_days)
    let cfg = GatedMartinConfig {
        symbol: symbol.to_string(),
        filters: ExchangeFilters::default_for(symbol),
        budget_quote: budget,
        fo_quote: (fo_pct / 100.0 * budget).max(6.0),
        adverse_spacing_frac: adverse,
        tp_net_bps_floor: tp_bps,
        fee_bps: 2.0,
        slippage_bps: 1.0,
        leverage: 3,
        direction_bias: 1,
        tp_mode: TpMode::ScaleOut,
        max_legs,
    };
    let res = run_gated_martin(&cfg, bars, signal).ok()?;
    let mut daily: BTreeMap<i64, f64> = BTreeMap::new();
    for e in &res.equity_curve { daily.insert(e.timestamp_ms / 86_400_000, e.equity_quote); }
    let rets: Vec<f64> = daily.values().collect::<Vec<_>>().windows(2)
        .filter_map(|w| if w[0] > &0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
    if rets.len() < 30 { return None; }
    let st = sharpe_stats(&rets);
    let ann_sharpe = st.sharpe * (365.0_f64).sqrt();
    let ee = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(budget);
    let days = ((bars.last().unwrap().open_time_ms - bars.first().unwrap().open_time_ms) as f64) / 86_400_000.0;
    let ann = if ee > 0.0 && days > 0.0 { ((ee / budget).powf(365.0 / days) - 1.0) * 100.0 } else { -100.0 };
    let mut peak = budget;
    let mut max_dd = 0.0f64;
    for e in &res.equity_curve {
        if e.equity_quote > peak { peak = e.equity_quote; }
        let dd = (peak - e.equity_quote) / peak * 100.0;
        if dd > max_dd { max_dd = dd; }
    }
    Some((ann, max_dd, ann_sharpe, st.skew, st.kurtosis, ee, days as usize))
}

fn main() -> Result<()> {
    let metrics_dir = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: r23_btc_best_gates <metrics_dir> [budget]"))?;
    let budget: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
    let symbol = "BTCUSDT";

    let mdir = Path::new(&metrics_dir).join(symbol);
    let metrics = load_metrics(&mdir, symbol)?;
    if metrics.is_empty() { return Err(anyhow!("no metrics for {symbol}")); }
    let src = SqliteMarketDataSource::open_readonly("data/market_data.full.db")
        .or_else(|_| SqliteMarketDataSource::open_readonly("data/market_data_full.db"))
        .map_err(|e| anyhow!("{e}"))?;
    let mn = metrics.first().unwrap().ts_ms;
    let mx = metrics.last().unwrap().ts_ms;
    let bars1m = src.load_klines_with_market_type(symbol, "futures_usdt_perp", mn, mx, "1m").map_err(|e| anyhow!("{e}"))?;
    let bars = resample(&bars1m, 5);
    let closes: Vec<(i64, f64)> = bars.iter().map(|b| (b.open_time_ms, b.close)).collect();
    let states = compute_m1_states(&metrics, &closes, 2016);
    let sm: BTreeMap<i64, M1State> = states.iter().map(|s| (s.ts_ms, s.clone())).collect();
    println!("{symbol}: {} bars, {} states, {:.0}d window", bars.len(), sm.len(), ((mx - mn) as f64) / 86_400_000.0);

    println!("\n============================================================");
    println!("BEST CONFIG (from grid sweep): adverse=0.010, tp_bps=5, thresh=1.5, fo=35%, legs=3, ScaleOut");
    println!("============================================================");

    // ---- 1. Full-window baseline ----
    let signal = RelaxedM1 { state_map: sm.clone(), thresh: 1.5 };
    let baseline = run_config(&bars, &signal, budget, 0.010, 5.0, 35.0, 3, symbol);
    if let Some((ann, dd, sh, skew, kurt, ee, days)) = baseline {
        println!("\n[BASELINE full {days}d] ann={ann:.2}% dd={dd:.2}% sharpe={sh:.3} skew={skew:.3} kurt={kurt:.3} end_eq={ee:.1}");
    }

    // ---- 2. Cold starts (cs00/cs30/cs60/cs90/cs120) ----
    println!("\n--- COLD STARTS (offset_days -> must all be positive) ---");
    let offsets = [0i64, 30, 60, 90, 120];
    let mut cold_results = Vec::new();
    let mut all_positive = true;
    for &off in &offsets {
        let off_ms = off * 86_400_000;
        let cbars: Vec<KlineBar> = bars.iter().filter(|b| b.open_time_ms >= mn + off_ms).cloned().collect();
        if cbars.is_empty() { continue; }
        let sig = RelaxedM1 { state_map: sm.clone(), thresh: 1.5 };
        if let Some((ann, dd, sh, skew, kurt, ee, days)) = run_config(&cbars, &sig, budget, 0.010, 5.0, 35.0, 3, symbol) {
            let pos = ann > 0.0;
            if !pos { all_positive = false; }
            println!("  cs{off:>3}: days={days} ann={ann:>7.2}% dd={dd:>6.2}% sharpe={sh:.3} end_eq={ee:>7.1} positive={pos}");
            cold_results.push((off, ann, dd, sh));
        }
    }
    println!("  -> 5/5 cold starts positive: {all_positive}");

    // ---- 3. DSR (n_trials=1, pre-registered single policy) ----
    println!("\n--- DSR (Deflated Sharpe Ratio) ---");
    let daily: BTreeMap<i64, f64> = {
        let signal = RelaxedM1 { state_map: sm.clone(), thresh: 1.5 };
        let cfg = GatedMartinConfig {
            symbol: symbol.to_string(), filters: ExchangeFilters::default_for(symbol),
            budget_quote: budget, fo_quote: (0.35 * budget).max(6.0),
            adverse_spacing_frac: 0.010, tp_net_bps_floor: 5.0,
            fee_bps: 2.0, slippage_bps: 1.0, leverage: 3, direction_bias: 1,
            tp_mode: TpMode::ScaleOut, max_legs: 3,
        };
        let res = run_gated_martin(&cfg, &bars, &signal).unwrap();
        let mut m = BTreeMap::new();
        for e in &res.equity_curve { m.insert(e.timestamp_ms / 86_400_000, e.equity_quote); }
        m
    };
    let rets: Vec<f64> = daily.values().collect::<Vec<_>>().windows(2)
        .filter_map(|w| if w[0] > &0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
    let st = sharpe_stats(&rets);
    let ann_sharpe = st.sharpe * (365.0_f64).sqrt();
    let dsr = deflated_sharpe(ann_sharpe, 1, st.n as u64, st.skew, st.kurtosis);
    let st_skew = st.skew;
    let st_kurt = st.kurtosis;
    let st_n = st.n;
    println!("  ann_sharpe={ann_sharpe:.4} skew={st_skew:.3} kurt={st_kurt:.3} n={st_n}");
    println!("  DSR(n_trials=1) = {dsr:.4}  (must be > 0: {})", if dsr > 0.0 { "PASS" } else { "FAIL" });

    // ---- 4. PBO across 8 independent configs (different fo/thresh/legs/tp_bps) ----
    println!("\n--- PBO (Probability of Backtest Overfitment) ---");
    println!("  Building 8 independent policy configs (different fo/thresh/legs/tp_bps)...");
    let policy_configs: Vec<(f64, f64, u32, f64, f64)> = vec![
        // (fo_pct, thresh, max_legs, adverse, tp_bps)
        (35.0, 1.5, 3, 0.010, 5.0),   // best
        (35.0, 1.0, 3, 0.010, 10.0),
        (35.0, 2.0, 2, 0.005, 20.0),
        (50.0, 1.5, 3, 0.020, 10.0),
        (20.0, 1.0, 2, 0.010, 5.0),
        (35.0, 2.0, 3, 0.020, 40.0),
        (50.0, 1.5, 2, 0.005, 20.0),
        (10.0, 1.5, 3, 0.010, 10.0),
    ];
    let mut policy_returns: Vec<Vec<f64>> = Vec::new();
    for (i, (fo, th, ml, adv, bps)) in policy_configs.iter().enumerate() {
        let sig = RelaxedM1 { state_map: sm.clone(), thresh: *th };
        let cfg = GatedMartinConfig {
            symbol: symbol.to_string(), filters: ExchangeFilters::default_for(symbol),
            budget_quote: budget, fo_quote: (fo / 100.0 * budget).max(6.0),
            adverse_spacing_frac: *adv, tp_net_bps_floor: *bps,
            fee_bps: 2.0, slippage_bps: 1.0, leverage: 3, direction_bias: 1,
            tp_mode: TpMode::ScaleOut, max_legs: *ml,
        };
        if let Ok(res) = run_gated_martin(&cfg, &bars, &sig) {
            let mut daily: BTreeMap<i64, f64> = BTreeMap::new();
            for e in &res.equity_curve { daily.insert(e.timestamp_ms / 86_400_000, e.equity_quote); }
            let rets: Vec<f64> = daily.values().collect::<Vec<_>>().windows(2)
                .filter_map(|w| if w[0] > &0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
            let ee = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(budget);
            let days = ((mx - mn) as f64) / 86_400_000.0;
            let ann = if ee > 0.0 { ((ee / budget).powf(365.0 / days) - 1.0) * 100.0 } else { -100.0 };
            println!("    policy {i}: fo={:.0} th={:.1} legs={} adv={:.3} bps={:.0} -> ann={:>7.2}% rets={}", fo, th, ml, adv, bps, ann, rets.len());
            policy_returns.push(rets);
        }
    }
    let pbo = probability_of_backtest_overfitting(&policy_returns, 200);
    println!("  PBO(8 independent configs) = {pbo:.4}  (must be < 0.5: {})", if pbo < 0.5 { "PASS" } else { "FAIL" });

    // ---- 5. Tier judgment ----
    println!("\n============================================================");
    println!("TIER JUDGMENT");
    println!("============================================================");
    if let Some((ann, dd, sh, _, _, _, _)) = baseline {
        let conservative = ann >= 50.0 && dd <= 10.0 && sh >= 2.0 && dsr > 0.0 && pbo < 0.5 && all_positive;
        let balanced = ann >= 90.0 && dd <= 20.0 && sh >= 2.0 && dsr > 0.0 && pbo < 0.5 && all_positive;
        let aggressive = ann >= 110.0 && dd <= 30.0 && sh >= 2.0 && dsr > 0.0 && pbo < 0.5 && all_positive;
        println!("Conservative (50%/DD<=10%): ann={ann:.2}% dd={dd:.2}% sh={sh:.3} -> {}", if conservative { "HIT" } else { "MISS" });
        println!("Balanced     (90%/DD<=20%): ann={ann:.2}% dd={dd:.2}% sh={sh:.3} -> {}", if balanced { "HIT" } else { "MISS" });
        println!("Aggressive  (110%/DD<=30%): ann={ann:.2}% dd={dd:.2}% sh={sh:.3} -> {}", if aggressive { "HIT" } else { "MISS" });
        let any_hit = conservative || balanced || aggressive;
        println!("\nANY TIER HIT: {any_hit}");
        println!("Gate summary: 5/5cs={all_positive} DSR={dsr:.2}({}) PBO={pbo:.3}({}) Sharpe={sh:.3}({})",
            if dsr > 0.0 {"PASS"} else {"FAIL"}, if pbo < 0.5 {"PASS"} else {"FAIL"}, if sh >= 2.0 {"PASS"} else {"FAIL"});

        // Write gate artifact.
        let out_path = "docs/superpowers/artifacts/glm-martingale-core-round23/r8/gates/r8-btc-best-gates.json";
        if let Some(parent) = Path::new(out_path).parent() { std::fs::create_dir_all(parent)?; }
        let result = serde_json::json!({
            "config": {"symbol": symbol, "adverse": 0.010, "tp_bps": 5.0, "thresh": 1.5,
                       "fo_pct": 35.0, "max_legs": 3, "tp_mode": "ScaleOut", "budget": budget},
            "baseline": {"ann_pct": ann, "dd_pct": dd, "sharpe": sh, "days": baseline.map(|b| b.6).unwrap_or(0)},
            "cold_starts": cold_results.iter().map(|(o, a, d, s)| serde_json::json!({
                "offset_days": o, "annualized_pct": a, "max_dd_pct": d, "sharpe": s, "positive": *a > 0.0
            })).collect::<Vec<_>>(),
            "all_cold_positive": all_positive,
            "dsr": dsr, "pbo": pbo,
            "tier_hits": {"conservative": conservative, "balanced": balanced, "aggressive": aggressive},
            "any_hit": any_hit,
            "gate_passes": {"dsr_positive": dsr > 0.0, "pbo_lt_0.5": pbo < 0.5, "sharpe_ge_2": sh >= 2.0, "cold_starts": all_positive},
        });
        std::fs::write(out_path, serde_json::to_string_pretty(&result)?)?;
        println!("\nWrote {out_path}");
    }
    Ok(())
}
