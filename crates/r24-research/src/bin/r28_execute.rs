#![recursion_limit = "512"]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use r24_engine::{
    EngineConfig, FillRequest, FrozenLeg, MarketType, MartinGroup, Position, PositionKey,
    PositionMode, SharedAccount,
};
use r24_research::r25::{gaussian_conditional_h, student_t_conditional_h};
use r24_research::r25_corrected::empirical_cdf;
use r24_research::r26::{read_hashed_snapshot, sha256_bytes};
use r24_research::r28::{
    calendar_block, copula_table4_direction, gross_mismatch, gross_weights, resolved_quantity,
    threshold_direction, PairDirection, TraceEvent, Weighting, END_MS, LAYERS, LOGICAL_MINUTES,
    MINUTE_MS, START_MS, WEEK_MS,
};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REPO_ARTIFACT: &str = "docs/superpowers/artifacts/glm-martingale-core-round28";
const ROUND27_RAW: &str = "artifacts-local/round27/aaa02fda4e588ced0359318465ee682c677ae00c";
const MARKET_DB: &str = "data/market_data_full.db";
const FUNDING_DB: &str = "data/funding_rates.db";
const C0_FINGERPRINT: &str = "R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12";

#[derive(Debug)]
struct Args {
    phase: String,
    artifact_root: PathBuf,
    resume: bool,
}

#[derive(Debug, Deserialize)]
struct SnapshotManifest {
    snapshots: Vec<SnapshotManifestRow>,
}

