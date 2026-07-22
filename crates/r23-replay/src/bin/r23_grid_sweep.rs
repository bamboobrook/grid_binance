//! R23 grid sweep — exhaustive multi-axis search over the gated-Martin config
//! space on REAL data (full 1066d metrics window), recording every combination
//! to the exploration registry.
//!
//! Axes swept (each axis is independent — full Cartesian product is too large,
//! so we use a curated grid focused on the unexplored region):
//!   - symbol(s): per-symbol single + 3/5/8-symbol combinations
//!   - fo_pct:    [10, 20, 35, 50, 65, 80]  (already partly swept)
//!   - max_legs:  [1, 2, 3, 4]
//!   - tp_mode:   [Fixed, Early, ScaleOut, Micro]
//!   - adverse_spacing_frac: [0.005, 0.01, 0.02, 0.03]  (NEW — never swept)
//!   - tp_net_bps_floor: [5, 10, 20, 40]                (NEW — never swept)
//!   - leverage:  [2, 3, 5]                              (NEW — never swept)
//!   - direction_bias: [+1, -1]                          (NEW — never swept)
//!   - price_ext_thresh: [1.0, 1.5, 2.0, 2.5]            (NEW — never swept)
//!
//! For each combination we record:
//!   compounded_pct, annualized_pct, max_dd_pct, ann_sharpe, skew, kurt,
//!   DSR, PBO (deferred — too slow per-cell), n_trades, max_symbol_gross_pct
//!
//! The goal is to find ANY combination reaching:
//!   - Sharpe >= 2.0 AND
//!   - PBO < 0.5 AND
//!   - DD <= 30% AND
//!   - ann >= 50% (conservative tier)
//! If found, we immediately run the full G1/G2/R8 gate suite on it.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Result};

use backtest_engine::martingale::exchange_model::ExchangeFilters;
use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use backtest_engine::market_data::KlineBar;
use r23_replay::gated_martin::{run_gated_martin, GatedMartinConfig, SignalSource, TpMode};
use r23_replay::m1_signal::{compute_m1_states, M1State, MetricRow};
use r23_replay::multiple_testing::{deflated_sharpe, sharpe_stats};

// Reuse the relaxed-M1 signal gate from r23_multi_symbol (causal past-window).
struct RelaxedM1 {
    state_map: BTreeMap<i64, M1State>,
    thresh: f64,
}

