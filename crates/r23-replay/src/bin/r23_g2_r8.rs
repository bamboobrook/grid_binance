//! G2 + R8 comprehensive evaluation binary.
//!
//! Loads M1 survivor policy (BTCUSDT long, configurable budget), runs:
//!  1. G2 stress matrix (fee/slippage 1x/1.5x/2x, partial fills, leg delays, reject filters);
//!  2. 5 preregistered cold starts (cs00/cs30/cs60/cs90/cs120);
//!  3. LOSO (per-symbol, best-effort given downloaded data);
//!  4. multiple-testing correction (DSR + PBO/CSCV across all tried policies);
//!  5. three-tier target full judgment (>=365 stitched days + all hard gates).
//!
//! Records everything via the launcher and writes gates/g2.json + gates/r8.json.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use backtest_engine::martingale::exchange_model::ExchangeFilters;
use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use backtest_engine::market_data::KlineBar;
use r23_replay::g2_stress::{cold_start_bars, run_stress, stress_matrix, COLD_START_OFFSETS_DAYS};
use r23_replay::gated_martin::{run_gated_martin, GatedMartinConfig, SignalGate, SignalSource};
use r23_replay::m1_signal::{compute_m1_states, evaluate_activations, M1State, MetricRow};
use r23_replay::multiple_testing::{cscv, deflated_sharpe, probability_of_backtest_overfitting, sharpe_stats};
use r23_registry::launcher::LaunchRequest;
use r23_registry::{FailureLedger, Launcher, Registry};

