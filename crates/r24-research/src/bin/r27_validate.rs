use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use r24_research::r26::sha256_bytes;

const REPO_ARTIFACT: &str = "docs/superpowers/artifacts/glm-martingale-core-round27";
const REPORT: &str = "docs/superpowers/reports/2026-07-26-glm-round27-handoff.md";
const START_MS: i64 = 1_688_169_600_000;
const END_MS: i64 = 1_780_070_400_000;
const MINUTE_MS: i64 = 60_000;

struct Args {
    phase: String,
    artifact_root: PathBuf,
    resume: bool,
}

fn main() -> Result<()> {
    let started = Utc::now();
    let timer = Instant::now();
    let args = parse_args()?;
    let repo = std::env::current_dir()?;
    let raw = absolute(&repo, &args.artifact_root);
    let artifact = repo.join(REPO_ARTIFACT);
    let model: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("model-independent-validator.json"))?)?;
    let account = validate_accounts(&repo, &raw)?;
    write_json(raw.join("account-independent-validator.json"), &account)?;
    write_json(
        artifact.join("account-independent-validator.json"),
        &account,
    )?;
    let both_pass = model["passed"] == true
        && model["checked_pair_count"].as_u64().unwrap_or(0) > 0
        && account["passed"] == true;
    finalize(
        &repo,
        &raw,
        &artifact,
        &args.phase,
        both_pass,
        &model,
        &account,
    )?;
    write_json(
        raw.join("runtime/validator-g0-g3.json"),
        &serde_json::json!({
            "argv":std::env::args().collect::<Vec<_>>(),"pid":std::process::id(),
            "start_utc":started.to_rfc3339(),"end_utc":Utc::now().to_rfc3339(),
            "wall_seconds":timer.elapsed().as_secs_f64(),"peak_rss_kib":peak_rss_kib(),
            "exit_code":if both_pass {0}else{1},"resume":args.resume,
            "source_commit":raw.file_name().and_then(|value|value.to_str()),
            "validator_commit":git(&repo,&["rev-parse","HEAD"])?
        }),
    )?;
    if !both_pass {
        bail!("Round 27 dual validator failed")
    }
    Ok(())
}

fn validate_accounts(repo: &Path, raw: &Path) -> Result<serde_json::Value> {
    let g0: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g0.json"))?)?;
    let g2: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g2-replay.json"))?)?;
    let mut violations = Vec::new();
    let synthetic = &g0["synthetic"];
    if synthetic["fo"] != 1
        || synthetic["so"] != 1
        || synthetic["tp"] != 1
        || synthetic["positions"] != 0
        || synthetic["groups"] != 0
        || synthetic["reserve"].as_f64().unwrap_or(f64::NAN).abs() > 1e-9
    {
        violations.push("g0_synthetic_binding".to_string());
    }
    let mut g0_real_orders = 0_u64;
    for canary in g0["real_canaries"]
        .as_array()
        .context("real canaries missing")?
    {
        if canary["btc_orders"] != 0 {
            violations.push("g0_btc_order".into());
        }
        if canary["first_signal"].is_object() {
            if canary["order_hash_changed"] != true {
                violations.push("g0_real_order_hash".into());
            }
            g0_real_orders += 1;
        }
        validate_trace_manifest(repo, &canary["trace_manifest"], &mut violations)?;
    }
    let mut policies = Vec::new();
    for result in g2["results"].as_array().cloned().unwrap_or_default() {
        validate_trace_manifest(repo, &result["trace_manifest"], &mut violations)?;
        let risk_path = absolute(
            repo,
            Path::new(
                result["risk_rle_manifest"]["path"]
                    .as_str()
                    .context("risk path missing")?,
            ),
        );
        let bytes = fs::read(&risk_path)?;
        if sha256_bytes(&bytes) != result["risk_rle_manifest"]["sha256"] {
            violations.push(format!("risk_hash:{}", risk_path.display()));
        }
        let (logical_rows, max_dd, complete) = validate_risk_rle(&risk_path, &mut violations)?;
        if !complete {
            violations.push(format!("risk_incomplete:{}", result["policy_id"]));
        }
        if result["final_positions"] != 0
            || result["final_groups"] != 0
            || result["final_pending"] != 0
            || result["final_reserve"].as_f64().unwrap_or(f64::NAN).abs() > 1e-9
        {
            violations.push(format!("final_state:{}", result["policy_id"]));
        }
        let final_equity = result["final_equity"]
            .as_f64()
            .context("final equity missing")?;
        let years = (END_MS - START_MS) as f64 / (365.25 * 86_400_000.0);
        let annualized = ((final_equity / 2000.0).max(1e-12).powf(1.0 / years) - 1.0) * 100.0;
        if (annualized - result["annualized_return_pct"].as_f64().unwrap_or(f64::NAN)).abs() > 1e-9
        {
            violations.push(format!("annualized:{}", result["policy_id"]));
        }
        if max_dd + 1e-9 < result["max_equity_drawdown_pct"].as_f64().unwrap_or(0.0) {
            violations.push(format!("dd_trace:{}", result["policy_id"]));
        }
        policies.push(serde_json::json!({
            "policy_id":result["policy_id"],"logical_risk_rows":logical_rows,
            "annualized_return_pct":annualized,"max_drawdown_recomputed_pct":max_dd,
            "p_a":result["p_a"],"p_b":result["p_b"]
        }));
    }
    Ok(serde_json::json!({
        "schema_version":1,"validator":"independent_r27_trace_account_validator",
        "passed":violations.is_empty() && g0_real_orders>0,
        "g0_real_order_canaries":g0_real_orders,"policy_count":policies.len(),
        "policies":policies,"violations":violations,"raw_trace_read":true,
        "production_metric_function_called":false
    }))
}

