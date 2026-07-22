//! Round 23 R0 bootstrap binary.
//!
//! Run: `cargo run -p r23-registry --bin r23_bootstrap -- <artifact_dir>`
//!
//! It performs the R0 contract:
//! 1. Runs the 15 injected canaries (§4.3) and asserts every one is rejected.
//! 2. Hashes the live plan document and the live R22 corrected authority.
//! 3. Captures the current git commit / dirty state / upstream remote commit.
//! 4. Writes the canonical 12-block / 5-cold-start manifest.
//! 5. Derives the canonical execution state. While R0 is the only completed
//!    phase the state is BLOCKED_ENGINE_DATA_OR_EXECUTION and search is forbidden.
//! 6. Writes `round23-execution-state.json`, `round23-authority.json`,
//!    `r0/manifest.json`, and initial (empty) `exploration-registry.jsonl` /
//!    `failure-ledger.jsonl` that the launcher will append to.
//!
//! Exit code 0 = R0 green and bootstrap complete. Non-zero = R0 failed; do not
//! proceed to any search.

use std::path::PathBuf;
use std::process::Command;

use r23_registry::canary::{run_all_canaries, CanaryOutcome, CANARIES};
use r23_registry::manifest::{canonical_manifest, require_valid};
use r23_registry::state_machine::{derive_execution_state, DeriveInput, ExecutionState};
use r23_registry::{FailureLedger, Registry};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: r23_bootstrap <round23_artifact_dir>");
        std::process::exit(2);
    }
    let artifact_dir = PathBuf::from(&args[1]);
    let repo_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap())
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let plan_path = repo_root
        .join("docs/superpowers/plans/2026-07-22-glm-martingale-core-round23-orderflow-factor-repair-plan.md");
    let r22_authority = repo_root
        .join("docs/superpowers/artifacts/glm-martingale-core-round22/round22-corrected-authority.json");

    // 1. Canaries.
    let outcomes = run_all_canaries();
    let mut canary_results = Vec::new();
    let mut all_green = true;
    for c in CANARIES {
        let o = outcomes.get(c.id).cloned().unwrap_or(CanaryOutcome::HarnessError("missing".into()));
        if !matches!(o, CanaryOutcome::RejectedCorrectly) {
            all_green = false;
        }
        canary_results.push(serde_json::json!({
            "id": c.id,
            "description": c.description,
            "outcome": format!("{:?}", o),
            "never_repeat": c.never_repeat,
        }));
    }
    println!("R0 canaries: {} total, all rejected = {}", canary_results.len(), all_green);
    if !all_green {
        eprintln!("R0 FAILED: one or more canaries were not rejected correctly");
        for r in &canary_results {
            if r["outcome"].as_str() != Some("RejectedCorrectly") {
                eprintln!("  BAD: {}", r["id"]);
            }
        }
        std::process::exit(1);
    }

    // 2. Hash plan + authority.
    let plan_bytes = std::fs::read(&plan_path).expect("read plan");
    let plan_sha = r23_registry::hash::sha256_hex(&plan_bytes);
    let authority_bytes = std::fs::read(&r22_authority).expect("read r22 authority");
    let authority_sha = r23_registry::hash::sha256_hex(&authority_bytes);

    // 3. Git state.
    let git_commit = git_output(&repo_root, &["rev-parse", "HEAD"]);
    let git_dirty = !git_output(&repo_root, &["status", "--porcelain"]).trim().is_empty();
    let upstream = git_output(&repo_root, &["rev-parse", "@{u}"]);
    let on_upstream = git_commit == upstream;

    println!("git commit: {git_commit}");
    println!("git dirty : {git_dirty}");
    println!("upstream  : {upstream} (matches HEAD: {on_upstream})");

    // 4. Manifest.
    let manifest = canonical_manifest(&plan_sha);
    require_valid(&manifest).expect("canonical manifest must be valid");

    // 5. Registry / ledger files (initially empty).
    std::fs::create_dir_all(&artifact_dir).unwrap();
    std::fs::create_dir_all(artifact_dir.join("r0")).unwrap();
    std::fs::create_dir_all(artifact_dir.join("gates")).unwrap();
    let registry_path = artifact_dir.join("exploration-registry.jsonl");
    let ledger_path = artifact_dir.join("failure-ledger.jsonl");
    if !registry_path.exists() {
        std::fs::write(&registry_path, b"").unwrap();
    }
    if !ledger_path.exists() {
        std::fs::write(&ledger_path, b"").unwrap();
    }
    let registry = Registry::new(&registry_path);
    let ledger = FailureLedger::new(&ledger_path);

    // 6. Execution state. Detect completed phases from registry terminal rows.
    let mut completed_phases = vec!["R0".to_string()];
    let reg_rows = registry.read_all().unwrap_or_default();
    use r23_registry::registry::{RowKind, TerminalStatus};
    let mut phases_with_complete: std::collections::HashSet<String> = std::collections::HashSet::new();
    for r in &reg_rows {
        if matches!(r.row_kind, RowKind::Terminal)
            && matches!(r.terminal_status, Some(TerminalStatus::Complete))
        {
            phases_with_complete.insert(r.phase.clone());
        }
    }
    for p in &phases_with_complete {
        if !completed_phases.iter().any(|c| c.eq_ignore_ascii_case(p)) {
            completed_phases.push(p.clone());
        }
    }
    let input = DeriveInput { registry: &registry, completed_phases };
    let state = derive_execution_state(&input);
    println!("execution state: {}", state.as_str());
    println!("completed phases (registry-detected): {:?}", input.completed_phases);

    // 7. Write artifacts.
    let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
    std::fs::write(artifact_dir.join("r0/manifest.json"), manifest_json).unwrap();

    let exec_state = serde_json::json!({
        "status": state.as_str(),
        "validated_at_utc": now_utc(),
        "plan_sha256": plan_sha,
        "r22_corrected_authority_sha256": authority_sha,
        "git_commit": git_commit,
        "git_dirty": git_dirty,
        "upstream_remote_commit": upstream,
        "commit_on_upstream": on_upstream,
        "registry_rows": registry.read_all().map(|r| r.len()).unwrap_or(0),
        "failure_ledger_rows": ledger.read_all().map(|r| r.len()).unwrap_or(0),
        "r0_canaries_all_rejected": all_green,
        "r0_canary_count": canary_results.len(),
        "canaries": canary_results,
        "blocked_reasons": blocked_reasons(&state, git_dirty, on_upstream),
        "phases_completed": input.completed_phases.clone(),
        "phases_remaining": compute_remaining_phases(&input.completed_phases),
    });
    std::fs::write(
        artifact_dir.join("round23-execution-state.json"),
        serde_json::to_string_pretty(&exec_state).unwrap(),
    )
    .unwrap();

    let authority = serde_json::json!({
        "authority_version": 1,
        "generated_at_utc": now_utc(),
        "historical_backtest_only": true,
        "plan_sha256": plan_sha,
        "plan_path": plan_path.strip_prefix(&repo_root).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        "r22_corrected_authority_sha256": authority_sha,
        "r22_corrected_authority_path": r22_authority.strip_prefix(&repo_root).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        "git_commit": git_commit,
        "git_dirty": git_dirty,
        "upstream_remote_commit": upstream,
        "machine_state": artifact_dir.join("round23-execution-state.json").to_string_lossy().into_owned(),
        "target_hit": false,
        "tiers": {"conservative": false, "balanced": false, "aggressive": false},
        "r0_canaries_all_rejected": all_green,
        "r0_canary_count": canary_results.len(),
        "status": state.as_str(),
        "never_repeat": [
            "do not promote any G1/G2/R8 result without a complete registry running+terminal row pair",
            "every replay parent commit must be clean and on the upstream remote",
            "all manifest hashes must be full 64-char lowercase-hex SHA-256",
            "an experiment_id is immutable and may not be reused",
            "every experiment_id needs exactly one running and one terminal row",
            "null/empty order/signal/margin traces are rejected",
            "config budget and argv budget must agree",
            "a G2 launch must reference a committed G1 terminal row",
            "only >=365 stitched test days may judge the annualized target",
            "a manifest with a deleted block is invalid",
            "the five cold starts are frozen before any return replay",
            "summary replay counts must equal registry terminal rows",
            "a selector in the planned state cannot seed any FO/SO",
            "metrics/depth more than one snapshot stale or future are rejected",
            "a non-complete terminal row must atomically write a failure ledger row",
        ],
    });
    std::fs::write(
        artifact_dir.join("round23-authority.json"),
        serde_json::to_string_pretty(&authority).unwrap(),
    )
    .unwrap();

    // R0 gate summary.
    let gate = serde_json::json!({
        "phase": "R0",
        "gate": "R0_registry_contract_valid",
        "passed": all_green && !git_dirty && on_upstream,
        "checks": {
            "canaries_all_rejected": all_green,
            "git_tree_clean": !git_dirty,
            "commit_on_upstream": on_upstream,
            "registry_writable": true,
            "ledger_writable": true,
            "manifest_valid": true,
        },
        "validated_at_utc": now_utc(),
    });
    std::fs::write(
        artifact_dir.join("gates/r0.json"),
        serde_json::to_string_pretty(&gate).unwrap(),
    )
    .unwrap();

    println!("R0 bootstrap complete. State = {}", state.as_str());
    if matches!(state, ExecutionState::BlockedEngineDataOrExecution) {
        println!("NOTE: search is forbidden until R1+ (production-conservative scored replay) is wired.");
    }
}

fn git_output(repo: &std::path::Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}

fn compute_remaining_phases(completed: &[String]) -> Vec<String> {
    let all = ["R1", "R2", "R3", "R4", "R5", "G0", "G1", "G2", "R8", "HANDOFF"];
    all.iter()
        .filter(|p| !completed.iter().any(|c| c.eq_ignore_ascii_case(p)))
        .map(|s| s.to_string())
        .collect()
}

fn now_utc() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn blocked_reasons(state: &ExecutionState, git_dirty: bool, on_upstream: bool) -> Vec<String> {
    let mut reasons = Vec::new();
    if !matches!(state, ExecutionState::HistoricalPrequentialTargetHit) {
        reasons.push("R1 production-conservative scored replay not yet wired".to_string());
        reasons.push("R3 metrics/bookDepth/aggTrades historical data not yet ingested".to_string());
        reasons.push("R4 M1/M2/M3 Martin families not yet implemented".to_string());
        reasons.push("G1 12-block continuous replays not yet run".to_string());
    }
    if git_dirty {
        reasons.push("git tree is dirty".to_string());
    }
    if !on_upstream {
        reasons.push("HEAD commit not on upstream remote".to_string());
    }
    reasons
}
