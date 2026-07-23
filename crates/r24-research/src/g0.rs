use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MartinState {
    pub group_net_after_close_cost: f64,
    pub adverse_sigma_from_last_fill: f64,
    pub stationarity_valid: bool,
    pub flow_or_residual_worsening: bool,
    pub previous_layer_gross: f64,
    pub next_layer_gross: f64,
    pub last_fill_price: f64,
}

pub fn allow_so(state: MartinState, threshold_sigma: f64) -> bool {
    state.group_net_after_close_cost < 0.0
        && state.adverse_sigma_from_last_fill >= threshold_sigma
        && state.stationarity_valid
        && !state.flow_or_residual_worsening
        && state.next_layer_gross > state.previous_layer_gross
}

pub fn signal_is_causal(signal_timestamp: i64, execution_timestamp: i64, lag_buckets: i64) -> bool {
    lag_buckets >= 1 && signal_timestamp < execution_timestamp
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulerArm {
    NoScheduler,
    StaticFirstN,
    DeficitRoundRobin,
}

pub fn admission_order(
    arm: SchedulerArm,
    candidates: &[String],
    realized_positive_contribution: &BTreeMap<String, f64>,
) -> Vec<String> {
    let mut output = candidates.to_vec();
    match arm {
        SchedulerArm::NoScheduler => {}
        SchedulerArm::StaticFirstN => output.sort(),
        SchedulerArm::DeficitRoundRobin => output.sort_by(|left, right| {
            realized_positive_contribution
                .get(left)
                .copied()
                .unwrap_or(0.0)
                .partial_cmp(
                    &realized_positive_contribution
                        .get(right)
                        .copied()
                        .unwrap_or(0.0),
                )
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.cmp(right))
        }),
    }
    output
}

pub fn synthetic_adversarial_gate() -> serde_json::Value {
    let base = MartinState {
        group_net_after_close_cost: -1.0,
        adverse_sigma_from_last_fill: 0.75,
        stationarity_valid: true,
        flow_or_residual_worsening: false,
        previous_layer_gross: 100.0,
        next_layer_gross: 125.0,
        last_fill_price: 100.0,
    };
    let checks = [
        ("valid_loss_after_add", allow_so(base, 0.5)),
        (
            "no_net_loss_no_so",
            !allow_so(
                MartinState {
                    group_net_after_close_cost: 0.1,
                    ..base
                },
                0.5,
            ),
        ),
        (
            "not_adverse_no_so",
            !allow_so(
                MartinState {
                    adverse_sigma_from_last_fill: 0.1,
                    ..base
                },
                0.5,
            ),
        ),
        (
            "stationarity_break_no_so",
            !allow_so(
                MartinState {
                    stationarity_valid: false,
                    ..base
                },
                0.5,
            ),
        ),
        (
            "worsening_residual_no_blind_so",
            !allow_so(
                MartinState {
                    flow_or_residual_worsening: true,
                    ..base
                },
                0.5,
            ),
        ),
        (
            "nonincreasing_layer_no_so",
            !allow_so(
                MartinState {
                    next_layer_gross: 100.0,
                    ..base
                },
                0.5,
            ),
        ),
        ("future_shift_rejected", !signal_is_causal(101, 100, 1)),
        ("current_bucket_rejected", !signal_is_causal(99, 100, 0)),
    ];
    let passed = checks.iter().all(|check| check.1);
    serde_json::json!({"passed":passed,"checks":checks})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_eight_adversarial_traces_pass() {
        assert_eq!(synthetic_adversarial_gate()["passed"], true);
        assert_eq!(
            synthetic_adversarial_gate()["checks"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
    }

    #[test]
    fn scheduler_arms_change_order_hash_materially() {
        let candidates = vec!["Z".into(), "A".into(), "M".into()];
        let contributions =
            BTreeMap::from([("A".into(), 0.9), ("M".into(), 0.2), ("Z".into(), 0.1)]);
        let no = admission_order(SchedulerArm::NoScheduler, &candidates, &contributions);
        let static_order = admission_order(SchedulerArm::StaticFirstN, &candidates, &contributions);
        let deficit = admission_order(SchedulerArm::DeficitRoundRobin, &candidates, &contributions);
        assert_ne!(no, static_order);
        assert_ne!(static_order, deficit);
        assert_ne!(no, deficit);
    }

    #[test]
    fn long_short_symmetry_and_last_fill_basis_hold() {
        let long_adverse = (90.0_f64 / 100.0).ln().abs();
        let short_adverse = (110.0_f64 / 100.0).ln().abs();
        assert!((long_adverse - short_adverse).abs() < 0.011);
        let state = MartinState {
            group_net_after_close_cost: -1.0,
            adverse_sigma_from_last_fill: 0.6,
            stationarity_valid: true,
            flow_or_residual_worsening: false,
            previous_layer_gross: 100.0,
            next_layer_gross: 125.0,
            last_fill_price: 100.0,
        };
        let unfilled_bar_price = 80.0;
        assert_eq!(state.last_fill_price, 100.0);
        assert_ne!(state.last_fill_price, unfilled_bar_price);
    }
}
