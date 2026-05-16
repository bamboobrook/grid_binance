use rust_decimal::prelude::ToPrimitive;
use shared_domain::martingale::{MartingaleDirection, MartingaleTakeProfitModel};

use crate::martingale::state::MartingaleLegState;

const BPS_DENOMINATOR: f64 = 10_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitDecision {
    None,
    TakeProfit,
    StrategyStop,
    SymbolStop,
    GlobalStop,
}

pub fn weighted_average_entry(legs: &[MartingaleLegState]) -> Result<f64, String> {
    let mut weighted_price = 0.0;
    let mut total_quantity = 0.0;

    for leg in legs {
        validate_positive_f64("leg.price", leg.price)?;
        validate_positive_f64("leg.quantity", leg.quantity)?;
        weighted_price += leg.price * leg.quantity;
        total_quantity += leg.quantity;
    }

    validate_positive_f64("total_quantity", total_quantity)?;
    validate_positive_price(weighted_price / total_quantity)
}

pub fn take_profit_price(
    average_entry: f64,
    direction: MartingaleDirection,
    model: &MartingaleTakeProfitModel,
    latest_atr: Option<f64>,
) -> Result<f64, String> {
    validate_positive_f64("average_entry", average_entry)?;

    let price = match model {
        MartingaleTakeProfitModel::Percent { bps } => {
            let offset = average_entry * *bps as f64 / BPS_DENOMINATOR;
            match direction {
                MartingaleDirection::Long => average_entry + offset,
                MartingaleDirection::Short => average_entry - offset,
            }
        }
        MartingaleTakeProfitModel::Amount { .. } => {
            return Err(
                "amount take profit cannot be computed by take_profit_price without total quantity context"
                    .to_string(),
            );
        }
        MartingaleTakeProfitModel::Atr { multiplier } => {
            let atr = latest_atr
                .ok_or_else(|| "latest_atr is required for ATR take profit".to_string())?;
            validate_positive_f64("latest_atr", atr)?;
            let multiplier = multiplier
                .to_f64()
                .ok_or_else(|| "multiplier cannot be represented as f64".to_string())?;
            validate_positive_f64("multiplier", multiplier)?;
            match direction {
                MartingaleDirection::Long => average_entry + atr * multiplier,
                MartingaleDirection::Short => average_entry - atr * multiplier,
            }
        }
        MartingaleTakeProfitModel::Trailing {
            activation_bps,
            callback_bps,
        } => {
            if *callback_bps == 0 {
                return Err("callback_bps must be > 0 for trailing take profit".to_string());
            }
            let offset = average_entry * *activation_bps as f64 / BPS_DENOMINATOR;
            match direction {
                MartingaleDirection::Long => average_entry + offset,
                MartingaleDirection::Short => average_entry - offset,
            }
        }
        MartingaleTakeProfitModel::Mixed { phases } => {
            let mut last_error = None;
            for phase in phases {
                match take_profit_price(average_entry, direction, phase, latest_atr) {
                    Ok(price) => return Ok(price),
                    Err(error) => last_error = Some(error),
                }
            }
            return Err(last_error.unwrap_or_else(|| {
                "mixed take profit requires at least one computable phase".to_string()
            }));
        }
    };

    validate_positive_price(price)
}

pub fn evaluate_exit_priority(
    global_stop: bool,
    symbol_stop: bool,
    strategy_stop: bool,
    take_profit: bool,
) -> ExitDecision {
    if global_stop {
        ExitDecision::GlobalStop
    } else if symbol_stop {
        ExitDecision::SymbolStop
    } else if strategy_stop {
        ExitDecision::StrategyStop
    } else if take_profit {
        ExitDecision::TakeProfit
    } else {
        ExitDecision::None
    }
}

fn validate_positive_f64(name: &str, value: f64) -> Result<(), String> {
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("{name} must be finite and > 0"));
    }
    Ok(())
}

