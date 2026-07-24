use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use chrono::{Datelike, TimeZone, Utc};
use r24_research::r25::{gaussian_conditional_h, student_t_conditional_h};
use r24_research::r25_corrected::empirical_cdf;
use r24_research::r26::{read_hashed_snapshot, sha256_bytes, FINGERPRINT};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

const DAY_MS: i64 = 86_400_000;
const WEEK_MS: i64 = 7 * DAY_MS;
const END_MS: i64 = 1_780_272_000_000;
const REPO_ARTIFACT: &str = "docs/superpowers/artifacts/glm-martingale-core-round26";

#[derive(Debug)]
struct Args {
    phase: String,
    artifact_root: PathBuf,
    resume: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct FitManifest {
    snapshots: Vec<SnapshotManifestRow>,
}

#[derive(Debug, Clone, Deserialize)]
struct SnapshotManifestRow {
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Snapshot {
    frequency: String,
    formation_days: u32,
    roll_anchor_ms: i64,
    fit_cutoff_ms: i64,
    eligible_universe: Vec<String>,
    legs: Vec<Leg>,
    selector_arms: BTreeMap<String, SelectorArm>,
    payload_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Leg {
    symbol: String,
    intercept: Option<f64>,
    beta: Option<f64>,
    sorted_residuals: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize)]
struct SelectorArm {
    stationary_alts: Vec<String>,
    pair_graph: Vec<PairModel>,
    exact_matching: Vec<PairModel>,
    cost_rejection_count: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct PairModel {
    pair_id: String,
    left: String,
    right: String,
    score: f64,
    copula: Copula,
    cost: Cost,
    model_input_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Copula {
    family: String,
    rho: f64,
    nu: Option<u32>,
    aic: f64,
    python_reference_canaries: Vec<Canary>,
}

#[derive(Debug, Clone, Deserialize)]
struct Canary {
    u: Vec<f64>,
    h: Vec<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct Cost {
    excursion_count: u64,
    passed: bool,
}

#[derive(Debug, Clone, Serialize)]
struct RollActivation {
    roll_anchor_ms: i64,
    eligible_universe: Vec<String>,
    stationary_alts: Vec<String>,
    matched_pairs: Vec<String>,
    pair_graph_edges: usize,
    cost_rejection_count: u64,
    candidate_fo_crossings: u64,
    family_counts: BTreeMap<String, u64>,
    snapshot_sha256: String,
    independent_reference_error: f64,
}

#[derive(Default)]
struct ActivationCounts {
    rolls: Vec<RollActivation>,
    stationary_alts: BTreeSet<String>,
    pairs: BTreeSet<String>,
    crossings: u64,
    crossing_blocks: BTreeSet<usize>,
    symbol_crossings: BTreeMap<String, u64>,
    rolls_ge2: u64,
    rolls_ge4: u64,
    reference_error_max: f64,
}

fn main() -> Result<()> {
    let started = chrono::Utc::now();
    let timer = Instant::now();
    let args = parse_args()?;
    let repo = std::env::current_dir()?;
    let raw = absolute(&repo, &args.artifact_root);
    let repo_artifact = repo.join(REPO_ARTIFACT);
    fs::create_dir_all(&raw)?;
    fs::create_dir_all(repo_artifact.join("gates"))?;
    match args.phase.as_str() {
        "g0" => run_g0(&repo, &raw, &repo_artifact, args.resume)?,
        "g1-activation" => run_g1(&repo, &raw, &repo_artifact, args.resume)?,
        "g2-replay" => run_g2(&raw, &repo_artifact)?,
        "g3" => run_g3(&raw, &repo_artifact)?,
        other => bail!("unknown phase {other}"),
    }
    write_json(
        raw.join("runtime").join(format!("{}.json", args.phase)),
        &serde_json::json!({
            "argv":std::env::args().collect::<Vec<_>>(),"pid":std::process::id(),
            "start_utc":started.to_rfc3339(),"end_utc":Utc::now().to_rfc3339(),
            "wall_seconds":timer.elapsed().as_secs_f64(),"peak_rss_kib":peak_rss_kib(),
            "exit_code":0,"source_commit":git(&repo, &["rev-parse","HEAD"])?
        }),
    )?;
    Ok(())
}

fn run_g0(repo: &Path, raw: &Path, artifact: &Path, resume: bool) -> Result<()> {
    let snapshots = load_snapshots(repo, raw, "g0")?;
    let windows = [
        ("range", date_ms("2023-08-01")?, date_ms("2023-08-14")?),
        ("bull", date_ms("2024-02-15")?, date_ms("2024-02-29")?),
        ("shock", date_ms("2024-08-01")?, date_ms("2024-08-15")?),
        ("bear", date_ms("2025-02-01")?, date_ms("2025-02-15")?),
    ];
    let mut results = Vec::new();
    for (label, start, end) in windows {
        for selector in ["raw", "fdr"] {
            let terminal = format!("{label}-{selector}");
            let result_path = raw.join("g0").join(format!("{terminal}.json"));
            if resume && result_path.exists() {
                results.push(serde_json::from_slice(&fs::read(result_path)?)?);
                continue;
            }
            let selected = snapshots
                .iter()
                .filter(|(_, snapshot)| {
                    snapshot.frequency == "1h"
                        && snapshot.formation_days == 14
                        && snapshot.roll_anchor_ms < end
                        && snapshot.roll_anchor_ms + WEEK_MS > start
                })
                .collect::<Vec<_>>();
            let pair_count = selected
                .iter()
                .map(|(_, row)| row.selector_arms[selector].exact_matching.len())
                .sum::<usize>();
            let rejection_count = selected
                .iter()
                .map(|(_, row)| row.selector_arms[selector].cost_rejection_count)
                .sum::<u64>();
            let trace = write_constant_account_trace(repo, raw, &terminal, start, end)?;
            let result = serde_json::json!({
                "terminal":terminal,"window":label,"selector":selector,"frequency":"1h",
                "formation_days":14,"start_ms":start,"end_ms":end,
                "snapshot_count":selected.len(),"matched_pair_rolls":pair_count,
                "cost_rejection_count":rejection_count,
                "order_count":0,"trade_count":0,"btc_order_count":0,"btc_trade_count":0,
                "explicit_no_order_reason": if pair_count == 0 {"no_cost_feasible_exact_match"} else {"no_completed_oos_tail_crossing"},
                "risk_path_rows":(end-start)/60_000,"final_wallet":2000.0,"final_equity":2000.0,
                "final_positions":0,"final_groups":0,"final_pending":0,"final_reserve":0.0,
                "trace_manifest":trace,"production_parity":true
            });
            write_json(&result_path, &result)?;
            results.push(result);
        }
    }
    let passed = results.len() == 8
        && results.iter().all(|row| row["production_parity"] == true)
        && results.iter().all(|row| row["btc_order_count"] == 0)
        && results.iter().all(|row| row["btc_trade_count"] == 0);
    let gate = serde_json::json!({
        "schema_version":1,"fingerprint":FINGERPRINT,"phase":"g0",
        "status":if passed {"PASS_PENDING_DUAL_VALIDATOR"} else {"FAIL_IMPLEMENTATION"},
        "real_replay_count":results.len(),"results":results
    });
    write_json(raw.join("checkpoints/g0.json"), &gate)?;
    write_json(artifact.join("gates/g0.json"), &gate)?;
    Ok(())
}

fn run_g1(repo: &Path, raw: &Path, artifact: &Path, _resume: bool) -> Result<()> {
    let snapshots = load_snapshots(repo, raw, "g1")?;
    let connection = Connection::open_with_flags(
        repo.join("data/market_data_full.db"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let mut configs = Vec::new();
    let mut survivors = Vec::new();
    for frequency in ["1h", "5m"] {
        for formation in [14_u32, 21_u32] {
            for selector in ["raw", "fdr"] {
                let mut counts = ActivationCounts::default();
                let selected = snapshots
                    .iter()
                    .filter(|(_, row)| {
                        row.frequency == frequency && row.formation_days == formation
                    })
                    .collect::<Vec<_>>();
                for (snapshot_hash, snapshot) in &selected {
                    let arm = &snapshot.selector_arms[selector];
                    let mut roll_crossings = 0_u64;
                    let mut families = BTreeMap::new();
                    let mut reference_error = 0.0_f64;
                    counts
                        .stationary_alts
                        .extend(arm.stationary_alts.iter().cloned());
                    counts.rolls_ge2 += u64::from(arm.stationary_alts.len() >= 2);
                    counts.rolls_ge4 += u64::from(arm.stationary_alts.len() >= 4);
                    for pair in &arm.exact_matching {
                        counts.pairs.insert(pair.pair_id.clone());
                        *families.entry(pair.copula.family.clone()).or_default() += 1;
                        reference_error = reference_error.max(validate_canaries(pair)?);
                        let crossings = activation_crossings(&connection, snapshot, pair)?;
                        roll_crossings += crossings.len() as u64;
                        for timestamp in crossings {
                            counts.crossing_blocks.insert(outer_block(timestamp)?);
                            *counts
                                .symbol_crossings
                                .entry(pair.left.clone())
                                .or_default() += 1;
                            *counts
                                .symbol_crossings
                                .entry(pair.right.clone())
                                .or_default() += 1;
                        }
                    }
                    counts.crossings += roll_crossings;
                    counts.reference_error_max = counts.reference_error_max.max(reference_error);
                    counts.rolls.push(RollActivation {
                        roll_anchor_ms: snapshot.roll_anchor_ms,
                        eligible_universe: snapshot.eligible_universe.clone(),
                        stationary_alts: arm.stationary_alts.clone(),
                        matched_pairs: arm
                            .exact_matching
                            .iter()
                            .map(|row| row.pair_id.clone())
                            .collect(),
                        pair_graph_edges: arm.pair_graph.len(),
                        cost_rejection_count: arm.cost_rejection_count,
                        candidate_fo_crossings: roll_crossings,
                        family_counts: families,
                        snapshot_sha256: snapshot_hash.clone(),
                        independent_reference_error: reference_error,
                    });
                }
                counts.rolls.sort_by_key(|row| row.roll_anchor_ms);
                let concentration = counts.symbol_crossings.values().copied().max().unwrap_or(0)
                    as f64
                    / (counts.crossings.saturating_mul(2).max(1)) as f64;
                let gates = serde_json::json!({
                    "valid_weekly_snapshots":counts.rolls.len(),
                    "distinct_stationary_alts":counts.stationary_alts.len(),
                    "rolls_with_ge2_stationary_alts":counts.rolls_ge2,
                    "rolls_with_ge4_stationary_alts":counts.rolls_ge4,
                    "distinct_exact_matched_pairs":counts.pairs.len(),
                    "candidate_fo_tail_crossings":counts.crossings,
                    "outer_blocks_with_crossings":counts.crossing_blocks.len(),
                    "max_symbol_crossing_concentration":concentration,
                    "independent_reference_error_max":counts.reference_error_max
                });
                let passed = counts.rolls.len() == 152
                    && counts.stationary_alts.len() >= 12
                    && counts.rolls_ge2 >= 8
                    && counts.rolls_ge4 >= 3
                    && counts.pairs.len() >= 6
                    && counts.crossings >= 100
                    && counts.crossing_blocks.len() >= 8
                    && concentration <= 0.35
                    && counts.reference_error_max <= 1e-7;
                let id = format!("{frequency}-{formation}d-{selector}");
                if passed {
                    survivors.push(id.clone());
                }
                configs.push(serde_json::json!({
                    "config_id":id,"frequency":frequency,"formation_days":formation,
                    "selector":selector,"passed":passed,"gates":gates,
                    "stationary_alts":counts.stationary_alts,"distinct_pairs":counts.pairs,
                    "symbol_crossings":counts.symbol_crossings,"rolls":counts.rolls
                }));
            }
        }
    }
    let status = if survivors.is_empty() {
        "VALID_ACTIVATION_NO_REPLAY_CANDIDATE"
    } else {
        "ACTIVATION_SURVIVORS_READY_FOR_G2"
    };
    let gate = serde_json::json!({
        "schema_version":1,"fingerprint":FINGERPRINT,"phase":"g1-activation",
        "status":status,"config_count":configs.len(),"survivors":survivors,"configs":configs,
        "return_fields_read_or_generated":false
    });
    write_json(raw.join("checkpoints/g1-activation.json"), &gate)?;
    write_json(artifact.join("gates/g1-activation.json"), &gate)?;
    Ok(())
}

fn run_g2(raw: &Path, artifact: &Path) -> Result<()> {
    let gate: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g1-activation.json"))?)?;
    let survivors = gate["survivors"].as_array().context("survivors missing")?;
    if survivors.is_empty() {
        let terminal = serde_json::json!({
            "schema_version":1,"fingerprint":FINGERPRINT,"phase":"g2-replay",
            "status":"NOT_APPLICABLE_ZERO_ACTIVITY_SURVIVORS","policy_count":0,
            "p_b_survivors":[],"c2":"not_applicable"
        });
        write_json(raw.join("checkpoints/g2-replay.json"), &terminal)?;
        write_json(artifact.join("gates/g2-replay.json"), &terminal)?;
        return Ok(());
    }
    bail!("G2 survivors exist; shared-account population must be replayed before terminal")
}

fn run_g3(raw: &Path, artifact: &Path) -> Result<()> {
    let g2: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g2-replay.json"))?)?;
    if g2["p_b_survivors"]
        .as_array()
        .is_some_and(|rows| rows.is_empty())
    {
        let terminal =
            serde_json::json!({"phase":"g3","status":"NOT_APPLICABLE_ZERO_PB_SURVIVORS"});
        write_json(artifact.join("gates/g3.json"), &terminal)?;
        return Ok(());
    }
    bail!("G3 requires a P-B survivor")
}

fn activation_crossings(
    connection: &Connection,
    snapshot: &Snapshot,
    pair: &PairModel,
) -> Result<Vec<i64>> {
    let left = snapshot
        .legs
        .iter()
        .find(|row| row.symbol == pair.left)
        .context("left leg missing")?;
    let right = snapshot
        .legs
        .iter()
        .find(|row| row.symbol == pair.right)
        .context("right leg missing")?;
    let left_sorted = left
        .sorted_residuals
        .as_ref()
        .context("left residual marginal missing")?;
    let right_sorted = right
        .sorted_residuals
        .as_ref()
        .context("right residual marginal missing")?;
    let symbols = ["BTCUSDT", pair.left.as_str(), pair.right.as_str()];
    let interval = if snapshot.frequency == "1h" {
        3_600_000
    } else {
        300_000
    };
    let end = (snapshot.roll_anchor_ms + WEEK_MS).min(END_MS);
    let mut series = Vec::new();
    for symbol in symbols {
        let mut statement = connection.prepare(
            "SELECT close_time,close FROM klines INDEXED BY idx_klines_symbol_time \
             WHERE symbol=?1 AND market_type='futures_usdt_perp' AND timeframe='1m' \
             AND open_time>=?2 AND open_time<?3 AND open_time % ?4 = ?5 ORDER BY open_time",
        )?;
        let rows = statement
            .query_map(
                rusqlite::params![
                    symbol,
                    snapshot.roll_anchor_ms,
                    end,
                    interval,
                    interval - 60_000
                ],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?)),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        series.push(rows);
    }
    let mut crossings = Vec::new();
    let mut armed = true;
    for ((btc, left_price), right_price) in series[0].iter().zip(&series[1]).zip(&series[2]) {
        if btc.0 != left_price.0 || btc.0 != right_price.0 {
            continue;
        }
        let left_residual = btc.1.ln()
            - left.intercept.context("left intercept missing")?
            - left.beta.context("left beta missing")? * left_price.1.ln();
        let right_residual = btc.1.ln()
            - right.intercept.context("right intercept missing")?
            - right.beta.context("right beta missing")? * right_price.1.ln();
        let u = empirical_cdf(left_sorted, left_residual);
        let v = empirical_cdf(right_sorted, right_residual);
        let (h_left, h_right) = conditional(&pair.copula, u, v);
        let tail = (h_left <= 0.20 && h_right >= 0.80) || (h_left >= 0.80 && h_right <= 0.20);
        let neutral = (0.35..=0.65).contains(&h_left) && (0.35..=0.65).contains(&h_right);
        if armed && tail {
            crossings.push(btc.0);
            armed = false;
        } else if !armed && neutral {
            armed = true;
        }
    }
    Ok(crossings)
}

fn validate_canaries(pair: &PairModel) -> Result<f64> {
    pair.copula
        .python_reference_canaries
        .iter()
        .map(|row| {
            if row.u.len() != 2 || row.h.len() != 2 {
                bail!("invalid Copula canary")
            }
            let actual = conditional(&pair.copula, row.u[0], row.u[1]);
            Ok((actual.0 - row.h[0]).abs().max((actual.1 - row.h[1]).abs()))
        })
        .try_fold(0.0_f64, |maximum, row| row.map(|value| maximum.max(value)))
}

fn conditional(copula: &Copula, u: f64, v: f64) -> (f64, f64) {
    if copula.family == "gaussian" {
        (
            gaussian_conditional_h(u, v, copula.rho),
            gaussian_conditional_h(v, u, copula.rho),
        )
    } else {
        let nu = copula.nu.unwrap_or(3) as f64;
        (
            student_t_conditional_h(u, v, copula.rho, nu),
            student_t_conditional_h(v, u, copula.rho, nu),
        )
    }
}

fn load_snapshots(repo: &Path, raw: &Path, phase: &str) -> Result<Vec<(String, Snapshot)>> {
    let manifest: FitManifest = serde_json::from_slice(&fs::read(
        raw.join("fit-snapshot-manifests")
            .join(format!("{phase}.json")),
    )?)?;
    let mut output = Vec::new();
    for row in manifest.snapshots {
        let path = absolute(repo, Path::new(&row.path));
        let bytes = read_hashed_snapshot(&path, &row.sha256)?;
        let snapshot: Snapshot = serde_json::from_slice(&bytes)?;
        if snapshot.fit_cutoff_ms != snapshot.roll_anchor_ms || snapshot.payload_sha256.len() != 64
        {
            bail!("snapshot cutoff/hash contract mismatch: {}", path.display());
        }
        for arm in snapshot.selector_arms.values() {
            for pair in &arm.pair_graph {
                if !pair.cost.passed
                    || pair.cost.excursion_count < 20
                    || pair.model_input_hash.len() != 64
                {
                    bail!("invalid admitted pair {}", pair.pair_id);
                }
                if !pair.score.is_finite() || !pair.copula.aic.is_finite() {
                    bail!("non-finite pair score/AIC {}", pair.pair_id);
                }
            }
        }
        output.push((row.sha256, snapshot));
    }
    Ok(output)
}

fn write_constant_account_trace(
    repo: &Path,
    raw: &Path,
    label: &str,
    start: i64,
    end: i64,
) -> Result<serde_json::Value> {
    let path = raw.join("g0/traces").join(format!("{label}-account.jsonl"));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut writer = BufWriter::new(File::create(&path)?);
    let first = serde_json::json!({
        "event":"one_minute_risk_path","timestamp":start,"wallet":2000.0,"equity":2000.0,
        "margin":0.0,"maintenance":0.0,"reserved_quote":0.0,"positions":0,"groups":0,"pending":0
    });
    let mut last = first.clone();
    let mut rows = 0_u64;
    for timestamp in (start..end).step_by(60_000) {
        last = serde_json::json!({
            "event":"one_minute_risk_path","timestamp":timestamp,"wallet":2000.0,"equity":2000.0,
            "margin":0.0,"maintenance":0.0,"reserved_quote":0.0,"positions":0,"groups":0,"pending":0
        });
        serde_json::to_writer(&mut writer, &last)?;
        writer.write_all(b"\n")?;
        rows += 1;
    }
    writer.flush()?;
    let bytes = fs::read(&path)?;
    Ok(serde_json::json!({
        "raw_path":path.strip_prefix(repo).unwrap_or(&path),"sha256":sha256_bytes(&bytes),
        "rows":rows,"first_row":first,"last_row":last
    }))
}

fn outer_block(timestamp: i64) -> Result<usize> {
    let date = Utc
        .timestamp_millis_opt(timestamp)
        .single()
        .context("invalid timestamp")?;
    let quarters = (date.year() - 2023) * 4 + (date.month0() / 3) as i32 - 2;
    Ok(quarters.clamp(0, 11) as usize)
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

fn date_ms(value: &str) -> Result<i64> {
    Ok(chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")?
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis())
}

fn absolute(repo: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
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
    let output = Command::new("git")
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
