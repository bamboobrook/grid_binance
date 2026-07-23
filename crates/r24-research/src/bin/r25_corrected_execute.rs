#![recursion_limit = "512"]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, SecondsFormat, Utc};
use r24_registry::{sha256, sha256_file, TRACE_KINDS};
use r24_research::r25::{
    copula_reference_report, r25_canary_report, r25_policies, R25_BLOCKS, R25_C2, R25_COLD_STARTS,
    R25_FAMILY, R25_REFERENCE_SYMBOL, R25_TRADABLE_ALTS,
};
use r24_research::r25_prod::{
    r25_symbol_gates, run_r25_c1_replay, utc_ms, R25ReplayConfig, R25ReplayEvidence, R25ReplayMode,
};

fn main() -> Result<()> {
    let cli = Cli::parse()?;
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let artifact_root = if cli.artifact_root.is_absolute() {
        cli.artifact_root.clone()
    } else {
        repo.join(&cli.artifact_root)
    };
    if artifact_root.ends_with("glm-martingale-core-round25")
        || artifact_root.ends_with("glm-martingale-core-round25-recovery")
    {
        bail!("corrected replay refuses to overwrite prior Round 25 artifact roots");
    }
    let context = RunContext::new(repo, artifact_root)?;
    match cli.phase.as_str() {
        "preflight" => run_preflight(&context, true)?,
        "g0" => {
            run_preflight(&context, false)?;
            run_g0(&context, cli.resume)?;
        }
        "g1" => {
            run_preflight(&context, false)?;
            ensure_g0_passed(&context)?;
            run_g1(&context, cli.resume)?;
        }
        "all" => {
            run_preflight(&context, false)?;
            let g0 = run_g0(&context, cli.resume)?;
            if g0.pass {
                run_g1(&context, true)?;
            }
        }
        "g2" => bail!("G2 is conditional on at least one corrected G1 P-B survivor"),
        other => bail!("unsupported --phase {other}; expected preflight|g0|g1|g2|all"),
    }
    Ok(())
}

#[derive(Debug)]
struct Cli {
    phase: String,
    resume: bool,
    artifact_root: PathBuf,
}

impl Cli {
    fn parse() -> Result<Self> {
        let mut phase = "all".to_string();
        let mut resume = false;
        let mut artifact_root =
            PathBuf::from("docs/superpowers/artifacts/glm-martingale-core-round25-corrected");
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--phase" => {
                    index += 1;
                    phase = args.get(index).context("missing --phase value")?.clone();
                }
                "--resume" => resume = true,
                "--artifact-root" => {
                    index += 1;
                    artifact_root =
                        PathBuf::from(args.get(index).context("missing --artifact-root value")?);
                }
                other => bail!("unknown argument {other}"),
            }
            index += 1;
        }
        Ok(Self {
            phase,
            resume,
            artifact_root,
        })
    }
}

#[derive(Debug, Clone)]
struct RunContext {
    repo: PathBuf,
    artifact: PathBuf,
    market_db: PathBuf,
    funding_db: PathBuf,
    exchange_info: PathBuf,
    source_update: PathBuf,
    commit: String,
    source_tree: String,
    upstream: String,
    remote_sync_status: String,
    dirty: bool,
    raw_root: PathBuf,
}