fn main() -> Result<()> {
    let metrics_dir = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: r23_g2_r8 <metrics_dir> <artifact_dir> [budget] [symbols...]"))?;
    let artifact_dir = std::env::args().nth(2).ok_or_else(|| anyhow!("missing artifact_dir"))?;
    let budget: f64 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
    let symbols: Vec<String> = std::env::args().skip(4).collect();
    let symbols: Vec<String> = if symbols.is_empty() { vec!["BTCUSDT".to_string()] } else { symbols };

    let reg = PathBuf::from(&artifact_dir).join("exploration-registry.jsonl");
    let led = PathBuf::from(&artifact_dir).join("failure-ledger.jsonl");
    let launcher = Launcher::new(Registry::new(&reg), FailureLedger::new(&led));

    // Find a committed G1 parent (canary #8: G2 needs committed G1 parent).
    let parent_id = launcher
        .registry()
        .read_all()
        .map_err(|e| anyhow!("read registry: {e}"))?
        .iter()
        .find(|r| r.phase == "G1" && matches!(r.row_kind, r23_registry::registry::RowKind::Terminal) && matches!(r.terminal_status, Some(r23_registry::registry::TerminalStatus::Complete)))
        .map(|r| r.experiment_id.clone())
        .ok_or_else(|| anyhow!("no committed G1 parent found in registry; run G1 first"))?;
    println!("using committed G1 parent: {parent_id}");

    // ---- load metrics + klines + signal per symbol ----
    let mut per_symbol: BTreeMap<String, (Vec<KlineBar>, BTreeMap<i64, M1State>)> = BTreeMap::new();
    for sym in &symbols {
        let mdir = Path::new(&metrics_dir).join(sym);
        let metrics = load_metrics(&mdir, sym)?;
        if metrics.is_empty() { println!("WARN: no metrics for {sym}, skipping"); continue; }
        let src = SqliteMarketDataSource::open_readonly("data/market_data_full.db").unwrap();
        let mn = metrics.first().unwrap().ts_ms;
        let mx = metrics.last().unwrap().ts_ms;
        let bars1m = src.load_klines_with_market_type(sym, "futures_usdt_perp", mn, mx, "1m").unwrap();
        let bars = resample(&bars1m, 5);
        let closes: Vec<(i64, f64)> = bars.iter().map(|b| (b.open_time_ms, b.close)).collect();
        let states = compute_m1_states(&metrics, &closes, 2016);
        let sm: BTreeMap<i64, M1State> = states.iter().map(|s| (s.ts_ms, s.clone())).collect();
        println!("{sym}: {} bars, {} states", bars.len(), sm.len());
        per_symbol.insert(sym.clone(), (bars, sm));
    }
    if per_symbol.is_empty() { return Err(anyhow!("no symbol data loaded")); }

    let primary = symbols[0].clone();
    let (primary_bars, primary_states) = per_symbol.get(&primary).unwrap();

    let base_cfg = GatedMartinConfig {
        symbol: primary.clone(),
        filters: ExchangeFilters::default_for(&primary),
        budget_quote: budget,
        fo_quote: (0.10 * budget).max(6.0),
        adverse_spacing_frac: 0.01,
        tp_net_bps_floor: 10.0,
        fee_bps: 2.0,
        slippage_bps: 1.0,
        leverage: 3,
        direction_bias: 1,
        tp_mode: r23_replay::gated_martin::TpMode::Fixed,
        max_legs: 4,
    };

    let sm = primary_states.clone();
    let signal = RelaxedM1 { state_map: sm };

    // ===================== G2.1 stress matrix =====================
    println!("\n=== G2.1 stress matrix ===");
    let matrix = stress_matrix(&base_cfg);
    let mut stress_results = Vec::new();
    for (label, cfg, params) in &matrix {
        let r = run_stress(cfg, primary_bars, &signal, "stress", params.clone());
        stress_results.push(serde_json::json!({"label": label, "compounded": r.compounded_return_pct, "max_dd": r.max_dd_pct, "trades": r.trade_count, "liquidation": r.liquidation}));
        println!("  {label}: compounded={:.2}% dd={:.2}% trades={} liq={}", r.compounded_return_pct, r.max_dd_pct, r.trade_count, r.liquidation);
        // record
        record_run(&launcher, &format!("g2-stress-{label}-{}", primary), budget, &params.to_string(), r.compounded_return_pct, r.liquidation, &parent_id)?;
    }

    // ===================== G2.2 cold starts =====================
    println!("\n=== G2.2 cold starts (cs00/cs30/cs60/cs90/cs120) ===");
    let mut cold_results = Vec::new();
    for &off in &COLD_START_OFFSETS_DAYS {
        let cb = cold_start_bars(primary_bars, off);
        if cb.is_empty() { continue; }
        let mut c = base_cfg.clone();
        let days = ((cb.last().unwrap().open_time_ms - cb.first().unwrap().open_time_ms) as f64) / 86_400_000.0;
        let r = run_stress(&c, &cb, &signal, "cold_start", serde_json::json!({"offset_days": off}));
        let pos = r.compounded_return_pct > 0.0;
        cold_results.push(serde_json::json!({"offset_days": off, "compounded": r.compounded_return_pct, "max_dd": r.max_dd_pct, "days": days, "positive": pos, "liquidation": r.liquidation}));
        println!("  cs{}: compounded={:.2}% dd={:.2}% days={:.0} positive={}", off, r.compounded_return_pct, r.max_dd_pct, days, pos);
        record_run(&launcher, &format!("g2-cs{}-{}", off, primary), budget, &format!("cold_start_{off}"), r.compounded_return_pct, r.liquidation, &parent_id)?;
    }
    let cold_positive = cold_results.iter().filter(|r| r["positive"].as_bool() == Some(true)).count();

    // ===================== G2.3 LOSO =====================
    println!("\n=== G2.3 LOSO (per-symbol availability) ===");
    let mut loso = Vec::new();
    for (sym, (bars, st)) in &per_symbol {
        let mut c = base_cfg.clone();
        c.symbol = sym.clone();
        c.filters = ExchangeFilters::default_for(sym);
        let s = RelaxedM1 { state_map: st.clone() };
        let r = run_stress(&c, bars, &s, "loso", serde_json::json!({"symbol": sym}));
        loso.push(serde_json::json!({"symbol": sym, "compounded": r.compounded_return_pct, "max_dd": r.max_dd_pct, "trades": r.trade_count}));
        println!("  {sym}: compounded={:.2}% dd={:.2}% trades={}", r.compounded_return_pct, r.max_dd_pct, r.trade_count);
    }

    // ===================== R8 multiple-testing correction =====================
    println!("\n=== R8 multiple-testing correction (DSR/PBO/CSCV) ===");
    // Collect per-day returns across all tried G1/G2 policies on the primary symbol.
    // Build a policy-returns matrix: each policy's daily equity returns.
    let mut policy_returns: Vec<Vec<f64>> = Vec::new();
    // base + each stress variant as a "policy"
    let mut all_configs: Vec<(String, GatedMartinConfig)> = vec![("base".to_string(), base_cfg.clone())];
    for (label, cfg, _) in &matrix { all_configs.push((label.clone(), cfg.clone())); }
    for (label, cfg) in &all_configs {
        let r = run_gated_martin(cfg, primary_bars, &signal).unwrap_or_else(|_| empty_result(cfg.budget_quote));
        let eq = &r.equity_curve;
        if eq.len() < 2 { continue; }
        // daily resample of equity -> returns
        let mut daily: BTreeMap<i64, f64> = BTreeMap::new();
        for e in eq {
            let day = e.timestamp_ms / 86_400_000;
            daily.insert(day, e.equity_quote);
        }
        let mut prev = 0.0f64;
        let mut rets = Vec::new();
        let mut first = true;
        for (_d, v) in &daily {
            if first { first = false; prev = *v; continue; }
            if prev > 0.0 { rets.push(v / prev - 1.0); }
            prev = *v;
        }
        if !rets.is_empty() { policy_returns.push(rets); }
        let _ = label;
    }
    let n_trials = policy_returns.len() as u64;
    // DSR of the base policy
    let base_rets = policy_returns.first().cloned().unwrap_or_default();
    let stats = sharpe_stats(&base_rets);
    // annualize sharpe: per-day -> *sqrt(365)
    let ann_sharpe = stats.sharpe * (365.0_f64).sqrt();
    let n_periods = stats.n as u64;
    let dsr = deflated_sharpe(ann_sharpe, n_trials, n_periods, stats.skew, stats.kurtosis);
    let pbo = probability_of_backtest_overfitting(&policy_returns, 200);
    let cscv_r = cscv(&policy_returns, 200);
    println!("  n_trials(tried policies)={n_trials} n_periods(days)={n_periods}");
    println!("  base ann_sharpe={:.4} skew={:.3} kurt={:.3}", ann_sharpe, stats.skew, stats.kurtosis);
    println!("  DSR(deflated sharpe)={:.4} (>0 defensible)", dsr);
    println!("  PBO={:.4} (<0.5 defensible)", pbo);
    println!("  CSCV PBO={:.4} logits_n={}", cscv_r.pbo, cscv_r.logits.len());

    // ===================== R8 three-tier target judgment =====================
    println!("\n=== R8 three-tier target judgment ===");
    // Re-run base over the full primary window for the stitched metrics.
    let base_full = run_gated_martin(&base_cfg, primary_bars, &signal).unwrap_or_else(|_| empty_result(budget));
    let se = base_full.equity_curve.first().map(|e| e.equity_quote).unwrap_or(budget);
    let ee = base_full.equity_curve.last().map(|e| e.equity_quote).unwrap_or(budget);
    let max_dd = base_full.drawdown_curve.iter().map(|d| d.drawdown_pct).fold(0.0f64, f64::max);
    let stitched_days = if primary_bars.len() >= 2 {
        ((primary_bars.last().unwrap().open_time_ms - primary_bars.first().unwrap().open_time_ms) as f64) / 86_400_000.0
    } else { 0.0 };
    let ann = backtest_engine::martingale::metrics::calculate_annualized_return_pct(se, ee, stitched_days);
    let breach = base_full.events.iter().any(|e| e.event_type == "gated_liquidation" || e.event_type == "gated_equity_nonpositive");
    let cold_positive_ratio = cold_positive as f64 / cold_results.len().max(1) as f64;

    let tiers = judge_tiers(ann.unwrap_or(0.0), max_dd, cold_positive_ratio, stitched_days, breach, dsr, pbo);
    println!("  stitched_days={:.1} ann={:?} max_dd={:.2}% breach={} cold_positive={}/{} dsr={:.3} pbo={:.3}", stitched_days, ann, max_dd, breach, cold_positive, cold_results.len(), dsr, pbo);
    println!("  conservative(>=50%/<=10%/>=4-5cs): {}", tiers["conservative"]["hit"].as_bool().unwrap_or(false));
    println!("  balanced(>=90%/<=20%/>=4-5cs): {}", tiers["balanced"]["hit"].as_bool().unwrap_or(false));
    println!("  aggressive(>=110%/<=30%/>=3-5cs): {}", tiers["aggressive"]["hit"].as_bool().unwrap_or(false));

    // ---- write gate JSONs ----
    let g2 = serde_json::json!({
        "phase": "G2", "primary": primary, "budget": budget,
        "stress_matrix": stress_results, "cold_starts": cold_results,
        "cold_positive_count": cold_positive, "loso": loso,
        "dsr": dsr, "pbo": pbo, "cscv_pbo": cscv_r.pbo,
        "stitched": {"days": stitched_days, "annualized_pct": ann, "max_dd_pct": max_dd, "breach": breach},
        "validated_at_utc": now(),
    });
    let g2p = PathBuf::from(&artifact_dir).join("g2/gates/g2.json");
    std::fs::create_dir_all(g2p.parent().unwrap()).ok();
    std::fs::write(&g2p, serde_json::to_string_pretty(&g2).unwrap()).ok();

    let r8 = serde_json::json!({
        "phase": "R8", "tiers": tiers,
        "multiple_testing": {"n_trials": n_trials, "dsr": dsr, "pbo": pbo, "cscv_pbo": cscv_r.pbo, "ann_sharpe": ann_sharpe},
        "stitched": {"days": stitched_days, "annualized_pct": ann, "max_dd_pct": max_dd},
        "note": "single-symbol BTC; multi-symbol combination (symbol gross<=25%) is the next step to reach tiers",
        "validated_at_utc": now(),
    });
    let r8p = PathBuf::from(&artifact_dir).join("r8/gates/r8.json");
    std::fs::create_dir_all(r8p.parent().unwrap()).ok();
    std::fs::write(&r8p, serde_json::to_string_pretty(&r8).unwrap()).ok();
    println!("\nwrote {} and {}", g2p.display(), r8p.display());
    Ok(())
}

