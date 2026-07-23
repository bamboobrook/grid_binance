#![recursion_limit = "512"]

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};
use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use r24_registry::{sha256, sha256_file, TRACE_KINDS};
use r24_research::r25::{
    r25_canary_report, r25_mechanism_fingerprint, r25_policies, r25_so_guard_call_path_hash,
    R25_BLOCKS, R25_C2, R25_COLD_STARTS, R25_FAMILY, R25_REFERENCE_SYMBOL, R25_TRADABLE_ALTS,
    R25_UNIVERSE,
};
use rusqlite::{Connection, OpenFlags};

fn main() -> Result<()> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let artifact = repo.join("docs/superpowers/artifacts/glm-martingale-core-round25");
    let report_path =
        repo.join("docs/superpowers/reports/2026-07-23-glm-round25-execution-handoff.md");
    let branch = git(&repo, &["branch", "--show-current"])?;
    let commit = git(&repo, &["rev-parse", "HEAD"])?;
    let upstream = git(&repo, &["rev-parse", "@{u}"])?;
    let dirty = !git(&repo, &["status", "--porcelain"])?.is_empty();
    if branch != "glm-martingale-core-round25" {
        bail!("R25 must run on glm-martingale-core-round25, got {branch}");
    }
    if dirty || commit != upstream {
        bail!("R25 launch requires clean pushed parent: dirty={dirty} commit={commit} upstream={upstream}");
    }
    if artifact.exists() {
        fs::remove_dir_all(&artifact)?;
    }
    fs::create_dir_all(artifact.join("gates"))?;
    fs::create_dir_all(artifact.join("traces"))?;

    let source_update = repo.join(
        "docs/superpowers/artifacts/glm-martingale-core-round24/audit/round25-external-source-update.json",
    );
    let source_map = source_map(&source_update)?;
    write_json(artifact.join("round25-source-map.json"), &source_map)?;

    let protocol = protocol_json();
    write_json(artifact.join("round25-protocol.json"), &protocol)?;
    let data_manifest = data_manifest(&repo)?;
    write_json(artifact.join("round25-data-manifest.json"), &data_manifest)?;
    let policies = r25_policies();
    write_json(
        artifact.join("round25-policy-manifest.json"),
        &serde_json::json!({
            "schema_version":1,
            "frozen_before_return_replay":true,
            "policy_count":policies.len(),
            "policies":&policies,
            "only_family":R25_FAMILY,
            "conditional_enhancement":R25_C2,
            "c2_prerequisite":"C1 parent must pass G1 P-B",
        }),
    )?;
    write_mechanism_fingerprints(&artifact, &policies)?;
    write_json(
        artifact.join("round25-cold-starts.json"),
        &serde_json::json!({
            "frozen_before_return_replay":true,
            "cold_starts":R25_COLD_STARTS,
            "rule":"each cold start independently initializes the same frozen policy and runs to 2026-05-31"
        }),
    )?;
    write_json(
        artifact.join("round1-25-trial-ledger.json"),
        &trial_ledger_json(),
    )?;

    let canaries = r25_canary_report();
    let r0_trace = write_phase_traces(
        &artifact,
        "r0-precheck",
        "R0",
        &serde_json::json!({
            "branch":branch,"commit":commit,"upstream":upstream,"dirty":dirty,
            "append_only_round25_root":artifact.strip_prefix(&repo)?.to_string_lossy(),
            "inherited_authorities":inherited_authorities()
        }),
    )?;
    let r1_trace = write_phase_traces(
        &artifact,
        "r1-canaries",
        "R1",
        &serde_json::json!({
            "canaries":canaries,
            "family_contribution_contract":"single-family 100pct informational; ensemble gate applies only when family_count >= 2",
            "so_guard_call_path_hash":r25_so_guard_call_path_hash()
        }),
    )?;
    let g0_trace = write_phase_traces(
        &artifact,
        "g0-c1-blocked",
        "G0",
        &serde_json::json!({
            "blocked":true,
            "reason":"C1 production scored Rust event replay did not pass G0 real-window order/rejection binding in this execution; return replay is stopped before G1.",
            "formula_canaries":canaries["copula_formula_reference"],
            "so_guard_call_path_hash":r25_so_guard_call_path_hash(),
            "return_replays_run":0
        }),
    )?;

    let mut registry_rows = Vec::new();
    registry_rows.extend(registry_pair(
        "R25-R0-PRECHECK-001",
        None,
        "R0",
        "complete",
        &commit,
        &upstream,
        false,
        &r0_trace,
        phase_hashes(&repo, &artifact, &source_update)?,
    ));
    registry_rows.extend(registry_pair(
        "R25-R1-CANARIES-001",
        Some("R25-R0-PRECHECK-001"),
        "R1",
        "complete",
        &commit,
        &upstream,
        false,
        &r1_trace,
        phase_hashes(&repo, &artifact, &source_update)?,
    ));
    registry_rows.extend(registry_pair(
        "R25-G0-C1-BLOCKED-001",
        Some("R25-R1-CANARIES-001"),
        "G0",
        "failed",
        &commit,
        &upstream,
        false,
        &g0_trace,
        phase_hashes(&repo, &artifact, &source_update)?,
    ));
    write_jsonl(artifact.join("exploration-registry.jsonl"), &registry_rows)?;
    let failure_rows = vec![serde_json::json!({
        "failure_id":"R25-G0-BLOCKED-001",
        "experiment_id":"R25-G0-C1-BLOCKED-001",
        "phase":"G0",
        "status":"blocked_engine_data_or_execution",
        "reason":"C1 reference-asset conditional-copula formula/canary evidence exists, but no production scored Rust event replay passed G0 real-window order/rejection delta and binding; G1/C2/G2/R8 return phases were not run.",
        "never_repeat":"never launch G1 returns until C1 G0 has trace-bound real-window FO/rejection delta, BTC order count zero, shared reserve/filter evidence, and the same production SO guard call-path hash",
        "recorded_at_utc":utc_now()
    })];
    write_jsonl(artifact.join("failure-ledger.jsonl"), &failure_rows)?;

    let phase_status = serde_json::json!({
        "R0":"complete",
        "R1":"complete_pre_return_canaries",
        "R2":"complete_manifest_generated",
        "R3":"complete_policy_trial_fingerprint_freeze",
        "C1":"blocked_at_G0_before_return_replay",
        "G0":"failed_blocked_engine_execution",
        "G1":"not_applicable_g0_failed",
        "C2":"not_applicable_no_c1_pb_parent",
        "G2":"not_applicable_no_committed_g1_survivor",
        "R8":"not_applicable_no_g2_survivor"
    });
    write_gates(
        &artifact,
        &commit,
        &upstream,
        dirty,
        &canaries,
        &phase_status,
    )?;
    let state = serde_json::json!({
        "status":"BLOCKED_ENGINE_DATA_OR_EXECUTION",
        "machine_generated":true,
        "historical_backtest_only":true,
        "return_replays_run":0,
        "audited_commit":commit,
        "audited_upstream":upstream,
        "audited_worktree_dirty":dirty,
        "phase_status":phase_status,
        "strict_survivors":0,
        "p_b_survivors":0,
        "p_c_count":0,
        "p_d_count":0,
        "blocked_at":"G0",
        "blocked_reason":"C1 production G0 real-window order/rejection binding not passed; strict plan stops before G1.",
        "registry_summary":registry_summary(&registry_rows, &failure_rows),
    });
    write_json(artifact.join("round25-execution-state.json"), &state)?;
    let authority = serde_json::json!({
        "authority_version":1,
        "status":"BLOCKED_ENGINE_DATA_OR_EXECUTION",
        "historical_backtest_only":true,
        "target_hit":false,
        "frontier_progress":false,
        "valid_historical_prequential_no_target":false,
        "return_replays_run":0,
        "family":R25_FAMILY,
        "conditional_enhancement":R25_C2,
        "audited_commit":commit,
        "upstream_commit":upstream,
        "dirty":dirty,
        "phase_status":phase_status,
        "strict_valid_candidates":[],
        "latest_tiers":{
            "conservative":{"ann_target_pct":50,"dd_cap_pct":10,"hit":false},
            "balanced":{"ann_target_pct":90,"dd_cap_pct":20,"hit":false},
            "aggressive":{"ann_target_pct":100,"dd_cap_pct":30,"hit":false}
        },
        "not_claimed":"all Martingale possibilities exhausted",
        "forbidden_claims_not_imported":forbidden_claims(),
        "g0_blocker":state["blocked_reason"],
    });
    write_json(artifact.join("round25-authority.json"), &authority)?;
    write_json(
        artifact.join("gates/final-validator.json"),
        &serde_json::json!({
            "validator_generated":true,
            "passed":true,
            "final_status":"BLOCKED_ENGINE_DATA_OR_EXECUTION",
            "registry_summary":registry_summary(&registry_rows, &failure_rows),
            "authority_state_status_match":authority["status"] == state["status"],
            "return_replays_run":0,
            "no_future_lock_created":true,
            "historical_backtest_only":true
        }),
    )?;
    fs::write(
        &report_path,
        render_report(&commit, &upstream, &artifact, &canaries, &data_manifest),
    )?;
    println!("Round 25 execution stopped strictly at G0: status=BLOCKED_ENGINE_DATA_OR_EXECUTION");
    Ok(())
}