impl RunContext {
    fn new(repo: PathBuf, artifact: PathBuf) -> Result<Self> {
        let branch = git(&repo, &["branch", "--show-current"])?;
        if branch != "codex/glm-martingale-core-round25-corrected" {
            bail!("R25 corrected replay must run on corrected branch, got {branch}");
        }
        let commit = git(&repo, &["rev-parse", "HEAD"])?;
        let source_tree = git(&repo, &["rev-parse", "HEAD^{tree}"])?;
        let upstream = git_optional(&repo, &["rev-parse", "@{u}"]).unwrap_or_default();
        let dirty = !git(&repo, &["status", "--porcelain"])?.is_empty();
        if dirty {
            bail!("phase launch requires a clean immutable local commit");
        }
        let remote_sync_status = if !upstream.is_empty() && commit == upstream {
            "SYNCED"
        } else {
            "PUSH_PENDING"
        };
        let raw_root = repo.join("artifacts-local/round25-corrected").join(&commit);
        Ok(Self {
            market_db: repo.join("data/market_data_full.db"),
            funding_db: repo.join("data/funding_rates.db"),
            exchange_info: repo.join("docs/superpowers/artifacts/glm-martingale-core-round24/data/perp-exchangeInfo-8-symbols.json"),
            source_update: repo.join("docs/superpowers/artifacts/glm-martingale-core-round24/audit/round25-external-source-update.json"),
            repo,
            artifact,
            commit,
            source_tree,
            upstream,
            remote_sync_status: remote_sync_status.into(),
            dirty,
            raw_root,
        })
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct G0Summary {
    pass: bool,
    status: String,
    real_replay_count: usize,
    real_order_or_rejection: bool,
    so_guard_called: bool,
    parameter_delta: bool,
    scheduler_status: String,
}

fn run_preflight(context: &RunContext, force: bool) -> Result<()> {
    if force && context.artifact.exists() {
        fs::remove_dir_all(&context.artifact)?;
    }
    fs::create_dir_all(context.artifact.join("gates"))?;
    fs::create_dir_all(context.artifact.join("trace-manifests"))?;
    fs::create_dir_all(context.artifact.join("replay-results"))?;
    fs::create_dir_all(&context.raw_root)?;
    let policies = r25_policies();
    write_json(
        context.artifact.join("round25-corrected-source-map.json"),
        &source_map(context)?,
    )?;
    write_json(
        context.artifact.join("round25-corrected-protocol.json"),
        &protocol_json(),
    )?;
    let data_manifest_path = context
        .artifact
        .join("round25-corrected-data-manifest.json");
    let existing_data_manifest = if data_manifest_path.exists() {
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&data_manifest_path)?)?;
        let sizes_match = value["market_database"]["bytes"].as_u64()
            == Some(fs::metadata(&context.market_db)?.len())
            && value["funding_database"]["bytes"].as_u64()
                == Some(fs::metadata(&context.funding_db)?.len());
        sizes_match.then_some(value)
    } else {
        None
    };
    write_json(
        data_manifest_path,
        &if let Some(manifest) = existing_data_manifest {
            manifest
        } else {
            data_manifest(context)?
        },
    )?;
    write_json(
        context
            .artifact
            .join("round25-corrected-policy-manifest.json"),
        &serde_json::json!({
            "schema_version":2,
            "corrects_invalid_attempt":"docs/superpowers/artifacts/glm-martingale-core-round25-recovery",
            "frozen_before_return_replay":true,
            "policy_count":policies.len(),
            "policies":&policies,
            "only_family":R25_FAMILY,
            "conditional_enhancement":R25_C2
        }),
    )?;
    write_fingerprints(context, &policies)?;
    write_json(
        context.artifact.join("round25-corrected-trial-ledger.json"),
        &serde_json::json!({
            "schema_version":2,
            "invalid_attempt_00":{
                "commit":"0e64767c",
                "artifact_root":"docs/superpowers/artifacts/glm-martingale-core-round25",
                "class":"MATERIALLY_INCOMPLETE_INVALID_RESULTS",
                "return_replays_run":0,
                "exact_failure_closure":false
            },
            "round25_invalid_replays_counted":16,
            "visible_trial_floor":1096,
            "round25_preregistered_c1_policy_population":16,
            "dsr_sensitivity_trial_counts":[10000,50000,100000,1000000],
            "historical_global_trial_count":"unknown_not_less_than_1096"
        }),
    )?;
    let canaries = r25_canary_report();
    write_json(
        context.artifact.join("gates/r0-r4.json"),
        &serde_json::json!({
            "phase":"R0-R4_AND_G0-S",
            "passed":canaries["passed"],
            "canaries":canaries,
            "copula_reference":copula_reference_report(),
            "command":"cargo test -p r24-engine && cargo test -p r24-research"
        }),
    )?;
    write_json(
        context.artifact.join("gates/r0.json"),
        &serde_json::json!({
            "phase":"R0","passed":true,
            "artifact_root":context.artifact.strip_prefix(&context.repo)?.to_string_lossy(),
            "old_artifact_read_only":[
                "docs/superpowers/artifacts/glm-martingale-core-round25",
                "docs/superpowers/artifacts/glm-martingale-core-round25-recovery"
            ],
            "git_commit":context.commit,"source_tree":context.source_tree,
            "upstream_commit":context.upstream,"dirty":context.dirty,
            "remote_sync_status":context.remote_sync_status,
            "raw_root":context.raw_root.strip_prefix(&context.repo)?.to_string_lossy()
        }),
    )?;
    fs::copy(
        context.repo.join("docs/superpowers/artifacts/glm-martingale-core-round25-recovery/audit/round25-corrected-failure-ledger.jsonl"),
        context
            .artifact
            .join("round25-corrected-failure-ledger.jsonl"),
    )?;
    write_json(
        context
            .artifact
            .join("round25-corrected-execution-state.json"),
        &serde_json::json!({
            "status":"PREFLIGHT_COMPLETE",
            "source_commit":context.commit,
            "source_tree":context.source_tree,
            "dirty":false,
            "remote_sync_status":context.remote_sync_status,
            "phases":{"R0":"complete","R1":"complete","R2":"complete","R3":"complete","R4":"complete","G0":"pending","G1":"pending","G2":"conditional"}
        }),
    )?;
    println!(
        "preflight complete: policies=16 artifact={}",
        context.artifact.display()
    );
    Ok(())
}

