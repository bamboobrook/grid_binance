use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use r24_registry::{
    sha256_file, validate_registry, Evidence, LaunchSpec, Launcher, Registry, TerminalStatus,
    TRACE_KINDS,
};
use r24_research::replay::{load_funding, run_f1_policy, F1Parameters, ReplayResult};
use r24_research::residual::load_hourly_perp;

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("child") {
        return child(&args);
    }
    let staging = PathBuf::from(args.get(1).context("usage: r24_g1 <staging-dir>")?);
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let commit = git(&repo, &["rev-parse", "HEAD"])?;
    let upstream = git(&repo, &["rev-parse", "@{u}"])?;
    let dirty = !git(&repo, &["status", "--porcelain"])?.is_empty();
    if dirty || commit != upstream {
        bail!("G1 requires clean pushed parent");
    }
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(staging.join("traces/g1"))?;
    fs::create_dir_all(staging.join("gates"))?;
    let authority = repo.join("docs/superpowers/artifacts/glm-martingale-core-round24");
    fs::copy(
        authority.join("exploration-registry.jsonl"),
        staging.join("exploration-registry.jsonl"),
    )?;
    fs::copy(
        authority.join("failure-ledger.jsonl"),
        staging.join("failure-ledger.jsonl"),
    )?;
    let registry = Registry::new(
        staging.join("exploration-registry.jsonl"),
        staging.join("failure-ledger.jsonl"),
    );
    let launcher = Launcher::new(registry.clone());
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(authority.join("round24-policy-manifest.json"))?)?;
    let policies = manifest["policies"]
        .as_array()
        .context("policy array missing")?
        .iter()
        .filter(|policy| policy["family"] == "F1_CAUSAL_RESIDUAL_DISJOINT_PAIR")
        .collect::<Vec<_>>();
    if policies.len() != 16 {
        bail!("expected 16 F1 policies, got {}", policies.len());
    }
    let executable = std::env::current_exe()?;
    let common_hashes = BTreeMap::from([
        ("binary".into(), sha256_file(&executable)?),
        (
            "protocol".into(),
            sha256_file(&authority.join("round24-protocol.json"))?,
        ),
        (
            "policy_manifest".into(),
            sha256_file(&authority.join("round24-policy-manifest.json"))?,
        ),
        (
            "data_manifest".into(),
            sha256_file(&authority.join("round24-data-manifest.json"))?,
        ),
        (
            "engine_source".into(),
            sha256_file(&repo.join("crates/r24-engine/src/lib.rs"))?,
        ),
        (
            "replay_source".into(),
            sha256_file(&repo.join("crates/r24-research/src/replay.rs"))?,
        ),
    ]);
    let mut results = Vec::new();
    let mut complete = 0;
    let mut failed = 0;
    for policy in policies {
        let policy_id = policy["policy_id"].as_str().unwrap();
        let parameters = &policy["parameters"];
        let trace_dir = staging.join("traces/g1").join(policy_id);
        fs::create_dir_all(&trace_dir)?;
        let child_args = vec![
            "child".into(),
            trace_dir.to_string_lossy().into_owned(),
            repo.join("data/market_data_full.db")
                .to_string_lossy()
                .into_owned(),
            repo.join("data/funding_rates_round12.db")
                .to_string_lossy()
                .into_owned(),
            policy_id.into(),
            parameters["fit_lookback_days"].to_string(),
            parameters["entry_z"].to_string(),
            parameters["so_residual_sigma"].to_string(),
            parameters["max_live_groups"].to_string(),
        ];
        let terminal = launcher.run(
            LaunchSpec {
                experiment_id: format!("R24-G1-{policy_id}"),
                parent_experiment_id: Some("R24-G0-F1-001".into()),
                phase: "G1".into(),
                mechanism_fingerprint: policy["mechanism_fingerprint"].as_str().unwrap().into(),
                git_commit: commit.clone(),
                git_dirty: dirty,
                upstream_commit: upstream.clone(),
                artifact_sha256: common_hashes.clone(),
                fit_cutoff_utc: "per_block_protocol_manifest".into(),
                purge_bars: 121 * 24,
                signal_ready_utc: "completed_hour_t_minus_1".into(),
                replay_utc: "2023-07-01_to_2026-05-31".into(),
                evidence: Evidence {
                    loaded_assets: 8,
                    pbo_policy_population: 16,
                    pbo_selected_population: 16,
                    ..Evidence::r0()
                },
            },
            &executable,
            &child_args,
            &trace_dir,
        )?;
        match terminal.terminal_status {
            Some(TerminalStatus::Complete) => complete += 1,
            _ => failed += 1,
        }
        let result: serde_json::Value =
            serde_json::from_slice(&fs::read(trace_dir.join("result.json"))?)?;
        results.push(result);
    }
    let summary = validate_registry(&registry.rows()?, &registry.failures()?, Some(19));
    if !summary.valid {
        bail!("G1 registry invalid: {:?}", summary.violations);
    }
    let p_b_survivors = results
        .iter()
        .filter(|result| result["p_b"] == true)
        .count();
    let gate = serde_json::json!({
        "phase":"G1","quota_complete":results.len()==16,"executed_policies":results.len(),
        "terminal_complete":complete,"terminal_failed_gate":failed,"p_b_survivors":p_b_survivors,
        "registry":summary,"git_commit":commit,"upstream_commit":upstream,"git_dirty":dirty,
        "policies":results,"F3_M1":"blocked_incomplete_data","F3_M2":"blocked_incomplete_data"
    });
    fs::write(
        staging.join("gates/g1.json"),
        serde_json::to_vec_pretty(&gate)?,
    )?;
    println!(
        "G1 complete: executed=16 complete={} failed_gate={} P-B={}",
        complete, failed, p_b_survivors
    );
    Ok(())
}

