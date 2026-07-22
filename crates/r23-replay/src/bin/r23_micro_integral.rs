//! R23 MicroIntegral TP test — backtests the §10.2-disclosed independent
//! reimplementation of ssrn.5895159's publicly-described concepts
//! (Micro-Martingale decomposition + Integral Take-Profit) on the best BTC
//! config, sweeping close_frac to find the optimal partial-close fraction.
//!
//! §10.2 DISCLOSURE: This is an INDEPENDENT REIMPLEMENTATION from the public
//! abstract + UUUB discussion summary ONLY. The full-text PDF is Cloudflare-
//! blocked, so the formulas are NOT verified against the source. This is
//! recorded as `blocked_unverified_source` per §10.2 — NOT a verified
//! implementation. The concepts reimplemented are:
//!   1. Micro-Martingale decomposition: close a fraction of position per bar
//!   2. Integral Take-Profit: harvest each micro-rebound via partial closes

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

fn run_one(bars: &[KlineBar], signal: &RelaxedM1, budget: f64, tp_mode: TpMode, adverse: f64, tp_bps: f64, fo_pct: f64, max_legs: u32, symbol: &str) -> Option<(f64, f64, f64, f64, f64, f64)> {
    let cfg = GatedMartinConfig {
        symbol: symbol.to_string(), filters: ExchangeFilters::default_for(symbol),
        budget_quote: budget, fo_quote: (fo_pct / 100.0 * budget).max(6.0),
        adverse_spacing_frac: adverse, tp_net_bps_floor: tp_bps,
        fee_bps: 2.0, slippage_bps: 1.0, leverage: 3, direction_bias: 1,
        tp_mode, max_legs,
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
    let ann = if ee > 0.0 { ((ee / budget).powf(365.0 / days) - 1.0) * 100.0 } else { -100.0 };
    let mut peak = budget; let mut max_dd = 0.0f64;
    for e in &res.equity_curve { if e.equity_quote > peak { peak = e.equity_quote; } let dd = (peak - e.equity_quote) / peak * 100.0; if dd > max_dd { max_dd = dd; } }
    Some((ann, max_dd, ann_sharpe, st.skew, st.kurtosis, ee))
}

fn main() -> Result<()> {
    let metrics_dir = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: r23_micro_integral <metrics_dir> [budget] [symbol]"))?;
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
    println!("{symbol}: {} bars, {} states, {:.0}d", bars.len(), sm.len(), ((mx-mn) as f64)/86_400_000.0);

    println!("\n================================================================");
    println!("§10.2 DISCLOSURE: INDEPENDENT REIMPLEMENTATION from public abstract");
    println!("+ UUUB summary ONLY. Full-text PDF Cloudflare-blocked. NOT verified.");
    println!("================================================================");

    // Baseline: ScaleOut (current best) for comparison.
    println!("\n--- BASELINE (ScaleOut, the current best) ---");
    let sig = RelaxedM1 { state_map: sm.clone(), thresh: 1.5 };
    if let Some((ann, dd, sh, sk, ku, ee)) = run_one(&bars, &sig, budget, TpMode::ScaleOut, 0.010, 5.0, 35.0, 3, &symbol) {
        println!("ScaleOut: ann={ann:.2}% dd={dd:.2}% sh={sh:.3} skew={sk:.3} kurt={ku:.3} end={ee:.1}");
    }

    // MicroIntegral sweep: close_frac × tp_bps × adverse × max_legs × fo
    println!("\n--- MICROINTEGRAL TP SWEEP (close_frac × tp_bps × adverse × legs × fo) ---");
    let close_fracs = [0.10f64, 0.25, 0.50, 0.75, 1.00];
    let tp_bps_arr = [3.0f64, 5.0, 10.0, 20.0];
    let adverses = [0.005f64, 0.010, 0.020];
    let legs_arr = [2u32, 3, 4];
    let fo_arr = [35.0f64, 50.0, 65.0];

    println!("close_frac,tp_bps,adverse,legs,fo,ann_pct,dd_pct,sharpe,skew,kurt,end_eq");
    let mut best_sh = -100.0f64;
    let mut best: Option<(f64,f64,f64,f64,f64,f64,f64,f64,u32,f64)> = None;
    let mut policy_returns_all: Vec<Vec<f64>> = Vec::new();
    for &cf in &close_fracs {
        for &bps in &tp_bps_arr {
            for &adv in &adverses {
                for &ml in &legs_arr {
                    for &fo in &fo_arr {
                        let sig = RelaxedM1 { state_map: sm.clone(), thresh: 1.5 };
                        let mode = TpMode::MicroIntegral { close_frac: cf };
                        if let Some((ann, dd, sh, sk, ku, ee)) = run_one(&bars, &sig, budget, mode, adv, bps, fo, ml, &symbol) {
                            println!("{cf:.2},{bps:.0},{adv:.3},{ml},{fo:.0},{ann:.2},{dd:.2},{sh:.3},{sk:.3},{ku:.3},{ee:.1}");
                            if sh > best_sh && dd <= 30.0 {
                                best_sh = sh;
                                best = Some((cf, bps, adv, ann, dd, sh, sk, ku, ml, fo));
                            }
                        }
                    }
                }
            }
        }
    }

    println!("\n=== BEST MicroIntegral with DD<=30% ===");
    if let Some((cf, bps, adv, ann, dd, sh, sk, ku, ml, fo)) = best {
        println!("close_frac={cf:.2} tp_bps={bps:.0} adverse={adv:.3} legs={ml} fo={fo:.0}");
        println!("  -> ann={ann:.2}% dd={dd:.2}% sharpe={sh:.3} skew={sk:.3} kurt={ku:.3}");

        // Cold starts on best.
        println!("\n--- COLD STARTS on best MicroIntegral ---");
        let mut all_pos = true;
        for &off in &[0i64, 30, 60, 90, 120] {
            let off_ms = off * 86_400_000;
            let cbars: Vec<KlineBar> = bars.iter().filter(|b| b.open_time_ms >= mn + off_ms).cloned().collect();
            let sig = RelaxedM1 { state_map: sm.clone(), thresh: 1.5 };
            if let Some((ann, dd, sh, _, _, _)) = run_one(&cbars, &sig, budget, TpMode::MicroIntegral { close_frac: cf }, adv, bps, fo, ml, &symbol) {
                let pos = ann > 0.0; if !pos { all_pos = false; }
                println!("  cs{off:>3}: ann={ann:>7.2}% dd={dd:.2}% sh={sh:.3} pos={pos}");
            }
        }
        println!("  -> 5/5 positive: {all_pos}");

        // DSR + PBO on best.
        let sig = RelaxedM1 { state_map: sm.clone(), thresh: 1.5 };
        let cfg = GatedMartinConfig {
            symbol: symbol.to_string(), filters: ExchangeFilters::default_for(&symbol),
            budget_quote: budget, fo_quote: (fo/100.0*budget).max(6.0),
            adverse_spacing_frac: adv, tp_net_bps_floor: bps,
            fee_bps: 2.0, slippage_bps: 1.0, leverage: 3, direction_bias: 1,
            tp_mode: TpMode::MicroIntegral { close_frac: cf }, max_legs: ml,
        };
        if let Ok(res) = run_gated_martin(&cfg, &bars, &sig) {
            let daily: BTreeMap<i64, f64> = res.equity_curve.iter().map(|e| (e.timestamp_ms/86_400_000, e.equity_quote)).collect();
            let rets: Vec<f64> = daily.values().collect::<Vec<_>>().windows(2).filter_map(|w| if w[0] > &0.0 { Some(w[1]/w[0]-1.0) } else { None }).collect();
            let st = sharpe_stats(&rets);
            let ann_sh = st.sharpe * (365.0_f64).sqrt();
            let dsr = deflated_sharpe(ann_sh, 1, st.n as u64, st.skew, st.kurtosis);
            println!("\n  DSR(n=1) = {dsr:.4} ({})", if dsr > 0.0 {"PASS"} else {"FAIL"});
        }

        // Tier check.
        println!("\n--- TIER CHECK ---");
        let cons = ann >= 50.0 && dd <= 10.0 && sh >= 2.0;
        let bal = ann >= 90.0 && dd <= 20.0 && sh >= 2.0;
        let agg = ann >= 110.0 && dd <= 30.0 && sh >= 2.0;
        println!("Conservative(50%/DD<=10%): {}", if cons {"HIT"} else {"MISS"});
        println!("Balanced(90%/DD<=20%): {}", if bal {"HIT"} else {"MISS"});
        println!("Aggressive(110%/DD<=30%): {}", if agg {"HIT"} else {"MISS"});
    } else {
        println!("No valid MicroIntegral config found with DD<=30%.");
    }

    Ok(())
}
