use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use r24_registry::sha256_file;

const PRINCIPAL: f64 = 2000.0;
const DAY_MS: i64 = 86_400_000;

fn main() -> Result<()> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let artifact = parse_artifact_root(&repo)?;
    let mut results = Vec::new();
    let mut violations = Vec::new();
    for entry in sorted_files(&artifact.join("replay-results"))? {
        let name = entry
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if !name.starts_with("full-R25-C1-") || !name.ends_with(".json") {
            continue;
        }
        let summary: serde_json::Value = serde_json::from_slice(&fs::read(&entry)?)?;
        let label = summary["label"].as_str().context("result label missing")?;
        let result = validate_policy(&repo, &artifact, label, &summary)?;
        violations.extend(
            result["violations"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str())
                .map(|value| format!("{label}:{value}")),
        );
        results.push(result);
    }
    if results.len() != 16 {
        violations.push(format!(
            "terminal_policy_count_expected_16_actual_{}",
            results.len()
        ));
    }
    for path in walk_files(&artifact)? {
        let bytes = fs::metadata(&path)?.len();
        if bytes > 10 * 1024 * 1024 {
            violations.push(format!(
                "committed_file_over_10mb:{}:{bytes}",
                path.display()
            ));
        }
    }
    let g1: serde_json::Value = serde_json::from_slice(&fs::read(artifact.join("gates/g1.json"))?)?;
    let p_b_survivors = g1["p_b_survivors"].as_u64().unwrap_or(0);
    let final_status = if p_b_survivors == 0 {
        "VALID_HISTORICAL_PREQUENTIAL_NO_TARGET"
    } else {
        "HISTORICAL_PREQUENTIAL_FRONTIER_PROGRESS"
    };
    let passed = violations.is_empty();
    let output = serde_json::json!({
        "validator_version":"round25-corrected-independent-v1",
        "passed":passed,"policy_count":results.len(),
        "recomputed_from_raw_traces":true,
        "replay_summary_metric_functions_called":false,
        "final_status":if passed {final_status} else {"INVALID_VALIDATION"},
        "policies":results,"violations":violations
    });
    write_json(artifact.join("independent-validator.json"), &output)?;
    if !passed {
        bail!("independent validation failed; see independent-validator.json");
    }
    finalize_authority(&repo, &artifact, final_status, p_b_survivors)?;
    println!("independent validator passed: policies={}", results.len());
    Ok(())
}

fn finalize_authority(
    repo: &Path,
    artifact: &Path,
    status: &str,
    p_b_survivors: u64,
) -> Result<()> {
    let authority_path = artifact.join("round25-corrected-authority.json");
    let mut authority: serde_json::Value = serde_json::from_slice(&fs::read(&authority_path)?)?;
    authority["status"] = serde_json::json!(status);
    authority["independent_validator_passed"] = serde_json::json!(true);
    authority["p_b_survivors"] = serde_json::json!(p_b_survivors);
    write_json(authority_path, &authority)?;

    let state_path = artifact.join("round25-corrected-execution-state.json");
    let mut state: serde_json::Value = serde_json::from_slice(&fs::read(&state_path)?)?;
    state["status"] = serde_json::json!(status);
    state["independent_validator_passed"] = serde_json::json!(true);
    write_json(state_path, &state)?;

    let report_path =
        repo.join("docs/superpowers/reports/2026-07-23-glm-round25-corrected-handoff.md");
    let mut report = fs::read_to_string(&report_path).unwrap_or_default();
    report.push_str(&format!(
        "\nIndependent validator: `PASS`. Final status: `{status}`. P-B survivors: `{p_b_survivors}`.\n"
    ));
    fs::write(report_path, report)?;
    Ok(())
}

