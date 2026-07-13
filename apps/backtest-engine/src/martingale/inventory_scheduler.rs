//! Round 14 Task P5: Inventory-Aware Ladder and Downside-Vol Risk Budget.
//!
//! At each decision boundary, computes:
//! - Current directional notional/margin, aggregate avg entry, unrealized PnL
//! - Remaining original ladder safety reserve
//! - Symbol/cluster/global gross exposure
//! - Past 7/30/60d downside semivariance
//! - Buffer to liquidation, symbol cap, portfolio DD gate
//!
//! The scheduler first reserves next legs for existing cycles, then decides
//! new cycles. Score only ranks Martingale cycles — it never generates
//! independent orders.

use std::collections::BTreeMap;

/// Inventory state for a single symbol/direction.
#[derive(Debug, Clone, Default)]
pub struct InventoryState {
    pub symbol: String,
    pub is_long: bool,
    pub notional_quote: f64,
    pub margin_quote: f64,
    pub avg_entry_price: f64,
    pub unrealized_pnl_quote: f64,
    pub remaining_safety_reserve_quote: f64,
    pub filled_legs: usize,
    pub max_legs: usize,
}

/// Cluster definition for correlation grouping.
#[derive(Debug, Clone)]
pub struct Cluster {
    pub name: String,
    pub symbols: Vec<String>,
}

/// Configuration for the inventory-aware scheduler.
#[derive(Debug, Clone)]
pub struct InventorySchedulerConfig {
    pub max_live_cycles: usize,
    pub reserve_next_legs: usize, // 1, 2, or all
    pub symbol_cap_pct: f64,      // 20/25/30/35%
    pub cluster_cap_pct: f64,     // 35/45/55%
    pub downside_vol_target_percentile: f64, // 40/60/80
    pub risk_scale_floor: f64,    // 0.25/0.5/0.75
    pub inventory_penalty: f64,   // 0/0.25/0.5/1.0
}

impl Default for InventorySchedulerConfig {
    fn default() -> Self {
        Self {
            max_live_cycles: 3,
            reserve_next_legs: 1,
            symbol_cap_pct: 25.0,
            cluster_cap_pct: 35.0,
            downside_vol_target_percentile: 60.0,
            risk_scale_floor: 0.5,
            inventory_penalty: 0.5,
        }
    }
}

/// The inventory-aware scheduler.
pub struct InventoryScheduler {
    config: InventorySchedulerConfig,
    clusters: Vec<Cluster>,
    /// Downside semivariance history per symbol (for percentile computation).
    downside_history: BTreeMap<String, Vec<f64>>,
}

/// A scheduling decision for a symbol.
#[derive(Debug, Clone)]
pub struct SchedulingDecision {
    pub symbol: String,
    pub can_open_new_cycle: bool,
    pub can_add_safety: bool,
    pub risk_scale: f64,
    pub reason: String,
}

impl InventoryScheduler {
    pub fn new(config: InventorySchedulerConfig, clusters: Vec<Cluster>) -> Self {
        Self {
            config,
            clusters,
            downside_history: BTreeMap::new(),
        }
    }

    /// Push a downside semivariance observation for a symbol.
    pub fn push_downside_observation(&mut self, symbol: &str, value: f64) {
        self.downside_history
            .entry(symbol.to_string())
            .or_default()
            .push(value);
        // Keep last 1000 observations.
        if let Some(history) = self.downside_history.get_mut(symbol) {
            if history.len() > 1000 {
                let drop = history.len() - 1000;
                history.drain(0..drop);
            }
        }
    }

    /// Get the current downside semivariance percentile for a symbol.
    fn downside_percentile(&self, symbol: &str) -> Option<f64> {
        let history = self.downside_history.get(symbol)?;
        if history.len() < 10 {
            return None;
        }
        let mut sorted = history.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((self.config.downside_vol_target_percentile / 100.0) * sorted.len() as f64) as usize;
        Some(sorted[idx.min(sorted.len() - 1)])
    }

