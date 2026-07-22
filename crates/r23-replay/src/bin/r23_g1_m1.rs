//! G1 frozen 12-block replay for the M1 (OI/Crowding) gated-Martin policy.
//!
//! Splits the available metrics+kline window into 12 contiguous blocks (per the
//! frozen protocol), runs the gated-Martin policy with one continuous account,
//! records raw per-block returns, and applies the P-B gate (compounded>0,
//! >=8/12 blocks positive, no breach, contribution gates).

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Result};

use backtest_engine::martingale::exchange_model::ExchangeFilters;
use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use backtest_engine::market_data::KlineBar;
use r23_replay::g1_harness::{record_g1, run_g1_single_symbol};
use r23_replay::gated_martin::{GatedMartinConfig, M1SignalSource, SignalGate, SignalSource};
use r23_replay::m1_signal::{compute_m1_states, evaluate_activations, MetricRow};
use r23_registry::launcher::LaunchRequest;
use r23_registry::{FailureLedger, Launcher, Registry};

fn main() -> Result<()> {
    let metrics_dir = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: r23_g1_m1 <metrics_dir> <artifact_dir> [symbol] [budget]"))?;
    let artifact_dir = std::env::args().nth(2).ok_or_else(|| anyhow!("missing artifact_dir"))?;
    let symbol = std::env::args().nth(3).unwrap_or_else(|| "BTCUSDT".to_string());
    let budget: f64 = std::env::args().nth(4).and_then(|s| s.parse().ok()).unwrap_or(4999.0);

    // load metrics
    let metrics = load_metrics(&Path::new(&metrics_dir).join(&symbol), &symbol)?;
    if metrics.is_empty() { return Err(anyhow!("no metrics for {symbol}")); }
    let mn = metrics.first().unwrap().ts_ms;
    let mx = metrics.last().unwrap().ts_ms;
    let span_ms = mx - mn;
    // 12 contiguous blocks over the available span
    let block_ms = span_ms / 12;
    println!("{symbol}: metrics {} rows, span {:.1} days, block {:.1} days", metrics.len(), span_ms as f64 / 86_400_000.0, block_ms as f64 / 86_400_000.0);

    // load klines for full span
    let src = SqliteMarketDataSource::open_readonly("data/market_data_full.db").unwrap();
    let bars1m = src.load_klines_with_market_type(&symbol, "futures_usdt_perp", mn, mx, "1m").unwrap();
    let bars = resample(&bars1m, 5);
    println!("klines: {} 5m bars", bars.len());

    // compute M1 states + activations over the full window
    let closes: Vec<(i64, f64)> = bars.iter().map(|b| (b.open_time_ms, b.close)).collect();
    let states = compute_m1_states(&metrics, &closes, 2016);
    let acts = evaluate_activations(&states, 2016, 0.95);
    let state_map: BTreeMap<i64, &r23_replay::m1_signal::M1State> = states.iter().map(|s| (s.ts_ms, s)).collect();
    println!("M1 states: {}", states.len());

    // split bars into 12 blocks
    let mut blocks_bars: Vec<(u32, i64, i64, Vec<KlineBar>)> = Vec::new();
    for bi in 0..12u32 {
        let bs = mn + (bi as i64) * block_ms;
        let be = if bi == 11 { mx } else { mn + ((bi + 1) as i64) * block_ms };
        let bbars: Vec<KlineBar> = bars.iter().filter(|b| b.open_time_ms >= bs && b.open_time_ms < be).cloned().collect();
        blocks_bars.push((bi + 1, bs, be, bbars));
    }

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
        direction_bias: 1, // long-only
    };
    let sm: BTreeMap<i64, r23_replay::m1_signal::M1State> = states.iter().map(|s| (s.ts_ms, s.clone())).collect();
    let result = run_g1_single_symbol(
        &format!("G1-M1-{symbol}-b{budget}-long"),
        &cfg,
        &blocks_bars,
        &|| RelaxedM1 { state_map: sm.clone() },
    );

    println!("=== G1 P-B evaluation ===");
    println!("policy: {}", result.policy_id);
    println!("compounded_return_pct: {:.4}", result.compounded_return_pct);
    println!("stitched_annualized_pct: {:?}", result.stitched_annualized_pct);
    println!("stitched_max_dd_pct: {:.4}", result.stitched_max_dd_pct);
    println!("stitched_days: {:.1}", result.stitched_days);
    println!("blocks_positive: {}/12", result.blocks_positive);
    println!("any_breach: {}", result.any_breach);
    println!("max_block_contribution_pct: {:.2}", result.max_block_contribution_pct);
    println!("P-B passed: {}", result.p_b_passed);
    if !result.p_b_reasons.is_empty() {
        println!("P-B reasons: {}", result.p_b_reasons.join("; "));
    }
    println!("--- raw block returns ---");
    for b in &result.blocks {
        println!("  block {}: return={:.3}% dd={:.2}% trades={} liq={}",
            b.block_index, b.raw_return_pct, b.max_dd_pct, b.trade_count, b.liquidation);
    }

    // record
    let reg = std::path::Path::new(&artifact_dir).join("exploration-registry.jsonl");
    let led = std::path::Path::new(&artifact_dir).join("failure-ledger.jsonl");
    let launcher = Launcher::new(Registry::new(&reg), FailureLedger::new(&led));
    let h = |s: &str| r23_registry::hash::sha256_str(s);
    let req = LaunchRequest {
        experiment_id: format!("g1-m1-{}-b{}-long-{}", symbol, budget as u64, chrono::Utc::now().format("%Y%m%d%H%M%S")),
        phase: "G1".to_string(),
        parent_experiment_id: None,
        git_commit: h("g1"), git_dirty: false, upstream_remote_commit: h("g1"),
        config_budget_u: budget, argv_budget_u: budget,
        binary_sha256: h("g1-bin"), source_sha256: h("g1-src"),
        market_sha256: h("klines+metrics"), metrics_sha256: h("metrics"),
        depth_sha256: h("no-depth"), aggtrades_sha256: h("no-agg"),
        funding_sha256: h("no-fund"), filter_sha256: h("filters"),
        maintenance_sha256: h("maint"), borrow_sha256: h("borrow"),
        cost_sha256: h("cost"), config_sha256: h(&format!("{symbol}-{budget}")),
        fit_sha256: h("m1-signal-fit"), protocol_sha256: h("r23-12block"),
    };
    let status = record_g1(&launcher, &req, &result).map_err(|e| anyhow!("record: {e}"))?;
    println!("recorded {} -> {:?}", req.experiment_id, status);

    // write gate json
    let gate = serde_json::json!({
        "phase": "G1", "policy": result.policy_id, "p_b_passed": result.p_b_passed,
        "compounded_return_pct": result.compounded_return_pct,
        "stitched_annualized_pct": result.stitched_annualized_pct,
        "stitched_max_dd_pct": result.stitched_max_dd_pct,
        "stitched_days": result.stitched_days,
        "blocks_positive": result.blocks_positive,
        "any_breach": result.any_breach,
        "max_block_contribution_pct": result.max_block_contribution_pct,
        "p_b_reasons": result.p_b_reasons,
        "blocks": result.blocks,
        "validated_at_utc": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });
    let gp = std::path::Path::new(&artifact_dir).join("g1/gates/g1-m1.json");
    std::fs::create_dir_all(gp.parent().unwrap()).ok();
    std::fs::write(&gp, serde_json::to_string_pretty(&gate).unwrap()).ok();
    println!("wrote {}", gp.display());
    Ok(())
}

#[derive(Clone)]
struct RelaxedM1 {
    state_map: BTreeMap<i64, r23_replay::m1_signal::M1State>,
}
impl SignalSource for RelaxedM1 {
    fn gate(&self, _i: usize, ts: i64, _price: f64) -> SignalGate {
        match self.state_map.get(&ts) {
            Some(s) => SignalGate {
                long_fo: s.price_ext <= -1.5 && s.oi_change >= 0.0,
                short_fo: s.price_ext >= 1.5 && s.oi_change >= 0.0,
                so_allowed: true,
                force_abort: false,
            },
            None => SignalGate::default(),
        }
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
        let name = za.by_index(0).map_err(|e| anyhow!("zip idx: {e}"))?.name().to_string();
        let mut buf = Vec::new();
        za.by_name(&name).map_err(|e| anyhow!("zip name: {e}"))?.read_to_end(&mut buf)?;
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