fn parse_artifact_root(repo: &Path) -> Result<PathBuf> {
    let mut artifact =
        repo.join("docs/superpowers/artifacts/glm-martingale-core-round25-corrected");
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--artifact-root" => {
                index += 1;
                let supplied = PathBuf::from(args.get(index).context("missing artifact root")?);
                artifact = if supplied.is_absolute() {
                    supplied
                } else {
                    repo.join(supplied)
                };
            }
            other => bail!("unknown argument {other}"),
        }
        index += 1;
    }
    Ok(artifact)
}

#[derive(Default)]
struct Recomputed {
    wallet: f64,
    max_drawdown_pct: f64,
    peak_equity: f64,
    risk_rows: u64,
    first_risk_ms: Option<i64>,
    last_risk_ms: Option<i64>,
    entry_fee: f64,
    entry_slippage: f64,
    close_fee: f64,
    close_slippage: f64,
    funding_pnl: f64,
    legging_cost: f64,
    liquidation_cost: f64,
    gross_profit: f64,
    gross_loss: f64,
    btc_orders: u64,
    btc_trades: u64,
    reserve_ledger: BTreeMap<String, f64>,
    reserve_mismatch_count: u64,
    symbol_pnl: BTreeMap<String, f64>,
    group_pnl: BTreeMap<String, f64>,
    daily_equity: BTreeMap<i64, f64>,
    blocks: BTreeMap<i64, RecomputedBlock>,
    equity_points: Vec<(i64, f64)>,
}

#[derive(Default)]
struct RecomputedBlock {
    start_equity: Option<f64>,
    end_equity: Option<f64>,
    peak_equity: f64,
    max_drawdown_pct: f64,
    pnl: f64,
}