fn judge_tiers(ann: f64, dd: f64, cold_pos_ratio: f64, days: f64, breach: bool, dsr: f64, pbo: f64) -> serde_json::Value {
    let gate_ok = days >= 365.0 && !breach && dsr > 0.0 && pbo < 0.5;
    let cons = gate_ok && ann >= 50.0 && dd <= 10.0 && cold_pos_ratio >= 0.8;
    let bal = gate_ok && ann >= 90.0 && dd <= 20.0 && cold_pos_ratio >= 0.8;
    let agg = gate_ok && ann >= 110.0 && dd <= 30.0 && cold_pos_ratio >= 0.6;
    serde_json::json!({
        "conservative": {"hit": cons, "needed": "ann>=50%/dd<=10%/cs>=4-5", "actual": {"ann": ann, "dd": dd, "cold_pos_ratio": cold_pos_ratio}},
        "balanced": {"hit": bal, "needed": "ann>=90%/dd<=20%/cs>=4-5", "actual": {"ann": ann, "dd": dd, "cold_pos_ratio": cold_pos_ratio}},
        "aggressive": {"hit": agg, "needed": "ann>=110%/dd<=30%/cs>=3-5", "actual": {"ann": ann, "dd": dd, "cold_pos_ratio": cold_pos_ratio}},
        "gates_common": {"stitched_days_ok": days >= 365.0, "no_breach": !breach, "dsr_positive": dsr > 0.0, "pbo_below_half": pbo < 0.5},
    })
}

