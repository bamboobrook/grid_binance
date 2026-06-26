use std::collections::BTreeMap;

use shared_domain::martingale::{MartingaleIndicatorConfig, MartingaleStrategyConfig};

use crate::indicators::{adx_advance, atr, bollinger, ema, rsi, sma, AdxState, IndicatorCandle};
use crate::market_data::KlineBar;

#[derive(Debug, Clone)]
pub struct IncrementalIndicatorState {
    pub previous_candle: Option<IndicatorCandle>,
    pub warmup_ranges: Vec<f64>,
    pub current_value: Option<f64>,
}

impl Default for IncrementalIndicatorState {
    fn default() -> Self {
        Self {
            previous_candle: None,
            warmup_ranges: Vec::new(),
            current_value: None,
        }
    }
}

pub type IndicatorKey = (String, String, usize);

#[derive(Debug, Clone)]
pub struct IndicatorRuntimeContext {
    pub bars_by_symbol: BTreeMap<String, Vec<KlineBar>>,
    pub incremental: BTreeMap<IndicatorKey, IncrementalIndicatorState>,
    pub incremental_adx: BTreeMap<IndicatorKey, AdxState>,
}

impl Default for IndicatorRuntimeContext {
    fn default() -> Self {
        Self {
            bars_by_symbol: BTreeMap::new(),
            incremental: BTreeMap::new(),
            incremental_adx: BTreeMap::new(),
        }
    }
}

impl IndicatorRuntimeContext {
    pub fn push_bar(&mut self, bar: &KlineBar) {
        self.bars_by_symbol
            .entry(bar.symbol.clone())
            .or_default()
            .push(bar.clone());
        let candle = indicator_candle(bar);
        let sym = bar.symbol.clone();
        if candle.high.is_finite() && candle.low.is_finite() && candle.close.is_finite() {
            for key in self
                .incremental
                .keys()
                .filter(|k| k.0 == sym)
                .cloned()
                .collect::<Vec<_>>()
            {
                if let Some(state) = self.incremental.get_mut(&key) {
                    let (_, _, period) = &key;
                    let period = *period;
                    let range = crate::indicators::true_range(
                        candle,
                        state
                            .previous_candle
                            .map(|c| c.close)
                            .filter(|v| v.is_finite()),
                    );
                    state.previous_candle = Some(candle);
                    let range = match range {
                        Some(r) if r.is_finite() => r,
                        _ => {
                            state.warmup_ranges.clear();
                            state.current_value = None;
                            continue;
                        }
                    };
                    state.current_value = match (state.current_value, range) {
                        (Some(prev), _) => {
                            Some(((prev * (period as f64 - 1.0)) + range) / period as f64)
                        }
                        (None, _) => {
                            state.warmup_ranges.push(range);
                            if state.warmup_ranges.len() == period {
                                let sum: f64 = state.warmup_ranges.iter().sum();
                                state.warmup_ranges.clear();
                                Some(sum / period as f64)
                            } else {
                                None
                            }
                        }
                    };
                }
            }
            // ADX 增量更新（避免 resolve_operand 每根 K线全量重算 O(n²)）
            for key in self
                .incremental_adx
                .keys()
                .filter(|k| k.0 == sym)
                .cloned()
                .collect::<Vec<_>>()
            {
                if let Some(state) = self.incremental_adx.get_mut(&key) {
                    adx_advance(state, candle, key.2);
                }
            }
        }
    }

    pub fn ensure_adx_cached(&mut self, symbol: &str, period: usize) {
        let key = (symbol.to_string(), "adx".to_string(), period);
        if self.incremental_adx.contains_key(&key) {
            return;
        }
        let bars = self.bars_by_symbol.get(symbol).cloned().unwrap_or_default();
        let mut state = AdxState::new(period);
        for bar in &bars {
            adx_advance(&mut state, indicator_candle(bar), period);
        }
        self.incremental_adx.insert(key, state);
    }

    pub fn latest_adx(&mut self, symbol: &str, period: usize) -> Option<f64> {
        self.ensure_adx_cached(symbol, period);
        let key = (symbol.to_string(), "adx".to_string(), period);
        self.incremental_adx.get(&key).and_then(|s| s.current_adx)
    }

