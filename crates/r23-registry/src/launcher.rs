//! Central launcher.
//!
//! §4.2: every registry row must pass the preconditions before it is written:
//! clean + pushed git commit, full SHA-256 hashes, no reused experiment_id,
//! config/argv budget match, G1 parent presence for G2. The launcher is the
//! single place that may call [`crate::Registry::append`].
//!
//! §4.3 (canary #15): a terminal row with a non-complete status and no matching
//! failure ledger row is rejected.

use std::sync::Mutex;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::failure_ledger::{FailureLedger, FailureRow};
use crate::registry::{Registry, RegistryRow, RowKind, TerminalStatus};

/// Inputs the launcher validates before writing a `Running` row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRequest {
    pub experiment_id: String,
    pub phase: String,
    /// Parent experiment_id. Required for G2 (must be a committed G1 row).
    #[serde(default)]
    pub parent_experiment_id: Option<String>,
    pub git_commit: String,
    pub git_dirty: bool,
    pub upstream_remote_commit: String,
    pub config_budget_u: f64,
    pub argv_budget_u: f64,
    /// Full SHA-256 hashes of every artifact class (validated for length).
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
}

/// Why a launch was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// §4.1 canary #2: dirty tree.
    DirtyCommit,
    /// §4.1 canary #2: commit not on upstream.
    CommitNotOnUpstream,
    /// §4.3 canary #3: any hash is not 64-char lowercase hex.
    ShortOrMalformedHash(String),
    /// §4.3 canary #4: experiment_id reused.
    ReusedExperimentId,
    /// §4.3 canary #7: config and argv budget disagree. Budgets are stored as
    /// micro-USDT integers so the error is Eq-derivable.
    ConfigArgvBudgetMismatch { config_micro_u: i64, argv_micro_u: i64 },
    /// §4.3 canary #8: G2 row without a committed G1 parent.
    G2WithoutCommittedG1Parent,
    /// §4.3 canary #9: a 90-day annualized figure tried to enter the target.
    ShortWindowAnnEnteredTarget,
    /// Append I/O failure.
    AppendFailed(String),
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LaunchError::DirtyCommit => write!(f, "git tree is dirty; commit/push before launch"),
            LaunchError::CommitNotOnUpstream => write!(f, "commit not present on upstream remote"),
            LaunchError::ShortOrMalformedHash(name) => write!(f, "hash field {name} is not a full 64-char lowercase-hex SHA-256"),
            LaunchError::ReusedExperimentId => write!(f, "experiment_id already used"),
            LaunchError::ConfigArgvBudgetMismatch { config_micro_u, argv_micro_u } => write!(f, "config budget {}u != argv budget {}u", config_micro_u, argv_micro_u),
            LaunchError::G2WithoutCommittedG1Parent => write!(f, "G2 launch without a committed G1 parent experiment"),
            LaunchError::ShortWindowAnnEnteredTarget => write!(f, "annualized figure from <365 stitched days tried to enter the target"),
            LaunchError::AppendFailed(s) => write!(f, "append failed: {s}"),
        }
    }
}

impl std::error::Error for LaunchError {}

/// The central launcher. Owns references to the registry and failure ledger and
/// serializes all writes through a mutex so the running/terminal invariant and
/// the failure-row pairing are atomic.
pub struct Launcher {
    registry: Registry,
    ledger: FailureLedger,
    /// Serializes the (terminal row + failure row) critical section.
    write_lock: Mutex<()>,
}

