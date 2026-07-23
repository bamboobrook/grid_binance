use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TRACE_KINDS: [&str; 8] = [
    "event",
    "trade",
    "order",
    "equity",
    "funding",
    "rejection",
    "signal",
    "margin",
];
pub const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowKind {
    Running,
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalStatus {
    Complete,
    Failed,
    Blocked,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceRef {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub rows: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Evidence {
    pub actual_filled_assets: u32,
    pub loaded_assets: u32,
    pub shared_account: bool,
    pub pbo_policy_population: u64,
    pub pbo_selected_population: u64,
    pub dsr_trial_count: Option<u64>,
    pub global_trial_floor: u64,
    pub open_perp_notional: f64,
    pub funding_cashflow_abs: f64,
    pub signal_lag_buckets: i32,
    pub block_cycle_preserved: bool,
    pub expected_blocks: u32,
    pub denominator_blocks: u32,
    pub source_is_pdf: bool,
    pub aggressive_target_ann_pct: f64,
    pub sharpe_is_user_hard_gate: bool,
    pub stitched_days: u32,
    pub configured_budget_u: f64,
    pub argv_budget_u: f64,
    pub selector_frozen: bool,
    pub cold_start_offsets_days: [u32; 5],
}

impl Evidence {
    pub fn r0() -> Self {
        Self {
            actual_filled_assets: 0,
            loaded_assets: 0,
            shared_account: true,
            pbo_policy_population: 1,
            pbo_selected_population: 1,
            dsr_trial_count: None,
            global_trial_floor: 1080,
            open_perp_notional: 0.0,
            funding_cashflow_abs: 0.0,
            signal_lag_buckets: 1,
            block_cycle_preserved: true,
            expected_blocks: 12,
            denominator_blocks: 12,
            source_is_pdf: true,
            aggressive_target_ann_pct: 100.0,
            sharpe_is_user_hard_gate: false,
            stitched_days: 1066,
            configured_budget_u: 1000.0,
            argv_budget_u: 1000.0,
            selector_frozen: true,
            cold_start_offsets_days: [0, 30, 60, 90, 120],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistryRow {
    pub experiment_id: String,
    pub parent_experiment_id: Option<String>,
    pub phase: String,
    pub mechanism_fingerprint: String,
    pub row_kind: RowKind,
    pub terminal_status: Option<TerminalStatus>,
    pub git_commit: String,
    pub git_dirty: bool,
    pub upstream_commit: String,
    pub artifact_sha256: BTreeMap<String, String>,
    pub raw_argv: Vec<String>,
    pub pid: u32,
    pub exit_code: Option<i32>,
    pub start_utc: String,
    pub end_utc: Option<String>,
    pub wall_seconds: Option<f64>,
    pub peak_rss_kb: Option<u64>,
    pub fit_cutoff_utc: String,
    pub purge_bars: u32,
    pub signal_ready_utc: String,
    pub replay_utc: String,
    pub traces: Option<BTreeMap<String, TraceRef>>,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FailureRow {
    pub experiment_id: String,
    pub phase: String,
    pub status: TerminalStatus,
    pub reason: String,
    pub never_repeat: String,
    pub recorded_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct LaunchSpec {
    pub experiment_id: String,
    pub parent_experiment_id: Option<String>,
    pub phase: String,
    pub mechanism_fingerprint: String,
    pub git_commit: String,
    pub git_dirty: bool,
    pub upstream_commit: String,
    pub artifact_sha256: BTreeMap<String, String>,
    pub fit_cutoff_utc: String,
    pub purge_bars: u32,
    pub signal_ready_utc: String,
    pub replay_utc: String,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub valid: bool,
    pub running_rows: usize,
    pub terminal_rows: usize,
    pub unique_experiments: usize,
    pub violations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Registry {
    path: PathBuf,
    ledger_path: PathBuf,
}

impl Registry {
    pub fn new(path: impl Into<PathBuf>, ledger_path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            ledger_path: ledger_path.into(),
        }
    }

    pub fn rows(&self) -> Result<Vec<RegistryRow>> {
        read_jsonl(&self.path)
    }

    pub fn failures(&self) -> Result<Vec<FailureRow>> {
        read_jsonl(&self.ledger_path)
    }

    fn append_running(&self, row: &RegistryRow) -> Result<()> {
        append_jsonl(&self.path, row)
    }

    fn append_terminal(&self, row: &RegistryRow, failure: Option<&FailureRow>) -> Result<()> {
        let mut rows = self.rows()?;
        rows.push(row.clone());
        let mut failures = self.failures()?;
        if let Some(failure) = failure {
            failures.push(failure.clone());
        }
        atomic_replace_jsonl(&self.path, &rows)?;
        if let Err(error) = atomic_replace_jsonl(&self.ledger_path, &failures) {
            rows.pop();
            let _ = atomic_replace_jsonl(&self.path, &rows);
            return Err(error);
        }
        Ok(())
    }
}

pub struct Launcher {
    registry: Registry,
}

impl Launcher {
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }

    pub fn run(
        &self,
        spec: LaunchSpec,
        program: &Path,
        args: &[String],
        trace_dir: &Path,
    ) -> Result<RegistryRow> {
        validate_launch(&spec, &self.registry, program, args)?;
        fs::create_dir_all(trace_dir)?;
        let start = Utc::now();
        let mut command = Command::new(program);
        command.args(args);
        let mut child = command.spawn().context("spawn replay process")?;
        let pid = child.id();
        let raw_argv = std::iter::once(program.to_string_lossy().into_owned())
            .chain(args.iter().cloned())
            .collect::<Vec<_>>();
        let running = RegistryRow {
            experiment_id: spec.experiment_id.clone(),
            parent_experiment_id: spec.parent_experiment_id.clone(),
            phase: spec.phase.clone(),
            mechanism_fingerprint: spec.mechanism_fingerprint.clone(),
            row_kind: RowKind::Running,
            terminal_status: None,
            git_commit: spec.git_commit.clone(),
            git_dirty: spec.git_dirty,
            upstream_commit: spec.upstream_commit.clone(),
            artifact_sha256: spec.artifact_sha256.clone(),
            raw_argv: raw_argv.clone(),
            pid,
            exit_code: None,
            start_utc: utc(start),
            end_utc: None,
            wall_seconds: None,
            peak_rss_kb: None,
            fit_cutoff_utc: spec.fit_cutoff_utc.clone(),
            purge_bars: spec.purge_bars,
            signal_ready_utc: spec.signal_ready_utc.clone(),
            replay_utc: spec.replay_utc.clone(),
            traces: None,
            evidence: spec.evidence.clone(),
        };
        self.registry.append_running(&running)?;

        let timer = Instant::now();
        let mut peak_rss_kb = 0;
        let status = loop {
            peak_rss_kb = peak_rss_kb.max(read_rss_kb(pid).unwrap_or(0));
            if let Some(status) = child.try_wait()? {
                break status;
            }
            thread::sleep(Duration::from_millis(5));
        };
        let elapsed = timer.elapsed().as_secs_f64();
        let end = Utc::now();
        let traces = collect_traces(trace_dir);
        let terminal_evidence = fs::read(trace_dir.join("evidence.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Evidence>(&bytes).ok())
            .unwrap_or_else(|| running.evidence.clone());
        let complete = status.success() && traces.is_ok();
        let terminal_status = if complete {
            TerminalStatus::Complete
        } else {
            TerminalStatus::Failed
        };
        let failure = (!complete).then(|| FailureRow {
            experiment_id: spec.experiment_id.clone(),
            phase: spec.phase.clone(),
            status: terminal_status,
            reason: format!(
                "exit={:?}; trace_error={:?}",
                status.code(),
                traces.as_ref().err()
            ),
            never_repeat:
                "do not promote a replay with failed process or absent/nonparseable traces".into(),
            recorded_at_utc: utc(Utc::now()),
        });
        let terminal = RegistryRow {
            row_kind: RowKind::Terminal,
            terminal_status: Some(terminal_status),
            exit_code: exit_code(status),
            end_utc: Some(utc(end)),
            wall_seconds: Some(elapsed.max(0.000_001)),
            peak_rss_kb: Some(peak_rss_kb),
            traces: traces.ok(),
            evidence: terminal_evidence,
            ..running
        };
        self.registry.append_terminal(&terminal, failure.as_ref())?;
        let summary = validate_registry(&self.registry.rows()?, &self.registry.failures()?, None);
        if !summary.valid {
            bail!(
                "post-run registry invalid: {}",
                summary.violations.join("; ")
            );
        }
        Ok(terminal)
    }
}

pub fn validate_registry(
    rows: &[RegistryRow],
    failures: &[FailureRow],
    claimed_replays: Option<usize>,
) -> ValidationSummary {
    let mut violations = Vec::new();
    let ids = rows
        .iter()
        .map(|row| row.experiment_id.clone())
        .collect::<BTreeSet<_>>();
    let running_rows = rows
        .iter()
        .filter(|row| row.row_kind == RowKind::Running)
        .count();
    let terminal_rows = rows
        .iter()
        .filter(|row| row.row_kind == RowKind::Terminal)
        .count();
    for id in &ids {
        let running = rows
            .iter()
            .filter(|row| row.experiment_id == *id && row.row_kind == RowKind::Running)
            .count();
        let terminal = rows
            .iter()
            .filter(|row| row.experiment_id == *id && row.row_kind == RowKind::Terminal)
            .count();
        if running != 1 {
            violations.push(format!("{id}:running_count={running}"));
        }
        if terminal != 1 {
            violations.push(format!("{id}:terminal_count={terminal}"));
        }
    }
    if claimed_replays.is_some_and(|count| count != terminal_rows) {
        violations.push("authority_registry_replay_count_mismatch".into());
    }
    for row in rows {
        validate_row(row, failures, &mut violations);
    }
    ValidationSummary {
        valid: violations.is_empty(),
        running_rows,
        terminal_rows,
        unique_experiments: ids.len(),
        violations,
    }
}

fn validate_row(row: &RegistryRow, failures: &[FailureRow], violations: &mut Vec<String>) {
    let id = &row.experiment_id;
    if !is_git_oid(&row.git_commit) || !is_git_oid(&row.upstream_commit) {
        violations.push(format!("{id}:invalid_git_oid"));
    }
    if row.git_dirty || row.git_commit != row.upstream_commit {
        violations.push(format!("{id}:dirty_or_unpushed_parent"));
    }
    if !is_sha256(&row.mechanism_fingerprint)
        || row.artifact_sha256.values().any(|hash| !is_sha256(hash))
    {
        violations.push(format!("{id}:invalid_sha256"));
    }
    if row.raw_argv.is_empty() || row.raw_argv.iter().any(|arg| arg.is_empty()) {
        violations.push(format!("{id}:empty_argv"));
    }
    if row.evidence.configured_budget_u != row.evidence.argv_budget_u {
        violations.push(format!("{id}:budget_mismatch"));
    }
    if row.phase == "G2" && row.parent_experiment_id.is_none() {
        violations.push(format!("{id}:g2_without_parent"));
    }
    if row.evidence.cold_start_offsets_days != [0, 30, 60, 90, 120] {
        violations.push(format!("{id}:cold_starts_changed"));
    }
    if row.evidence.signal_lag_buckets < 1 {
        violations.push(format!("{id}:future_or_current_signal"));
    }
    if !row.evidence.block_cycle_preserved {
        violations.push(format!("{id}:block_cycle_reset"));
    }
    if row.evidence.denominator_blocks != row.evidence.expected_blocks {
        violations.push(format!("{id}:block_denominator_changed"));
    }
    if !row.evidence.source_is_pdf {
        violations.push(format!("{id}:source_not_pdf"));
    }
    if (row.evidence.aggressive_target_ann_pct - 100.0).abs() > f64::EPSILON {
        violations.push(format!("{id}:wrong_aggressive_target"));
    }
    if row.evidence.sharpe_is_user_hard_gate {
        violations.push(format!("{id}:sharpe_promoted_to_hard_gate"));
    }
    if row.evidence.stitched_days < 365 && row.phase != "R0" {
        violations.push(format!("{id}:short_window_annualization"));
    }
    if row.terminal_status == Some(TerminalStatus::Complete)
        && matches!(row.phase.as_str(), "G1" | "G2" | "R8")
        && row.evidence.loaded_assets >= 5
        && row.evidence.actual_filled_assets < 5
    {
        violations.push(format!("{id}:actual_asset_gate_failed"));
    }
    if !row.evidence.shared_account {
        violations.push(format!("{id}:independent_accounts_summed"));
    }
    if row.evidence.pbo_selected_population != row.evidence.pbo_policy_population {
        violations.push(format!("{id}:selected_subset_pbo"));
    }
    if row
        .evidence
        .dsr_trial_count
        .is_some_and(|count| count < row.evidence.global_trial_floor)
    {
        violations.push(format!("{id}:dsr_trial_count_below_global_floor"));
    }
    if row.evidence.open_perp_notional > 0.0 && row.evidence.funding_cashflow_abs == 0.0 {
        violations.push(format!("{id}:perp_funding_cashflow_missing"));
    }
    match row.row_kind {
        RowKind::Running => {
            if row.terminal_status.is_some()
                || row.exit_code.is_some()
                || row.end_utc.is_some()
                || row.wall_seconds.is_some()
                || row.peak_rss_kb.is_some()
                || row.traces.is_some()
            {
                violations.push(format!("{id}:running_has_terminal_fields"));
            }
        }
        RowKind::Terminal => {
            if row.terminal_status.is_none() || row.exit_code.is_none() {
                violations.push(format!("{id}:terminal_fields_missing"));
            }
            let timing_ok = row
                .end_utc
                .as_deref()
                .and_then(parse_utc)
                .zip(parse_utc(&row.start_utc))
                .is_some_and(|(end, start)| end > start)
                && row.wall_seconds.is_some_and(|value| value > 0.0)
                && row.peak_rss_kb.is_some_and(|value| value > 0);
            if !timing_ok {
                violations.push(format!("{id}:invalid_runtime_metrics"));
            }
            if row.terminal_status == Some(TerminalStatus::Complete) {
                match &row.traces {
                    Some(traces) => {
                        for kind in TRACE_KINDS {
                            match traces.get(kind) {
                                Some(trace)
                                    if trace.bytes > 0
                                        && trace.rows > 0
                                        && is_sha256(&trace.sha256)
                                        && trace.sha256 != EMPTY_SHA256 => {}
                                _ => violations.push(format!("{id}:invalid_{kind}_trace")),
                            }
                        }
                    }
                    None => violations.push(format!("{id}:complete_without_traces")),
                }
            } else if !failures.iter().any(|failure| failure.experiment_id == *id) {
                violations.push(format!("{id}:failed_without_ledger"));
            }
        }
    }
    if !row.evidence.selector_frozen && row.phase != "R0" {
        violations.push(format!("{id}:selector_not_frozen"));
    }
}

pub fn run_all_canaries() -> BTreeMap<String, bool> {
    let (base_rows, base_failures) = valid_fixture();
    let mut outcomes = BTreeMap::new();
    let mut inject =
        |id: &str,
         mutation: fn(&mut Vec<RegistryRow>, &mut Vec<FailureRow>, &mut Option<usize>)| {
            let mut rows = base_rows.clone();
            let mut failures = base_failures.clone();
            let mut claimed = Some(1);
            mutation(&mut rows, &mut failures, &mut claimed);
            outcomes.insert(
                id.to_string(),
                !validate_registry(&rows, &failures, claimed).valid,
            );
        };
    inject("r23_dirty_commit", |r, _, _| r[0].git_dirty = true);
    inject("r23_unpushed_commit", |r, _, _| {
        r[0].upstream_commit = "b".repeat(40)
    });
    inject("r23_short_hash", |r, _, _| {
        r[0].artifact_sha256.insert("source".into(), "a".repeat(63));
    });
    inject("r23_reused_experiment_id", |r, _, _| {
        let duplicate = r[0].clone();
        r.push(duplicate);
    });
    inject("r23_missing_running", |r, _, _| {
        r.remove(0);
    });
    inject("r23_null_trace", |r, _, _| {
        r[1].traces.as_mut().unwrap().remove("order");
    });
    inject("r23_budget_mismatch", |r, _, _| {
        r[1].evidence.argv_budget_u = 999.0
    });
    inject("r23_g2_without_parent", |r, _, _| {
        r[1].phase = "G2".into();
        r[1].parent_experiment_id = None;
    });
    inject("r23_short_window_ann", |r, _, _| {
        r[1].phase = "G1".into();
        r[1].evidence.stitched_days = 90;
    });
    inject("r23_deleted_block", |r, _, _| {
        r[1].evidence.denominator_blocks = 11
    });
    inject("r23_changed_cold_starts", |r, _, _| {
        r[1].evidence.cold_start_offsets_days = [0, 31, 60, 90, 120]
    });
    inject("r23_summary_count_mismatch", |_, _, c| *c = Some(2));
    inject("r23_unfrozen_selector", |r, _, _| {
        r[1].phase = "G1".into();
        r[1].evidence.selector_frozen = false;
    });
    inject("r23_future_signal", |r, _, _| {
        r[1].evidence.signal_lag_buckets = 0
    });
    inject("r23_failure_without_ledger", |r, f, _| {
        r[1].terminal_status = Some(TerminalStatus::Failed);
        f.clear();
    });
    inject("r24_label_as_git_commit", |r, _, _| {
        r[0].git_commit = "a".repeat(64)
    });
    inject("r24_empty_trace_sha", |r, _, _| {
        r[1].traces
            .as_mut()
            .unwrap()
            .get_mut("event")
            .unwrap()
            .sha256 = EMPTY_SHA256.into()
    });
    inject("r24_terminal_only_summary", |r, _, _| {
        r.remove(0);
    });
    inject("r24_zero_runtime", |r, _, _| {
        r[1].end_utc = Some(r[1].start_utc.clone());
        r[1].wall_seconds = Some(0.0);
        r[1].peak_rss_kb = Some(0);
        r[1].raw_argv.clear();
    });
    inject("r24_authority_registry_mismatch", |_, _, c| *c = Some(99));
    inject("r24_state_machine_handwritten", |r, _, _| {
        r[1].artifact_sha256
            .insert("machine_state".into(), "HANDWRITTEN".into());
    });
    inject("r24_loaded_not_actual_assets", |r, _, _| {
        r[1].phase = "G1".into();
        r[1].evidence.loaded_assets = 8;
        r[1].evidence.actual_filled_assets = 4;
    });
    inject("r24_independent_accounts", |r, _, _| {
        r[1].evidence.shared_account = false
    });
    inject("r24_selected_pbo", |r, _, _| {
        r[1].evidence.pbo_policy_population = 16;
        r[1].evidence.pbo_selected_population = 8;
    });
    inject("r24_dsr_below_global", |r, _, _| {
        r[1].evidence.dsr_trial_count = Some(100)
    });
    inject("r24_zero_funding_perp", |r, _, _| {
        r[1].evidence.open_perp_notional = 100.0
    });
    inject("r24_current_bucket_signal", |r, _, _| {
        r[1].evidence.signal_lag_buckets = 0
    });
    inject("r24_block_cycle_reset", |r, _, _| {
        r[1].evidence.block_cycle_preserved = false
    });
    inject("r24_empty_block_deleted", |r, _, _| {
        r[1].evidence.denominator_blocks = 11
    });
    inject("r24_html_source", |r, _, _| {
        r[1].evidence.source_is_pdf = false
    });
    inject("r24_aggressive_110", |r, _, _| {
        r[1].evidence.aggressive_target_ann_pct = 110.0
    });
    inject("r24_sharpe_hard_gate", |r, _, _| {
        r[1].evidence.sharpe_is_user_hard_gate = true
    });
    outcomes
}

fn valid_fixture() -> (Vec<RegistryRow>, Vec<FailureRow>) {
    let traces = TRACE_KINDS
        .into_iter()
        .map(|kind| {
            (
                kind.into(),
                TraceRef {
                    path: format!("traces/{kind}.jsonl"),
                    sha256: sha256(kind.as_bytes()),
                    bytes: 10,
                    rows: 1,
                },
            )
        })
        .collect();
    let running = RegistryRow {
        experiment_id: "fixture".into(),
        parent_experiment_id: None,
        phase: "R0".into(),
        mechanism_fingerprint: sha256(b"fixture"),
        row_kind: RowKind::Running,
        terminal_status: None,
        git_commit: "a".repeat(40),
        git_dirty: false,
        upstream_commit: "a".repeat(40),
        artifact_sha256: BTreeMap::from([("source".into(), sha256(b"source"))]),
        raw_argv: vec!["fixture".into()],
        pid: 1,
        exit_code: None,
        start_utc: "2026-07-23T00:00:00Z".into(),
        end_utc: None,
        wall_seconds: None,
        peak_rss_kb: None,
        fit_cutoff_utc: "2023-06-30T00:00:00Z".into(),
        purge_bars: 1,
        signal_ready_utc: "2023-07-01T00:00:00Z".into(),
        replay_utc: "2023-07-01T00:00:00Z".into(),
        traces: None,
        evidence: Evidence::r0(),
    };
    let terminal = RegistryRow {
        row_kind: RowKind::Terminal,
        terminal_status: Some(TerminalStatus::Complete),
        exit_code: Some(0),
        end_utc: Some("2026-07-23T00:00:01Z".into()),
        wall_seconds: Some(1.0),
        peak_rss_kb: Some(1),
        traces: Some(traces),
        ..running.clone()
    };
    (vec![running, terminal], vec![])
}

fn validate_launch(
    spec: &LaunchSpec,
    registry: &Registry,
    program: &Path,
    args: &[String],
) -> Result<()> {
    if spec.git_dirty || spec.git_commit != spec.upstream_commit {
        bail!("dirty or unpushed parent");
    }
    if !is_git_oid(&spec.git_commit) || !is_git_oid(&spec.upstream_commit) {
        bail!("git OID must be 40 lowercase hex");
    }
    if !is_sha256(&spec.mechanism_fingerprint) {
        bail!("invalid mechanism fingerprint");
    }
    if spec.artifact_sha256.values().any(|hash| !is_sha256(hash)) {
        bail!("invalid artifact SHA256");
    }
    if registry
        .rows()?
        .iter()
        .any(|row| row.experiment_id == spec.experiment_id)
    {
        bail!("reused experiment ID");
    }
    if args.iter().any(|arg| arg.is_empty()) || program.as_os_str().is_empty() {
        bail!("empty argv");
    }
    if spec.phase == "G2" && spec.parent_experiment_id.is_none() {
        bail!("G2 requires parent");
    }
    Ok(())
}

fn collect_traces(dir: &Path) -> Result<BTreeMap<String, TraceRef>> {
    let mut traces = BTreeMap::new();
    for kind in TRACE_KINDS {
        let path = dir.join(format!("{kind}.jsonl"));
        let bytes = fs::read(&path).with_context(|| format!("read {kind} trace"))?;
        if bytes.is_empty() {
            bail!("empty {kind} trace");
        }
        let text = std::str::from_utf8(&bytes)?;
        let mut rows = 0;
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            serde_json::from_str::<serde_json::Value>(line)
                .with_context(|| format!("parse {kind} trace"))?;
            rows += 1;
        }
        if rows == 0 {
            bail!("no parseable {kind} trace rows");
        }
        traces.insert(
            kind.into(),
            TraceRef {
                path: path.to_string_lossy().into_owned(),
                sha256: sha256(&bytes),
                bytes: bytes.len() as u64,
                rows,
            },
        );
    }
    Ok(traces)
}

fn read_rss_kb(pid: u32) -> Result<u64> {
    let status = fs::read_to_string(format!("/proc/{pid}/status"))?;
    for key in ["VmHWM:", "VmRSS:"] {
        if let Some(line) = status.lines().find(|line| line.starts_with(key)) {
            if let Some(value) = line.split_whitespace().nth(1) {
                return Ok(value.parse()?);
            }
        }
    }
    bail!("RSS unavailable")
}

fn exit_code(status: ExitStatus) -> Option<i32> {
    status.code().or(Some(128))
}
fn utc(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Micros, true)
}
fn parse_utc(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|v| v.with_timezone(&Utc))
}
pub fn is_git_oid(value: &str) -> bool {
    value.len() == 40 && lower_hex(value)
}
pub fn is_sha256(value: &str) -> bool {
    value.len() == 64 && lower_hex(value)
}
fn lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
pub fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
pub fn sha256_file(path: &Path) -> Result<String> {
    Ok(sha256(&fs::read(path)?))
}

fn append_jsonl<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut values = Vec::new();
    for (index, line) in BufReader::new(File::open(path)?).lines().enumerate() {
        let line = line?;
        if !line.trim().is_empty() {
            values.push(
                serde_json::from_str(&line)
                    .with_context(|| format!("parse {} line {}", path.display(), index + 1))?,
            );
        }
    }
    Ok(values)
}

fn atomic_replace_jsonl<T: Serialize>(path: &Path, values: &[T]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let parent = path.parent().unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    for value in values {
        serde_json::to_writer(&mut temp, value)?;
        temp.write_all(b"\n")?;
    }
    temp.as_file_mut().sync_all()?;
    temp.persist(path).map_err(|error| anyhow!(error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_32_canaries_are_rejected_by_real_validator() {
        let outcomes = run_all_canaries();
        assert_eq!(outcomes.len(), 32);
        let failed = outcomes
            .iter()
            .filter(|(_, passed)| !**passed)
            .collect::<Vec<_>>();
        assert!(failed.is_empty(), "canaries accepted: {failed:?}");
    }

    #[test]
    fn git_and_sha_types_are_distinct() {
        assert!(is_git_oid(&"a".repeat(40)));
        assert!(!is_git_oid(&"a".repeat(64)));
        assert!(is_sha256(&"b".repeat(64)));
        assert!(!is_sha256(&"b".repeat(40)));
    }
}