    pub fn ensure_atr_cached(&mut self, symbol: &str, period: usize) {
        let key = (symbol.to_string(), "atr".to_string(), period);
        if self.incremental.contains_key(&key) {
            return;
        }
        let bars = self.bars_by_symbol.get(symbol).cloned().unwrap_or_default();
        let mut state = IncrementalIndicatorState::default();
        for bar in &bars {
            let candle = indicator_candle(bar);
            if !candle.high.is_finite() || !candle.low.is_finite() || !candle.close.is_finite() {
                state.warmup_ranges.clear();
                state.current_value = None;
                state.previous_candle = None;
                continue;
            }
            let range = crate::indicators::true_range(
                candle,
                state
                    .previous_candle
                    .map(|c| c.close)
                    .filter(|v| v.is_finite()),
            );
            state.previous_candle = Some(candle);
            let range = match range {
                Some(r) if r.is_finite() => r,
                _ => {
                    state.warmup_ranges.clear();
                    state.current_value = None;
                    continue;
                }
            };
            state.current_value = match (state.current_value, range) {
                (Some(prev), _) => Some(((prev * (period as f64 - 1.0)) + range) / period as f64),
                (None, _) => {
                    state.warmup_ranges.push(range);
                    if state.warmup_ranges.len() == period {
                        let sum: f64 = state.warmup_ranges.iter().sum();
                        state.warmup_ranges.clear();
                        Some(sum / period as f64)
                    } else {
                        None
                    }
                }
            };
        }
        self.incremental.insert(key, state);
    }

    pub fn evaluate_expression(
        &mut self,
        symbol: &str,
        expression: &str,
    ) -> Result<Option<bool>, String> {
        let expression = expression.trim();
        let Some((operator, left, right)) = split_comparison(expression) else {
            return Err(format!("unsupported indicator expression: {expression}"));
        };
        let Some(left) = self.resolve_operand(symbol, left.trim())? else {
            return Ok(None);
        };
        let Some(right) = self.resolve_operand(symbol, right.trim())? else {
            return Ok(None);
        };
        Ok(Some(match operator {
            ">" => left > right,
            ">=" => left >= right,
            "<" => left < right,
            "<=" => left <= right,
            "==" => (left - right).abs() <= f64::EPSILON,
            "!=" => (left - right).abs() > f64::EPSILON,
            _ => return Err(format!("unsupported indicator operator: {operator}")),
        }))
    }

    fn resolve_operand(&mut self, symbol: &str, operand: &str) -> Result<Option<f64>, String> {
        if let Ok(value) = operand.parse::<f64>() {
            return Ok(Some(value));
        }
        let bars = self
            .bars_by_symbol
            .get(symbol)
            .ok_or_else(|| format!("no indicator bars for symbol {symbol}"))?;
        let latest = bars
            .last()
            .ok_or_else(|| format!("no indicator bars for symbol {symbol}"))?;
        match operand.to_ascii_lowercase().as_str() {
            "open" => return Ok(Some(latest.open)),
            "high" => return Ok(Some(latest.high)),
            "low" => return Ok(Some(latest.low)),
            "close" => return Ok(Some(latest.close)),
            _ => {}
        }

        let Some((name, args)) = parse_indicator_call(operand)? else {
            return Err(format!("unsupported indicator operand: {operand}"));
        };
        let closes = bars.iter().map(|bar| bar.close).collect::<Vec<_>>();
        let candles = bars.iter().map(indicator_candle).collect::<Vec<_>>();
        let value = match name.as_str() {
            "sma" => sma(&closes, one_usize_arg(&name, &args)?)
                .last()
                .copied()
                .flatten(),
            "ema" => ema(&closes, one_usize_arg(&name, &args)?)
                .last()
                .copied()
                .flatten(),
            "rsi" => rsi(&closes, one_usize_arg(&name, &args)?)
                .last()
                .copied()
                .flatten(),
            "atr" => atr(&candles, one_usize_arg(&name, &args)?)
                .last()
                .copied()
                .flatten(),
            "adx" => self.latest_adx(symbol, one_usize_arg(&name, &args)?),
            "bb_upper" => {
                let (period, stddev) = bollinger_args(&name, &args)?;
                bollinger(&closes, period, stddev)
                    .last()
                    .copied()
                    .flatten()
                    .map(|point| point.upper)
            }
            "bb_middle" => {
                let (period, stddev) = bollinger_args(&name, &args)?;
                bollinger(&closes, period, stddev)
                    .last()
                    .copied()
                    .flatten()
                    .map(|point| point.middle)
            }
            "bb_lower" => {
                let (period, stddev) = bollinger_args(&name, &args)?;
                bollinger(&closes, period, stddev)
                    .last()
                    .copied()
                    .flatten()
                    .map(|point| point.lower)
            }
            "bb_bandwidth" => {
                let (period, stddev) = bollinger_args(&name, &args)?;
                bollinger(&closes, period, stddev)
                    .last()
                    .copied()
                    .flatten()
                    .map(|point| point.bandwidth)
            }
            _ => return Err(format!("unsupported indicator operand: {operand}")),
        };
        Ok(value)
    }