impl Launcher {
    pub fn new(registry: Registry, ledger: FailureLedger) -> Self {
        Self { registry, ledger, write_lock: Mutex::new(()) }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn ledger(&self) -> &FailureLedger {
        &self.ledger
    }

    /// Validate and write a `Running` row. Returns the validated row.
    pub fn launch(&self, req: &LaunchRequest) -> Result<RegistryRow, LaunchError> {
        // §4.1 canary #2: dirty.
        if req.git_dirty {
            return Err(LaunchError::DirtyCommit);
        }
        // §4.1 canary #2: commit must equal upstream remote commit.
        if req.git_commit != req.upstream_remote_commit {
            return Err(LaunchError::CommitNotOnUpstream);
        }
        // §4.3 canary #3: full hashes.
        for (name, val) in self.hash_pairs(req) {
            if !crate::hash::is_full_sha256_hex(val) {
                return Err(LaunchError::ShortOrMalformedHash(name.to_string()));
            }
        }
        // §4.3 canary #4: no reused experiment_id.
        if self.registry.contains_id(&req.experiment_id).unwrap_or(true) {
            return Err(LaunchError::ReusedExperimentId);
        }
        // §4.3 canary #7: config/argv budget match. Compare as micro-USDT ints.
        let config_micro = (req.config_budget_u * 1_000_000.0).round() as i64;
        let argv_micro = (req.argv_budget_u * 1_000_000.0).round() as i64;
        if config_micro != argv_micro {
            return Err(LaunchError::ConfigArgvBudgetMismatch {
                config_micro_u: config_micro,
                argv_micro_u: argv_micro,
            });
        }
        // §4.3 canary #8: G2 without committed G1 parent.
        if req.phase == "G2" {
            match &req.parent_experiment_id {
                None => return Err(LaunchError::G2WithoutCommittedG1Parent),
                Some(parent) => {
                    if !self.registry.has_committed_parent(parent).unwrap_or(false) {
                        return Err(LaunchError::G2WithoutCommittedG1Parent);
                    }
                }
            }
        }
        let row = self.build_running_row(req);
        // Acquire the write lock around the append so a concurrent terminal
        // writer cannot interleave.
        let _g = self.write_lock.lock().unwrap();
        self.registry
            .append(&row)
            .map_err(|e| LaunchError::AppendFailed(e.to_string()))?;
        Ok(row)
    }

    /// Record a terminal row. If the status is non-complete, atomically writes a
    /// paired failure ledger row in the same critical section (§4.3 canary #15).
    pub fn record_terminal(
        &self,
        experiment_id: &str,
        _phase: &str,
        status: TerminalStatus,
        failure: Option<FailureRow>,
        trace_hashes: TraceHashes,
    ) -> Result<RegistryRow, LaunchError> {
        let _g = self.write_lock.lock().unwrap();

        // Read existing rows to build the terminal row from the running row.
        let rows = self
            .registry
            .read_all()
            .map_err(|e| LaunchError::AppendFailed(e.to_string()))?;
        let running = rows
            .iter()
            .find(|r| r.experiment_id == experiment_id && r.row_kind == RowKind::Running)
            .cloned();
        let mut row = match running {
            Some(r) => r,
            None => {
                // §4.3 canary #5: missing running row.
                return Err(LaunchError::AppendFailed(format!(
                    "no running row for {experiment_id}; cannot record terminal"
                )));
            }
        };
        row.row_kind = RowKind::Terminal;
        row.terminal_status = Some(status);
        row.exit_code = Some(if status == TerminalStatus::Complete { 0 } else { 1 });
        row.event_trace_sha256 = trace_hashes.event;
        row.trade_trace_sha256 = trace_hashes.trade;
        row.order_trace_sha256 = trace_hashes.order;
        row.equity_trace_sha256 = trace_hashes.equity;
        row.funding_trace_sha256 = trace_hashes.funding;
        row.rejection_trace_sha256 = trace_hashes.rejection;
        row.signal_trace_sha256 = trace_hashes.signal;
        row.margin_trace_sha256 = trace_hashes.margin;
        if !row.hashes_are_full() {
            return Err(LaunchError::ShortOrMalformedHash("trace".to_string()));
        }

        // §4.3 canary #15: non-complete terminal requires a failure row.
        let needs_failure = row.needs_failure_row();
        if needs_failure && failure.is_none() {
            return Err(LaunchError::AppendFailed(format!(
                "terminal status {status:?} requires a failure ledger row for {experiment_id}"
            )));
        }

        self.registry
            .append(&row)
            .map_err(|e| LaunchError::AppendFailed(e.to_string()))?;
        if needs_failure {
            let f = failure.unwrap();
            self.ledger
                .append(&f)
                .map_err(|e| LaunchError::AppendFailed(e.to_string()))?;
        }
        Ok(row)
    }

    fn hash_pairs<'a>(&'a self, req: &'a LaunchRequest) -> Vec<(&'static str, &'a str)> {
        vec![
            ("binary", req.binary_sha256.as_str()),
            ("source", req.source_sha256.as_str()),
            ("market", req.market_sha256.as_str()),
            ("metrics", req.metrics_sha256.as_str()),
            ("depth", req.depth_sha256.as_str()),
            ("aggtrades", req.aggtrades_sha256.as_str()),
            ("funding", req.funding_sha256.as_str()),
            ("filter", req.filter_sha256.as_str()),
            ("maintenance", req.maintenance_sha256.as_str()),
            ("borrow", req.borrow_sha256.as_str()),
            ("cost", req.cost_sha256.as_str()),
            ("config", req.config_sha256.as_str()),
            ("fit", req.fit_sha256.as_str()),
            ("protocol", req.protocol_sha256.as_str()),
            ("git_commit", req.git_commit.as_str()),
            ("upstream_remote_commit", req.upstream_remote_commit.as_str()),
        ]
    }