fn validate_policy(
    repo: &Path,
    artifact: &Path,
    label: &str,
    summary: &serde_json::Value,
) -> Result<serde_json::Value> {
    let manifest_path = artifact
        .join("trace-manifests/g1")
        .join(label)
        .join("trace-manifest.json");
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let mut violations = Vec::new();
    let mut raw_paths = BTreeMap::new();
    for (stream, metadata) in manifest["streams"]
        .as_object()
        .context("trace streams missing")?
    {
        let raw = repo.join(metadata["raw_path"].as_str().context("raw path missing")?);
        let expected_hash = metadata["sha256"].as_str().context("trace hash missing")?;
        if sha256_file(&raw)? != expected_hash {
            violations.push(format!("{stream}_sha256_mismatch"));
        }
        if count_rows(&raw)? != metadata["rows"].as_u64().unwrap_or(u64::MAX) {
            violations.push(format!("{stream}_row_count_mismatch"));
        }
        raw_paths.insert(stream.clone(), raw);
    }
    let mut recomputed = Recomputed {
        wallet: PRINCIPAL,
        peak_equity: PRINCIPAL,
        ..Recomputed::default()
    };
    read_event_trace(&raw_paths["event"], &mut recomputed)?;
    read_equity_trace(&raw_paths["equity"], &mut recomputed)?;
    read_trade_trace(&raw_paths["trade"], &mut recomputed)?;
    read_funding_trace(&raw_paths["funding"], &mut recomputed)?;
    read_margin_trace(&raw_paths["margin"], &mut recomputed)?;
    read_btc_counts(&raw_paths["order"], &mut recomputed, true)?;
    read_btc_counts(&raw_paths["trade"], &mut recomputed, false)?;
    recomputed
        .equity_points
        .sort_by_key(|(timestamp, _)| *timestamp);
    recomputed.peak_equity = PRINCIPAL;
    recomputed.max_drawdown_pct = 0.0;
    for (_, equity) in &recomputed.equity_points {
        recomputed.peak_equity = recomputed.peak_equity.max(*equity);
        recomputed.max_drawdown_pct = recomputed
            .max_drawdown_pct
            .max((recomputed.peak_equity - *equity) / recomputed.peak_equity.max(1e-9) * 100.0);
    }

    let start_ms = summary["start_ms"].as_i64().context("start missing")?;
    let end_ms = summary["end_ms"].as_i64().context("end missing")?;
    let final_equity = summary["final_equity"].as_f64().context("equity missing")?;
    let days = (end_ms - start_ms) as f64 / DAY_MS as f64;
    let annualized = ((recomputed.wallet / PRINCIPAL).max(1e-9).powf(365.0 / days) - 1.0) * 100.0;
    check_close(
        "wallet",
        recomputed.wallet,
        final_equity,
        1e-6,
        &mut violations,
    );
    check_close(
        "annualized",
        annualized,
        summary["annualized_return_pct"]
            .as_f64()
            .unwrap_or(f64::NAN),
        1e-7,
        &mut violations,
    );
    check_close(
        "drawdown",
        recomputed.max_drawdown_pct,
        summary["max_equity_drawdown_pct"]
            .as_f64()
            .unwrap_or(f64::NAN),
        1e-6,
        &mut violations,
    );
    if recomputed.risk_rows != ((end_ms - start_ms) / 60_000) as u64
        || recomputed.first_risk_ms != Some(start_ms)
        || recomputed.last_risk_ms != Some(end_ms - 60_000)
    {
        violations.push("one_minute_risk_range_mismatch".into());
    }
    if !recomputed.reserve_ledger.is_empty() || recomputed.reserve_mismatch_count > 0 {
        violations.push("reserve_reconciliation_failed".into());
    }
    if recomputed.btc_orders > 0 || recomputed.btc_trades > 0 {
        violations.push("btc_reference_traded".into());
    }
    let metrics = &summary["metrics"];
    check_close(
        "entry_fee",
        recomputed.entry_fee,
        metrics["costs"]["entry_fee_cost"]
            .as_f64()
            .unwrap_or(f64::NAN),
        1e-7,
        &mut violations,
    );
    check_close(
        "entry_slippage",
        recomputed.entry_slippage,
        metrics["costs"]["entry_slippage_cost"]
            .as_f64()
            .unwrap_or(f64::NAN),
        1e-7,
        &mut violations,
    );
    check_close(
        "close_fee",
        recomputed.close_fee,
        metrics["costs"]["close_fee_cost"]
            .as_f64()
            .unwrap_or(f64::NAN),
        1e-7,
        &mut violations,
    );
    check_close(
        "close_slippage",
        recomputed.close_slippage,
        metrics["costs"]["close_slippage_cost"]
            .as_f64()
            .unwrap_or(f64::NAN),
        1e-7,
        &mut violations,
    );
    check_close(
        "funding",
        recomputed.funding_pnl,
        metrics["costs"]["funding_pnl"].as_f64().unwrap_or(f64::NAN),
        1e-7,
        &mut violations,
    );
    let block_rows = recompute_blocks(&recomputed);
    let positive_blocks = block_rows
        .iter()
        .filter(|row| row["positive"] == true)
        .count() as u64;
    if positive_blocks != summary["positive_blocks"].as_u64().unwrap_or(u64::MAX) {
        violations.push("positive_block_count_mismatch".into());
    }
    let symbol_concentration = positive_concentration(&recomputed.symbol_pnl);
    let group_concentration = positive_concentration(&recomputed.group_pnl);
    check_close(
        "symbol_concentration",
        symbol_concentration,
        metrics["contribution"]["symbol_max_positive_pct"]
            .as_f64()
            .unwrap_or(f64::NAN),
        1e-6,
        &mut violations,
    );
    check_close(
        "group_concentration",
        group_concentration,
        metrics["contribution"]["group_max_positive_pct"]
            .as_f64()
            .unwrap_or(f64::NAN),
        1e-6,
        &mut violations,
    );
    Ok(serde_json::json!({
        "policy_id":summary["policy_id"],"label":label,
        "passed":violations.is_empty(),"violations":violations,
        "recomputed":{
            "wallet":recomputed.wallet,"annualized_return_pct":annualized,
            "max_equity_drawdown_pct":recomputed.max_drawdown_pct,
            "risk_path_rows":recomputed.risk_rows,
            "funding_pnl":recomputed.funding_pnl,
            "costs":{
                "entry_fee":recomputed.entry_fee,"entry_slippage":recomputed.entry_slippage,
                "close_fee":recomputed.close_fee,"close_slippage":recomputed.close_slippage,
                "legging":recomputed.legging_cost,"liquidation":recomputed.liquidation_cost
            },
            "positive_blocks":positive_blocks,"blocks":block_rows,
            "symbol_concentration_pct":symbol_concentration,
            "group_concentration_pct":group_concentration
        }
    }))
}

