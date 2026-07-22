//! Immutable, append-only JSONL experiment registry.
//!
//! §4.2: each immutable `experiment_id` must have exactly one `running` row and
//! one terminal row. non-complete terminal rows must atomically write a failure
//! ledger entry. Registry/ledger/trace append failure fails the experiment.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::hash::is_full_sha256_hex;

/// Kind of registry row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowKind {
    /// Written before the experiment body runs. At most one per experiment_id.
    Running,
    /// Written after the experiment body returns. Exactly one per experiment_id.
    Terminal,
}

/// Terminal outcome. `Complete` requires the matching `Running` row and a full
/// trace hash set; every other value forces a paired failure-ledger row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalStatus {
    Complete,
    FailedEngine,
    FailedData,
    FailedExecution,
    FailedGate,
    Aborted,
}

/// One immutable registry row. Fields mirror §4.2's required manifest.
///
/// Every `*_sha256` field MUST be a full 64-char lowercase-hex digest; the
/// launcher rejects rows whose hashes are short or malformed (§4.1 canary #3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryRow {
    pub experiment_id: String,
    pub phase: String, // e.g. "R0", "R1", "G1", "G2", "R8"
    pub row_kind: RowKind,
    /// Only meaningful for terminal rows (`None` for running rows).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_status: Option<TerminalStatus>,
    /// Parent experiment_id for dependent phases (e.g. G2's parent is a committed G1 row).
    /// §4.3 canary #8: a G2 row with no committed G1 parent is rejected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_experiment_id: Option<String>,

    // §4.2 line 1: git_commit/git_dirty/upstream_remote_commit
    pub git_commit: String,
    pub git_dirty: bool,
    pub upstream_remote_commit: String,
    pub recorded_at_utc: String,

    // §4.2 line 2: full SHA-256 of every artifact class
    pub binary_sha256: String,
    pub source_sha256: String,
    pub market_sha256: String,
    pub metrics_sha256: String,
    pub depth_sha256: String,
    pub aggtrades_sha256: String,
    pub funding_sha256: String,
    pub filter_sha256: String,
    pub maintenance_sha256: String,
    pub borrow_sha256: String,
    pub cost_sha256: String,
    pub config_sha256: String,
    pub fit_sha256: String,
    pub protocol_sha256: String,

    // §4.2 line 3: raw argv/pid/exit/wall/RSS/start/end/fit/purge/replay timestamps
    pub raw_argv: Vec<String>,
    pub pid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub wall_seconds: f64,
    pub rss_kb: u64,
    pub start_utc: String,
    pub end_utc: String,
    pub fit_cutoff_utc: String,
    pub purge_days: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replay_utc: Option<String>,

    // §4.2 line 4: full trace SHA-256
    pub event_trace_sha256: String,
    pub trade_trace_sha256: String,
    pub order_trace_sha256: String,
    pub equity_trace_sha256: String,
    pub funding_trace_sha256: String,
    pub rejection_trace_sha256: String,
    pub signal_trace_sha256: String,
    pub margin_trace_sha256: String,

    // budget provenance for §4.3 canary #7 (config/argv budget mismatch)
    pub config_budget_u: f64,
    pub argv_budget_u: f64,
}

impl RegistryRow {
    /// Every `*_sha256` field on the row, returned as (name, value) pairs.
    pub fn all_hashes(&self) -> Vec<(&'static str, &str)> {
        vec![
            ("binary", &self.binary_sha256),
            ("source", &self.source_sha256),
            ("market", &self.market_sha256),
            ("metrics", &self.metrics_sha256),
            ("depth", &self.depth_sha256),
            ("aggtrades", &self.aggtrades_sha256),
            ("funding", &self.funding_sha256),
            ("filter", &self.filter_sha256),
            ("maintenance", &self.maintenance_sha256),
            ("borrow", &self.borrow_sha256),
            ("cost", &self.cost_sha256),
            ("config", &self.config_sha256),
            ("fit", &self.fit_sha256),
            ("protocol", &self.protocol_sha256),
            ("event_trace", &self.event_trace_sha256),
            ("trade_trace", &self.trade_trace_sha256),
            ("order_trace", &self.order_trace_sha256),
            ("equity_trace", &self.equity_trace_sha256),
            ("funding_trace", &self.funding_trace_sha256),
            ("rejection_trace", &self.rejection_trace_sha256),
            ("signal_trace", &self.signal_trace_sha256),
            ("margin_trace", &self.margin_trace_sha256),
        ]
    }