fn child(args: &[String]) -> Result<()> {
    let trace_dir = PathBuf::from(args.get(2).context("missing trace dir")?);
    let market = PathBuf::from(args.get(3).context("missing market db")?);
    let funding_db = PathBuf::from(args.get(4).context("missing funding db")?);
    let policy_id = args.get(5).context("missing policy id")?;
    let parameters = F1Parameters {
        fit_lookback_days: args.get(6).unwrap().parse()?,
        entry_z: args.get(7).unwrap().parse()?,
        so_residual_sigma: args.get(8).unwrap().parse()?,
        max_live_groups: args.get(9).unwrap().parse()?,
    };
    let start = date_ms("2023-01-01");
    let end = date_ms("2026-06-01");
    let data = load_hourly_perp(&market, start, end)?;
    let funding = load_funding(&funding_db, start, end)?;
    let result = run_f1_policy(policy_id, parameters, &data, &funding)?;
    fs::create_dir_all(&trace_dir)?;
    fs::write(
        trace_dir.join("result.json"),
        serde_json::to_vec_pretty(&result_without_traces(&result))?,
    )?;
    let evidence = Evidence {
        actual_filled_assets: result.actual_assets.len() as u32,
        loaded_assets: 8,
        pbo_policy_population: 16,
        pbo_selected_population: 16,
        open_perp_notional: 0.0,
        funding_cashflow_abs: result
            .trace_rows
            .iter()
            .filter(|row| row.stream == "funding")
            .filter_map(|row| row.quote)
            .map(f64::abs)
            .sum(),
        ..Evidence::r0()
    };
    fs::write(
        trace_dir.join("evidence.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    for kind in TRACE_KINDS {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(trace_dir.join(format!("{kind}.jsonl")))?;
        let rows = result
            .trace_rows
            .iter()
            .filter(|row| row.stream == kind)
            .collect::<Vec<_>>();
        if rows.is_empty() {
            serde_json::to_writer(
                &mut file,
                &serde_json::json!({"stream":kind,"event":"empty_activity_preserved","policy_id":policy_id}),
            )?;
            file.write_all(b"\n")?;
        } else {
            for row in rows {
                serde_json::to_writer(&mut file, row)?;
                file.write_all(b"\n")?;
            }
        }
    }
    if !result.immediate_fail_reasons.is_empty() || !result.p_a {
        bail!("G1 immediate fail: {:?}", result.immediate_fail_reasons);
    }
    Ok(())
}

fn result_without_traces(result: &ReplayResult) -> serde_json::Value {
    let mut value = serde_json::to_value(result).unwrap();
    value.as_object_mut().unwrap().remove("trace_rows");
    value
}

fn date_ms(value: &str) -> i64 {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !output.status.success() {
        bail!("git failed");
    }
    Ok(String::from_utf8(output.stdout)?.trim().into())
}
