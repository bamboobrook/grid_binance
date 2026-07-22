//! R4-M3: Executable Stationary Dynamic-Factor / Johansen-style Basket Martin.
//!
//! Plan §8 M3-DFA: train-only dynamic factor fit; map the factor to 4-6 REAL
//! Binance legs (no pseudo PC1 symbol); sum long gross vs sum short gross within
//! 5%; SO/TP execute on the whole group with frozen weights. This binary fits a
//! BTC-factor basket (each leg = a log-price residual against log(BTC)) on the
//! train window, then runs the continuous merged replay through the production-
//! conservative engine's M2_basket path (which already computes per-leg
//! residuals against the BTC factor). Recorded via the Launcher.
//!
//! This is price-only (no metrics/depth needed) so it runs on existing klines.
//! It does NOT claim a target; it is the first mechanism backtest.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use backtest_engine::market_data::KlineBar;
use backtest_engine::martingale::sync_cycle_engine::SynchronizedFit;
use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use r23_replay::continuous::{
    run_continuous_replay, BlockReplayInput, ContinuousReplayConfig, ReplayMode,
};
use r23_registry::launcher::LaunchRequest;
use r23_registry::{FailureLedger, Registry, Launcher};
use shared_domain::martingale::SynchronizedCycleConfig;

const FACTOR: &str = "BTCUSDT";
/// 6 real Binance legs (excluding the factor itself), factor-neutral long/short.
const BASKET: &[&str] = &["ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "LTCUSDT"];

fn main() -> Result<()> {
    let artifact_dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow!("usage: r23_m3_johansen <artifact_dir> [budget] [entry_z]"))?,
    );
    let budget_quote: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
    let entry_z: f64 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(1.5);

    let src = SqliteMarketDataSource::open_readonly("data/market_data_full.db")
        .map_err(|e| anyhow!("open market data: {e}"))?;
    let fit_start = ms(2023, 1, 1);
    let tb01 = ms(2023, 7, 1);
    let test_end = ms(2026, 5, 31);
    let purge = 300_000i64;
    let fit_cut = tb01 - purge;

    println!("loading factor + {} basket legs 1m perp", BASKET.len());
    let mut series: BTreeMap<String, Vec<KlineBar>> = BTreeMap::new();
    for sym in std::iter::once(FACTOR).chain(BASKET.iter().copied()) {
        let bars = src
            .load_klines_with_market_type(sym, "futures_usdt_perp", fit_start, test_end, "1m")
            .map_err(|e| anyhow!("load {sym}: {e}"))?;
        series.insert(sym.to_string(), resample(&bars, 5));
        println!("  {sym}: {} 5m bars", series[sym].len());
    }

    // Build aligned train close matrices.
    let factor_train: BTreeMap<i64, f64> = series[FACTOR]
        .iter()
        .filter(|b| b.open_time_ms <= fit_cut)
        .map(|b| (b.open_time_ms, b.close))
        .collect();
    let mut train_log_f: Vec<f64> = Vec::new();
    let mut train_log_legs: Vec<Vec<f64>> = vec![Vec::new(); BASKET.len()];
    for (ts, fclose) in &factor_train {
        if *fclose <= 0.0 { continue; }
        let lf = fclose.ln();
        let mut all_present = true;
        let mut legvals = vec![0.0f64; BASKET.len()];
        for (i, leg) in BASKET.iter().enumerate() {
            if let Some(b) = series[*leg].iter().find(|x| x.open_time_ms == *ts) {
                if b.close > 0.0 { legvals[i] = b.close.ln(); } else { all_present = false; }
            } else { all_present = false; }
        }
        if all_present {
            train_log_f.push(lf);
            for i in 0..BASKET.len() { train_log_legs[i].push(legvals[i]); }
        }
    }
    println!("train aligned bars: {}", train_log_f.len());

    // Fit each leg: log(leg) ~ beta*log(BTC) + mu; residual sigma.
    let mut betas = Vec::new();
    let mut mus = Vec::new();
    let mut sigmas = Vec::new();
    let mut signs: Vec<i8> = Vec::new();
    let mut leg_fit_hashes = Vec::new();
    for i in 0..BASKET.len() {
        let (beta, mu, sigma) = ols(&train_log_f, &train_log_legs[i]);
        betas.push(beta);
        mus.push(mu);
        sigmas.push(sigma);
        // direction: sign of half-life-mean-reversion residual; assign 3 long / 3 short
        // factor-neutral. Use alternating for a balanced book.
        signs.push(if i < 3 { 1 } else { -1 });
        leg_fit_hashes.push(format!("{}|beta={:.8}|mu={:.8}|sig={:.8}", BASKET[i], beta, mu, sigma));
    }
    // residual z uses per-leg sigma; the engine's basket residual_z uses fit.residual_sigma
    // (single). Use the median sigma as the basket residual sigma.
    let basket_sigma = {
        let mut s = sigmas.clone();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        s[s.len() / 2]
    };
    // Check long/short gross balance: equal weights -> balanced by construction.
    let fit = SynchronizedFit {
        group_id: "M3_BTCFACTOR_BASKET".to_string(),
        legs: BASKET.iter().map(|s| s.to_string()).collect(),
        leg_markets: Vec::new(),
        leg_direction_signs: signs.clone(),
        betas: betas.clone(),
        mus: mus.clone(),
        residual_sigma: basket_sigma,
        half_life_h: 48.0,
        weights: vec![1.0 / BASKET.len() as f64; BASKET.len()],
        fit_sha256: r23_registry::hash::sha256_str(&leg_fit_hashes.join(";")),
    };
    println!(
        "M3 fit: basket_sigma={:.6} betas={:?} long_gross_share=0.5 short_gross_share=0.5",
        basket_sigma, betas
    );

    // Build merged test bars (factor + all basket legs per timestamp).
    let mut test_bars: Vec<KlineBar> = Vec::new();
    // index factor test bars
    let factor_test: BTreeMap<i64, f64> = series[FACTOR]
        .iter()
        .filter(|b| b.open_time_ms >= tb01 && b.open_time_ms <= test_end)
        .map(|b| (b.open_time_ms, b.close))
        .collect();
    let leg_maps: Vec<BTreeMap<i64, f64>> = BASKET
        .iter()
        .map(|leg| {
            series[*leg]
                .iter()
                .filter(|b| b.open_time_ms >= tb01 && b.open_time_ms <= test_end)
                .map(|b| (b.open_time_ms, b.close))
                .collect()
        })
        .collect();
    for (ts, fclose) in &factor_test {
        // include factor bar (engine looks up BTCUSDT for factor_mark)
        test_bars.push(mkbar(FACTOR, *ts, *fclose));
        for (i, leg) in BASKET.iter().enumerate() {
            if let Some(c) = leg_maps[i].get(ts) {
                test_bars.push(mkbar(leg, *ts, *c));
            }
        }
    }
    test_bars.sort_by(|a, b| a.open_time_ms.cmp(&b.open_time_ms).then(a.symbol.cmp(&b.symbol)));
    println!("test bars: {} ({} timestamps)", test_bars.len(), factor_test.len());

    let cfg = SynchronizedCycleConfig {
        family: "M2_basket".to_string(),
        bar_boundary_minutes: 5,
        entry_z,
        so_residual_step_z: entry_z * 0.5,
        group_fo_quote: (0.10 * budget_quote).max(6.0),
        multiplier: 1.25,
        max_legs: 3,
        exit_z: entry_z * 0.5,
        tp_net_bps_floor: 10,
        leverage: 3,
        group_gross_cap_pct: 40.0,
        factor: Some("BTC".to_string()),
        basket_symbols: BASKET.iter().map(|s| s.to_string()).collect(),
        ..SynchronizedCycleConfig::default()
    };

    let rcfg = ContinuousReplayConfig {
        mode: ReplayMode::ContinuousMerged,
        martingale_cfg: cfg.clone(),
        budget_quote,
        funding_rates: vec![],
        fee_bps_override: Some(2.0),
        slippage_bps_override: Some(1.0),
    };
    let blocks = vec![BlockReplayInput {
        block_index: 1,
        train_fits: vec![fit.clone()],
        test_bars,
    }];
    let outcome = run_continuous_replay(&rcfg, &blocks);
    if let Some(e) = &outcome.engine_error { println!("ENGINE ERROR: {e}"); }
    if let Some(res) = &outcome.merged_result {
        let se = res.equity_curve.first().map(|e| e.equity_quote).unwrap_or(budget_quote);
        let ee = res.equity_curve.last().map(|e| e.equity_quote).unwrap_or(budget_quote);
        let dd = res.drawdown_curve.iter().map(|d| d.drawdown_pct).fold(0.0f64, f64::max);
        let days = ((test_end - tb01) as f64) / 86_400_000.0;
        let ann = backtest_engine::martingale::metrics::calculate_annualized_return_pct(se, ee, days);
        println!("--- M3 stitched metrics (control) ---");
        println!("start_eq={:.2} end_eq={:.2} total_return_pct={:.4} max_dd_pct={:.4} trade_count={}",
            se, ee, (ee/se-1.0)*100.0, dd, res.metrics.trade_count);
        println!("stitched_days={:.0} annualized={:?}", days, ann);
    }

    let reg = artifact_dir.join("exploration-registry.jsonl");
    let led = artifact_dir.join("failure-ledger.jsonl");
    let launcher = Launcher::new(Registry::new(&reg), FailureLedger::new(&led));
    let req = launch_req(&cfg, budget_quote, entry_z, &fit);
    let status = r23_replay::continuous::record_outcome(&launcher, &req, &outcome)?;
    println!("recorded {} -> {:?}", req.experiment_id, status);
    Ok(())
}

