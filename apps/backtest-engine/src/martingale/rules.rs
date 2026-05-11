use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleSizingModel, MartingaleSpacingModel,
};

const BPS_DENOMINATOR: f64 = 10_000.0;

pub fn compute_leg_trigger_prices(
    anchor_price: f64,
    direction: MartingaleDirection,
    spacing: &MartingaleSpacingModel,
    latest_atr: Option<f64>,
    max_legs: u32,
) -> Result<Vec<f64>, String> {
    validate_positive_f64("anchor_price", anchor_price)?;

    let distances_bps = expand_spacing_bps(anchor_price, spacing, latest_atr, max_legs)?;
    distances_bps
        .into_iter()
        .map(|distance_bps| trigger_price(anchor_price, direction, distance_bps))
        .collect()
}

pub fn compute_leg_notionals(
    sizing: &MartingaleSizingModel,
    portfolio_budget_quote: f64,
    exchange_min_notional: f64,
) -> Result<Vec<f64>, String> {
    validate_non_negative_f64("portfolio_budget_quote", portfolio_budget_quote)?;
    validate_non_negative_f64("exchange_min_notional", exchange_min_notional)?;

    let (notionals, enforce_budget) = match sizing {
        MartingaleSizingModel::Multiplier {
            first_order_quote,
            multiplier,
            max_legs,
        } => (
            geometric_series(*first_order_quote, *multiplier, *max_legs)?,
            true,
        ),
        MartingaleSizingModel::CustomSequence { notionals } => (
            notionals
                .iter()
                .map(|notional| decimal_to_f64("notional", *notional))
                .collect::<Result<Vec<_>, _>>()?,
            true,
        ),
        MartingaleSizingModel::BudgetScaled {
            first_order_quote,
            multiplier,
            max_legs,
            max_budget_quote,
        } => {
            let mut values = geometric_series(*first_order_quote, *multiplier, *max_legs)?;
            let budget =
                decimal_to_f64("max_budget_quote", *max_budget_quote)?.min(portfolio_budget_quote);
            validate_non_negative_f64("budget", budget)?;

            let total = sum_values(&values)?;
            if total > budget && total > 0.0 {
                let scale = budget / total;
                for value in &mut values {
                    *value *= scale;
                }
            }
            (values, false)
        }
    };

    validate_min_notional(&notionals, exchange_min_notional)?;

    if enforce_budget {
        let total = sum_values(&notionals)?;
        if total > portfolio_budget_quote {
            return Err(format!(
                "total notional {total} exceeds portfolio budget {portfolio_budget_quote}"
            ));
        }
    }

    Ok(notionals)
}

fn expand_spacing_bps(
    anchor_price: f64,
    spacing: &MartingaleSpacingModel,
    latest_atr: Option<f64>,
    max_legs: u32,
) -> Result<Vec<f64>, String> {
    let max_legs = max_legs as usize;
    if max_legs == 0 {
        return Ok(Vec::new());
    }

    match spacing {
        MartingaleSpacingModel::FixedPercent { step_bps } => Ok((1..=max_legs)
            .map(|leg| *step_bps as f64 * leg as f64)
            .collect()),
        MartingaleSpacingModel::Multiplier {
            first_step_bps,
            multiplier,
        } => {
            let multiplier = decimal_to_f64("multiplier", *multiplier)?;
            validate_positive_f64("multiplier", multiplier)?;
            let mut step_bps = *first_step_bps as f64;
            let mut distances = Vec::with_capacity(max_legs);

            for leg_index in 0..max_legs {
                validate_non_negative_f64("distance_bps", step_bps)?;
                distances.push(step_bps);
                if leg_index + 1 < max_legs {
                    step_bps *= multiplier;
                    validate_finite("distance_bps", step_bps)?;
                }
            }

            Ok(distances)
        }
        MartingaleSpacingModel::Atr {
            multiplier,
            min_step_bps,
            max_step_bps,
        } => {
            if min_step_bps > max_step_bps {
                return Err(format!(
                    "min_step_bps {min_step_bps} must be <= max_step_bps {max_step_bps}"
                ));
            }
            let atr =
                latest_atr.ok_or_else(|| "latest_atr is required for ATR spacing".to_string())?;
            validate_positive_f64("latest_atr", atr)?;
            let multiplier = decimal_to_f64("multiplier", *multiplier)?;
            validate_positive_f64("multiplier", multiplier)?;

            let step_bps = (atr * multiplier / anchor_price * BPS_DENOMINATOR)
                .clamp(*min_step_bps as f64, *max_step_bps as f64);
            validate_non_negative_f64("distance_bps", step_bps)?;

            (1..=max_legs)
                .map(|leg| {
                    let distance_bps = step_bps * leg as f64;
                    validate_non_negative_f64("distance_bps", distance_bps)?;
                    Ok(distance_bps)
                })
                .collect()
        }
        MartingaleSpacingModel::CustomSequence { steps_bps } => Ok(steps_bps
            .iter()
            .take(max_legs)
            .map(|step_bps| *step_bps as f64)
            .collect()),
        MartingaleSpacingModel::Mixed { phases } => {
            let mut distances = Vec::with_capacity(max_legs);
            for (index, phase) in phases.iter().enumerate() {
                if distances.len() == max_legs {
                    break;
                }
                let is_last_phase = index + 1 == phases.len();
                let phase_max_legs =
                    phase_leg_limit(phase, max_legs - distances.len(), is_last_phase);
                distances.extend(expand_spacing_bps(
                    anchor_price,
                    phase,
                    latest_atr,
                    phase_max_legs,
                )?);
            }
            Ok(distances)
        }
    }
}