    /// True iff every SHA-256 field is a full 64-char lowercase-hex digest.
    pub fn hashes_are_full(&self) -> bool {
        self.all_hashes().iter().all(|(_, v)| is_full_sha256_hex(v))
    }

    /// True iff this terminal row's outcome is non-complete (needs a failure row).
    pub fn needs_failure_row(&self) -> bool {
        matches!(self.row_kind, RowKind::Terminal)
            && matches!(self.terminal_status, Some(s) if s != TerminalStatus::Complete)
    }
}

/// Append-only JSONL registry backed by a file.
pub struct Registry {
    path: PathBuf,
}

impl Registry {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self { path: path.as_ref().to_path_buf() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read and parse every row.
    pub fn read_all(&self) -> Result<Vec<RegistryRow>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let f = File::open(&self.path).with_context(|| format!("open registry {}", self.path.display()))?;
        let reader = BufReader::new(f);
        let mut rows = Vec::new();
        for (i, line) in reader.lines().enumerate() {
            let line = line.with_context(|| format!("read registry line {}", i))?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let row: RegistryRow = serde_json::from_str(trimmed)
                .with_context(|| format!("parse registry line {} as JSON", i))?;
            rows.push(row);
        }
        Ok(rows)
    }

    /// Atomically append one row. `append_failed` is returned (not panicked) if
    /// the OS write fails — per §4.2 an append failure fails the experiment.
    pub fn append(&self, row: &RegistryRow) -> Result<()> {
        // Validate hashes before writing (§4.1 canary #3).
        if !row.hashes_are_full() {
            bail!("registry row has a non-full SHA-256 field; refusing to append");
        }
        let mut json = serde_json::to_string(row)?;
        json.push('\n');
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open registry for append {}", self.path.display()))?;
        f.write_all(json.as_bytes())
            .with_context(|| format!("write registry row {}", self.path.display()))?;
        f.flush()?;
        Ok(())
    }

    /// Verify the running/terminal invariant for every experiment_id.
    ///
    /// Returns the list of invariant violations (empty = healthy). Used by the
    /// launcher pre-check and by the state machine.
    pub fn invariant_violations(&self) -> Vec<String> {
        let rows = match self.read_all() {
            Ok(r) => r,
            Err(e) => return vec![format!("registry_unreadable: {e}")],
        };
        let mut running = HashSet::new();
        let mut terminal = HashSet::new();
        let mut seen_ids: HashSet<String> = HashSet::new();
        let mut violations = Vec::new();
        for r in &rows {
            let id = &r.experiment_id;
            match r.row_kind {
                RowKind::Running => {
                    if running.contains(id) {
                        violations.push(format!("duplicate_running_row: {id}"));
                    }
                    running.insert(id.clone());
                }
                RowKind::Terminal => {
                    if terminal.contains(id) {
                        violations.push(format!("duplicate_terminal_row: {id}"));
                    }
                    terminal.insert(id.clone());
                }
            }
            seen_ids.insert(id.clone());
        }
        // Every id must have exactly one running and one terminal.
        for id in &seen_ids {
            if !running.contains(id) {
                violations.push(format!("missing_running_row: {id}"));
            }
            if !terminal.contains(id) {
                violations.push(format!("missing_terminal_row: {id}"));
            }
        }
        violations
    }

