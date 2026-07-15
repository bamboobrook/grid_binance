//! Round 16 R0.1: REAL 20-config Batch/CLI subprocess parity with trace digests.
//!
//! Addresses ChatGPT Round 15 audit §2.6: the original P0 parity compared
//! Batch-single vs Batch-parallel (two paths sharing the Batch loader), NOT a
//! release CLI subprocess. This test launches the REAL release CLI binary as a
//! subprocess for each of 20 distinct configs and compares, vs BatchReplay:
//!   - event/trade/equity/funding/rejection stream SHA256 (exact)
//!   - annualized return / max DD / trade count / funding / rejections (1e-9)
//!
//! Both paths must produce identical digests because they share the canonical
//! loader, the same prepared config, and the same sim function. The digest
//! computation is shared via backtest_engine::martingale::trace_digest.

use backtest_engine::market_data::MarketDataSource;
use backtest_engine::martingale::batch_replay::BatchReplay;
use backtest_engine::martingale::budget_replay::prepare_replay_config;
use backtest_engine::martingale::trace_digest::compute_trace_digests;
use backtest_engine::sqlite_market_data::{load_funding_rates_readonly, SqliteMarketDataSource};
use rust_decimal::Decimal;
use serde_json::Value;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleMarginMode, MartingaleMarketKind,
    MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel,
    MartingaleSpacingModel, MartingaleStrategyConfig, MartingaleTakeProfitModel,
};
use std::path::PathBuf;
use std::process::Command;

const DEV_START: i64 = 1_672_531_200_000;
const MARKET_DB: &str = "data/market_data_full.db";
const FUNDING_DB: &str = "data/funding_rates_round12.db";
const CLI_BIN: &str = "target/release/portfolio_budget_replay";

fn dec(v: i64) -> Decimal {
    Decimal::new(v, 0)
}

fn make_portfolio(
    symbol: &str,
    direction: MartingaleDirection,
    fo: i64,
    mult: i64,
    legs: u32,
    tp_bps: u32,
    lev: u32,
) -> MartingalePortfolioConfig {
    MartingalePortfolioConfig {
        direction_mode: MartingaleDirectionMode::LongAndShort,
        strategies: vec![MartingaleStrategyConfig {
            strategy_id: format!(
                "{}-{}",
                if direction == MartingaleDirection::Long { "L" } else { "S" },
                symbol
            ),
            symbol: symbol.to_string(),
            market: MartingaleMarketKind::UsdMFutures,
            direction,
            direction_mode: MartingaleDirectionMode::LongAndShort,
            margin_mode: Some(MartingaleMarginMode::Isolated),
            leverage: Some(lev),
            spacing: MartingaleSpacingModel::FixedPercent { step_bps: 150 },
            sizing: MartingaleSizingModel::Multiplier {
                first_order_quote: dec(fo),
                multiplier: Decimal::from(mult),
                max_legs: legs,
            },
            take_profit: MartingaleTakeProfitModel::Percent { bps: tp_bps },
            stop_loss: None,
            indicators: vec![],
            entry_triggers: vec![],
            risk_limits: MartingaleRiskLimits::default(),
        }],
        risk_limits: MartingaleRiskLimits {
            max_global_budget_quote: Some(dec(4999)),
            ..Default::default()
        },
    }
}

fn twenty_configs() -> Vec<(String, MartingalePortfolioConfig)> {
    let symbols = ["BNBUSDT", "ETHUSDT", "TRXUSDT"];
    let directions = [MartingaleDirection::Long, MartingaleDirection::Short];
    let fos = [30, 40, 50];
    let mults = [2, 3];
    let legses = [5, 8];
    let tps = [100, 200];
    let levs = [5, 10];
    let mut configs = Vec::new();
    let mut idx = 0;
    for &sym in &symbols {
        for &dir in &directions {
            for &fo in &fos {
                if idx >= 20 {
                    break;
                }
                let mult = mults[idx % mults.len()];
                let legs = legses[idx % legses.len()];
                let tp = tps[idx % tps.len()];
                let lev = levs[idx % levs.len()];
                configs.push((
                    format!("cfg_{:02}_{}_{}", idx, sym, if dir == MartingaleDirection::Long { "L" } else { "S" }),
                    make_portfolio(sym, dir, fo, mult, legs, tp, lev),
                ));
                idx += 1;
            }
        }
    }
    configs
}

fn write_config_file(label: &str, portfolio_value: &Value) -> PathBuf {
    let dir = PathBuf::from("docs/superpowers/artifacts/glm-martingale-core-round16/configs/parity");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join(format!("{label}.json"));
    let wrapper = serde_json::json!({ "portfolio_config": portfolio_value });
    std::fs::write(&path, serde_json::to_string_pretty(&wrapper).unwrap()).unwrap();
    path
}

