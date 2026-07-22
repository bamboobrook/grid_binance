//! Canonical execution state (§14).
//!
//! Derives the Round 23 machine state from the registry + canary results. The
//! only allowed terminal states are:
//!
//! ```text
//! HISTORICAL_PREQUENTIAL_TARGET_HIT
//! HISTORICAL_PREQUENTIAL_FRONTIER_PROGRESS
//! VALID_HISTORICAL_PREQUENTIAL_NO_TARGET
//! BLOCKED_ENGINE_DATA_OR_EXECUTION
//! MATERIALLY_INCOMPLETE_INVALID_RESULTS
//! ```
//!
//! While R0 is the only completed phase the state MUST be
//! `BLOCKED_ENGINE_DATA_OR_EXECUTION` and search is forbidden (§4.3 last line).

use serde::{Deserialize, Serialize};

use crate::canary::{run_all_canaries, CanaryOutcome};
use crate::registry::Registry;

/// One of the five canonical states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionState {
    HistoricalPrequentialTargetHit,
    HistoricalPrequentialFrontierProgress,
    ValidHistoricalPrequentialNoTarget,
    BlockedEngineDataOrExecution,
    MateriallyIncompleteInvalidResults,
}

impl ExecutionState {
    pub fn as_str(self) -> &'static str {
        match self {
            ExecutionState::HistoricalPrequentialTargetHit => "HISTORICAL_PREQUENTIAL_TARGET_HIT",
            ExecutionState::HistoricalPrequentialFrontierProgress => "HISTORICAL_PREQUENTIAL_FRONTIER_PROGRESS",
            ExecutionState::ValidHistoricalPrequentialNoTarget => "VALID_HISTORICAL_PREQUENTIAL_NO_TARGET",
            ExecutionState::BlockedEngineDataOrExecution => "BLOCKED_ENGINE_DATA_OR_EXECUTION",
            ExecutionState::MateriallyIncompleteInvalidResults => "MATERIALLY_INCOMPLETE_INVALID_RESULTS",
        }
    }
}

/// Inputs for deriving the state.
pub struct DeriveInput<'a> {
    pub registry: &'a Registry,
    /// Phases with at least one complete terminal row.
    pub completed_phases: Vec<String>,
}

/// Derive the state. The decision is intentionally conservative:
/// - If any canary was wrongly accepted -> MATERIALLY_INCOMPLETE_INVALID_RESULTS.
/// - If R0 is not among completed phases -> BLOCKED_ENGINE_DATA_OR_EXECUTION.
/// - Otherwise, downstream of R0, the state remains BLOCKED until the G1/G2/R8
///   gates pass (those gates are implemented in later phases).
pub fn derive_execution_state(input: &DeriveInput<'_>) -> ExecutionState {
    // Run the canaries against a fresh registry (the canaries carry their own
    // fixtures; they do not depend on the live registry contents).
    let outcomes = run_all_canaries();
    if outcomes
        .values()
        .any(|o| !matches!(o, CanaryOutcome::RejectedCorrectly))
    {
        return ExecutionState::MateriallyIncompleteInvalidResults;
    }

    // Registry must have the running/terminal invariant hold.
    if !input.registry.invariant_violations().is_empty() {
        return ExecutionState::BlockedEngineDataOrExecution;
    }

    let r0_done = input
        .completed_phases
        .iter()
        .any(|p| p.eq_ignore_ascii_case("R0"));
    if !r0_done {
        return ExecutionState::BlockedEngineDataOrExecution;
    }

    // R0 complete + canaries green. No G1/G2/R8 work has been done yet in this
    // session, so the state stays BLOCKED (engine is ready, but no search result
    // exists). The state will advance once G1 passes P-B etc.
    ExecutionState::BlockedEngineDataOrExecution
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Registry;
    use tempfile::NamedTempFile;

    fn keep_temp() -> std::path::PathBuf {
        let p = NamedTempFile::new().unwrap().into_temp_path().keep().unwrap();
        std::fs::write(&p, b"").unwrap();
        p
    }

    #[test]
    fn empty_registry_with_r0_not_done_is_blocked() {
        let reg = Registry::new(keep_temp());
        let input = DeriveInput { registry: &reg, completed_phases: vec![] };
        let s = derive_execution_state(&input);
        assert_eq!(s, ExecutionState::BlockedEngineDataOrExecution);
    }

    #[test]
    fn r0_done_canaries_green_still_blocked_without_g1() {
        let reg = Registry::new(keep_temp());
        let input = DeriveInput { registry: &reg, completed_phases: vec!["R0".to_string()] };
        let s = derive_execution_state(&input);
        assert_eq!(s, ExecutionState::BlockedEngineDataOrExecution);
    }
}
