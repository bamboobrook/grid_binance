use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use r24_registry::{
    run_all_canaries, sha256, sha256_file, validate_registry, Evidence, LaunchSpec, Launcher,
    Registry, TRACE_KINDS,
};

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("trace-child") {
        return trace_child(Path::new(args.get(2).context("missing trace dir")?));
    }
    let output = PathBuf::from(args.get(1).context("usage: r24_bootstrap <staging-dir>")?);
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let plan = repo.join("docs/superpowers/plans/2026-07-23-glm-martingale-core-round24-causal-ensemble-recovery-plan.md");
    let authority = repo.join(
        "docs/superpowers/artifacts/glm-martingale-core-round23/round23-corrected-authority.json",
    );
    let audit = repo.join("docs/superpowers/artifacts/glm-martingale-core-round23/audit/round23-independent-audit.json");
    let commit = git(&repo, &["rev-parse", "HEAD"])?;
    let upstream = git(&repo, &["rev-parse", "@{u}"])?;
    let dirty = !git(&repo, &["status", "--porcelain"])?.is_empty();
    if dirty || commit != upstream {
        bail!("R0 requires clean pushed commit");
    }
    if output.exists() {
        fs::remove_dir_all(&output)?;
    }
    fs::create_dir_all(output.join("traces/r0-bootstrap"))?;
    fs::create_dir_all(output.join("gates"))?;

    let canaries = run_all_canaries();
    if canaries.len() != 32 || canaries.values().any(|passed| !passed) {
        bail!("R0 canary failure: {canaries:?}");
    }
    let registry = Registry::new(
        output.join("exploration-registry.jsonl"),
        output.join("failure-ledger.jsonl"),
    );
    let launcher = Launcher::new(registry.clone());
    let executable = std::env::current_exe()?;
    let trace_dir = output.join("traces/r0-bootstrap");
    let hashes = BTreeMap::from([
        ("source".into(), sha256_file(&plan)?),
        ("round23_authority".into(), sha256_file(&authority)?),
        ("round23_audit".into(), sha256_file(&audit)?),
        ("binary".into(), sha256_file(&executable)?),
    ]);
    let mechanism = sha256(b"r24-r0-real-launcher-registry-validator-v1");
    launcher.run(
        LaunchSpec {
            experiment_id: "R24-R0-BOOTSTRAP-001".into(),
            parent_experiment_id: None,
            phase: "R0".into(),
            mechanism_fingerprint: mechanism,
            git_commit: commit.clone(),
            git_dirty: dirty,
            upstream_commit: upstream.clone(),
            artifact_sha256: hashes,
            fit_cutoff_utc: "2023-06-30T23:59:59Z".into(),
            purge_bars: 1,
            signal_ready_utc: "2023-07-01T00:00:00Z".into(),
            replay_utc: "2023-07-01T00:00:00Z".into(),
            evidence: Evidence::r0(),
        },
        &executable,
        &[
            "trace-child".into(),
            trace_dir.to_string_lossy().into_owned(),
        ],
        &trace_dir,
    )?;
    let summary = validate_registry(&registry.rows()?, &registry.failures()?, Some(1));
    if !summary.valid {
        bail!("R0 final validation failed: {:?}", summary.violations);
    }
    let gate = serde_json::json!({
        "phase":"R0", "passed":true, "validator":"r24_registry::validate_registry",
        "canary_count":canaries.len(), "canaries":canaries, "registry":summary,
        "git_commit":commit, "upstream_commit":upstream, "git_dirty":dirty,
        "search_allowed":true
    });
    fs::write(
        output.join("gates/r0.json"),
        serde_json::to_vec_pretty(&gate)?,
    )?;
    fs::write(
        output.join("round24-execution-state.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "status":"BLOCKED_ENGINE_DATA_OR_EXECUTION", "machine_derived":true,
            "phases":{"R0":"complete","R1":"pending","R2":"pending","R3":"pending","F1":"pending","F3":"pending","G0":"pending","G1":"pending","E1":"pending","F2":"pending","G2":"pending","R8":"pending"},
            "registry":summary
        }))?,
    )?;
    println!("R0 complete: 32/32 canaries rejected, one real running+terminal pair validated");
    Ok(())
}

fn trace_child(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    let allocation = vec![7_u8; 2 * 1024 * 1024];
    for kind in TRACE_KINDS {
        let row = serde_json::json!({"trace_kind":kind,"event":"r0_probe","sequence":1});
        fs::write(
            dir.join(format!("{kind}.jsonl")),
            format!("{}\n", serde_json::to_string(&row)?),
        )?;
    }
    thread::sleep(Duration::from_millis(40));
    std::hint::black_box(allocation);
    Ok(())
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !output.status.success() {
        bail!("git {:?} failed", args);
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