fn run_g0(context: &RunContext, resume: bool) -> Result<G0Summary> {
    let phase_started = Instant::now();
    let gate_path = context.artifact.join("gates/g0.json");
    if resume && gate_path.exists() {
        let gate: serde_json::Value = serde_json::from_slice(&fs::read(&gate_path)?)?;
        if gate["passed"].as_bool().unwrap_or(false) {
            return Ok(G0Summary {
                pass: true,
                status: gate["status"].as_str().unwrap_or("PASS").into(),
                real_replay_count: gate["real_replay_count"].as_u64().unwrap_or(0) as usize,
                real_order_or_rejection: true,
                so_guard_called: true,
                parameter_delta: true,
                scheduler_status: gate["scheduler_status"]
                    .as_str()
                    .unwrap_or("unknown")
                    .into(),
            });
        }
    }
    let policies = r25_policies();
    let configs = [policies[0].clone(), policies[15].clone()];
    let windows = [
        ("range", "2023-08-01", "2023-09-30"),
        ("bull", "2024-02-01", "2024-03-31"),
        ("shock", "2024-04-01", "2024-05-31"),
        ("bear", "2025-02-01", "2025-03-31"),
    ];
    let mut evidences = Vec::new();
    let mut scan_rows = Vec::new();
    for (label, start, end) in windows {
        for policy in &configs {
            let evidence = run_one_replay(
                context,
                policy.clone(),
                R25ReplayMode::G0,
                &format!("{label}-{}", policy.policy_id),
                utc_ms(start),
                utc_ms(end),
                None,
            )?;
            write_replay_artifact(context, "g0", &evidence)?;
            scan_rows.push(scan_row(label, &evidence));
            evidences.push(evidence);
        }
    }
    write_json(
        context.artifact.join("gates/g0-real-scan.json"),
        &serde_json::json!({"scan":scan_rows}),
    )?;
    let real_replay_count = evidences.len();
    let fitted = evidences.iter().any(|e| e.fitted_pair_count > 0);
    let real_order_or_rejection = evidences
        .iter()
        .any(|e| e.order_submit_count + e.rejection_count > 0);
    let so_guard_called = evidences.iter().any(|e| e.so_decision_count > 0);
    let btc_zero = evidences
        .iter()
        .all(|e| e.btc_order_count == 0 && e.btc_trade_count == 0);
    let hashes = evidences
        .iter()
        .map(|e| format!("{}:{}:{}", e.policy_id, e.fit_hash, e.decision_hash))
        .collect::<BTreeSet<_>>();
    let parameter_delta = hashes.len() > 1;
    let filter_feasible_path = evidences.iter().any(|evidence| {
        evidence.streams.values().flatten().any(|row| {
            matches!(row["symbol"].as_str(), Some("ETHUSDT" | "LINKUSDT"))
                && matches!(
                    row["event"].as_str(),
                    Some("submit_attempt" | "atomic_filter_reject" | "account_submit_reject")
                )
        })
    });
    let reserve_reconciled = evidences.iter().all(|e| {
        e.final_positions == 0
            && e.final_groups == 0
            && e.final_pending == 0
            && e.final_reserved_quote.abs() <= 1e-9
    });
    let one_minute_risk_complete = evidences.iter().all(|e| {
        e.risk_path_rows == ((e.end_ms - e.start_ms) / 60_000) as u64
            && e.risk_path_first_ms == Some(e.start_ms)
            && e.risk_path_last_ms == Some(e.end_ms - 60_000)
    });
    let frozen_model_persisted = evidences.iter().any(|e| {
        e.streams["event"].iter().any(|row| {
            row["event"] == "fit_roll"
                && row["pairs"].as_array().is_some_and(|pairs| {
                    pairs.iter().any(|pair| {
                        pair["family"].is_string()
                            && pair["rho"].is_number()
                            && pair["fit_cutoff_ms"].is_number()
                    })
                })
        })
    });
    let scheduler_status = "not_testable_in_g0_real";
    let pass = fitted
        && real_order_or_rejection
        && so_guard_called
        && btc_zero
        && parameter_delta
        && filter_feasible_path
        && reserve_reconciled
        && one_minute_risk_complete
        && frozen_model_persisted;
    let status = if pass {
        "PASS_WITH_REAL_SCHEDULER_NOT_TESTABLE"
    } else if !fitted || !real_order_or_rejection {
        "VALID_NO_ACTIVATION"
    } else {
        "IMPLEMENTATION_OR_DATA_BLOCKED"
    };
    let summary = G0Summary {
        pass,
        status: status.into(),
        real_replay_count,
        real_order_or_rejection,
        so_guard_called,
        parameter_delta,
        scheduler_status: scheduler_status.into(),
    };
    let experiment_id = format!("R25R-G0-REAL-{}", &context.commit[..8]);
    append_registry_for_phase(
        context,
        "G0",
        &experiment_id,
        None,
        pass,
        real_replay_count,
        &serde_json::to_value(&scan_rows)?,
        phase_started.elapsed().as_secs_f64(),
    )?;
    write_json(
        gate_path,
        &serde_json::json!({
            "phase":"G0",
            "experiment_id":experiment_id,
            "passed":pass,
            "status":status,
            "real_replay_count":real_replay_count,
            "fitted_pair_seen":fitted,
            "real_order_or_rejection":real_order_or_rejection,
            "so_guard_called":so_guard_called,
            "btc_order_trade_count_zero":btc_zero,
            "parameter_delta":parameter_delta,
            "filter_feasible_eth_or_link_path":filter_feasible_path,
            "reserve_reconciled":reserve_reconciled,
            "one_minute_risk_complete":one_minute_risk_complete,
            "frozen_model_persisted":frozen_model_persisted,
            "scheduler_status":scheduler_status,
            "return_replays_run":real_replay_count
        }),
    )?;
    println!("g0 complete: status={status} real_replay_count={real_replay_count}");
    Ok(summary)
}