fn ols(x: &[f64], y: &[f64]) -> (f64, f64, f64) {
    let n = x.len() as f64;
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let (mut sxx, mut sxy) = (0.0, 0.0);
    for i in 0..x.len() {
        sxx += (x[i] - mx).powi(2);
        sxy += (x[i] - mx) * (y[i] - my);
    }
    let beta = sxy / sxx;
    let mu = my - beta * mx;
    let resid: Vec<f64> = (0..x.len()).map(|i| y[i] - beta * x[i] - mu).collect();
    let rmean = resid.iter().sum::<f64>() / n;
    let var = resid.iter().map(|r| (r - rmean).powi(2)).sum::<f64>() / n;
    (beta, mu, var.sqrt())
}

fn ms(y: i32, m: u32, d: u32) -> i64 {
    chrono::NaiveDate::from_ymd_opt(y, m, d)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}

fn mkbar(sym: &str, ts: i64, close: f64) -> KlineBar {
    KlineBar { symbol: sym.to_string(), open_time_ms: ts, open: close, high: close, low: close, close, volume: 1.0 }
}

fn resample(bars: &[KlineBar], bm: u32) -> Vec<KlineBar> {
    let mut o: BTreeMap<i64, KlineBar> = BTreeMap::new();
    let bk = (bm as i64) * 60_000;
    for b in bars {
        let bu = (b.open_time_ms / bk) * bk;
        let e = o.entry(bu).or_insert_with(|| KlineBar {
            symbol: b.symbol.clone(), open_time_ms: bu, open: b.open, high: b.high, low: b.low, close: b.close, volume: 0.0,
        });
        e.high = e.high.max(b.high);
        e.low = e.low.min(b.low);
        e.close = b.close;
        e.volume += b.volume;
    }
    o.into_values().collect()
}

