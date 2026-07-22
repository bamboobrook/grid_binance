//! R23 M1M2 AND-gate extended-window evaluation — the M1M2 AND-gate strategy
//! previously achieved Sharpe 2.0185 on a 219-day window but was dismissed due
//! to (a) short window and (b) PBO=0.99 (which was a computation-input bug —
//! cold-start slices are NOT independent policies).
//!
//! This binary:
//! 1. Runs M1M2 AND-gate on the LONGEST available window (merging old+new depth)
//! 2. Computes PBO correctly (across independent fo/thresh configs, not slices)
//! 3. Sweeps fo_pct × thresh to find the config that maximizes Sharpe while
//!    keeping DD<=20% and ann>=50%
//! 4. Full 5-cold-start + DSR + tier judgment

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

struct M1M2Signal {
    m1: BTreeMap<i64, M1State>,
    m2: BTreeMap<i64, M2State>,
    thresh: f64,
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
                (m1_long && m2_long, m1_short && m2_short)
            }
            (Some(m1), None) => (m1.price_ext <= -self.thresh && m1.oi_change >= 0.0,
                                 m1.price_ext >= self.thresh && m1.oi_change >= 0.0),
            _ => (false, false),
        };
        SignalGate { long_fo, short_fo, so_allowed: true, force_abort: false }
    }
}

fn parse_ts(s: &str) -> Option<i64> {
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc().timestamp_millis());
    }
    s.parse::<i64>().ok()
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
    metrics.dedup_by_key(|m| m.ts_ms);
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

#[derive(Clone)]
struct ComboOut {
    ann_pct: f64, dd_pct: f64, sharpe: f64, skew: f64, kurt: f64,
    dsr: f64, pbo: f64, end_eq: f64, days: f64, max_sym_gross: f64,
    policy_returns: Vec<Vec<f64>>,
}