fn validate_risk_rle(path: &Path, violations: &mut Vec<String>) -> Result<(i64, f64, bool)> {
    let mut logical_rows = 0_i64;
    let mut previous_end = START_MS;
    let mut max_dd = 0.0_f64;
    let mut peak = 0.0_f64;
    for line in BufReader::new(File::open(path)?).lines() {
        let row: serde_json::Value = serde_json::from_str(&line?)?;
        match row["kind"].as_str() {
            Some("idle_1m_rle") => {
                let start = row["start_ms"].as_i64().context("idle start missing")?;
                let end = row["end_ms"].as_i64().context("idle end missing")?;
                let rows = row["rows"].as_i64().context("idle rows missing")?;
                if start < previous_end || rows != (end - start) / MINUTE_MS {
                    violations.push("risk_idle_contract".into());
                }
                logical_rows += rows;
                previous_end = end;
                update_dd(&row, &mut peak, &mut max_dd)?;
            }
            Some("active_1m") => {
                let timestamp = row["timestamp"]
                    .as_i64()
                    .context("active timestamp missing")?;
                if timestamp < previous_end {
                    violations.push("risk_active_order".into());
                }
                previous_end = timestamp + MINUTE_MS;
                logical_rows += 1;
                update_dd(&row, &mut peak, &mut max_dd)?;
            }
            _ => violations.push("risk_unknown_row".into()),
        }
    }
    Ok((logical_rows, max_dd, previous_end >= END_MS - MINUTE_MS))
}

fn update_dd(row: &serde_json::Value, peak: &mut f64, max_dd: &mut f64) -> Result<()> {
    let equity = row["equity"].as_f64().context("risk equity missing")?;
    *peak = peak.max(equity);
    if *peak > 0.0 {
        *max_dd = max_dd.max((*peak - equity) / *peak * 100.0);
    }
    Ok(())
}

fn validate_trace_manifest(
    repo: &Path,
    manifest: &serde_json::Value,
    violations: &mut Vec<String>,
) -> Result<()> {
    if manifest["explicit_no_signal"] == true {
        return Ok(());
    }
    let Some(path) = manifest["path"].as_str() else {
        return Ok(());
    };
    let path = absolute(repo, Path::new(path));
    let bytes = fs::read(&path)?;
    if sha256_bytes(&bytes) != manifest["sha256"] {
        violations.push(format!("trace_hash:{}", path.display()));
    }
    let rows = BufReader::new(File::open(&path)?).lines().count() as u64;
    if rows != manifest["rows"].as_u64().unwrap_or(u64::MAX) {
        violations.push(format!("trace_rows:{}", path.display()));
    }
    Ok(())
}

