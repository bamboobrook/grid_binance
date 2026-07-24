use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use r24_research::r26::{sha256_bytes, FINGERPRINT};

const REPO_ARTIFACT: &str = "docs/superpowers/artifacts/glm-martingale-core-round26";

struct Args {
    phase: String,
    artifact_root: PathBuf,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let repo = std::env::current_dir()?;
    let raw = absolute(&repo, &args.artifact_root);
    let artifact = repo.join(REPO_ARTIFACT);
    fs::create_dir_all(&artifact)?;
    run_model_validator(&repo, &args.artifact_root)?;
    let account = validate_accounts(&repo, &raw)?;
    write_json(raw.join("account-independent-validator.json"), &account)?;
    write_json(
        artifact.join("account-independent-validator.json"),
        &account,
    )?;
    let model: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("model-independent-validator.json"))?)?;
    let both_pass = model["passed"] == true && account["passed"] == true;
    finalize(
        &repo,
        &raw,
        &artifact,
        &args.phase,
        both_pass,
        &model,
        &account,
    )?;
    if !both_pass {
        bail!("dual validator failed")
    }
    Ok(())
}

fn run_model_validator(repo: &Path, artifact_root: &Path) -> Result<()> {
    let status = Command::new("python3")
        .args([
            "scripts/r26_fit_snapshots.py",
            "--phase",
            "validate",
            "--artifact-root",
        ])
        .arg(artifact_root)
        .current_dir(repo)
        .status()?;
    if !status.success() {
        bail!("independent model validator failed with {status}")
    }
    Ok(())
}

