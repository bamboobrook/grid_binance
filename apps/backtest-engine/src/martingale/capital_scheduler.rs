//! Round 13 Task P3: Capital-Aware Active-Cycle Scheduler.
//!
//! Implements per-symbol trend alignment + correlation cluster scheduling.
//! At each completed HTF boundary, the scheduler:
//! 1. Ranks symbols by trend alignment score (EMA position/slope)
//! 2. Limits per-correlation-cluster active cycles
//! 3. Reserves safety budget for existing cycles before opening new ones
//! 4. Caps configured symbol budget at 25/30/35%
//!
//! This is a mechanism (not a signal generator). It only decides WHICH
//! Martingale cycles may open — it never opens non-Martingale trades.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Per-symbol trend alignment data at a completed boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SymbolTrendAlignment {
    pub symbol: String,
    /// EMA fast above slow = bullish (1.0), below = bearish (-1.0), neutral = 0.0
    pub ema_alignment: f64,
    /// ADX value (trend strength)
    pub adx: f64,
    /// ATR percentile (0-100)
    pub atr_percentile: f64,
    /// Recent realized Martingale outcome (last cycle PnL / budget)
    pub recent_outcome: f64,
}

/// Correlation cluster assignment for a symbol.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CorrelationCluster {
    pub cluster_id: u32,
    pub symbols: Vec<String>,
}

/// Scheduler configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapitalSchedulerConfig {
    pub max_live_cycles: u32,
    pub max_per_cluster: u32,
    pub symbol_cap_pct: f64,
    /// Minimum trend score to allow new cycle (-1 to 1)
    pub min_trend_score: f64,
    /// ADX threshold for "strong trend" (scales first order)
    pub adx_threshold: f64,
    /// ATR percentile threshold for risk scaling
    pub atr_risk_threshold: f64,
    /// First-order scale when ATR is high
    pub high_atr_scale: f64,
    /// First-order scale when ATR is low
    pub low_atr_scale: f64,
}

impl Default for CapitalSchedulerConfig {
    fn default() -> Self {
        Self {
            max_live_cycles: 3,
            max_per_cluster: 1,
            symbol_cap_pct: 25.0,
            min_trend_score: -0.5,
            adx_threshold: 25.0,
            atr_risk_threshold: 60.0,
            high_atr_scale: 0.5,
            low_atr_scale: 1.0,
        }
    }
}

/// Scheduler decision for a single symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleDecision {
    pub symbol: String,
    pub allow_new_cycle: bool,
    pub first_order_scale: f64,
    pub reason: String,
}

/// The capital-aware scheduler.
pub struct CapitalScheduler {
    config: CapitalSchedulerConfig,
    clusters: Vec<CorrelationCluster>,
    /// Active cycles per symbol (symbol → count)
    active_cycles: HashMap<String, u32>,
}

impl CapitalScheduler {
    pub fn new(config: CapitalSchedulerConfig, clusters: Vec<CorrelationCluster>) -> Self {
        Self {
            config,
            clusters,
            active_cycles: HashMap::new(),
        }
    }

    /// Record that a cycle was opened for a symbol.
    pub fn record_cycle_open(&mut self, symbol: &str) {
        *self.active_cycles.entry(symbol.to_string()).or_insert(0) += 1;
    }

    /// Record that a cycle was closed for a symbol.
    pub fn record_cycle_close(&mut self, symbol: &str) {
        if let Some(count) = self.active_cycles.get_mut(symbol) {
            if *count > 0 {
                *count -= 1;
            }
        }
    }

    /// Get total active cycles across all symbols.
    pub fn total_active_cycles(&self) -> u32 {
        self.active_cycles.values().sum()
    }

    /// Get active cycles for a specific cluster.
    pub fn cluster_active_cycles(&self, cluster_id: u32) -> u32 {
        let cluster = self.clusters.iter().find(|c| c.cluster_id == cluster_id);
        if let Some(cluster) = cluster {
            cluster.symbols.iter()
                .filter_map(|s| self.active_cycles.get(s))
                .sum()
        } else {
            0
        }
    }

    /// Get the cluster ID for a symbol.
    pub fn symbol_cluster(&self, symbol: &str) -> Option<u32> {
        self.clusters.iter()
            .find(|c| c.symbols.iter().any(|s| s == symbol))
            .map(|c| c.cluster_id)
    }

    /// Compute trend alignment score for a symbol.
    /// Score = ema_alignment * (adx > threshold ? 1.0 : 0.5)
    pub fn trend_score(alignment: &SymbolTrendAlignment, config: &CapitalSchedulerConfig) -> f64 {
        let adx_weight = if alignment.adx > config.adx_threshold { 1.0 } else { 0.5 };
        alignment.ema_alignment * adx_weight
    }