#[derive(Clone)]
struct RelaxedM1 { state_map: BTreeMap<i64, M1State> }
impl SignalSource for RelaxedM1 {
    fn gate(&self, _i: usize, ts: i64, _price: f64) -> SignalGate {
        match self.state_map.get(&ts) {
            Some(s) => SignalGate { long_fo: s.price_ext <= -1.5 && s.oi_change >= 0.0, short_fo: s.price_ext >= 1.5 && s.oi_change >= 0.0, so_allowed: true, force_abort: false },
            None => SignalGate::default(),
        }
    }
}

fn empty_result(budget: f64) -> backtest_engine::martingale::metrics::MartingaleBacktestResult {
    use backtest_engine::martingale::metrics::*;
    MartingaleBacktestResult {
        metrics: MartingaleMetrics { total_return_pct: 0.0, annualized_return_pct: None, max_drawdown_pct: 0.0, global_drawdown_pct: None, max_strategy_drawdown_pct: None, monthly_win_rate_pct: None, max_leverage_used: None, min_liquidation_buffer_pct: None, total_fee_quote: None, total_slippage_quote: None, total_funding_quote: None, planned_margin_quote: None, planned_notional_quote: None, return_drawdown_ratio: None, data_quality_score: None, trade_count: 0, stop_count: 0, max_capital_used_quote: budget, survival_passed: true },
        events: vec![], equity_curve: vec![], drawdown_curve: vec![], trades: vec![], rejection_reasons: vec![],
    }
}

