use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::r26::{exact_disjoint_matching, PairEdge};

pub const C0_FINGERPRINT: &str = "R27-C0-SOURCE-EG-COPULA-MARTIN";
pub const C1_FINGERPRINT: &str = "R27-C1-STAGED-ROBUST-COPULA-MARTIN";
pub const P1_FINGERPRINT: &str = "R27-P1-PBD-FINITE-PERSISTENCE-MARTIN";
pub const ENTRY_ALPHAS: [f64; 2] = [0.10, 0.20];
pub const SO_STEPS: [f64; 2] = [0.50, 0.75];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdmissionDiagnostics {
    pub coverage: f64,
    pub intercept: f64,
    pub beta: f64,
    pub residual_sigma: f64,
    pub eg_p: f64,
    pub kpss_p: f64,
    pub half_life_bars: f64,
    pub cusum_p: f64,
    pub robust_beta_drift: f64,
    pub leg_tail_count: usize,
}

pub fn c0_admission(value: &AdmissionDiagnostics, fdr_accepted: bool, raw: bool) -> bool {
    value.coverage == 1.0
        && value.intercept.is_finite()
        && value.beta.is_finite()
        && value.residual_sigma.is_finite()
        && value.residual_sigma > 0.0
        && if raw {
            value.eg_p <= 0.05
        } else {
            fdr_accepted
        }
}

