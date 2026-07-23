use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub mod g0;
pub mod residual;

pub const UNIVERSE: [&str; 8] = [
    "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "LINKUSDT", "LTCUSDT",
];

pub const COLD_STARTS: [&str; 5] = [
    "2023-07-01",
    "2023-07-31",
    "2023-08-30",
    "2023-09-29",
    "2023-10-29",
];

pub const BLOCKS: [(&str, &str); 12] = [
    ("2023-07-01", "2023-09-30"),
    ("2023-10-01", "2023-12-31"),
    ("2024-01-01", "2024-03-31"),
    ("2024-04-01", "2024-06-30"),
    ("2024-07-01", "2024-09-30"),
    ("2024-10-01", "2024-12-31"),
    ("2025-01-01", "2025-03-31"),
    ("2025-04-01", "2025-06-30"),
    ("2025-07-01", "2025-09-30"),
    ("2025-10-01", "2025-12-31"),
    ("2026-01-01", "2026-03-31"),
    ("2026-04-01", "2026-05-31"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub policy_id: String,
    pub family: String,
    pub status: String,
    pub parameters: BTreeMap<String, serde_json::Value>,
    pub mechanism_fingerprint: String,
    pub candidate_quota: u32,
    pub blocked_reason: Option<String>,
}

pub fn mechanism_fingerprint(
    engine_semantics: &str,
    family_formula: &str,
    signal_lag: &str,
    fit_protocol: &str,
    group_construction: &str,
    layer_schedule: &str,
    exit_rule: &str,
    cost_model: &str,
    account_model: &str,
    parameters: &BTreeMap<String, serde_json::Value>,
) -> String {
    let canonical = serde_json::json!({
        "engine_semantics":engine_semantics,
        "family_formula":family_formula,
        "signal_lag":signal_lag,
        "fit_protocol":fit_protocol,
        "group_construction":group_construction,
        "layer_schedule":layer_schedule,
        "exit_rule":exit_rule,
        "cost_model":cost_model,
        "account_model":account_model,
        "parameters":parameters,
    });
    r24_registry::sha256(&serde_json::to_vec(&canonical).unwrap())
}

pub fn preregister_policies() -> Vec<Policy> {
    let mut policies = Vec::new();
    let mut sequence = 0_u32;
    for lookback in [60, 120] {
        for entry_z in [1.5, 2.0] {
            for so_step in [0.5, 0.75] {
                for max_groups in [2, 3] {
                    sequence += 1;
                    let parameters = params([
                        ("fit_lookback_days", serde_json::json!(lookback)),
                        ("hedge_ratio_window_hours", serde_json::json!(168)),
                        ("entry_z", serde_json::json!(entry_z)),
                        ("so_residual_sigma", serde_json::json!(so_step)),
                        ("max_live_groups", serde_json::json!(max_groups)),
                    ]);
                    policies.push(policy(
                        format!("R24-F1-{sequence:02}"),
                        "F1_CAUSAL_RESIDUAL_DISJOINT_PAIR",
                        "preregistered",
                        parameters,
                        4,
                        None,
                    ));
                }
            }
        }
    }
    sequence = 0;
    for window in [7, 30] {
        for quantile in [95.0, 97.5] {
            for atr in [1.0, 1.5] {
                sequence += 1;
                let parameters = params([
                    ("robust_window_days", serde_json::json!(window)),
                    ("tail_quantile_pct", serde_json::json!(quantile)),
                    ("adverse_spacing_atr", serde_json::json!(atr)),
                ]);
                policies.push(policy(
                    format!("R24-F3-M1-{sequence:02}"),
                    "F3_STRICT_LAGGED_M1",
                    "blocked_incomplete_data",
                    parameters,
                    4,
                    Some(
                        "metrics archives start 2023-07-01; required fit starts 2023-01-01".into(),
                    ),
                ));
            }
        }
    }
    sequence = 0;
    for level in ["exact_1pct", "median_exact_1pct_2pct"] {
        for window in [15, 60] {
            for quantile in [95.0, 97.5] {
                sequence += 1;
                let parameters = params([
                    ("depth_level", serde_json::json!(level)),
                    ("flow_window_minutes", serde_json::json!(window)),
                    ("tail_quantile_pct", serde_json::json!(quantile)),
                ]);
                policies.push(policy(
                    format!("R24-F3-M2-{sequence:02}"),
                    "F3_AVAILABLE_LEVEL_M2",
                    "blocked_incomplete_data",
                    parameters,
                    4,
                    Some(
                        "bookDepth lacks full protocol/six assets and aggTrades ends 2023-07-15"
                            .into(),
                    ),
                ));
            }
        }
    }
    for parent in ["eligible_parent_1", "eligible_parent_2"] {
        for model in ["regularized_logistic", "gam"] {
            for side_mode in ["veto", "side_select"] {
                sequence += 1;
                let parameters = params([
                    ("parent_slot", serde_json::json!(parent)),
                    ("model", serde_json::json!(model)),
                    ("sel_mode", serde_json::json!(side_mode)),
                    ("layer_schedule", serde_json::json!("linspace(1,5,10)")),
                ]);
                policies.push(policy(
                    format!("R24-E1-TEMPLATE-{sequence:02}"),
                    "E1_EXACT_SOFT_SEL",
                    "conditional_g1_parent_gate",
                    parameters,
                    8,
                    None,
                ));
            }
        }
    }
    for index in 1..=12 {
        let parameters = params([
            ("combination_slot", serde_json::json!(index)),
            (
                "allocator",
                serde_json::json!(if index % 2 == 0 {
                    "equal_risk"
                } else {
                    "capped_inverse_expected_shortfall"
                }),
            ),
        ]);
        policies.push(policy(
            format!("R24-F2-TEMPLATE-{index:02}"),
            "F2_EVENT_LEVEL_ENSEMBLE",
            "conditional_two_distinct_g1_parents",
            parameters,
            12,
            None,
        ));
    }
    policies
}

fn policy(
    policy_id: String,
    family: &str,
    status: &str,
    parameters: BTreeMap<String, serde_json::Value>,
    quota: u32,
    blocked_reason: Option<String>,
) -> Policy {
    let fingerprint = mechanism_fingerprint(
        "r24-shared-account-event-engine-v1",
        family,
        "completed_bucket_t_minus_1",
        "12-block-prequential-purge-121d",
        "train-only-disjoint-or-family-specific",
        "fixed-soft-ladder-or-exact-e1",
        "martin-fo-so-tp-reduce-abort",
        "fee-slippage-funding-close-liquidation",
        "one-continuous-shared-wallet-margin-reserve",
        &parameters,
    );
    Policy {
        policy_id,
        family: family.into(),
        status: status.into(),
        parameters,
        mechanism_fingerprint: fingerprint,
        candidate_quota: quota,
        blocked_reason,
    }
}

fn params<const N: usize>(
    entries: [(&str, serde_json::Value); N],
) -> BTreeMap<String, serde_json::Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.into(), value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_policy_quotas_are_frozen() {
        let policies = preregister_policies();
        assert_eq!(
            policies
                .iter()
                .filter(|p| p.family.starts_with("F1_"))
                .count(),
            16
        );
        assert_eq!(
            policies
                .iter()
                .filter(|p| p.family == "F3_STRICT_LAGGED_M1")
                .count(),
            8
        );
        assert_eq!(
            policies
                .iter()
                .filter(|p| p.family == "F3_AVAILABLE_LEVEL_M2")
                .count(),
            8
        );
        assert_eq!(
            policies
                .iter()
                .filter(|p| p.family.starts_with("E1_"))
                .count(),
            8
        );
        assert_eq!(
            policies
                .iter()
                .filter(|p| p.family.starts_with("F2_"))
                .count(),
            12
        );
    }

    #[test]
    fn protocol_has_12_contiguous_blocks_and_five_cold_starts() {
        assert_eq!(BLOCKS.len(), 12);
        assert_eq!(
            COLD_STARTS,
            [
                "2023-07-01",
                "2023-07-31",
                "2023-08-30",
                "2023-09-29",
                "2023-10-29"
            ]
        );
        for pair in BLOCKS.windows(2) {
            let previous_end = chrono::NaiveDate::parse_from_str(pair[0].1, "%Y-%m-%d").unwrap();
            let next_start = chrono::NaiveDate::parse_from_str(pair[1].0, "%Y-%m-%d").unwrap();
            assert_eq!(previous_end.succ_opt().unwrap(), next_start);
        }
    }
}