fn read_event_trace(path: &Path, output: &mut Recomputed) -> Result<()> {
    for row in jsonl(path)? {
        let row = row?;
        if row["event"] != "one_minute_risk_path" {
            continue;
        }
        let timestamp = row["timestamp"]
            .as_i64()
            .context("risk timestamp missing")?;
        let equity = row["equity"].as_f64().context("risk equity missing")?;
        output.risk_rows += 1;
        output.first_risk_ms.get_or_insert(timestamp);
        output.last_risk_ms = Some(timestamp);
        output.equity_points.push((timestamp, equity));
        output.daily_equity.insert(timestamp / DAY_MS, equity);
        let block = output.blocks.entry(block_start(timestamp)).or_default();
        block.start_equity.get_or_insert(equity);
        block.end_equity = Some(equity);
        block.peak_equity = block.peak_equity.max(equity);
        block.max_drawdown_pct = block
            .max_drawdown_pct
            .max((block.peak_equity - equity) / block.peak_equity.max(1e-9) * 100.0);
    }
    Ok(())
}

fn read_equity_trace(path: &Path, output: &mut Recomputed) -> Result<()> {
    for row in jsonl(path)? {
        let row = row?;
        if row["event"] == "equity" {
            if let (Some(timestamp), Some(equity)) =
                (row["timestamp"].as_i64(), row["quote"].as_f64())
            {
                output.equity_points.push((timestamp, equity));
            }
        }
    }
    Ok(())
}

fn read_trade_trace(path: &Path, output: &mut Recomputed) -> Result<()> {
    for row in jsonl(path)? {
        let row = row?;
        let metadata = &row["metadata"];
        if !metadata.is_object() {
            continue;
        }
        if let Some(fee) = metadata["entry_fee_cost"].as_f64() {
            let slippage = metadata["entry_slippage_cost"].as_f64().unwrap_or(0.0);
            output.entry_fee += fee;
            output.entry_slippage += slippage;
            output.wallet -= fee + slippage;
        }
        if let Some(net) = metadata["net_wallet_change"].as_f64() {
            output.wallet += net;
            let gross = metadata["gross_pnl"].as_f64().unwrap_or(0.0);
            output.gross_profit += gross.max(0.0);
            output.gross_loss += (-gross).max(0.0);
            output.close_fee += metadata["close_fee_cost"].as_f64().unwrap_or(0.0);
            output.close_slippage += metadata["close_slippage_cost"].as_f64().unwrap_or(0.0);
            output.liquidation_cost += metadata["liquidation_cost"].as_f64().unwrap_or(0.0);
            if let Some(symbol) = row["symbol"].as_str() {
                *output.symbol_pnl.entry(symbol.into()).or_default() += net;
            }
            if let Some(group) = metadata["owner_group"].as_str() {
                *output.group_pnl.entry(group.into()).or_default() += net;
            }
            if let Some(timestamp) = row["timestamp"].as_i64() {
                output.blocks.entry(block_start(timestamp)).or_default().pnl += net;
            }
        }
        if let Some(cost) = metadata["legging_cost"].as_f64() {
            output.legging_cost += cost;
            output.wallet -= cost;
        }
    }
    Ok(())
}

fn read_funding_trace(path: &Path, output: &mut Recomputed) -> Result<()> {
    for row in jsonl(path)? {
        let row = row?;
        if let Some(cashflow) = row["metadata"]["cashflow"].as_f64() {
            output.funding_pnl += cashflow;
            output.wallet += cashflow;
        }
    }
    Ok(())
}

