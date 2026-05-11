#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndicatorCandle {
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BollingerPoint {
    pub middle: f64,
    pub upper: f64,
    pub lower: f64,
    pub bandwidth: f64,
}

pub fn sma(values: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut result = vec![None; values.len()];
    if period == 0 || values.len() < period {
        return result;
    }

    for index in (period - 1)..values.len() {
        let window = &values[index + 1 - period..=index];
        if window.iter().all(|value| value.is_finite()) {
            result[index] = finite_option(window.iter().sum::<f64>() / period as f64);
        }
    }

    result
}

pub fn ema(values: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut result = vec![None; values.len()];
    if period == 0 || values.len() < period {
        return result;
    }

    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut current: Option<f64> = None;

    for index in 0..values.len() {
        let value = values[index];
        if !value.is_finite() {
            current = None;
            continue;
        }

        current = match current {
            Some(previous) => finite_option((value - previous) * multiplier + previous),
            None if index + 1 >= period => finite_average(&values[index + 1 - period..=index]),
            None => None,
        };
        result[index] = current;
    }

    result
}

pub fn atr(candles: &[IndicatorCandle], period: usize) -> Vec<Option<f64>> {
    let mut result = vec![None; candles.len()];
    if period == 0 || candles.len() < period {
        return result;
    }

    let mut current: Option<f64> = None;
    let mut warmup_ranges = Vec::with_capacity(period);
    let mut previous_candle: Option<IndicatorCandle> = None;

    for (index, candle) in candles.iter().copied().enumerate() {
        if !is_valid_candle(candle) {
            current = None;
            warmup_ranges.clear();
            previous_candle = None;
            continue;
        }

        let range = true_range(candle, previous_candle.and_then(finite_close));
        previous_candle = Some(candle);

        current = match (current, range) {
            (Some(previous_atr), Some(range)) => {
                finite_option(((previous_atr * (period as f64 - 1.0)) + range) / period as f64)
            }
            (None, Some(range)) => {
                warmup_ranges.push(range);
                if warmup_ranges.len() == period {
                    finite_average(&warmup_ranges)
                } else {
                    None
                }
            }
            _ => None,
        };
        result[index] = current;
    }

    result
}

pub fn rsi(closes: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut result = vec![None; closes.len()];
    if period == 0 || closes.len() <= period {
        return result;
    }

    let mut average_gain: Option<f64> = None;
    let mut average_loss: Option<f64> = None;

    for index in period..closes.len() {
        let delta = close_delta(closes[index - 1], closes[index]);
        if let Some(delta) = delta {
            let gain = delta.max(0.0);
            let loss = (-delta).max(0.0);
            match (average_gain, average_loss) {
                (Some(previous_gain), Some(previous_loss)) => {
                    average_gain = finite_option(
                        ((previous_gain * (period as f64 - 1.0)) + gain) / period as f64,
                    );
                    average_loss = finite_option(
                        ((previous_loss * (period as f64 - 1.0)) + loss) / period as f64,
                    );
                }
                _ => {
                    let seed = wilder_gain_loss_seed(&closes[index - period..=index]);
                    average_gain = seed.map(|(gain_seed, _)| gain_seed);
                    average_loss = seed.map(|(_, loss_seed)| loss_seed);
                }
            }
        } else {
            average_gain = None;
            average_loss = None;
        }

        result[index] = match (average_gain, average_loss) {
            (Some(gain), Some(loss)) if gain.is_finite() && loss.is_finite() => {
                finite_option(rsi_value(gain, loss))
            }
            _ => None,
        };
    }

    result
}

pub fn bollinger(closes: &[f64], period: usize, stddev: f64) -> Vec<Option<BollingerPoint>> {
    let mut result = vec![None; closes.len()];
    if period == 0 || closes.len() < period || !stddev.is_finite() {
        return result;
    }

    for index in (period - 1)..closes.len() {
        let window = &closes[index + 1 - period..=index];
        if let Some(middle) = finite_average(window) {
            if middle == 0.0 || !middle.is_finite() {
                continue;
            }
            let variance = window
                .iter()
                .map(|value| (value - middle).powi(2))
                .sum::<f64>()
                / period as f64;
            let Some(deviation) = finite_option(variance)
                .and_then(|variance| finite_option(variance.sqrt() * stddev))
            else {
                continue;
            };
            let upper = middle + deviation;
            let lower = middle - deviation;
            let bandwidth = (upper - lower) / middle;
            if let (Some(upper), Some(lower), Some(bandwidth)) = (
                finite_option(upper),
                finite_option(lower),
                finite_option(bandwidth),
            ) {
                result[index] = Some(BollingerPoint {
                    middle,
                    upper,
                    lower,
                    bandwidth,
                });
            }
        }
    }

    result
}