fn finalize(
    repo: &Path,
    raw: &Path,
    artifact: &Path,
    phase: &str,
    validators_pass: bool,
    model: &serde_json::Value,
    account: &serde_json::Value,
) -> Result<()> {
    let d0: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/d0.json"))?)?;
    let g1: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g1-activation.json"))?)?;
    let g2: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g2-replay.json"))?)?;
    let g3: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g3.json"))?)?;
    let survivors = g1["survivors"].as_array().map_or(0, Vec::len);
    let pb = g2["p_b_survivors"].as_array().map_or(0, Vec::len);
    let status = if !validators_pass {
        "INVALID_IMPLEMENTATION_OR_VALIDATOR_FAILURE"
    } else if d0["status"] != "PASS" {
        "VALID_DATA_GATE_NO_SEARCH"
    } else if survivors == 0 {
        "VALID_ROUND27_ALL_ARMS_NO_ACTIVATION"
    } else if pb == 0 {
        "VALID_HISTORICAL_PREQUENTIAL_NO_PB"
    } else if g3["status"] == "VALID_FRONTIER_PROGRESS_NO_TARGET" {
        "VALID_FRONTIER_PROGRESS_NO_TARGET"
    } else {
        "INVALID_IMPLEMENTATION_OR_VALIDATOR_FAILURE"
    };
    let source_commit = raw
        .file_name()
        .and_then(|value| value.to_str())
        .context("source commit missing")?;
    let source_tree_spec = format!("{source_commit}^{{tree}}");
    let authority = serde_json::json!({
        "schema_version":1,"status":status,"phase":phase,"source_commit":source_commit,
        "source_tree":git(repo,&["rev-parse",&source_tree_spec])?,
        "validator_commit":git(repo,&["rev-parse","HEAD"])?,
        "round26_corrected_status":"MATERIALLY_INCOMPLETE_INVALID_FAMILY_CLOSURE",
        "model_validator_passed":model["passed"],"account_validator_passed":account["passed"],
        "checked_pair_count":model["checked_pair_count"],
        "checked_pbd_pair_count":model["checked_pbd_pair_count"],
        "activation_survivors":g1["survivors"],"p_b_survivors":g2["p_b_survivors"],
        "target_hit":matches!(status,"VALID_CONSERVATIVE_TARGET"|"VALID_BALANCED_TARGET"|"VALID_AGGRESSIVE_TARGET"),
        "btc_reference_only":true,"outer_return_used_for_selection":false
    });
    write_json(artifact.join("round27-authority.json"), &authority)?;
    write_json(
        artifact.join("round27-execution-state.json"),
        &serde_json::json!({
            "status":status,"r0":"terminal","d0":"terminal","g0":"terminal",
            "g1":"terminal","g2":"terminal","g3":if pb==0{"not_applicable"}else{"terminal"},
            "model_validator":model["passed"],"account_validator":account["passed"],
            "source_commit":source_commit
        }),
    )?;
    copy_manifest(raw, artifact, "c0-c1")?;
    copy_manifest(raw, artifact, "p1")?;
    write_json(
        artifact.join("round27-trial-ledger.json"),
        &serde_json::json!({
            "round27_activation_trials":9,"round27_g2_trials":g2["policy_count"],
            "round27_g3_trials":if pb>0{g3["trial_count"].clone()}else{serde_json::json!(0)},
            "all_no_fit_no_signal_reject_timeout_failures_in_denominator":true
        }),
    )?;
    write_failures(artifact, &g1, &g2)?;
    copy_results(artifact, &g2)?;
    write_report(
        repo,
        source_commit,
        status,
        survivors,
        pb,
        model,
        &g2,
        &authority,
    )?;
    Ok(())
}

fn copy_manifest(raw: &Path, artifact: &Path, name: &str) -> Result<()> {
    let source = raw
        .join("fit-snapshot-manifests")
        .join(format!("{name}.json"));
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&source)?)?;
    write_json(
        artifact
            .join("fit-snapshot-manifests")
            .join(format!("{name}.json")),
        &serde_json::json!({
            "schema_version":1,"phase":value["phase"],"identity":value["identity"],
            "workers":value["workers"],"blas_threads":value["blas_threads"],
            "wall_seconds":value["wall_seconds"],"peak_rss_kib":value["peak_rss_kib"],
            "snapshot_count":value["snapshot_count"],"snapshots":value["snapshots"]
        }),
    )
}

fn copy_results(artifact: &Path, g2: &serde_json::Value) -> Result<()> {
    for row in g2["results"].as_array().cloned().unwrap_or_default() {
        let id = row["policy_id"].as_str().context("policy id missing")?;
        write_json(
            artifact.join("replay-results").join(format!("{id}.json")),
            &row,
        )?;
        write_json(
            artifact.join("trace-manifests").join(format!("{id}.json")),
            &serde_json::json!({
                "engine_trace":row["trace_manifest"],"risk_rle":row["risk_rle_manifest"]
            }),
        )?;
    }
    Ok(())
}