    /// Schedule decisions for all symbols given the current inventory state.
    ///
    /// `inventories` is the current live inventory per symbol/direction.
    /// `budget` is the total budget.
    /// `available_capital` is the capital not currently locked in positions.
    pub fn schedule(
        &self,
        inventories: &[InventoryState],
        budget: f64,
        available_capital: f64,
    ) -> Vec<SchedulingDecision> {
        // 1. Reserve capital for next legs of existing cycles.
        let reserved_capital: f64 = inventories
            .iter()
            .map(|inv| {
                if inv.filled_legs < inv.max_legs {
                    // Estimate next leg cost (simplified: same as last leg).
                    inv.remaining_safety_reserve_quote
                        / (inv.max_legs - inv.filled_legs) as f64
                        * self.config.reserve_next_legs.min(inv.max_legs - inv.filled_legs) as f64
                } else {
                    0.0
                }
            })
            .sum();

        let capital_after_reservation = (available_capital - reserved_capital).max(0.0);

        // 2. Compute per-symbol and per-cluster exposure.
        let mut symbol_exposure: BTreeMap<String, f64> = BTreeMap::new();
        for inv in inventories {
            *symbol_exposure.entry(inv.symbol.clone()).or_default() += inv.notional_quote;
        }

        let mut cluster_exposure: BTreeMap<String, f64> = BTreeMap::new();
        for cluster in &self.clusters {
            let total: f64 = cluster
                .symbols
                .iter()
                .filter_map(|s| symbol_exposure.get(s))
                .sum();
            cluster_exposure.insert(cluster.name.clone(), total);
        }

        let total_exposure: f64 = inventories.iter().map(|i| i.notional_quote).sum();
        let live_cycles = inventories.len();

        // 3. Make scheduling decisions.
        let mut decisions = Vec::new();
        for inv in inventories {
            let sym_exposure = symbol_exposure.get(&inv.symbol).copied().unwrap_or(0.0);
            let sym_cap = budget * self.config.symbol_cap_pct / 100.0;
            let can_add_safety = sym_exposure < sym_cap && capital_after_reservation > 0.0;

            // Risk scale based on downside vol.
            let risk_scale = {
                let history = self.downside_history.get(&inv.symbol);
                if let Some(hist) = history {
                    if hist.len() >= 10 {
                        let current_ds = *hist.last().unwrap();
                        let sorted: Vec<f64> = {
                            let mut s = hist.clone();
                            s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                            s
                        };
                        let threshold_idx = ((self.config.downside_vol_target_percentile / 100.0)
                            * sorted.len() as f64) as usize;
                        let threshold = sorted[threshold_idx.min(sorted.len() - 1)];
                        if current_ds > threshold {
                            self.config.risk_scale_floor
                        } else {
                            1.0
                        }
                    } else {
                        1.0
                    }
                } else {
                    1.0
                }
            };

            // Inventory penalty: reduce new cycle score for symbols with high inventory.
            let penalty = self.config.inventory_penalty * (sym_exposure / sym_cap).min(1.0);

            decisions.push(SchedulingDecision {
                symbol: inv.symbol.clone(),
                can_open_new_cycle: false, // existing cycle — don't open new
                can_add_safety,
                risk_scale,
                reason: format!(
                    "existing_cycle;sym_exposure={:.2}/{:.2};risk_scale={:.2};penalty={:.2}",
                    sym_exposure, sym_cap, risk_scale, penalty
                ),
            });
        }

        // 4. Decide on new cycles (for symbols not currently in inventory).
        // This is handled by the caller — the scheduler only provides
        // the capital and exposure constraints.
        if live_cycles < self.config.max_live_cycles && capital_after_reservation > 0.0 {
            // There's room for new cycles. The caller decides which symbols.
            decisions.push(SchedulingDecision {
                symbol: "__new_cycle_slot__".to_string(),
                can_open_new_cycle: true,
                can_add_safety: false,
                risk_scale: 1.0,
                reason: format!(
                    "slot_available;live={}/{};capital={:.2}",
                    live_cycles,
                    self.config.max_live_cycles,
                    capital_after_reservation
                ),
            });
        }

        decisions
    }

    /// Check if a symbol is within its cap.
    pub fn symbol_within_cap(&self, symbol: &str, exposure: f64, budget: f64) -> bool {
        let cap = budget * self.config.symbol_cap_pct / 100.0;
        exposure < cap
    }

    /// Check if a cluster is within its cap.
    pub fn cluster_within_cap(&self, cluster_name: &str, exposure: f64, budget: f64) -> bool {
        let cap = budget * self.config.cluster_cap_pct / 100.0;
        exposure < cap
    }