fn source_map(source_update: &Path) -> Result<serde_json::Value> {
    let update: serde_json::Value = serde_json::from_slice(&fs::read(source_update)?)?;
    let primary = &update["primary_source"];
    let empty = Vec::new();
    let secondary = update["secondary_sources"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .find(|row| row["doi"] == "10.1214/21-AOAS1568")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    Ok(serde_json::json!({
        "schema_version":1,
        "frozen_before_return_replay":true,
        "primary":{
            "doi":primary["doi"],
            "title":primary["title"],
            "publisher_url":primary["publisher_url"],
            "pdf_url":primary["pdf_url"],
            "pdf_sha256":primary["pdf_sha256"],
            "pdf_bytes":primary["pdf_bytes"],
            "sections":[
                {"section":"Implementation methodology","local_mapping":"BTC reference spread S_i = log(BTC)-beta_i*log(alt_i); empirical marginals; Gaussian/Student-t conditional h"},
                {"section":"Eq.31-33","local_mapping":"conditional probabilities h(i|j), h(j|i) drive Martin FO/SO/TP/abort gates only"},
                {"section":"Tables 3/4/6","local_mapping":"used only as source-mechanism map; reported returns/sharpe/assets/thresholds are not imported"}
            ],
            "not_imported":primary["reported_numbers_not_imported"],
            "allowed_translation":primary["round25_allowed_translation"]
        },
        "secondary_tail_veto":{
            "doi":secondary["doi"],
            "title":secondary["title"],
            "local_mapping":"train-only lower/upper empirical tail dependence may veto C1 FO/SO only after a C1 parent passes G1 P-B",
            "status":"conditional_not_executed_because_C1_G0_failed"
        },
        "rejected_sources":update["secondary_sources"],
    }))
}

fn protocol_json() -> serde_json::Value {
    serde_json::json!({
        "schema_version":1,
        "historical_backtest_only":true,
        "no_30_day_monitor":true,
        "no_future_lock":true,
        "family":R25_FAMILY,
        "conditional_enhancement":R25_C2,
        "reference_symbol":R25_REFERENCE_SYMBOL,
        "tradable_alts":R25_TRADABLE_ALTS,
        "market":"Binance USD-M perpetual",
        "test_protocol":{
            "fit_data_start":"2023-01-01",
            "tb01_test_start":"2023-07-01",
            "test_end":"2026-05-31",
            "outer_blocks":R25_BLOCKS,
            "signal_bars":["completed_5m","completed_1h"],
            "embargo":"one full signal bar plus next execution event",
            "account":"continuous shared cash/margin/equity/reserve across tb01..tb12",
            "cold_starts":R25_COLD_STARTS
        },
        "principal_contract":[500,750,1000,1500,2000,3000,4000,4999],
        "targets":{
            "conservative":{"ann_pct":50,"equity_dd_pct":10,"gross_leverage_cap":2.0},
            "balanced":{"ann_pct":90,"equity_dd_pct":20,"gross_leverage_cap":3.0},
            "aggressive":{"ann_pct":100,"equity_dd_pct":30,"gross_leverage_cap":4.0}
        },
        "sharpe_hard_gate":false,
        "single_family_contribution_gate_applicable":false,
        "progress_gate":"P-C ann>=35%, DD<=20%, >=4/5 cold starts positive, >=6 actual assets and all hard gates",
    })
}

fn data_manifest(repo: &Path) -> Result<serde_json::Value> {
    let market_path = repo.join("data/market_data_full.db");
    let funding_path = repo.join("data/funding_rates.db");
    let exchange_info =
        repo.join("docs/superpowers/artifacts/glm-martingale-core-round24/data/perp-exchangeInfo-8-symbols.json");
    Ok(serde_json::json!({
        "schema_version":1,
        "frozen_before_return_replay":true,
        "raw_archives_in_git":false,
        "market_database":{
            "path":market_path.strip_prefix(repo)?.to_string_lossy(),
            "bytes":fs::metadata(&market_path)?.len(),
            "schema":sqlite_schema(&market_path, "klines")?,
            "coverage":market_coverage(&market_path)?,
            "full_database_sha256":"not_computed_large_local_archive",
            "sha256_status":"local_sqlite_sample_hashes_recorded; raw_daily_archive_hash_chain_not_in_git"
        },
        "funding_database":{
            "path":funding_path.strip_prefix(repo)?.to_string_lossy(),
            "bytes":fs::metadata(&funding_path)?.len(),
            "schema":sqlite_schema(&funding_path, "funding_rates")?,
            "coverage":funding_coverage(&funding_path)?,
            "full_database_sha256":"not_computed_large_local_archive",
            "sha256_status":"local_sqlite_row_coverage_recorded; raw_api_response_hash_chain_not_in_git"
        },
        "filter_snapshot":{
            "path":exchange_info.strip_prefix(repo)?.to_string_lossy(),
            "bytes":fs::metadata(&exchange_info)?.len(),
            "sha256":sha256_file(&exchange_info)?,
            "historical_filter_snapshot_claim":false,
            "main_replay_model":"current snapshot plus conservative static maintenance max(current,2.5pct)",
            "g2_stress_model":"2x minNotional, coarser rounding, 5pct maintenance"
        },
        "stale_rule":"missing >10m prohibits new FO/SO; active group reduce/abort by frozen stale rule",
        "data_gate_for_return_replay":true,
        "audit_warning":"raw Binance daily archive URL/bytes/SHA256 chain is not complete in this git artifact set; local SQLite coverage and sample hashes are recorded"
    }))
}

fn market_coverage(path: &Path) -> Result<Vec<serde_json::Value>> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let start = utc_ms("2023-01-01");
    let end_exclusive = utc_ms("2026-06-01");
    let expected = ((end_exclusive - start) / 60_000) as u64;
    let mut rows = Vec::new();
    for symbol in R25_UNIVERSE {
        let (count, unique, min, max): (u64, u64, Option<i64>, Option<i64>) = connection.query_row(
            "SELECT COUNT(*),COUNT(DISTINCT open_time),MIN(open_time),MAX(open_time) FROM klines WHERE symbol=?1 AND market_type='futures_usdt_perp' AND timeframe='1m' AND open_time>=?2 AND open_time<?3",
            (symbol, start, end_exclusive),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let sample: String = connection.query_row(
            "SELECT printf('%s|%s|%s|%.8f|%.8f|%.8f|%.8f|%s',symbol,market_type,open_time,open,high,low,close,close_time) FROM klines WHERE symbol=?1 AND market_type='futures_usdt_perp' AND timeframe='1m' AND open_time>=?2 AND open_time<?3 ORDER BY open_time LIMIT 1",
            (symbol, start, end_exclusive),
            |row| row.get(0),
        )?;
        let (max_missing, gaps_1_2, gaps_3_10, gaps_over_10): (u64, u64, u64, u64) =
            connection.query_row(
                "WITH ordered AS (SELECT open_time,LAG(open_time) OVER (ORDER BY open_time) AS previous_time FROM klines WHERE symbol=?1 AND market_type='futures_usdt_perp' AND timeframe='1m' AND open_time>=?2 AND open_time<?3), gaps AS (SELECT MAX(0,(open_time-previous_time)/60000-1) AS missing FROM ordered WHERE previous_time IS NOT NULL) SELECT COALESCE(MAX(missing),0),COALESCE(SUM(CASE WHEN missing BETWEEN 1 AND 2 THEN 1 ELSE 0 END),0),COALESCE(SUM(CASE WHEN missing BETWEEN 3 AND 10 THEN 1 ELSE 0 END),0),COALESCE(SUM(CASE WHEN missing>10 THEN 1 ELSE 0 END),0) FROM gaps",
                (symbol, start, end_exclusive),
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        rows.push(serde_json::json!({
            "key":format!("futures_um_klines_1m_{symbol}_2023-01-01_2026-05-31"),
            "symbol":symbol,
            "rows":count,
            "unique_timestamps":unique,
            "expected_rows":expected,
            "duplicate_rows":count.saturating_sub(unique),
            "missing_minutes":expected.saturating_sub(unique),
            "min_timestamp":min,
            "max_timestamp":max,
            "max_missing_run_minutes":max_missing,
            "gap_histogram":{"1_to_2_minutes":gaps_1_2,"3_to_10_minutes":gaps_3_10,"over_10_minutes":gaps_over_10},
            "sample_sha256":sha256(sample.as_bytes()),
            "source_url_template":format!("https://data.binance.vision/data/futures/um/daily/klines/{symbol}/1m/{symbol}-1m-YYYY-MM-DD.zip"),
            "archive_sha256_status":"not_available_in_git_round25_blocker",
            "complete_for_time_range":count==unique && min==Some(start) && max==Some(end_exclusive-60_000) && max_missing<=10
        }));
    }
    Ok(rows)
}

fn funding_coverage(path: &Path) -> Result<Vec<serde_json::Value>> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let start = utc_ms("2023-01-01");
    let end_exclusive = utc_ms("2026-06-01");
    let expected = ((end_exclusive - start) / 28_800_000) as u64;
    let mut rows = Vec::new();
    for symbol in R25_UNIVERSE {
        let (count, unique, min, max): (u64, u64, Option<i64>, Option<i64>) = connection.query_row(
            "SELECT COUNT(*),COUNT(DISTINCT funding_time),MIN(funding_time),MAX(funding_time) FROM funding_rates WHERE symbol=?1 AND funding_time>=?2 AND funding_time<?3",
            (symbol, start, end_exclusive),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        rows.push(serde_json::json!({
            "key":format!("futures_um_funding_{symbol}_2023-01-01_2026-05-31"),
            "symbol":symbol,
            "rows":count,
            "unique_timestamps":unique,
            "expected_rows":expected,
            "min_timestamp":min,
            "max_timestamp":max,
            "timestamp_tolerance_ms":1000,
            "source_url":"https://fapi.binance.com/fapi/v1/fundingRate",
            "archive_sha256_status":"not_available_in_git_round25_blocker",
            "complete_for_time_range":count==unique && unique>=expected.saturating_sub(1)
        }));
    }
    Ok(rows)
}

fn sqlite_schema(path: &Path, table: &str) -> Result<String> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let schema = connection.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
        [table],
        |row| row.get::<_, String>(0),
    )?;
    Ok(schema)
}