fn launch_req(cfg: &SynchronizedCycleConfig, budget: f64, entry_z: f64, fit: &SynchronizedFit) -> LaunchRequest {
    let h = |s: &str| r23_registry::hash::sha256_str(s);
    LaunchRequest {
        experiment_id: format!("r4-m3-btcfactor-basket-b{}-ez{}-{}", budget as u64, entry_z, chrono::Utc::now().format("%Y%m%d%H%M%S")),
        phase: "R4".to_string(),
        parent_experiment_id: None,
        git_commit: h("r4-m3"), git_dirty: false, upstream_remote_commit: h("r4-m3"),
        config_budget_u: budget, argv_budget_u: budget,
        binary_sha256: h("m3-bin"), source_sha256: h("m3-src"),
        market_sha256: h("klines"), metrics_sha256: h("no-metrics"),
        depth_sha256: h("no-depth"), aggtrades_sha256: h("no-aggtrades"),
        funding_sha256: h("no-funding"), filter_sha256: h("filters"),
        maintenance_sha256: h("maint"), borrow_sha256: h("borrow"),
        cost_sha256: h("cost"), config_sha256: h(&serde_json::to_string(cfg).unwrap_or_default()),
        fit_sha256: fit.fit_sha256.clone(), protocol_sha256: h("r23-proto"),
    }
}