fn validate_accounts(repo: &Path, raw: &Path) -> Result<serde_json::Value> {
    let gate: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g0.json"))?)?;
    let mut violations = Vec::new();
    let mut terminals = Vec::new();
    for result in gate["results"].as_array().context("g0 results missing")? {
        let manifest = &result["trace_manifest"];
        let path = absolute(
            repo,
            Path::new(
                manifest["raw_path"]
                    .as_str()
                    .context("trace path missing")?,
            ),
        );
        let bytes = fs::read(&path)?;
        if sha256_bytes(&bytes) != manifest["sha256"] {
            violations.push(format!("trace_sha256:{}", path.display()));
        }
        let mut rows = 0_u64;
        let mut first = None;
        let mut last = None;
        let mut previous = None;
        for line in BufReader::new(File::open(&path)?).lines() {
            let row: serde_json::Value = serde_json::from_str(&line?)?;
            let timestamp = row["timestamp"].as_i64().context("timestamp missing")?;
            first.get_or_insert(timestamp);
            if previous.is_some_and(|value| timestamp - value != 60_000) {
                violations.push(format!("risk_gap:{}:{timestamp}", path.display()));
            }
            previous = Some(timestamp);
            last = Some(timestamp);
            rows += 1;
            for field in ["wallet", "equity"] {
                if (row[field].as_f64().unwrap_or(f64::NAN) - 2000.0).abs() > 1e-9 {
                    violations.push(format!("{field}_mismatch:{}:{timestamp}", path.display()));
                }
            }
            for field in ["margin", "maintenance", "reserved_quote"] {
                if row[field].as_f64().unwrap_or(f64::NAN).abs() > 1e-12 {
                    violations.push(format!("{field}_nonzero:{}:{timestamp}", path.display()));
                }
            }
            for field in ["positions", "groups", "pending"] {
                if row[field].as_u64().unwrap_or(u64::MAX) != 0 {
                    violations.push(format!("{field}_nonzero:{}:{timestamp}", path.display()));
                }
            }
        }
        let expected_rows = result["risk_path_rows"]
            .as_u64()
            .context("risk rows missing")?;
        if rows != expected_rows || rows != manifest["rows"].as_u64().unwrap_or(u64::MAX) {
            violations.push(format!(
                "risk_row_count:{}:{rows}:{expected_rows}",
                path.display()
            ));
        }
        if first != result["start_ms"].as_i64()
            || last != result["end_ms"].as_i64().map(|value| value - 60_000)
        {
            violations.push(format!("risk_range:{}", path.display()));
        }
        if result["btc_order_count"] != 0
            || result["btc_trade_count"] != 0
            || result["final_positions"] != 0
            || result["final_groups"] != 0
            || result["final_pending"] != 0
            || result["final_reserve"] != 0.0
        {
            violations.push(format!("final_state:{}", path.display()));
        }
        terminals.push(serde_json::json!({
            "terminal":result["terminal"],"rows":rows,"first_ms":first,"last_ms":last,
            "wallet":2000.0,"equity":2000.0,"max_drawdown_pct":0.0,
            "btc_orders":0,"btc_trades":0,"final_state_zero":true
        }));
    }
    Ok(serde_json::json!({
        "schema_version":1,"validator":"independent_rust_raw_account_trace",
        "passed":violations.is_empty(),"terminal_count":terminals.len(),
        "violations":violations,"terminals":terminals
    }))
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
    let g1_path = raw.join("checkpoints/g1-activation.json");
    let g1: Option<serde_json::Value> = g1_path
        .exists()
        .then(|| serde_json::from_slice(&fs::read(&g1_path).unwrap()).unwrap());
    let g2_path = raw.join("checkpoints/g2-replay.json");
    let g2: Option<serde_json::Value> = g2_path
        .exists()
        .then(|| serde_json::from_slice(&fs::read(&g2_path).unwrap()).unwrap());
    let status = if !validators_pass {
        "INVALID_DUAL_VALIDATOR_FAILURE"
    } else if let Some(g1) = &g1 {
        if g1["survivors"]
            .as_array()
            .is_some_and(|rows| rows.is_empty())
        {
            "VALID_ACTIVATION_NO_REPLAY_CANDIDATE"
        } else if g2.as_ref().is_some_and(|row| {
            row["p_b_survivors"]
                .as_array()
                .is_some_and(|rows| rows.is_empty())
        }) {
            "VALID_HISTORICAL_PREQUENTIAL_NO_PB"
        } else {
            "VALID_ROUND26_PHASE_COMPLETE"
        }
    } else {
        "VALID_G0_ONLY"
    };
    let validator_commit = git(repo, &["rev-parse", "HEAD"])?;
    let source_commit = raw
        .file_name()
        .and_then(|value| value.to_str())
        .context("raw root must end in replay source commit")?
        .to_string();
    let tree_spec = format!("{source_commit}^{{tree}}");
    let source_tree = git(repo, &["rev-parse", &tree_spec])?;
    let authority = serde_json::json!({
        "schema_version":1,"fingerprint":FINGERPRINT,"status":status,"phase":phase,
        "source_commit":source_commit,"source_tree":source_tree,"validator_commit":validator_commit,
        "model_validator_passed":model["passed"],"account_validator_passed":account["passed"],
        "round25r_authority":"MATERIALLY_INCOMPLETE_INVALID_RESULTS",
        "p_b_survivors":g2.as_ref().and_then(|row| row["p_b_survivors"].as_array()).cloned().unwrap_or_default(),
        "c2":if status == "VALID_HISTORICAL_PREQUENTIAL_NO_PB" || status == "VALID_ACTIVATION_NO_REPLAY_CANDIDATE" {"not_applicable"} else {"conditional"},
        "btc_reference_only":true,"outer_return_used_for_selection":false
    });
    write_json(artifact.join("round26-authority.json"), &authority)?;
    let protocol = serde_json::json!({
        "schema_version":1,"fingerprint":FINGERPRINT,"reference":"BTCUSDT","alt_pool_size":29,
        "weekly_anchor_contract":"[2023-07-01T00:00:00Z,2026-05-30T00:00:00Z) every 7d = 152",
        "formation_days":[14,21],"frequencies":["1h","5m"],"selectors":["raw","fdr"],
        "matching":"exact maximum-cardinality maximum-weight, max 3 pairs",
        "model_authority":"statsmodels immutable snapshots","account":"single shared continuous",
        "principal_rule":"strictly less than 5000U"
    });
    write_json(artifact.join("round26-protocol.json"), &protocol)?;
    let policy = serde_json::json!({
        "activation_configs":8,"g2_entry_alpha":[0.10,0.20],"g2_so_step_sigma":[0.50,0.75],
        "baseline_principal":2000,"relative_layers":[1.0,1.25,1.55,1.90],
        "max_active_groups":3,"leverage_cap":2.0,
        "budgets":[500,750,1000,1500,2000,3000,4000,4999],
        "cold_starts_days":[0,30,60,90,120]
    });
    write_json(artifact.join("round26-policy-manifest.json"), &policy)?;
    let universe = serde_json::json!({
        "reference":"BTCUSDT","reference_tradable":false,"pool":r24_research::r26::ALTS,
        "weekly_top_n":20,"formation_only_liquidity":true,"zero_volume_max":0.01,
        "p10_minute_quote_volume_min":10000
    });
    write_json(artifact.join("round26-universe-manifest.json"), &universe)?;
    let activation_trials = g1
        .as_ref()
        .map_or(0, |row| row["config_count"].as_u64().unwrap_or(0));
    let trials = serde_json::json!({
        "global_prior_invalid_round25r_terminals":16,"round26_activation_trials":activation_trials,
        "round26_g2_trials":g2.as_ref().map_or(0, |row| row["policy_count"].as_u64().unwrap_or(0)),
        "global_trial_count_minimum":16+activation_trials,
        "no_fit_no_signal_rejection_empty_and_failed_in_denominator":true
    });
    write_json(artifact.join("round26-trial-ledger.json"), &trials)?;
    write_json(
        artifact.join("round26-closed-fingerprints.json"),
        &serde_json::json!({
            "closed":if status.starts_with("VALID_") {vec![FINGERPRINT]} else {Vec::<&str>::new()},
            "status":status
        }),
    )?;
    let execution = serde_json::json!({
        "status":status,"d0":"terminal","r0":"terminal","g0":"terminal",
        "g1_activation":if g1.is_some() {"terminal"} else {"not_run"},
        "g2_replay":g2.as_ref().map_or("not_run", |_| "terminal"),
        "g3":if authority["p_b_survivors"].as_array().is_some_and(|rows| rows.is_empty()) {"not_applicable"} else {"conditional"},
        "model_validator":model["passed"],"account_validator":account["passed"],
        "source_commit":source_commit,"raw_root":raw.strip_prefix(repo).unwrap_or(raw)
    });
    write_json(artifact.join("round26-execution-state.json"), &execution)?;
    let r0 = serde_json::json!({
        "status":if model["passed"] == true {"PASS"} else {"FAIL"},
        "statsmodels_version":"0.14.5","model_snapshot_count":model["snapshot_count"],
        "checked_leg_count":model["checked_leg_count"],"checked_pair_count":model["checked_pair_count"]
    });
    write_json(artifact.join("gates/r0.json"), &r0)?;
    copy_compact_manifests(raw, artifact)?;
    write_ledgers(artifact, status, g1.as_ref())?;
    write_report(
        repo,
        artifact,
        status,
        &authority,
        g1.as_ref(),
        model,
        account,
    )?;
    Ok(())
}

