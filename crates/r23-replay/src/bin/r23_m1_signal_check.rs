//! R4-M1 signal validation: loads downloaded metrics + aligned klines, computes
//! the OI/Crowding Exhaustion state and activation flags, reports counts.
//! This proves the M1 signal layer runs on real data; the engine wiring
//! (FO/SO/TP/abort gated by these flags) is the remaining M1 work.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Result};

use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use backtest_engine::market_data::KlineBar;
use r23_replay::m1_signal::{compute_m1_states, evaluate_activations, MetricRow};

fn main() -> Result<()> {
    let metrics_dir = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("usage: r23_m1_signal_check <metrics_dir> [symbol]"))?;
    let symbol = std::env::args().nth(2).unwrap_or_else(|| "BTCUSDT".to_string());
    let sym_dir = Path::new(&metrics_dir).join(&symbol);
    println!("loading metrics for {symbol} from {}", sym_dir.display());

    // Parse every downloaded metrics zip for this symbol.
    let mut metrics: Vec<MetricRow> = Vec::new();
    for entry in std::fs::read_dir(&sym_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("zip") { continue; }
        let bytes = std::fs::read(&path)?;
        let mut za = zip::ZipArchive::new(std::io::Cursor::new(bytes.as_slice()))
            .map_err(|e| anyhow!("zip open {:?}: {e}", path))?;
        let name = za.by_index(0).map_err(|e| anyhow!("zip idx: {e}"))?.name().to_string();
        let mut buf = Vec::new();
        za.by_name(&name).map_err(|e| anyhow!("zip name: {e}"))?.read_to_end(&mut buf)?;
        let text = String::from_utf8_lossy(&buf);
        let mut rdr = csv::ReaderBuilder::new().has_headers(true).from_reader(text.as_bytes());
        for rec in rdr.records() {
            let r = match rec { Ok(r) => r, Err(_) => continue };
            // create_time,symbol,sum_open_interest,sum_open_interest_value,count_toptrader...,
            // sum_toptrader_long_short_ratio,count_long_short_ratio,sum_taker_long_short_vol_ratio
            let ts = match parse_ts(&r[0]) { Some(t) => t, None => continue };
            let oiv: f64 = r.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let top: f64 = r.get(5).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            let all: f64 = r.get(6).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            let taker: f64 = r.get(7).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            metrics.push(MetricRow { ts_ms: ts, symbol: symbol.clone(), open_interest_value: oiv, top_trader_ls_ratio: top, all_trader_ls_ratio: all, taker_ls_vol_ratio: taker });
        }
    }
    metrics.sort_by_key(|m| m.ts_ms);
    println!("parsed {} metric rows", metrics.len());
    if metrics.is_empty() { return Err(anyhow!("no metrics parsed")); }

    // Load matching klines (5m) for the same window.
    let src = SqliteMarketDataSource::open_readonly("data/market_data_full.db").unwrap();
    let mn = metrics.first().unwrap().ts_ms;
    let mx = metrics.last().unwrap().ts_ms;
    let bars = src.load_klines_with_market_type(&symbol, "futures_usdt_perp", mn, mx, "1m").unwrap();
    let r = resample(&bars, 5);
    let closes: Vec<(i64, f64)> = r.iter().map(|b| (b.open_time_ms, b.close)).collect();
    println!("aligned klines: {} 5m bars in window", closes.len());

    let window = 2016; // 7 days * 288
    let states = compute_m1_states(&metrics, &closes, window);
    println!("computed {} M1 states", states.len());
    let acts = evaluate_activations(&states, window, 0.95);
    let long_fo = acts.iter().filter(|a| a.long_fo).count();
    let short_fo = acts.iter().filter(|a| a.short_fo).count();
    let long_so = acts.iter().filter(|a| a.long_so_exhaustion).count();
    let short_so = acts.iter().filter(|a| a.short_so_exhaustion).count();
    println!("--- M1 activation counts (window=7d, q=0.95) ---");
    println!("long_fo={long_fo} short_fo={short_fo} long_so_exhaustion={long_so} short_so_exhaustion={short_so}");
    println!("signal state SHA-256: {}", r23_registry::hash::sha256_str(&format!("{:?}", &states[..states.len().min(100)])));
    Ok(())
}

fn parse_ts(s: &str) -> Option<i64> {
    // "2023-07-01 00:00:00"
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