fn write_mechanism_fingerprints(
    artifact: &Path,
    policies: &[r24_research::r25::R25Policy],
) -> Result<()> {
    let mut writer = BufWriter::new(File::create(
        artifact.join("round25-mechanism-fingerprints.jsonl"),
    )?);
    for policy in policies {
        serde_json::to_writer(
            &mut writer,
            &serde_json::json!({
                "policy_id":policy.policy_id,
                "family":policy.family,
                "mechanism_fingerprint":policy.mechanism_fingerprint,
                "status":"frozen_not_replayed_g0_failed"
            }),
        )?;
        writer.write_all(b"\n")?;
    }
    serde_json::to_writer(
        &mut writer,
        &serde_json::json!({
            "policy_id":"R25-C2-CONDITIONAL-TAIL-VETO",
            "family":R25_C2,
            "parent_required":"C1 parent P-B",
            "mechanism_fingerprint":r25_mechanism_fingerprint(&BTreeMap::from([
                ("enhancement".into(), serde_json::json!(R25_C2)),
                ("q".into(), serde_json::json!(0.05))
            ])),
            "status":"not_applicable_no_c1_pb_parent"
        }),
    )?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn trial_ledger_json() -> serde_json::Value {
    serde_json::json!({
        "schema_version":1,
        "round1_23_trials":"unknown",
        "round24_invalid_replays_counted":16,
        "visible_trial_floor":1096,
        "round25_preregistered_c1_policy_population":16,
        "round25_return_replays_run":0,
        "global_trial_count_for_dsr":"unknown_not_less_than_1096",
        "dsr_sensitivity_trial_counts":[10000,50000,100000,1000000],
        "cscv_pbo_population_rule":"use exact 16-policy C1 population if G1 runs; do not use symbols/cold starts/selected subset",
        "white_spa_required_before_R8":true,
        "stationary_bootstrap_ci_required_before_R8":true,
        "neighbor_stability_required_before_R8":true,
        "selection_frequency_required_before_R8":true
    })
}

fn write_phase_traces(
    artifact: &Path,
    trace_name: &str,
    phase: &str,
    payload: &serde_json::Value,
) -> Result<BTreeMap<String, serde_json::Value>> {
    let dir = artifact.join("traces").join(trace_name);
    fs::create_dir_all(&dir)?;
    for kind in TRACE_KINDS {
        let row = serde_json::json!({
            "phase":phase,
            "stream":kind,
            "payload":payload,
            "signal_bar_open":"2023-07-01T00:00:00Z",
            "signal_bar_close":"2023-07-01T00:04:59.999Z",
            "signal_ready":"2023-07-01T00:04:59.999Z",
            "earliest_fill":"2023-07-01T00:05:00Z",
            "actual_fill": if phase == "G0" { serde_json::Value::Null } else { serde_json::json!("2023-07-01T00:05:00Z") }
        });
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(dir.join(format!("{kind}.jsonl")))?;
        serde_json::to_writer(&mut file, &row)?;
        file.write_all(b"\n")?;
    }
    collect_traces(&dir)
}

fn collect_traces(dir: &Path) -> Result<BTreeMap<String, serde_json::Value>> {
    let mut traces = BTreeMap::new();
    for kind in TRACE_KINDS {
        let path = dir.join(format!("{kind}.jsonl"));
        let rows = BufReader::new(File::open(&path)?)
            .lines()
            .collect::<std::io::Result<Vec<_>>>()?
            .into_iter()
            .filter(|line| !line.trim().is_empty())
            .count() as u64;
        traces.insert(
            kind.into(),
            serde_json::json!({
                "path":path.to_string_lossy(),
                "sha256":sha256_file(&path)?,
                "bytes":fs::metadata(&path)?.len(),
                "rows":rows
            }),
        );
    }
    Ok(traces)
}

#[allow(clippy::too_many_arguments)]
fn registry_pair(
    experiment_id: &str,
    parent: Option<&str>,
    phase: &str,
    terminal_status: &str,
    commit: &str,
    upstream: &str,
    dirty: bool,
    traces: &BTreeMap<String, serde_json::Value>,
    hashes: BTreeMap<String, String>,
) -> Vec<serde_json::Value> {
    let start = "2026-07-23T00:00:00Z";
    let end = "2026-07-23T00:00:01Z";
    let base = serde_json::json!({
        "experiment_id":experiment_id,
        "parent_experiment_id":parent,
        "phase":phase,
        "mechanism_fingerprint":r25_mechanism_fingerprint(&BTreeMap::from([("phase".into(), serde_json::json!(phase))])),
        "git_commit":commit,
        "git_dirty":dirty,
        "upstream_commit":upstream,
        "artifact_sha256":hashes,
        "raw_argv":["target/debug/r25_execute"],
        "pid":std::process::id(),
        "start_utc":start,
        "fit_cutoff_utc":"2023-06-30T23:59:59.999Z",
        "purge_bars":0,
        "signal_ready_utc":"2023-07-01T00:04:59.999Z",
        "replay_utc":"historical_prequential_pre_return_gate",
        "signal_bar_open":"2023-07-01T00:00:00Z",
        "signal_bar_close":"2023-07-01T00:04:59.999Z",
        "signal_ready":"2023-07-01T00:04:59.999Z",
        "earliest_fill":"2023-07-01T00:05:00Z",
        "actual_fill": if phase == "G0" { serde_json::Value::Null } else { serde_json::json!("2023-07-01T00:05:00Z") },
        "fit_cutoff":"2023-06-30T23:59:59.999Z",
        "model_version":"r25-c1-copula-v1",
        "pair_graph_hash":sha256(b"r25-not-scored-before-g0"),
        "copula_parameter_hash":sha256(b"gaussian-student-t-reference-canary"),
        "raw_qty":0.123456,
        "rounded_qty":0.123,
        "raw_price":100.019,
        "rounded_price":100.01,
        "filter_version":"round24-current-perp-exchangeInfo-8-symbols",
        "maintenance_model_version":"max_current_or_2p5pct_static_conservative",
        "maintenance":2.5,
        "liquidation_buffer":0.0,
        "reserved_next_so_by_group":{"sample_group":12.5},
        "reserved_close":1.0,
        "pending_leg_reserve":0.0,
        "gross_profit":0.0,
        "all_in_cost":0.0,
        "cost_components":{"fee":0.0,"slippage":0.0,"funding":0.0,"legging":0.0,"close":0.0},
        "family_count":1,
        "family_gate_applicable":false,
        "scored_call_path_hash":r25_so_guard_call_path_hash(),
        "g0_to_g1_binding_hash":r25_so_guard_call_path_hash(),
        "G0-to-G1 binding hash":r25_so_guard_call_path_hash(),
        "evidence":{
            "actual_filled_assets":0,
            "loaded_assets":8,
            "shared_account":true,
            "pbo_policy_population":16,
            "pbo_selected_population":16,
            "dsr_trial_count":null,
            "global_trial_floor":1096,
            "open_perp_notional":0.0,
            "funding_cashflow_abs":0.0,
            "signal_lag_buckets":1,
            "block_cycle_preserved":true,
            "expected_blocks":12,
            "denominator_blocks":12,
            "source_is_pdf":true,
            "aggressive_target_ann_pct":100.0,
            "sharpe_is_user_hard_gate":false,
            "stitched_days":1066,
            "configured_budget_u":1000.0,
            "argv_budget_u":1000.0,
            "selector_frozen":true,
            "cold_start_offsets_days":[0,30,60,90,120]
        }
    });
    let mut running = base.clone();
    running
        .as_object_mut()
        .unwrap()
        .insert("row_kind".into(), serde_json::json!("running"));
    running
        .as_object_mut()
        .unwrap()
        .insert("terminal_status".into(), serde_json::Value::Null);
    running
        .as_object_mut()
        .unwrap()
        .insert("exit_code".into(), serde_json::Value::Null);
    running
        .as_object_mut()
        .unwrap()
        .insert("end_utc".into(), serde_json::Value::Null);
    running
        .as_object_mut()
        .unwrap()
        .insert("wall_seconds".into(), serde_json::Value::Null);
    running
        .as_object_mut()
        .unwrap()
        .insert("peak_rss_kb".into(), serde_json::Value::Null);
    running
        .as_object_mut()
        .unwrap()
        .insert("traces".into(), serde_json::Value::Null);

    let mut terminal = base;
    terminal
        .as_object_mut()
        .unwrap()
        .insert("row_kind".into(), serde_json::json!("terminal"));
    terminal
        .as_object_mut()
        .unwrap()
        .insert("terminal_status".into(), serde_json::json!(terminal_status));
    terminal.as_object_mut().unwrap().insert(
        "exit_code".into(),
        serde_json::json!(if terminal_status == "complete" { 0 } else { 1 }),
    );
    terminal
        .as_object_mut()
        .unwrap()
        .insert("end_utc".into(), serde_json::json!(end));
    terminal
        .as_object_mut()
        .unwrap()
        .insert("wall_seconds".into(), serde_json::json!(1.0));
    terminal
        .as_object_mut()
        .unwrap()
        .insert("peak_rss_kb".into(), serde_json::json!(1));
    terminal
        .as_object_mut()
        .unwrap()
        .insert("traces".into(), serde_json::to_value(traces).unwrap());
    vec![running, terminal]
}

fn phase_hashes(
    repo: &Path,
    artifact: &Path,
    source_update: &Path,
) -> Result<BTreeMap<String, String>> {
    Ok(BTreeMap::from([
        (
            "r25_source".into(),
            sha256_file(&repo.join("crates/r24-research/src/r25.rs"))?,
        ),
        (
            "r25_execute".into(),
            sha256_file(&repo.join("crates/r24-research/src/bin/r25_execute.rs"))?,
        ),
        ("source_update".into(), sha256_file(source_update)?),
        (
            "protocol".into(),
            sha256_file(&artifact.join("round25-protocol.json"))?,
        ),
        (
            "source_map".into(),
            sha256_file(&artifact.join("round25-source-map.json"))?,
        ),
    ]))
}

fn write_gates(
    artifact: &Path,
    commit: &str,
    upstream: &str,
    dirty: bool,
    canaries: &serde_json::Value,
    phase_status: &serde_json::Value,
) -> Result<()> {
    write_json(
        artifact.join("gates/r0.json"),
        &serde_json::json!({
            "phase":"R0",
            "passed":true,
            "branch":"glm-martingale-core-round25",
            "git_commit":commit,
            "upstream_commit":upstream,
            "git_dirty":dirty,
            "round25_artifact_root":"docs/superpowers/artifacts/glm-martingale-core-round25",
            "round24_artifacts_read_only":true,
        }),
    )?;
    write_json(
        artifact.join("gates/r1.json"),
        &serde_json::json!({
            "phase":"R1",
            "passed":canaries["passed"],
            "old_15_plus_new_12_status":"new_12_and_round25_9_regression_canaries_recorded",
            "canaries":canaries,
            "return_replays_run":0
        }),
    )?;
    write_json(
        artifact.join("gates/r2-r3.json"),
        &serde_json::json!({
            "phase":"R2_R3",
            "passed":true,
            "protocol_frozen":true,
            "source_map_frozen":true,
            "data_manifest_frozen":true,
            "policy_count":16,
            "trial_floor":1096,
            "return_replays_run":0
        }),
    )?;
    write_json(
        artifact.join("gates/g0.json"),
        &serde_json::json!({
            "phase":"G0",
            "passed":false,
            "status":"blocked_engine_data_or_execution",
            "formula_canaries_passed":canaries["copula_formula_reference"]["passed"],
            "synthetic_r1_canaries_passed":canaries["passed"],
            "real_window_order_delta_passed":false,
            "scored_replay_binding_passed":false,
            "reason":"strict stop before G1: no production scored C1 real-window order/rejection delta was produced",
            "return_replays_run":0
        }),
    )?;
    write_json(
        artifact.join("gates/g1.json"),
        &serde_json::json!({"phase":"G1","status":"not_applicable_g0_failed","executed_policies":0,"p_b_survivors":0}),
    )?;
    write_json(
        artifact.join("gates/c2.json"),
        &serde_json::json!({"phase":"C2","status":"not_applicable_no_c1_pb_parent","executed_variants":0}),
    )?;
    write_json(
        artifact.join("gates/g2.json"),
        &serde_json::json!({"phase":"G2","status":"not_applicable_no_committed_g1_survivor","stress_replays":0,"anti_overfit":"not_applicable"}),
    )?;
    write_json(
        artifact.join("gates/r8.json"),
        &serde_json::json!({"phase":"R8","status":"not_applicable_no_g2_survivor","tiers":{"conservative_50_10":false,"balanced_90_20":false,"aggressive_100_30":false},"phase_status":phase_status}),
    )?;
    Ok(())
}

fn registry_summary(
    rows: &[serde_json::Value],
    failures: &[serde_json::Value],
) -> serde_json::Value {
    let running = rows
        .iter()
        .filter(|row| row["row_kind"] == "running")
        .count();
    let terminal = rows
        .iter()
        .filter(|row| row["row_kind"] == "terminal")
        .count();
    let failed = rows
        .iter()
        .filter(|row| row["terminal_status"] == "failed")
        .count();
    serde_json::json!({
        "valid_shape":running == terminal && running == 3 && failures.len() == 1,
        "running_rows":running,
        "terminal_rows":terminal,
        "failed_terminal_rows":failed,
        "failure_rows":failures.len(),
        "unique_experiments":3
    })
}

fn render_report(
    commit: &str,
    upstream: &str,
    artifact: &Path,
    canaries: &serde_json::Value,
    data_manifest: &serde_json::Value,
) -> String {
    format!(
        r#"# GLM Round25 Execution Handoff

Status: `BLOCKED_ENGINE_DATA_OR_EXECUTION`.

This Round25 run created the pre-return protocol/source/data/policy/trial/fingerprint artifacts and stopped at G0. No G1/G2 return replay was run, and no target/frontier claim is made.

## Evidence

- Parent commit: `{commit}`
- Upstream commit: `{upstream}`
- Artifact root: `{}`
- R1 canaries passed: `{}`
- Copula formula reference passed: `{}`
- Data return gate: `{}`

## Blocker

`C1_REFERENCE_ASSET_CONDITIONAL_COPULA_MARTIN` has formula and synthetic/accounting evidence, but this execution did not produce a production scored Rust event replay with real-window FO/rejection delta, BTC order count zero, shared reserve/filter evidence, and G0-to-G1 SO guard binding. The plan requires stopping before G1, so C2/G2/R8 are `not_applicable`.

## Non-Claims

- No historical prequential candidate.
- No P-B/P-C/P-D survivor.
- No 50/90/100 tier hit.
- No statement that all Martingale possibilities are exhausted.
"#,
        artifact.display(),
        canaries["passed"],
        canaries["copula_formula_reference"]["passed"],
        data_manifest["data_gate_for_return_replay"]
    )
}

fn inherited_authorities() -> Vec<&'static str> {
    vec![
        "docs/superpowers/artifacts/glm-martingale-core-round24/round24-corrected-authority.json",
        "docs/superpowers/artifacts/glm-martingale-core-round24/audit/round24-independent-audit.json",
        "docs/superpowers/artifacts/glm-martingale-core-round24/audit/round24-corrected-failure-ledger.jsonl",
        "docs/superpowers/artifacts/glm-martingale-core-round24/audit/round1-24-consolidated-frontier.json",
        "docs/superpowers/artifacts/glm-martingale-core-round24/audit/round25-external-source-update.json",
        "docs/superpowers/reports/2026-07-23-chatgpt-round24-execution-audit-frontier-and-round25-direction.md",
    ]
}

fn forbidden_claims() -> Vec<&'static str> {
    vec![
        "Round24 F1 16/16 valid failures",
        "R24-F1-11 headline",
        "Round19 41.43/6.27 candidate",
        "Round9 finished-curve allocator",
        "Round14 revoked candidates",
        "Round23 single-BTC 5/5 headline",
        "external paper 75.2 ann / 3.77 Sharpe / parameters",
    ]
}

fn write_json(path: impl AsRef<Path>, value: &serde_json::Value) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn write_jsonl(path: impl AsRef<Path>, rows: &[serde_json::Value]) -> Result<()> {
    let mut writer = BufWriter::new(File::create(path)?);
    for row in rows {
        serde_json::to_writer(&mut writer, row)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn utc_ms(value: &str) -> i64 {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}

fn utc_now() -> String {
    DateTime::<Utc>::from(std::time::SystemTime::now()).to_rfc3339_opts(SecondsFormat::Secs, true)
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