    fn build_running_row(&self, req: &LaunchRequest) -> RegistryRow {
        let h = |s: &str| req_field(req, s).to_string();
        RegistryRow {
            experiment_id: req.experiment_id.clone(),
            phase: req.phase.clone(),
            row_kind: RowKind::Running,
            terminal_status: None,
            parent_experiment_id: req.parent_experiment_id.clone(),
            git_commit: req.git_commit.clone(),
            git_dirty: req.git_dirty,
            upstream_remote_commit: req.upstream_remote_commit.clone(),
            recorded_at_utc: now_utc(),
            binary_sha256: h("binary"),
            source_sha256: h("source"),
            market_sha256: h("market"),
            metrics_sha256: h("metrics"),
            depth_sha256: h("depth"),
            aggtrades_sha256: h("aggtrades"),
            funding_sha256: h("funding"),
            filter_sha256: h("filter"),
            maintenance_sha256: h("maintenance"),
            borrow_sha256: h("borrow"),
            cost_sha256: h("cost"),
            config_sha256: h("config"),
            fit_sha256: h("fit"),
            protocol_sha256: h("protocol"),
            raw_argv: vec![],
            pid: std::process::id(),
            exit_code: None,
            wall_seconds: 0.0,
            rss_kb: 0,
            start_utc: now_utc(),
            end_utc: now_utc(),
            fit_cutoff_utc: String::new(),
            purge_days: 1,
            replay_utc: None,
            // Trace hashes for a running row are placeholders; record_terminal fills the real ones.
            event_trace_sha256: empty_hash(),
            trade_trace_sha256: empty_hash(),
            order_trace_sha256: empty_hash(),
            equity_trace_sha256: empty_hash(),
            funding_trace_sha256: empty_hash(),
            rejection_trace_sha256: empty_hash(),
            signal_trace_sha256: empty_hash(),
            margin_trace_sha256: empty_hash(),
            config_budget_u: req.config_budget_u,
            argv_budget_u: req.argv_budget_u,
        }
    }
}

/// The seven trace hashes a terminal row must carry.
#[derive(Debug, Clone, Default)]
pub struct TraceHashes {
    pub event: String,
    pub trade: String,
    pub order: String,
    pub equity: String,
    pub funding: String,
    pub rejection: String,
    pub signal: String,
    pub margin: String,
}

fn req_field<'a>(req: &'a LaunchRequest, name: &str) -> &'a str {
    match name {
        "binary" => &req.binary_sha256,
        "source" => &req.source_sha256,
        "market" => &req.market_sha256,
        "metrics" => &req.metrics_sha256,
        "depth" => &req.depth_sha256,
        "aggtrades" => &req.aggtrades_sha256,
        "funding" => &req.funding_sha256,
        "filter" => &req.filter_sha256,
        "maintenance" => &req.maintenance_sha256,
        "borrow" => &req.borrow_sha256,
        "cost" => &req.cost_sha256,
        "config" => &req.config_sha256,
        "fit" => &req.fit_sha256,
        "protocol" => &req.protocol_sha256,
        _ => "",
    }
}

pub(crate) fn empty_hash() -> String {
    crate::hash::sha256_str("")
}