fn run_g1(context: &RunContext, resume: bool) -> Result<()> {
    let phase_started = Instant::now();
    let gate_path = context.artifact.join("gates/g1.json");
    if resume && gate_path.exists() {
        let gate: serde_json::Value = serde_json::from_slice(&fs::read(&gate_path)?)?;
        if gate["executed_policies"].as_u64().unwrap_or(0) == 16 {
            println!("g1 already complete; resume skipped");
            return Ok(());
        }
    }
    let policies = r25_policies();
    let mut activation = Vec::new();
    let mut results = Vec::new();
    for policy in &policies {
        let label = format!("activation-{}", policy.policy_id);
        let cached = if resume {
            load_replay_result(context, &label)?
        } else {
            None
        };
        let census = if let Some(cached) = cached {
            cached
        } else {
            run_one_replay(
                context,
                policy.clone(),
                R25ReplayMode::ActivationCensus,
                &label,
                utc_ms("2023-07-01"),
                utc_ms("2026-06-01"),
                None,
            )?
        };
        if !context
            .artifact
            .join("trace-manifests/g1-activation")
            .join(&label)
            .exists()
        {
            write_replay_artifact(context, "g1-activation", &census)?;
        }
        activation.push(evidence_summary(&census));
    }
    for policy in &policies {
        let label = format!("full-{}", policy.policy_id);
        let cached = if resume {
            load_replay_result(context, &label)?
        } else {
            None
        };
        let evidence = if let Some(cached) = cached {
            cached
        } else {
            run_one_replay(
                context,
                policy.clone(),
                R25ReplayMode::Full,
                &label,
                utc_ms("2023-07-01"),
                utc_ms("2026-06-01"),
                None,
            )?
        };
        if !context
            .artifact
            .join("trace-manifests/g1")
            .join(&label)
            .exists()
        {
            write_replay_artifact(context, "g1", &evidence)?;
        }
        results.push(evidence);
    }
    let p_b_survivors = results.iter().filter(|e| e.p_b).count();
    let valid_failures = results
        .iter()
        .filter(|e| !e.p_b && !e.immediate_fail_reasons.is_empty())
        .count();
    let result_rows = results.iter().map(evidence_summary).collect::<Vec<_>>();
    let failure_path = context
        .artifact
        .join("round25-corrected-failure-ledger.jsonl");
    let mut failure_rows = read_jsonl(&failure_path)?;
    failure_rows.retain(|row| row["class"] != "corrected_g1_terminal");
    for evidence in results.iter().filter(|evidence| !evidence.p_b) {
        failure_rows.push(serde_json::json!({
            "failure_id":format!("R25R-{}",evidence.policy_id),
            "class":"corrected_g1_terminal","status":"exact_valid_failure",
            "policy_id":evidence.policy_id,"reasons":evidence.immediate_fail_reasons,
            "order_hash":evidence.order_hash,"decision_hash":evidence.decision_hash,
            "source_commit":context.commit
        }));
    }
    write_jsonl(failure_path, &failure_rows)?;
    write_json(
        context.artifact.join("round25-corrected-trial-ledger.json"),
        &serde_json::json!({
            "schema_version":3,
            "invalid_round25_recovery_trials":16,
            "corrected_policy_population":policies.len(),
            "corrected_terminal_count":results.len(),
            "corrected_pb_survivors":p_b_survivors,
            "global_trial_floor_before_corrected":1096,
            "global_trial_floor_after_corrected":1096 + results.len(),
            "terminals":result_rows
        }),
    )?;
    let g0_gate: serde_json::Value =
        serde_json::from_slice(&fs::read(context.artifact.join("gates/g0.json"))?)?;
    append_registry_for_phase(
        context,
        "G1",
        &format!("R25R-G1-16POLICY-{}", &context.commit[..8]),
        g0_gate["experiment_id"].as_str(),
        true,
        results.len(),
        &serde_json::json!({"policies":result_rows}),
        phase_started.elapsed().as_secs_f64(),
    )?;
    write_json(
        context.artifact.join("gates/g1-activation-census.json"),
        &serde_json::json!({
            "phase":"G1_ACTIVATION_CENSUS",
            "policy_count":activation.len(),
            "policies":activation
        }),
    )?;
    write_json(
        gate_path,
        &serde_json::json!({
            "phase":"G1",
            "passed":true,
            "executed_policies":results.len(),
            "frozen_policy_population":policies.len(),
            "p_b_survivors":p_b_survivors,
            "valid_failures":valid_failures,
            "policies":result_rows
        }),
    )?;
    if p_b_survivors == 0 {
        write_json(
            context.artifact.join("gates/g2.json"),
            &serde_json::json!({
                "phase":"G2","status":"not_applicable_no_corrected_c1_pb_survivor",
                "passed":true,"executed":false
            }),
        )?;
    }
    let final_status = if p_b_survivors == 0 {
        "VALID_HISTORICAL_PREQUENTIAL_NO_TARGET"
    } else {
        "HISTORICAL_PREQUENTIAL_FRONTIER_PROGRESS"
    };
    write_final_authority(context, final_status, p_b_survivors, &results)?;
    println!(
        "g1 complete: policies={} P-B={p_b_survivors}",
        results.len()
    );
    Ok(())
}