fn read_margin_trace(path: &Path, output: &mut Recomputed) -> Result<()> {
    for row in jsonl(path)? {
        let row = row?;
        let metadata = &row["metadata"];
        let Some(owner) = metadata["owner"].as_str() else {
            continue;
        };
        if row["event"] == "reserve" {
            let total = metadata["components"]
                .as_object()
                .map(|components| components.values().filter_map(|value| value.as_f64()).sum())
                .unwrap_or(0.0);
            output.reserve_ledger.insert(owner.into(), total);
        } else if row["event"] == "reserve_released" {
            output.reserve_ledger.remove(owner);
        }
        let recomputed = output.reserve_ledger.values().sum::<f64>();
        let recorded = metadata["reserved_quote_after"]
            .as_f64()
            .unwrap_or(f64::NAN);
        if !close(recomputed, recorded, 1e-8) {
            output.reserve_mismatch_count += 1;
        }
    }
    Ok(())
}

fn read_btc_counts(path: &Path, output: &mut Recomputed, order_stream: bool) -> Result<()> {
    for row in jsonl(path)? {
        let row = row?;
        if row["symbol"] == "BTCUSDT" {
            if order_stream {
                output.btc_orders += 1;
            } else {
                output.btc_trades += 1;
            }
        }
    }
    Ok(())
}

fn recompute_blocks(output: &Recomputed) -> Vec<serde_json::Value> {
    output
        .blocks
        .iter()
        .map(|(start, block)| {
            let start_equity = block.start_equity.unwrap_or(PRINCIPAL);
            let end_equity = block.end_equity.unwrap_or(start_equity);
            let raw_return_pct = (end_equity / start_equity - 1.0) * 100.0;
            serde_json::json!({
                "start_ms":start,"start_equity":start_equity,"end_equity":end_equity,
                "raw_return_pct":raw_return_pct,"local_drawdown_pct":block.max_drawdown_pct,
                "positive":raw_return_pct > 0.0,"realized_pnl":block.pnl
            })
        })
        .collect()
}

fn positive_concentration(values: &BTreeMap<String, f64>) -> f64 {
    let total = values.values().filter(|value| **value > 0.0).sum::<f64>();
    if total <= 0.0 {
        0.0
    } else {
        values.values().copied().fold(0.0, f64::max) / total * 100.0
    }
}

fn block_start(timestamp: i64) -> i64 {
    let date = chrono::DateTime::from_timestamp_millis(timestamp)
        .unwrap()
        .date_naive();
    let month = chrono::Datelike::month(&date);
    let quarter_month = ((month - 1) / 3) * 3 + 1;
    chrono::NaiveDate::from_ymd_opt(chrono::Datelike::year(&date), quarter_month, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}

fn jsonl(path: &Path) -> Result<impl Iterator<Item = Result<serde_json::Value>>> {
    let reader = BufReader::new(File::open(path)?);
    Ok(reader.lines().filter_map(|line| match line {
        Ok(line) if line.trim().is_empty() => None,
        Ok(line) => Some(serde_json::from_str(&line).map_err(Into::into)),
        Err(error) => Some(Err(error.into())),
    }))
}

fn count_rows(path: &Path) -> Result<u64> {
    Ok(BufReader::new(File::open(path)?)
        .lines()
        .filter_map(|line| line.ok())
        .filter(|line| !line.trim().is_empty())
        .count() as u64)
}

fn check_close(name: &str, left: f64, right: f64, tolerance: f64, errors: &mut Vec<String>) {
    if !close(left, right, tolerance) {
        errors.push(format!("{name}_mismatch:{left}:{right}"));
    }
}

fn close(left: f64, right: f64, tolerance: f64) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= tolerance
}

fn sorted_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(path)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn walk_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![path.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}

fn write_json(path: PathBuf, value: &serde_json::Value) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