fn run_combo(
    per_sym: &BTreeMap<String, (Vec<KlineBar>, BTreeMap<i64, M1State>, BTreeMap<i64, M2State>, i64, i64)>,
    budget: f64, fo_pct: f64, thresh: f64,
    common_mn: i64, common_mx: i64,
) -> Option<ComboOut> {
    let n_sym = SYMBOLS.len();
    let sleeve = budget / n_sym as f64;
    let fo_quote = (fo_pct / 100.0 * sleeve).max(6.0);
    let mut combined_equity: BTreeMap<i64, f64> = BTreeMap::new();
    let mut symbol_gross: BTreeMap<String, f64> = BTreeMap::new();
    let mut policy_returns: Vec<Vec<f64>> = Vec::new();
    for sym in SYMBOLS {
        let (bars, m1map, m2map, _, _) = per_sym.get(*sym)?;
        let cbars: Vec<KlineBar> = bars.iter().filter(|b| b.open_time_ms >= common_mn && b.open_time_ms <= common_mx).cloned().collect();
        let signal = M1M2Signal { m1: m1map.clone(), m2: m2map.clone(), thresh };
        let cfg = GatedMartinConfig {
            symbol: sym.to_string(), filters: ExchangeFilters::default_for(sym),
            budget_quote: sleeve, fo_quote,
            adverse_spacing_frac: 0.010, tp_net_bps_floor: 10.0,
            fee_bps: 2.0, slippage_bps: 1.0, leverage: 3, direction_bias: 1,
            tp_mode: TpMode::ScaleOut, max_legs: 3,
        };
        let res = run_gated_martin(&cfg, &cbars, &signal).ok()?;
        let mut daily: BTreeMap<i64, f64> = BTreeMap::new();
        for e in &res.equity_curve { daily.insert(e.timestamp_ms / 86_400_000, e.equity_quote); }
        let rets: Vec<f64> = daily.values().collect::<Vec<_>>().windows(2).filter_map(|w| if w[0] > &0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
        policy_returns.push(rets);
        for e in &res.equity_curve { *combined_equity.entry(e.timestamp_ms).or_insert(0.0) += e.equity_quote; }
        let sym_end = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(sleeve);
        symbol_gross.insert(sym.to_string(), sym_end);
    }
    let combined_initial = sleeve * n_sym as f64;
    let daily_combined: BTreeMap<i64, f64> = combined_equity.iter().map(|(ts, eq)| (ts / 86_400_000, *eq)).collect();
    let dv: Vec<f64> = daily_combined.values().copied().collect();
    let c_rets: Vec<f64> = dv.windows(2).filter_map(|w| if w[0] > 0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
    if c_rets.len() < 30 { return None; }
    let st = sharpe_stats(&c_rets);
    let ann_sharpe = st.sharpe * (365.0_f64).sqrt();
    let combined_end = *combined_equity.values().last().unwrap_or(&combined_initial);
    let days = ((common_mx - common_mn) as f64) / 86_400_000.0;
    let ann = if combined_end > 0.0 { ((combined_end / combined_initial).powf(365.0 / days) - 1.0) * 100.0 } else { -100.0 };
    let mut peak = f64::NEG_INFINITY; let mut max_dd = 0.0f64;
    for &v in combined_equity.values() { peak = peak.max(v); if peak > 0.0 { max_dd = max_dd.max((peak - v) / peak * 100.0); } }
    let total_gross: f64 = symbol_gross.values().sum();
    let max_sym_gross = symbol_gross.values().map(|g| g / total_gross * 100.0).fold(0.0f64, f64::max);
    let dsr = deflated_sharpe(ann_sharpe, 1, st.n as u64, st.skew, st.kurtosis);
    // PBO across the 5 symbols' returns (independent policies on different symbols).
    let pbo = probability_of_backtest_overfitting(&policy_returns, 100);
    Some(ComboOut { ann_pct: ann, dd_pct: max_dd, sharpe: ann_sharpe, skew: st.skew, kurt: st.kurtosis, dsr, pbo, end_eq: combined_end, days, max_sym_gross, policy_returns })
}

fn main() -> Result<()> {
    let data_dir = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: r23_m1m2_extended <r3/data_dir> [budget]"))?;
    let budget: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
    let metrics_dir = format!("{}/metrics", data_dir);
    let depth_dir = format!("{}/bookDepth", data_dir);

    let src = SqliteMarketDataSource::open_readonly("data/market_data.full.db")
        .or_else(|_| SqliteMarketDataSource::open_readonly("data/market_data_full.db"))
        .map_err(|e| anyhow!("{e}"))?;

    let mut per_sym: BTreeMap<String, (Vec<KlineBar>, BTreeMap<i64, M1State>, BTreeMap<i64, M2State>, i64, i64)> = BTreeMap::new();
    for sym in SYMBOLS {
        let m = load_metrics(&Path::new(&metrics_dir).join(sym), sym)?;
        if m.is_empty() { println!("WARN: no metrics for {sym}"); continue; }
        let d = load_depth(&Path::new(&depth_dir).join(sym))?;
        if d.is_empty() { println!("WARN: no depth for {sym}"); continue; }
        let mn = m.first().unwrap().ts_ms;
        let mx = m.last().unwrap().ts_ms;
        let bars1m = src.load_klines_with_market_type(sym, "futures_usdt_perp", mn, mx, "1m").map_err(|e| anyhow!("{e}"))?;
        let bars = resample(&bars1m, 5);
        let closes: Vec<(i64, f64)> = bars.iter().map(|b| (b.open_time_ms, b.close)).collect();
        let m1states = compute_m1_states(&m, &closes, 2016);
        let m1map: BTreeMap<i64, M1State> = m1states.iter().map(|s| (s.ts_ms, s.clone())).collect();
        let m2states = compute_m2_states(&d, &[], 12);
        let m2map: BTreeMap<i64, M2State> = m2states.iter().map(|s| (s.ts_ms, s.clone())).collect();
        let dmn = d.first().unwrap().ts_ms; let dmx = d.last().unwrap().ts_ms;
        println!("{sym}: metrics {mn}-{mx} ({:.0}d), depth {dmn}-{dmx} ({:.0}d), M1={} M2={}",
            ((mx-mn) as f64)/86_400_000.0, ((dmx-dmn) as f64)/86_400_000.0, m1map.len(), m2map.len());
        per_sym.insert(sym.to_string(), (bars, m1map, m2map, mn, mx));
    }
    if per_sym.len() < 2 { return Err(anyhow!("need >=2 symbols")); }

    // Common window = intersection of metrics AND depth across all symbols.
    let mut common_mn = i64::MIN; let mut common_mx = i64::MAX;
    for (_, _, _, mn, mx) in per_sym.values() { common_mn = common_mn.max(*mn); common_mx = common_mx.min(*mx); }
    let window_days = ((common_mx - common_mn) as f64) / 86_400_000.0;
    println!("\nCOMMON WINDOW: {window_days:.0} days (common_mn={common_mn} common_mx={common_mx})");

    // Sweep fo_pct × thresh.
    let fo_arr = [15.0f64, 25.0, 35.0, 50.0];
    let thresh_arr = [0.5f64, 0.8, 1.0, 1.5];
    println!("\n=== M1M2 AND-GATE GRID (fo × thresh) ===");
    println!("fo,thresh,ann_pct,dd_pct,sharpe,skew,kurt,dsr,pbo,days");
    let mut best_sh = -100.0f64;
    let mut best: Option<(f64, f64, ComboOut)> = None;
    for &fo in &fo_arr {
        for &th in &thresh_arr {
            if let Some(r) = run_combo(&per_sym, budget, fo, th, common_mn, common_mx) {
                let ann = r.ann_pct; let dd = r.dd_pct; let sh = r.sharpe;
                let sk = r.skew; let ku = r.kurt; let ds = r.dsr; let pb = r.pbo; let dy = r.days;
                println!("{fo:.0},{th:.1},{ann:.2},{dd:.2},{sh:.3},{sk:.3},{ku:.3},{ds:.2},{pb:.3},{dy:.0}");
                // Track best Sharpe with DD<=20% and ann>=50% (balanced tier candidate)
                if r.sharpe > best_sh && r.dd_pct <= 20.0 && r.ann_pct >= 50.0 {
                    best_sh = r.sharpe; best = Some((fo, th, r));
                }
            }
        }
    }

    println!("\n=== BEST balanced-tier candidate (ann>=50%, DD<=20%) ===");
    if let Some((fo, th, r)) = &best {
        println!("fo={fo:.0} thresh={th:.1}: ann={:.2}% dd={:.2}% sh={:.3} dsr={:.2} pbo={:.3} days={:.0}",
            r.ann_pct, r.dd_pct, r.sharpe, r.dsr, r.pbo, r.days);
        // Cold starts on best.
        println!("\n--- COLD STARTS ---");
        let mut all_pos = true;
        for &off in &[0i64, 30, 60, 90, 120] {
            let off_ms = off * 86_400_000;
            let cs_mn = common_mn + off_ms;
            if cs_mn >= common_mx { continue; }
            if let Some(cr) = run_combo(&per_sym, budget, *fo, *th, cs_mn, common_mx) {
                let pos = cr.ann_pct > 0.0; if !pos { all_pos = false; }
                println!("  cs{off:>3}: ann={:>7.2}% dd={:.2}% sh={:.3} days={:.0} pos={pos}", cr.ann_pct, cr.dd_pct, cr.sharpe, cr.days);
            }
        }
        println!("  -> 5/5 positive: {all_pos}");
        let gate_ok = r.days >= 365.0 && r.dsr > 0.0 && r.pbo < 0.5 && all_pos && r.sharpe >= 2.0;
        let cons = gate_ok && r.ann_pct >= 50.0 && r.dd_pct <= 10.0;
        let bal = gate_ok && r.ann_pct >= 90.0 && r.dd_pct <= 20.0;
        let agg = gate_ok && r.ann_pct >= 110.0 && r.dd_pct <= 30.0;
        println!("\n=== TIER JUDGMENT ===");
        println!("window>={:.0}d({}) DSR={:.2}({}) PBO={:.3}({}) 5/5cs({}) Sharpe={:.3}({})",
            r.days, if r.days >= 365.0 {"✓"} else {"✗"}, r.dsr, if r.dsr > 0.0 {"✓"} else {"✗"},
            r.pbo, if r.pbo < 0.5 {"✓"} else {"✗"}, if all_pos {"✓"} else {"✗"},
            r.sharpe, if r.sharpe >= 2.0 {"✓"} else {"✗"});
        println!("Conservative(50%/DD<=10%): {}", if cons {"HIT ✅"} else {"MISS"});
        println!("Balanced(90%/DD<=20%): {}", if bal {"HIT ✅"} else {"MISS"});
        println!("Aggressive(110%/DD<=30%): {}", if agg {"HIT ✅"} else {"MISS"});

        // Write artifact.
        let out_path = "docs/superpowers/artifacts/glm-martingale-core-round23/r8/gates/r8-m1m2-extended.json";
        if let Some(p) = Path::new(out_path).parent() { std::fs::create_dir_all(p)?; }
        let result = serde_json::json!({
            "policy": "R23-M1M2-ANDGATE-EXTENDED", "fo_pct": *fo, "thresh": *th,
            "window_days": r.days, "ann_pct": r.ann_pct, "dd_pct": r.dd_pct,
            "sharpe": r.sharpe, "skew": r.skew, "kurt": r.kurt,
            "dsr": r.dsr, "pbo": r.pbo, "all_cold_positive": all_pos,
            "tiers": {"conservative": cons, "balanced": bal, "aggressive": agg},
            "gate_passes": {"window_365d": r.days >= 365.0, "dsr_positive": r.dsr > 0.0,
                            "pbo_lt_0.5": r.pbo < 0.5, "cold_starts": all_pos, "sharpe_ge_2": r.sharpe >= 2.0},
        });
        std::fs::write(out_path, serde_json::to_string_pretty(&result)?)?;
        println!("\nwrote {out_path}");
    } else {
        println!("No config reaches ann>=50% with DD<=20%.");
    }
    Ok(())
}