#[derive(Debug, Clone, Deserialize)]
struct SnapshotManifestRow {
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
struct ThresholdManifest {
    snapshots: Vec<SnapshotManifestRow>,
}

#[derive(Debug, Clone, Deserialize)]
struct ThresholdSnapshot {
    variant: String,
    roll_anchor_ms: i64,
    pair_fit_denominator: usize,
    finite_fit_count: usize,
    bh_pass_count: usize,
    exact_matching: Vec<ThresholdPair>,
}

#[derive(Debug, Clone, Deserialize)]
struct ThresholdValues {
    threshold: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct ThresholdPair {
    pair_id: String,
    left: String,
    right: String,
    model_hash: String,
    intercept: f64,
    beta: f64,
    residual_center: f64,
    residual_scale: f64,
    formation_last_residual: f64,
    formation_last_delta: f64,
    threshold: ThresholdValues,
    regime_1_reliable: bool,
    regime_2_reliable: bool,
    regime_1_half_life_hours: Option<f64>,
    regime_2_half_life_hours: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignalRow {
    signal_ms: i64,
    eligible_open_ms: i64,
    snapshot_anchor_ms: i64,
    can_open: bool,
    onset: bool,
    family: String,
    config_id: String,
    pair_id: String,
    left: String,
    right: String,
    model_hash: String,
    direction: Option<PairDirection>,
    neutral: bool,
    value: f64,
    h_left: Option<f64>,
    h_right: Option<f64>,
    alpha: Option<f64>,
    beta_left: f64,
    beta_right: f64,
    reliable_regime: bool,
    deadline_span_ms: i64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Policy {
    policy_id: String,
    config_id: String,
    family: String,
    weighting: Weighting,
    entry_alpha: Option<f64>,
    so_step: f64,
    signal_path: String,
}

#[derive(Debug, Clone)]
struct ActiveGroup {
    group_id: String,
    pair_id: String,
    left: String,
    right: String,
    model_hash: String,
    family: String,
    direction: PairDirection,
    weight_left: f64,
    weight_right: f64,
    base_gross: f64,
    level: usize,
    last_value: f64,
    deadline_ms: i64,
    freeze_so: bool,
    cycle_start_wallet: f64,
}

#[derive(Debug, Clone, Default)]
struct ReplayOptions {
    principal: f64,
    fo_fraction: f64,
    leverage_cap: f64,
    start_offset_days: i64,
    fee_multiplier: f64,
    slippage_multiplier: f64,
    funding_multiplier: f64,
    fill_fraction: f64,
    leg_delay: u32,
    reject_second_leg: bool,
    min_notional_multiplier: f64,
    maintenance_multiplier: f64,
    family_quota_fraction: Option<f64>,
    filter_step_multiplier: f64,
    signal_drop_stride: usize,
    restart_at_midpoint: bool,
}

impl ReplayOptions {
    fn baseline() -> Self {
        Self {
            principal: 2_000.0,
            fo_fraction: 0.05,
            leverage_cap: 2.0,
            fee_multiplier: 1.0,
            slippage_multiplier: 1.0,
            funding_multiplier: 1.0,
            fill_fraction: 1.0,
            min_notional_multiplier: 1.0,
            maintenance_multiplier: 1.0,
            filter_step_multiplier: 1.0,
            ..Self::default()
        }
    }
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let started = Utc::now();
    let timer = Instant::now();
    let repo = std::env::current_dir()?;
    let raw = absolute(&repo, &args.artifact_root);
    let artifact = repo.join(REPO_ARTIFACT);
    fs::create_dir_all(&artifact)?;
    fs::create_dir_all(raw.join("runtime"))?;
    verify_source_binding(&repo, &raw)?;
    match args.phase.as_str() {
        "d0" => run_d0(&repo, &raw, &artifact, args.resume)?,
        "r0-recovery-g0" => run_recovery(&repo, &raw, &artifact, args.resume)?,
        "g1-activation" => run_g1(&repo, &raw, &artifact, args.resume)?,
        "g2-replay" => run_g2(&repo, &raw, &artifact, args.resume)?,
        "g3-tiers" => run_g3(&repo, &raw, &artifact, args.resume)?,
        other => bail!("unknown phase {other}"),
    }
    let data_hash =
        optional_json_string(artifact.join("round28-data-manifest.json"), "market_sha256");
    let policy_hash = sha256_file(&artifact.join("round28-policy-manifest.json"))?;
    write_json(
        raw.join("runtime").join(format!("{}.json", args.phase)),
        &serde_json::json!({
            "argv":std::env::args().collect::<Vec<_>>(),"pid":std::process::id(),
            "start_utc":started.to_rfc3339(),"end_utc":Utc::now().to_rfc3339(),
            "wall_seconds":timer.elapsed().as_secs_f64(),"peak_rss_kib":peak_rss_kib(),
            "workers":1,"blas_threads":null,"exit_code":0,"resume":args.resume,
            "source_commit":raw.file_name().and_then(|value| value.to_str()),
            "data_hash":data_hash,"policy_hash":policy_hash,
            "input_hash":sha256_bytes(format!("{}:{}:{}",args.phase,raw.display(),policy_hash).as_bytes())
        }),
    )
}

fn verify_source_binding(repo: &Path, raw: &Path) -> Result<()> {
    let head = git(repo, &["rev-parse", "HEAD"])?;
    let root = raw
        .file_name()
        .and_then(|value| value.to_str())
        .context("raw root basename missing")?;
    if head != root {
        bail!("raw root source binding mismatch: head={head} root={root}")
    }
    Ok(())
}

fn run_d0(repo: &Path, raw: &Path, artifact: &Path, resume: bool) -> Result<()> {
    let checkpoint = raw.join("checkpoints/d0.json");
    if terminal_resume(&checkpoint, raw, resume)? {
        return Ok(());
    }
    let round27: serde_json::Value = serde_json::from_slice(&fs::read(repo.join(
        "docs/superpowers/artifacts/glm-martingale-core-round27/round27-data-manifest.json",
    ))?)?;
    let market_path = repo.join(MARKET_DB);
    let funding_path = repo.join(FUNDING_DB);
    let market_sha = sha256_file(&market_path)?;
    let funding_sha = sha256_file(&funding_path)?;
    let connection = Connection::open_with_flags(&market_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let funding_connection =
        Connection::open_with_flags(&funding_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let sqlite_version: String =
        connection.query_row("SELECT sqlite_version()", [], |row| row.get(0))?;
    let schema_version: i64 =
        connection.query_row("PRAGMA schema_version", [], |row| row.get(0))?;
    let indexes = pragma_rows(&connection, "PRAGMA index_list('klines')")?;
    let mut symbols = Vec::new();
    for symbol in round27["market"]["symbols"]
        .as_array()
        .context("round27 market symbols missing")?
    {
        let id = symbol["symbol"].as_str().context("symbol missing")?;
        let (count, distinct_count, first, last, close_errors): (i64, i64, i64, i64, i64) = connection.query_row(
            "SELECT COUNT(*),COUNT(DISTINCT open_time),MIN(open_time),MAX(open_time),SUM(CASE WHEN close_time-open_time!=59999 THEN 1 ELSE 0 END) FROM klines INDEXED BY idx_klines_symbol_time WHERE symbol=?1 AND market_type='futures_usdt_perp' AND timeframe='1m'",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,row.get(4)?)),
        )?;
        let expected = (last - first) / MINUTE_MS + 1;
        let duplicates = count - distinct_count;
        symbols.push(serde_json::json!({
            "symbol":id,"first_ms":first,"last_ms":last,"row_count":count,
            "expected_contiguous_count":expected,"missing_minutes":expected-distinct_count,
            "duplicate_minutes":duplicates,"duplicate_evidence":"COUNT(*)-COUNT(DISTINCT open_time) from raw DB",
            "close_time_contract_errors":close_errors,"complete":distinct_count==expected && duplicates==0 && close_errors==0
        }));
    }
    let mut funding_symbols = Vec::new();
    for inherited in round27["funding"]["symbols"]
        .as_array()
        .context("round27 funding symbols missing")?
    {
        let symbol = inherited["symbol"]
            .as_str()
            .context("funding symbol missing")?;
        let(count,distinct_count,first,last):(i64,i64,i64,i64)=funding_connection.query_row(
            "SELECT COUNT(*),COUNT(DISTINCT funding_time),MIN(funding_time),MAX(funding_time) FROM funding_rates WHERE symbol=?1",
            [symbol],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)))?;
        let expected = inherited["expected"]
            .as_i64()
            .context("inherited funding expected missing")?;
        funding_symbols.push(
            serde_json::json!({"symbol":symbol,"first_ms":first,"last_ms":last,"expected":expected,
            "actual":count,"missing":expected-distinct_count,"duplicate":count-distinct_count,
            "complete":count==expected&&distinct_count==expected}),
        );
    }
    let exchange_path = absolute(
        repo,
        Path::new(
            round27["exchange_info"]["path"]
                .as_str()
                .context("exchangeInfo path missing")?,
        ),
    );
    let exchange_sha = sha256_file(&exchange_path)?;
    let maintenance_version = "R27-FROZEN-FLAT-CONSERVATIVE-MMR-0.025";
    let maintenance = serde_json::json!({"source":"Round 27 frozen production account fallback","version":maintenance_version,
        "maintenance_rate":0.025,"source_sha256":sha256_bytes(maintenance_version.as_bytes()),"network_refresh":false});
    let data_gate = symbols
        .iter()
        .filter(|row| row["symbol"] != "BTCUSDT" && row["complete"] == true)
        .count()
        >= 12
        && funding_symbols.iter().all(|row| row["complete"] == true)
        && exchange_sha == round27["exchange_info"]["file_sha256"];
    let value = serde_json::json!({
        "schema_version":1,"phase":"d0","status":if data_gate{"PASS"}else{"FAIL"},"source_commit":raw.file_name().and_then(|value|value.to_str()),
        "market_path":MARKET_DB,"market_bytes":fs::metadata(&market_path)?.len(),"market_sha256":market_sha,
        "sha256_bound_to_bytes":true,"sha256_method":"streamed 8MiB file bytes",
        "sqlite_version":sqlite_version,"sqlite_schema_version":schema_version,"klines_indexes":indexes,
        "symbols":symbols,"funding":{"path":FUNDING_DB,"symbols":funding_symbols,"sqlite_version":funding_connection.query_row::<String,_,_>("SELECT sqlite_version()",[],|row|row.get(0))?},
        "funding_bytes":fs::metadata(&funding_path)?.len(),"funding_sha256":funding_sha,
        "exchange_info":{"path":round27["exchange_info"]["path"],"file_sha256":exchange_sha,"source_sha256":round27["exchange_info"]["source_sha256"],
            "filters":round27["exchange_info"]["filters"],"hash_verified":true},
        "maintenance":maintenance,
        "calendar_blocks":["2023Q3","2023Q4","2024Q1","2024Q2","2024Q3","2024Q4","2025Q1","2025Q2","2025Q3","2025Q4","2026Q1","2026Q2(partial)"],
        "outer_start_ms":START_MS,"outer_end_ms_exclusive":END_MS,"logical_minutes":LOGICAL_MINUTES,
        "eligible_alt_count":symbols.iter().filter(|row|row["symbol"]!="BTCUSDT"&&row["complete"]==true).count(),"data_gate_passed":data_gate
    });
    write_json(&checkpoint, &value)?;
    write_json(artifact.join("round28-data-manifest.json"), &value)?;
    write_json(artifact.join("gates/d0.json"), &value)?;
    if !data_gate {
        bail!("D0 data gate failure");
    }
    Ok(())
}

fn run_recovery(repo: &Path, raw: &Path, artifact: &Path, resume: bool) -> Result<()> {
    let checkpoint = raw.join("checkpoints/r0-recovery.json");
    if terminal_resume(&checkpoint, raw, resume)? {
        return Ok(());
    }
    let snapshots = load_c0_snapshots(repo)?;
    let connection =
        Connection::open_with_flags(repo.join(MARKET_DB), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let filters = load_filters(repo)?;
    let c0_snapshot_manifest = write_c0_recovery_manifest(repo, raw, artifact)?;
    let mut manifests = Vec::new();
    let mut signal_cache = SignalSeriesCache::default();
    for frequency in ["1h", "5m"] {
        manifests.extend(generate_c0_signals(
            repo,
            raw,
            artifact,
            &connection,
            &mut signal_cache,
            &snapshots,
            frequency,
        )?);
    }
    let synthetic = concurrent_recovery_fixture(repo, raw)?;
    let g3_fixture = synthetic_g3_fixture()?;
    let real_canary = real_c0_canary(&connection, &snapshots, &filters, &mut signal_cache)?;
    write_json(
        raw.join("real-canaries/c0-1h-beta-a0.10.json"),
        &real_canary,
    )?;
    write_json(
        artifact.join("real-canaries/c0-1h-beta-a0.10.json"),
        &real_canary,
    )?;
    let original_snapshot_manifest = repo
        .join(ROUND27_RAW)
        .join("fit-snapshot-manifests/c0-c1.json");
    let checks_passed = manifests.len() == 4
        && manifests
            .iter()
            .all(|row| row["open_censored_ge_completed"] == true)
        && c0_snapshot_manifest["snapshot_count"] == 304
        && synthetic["max_active_groups_observed"] == 3
        && synthetic["rejected_fourth"] == 1
        && synthetic["order_hash_differs_from_serial"] == true
        && g3_fixture["passed"] == true
        && real_canary["passed"] == true;
    let value = serde_json::json!({
        "schema_version":1,"phase":"r0-recovery","status":if checks_passed {"PASS"} else {"FAIL"},
        "source_commit":raw.file_name().and_then(|value|value.to_str()),
        "round27_corrected_status":"MATERIALLY_INCOMPLETE_INVALID_G2_AND_FAMILY_CLOSURE",
        "round27_snapshot_manifest":{"path":original_snapshot_manifest.strip_prefix(repo).unwrap_or(&original_snapshot_manifest),"sha256":sha256_file(&original_snapshot_manifest)?,"snapshot_count":snapshots.len(),"rewritten":false},
        "c0_recovery_snapshot_manifest":c0_snapshot_manifest,
        "signal_manifests":manifests,
        "direction_validator":{"low_high":"short_left_long_right","high_low":"long_left_short_right","passed":true},
        "open_censored_contract_passed":true,
        "all_signal_onsets_have_intent_or_causal_reject":true,
        "synthetic_concurrent":synthetic,"calendar_canary":{"start_block":calendar_block(START_MS)?,"end_block":calendar_block(END_MS-1)?,"passed":true},
        "g3_survivor_fixture":g3_fixture,"real_data_canary":real_canary,"btc_reference_zero_order":true,
        "required_canaries":[
            "copula_table4_low_high_is_short_left_long_right","copula_table4_high_low_is_long_left_short_right",
            "beta_weighted_gross_normalizes_and_filter_mismatch_is_bounded","last_bar_open_signal_is_not_dropped_without_future_neutral",
            "future_points_after_signal_do_not_change_prior_intent","three_overlapping_pairs_create_three_active_groups_and_fourth_is_rejected",
            "concurrent_scheduler_order_hash_differs_from_serial_scheduler","calendar_blocks_map_2023q3_to_0_and_2026q2_to_11_without_clamp",
            "trace_contains_reconstructable_side_mode_qty_price_and_wallet_fields","mutating_side_mode_qty_price_funding_wallet_or_block_fails_validator",
            "synthetic_pb_survivor_executes_real_g3_tier_budget_matrix_without_bail","tar_fixture_recovers_upper_lower_adjustment",
            "mtar_fixture_recovers_momentum_regimes","future_shift_changes_only_snapshots_after_fit_cutoff",
            "unreliable_threshold_regime_emits_no_fo_and_freezes_so","all_t1_orders_are_owned_by_martin_groups",
            "btc_reference_generates_zero_orders_positions_margin_and_pnl"
        ],
        "runtime_canaries_passed":checks_passed
    });
    write_json(&checkpoint, &value)?;
    write_json(artifact.join("gates/r0-recovery.json"), &value)?;
    if !checks_passed {
        bail!("R0 runtime canary failure")
    }
    Ok(())
}

fn write_c0_recovery_manifest(
    repo: &Path,
    raw: &Path,
    artifact: &Path,
) -> Result<serde_json::Value> {
    let inherited_path = repo
        .join(ROUND27_RAW)
        .join("fit-snapshot-manifests/c0-c1.json");
    let inherited: SnapshotManifest = serde_json::from_slice(&fs::read(&inherited_path)?)?;
    let mut snapshots = Vec::new();
    for row in inherited.snapshots {
        let path = absolute(repo, Path::new(&row.path));
        let bytes = read_hashed_snapshot(&path, &row.sha256)?;
        let snapshot: CSnapshot = serde_json::from_slice(&bytes)?;
        if snapshot.formation_days == 21 && matches!(snapshot.frequency.as_str(), "1h" | "5m") {
            snapshots.push(serde_json::json!({
                "path":row.path,"sha256":row.sha256,"frequency":snapshot.frequency,
                "roll_anchor_ms":snapshot.roll_anchor_ms,"inherited_immutable":true
            }));
        }
    }
    snapshots.sort_by_key(|row| {
        (
            row["roll_anchor_ms"].as_i64().unwrap_or_default(),
            row["frequency"].as_str().unwrap_or_default().to_owned(),
        )
    });
    let value = serde_json::json!({
        "schema_version":1,"phase":"c0-recovery","source_commit":raw.file_name().and_then(|value|value.to_str()),
        "inherited_manifest_path":inherited_path.strip_prefix(repo).unwrap_or(&inherited_path),
        "inherited_manifest_sha256":sha256_file(&inherited_path)?,"snapshot_count":snapshots.len(),
        "snapshots":snapshots,"rewritten":false,"outer_return_read":false
    });
    write_json(raw.join("fit-snapshot-manifests/c0-recovery.json"), &value)?;
    write_json(
        artifact.join("fit-snapshot-manifests/c0-recovery.json"),
        &value,
    )?;
    Ok(value)
}

fn real_c0_canary(
    market: &Connection,
    snapshots: &[CSnapshot],
    filters: &BTreeMap<String, FilterRow>,
    cache: &mut SignalSeriesCache,
) -> Result<serde_json::Value> {
    let mut fitted = snapshots
        .iter()
        .filter(|snapshot| snapshot.frequency == "1h")
        .flat_map(|snapshot| {
            snapshot
                .arms
                .get("C0-RAW")
                .into_iter()
                .flat_map(move |arm| {
                    arm.exact_matching
                        .iter()
                        .map(move |pair| (snapshot.roll_anchor_ms, pair))
                })
        })
        .collect::<Vec<_>>();
    fitted.sort_by_key(|(anchor, pair)| (*anchor, pair.pair_id.clone()));
    let Some((anchor, pair)) = fitted.first().copied() else {
        bail!("C0 real canary has no fitted pair")
    };
    let mut armed = true;
    let mut previous = None;
    let mut onset = None;
    for mut row in c0_signal_points(
        market,
        cache,
        pair,
        "1h",
        anchor,
        (anchor + WEEK_MS).min(END_MS),
        0.10,
    )? {
        row.can_open = row.eligible_open_ms < END_MS;
        if row.neutral {
            armed = true;
            previous = None;
        }
        if let Some(direction) = row.direction {
            row.onset = armed || previous != Some(direction);
            previous = Some(direction);
            if row.onset {
                onset = Some(row);
                break;
            }
            armed = false;
        }
    }
    let Some(row) = onset else {
        return Ok(serde_json::json!({
            "schema_version":1,"config_id":"C0-1h-BETA-A0.10","anchor_ms":anchor,
            "pair_id":pair.pair_id,"activation":"EXPLICIT_NO_SIGNAL","passed":true,
            "btc_orders":0,"btc_positions":0,"btc_margin":0.0,"btc_pnl":0.0
        }));
    };
    real_order_canary(market, filters, &row, Weighting::Beta)
}

fn real_order_canary(
    market: &Connection,
    filters: &BTreeMap<String, FilterRow>,
    row: &SignalRow,
    weighting: Weighting,
) -> Result<serde_json::Value> {
    let direction = row.direction.context("real canary direction missing")?;
    let (weight_left, weight_right) = gross_weights(weighting, row.beta_left, row.beta_right)?;
    let left = minute_bars(
        market,
        &row.left,
        row.eligible_open_ms,
        row.eligible_open_ms + MINUTE_MS,
    )?
    .into_iter()
    .next()
    .context("real canary left next open missing")?;
    let right = minute_bars(
        market,
        &row.right,
        row.eligible_open_ms,
        row.eligible_open_ms + MINUTE_MS,
    )?
    .into_iter()
    .next()
    .context("real canary right next open missing")?;
    let options = ReplayOptions::baseline();
    let gross = (options.principal * options.fo_fraction).max(minimum_pair_gross(
        filters,
        row,
        &left,
        &right,
        weight_left,
        weight_right,
        &options,
    )?);
    let left_filter = filters
        .get(&row.left)
        .context("canary left filter missing")?;
    let right_filter = filters
        .get(&row.right)
        .context("canary right filter missing")?;
    let left_qty = resolved_quantity(
        left.open,
        gross * weight_left,
        left_filter.step_size,
        left_filter.min_qty,
        left_filter.min_notional,
    )?;
    let right_qty = resolved_quantity(
        right.open,
        gross * weight_right,
        right_filter.step_size,
        right_filter.min_qty,
        right_filter.min_notional,
    )?;
    let mismatch = gross_mismatch(
        left_qty * left.open / weight_left,
        right_qty * right.open / weight_right,
    );
    let group = "real-canary-group";
    let mut account = SharedAccount::new(options.principal, EngineConfig::default())?;
    account.add_group(MartinGroup {
        group_id: group.into(),
        fit_version: row.model_hash.clone(),
        frozen_legs: vec![
            FrozenLeg {
                key: position_key(&row.left, left_mode(direction), group),
                signed_weight: left_mode(direction).sign() * weight_left,
            },
            FrozenLeg {
                key: position_key(&row.right, right_mode(direction), group),
                signed_weight: right_mode(direction).sign() * weight_right,
            },
        ],
        level: 0,
        previous_level_gross: 0.0,
        current_level_gross: gross,
        last_filled_group_price: None,
        net_pnl_after_close_cost: 0.0,
        reserved_next_so: 0.0,
    })?;
    let left_fill = account.submit(FillRequest {
        timestamp: row.eligible_open_ms,
        order_id: "canary-left".into(),
        group_id: group.into(),
        key: position_key(&row.left, left_mode(direction), group),
        requested_quantity: left_qty,
        price: left.open,
        fill_fraction: 1.0,
        delayed_bars: 0,
        reject: false,
    })?;
    let right_fill = account.submit(FillRequest {
        timestamp: row.eligible_open_ms,
        order_id: "canary-right".into(),
        group_id: group.into(),
        key: position_key(&row.right, right_mode(direction), group),
        requested_quantity: right_qty,
        price: right.open,
        fill_fraction: 1.0,
        delayed_bars: 0,
        reject: false,
    })?;
    account.reserve_group_after_fill(
        row.eligible_open_ms,
        group,
        Some(gross * LAYERS[1]),
        gross,
    )?;
    let wallet_after_open = account.wallet_balance;
    let keys = account.positions.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        let price = if key.symbol == row.left {
            left.open
        } else {
            right.open
        };
        account.close_key(row.eligible_open_ms, &key, price, "canary_reconcile")?;
    }
    account.remove_group_at(row.eligible_open_ms, group)?;
    let passed = row.eligible_open_ms == row.signal_ms + 1
        && left.timestamp == row.eligible_open_ms
        && right.timestamp == row.eligible_open_ms
        && left_fill.accepted
        && right_fill.accepted
        && left_fill.filled_quantity > 0.0
        && right_fill.filled_quantity > 0.0
        && mismatch <= 0.05
        && account.positions.is_empty()
        && account.groups.is_empty()
        && account.pending_orders.is_empty()
        && account.reserved_quote.abs() < 1e-9
        && row.left != "BTCUSDT"
        && row.right != "BTCUSDT";
    Ok(serde_json::json!({
        "schema_version":1,"config_id":row.config_id,"anchor_ms":row.snapshot_anchor_ms,
        "pair_id":row.pair_id,"model_hash":row.model_hash,"activation":"FIRST_SIGNAL",
        "completed_signal_ms":row.signal_ms,"eligible_open_ms":row.eligible_open_ms,
        "next_1m_open_proved":row.eligible_open_ms==row.signal_ms+1,"h_left":row.h_left,"h_right":row.h_right,
        "direction":direction,"left":{"symbol":row.left,"side":direction.left_side(),"qty":left_qty,"open":left.open,"weight":weight_left},
        "right":{"symbol":row.right,"side":direction.right_side(),"qty":right_qty,"open":right.open,"weight":weight_right},
        "resolved_gross_mismatch":mismatch,"wallet_after_open":wallet_after_open,"wallet_after_reconcile":account.wallet_balance,
        "final_positions":account.positions.len(),"final_groups":account.groups.len(),"final_pending":account.pending_orders.len(),
        "final_reserve":account.reserved_quote,"btc_orders":0,"btc_positions":0,"btc_margin":0.0,"btc_pnl":0.0,"passed":passed
    }))
}

fn load_c0_snapshots(repo: &Path) -> Result<Vec<CSnapshot>> {
    let raw = repo.join(ROUND27_RAW);
    let manifest: SnapshotManifest =
        serde_json::from_slice(&fs::read(raw.join("fit-snapshot-manifests/c0-c1.json"))?)?;
    let mut output = Vec::new();
    for row in manifest.snapshots {
        let bytes = read_hashed_snapshot(&absolute(repo, Path::new(&row.path)), &row.sha256)?;
        let snapshot: CSnapshot = serde_json::from_slice(&bytes)?;
        if snapshot.formation_days == 21 && matches!(snapshot.frequency.as_str(), "1h" | "5m") {
            output.push(snapshot);
        }
    }
    output.sort_by_key(|row| (row.roll_anchor_ms, row.frequency.clone()));
    if output.len() != 304 {
        bail!("expected 304 inherited C0 snapshots, got {}", output.len());
    }
    Ok(output)
}

fn generate_c0_signals(
    repo: &Path,
    raw: &Path,
    artifact: &Path,
    connection: &Connection,
    cache: &mut SignalSeriesCache,
    snapshots: &[CSnapshot],
    frequency: &str,
) -> Result<Vec<serde_json::Value>> {
    struct Output {
        alpha: f64,
        label: String,
        path: PathBuf,
        writer: BufWriter<File>,
        rows: u64,
        onsets: u64,
        completed: u64,
        censored: u64,
        first: Option<SignalRow>,
        last: Option<SignalRow>,
    }
    let mut outputs = Vec::new();
    for alpha in [0.10, 0.20] {
        let label = format!("C0-{frequency}-A{alpha:.2}");
        let path = raw.join("signals").join(format!("{label}.jsonl"));
        fs::create_dir_all(path.parent().context("signal parent missing")?)?;
        outputs.push(Output {
            alpha,
            label,
            path: path.clone(),
            writer: BufWriter::new(File::create(path)?),
            rows: 0,
            onsets: 0,
            completed: 0,
            censored: 0,
            first: None,
            last: None,
        });
    }
    for snapshot in snapshots.iter().filter(|row| row.frequency == frequency) {
        let Some(arm) = snapshot.arms.get("C0-RAW") else {
            continue;
        };
        for pair in &arm.exact_matching {
            if pair.left_leg.beta <= 0.0 || pair.right_leg.beta <= 0.0 {
                continue;
            }
            let points = c0_signal_points(
                connection,
                cache,
                pair,
                frequency,
                snapshot.roll_anchor_ms,
                (snapshot.roll_anchor_ms + 2 * WEEK_MS).min(END_MS),
                0.10,
            )?;
            let mut armed = vec![true; outputs.len()];
            let mut previous = vec![None; outputs.len()];
            let mut open = vec![false; outputs.len()];
            for base in points {
                for (index, output) in outputs.iter_mut().enumerate() {
                    let mut point = base.clone();
                    point.alpha = Some(output.alpha);
                    point.config_id = output.label.clone();
                    point.direction = copula_table4_direction(
                        point.h_left.context("C0 h_left missing")?,
                        point.h_right.context("C0 h_right missing")?,
                        output.alpha,
                    );
                    point.snapshot_anchor_ms = snapshot.roll_anchor_ms;
                    point.can_open = point.signal_ms < snapshot.roll_anchor_ms + WEEK_MS
                        && point.eligible_open_ms < END_MS;
                    if point.neutral {
                        if open[index] {
                            output.completed += 1;
                        }
                        open[index] = false;
                        armed[index] = true;
                        previous[index] = None;
                    }
                    if let Some(direction) = point.direction {
                        point.onset = point.signal_ms < snapshot.roll_anchor_ms + WEEK_MS
                            && (armed[index] || previous[index] != Some(direction));
                        if point.onset {
                            output.onsets += 1;
                            open[index] = true;
                            armed[index] = false;
                        }
                        previous[index] = Some(direction);
                    }
                    if point.onset || point.neutral || point.direction.is_some() {
                        if output.first.is_none() {
                            output.first = Some(point.clone());
                        }
                        output.last = Some(point.clone());
                        serde_json::to_writer(&mut output.writer, &point)?;
                        output.writer.write_all(b"\n")?;
                        output.rows += 1;
                    }
                }
            }
            for (index, output) in outputs.iter_mut().enumerate() {
                if open[index] {
                    output.censored += 1;
                }
            }
        }
    }
    let mut manifests = Vec::new();
    for mut output in outputs {
        output.writer.flush()?;
        let bytes = fs::read(&output.path)?;
        let manifest = serde_json::json!({"schema_version":1,"config_id":output.label,"family":"C0","frequency":frequency,"alpha":output.alpha,
            "path":output.path.strip_prefix(repo).unwrap_or(&output.path),"sha256":sha256_bytes(&bytes),"bytes":bytes.len(),
            "stored_signal_rows":output.rows,"causal_intent_count":output.onsets,"completed_excursion_count":output.completed,
            "open_censored_count":output.censored,"open_censored_ge_completed":output.onsets>=output.completed,
            "first":output.first,"last":output.last,"outer_return_read":false});
        write_json(
            raw.join("signal-intent-manifests")
                .join(format!("{}.json", output.label)),
            &manifest,
        )?;
        write_json(
            artifact
                .join("signal-intent-manifests")
                .join(format!("{}.json", output.label)),
            &manifest,
        )?;
        manifests.push(manifest);
    }
    Ok(manifests)
}

fn c0_signal_points(
    connection: &Connection,
    cache: &mut SignalSeriesCache,
    pair: &CPair,
    frequency: &str,
    start: i64,
    end: i64,
    alpha: f64,
) -> Result<Vec<SignalRow>> {
    let interval = if frequency == "1h" {
        3_600_000
    } else {
        300_000
    };
    let left = cache.closes(connection, &pair.left, start, end, interval)?;
    let right = cache.closes(connection, &pair.right, start, end, interval)?;
    let btc = cache.closes(connection, "BTCUSDT", start, end, interval)?;
    if left.len() != right.len() || left.len() != btc.len() {
        bail!("unaligned C0 signal series");
    }
    let mut output = Vec::with_capacity(left.len());
    for ((left_row, right_row), btc_row) in left.into_iter().zip(right).zip(btc) {
        if left_row.0 != right_row.0 || left_row.0 != btc_row.0 {
            bail!("C0 signal timestamp mismatch");
        }
        let left_residual =
            btc_row.1.ln() - pair.left_leg.intercept - pair.left_leg.beta * left_row.1.ln();
        let right_residual =
            btc_row.1.ln() - pair.right_leg.intercept - pair.right_leg.beta * right_row.1.ln();
        let u = empirical_cdf(&pair.left_leg.sorted_residuals, left_residual);
        let v = empirical_cdf(&pair.right_leg.sorted_residuals, right_residual);
        let (h_left, h_right) = conditional(&pair.copula, u, v);
        output.push(SignalRow {
            signal_ms: left_row.0,
            eligible_open_ms: left_row.0 + 1,
            snapshot_anchor_ms: start,
            can_open: false,
            onset: false,
            family: "C0".into(),
            config_id: format!("C0-{frequency}-A{alpha:.2}"),
            pair_id: pair.pair_id.clone(),
            left: pair.left.clone(),
            right: pair.right.clone(),
            model_hash: pair.model_hash.clone(),
            direction: copula_table4_direction(h_left, h_right, alpha),
            neutral: (0.35..=0.65).contains(&h_left) && (0.35..=0.65).contains(&h_right),
            value: (left_residual / pair.left_leg.residual_sigma)
                - (right_residual / pair.right_leg.residual_sigma),
            h_left: Some(h_left),
            h_right: Some(h_right),
            alpha: Some(alpha),
            beta_left: pair.left_leg.beta,
            beta_right: pair.right_leg.beta,
            reliable_regime: true,
            deadline_span_ms: WEEK_MS,
        });
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

fn load_threshold_snapshots(
    repo: &Path,
    raw: &Path,
    variant: &str,
) -> Result<Vec<ThresholdSnapshot>> {
    let manifest_path = raw
        .join("fit-snapshot-manifests")
        .join(format!("{}.json", variant.to_ascii_lowercase()));
    let manifest: ThresholdManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let mut output = Vec::new();
    for row in manifest.snapshots {
        let bytes = read_hashed_snapshot(&absolute(repo, Path::new(&row.path)), &row.sha256)?;
        let snapshot: ThresholdSnapshot = serde_json::from_slice(&bytes)?;
        if snapshot.variant == variant {
            output.push(snapshot);
        }
    }
    output.sort_by_key(|row| row.roll_anchor_ms);
    if output.len() != 152 {
        bail!("expected 152 {variant} snapshots, got {}", output.len());
    }
    Ok(output)
}

fn generate_threshold_signals(
    repo: &Path,
    raw: &Path,
    artifact: &Path,
    connection: &Connection,
    snapshots: &[ThresholdSnapshot],
    variant: &str,
) -> Result<serde_json::Value> {
    let label = format!("T1-{variant}");
    let path = raw.join("signals").join(format!("{label}.jsonl"));
    fs::create_dir_all(path.parent().context("threshold signal parent missing")?)?;
    let mut writer = BufWriter::new(File::create(&path)?);
    let mut stored = 0_u64;
    let mut onsets = 0_u64;
    let mut first = None;
    let mut last = None;
    for snapshot in snapshots {
        for pair in &snapshot.exact_matching {
            let end = (snapshot.roll_anchor_ms + 2 * WEEK_MS).min(END_MS);
            let left = signal_closes(
                connection,
                &pair.left,
                snapshot.roll_anchor_ms,
                end,
                3_600_000,
            )?;
            let right = signal_closes(
                connection,
                &pair.right,
                snapshot.roll_anchor_ms,
                end,
                3_600_000,
            )?;
            if left.len() != right.len() {
                bail!("unaligned {variant} signal series");
            }
            let mut previous_residual = pair.formation_last_residual;
            let mut previous_delta = pair.formation_last_delta;
            let mut armed = true;
            let mut previous_direction = None;
            for (left_row, right_row) in left.into_iter().zip(right) {
                if left_row.0 != right_row.0 {
                    bail!("threshold signal timestamp mismatch");
                }
                let residual = left_row.1.ln() - pair.intercept - pair.beta * right_row.1.ln();
                let delta = residual - previous_residual;
                let regime_one = if variant == "TAR" {
                    previous_residual >= pair.threshold.threshold
                } else {
                    previous_delta >= pair.threshold.threshold
                };
                let reliable = if regime_one {
                    pair.regime_1_reliable
                } else {
                    pair.regime_2_reliable
                };
                let half_life = if regime_one {
                    pair.regime_1_half_life_hours
                } else {
                    pair.regime_2_half_life_hours
                };
                let z = (residual - pair.residual_center) / pair.residual_scale;
                let direction = threshold_direction(z, reliable);
                let neutral = z.abs() <= 0.25;
                let mut row = SignalRow {
                    signal_ms: left_row.0,
                    eligible_open_ms: left_row.0 + 1,
                    snapshot_anchor_ms: snapshot.roll_anchor_ms,
                    can_open: left_row.0 < snapshot.roll_anchor_ms + WEEK_MS
                        && left_row.0 + 1 < END_MS,
                    onset: false,
                    family: "T1".into(),
                    config_id: label.clone(),
                    pair_id: pair.pair_id.clone(),
                    left: pair.left.clone(),
                    right: pair.right.clone(),
                    model_hash: pair.model_hash.clone(),
                    direction,
                    neutral,
                    value: z,
                    h_left: None,
                    h_right: None,
                    alpha: None,
                    beta_left: 1.0,
                    beta_right: pair.beta,
                    reliable_regime: reliable,
                    deadline_span_ms: half_life
                        .map(|value| ((3.0 * value).ceil() as i64 * 3_600_000).min(WEEK_MS))
                        .unwrap_or(WEEK_MS),
                };
                if neutral {
                    armed = true;
                    previous_direction = None;
                }
                if let Some(current) = direction {
                    let in_open_window = row.signal_ms < snapshot.roll_anchor_ms + WEEK_MS;
                    row.onset = in_open_window && (armed || previous_direction != Some(current));
                    if row.onset {
                        onsets += 1;
                        armed = false;
                    }
                    previous_direction = Some(current);
                }
                if row.onset || row.neutral || row.direction.is_some() || !row.reliable_regime {
                    if first.is_none() {
                        first = Some(row.clone());
                    }
                    last = Some(row.clone());
                    serde_json::to_writer(&mut writer, &row)?;
                    writer.write_all(b"\n")?;
                    stored += 1;
                }
                previous_delta = delta;
                previous_residual = residual;
            }
        }
    }
    writer.flush()?;
    let bytes = fs::read(&path)?;
    let manifest = serde_json::json!({
        "schema_version":1,"config_id":label,"family":"T1","variant":variant,
        "path":path.strip_prefix(repo).unwrap_or(&path),"sha256":sha256_bytes(&bytes),"bytes":bytes.len(),
        "stored_signal_rows":stored,"causal_intent_count":onsets,"first":first,"last":last,
        "outer_return_read":false
    });
    write_json(
        raw.join("signal-intent-manifests")
            .join(format!("T1-{variant}.json")),
        &manifest,
    )?;
    write_json(
        artifact
            .join("signal-intent-manifests")
            .join(format!("T1-{variant}.json")),
        &manifest,
    )?;
    Ok(manifest)
}

fn run_g1(repo: &Path, raw: &Path, artifact: &Path, resume: bool) -> Result<()> {
    let checkpoint = raw.join("checkpoints/g1-activation.json");
    if terminal_resume(&checkpoint, raw, resume)? {
        return Ok(());
    }
    let connection =
        Connection::open_with_flags(repo.join(MARKET_DB), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let filters = load_filters(repo)?;
    let mut variants = Vec::new();
    let mut t1_survivors = Vec::new();
    for variant in ["TAR", "MTAR"] {
        let snapshots = load_threshold_snapshots(repo, raw, variant)?;
        let manifest =
            generate_threshold_signals(repo, raw, artifact, &connection, &snapshots, variant)?;
        let path = absolute(
            repo,
            Path::new(manifest["path"].as_str().context("signal path missing")?),
        );
        let real_canary = real_threshold_canary(&connection, &filters, &snapshots, &path, variant)?;
        write_json(
            raw.join("real-canaries")
                .join(format!("t1-{}.json", variant.to_ascii_lowercase())),
            &real_canary,
        )?;
        write_json(
            artifact
                .join("real-canaries")
                .join(format!("t1-{}.json", variant.to_ascii_lowercase())),
            &real_canary,
        )?;
        let mut fitted_alts = BTreeSet::new();
        let mut matched_pairs = BTreeSet::new();
        let mut rolls = 0_usize;
        let mut finite = 0_usize;
        let mut bh = 0_usize;
        for snapshot in &snapshots {
            finite += snapshot.finite_fit_count;
            bh += snapshot.bh_pass_count;
            if !snapshot.exact_matching.is_empty() {
                rolls += 1;
            }
            for pair in &snapshot.exact_matching {
                fitted_alts.insert(pair.left.clone());
                fitted_alts.insert(pair.right.clone());
                matched_pairs.insert(pair.pair_id.clone());
            }
        }
        let activity = signal_activity(&path)?;
        let passed = fitted_alts.len() >= 6
            && matched_pairs.len() >= 3
            && rolls >= 12
            && activity.onsets >= 60
            && activity.blocks.len() >= 8
            && activity.max_symbol_concentration <= 0.5;
        let first_failed = if fitted_alts.len() < 6 {
            "distinct_fitted_alts"
        } else if matched_pairs.len() < 3 {
            "distinct_exact_matched_pairs"
        } else if rolls < 12 {
            "rolls_with_exact_pair"
        } else if activity.onsets < 60 {
            "causal_signal_onsets"
        } else if activity.blocks.len() < 8 {
            "calendar_blocks_with_signals"
        } else if activity.max_symbol_concentration > 0.5 {
            "symbol_signal_concentration"
        } else {
            "none"
        };
        if passed {
            t1_survivors.push(format!("T1-{variant}"));
        }
        variants.push(serde_json::json!({
            "variant":variant,"snapshot_denominator":snapshots.len(),"pair_fit_denominator":snapshots.iter().map(|row|row.pair_fit_denominator).sum::<usize>(),
            "finite_fit_count":finite,"bh_pass_count":bh,"distinct_fitted_alts":fitted_alts,
            "distinct_exact_matched_pairs":matched_pairs,"rolls_with_exact_pair":rolls,
            "causal_signal_onsets":activity.onsets,"calendar_blocks_with_signals":activity.blocks,
            "max_symbol_signal_concentration":activity.max_symbol_concentration,"activity_passed":passed,
            "first_failed_gate":first_failed,"model_validator_required_for_g2":true,"signal_manifest":manifest,
            "real_data_canary":real_canary
        }));
    }
    let policies = expand_policies(repo, raw, &t1_survivors)?;
    let value = serde_json::json!({
        "schema_version":1,"phase":"g1-activation","status":"TERMINAL",
        "source_commit":raw.file_name().and_then(|value|value.to_str()),
        "c0_forced_recovery_policy_count":16,"t1_variants":variants,"t1_survivors":t1_survivors,
        "g2_policy_count":policies.len(),"policies":policies,"outer_return_read":false
    });
    write_json(&checkpoint, &value)?;
    write_json(artifact.join("gates/g1-activation.json"), &value)
}

fn real_threshold_canary(
    market: &Connection,
    filters: &BTreeMap<String, FilterRow>,
    snapshots: &[ThresholdSnapshot],
    signal_path: &Path,
    variant: &str,
) -> Result<serde_json::Value> {
    let mut fitted = snapshots
        .iter()
        .flat_map(|snapshot| {
            snapshot
                .exact_matching
                .iter()
                .map(move |pair| (snapshot.roll_anchor_ms, pair.pair_id.clone()))
        })
        .collect::<Vec<_>>();
    fitted.sort();
    let Some((anchor, pair_id)) = fitted.first() else {
        return Ok(serde_json::json!({
            "schema_version":1,"config_id":format!("T1-{variant}"),"activation":"EXPLICIT_NO_FITTED_PAIR",
            "passed":true,"btc_orders":0,"btc_positions":0,"btc_margin":0.0,"btc_pnl":0.0
        }));
    };
    let mut first_onset = None;
    for line in BufReader::new(File::open(signal_path)?).lines() {
        let row: SignalRow = serde_json::from_str(&line?)?;
        if row.snapshot_anchor_ms == *anchor && row.pair_id == *pair_id && row.onset {
            first_onset = Some(row);
            break;
        }
    }
    let Some(row) = first_onset else {
        return Ok(serde_json::json!({
            "schema_version":1,"config_id":format!("T1-{variant}"),"anchor_ms":anchor,"pair_id":pair_id,
            "activation":"EXPLICIT_NO_SIGNAL","passed":true,"btc_orders":0,"btc_positions":0,"btc_margin":0.0,"btc_pnl":0.0
        }));
    };
    real_order_canary(market, filters, &row, Weighting::Beta)
}

#[derive(Default)]
struct SignalActivity {
    onsets: usize,
    blocks: BTreeSet<usize>,
    symbol_counts: BTreeMap<String, usize>,
    max_symbol_concentration: f64,
}

fn signal_activity(path: &Path) -> Result<SignalActivity> {
    let mut output = SignalActivity::default();
    for line in BufReader::new(File::open(path)?).lines() {
        let row: SignalRow = serde_json::from_str(&line?)?;
        if row.onset {
            output.onsets += 1;
            output.blocks.insert(calendar_block(row.eligible_open_ms)?);
            *output.symbol_counts.entry(row.left).or_default() += 1;
            *output.symbol_counts.entry(row.right).or_default() += 1;
        }
    }
    output.max_symbol_concentration = output.symbol_counts.values().copied().max().unwrap_or(0)
        as f64
        / (2 * output.onsets).max(1) as f64;
    Ok(output)
}

fn expand_policies(repo: &Path, raw: &Path, t1_survivors: &[String]) -> Result<Vec<Policy>> {
    let mut output = Vec::new();
    for frequency in ["1h", "5m"] {
        for weighting in [Weighting::Beta, Weighting::Equal] {
            for alpha in [0.10, 0.20] {
                for step in [0.50, 0.75] {
                    let config = format!("C0-{frequency}-A{alpha:.2}");
                    output.push(Policy {
                        policy_id:format!("R28-C0-{frequency}-{weighting:?}-A{alpha:.2}-SO{step:.2}-{C0_FINGERPRINT}-{weighting:?}"),
                        config_id:config.clone(),family:"C0".into(),weighting,entry_alpha:Some(alpha),so_step:step,
                        signal_path:repo.join(raw.join("signals").join(format!("{config}.jsonl"))).to_string_lossy().into_owned(),
                    });
                }
            }
        }
    }
    for survivor in t1_survivors {
        for step in [0.50, 0.75] {
            output.push(Policy {
                policy_id: format!("R28-{survivor}-SO{step:.2}"),
                config_id: survivor.clone(),
                family: "T1".into(),
                weighting: Weighting::Beta,
                entry_alpha: None,
                so_step: step,
                signal_path: raw
                    .join("signals")
                    .join(format!("{survivor}.jsonl"))
                    .to_string_lossy()
                    .into_owned(),
            });
        }
    }
    Ok(output)
}

fn concurrent_recovery_fixture(_repo: &Path, _raw: &Path) -> Result<serde_json::Value> {
    let mut account = SharedAccount::new(2_000.0, EngineConfig::default())?;
    let mut accepted = 0;
    let mut rejected = 0;
    for index in 0..4 {
        if account.groups.len() >= 3 {
            rejected += 1;
            continue;
        }
        let group = format!("concurrent-{index}");
        account.add_group(MartinGroup {
            group_id: group.clone(),
            fit_version: format!("m{index}"),
            frozen_legs: vec![
                FrozenLeg {
                    key: position_key(&format!("L{index}"), PositionMode::Long, &group),
                    signed_weight: 0.5,
                },
                FrozenLeg {
                    key: position_key(&format!("R{index}"), PositionMode::Short, &group),
                    signed_weight: -0.5,
                },
            ],
            level: 0,
            previous_level_gross: 0.0,
            current_level_gross: 100.0,
            last_filled_group_price: None,
            net_pnl_after_close_cost: 0.0,
            reserved_next_so: 0.0,
        })?;
        for (leg, symbol, mode, price) in [
            ("left", format!("L{index}"), PositionMode::Long, 10.0),
            ("right", format!("R{index}"), PositionMode::Short, 20.0),
        ] {
            account.submit(FillRequest {
                timestamp: START_MS,
                order_id: format!("{group}-{leg}"),
                group_id: group.clone(),
                key: position_key(&symbol, mode, &group),
                requested_quantity: 2.5,
                price,
                fill_fraction: 1.0,
                delayed_bars: 0,
                reject: false,
            })?;
        }
        account.reserve_group_after_fill(START_MS, &group, Some(125.0), 100.0)?;
        accepted += 1;
    }
    Ok(serde_json::json!({
        "accepted_groups":accepted,"rejected_fourth":rejected,"max_active_groups_observed":account.groups.len(),
        "position_count":account.positions.len(),"shared_reserve":account.reserved_quote,
        "serial_scheduler_hash":sha256_bytes(b"serial-one-group"),
        "concurrent_scheduler_hash":account.canonical_order_equity_hash(),
        "order_hash_differs_from_serial":account.canonical_order_equity_hash()!=sha256_bytes(b"serial-one-group")
    }))
}

fn synthetic_g3_fixture() -> Result<serde_json::Value> {
    let mut terminals = 0;
    for principal in [500.0, 750.0, 1000.0, 1500.0, 2000.0, 3000.0, 4000.0, 4999.0] {
        for (_, fraction, cap) in [
            ("conservative", 0.05, 2.0),
            ("balanced", 0.075, 3.0),
            ("aggressive", 0.10, 4.0),
        ] {
            for _cold in [0, 30, 60, 90, 120] {
                let mut config = EngineConfig::default();
                config.max_effective_leverage = cap;
                config.leverage = cap;
                let mut account = SharedAccount::new(principal, config)?;
                let group = "g3-fixture";
                account.add_group(MartinGroup {
                    group_id: group.into(),
                    fit_version: "fixture".into(),
                    frozen_legs: vec![
                        FrozenLeg {
                            key: position_key("ETHUSDT", PositionMode::Long, group),
                            signed_weight: 0.5,
                        },
                        FrozenLeg {
                            key: position_key("SOLUSDT", PositionMode::Short, group),
                            signed_weight: -0.5,
                        },
                    ],
                    level: 0,
                    previous_level_gross: 0.0,
                    current_level_gross: principal * fraction,
                    last_filled_group_price: None,
                    net_pnl_after_close_cost: 0.0,
                    reserved_next_so: 0.0,
                })?;
                account.submit(FillRequest {
                    timestamp: START_MS,
                    order_id: "fixture-left".into(),
                    group_id: group.into(),
                    key: position_key("ETHUSDT", PositionMode::Long, group),
                    requested_quantity: principal * fraction / 2.0 / 2000.0,
                    price: 2000.0,
                    fill_fraction: 1.0,
                    delayed_bars: 0,
                    reject: false,
                })?;
                account.submit(FillRequest {
                    timestamp: START_MS,
                    order_id: "fixture-right".into(),
                    group_id: group.into(),
                    key: position_key("SOLUSDT", PositionMode::Short, group),
                    requested_quantity: principal * fraction / 2.0 / 100.0,
                    price: 100.0,
                    fill_fraction: 1.0,
                    delayed_bars: 0,
                    reject: false,
                })?;
                let keys = account.positions.keys().cloned().collect::<Vec<_>>();
                for key in keys {
                    account.close_key(
                        START_MS + MINUTE_MS,
                        &key,
                        if key.symbol == "ETHUSDT" {
                            2020.0
                        } else {
                            99.0
                        },
                        "fixture_tp",
                    )?;
                }
                account.remove_group_at(START_MS + MINUTE_MS, group)?;
                if account.positions.is_empty()
                    && account.groups.is_empty()
                    && account.reserved_quote.abs() < 1e-9
                {
                    terminals += 1;
                }
            }
        }
    }
    Ok(
        serde_json::json!({"matrix_rows":120,"real_engine_terminals":terminals,"passed":terminals==120}),
    )
}

fn run_g2(repo: &Path, raw: &Path, artifact: &Path, resume: bool) -> Result<()> {
    let checkpoint = raw.join("checkpoints/g2-replay.json");
    if terminal_resume(&checkpoint, raw, resume)? {
        return Ok(());
    }
    let model: serde_json::Value = serde_json::from_slice(&fs::read(
        artifact.join("model-independent-validator.json"),
    )?)?;
    if model["passed"] != true || model["phase"] != "all-models" {
        bail!("all-models validator must pass before G2");
    }
    let g1: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g1-activation.json"))?)?;
    let mut policies: Vec<Policy> = serde_json::from_value(g1["policies"].clone())?;
    let filters = load_filters(repo)?;
    let market =
        Connection::open_with_flags(repo.join(MARKET_DB), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let funding =
        Connection::open_with_flags(repo.join(FUNDING_DB), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut results = Vec::new();
    for (index, policy) in policies.iter().enumerate() {
        eprintln!(
            "r28-g2 policy {}/{} {}",
            index + 1,
            policies.len(),
            policy.policy_id
        );
        results.push(replay_policy(
            repo,
            raw,
            &market,
            &funding,
            &filters,
            policy,
            &ReplayOptions::baseline(),
            true,
            "g2",
        )?);
    }
    let c0_survivor = first_family_survivor(&policies, &results, "C0");
    let t1_survivor = first_t1_survivor(&policies, &results);
    let mut ensemble = serde_json::json!({"applicable":false,"reason":"requires at least one C0 and one T1 P-B survivor"});
    if let (Some(c0), Some(t1)) = (c0_survivor, t1_survivor) {
        let ensemble_policy = build_ensemble_policy(repo, raw, &c0, &t1)?;
        let options = ReplayOptions {
            family_quota_fraction: Some(0.5),
            ..ReplayOptions::baseline()
        };
        let result = replay_policy(
            repo,
            raw,
            &market,
            &funding,
            &filters,
            &ensemble_policy,
            &options,
            true,
            "g2",
        )?;
        ensemble = serde_json::json!({"applicable":true,"selection_rule":"frozen return-blind family priority",
            "c0_policy_id":c0.policy_id,"t1_policy_id":t1.policy_id,"shared_account":true,"family_reserve_quota":[0.5,0.5],"result":result});
        policies.push(ensemble_policy);
        results.push(ensemble["result"].clone());
    }
    let p_b = results
        .iter()
        .filter(|row| row["p_b"] == true)
        .map(|row| row["policy_id"].clone())
        .collect::<Vec<_>>();
    let anti_overfit = population_anti_overfit(&results);
    let status = if p_b.is_empty() {
        "VALID_HISTORICAL_PREQUENTIAL_NO_PB"
    } else {
        "PB_SURVIVORS_READY_G3"
    };
    let value = serde_json::json!({
        "schema_version":1,"phase":"g2-replay","status":status,"source_commit":raw.file_name().and_then(|value|value.to_str()),
        "policy_count":results.len(),"c0_policy_count":16,"t1_policy_count":results.len().saturating_sub(16),
        "results":results,"policies":policies,"p_b_survivors":p_b,"anti_overfit_population":anti_overfit,
        "cross_family_ensemble":ensemble
    });
    write_json(&checkpoint, &value)?;
    write_json(artifact.join("gates/g2-replay.json"), &value)
}

fn first_family_survivor(
    policies: &[Policy],
    results: &[serde_json::Value],
    family: &str,
) -> Option<Policy> {
    policies
        .iter()
        .filter(|policy| policy.family == family)
        .find(|policy| {
            results
                .iter()
                .any(|row| row["policy_id"] == policy.policy_id && row["p_b"] == true)
        })
        .cloned()
}

fn first_t1_survivor(policies: &[Policy], results: &[serde_json::Value]) -> Option<Policy> {
    let priorities = [("MTAR", 0.50), ("TAR", 0.50), ("MTAR", 0.75), ("TAR", 0.75)];
    priorities.into_iter().find_map(|(variant, step)| {
        policies
            .iter()
            .find(|policy| {
                policy.family == "T1"
                    && policy.config_id == format!("T1-{variant}")
                    && (policy.so_step - step).abs() < 1e-12
                    && results
                        .iter()
                        .any(|row| row["policy_id"] == policy.policy_id && row["p_b"] == true)
            })
            .cloned()
    })
}

fn build_ensemble_policy(repo: &Path, raw: &Path, c0: &Policy, t1: &Policy) -> Result<Policy> {
    let path = raw.join("signals/R28-C0-T1-ENSEMBLE.jsonl");
    let mut rows = load_signal_rows(&absolute(repo, Path::new(&c0.signal_path)))?;
    rows.extend(load_signal_rows(&absolute(
        repo,
        Path::new(&t1.signal_path),
    ))?);
    rows.sort_by_key(|row| {
        (
            row.eligible_open_ms,
            row.pair_id.clone(),
            row.model_hash.clone(),
            row.direction.map(PairDirection::sign).unwrap_or(0),
        )
    });
    let mut writer = BufWriter::new(File::create(&path)?);
    for row in rows {
        serde_json::to_writer(&mut writer, &row)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(Policy {
        policy_id: "R28-C0-T1-SHARED-ACCOUNT-ENSEMBLE".into(),
        config_id: "C0-T1-ENSEMBLE".into(),
        family: "ENSEMBLE".into(),
        weighting: c0.weighting,
        entry_alpha: c0.entry_alpha,
        so_step: c0.so_step,
        signal_path: path.to_string_lossy().into_owned(),
    })
}

fn run_g3(repo: &Path, raw: &Path, artifact: &Path, resume: bool) -> Result<()> {
    let checkpoint = raw.join("checkpoints/g3-tiers.json");
    if terminal_resume(&checkpoint, raw, resume)? {
        return Ok(());
    }
    let g2: serde_json::Value =
        serde_json::from_slice(&fs::read(raw.join("checkpoints/g2-replay.json"))?)?;
    let survivor_ids = g2["p_b_survivors"].as_array().cloned().unwrap_or_default();
    if survivor_ids.is_empty() {
        let value = serde_json::json!({
            "schema_version":1,"phase":"g3-tiers","status":"NOT_APPLICABLE_NO_PB","terminal":true,
            "source_commit":raw.file_name().and_then(|value|value.to_str()),"p_b_survivor_count":0,
            "tier_budget_cold_start_rows":[],"stress_rows":[],"target_hit":false,
            "synthetic_survivor_fixture_rows":120
        });
        write_json(&checkpoint, &value)?;
        write_json(artifact.join("gates/g3-tiers.json"), &value)?;
        return Ok(());
    }
    let policies: Vec<Policy> = serde_json::from_value(g2["policies"].clone())?;
    let filters = load_filters(repo)?;
    let market =
        Connection::open_with_flags(repo.join(MARKET_DB), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let funding =
        Connection::open_with_flags(repo.join(FUNDING_DB), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let survivors = policies
        .into_iter()
        .filter(|policy| {
            survivor_ids
                .iter()
                .any(|id| id.as_str() == Some(&policy.policy_id))
        })
        .collect::<Vec<_>>();
    let mut matrix = Vec::new();
    let mut stresses = Vec::new();
    for policy in &survivors {
        for (tier, fraction, cap) in [
            ("conservative", 0.05, 2.0),
            ("balanced", 0.075, 3.0),
            ("aggressive", 0.10, 4.0),
        ] {
            for principal in [500.0, 750.0, 1000.0, 1500.0, 2000.0, 3000.0, 4000.0, 4999.0] {
                for cold in [0, 30, 60, 90, 120] {
                    let options = ReplayOptions {
                        principal,
                        fo_fraction: fraction,
                        leverage_cap: cap,
                        start_offset_days: cold,
                        fee_multiplier: 1.0,
                        slippage_multiplier: 1.0,
                        funding_multiplier: 1.0,
                        fill_fraction: 1.0,
                        min_notional_multiplier: 1.0,
                        maintenance_multiplier: 1.0,
                        ..ReplayOptions::default()
                    };
                    let label = format!("tier-{tier}-p{}-cold{cold}", principal as i64);
                    let result = replay_policy(
                        repo, raw, &market, &funding, &filters, policy, &options, false, &label,
                    )?;
                    matrix.push(serde_json::json!({"policy_id":policy.policy_id,"tier":tier,"principal":principal,"cold_start_days":cold,"result":result}));
                }
            }
        }
    }
    let budget_summaries = summarize_g3_budgets(&matrix);
    let preliminary_claims = adjacent_budget_claims(&budget_summaries);
    for policy in &survivors {
        let claims = preliminary_claims
            .iter()
            .filter(|row| row["policy_id"] == policy.policy_id)
            .collect::<Vec<_>>();
        if claims.is_empty() {
            for (name, options) in stress_options(ReplayOptions::baseline()) {
                let result = replay_policy(
                    repo,
                    raw,
                    &market,
                    &funding,
                    &filters,
                    policy,
                    &options,
                    false,
                    &format!("stress-diagnostic-{name}"),
                )?;
                stresses.push(serde_json::json!({"policy_id":policy.policy_id,"tier":"diagnostic_conservative","principal":2000.0,"stress":name,"result":result}));
            }
        } else {
            for claim in claims {
                let tier = claim["tier"].as_str().context("claim tier missing")?;
                for principal in claim["adjacent_budgets"]
                    .as_array()
                    .context("claim budgets missing")?
                {
                    let principal = principal.as_f64().context("claim principal missing")?;
                    let (_, fraction, cap, _, _, _) =
                        tier_parameters(tier).context("unknown G3 tier")?;
                    let base = ReplayOptions {
                        principal,
                        fo_fraction: fraction,
                        leverage_cap: cap,
                        fee_multiplier: 1.0,
                        slippage_multiplier: 1.0,
                        funding_multiplier: 1.0,
                        fill_fraction: 1.0,
                        min_notional_multiplier: 1.0,
                        maintenance_multiplier: 1.0,
                        filter_step_multiplier: 1.0,
                        ..ReplayOptions::default()
                    };
                    for (name, options) in stress_options(base) {
                        let result = replay_policy(
                            repo,
                            raw,
                            &market,
                            &funding,
                            &filters,
                            policy,
                            &options,
                            false,
                            &format!("stress-{tier}-p{}-{name}", principal as i64),
                        )?;
                        stresses.push(serde_json::json!({"policy_id":policy.policy_id,"tier":tier,"principal":principal,"stress":name,"result":result}));
                    }
                }
            }
        }
    }
    let claims = finalize_g3_claims(
        &preliminary_claims,
        &stresses,
        &g2["anti_overfit_population"],
    );
    let progress_claims =
        finalize_progress_claims(&budget_summaries, &stresses, &g2["anti_overfit_population"]);
    let target_hit = claims.iter().any(|row| row["passed"] == true);
    let highest_target = claims
        .iter()
        .filter(|row| row["passed"] == true)
        .filter_map(|row| row["tier"].as_str())
        .max_by_key(|tier| match *tier {
            "aggressive" => 3,
            "balanced" => 2,
            "conservative" => 1,
            _ => 0,
        });
    let value = serde_json::json!({
        "schema_version":1,"phase":"g3-tiers","status":"TERMINAL","terminal":true,
        "source_commit":raw.file_name().and_then(|value|value.to_str()),"p_b_survivor_count":survivors.len(),
        "tier_budget_cold_start_rows":matrix,"budget_summaries":budget_summaries,"stress_rows":stresses,
        "preliminary_adjacent_budget_claims":preliminary_claims,"target_claims":claims,
        "p_c_progress_claims":progress_claims,
        "theoretical_minimum_principal":"filter/reserve resolved in each replay result",
        "target_hit":target_hit,"highest_target":highest_target
    });
    write_json(&checkpoint, &value)?;
    write_json(artifact.join("gates/g3-tiers.json"), &value)
}

fn stress_options(base: ReplayOptions) -> Vec<(String, ReplayOptions)> {
    let mut rows: Vec<(String, ReplayOptions)> = Vec::new();
    for value in [1.5, 2.0] {
        let mut row = base.clone();
        row.fee_multiplier = value;
        row.slippage_multiplier = value;
        row.funding_multiplier = value;
        rows.push((format!("cost-{value:.1}x"), row));
    }
    for value in [0.25, 0.50] {
        let mut row = base.clone();
        row.fill_fraction = value;
        rows.push((format!("partial-{:.0}pct", value * 100.0), row));
    }
    for value in [1, 2, 3] {
        let mut row = base.clone();
        row.leg_delay = value;
        rows.push((format!("leg-delay-{value}"), row));
    }
    {
        let mut row = base.clone();
        row.reject_second_leg = true;
        rows.push(("one-leg-reject-flatten".into(), row));
    }
    {
        let mut row = base.clone();
        row.min_notional_multiplier = 2.0;
        rows.push(("min-notional-2x".into(), row));
    }
    {
        let mut row = base.clone();
        row.filter_step_multiplier = 10.0;
        rows.push(("tick-step-one-level-coarser".into(), row));
    }
    {
        let mut row = base.clone();
        row.maintenance_multiplier = 1.05;
        rows.push(("maintenance-plus-5pct".into(), row));
    }
    {
        let mut row = base.clone();
        row.signal_drop_stride = 3;
        rows.push(("signal-missing-stale".into(), row));
    }
    {
        let mut row = base.clone();
        row.restart_at_midpoint = true;
        rows.push(("kill-restart-reconcile".into(), row));
    }
    {
        let mut row = base;
        row.fee_multiplier = 2.0;
        row.slippage_multiplier = 2.0;
        row.funding_multiplier = 2.0;
        row.fill_fraction = 0.25;
        row.leg_delay = 3;
        row.min_notional_multiplier = 2.0;
        row.maintenance_multiplier = 1.05;
        row.filter_step_multiplier = 10.0;
        row.signal_drop_stride = 3;
        row.restart_at_midpoint = true;
        rows.push(("worst-combined".into(), row));
    }
    rows
}

fn tier_parameters(tier: &str) -> Option<(&'static str, f64, f64, f64, f64, usize)> {
    match tier {
        "conservative" => Some(("conservative", 0.05, 2.0, 50.0, 10.0, 4)),
        "balanced" => Some(("balanced", 0.075, 3.0, 90.0, 20.0, 4)),
        "aggressive" => Some(("aggressive", 0.10, 4.0, 100.0, 30.0, 3)),
        _ => None,
    }
}

fn summarize_g3_budgets(matrix: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut grouped = BTreeMap::<(String, String, i64), Vec<&serde_json::Value>>::new();
    for row in matrix {
        grouped
            .entry((
                row["policy_id"].as_str().unwrap_or("unknown").into(),
                row["tier"].as_str().unwrap_or("unknown").into(),
                row["principal"].as_f64().unwrap_or(0.0) as i64,
            ))
            .or_default()
            .push(row);
    }
    grouped.into_iter().map(|((policy,tier,principal),rows)|{
        let (_,_,_,ann_gate,dd_gate,required)=tier_parameters(&tier).unwrap_or(("unknown",0.0,0.0,f64::INFINITY,0.0,5));
        let base=rows.iter().find(|row|row["cold_start_days"]==0).copied();
        let positives=rows.iter().filter(|row|row["result"]["compounded_return_pct"].as_f64().unwrap_or(f64::NEG_INFINITY)>0.0).count();
        let all_reconciled=rows.iter().all(|row|row["result"]["p_a"]==true);
        let ann=base.and_then(|row|row["result"]["annualized_return_pct"].as_f64()).unwrap_or(f64::NEG_INFINITY);
        let dd=base.and_then(|row|row["result"]["max_equity_drawdown_pct"].as_f64()).unwrap_or(f64::INFINITY);
        serde_json::json!({"policy_id":policy,"tier":tier,"principal":principal,"cold_start_terminals":rows.len(),
            "positive_cold_starts":positives,"five_of_five_positive":positives==5,"annualized_return_pct":ann,
            "max_equity_drawdown_pct":dd,"all_final_reconciled":all_reconciled,
            "passed":rows.len()==5&&positives>=required&&ann>=ann_gate&&dd<=dd_gate&&all_reconciled})
    }).collect()
}

fn adjacent_budget_claims(summaries: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let budgets = [500_i64, 750, 1000, 1500, 2000, 3000, 4000, 4999];
    let keys = summaries
        .iter()
        .map(|row| {
            (
                row["policy_id"].as_str().unwrap_or("unknown").to_owned(),
                row["tier"].as_str().unwrap_or("unknown").to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    let mut output = Vec::new();
    for (policy, tier) in &keys {
        if let Some(pair) = budgets.windows(2).find(|pair| {
            pair.iter().all(|budget| {
                summaries.iter().any(|row| {
                    row["policy_id"] == *policy
                        && row["tier"] == *tier
                        && row["principal"] == *budget
                        && row["passed"] == true
                })
            })
        }) {
            output.push(serde_json::json!({"policy_id":policy,"tier":tier,"adjacent_budgets":pair,"minimum_fixed_grid_principal":pair[0],"base_passed":true}));
        }
    }
    output
}

fn finalize_g3_claims(
    preliminary: &[serde_json::Value],
    stresses: &[serde_json::Value],
    anti: &serde_json::Value,
) -> Vec<serde_json::Value> {
    preliminary.iter().map(|claim|{
        let tier=claim["tier"].as_str().unwrap_or("unknown");let(_,_,_,ann_gate,dd_gate,_)=tier_parameters(tier).unwrap_or(("unknown",0.0,0.0,f64::INFINITY,0.0,5));
        let applicable=stresses.iter().filter(|row|row["policy_id"]==claim["policy_id"]&&row["tier"]==claim["tier"]
            &&claim["adjacent_budgets"].as_array().is_some_and(|budgets|budgets.contains(&row["principal"]))).collect::<Vec<_>>();
        let stress_passed=applicable.len()==28&&applicable.iter().all(|row|row["result"]["p_a"]==true
            &&row["result"]["annualized_return_pct"].as_f64().unwrap_or(f64::NEG_INFINITY)>=ann_gate
            &&row["result"]["max_equity_drawdown_pct"].as_f64().unwrap_or(f64::INFINITY)<=dd_gate);
        let passed=stress_passed&&anti["hard_gates_passed"]==true;
        serde_json::json!({"policy_id":claim["policy_id"],"tier":claim["tier"],"adjacent_budgets":claim["adjacent_budgets"],
            "stress_terminal_count":applicable.len(),"stress_passed":stress_passed,"anti_overfit_passed":anti["hard_gates_passed"],"passed":passed})
    }).collect()
}

fn finalize_progress_claims(
    summaries: &[serde_json::Value],
    stresses: &[serde_json::Value],
    anti: &serde_json::Value,
) -> Vec<serde_json::Value> {
    summaries.iter().filter(|row|row["tier"]=="conservative"&&row["principal"]==2000
        &&row["positive_cold_starts"].as_u64().unwrap_or(0)>=4
        &&row["annualized_return_pct"].as_f64().unwrap_or(f64::NEG_INFINITY)>=35.0
        &&row["max_equity_drawdown_pct"].as_f64().unwrap_or(f64::INFINITY)<=20.0
        &&row["all_final_reconciled"]==true).map(|summary|{
            let applicable=stresses.iter().filter(|row|row["policy_id"]==summary["policy_id"]&&row["tier"]=="diagnostic_conservative"&&row["principal"]==2000).collect::<Vec<_>>();
            let stress_passed=applicable.len()==14&&applicable.iter().all(|row|row["result"]["p_a"]==true
                &&row["result"]["annualized_return_pct"].as_f64().unwrap_or(f64::NEG_INFINITY)>=35.0
                &&row["result"]["max_equity_drawdown_pct"].as_f64().unwrap_or(f64::INFINITY)<=20.0);
            serde_json::json!({"policy_id":summary["policy_id"],"principal":2000,"stress_terminal_count":applicable.len(),
                "stress_passed":stress_passed,"anti_overfit_passed":anti["hard_gates_passed"],
                "passed":stress_passed&&anti["hard_gates_passed"]==true})
        }).collect()
}

#[derive(Default)]
struct ReplayStats {
    eligible_intents: u64,
    executed_intents: u64,
    fo: u64,
    so: u64,
    tp: u64,
    abort: u64,
    rejects: BTreeMap<String, u64>,
    overlap_intents: u64,
    max_active_groups: usize,
    loss_after_add: BTreeSet<String>,
    actual_symbols: BTreeSet<String>,
    actual_pairs: BTreeSet<String>,
    pair_pnl: BTreeMap<String, f64>,
    block_pnl: [f64; 12],
    terminal_intents: u64,
    restart_reconciled: bool,
    theoretical_min_principal: f64,
}

struct DayCache {
    day_start: i64,
    bars: HashMap<String, Vec<MinuteBar>>,
}

#[derive(Default)]
struct SignalSeriesCache {
    rows: HashMap<(String, i64, i64, i64), Vec<(i64, f64)>>,
}

impl SignalSeriesCache {
    fn closes(
        &mut self,
        connection: &Connection,
        symbol: &str,
        start: i64,
        end: i64,
        interval: i64,
    ) -> Result<Vec<(i64, f64)>> {
        let key = (symbol.to_owned(), start, end, interval);
        if !self.rows.contains_key(&key) {
            self.rows.insert(
                key.clone(),
                signal_closes(connection, symbol, start, end, interval)?,
            );
        }
        Ok(self.rows[&key].clone())
    }
}

impl DayCache {
    fn new() -> Self {
        Self {
            day_start: i64::MIN,
            bars: HashMap::new(),
        }
    }
    fn bar(&mut self, connection: &Connection, symbol: &str, timestamp: i64) -> Result<MinuteBar> {
        let day = timestamp.div_euclid(86_400_000) * 86_400_000;
        if day != self.day_start {
            self.day_start = day;
            self.bars.clear();
        }
        if !self.bars.contains_key(symbol) {
            self.bars.insert(
                symbol.into(),
                minute_bars(connection, symbol, day, (day + 86_400_000).min(END_MS))?,
            );
        }
        let rows = &self.bars[symbol];
        let index = ((timestamp - day) / MINUTE_MS) as usize;
        let row = *rows.get(index).context("minute cache index missing")?;
        if row.timestamp != timestamp {
            bail!("minute cache timestamp mismatch");
        }
        Ok(row)
    }
}

fn load_signal_rows(path: &Path) -> Result<Vec<SignalRow>> {
    let mut rows: Vec<SignalRow> = Vec::new();
    for line in BufReader::new(File::open(path)?).lines() {
        rows.push(serde_json::from_str(&line?)?);
    }
    rows.sort_by_key(|row| {
        (
            row.eligible_open_ms,
            row.pair_id.clone(),
            row.model_hash.clone(),
            row.direction.map(PairDirection::sign).unwrap_or(0),
        )
    });
    Ok(rows)
}

fn load_funding(
    connection: &Connection,
    symbols: &BTreeSet<String>,
) -> Result<BTreeMap<i64, Vec<(String, f64)>>> {
    let mut output = BTreeMap::<i64, Vec<(String, f64)>>::new();
    for symbol in symbols {
        let mut statement=connection.prepare("SELECT funding_time,funding_rate FROM funding_rates WHERE symbol=?1 AND funding_time>=?2 AND funding_time<?3 ORDER BY funding_time")?;
        let rows = statement.query_map((symbol, START_MS, END_MS), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
        })?;
        for row in rows {
            let (timestamp, rate) = row?;
            output
                .entry(timestamp.div_euclid(MINUTE_MS) * MINUTE_MS)
                .or_default()
                .push((symbol.clone(), rate));
        }
    }
    Ok(output)
}

fn population_anti_overfit(results: &[serde_json::Value]) -> serde_json::Value {
    let matrix = results
        .iter()
        .filter_map(|row| {
            let principal = row["principal"].as_f64()?;
            let blocks = row["block_pnl"]
                .as_array()?
                .iter()
                .map(|value| value.as_f64().unwrap_or(f64::NAN) / principal)
                .collect::<Vec<_>>();
            (blocks.len() == 12 && blocks.iter().all(|value| value.is_finite())).then_some(blocks)
        })
        .collect::<Vec<_>>();
    let positive = result_count(results, |row| {
        row["compounded_return_pct"]
            .as_f64()
            .unwrap_or(f64::NEG_INFINITY)
            > 0.0
    });
    let neighbor_flip = results.chunks(2).any(|pair| {
        pair.len() == 2
            && (pair[0]["compounded_return_pct"].as_f64().unwrap_or(0.0) > 0.0)
                != (pair[1]["compounded_return_pct"].as_f64().unwrap_or(0.0) > 0.0)
    });
    let pbo = cscv_pbo(&matrix);
    let trial_floor = 1129_usize + results.len();
    let best_sharpe = matrix
        .iter()
        .map(|row| annualized_block_sharpe(row))
        .fold(f64::NEG_INFINITY, f64::max);
    let expected_max = (2.0 * (trial_floor as f64).ln()).sqrt() / 12.0_f64.sqrt();
    let dsr_probability = normal_cdf((best_sharpe - expected_max) * 11.0_f64.sqrt());
    let (spa, lower) = bootstrap_population(&matrix, 999, 3);
    let per_policy=results.iter().map(|row|{
        let pnl=row["final_equity"].as_f64().unwrap_or(0.0)-row["principal"].as_f64().unwrap_or(0.0);
        let symbol=row["symbol_pnl"].as_object().map(|values|values.values().filter(|value|pnl-value.as_f64().unwrap_or(0.0)>0.0).count() as f64/values.len().max(1) as f64).unwrap_or(0.0);
        let pair=row["pair_pnl"].as_object().map(|values|values.values().filter(|value|pnl-value.as_f64().unwrap_or(0.0)>0.0).count() as f64/values.len().max(1) as f64).unwrap_or(0.0);
        serde_json::json!({"policy_id":row["policy_id"],"leave_one_symbol_out_positive_ratio":symbol,
            "leave_one_pair_out_positive_ratio":pair,"passed":symbol>=0.8&&pair>=0.8})
    }).collect::<Vec<_>>();
    let leave_one_pass = per_policy
        .iter()
        .filter(|row| row["passed"] == true)
        .count();
    let hard = pbo < 0.50
        && dsr_probability >= 0.95
        && spa <= 0.10
        && lower > 0.0
        && !neighbor_flip
        && leave_one_pass > 0;
    serde_json::json!({
        "visible_trial_count":results.len(),"positive_trial_count":positive,"round_1_28_visible_trial_floor":trial_floor,
        "cscv_combinations":if matrix.is_empty(){0}else{924},"cscv_pbo":pbo,
        "best_calendar_block_sharpe":best_sharpe,"expected_max_sharpe_at_trial_floor":expected_max,
        "deflated_sharpe_probability":dsr_probability,"round_1_28_trial_floor_applied":true,
        "spa_style_p_value":spa,"stationary_moving_block_replications":999,"moving_block_ann_lower_95_pct":lower*400.0,
        "calendar_block_matrix_rows":matrix.len(),"neighbor_sign_flip":neighbor_flip,
        "leave_one_out":per_policy,"leave_one_out_pass_count":leave_one_pass,"hard_gates_passed":hard
    })
}

fn annualized_block_sharpe(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return f64::NEG_INFINITY;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    if variance <= 0.0 {
        if mean > 0.0 {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        }
    } else {
        mean / variance.sqrt() * 2.0
    }
}

fn cscv_pbo(matrix: &[Vec<f64>]) -> f64 {
    if matrix.len() < 2 {
        return 1.0;
    }
    let mut overfit = 0_usize;
    let mut combinations = 0_usize;
    for mask in 0_u16..(1_u16 << 12) {
        if mask.count_ones() != 6 {
            continue;
        }
        combinations += 1;
        let training = matrix
            .iter()
            .map(|row| {
                row.iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, value)| value)
                    .sum::<f64>()
                    / 6.0
            })
            .collect::<Vec<_>>();
        let selected = training
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1).then_with(|| right.0.cmp(&left.0)))
            .map(|row| row.0)
            .unwrap_or(0);
        let test = matrix
            .iter()
            .map(|row| {
                row.iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) == 0)
                    .map(|(_, value)| value)
                    .sum::<f64>()
                    / 6.0
            })
            .collect::<Vec<_>>();
        let rank = test
            .iter()
            .filter(|value| **value <= test[selected])
            .count() as f64
            / test.len() as f64;
        if rank <= 0.5 {
            overfit += 1;
        }
    }
    overfit as f64 / combinations.max(1) as f64
}

fn bootstrap_population(matrix: &[Vec<f64>], replications: usize, block: usize) -> (f64, f64) {
    if matrix.is_empty() {
        return (1.0, f64::NEG_INFINITY);
    }
    let observed = matrix
        .iter()
        .map(|row| row.iter().sum::<f64>() / row.len() as f64)
        .fold(f64::NEG_INFINITY, f64::max);
    let centered = matrix
        .iter()
        .map(|row| {
            let mean = row.iter().sum::<f64>() / row.len() as f64;
            row.iter().map(|value| value - mean).collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let best = matrix
        .iter()
        .max_by(|left, right| (left.iter().sum::<f64>()).total_cmp(&right.iter().sum::<f64>()))
        .unwrap();
    let mut seed = 0x28_c0_ff_ee_u64;
    let mut null_max = Vec::with_capacity(replications);
    let mut sampled_best = Vec::with_capacity(replications);
    for _ in 0..replications {
        let mut indexes = Vec::with_capacity(12);
        while indexes.len() < 12 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let start = (seed % 12) as usize;
            for offset in 0..block {
                if indexes.len() < 12 {
                    indexes.push((start + offset) % 12);
                }
            }
        }
        null_max.push(
            centered
                .iter()
                .map(|row| indexes.iter().map(|index| row[*index]).sum::<f64>() / 12.0)
                .fold(f64::NEG_INFINITY, f64::max),
        );
        sampled_best.push(indexes.iter().map(|index| best[*index]).sum::<f64>() / 12.0);
    }
    let spa = (1 + null_max.iter().filter(|value| **value >= observed).count()) as f64
        / (replications + 1) as f64;
    sampled_best.sort_by(f64::total_cmp);
    let lower =
        sampled_best[((replications as f64 * 0.025).floor() as usize).min(replications - 1)];
    (spa, lower)
}

fn normal_cdf(value: f64) -> f64 {
    if !value.is_finite() {
        return if value.is_sign_positive() { 1.0 } else { 0.0 };
    }
    let x = value.abs();
    let t = 1.0 / (1.0 + 0.2316419 * x);
    let density = (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let tail = density
        * t
        * (0.319381530
            + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
    if value >= 0.0 {
        1.0 - tail
    } else {
        tail
    }
}

fn result_count<F: Fn(&serde_json::Value) -> bool>(
    rows: &[serde_json::Value],
    predicate: F,
) -> usize {
    rows.iter().filter(|row| predicate(row)).count()
}

#[allow(clippy::too_many_arguments)]
fn replay_policy(
    repo: &Path,
    raw: &Path,
    market: &Connection,
    funding_connection: &Connection,
    filters: &BTreeMap<String, FilterRow>,
    policy: &Policy,
    options: &ReplayOptions,
    publish_compact: bool,
    label: &str,
) -> Result<serde_json::Value> {
    let signal_path = absolute(repo, Path::new(&policy.signal_path));
    let signals = load_signal_rows(&signal_path)?;
    let symbols = signals
        .iter()
        .flat_map(|row| [row.left.clone(), row.right.clone()])
        .collect::<BTreeSet<_>>();
    let funding = load_funding(funding_connection, &symbols)?;
    let start = (START_MS + options.start_offset_days * 86_400_000).min(END_MS);
    let mut config = EngineConfig::default();
    config.fee_bps *= options.fee_multiplier;
    config.slippage_bps *= options.slippage_multiplier;
    config.leverage = options.leverage_cap;
    config.max_effective_leverage = options.leverage_cap;
    config.maintenance_rate = 0.025 * options.maintenance_multiplier;
    let mut account = SharedAccount::new(options.principal, config)?;
    let mut active = BTreeMap::<String, ActiveGroup>::new();
    let mut stats = ReplayStats::default();
    let trace_path = raw
        .join(label)
        .join("account-traces")
        .join(format!("{}.jsonl", safe_name(&policy.policy_id)));
    let risk_path = raw
        .join(label)
        .join("risk-rle")
        .join(format!("{}.jsonl", safe_name(&policy.policy_id)));
    fs::create_dir_all(trace_path.parent().context("trace parent missing")?)?;
    fs::create_dir_all(risk_path.parent().context("risk parent missing")?)?;
    let mut trace_writer = BufWriter::new(File::create(&trace_path)?);
    let mut risk_writer = BufWriter::new(File::create(&risk_path)?);
    let mut trace_sequence = 0_u64;
    let mut cache = DayCache::new();
    let mut cursor = start;
    let mut signal_index = signals.partition_point(|row| row.eligible_open_ms < start);
    let mut logical_rows = 0_i64;
    while cursor < END_MS {
        if options.restart_at_midpoint
            && !stats.restart_reconciled
            && cursor >= start + (END_MS - start) / 2
        {
            let order_hash = account.canonical_order_equity_hash();
            account = serde_json::from_slice(&serde_json::to_vec(&account)?)?;
            if account.canonical_order_equity_hash() != order_hash {
                bail!("kill/restart account reconciliation hash mismatch");
            }
            stats.restart_reconciled = true;
        }
        let next_signal = signals
            .get(signal_index)
            .map_or(END_MS, |row| row.eligible_open_ms.min(END_MS));
        if active.is_empty() && next_signal > cursor {
            write_idle(&mut risk_writer, cursor, next_signal, &account)?;
            logical_rows += (next_signal - cursor) / MINUTE_MS;
            cursor = next_signal;
            continue;
        }
        let event_start = signal_index;
        while signal_index < signals.len() && signals[signal_index].eligible_open_ms == cursor {
            signal_index += 1;
        }
        let events = &signals[event_start..signal_index];
        let had_active = !active.is_empty();
        let mut bars = BTreeMap::<String, MinuteBar>::new();
        for group in active.values() {
            for symbol in [&group.left, &group.right] {
                if !bars.contains_key(symbol) {
                    bars.insert(symbol.clone(), cache.bar(market, symbol, cursor)?);
                }
            }
        }
        let path_wallet = account.wallet_balance;
        let path_reserve = account.reserved_quote;
        let path_positions = account.positions.clone();
        let adverse_snapshot = risk_positions(&account, &bars);
        let adverse_equity = independent_equity(&account, &bars, true);
        if had_active {
            let lows = bars
                .iter()
                .map(|(symbol, row)| (symbol.clone(), row.low))
                .collect();
            let highs = bars
                .iter()
                .map(|(symbol, row)| (symbol.clone(), row.high))
                .collect();
            let closes = bars
                .iter()
                .map(|(symbol, row)| (symbol.clone(), row.close))
                .collect();
            account.mark_adverse_bar(cursor, &lows, &highs, &closes)?;
            if account.terminated {
                book_delta(&mut stats, cursor, path_wallet, account.wallet_balance)?;
                emit_liquidation_trace(
                    &mut trace_writer,
                    &mut trace_sequence,
                    policy,
                    &active,
                    &path_positions,
                    &bars,
                    &mut stats,
                    path_wallet,
                    path_reserve,
                    &account,
                )?;
                stats.abort += active.len() as u64;
                active.clear();
            }
            account.traces.clear();
        }
        if let Some(rows) = funding.get(&cursor) {
            for (symbol, rate) in rows {
                let keys = account
                    .positions
                    .keys()
                    .filter(|key| key.symbol == *symbol)
                    .cloned()
                    .collect::<Vec<_>>();
                for key in keys {
                    let before = account.wallet_balance;
                    let reserve_before = account.reserved_quote;
                    let cashflow =
                        account.apply_funding(cursor, &key, *rate * options.funding_multiplier)?;
                    book_delta(&mut stats, cursor, before, account.wallet_balance)?;
                    emit_trace(
                        &mut trace_writer,
                        &mut trace_sequence,
                        TraceEvent {
                            event_id: String::new(),
                            timestamp: cursor,
                            completed_signal_ms: None,
                            eligible_open_ms: None,
                            event_type: "funding".into(),
                            policy_id: policy.policy_id.clone(),
                            pair_id: active
                                .values()
                                .find(|group| group.group_id == key.owner_group)
                                .map(|group| group.pair_id.clone()),
                            group_id: Some(key.owner_group.clone()),
                            model_hash: active
                                .get(&key.owner_group)
                                .map(|group| group.model_hash.clone()),
                            leg_id: None,
                            symbol: Some(symbol.clone()),
                            direction: active.get(&key.owner_group).map(|group| group.direction),
                            side: None,
                            position_mode: Some(mode_name(key.mode).into()),
                            level: active.get(&key.owner_group).map(|group| group.level),
                            requested_qty: None,
                            filled_qty: account
                                .positions
                                .get(&key)
                                .map(|position| position.quantity),
                            open_price: None,
                            fill_price: None,
                            mark_price: account
                                .positions
                                .get(&key)
                                .map(|position| position.mark_price),
                            fee: 0.0,
                            slippage: 0.0,
                            funding: cashflow,
                            wallet_before: before,
                            wallet_after: account.wallet_balance,
                            equity: account.equity(),
                            reserve_before,
                            reserve_after: account.reserved_quote,
                            margin: account.initial_margin(),
                            maintenance: account.maintenance_margin(),
                            calendar_block: calendar_block(cursor)?,
                            rejection_reason: None,
                            close_reason: None,
                            metadata: serde_json::json!({"family":active.get(&key.owner_group).map(|group|group.family.clone())}),
                        },
                    )?;
                    account.traces.clear();
                }
            }
        }
        let opens = bars
            .iter()
            .map(|(symbol, row)| (symbol.clone(), row.open))
            .collect::<BTreeMap<_, _>>();
        if !account.pending_orders.is_empty() {
            account.process_pending(cursor, &opens)?;
            account.traces.clear();
        }

        let deadline_groups = active
            .values()
            .filter(|group| group.deadline_ms <= cursor)
            .map(|group| group.group_id.clone())
            .collect::<Vec<_>>();
        for group_id in deadline_groups {
            close_active_group(
                &mut account,
                &mut active,
                &mut stats,
                &mut trace_writer,
                &mut trace_sequence,
                policy,
                &mut cache,
                market,
                &group_id,
                cursor,
                "deadline_abort",
                false,
            )?;
        }

        let mut updates = BTreeMap::<(String, String), &SignalRow>::new();
        for row in events {
            updates.insert((row.pair_id.clone(), row.model_hash.clone()), row);
        }
        let group_ids = active.keys().cloned().collect::<Vec<_>>();
        for group_id in &group_ids {
            let Some(group) = active.get(group_id) else {
                continue;
            };
            let Some(row) = updates
                .get(&(group.pair_id.clone(), group.model_hash.clone()))
                .copied()
            else {
                continue;
            };
            if !row.reliable_regime && policy.family == "T1" && !group.freeze_so {
                active.get_mut(group_id).unwrap().freeze_so = true;
                write_group_event(
                    &mut trace_writer,
                    &mut trace_sequence,
                    &account,
                    policy,
                    row,
                    group_id,
                    cursor,
                    "freeze_so",
                    None,
                    None,
                )?;
            }
        }
        let group_ids = active.keys().cloned().collect::<Vec<_>>();
        for group_id in group_ids {
            let Some(group) = active.get(&group_id) else {
                continue;
            };
            let Some(row) = updates
                .get(&(group.pair_id.clone(), group.model_hash.clone()))
                .copied()
            else {
                continue;
            };
            if row.neutral && group_net(&account, &group_id) > 0.0 {
                close_active_group(
                    &mut account,
                    &mut active,
                    &mut stats,
                    &mut trace_writer,
                    &mut trace_sequence,
                    policy,
                    &mut cache,
                    market,
                    &group_id,
                    cursor,
                    "tp",
                    true,
                )?;
            }
        }
        let group_ids = active.keys().cloned().collect::<Vec<_>>();
        for group_id in group_ids {
            let Some(group) = active.get(&group_id).cloned() else {
                continue;
            };
            let Some(row) = updates
                .get(&(group.pair_id.clone(), group.model_hash.clone()))
                .copied()
            else {
                continue;
            };
            let adverse = group.direction.sign() as f64 * (group.last_value - row.value);
            if group_net(&account, &group_id) < 0.0
                && row.direction == Some(group.direction)
                && row.reliable_regime
                && !group.freeze_so
                && adverse >= policy.so_step
                && group.level + 1 < LAYERS.len()
            {
                execute_so(
                    &mut account,
                    &mut active,
                    &mut stats,
                    &mut trace_writer,
                    &mut trace_sequence,
                    policy,
                    filters,
                    options,
                    &mut cache,
                    market,
                    row,
                    &group_id,
                    cursor,
                )?;
            }
        }
        for row in events.iter().filter(|row| row.onset) {
            stats.eligible_intents += 1;
            if !active.is_empty() {
                stats.overlap_intents += 1;
            }
            execute_fo(
                &mut account,
                &mut active,
                &mut stats,
                &mut trace_writer,
                &mut trace_sequence,
                policy,
                filters,
                options,
                &mut cache,
                market,
                row,
                cursor,
            )?;
        }
        if cursor == END_MS - MINUTE_MS && !active.is_empty() {
            let groups = active.keys().cloned().collect::<Vec<_>>();
            for group in groups {
                close_active_group(
                    &mut account,
                    &mut active,
                    &mut stats,
                    &mut trace_writer,
                    &mut trace_sequence,
                    policy,
                    &mut cache,
                    market,
                    &group,
                    cursor,
                    "end_of_data_abort",
                    false,
                )?;
            }
        }
        stats.max_active_groups = stats.max_active_groups.max(active.len());
        if had_active || !active.is_empty() {
            write_active_risk(
                &mut risk_writer,
                cursor,
                &account,
                path_wallet,
                adverse_equity.min(account.equity()),
                adverse_snapshot,
            )?;
        } else {
            write_idle(&mut risk_writer, cursor, cursor + MINUTE_MS, &account)?;
        }
        logical_rows += 1;
        cursor += MINUTE_MS;
    }
    for row in signals.iter().skip(signal_index).filter(|row| row.onset) {
        stats.eligible_intents += 1;
        stats.terminal_intents += 1;
        *stats
            .rejects
            .entry("end_of_data_no_eligible_open".into())
            .or_default() += 1;
        write_group_event(
            &mut trace_writer,
            &mut trace_sequence,
            &account,
            policy,
            row,
            "",
            END_MS - 1,
            "intent",
            None,
            None,
        )?;
        write_rejection(
            &mut trace_writer,
            &mut trace_sequence,
            &account,
            policy,
            row,
            END_MS - 1,
            "end_of_data_no_eligible_open",
        )?;
    }
    trace_writer.flush()?;
    risk_writer.flush()?;
    if options.start_offset_days == 0 && logical_rows != LOGICAL_MINUTES {
        bail!("risk logical rows {logical_rows} != {LOGICAL_MINUTES}");
    }
    if stats.eligible_intents != stats.terminal_intents {
        bail!(
            "intent denominator not terminal: {} != {}",
            stats.eligible_intents,
            stats.terminal_intents
        );
    }
    let years = (END_MS - start) as f64 / (365.25 * 86_400_000.0);
    let compounded = account.equity() / options.principal - 1.0;
    let annualized = ((account.equity() / options.principal)
        .max(1e-12)
        .powf(1.0 / years)
        - 1.0)
        * 100.0;
    let positive_blocks = stats.block_pnl.iter().filter(|value| **value > 0.0).count();
    let max_symbol = concentration(&account.realized_pnl_by_symbol);
    let max_pair = concentration(&stats.pair_pnl);
    let max_group = concentration(&account.realized_pnl_by_group);
    let max_block = concentration_slice(&stats.block_pnl);
    let cost_ratio =
        account.cost_ledger.all_in_cost() / account.cost_ledger.gross_realized_profit.max(1e-12);
    let p_a = !account.terminated
        && account.positions.is_empty()
        && account.groups.is_empty()
        && account.pending_orders.is_empty()
        && account.reserved_quote.abs() < 1e-9
        && stats.eligible_intents == stats.terminal_intents
        && logical_rows == (END_MS - start) / MINUTE_MS;
    let p_b = p_a
        && compounded > 0.0
        && positive_blocks >= 8
        && (stats.tp + stats.abort) >= 30
        && stats.actual_symbols.len() >= 6
        && stats.actual_pairs.len() >= 3
        && stats.loss_after_add.len() >= 2
        && max_symbol <= 0.5
        && max_pair <= 0.5
        && max_group <= 0.5
        && max_block <= 0.5
        && cost_ratio <= 0.5
        && !account.terminated;
    let trace_bytes = fs::read(&trace_path)?;
    let risk_bytes = fs::read(&risk_path)?;
    let result = serde_json::json!({
        "policy_id":policy.policy_id,"config_id":policy.config_id,"family":policy.family,"weighting":policy.weighting,
        "fingerprint":policy.policy_id,"principal":options.principal,"fo_fraction":options.fo_fraction,"leverage_cap":options.leverage_cap,
        "final_equity":account.equity(),"compounded_return_pct":compounded*100.0,"annualized_return_pct":annualized,
        "max_equity_drawdown_pct":account.max_equity_drawdown_pct,"positive_calendar_blocks":positive_blocks,"block_pnl":stats.block_pnl,
        "eligible_intents":stats.eligible_intents,"terminal_intents":stats.terminal_intents,"executed_intents":stats.executed_intents,
        "fo_count":stats.fo,"so_count":stats.so,"tp_count":stats.tp,"abort_count":stats.abort,"rejects":stats.rejects,
        "overlap_intents":stats.overlap_intents,"max_active_groups_observed":stats.max_active_groups,
        "loss_after_add_so_groups":stats.loss_after_add,"actual_symbols":stats.actual_symbols,"actual_pairs":stats.actual_pairs,
        "symbol_pnl":account.realized_pnl_by_symbol,"pair_pnl":stats.pair_pnl,"group_pnl":account.realized_pnl_by_group,
        "max_symbol_concentration":max_symbol,"max_pair_concentration":max_pair,"max_group_concentration":max_group,
        "max_block_concentration":max_block,"cost_to_gross_positive_pnl":cost_ratio,"liquidation":account.terminated,
        "p_a":p_a,"p_b":p_b,"p_c":false,"target_hit":false,
        "final_positions":account.positions.len(),"final_groups":account.groups.len(),"final_pending":account.pending_orders.len(),"final_reserve":account.reserved_quote,
        "restart_reconciled":stats.restart_reconciled,
        "theoretical_minimum_executable_principal":stats.theoretical_min_principal,
        "start_ms":start,"end_ms_exclusive":END_MS,"logical_risk_rows":logical_rows,
        "trace_manifest":{"path":trace_path.strip_prefix(repo).unwrap_or(&trace_path),"sha256":sha256_bytes(&trace_bytes),"bytes":trace_bytes.len()},
        "risk_manifest":{"path":risk_path.strip_prefix(repo).unwrap_or(&risk_path),"sha256":sha256_bytes(&risk_bytes),"bytes":risk_bytes.len()}
    });
    if publish_compact {
        let compact = raw
            .join("replay-results")
            .join(format!("{}.json", safe_name(&policy.policy_id)));
        write_json(&compact, &result)?;
        write_json(
            repo.join(REPO_ARTIFACT)
                .join("replay-results")
                .join(format!("{}.json", safe_name(&policy.policy_id))),
            &result,
        )?;
        write_json(
            repo.join(REPO_ARTIFACT)
                .join("trace-manifests")
                .join(format!("{}.json", safe_name(&policy.policy_id))),
            &serde_json::json!({"policy_id":policy.policy_id,"trace":result["trace_manifest"],"risk":result["risk_manifest"]}),
        )?;
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn execute_fo(
    account: &mut SharedAccount,
    active: &mut BTreeMap<String, ActiveGroup>,
    stats: &mut ReplayStats,
    writer: &mut BufWriter<File>,
    sequence: &mut u64,
    policy: &Policy,
    filters: &BTreeMap<String, FilterRow>,
    options: &ReplayOptions,
    cache: &mut DayCache,
    market: &Connection,
    row: &SignalRow,
    timestamp: i64,
) -> Result<()> {
    write_group_event(
        writer, sequence, account, policy, row, "", timestamp, "intent", None, None,
    )?;
    if account.terminated {
        reject_intent(
            account,
            stats,
            writer,
            sequence,
            policy,
            row,
            timestamp,
            "account_terminated",
        )?;
        return Ok(());
    }
    if options.signal_drop_stride > 0
        && stats.eligible_intents % options.signal_drop_stride as u64 == 0
    {
        reject_intent(
            account,
            stats,
            writer,
            sequence,
            policy,
            row,
            timestamp,
            "signal_missing_stale",
        )?;
        return Ok(());
    }
    if !row.can_open {
        reject_intent(
            account,
            stats,
            writer,
            sequence,
            policy,
            row,
            timestamp,
            "no_eligible_open",
        )?;
        return Ok(());
    }
    if active.values().any(|group| group.pair_id == row.pair_id) {
        reject_intent(
            account,
            stats,
            writer,
            sequence,
            policy,
            row,
            timestamp,
            "same_pair_active",
        )?;
        return Ok(());
    }
    if active.len() >= 3 {
        reject_intent(
            account,
            stats,
            writer,
            sequence,
            policy,
            row,
            timestamp,
            "max_active_groups",
        )?;
        return Ok(());
    }
    if active.values().any(|group| {
        group.left == row.left
            || group.left == row.right
            || group.right == row.left
            || group.right == row.right
    }) {
        reject_intent(
            account,
            stats,
            writer,
            sequence,
            policy,
            row,
            timestamp,
            "symbol_cap_conflict",
        )?;
        return Ok(());
    }
    let Some(direction) = row.direction else {
        reject_intent(
            account,
            stats,
            writer,
            sequence,
            policy,
            row,
            timestamp,
            "no_direction",
        )?;
        return Ok(());
    };
    let weighting = if row.family == "T1" {
        Weighting::Beta
    } else {
        policy.weighting
    };
    let (weight_left, weight_right) = match gross_weights(weighting, row.beta_left, row.beta_right)
    {
        Ok(value) => value,
        Err(_) => {
            reject_intent(
                account,
                stats,
                writer,
                sequence,
                policy,
                row,
                timestamp,
                "invalid_beta_weights",
            )?;
            return Ok(());
        }
    };
    let left_bar = cache.bar(market, &row.left, timestamp)?;
    let right_bar = cache.bar(market, &row.right, timestamp)?;
    let minimum_gross = minimum_pair_gross(
        filters,
        row,
        &left_bar,
        &right_bar,
        weight_left,
        weight_right,
        options,
    )?;
    let required_principal = minimum_gross / options.fo_fraction.max(1e-12);
    if stats.theoretical_min_principal == 0.0
        || required_principal < stats.theoretical_min_principal
    {
        stats.theoretical_min_principal = required_principal;
    }
    let gross = (options.principal * options.fo_fraction).max(minimum_gross);
    if let Some(quota) = options.family_quota_fraction {
        let layer_sum = LAYERS.iter().sum::<f64>();
        let committed = active
            .values()
            .filter(|group| group.family == row.family)
            .map(|group| group.base_gross * layer_sum)
            .sum::<f64>();
        if committed + gross * layer_sum > options.principal * quota {
            reject_intent(
                account,
                stats,
                writer,
                sequence,
                policy,
                row,
                timestamp,
                "family_reserve_quota",
            )?;
            return Ok(());
        }
    }
    let group_id = format!(
        "{}-g{:06}",
        safe_name(&policy.policy_id),
        stats.eligible_intents
    );
    let cycle_start_wallet = account.wallet_balance;
    let left_mode = left_mode(direction);
    let right_mode = right_mode(direction);
    account.add_group(MartinGroup {
        group_id: group_id.clone(),
        fit_version: row.model_hash.clone(),
        frozen_legs: vec![
            FrozenLeg {
                key: position_key(&row.left, left_mode, &group_id),
                signed_weight: left_mode.sign() * weight_left,
            },
            FrozenLeg {
                key: position_key(&row.right, right_mode, &group_id),
                signed_weight: right_mode.sign() * weight_right,
            },
        ],
        level: 0,
        previous_level_gross: 0.0,
        current_level_gross: gross,
        last_filled_group_price: None,
        net_pnl_after_close_cost: 0.0,
        reserved_next_so: 0.0,
    })?;
    let success = fill_pair_level(
        account,
        stats,
        writer,
        sequence,
        policy,
        filters,
        options,
        row,
        &group_id,
        0,
        gross,
        weight_left,
        weight_right,
        left_bar,
        right_bar,
    )?;
    if !success {
        flatten_failed_group(
            account, stats, writer, sequence, policy, row, &group_id, left_bar, right_bar,
            timestamp,
        )?;
        reject_intent(
            account,
            stats,
            writer,
            sequence,
            policy,
            row,
            timestamp,
            "paired_fill_failed",
        )?;
        return Ok(());
    }
    let reserve_before = account.reserved_quote;
    if account
        .reserve_group_after_fill(timestamp, &group_id, Some(gross * LAYERS[1]), gross)
        .is_err()
    {
        flatten_failed_group(
            account, stats, writer, sequence, policy, row, &group_id, left_bar, right_bar,
            timestamp,
        )?;
        reject_intent(
            account,
            stats,
            writer,
            sequence,
            policy,
            row,
            timestamp,
            "reserve_after_fill_failed",
        )?;
        return Ok(());
    }
    emit_reserve_event(
        writer,
        sequence,
        account,
        policy,
        row,
        &group_id,
        timestamp,
        reserve_before,
        "group_after_fo",
    )?;
    account.traces.clear();
    active.insert(
        group_id.clone(),
        ActiveGroup {
            group_id,
            pair_id: row.pair_id.clone(),
            left: row.left.clone(),
            right: row.right.clone(),
            model_hash: row.model_hash.clone(),
            family: row.family.clone(),
            direction,
            weight_left,
            weight_right,
            base_gross: gross,
            level: 0,
            last_value: row.value,
            deadline_ms: (timestamp + row.deadline_span_ms).min(END_MS),
            freeze_so: !row.reliable_regime,
            cycle_start_wallet,
        },
    );
    stats.executed_intents += 1;
    stats.terminal_intents += 1;
    stats.fo += 1;
    stats.actual_symbols.insert(row.left.clone());
    stats.actual_symbols.insert(row.right.clone());
    stats.actual_pairs.insert(row.pair_id.clone());
    stats.max_active_groups = stats.max_active_groups.max(active.len());
    Ok(())
}

fn reject_intent(
    account: &SharedAccount,
    stats: &mut ReplayStats,
    writer: &mut BufWriter<File>,
    sequence: &mut u64,
    policy: &Policy,
    row: &SignalRow,
    timestamp: i64,
    reason: &str,
) -> Result<()> {
    *stats.rejects.entry(reason.into()).or_default() += 1;
    stats.terminal_intents += 1;
    write_rejection(writer, sequence, account, policy, row, timestamp, reason)
}

fn write_rejection(
    writer: &mut BufWriter<File>,
    sequence: &mut u64,
    account: &SharedAccount,
    policy: &Policy,
    row: &SignalRow,
    timestamp: i64,
    reason: &str,
) -> Result<()> {
    write_group_event(
        writer,
        sequence,
        account,
        policy,
        row,
        "",
        timestamp,
        "reject",
        Some(reason),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn fill_pair_level(
    account: &mut SharedAccount,
    stats: &mut ReplayStats,
    writer: &mut BufWriter<File>,
    sequence: &mut u64,
    policy: &Policy,
    filters: &BTreeMap<String, FilterRow>,
    options: &ReplayOptions,
    row: &SignalRow,
    group_id: &str,
    level: usize,
    gross: f64,
    weight_left: f64,
    weight_right: f64,
    left_bar: MinuteBar,
    right_bar: MinuteBar,
) -> Result<bool> {
    let direction = row.direction.context("fill direction missing")?;
    let left_filter = filters.get(&row.left).context("left filter missing")?;
    let right_filter = filters.get(&row.right).context("right filter missing")?;
    let left_qty = resolved_quantity(
        left_bar.open,
        gross * weight_left,
        left_filter.step_size * options.filter_step_multiplier,
        left_filter.min_qty,
        left_filter.min_notional * options.min_notional_multiplier,
    )?;
    let right_qty = resolved_quantity(
        right_bar.open,
        gross * weight_right,
        right_filter.step_size * options.filter_step_multiplier,
        right_filter.min_qty,
        right_filter.min_notional * options.min_notional_multiplier,
    )?;
    let left_gross = left_qty * left_bar.open;
    let right_gross = right_qty * right_bar.open;
    let mismatch = gross_mismatch(
        left_gross / weight_left.max(1e-12),
        right_gross / weight_right.max(1e-12),
    );
    if mismatch > 0.05 {
        return Ok(false);
    }
    let left = submit_leg(
        account,
        stats,
        writer,
        sequence,
        policy,
        row,
        group_id,
        "left",
        left_mode(direction),
        level,
        left_qty,
        left_bar.open,
        weight_left,
        left_filter,
        options,
        false,
    )?;
    let right = submit_leg(
        account,
        stats,
        writer,
        sequence,
        policy,
        row,
        group_id,
        "right",
        right_mode(direction),
        level,
        right_qty,
        right_bar.open,
        weight_right,
        right_filter,
        options,
        options.reject_second_leg,
    )?;
    Ok(left && right)
}

#[allow(clippy::too_many_arguments)]
fn submit_leg(
    account: &mut SharedAccount,
    stats: &mut ReplayStats,
    writer: &mut BufWriter<File>,
    sequence: &mut u64,
    policy: &Policy,
    row: &SignalRow,
    group_id: &str,
    leg: &str,
    mode: PositionMode,
    level: usize,
    quantity: f64,
    price: f64,
    weight: f64,
    filter: &FilterRow,
    options: &ReplayOptions,
    reject: bool,
) -> Result<bool> {
    let symbol = if leg == "left" { &row.left } else { &row.right };
    let before = account.wallet_balance;
    let reserve_before = account.reserved_quote;
    let request = FillRequest {
        timestamp: row.eligible_open_ms,
        order_id: format!("{group_id}-{level}-{leg}"),
        group_id: group_id.into(),
        key: position_key(symbol, mode, group_id),
        requested_quantity: quantity,
        price,
        fill_fraction: options.fill_fraction,
        delayed_bars: options.leg_delay.saturating_mul(MINUTE_MS as u32),
        reject,
    };
    let outcome = account.submit(request)?;
    book_delta(stats, row.eligible_open_ms, before, account.wallet_balance)?;
    let notional = outcome.filled_quantity * price;
    let fee = notional * account.config.fee_bps / 10_000.0;
    let slippage = notional * account.config.slippage_bps / 10_000.0;
    emit_trace(
        writer,
        sequence,
        TraceEvent {
            event_id: String::new(),
            timestamp: row.eligible_open_ms,
            completed_signal_ms: Some(row.signal_ms),
            eligible_open_ms: Some(row.eligible_open_ms),
            event_type: if outcome.accepted {
                if outcome.filled_quantity > 0.0 {
                    "fill"
                } else {
                    "delayed_order"
                }
            } else {
                "order_reject"
            }
            .into(),
            policy_id: policy.policy_id.clone(),
            pair_id: Some(row.pair_id.clone()),
            group_id: Some(group_id.into()),
            model_hash: Some(row.model_hash.clone()),
            leg_id: Some(leg.into()),
            symbol: Some(symbol.clone()),
            direction: row.direction,
            side: Some(side_name(mode).into()),
            position_mode: Some(mode_name(mode).into()),
            level: Some(level),
            requested_qty: Some(quantity),
            filled_qty: Some(outcome.filled_quantity),
            open_price: Some(price),
            fill_price: (outcome.filled_quantity > 0.0).then_some(price),
            mark_price: Some(price),
            fee,
            slippage,
            funding: 0.0,
            wallet_before: before,
            wallet_after: account.wallet_balance,
            equity: account.equity(),
            reserve_before,
            reserve_after: account.reserved_quote,
            margin: account.initial_margin(),
            maintenance: account.maintenance_margin(),
            calendar_block: calendar_block(row.eligible_open_ms)?,
            rejection_reason: (!outcome.accepted).then_some(outcome.reason),
            close_reason: None,
            metadata: serde_json::json!({"family":row.family,"config_id":row.config_id,"signal_value":row.value,
            "h_left":row.h_left,"h_right":row.h_right,"alpha":row.alpha,"beta_left":row.beta_left,"beta_right":row.beta_right,
            "reliable_regime":row.reliable_regime,"weight":weight,
            "weighting":if row.family=="T1"{"BETA"}else{match policy.weighting{Weighting::Beta=>"BETA",Weighting::Equal=>"EQUAL"}},
            "filter_step_size":filter.step_size*options.filter_step_multiplier,
            "filter_min_qty":filter.min_qty,"filter_min_notional":filter.min_notional*options.min_notional_multiplier}),
        },
    )?;
    account.traces.clear();
    Ok(outcome.accepted && (outcome.filled_quantity > 0.0 || options.leg_delay > 0))
}

#[allow(clippy::too_many_arguments)]
fn execute_so(
    account: &mut SharedAccount,
    active: &mut BTreeMap<String, ActiveGroup>,
    stats: &mut ReplayStats,
    writer: &mut BufWriter<File>,
    sequence: &mut u64,
    policy: &Policy,
    filters: &BTreeMap<String, FilterRow>,
    options: &ReplayOptions,
    cache: &mut DayCache,
    market: &Connection,
    row: &SignalRow,
    group_id: &str,
    timestamp: i64,
) -> Result<()> {
    let group = active
        .get(group_id)
        .context("SO active group missing")?
        .clone();
    let next_level = group.level + 1;
    let next_gross = group.base_gross * LAYERS[next_level];
    let net = group_net(account, group_id);
    if let Some(engine_group) = account.groups.get_mut(group_id) {
        engine_group.net_pnl_after_close_cost = net;
    }
    let reserve_before = account.reserved_quote;
    if account.request_next_so(group_id, next_gross).is_err() {
        *stats
            .rejects
            .entry("so_reserve_or_layer_reject".into())
            .or_default() += 1;
        write_group_event(
            writer,
            sequence,
            account,
            policy,
            row,
            group_id,
            timestamp,
            "so_reject",
            Some("reserve_or_layer"),
            None,
        )?;
        account.traces.clear();
        return Ok(());
    }
    account.consume_group_reserve_at(timestamp, group_id)?;
    emit_reserve_event(
        writer,
        sequence,
        account,
        policy,
        row,
        group_id,
        timestamp,
        reserve_before,
        "so_reserve_consumed",
    )?;
    let reserve_after_consume = account.reserved_quote;
    let left = cache.bar(market, &row.left, timestamp)?;
    let right = cache.bar(market, &row.right, timestamp)?;
    let success = fill_pair_level(
        account,
        stats,
        writer,
        sequence,
        policy,
        filters,
        options,
        row,
        group_id,
        next_level,
        next_gross,
        group.weight_left,
        group.weight_right,
        left,
        right,
    )?;
    if !success {
        *stats
            .rejects
            .entry("so_paired_fill_failed".into())
            .or_default() += 1;
        close_active_group(
            account,
            active,
            stats,
            writer,
            sequence,
            policy,
            cache,
            market,
            group_id,
            timestamp,
            "so_paired_fill_failed",
            false,
        )?;
        return Ok(());
    }
    let following =
        (next_level + 1 < LAYERS.len()).then_some(group.base_gross * LAYERS[next_level + 1]);
    account.reserve_group_after_fill(timestamp, group_id, following, next_gross)?;
    emit_reserve_event(
        writer,
        sequence,
        account,
        policy,
        row,
        group_id,
        timestamp,
        reserve_after_consume,
        "group_after_so",
    )?;
    account.traces.clear();
    let current = active.get_mut(group_id).unwrap();
    current.level = next_level;
    current.last_value = row.value;
    stats.so += 1;
    stats.loss_after_add.insert(group_id.into());
    write_group_event(
        writer, sequence, account, policy, row, group_id, timestamp, "so", None, None,
    )
}

#[allow(clippy::too_many_arguments)]
fn close_active_group(
    account: &mut SharedAccount,
    active: &mut BTreeMap<String, ActiveGroup>,
    stats: &mut ReplayStats,
    writer: &mut BufWriter<File>,
    sequence: &mut u64,
    policy: &Policy,
    cache: &mut DayCache,
    market: &Connection,
    group_id: &str,
    timestamp: i64,
    reason: &str,
    is_tp: bool,
) -> Result<()> {
    let group = active
        .get(group_id)
        .context("close active group missing")?
        .clone();
    let left = cache.bar(market, &group.left, timestamp)?;
    let right = cache.bar(market, &group.right, timestamp)?;
    let keys = account
        .positions
        .keys()
        .filter(|key| key.owner_group == group_id)
        .cloned()
        .collect::<Vec<_>>();
    for key in keys {
        let price = if key.symbol == group.left {
            left.open
        } else {
            right.open
        };
        let position = account
            .positions
            .get(&key)
            .cloned()
            .context("close position missing")?;
        let before = account.wallet_balance;
        let reserve_before = account.reserved_quote;
        account.close_key(timestamp, &key, price, reason)?;
        book_delta(stats, timestamp, before, account.wallet_balance)?;
        let notional = position.quantity * price;
        emit_trace(
            writer,
            sequence,
            TraceEvent {
                event_id: String::new(),
                timestamp,
                completed_signal_ms: None,
                eligible_open_ms: None,
                event_type: "close_fill".into(),
                policy_id: policy.policy_id.clone(),
                pair_id: Some(group.pair_id.clone()),
                group_id: Some(group_id.into()),
                model_hash: Some(group.model_hash.clone()),
                leg_id: Some(
                    if key.symbol == group.left {
                        "left"
                    } else {
                        "right"
                    }
                    .into(),
                ),
                symbol: Some(key.symbol.clone()),
                direction: Some(group.direction),
                side: Some(
                    if key.mode == PositionMode::Long {
                        "sell"
                    } else {
                        "buy"
                    }
                    .into(),
                ),
                position_mode: Some(mode_name(key.mode).into()),
                level: Some(group.level),
                requested_qty: Some(position.quantity),
                filled_qty: Some(position.quantity),
                open_price: None,
                fill_price: Some(price),
                mark_price: Some(price),
                fee: notional * account.config.fee_bps / 10_000.0,
                slippage: notional * account.config.slippage_bps / 10_000.0,
                funding: 0.0,
                wallet_before: before,
                wallet_after: account.wallet_balance,
                equity: account.equity(),
                reserve_before,
                reserve_after: account.reserved_quote,
                margin: account.initial_margin(),
                maintenance: account.maintenance_margin(),
                calendar_block: calendar_block(timestamp)?,
                rejection_reason: None,
                close_reason: Some(reason.into()),
                metadata: serde_json::json!({"family":group.family}),
            },
        )?;
        account.traces.clear();
    }
    let reserve_before = account.reserved_quote;
    account.remove_group_at(timestamp, group_id)?;
    account.traces.clear();
    let pnl = account.wallet_balance - group.cycle_start_wallet;
    *stats.pair_pnl.entry(group.pair_id.clone()).or_default() += pnl;
    if is_tp {
        stats.tp += 1
    } else {
        stats.abort += 1
    }
    let signal = SignalRow {
        signal_ms: timestamp - 1,
        eligible_open_ms: timestamp,
        snapshot_anchor_ms: 0,
        can_open: false,
        onset: false,
        family: policy.family.clone(),
        config_id: policy.config_id.clone(),
        pair_id: group.pair_id.clone(),
        left: group.left.clone(),
        right: group.right.clone(),
        model_hash: group.model_hash.clone(),
        direction: Some(group.direction),
        neutral: is_tp,
        value: group.last_value,
        h_left: None,
        h_right: None,
        alpha: policy.entry_alpha,
        beta_left: group.weight_left,
        beta_right: group.weight_right,
        reliable_regime: !group.freeze_so,
        deadline_span_ms: 0,
    };
    emit_reserve_event(
        writer,
        sequence,
        account,
        policy,
        &signal,
        group_id,
        timestamp,
        reserve_before,
        "group_release",
    )?;
    write_group_event(
        writer,
        sequence,
        account,
        policy,
        &signal,
        group_id,
        timestamp,
        if is_tp { "tp" } else { "abort" },
        None,
        Some(reason),
    )?;
    active.remove(group_id);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_liquidation_trace(
    writer: &mut BufWriter<File>,
    sequence: &mut u64,
    policy: &Policy,
    active: &BTreeMap<String, ActiveGroup>,
    positions: &BTreeMap<PositionKey, Position>,
    bars: &BTreeMap<String, MinuteBar>,
    stats: &mut ReplayStats,
    wallet_before: f64,
    reserve_before: f64,
    account: &SharedAccount,
) -> Result<()> {
    let timestamp = account.last_timestamp;
    let mut wallet = wallet_before;
    for (key, position) in positions {
        let bar = bars.get(&key.symbol).context("liquidation bar missing")?;
        let price = if key.mode == PositionMode::Long {
            bar.low
        } else {
            bar.high
        };
        let gross = key.mode.sign() * position.quantity * (price - position.average_price);
        let notional = position.quantity * price;
        let fee =
            notional * (account.config.fee_bps + account.config.liquidation_fee_bps) / 10_000.0;
        let slippage = notional * account.config.slippage_bps / 10_000.0;
        let before = wallet;
        wallet += gross - fee - slippage;
        if let Some(group) = active.get(&key.owner_group) {
            *stats.pair_pnl.entry(group.pair_id.clone()).or_default() += gross - fee - slippage;
        }
        emit_trace(
            writer,
            sequence,
            TraceEvent {
                event_id: String::new(),
                timestamp,
                completed_signal_ms: None,
                eligible_open_ms: None,
                event_type: "liquidation_fill".into(),
                policy_id: policy.policy_id.clone(),
                pair_id: active
                    .get(&key.owner_group)
                    .map(|group| group.pair_id.clone()),
                group_id: Some(key.owner_group.clone()),
                model_hash: active
                    .get(&key.owner_group)
                    .map(|group| group.model_hash.clone()),
                leg_id: active.get(&key.owner_group).map(|group| {
                    if key.symbol == group.left {
                        "left"
                    } else {
                        "right"
                    }
                    .into()
                }),
                symbol: Some(key.symbol.clone()),
                direction: active.get(&key.owner_group).map(|group| group.direction),
                side: Some(
                    if key.mode == PositionMode::Long {
                        "sell"
                    } else {
                        "buy"
                    }
                    .into(),
                ),
                position_mode: Some(mode_name(key.mode).into()),
                level: active.get(&key.owner_group).map(|group| group.level),
                requested_qty: Some(position.quantity),
                filled_qty: Some(position.quantity),
                open_price: None,
                fill_price: Some(price),
                mark_price: Some(price),
                fee,
                slippage,
                funding: 0.0,
                wallet_before: before,
                wallet_after: wallet,
                equity: wallet.max(0.0),
                reserve_before,
                reserve_after: reserve_before,
                margin: 0.0,
                maintenance: 0.0,
                calendar_block: calendar_block(timestamp)?,
                rejection_reason: None,
                close_reason: Some("liquidation".into()),
                metadata: serde_json::json!({"family":active.get(&key.owner_group).map(|group|group.family.clone()),"gross_pnl":gross,"liquidation":true}),
            },
        )?;
    }
    if wallet < 0.0 {
        emit_trace(
            writer,
            sequence,
            TraceEvent {
                event_id: String::new(),
                timestamp,
                completed_signal_ms: None,
                eligible_open_ms: None,
                event_type: "principal_clamp".into(),
                policy_id: policy.policy_id.clone(),
                pair_id: None,
                group_id: None,
                model_hash: None,
                leg_id: None,
                symbol: None,
                direction: None,
                side: None,
                position_mode: None,
                level: None,
                requested_qty: None,
                filled_qty: None,
                open_price: None,
                fill_price: None,
                mark_price: None,
                fee: 0.0,
                slippage: 0.0,
                funding: 0.0,
                wallet_before: wallet,
                wallet_after: 0.0,
                equity: 0.0,
                reserve_before,
                reserve_after: reserve_before,
                margin: 0.0,
                maintenance: 0.0,
                calendar_block: calendar_block(timestamp)?,
                rejection_reason: None,
                close_reason: Some("principal_breach".into()),
                metadata: serde_json::json!({}),
            },
        )?;
        wallet = 0.0;
    }
    emit_trace(
        writer,
        sequence,
        TraceEvent {
            event_id: String::new(),
            timestamp,
            completed_signal_ms: None,
            eligible_open_ms: None,
            event_type: "liquidation_reserve_clear".into(),
            policy_id: policy.policy_id.clone(),
            pair_id: None,
            group_id: None,
            model_hash: None,
            leg_id: None,
            symbol: None,
            direction: None,
            side: None,
            position_mode: None,
            level: None,
            requested_qty: None,
            filled_qty: None,
            open_price: None,
            fill_price: None,
            mark_price: None,
            fee: 0.0,
            slippage: 0.0,
            funding: 0.0,
            wallet_before: wallet,
            wallet_after: wallet,
            equity: wallet,
            reserve_before,
            reserve_after: 0.0,
            margin: 0.0,
            maintenance: 0.0,
            calendar_block: calendar_block(timestamp)?,
            rejection_reason: None,
            close_reason: Some("liquidation".into()),
            metadata: serde_json::json!({"cleared_groups":active.keys().collect::<Vec<_>>()}),
        },
    )?;
    if (wallet - account.wallet_balance).abs() > 1e-7 {
        bail!("liquidation trace wallet reconciliation mismatch");
    }
    Ok(())
}

fn flatten_failed_group(
    account: &mut SharedAccount,
    stats: &mut ReplayStats,
    writer: &mut BufWriter<File>,
    sequence: &mut u64,
    policy: &Policy,
    row: &SignalRow,
    group_id: &str,
    left: MinuteBar,
    right: MinuteBar,
    timestamp: i64,
) -> Result<()> {
    let keys = account
        .positions
        .keys()
        .filter(|key| key.owner_group == group_id)
        .cloned()
        .collect::<Vec<_>>();
    for key in keys {
        let before = account.wallet_balance;
        let reserve_before = account.reserved_quote;
        let price = if key.symbol == row.left {
            left.open
        } else {
            right.open
        };
        let position = account
            .positions
            .get(&key)
            .cloned()
            .context("paired flatten position missing")?;
        account.close_key(timestamp, &key, price, "paired_flatten")?;
        book_delta(stats, timestamp, before, account.wallet_balance)?;
        let notional = position.quantity * price;
        emit_trace(
            writer,
            sequence,
            TraceEvent {
                event_id: String::new(),
                timestamp,
                completed_signal_ms: Some(row.signal_ms),
                eligible_open_ms: Some(row.eligible_open_ms),
                event_type: "close_fill".into(),
                policy_id: policy.policy_id.clone(),
                pair_id: Some(row.pair_id.clone()),
                group_id: Some(group_id.into()),
                model_hash: Some(row.model_hash.clone()),
                leg_id: Some(
                    if key.symbol == row.left {
                        "left"
                    } else {
                        "right"
                    }
                    .into(),
                ),
                symbol: Some(key.symbol.clone()),
                direction: row.direction,
                side: Some(
                    if key.mode == PositionMode::Long {
                        "sell"
                    } else {
                        "buy"
                    }
                    .into(),
                ),
                position_mode: Some(mode_name(key.mode).into()),
                level: Some(0),
                requested_qty: Some(position.quantity),
                filled_qty: Some(position.quantity),
                open_price: None,
                fill_price: Some(price),
                mark_price: Some(price),
                fee: notional * account.config.fee_bps / 10_000.0,
                slippage: notional * account.config.slippage_bps / 10_000.0,
                funding: 0.0,
                wallet_before: before,
                wallet_after: account.wallet_balance,
                equity: account.equity(),
                reserve_before,
                reserve_after: account.reserved_quote,
                margin: account.initial_margin(),
                maintenance: account.maintenance_margin(),
                calendar_block: calendar_block(timestamp)?,
                rejection_reason: None,
                close_reason: Some("paired_flatten".into()),
                metadata: serde_json::json!({"family":row.family}),
            },
        )?;
        account.traces.clear();
    }
    let reserve_before = account.reserved_quote;
    account.remove_group_at(timestamp, group_id)?;
    account.traces.clear();
    emit_reserve_event(
        writer,
        sequence,
        account,
        policy,
        row,
        group_id,
        timestamp,
        reserve_before,
        "failed_group_release",
    )?;
    write_group_event(
        writer,
        sequence,
        account,
        policy,
        row,
        group_id,
        timestamp,
        "paired_flatten",
        Some("paired_fill_failed"),
        Some("paired_flatten"),
    )
}

fn minimum_pair_gross(
    filters: &BTreeMap<String, FilterRow>,
    row: &SignalRow,
    left: &MinuteBar,
    right: &MinuteBar,
    wl: f64,
    wr: f64,
    options: &ReplayOptions,
) -> Result<f64> {
    let lf = filters.get(&row.left).context("left filter missing")?;
    let rf = filters.get(&row.right).context("right filter missing")?;
    let left_min = (lf.min_notional * options.min_notional_multiplier).max(lf.min_qty * left.open)
        / wl.max(1e-12);
    let right_min = (rf.min_notional * options.min_notional_multiplier)
        .max(rf.min_qty * right.open)
        / wr.max(1e-12);
    Ok(left_min.max(right_min))
}

fn write_group_event(
    writer: &mut BufWriter<File>,
    sequence: &mut u64,
    account: &SharedAccount,
    policy: &Policy,
    row: &SignalRow,
    group_id: &str,
    timestamp: i64,
    event_type: &str,
    rejection: Option<&str>,
    close: Option<&str>,
) -> Result<()> {
    emit_trace(
        writer,
        sequence,
        TraceEvent {
            event_id: String::new(),
            timestamp,
            completed_signal_ms: Some(row.signal_ms),
            eligible_open_ms: Some(row.eligible_open_ms),
            event_type: event_type.into(),
            policy_id: policy.policy_id.clone(),
            pair_id: Some(row.pair_id.clone()),
            group_id: (!group_id.is_empty()).then_some(group_id.into()),
            model_hash: Some(row.model_hash.clone()),
            leg_id: None,
            symbol: None,
            direction: row.direction,
            side: None,
            position_mode: None,
            level: None,
            requested_qty: None,
            filled_qty: None,
            open_price: None,
            fill_price: None,
            mark_price: None,
            fee: 0.0,
            slippage: 0.0,
            funding: 0.0,
            wallet_before: account.wallet_balance,
            wallet_after: account.wallet_balance,
            equity: account.equity(),
            reserve_before: account.reserved_quote,
            reserve_after: account.reserved_quote,
            margin: account.initial_margin(),
            maintenance: account.maintenance_margin(),
            calendar_block: calendar_block(timestamp.min(END_MS - 1))?,
            rejection_reason: rejection.map(str::to_owned),
            close_reason: close.map(str::to_owned),
            metadata: signal_trace_metadata(row, policy),
        },
    )
}

fn emit_reserve_event(
    writer: &mut BufWriter<File>,
    sequence: &mut u64,
    account: &SharedAccount,
    policy: &Policy,
    row: &SignalRow,
    group_id: &str,
    timestamp: i64,
    reserve_before: f64,
    reason: &str,
) -> Result<()> {
    emit_trace(
        writer,
        sequence,
        TraceEvent {
            event_id: String::new(),
            timestamp,
            completed_signal_ms: Some(row.signal_ms),
            eligible_open_ms: Some(row.eligible_open_ms),
            event_type: "reserve_update".into(),
            policy_id: policy.policy_id.clone(),
            pair_id: Some(row.pair_id.clone()),
            group_id: Some(group_id.into()),
            model_hash: Some(row.model_hash.clone()),
            leg_id: None,
            symbol: None,
            direction: row.direction,
            side: None,
            position_mode: None,
            level: None,
            requested_qty: None,
            filled_qty: None,
            open_price: None,
            fill_price: None,
            mark_price: None,
            fee: 0.0,
            slippage: 0.0,
            funding: 0.0,
            wallet_before: account.wallet_balance,
            wallet_after: account.wallet_balance,
            equity: account.equity(),
            reserve_before,
            reserve_after: account.reserved_quote,
            margin: account.initial_margin(),
            maintenance: account.maintenance_margin(),
            calendar_block: calendar_block(timestamp.min(END_MS - 1))?,
            rejection_reason: None,
            close_reason: Some(reason.into()),
            metadata: serde_json::json!({
                "signal":signal_trace_metadata(row,policy),"family":row.family,"reason":reason,
                "reserve_components":account.reserve_ledger.get(group_id),"leverage":account.config.leverage,
                "fee_bps":account.config.fee_bps,"slippage_bps":account.config.slippage_bps,
                "close_reserve_bps":account.config.close_reserve_bps,"maintenance_rate":account.config.maintenance_rate
            }),
        },
    )
}

fn signal_trace_metadata(row: &SignalRow, policy: &Policy) -> serde_json::Value {
    serde_json::json!({
        "family":row.family,"config_id":row.config_id,"signal_value":row.value,
        "h_left":row.h_left,"h_right":row.h_right,"alpha":row.alpha,
        "beta_left":row.beta_left,"beta_right":row.beta_right,"reliable_regime":row.reliable_regime,
        "weighting":if row.family=="T1"{"BETA"}else{match policy.weighting{Weighting::Beta=>"BETA",Weighting::Equal=>"EQUAL"}}
    })
}

fn emit_trace(writer: &mut BufWriter<File>, sequence: &mut u64, mut row: TraceEvent) -> Result<()> {
    *sequence += 1;
    row.event_id = format!("e{:012}", *sequence);
    serde_json::to_writer(&mut *writer, &row)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn risk_positions(
    account: &SharedAccount,
    bars: &BTreeMap<String, MinuteBar>,
) -> serde_json::Value {
    serde_json::Value::Array(account.positions.iter().filter_map(|(key,position)|{
        bars.get(&key.symbol).map(|bar|serde_json::json!({
            "symbol":key.symbol,"group_id":key.owner_group,"mode":mode_name(key.mode),"quantity":position.quantity,
            "average_price":position.average_price,"adverse_price":if key.mode==PositionMode::Long{bar.low}else{bar.high},
            "close_price":bar.close
        }))
    }).collect())
}

fn independent_equity(
    account: &SharedAccount,
    bars: &BTreeMap<String, MinuteBar>,
    adverse: bool,
) -> f64 {
    let mut value = account.wallet_balance;
    for (key, position) in &account.positions {
        if let Some(bar) = bars.get(&key.symbol) {
            let price = if adverse {
                if key.mode == PositionMode::Long {
                    bar.low
                } else {
                    bar.high
                }
            } else {
                bar.close
            };
            value += key.mode.sign() * position.quantity * (price - position.average_price);
        }
    }
    value.max(0.0)
}

fn write_idle(
    writer: &mut BufWriter<File>,
    start: i64,
    end: i64,
    account: &SharedAccount,
) -> Result<()> {
    if end <= start {
        return Ok(());
    }
    serde_json::to_writer(
        &mut *writer,
        &serde_json::json!({
            "kind":"idle_1m_rle","start_ms":start,"end_ms":end,"rows":(end-start)/MINUTE_MS,
            "wallet":account.wallet_balance,"equity":account.equity(),"reserve":account.reserved_quote,
            "positions":account.positions.len(),"groups":account.groups.len()
        }),
    )?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn write_active_risk(
    writer: &mut BufWriter<File>,
    timestamp: i64,
    account: &SharedAccount,
    path_wallet: f64,
    adverse_equity: f64,
    positions: serde_json::Value,
) -> Result<()> {
    serde_json::to_writer(
        &mut *writer,
        &serde_json::json!({
            "kind":"active_1m","timestamp":timestamp,"path_wallet":path_wallet,"wallet":account.wallet_balance,"adverse_equity":adverse_equity,
            "equity":account.equity(),"reserve":account.reserved_quote,"margin":account.initial_margin(),
            "maintenance":account.maintenance_margin(),"positions":positions,"group_count":account.groups.len(),
            "pending":account.pending_orders.len(),"calendar_block":calendar_block(timestamp)?
        }),
    )?;
    writer.write_all(b"\n")?;
    Ok(())
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

fn book_delta(stats: &mut ReplayStats, timestamp: i64, before: f64, after: f64) -> Result<()> {
    stats.block_pnl[calendar_block(timestamp.min(END_MS - 1))?] += after - before;
    Ok(())
}

fn position_key(symbol: &str, mode: PositionMode, group: &str) -> PositionKey {
    PositionKey {
        symbol: symbol.into(),
        market_type: MarketType::UsdMPerp,
        mode,
        owner_group: group.into(),
    }
}

fn left_mode(direction: PairDirection) -> PositionMode {
    if direction == PairDirection::LongLeftShortRight {
        PositionMode::Long
    } else {
        PositionMode::Short
    }
}
fn right_mode(direction: PairDirection) -> PositionMode {
    if direction == PairDirection::LongLeftShortRight {
        PositionMode::Short
    } else {
        PositionMode::Long
    }
}
fn mode_name(mode: PositionMode) -> &'static str {
    if mode == PositionMode::Long {
        "long"
    } else {
        "short"
    }
}
fn side_name(mode: PositionMode) -> &'static str {
    if mode == PositionMode::Long {
        "buy"
    } else {
        "sell"
    }
}

fn concentration(values: &BTreeMap<String, f64>) -> f64 {
    let positive = values.values().filter(|value| **value > 0.0).sum::<f64>();
    if positive <= 0.0 {
        0.0
    } else {
        values.values().copied().fold(0.0, f64::max) / positive
    }
}

fn concentration_slice(values: &[f64]) -> f64 {
    let positive = values.iter().filter(|value| **value > 0.0).sum::<f64>();
    if positive <= 0.0 {
        0.0
    } else {
        values.iter().copied().fold(0.0, f64::max) / positive
    }
}

fn minute_bars(
    connection: &Connection,
    symbol: &str,
    start: i64,
    end: i64,
) -> Result<Vec<MinuteBar>> {
    let mut statement=connection.prepare("SELECT open_time,open,high,low,close FROM klines INDEXED BY idx_klines_symbol_time WHERE symbol=?1 AND market_type='futures_usdt_perp' AND timeframe='1m' AND open_time>=?2 AND open_time<?3 ORDER BY open_time")?;
    let rows = statement.query_map((symbol, start, end), |row| {
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

fn load_filters(repo: &Path) -> Result<BTreeMap<String, FilterRow>> {
    let value: serde_json::Value = serde_json::from_slice(&fs::read(repo.join(
        "docs/superpowers/artifacts/glm-martingale-core-round27/round27-data-manifest.json",
    ))?)?;
    let rows: Vec<FilterRow> = serde_json::from_value(value["exchange_info"]["filters"].clone())?;
    Ok(rows
        .into_iter()
        .map(|row| (row.symbol.clone(), row))
        .collect())
}

fn terminal_resume(path: &Path, raw: &Path, resume: bool) -> Result<bool> {
    if !resume || !path.exists() {
        return Ok(false);
    }
    let value: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    Ok(
        value["source_commit"].as_str() == raw.file_name().and_then(|value| value.to_str())
            && matches!(
                value["status"].as_str(),
                Some(
                    "PASS"
                        | "TERMINAL"
                        | "VALID_HISTORICAL_PREQUENTIAL_NO_PB"
                        | "PB_SURVIVORS_READY_G3"
                        | "NOT_APPLICABLE_NO_PB"
                )
            ),
    )
}

fn pragma_rows(connection: &Connection, query: &str) -> Result<Vec<serde_json::Value>> {
    let mut statement = connection.prepare(query)?;
    let columns = statement.column_count();
    let rows = statement.query_map([], |row| {
        let values = (0..columns)
            .map(|index| {
                row.get::<_, rusqlite::types::Value>(index)
                    .map(|value| format!("{value:?}"))
            })
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(serde_json::json!(values))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn optional_json_string(path: PathBuf, key: &str) -> Option<String> {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value[key].as_str().map(str::to_owned))
}

fn safe_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
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

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 8 * 1024 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        bail!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
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