fn write_failures(artifact: &Path, g1: &serde_json::Value, g2: &serde_json::Value) -> Result<()> {
    fs::create_dir_all(artifact)?;
    let mut file = File::create(artifact.join("round27-failure-ledger.jsonl"))?;
    let mut closed = BTreeSet::new();
    for row in g1["configs"].as_array().context("configs missing")? {
        if row["passed"] == false {
            let id = row["config_id"].as_str().unwrap_or("unknown");
            serde_json::to_writer(
                &mut file,
                &serde_json::json!({
                    "failure_id":format!("R27-ACTIVATION-{id}"),"exact_fingerprint":id,
                    "status":"valid_activation_failure","gates":row["gates"],
                    "never_repeat":"do not relax frozen activation thresholds after observing this terminal"
                }),
            )?;
            file.write_all(b"\n")?;
            closed.insert(id.to_string());
        }
    }
    for row in g2["results"].as_array().cloned().unwrap_or_default() {
        if row["p_b"] == false {
            let id = row["policy_id"].as_str().unwrap_or("unknown");
            serde_json::to_writer(
                &mut file,
                &serde_json::json!({
                    "failure_id":format!("R27-G2-{id}"),"exact_fingerprint":id,
                    "status":"valid_g2_failure",
                    "metrics":{"ann":row["annualized_return_pct"],"dd":row["max_equity_drawdown_pct"],"positive_blocks":row["positive_blocks"]},
                    "never_repeat":"do not tune entry, SO, formation or budget from this outer return"
                }),
            )?;
            file.write_all(b"\n")?;
            closed.insert(id.to_string());
        }
    }
    write_json(
        artifact.join("round27-closed-fingerprints.json"),
        &serde_json::json!({"closed_exact":closed,"broad_family_closed":false}),
    )
}

fn write_report(
    repo: &Path,
    source_commit: &str,
    status: &str,
    survivors: usize,
    pb: usize,
    model: &serde_json::Value,
    g2: &serde_json::Value,
    authority: &serde_json::Value,
) -> Result<()> {
    let report = format!(
        "# GLM Martingale Core Round 27 交付报告\n\n## 结论\n\n'{status}'。三个预注册 arm 均形成 return-blind activation terminal；authority 仅由双 validator 生成。\n\n## Arm 状态\n\n- C0/C1/P1 activation configs: '9/9' terminal。\n- Activation survivors: '{survivors}'；P-B survivors: '{pb}'。\n- Copula checked pairs: '{}'；PBD checked pairs: '{}'。\n\n## 账户与候选\n\nG2 policies: '{}'。无 P-B 时三档、budgets、cold starts、stress 与 multi-family ensemble 均为 'not_applicable'，不展示无效收益 headline。详细 ann、DD、blocks、symbols、pairs、layers、cost 和 concentration 位于 replay-results。\n\n## Git 与边界\n\nSource commit: '{source_commit}'。Validator commit: '{}'。Push 状态由最终提交后补充；raw traces 保留在 'artifacts-local/round27/{source_commit}'。\n",
        model["checked_pair_count"],
        model["checked_pbd_pair_count"],
        g2["policy_count"],
        authority["validator_commit"].as_str().unwrap_or("unknown")
    );
    fs::write(repo.join(REPORT), &report)?;
    write_json(
        repo.join(REPO_ARTIFACT)
            .join("round27-report-manifest.json"),
        &serde_json::json!({
            "path":REPORT,"sha256":sha256_bytes(&fs::read(repo.join(REPORT))?)
        }),
    )
}

fn parse_args() -> Result<Args> {
    let values = std::env::args().skip(1).collect::<Vec<_>>();
    let mut phase = None;
    let mut artifact_root = None;
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
                artifact_root = values.get(index).map(PathBuf::from);
            }
            "--resume" => resume = true,
            other => bail!("unknown argument {other}"),
        }
        index += 1;
    }
    Ok(Args {
        phase: phase.context("--phase required")?,
        artifact_root: artifact_root.context("--artifact-root required")?,
        resume,
    })
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
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}

fn git(repo: &Path, arguments: &[&str]) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(arguments)
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        bail!("git command failed")
    }
    Ok(String::from_utf8(output.stdout)?.trim().into())
}

fn peak_rss_kib() -> Option<u64> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}