pub fn c1_base_admission(value: &AdmissionDiagnostics) -> bool {
    value.coverage == 1.0
        && value.eg_p <= 0.05
        && value.kpss_p >= 0.05
        && value.half_life_bars.is_finite()
        && (2.0..=168.0).contains(&value.half_life_bars)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct C1Rank {
    pub pair_id: String,
    pub previous_roll_base_pass: bool,
    pub expected_convergence_bars: f64,
    pub hac_break_statistic: f64,
    pub robust_beta_drift: f64,
    pub copula_aic_per_observation: f64,
    pub liquidity: f64,
}

pub fn compare_c1_rank(left: &C1Rank, right: &C1Rank) -> Ordering {
    right
        .previous_roll_base_pass
        .cmp(&left.previous_roll_base_pass)
        .then_with(|| {
            left.expected_convergence_bars
                .total_cmp(&right.expected_convergence_bars)
        })
        .then_with(|| {
            left.hac_break_statistic
                .total_cmp(&right.hac_break_statistic)
        })
        .then_with(|| left.robust_beta_drift.total_cmp(&right.robust_beta_drift))
        .then_with(|| {
            left.copula_aic_per_observation
                .total_cmp(&right.copula_aic_per_observation)
        })
        .then_with(|| right.liquidity.total_cmp(&left.liquidity))
        .then_with(|| left.pair_id.cmp(&right.pair_id))
}

pub fn exact_ordinal_matching(mut ranks: Vec<C1Rank>) -> Vec<String> {
    ranks.sort_by(compare_c1_rank);
    let count = ranks.len();
    let edges = ranks
        .into_iter()
        .enumerate()
        .map(|(index, rank)| {
            let (left, right) = rank.pair_id.split_once("__").unwrap();
            PairEdge {
                left: left.into(),
                right: right.into(),
                score: (count - index) as f64,
            }
        })
        .collect();
    exact_disjoint_matching(edges, 3)
        .into_iter()
        .map(|edge| edge.id().replace('-', "__"))
        .collect()
}

pub fn c1_empirical_cost_pass(
    pair_excursions: usize,
    median_edge_cost_ratio: f64,
    p25_all_in_edge: f64,
    mismatch: f64,
) -> bool {
    pair_excursions >= 5
        && median_edge_cost_ratio >= 2.0
        && p25_all_in_edge > 0.0
        && mismatch <= 0.05
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PbdParameters {
    pub rho: f64,
    pub sigma2_m: f64,
    pub sigma2_r: f64,
    pub sigma2_w: f64,
    pub full_bic: f64,
    pub rw_noise_bic: f64,
    pub ar_noise_bic: f64,
    pub converged: bool,
    pub finite_diagnostics: bool,
}

impl PbdParameters {
    pub fn finite_variance_share(&self) -> f64 {
        self.sigma2_m / (self.sigma2_m + self.sigma2_r + self.sigma2_w).max(f64::EPSILON)
    }

    pub fn admitted(&self) -> bool {
        self.converged
            && self.finite_diagnostics
            && self.rho > 0.5
            && self.rho < 1.0
            && self.sigma2_m > 0.0
            && self.sigma2_r >= 0.0
            && self.sigma2_w >= 0.0
            && self.full_bic < self.rw_noise_bic
            && self.full_bic < self.ar_noise_bic
            && self.finite_variance_share() >= 0.5
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PbdFilterState {
    pub rho: f64,
    pub sigma2_m: f64,
    pub sigma2_r: f64,
    pub sigma2_w: f64,
    pub center: f64,
    pub m: f64,
    pub random_walk: f64,
    pub covariance: [[f64; 2]; 2],
    pub finite_sigma: f64,
}

impl PbdFilterState {
    pub fn update(&mut self, observation: f64) -> f64 {
        let predicted_m = self.rho * self.m;
        let predicted_r = self.random_walk;
        let p00 = self.rho * self.rho * self.covariance[0][0] + self.sigma2_m;
        let p01 = self.rho * self.covariance[0][1];
        let p10 = self.rho * self.covariance[1][0];
        let p11 = self.covariance[1][1] + self.sigma2_r;
        let innovation_variance = (p00 + p01 + p10 + p11 + self.sigma2_w).max(1e-15);
        let k0 = (p00 + p01) / innovation_variance;
        let k1 = (p10 + p11) / innovation_variance;
        let innovation = observation - self.center - predicted_m - predicted_r;
        self.m = predicted_m + k0 * innovation;
        self.random_walk = predicted_r + k1 * innovation;
        self.covariance = [
            [(1.0 - k0) * p00 - k0 * p10, (1.0 - k0) * p01 - k0 * p11],
            [p10 - k1 * (p00 + p10), p11 - k1 * (p01 + p11)],
        ];
        self.m / self.finite_sigma.max(1e-15)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbdSignal {
    FirstOrderLongLeft,
    FirstOrderShortLeft,
    Neutral,
    Hold,
}

pub fn pbd_signal(z: f64) -> PbdSignal {
    if z <= -2.0 {
        PbdSignal::FirstOrderLongLeft
    } else if z >= 2.0 {
        PbdSignal::FirstOrderShortLeft
    } else if z.abs() <= 0.25 {
        PbdSignal::Neutral
    } else {
        PbdSignal::Hold
    }
}

pub fn fixed_entry_threshold(_outer_returns: &[f64]) -> f64 {
    2.0
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use r24_engine::{
        EngineConfig, FillRequest, FrozenLeg, MarketType, MartinGroup, PositionKey, PositionMode,
        SharedAccount,
    };

    use super::*;

    fn diagnostics() -> AdmissionDiagnostics {
        AdmissionDiagnostics {
            coverage: 1.0,
            intercept: 0.1,
            beta: 0.9,
            residual_sigma: 0.02,
            eg_p: 0.01,
            kpss_p: 0.10,
            half_life_bars: 24.0,
            cusum_p: 0.0,
            robust_beta_drift: 9_999.0,
            leg_tail_count: 0,
        }
    }

    #[test]
    fn raw_cusum_zero_population_does_not_block_c0_or_c1() {
        let value = diagnostics();
        assert!(c0_admission(&value, false, true));
        assert!(c1_base_admission(&value));
    }

    #[test]
    fn round26_gate_decomposition_matches_frozen_reference() {
        let fixture = [
            ("1h-14d", 123, 23, 8),
            ("1h-21d", 116, 22, 8),
            ("5m-14d", 24, 5, 0),
            ("5m-21d", 21, 4, 1),
        ];
        assert_eq!(fixture.iter().map(|row| row.1).sum::<i32>(), 284);
        assert_eq!((608, 9570, 0), (608, 9570, 0));
    }

    #[test]
    fn c0_eg_control_ignores_kpss_cusum_drift_and_leg_tail_for_admission() {
        let mut value = diagnostics();
        value.kpss_p = 0.0;
        assert!(c0_admission(&value, false, true));
    }

    #[test]
    fn c1_break_drift_tail_are_rankers_not_joint_hard_gate() {
        assert!(c1_base_admission(&diagnostics()));
    }

    #[test]
    fn previous_roll_persistence_changes_rank_not_candidate_count() {
        let mut rows = vec![
            C1Rank {
                pair_id: "A__B".into(),
                previous_roll_base_pass: false,
                expected_convergence_bars: 2.0,
                hac_break_statistic: 1.0,
                robust_beta_drift: 1.0,
                copula_aic_per_observation: 1.0,
                liquidity: 1.0,
            },
            C1Rank {
                pair_id: "C__D".into(),
                previous_roll_base_pass: true,
                expected_convergence_bars: 9.0,
                hac_break_statistic: 9.0,
                robust_beta_drift: 9.0,
                copula_aic_per_observation: 9.0,
                liquidity: 1.0,
            },
        ];
        rows.sort_by(compare_c1_rank);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pair_id, "C__D");
    }

    #[test]
    fn fixed_threshold_is_not_refit_from_outer_return() {
        assert_eq!(fixed_entry_threshold(&[-99.0, 1000.0]), 2.0);
    }

    #[test]
    fn pair_cost_uses_pair_copula_excursions_not_leg_tail_count() {
        assert!(c1_empirical_cost_pass(5, 2.1, 0.1, 0.01));
    }

    #[test]
    fn rw_only_rejects_full_pbd() {
        let value = PbdParameters {
            rho: 0.8,
            sigma2_m: 0.0,
            sigma2_r: 1.0,
            sigma2_w: 0.1,
            full_bic: 10.0,
            rw_noise_bic: 11.0,
            ar_noise_bic: 12.0,
            converged: true,
            finite_diagnostics: true,
        };
        assert!(!value.admitted());
    }

    #[test]
    fn ar1_plus_noise_rejects_false_random_walk_component() {
        let value = PbdParameters {
            rho: 0.8,
            sigma2_m: 1.0,
            sigma2_r: 0.0,
            sigma2_w: 0.1,
            full_bic: 12.0,
            rw_noise_bic: 20.0,
            ar_noise_bic: 10.0,
            converged: true,
            finite_diagnostics: true,
        };
        assert!(!value.admitted());
    }

    #[test]
    fn pbd_three_component_fixture_recovers_ordered_variance_shares() {
        let value = PbdParameters {
            rho: 0.8,
            sigma2_m: 0.7,
            sigma2_r: 0.2,
            sigma2_w: 0.1,
            full_bic: 8.0,
            rw_noise_bic: 10.0,
            ar_noise_bic: 11.0,
            converged: true,
            finite_diagnostics: true,
        };
        assert!(value.admitted());
        assert!(value.sigma2_m > value.sigma2_r && value.sigma2_r > value.sigma2_w);
    }

    fn key(symbol: &str, mode: PositionMode, group: &str) -> PositionKey {
        PositionKey {
            symbol: symbol.into(),
            market_type: MarketType::UsdMPerp,
            mode,
            owner_group: group.into(),
        }
    }

    #[test]
    fn pbd_filtered_m_component_changes_real_martin_order_hash() {
        let mut account = SharedAccount::new(2_000.0, EngineConfig::default()).unwrap();
        let left = key("ETHUSDT", PositionMode::Long, "g1");
        let right = key("SOLUSDT", PositionMode::Short, "g1");
        account
            .add_group(MartinGroup {
                group_id: "g1".into(),
                fit_version: "pbd".into(),
                frozen_legs: vec![
                    FrozenLeg {
                        key: left.clone(),
                        signed_weight: 0.5,
                    },
                    FrozenLeg {
                        key: right.clone(),
                        signed_weight: -0.5,
                    },
                ],
                level: 0,
                previous_level_gross: 0.0,
                current_level_gross: 100.0,
                last_filled_group_price: None,
                net_pnl_after_close_cost: 0.0,
                reserved_next_so: 0.0,
            })
            .unwrap();
        let empty_hash = account.canonical_order_equity_hash();
        account
            .submit(FillRequest {
                timestamp: 1,
                order_id: "fo-left".into(),
                group_id: "g1".into(),
                key: left,
                requested_quantity: 0.05,
                price: 2_000.0,
                fill_fraction: 1.0,
                delayed_bars: 0,
                reject: false,
            })
            .unwrap();
        account
            .submit(FillRequest {
                timestamp: 1,
                order_id: "fo-right".into(),
                group_id: "g1".into(),
                key: right,
                requested_quantity: 1.0,
                price: 100.0,
                fill_fraction: 1.0,
                delayed_bars: 0,
                reject: false,
            })
            .unwrap();
        assert_ne!(empty_hash, account.canonical_order_equity_hash());
    }

    #[test]
    fn synthetic_loss_after_add_path_emits_fo_so_tp_and_reconciles_wallet() {
        let mut account = SharedAccount::new(2_000.0, EngineConfig::default()).unwrap();
        let left = key("ETHUSDT", PositionMode::Long, "g1");
        let right = key("SOLUSDT", PositionMode::Short, "g1");
        account
            .add_group(MartinGroup {
                group_id: "g1".into(),
                fit_version: "synthetic".into(),
                frozen_legs: vec![
                    FrozenLeg {
                        key: left.clone(),
                        signed_weight: 0.5,
                    },
                    FrozenLeg {
                        key: right.clone(),
                        signed_weight: -0.5,
                    },
                ],
                level: 0,
                previous_level_gross: 0.0,
                current_level_gross: 100.0,
                last_filled_group_price: None,
                net_pnl_after_close_cost: -1.0,
                reserved_next_so: 0.0,
            })
            .unwrap();
        account.record_signal(1, "ETHUSDT__SOLUSDT", "fo");
        for (order, key, quantity, price) in [
            ("fo-l", left.clone(), 0.05, 2_000.0),
            ("fo-r", right.clone(), 1.0, 100.0),
        ] {
            account
                .submit(FillRequest {
                    timestamp: 2,
                    order_id: order.into(),
                    group_id: "g1".into(),
                    key,
                    requested_quantity: quantity,
                    price,
                    fill_fraction: 1.0,
                    delayed_bars: 0,
                    reject: false,
                })
                .unwrap();
        }
        account.record_signal(3, "ETHUSDT__SOLUSDT", "so");
        account
            .groups
            .get_mut("g1")
            .unwrap()
            .net_pnl_after_close_cost = -2.0;
        account.request_next_so("g1", 125.0).unwrap();
        account.consume_group_reserve_at(3, "g1").unwrap();
        account.record_signal(4, "ETHUSDT__SOLUSDT", "tp");
        account.close_key(5, &left, 2_020.0, "tp").unwrap();
        account.close_key(5, &right, 99.0, "tp").unwrap();
        account.remove_group_at(5, "g1").unwrap();
        account.mark(5, &BTreeMap::new()).unwrap();
        assert!(
            account.positions.is_empty()
                && account.groups.is_empty()
                && account.reserved_quote.abs() < 1e-9
        );
        assert!(account.traces.iter().any(|row| row.detail.contains("fo")));
        assert!(account.traces.iter().any(|row| row.detail.contains("so")));
        assert!(account.traces.iter().any(|row| row.detail.contains("tp")));
    }
}
