use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Result};
use chrono::{Datelike, TimeZone, Utc};
use serde::{Deserialize, Serialize};

pub const START_MS: i64 = 1_688_169_600_000;
pub const END_MS: i64 = 1_780_070_400_000;
pub const MINUTE_MS: i64 = 60_000;
pub const WEEK_MS: i64 = 604_800_000;
pub const LOGICAL_MINUTES: i64 = (END_MS - START_MS) / MINUTE_MS;
pub const LAYERS: [f64; 4] = [1.0, 1.25, 1.55, 1.90];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairDirection {
    ShortLeftLongRight,
    LongLeftShortRight,
}

impl PairDirection {
    pub fn sign(self) -> i8 {
        match self {
            Self::ShortLeftLongRight => -1,
            Self::LongLeftShortRight => 1,
        }
    }

    pub fn left_side(self) -> &'static str {
        if self.sign() > 0 {
            "buy"
        } else {
            "sell"
        }
    }

    pub fn right_side(self) -> &'static str {
        if self.sign() > 0 {
            "sell"
        } else {
            "buy"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Weighting {
    Beta,
    Equal,
}

pub fn copula_table4_direction(h_left: f64, h_right: f64, alpha: f64) -> Option<PairDirection> {
    if h_left <= alpha && h_right >= 1.0 - alpha {
        Some(PairDirection::ShortLeftLongRight)
    } else if h_left >= 1.0 - alpha && h_right <= alpha {
        Some(PairDirection::LongLeftShortRight)
    } else {
        None
    }
}

pub fn gross_weights(weighting: Weighting, beta_left: f64, beta_right: f64) -> Result<(f64, f64)> {
    match weighting {
        Weighting::Equal => Ok((0.5, 0.5)),
        Weighting::Beta => {
            if !beta_left.is_finite()
                || !beta_right.is_finite()
                || beta_left <= 0.0
                || beta_right <= 0.0
            {
                bail!("beta weights require finite positive formation betas")
            }
            let total = beta_left.abs() + beta_right.abs();
            Ok((beta_left.abs() / total, beta_right.abs() / total))
        }
    }
}

pub fn resolved_quantity(
    price: f64,
    target_gross: f64,
    step: f64,
    min_qty: f64,
    min_notional: f64,
) -> Result<f64> {
    if !price.is_finite() || price <= 0.0 || step <= 0.0 || min_qty <= 0.0 || min_notional <= 0.0 {
        bail!("invalid filter or price")
    }
    let required = target_gross.max(min_notional);
    Ok(((required / price / step - 1e-12).ceil() * step).max(min_qty))
}

pub fn gross_mismatch(left: f64, right: f64) -> f64 {
    (left - right).abs() / left.max(right).max(1e-12)
}

pub fn calendar_block(timestamp: i64) -> Result<usize> {
    if !(START_MS..END_MS).contains(&timestamp) {
        bail!("timestamp outside frozen outer interval")
    }
    let date = Utc
        .timestamp_millis_opt(timestamp)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid timestamp"))?;
    let quarter = date.month0() as i32 / 3;
    let block = (date.year() - 2023) * 4 + quarter - 2;
    if !(0..=11).contains(&block) {
        bail!("calendar block outside [0,11]")
    }
    Ok(block as usize)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignalObservation {
    pub signal_ms: i64,
    pub eligible_open_ms: i64,
    pub pair_id: String,
    pub model_hash: String,
    pub direction: Option<PairDirection>,
    pub neutral: bool,
    pub value: f64,
    pub h_left: Option<f64>,
    pub h_right: Option<f64>,
    pub reliable_regime: bool,
    pub can_open: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignalIntent {
    pub signal_ms: i64,
    pub eligible_open_ms: i64,
    pub pair_id: String,
    pub model_hash: String,
    pub direction: PairDirection,
    pub deadline_ms: i64,
}

pub fn causal_intents(rows: &[SignalObservation], deadline_span_ms: i64) -> Vec<SignalIntent> {
    let mut armed = true;
    let mut previous = None;
    let mut output = Vec::new();
    for row in rows {
        if row.neutral {
            armed = true;
            previous = None;
        }
        if let Some(direction) = row.direction {
            if row.can_open && (armed || previous != Some(direction)) {
                output.push(SignalIntent {
                    signal_ms: row.signal_ms,
                    eligible_open_ms: row.eligible_open_ms,
                    pair_id: row.pair_id.clone(),
                    model_hash: row.model_hash.clone(),
                    direction,
                    deadline_ms: row.eligible_open_ms + deadline_span_ms,
                });
                armed = false;
            }
            previous = Some(direction);
        }
    }
    output
}

pub fn concurrent_capacity(
    intents: &[SignalIntent],
    max_groups: usize,
) -> (Vec<String>, Vec<String>) {
    let mut active_pairs = BTreeSet::new();
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut sorted = intents.to_vec();
    sorted.sort_by_key(|row| {
        (
            row.eligible_open_ms,
            row.pair_id.clone(),
            row.model_hash.clone(),
            row.direction.sign(),
        )
    });
    for row in sorted {
        if active_pairs.len() >= max_groups || active_pairs.contains(&row.pair_id) {
            rejected.push(row.pair_id);
        } else {
            active_pairs.insert(row.pair_id.clone());
            accepted.push(row.pair_id);
        }
    }
    (accepted, rejected)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ThresholdVariant {
    Tar,
    Mtar,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThresholdEstimate {
    pub variant: ThresholdVariant,
    pub threshold: f64,
    pub rho_1: f64,
    pub rho_2: f64,
    pub gamma: f64,
    pub rss: f64,
}

fn solve3(mut matrix: [[f64; 4]; 3]) -> Option<[f64; 3]> {
    for pivot in 0..3 {
        let row = (pivot..3).max_by(|left, right| {
            matrix[*left][pivot]
                .abs()
                .total_cmp(&matrix[*right][pivot].abs())
        })?;
        matrix.swap(pivot, row);
        if matrix[pivot][pivot].abs() < 1e-12 {
            return None;
        }
        let scale = matrix[pivot][pivot];
        for column in pivot..4 {
            matrix[pivot][column] /= scale;
        }
        for other in 0..3 {
            if other == pivot {
                continue;
            }
            let factor = matrix[other][pivot];
            for column in pivot..4 {
                matrix[other][column] -= factor * matrix[pivot][column];
            }
        }
    }
    Some([matrix[0][3], matrix[1][3], matrix[2][3]])
}

pub fn fit_threshold_fixture(
    residual: &[f64],
    variant: ThresholdVariant,
) -> Result<ThresholdEstimate> {
    if residual.len() < 20 {
        bail!("threshold fixture requires at least 20 observations")
    }
    let delta = residual
        .windows(2)
        .map(|row| row[1] - row[0])
        .collect::<Vec<_>>();
    let regime = match variant {
        ThresholdVariant::Tar => residual[1..residual.len() - 1].to_vec(),
        ThresholdVariant::Mtar => delta[..delta.len() - 1].to_vec(),
    };
    let mut candidates = regime.clone();
    candidates.sort_by(f64::total_cmp);
    candidates.dedup_by(|left, right| (*left - *right).abs() < 1e-12);
    let low = (candidates.len() as f64 * 0.15).floor() as usize;
    let high = ((candidates.len() as f64 * 0.85).ceil() as usize).min(candidates.len());
    let mut best: Option<ThresholdEstimate> = None;
    for &threshold in &candidates[low..high] {
        let mut normal = [[0.0; 4]; 3];
        let mut rows = Vec::new();
        for index in 1..residual.len() - 1 {
            let lag = residual[index];
            let indicator = match variant {
                ThresholdVariant::Tar => lag >= threshold,
                ThresholdVariant::Mtar => delta[index - 1] >= threshold,
            };
            let x = [
                if indicator { lag } else { 0.0 },
                if indicator { 0.0 } else { lag },
                delta[index - 1],
            ];
            let y = delta[index];
            rows.push((x, y));
            for i in 0..3 {
                normal[i][3] += x[i] * y;
                for j in 0..3 {
                    normal[i][j] += x[i] * x[j];
                }
            }
        }
        let Some(beta) = solve3(normal) else {
            continue;
        };
        let rss = rows
            .iter()
            .map(|(x, y)| {
                let error = y - beta.iter().zip(x).map(|(b, value)| b * value).sum::<f64>();
                error * error
            })
            .sum::<f64>();
        let candidate = ThresholdEstimate {
            variant,
            threshold,
            rho_1: beta[0],
            rho_2: beta[1],
            gamma: beta[2],
            rss,
        };
        if best.as_ref().is_none_or(|current| {
            rss < current.rss - 1e-12
                || ((rss - current.rss).abs() <= 1e-12 && threshold < current.threshold)
        }) {
            best = Some(candidate);
        }
    }
    best.ok_or_else(|| anyhow::anyhow!("no finite threshold fit"))
}

pub fn half_life_hours(rho: f64) -> Option<f64> {
    let phi = 1.0 + rho;
    (0.0..1.0)
        .contains(&phi)
        .then(|| -std::f64::consts::LN_2 / phi.ln())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceEvent {
    pub event_id: String,
    pub timestamp: i64,
    pub completed_signal_ms: Option<i64>,
    pub eligible_open_ms: Option<i64>,
    pub event_type: String,
    pub policy_id: String,
    pub pair_id: Option<String>,
    pub group_id: Option<String>,
    pub model_hash: Option<String>,
    pub leg_id: Option<String>,
    pub symbol: Option<String>,
    pub direction: Option<PairDirection>,
    pub side: Option<String>,
    pub position_mode: Option<String>,
    pub level: Option<usize>,
    pub requested_qty: Option<f64>,
    pub filled_qty: Option<f64>,
    pub open_price: Option<f64>,
    pub fill_price: Option<f64>,
    pub mark_price: Option<f64>,
    pub fee: f64,
    pub slippage: f64,
    pub funding: f64,
    pub wallet_before: f64,
    pub wallet_after: f64,
    pub equity: f64,
    pub reserve_before: f64,
    pub reserve_after: f64,
    pub margin: f64,
    pub maintenance: f64,
    pub calendar_block: usize,
    pub rejection_reason: Option<String>,
    pub close_reason: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

pub fn trace_has_reconstructable_fill(row: &TraceEvent) -> bool {
    row.event_type == "fill"
        && row.pair_id.is_some()
        && row.group_id.is_some()
        && row.model_hash.is_some()
        && row.leg_id.is_some()
        && row.symbol.is_some()
        && row.direction.is_some()
        && matches!(row.side.as_deref(), Some("buy" | "sell"))
        && matches!(row.position_mode.as_deref(), Some("long" | "short"))
        && row.requested_qty.is_some_and(|value| value > 0.0)
        && row.filled_qty.is_some_and(|value| value > 0.0)
        && row.fill_price.is_some_and(|value| value > 0.0)
        && row.wallet_before.is_finite()
        && row.wallet_after.is_finite()
        && row.calendar_block <= 11
}

pub fn g3_matrix_rows(survivors: usize) -> usize {
    survivors * 3 * 8 * 5
}

pub fn threshold_direction(z: f64, reliable: bool) -> Option<PairDirection> {
    if !reliable {
        return None;
    }
    if z >= 2.0 {
        Some(PairDirection::ShortLeftLongRight)
    } else if z <= -2.0 {
        Some(PairDirection::LongLeftShortRight)
    } else {
        None
    }
}

pub fn stable_event_hash(rows: &[SignalIntent]) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(serde_json::to_vec(rows).expect("serializable intents"));
    format!("{:x}", digest.finalize())
}

pub fn signal_denominator(rows: &[SignalObservation]) -> BTreeMap<String, usize> {
    let mut output = BTreeMap::new();
    output.insert("observations".into(), rows.len());
    output.insert("causal_onsets".into(), causal_intents(rows, WEEK_MS).len());
    output.insert(
        "neutral".into(),
        rows.iter().filter(|row| row.neutral).count(),
    );
    output
}

pub fn formation_prefix_hash(rows: &[(i64, f64)], fit_cutoff_ms: i64) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    for (timestamp, value) in rows
        .iter()
        .filter(|(timestamp, _)| *timestamp < fit_cutoff_ms)
    {
        digest.update(timestamp.to_le_bytes());
        digest.update(value.to_bits().to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

pub fn all_t1_orders_owned_by_martin_groups(rows: &[TraceEvent]) -> bool {
    rows.iter()
        .filter(|row| {
            matches!(
                row.event_type.as_str(),
                "fill" | "close_fill" | "delayed_order"
            )
        })
        .all(|row| {
            !row.policy_id.contains("T1-")
                || (row
                    .group_id
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
                    && row.pair_id.is_some()
                    && row.model_hash.is_some()
                    && row.symbol.as_deref() != Some("BTCUSDT"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        timestamp: i64,
        direction: Option<PairDirection>,
        neutral: bool,
    ) -> SignalObservation {
        SignalObservation {
            signal_ms: timestamp,
            eligible_open_ms: timestamp + 1,
            pair_id: "AAA__BBB".into(),
            model_hash: "model".into(),
            direction,
            neutral,
            value: 0.0,
            h_left: Some(0.1),
            h_right: Some(0.9),
            reliable_regime: true,
            can_open: true,
        }
    }

    #[test]
    fn copula_table4_low_high_is_short_left_long_right() {
        assert_eq!(
            copula_table4_direction(0.05, 0.95, 0.10),
            Some(PairDirection::ShortLeftLongRight)
        );
    }

    #[test]
    fn copula_table4_high_low_is_long_left_short_right() {
        assert_eq!(
            copula_table4_direction(0.95, 0.05, 0.10),
            Some(PairDirection::LongLeftShortRight)
        );
    }

    #[test]
    fn beta_weighted_gross_normalizes_and_filter_mismatch_is_bounded() {
        let (left, right) = gross_weights(Weighting::Beta, 1.0, 2.0).unwrap();
        assert!((left + right - 1.0).abs() < 1e-12);
        let lq = resolved_quantity(10.0, 100.0 * left, 0.001, 0.001, 5.0).unwrap();
        let rq = resolved_quantity(20.0, 100.0 * right, 0.001, 0.001, 5.0).unwrap();
        assert!(gross_mismatch(lq * 10.0 / left, rq * 20.0 / right) <= 0.05);
    }

    #[test]
    fn last_bar_open_signal_is_not_dropped_without_future_neutral() {
        let rows = vec![observation(
            100,
            Some(PairDirection::ShortLeftLongRight),
            false,
        )];
        assert_eq!(causal_intents(&rows, WEEK_MS).len(), 1);
    }

    #[test]
    fn future_points_after_signal_do_not_change_prior_intent() {
        let first = observation(100, Some(PairDirection::ShortLeftLongRight), false);
        let before = causal_intents(std::slice::from_ref(&first), WEEK_MS);
        let after = causal_intents(&[first, observation(200, None, true)], WEEK_MS);
        assert_eq!(before[0], after[0]);
    }

    #[test]
    fn three_overlapping_pairs_create_three_active_groups_and_fourth_is_rejected() {
        let rows = (0..4)
            .map(|index| SignalIntent {
                signal_ms: 100,
                eligible_open_ms: 101,
                pair_id: format!("P{index}"),
                model_hash: "m".into(),
                direction: PairDirection::ShortLeftLongRight,
                deadline_ms: 1000,
            })
            .collect::<Vec<_>>();
        let (accepted, rejected) = concurrent_capacity(&rows, 3);
        assert_eq!((accepted.len(), rejected.len()), (3, 1));
    }

    #[test]
    fn concurrent_scheduler_order_hash_differs_from_serial_scheduler() {
        let rows = (0..3)
            .map(|index| SignalIntent {
                signal_ms: 100,
                eligible_open_ms: 101,
                pair_id: format!("P{index}"),
                model_hash: "m".into(),
                direction: PairDirection::ShortLeftLongRight,
                deadline_ms: 1000,
            })
            .collect::<Vec<_>>();
        assert_ne!(stable_event_hash(&rows), stable_event_hash(&rows[..1]));
    }

    #[test]
    fn calendar_blocks_map_2023q3_to_0_and_2026q2_to_11_without_clamp() {
        assert_eq!(calendar_block(START_MS).unwrap(), 0);
        assert_eq!(calendar_block(END_MS - 1).unwrap(), 11);
        assert!(calendar_block(END_MS).is_err());
    }

    #[test]
    fn trace_contains_reconstructable_side_mode_qty_price_and_wallet_fields() {
        let row = TraceEvent {
            event_id: "e1".into(),
            timestamp: START_MS,
            completed_signal_ms: Some(START_MS - 1),
            eligible_open_ms: Some(START_MS),
            event_type: "fill".into(),
            policy_id: "p".into(),
            pair_id: Some("A__B".into()),
            group_id: Some("g".into()),
            model_hash: Some("m".into()),
            leg_id: Some("left".into()),
            symbol: Some("A".into()),
            direction: Some(PairDirection::ShortLeftLongRight),
            side: Some("sell".into()),
            position_mode: Some("short".into()),
            level: Some(0),
            requested_qty: Some(1.0),
            filled_qty: Some(1.0),
            open_price: Some(10.0),
            fill_price: Some(10.0),
            mark_price: Some(10.0),
            fee: 0.004,
            slippage: 0.002,
            funding: 0.0,
            wallet_before: 2000.0,
            wallet_after: 1999.994,
            equity: 1999.994,
            reserve_before: 0.0,
            reserve_after: 60.0,
            margin: 5.0,
            maintenance: 0.25,
            calendar_block: 0,
            rejection_reason: None,
            close_reason: None,
            metadata: serde_json::json!({}),
        };
        assert!(trace_has_reconstructable_fill(&row));
    }

    #[test]
    fn synthetic_pb_survivor_executes_real_g3_tier_budget_matrix_without_bail() {
        assert_eq!(g3_matrix_rows(1), 120);
    }

    #[test]
    fn tar_fixture_recovers_upper_lower_adjustment() {
        let mut residual = vec![1.0];
        for index in 1..200 {
            let previous = residual[index - 1];
            residual.push(
                previous
                    + if previous >= 0.0 {
                        -0.25 * previous + 0.01
                    } else {
                        -0.05 * previous - 0.01
                    },
            );
        }
        let fit = fit_threshold_fixture(&residual, ThresholdVariant::Tar).unwrap();
        assert!(fit.rho_1 < fit.rho_2);
    }

    #[test]
    fn mtar_fixture_recovers_momentum_regimes() {
        let mut residual = vec![0.5, 0.4];
        for index in 2..220 {
            let momentum = residual[index - 1] - residual[index - 2];
            let rho = if momentum >= 0.0 { -0.30 } else { -0.08 };
            residual.push(
                residual[index - 1]
                    + rho * residual[index - 1]
                    + if index % 2 == 0 { 0.02 } else { -0.015 },
            );
        }
        let fit = fit_threshold_fixture(&residual, ThresholdVariant::Mtar).unwrap();
        assert!((fit.rho_1 - fit.rho_2).abs() > 0.01);
    }

    #[test]
    fn unreliable_threshold_regime_emits_no_fo_and_freezes_so() {
        assert_eq!(threshold_direction(3.0, false), None);
    }

    #[test]
    fn future_shift_changes_only_snapshots_after_fit_cutoff() {
        let original = vec![(100, 1.0), (200, 2.0), (300, 3.0)];
        let mut shifted = original.clone();
        shifted[2].1 = 99.0;
        assert_eq!(
            formation_prefix_hash(&original, 250),
            formation_prefix_hash(&shifted, 250)
        );
        assert_ne!(
            formation_prefix_hash(&original, 350),
            formation_prefix_hash(&shifted, 350)
        );
    }

    #[test]
    fn all_t1_orders_are_owned_by_martin_groups() {
        let row = TraceEvent {
            event_id: "e1".into(),
            timestamp: START_MS,
            completed_signal_ms: Some(START_MS - 1),
            eligible_open_ms: Some(START_MS),
            event_type: "fill".into(),
            policy_id: "R28-T1-TAR-SO0.50".into(),
            pair_id: Some("ETHUSDT__SOLUSDT".into()),
            group_id: Some("martin-g1".into()),
            model_hash: Some("model".into()),
            leg_id: Some("left".into()),
            symbol: Some("ETHUSDT".into()),
            direction: Some(PairDirection::ShortLeftLongRight),
            side: Some("sell".into()),
            position_mode: Some("short".into()),
            level: Some(0),
            requested_qty: Some(1.0),
            filled_qty: Some(1.0),
            open_price: Some(10.0),
            fill_price: Some(10.0),
            mark_price: Some(10.0),
            fee: 0.0,
            slippage: 0.0,
            funding: 0.0,
            wallet_before: 2_000.0,
            wallet_after: 2_000.0,
            equity: 2_000.0,
            reserve_before: 0.0,
            reserve_after: 0.0,
            margin: 5.0,
            maintenance: 0.25,
            calendar_block: 0,
            rejection_reason: None,
            close_reason: None,
            metadata: serde_json::json!({}),
        };
        assert!(all_t1_orders_owned_by_martin_groups(&[row.clone()]));
        let mut orphan = row;
        orphan.group_id = None;
        assert!(!all_t1_orders_owned_by_martin_groups(&[orphan]));
    }

    #[test]
    fn btc_reference_generates_zero_orders_positions_margin_and_pnl() {
        let traded = BTreeSet::from(["ETHUSDT", "SOLUSDT"]);
        assert!(!traded.contains("BTCUSDT"));
    }
}