fn trigger_price(
    anchor_price: f64,
    direction: MartingaleDirection,
    distance_bps: f64,
) -> Result<f64, String> {
    validate_non_negative_f64("distance_bps", distance_bps)?;
    let offset = anchor_price * distance_bps / BPS_DENOMINATOR;
    validate_finite("offset", offset)?;

    let trigger_price = match direction {
        MartingaleDirection::Long => anchor_price - offset,
        MartingaleDirection::Short => anchor_price + offset,
    };

    if !trigger_price.is_finite() || trigger_price <= 0.0 {
        return Err(format!(
            "trigger price must be finite and positive, got {trigger_price}"
        ));
    }

    Ok(trigger_price)
}

// Mixed phases append in order. Infinite phases contribute one leg unless they
// are the final phase, where they consume all remaining legs.
fn phase_leg_limit(
    phase: &MartingaleSpacingModel,
    remaining_legs: usize,
    is_last_phase: bool,
) -> u32 {
    let phase_legs = match phase {
        MartingaleSpacingModel::CustomSequence { steps_bps } => steps_bps.len(),
        MartingaleSpacingModel::Mixed { phases } => phases
            .iter()
            .enumerate()
            .map(|(index, phase)| {
                phase_leg_limit(phase, remaining_legs, index + 1 == phases.len()) as usize
            })
            .sum(),
        MartingaleSpacingModel::FixedPercent { .. }
        | MartingaleSpacingModel::Multiplier { .. }
        | MartingaleSpacingModel::Atr { .. } => {
            if is_last_phase {
                remaining_legs
            } else {
                1
            }
        }
    };

    phase_legs.min(remaining_legs) as u32
}

fn geometric_series(
    first_order_quote: Decimal,
    multiplier: Decimal,
    max_legs: u32,
) -> Result<Vec<f64>, String> {
    let first_order_quote = decimal_to_f64("first_order_quote", first_order_quote)?;
    let multiplier = decimal_to_f64("multiplier", multiplier)?;
    validate_non_negative_f64("first_order_quote", first_order_quote)?;
    validate_positive_f64("multiplier", multiplier)?;

    let mut values = Vec::with_capacity(max_legs as usize);
    let mut current = first_order_quote;
    for _ in 0..max_legs {
        validate_finite("notional", current)?;
        values.push(current);
        current *= multiplier;
    }
    Ok(values)
}

fn decimal_to_f64(name: &str, value: Decimal) -> Result<f64, String> {
    value
        .to_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{name} cannot be represented as f64"))
}

fn validate_min_notional(notionals: &[f64], exchange_min_notional: f64) -> Result<(), String> {
    for notional in notionals {
        validate_finite("notional", *notional)?;
        if *notional < exchange_min_notional {
            return Err(format!(
                "leg notional {notional} is below exchange minimum notional {exchange_min_notional}"
            ));
        }
    }
    Ok(())
}

fn sum_values(values: &[f64]) -> Result<f64, String> {
    let total: f64 = values.iter().sum();
    validate_finite("total notional", total)?;
    Ok(total)
}

fn validate_positive_f64(name: &str, value: f64) -> Result<(), String> {
    validate_finite(name, value)?;
    if value <= 0.0 {
        return Err(format!("{name} must be positive"));
    }
    Ok(())
}

fn validate_non_negative_f64(name: &str, value: f64) -> Result<(), String> {
    validate_finite(name, value)?;
    if value < 0.0 {
        return Err(format!("{name} must be non-negative"));
    }
    Ok(())
}