fn load_replay_result(context: &RunContext, label: &str) -> Result<Option<R25ReplayEvidence>> {
    let path = context
        .artifact
        .join("replay-results")
        .join(format!("{label}.json"));
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
}

fn run_one_replay(
    context: &RunContext,
    policy: r24_research::r25::R25Policy,
    mode: R25ReplayMode,
    label: &str,
    start_ms: i64,
    end_ms: i64,
    max_steps: Option<usize>,
) -> Result<R25ReplayEvidence> {
    run_r25_c1_replay(&R25ReplayConfig {
        market_db: context.market_db.clone(),
        funding_db: context.funding_db.clone(),
        exchange_info: context.exchange_info.clone(),
        policy,
        start_ms,
        end_ms,
        label: label.into(),
        mode,
        max_steps,
    })
}

fn write_replay_artifact(
    context: &RunContext,
    phase_dir: &str,
    evidence: &R25ReplayEvidence,
) -> Result<()> {
    let raw_root = context.raw_root.join(phase_dir).join(&evidence.label);
    let manifest_root = context
        .artifact
        .join("trace-manifests")
        .join(phase_dir)
        .join(&evidence.label);
    fs::create_dir_all(&raw_root)?;
    fs::create_dir_all(&manifest_root)?;
    let mut manifest = BTreeMap::new();
    for kind in TRACE_KINDS {
        let path = raw_root.join(format!("{kind}.jsonl"));
        let mut writer = BufWriter::new(File::create(&path)?);
        let rows = evidence.streams.get(kind).map(Vec::as_slice).unwrap_or(&[]);
        for row in rows {
            serde_json::to_writer(&mut writer, row)?;
            writer.write_all(b"\n")?;
        }
        writer.flush()?;
        let sample = deterministic_sample(rows, 100);
        write_json(
            manifest_root.join(format!("{kind}-sample.json")),
            &serde_json::json!({
                "schema":"round25-corrected-deterministic-trace-sample-v1",
                "stream":kind,
                "row_count":rows.len(),
                "first":rows.first(),
                "last":rows.last(),
                "sample":sample
            }),
        )?;
        manifest.insert(
            kind.to_string(),
            serde_json::json!({
                "raw_path":path.strip_prefix(&context.repo).unwrap_or(&path).to_string_lossy(),
                "sha256":sha256_file(&path)?,
                "bytes":fs::metadata(&path)?.len(),
                "rows":rows.len(),
                "schema":"jsonl/round25-corrected-trace-v1",
                "sample_path":manifest_root.join(format!("{kind}-sample.json")).strip_prefix(&context.repo)?.to_string_lossy()
            }),
        );
    }
    write_json(
        manifest_root.join("trace-manifest.json"),
        &serde_json::json!({
            "source_commit":context.commit,
            "source_tree":context.source_tree,
            "data_hashes":{
                "manifest_sha256":sha256_file(&context.artifact.join("round25-corrected-data-manifest.json"))?,
                "exchange_info_sha256":sha256_file(&context.exchange_info)?
            },
            "policy_hash":sha256(&serde_json::to_vec(&evidence.policy_id)?),
            "streams":manifest
        }),
    )?;
    let mut compact = evidence.clone();
    compact.streams.clear();
    write_json(
        context
            .artifact
            .join("replay-results")
            .join(format!("{}.json", evidence.label)),
        &serde_json::to_value(compact)?,
    )?;
    Ok(())
}

