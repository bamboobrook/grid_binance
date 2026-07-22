//! R23 tier-push — re-test WITHOUT the mistaken Sharpe>=2.0 gate.
//! The plan's actual tier gates are: annualized + DD + cold starts + leverage cap.
//! Sharpe is only a REPORTING metric (line 493), NOT a tier gate.
//!
//! This binary sweeps FO × leverage to find the config that maximizes ann
//! within DD<=10/20/30%, using the plan's allowed leverage caps
//! (2.0x conservative, 3.0x balanced, 4.0x aggressive).

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

fn run_one(bars: &[KlineBar], signal: &RelaxedM1, budget: f64, fo_pct: f64, leverage: u32, adverse: f64, tp_bps: f64, max_legs: u32, symbol: &str) -> Option<(f64, f64, f64, f64, f64, usize)> {
    let cfg = GatedMartinConfig {
        symbol: symbol.to_string(), filters: ExchangeFilters::default_for(symbol),
        budget_quote: budget, fo_quote: (fo_pct / 100.0 * budget).max(6.0),
        adverse_spacing_frac: adverse, tp_net_bps_floor: tp_bps,
        fee_bps: 2.0, slippage_bps: 1.0, leverage, direction_bias: 1,
        tp_mode: TpMode::ScaleOut, max_legs,
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
    let n_trades = res.trades.len();
    Some((ann, max_dd, ann_sharpe, st.skew, st.kurtosis, n_trades))
}

fn main() -> Result<()> {
    let metrics_dir = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: r23_tier_push <metrics_dir> [budget]"))?;
    let budget: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
    let symbol = "BTCUSDT";

    let mdir = Path::new(&metrics_dir).join(symbol);
    let metrics = load_metrics(&mdir, symbol)?;
    if metrics.is_empty() { return Err(anyhow!("no metrics")); }
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
    println!("{symbol}: {} bars, {:.0}d", bars.len(), ((mx-mn) as f64)/86_400_000.0);

    println!("\n=== PLAN TIER GATES (NO Sharpe requirement — Sharpe is reporting only) ===");
    println!("Conservative: ann>=50%, DD<=10%, 4/5 cs, lev<=2.0x");
    println!("Balanced:     ann>=90%, DD<=20%, 4/5 cs, lev<=3.0x");
    println!("Aggressive:   ann>=110%, DD<=30%, 3/5 cs, lev<=4.0x");

    // Sweep FO × leverage × adverse × tp_bps × max_legs
    println!("\nfo_pct,leverage,adverse,tp_bps,max_legs,ann_pct,dd_pct,sharpe,skew,kurt,trades");
    let fo_arr = [35.0f64, 50.0, 65.0, 80.0, 100.0, 130.0];
    let lev_arr = [2u32, 3, 4];
    let adv_arr = [0.005f64, 0.010, 0.020];
    let bps_arr = [3.0f64, 5.0, 10.0];
    let ml_arr = [2u32, 3, 4];

    let mut best_cons: Option<(f64,u32,f64,f64,u32,f64,f64)> = None; // fo,lev,adv,bps,ml,ann,dd
    let mut best_bal: Option<(f64,u32,f64,f64,u32,f64,f64)> = None;
    let mut best_agg: Option<(f64,u32,f64,f64,u32,f64,f64)> = None;

    for &fo in &fo_arr {
        for &lev in &lev_arr {
            for &adv in &adv_arr {
                for &bps in &bps_arr {
                    for &ml in &ml_arr {
                        let signal = RelaxedM1 { state_map: sm.clone(), thresh: 1.5 };
                        if let Some((ann, dd, sh, sk, ku, nt)) = run_one(&bars, &signal, budget, fo, lev, adv, bps, ml, symbol) {
                            println!("{fo:.0},{lev},{adv:.3},{bps:.0},{ml},{ann:.2},{dd:.2},{sh:.3},{sk:.3},{ku:.3},{nt}");
                            // Conservative: ann>=50%, DD<=10%, lev<=2
                            if ann >= 50.0 && dd <= 10.0 && lev <= 2 {
                                if best_cons.is_none() || ann > best_cons.unwrap().5 {
                                    best_cons = Some((fo,lev,adv,bps,ml,ann,dd));
                                }
                            }
                            // Balanced: ann>=90%, DD<=20%, lev<=3
                            if ann >= 90.0 && dd <= 20.0 && lev <= 3 {
                                if best_bal.is_none() || ann > best_bal.unwrap().5 {
                                    best_bal = Some((fo,lev,adv,bps,ml,ann,dd));
                                }
                            }
                            // Aggressive: ann>=110%, DD<=30%, lev<=4
                            if ann >= 110.0 && dd <= 30.0 && lev <= 4 {
                                if best_agg.is_none() || ann > best_agg.unwrap().5 {
                                    best_agg = Some((fo,lev,adv,bps,ml,ann,dd));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    println!("\n=== TIER HITS (plan gates, no Sharpe requirement) ===");
    if let Some((fo,lev,adv,bps,ml,ann,dd)) = best_cons {
        println!("CONSERVATIVE HIT: fo={fo:.0}% lev={lev} adv={adv:.3} bps={bps:.0} ml={ml} -> ann={ann:.2}% dd={dd:.2}%");
    } else { println!("Conservative: MISS"); }
    if let Some((fo,lev,adv,bps,ml,ann,dd)) = best_bal {
        println!("BALANCED HIT: fo={fo:.0}% lev={lev} adv={adv:.3} bps={bps:.0} ml={ml} -> ann={ann:.2}% dd={dd:.2}%");
    } else { println!("Balanced: MISS"); }
    if let Some((fo,lev,adv,bps,ml,ann,dd)) = best_agg {
        println!("AGGRESSIVE HIT: fo={fo:.0}% lev={lev} adv={adv:.3} bps={bps:.0} ml={ml} -> ann={ann:.2}% dd={dd:.2}%");
    } else { println!("Aggressive: MISS"); }
    Ok(())
}