fn validate_finite(name: &str, value: f64) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("{name} must be finite"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn long_fixed_percent_triggers_move_below_anchor() {
        let prices = compute_leg_trigger_prices(
            100.0,
            MartingaleDirection::Long,
            &MartingaleSpacingModel::FixedPercent { step_bps: 100 },
            None,
            3,
        )
        .unwrap();

        assert_eq!(prices, vec![99.0, 98.0, 97.0]);
    }

    #[test]
    fn short_fixed_percent_triggers_move_above_anchor() {
        let prices = compute_leg_trigger_prices(
            100.0,
            MartingaleDirection::Short,
            &MartingaleSpacingModel::FixedPercent { step_bps: 100 },
            None,
            3,
        )
        .unwrap();

        assert_eq!(prices, vec![101.0, 102.0, 103.0]);
    }

    #[test]
    fn multiplier_sizing_matches_martingale_example() {
        let notionals = compute_leg_notionals(
            &MartingaleSizingModel::Multiplier {
                first_order_quote: Decimal::from(10),
                multiplier: Decimal::from(2),
                max_legs: 4,
            },
            1_000.0,
            5.0,
        )
        .unwrap();

        assert_eq!(notionals, vec![10.0, 20.0, 40.0, 80.0]);
    }

    #[test]
    fn budget_scaled_rejects_when_scaled_leg_below_min_notional() {
        let error = compute_leg_notionals(
            &MartingaleSizingModel::BudgetScaled {
                first_order_quote: Decimal::from(10),
                multiplier: Decimal::from(2),
                max_legs: 4,
                max_budget_quote: Decimal::from(8),
            },
            8.0,
            5.0,
        )
        .expect_err("too small");

        assert!(error.contains("minimum notional"));
    }

    #[test]
    fn mixed_spacing_allows_later_phase_to_contribute() {
        let prices = compute_leg_trigger_prices(
            100.0,
            MartingaleDirection::Long,
            &MartingaleSpacingModel::Mixed {
                phases: vec![
                    MartingaleSpacingModel::FixedPercent { step_bps: 100 },
                    MartingaleSpacingModel::CustomSequence {
                        steps_bps: vec![150],
                    },
                ],
            },
            None,
            2,
        )
        .unwrap();

        assert_eq!(prices, vec![99.0, 98.5]);
    }

    #[test]
    fn mixed_spacing_last_infinite_phase_consumes_remaining_legs() {
        let prices = compute_leg_trigger_prices(
            100.0,
            MartingaleDirection::Long,
            &MartingaleSpacingModel::Mixed {
                phases: vec![
                    MartingaleSpacingModel::CustomSequence {
                        steps_bps: vec![100, 100],
                    },
                    MartingaleSpacingModel::Multiplier {
                        first_step_bps: 150,
                        multiplier: Decimal::from(2),
                    },
                ],
            },
            None,
            4,
        )
        .unwrap();

        assert_eq!(prices, vec![99.0, 99.0, 98.5, 97.0]);
    }

    #[test]
    fn long_trigger_prices_reject_zero_or_negative_prices() {
        let error = compute_leg_trigger_prices(
            100.0,
            MartingaleDirection::Long,
            &MartingaleSpacingModel::FixedPercent { step_bps: 10_000 },
            None,
            2,
        )
        .expect_err("invalid trigger price");

        assert!(error.contains("trigger price"));
    }

    #[test]
    fn multiplier_spacing_rejects_overflow() {
        let error = compute_leg_trigger_prices(
            100.0,
            MartingaleDirection::Short,
            &MartingaleSpacingModel::Multiplier {
                first_step_bps: 1,
                multiplier: Decimal::from(1_000_000_000_000_000_000_u64),
            },
            None,
            25,
        )
        .expect_err("overflowing spacing");

        assert!(error.contains("finite"));
    }

    #[test]
    fn multiplier_spacing_allows_next_unrequested_leg_to_overflow() {
        let prices = compute_leg_trigger_prices(
            100.0,
            MartingaleDirection::Short,
            &MartingaleSpacingModel::Multiplier {
                first_step_bps: 1,
                multiplier: Decimal::from(1_000_000_000_000_000_000_u64),
            },
            None,
            18,
        )
        .unwrap();

        assert_eq!(prices.len(), 18);
        assert!(prices.iter().all(|price| price.is_finite() && *price > 0.0));
    }

    #[test]
    fn mixed_last_multiplier_allows_next_unrequested_leg_to_overflow() {
        let prices = compute_leg_trigger_prices(
            100.0,
            MartingaleDirection::Short,
            &MartingaleSpacingModel::Mixed {
                phases: vec![
                    MartingaleSpacingModel::CustomSequence { steps_bps: vec![1] },
                    MartingaleSpacingModel::Multiplier {
                        first_step_bps: 1,
                        multiplier: Decimal::from(1_000_000_000_000_000_000_u64),
                    },
                ],
            },
            None,
            19,
        )
        .unwrap();

        assert_eq!(prices.len(), 19);
        assert!(prices.iter().all(|price| price.is_finite() && *price > 0.0));
    }

    #[test]
    fn atr_spacing_rejects_reversed_bounds_without_panicking() {
        let error = compute_leg_trigger_prices(
            100.0,
            MartingaleDirection::Long,
            &MartingaleSpacingModel::Atr {
                multiplier: Decimal::from(1),
                min_step_bps: 200,
                max_step_bps: 100,
            },
            Some(1.0),
            1,
        )
        .expect_err("reversed ATR bounds");

        assert!(error.contains("min_step_bps"));
        assert!(error.contains("max_step_bps"));
    }
}