fn deterministic_sample(rows: &[serde_json::Value], target: usize) -> Vec<serde_json::Value> {
    if rows.len() <= target {
        return rows.to_vec();
    }
    (0..target)
        .map(|index| {
            let row_index = index * (rows.len() - 1) / (target - 1);
            rows[row_index].clone()
        })
        .collect()
}

fn ensure_g0_passed(context: &RunContext) -> Result<()> {
    let gate: serde_json::Value =
        serde_json::from_slice(&fs::read(context.artifact.join("gates/g0.json"))?)?;
    if gate["passed"] != true {
        bail!("G1 requires G0 PASS/PASS_WITH_REAL_SCHEDULER_NOT_TESTABLE");
    }
    Ok(())
}

fn data_manifest(context: &RunContext) -> Result<serde_json::Value> {
    let gates = r25_symbol_gates(
        &context.market_db,
        &context.funding_db,
        &context.exchange_info,
    )?;
    let eligible_alts = gates
        .iter()
        .filter(|g| g.eligible_for_trade)
        .map(|g| g.symbol.clone())
        .collect::<Vec<_>>();
    let g0_data_gate = eligible_alts.len() >= 2;
    let g1_data_gate = eligible_alts.len() >= 6;
    Ok(serde_json::json!({
        "schema_version":2,
        "symbol_gates":gates,
        "eligible_alts":eligible_alts,
        "reference_symbol":R25_REFERENCE_SYMBOL,
        "g0_data_gate":g0_data_gate,
        "g1_data_gate":g1_data_gate,
        "ltc_rule":"LTCUSDT has no local funding rows and is ineligible_for_trade unless funding is补齐",
        "market_database":{"path":context.market_db.strip_prefix(&context.repo)?.to_string_lossy(),"bytes":fs::metadata(&context.market_db)?.len(),"sha256":sha256_file(&context.market_db)?},
        "funding_database":{"path":context.funding_db.strip_prefix(&context.repo)?.to_string_lossy(),"bytes":fs::metadata(&context.funding_db)?.len(),"sha256":sha256_file(&context.funding_db)?},
        "filter_snapshot":{"path":context.exchange_info.strip_prefix(&context.repo)?.to_string_lossy(),"sha256":sha256_file(&context.exchange_info)?}
    }))
}

fn protocol_json() -> serde_json::Value {
    serde_json::json!({
        "schema_version":2,
        "historical_backtest_only":true,
        "family":R25_FAMILY,
        "conditional_enhancement":R25_C2,
        "reference_symbol":R25_REFERENCE_SYMBOL,
        "tradable_alts":R25_TRADABLE_ALTS,
        "outer_blocks":R25_BLOCKS,
        "cold_starts":R25_COLD_STARTS,
        "completed_bar_contract":"signal_close < signal_ready < order_time <= fill_time",
        "shared_account":true,
        "calibration_principal_u":2000,
        "group_fo_target_gross_u":50,
        "target_layer_gross_u":[50.0,62.5,77.5,95.0],
        "max_active_groups":3,
        "leverage_cap":2.0,
        "copula_fit":"timestamp-aligned Kendall tau; Gaussian versus Student-t nu=3..30 by train AIC",
        "risk_path":"every 1m high/low adverse path then close mark",
        "principal_contract":[500,750,1000,1500,2000,3000,4000,4999],
        "targets":{"conservative":"50/10","balanced":"90/20","aggressive":"100/30"}
    })
}

fn source_map(context: &RunContext) -> Result<serde_json::Value> {
    let update: serde_json::Value = serde_json::from_slice(&fs::read(&context.source_update)?)?;
    Ok(serde_json::json!({
        "schema_version":2,
        "primary":update["primary_source"],
        "secondary_sources":update["secondary_sources"],
        "local_mapping":"C1 uses BTC reference spreads and conditional copula probabilities only as Martin FO/SO/TP/abort controls; BTC is never traded."
    }))
}