pub(crate) fn now_utc() -> String {
    // RFC3339 UTC using chrono (already a workspace dep).
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn tmp_paths() -> (std::path::PathBuf, std::path::PathBuf) {
        let r = NamedTempFile::new().unwrap().into_temp_path().keep().unwrap();
        let l = NamedTempFile::new().unwrap().into_temp_path().keep().unwrap();
        (r, l)
    }

    fn valid_req(id: &str, phase: &str) -> LaunchRequest {
        let h = empty_hash();
        LaunchRequest {
            experiment_id: id.to_string(),
            phase: phase.to_string(),
            parent_experiment_id: None,
            git_commit: h.clone(),
            git_dirty: false,
            upstream_remote_commit: h.clone(),
            config_budget_u: 500.0,
            argv_budget_u: 500.0,
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
            protocol_sha256: h,
        }
    }

    #[test]
    fn valid_launch_appends_running_row() {
        let (r, l) = tmp_paths();
        let launcher = Launcher::new(Registry::new(&r), FailureLedger::new(&l));
        let req = valid_req("e1", "R0");
        let row = launcher.launch(&req).unwrap();
        assert_eq!(row.experiment_id, "e1");
        assert_eq!(launcher.registry().read_all().unwrap().len(), 1);
    }

    #[test]
    fn dirty_commit_rejected() {
        let (r, l) = tmp_paths();
        let launcher = Launcher::new(Registry::new(&r), FailureLedger::new(&l));
        let mut req = valid_req("e1", "R0");
        req.git_dirty = true;
        let err = launcher.launch(&req).unwrap_err();
        assert!(matches!(err, LaunchError::DirtyCommit), "got {err:?}");
    }

    #[test]
    fn reused_experiment_id_rejected() {
        let (r, l) = tmp_paths();
        let launcher = Launcher::new(Registry::new(&r), FailureLedger::new(&l));
        launcher.launch(&valid_req("e1", "R0")).unwrap();
        let err = launcher.launch(&valid_req("e1", "R0")).unwrap_err();
        assert!(matches!(err, LaunchError::ReusedExperimentId), "got {err:?}");
    }

    #[test]
    fn short_hash_rejected() {
        let (r, l) = tmp_paths();
        let launcher = Launcher::new(Registry::new(&r), FailureLedger::new(&l));
        let mut req = valid_req("e1", "R0");
        req.binary_sha256 = "a".repeat(63);
        let err = launcher.launch(&req).unwrap_err();
        assert!(matches!(err, LaunchError::ShortOrMalformedHash(_)));
    }

    #[test]
    fn budget_mismatch_rejected() {
        let (r, l) = tmp_paths();
        let launcher = Launcher::new(Registry::new(&r), FailureLedger::new(&l));
        let mut req = valid_req("e1", "R0");
        req.argv_budget_u = 750.0;
        let err = launcher.launch(&req).unwrap_err();
        assert!(matches!(err, LaunchError::ConfigArgvBudgetMismatch { .. }));
    }

    #[test]
    fn g2_without_committed_g1_rejected() {
        let (r, l) = tmp_paths();
        let launcher = Launcher::new(Registry::new(&r), FailureLedger::new(&l));
        let mut req = valid_req("g2-1", "G2");
        req.parent_experiment_id = None;
        let err = launcher.launch(&req).unwrap_err();
        assert!(matches!(err, LaunchError::G2WithoutCommittedG1Parent), "got {err:?}");
    }

    #[test]
    fn non_complete_terminal_requires_failure_row() {
        let (r, l) = tmp_paths();
        let launcher = Launcher::new(Registry::new(&r), FailureLedger::new(&l));
        launcher.launch(&valid_req("e1", "R0")).unwrap();
        let traces = TraceHashes {
            event: empty_hash(),
            trade: empty_hash(),
            order: empty_hash(),
            equity: empty_hash(),
            funding: empty_hash(),
            rejection: empty_hash(),
            signal: empty_hash(),
            margin: empty_hash(),
        };
        let err = launcher
            .record_terminal("e1", "R0", TerminalStatus::FailedGate, None, traces)
            .unwrap_err();
        assert!(matches!(err, LaunchError::AppendFailed(_)));
    }
}