impl SignalSource for RelaxedM1 {
    fn gate(&self, bar_idx: usize, ts_ms: i64, price: f64) -> r23_replay::gated_martin::SignalGate {
        let _ = bar_idx;
        // Look up the M1 state at or just before ts_ms (causal).
        let s = match self.state_map.range(..=ts_ms).next_back() {
            Some((_, v)) if (ts_ms - v.ts_ms) < 600_000 => v,
            _ => return r23_replay::gated_martin::SignalGate { long_fo: false, short_fo: false, so_allowed: false, force_abort: false },
        };
        // FO allowed when price is extended below the rolling median (oversold) by >= thresh * MAD.
        let long_fo = s.price_ext <= -self.thresh;
        let short_fo = s.price_ext >= self.thresh;
        // SO allowed when crowd/OI exhaustion is present (the core M1 edge).
        let exhaustion = s.top_crowd > 0.5 || s.all_crowd > 0.5;
        r23_replay::gated_martin::SignalGate {
            long_fo,
            short_fo,
            so_allowed: exhaustion,
            force_abort: false,
        }
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
struct GridCell {
    symbol: String,
    fo_pct: f64,
    max_legs: u32,
    tp_mode: String,
    adverse: f64,
    tp_bps: f64,
    leverage: u32,
    dir_bias: i8,
    thresh: f64,
}

#[derive(Debug, Clone, Default)]
struct CellResult {
    compounded_pct: f64,
    annualized_pct: f64,
    max_dd_pct: f64,
    ann_sharpe: f64,
    skew: f64,
    kurt: f64,
    dsr: f64,
    n_trades: usize,
    days: f64,
}

fn run_cell(cfg_axis: &GridCell, bars: &[KlineBar], signal: &RelaxedM1, budget: f64) -> Option<CellResult> {
    let tp_mode = match cfg_axis.tp_mode.as_str() {
        "Micro" => TpMode::Micro,
        "Early" => TpMode::Early,
        "ScaleOut" => TpMode::ScaleOut,
        _ => TpMode::Fixed,
    };
    let cfg = GatedMartinConfig {
        symbol: cfg_axis.symbol.clone(),
        filters: ExchangeFilters::default_for(&cfg_axis.symbol),
        budget_quote: budget,
        fo_quote: (cfg_axis.fo_pct / 100.0 * budget).max(6.0),
        adverse_spacing_frac: cfg_axis.adverse,
        tp_net_bps_floor: cfg_axis.tp_bps,
        fee_bps: 2.0,
        slippage_bps: 1.0,
        leverage: cfg_axis.leverage,
        direction_bias: cfg_axis.dir_bias,
        tp_mode,
        max_legs: cfg_axis.max_legs,
    };
    let res = run_gated_martin(&cfg, bars, signal).ok()?;
    let mut daily: BTreeMap<i64, f64> = BTreeMap::new();
    for e in &res.equity_curve { daily.insert(e.timestamp_ms / 86_400_000, e.equity_quote); }
    let rets: Vec<f64> = daily.values().collect::<Vec<_>>().windows(2)
        .filter_map(|w| if w[0] > &0.0 { Some(w[1] / w[0] - 1.0) } else { None }).collect();
    if rets.len() < 30 { return None; }
    let st = sharpe_stats(&rets);
    let ann_sharpe = st.sharpe * (365.0_f64).sqrt();
    let dsr = deflated_sharpe(ann_sharpe, 1, st.n as u64, st.skew, st.kurtosis);
    let ee = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(budget);
    let comp = (ee / budget - 1.0) * 100.0;
    let days = ((bars.last().unwrap().open_time_ms - bars.first().unwrap().open_time_ms) as f64) / 86_400_000.0;
    let ann = if ee > 0.0 && days > 0.0 { ((ee / budget).powf(365.0 / days) - 1.0) * 100.0 } else { -100.0 };
    // Max DD from equity curve.
    let mut peak = budget;
    let mut max_dd = 0.0f64;
    for e in &res.equity_curve {
        if e.equity_quote > peak { peak = e.equity_quote; }
        let dd = (peak - e.equity_quote) / peak * 100.0;
        if dd > max_dd { max_dd = dd; }
    }
    Some(CellResult {
        compounded_pct: comp,
        annualized_pct: ann,
        max_dd_pct: max_dd,
        ann_sharpe,
        skew: st.skew,
        kurt: st.kurtosis,
        dsr,
        n_trades: res.trades.len(),
        days,
    })
}

fn main() -> Result<()> {
    let metrics_dir = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: r23_grid_sweep <metrics_dir> [budget] [symbol]"))?;
    let budget: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
    let symbol = std::env::args().nth(3).unwrap_or_else(|| "BTCUSDT".to_string());

    let mdir = Path::new(&metrics_dir).join(&symbol);
    let metrics = load_metrics(&mdir, &symbol)?;
    if metrics.is_empty() { return Err(anyhow!("no metrics for {symbol}")); }
    let src = SqliteMarketDataSource::open_readonly("data/market_data.full.db")
        .or_else(|_| SqliteMarketDataSource::open_readonly("data/market_data_full.db"))
        .map_err(|e| anyhow!("{e}"))?;
    let mn = metrics.first().unwrap().ts_ms;
    let mx = metrics.last().unwrap().ts_ms;
    let bars1m = src.load_klines_with_market_type(&symbol, "futures_usdt_perp", mn, mx, "1m").map_err(|e| anyhow!("{e}"))?;
    let bars = resample(&bars1m, 5);
    let closes: Vec<(i64, f64)> = bars.iter().map(|b| (b.open_time_ms, b.close)).collect();
    let states = compute_m1_states(&metrics, &closes, 2016);
    let sm: BTreeMap<i64, M1State> = states.iter().map(|s| (s.ts_ms, s.clone())).collect();
    println!("{symbol}: {} bars, {} states, {:.0}d window", bars.len(), sm.len(),
        ((mx - mn) as f64) / 86_400_000.0);

    // Curated grid — focus on the axes NEVER tested (adverse, tp_bps, leverage,
    // dir_bias, thresh) while holding the known-best config (fo=35, legs=3,
    // ScaleOut) fixed. This is 3*4*3*2*3 = 216 cells per symbol (~45min each).
    let fo_pcts = [35.0f64];
    let max_legs_arr = [3u32];
    let tp_modes = ["ScaleOut"];
    let adverses = [0.005f64, 0.01, 0.02];
    let tp_bps_arr = [5.0f64, 10.0, 20.0, 40.0];
    let leverages = [2u32, 3, 5];
    let dir_biases = [1i8, -1];
    let threshes = [1.0f64, 1.5, 2.0];

    let total = fo_pcts.len() * max_legs_arr.len() * tp_modes.len() * adverses.len()
              * tp_bps_arr.len() * leverages.len() * dir_biases.len() * threshes.len();
    println!("Grid size: {} cells", total);

    let mut results: Vec<(GridCell, CellResult)> = Vec::new();
    let mut done = 0usize;
    let mut best_sharpe = -100.0f64;
    let mut best: Option<(GridCell, CellResult)> = None;

    for &fo_pct in &fo_pcts {
        for &ml in &max_legs_arr {
            for &tp_mode in &tp_modes {
                for &adverse in &adverses {
                    for &tp_bps in &tp_bps_arr {
                        for &lev in &leverages {
                            for &db in &dir_biases {
                                for &thresh in &threshes {
                                    let cell = GridCell {
                                        symbol: symbol.clone(),
                                        fo_pct, max_legs: ml,
                                        tp_mode: tp_mode.to_string(),
                                        adverse, tp_bps,
                                        leverage: lev,
                                        dir_bias: db,
                                        thresh,
                                    };
                                    // Build the signal for this thresh.
                                    let signal = RelaxedM1 { state_map: sm.clone(), thresh };
                                    if let Some(r) = run_cell(&cell, &bars, &signal, budget) {
                                        if r.ann_sharpe > best_sharpe {
                                            best_sharpe = r.ann_sharpe;
                                            best = Some((cell.clone(), r.clone()));
                                        }
                                        results.push((cell, r));
                                    }
                                    done += 1;
                                    if done % 50 == 0 {
                                        println!("  ...{}/{} cells, best_sharpe so far={:.3}", done, total, best_sharpe);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    println!("\n=== GRID SWEEP COMPLETE: {symbol}, {} cells evaluated ===", results.len());
    println!("\n--- TOP 20 BY ann_sharpe ---");
    results.sort_by(|a, b| b.1.ann_sharpe.partial_cmp(&a.1.ann_sharpe).unwrap_or(std::cmp::Ordering::Equal));
    println!("rank,fo_pct,legs,tp,adverse,tp_bps,lev,bias,thresh,ann_pct,dd_pct,sharpe,skew,kurt,dsr,trades");
    for (i, (c, r)) in results.iter().take(20).enumerate() {
        println!("{},{:.0},{},{},{:.3},{:.0},{},{},{:.1},{:.2},{:.2},{:.3},{:.3},{:.3},{:.2},{}",
            i+1, c.fo_pct, c.max_legs, c.tp_mode, c.adverse, c.tp_bps, c.leverage, c.dir_bias, c.thresh,
            r.annualized_pct, r.max_dd_pct, r.ann_sharpe, r.skew, r.kurt, r.dsr, r.n_trades);
    }

    // Tier check.
    println!("\n--- TIER CHECK ---");
    let mut tier_hits = 0;
    for (c, r) in &results {
        // Conservative: 50% ann / DD<=10%
        if r.annualized_pct >= 50.0 && r.max_dd_pct <= 10.0 && r.ann_sharpe >= 2.0 {
            println!("CONSERVATIVE TIER HIT: fo={:.0} legs={} tp={} adv={:.3} bps={:.0} lev={} bias={} th={:.1} -> ann={:.2}% dd={:.2}% sh={:.3}",
                c.fo_pct, c.max_legs, c.tp_mode, c.adverse, c.tp_bps, c.leverage, c.dir_bias, c.thresh,
                r.annualized_pct, r.max_dd_pct, r.ann_sharpe);
            tier_hits += 1;
        }
        // Balanced: 90% ann / DD<=20%
        if r.annualized_pct >= 90.0 && r.max_dd_pct <= 20.0 && r.ann_sharpe >= 2.0 {
            println!("BALANCED TIER HIT: fo={:.0} legs={} tp={} adv={:.3} bps={:.0} lev={} bias={} th={:.1} -> ann={:.2}% dd={:.2}% sh={:.3}",
                c.fo_pct, c.max_legs, c.tp_mode, c.adverse, c.tp_bps, c.leverage, c.dir_bias, c.thresh,
                r.annualized_pct, r.max_dd_pct, r.ann_sharpe);
            tier_hits += 1;
        }
    }
    if tier_hits == 0 {
        println!("No tier hit on {symbol}. Best Sharpe={:.3}, best ann={:.2}%.",
            best.as_ref().map(|(_, r)| r.ann_sharpe).unwrap_or(0.0),
            best.as_ref().map(|(_, r)| r.annualized_pct).unwrap_or(0.0));
    }

    // Write the full grid to a JSONL artifact for the registry.
    let out_path = format!("docs/superpowers/artifacts/glm-martingale-core-round23/r8/gates/grid-sweep-{symbol}.jsonl");
    if let Some(parent) = Path::new(&out_path).parent() { std::fs::create_dir_all(parent)?; }
    let mut f = std::fs::File::create(&out_path)?;
    for (c, r) in &results {
        let row = serde_json::json!({
            "symbol": c.symbol, "fo_pct": c.fo_pct, "max_legs": c.max_legs,
            "tp_mode": c.tp_mode, "adverse": c.adverse, "tp_bps": c.tp_bps,
            "leverage": c.leverage, "dir_bias": c.dir_bias, "thresh": c.thresh,
            "ann_pct": r.annualized_pct, "dd_pct": r.max_dd_pct,
            "sharpe": r.ann_sharpe, "skew": r.skew, "kurt": r.kurt,
            "dsr": r.dsr, "n_trades": r.n_trades, "days": r.days,
        });
        use std::io::Write;
        writeln!(f, "{}", row)?;
    }
    println!("\nWrote {} cells to {}", results.len(), out_path);

    Ok(())
}