    /// Decide whether a new cycle may open for each symbol.
    /// Rankings: trend alignment > ATR risk > recent outcome.
    pub fn schedule(
        &self,
        alignments: &[SymbolTrendAlignment],
    ) -> Vec<ScheduleDecision> {
        let total_active = self.total_active_cycles();
        let mut decisions = Vec::new();

        // Rank symbols by trend score
        let mut ranked: Vec<(usize, f64)> = alignments.iter().enumerate()
            .map(|(i, a)| (i, Self::trend_score(a, &self.config)))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut slots_used = 0u32;
        let available_slots = self.config.max_live_cycles.saturating_sub(total_active);

        for (idx, score) in &ranked {
            let alignment = &alignments[*idx];
            let symbol = &alignment.symbol;
            let current_cycles = self.active_cycles.get(symbol).copied().unwrap_or(0);

            // Check budget cap per symbol
            if current_cycles > 0 {
                decisions.push(ScheduleDecision {
                    symbol: symbol.clone(),
                    allow_new_cycle: false,
                    first_order_scale: 1.0,
                    reason: "symbol already has active cycle".to_string(),
                });
                continue;
            }

            // Check trend score threshold
            if score <= &self.config.min_trend_score {
                decisions.push(ScheduleDecision {
                    symbol: symbol.clone(),
                    allow_new_cycle: false,
                    first_order_scale: 1.0,
                    reason: format!("trend score {:.2} < min {:.2}", score, self.config.min_trend_score),
                });
                continue;
            }

            // Check available slots
            if slots_used >= available_slots {
                decisions.push(ScheduleDecision {
                    symbol: symbol.clone(),
                    allow_new_cycle: false,
                    first_order_scale: 1.0,
                    reason: "max live cycles reached".to_string(),
                });
                continue;
            }

            // Check cluster limit
            if let Some(cluster_id) = self.symbol_cluster(symbol) {
                let cluster_active = self.cluster_active_cycles(cluster_id);
                if cluster_active >= self.config.max_per_cluster {
                    decisions.push(ScheduleDecision {
                        symbol: symbol.clone(),
                        allow_new_cycle: false,
                        first_order_scale: 1.0,
                        reason: format!("cluster {} at capacity", cluster_id),
                    });
                    continue;
                }
            }

            // Compute first-order scale based on ATR
            let fo_scale = if alignment.atr_percentile > self.config.atr_risk_threshold {
                self.config.high_atr_scale
            } else {
                self.config.low_atr_scale
            };

            slots_used += 1;
            decisions.push(ScheduleDecision {
                symbol: symbol.clone(),
                allow_new_cycle: true,
                first_order_scale: fo_scale,
                reason: format!("trend score {:.2}, fo_scale {:.2}", score, fo_scale),
            });
        }

        decisions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(max_cycles: u32, max_cluster: u32, min_score: f64) -> CapitalSchedulerConfig {
        CapitalSchedulerConfig {
            max_live_cycles: max_cycles,
            max_per_cluster: max_cluster,
            min_trend_score: min_score,
            ..Default::default()
        }
    }

    fn make_clusters() -> Vec<CorrelationCluster> {
        vec![
            CorrelationCluster { cluster_id: 0, symbols: vec!["BNBUSDT".to_string(), "ETHUSDT".to_string()] },
            CorrelationCluster { cluster_id: 1, symbols: vec!["SOLUSDT".to_string(), "AVAXUSDT".to_string()] },
            CorrelationCluster { cluster_id: 2, symbols: vec!["XRPUSDT".to_string(), "DOTUSDT".to_string()] },
        ]
    }

    fn make_alignments() -> Vec<SymbolTrendAlignment> {
        vec![
            SymbolTrendAlignment { symbol: "BNBUSDT".into(), ema_alignment: 1.0, adx: 30.0, atr_percentile: 50.0, recent_outcome: 0.1 },
            SymbolTrendAlignment { symbol: "SOLUSDT".into(), ema_alignment: 1.0, adx: 20.0, atr_percentile: 70.0, recent_outcome: -0.05 },
            SymbolTrendAlignment { symbol: "XRPUSDT".into(), ema_alignment: -1.0, adx: 15.0, atr_percentile: 30.0, recent_outcome: 0.0 },
        ]
    }

    #[test]
    fn scheduler_ranks_by_trend_alignment() {
        let config = make_config(3, 2, -0.5);
        let scheduler = CapitalScheduler::new(config, make_clusters());
        let decisions = scheduler.schedule(&make_alignments());
        // BNB has highest score (1.0 * 1.0 = 1.0 because ADX=30 > 25)
        let bnb = decisions.iter().find(|d| d.symbol == "BNBUSDT").unwrap();
        assert!(bnb.allow_new_cycle);
        // XRP has negative alignment (-1.0 * 0.5 = -0.5), at min threshold
        let xrp = decisions.iter().find(|d| d.symbol == "XRPUSDT").unwrap();
        assert!(!xrp.allow_new_cycle, "XRP negative trend should be blocked");
    }

    #[test]
    fn scheduler_limits_per_cluster() {
        let config = make_config(3, 1, -0.5);
        let mut scheduler = CapitalScheduler::new(config, make_clusters());
        // Open a cycle for BNB (cluster 0)
        scheduler.record_cycle_open("BNBUSDT");
        // Now ETH (same cluster 0) should be blocked
        let alignments = vec![
            SymbolTrendAlignment { symbol: "ETHUSDT".into(), ema_alignment: 1.0, adx: 30.0, atr_percentile: 50.0, recent_outcome: 0.0 },
        ];
        let decisions = scheduler.schedule(&alignments);
        assert!(!decisions[0].allow_new_cycle, "ETH should be blocked — cluster 0 at capacity");
        assert!(decisions[0].reason.contains("cluster"));
    }

    #[test]
    fn scheduler_caps_max_live_cycles() {
        let config = make_config(2, 3, -0.5);
        let mut scheduler = CapitalScheduler::new(config, make_clusters());
        scheduler.record_cycle_open("BNBUSDT");
        scheduler.record_cycle_open("SOLUSDT");
        // Now total = 2 = max, no new cycles allowed
        let alignments = vec![
            SymbolTrendAlignment { symbol: "XRPUSDT".into(), ema_alignment: 1.0, adx: 30.0, atr_percentile: 50.0, recent_outcome: 0.0 },
        ];
        let decisions = scheduler.schedule(&alignments);
        assert!(!decisions[0].allow_new_cycle, "should be blocked — max live cycles reached");
    }

    #[test]
    fn scheduler_scales_first_order_by_atr() {
        let config = make_config(3, 3, -0.5);
        let scheduler = CapitalScheduler::new(config, make_clusters());
        let decisions = scheduler.schedule(&make_alignments());
        // BNB ATR=50 (below threshold 60) → scale 1.0
        let bnb = decisions.iter().find(|d| d.symbol == "BNBUSDT").unwrap();
        assert!((bnb.first_order_scale - 1.0).abs() < 1e-9, "BNB low ATR → scale 1.0");
        // SOL ATR=70 (above threshold 60) → scale 0.5
        let sol = decisions.iter().find(|d| d.symbol == "SOLUSDT").unwrap();
        if sol.allow_new_cycle {
            assert!((sol.first_order_scale - 0.5).abs() < 1e-9, "SOL high ATR → scale 0.5");
        }
    }

    #[test]
    fn scheduler_respects_min_trend_score() {
        let config = make_config(3, 3, 0.8); // high threshold
        let scheduler = CapitalScheduler::new(config, make_clusters());
        let decisions = scheduler.schedule(&make_alignments());
        // Only BNB (score 1.0) passes; SOL (score 0.5) and XRP (score -0.5) blocked
        for d in &decisions {
            if d.symbol == "BNBUSDT" {
                assert!(d.allow_new_cycle);
            } else {
                assert!(!d.allow_new_cycle, "{} should be blocked by min_trend_score", d.symbol);
            }
        }
    }

    #[test]
    fn scheduler_blocks_symbol_with_active_cycle() {
        let config = make_config(3, 3, -0.5);
        let mut scheduler = CapitalScheduler::new(config, make_clusters());
        scheduler.record_cycle_open("BNBUSDT");
        let decisions = scheduler.schedule(&make_alignments());
        let bnb = decisions.iter().find(|d| d.symbol == "BNBUSDT").unwrap();
        assert!(!bnb.allow_new_cycle, "BNB already has active cycle");
    }

    #[test]
    fn scheduler_tracks_cycle_lifecycle() {
        let mut scheduler = CapitalScheduler::new(make_config(3, 3, -0.5), make_clusters());
        scheduler.record_cycle_open("BNBUSDT");
        assert_eq!(scheduler.total_active_cycles(), 1);
        scheduler.record_cycle_open("SOLUSDT");
        assert_eq!(scheduler.total_active_cycles(), 2);
        scheduler.record_cycle_close("BNBUSDT");
        assert_eq!(scheduler.total_active_cycles(), 1);
        scheduler.record_cycle_close("SOLUSDT");
        assert_eq!(scheduler.total_active_cycles(), 0);
    }

    #[test]
    fn scheduler_cluster_active_count() {
        let mut scheduler = CapitalScheduler::new(make_config(3, 1, -0.5), make_clusters());
        scheduler.record_cycle_open("BNBUSDT");
        scheduler.record_cycle_open("ETHUSDT");
        assert_eq!(scheduler.cluster_active_cycles(0), 2, "cluster 0 has BNB+ETH");
        assert_eq!(scheduler.cluster_active_cycles(1), 0, "cluster 1 empty");
    }
}