    /// Return whether an experiment_id already exists (for §4.3 canary #4).
    pub fn contains_id(&self, experiment_id: &str) -> Result<bool> {
        Ok(self.read_all()?.iter().any(|r| r.experiment_id == experiment_id))
    }

    /// Return whether a committed (terminal Complete) G1 row exists for a given
    /// experiment_id (for §4.3 canary #8: G2 without committed G1 parent).
    pub fn has_committed_parent(&self, parent_id: &str) -> Result<bool> {
        Ok(self.read_all()?.iter().any(|r| {
            r.experiment_id == parent_id
                && matches!(r.row_kind, RowKind::Terminal)
                && matches!(r.terminal_status, Some(TerminalStatus::Complete))
        }))
    }
}

/// Build a minimal-but-valid registry row for testing. All hashes default to the
/// canonical empty-input digest so the row passes the full-hex check.
pub fn valid_running_row(experiment_id: &str, phase: &str) -> RegistryRow {
    let h = crate::hash::sha256_str("");
    RegistryRow {
        experiment_id: experiment_id.to_string(),
        phase: phase.to_string(),
        row_kind: RowKind::Running,
        terminal_status: None,
        parent_experiment_id: None,
        git_commit: h.clone(),
        git_dirty: false,
        upstream_remote_commit: h.clone(),
        recorded_at_utc: "2026-07-22T00:00:00Z".to_string(),
        binary_sha256: h.clone(),
        source_sha256: h.clone(),
        market_sha256: h.clone(),
        metrics_sha256: h.clone(),
        depth_sha256: h.clone(),
        aggtrades_sha256: h.clone(),
        funding_sha256: h.clone(),
        filter_sha256: h.clone(),
        maintenance_sha256: h.clone(),
        borrow_sha256: h.clone(),
        cost_sha256: h.clone(),
        config_sha256: h.clone(),
        fit_sha256: h.clone(),
        protocol_sha256: h.clone(),
        raw_argv: vec!["replay".to_string()],
        pid: 1,
        exit_code: None,
        wall_seconds: 0.0,
        rss_kb: 0,
        start_utc: "2026-07-22T00:00:00Z".to_string(),
        end_utc: "2026-07-22T00:00:00Z".to_string(),
        fit_cutoff_utc: "2023-06-30T00:00:00Z".to_string(),
        purge_days: 1,
        replay_utc: None,
        event_trace_sha256: h.clone(),
        trade_trace_sha256: h.clone(),
        order_trace_sha256: h.clone(),
        equity_trace_sha256: h.clone(),
        funding_trace_sha256: h.clone(),
        rejection_trace_sha256: h.clone(),
        signal_trace_sha256: h.clone(),
        margin_trace_sha256: h,
        config_budget_u: 500.0,
        argv_budget_u: 500.0,
    }
}

/// Errors surfaced to the launcher when a row violates a precondition.
pub fn short_hash_error(field: &str, value: &str) -> anyhow::Error {
    anyhow!("field {field} is not a full 64-char lowercase-hex SHA-256 (got len {}): {value}", value.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn tmp() -> PathBuf {
        NamedTempFile::new().unwrap().into_temp_path().keep().unwrap()
    }

    #[test]
    fn append_and_read_roundtrip() {
        let p = tmp();
        let reg = Registry::new(&p);
        let row = valid_running_row("exp001", "R0");
        reg.append(&row).unwrap();
        let rows = reg.read_all().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].experiment_id, "exp001");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn short_hash_is_rejected_on_append() {
        let p = tmp();
        let reg = Registry::new(&p);
        let mut row = valid_running_row("exp_short", "R0");
        row.binary_sha256 = "a".repeat(63); // §4.3 canary #3
        let err = reg.append(&row).unwrap_err().to_string();
        assert!(err.contains("non-full SHA-256"));
        let _ = std::fs::remove_file(&p);
    }
}
