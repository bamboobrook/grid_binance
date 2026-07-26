use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use chrono::{TimeZone, Utc};
use r24_engine::{
    EngineConfig, FillRequest, FrozenLeg, MarketType, MartinGroup, PositionKey, PositionMode,
    SharedAccount, SOFT_LADDER,
};
use r24_research::r25::{gaussian_conditional_h, student_t_conditional_h};
use r24_research::r25_corrected::empirical_cdf;
use r24_research::r26::{read_hashed_snapshot, sha256_bytes};
use r24_research::r27::{pbd_signal, PbdFilterState, PbdSignal};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

const REPO_ARTIFACT: &str = "docs/superpowers/artifacts/glm-martingale-core-round27";
const MARKET_DB: &str = "data/market_data_full.db";
const START_MS: i64 = 1_688_169_600_000;
const END_MS: i64 = 1_780_070_400_000;
const MINUTE_MS: i64 = 60_000;
const WEEK_MS: i64 = 604_800_000;

#[derive(Debug)]
struct Args {
    phase: String,
    artifact_root: PathBuf,
    resume: bool,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    snapshots: Vec<ManifestRow>,
}

#[derive(Debug, Deserialize)]
struct ManifestRow {
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CArm {
    exact_matching: Vec<CPair>,
}

#[derive(Debug, Clone, Deserialize)]
struct CSnapshot {
    frequency: String,
    formation_days: i64,
    roll_anchor_ms: i64,
    arms: BTreeMap<String, CArm>,
}

#[derive(Debug, Clone, Deserialize)]
struct LegModel {
    intercept: f64,
    beta: f64,
    residual_sigma: f64,
    sorted_residuals: Vec<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct CopulaModel {
    family: String,
    rho: f64,
    nu: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct CPair {
    pair_id: String,
    left: String,
    right: String,
    left_leg: LegModel,
    right_leg: LegModel,
    copula: CopulaModel,
    model_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PSnapshot {
    roll_anchor_ms: i64,
    exact_matching: Vec<PPair>,
}

#[derive(Debug, Clone, Deserialize)]
struct PPair {
    pair_id: String,
    left: String,
    right: String,
    rho: f64,
    sigma2_m: f64,
    sigma2_r: f64,
    sigma2_w: f64,
    center: f64,
    finite_component_sigma: f64,
    finite_half_life_bars: f64,
    filtered_state_last: Vec<f64>,
    filtered_state_cov_last: Vec<Vec<f64>>,
    model_hash: String,
}

#[derive(Debug, Clone)]
enum FrozenPair {
    Copula(CPair),
    Pbd(PPair),
}

impl FrozenPair {
    fn id(&self) -> &str {
        match self {
            Self::Copula(value) => &value.pair_id,
            Self::Pbd(value) => &value.pair_id,
        }
    }

    fn symbols(&self) -> (&str, &str) {
        match self {
            Self::Copula(value) => (&value.left, &value.right),
            Self::Pbd(value) => (&value.left, &value.right),
        }
    }

    fn model_hash(&self) -> &str {
        match self {
            Self::Copula(value) => &value.model_hash,
            Self::Pbd(value) => &value.model_hash,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignalPoint {
    timestamp: i64,
    value: f64,
    direction: i8,
    neutral: bool,
}

#[derive(Debug, Clone)]
struct Excursion {
    entry_ms: i64,
    exit_ms: i64,
    direction: i8,
    entry_value: f64,
    points: Vec<SignalPoint>,
    pair: FrozenPair,
    block: usize,
}

#[derive(Debug, Clone, Copy)]
struct MinuteBar {
    timestamp: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct FilterRow {
    symbol: String,
    step_size: f64,
    min_qty: f64,
    min_notional: f64,
}

#[derive(Debug, Clone)]
struct Policy {
    policy_id: String,
    config_id: String,
    family: String,
    entry_alpha: f64,
    so_step: f64,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let started = Utc::now();
    let timer = Instant::now();
    let repo = std::env::current_dir()?;
    let raw = absolute(&repo, &args.artifact_root);
    let artifact = repo.join(REPO_ARTIFACT);
    fs::create_dir_all(&artifact)?;
    match args.phase.as_str() {
        "g0" => run_g0(&repo, &raw, &artifact, args.resume)?,
        "g1-activation" => run_g1(&repo, &raw, &artifact, args.resume)?,
        "g2-replay" => run_g2(&repo, &raw, &artifact, args.resume)?,
        "g3" => run_g3(&raw, &artifact)?,
        other => bail!("unknown phase {other}"),
    }
    write_json(
        raw.join("runtime").join(format!("{}.json", args.phase)),
        &serde_json::json!({
            "argv":std::env::args().collect::<Vec<_>>(),"pid":std::process::id(),
            "start_utc":started.to_rfc3339(),"end_utc":Utc::now().to_rfc3339(),
            "wall_seconds":timer.elapsed().as_secs_f64(),"peak_rss_kib":peak_rss_kib(),
            "exit_code":0,"source_commit":raw.file_name().and_then(|value| value.to_str())
        }),
    )?;
    Ok(())
}

fn run_g0(repo: &Path, raw: &Path, artifact: &Path, resume: bool) -> Result<()> {
    let checkpoint = raw.join("checkpoints/g0.json");
    if resume && checkpoint.exists() {
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&checkpoint)?)?;
        if value["source_commit"].as_str() == raw.file_name().and_then(|value| value.to_str()) {
            return Ok(());
        }
    }
    let c = load_c_snapshots(repo, raw)?;
    let p = load_p_snapshots(repo, raw)?;
    let mut first_pairs = BTreeMap::<String, (i64, FrozenPair, String)>::new();
    for snapshot in &c {
        for (name, arm) in &snapshot.arms {
            if let Some(pair) = arm.exact_matching.first() {
                let config = c_config_id(snapshot, name)?;
                first_pairs.entry(config).or_insert((
                    snapshot.roll_anchor_ms,
                    FrozenPair::Copula(pair.clone()),
                    snapshot.frequency.clone(),
                ));
            }
        }
    }
    for snapshot in &p {
        if let Some(pair) = snapshot.exact_matching.first() {
            first_pairs.entry("P1-5m-7d".into()).or_insert((
                snapshot.roll_anchor_ms,
                FrozenPair::Pbd(pair.clone()),
                "5m".into(),
            ));
        }
    }
    let connection = Connection::open_with_flags(MARKET_DB, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut canaries = Vec::new();
    for (config, (anchor, pair, frequency)) in first_pairs {
        let alpha = if config.starts_with("C0") || config.starts_with("C1") {
            0.20
        } else {
            0.0
        };
        let points = signal_points(
            &connection,
            &pair,
            &frequency,
            anchor,
            anchor + WEEK_MS,
            alpha,
        )?;
        let first = points.iter().find(|point| point.direction != 0).cloned();
        let (order_delta, btc_orders, trace) = if let Some(signal) = &first {
            let mut account = SharedAccount::new(2_000.0, EngineConfig::default())?;
            let before = account.canonical_order_equity_hash();
            let bars = next_bars(
                &connection,
                pair.symbols(),
                signal.timestamp + 1,
                signal.timestamp + 1 + 2 * MINUTE_MS,
            )?;
            if bars.0.is_empty() || bars.1.is_empty() {
                bail!("real canary next-minute bars missing")
            }
            open_pair(
                &mut account,
                &pair,
                signal.direction,
                100.0,
                &bars.0[0],
                &bars.1[0],
                "g0-real",
                0,
            )?;
            close_group(
                &mut account,
                "g0-real",
                &bars.0[1],
                &bars.1[1],
                "g0_canary_flatten",
            )?;
            let after = account.canonical_order_equity_hash();
            (
                before != after,
                0_u64,
                write_engine_trace(repo, raw, &format!("g0-{config}"), &account)?,
            )
        } else {
            (
                false,
                0,
                serde_json::json!({"rows":0,"explicit_no_signal":true}),
            )
        };
        canaries.push(serde_json::json!({
            "config_id":config,"anchor":anchor,"pair_id":pair.id(),"model_hash":pair.model_hash(),
            "first_signal":first,"order_hash_changed":order_delta,"btc_orders":btc_orders,
            "trace_manifest":trace,"selection":"first anchor then pair_id; return blind"
        }));
    }
    let synthetic = synthetic_canary(repo, raw)?;
    let value = serde_json::json!({
        "schema_version":1,"phase":"g0","status":"PASS","source_commit":raw.file_name().and_then(|value| value.to_str()),
        "synthetic":synthetic,"real_canaries":canaries,"btc_reference_only":true
    });
    write_json(&checkpoint, &value)?;
    write_json(artifact.join("gates/g0.json"), &value)?;
    Ok(())
}

fn synthetic_canary(repo: &Path, raw: &Path) -> Result<serde_json::Value> {
    let pair = FrozenPair::Copula(CPair {
        pair_id: "ETHUSDT__SOLUSDT".into(),
        left: "ETHUSDT".into(),
        right: "SOLUSDT".into(),
        left_leg: LegModel {
            intercept: 0.0,
            beta: 1.0,
            residual_sigma: 0.02,
            sorted_residuals: vec![-1.0, 0.0, 1.0],
        },
        right_leg: LegModel {
            intercept: 0.0,
            beta: 1.0,
            residual_sigma: 0.02,
            sorted_residuals: vec![-1.0, 0.0, 1.0],
        },
        copula: CopulaModel {
            family: "gaussian".into(),
            rho: 0.2,
            nu: None,
        },
        model_hash: "synthetic".into(),
    });
    let mut account = SharedAccount::new(2_000.0, EngineConfig::default())?;
    let left = MinuteBar {
        timestamp: 1,
        open: 2_000.0,
        high: 2_000.0,
        low: 2_000.0,
        close: 2_000.0,
    };
    let right = MinuteBar {
        timestamp: 1,
        open: 100.0,
        high: 100.0,
        low: 100.0,
        close: 100.0,
    };
    open_pair(&mut account, &pair, 1, 100.0, &left, &right, "synthetic", 0)?;
    account.record_signal(2, pair.id(), "so_after_loss");
    account
        .groups
        .get_mut("synthetic")
        .unwrap()
        .net_pnl_after_close_cost = -1.0;
    account.request_next_so("synthetic", 125.0)?;
    account.consume_group_reserve_at(2, "synthetic")?;
    close_group(
        &mut account,
        "synthetic",
        &MinuteBar {
            timestamp: 3,
            open: 2_020.0,
            high: 2_020.0,
            low: 2_020.0,
            close: 2_020.0,
        },
        &MinuteBar {
            timestamp: 3,
            open: 99.0,
            high: 99.0,
            low: 99.0,
            close: 99.0,
        },
        "tp",
    )?;
    let trace = write_engine_trace(repo, raw, "g0-synthetic", &account)?;
    Ok(serde_json::json!({
        "fo":1,"so":1,"tp":1,"wallet":account.wallet_balance,"equity":account.equity(),
        "positions":account.positions.len(),"groups":account.groups.len(),"reserve":account.reserved_quote,
        "trace_manifest":trace
    }))
}

fn run_g1(repo: &Path, raw: &Path, artifact: &Path, resume: bool) -> Result<()> {
    let checkpoint = raw.join("checkpoints/g1-activation.json");
    if resume && checkpoint.exists() {
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&checkpoint)?)?;
        if value["source_commit"].as_str() == raw.file_name().and_then(|value| value.to_str()) {
            return Ok(());
        }
    }
    let c = load_c_snapshots(repo, raw)?;
    let p = load_p_snapshots(repo, raw)?;
    let connection = Connection::open_with_flags(MARKET_DB, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut grouped = BTreeMap::<String, Vec<(i64, String, Vec<FrozenPair>)>>::new();
    for snapshot in c {
        for (arm_name, arm) in &snapshot.arms {
            let config = c_config_id(&snapshot, &arm_name)?;
            grouped.entry(config).or_default().push((
                snapshot.roll_anchor_ms,
                snapshot.frequency.clone(),
                arm.exact_matching
                    .iter()
                    .cloned()
                    .map(FrozenPair::Copula)
                    .collect(),
            ));
        }
    }
    for snapshot in p {
        grouped.entry("P1-5m-7d".into()).or_default().push((
            snapshot.roll_anchor_ms,
            "5m".into(),
            snapshot
                .exact_matching
                .into_iter()
                .map(FrozenPair::Pbd)
                .collect(),
        ));
    }
    let expected_configs = [
        "C0-1h-21d-RAW",
        "C0-1h-21d-FDR",
        "C0-5m-21d-RAW",
        "C0-5m-21d-FDR",
        "C1-1h-14d-E0",
        "C1-1h-14d-E1",
        "C1-1h-21d-E0",
        "C1-1h-21d-E1",
        "P1-5m-7d",
    ];
    let mut configs = Vec::new();
    for id in expected_configs {
        let mut rolls = grouped.remove(id).unwrap_or_default();
        rolls.sort_by_key(|row| row.0);
        let mut fitted_alts = BTreeSet::new();
        let mut distinct_pairs = BTreeSet::new();
        let mut crossing_symbols = BTreeMap::<String, u64>::new();
        let mut block_crossings = [0_u64; 12];
        let mut crossings = 0_u64;
        let mut rolls_with_pair = 0;
        let alpha = 0.20;
        for (anchor, frequency, pairs) in &rolls {
            rolls_with_pair += usize::from(!pairs.is_empty());
            for pair in pairs {
                let (left, right) = pair.symbols();
                fitted_alts.insert(left.to_string());
                fitted_alts.insert(right.to_string());
                distinct_pairs.insert(pair.id().to_string());
                let points = signal_points(
                    &connection,
                    pair,
                    frequency,
                    *anchor,
                    *anchor + WEEK_MS,
                    alpha,
                )?;
                let mut armed = true;
                for point in points {
                    if armed && point.direction != 0 {
                        crossings += 1;
                        block_crossings[outer_block(point.timestamp)] += 1;
                        *crossing_symbols.entry(left.into()).or_default() += 1;
                        *crossing_symbols.entry(right.into()).or_default() += 1;
                        armed = false;
                    } else if !armed && point.neutral {
                        armed = true;
                    }
                }
            }
        }
        let max_symbol = crossing_symbols.values().copied().max().unwrap_or(0) as f64
            / crossings.max(1) as f64
            / 2.0;
        let distributed_blocks = block_crossings.iter().filter(|value| **value > 0).count();
        let gates = serde_json::json!({
            "valid_weekly_snapshots":rolls.len(),"distinct_fitted_alts":fitted_alts.len(),
            "distinct_exact_matched_pairs":distinct_pairs.len(),"rolls_with_exact_pair":rolls_with_pair,
            "candidate_fo_crossings":crossings,"outer_blocks_with_crossings":distributed_blocks,
            "max_symbol_crossing_concentration":max_symbol,"model_snapshot_hash_contract_passed":true
        });
        let passed = rolls.len() == 152
            && fitted_alts.len() >= 6
            && distinct_pairs.len() >= 3
            && rolls_with_pair >= 12
            && crossings >= 100
            && distributed_blocks >= 8
            && max_symbol <= 0.50;
        configs.push(serde_json::json!({
            "config_id":id,"passed":passed,"gates":gates,"fitted_alts":fitted_alts,
            "distinct_pairs":distinct_pairs,"block_crossings":block_crossings
        }));
    }
    let priority = [
        "C0-5m-21d-RAW",
        "C0-1h-21d-RAW",
        "C1-1h-21d-E0",
        "C1-1h-14d-E0",
        "C1-1h-21d-E1",
        "C1-1h-14d-E1",
        "P1-5m-7d",
        "C0-5m-21d-FDR",
        "C0-1h-21d-FDR",
    ];
    let survivors = priority
        .iter()
        .filter(|id| {
            configs
                .iter()
                .any(|row| row["config_id"] == **id && row["passed"] == true)
        })
        .take(3)
        .copied()
        .collect::<Vec<_>>();
    let status = if survivors.is_empty() {
        "VALID_ROUND27_ALL_ARMS_NO_ACTIVATION"
    } else {
        "ACTIVATION_SURVIVORS_READY_G2"
    };
    let value = serde_json::json!({
        "schema_version":1,"phase":"g1-activation","status":status,
        "source_commit":raw.file_name().and_then(|value| value.to_str()),
        "config_count":configs.len(),"configs":configs,"survivors":survivors,
        "outer_return_read":false,"model_validator_required_for_final_authority":true
    });
    write_json(&checkpoint, &value)?;
    write_json(artifact.join("gates/g1-activation.json"), &value)?;
    Ok(())
}

fn run_g2(repo: &Path, raw: &Path, artifact: &Path, resume: bool) -> Result<()> {
    let checkpoint = raw.join("checkpoints/g2-replay.json");
    if resume && checkpoint.exists() {
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&checkpoint)?)?;
        if value["source_commit"].as_str() == raw.file_name().and_then(|value| value.to_str()) {
            return Ok(());
        }
    }
    let activation: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g1-activation.json"))?)?;
    let survivors = activation["survivors"]
        .as_array()
        .context("survivors missing")?;
    if survivors.is_empty() {
        let value = serde_json::json!({
            "schema_version":1,"phase":"g2-replay","status":"NOT_APPLICABLE_ZERO_ACTIVATION_SURVIVORS",
            "source_commit":raw.file_name().and_then(|value| value.to_str()),"policy_count":0,"p_b_survivors":[]
        });
        write_json(&checkpoint, &value)?;
        write_json(artifact.join("gates/g2-replay.json"), &value)?;
        return Ok(());
    }
    let c = load_c_snapshots(repo, raw)?;
    let p = load_p_snapshots(repo, raw)?;
    let policies = expand_policies(survivors)?;
    let filters = load_filters(artifact)?;
    let connection = Connection::open_with_flags(MARKET_DB, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut results = Vec::new();
    for policy in policies {
        let excursions = policy_excursions(&connection, &policy, &c, &p)?;
        results.push(replay_policy(
            repo,
            raw,
            &connection,
            &filters,
            &policy,
            excursions,
        )?);
    }
    let p_b_survivors = results
        .iter()
        .filter(|row| row["p_b"] == true)
        .map(|row| row["policy_id"].clone())
        .collect::<Vec<_>>();
    let status = if p_b_survivors.is_empty() {
        "VALID_HISTORICAL_PREQUENTIAL_NO_PB"
    } else {
        "PB_SURVIVORS_READY_G3"
    };
    let value = serde_json::json!({
        "schema_version":1,"phase":"g2-replay","status":status,
        "source_commit":raw.file_name().and_then(|value| value.to_str()),
        "policy_count":results.len(),"results":results,"p_b_survivors":p_b_survivors
    });
    write_json(&checkpoint, &value)?;
    write_json(artifact.join("gates/g2-replay.json"), &value)?;
    Ok(())
}

fn run_g3(raw: &Path, artifact: &Path) -> Result<()> {
    let g2: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g2-replay.json"))?)?;
    let has_pb = g2["p_b_survivors"]
        .as_array()
        .is_some_and(|rows| !rows.is_empty());
    let value = serde_json::json!({
        "schema_version":1,"phase":"g3","status":if has_pb {"CONDITIONAL_G3_REQUIRES_TIER_REPLAY"} else {"NOT_APPLICABLE_NO_PB"},
        "terminal":!has_pb,"p_b_survivor_count":g2["p_b_survivors"].as_array().map_or(0,Vec::len)
    });
    write_json(raw.join("checkpoints/g3.json"), &value)?;
    write_json(artifact.join("gates/g3.json"), &value)?;
    if has_pb {
        bail!("G3 tier replay implementation required for P-B survivors")
    }
    Ok(())
}

fn expand_policies(survivors: &[serde_json::Value]) -> Result<Vec<Policy>> {
    let mut output = Vec::new();
    for value in survivors {
        let config = value.as_str().context("survivor id missing")?;
        if config.starts_with("P1") {
            for step in [0.50, 0.75] {
                output.push(Policy {
                    policy_id: format!("{config}-SO{step:.2}"),
                    config_id: config.into(),
                    family: "P1".into(),
                    entry_alpha: 0.0,
                    so_step: step,
                });
            }
        } else {
            for alpha in [0.10, 0.20] {
                for step in [0.50, 0.75] {
                    output.push(Policy {
                        policy_id: format!("{config}-A{alpha:.2}-SO{step:.2}"),
                        config_id: config.into(),
                        family: if config.starts_with("C0") {
                            "C0".into()
                        } else {
                            "C1".into()
                        },
                        entry_alpha: alpha,
                        so_step: step,
                    });
                }
            }
        }
    }
    Ok(output)
}

fn policy_excursions(
    connection: &Connection,
    policy: &Policy,
    c: &[CSnapshot],
    p: &[PSnapshot],
) -> Result<Vec<Excursion>> {
    let mut output = Vec::new();
    if policy.config_id.starts_with("P1") {
        for snapshot in p {
            for pair in &snapshot.exact_matching {
                output.extend(excursions_from_points(
                    FrozenPair::Pbd(pair.clone()),
                    signal_points(
                        connection,
                        &FrozenPair::Pbd(pair.clone()),
                        "5m",
                        snapshot.roll_anchor_ms,
                        snapshot.roll_anchor_ms + WEEK_MS,
                        0.0,
                    )?,
                ));
            }
        }
    } else {
        for snapshot in c {
            for (arm_name, arm) in &snapshot.arms {
                if c_config_id(snapshot, arm_name)? != policy.config_id {
                    continue;
                }
                for pair in &arm.exact_matching {
                    output.extend(excursions_from_points(
                        FrozenPair::Copula(pair.clone()),
                        signal_points(
                            connection,
                            &FrozenPair::Copula(pair.clone()),
                            &snapshot.frequency,
                            snapshot.roll_anchor_ms,
                            snapshot.roll_anchor_ms + WEEK_MS,
                            policy.entry_alpha,
                        )?,
                    ));
                }
            }
        }
    }
    output.sort_by_key(|row| (row.entry_ms, row.pair.id().to_string()));
    Ok(output)
}

fn excursions_from_points(pair: FrozenPair, points: Vec<SignalPoint>) -> Vec<Excursion> {
    let mut output = Vec::new();
    let mut active: Option<(SignalPoint, Vec<SignalPoint>)> = None;
    let deadline_ms = match &pair {
        FrozenPair::Pbd(model) => {
            ((3.0 * model.finite_half_life_bars).ceil() as i64 * 300_000).min(WEEK_MS)
        }
        FrozenPair::Copula(_) => WEEK_MS,
    };
    for point in points {
        if active.is_none() && point.direction != 0 {
            active = Some((point.clone(), vec![point]));
        } else if let Some((entry, rows)) = active.as_mut() {
            rows.push(point.clone());
            if point.neutral || point.timestamp - entry.timestamp >= deadline_ms {
                let (entry, rows) = active.take().unwrap();
                output.push(Excursion {
                    entry_ms: entry.timestamp,
                    exit_ms: point.timestamp,
                    direction: entry.direction,
                    entry_value: entry.value,
                    points: rows,
                    pair: pair.clone(),
                    block: outer_block(entry.timestamp),
                });
            }
        }
    }
    output
}

fn replay_policy(
    repo: &Path,
    raw: &Path,
    connection: &Connection,
    filters: &BTreeMap<String, FilterRow>,
    policy: &Policy,
    excursions: Vec<Excursion>,
) -> Result<serde_json::Value> {
    let mut config = EngineConfig::default();
    config.max_effective_leverage = 2.0;
    config.leverage = 2.0;
    config.maintenance_rate = 0.025;
    let mut account = SharedAccount::new(2_000.0, config)?;
    let mut last_processed = START_MS;
    let mut fo = 0_u64;
    let mut so = 0_u64;
    let mut tp = 0_u64;
    let mut abort = 0_u64;
    let mut loss_after_add = 0_u64;
    let mut actual_symbols = BTreeSet::new();
    let mut actual_pairs = BTreeSet::new();
    let mut pair_pnl = BTreeMap::<String, f64>::new();
    let mut block_pnl = [0.0_f64; 12];
    let risk_path = raw
        .join("g2/risk-rle")
        .join(format!("{}.jsonl", policy.policy_id));
    fs::create_dir_all(risk_path.parent().unwrap())?;
    let mut writer = BufWriter::new(File::create(&risk_path)?);
    for (sequence, excursion) in excursions.into_iter().enumerate() {
        if excursion.entry_ms < last_processed || account.terminated {
            continue;
        }
        let entry_open_ms = excursion.entry_ms + 1;
        write_idle_span(&mut writer, last_processed, entry_open_ms, &account)?;
        let mut last_risk_timestamp = entry_open_ms - MINUTE_MS;
        let symbols = excursion.pair.symbols();
        let (left, right) = next_bars(
            connection,
            symbols,
            excursion.entry_ms + 1,
            excursion.exit_ms + 2 * MINUTE_MS,
        )?;
        if left.len() < 2 || right.len() < 2 {
            continue;
        }
        let group_id = format!("{}-g{:06}", policy.policy_id, sequence + 1);
        let cycle_start_wallet = account.wallet_balance;
        open_pair_with_filters(
            &mut account,
            &excursion.pair,
            excursion.direction,
            100.0,
            &left[0],
            &right[0],
            &group_id,
            0,
            filters,
        )?;
        fo += 1;
        actual_symbols.insert(symbols.0.to_string());
        actual_symbols.insert(symbols.1.to_string());
        actual_pairs.insert(excursion.pair.id().to_string());
        let mut level = 0_usize;
        let mut last_value = excursion.entry_value;
        let funding = funding_events(
            connection,
            symbols,
            excursion.entry_ms,
            excursion.exit_ms + MINUTE_MS,
        )?;
        let point_map = excursion
            .points
            .iter()
            .map(|row| ((row.timestamp + 1) / MINUTE_MS * MINUTE_MS, row))
            .collect::<BTreeMap<_, _>>();
        let row_count = left.len().min(right.len());
        for index in 0..row_count {
            let closes = BTreeMap::from([
                (symbols.0.to_string(), left[index].close),
                (symbols.1.to_string(), right[index].close),
            ]);
            let highs = BTreeMap::from([
                (symbols.0.to_string(), left[index].high),
                (symbols.1.to_string(), right[index].high),
            ]);
            let lows = BTreeMap::from([
                (symbols.0.to_string(), left[index].low),
                (symbols.1.to_string(), right[index].low),
            ]);
            account.mark_adverse_bar(left[index].timestamp, &lows, &highs, &closes)?;
            if let Some(events) = funding.get(&left[index].timestamp) {
                let keys = account.positions.keys().cloned().collect::<Vec<_>>();
                for (symbol, rate) in events {
                    for key in keys.iter().filter(|key| key.symbol == *symbol) {
                        account.apply_funding(left[index].timestamp, key, *rate)?;
                    }
                }
            }
            write_risk_minute(&mut writer, left[index].timestamp, &account)?;
            last_risk_timestamp = left[index].timestamp;
            if account.terminated {
                break;
            }
            if let Some(point) = point_map.get(&left[index].timestamp) {
                let net = group_net(&account, &group_id);
                if point.neutral && net > 0.0 {
                    close_group(&mut account, &group_id, &left[index], &right[index], "tp")?;
                    tp += 1;
                    break;
                }
                let adverse = excursion.direction as f64 * (last_value - point.value);
                if net < 0.0
                    && point.direction == excursion.direction
                    && adverse >= policy.so_step
                    && level + 1 < SOFT_LADDER.len()
                {
                    level += 1;
                    if let Some(group) = account.groups.get_mut(&group_id) {
                        group.net_pnl_after_close_cost = net;
                    }
                    let next_gross = 100.0 * SOFT_LADDER[level];
                    if account.request_next_so(&group_id, next_gross).is_ok() {
                        account.consume_group_reserve_at(left[index].timestamp, &group_id)?;
                        fill_pair_layer(
                            &mut account,
                            &excursion.pair,
                            excursion.direction,
                            next_gross,
                            &left[index],
                            &right[index],
                            &group_id,
                            level,
                            filters,
                        )?;
                        so += 1;
                        loss_after_add += 1;
                        last_value = point.value;
                    }
                }
            }
        }
        if account.groups.contains_key(&group_id) {
            let index = row_count - 1;
            close_group(
                &mut account,
                &group_id,
                &left[index],
                &right[index],
                "deadline_abort",
            )?;
            abort += 1;
        }
        let cycle_pnl = account.wallet_balance - cycle_start_wallet;
        *pair_pnl.entry(excursion.pair.id().to_string()).or_default() += cycle_pnl;
        block_pnl[excursion.block] += cycle_pnl;
        last_processed = last_risk_timestamp + MINUTE_MS;
    }
    write_idle_span(&mut writer, last_processed, END_MS, &account)?;
    writer.flush()?;
    let years = (END_MS - START_MS) as f64 / (365.25 * 86_400_000.0);
    let compounded = account.equity() / 2_000.0 - 1.0;
    let annualized = ((account.equity() / 2_000.0).max(1e-12).powf(1.0 / years) - 1.0) * 100.0;
    let positive_blocks = block_pnl.iter().filter(|value| **value > 0.0).count();
    let max_symbol = concentration(&account.realized_pnl_by_symbol);
    let max_pair = concentration(&pair_pnl);
    let max_group = concentration(&account.realized_pnl_by_group);
    let max_block = concentration_slice(&block_pnl);
    let cost_ratio =
        account.cost_ledger.all_in_cost() / account.cost_ledger.gross_realized_profit.max(1e-12);
    let p_a = !account.terminated
        && account.positions.is_empty()
        && account.groups.is_empty()
        && account.pending_orders.is_empty()
        && account.reserved_quote.abs() < 1e-9;
    let p_b = p_a
        && compounded > 0.0
        && positive_blocks >= 8
        && tp + abort >= 30
        && actual_symbols.len() >= 6
        && actual_pairs.len() >= 3
        && loss_after_add >= 2
        && max_symbol <= 0.5
        && max_pair <= 0.5
        && max_block <= 0.5
        && cost_ratio <= 0.5;
    let trace = write_engine_trace(repo, raw, &format!("g2-{}", policy.policy_id), &account)?;
    let risk_bytes = fs::read(&risk_path)?;
    Ok(serde_json::json!({
        "policy_id":policy.policy_id,"config_id":policy.config_id,"family":policy.family,
        "entry_alpha":policy.entry_alpha,"so_step":policy.so_step,"principal":2000.0,
        "final_equity":account.equity(),"compounded_return_pct":compounded*100.0,
        "annualized_return_pct":annualized,"max_equity_drawdown_pct":account.max_equity_drawdown_pct,
        "positive_blocks":positive_blocks,"closed_cycles":tp+abort,"fo_count":fo,"so_count":so,
        "loss_after_add_so_groups":loss_after_add,"tp_count":tp,"abort_count":abort,
        "actual_symbols":actual_symbols,"actual_pairs":actual_pairs,"max_symbol_concentration":max_symbol,
        "max_pair_concentration":max_pair,"max_group_concentration":max_group,
        "max_block_concentration":max_block,"block_pnl":block_pnl,
        "cost_to_gross_profit":cost_ratio,"liquidation":account.terminated,"p_a":p_a,"p_b":p_b,
        "final_positions":account.positions.len(),"final_groups":account.groups.len(),
        "final_pending":account.pending_orders.len(),"final_reserve":account.reserved_quote,
        "trace_manifest":trace,"risk_rle_manifest":{"path":risk_path.strip_prefix(repo).unwrap_or(&risk_path),"sha256":sha256_bytes(&risk_bytes),"bytes":risk_bytes.len()}
    }))
}

fn signal_points(
    connection: &Connection,
    pair: &FrozenPair,
    frequency: &str,
    start: i64,
    end: i64,
    alpha: f64,
) -> Result<Vec<SignalPoint>> {
    let interval = if frequency == "1h" {
        3_600_000
    } else {
        300_000
    };
    let (left_symbol, right_symbol) = pair.symbols();
    let left = signal_closes(connection, left_symbol, start, end, interval)?;
    let right = signal_closes(connection, right_symbol, start, end, interval)?;
    if left.len() != right.len() {
        bail!("unaligned outer signal series")
    }
    let mut output = Vec::new();
    match pair {
        FrozenPair::Copula(model) => {
            for ((timestamp, left_price), (right_timestamp, right_price)) in
                left.into_iter().zip(right)
            {
                if timestamp != right_timestamp {
                    bail!("signal timestamp mismatch")
                }
                let btc = signal_close(connection, "BTCUSDT", timestamp, interval)?;
                let left_residual =
                    btc.ln() - model.left_leg.intercept - model.left_leg.beta * left_price.ln();
                let right_residual =
                    btc.ln() - model.right_leg.intercept - model.right_leg.beta * right_price.ln();
                let u = empirical_cdf(&model.left_leg.sorted_residuals, left_residual);
                let v = empirical_cdf(&model.right_leg.sorted_residuals, right_residual);
                let (h_left, h_right) = conditional(&model.copula, u, v);
                let direction = if h_left <= alpha && h_right >= 1.0 - alpha {
                    1
                } else if h_left >= 1.0 - alpha && h_right <= alpha {
                    -1
                } else {
                    0
                };
                let neutral = (0.35..=0.65).contains(&h_left) && (0.35..=0.65).contains(&h_right);
                output.push(SignalPoint {
                    timestamp,
                    value: (left_residual / model.left_leg.residual_sigma)
                        - (right_residual / model.right_leg.residual_sigma),
                    direction,
                    neutral,
                });
            }
        }
        FrozenPair::Pbd(model) => {
            let covariance = [
                [
                    model.filtered_state_cov_last[1][1],
                    model.filtered_state_cov_last[1][0],
                ],
                [
                    model.filtered_state_cov_last[0][1],
                    model.filtered_state_cov_last[0][0],
                ],
            ];
            let mut filter = PbdFilterState {
                rho: model.rho,
                sigma2_m: model.sigma2_m,
                sigma2_r: model.sigma2_r,
                sigma2_w: model.sigma2_w,
                center: model.center,
                m: model.filtered_state_last[1],
                random_walk: model.filtered_state_last[0],
                covariance,
                finite_sigma: model.finite_component_sigma,
            };
            for ((timestamp, left_price), (right_timestamp, right_price)) in
                left.into_iter().zip(right)
            {
                if timestamp != right_timestamp {
                    bail!("PBD timestamp mismatch")
                }
                let z = filter.update(left_price.ln() - right_price.ln());
                let signal = pbd_signal(z);
                let direction = match signal {
                    PbdSignal::FirstOrderLongLeft => 1,
                    PbdSignal::FirstOrderShortLeft => -1,
                    _ => 0,
                };
                output.push(SignalPoint {
                    timestamp,
                    value: z,
                    direction,
                    neutral: signal == PbdSignal::Neutral,
                });
            }
        }
    }
    Ok(output)
}

fn conditional(copula: &CopulaModel, u: f64, v: f64) -> (f64, f64) {
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

fn open_pair(
    account: &mut SharedAccount,
    pair: &FrozenPair,
    direction: i8,
    gross: f64,
    left: &MinuteBar,
    right: &MinuteBar,
    group_id: &str,
    level: usize,
) -> Result<()> {
    let filters = BTreeMap::from([
        (
            pair.symbols().0.to_string(),
            FilterRow {
                symbol: pair.symbols().0.into(),
                step_size: 0.000001,
                min_qty: 0.000001,
                min_notional: 5.0,
            },
        ),
        (
            pair.symbols().1.to_string(),
            FilterRow {
                symbol: pair.symbols().1.into(),
                step_size: 0.000001,
                min_qty: 0.000001,
                min_notional: 5.0,
            },
        ),
    ]);
    open_pair_with_filters(
        account, pair, direction, gross, left, right, group_id, level, &filters,
    )
}

fn open_pair_with_filters(
    account: &mut SharedAccount,
    pair: &FrozenPair,
    direction: i8,
    gross: f64,
    left: &MinuteBar,
    right: &MinuteBar,
    group_id: &str,
    level: usize,
    filters: &BTreeMap<String, FilterRow>,
) -> Result<()> {
    let (left_symbol, right_symbol) = pair.symbols();
    let left_mode = if direction > 0 {
        PositionMode::Long
    } else {
        PositionMode::Short
    };
    let right_mode = if direction > 0 {
        PositionMode::Short
    } else {
        PositionMode::Long
    };
    let left_key = position_key(left_symbol, left_mode, group_id);
    let right_key = position_key(right_symbol, right_mode, group_id);
    account.add_group(MartinGroup {
        group_id: group_id.into(),
        fit_version: pair.model_hash().into(),
        frozen_legs: vec![
            FrozenLeg {
                key: left_key.clone(),
                signed_weight: 0.5,
            },
            FrozenLeg {
                key: right_key.clone(),
                signed_weight: -0.5,
            },
        ],
        level,
        previous_level_gross: 0.0,
        current_level_gross: gross,
        last_filled_group_price: None,
        net_pnl_after_close_cost: 0.0,
        reserved_next_so: 0.0,
    })?;
    fill_pair_layer(
        account, pair, direction, gross, left, right, group_id, level, filters,
    )?;
    account.reserve_group_after_fill(left.timestamp, group_id, Some(gross * 1.25), gross)?;
    Ok(())
}

fn fill_pair_layer(
    account: &mut SharedAccount,
    pair: &FrozenPair,
    direction: i8,
    gross: f64,
    left: &MinuteBar,
    right: &MinuteBar,
    group_id: &str,
    level: usize,
    filters: &BTreeMap<String, FilterRow>,
) -> Result<()> {
    let (ls, rs) = pair.symbols();
    let lm = if direction > 0 {
        PositionMode::Long
    } else {
        PositionMode::Short
    };
    let rm = if direction > 0 {
        PositionMode::Short
    } else {
        PositionMode::Long
    };
    for (symbol, mode, bar, side) in [(ls, lm, left, "left"), (rs, rm, right, "right")] {
        let quantity = resolve_quantity(
            filters.get(symbol).context("filter missing")?,
            bar.open,
            gross / 2.0,
        );
        account.submit(FillRequest {
            timestamp: bar.timestamp,
            order_id: format!("{group_id}-{level}-{side}"),
            group_id: group_id.into(),
            key: position_key(symbol, mode, group_id),
            requested_quantity: quantity,
            price: bar.open,
            fill_fraction: 1.0,
            delayed_bars: 0,
            reject: false,
        })?;
    }
    Ok(())
}

fn close_group(
    account: &mut SharedAccount,
    group_id: &str,
    left: &MinuteBar,
    right: &MinuteBar,
    reason: &str,
) -> Result<()> {
    let keys = account
        .positions
        .keys()
        .filter(|key| key.owner_group == group_id)
        .cloned()
        .collect::<Vec<_>>();
    for key in keys {
        let price = if key.symbol == account.groups[group_id].frozen_legs[0].key.symbol {
            left.open
        } else {
            right.open
        };
        account.close_key(left.timestamp, &key, price, reason)?;
    }
    account.remove_group_at(left.timestamp, group_id)
}

fn group_net(account: &SharedAccount, group_id: &str) -> f64 {
    account
        .positions
        .iter()
        .filter(|(key, _)| key.owner_group == group_id)
        .map(|(key, position)| {
            key.mode.sign() * position.quantity * (position.mark_price - position.average_price)
                - position.quantity
                    * position.mark_price
                    * (account.config.fee_bps + account.config.slippage_bps)
                    / 10_000.0
        })
        .sum()
}
fn position_key(symbol: &str, mode: PositionMode, group: &str) -> PositionKey {
    PositionKey {
        symbol: symbol.into(),
        market_type: MarketType::UsdMPerp,
        mode,
        owner_group: group.into(),
    }
}
fn resolve_quantity(filter: &FilterRow, price: f64, target: f64) -> f64 {
    let needed = target.max(filter.min_notional) / price;
    (needed / filter.step_size)
        .ceil()
        .max((filter.min_qty / filter.step_size).ceil())
        * filter.step_size
}

fn load_c_snapshots(repo: &Path, raw: &Path) -> Result<Vec<CSnapshot>> {
    load_snapshots(repo, raw, "c0-c1")
}
fn load_p_snapshots(repo: &Path, raw: &Path) -> Result<Vec<PSnapshot>> {
    load_snapshots(repo, raw, "p1")
}
fn load_snapshots<T: for<'de> Deserialize<'de>>(
    repo: &Path,
    raw: &Path,
    name: &str,
) -> Result<Vec<T>> {
    let manifest: Manifest = serde_json::from_slice(&fs::read(
        raw.join("fit-snapshot-manifests")
            .join(format!("{name}.json")),
    )?)?;
    manifest
        .snapshots
        .into_iter()
        .map(|row| {
            let bytes = read_hashed_snapshot(&absolute(repo, Path::new(&row.path)), &row.sha256)?;
            Ok(serde_json::from_slice(&bytes)?)
        })
        .collect()
}
fn c_config_id(snapshot: &CSnapshot, arm: &str) -> Result<String> {
    let id = match arm {
        "C0-RAW" => format!("C0-{}-21d-RAW", snapshot.frequency),
        "C0-FDR" => format!("C0-{}-21d-FDR", snapshot.frequency),
        "C1-E0" => format!("C1-1h-{}d-E0", snapshot.formation_days),
        "C1-E1" => format!("C1-1h-{}d-E1", snapshot.formation_days),
        other => bail!("unknown arm {other}"),
    };
    Ok(id)
}

fn signal_closes(
    connection: &Connection,
    symbol: &str,
    start: i64,
    end: i64,
    interval: i64,
) -> Result<Vec<(i64, f64)>> {
    let mut statement=connection.prepare("SELECT close_time,close FROM klines INDEXED BY idx_klines_symbol_time WHERE symbol=?1 AND market_type='futures_usdt_perp' AND timeframe='1m' AND open_time>=?2 AND open_time<?3 AND (open_time % ?4)=?5 ORDER BY open_time")?;
    let rows = statement.query_map(
        (symbol, start, end, interval, interval - MINUTE_MS),
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}
fn signal_close(
    connection: &Connection,
    symbol: &str,
    close_time: i64,
    interval: i64,
) -> Result<f64> {
    let _ = interval;
    let open_time = close_time - (MINUTE_MS - 1);
    Ok(connection.query_row("SELECT close FROM klines INDEXED BY idx_klines_symbol_time WHERE symbol=?1 AND market_type='futures_usdt_perp' AND timeframe='1m' AND open_time=?2",(symbol,open_time),|row|row.get(0))?)
}
fn next_bars(
    connection: &Connection,
    symbols: (&str, &str),
    start: i64,
    end: i64,
) -> Result<(Vec<MinuteBar>, Vec<MinuteBar>)> {
    Ok((
        minute_bars(connection, symbols.0, start, end)?,
        minute_bars(connection, symbols.1, start, end)?,
    ))
}
fn minute_bars(
    connection: &Connection,
    symbol: &str,
    start: i64,
    end: i64,
) -> Result<Vec<MinuteBar>> {
    let start_open = start / MINUTE_MS * MINUTE_MS;
    let mut statement=connection.prepare("SELECT open_time,open,high,low,close FROM klines INDEXED BY idx_klines_symbol_time WHERE symbol=?1 AND market_type='futures_usdt_perp' AND timeframe='1m' AND open_time>=?2 AND open_time<?3 ORDER BY open_time")?;
    let rows = statement.query_map((symbol, start_open, end), |row| {
        Ok(MinuteBar {
            timestamp: row.get(0)?,
            open: row.get(1)?,
            high: row.get(2)?,
            low: row.get(3)?,
            close: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn funding_events(
    connection: &Connection,
    symbols: (&str, &str),
    start: i64,
    end: i64,
) -> Result<BTreeMap<i64, Vec<(String, f64)>>> {
    let funding =
        Connection::open_with_flags("data/funding_rates.db", OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut output = BTreeMap::<i64, Vec<(String, f64)>>::new();
    let _ = connection;
    for symbol in [symbols.0, symbols.1] {
        let mut statement = funding.prepare("SELECT funding_time,funding_rate FROM funding_rates WHERE symbol=?1 AND funding_time>=?2 AND funding_time<?3 ORDER BY funding_time")?;
        let rows = statement.query_map((symbol, start, end), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
        })?;
        for row in rows {
            let (timestamp, rate) = row?;
            output
                .entry(timestamp / MINUTE_MS * MINUTE_MS)
                .or_default()
                .push((symbol.into(), rate));
        }
    }
    Ok(output)
}
fn load_filters(artifact: &Path) -> Result<BTreeMap<String, FilterRow>> {
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(artifact.join("round27-data-manifest.json"))?)?;
    let rows: Vec<FilterRow> = serde_json::from_value(value["exchange_info"]["filters"].clone())?;
    Ok(rows
        .into_iter()
        .map(|row| (row.symbol.clone(), row))
        .collect())
}

fn write_idle_span(
    writer: &mut BufWriter<File>,
    start: i64,
    end: i64,
    account: &SharedAccount,
) -> Result<()> {
    if end > start {
        serde_json::to_writer(
            &mut *writer,
            &serde_json::json!({"kind":"idle_1m_rle","start_ms":start,"end_ms":end,"rows":(end-start)/MINUTE_MS,"wallet":account.wallet_balance,"equity":account.equity(),"reserve":account.reserved_quote}),
        )?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}
fn write_risk_minute(
    writer: &mut BufWriter<File>,
    timestamp: i64,
    account: &SharedAccount,
) -> Result<()> {
    let adverse_equity = account
        .traces
        .iter()
        .rev()
        .find(|row| row.timestamp == timestamp && row.event == "adverse_bar_path")
        .and_then(|row| row.quote)
        .unwrap_or_else(|| account.equity());
    serde_json::to_writer(
        &mut *writer,
        &serde_json::json!({"kind":"active_1m","timestamp":timestamp,"wallet":account.wallet_balance,
            "adverse_equity":adverse_equity,"equity":account.equity(),"reserve":account.reserved_quote,
            "margin":account.initial_margin(),"maintenance":account.maintenance_margin(),
            "positions":account.positions.len(),"groups":account.groups.len(),"pending":account.pending_orders.len()}),
    )?;
    writer.write_all(b"\n")?;
    Ok(())
}
fn write_engine_trace(
    repo: &Path,
    raw: &Path,
    label: &str,
    account: &SharedAccount,
) -> Result<serde_json::Value> {
    let path = raw.join("traces").join(format!("{label}.jsonl"));
    fs::create_dir_all(path.parent().unwrap())?;
    let mut writer = BufWriter::new(File::create(&path)?);
    for row in &account.traces {
        serde_json::to_writer(&mut writer, row)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    let bytes = fs::read(&path)?;
    Ok(
        serde_json::json!({"path":path.strip_prefix(repo).unwrap_or(&path),"sha256":sha256_bytes(&bytes),"rows":account.traces.len()}),
    )
}
fn concentration(values: &BTreeMap<String, f64>) -> f64 {
    let positives = values.values().map(|value| value.max(0.0)).sum::<f64>();
    if positives <= 0.0 {
        0.0
    } else {
        values
            .values()
            .map(|value| value.max(0.0) / positives)
            .fold(0.0, f64::max)
    }
}
fn concentration_slice(values: &[f64]) -> f64 {
    let positives = values.iter().map(|value| value.max(0.0)).sum::<f64>();
    if positives <= 0.0 {
        0.0
    } else {
        values
            .iter()
            .map(|value| value.max(0.0) / positives)
            .fold(0.0, f64::max)
    }
}
fn outer_block(timestamp: i64) -> usize {
    let date = Utc.timestamp_millis_opt(timestamp).single().unwrap();
    let year = date.format("%Y").to_string().parse::<i32>().unwrap();
    let quarter = (date.format("%m").to_string().parse::<usize>().unwrap() - 1) / 3;
    ((year - 2023) as usize * 4 + quarter).min(11)
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