pub fn adx(candles: &[IndicatorCandle], period: usize) -> Vec<Option<f64>> {
    let mut result = vec![None; candles.len()];
    if period == 0 || candles.len() <= period {
        return result;
    }

    let mut state = AdxState::new(period);

    for (index, current) in candles.iter().copied().enumerate() {
        if !is_valid_candle(current) {
            state.reset();
            continue;
        }

        let Some(previous) = state.previous_candle else {
            state.previous_candle = Some(current);
            continue;
        };
        state.previous_candle = Some(current);

        let Some((range, plus_dm, minus_dm)) = directional_movement(current, previous) else {
            state.reset();
            continue;
        };

        state.smoothed_tr =
            smooth_wilder_value(state.smoothed_tr, &mut state.tr_warmup, range, period);
        state.smoothed_plus_dm = smooth_wilder_value(
            state.smoothed_plus_dm,
            &mut state.plus_dm_warmup,
            plus_dm,
            period,
        );
        state.smoothed_minus_dm = smooth_wilder_value(
            state.smoothed_minus_dm,
            &mut state.minus_dm_warmup,
            minus_dm,
            period,
        );

        let dx = match (
            state.smoothed_tr,
            state.smoothed_plus_dm,
            state.smoothed_minus_dm,
        ) {
            (Some(tr), Some(plus_dm), Some(minus_dm)) if tr > 0.0 => {
                let plus_di = 100.0 * plus_dm / tr;
                let minus_di = 100.0 * minus_dm / tr;
                let denominator = plus_di + minus_di;
                if denominator > 0.0 && denominator.is_finite() {
                    finite_option(100.0 * (plus_di - minus_di).abs() / denominator)
                } else {
                    Some(0.0)
                }
            }
            _ => None,
        };

        if let Some(dx) = dx.filter(|value| value.is_finite()) {
            if state.current_adx.is_none() && state.dx_seed.len() < period {
                state.dx_seed.push(dx);
            }
            state.current_adx = match state.current_adx {
                Some(previous) => {
                    finite_option(((previous * (period as f64 - 1.0)) + dx) / period as f64)
                }
                None if state.dx_seed.len() == period => {
                    finite_option(state.dx_seed.iter().sum::<f64>() / period as f64)
                }
                None => None,
            };
        } else {
            state.current_adx = None;
            state.dx_seed.clear();
        }

        result[index] = state.current_adx.filter(|value| value.is_finite());
    }

    result
}

fn finite_average(values: &[f64]) -> Option<f64> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }

    finite_option(values.iter().sum::<f64>() / values.len() as f64)
}

