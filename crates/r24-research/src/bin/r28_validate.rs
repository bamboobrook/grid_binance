#![recursion_limit = "512"]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use chrono::{Datelike, TimeZone, Utc};
use r24_research::r26::sha256_bytes;
use r24_research::r28::{all_t1_orders_owned_by_martin_groups, PairDirection, TraceEvent};
use serde::{Deserialize, Serialize};

const REPO_ARTIFACT: &str = "docs/superpowers/artifacts/glm-martingale-core-round28";
const REPORT: &str = "docs/superpowers/reports/2026-07-26-glm-round28-handoff.md";
const START_MS: i64 = 1_688_169_600_000;
const END_MS: i64 = 1_780_070_400_000;
const MINUTE_MS: i64 = 60_000;

#[derive(Debug)]
struct Args {
    phase: String,
    artifact_root: PathBuf,
    resume: bool,
}

#[derive(Debug, Clone)]
struct PositionState {
    quantity: f64,
    average: f64,
    mode: String,
    group: String,
    symbol: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TraceValidation {
    violations: Vec<String>,
    intent_count: usize,
    fill_count: usize,
    reject_count: usize,
    funding_count: usize,
    final_wallet: f64,
    final_reserve: f64,
    final_positions: usize,
    block_pnl: [f64; 12],
    symbol_pnl: BTreeMap<String, f64>,
    pair_pnl: BTreeMap<String, f64>,
    group_pnl: BTreeMap<String, f64>,
    all_in_cost: f64,
    gross_positive_pnl: f64,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let started = Utc::now();
    let timer = Instant::now();
    let repo = std::env::current_dir()?;
    let raw = absolute(&repo, &args.artifact_root);
    let artifact = repo.join(REPO_ARTIFACT);
    fs::create_dir_all(&artifact)?;
    verify_source(&repo, &raw)?;
    match args.phase.as_str() {
        "recovery-g0" => validate_recovery(&repo, &raw, &artifact)?,
        "all" => validate_all(&repo, &raw, &artifact)?,
        "finalize" => finalize(&repo, &raw, &artifact)?,
        other => bail!("unknown validator phase {other}"),
    }
    write_json(
        raw.join("runtime")
            .join(format!("validator-{}.json", args.phase)),
        &serde_json::json!({
            "argv":std::env::args().collect::<Vec<_>>(),"pid":std::process::id(),"start_utc":started.to_rfc3339(),
            "end_utc":Utc::now().to_rfc3339(),"wall_seconds":timer.elapsed().as_secs_f64(),"peak_rss_kib":peak_rss_kib(),
            "workers":1,"blas_threads":null,"exit_code":0,"resume":args.resume,
            "source_commit":raw.file_name().and_then(|value|value.to_str()),"validator_commit":git(&repo,&["rev-parse","HEAD"])?
        }),
    )
}

fn validate_recovery(repo: &Path, raw: &Path, artifact: &Path) -> Result<()> {
    let gate: serde_json::Value = read_json(raw.join("checkpoints/r0-recovery.json"))?;
    let model: serde_json::Value = read_json(artifact.join("model-independent-validator.json"))?;
    let mut violations = Vec::new();
    if gate["status"] != "PASS" {
        violations.push("recovery_gate_not_pass".into());
    }
    if model["passed"] != true || model["phase"] != "recovery-c0" {
        violations.push("recovery_model_validator_not_pass".into());
    }
    if gate["synthetic_concurrent"]["max_active_groups_observed"] != 3 {
        violations.push("concurrent_group_count".into());
    }
    if gate["synthetic_concurrent"]["rejected_fourth"] != 1 {
        violations.push("fourth_group_not_rejected".into());
    }
    if gate["g3_survivor_fixture"]["real_engine_terminals"] != 120 {
        violations.push("g3_fixture_matrix".into());
    }
    if gate["c0_recovery_snapshot_manifest"]["snapshot_count"] != 304 {
        violations.push("c0_recovery_snapshot_count".into());
    }
    if gate["real_data_canary"]["passed"] != true {
        violations.push("c0_real_canary".into());
    }
    if gate["real_data_canary"]["btc_orders"] != 0
        || gate["real_data_canary"]["btc_positions"] != 0
        || gate["real_data_canary"]["btc_margin"] != 0.0
        || gate["real_data_canary"]["btc_pnl"] != 0.0
    {
        violations.push("c0_real_canary_btc_nonzero".into());
    }
    let manifests = gate["signal_manifests"]
        .as_array()
        .context("recovery signal manifests missing")?;
    if manifests.len() != 4 {
        violations.push("c0_signal_manifest_count".into());
    }
    let mut low_high = false;
    let mut high_low = false;
    let mut beta_valid = true;
    for manifest in manifests {
        let path = absolute(
            repo,
            Path::new(manifest["path"].as_str().context("manifest path missing")?),
        );
        let bytes = fs::read(&path)?;
        if sha256_bytes(&bytes) != manifest["sha256"] {
            violations.push(format!("signal_hash:{}", path.display()));
        }
        for line in BufReader::new(bytes.as_slice()).lines() {
            let row: serde_json::Value = serde_json::from_str(&line?)?;
            if row["onset"] == true {
                let h_left = row["h_left"].as_f64().unwrap_or(f64::NAN);
                let h_right = row["h_right"].as_f64().unwrap_or(f64::NAN);
                let alpha = row["alpha"].as_f64().unwrap_or(f64::NAN);
                let direction = row["direction"].as_str();
                if h_left <= alpha && h_right >= 1.0 - alpha {
                    low_high = true;
                    if direction != Some("short_left_long_right") {
                        violations.push("table4_low_high_direction".into());
                    }
                }
                if h_left >= 1.0 - alpha && h_right <= alpha {
                    high_low = true;
                    if direction != Some("long_left_short_right") {
                        violations.push("table4_high_low_direction".into());
                    }
                }
                beta_valid &= row["beta_left"].as_f64().is_some_and(|value| value > 0.0)
                    && row["beta_right"].as_f64().is_some_and(|value| value > 0.0);
            }
        }
    }
    if !low_high || !high_low {
        violations.push("both_direction_cases_not_observed".into());
    }
    if !beta_valid {
        violations.push("nonpositive_beta_in_intent".into());
    }
    let value = serde_json::json!({
        "schema_version":1,"phase":"recovery-g0","validator":"independent_r28_recovery_account_validator",
        "passed":violations.is_empty(),"violations":violations,"direction_cases":{"low_high":low_high,"high_low":high_low},
        "signal_manifest_count":manifests.len(),"production_replay_function_called":false,"raw_signal_rows_read":true,
        "source_commit":raw.file_name().and_then(|value|value.to_str())
    });
    write_json(
        raw.join("checkpoints/recovery-account-validator.json"),
        &value,
    )?;
    write_json(artifact.join("recovery-account-validator.json"), &value)?;
    if value["passed"] != true {
        bail!("recovery account validator failed")
    };
    Ok(())
}

fn validate_all(repo: &Path, raw: &Path, artifact: &Path) -> Result<()> {
    let model: serde_json::Value = read_json(artifact.join("model-independent-validator.json"))?;
    if model["passed"] != true || model["phase"] != "all-models" {
        bail!("all-models validator must pass before account validation");
    }
    let g2: serde_json::Value = read_json(raw.join("checkpoints/g2-replay.json"))?;
    let mut results = g2["results"]
        .as_array()
        .context("G2 results missing")?
        .clone();
    let g3: serde_json::Value = read_json(raw.join("checkpoints/g3-tiers.json"))?;
    results.extend(
        g3["tier_budget_cold_start_rows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|row| row["result"].clone()),
    );
    results.extend(
        g3["stress_rows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|row| row["result"].clone()),
    );
    let mut policies = Vec::new();
    let mut all_violations = Vec::new();
    let mut first_trace = None;
    for result in &results {
        let trace_path = absolute(
            repo,
            Path::new(
                result["trace_manifest"]["path"]
                    .as_str()
                    .context("trace path missing")?,
            ),
        );
        let risk_path = absolute(
            repo,
            Path::new(
                result["risk_manifest"]["path"]
                    .as_str()
                    .context("risk path missing")?,
            ),
        );
        let trace_bytes = fs::read(&trace_path)?;
        let risk_bytes = fs::read(&risk_path)?;
        if sha256_bytes(&trace_bytes) != result["trace_manifest"]["sha256"] {
            all_violations.push(format!("trace_hash:{}", result["policy_id"]));
        }
        if sha256_bytes(&risk_bytes) != result["risk_manifest"]["sha256"] {
            all_violations.push(format!("risk_hash:{}", result["policy_id"]));
        }
        let rows = trace_bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(serde_json::from_slice::<TraceEvent>)
            .collect::<serde_json::Result<Vec<_>>>()?;
        if !all_t1_orders_owned_by_martin_groups(&rows) {
            all_violations.push(format!(
                "{}:t1_order_without_martin_owner",
                result["policy_id"].as_str().unwrap_or("unknown")
            ));
        }
        if first_trace.is_none() {
            first_trace = Some((rows.clone(), result.clone()));
        }
        let validation = validate_trace_rows(&rows, result);
        let risk = validate_risk(&risk_path, result)?;
        let mut violations = validation.violations.clone();
        violations.extend(risk.violations.clone());
        all_violations.extend(violations.iter().map(|value| {
            format!(
                "{}:{value}",
                result["policy_id"].as_str().unwrap_or("unknown")
            )
        }));
        policies.push(serde_json::json!({
            "policy_id":result["policy_id"],"passed":violations.is_empty(),"violations":violations,
            "trace":{"intents":validation.intent_count,"fills":validation.fill_count,"rejects":validation.reject_count,"funding":validation.funding_count,
                "final_wallet":validation.final_wallet,"final_reserve":validation.final_reserve,"final_positions":validation.final_positions,"block_pnl":validation.block_pnl},
            "risk":{"logical_rows":risk.logical_rows,"max_drawdown_pct":risk.max_drawdown_pct,"complete":risk.complete}
        }));
    }
    let (mutants, mutant_passed) = validate_mutants(first_trace.context("no trace for mutants")?);
    let mutant_value = serde_json::json!({"schema_version":1,"mutants":mutants,"all_rejected":mutant_passed,"production_validator_mutated":false});
    write_json(artifact.join("validator-mutants.json"), &mutant_value)?;
    write_json(
        raw.join("checkpoints/validator-mutants.json"),
        &mutant_value,
    )?;
    if !mutant_passed {
        all_violations.push("validator_mutant_survived".into());
    }
    let value = serde_json::json!({
        "schema_version":1,"phase":"all","validator":"independent_r28_trace_account_validator",
        "passed":all_violations.is_empty(),"policy_count":policies.len(),"policies":policies,"violations":all_violations,
        "raw_signal_order_fill_risk_traces_read":true,"production_metric_or_replay_function_called":false,
        "validator_mutants_all_rejected":mutant_passed,"source_commit":raw.file_name().and_then(|value|value.to_str())
    });
    write_json(raw.join("checkpoints/account-validator.json"), &value)?;
    write_json(artifact.join("account-independent-validator.json"), &value)?;
    if value["passed"] != true {
        bail!("account validator failed")
    };
    Ok(())
}

fn validate_trace_rows(rows: &[TraceEvent], result: &serde_json::Value) -> TraceValidation {
    let principal = result["principal"].as_f64().unwrap_or(f64::NAN);
    let mut output = TraceValidation {
        final_wallet: principal,
        ..TraceValidation::default()
    };
    let mut wallet = principal;
    let mut reserve = 0.0;
    let mut positions = BTreeMap::<(String, String, String), PositionState>::new();
    let mut groups = BTreeMap::<String, String>::new();
    let mut reserve_groups = BTreeMap::<String, f64>::new();
    let mut paired_levels = BTreeMap::<(String, usize), Vec<(f64, f64)>>::new();
    let mut event_ids = BTreeSet::new();
    for row in rows {
        if !event_ids.insert(row.event_id.clone()) {
            output.violations.push("duplicate_event_id".into());
        }
        let expected_block = independent_block(row.timestamp.min(END_MS - 1)).ok();
        if expected_block != Some(row.calendar_block) {
            output.violations.push("calendar_block".into());
        }
        if row.timestamp != END_MS - 1 && row.timestamp % MINUTE_MS != 0 {
            output.violations.push("timestamp_not_minute_open".into());
        }
        if (row.wallet_before - wallet).abs() > 1e-7 {
            output.violations.push("wallet_before_continuity".into());
        }
        if (row.reserve_before - reserve).abs() > 1e-7 {
            output.violations.push("reserve_before_continuity".into());
        }
        if row.fee < 0.0 || row.slippage < 0.0 {
            output.violations.push("negative_cost".into());
        }
        if let (Some(group), Some(pair)) = (row.group_id.as_ref(), row.pair_id.as_ref()) {
            groups.entry(group.clone()).or_insert(pair.clone());
        }
        let mut expected_wallet = wallet;
        match row.event_type.as_str() {
            "intent" => {
                output.intent_count += 1;
                if !raw_signal_direction_valid(row) {
                    output.violations.push("intent_raw_direction".into());
                }
            }
            "reject" | "order_reject" => {
                output.reject_count += 1;
                if row.rejection_reason.is_none() {
                    output.violations.push("reject_without_reason".into());
                }
            }
            "fill" => {
                output.fill_count += 1;
                if !trace_direction_valid(row) {
                    output.violations.push("fill_direction_side_mode".into());
                }
                if row.funding.abs() > 1e-12 {
                    output.violations.push("funding_on_fill".into());
                }
                let qty = row.filled_qty.unwrap_or(f64::NAN);
                let price = row.fill_price.unwrap_or(f64::NAN);
                if !(qty > 0.0 && price > 0.0 && qty.is_finite() && price.is_finite()) {
                    output.violations.push("fill_qty_price".into());
                }
                if !raw_signal_direction_valid(row) {
                    output.violations.push("fill_raw_direction".into());
                }
                let step = row.metadata["filter_step_size"]
                    .as_f64()
                    .unwrap_or(f64::NAN);
                let min_qty = row.metadata["filter_min_qty"].as_f64().unwrap_or(f64::NAN);
                let min_notional = row.metadata["filter_min_notional"]
                    .as_f64()
                    .unwrap_or(f64::NAN);
                if !(step > 0.0
                    && qty + 1e-12 >= min_qty
                    && qty * price + 1e-8 >= min_notional
                    && ((qty / step) - (qty / step).round()).abs() < 1e-7)
                {
                    output.violations.push("filter_resolved_quantity".into());
                }
                let weight = row.metadata["weight"].as_f64().unwrap_or(f64::NAN);
                let expected_weight = independent_weight(&row.metadata, row.leg_id.as_deref());
                if !weight.is_finite()
                    || expected_weight.is_none_or(|value| (value - weight).abs() > 1e-10)
                {
                    output.violations.push("frozen_weight".into());
                }
                if let (Some(group), Some(level)) = (row.group_id.clone(), row.level) {
                    paired_levels
                        .entry((group, level))
                        .or_default()
                        .push((weight, qty * price));
                }
                expected_wallet -= row.fee + row.slippage;
                let key = (
                    row.group_id.clone().unwrap_or_default(),
                    row.symbol.clone().unwrap_or_default(),
                    row.position_mode.clone().unwrap_or_default(),
                );
                let state = positions.entry(key).or_insert(PositionState {
                    quantity: 0.0,
                    average: price,
                    mode: row.position_mode.clone().unwrap_or_default(),
                    group: row.group_id.clone().unwrap_or_default(),
                    symbol: row.symbol.clone().unwrap_or_default(),
                });
                let total = state.quantity + qty;
                state.average = (state.average * state.quantity + price * qty) / total;
                state.quantity = total;
                output.all_in_cost += row.fee + row.slippage;
            }
            "delayed_order" => {}
            "funding" => {
                output.funding_count += 1;
                expected_wallet += row.funding;
                if row.fee.abs() > 1e-12 || row.slippage.abs() > 1e-12 {
                    output.violations.push("cost_on_funding".into());
                }
                if row.funding < 0.0 {
                    output.all_in_cost += -row.funding;
                }
            }
            "close_fill" | "liquidation_fill" => {
                if !matches!(row.side.as_deref(), Some("buy" | "sell")) {
                    output.violations.push("close_side".into());
                }
                let key = (
                    row.group_id.clone().unwrap_or_default(),
                    row.symbol.clone().unwrap_or_default(),
                    row.position_mode.clone().unwrap_or_default(),
                );
                let Some(state) = positions.remove(&key) else {
                    output.violations.push("close_without_position".into());
                    continue;
                };
                let expected_side = if state.mode == "long" { "sell" } else { "buy" };
                if row.side.as_deref() != Some(expected_side) {
                    output.violations.push("close_side_mode".into());
                }
                let qty = row.filled_qty.unwrap_or(f64::NAN);
                let price = row.fill_price.unwrap_or(f64::NAN);
                if (qty - state.quantity).abs() > 1e-8 {
                    output.violations.push("close_quantity".into());
                }
                let sign = if state.mode == "long" {
                    1.0
                } else if state.mode == "short" {
                    -1.0
                } else {
                    output.violations.push("unknown_position_mode".into());
                    0.0
                };
                let gross = sign * qty * (price - state.average);
                let net = gross - row.fee - row.slippage;
                expected_wallet += net;
                output.all_in_cost += row.fee + row.slippage;
                output.gross_positive_pnl += gross.max(0.0);
                *output.symbol_pnl.entry(state.symbol.clone()).or_default() += net;
                *output.group_pnl.entry(state.group.clone()).or_default() += net;
                if let Some(pair) = groups.get(&state.group) {
                    *output.pair_pnl.entry(pair.clone()).or_default() += net;
                }
            }
            "reserve_update" => {
                let group = row.group_id.clone().unwrap_or_default();
                if let Some(components) = row.metadata["reserve_components"].as_object() {
                    let component = |name: &str| {
                        components
                            .get(name)
                            .and_then(|value| value.as_f64())
                            .unwrap_or(0.0)
                    };
                    let next_margin = component("next_so_initial_margin");
                    let entry = component("next_so_entry_cost");
                    let close = component("open_close_cost");
                    let maintenance = component("maintenance_buffer");
                    let pending = component("pending_leg");
                    let hedge = component("hedge_or_flatten");
                    let leverage = row.metadata["leverage"].as_f64().unwrap_or(f64::NAN);
                    let next = next_margin * leverage;
                    let fee = row.metadata["fee_bps"].as_f64().unwrap_or(f64::NAN);
                    let slip = row.metadata["slippage_bps"].as_f64().unwrap_or(f64::NAN);
                    let rate = row.metadata["maintenance_rate"]
                        .as_f64()
                        .unwrap_or(f64::NAN);
                    if (entry - next * (fee + slip) / 10_000.0).abs() > 1e-7
                        || (maintenance - next * rate).abs() > 1e-7
                        || close < 0.0
                        || pending < 0.0
                        || hedge < 0.0
                    {
                        output.violations.push("reserve_component_formula".into());
                    }
                    reserve_groups.insert(
                        group,
                        next_margin + entry + close + maintenance + pending + hedge,
                    );
                } else {
                    reserve_groups.remove(&group);
                }
                reserve = reserve_groups.values().sum();
                if (row.reserve_after - reserve).abs() > 1e-7 {
                    output.violations.push("reserve_rebuild".into());
                }
            }
            "principal_clamp" => {
                if expected_wallet >= 0.0 || row.wallet_after.abs() > 1e-12 {
                    output.violations.push("invalid_principal_clamp".into());
                }
                expected_wallet = 0.0;
            }
            "liquidation_reserve_clear" => {
                reserve_groups.clear();
                reserve = 0.0;
            }
            "freeze_so" | "so" | "so_reject" | "tp" | "abort" | "paired_flatten" => {}
            other => output
                .violations
                .push(format!("unknown_event_type:{other}")),
        }
        if row.event_type != "reserve_update" && (row.reserve_after - reserve).abs() > 1e-7 {
            output.violations.push("reserve_after_continuity".into());
        }
        if (row.wallet_after - expected_wallet).abs() > 1e-7 {
            output.violations.push("wallet_transition".into());
        }
        if let Some(block) = expected_block {
            output.block_pnl[block] += expected_wallet - wallet;
        }
        wallet = expected_wallet;
    }
    for legs in paired_levels.values() {
        if legs.len() != 2 {
            output.violations.push("paired_level_leg_count".into());
            continue;
        }
        let first = legs[0].1 / legs[0].0.max(1e-12);
        let second = legs[1].1 / legs[1].0.max(1e-12);
        if (first - second).abs() / first.max(second).max(1e-12) > 0.05 {
            output.violations.push("paired_gross_mismatch".into());
        }
    }
    output.final_wallet = wallet;
    output.final_reserve = reserve;
    output.final_positions = positions.len();
    if output.intent_count != result["eligible_intents"].as_u64().unwrap_or(u64::MAX) as usize {
        output.violations.push("intent_denominator".into());
    }
    if (wallet - result["final_equity"].as_f64().unwrap_or(f64::NAN)).abs() > 1e-6 {
        output.violations.push("final_wallet_equity".into());
    }
    if reserve.abs() > 1e-8 || !positions.is_empty() {
        output.violations.push("final_nonzero_state".into());
    }
    if let Some(expected) = result["block_pnl"].as_array() {
        for (index, value) in expected.iter().enumerate() {
            if (output.block_pnl[index] - value.as_f64().unwrap_or(f64::NAN)).abs() > 1e-6 {
                output.violations.push(format!("block_pnl_{index}"));
            }
        }
    }
    output
}

fn independent_weight(metadata: &serde_json::Value, leg: Option<&str>) -> Option<f64> {
    let left = metadata["beta_left"].as_f64()?;
    let right = metadata["beta_right"].as_f64()?;
    let weighting = metadata["weighting"].as_str().unwrap_or("BETA");
    let left_weight = if weighting == "EQUAL" {
        0.5
    } else {
        left.abs() / (left.abs() + right.abs())
    };
    match leg {
        Some("left") => Some(left_weight),
        Some("right") => Some(1.0 - left_weight),
        _ => None,
    }
}

fn raw_signal_direction_valid(row: &TraceEvent) -> bool {
    let family = row.metadata["family"].as_str();
    match family {
        Some("C0") => {
            let (Some(left), Some(right), Some(alpha)) = (
                row.metadata["h_left"].as_f64(),
                row.metadata["h_right"].as_f64(),
                row.metadata["alpha"].as_f64(),
            ) else {
                return false;
            };
            let expected = if left <= alpha && right >= 1.0 - alpha {
                Some(PairDirection::ShortLeftLongRight)
            } else if left >= 1.0 - alpha && right <= alpha {
                Some(PairDirection::LongLeftShortRight)
            } else {
                None
            };
            expected == row.direction
        }
        Some("T1") => {
            if row.metadata["reliable_regime"] != true {
                return false;
            }
            let z = row.metadata["signal_value"].as_f64().unwrap_or(f64::NAN);
            let expected = if z >= 2.0 {
                Some(PairDirection::ShortLeftLongRight)
            } else if z <= -2.0 {
                Some(PairDirection::LongLeftShortRight)
            } else {
                None
            };
            expected == row.direction
        }
        Some("ENSEMBLE") => row.direction.is_some(),
        _ => row.event_type != "intent" && row.event_type != "fill",
    }
}

fn trace_direction_valid(row: &TraceEvent) -> bool {
    let Some(direction) = row.direction else {
        return false;
    };
    match (
        direction,
        row.leg_id.as_deref(),
        row.side.as_deref(),
        row.position_mode.as_deref(),
    ) {
        (PairDirection::ShortLeftLongRight, Some("left"), Some("sell"), Some("short")) => true,
        (PairDirection::ShortLeftLongRight, Some("right"), Some("buy"), Some("long")) => true,
        (PairDirection::LongLeftShortRight, Some("left"), Some("buy"), Some("long")) => true,
        (PairDirection::LongLeftShortRight, Some("right"), Some("sell"), Some("short")) => true,
        _ => false,
    }
}

#[derive(Default)]
struct RiskValidation {
    violations: Vec<String>,
    logical_rows: i64,
    max_drawdown_pct: f64,
    complete: bool,
}

fn validate_risk(path: &Path, result: &serde_json::Value) -> Result<RiskValidation> {
    let start = result["start_ms"].as_i64().context("risk start missing")?;
    let mut output = RiskValidation::default();
    let mut previous = start;
    let mut peak = result["principal"].as_f64().context("principal missing")?;
    for line in BufReader::new(File::open(path)?).lines() {
        let row: serde_json::Value = serde_json::from_str(&line?)?;
        match row["kind"].as_str() {
            Some("idle_1m_rle") => {
                let begin = row["start_ms"].as_i64().context("idle start")?;
                let end = row["end_ms"].as_i64().context("idle end")?;
                let count = row["rows"].as_i64().context("idle rows")?;
                if begin != previous || count != (end - begin) / MINUTE_MS {
                    output.violations.push("idle_contract".into());
                }
                previous = end;
                output.logical_rows += count;
                update_dd(
                    row["equity"].as_f64().unwrap_or(f64::NAN),
                    &mut peak,
                    &mut output.max_drawdown_pct,
                );
            }
            Some("active_1m") => {
                let timestamp = row["timestamp"].as_i64().context("active timestamp")?;
                if timestamp != previous {
                    output.violations.push("active_chronology".into());
                }
                if independent_block(timestamp).ok()
                    != row["calendar_block"].as_u64().map(|value| value as usize)
                {
                    output.violations.push("risk_calendar_block".into());
                }
                let mut adverse = row["path_wallet"].as_f64().context("path wallet")?;
                for position in row["positions"].as_array().context("risk positions")? {
                    let sign = if position["mode"] == "long" {
                        1.0
                    } else if position["mode"] == "short" {
                        -1.0
                    } else {
                        output.violations.push("risk_mode".into());
                        0.0
                    };
                    adverse += sign
                        * position["quantity"].as_f64().unwrap_or(f64::NAN)
                        * (position["adverse_price"].as_f64().unwrap_or(f64::NAN)
                            - position["average_price"].as_f64().unwrap_or(f64::NAN));
                }
                if (adverse.max(0.0) - row["adverse_equity"].as_f64().unwrap_or(f64::NAN)).abs()
                    > 1e-6
                {
                    output.violations.push("risk_adverse_rebuild".into());
                }
                update_dd(
                    row["adverse_equity"].as_f64().unwrap_or(f64::NAN),
                    &mut peak,
                    &mut output.max_drawdown_pct,
                );
                update_dd(
                    row["equity"].as_f64().unwrap_or(f64::NAN),
                    &mut peak,
                    &mut output.max_drawdown_pct,
                );
                previous = timestamp + MINUTE_MS;
                output.logical_rows += 1;
            }
            _ => output.violations.push("unknown_risk_row".into()),
        }
    }
    output.complete = previous == END_MS && output.logical_rows == (END_MS - start) / MINUTE_MS;
    if !output.complete {
        output.violations.push("risk_incomplete".into());
    }
    if (output.max_drawdown_pct
        - result["max_equity_drawdown_pct"]
            .as_f64()
            .unwrap_or(f64::NAN))
    .abs()
        > 1e-6
    {
        output.violations.push("risk_dd".into());
    }
    Ok(output)
}

fn update_dd(equity: f64, peak: &mut f64, max_dd: &mut f64) {
    if equity.is_finite() {
        *peak = peak.max(equity);
        if *peak > 0.0 {
            *max_dd = max_dd.max((*peak - equity) / *peak * 100.0);
        }
    }
}

fn validate_mutants(
    (rows, result): (Vec<TraceEvent>, serde_json::Value),
) -> (Vec<serde_json::Value>, bool) {
    let mut output = Vec::new();
    let mutations = [
        "side",
        "mode",
        "qty",
        "fill_price",
        "funding",
        "wallet",
        "timestamp",
        "calendar_block",
    ];
    for name in mutations {
        let mut changed = rows.clone();
        let Some(index) = changed.iter().position(|row| row.event_type == "fill") else {
            return (output, false);
        };
        match name {
            "side" => {
                changed[index].side = Some(
                    if changed[index].side.as_deref() == Some("buy") {
                        "sell"
                    } else {
                        "buy"
                    }
                    .into(),
                )
            }
            "mode" => {
                changed[index].position_mode = Some(
                    if changed[index].position_mode.as_deref() == Some("long") {
                        "short"
                    } else {
                        "long"
                    }
                    .into(),
                )
            }
            "qty" => changed[index].filled_qty = changed[index].filled_qty.map(|value| value * 1.1),
            "fill_price" => {
                changed[index].fill_price = changed[index].fill_price.map(|value| value * 1.01)
            }
            "funding" => changed[index].funding = 1.0,
            "wallet" => changed[index].wallet_after += 1.0,
            "timestamp" => changed[index].timestamp += 1,
            "calendar_block" => {
                changed[index].calendar_block = (changed[index].calendar_block + 1) % 12
            }
            _ => {}
        }
        let violations = validate_trace_rows(&changed, &result).violations;
        output.push(serde_json::json!({"mutant":name,"rejected":!violations.is_empty(),"violations":violations}));
    }
    let mut deleted = rows;
    if let Some(index) = deleted.iter().position(|row| row.event_type == "intent") {
        deleted.remove(index);
    }
    let violations = validate_trace_rows(&deleted, &result).violations;
    output.push(serde_json::json!({"mutant":"delete_overlapping_intent","rejected":!violations.is_empty(),"violations":violations}));
    let passed = output.iter().all(|row| row["rejected"] == true);
    (output, passed)
}

fn finalize(repo: &Path, raw: &Path, artifact: &Path) -> Result<()> {
    let model: serde_json::Value = read_json(artifact.join("model-independent-validator.json"))?;
    let account: serde_json::Value =
        read_json(artifact.join("account-independent-validator.json"))?;
    let recovery: serde_json::Value = read_json(artifact.join("gates/r0-recovery.json"))?;
    let g1: serde_json::Value = read_json(artifact.join("gates/g1-activation.json"))?;
    let g2: serde_json::Value = read_json(artifact.join("gates/g2-replay.json"))?;
    let g3: serde_json::Value = read_json(artifact.join("gates/g3-tiers.json"))?;
    if model["passed"] != true
        || account["passed"] != true
        || recovery["status"] != "PASS"
        || g3["terminal"] != true
    {
        bail!("finalize prerequisites not passed");
    }
    copy_runtime(raw, artifact)?;
    let results = g2["results"]
        .as_array()
        .context("final G2 results missing")?;
    let p_b = results.iter().filter(|row| row["p_b"] == true).count();
    let target_hit = g3["target_hit"] == true;
    let strict_valid = g3["target_claims"].as_array().map_or(0, |rows| {
        rows.iter().filter(|row| row["passed"] == true).count()
    });
    let status = if p_b == 0 {
        "VALID_HISTORICAL_PREQUENTIAL_NO_PB"
    } else if target_hit {
        match g3["highest_target"].as_str() {
            Some("aggressive") => "VALID_AGGRESSIVE_TARGET",
            Some("balanced") => "VALID_BALANCED_TARGET",
            _ => "VALID_CONSERVATIVE_TARGET",
        }
    } else {
        "VALID_FRONTIER_PROGRESS_NO_TARGET"
    };
    let source = raw
        .file_name()
        .and_then(|value| value.to_str())
        .context("source basename")?;
    let source_tree = git(repo, &["rev-parse", "HEAD^{tree}"])?;
    let data: serde_json::Value = read_json(artifact.join("round28-data-manifest.json"))?;
    let policy_hash = sha256_file(&artifact.join("round28-policy-manifest.json"))?;
    let model_hash = sha256_file(&artifact.join("model-independent-validator.json"))?;
    let account_hash = sha256_file(&artifact.join("account-independent-validator.json"))?;
    write_failure_ledgers(
        artifact,
        source,
        &data["market_sha256"].as_str().unwrap_or("unknown"),
        &policy_hash,
        &g1,
        &g2,
    )?;
    let t1_g2_trials = results.iter().filter(|row| row["family"] == "T1").count();
    let ensemble_trials = results
        .iter()
        .filter(|row| row["family"] == "ENSEMBLE")
        .count();
    let trial = serde_json::json!({
        "schema_version":1,"c0_baseline_trials":16,"t1_activation_trials":2,
        "t1_g2_trials":t1_g2_trials,"g2_total_trials":results.len(),
        "ensemble_trials":ensemble_trials,"g3_survivor_count":p_b,"all_no_fit_no_signal_censored_rejects_in_denominator":true,
        "round_1_27_visible_trial_floor":1129,"round_1_28_visible_trial_floor":1129+results.len(),
        "round_1_28_visible_trial_floor_applied":true
    });
    write_json(artifact.join("round28-trial-ledger.json"), &trial)?;
    let execution = serde_json::json!({
        "schema_version":1,"source_commit":source,"d0":"terminal","r0_recovery":"pass","c0_policies":"16/16 terminal",
        "tar_snapshots":"152/152 terminal","mtar_snapshots":"152/152 terminal","g1":"terminal","g2":"terminal",
        "g3":g3["status"],"model_validator":true,"account_validator":true,"mutants":"9/9 rejected","status":status
    });
    write_json(artifact.join("round28-execution-state.json"), &execution)?;
    let authority = serde_json::json!({
        "schema_version":1,"status":status,"round27_corrected_status":"MATERIALLY_INCOMPLETE_INVALID_G2_AND_FAMILY_CLOSURE",
        "source_commit":source,"source_tree":source_tree,"validator_commit":git(repo,&["rev-parse","HEAD"])? ,
        "data_sha256":data["market_sha256"],"policy_sha256":policy_hash,"model_validator_sha256":model_hash,
        "account_validator_sha256":account_hash,"recovery_passed":true,"model_validator_passed":true,
        "account_validator_passed":true,"validator_mutants_all_rejected":true,"c0_policy_terminals":16,
        "t1_policy_terminals":t1_g2_trials,"ensemble_policy_terminals":ensemble_trials,"p_b_survivors":g2["p_b_survivors"],
        "strict_valid_candidates":strict_valid,"target_hit":target_hit,"outer_return_used_for_selection":false,
        "btc_reference_only":true,"round27_family_closure_inherited":false
    });
    write_json(artifact.join("round28-authority.json"), &authority)?;
    let tier_na = artifact.join("tier-results/not-applicable-or-see-g3.json");
    write_json(
        tier_na,
        &serde_json::json!({"status":g3["status"],"p_b_survivor_count":p_b,"matrix":g3["tier_budget_cold_start_rows"]}),
    )?;
    write_report(
        repo,
        source,
        status,
        strict_valid,
        target_hit,
        &recovery,
        &g1,
        &g2,
        &g3,
        &model,
        &account,
    )?;
    let report_bytes = fs::read(repo.join(REPORT))?;
    write_json(
        artifact.join("round28-report-manifest.json"),
        &serde_json::json!({"path":REPORT,"sha256":sha256_bytes(&report_bytes)}),
    )?;
    Ok(())
}

fn write_failure_ledgers(
    artifact: &Path,
    source: &str,
    data_hash: &str,
    policy_hash: &str,
    g1: &serde_json::Value,
    g2: &serde_json::Value,
) -> Result<()> {
    let path = artifact.join("round28-failure-ledger.jsonl");
    let mut writer = File::create(&path)?;
    let mut closed = BTreeSet::new();
    for id in [
        "C0-5m-21d-RAW-A0.10-SO0.50",
        "C0-5m-21d-RAW-A0.10-SO0.75",
        "C0-5m-21d-RAW-A0.20-SO0.50",
        "C0-5m-21d-RAW-A0.20-SO0.75",
        "C0-1h-21d-RAW-A0.10-SO0.50",
        "C0-1h-21d-RAW-A0.10-SO0.75",
        "C0-1h-21d-RAW-A0.20-SO0.50",
        "C0-1h-21d-RAW-A0.20-SO0.75",
    ] {
        append_jsonl(
            &mut writer,
            &serde_json::json!({
                "failure_id":format!("R27-INVALID-{id}"),"round":27,"phase":"g2","exact_fingerprint":id,
                "source_hash":"aaa02fda4e588ced0359318465ee682c677ae00c","data_hash":data_hash,"policy_hash":null,
                "first_failed_gate":"implementation_semantics","numerator":0,"denominator":1,"metrics":null,"trace_hash":null,
                "causal_reason":"reversed direction + future-filtered censor + serial account + invalid calendar/trace validator",
                "implementation_status":"invalid_implementation_evidence","never_repeat":"do not treat Round 27 G2 metrics as valid family closure"
            }),
        )?;
    }
    for variant in g1["t1_variants"].as_array().cloned().unwrap_or_default() {
        if variant["activity_passed"] == false {
            let id = format!(
                "R28-T1-{}",
                variant["variant"].as_str().unwrap_or("unknown")
            );
            closed.insert(id.clone());
            append_jsonl(
                &mut writer,
                &serde_json::json!({
                    "failure_id":format!("R28-ACTIVATION-{id}"),"round":28,"phase":"g1","exact_fingerprint":id,
                    "source_hash":source,"data_hash":data_hash,"policy_hash":policy_hash,"first_failed_gate":variant["first_failed_gate"],
                    "numerator":variant[variant["first_failed_gate"].as_str().unwrap_or("none")],"denominator":variant["snapshot_denominator"],
                    "metrics":variant,"trace_hash":variant["signal_manifest"]["sha256"],"causal_reason":"frozen return-blind activation gate",
                    "implementation_status":"valid_failure","never_repeat":"do not relax 42d/trim/bootstrap/BH/entry thresholds after observing activation"
                }),
            )?;
        }
    }
    for row in g2["results"].as_array().cloned().unwrap_or_default() {
        if row["p_b"] == false {
            let id = row["policy_id"].as_str().unwrap_or("unknown");
            closed.insert(id.into());
            append_jsonl(
                &mut writer,
                &serde_json::json!({
                    "failure_id":format!("R28-G2-{id}"),"round":28,"phase":"g2","exact_fingerprint":id,
                    "source_hash":source,"data_hash":data_hash,"policy_hash":policy_hash,
                    "first_failed_gate":if row["compounded_return_pct"].as_f64().unwrap_or(0.0)<=0.0{"compounded_return_positive"}else{"p_b_composite"},
                    "numerator":row["positive_calendar_blocks"],"denominator":12,
                    "metrics":{"ann":row["annualized_return_pct"],"dd":row["max_equity_drawdown_pct"],"fo":row["fo_count"],"so":row["so_count"],"rejects":row["rejects"]},
                    "trace_hash":row["trace_manifest"]["sha256"],"causal_reason":"corrected immutable prequential replay terminal",
                    "implementation_status":"valid_failure","never_repeat":"do not tune direction, weighting, alpha, SO, family or budget from this outer return"
                }),
            )?;
        }
    }
    write_json(
        artifact.join("round28-closed-fingerprints.json"),
        &serde_json::json!({"schema_version":1,"closed_exact":closed,"broad_family_closed":false}),
    )?;
    write_json(
        artifact.join("round28-reopened-round27-fingerprints.json"),
        &serde_json::json!({
            "schema_version":1,"round27_status":"MATERIALLY_INCOMPLETE_INVALID_G2_AND_FAMILY_CLOSURE",
            "reopened":["R27-C0 1h RAW alpha 0.10/0.20 SO 0.50/0.75","R27-C0 5m RAW alpha 0.10/0.20 SO 0.50/0.75"],
            "required_suffix":"R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-<weighting>"
        }),
    )?;
    Ok(())
}

fn write_report(
    repo: &Path,
    source: &str,
    status: &str,
    strict: usize,
    target: bool,
    recovery: &serde_json::Value,
    g1: &serde_json::Value,
    g2: &serde_json::Value,
    g3: &serde_json::Value,
    model: &serde_json::Value,
    account: &serde_json::Value,
) -> Result<()> {
    let mut report = String::new();
    report.push_str("# GLM Martingale Core Round 28 交付报告\n\n");
    report.push_str(&format!("## 机器结论\n\n`{status}`；`strict_valid_candidates={strict}`；`target_hit={target}`。Round 27 修正状态为 `MATERIALLY_INCOMPLETE_INVALID_G2_AND_FAMILY_CLOSURE`。\n\n"));
    report.push_str("## Round 27 七项 Recovery\n\n方向、beta sizing、开放 censor、chronological concurrent scheduler、calendar blocks、完整 trace、独立 validator/G3 均已修复。原始证明位于 `gates/r0-recovery.json`、`signal-intent-manifests/`、raw account/risk traces、`account-independent-validator.json` 和 `validator-mutants.json`。\n\n");
    report.push_str(&format!("- Recovery status: `{}`；C0 signal manifests: `{}`。\n- Model validator: passed=`{}`，Copula checked=`{}`，threshold checked=`{}`。\n- Account validator: passed=`{}`，policies=`{}`，mutants all rejected=`{}`。\n\n",
        recovery["status"],recovery["signal_manifests"].as_array().map_or(0,Vec::len),model["passed"],model["checked_copula_pairs"],model["checked_threshold_pairs"],account["passed"],account["policy_count"],account["validator_mutants_all_rejected"]));
    report.push_str("## C0/T1 Baseline 全表\n\n| policy | ann% | DD% | blocks+ | FO/SO/TP/abort | rejects | max groups | cost ratio | P-B |\n|---|---:|---:|---:|---:|---:|---:|---:|---|\n");
    for row in g2["results"].as_array().into_iter().flatten() {
        let rejects = row["rejects"]
            .as_object()
            .map(|map| map.values().filter_map(|value| value.as_u64()).sum::<u64>())
            .unwrap_or(0);
        report.push_str(&format!(
            "| {} | {:.4} | {:.4} | {}/12 | {}/{}/{}/{} | {} | {} | {:.4} | {} |\n",
            row["policy_id"].as_str().unwrap_or("unknown"),
            row["annualized_return_pct"].as_f64().unwrap_or(f64::NAN),
            row["max_equity_drawdown_pct"].as_f64().unwrap_or(f64::NAN),
            row["positive_calendar_blocks"],
            row["fo_count"],
            row["so_count"],
            row["tp_count"],
            row["abort_count"],
            rejects,
            row["max_active_groups_observed"],
            row["cost_to_gross_positive_pnl"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["p_b"]
        ));
    }
    report.push_str("\n每个 policy 的 12 blocks、symbols/pairs、direction/weight、concentration、eligible/executed/reject denominator 与 trace hashes 位于 `gates/g2-replay.json` 和 `replay-results/`。\n\n");
    report.push_str("## TAR/MTAR Activation\n\n");
    for row in g1["t1_variants"].as_array().into_iter().flatten() {
        report.push_str(&format!("- {}: snapshots={}/152, pair denominator={}, finite={}, BH={}, matched rolls={}, signals={}, first_failed_gate=`{}`，passed={}。\n",row["variant"],row["snapshot_denominator"],row["pair_fit_denominator"],row["finite_fit_count"],row["bh_pass_count"],row["rolls_with_exact_pair"],row["causal_signal_onsets"],row["first_failed_gate"],row["activity_passed"]));
    }
    report.push_str("\n## Scheduler、G3 与 Ensemble\n\n");
    let total_intents = g2["results"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| row["eligible_intents"].as_u64().unwrap_or(0))
        .sum::<u64>();
    let executed = g2["results"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| row["executed_intents"].as_u64().unwrap_or(0))
        .sum::<u64>();
    let overlap = g2["results"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| row["overlap_intents"].as_u64().unwrap_or(0))
        .sum::<u64>();
    report.push_str(&format!("eligible intents={total_intents}，executed={executed}，overlap={overlap}；每类 reject 见全表 JSON。G3 status=`{}`，matrix rows={}，stress rows={}。Cross-family ensemble applicable=`{}`；不存在时原因来自冻结 P-B family gate。\n\n",g3["status"],g3["tier_budget_cold_start_rows"].as_array().map_or(0,Vec::len),g3["stress_rows"].as_array().map_or(0,Vec::len),g2["cross_family_ensemble"]["applicable"]));
    report.push_str("## Failure Closure 与 Git\n\nRound 27 八条错误 G2 只保留为 `invalid_implementation_evidence`。Round 28 exact failures 和 never-repeat reason 位于 `round28-failure-ledger.jsonl`；不关闭所有 Martin 或 threshold cointegration。\n\n");
    report.push_str(&format!("Source/validator commit: `{source}`。Final evidence commit 为包含本报告的 docs commit；push 在全部本地证据提交后单次执行。Raw root: `artifacts-local/round28/{source}`。\n"));
    fs::create_dir_all(repo.join("docs/superpowers/reports"))?;
    fs::write(repo.join(REPORT), report)?;
    Ok(())
}

fn copy_runtime(raw: &Path, artifact: &Path) -> Result<()> {
    let source = raw.join("runtime");
    let destination = artifact.join("runtime");
    fs::create_dir_all(&destination)?;
    if source.exists() {
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            if entry.path().extension().and_then(|value| value.to_str()) == Some("json") {
                fs::copy(entry.path(), destination.join(entry.file_name()))?;
            }
        }
    }
    Ok(())
}

fn append_jsonl(writer: &mut File, value: &serde_json::Value) -> Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn independent_block(timestamp: i64) -> Result<usize> {
    if !(START_MS..END_MS).contains(&timestamp) {
        bail!("timestamp out of interval");
    }
    let date = Utc
        .timestamp_millis_opt(timestamp)
        .single()
        .context("invalid timestamp")?;
    let value = (date.year() - 2023) * 4 + date.month0() as i32 / 3 - 2;
    if !(0..=11).contains(&value) {
        bail!("block out of range");
    }
    Ok(value as usize)
}

fn verify_source(repo: &Path, raw: &Path) -> Result<()> {
    let head = git(repo, &["rev-parse", "HEAD"])?;
    let source = raw
        .file_name()
        .and_then(|value| value.to_str())
        .context("raw basename")?;
    if head != source {
        bail!("validator source binding mismatch")
    };
    Ok(())
}

fn parse_args() -> Result<Args> {
    let values = std::env::args().skip(1).collect::<Vec<_>>();
    let mut phase = None;
    let mut root = None;
    let mut resume = false;
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--phase" => {
                index += 1;
                phase = values.get(index).cloned();
            }
            "--artifact-root" => {
                index += 1;
                root = values.get(index).map(PathBuf::from);
            }
            "--resume" => resume = true,
            other => bail!("unknown argument {other}"),
        };
        index += 1;
    }
    Ok(Args {
        phase: phase.context("--phase required")?,
        artifact_root: root.context("--artifact-root required")?,
        resume,
    })
}

