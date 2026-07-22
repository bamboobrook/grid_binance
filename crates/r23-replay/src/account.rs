//! Continuous shared cash/margin/equity account (§5.1 items 4, 9).
//!
//! §5.1.4: spot cash, perp wallet, inventory, initial/maintenance margin share
//! one account. §5.1.9: block transitions never reset cash or equity. This
//! struct carries the account state across the 12-block timeline.
//!
//! The sync cycle engine returns a settled ending equity per call. In
//! [`BlockWiseCarry`][crate::ReplayMode] mode the driver seeds each block with
//! the previous block's settled equity as its budget, proving the account is
//! continuous and never resets the principal arbitrarily.

use serde::{Deserialize, Serialize};

/// Snapshot of the continuous account at a block boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSnapshot {
    pub block_index: u32,
    pub starting_equity_quote: f64,
    pub ending_equity_quote: f64,
    pub block_realized_pnl_quote: f64,
    pub block_funding_quote: f64,
    pub block_fees_quote: f64,
    /// True if equity ever dropped to/below zero in this block (§5.1.12 breach).
    pub equity_nonpositive_observed: bool,
    pub liquidation_count: u64,
}

/// The continuous account. Seed with the principal; carry equity forward.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousAccount {
    pub principal_quote: f64,
    pub current_equity_quote: f64,
    pub snapshots: Vec<AccountSnapshot>,
    pub any_liquidation: bool,
    pub any_equity_nonpositive: bool,
}

impl ContinuousAccount {
    pub fn new(principal_quote: f64) -> Self {
        Self {
            principal_quote,
            current_equity_quote: principal_quote,
            snapshots: Vec::new(),
            any_liquidation: false,
            any_equity_nonpositive: false,
        }
    }

    /// Record a block's settled outcome. Carries equity forward WITHOUT resetting
    /// principal (§5.1.9). Flags breaches (§5.1.12).
    pub fn settle_block(
        &mut self,
        block_index: u32,
        starting_equity_quote: f64,
        ending_equity_quote: f64,
        block_realized_pnl_quote: f64,
        block_funding_quote: f64,
        block_fees_quote: f64,
        equity_nonpositive_observed: bool,
        liquidation_count: u64,
    ) {
        if equity_nonpositive_observed {
            self.any_equity_nonpositive = true;
        }
        if liquidation_count > 0 {
            self.any_liquidation = true;
        }
        self.snapshots.push(AccountSnapshot {
            block_index,
            starting_equity_quote,
            ending_equity_quote,
            block_realized_pnl_quote,
            block_funding_quote,
            block_fees_quote,
            equity_nonpositive_observed,
            liquidation_count,
        });
        // Carry the settled equity forward; the principal is never reset.
        self.current_equity_quote = ending_equity_quote;
    }

    /// §5.1 hard gate: any liquidation or equity <= 0 is an immediate terminal.
    pub fn breached(&self) -> bool {
        self.any_liquidation || self.any_equity_nonpositive
    }

    /// The budget to seed the next block with (the carried-forward equity).
    pub fn next_block_budget(&self) -> f64 {
        self.current_equity_quote.max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_carries_equity_without_resetting_principal() {
        let mut acc = ContinuousAccount::new(500.0);
        assert_eq!(acc.current_equity_quote, 500.0);
        // block 1: +50
        acc.settle_block(1, 500.0, 550.0, 50.0, -1.0, -2.0, false, 0);
        assert_eq!(acc.current_equity_quote, 550.0);
        assert_eq!(acc.principal_quote, 500.0); // principal unchanged
        // block 2 seeds with carried equity, not principal
        let b2_budget = acc.next_block_budget();
        assert_eq!(b2_budget, 550.0);
        acc.settle_block(2, 550.0, 530.0, -20.0, -0.5, -1.0, false, 0);
        assert_eq!(acc.current_equity_quote, 530.0);
        assert!(!acc.breached());
    }

    #[test]
    fn liquidation_flags_breach() {
        let mut acc = ContinuousAccount::new(500.0);
        acc.settle_block(1, 500.0, 0.0, -500.0, 0.0, 0.0, true, 1);
        assert!(acc.breached());
    }
}