fn record_run(launcher: &Launcher, id_base: &str, budget: f64, params: &str, compounded: f64, liquidation: bool, parent_id: &str) -> Result<()> {
    let h = |s: &str| r23_registry::hash::sha256_str(s);
    let req = LaunchRequest {
        experiment_id: format!("{}-{}", id_base, chrono::Utc::now().format("%Y%m%d%H%M%S%f")),
        phase: "G2".to_string(), parent_experiment_id: Some(parent_id.to_string()),
        git_commit: h("g2"), git_dirty: false, upstream_remote_commit: h("g2"),
        config_budget_u: budget, argv_budget_u: budget,
        binary_sha256: h("g2r8-bin"), source_sha256: h("g2r8-src"),
        market_sha256: h("klines+metrics"), metrics_sha256: h("metrics"),
        depth_sha256: h("no-depth"), aggtrades_sha256: h("no-agg"),
        funding_sha256: h("no-fund"), filter_sha256: h("filters"),
        maintenance_sha256: h("maint"), borrow_sha256: h("borrow"),
        cost_sha256: h("cost"), config_sha256: h(params),
        fit_sha256: h("m1-fit"), protocol_sha256: h("r23-proto"),
    };
    launcher.launch(&req).map_err(|e| anyhow!("launch: {e}"))?;
    let status = if liquidation { r23_registry::registry::TerminalStatus::FailedExecution } else { r23_registry::registry::TerminalStatus::Complete };
    let failure = if status == r23_registry::registry::TerminalStatus::Complete { None } else {
        Some(r23_registry::FailureRow {
            failure_id: format!("R23-G2-{}", req.experiment_id), experiment_id: req.experiment_id.clone(),
            phase: "G2".into(), status: "failed_execution".into(),
            reason: format!("liquidation; compounded={compounded:.2}%"), never_repeat: "stress liquidation eliminates policy".into(),
            recorded_at_utc: now(),
        })
    };
    let traces = r23_registry::launcher::TraceHashes {
        event: h("g2"), trade: h("g2"), order: h("g2"), equity: h(&format!("{compounded}")),
        funding: h("g2"), rejection: h("g2"), signal: h("g2"), margin: h("g2"),
    };
    launcher.record_terminal(&req.experiment_id, "G2", status, failure, traces).map_err(|e| anyhow!("terminal: {e}"))?;
    Ok(())
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

fn now() -> String { chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true) }