fn validate_positive_price(price: f64) -> Result<f64, String> {
    validate_positive_f64("price", price)?;
    Ok(price)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn open_leg(price: f64, quantity: f64) -> MartingaleLegState {
        MartingaleLegState {
            leg_index: 0,
            price,
            quantity,
            notional_quote: price * quantity,
            fee_quote: 0.0,
            slippage_quote: 0.0,
        }
    }

    #[test]
    fn weighted_entry_for_long_cycle_uses_quantity_weighting() {
        let legs = vec![open_leg(100.0, 0.1), open_leg(90.0, 0.2)];
        let avg = weighted_average_entry(&legs).unwrap();
        assert!((avg - 93.3333333333).abs() < 0.0001);
    }

    #[test]
    fn weighted_entry_rejects_empty_legs() {
        assert!(weighted_average_entry(&[]).is_err());
    }

    #[test]
    fn weighted_entry_rejects_non_positive_or_non_finite_price() {
        for price in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(weighted_average_entry(&[open_leg(price, 1.0)]).is_err());
        }
    }

    #[test]
    fn weighted_entry_rejects_non_positive_or_non_finite_quantity() {
        for quantity in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(weighted_average_entry(&[open_leg(100.0, quantity)]).is_err());
        }
    }

    #[test]
    fn weighted_entry_rejects_product_or_accumulator_overflow() {
        assert!(weighted_average_entry(&[open_leg(f64::MAX, 2.0)]).is_err());

        let legs = vec![open_leg(f64::MAX, 1.0), open_leg(f64::MAX, 1.0)];
        assert!(weighted_average_entry(&legs).is_err());
    }

    #[test]
    fn percent_take_profit_for_short_is_below_average_entry() {
        let trigger = take_profit_price(
            100.0,
            MartingaleDirection::Short,
            &MartingaleTakeProfitModel::Percent { bps: 100 },
            None,
        )
        .unwrap();
        assert_eq!(trigger, 99.0);
    }

    #[test]
    fn amount_take_profit_returns_clear_error() {
        let error = take_profit_price(
            100.0,
            MartingaleDirection::Long,
            &MartingaleTakeProfitModel::Amount {
                quote: Decimal::ONE,
            },
            None,
        )
        .unwrap_err();

        assert!(error.contains("cannot be computed by take_profit_price"));
    }

    #[test]
    fn atr_take_profit_requires_latest_atr() {
        assert!(take_profit_price(
            100.0,
            MartingaleDirection::Long,
            &MartingaleTakeProfitModel::Atr {
                multiplier: Decimal::ONE,
            },
            None,
        )
        .is_err());
    }

    #[test]
    fn atr_take_profit_rejects_non_positive_multiplier() {
        for multiplier in [Decimal::ZERO, Decimal::NEGATIVE_ONE] {
            assert!(take_profit_price(
                100.0,
                MartingaleDirection::Long,
                &MartingaleTakeProfitModel::Atr { multiplier },
                Some(5.0),
            )
            .is_err());
        }
    }

    #[test]
    fn trailing_take_profit_uses_activation_price_for_long_and_short() {
        let model = MartingaleTakeProfitModel::Trailing {
            activation_bps: 250,
            callback_bps: 50,
        };

        assert_eq!(
            take_profit_price(100.0, MartingaleDirection::Long, &model, None).unwrap(),
            102.5
        );
        assert_eq!(
            take_profit_price(100.0, MartingaleDirection::Short, &model, None).unwrap(),
            97.5
        );
    }

    #[test]
    fn trailing_take_profit_rejects_zero_callback_bps() {
        let error = take_profit_price(
            100.0,
            MartingaleDirection::Long,
            &MartingaleTakeProfitModel::Trailing {
                activation_bps: 250,
                callback_bps: 0,
            },
            None,
        )
        .unwrap_err();

        assert!(error.contains("callback_bps"));
    }

    #[test]
    fn mixed_take_profit_rejects_empty_phases() {
        assert!(take_profit_price(
            100.0,
            MartingaleDirection::Long,
            &MartingaleTakeProfitModel::Mixed { phases: vec![] },
            None,
        )
        .is_err());
    }

    #[test]
    fn mixed_take_profit_uses_first_computable_phase() {
        let trigger = take_profit_price(
            100.0,
            MartingaleDirection::Long,
            &MartingaleTakeProfitModel::Mixed {
                phases: vec![
                    MartingaleTakeProfitModel::Amount {
                        quote: Decimal::ONE,
                    },
                    MartingaleTakeProfitModel::Percent { bps: 100 },
                ],
            },
            None,
        )
        .unwrap();

        assert_eq!(trigger, 101.0);
    }

    #[test]
    fn mixed_take_profit_returns_error_when_all_phases_fail() {
        assert!(take_profit_price(
            100.0,
            MartingaleDirection::Long,
            &MartingaleTakeProfitModel::Mixed {
                phases: vec![
                    MartingaleTakeProfitModel::Amount {
                        quote: Decimal::ONE,
                    },
                    MartingaleTakeProfitModel::Atr {
                        multiplier: Decimal::ONE,
                    },
                ],
            },
            None,
        )
        .is_err());
    }

    #[test]
    fn short_take_profit_rejects_non_positive_output_price() {
        assert!(take_profit_price(
            100.0,
            MartingaleDirection::Short,
            &MartingaleTakeProfitModel::Percent { bps: 10_000 },
            None,
        )
        .is_err());
    }

    #[test]
    fn global_stop_has_priority_over_take_profit() {
        let decision = evaluate_exit_priority(true, true, true, true);
        assert_eq!(decision, ExitDecision::GlobalStop);
    }
}