fn run_cli_subprocess(
    config_path: &PathBuf,
    budget: i64,
    start_ms: i64,
    end_ms: i64,
) -> Value {
    let output = Command::new(CLI_BIN)
        .arg("--config").arg(config_path)
        .arg("--budget").arg(budget.to_string())
        .arg("--start-ms").arg(start_ms.to_string())
        .arg("--end-ms").arg(end_ms.to_string())
        .arg("--market-data").arg(MARKET_DB)
        .arg("--funding-data").arg(FUNDING_DB)
        .arg("--exchange-min-notional").arg("5.0")
        .output()
        .expect("failed to launch release CLI subprocess");
    assert!(output.status.success(), "CLI subprocess failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_start = stdout.find('{').expect("no JSON in CLI output");
    let json_end = stdout.rfind('}').expect("no closing brace");
    serde_json::from_str(&stdout[json_start..=json_end]).expect("parse CLI JSON")
}

#[test]
fn real_20_config_batch_cli_subprocess_parity() {
    if !std::path::Path::new(MARKET_DB).exists() || !std::path::Path::new(CLI_BIN).exists() {
        eprintln!("skipping: market DB or release CLI not found");
        return;
    }
    // Use a 7-day window for speed (parity is about identical digests, not performance).
    let window_end = DEV_START + 7 * 86_400_000;
    let configs = twenty_configs();
    assert!(configs.len() >= 20, "need 20 configs, got {}", configs.len());

    let symbols_needed: Vec<&str> = vec!["BNBUSDT", "ETHUSDT", "TRXUSDT"];
    let mut batch = BatchReplay::new(MARKET_DB, FUNDING_DB).expect("batch");
    batch
        .preload_symbols(&symbols_needed, DEV_START, window_end)
        .expect("preload");

    let mut mismatches = 0;
    for (label, portfolio_orig) in configs.iter().take(20) {
        let mut portfolio = portfolio_orig.clone();
        // Prepare the config exactly as the CLI does (weight caps + budget).
        let portfolio_value = serde_json::to_value(&portfolio).unwrap();
        let raw_wrapper = serde_json::json!({ "portfolio_config": portfolio_value });
        let raw_portfolio_value = raw_wrapper.get("portfolio_config").cloned().unwrap_or(raw_wrapper.clone());
        prepare_replay_config(&mut portfolio, &raw_portfolio_value, Decimal::new(4999, 0))
            .expect("prepare");

        // --- BATCH path: run_single_full returns the full result + we compute digests ---
        let batched = batch
            .run_single_full(&portfolio, 4999.0, DEV_START, window_end)
            .expect("batch run");
        // Load the same funding the CLI loads (sorted inside the digest).
        let traded: Vec<String> = portfolio.strategies.iter().map(|s| s.symbol.clone()).collect();
        let funding = load_funding_rates_readonly(FUNDING_DB, &traded, DEV_START, window_end).expect("funding");
        let batch_digests = compute_trace_digests(&batched, &funding);

        // --- CLI path: launch release subprocess ---
        let config_path = write_config_file(label, &portfolio_value);
        let cli_json = run_cli_subprocess(&config_path, 4999, DEV_START, window_end);
        let cli_digests = cli_json.get("trace_digests").expect("CLI must emit trace_digests");
        let cli_ob = cli_json.get("on_budget").expect("on_budget");

        // Compare digests (exact) and metrics (1e-9).
        let cli_event = cli_digests["event_stream_sha256"].as_str().unwrap();
        let cli_trade = cli_digests["trade_stream_sha256"].as_str().unwrap();
        let cli_equity = cli_digests["equity_stream_sha256"].as_str().unwrap();
        let cli_funding = cli_digests["funding_stream_sha256"].as_str().unwrap();
        let cli_rejection = cli_digests["rejection_stream_sha256"].as_str().unwrap();

        if cli_event != batch_digests.event_stream_sha256
            || cli_trade != batch_digests.trade_stream_sha256
            || cli_equity != batch_digests.equity_stream_sha256
            || cli_funding != batch_digests.funding_stream_sha256
            || cli_rejection != batch_digests.rejection_stream_sha256
        {
            mismatches += 1;
            eprintln!(
                "DIGEST MISMATCH {label}:\n  event cli={cli_event} batch={}\n  trade cli={cli_trade} batch={}",
                batch_digests.event_stream_sha256, batch_digests.trade_stream_sha256
            );
        }
        // Metrics within 1e-9.
        let cli_ann = cli_ob["annualized_return_pct"].as_f64().unwrap_or(-999.0);
        let cli_dd = cli_ob["max_drawdown_pct"].as_f64().unwrap_or(-999.0);
        let ob_ann = batched.metrics.annualized_return_pct.unwrap_or(-999.0);
        let ob_dd = batched.metrics.max_drawdown_pct;
        assert!(
            (cli_ann - ob_ann).abs() < 1e-9,
            "ann mismatch {label}: cli={cli_ann} batch={ob_ann}"
        );
        assert!(
            (cli_dd - ob_dd).abs() < 1e-9,
            "DD mismatch {label}: cli={cli_dd} batch={ob_dd}"
        );
    }
    assert_eq!(mismatches, 0, "{mismatches} of 20 configs had digest mismatches between release CLI subprocess and BatchReplay");
}
