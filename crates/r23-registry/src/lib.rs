//! # r23-registry — Round 23 immutable experiment registry, failure ledger and launcher
//!
//! Implements the **R0 contract** of the Round 23 plan
//! (`docs/superpowers/plans/2026-07-22-glm-martingale-core-round23-orderflow-factor-repair-plan.md`,
//! sections 4.1 / 4.2 / 4.3).
//!
//! ## Why this crate exists
//!
//! The Round 22 audit (`audit/round22-independent-audit.json`) found that the
//! experiment registry / failure ledger existed only as Python stubs and were
//! left **empty (0 rows)** while downstream G1/G2 results were claimed. The
//! first failed gate was `R0_registry_contract_valid`. R23 §4 makes this
//! contract non-bypassable: every immutable `experiment_id` must have exactly
//! one `running` row and one terminal row, every terminal-non-complete row
//! must atomically write a failure ledger entry, and the launcher must reject
//! dirty/unpushed commits, short hashes, reused IDs, G2-without-G1-parent,
//! future/stale metrics and the rest of the §4.3 canary list.
//!
//! ## What this crate provides
//!
//! - [`Registry`] — append-only JSONL registry with running/terminal invariant enforcement.
//! - [`FailureLedger`] — atomic failure ledger, appended in the same critical
//!   section as a non-complete terminal registry row.
//! - [`Launcher`] — central launcher that enforces the §4.2 preconditions
//!   (clean + pushed git commit, full 64-char SHA-256 hashes, no reused
//!   experiment_id, config/argv budget match, G1 parent presence for G2) before
//!   a registry row may be written.
//! - [`canary`] — the 15 injected-canary checks from §4.3, each of which the
//!   launcher must *reject*. These are the automated assertion tests.
//! - [`state_machine`] — derives the canonical Round 23 execution state
//!   (`BLOCKED_ENGINE_DATA_OR_EXECUTION` while R0 is the only completed phase).
//!
//! All append operations are atomic per-line writes; a registry or ledger
//! append failure fails the experiment itself (§4.2 last paragraph).

pub mod registry;
pub mod failure_ledger;
pub mod launcher;
pub mod canary;
pub mod state_machine;
pub mod hash;
pub mod manifest;

pub use registry::{Registry, RegistryRow, RowKind, TerminalStatus};
pub use failure_ledger::{FailureLedger, FailureRow};
pub use launcher::{Launcher, LaunchRequest, LaunchError};
pub use canary::{CanaryCheck, CanaryOutcome, run_all_canaries};
pub use state_machine::{ExecutionState, derive_execution_state};