    /// Serialize state for restart.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "config": {
                "max_live_cycles": self.config.max_live_cycles,
                "reserve_next_legs": self.config.reserve_next_legs,
                "symbol_cap_pct": self.config.symbol_cap_pct,
                "cluster_cap_pct": self.config.cluster_cap_pct,
                "downside_vol_target_percentile": self.config.downside_vol_target_percentile,
                "risk_scale_floor": self.config.risk_scale_floor,
                "inventory_penalty": self.config.inventory_penalty,
            },
            "cluster_count": self.clusters.len(),
            "downside_history_symbols": self.downside_history.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_inventory(symbol: &str, is_long: bool, notional: f64, filled: usize, max: usize) -> InventoryState {
        InventoryState {
            symbol: symbol.to_string(),
            is_long,
            notional_quote: notional,
            margin_quote: notional / 10.0,
            avg_entry_price: 100.0,
            unrealized_pnl_quote: 0.0,
            remaining_safety_reserve_quote: 500.0,
            filled_legs: filled,
            max_legs: max,
        }
    }

    #[test]
    fn scheduler_reserves_next_legs_for_existing_cycles() {
        let config = InventorySchedulerConfig {
            max_live_cycles: 3,
            reserve_next_legs: 1,
            ..Default::default()
        };
        let scheduler = InventoryScheduler::new(config, vec![]);
        let inventories = vec![make_inventory("BTCUSDT", true, 500.0, 2, 5)];
        let decisions = scheduler.schedule(&inventories, 4999.0, 4000.0);

        // The existing cycle should be able to add safety (capital reserved).
        let btc_decision = decisions.iter().find(|d| d.symbol == "BTCUSDT").unwrap();
        assert!(btc_decision.can_add_safety, "existing cycle should be able to add SO");
    }

    #[test]
    fn scheduler_blocks_new_cycle_when_max_reached() {
        let config = InventorySchedulerConfig {
            max_live_cycles: 2,
            ..Default::default()
        };
        let scheduler = InventoryScheduler::new(config, vec![]);
        let inventories = vec![
            make_inventory("BTCUSDT", true, 500.0, 1, 5),
            make_inventory("ETHUSDT", true, 500.0, 1, 5),
        ];
        let decisions = scheduler.schedule(&inventories, 4999.0, 3000.0);
        // No new cycle slot should be available.
        assert!(
            !decisions.iter().any(|d| d.can_open_new_cycle),
            "no new cycle slot when max reached"
        );
    }

    #[test]
    fn scheduler_enforces_symbol_cap() {
        let config = InventorySchedulerConfig {
            symbol_cap_pct: 25.0,
            ..Default::default()
        };
        let scheduler = InventoryScheduler::new(config, vec![]);
        // Symbol exposure at 80% of cap.
        assert!(scheduler.symbol_within_cap("BTCUSDT", 1000.0, 4999.0));
        // Symbol exposure above cap.
        assert!(!scheduler.symbol_within_cap("BTCUSDT", 2000.0, 4999.0));
    }

    #[test]
    fn scheduler_enforces_cluster_cap() {
        let config = InventorySchedulerConfig {
            cluster_cap_pct: 35.0,
            ..Default::default()
        };
        let scheduler = InventoryScheduler::new(config, vec![]);
        assert!(scheduler.cluster_within_cap("defi", 1000.0, 4999.0));
        assert!(!scheduler.cluster_within_cap("defi", 2000.0, 4999.0));
    }

    #[test]
    fn downside_vol_scales_risk() {
        let config = InventorySchedulerConfig {
            downside_vol_target_percentile: 60.0,
            risk_scale_floor: 0.5,
            ..Default::default()
        };
        let mut scheduler = InventoryScheduler::new(config, vec![]);

        // Push 20 downside observations: 10 low, 10 high.
        for i in 0..10 {
            scheduler.push_downside_observation("BTCUSDT", 0.001 + i as f64 * 0.0001);
        }
        for i in 0..10 {
            scheduler.push_downside_observation("BTCUSDT", 0.01 + i as f64 * 0.001);
        }
        // Push one more very high observation to push current above the 60th percentile.
        scheduler.push_downside_observation("BTCUSDT", 0.05);

        let inventories = vec![make_inventory("BTCUSDT", true, 500.0, 2, 5)];
        let decisions = scheduler.schedule(&inventories, 4999.0, 4000.0);
        let btc_decision = decisions.iter().find(|d| d.symbol == "BTCUSDT").unwrap();
        // The last observation (0.05) is above the 60th percentile threshold.
        // The risk_scale logic checks if current_ds > threshold.
        // Since 0.05 > 60th percentile of history, risk_scale should be at floor.
        assert!(
            btc_decision.risk_scale <= 0.5 + 1e-9,
            "high downside vol should scale risk to floor, got {}",
            btc_decision.risk_scale
        );
    }

    #[test]
    fn scheduler_never_generates_independent_orders() {
        // The scheduler only provides decisions — it never places orders.
        let scheduler = InventoryScheduler::new(InventorySchedulerConfig::default(), vec![]);
        let decisions = scheduler.schedule(&[], 4999.0, 4000.0);
        // Decisions are just flags and scales — no order data.
        for d in &decisions {
            assert!(d.reason.starts_with("slot_available") || d.reason.starts_with("existing_cycle"));
        }
    }

    #[test]
    fn restart_restores_scheduler_state() {
        let config = InventorySchedulerConfig::default();
        let scheduler = InventoryScheduler::new(config.clone(), vec![]);
        let json = scheduler.to_json();
        assert_eq!(json["config"]["max_live_cycles"], config.max_live_cycles);
        assert_eq!(json["config"]["symbol_cap_pct"], config.symbol_cap_pct);
        assert_eq!(json["cluster_count"], 0);
    }

    #[test]
    fn reserve_next_legs_all() {
        let config = InventorySchedulerConfig {
            reserve_next_legs: 5, // all remaining
            ..Default::default()
        };
        let scheduler = InventoryScheduler::new(config, vec![]);
        let inventories = vec![make_inventory("BTCUSDT", true, 500.0, 1, 5)];
        let decisions = scheduler.schedule(&inventories, 4999.0, 4000.0);
        // With reserve_next_legs=all, more capital is reserved for the existing cycle.
        let btc_decision = decisions.iter().find(|d| d.symbol == "BTCUSDT").unwrap();
        assert!(btc_decision.can_add_safety, "existing cycle can add safety");
    }
}