fn finite_option(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

fn finite_close(candle: IndicatorCandle) -> Option<f64> {
    candle.close.is_finite().then_some(candle.close)
}

fn is_valid_candle(candle: IndicatorCandle) -> bool {
    candle.high.is_finite()
        && candle.low.is_finite()
        && candle.close.is_finite()
        && candle.high >= candle.low
}

fn true_range(candle: IndicatorCandle, previous_close: Option<f64>) -> Option<f64> {
    if !is_valid_candle(candle) {
        return None;
    }

    let high_low = candle.high - candle.low;
    let range = previous_close
        .filter(|close| close.is_finite())
        .map(|close| {
            high_low
                .max((candle.high - close).abs())
                .max((candle.low - close).abs())
        })
        .unwrap_or(high_low);

    range.is_finite().then_some(range.max(0.0))
}

fn directional_movement(
    current: IndicatorCandle,
    previous: IndicatorCandle,
) -> Option<(f64, f64, f64)> {
    if !is_valid_candle(current) || !is_valid_candle(previous) {
        return None;
    }

    let range = true_range(current, finite_close(previous))?;
    let up_move = finite_option(current.high - previous.high)?;
    let down_move = finite_option(previous.low - current.low)?;
    let plus_dm = if up_move > down_move && up_move > 0.0 {
        up_move
    } else {
        0.0
    };
    let minus_dm = if down_move > up_move && down_move > 0.0 {
        down_move
    } else {
        0.0
    };

    Some((range, plus_dm, minus_dm))
}

fn close_delta(previous: f64, current: f64) -> Option<f64> {
    (previous.is_finite() && current.is_finite())
        .then_some(current - previous)
        .and_then(finite_option)
}

fn wilder_gain_loss_seed(values: &[f64]) -> Option<(f64, f64)> {
    if values.len() < 2 || values.iter().any(|value| !value.is_finite()) {
        return None;
    }

    let mut gain = 0.0;
    let mut loss = 0.0;
    for window in values.windows(2) {
        let delta = window[1] - window[0];
        let delta = finite_option(delta)?;
        gain = finite_option(gain + delta.max(0.0))?;
        loss = finite_option(loss + (-delta).max(0.0))?;
    }

    let period = values.len() - 1;
    Some((
        finite_option(gain / period as f64)?,
        finite_option(loss / period as f64)?,
    ))
}

fn rsi_value(average_gain: f64, average_loss: f64) -> f64 {
    if average_loss == 0.0 {
        if average_gain == 0.0 {
            50.0
        } else {
            100.0
        }
    } else {
        100.0 - (100.0 / (1.0 + average_gain / average_loss))
    }
}

fn smooth_wilder_value(
    previous: Option<f64>,
    warmup: &mut Vec<f64>,
    current: f64,
    period: usize,
) -> Option<f64> {
    match previous {
        Some(previous) => finite_option(previous - (previous / period as f64) + current),
        None => {
            warmup.push(current);
            if warmup.len() == period {
                finite_average(warmup).and_then(|average| finite_option(average * period as f64))
            } else {
                None
            }
        }
    }
}

struct AdxState {
    previous_candle: Option<IndicatorCandle>,
    smoothed_tr: Option<f64>,
    smoothed_plus_dm: Option<f64>,
    smoothed_minus_dm: Option<f64>,
    current_adx: Option<f64>,
    tr_warmup: Vec<f64>,
    plus_dm_warmup: Vec<f64>,
    minus_dm_warmup: Vec<f64>,
    dx_seed: Vec<f64>,
}

impl AdxState {
    fn new(period: usize) -> Self {
        Self {
            previous_candle: None,
            smoothed_tr: None,
            smoothed_plus_dm: None,
            smoothed_minus_dm: None,
            current_adx: None,
            tr_warmup: Vec::with_capacity(period),
            plus_dm_warmup: Vec::with_capacity(period),
            minus_dm_warmup: Vec::with_capacity(period),
            dx_seed: Vec::with_capacity(period),
        }
    }

    fn reset(&mut self) {
        self.previous_candle = None;
        self.smoothed_tr = None;
        self.smoothed_plus_dm = None;
        self.smoothed_minus_dm = None;
        self.current_adx = None;
        self.tr_warmup.clear();
        self.plus_dm_warmup.clear();
        self.minus_dm_warmup.clear();
        self.dx_seed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sma_and_ema_return_none_until_warmup() {
        let closes = [1.0, 2.0, 3.0, 4.0];

        assert_eq!(sma(&closes, 3), vec![None, None, Some(2.0), Some(3.0)]);
        assert_eq!(ema(&closes, 3)[0], None);
    }

    #[test]
    fn atr_returns_values_after_warmup() {
        let candles = vec![
            IndicatorCandle {
                high: 11.0,
                low: 9.0,
                close: 10.0,
            },
            IndicatorCandle {
                high: 12.0,
                low: 10.0,
                close: 11.0,
            },
            IndicatorCandle {
                high: 13.0,
                low: 11.0,
                close: 12.0,
            },
        ];

        let values = atr(&candles, 2);
        assert!(values[0].is_none());
        assert!(values[1].unwrap() > 0.0);
    }

    #[test]
    fn rsi_bollinger_and_adx_handle_warmup() {
        let closes = [1.0, 1.1, 1.2, 1.1, 1.3, 1.4, 1.2, 1.5];

        assert!(rsi(&closes, 3).iter().any(Option::is_some));
        assert!(bollinger(&closes, 3, 2.0).iter().any(Option::is_some));
        let candles: Vec<_> = closes
            .iter()
            .map(|close| IndicatorCandle {
                high: close + 0.1,
                low: close - 0.1,
                close: *close,
            })
            .collect();
        assert!(adx(&candles, 3).iter().any(Option::is_some));
    }

    #[test]
    fn indicators_keep_length_and_skip_non_finite_values() {
        let closes = [1.0, f64::NAN, 3.0, f64::INFINITY, 5.0];

        assert_eq!(sma(&closes, 0).len(), closes.len());
        assert_eq!(ema(&closes, 2).len(), closes.len());
        assert_eq!(rsi(&closes, 2).len(), closes.len());
        assert_eq!(bollinger(&closes, 2, 2.0).len(), closes.len());
        assert!(sma(&closes, 2).iter().all(|value| value.is_none()));
        assert!(ema(&closes, 2).into_iter().flatten().all(f64::is_finite));
    }

    #[test]
    fn extreme_finite_inputs_never_emit_non_finite_outputs() {
        let closes = [
            f64::MAX / 2.0,
            f64::MAX / 2.0,
            f64::MAX / 2.0,
            f64::MAX / 2.0,
            f64::MAX / 2.0,
        ];
        let candles: Vec<_> = closes
            .iter()
            .map(|close| IndicatorCandle {
                high: *close,
                low: -*close,
                close: *close,
            })
            .collect();

        assert_all_finite(sma(&closes, 2));
        assert_all_finite(ema(&closes, 2));
        assert_all_finite(atr(&candles, 2));
        assert_all_finite(rsi(&closes, 2));
        assert_all_finite(adx(&candles, 2));
        assert!(bollinger(&closes, 2, 2.0)
            .into_iter()
            .flatten()
            .all(|point| point.middle.is_finite()
                && point.upper.is_finite()
                && point.lower.is_finite()
                && point.bandwidth.is_finite()));
    }

    #[test]
    fn atr_rsi_and_adx_lock_warmup_positions() {
        let candles = vec![
            IndicatorCandle {
                high: 10.0,
                low: 9.0,
                close: 9.5,
            },
            IndicatorCandle {
                high: 11.0,
                low: 10.0,
                close: 10.5,
            },
            IndicatorCandle {
                high: 12.0,
                low: 11.0,
                close: 11.5,
            },
            IndicatorCandle {
                high: 13.0,
                low: 12.0,
                close: 12.5,
            },
        ];
        let closes = [1.0, 2.0, 3.0, 4.0];

        let atr_values = atr(&candles, 3);
        assert_eq!(atr_values[0], None);
        assert_eq!(atr_values[1], None);
        assert_approx_eq(atr_values[2].unwrap(), 4.0 / 3.0);
        assert_approx_eq(atr_values[3].unwrap(), 25.0 / 18.0);
        assert_eq!(rsi(&closes, 3), vec![None, None, None, Some(100.0)]);
        assert_eq!(adx(&candles, 2), vec![None, None, None, Some(100.0)]);
    }

    #[test]
    fn atr_and_adx_restart_warmup_after_invalid_candle() {
        let candles = vec![
            IndicatorCandle {
                high: 10.0,
                low: 9.0,
                close: 9.5,
            },
            IndicatorCandle {
                high: 11.0,
                low: 10.0,
                close: 10.5,
            },
            IndicatorCandle {
                high: 12.0,
                low: 11.0,
                close: 11.5,
            },
            IndicatorCandle {
                high: 13.0,
                low: 12.0,
                close: 12.5,
            },
            IndicatorCandle {
                high: 14.0,
                low: 13.0,
                close: f64::NAN,
            },
            IndicatorCandle {
                high: 15.0,
                low: 14.0,
                close: 14.5,
            },
            IndicatorCandle {
                high: 16.0,
                low: 15.0,
                close: 15.5,
            },
            IndicatorCandle {
                high: 17.0,
                low: 16.0,
                close: 16.5,
            },
        ];

        let atr_values = atr(&candles, 3);
        assert!(atr_values[2].is_some());
        assert!(atr_values[3].is_some());
        assert_eq!(atr_values[4], None);
        assert_eq!(atr_values[5], None);
        assert_eq!(atr_values[6], None);
        assert!(atr_values[7].is_some());

        let adx_values = adx(&candles, 2);
        assert!(adx_values[3].is_some());
        assert_eq!(adx_values[4], None);
        assert_eq!(adx_values[5], None);
        assert_eq!(adx_values[6], None);
        assert_eq!(adx_values[7], None);
    }

    fn assert_all_finite(values: Vec<Option<f64>>) {
        assert!(values.into_iter().flatten().all(f64::is_finite));
    }

    fn assert_approx_eq(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
    }
}