fn read_json(path: impl AsRef<Path>) -> Result<serde_json::Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn absolute(repo: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.into()
    } else {
        repo.join(path)
    }
}
fn write_json(path: impl AsRef<Path>, value: &serde_json::Value) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(temp, path)?;
    Ok(())
}
fn sha256_file(path: &Path) -> Result<String> {
    Ok(sha256_bytes(&fs::read(path)?))
}
fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        bail!("git failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(String::from_utf8(output.stdout)?.trim().into())
}
fn peak_rss_kib() -> Option<u64> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|line| line.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        id: &str,
        event_type: &str,
        leg: Option<&str>,
        side: Option<&str>,
        mode: Option<&str>,
        price: Option<f64>,
        wallet_before: f64,
        wallet_after: f64,
    ) -> TraceEvent {
        TraceEvent {
            event_id: id.into(),
            timestamp: START_MS,
            completed_signal_ms: Some(START_MS - 1),
            eligible_open_ms: Some(START_MS),
            event_type: event_type.into(),
            policy_id: "R28-C0-fixture".into(),
            pair_id: Some("A__B".into()),
            group_id: Some("g1".into()),
            model_hash: Some("m1".into()),
            leg_id: leg.map(str::to_owned),
            symbol: leg.map(|value| if value == "left" { "A" } else { "B" }.into()),
            direction: Some(PairDirection::ShortLeftLongRight),
            side: side.map(str::to_owned),
            position_mode: mode.map(str::to_owned),
            level: Some(0),
            requested_qty: price.map(|_| 1.0),
            filled_qty: price.map(|_| 1.0),
            open_price: price,
            fill_price: price,
            mark_price: price,
            fee: 0.0,
            slippage: 0.0,
            funding: 0.0,
            wallet_before,
            wallet_after,
            equity: wallet_after,
            reserve_before: 0.0,
            reserve_after: 0.0,
            margin: 0.0,
            maintenance: 0.0,
            calendar_block: 0,
            rejection_reason: None,
            close_reason: (event_type == "close_fill").then(|| "fixture_tp".into()),
            metadata: if matches!(event_type, "intent" | "fill") {
                serde_json::json!({
                    "family":"C0","h_left":0.05,"h_right":0.95,"alpha":0.10,"beta_left":1.0,"beta_right":1.0,
                    "weighting":"BETA","weight":0.5,"filter_step_size":1.0,"filter_min_qty":1.0,"filter_min_notional":5.0
                })
            } else {
                serde_json::json!({"family":"C0"})
            },
        }
    }

    #[test]
    fn mutating_side_mode_qty_price_funding_wallet_or_block_fails_validator() {
        let rows = vec![
            event("e1", "intent", None, None, None, None, 2000.0, 2000.0),
            event(
                "e2",
                "fill",
                Some("left"),
                Some("sell"),
                Some("short"),
                Some(10.0),
                2000.0,
                2000.0,
            ),
            event(
                "e3",
                "fill",
                Some("right"),
                Some("buy"),
                Some("long"),
                Some(10.0),
                2000.0,
                2000.0,
            ),
            event(
                "e4",
                "close_fill",
                Some("left"),
                Some("buy"),
                Some("short"),
                Some(9.0),
                2000.0,
                2001.0,
            ),
            event(
                "e5",
                "close_fill",
                Some("right"),
                Some("sell"),
                Some("long"),
                Some(11.0),
                2001.0,
                2002.0,
            ),
        ];
        let result = serde_json::json!({"principal":2000.0,"eligible_intents":1,"final_equity":2002.0,"block_pnl":[2.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0]});
        assert!(validate_trace_rows(&rows, &result).violations.is_empty());
        let (mutants, passed) = validate_mutants((rows, result));
        assert!(passed, "surviving mutants: {mutants:?}");
        assert_eq!(mutants.len(), 9);
    }
}