fn write_fingerprints(
    context: &RunContext,
    policies: &[r24_research::r25::R25Policy],
) -> Result<()> {
    let mut writer = BufWriter::new(File::create(
        context
            .artifact
            .join("round25-corrected-mechanism-fingerprints.jsonl"),
    )?);
    for policy in policies {
        serde_json::to_writer(
            &mut writer,
            &serde_json::json!({
                "policy_id":policy.policy_id,
                "family":policy.family,
                "mechanism_fingerprint":policy.mechanism_fingerprint,
                "status":"frozen_for_corrected_replay"
            }),
        )?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn append_registry_for_phase(
    context: &RunContext,
    phase: &str,
    experiment_id: &str,
    parent: Option<&str>,
    complete: bool,
    replay_count: usize,
    payload: &serde_json::Value,
    wall_seconds: f64,
) -> Result<()> {
    let path = context.artifact.join("exploration-registry.jsonl");
    let mut rows = if path.exists() {
        read_jsonl(&path)?
    } else {
        Vec::new()
    };
    if rows
        .iter()
        .any(|row| row["experiment_id"] == experiment_id && row["row_kind"] == "terminal")
    {
        return Ok(());
    }
    let start = utc_now();
    let base = serde_json::json!({
        "experiment_id":experiment_id,
        "parent_experiment_id":parent,
        "phase":phase,
        "mechanism_fingerprint":sha256(format!("r25-corrected-{phase}-{experiment_id}").as_bytes()),
        "git_commit":context.commit,
        "git_dirty":context.dirty,
        "upstream_commit":context.upstream,
        "raw_argv":std::env::args().collect::<Vec<_>>(),
        "pid":std::process::id(),
        "start_utc":start,
        "artifact_sha256":{
            "r25_prod":sha256_file(&context.repo.join("crates/r24-research/src/r25_prod.rs"))?,
            "r25_execute":sha256_file(&context.repo.join("crates/r24-research/src/bin/r25_execute.rs"))?
        },
        "replay_count":replay_count,
        "payload_hash":sha256(&serde_json::to_vec(payload)?),
        "signal_bar_open":"trace_bound",
        "signal_bar_close":"trace_bound",
        "signal_ready":"trace_bound",
        "earliest_fill":"trace_bound",
        "actual_fill":"trace_bound_or_rejection",
        "family_count":1,
        "family_gate_applicable":false
    });
    let mut running = base.clone();
    running["row_kind"] = serde_json::json!("running");
    running["terminal_status"] = serde_json::Value::Null;
    running["exit_code"] = serde_json::Value::Null;
    rows.push(running);
    let mut terminal = base;
    terminal["row_kind"] = serde_json::json!("terminal");
    terminal["terminal_status"] = serde_json::json!(if complete { "complete" } else { "failed" });
    terminal["exit_code"] = serde_json::json!(if complete { 0 } else { 1 });
    terminal["end_utc"] = serde_json::json!(utc_now());
    terminal["wall_seconds"] = serde_json::json!(wall_seconds);
    terminal["peak_rss_kb"] = serde_json::json!(peak_rss_kb());
    rows.push(terminal);
    write_jsonl(path, &rows)?;
    Ok(())
}

fn write_final_authority(
    context: &RunContext,
    status: &str,
    p_b_survivors: usize,
    results: &[R25ReplayEvidence],
) -> Result<()> {
    let phase_status = serde_json::json!({
        "R0":"complete",
        "R1":"complete_preflight_and_g0_s",
        "R2":"complete_symbol_data_gate",
        "R3":"complete_policy_trial_fingerprint_freeze",
        "G0":"complete_pass",
        "G1":"complete",
        "C2":if p_b_survivors > 0 {"pending_parent_survivor"} else {"not_applicable_no_c1_pb_parent"},
        "G2":if p_b_survivors > 0 {"pending_parent_survivor"} else {"not_applicable_no_committed_g1_survivor"},
        "R8":if p_b_survivors > 0 {"pending_parent_survivor"} else {"complete_no_target"}
    });
    let state = serde_json::json!({
        "status":"PENDING_INDEPENDENT_VALIDATION",
        "provisional_replay_status":status,
        "historical_backtest_only":true,
        "return_replays_run":results.len(),
        "audited_commit":context.commit,
        "audited_upstream":context.upstream,
        "phase_status":phase_status,
        "p_b_survivors":p_b_survivors,
        "target_hit":false,
        "frontier_progress":false
    });
    write_json(
        context
            .artifact
            .join("round25-corrected-execution-state.json"),
        &state,
    )?;
    write_json(
        context.artifact.join("round25-corrected-authority.json"),
        &serde_json::json!({
            "authority_version":2,
            "status":"PENDING_INDEPENDENT_VALIDATION",
            "provisional_replay_status":status,
            "historical_backtest_only":true,
            "family":R25_FAMILY,
            "target_hit":false,
            "frontier_progress":false,
            "return_replays_run":results.len(),
            "p_b_survivors":p_b_survivors,
            "strict_valid_candidates":[],
            "phase_status":phase_status,
            "not_claimed":"all Martingale possibilities exhausted"
        }),
    )?;
    fs::write(
        context
            .repo
            .join("docs/superpowers/reports/2026-07-23-glm-round25-corrected-handoff.md"),
        render_report(context, status, p_b_survivors, results),
    )?;
    Ok(())
}

fn scan_row(label: &str, evidence: &R25ReplayEvidence) -> serde_json::Value {
    serde_json::json!({
        "label":label,
        "policy_id":evidence.policy_id,
        "fitted_pair_count":evidence.fitted_pair_count,
        "order_submit_count":evidence.order_submit_count,
        "rejection_count":evidence.rejection_count,
        "so_decision_count":evidence.so_decision_count,
        "btc_order_count":evidence.btc_order_count,
        "order_hash":evidence.order_hash,
        "rejection_hash":evidence.rejection_hash,
        "decision_hash":evidence.decision_hash,
        "fit_hash":evidence.fit_hash
    })
}

fn evidence_summary(e: &R25ReplayEvidence) -> serde_json::Value {
    serde_json::json!({
        "policy_id":e.policy_id,
        "mode":e.mode,
        "label":e.label,
        "eligible_alts":e.eligible_alts,
        "fitted_pair_count":e.fitted_pair_count,
        "distinct_fitted_pairs":e.distinct_fitted_pairs,
        "fo_signal_count":e.fo_signal_count,
        "order_submit_count":e.order_submit_count,
        "rejection_count":e.rejection_count,
        "fill_count":e.fill_count,
        "so_decision_count":e.so_decision_count,
        "so_fill_count":e.so_fill_count,
        "actual_assets":e.actual_assets,
        "distinct_traded_pairs":e.distinct_traded_pairs,
        "btc_order_count":e.btc_order_count,
        "btc_trade_count":e.btc_trade_count,
        "final_equity":e.final_equity,
        "annualized_return_pct":e.annualized_return_pct,
        "max_equity_drawdown_pct":e.max_equity_drawdown_pct,
        "positive_blocks":e.positive_blocks,
        "risk_path_rows":e.risk_path_rows,
        "final_reserved_quote":e.final_reserved_quote,
        "metrics":e.metrics,
        "p_a":e.p_a,
        "p_b":e.p_b,
        "immediate_fail_reasons":e.immediate_fail_reasons,
        "order_hash":e.order_hash,
        "rejection_hash":e.rejection_hash,
        "decision_hash":e.decision_hash,
        "so_guard_call_path_hash":e.so_guard_call_path_hash
    })
}

fn render_report(
    context: &RunContext,
    status: &str,
    p_b_survivors: usize,
    results: &[R25ReplayEvidence],
) -> String {
    format!(
        "# GLM Round25 Corrected Handoff\n\nProvisional replay status: `{status}`; final authority requires `r25_corrected_validate`.\n\n- Source commit: `{}`\n- Artifact root: `{}`\n- Local raw root: `{}`\n- G1 policies executed: `{}`\n- P-B survivors: `{p_b_survivors}`\n- Historical only: `true`\n- Remote sync: `{}`\n\nG0 and G1 use the same corrected production replay. Raw traces remain local; committed evidence contains hashes, schemas, first/last rows, deterministic samples, and compact metrics.\n",
        context.commit,
        context.artifact.display(),
        context.raw_root.display(),
        results.len(),
        context.remote_sync_status
    )
}

fn read_jsonl(path: &Path) -> Result<Vec<serde_json::Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    BufReader::new(File::open(path)?)
        .lines()
        .filter_map(|line| line.ok())
        .filter(|line| !line.trim().is_empty())
        .map(|line| Ok(serde_json::from_str(&line)?))
        .collect()
}

fn write_json(path: impl AsRef<Path>, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn write_jsonl(path: impl AsRef<Path>, rows: &[serde_json::Value]) -> Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    let mut writer = BufWriter::new(File::create(path)?);
    for row in rows {
        serde_json::to_writer(&mut writer, row)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn utc_now() -> String {
    DateTime::<Utc>::from(std::time::SystemTime::now()).to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn peak_rss_kb() -> u64 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                line.strip_prefix("VmHWM:")?
                    .split_whitespace()
                    .next()?
                    .parse()
                    .ok()
            })
        })
        .unwrap_or(0)
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !output.status.success() {
        bail!(
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim().into())
}

fn git_optional(repo: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}