    pub fn latest_atr(&mut self, symbol: &str, period: usize) -> Option<f64> {
        self.ensure_atr_cached(symbol, period);
        let key = (symbol.to_string(), "atr".to_string(), period);
        self.incremental.get(&key).and_then(|s| s.current_value)
    }
}

pub fn latest_atr_for_strategy(
    indicator_context: &mut IndicatorRuntimeContext,
    strategy: &MartingaleStrategyConfig,
) -> Option<f64> {
    let period = strategy
        .indicators
        .iter()
        .find_map(|indicator| match indicator {
            MartingaleIndicatorConfig::Atr { period } => Some(*period as usize),
            _ => None,
        })
        .unwrap_or(2);
    indicator_context.latest_atr(&strategy.symbol, period)
}

pub fn indicator_candle(bar: &KlineBar) -> IndicatorCandle {
    IndicatorCandle {
        high: bar.high,
        low: bar.low,
        close: bar.close,
    }
}

pub fn split_comparison(expression: &str) -> Option<(&'static str, &str, &str)> {
    for operator in [">=", "<=", "==", "!=", ">", "<"] {
        if let Some(index) = expression.find(operator) {
            let left = &expression[..index];
            let right = &expression[index + operator.len()..];
            if !left.trim().is_empty() && !right.trim().is_empty() {
                return Some((operator, left, right));
            }
        }
    }
    None
}

pub fn parse_indicator_call(operand: &str) -> Result<Option<(String, Vec<String>)>, String> {
    let operand = operand.trim().to_ascii_lowercase();
    let Some(open_paren) = operand.find('(') else {
        return Ok(None);
    };
    if !operand.ends_with(')') {
        return Err(format!("invalid indicator operand: {operand}"));
    }
    let name = operand[..open_paren].trim().to_string();
    let args = operand[open_paren + 1..operand.len() - 1]
        .split(',')
        .map(|arg| arg.trim().to_string())
        .filter(|arg| !arg.is_empty())
        .collect::<Vec<_>>();
    Ok(Some((name, args)))
}

pub fn one_usize_arg(name: &str, args: &[String]) -> Result<usize, String> {
    if args.len() != 1 {
        return Err(format!("{name} requires exactly one period argument"));
    }
    let period = args[0]
        .parse::<usize>()
        .map_err(|_| format!("invalid indicator period for {name}"))?;
    if period == 0 {
        return Err(format!("indicator period must be > 0 for {name}"));
    }
    Ok(period)
}

pub fn bollinger_args(name: &str, args: &[String]) -> Result<(usize, f64), String> {
    if !(1..=2).contains(&args.len()) {
        return Err(format!(
            "{name} requires period and optional stddev arguments"
        ));
    }
    let period = one_usize_arg(name, &args[..1])?;
    let stddev = if args.len() == 2 {
        args[1]
            .parse::<f64>()
            .map_err(|_| format!("invalid bollinger stddev for {name}"))?
    } else {
        2.0
    };
    validate_positive_f64("bollinger.stddev", stddev)?;
    Ok((period, stddev))
}

fn validate_positive_f64(label: &str, value: f64) -> Result<(), String> {
    if value <= 0.0 {
        return Err(format!("{label} must be positive, got {value}"));
    }
    Ok(())
}