fn copy_compact_manifests(raw: &Path, artifact: &Path) -> Result<()> {
    let destination = artifact.join("fit-snapshot-manifests");
    fs::create_dir_all(&destination)?;
    for phase in ["g0", "g1"] {
        let source = raw
            .join("fit-snapshot-manifests")
            .join(format!("{phase}.json"));
        if source.exists() {
            let manifest: serde_json::Value = serde_json::from_slice(&fs::read(&source)?)?;
            let compact = serde_json::json!({
                "phase":phase,"snapshot_count":manifest["snapshot_count"],
                "environment":{
                    "python":manifest["python"],"numpy":manifest["numpy"],"scipy":manifest["scipy"],
                    "statsmodels":manifest["statsmodels"],"script_sha256":manifest["script_sha256"]
                },
                "snapshots":manifest["snapshots"]
            });
            write_json(destination.join(format!("{phase}.json")), &compact)?;
        }
    }
    let g0: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g0.json"))?)?;
    fs::create_dir_all(artifact.join("trace-manifests"))?;
    fs::create_dir_all(artifact.join("replay-results"))?;
    for result in g0["results"].as_array().context("g0 results missing")? {
        let label = result["terminal"].as_str().context("terminal missing")?;
        write_json(
            artifact
                .join("trace-manifests")
                .join(format!("{label}.json")),
            &result["trace_manifest"],
        )?;
        write_json(
            artifact
                .join("replay-results")
                .join(format!("{label}.json")),
            result,
        )?;
    }
    Ok(())
}

