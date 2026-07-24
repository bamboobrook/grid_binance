use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FINGERPRINT: &str = "R26_WEEKLY_TOP20_REFERENCE_SPREAD_COPULA_MARTIN";
pub const REFERENCE: &str = "BTCUSDT";
pub const ALTS: [&str; 29] = [
    "AAVEUSDT", "ADAUSDT", "ALGOUSDT", "APTUSDT", "ATOMUSDT", "AVAXUSDT", "BCHUSDT", "BNBUSDT",
    "COMPUSDT", "CRVUSDT", "DASHUSDT", "DOGEUSDT", "DOTUSDT", "DYDXUSDT", "EGLDUSDT", "ETCUSDT",
    "ETHUSDT", "FILUSDT", "GALAUSDT", "HBARUSDT", "ICPUSDT", "INJUSDT", "LINKUSDT", "NEARUSDT",
    "SOLUSDT", "TRXUSDT", "UNIUSDT", "XRPUSDT", "ZECUSDT",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum SelectorArm {
    Raw,
    Fdr,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PairEdge {
    pub left: String,
    pub right: String,
    pub score: f64,
}

impl PairEdge {
    pub fn normalized(mut self) -> Self {
        if self.right < self.left {
            std::mem::swap(&mut self.left, &mut self.right);
        }
        self
    }

    pub fn id(&self) -> String {
        format!("{}-{}", self.left, self.right)
    }
}

/// Exact maximum-cardinality, then maximum-weight disjoint matching.
/// The population is capped at three pairs, so enumerating valid combinations is bounded.
pub fn exact_disjoint_matching(mut edges: Vec<PairEdge>, max_pairs: usize) -> Vec<PairEdge> {
    edges = edges.into_iter().map(PairEdge::normalized).collect();
    edges.sort_by_key(PairEdge::id);
    let mut best = Vec::new();
    let mut current = Vec::new();
    let mut used = BTreeSet::new();
    enumerate_matchings(&edges, 0, max_pairs, &mut used, &mut current, &mut best);
    best
}

fn enumerate_matchings(
    edges: &[PairEdge],
    start: usize,
    max_pairs: usize,
    used: &mut BTreeSet<String>,
    current: &mut Vec<PairEdge>,
    best: &mut Vec<PairEdge>,
) {
    if better_matching(current, best) {
        *best = current.clone();
    }
    if current.len() == max_pairs {
        return;
    }
    for index in start..edges.len() {
        let edge = &edges[index];
        if used.contains(&edge.left) || used.contains(&edge.right) {
            continue;
        }
        used.insert(edge.left.clone());
        used.insert(edge.right.clone());
        current.push(edge.clone());
        enumerate_matchings(edges, index + 1, max_pairs, used, current, best);
        current.pop();
        used.remove(&edge.left);
        used.remove(&edge.right);
    }
}

fn better_matching(candidate: &[PairEdge], incumbent: &[PairEdge]) -> bool {
    if candidate.len() != incumbent.len() {
        return candidate.len() > incumbent.len();
    }
    let candidate_weight = candidate.iter().map(|edge| edge.score).sum::<f64>();
    let incumbent_weight = incumbent.iter().map(|edge| edge.score).sum::<f64>();
    if (candidate_weight - incumbent_weight).abs() > 1e-12 {
        return candidate_weight > incumbent_weight;
    }
    let candidate_ids = candidate.iter().map(PairEdge::id).collect::<Vec<_>>();
    let incumbent_ids = incumbent.iter().map(PairEdge::id).collect::<Vec<_>>();
    candidate_ids < incumbent_ids
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExcursionEdge {
    pub gross_edge_quote: f64,
    pub censored: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CostGateDecision {
    pub excursion_count: usize,
    pub censored_count: usize,
    pub median_all_in_edge_quote: f64,
    pub p25_all_in_edge_quote: f64,
    pub median_edge_cost_ratio: f64,
    pub round_trip_cost_quote: f64,
    pub gross_mismatch_ratio: f64,
    pub passed: bool,
}

pub fn evaluate_cost_gate(
    excursions: &[ExcursionEdge],
    round_trip_cost_quote: f64,
    gross_mismatch_ratio: f64,
) -> CostGateDecision {
    let mut all_in = excursions
        .iter()
        .map(|row| {
            let gross = if row.censored {
                row.gross_edge_quote.min(0.0)
            } else {
                row.gross_edge_quote
            };
            gross - round_trip_cost_quote
        })
        .collect::<Vec<_>>();
    all_in.sort_by(f64::total_cmp);
    let median = quantile(&all_in, 0.5);
    let p25 = quantile(&all_in, 0.25);
    let ratio = if round_trip_cost_quote > 0.0 {
        median / round_trip_cost_quote
    } else {
        f64::NEG_INFINITY
    };
    let passed =
        excursions.len() >= 20 && ratio >= 2.0 && p25 > 0.0 && gross_mismatch_ratio <= 0.05;
    CostGateDecision {
        excursion_count: excursions.len(),
        censored_count: excursions.iter().filter(|row| row.censored).count(),
        median_all_in_edge_quote: median,
        p25_all_in_edge_quote: p25,
        median_edge_cost_ratio: ratio,
        round_trip_cost_quote,
        gross_mismatch_ratio,
        passed,
    }
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NEG_INFINITY;
    }
    let position = q.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower as f64)
    }
}

pub fn benjamini_hochberg(p_values: &[f64], q: f64) -> Vec<bool> {
    let mut ordered = p_values.iter().copied().enumerate().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    let cutoff = ordered
        .iter()
        .enumerate()
        .filter(|(rank, (_, p))| *p <= q * (*rank + 1) as f64 / p_values.len() as f64)
        .map(|(rank, _)| rank + 1)
        .max()
        .unwrap_or(0);
    let mut accepted = vec![false; p_values.len()];
    for (index, _) in ordered.into_iter().take(cutoff) {
        accepted[index] = true;
    }
    accepted
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiquidityObservation {
    pub symbol: String,
    pub zero_volume_ratio: f64,
    pub p10_minute_quote_volume: f64,
    pub median_daily_quote_volume: f64,
    pub fit_cutoff_ms: i64,
}

pub fn weekly_top20(
    rows: &[LiquidityObservation],
    roll_anchor_ms: i64,
) -> Vec<LiquidityObservation> {
    let mut eligible = rows
        .iter()
        .filter(|row| row.fit_cutoff_ms < roll_anchor_ms)
        .filter(|row| row.zero_volume_ratio <= 0.01)
        .filter(|row| row.p10_minute_quote_volume >= 10_000.0)
        .cloned()
        .collect::<Vec<_>>();
    eligible.sort_by(|left, right| {
        right
            .median_daily_quote_volume
            .total_cmp(&left.median_daily_quote_volume)
            .then_with(|| left.symbol.cmp(&right.symbol))
    });
    eligible.truncate(20);
    eligible
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrozenModel {
    pub snapshot_sha256: String,
    pub beta_left: f64,
    pub beta_right: f64,
    pub family: String,
    pub rho: f64,
    pub nu: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveCycle {
    pub group_id: String,
    pub model: FrozenModel,
}

impl ActiveCycle {
    pub fn roll_selector(&mut self, _new_model: &FrozenModel) {
        // Existing cycles deliberately retain their open-time model.
    }
}

pub fn independent_reference_error(actual: &[f64], expected: &[f64]) -> Result<f64> {
    if actual.len() != expected.len() || actual.is_empty() {
        bail!("reference vector length mismatch");
    }
    Ok(actual
        .iter()
        .zip(expected)
        .map(|(left, right)| (left - right).abs())
        .fold(0.0, f64::max))
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn read_hashed_snapshot(path: &Path, expected_sha256: &str) -> Result<Vec<u8>> {
    let bytes = fs::read(path).with_context(|| format!("read snapshot {}", path.display()))?;
    let actual = sha256_bytes(&bytes);
    if actual != expected_sha256 {
        bail!("model_snapshot_hash_mismatch:{expected_sha256}:{actual}");
    }
    Ok(bytes)
}

pub fn observations_through_cutoff(observations: &[(i64, f64)], cutoff_ms: i64) -> Vec<(i64, f64)> {
    observations
        .iter()
        .copied()
        .filter(|(timestamp, _)| *timestamp < cutoff_ms)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r25::normal_cdf;
    use crate::r25_corrected::kendall_tau_b;
    use serde_json::Value;

    fn audit() -> Value {
        serde_json::from_str(include_str!(
            "../../../docs/superpowers/artifacts/glm-martingale-core-round25-corrected/audit/round25r-selected-fit-cache-independent-audit.json"
        ))
        .unwrap()
    }

    fn audit_leg(symbol: &str, field: &str) -> Vec<f64> {
        audit()["caches"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|cache| cache["models"].as_array().unwrap())
            .flat_map(|model| model["legs"].as_array().unwrap())
            .filter(|leg| leg["symbol"] == symbol)
            .filter_map(|leg| leg["independent"][field].as_f64())
            .collect()
    }

    #[test]
    fn normal_cdf_is_not_adf_or_engle_granger_p_value() {
        let statistic = -2.511_238_257_733_107_4;
        let authoritative_adf_p = 0.112_763_825_422_620_37;
        assert!((normal_cdf(statistic) - authoritative_adf_p).abs() > 0.05);
    }

    #[test]
    fn round25r_doge_cache_matches_statsmodels_residual_adf_0_1127638() {
        let values = audit_leg("DOGEUSDT", "residual_adf_p_value");
        assert!(values
            .iter()
            .any(|value| (*value - 0.112_763_825_422_620_37).abs() < 1e-15));
    }

    #[test]
    fn round25r_link_cache_matches_statsmodels_kpss_0_0331150() {
        let values = audit_leg("LINKUSDT", "kpss_p_value");
        assert!(values
            .iter()
            .any(|value| (*value - 0.033_114_955_659_089_99).abs() < 1e-15));
    }

    #[test]
    fn only_sol_doge_passes_selected_round25r_pairs_under_independent_eg_kpss() {
        let audit = audit();
        let passed = audit["caches"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|cache| cache["models"].as_array().unwrap())
            .filter(|model| model["independently_stationary"] == true)
            .map(|model| model["pair"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(passed, vec!["SOLUSDT-DOGEUSDT"]);
    }

    #[test]
    fn bh_q05_matches_frozen_reference_vector() {
        assert_eq!(
            benjamini_hochberg(&[0.001, 0.01, 0.03, 0.2, 0.9], 0.05),
            vec![true, true, true, false, false]
        );
    }

    #[test]
    fn future_shift_changes_only_snapshots_after_fit_cutoff() {
        let base = vec![(1, 10.0), (2, 11.0), (3, 12.0), (4, 13.0)];
        let shifted = vec![(1, 10.0), (2, 11.0), (3, 1_200.0), (4, 1_300.0)];
        assert_eq!(
            observations_through_cutoff(&base, 3),
            observations_through_cutoff(&shifted, 3)
        );
        assert_ne!(
            observations_through_cutoff(&base, 5),
            observations_through_cutoff(&shifted, 5)
        );
    }

    #[test]
    fn permuting_time_changes_copula_but_not_marginals() {
        let aligned = vec![(1.0, 1.0), (2.0, 2.0), (3.0, 3.0), (4.0, 4.0)];
        let permuted = vec![(1.0, 4.0), (2.0, 2.0), (3.0, 1.0), (4.0, 3.0)];
        let mut first = aligned.iter().map(|row| row.1).collect::<Vec<_>>();
        let mut second = permuted.iter().map(|row| row.1).collect::<Vec<_>>();
        first.sort_by(f64::total_cmp);
        second.sort_by(f64::total_cmp);
        assert_eq!(first, second);
        assert_ne!(kendall_tau_b(&aligned), kendall_tau_b(&permuted));
    }

    #[test]
    fn independent_reference_error_is_computed_not_constant() {
        let error = independent_reference_error(&[0.1, 0.8], &[0.1, 0.75]).unwrap();
        assert!((error - 0.05).abs() < 1e-12);
        assert_ne!(error, 0.0);
    }

    #[test]
    fn greedy_counterexample_returns_exact_two_edge_weight_18_not_one_edge_weight_10() {
        let edges = vec![
            PairEdge {
                left: "A".into(),
                right: "B".into(),
                score: 10.0,
            },
            PairEdge {
                left: "A".into(),
                right: "C".into(),
                score: 9.0,
            },
            PairEdge {
                left: "B".into(),
                right: "D".into(),
                score: 9.0,
            },
        ];
        let selected = exact_disjoint_matching(edges, 3);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected.iter().map(|edge| edge.score).sum::<f64>(), 18.0);
    }

    #[test]
    fn sigma_without_positive_edge_over_cost_is_rejected() {
        let rows = (0..20)
            .map(|_| ExcursionEdge {
                gross_edge_quote: 0.1,
                censored: false,
            })
            .collect::<Vec<_>>();
        assert!(!evaluate_cost_gate(&rows, 1.0, 0.01).passed);
    }

    #[test]
    fn censored_non_reversion_is_included_in_cost_gate() {
        let mut rows = (0..10)
            .map(|_| ExcursionEdge {
                gross_edge_quote: 5.0,
                censored: false,
            })
            .collect::<Vec<_>>();
        rows.extend((0..10).map(|_| ExcursionEdge {
            gross_edge_quote: 5.0,
            censored: true,
        }));
        let decision = evaluate_cost_gate(&rows, 1.0, 0.01);
        assert_eq!(decision.excursion_count, 20);
        assert_eq!(decision.censored_count, 10);
        assert!(!decision.passed);
    }

    #[test]
    fn weekly_top20_uses_formation_volume_only() {
        let rows = vec![
            LiquidityObservation {
                symbol: "A".into(),
                zero_volume_ratio: 0.0,
                p10_minute_quote_volume: 20_000.0,
                median_daily_quote_volume: 2.0,
                fit_cutoff_ms: 99,
            },
            LiquidityObservation {
                symbol: "B".into(),
                zero_volume_ratio: 0.0,
                p10_minute_quote_volume: 20_000.0,
                median_daily_quote_volume: 1.0,
                fit_cutoff_ms: 99,
            },
            LiquidityObservation {
                symbol: "FUTURE".into(),
                zero_volume_ratio: 0.0,
                p10_minute_quote_volume: 1_000_000.0,
                median_daily_quote_volume: 1_000.0,
                fit_cutoff_ms: 101,
            },
        ];
        assert_eq!(
            weekly_top20(&rows, 100)
                .iter()
                .map(|row| row.symbol.as_str())
                .collect::<Vec<_>>(),
            vec!["A", "B"]
        );
    }

    #[test]
    fn active_cycle_keeps_frozen_model_across_weekly_roll() {
        let old = FrozenModel {
            snapshot_sha256: "old".into(),
            beta_left: 1.0,
            beta_right: 2.0,
            family: "gaussian".into(),
            rho: 0.2,
            nu: None,
        };
        let new = FrozenModel {
            snapshot_sha256: "new".into(),
            beta_left: 3.0,
            beta_right: 4.0,
            family: "student_t".into(),
            rho: 0.7,
            nu: Some(4),
        };
        let mut cycle = ActiveCycle {
            group_id: "g".into(),
            model: old.clone(),
        };
        cycle.roll_selector(&new);
        assert_eq!(cycle.model, old);
    }

    #[test]
    fn model_snapshot_hash_mismatch_fails_closed() {
        let path = std::env::temp_dir().join(format!(
            "r26-snapshot-hash-test-{}.json",
            std::process::id()
        ));
        fs::write(&path, b"snapshot").unwrap();
        let error = read_hashed_snapshot(&path, &sha256_bytes(b"different")).unwrap_err();
        fs::remove_file(path).unwrap();
        assert!(error.to_string().contains("model_snapshot_hash_mismatch"));
    }
}