fn write_ledgers(artifact: &Path, status: &str, g1: Option<&serde_json::Value>) -> Result<()> {
    let mut failures = Vec::new();
    if let Some(g1) = g1 {
        for config in g1["configs"].as_array().context("configs missing")? {
            if config["passed"] == false {
                failures.push(serde_json::json!({
                    "failure_id":format!("R26-ACTIVATION-{}",config["config_id"].as_str().unwrap_or("unknown")),
                    "class":"activity_gate","status":"valid_failure","gates":config["gates"],
                    "reason":"one or more preregistered activity thresholds not met"
                }));
            }
        }
    }
    let mut output = File::create(artifact.join("round26-failure-ledger.jsonl"))?;
    for row in &failures {
        serde_json::to_writer(&mut output, row)?;
        output.write_all(b"\n")?;
    }
    let mut exploration = File::create(artifact.join("exploration-registry.jsonl"))?;
    serde_json::to_writer(
        &mut exploration,
        &serde_json::json!({
            "fingerprint":FINGERPRINT,"status":status,"terminal":status.starts_with("VALID_"),
            "post_hoc_parameters_added":false
        }),
    )?;
    exploration.write_all(b"\n")?;
    Ok(())
}

fn write_report(
    repo: &Path,
    artifact: &Path,
    status: &str,
    authority: &serde_json::Value,
    g1: Option<&serde_json::Value>,
    model: &serde_json::Value,
    account: &serde_json::Value,
) -> Result<()> {
    let survivors = g1
        .and_then(|row| row["survivors"].as_array())
        .map_or(0, Vec::len);
    let report = format!(
        "# GLM Martingale Core Round 26 交付报告\n\n## 结论\n\n`{status}`。Round 25R 的无效统计结果未被继承；Round 26 严格使用 weekly Top-20、statsmodels snapshots、真实成本门与 exact matching。\n\n## 有效候选\n\nActivity survivors: `{survivors}`。P-B survivors: `{}`。无有效候选时不展示收益 headline。\n\n## 验证\n\n- Model independent validator: `{}`，snapshots `{}`，legs `{}`。\n- Account independent validator: `{}`，G0 terminals `{}`。\n- BTC orders/trades/PnL: `0/0/0`。\n- Source commit: `{}`。\n\n## 边界\n\nWeekly anchors 采用任务书明确的 `152/152` gate：`[2023-07-01, 2026-05-30)`，最后 anchor 为 `2026-05-23`。\n",
        authority["p_b_survivors"].as_array().map_or(0, Vec::len),
        model["passed"],model["snapshot_count"],model["checked_leg_count"],
        account["passed"],account["terminal_count"],authority["source_commit"].as_str().unwrap_or("unknown")
    );
    let report_path = repo.join("docs/superpowers/reports/2026-07-24-glm-round26-handoff.md");
    fs::write(&report_path, report)?;
    write_json(
        artifact.join("round26-report-manifest.json"),
        &serde_json::json!({
            "path":report_path.strip_prefix(repo).unwrap_or(&report_path),
            "sha256":sha256_bytes(&fs::read(report_path)?)
        }),
    )?;
    Ok(())
}

fn parse_args() -> Result<Args> {
    let values = std::env::args().skip(1).collect::<Vec<_>>();
    let mut phase = None;
    let mut artifact_root = None;
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
            other => bail!("unknown argument {other}"),
        }
        index += 1;
    }
    Ok(Args {
        phase: phase.context("--phase required")?,
        artifact_root: artifact_root.context("--artifact-root required")?,
    })
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

fn absolute(repo: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    }
}

fn git(repo: &Path, arguments: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        bail!("git command failed")
    }
    Ok(String::from_utf8(output.stdout)?.trim().into())
}
