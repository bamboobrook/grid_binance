use std::collections::BTreeMap;

use shared_domain::martingale::{
    MartingaleIndicatorConfig, MartingalePortfolioConfig, MartingaleStrategyConfig,
};

use crate::indicators::{adx_advance, AdxState, BollingerPoint, IndicatorCandle};
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
pub type BollingerKey = (String, usize, String);

#[derive(Debug, Clone, Default)]
pub struct SmaIndicatorState {
    pub window: Vec<f64>,
    pub current_value: Option<f64>,
}

impl SmaIndicatorState {
    fn advance(&mut self, close: f64, period: usize) {
        push_window(&mut self.window, close, period);
        self.current_value =
            if self.window.len() == period && self.window.iter().all(|v| v.is_finite()) {
                finite_value(self.window.iter().sum::<f64>() / period as f64)
            } else {
                None
            };
    }
}

#[derive(Debug, Clone, Default)]
pub struct EmaIndicatorState {
    pub index: usize,
    pub window: Vec<f64>,
    pub current_value: Option<f64>,
}

impl EmaIndicatorState {
    fn advance(&mut self, close: f64, period: usize) {
        let index = self.index;
        self.index += 1;
        push_window(&mut self.window, close, period);
        if !close.is_finite() {
            self.current_value = None;
            return;
        }
        let multiplier = 2.0 / (period as f64 + 1.0);
        self.current_value = match self.current_value {
            Some(previous) => finite_value((close - previous) * multiplier + previous),
            None if index + 1 >= period
                && self.window.len() == period
                && self.window.iter().all(|v| v.is_finite()) =>
            {
                finite_value(self.window.iter().sum::<f64>() / period as f64)
            }
            None => None,
        };
    }
}

#[derive(Debug, Clone, Default)]
pub struct RsiIndicatorState {
    pub index: usize,
    pub previous_close: Option<f64>,
    pub window: Vec<f64>,
    pub average_gain: Option<f64>,
    pub average_loss: Option<f64>,
    pub current_value: Option<f64>,
}

impl RsiIndicatorState {
    fn advance(&mut self, close: f64, period: usize) {
        let index = self.index;
        self.index += 1;
        push_window(&mut self.window, close, period + 1);

        if index >= period {
            let delta = self
                .previous_close
                .filter(|previous| previous.is_finite() && close.is_finite())
                .and_then(|previous| finite_value(close - previous));
            if let Some(delta) = delta {
                let gain = delta.max(0.0);
                let loss = (-delta).max(0.0);
                match (self.average_gain, self.average_loss) {
                    (Some(previous_gain), Some(previous_loss)) => {
                        self.average_gain = finite_value(
                            ((previous_gain * (period as f64 - 1.0)) + gain) / period as f64,
                        );
                        self.average_loss = finite_value(
                            ((previous_loss * (period as f64 - 1.0)) + loss) / period as f64,
                        );
                    }
                    _ => {
                        if self.window.len() == period + 1
                            && self.window.iter().all(|value| value.is_finite())
                        {
                            let mut gain_seed = 0.0;
                            let mut loss_seed = 0.0;
                            for window in self.window.windows(2) {
                                let delta = window[1] - window[0];
                                gain_seed += delta.max(0.0);
                                loss_seed += (-delta).max(0.0);
                            }
                            self.average_gain = finite_value(gain_seed / period as f64);
                            self.average_loss = finite_value(loss_seed / period as f64);
                        } else {
                            self.average_gain = None;
                            self.average_loss = None;
                        }
                    }
                }
            } else {
                self.average_gain = None;
                self.average_loss = None;
            }

            self.current_value = match (self.average_gain, self.average_loss) {
                (Some(gain), Some(loss)) if gain.is_finite() && loss.is_finite() => {
                    finite_value(rsi_value(gain, loss))
                }
                _ => None,
            };
        } else {
            self.current_value = None;
        }

        self.previous_close = Some(close);
    }
}

#[derive(Debug, Clone)]
pub struct BollingerIndicatorState {
    pub window: Vec<f64>,
    pub stddev: f64,
    pub current_value: Option<BollingerPoint>,
}

impl BollingerIndicatorState {
    fn new(stddev: f64) -> Self {
        Self {
            window: Vec::new(),
            stddev,
            current_value: None,
        }
    }

    fn advance(&mut self, close: f64, period: usize) {
        push_window(&mut self.window, close, period);
        self.current_value = None;
        if self.window.len() != period || !self.window.iter().all(|value| value.is_finite()) {
            return;
        }
        let Some(middle) = finite_value(self.window.iter().sum::<f64>() / period as f64) else {
            return;
        };
        if middle == 0.0 {
            return;
        }
        let variance = self
            .window
            .iter()
            .map(|value| (value - middle).powi(2))
            .sum::<f64>()
            / period as f64;
        let Some(deviation) =
            finite_value(variance).and_then(|v| finite_value(v.sqrt() * self.stddev))
        else {
            return;
        };
        let upper = middle + deviation;
        let lower = middle - deviation;
        let bandwidth = (upper - lower) / middle;
        if let (Some(upper), Some(lower), Some(bandwidth)) = (
            finite_value(upper),
            finite_value(lower),
            finite_value(bandwidth),
        ) {
            self.current_value = Some(BollingerPoint {
                middle,
                upper,
                lower,
                bandwidth,
            });
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndicatorRuntimeContext {
    pub bars_by_symbol: BTreeMap<String, Vec<KlineBar>>,
    pub incremental: BTreeMap<IndicatorKey, IncrementalIndicatorState>,
    pub incremental_adx: BTreeMap<IndicatorKey, AdxState>,
    pub incremental_sma: BTreeMap<IndicatorKey, SmaIndicatorState>,
    pub incremental_ema: BTreeMap<IndicatorKey, EmaIndicatorState>,
    pub incremental_rsi: BTreeMap<IndicatorKey, RsiIndicatorState>,
    pub incremental_bollinger: BTreeMap<BollingerKey, BollingerIndicatorState>,
}

impl Default for IndicatorRuntimeContext {
    fn default() -> Self {
        Self {
            bars_by_symbol: BTreeMap::new(),
            incremental: BTreeMap::new(),
            incremental_adx: BTreeMap::new(),
            incremental_sma: BTreeMap::new(),
            incremental_ema: BTreeMap::new(),
            incremental_rsi: BTreeMap::new(),
            incremental_bollinger: BTreeMap::new(),
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
        for key in self
            .incremental_sma
            .keys()
            .filter(|k| k.0 == sym)
            .cloned()
            .collect::<Vec<_>>()
        {
            if let Some(state) = self.incremental_sma.get_mut(&key) {
                state.advance(bar.close, key.2);
            }
        }
        for key in self
            .incremental_ema
            .keys()
            .filter(|k| k.0 == sym)
            .cloned()
            .collect::<Vec<_>>()
        {
            if let Some(state) = self.incremental_ema.get_mut(&key) {
                state.advance(bar.close, key.2);
            }
        }
        for key in self
            .incremental_rsi
            .keys()
            .filter(|k| k.0 == sym)
            .cloned()
            .collect::<Vec<_>>()
        {
            if let Some(state) = self.incremental_rsi.get_mut(&key) {
                state.advance(bar.close, key.2);
            }
        }
        for key in self
            .incremental_bollinger
            .keys()
            .filter(|k| k.0 == sym)
            .cloned()
            .collect::<Vec<_>>()
        {
            if let Some(state) = self.incremental_bollinger.get_mut(&key) {
                state.advance(bar.close, key.1);
            }
        }
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

    pub fn ensure_sma_cached(&mut self, symbol: &str, period: usize) {
        let key = (symbol.to_string(), "sma".to_string(), period);
        if self.incremental_sma.contains_key(&key) {
            return;
        }
        let mut state = SmaIndicatorState::default();
        if let Some(bars) = self.bars_by_symbol.get(symbol) {
            for bar in bars {
                state.advance(bar.close, period);
            }
        }
        self.incremental_sma.insert(key, state);
    }

    pub fn latest_sma(&mut self, symbol: &str, period: usize) -> Option<f64> {
        self.ensure_sma_cached(symbol, period);
        let key = (symbol.to_string(), "sma".to_string(), period);
        self.incremental_sma
            .get(&key)
            .and_then(|state| state.current_value)
    }

    pub fn ensure_ema_cached(&mut self, symbol: &str, period: usize) {
        let key = (symbol.to_string(), "ema".to_string(), period);
        if self.incremental_ema.contains_key(&key) {
            return;
        }
        let mut state = EmaIndicatorState::default();
        if let Some(bars) = self.bars_by_symbol.get(symbol) {
            for bar in bars {
                state.advance(bar.close, period);
            }
        }
        self.incremental_ema.insert(key, state);
    }

    pub fn latest_ema(&mut self, symbol: &str, period: usize) -> Option<f64> {
        self.ensure_ema_cached(symbol, period);
        let key = (symbol.to_string(), "ema".to_string(), period);
        self.incremental_ema
            .get(&key)
            .and_then(|state| state.current_value)
    }

    pub fn ensure_rsi_cached(&mut self, symbol: &str, period: usize) {
        let key = (symbol.to_string(), "rsi".to_string(), period);
        if self.incremental_rsi.contains_key(&key) {
            return;
        }
        let mut state = RsiIndicatorState::default();
        if let Some(bars) = self.bars_by_symbol.get(symbol) {
            for bar in bars {
                state.advance(bar.close, period);
            }
        }
        self.incremental_rsi.insert(key, state);
    }

    pub fn latest_rsi(&mut self, symbol: &str, period: usize) -> Option<f64> {
        self.ensure_rsi_cached(symbol, period);
        let key = (symbol.to_string(), "rsi".to_string(), period);
        self.incremental_rsi
            .get(&key)
            .and_then(|state| state.current_value)
    }

    pub fn ensure_bollinger_cached(&mut self, symbol: &str, period: usize, stddev: f64) {
        let key = (symbol.to_string(), period, bollinger_stddev_key(stddev));
        if self.incremental_bollinger.contains_key(&key) {
            return;
        }
        let mut state = BollingerIndicatorState::new(stddev);
        if let Some(bars) = self.bars_by_symbol.get(symbol) {
            for bar in bars {
                state.advance(bar.close, period);
            }
        }
        self.incremental_bollinger.insert(key, state);
    }

    pub fn latest_bollinger(
        &mut self,
        symbol: &str,
        period: usize,
        stddev: f64,
    ) -> Option<BollingerPoint> {
        self.ensure_bollinger_cached(symbol, period, stddev);
        let key = (symbol.to_string(), period, bollinger_stddev_key(stddev));
        self.incremental_bollinger
            .get(&key)
            .and_then(|state| state.current_value)
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
        // Bare OHLC token (open/high/low/close) for the strategy's own symbol,
        // OR a cross-symbol OHLC reference like `btcusdt.close` (no parens).
        let lower = operand.trim().to_ascii_lowercase();
        let (ohlc_field, ohlc_symbol_override) = if let Some(dot) = lower.find('.') {
            let prefix = &lower[..dot];
            let field = lower[dot + 1..].trim();
            if !field.is_empty() {
                (Some(field), Some(prefix.to_ascii_uppercase()))
            } else {
                (None, None)
            }
        } else {
            (Some(lower.as_str()), None)
        };
        if let Some(field) = ohlc_field {
            if matches!(field, "open" | "high" | "low" | "close") {
                let effective_symbol = ohlc_symbol_override.as_deref().unwrap_or(symbol);
                let bars = self
                    .bars_by_symbol
                    .get(effective_symbol)
                    .ok_or_else(|| format!("no indicator bars for symbol {effective_symbol}"))?;
                let latest = bars
                    .last()
                    .ok_or_else(|| format!("no indicator bars for symbol {effective_symbol}"))?;
                return Ok(Some(match field {
                    "open" => latest.open,
                    "high" => latest.high,
                    "low" => latest.low,
                    "close" => latest.close,
                    _ => unreachable!(),
                }));
            }
        }

        let Some((name, args)) = parse_indicator_call(operand)? else {
            return Err(format!("unsupported indicator operand: {operand}"));
        };
        // Detect an optional `symbol.` prefix (e.g. `btcusdt.ema(50)`) so a
        // strategy can gate entries on a different symbol's indicators. When
        // present, lookups use the override symbol; otherwise the strategy's
        // own symbol is used.
        let (indicator_name, symbol_override) = split_symbol_prefix(&name);
        let effective_symbol = symbol_override.as_deref().unwrap_or(symbol);
        let value = match indicator_name.as_str() {
            "sma" => self.latest_sma(effective_symbol, one_usize_arg(&name, &args)?),
            "ema" => self.latest_ema(effective_symbol, one_usize_arg(&name, &args)?),
            "rsi" => self.latest_rsi(effective_symbol, one_usize_arg(&name, &args)?),
            "atr" => self.latest_atr(effective_symbol, one_usize_arg(&name, &args)?),
            "adx" => self.latest_adx(effective_symbol, one_usize_arg(&name, &args)?),
            "bb_upper" => {
                let (period, stddev) = bollinger_args(&name, &args)?;
                self.latest_bollinger(effective_symbol, period, stddev)
                    .map(|point| point.upper)
            }
            "bb_middle" => {
                let (period, stddev) = bollinger_args(&name, &args)?;
                self.latest_bollinger(effective_symbol, period, stddev)
                    .map(|point| point.middle)
            }
            "bb_lower" => {
                let (period, stddev) = bollinger_args(&name, &args)?;
                self.latest_bollinger(effective_symbol, period, stddev)
                    .map(|point| point.lower)
            }
            "bb_bandwidth" => {
                let (period, stddev) = bollinger_args(&name, &args)?;
                self.latest_bollinger(effective_symbol, period, stddev)
                    .map(|point| point.bandwidth)
            }
            "atr_percent" => {
                // ATR as a percentage of close price: atr(period) / close * 100.
                // Lets users write volatility-regime filters like
                // `BTCUSDT.atr_percent(14) < 2.0` without needing arithmetic
                // in the expression language.
                let period = one_usize_arg(&name, &args)?;
                let atr = self.latest_atr(effective_symbol, period);
                let bars = self.bars_by_symbol.get(effective_symbol);
                let close = bars.and_then(|b| b.last()).map(|b| b.close);
                match (atr, close) {
                    (Some(atr), Some(close)) if close > 0.0 => Some(atr / close * 100.0),
                    _ => None,
                }
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

/// Parse an optional `symbol.` prefix from an indicator-call name, returning
/// `(indicator_name, Option<symbol_override>)`. The override is upper-cased so
/// it matches `bars_by_symbol` keys (which store symbols verbatim from the
/// market-data source, e.g. `BTCUSDT`). When no `.` is present, the override is
/// `None` and callers fall back to the strategy's own symbol.
///
/// Examples:
///   `"ema"`                 -> `("ema", None)`
///   `"btcusdt.ema"`         -> `("ema", Some("BTCUSDT"))`
///   `"BTCUSDT.bb_bandwidth"`-> `("bb_bandwidth", Some("BTCUSDT"))`
fn split_symbol_prefix(name: &str) -> (String, Option<String>) {
    if let Some(dot) = name.find('.') {
        let symbol = name[..dot].trim().to_ascii_uppercase();
        let indicator = name[dot + 1..].trim().to_string();
        if symbol.is_empty() || indicator.is_empty() {
            return (name.to_string(), None);
        }
        (indicator, Some(symbol))
    } else {
        (name.to_string(), None)
    }
}

/// Extract any cross-symbol reference from a single comparison operand,
/// mirroring `resolve_operand`'s two forms:
///   - `SYMBOL.indicator(args)`  (parsed via `parse_indicator_call` + `split_symbol_prefix`)
///   - `symbol.ohlc`             (e.g. `btcusdt.close`, a bare dot lookup)
/// Returns the upper-cased referenced symbol when it differs from the strategy's
/// own symbol; otherwise `None`. Only references to *other* symbols count as
/// dependencies — a strategy referencing its own symbol/indicators needs no
/// extra market data.
fn extract_symbol_ref_from_operand(operand: &str, strategy_symbol: &str) -> Option<String> {
    let operand = operand.trim();
    // Numeric literal — no symbol reference.
    if operand.parse::<f64>().is_ok() {
        return None;
    }
    let own_upper = strategy_symbol.to_ascii_uppercase();
    let lower = operand.to_ascii_lowercase();
    if let Some(dot) = lower.find('.') {
        let prefix = lower[..dot].trim().to_ascii_uppercase();
        // A bare-symbol dot reference (`btcusdt.close`) OR an indicator-call
        // (`btcusdt.ema(50)`): both carry the symbol in the prefix.
        if !prefix.is_empty() && prefix != own_upper {
            return Some(prefix);
        }
    }
    None
}

/// Scan a single indicator expression string for cross-symbol references,
/// appending any found to `deps`. Handles both operands of a comparison
/// (`a op b`). Silently ignores operands that do not parse into a comparison
/// (mirrors `evaluate_expression`'s tolerance for unsupported forms).
fn append_deps_from_expression(expression: &str, strategy_symbol: &str, deps: &mut Vec<String>) {
    let Some((_, left, right)) = split_comparison(expression.trim()) else {
        return;
    };
    if let Some(symbol) = extract_symbol_ref_from_operand(left, strategy_symbol) {
        deps.push(symbol);
    }
    if let Some(symbol) = extract_symbol_ref_from_operand(right, strategy_symbol) {
        deps.push(symbol);
    }
}

/// Extract the set of market-data dependency symbols referenced by a portfolio's
/// cross-symbol indicator expressions (entry triggers and indicator stop-loss).
///
/// A "dependency" is a symbol referenced via `SYMBOL.indicator(...)` or
/// `symbol.ohlc` in a strategy's expressions that is NOT the strategy's own
/// trading symbol. Such symbols must have their 1m bars loaded (backtest) /
/// kline ticks subscribed (live) so the expressions can be evaluated, even
/// though they are never traded.
///
/// Returns dependency symbols upper-cased, de-duplicated, and sorted (for
/// deterministic output). Strategies' own trading symbols are excluded.
pub fn extract_symbol_dependencies(config: &MartingalePortfolioConfig) -> Vec<String> {
    use shared_domain::martingale::{MartingaleEntryTrigger, MartingaleStopLossModel};

    let mut deps: Vec<String> = Vec::new();
    for strategy in &config.strategies {
        let own_symbol = strategy.symbol.trim().to_ascii_uppercase();
        for trigger in &strategy.entry_triggers {
            if let MartingaleEntryTrigger::IndicatorExpression { expression } = trigger {
                append_deps_from_expression(expression.as_str(), own_symbol.as_str(), &mut deps);
            }
        }
        if let Some(MartingaleStopLossModel::Indicator { expression }) = &strategy.stop_loss {
            append_deps_from_expression(expression.as_str(), own_symbol.as_str(), &mut deps);
        }
    }

    // A dependency is a referenced symbol that is NOT one of the portfolio's own
    // trading symbols. De-dup into a set, then return a sorted, deterministic Vec.
    let own_symbols: std::collections::HashSet<String> = config
        .strategies
        .iter()
        .map(|s| s.symbol.trim().to_ascii_uppercase())
        .collect();
    let mut result: Vec<String> = deps
        .into_iter()
        .filter(|sym| !own_symbols.contains(sym))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    result.sort();
    result.dedup();
    result
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

fn push_window(window: &mut Vec<f64>, value: f64, max_len: usize) {
    if max_len == 0 {
        window.clear();
        return;
    }
    window.push(value);
    if window.len() > max_len {
        window.remove(0);
    }
}

fn finite_value(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
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

fn bollinger_stddev_key(stddev: f64) -> String {
    format!("{stddev:.8}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indicators::{bollinger, ema, rsi, sma};

    fn bar(index: usize, close: f64) -> KlineBar {
        KlineBar {
            symbol: "BTCUSDT".to_string(),
            open_time_ms: index as i64 * 60_000,
            open: close,
            high: close + 2.0,
            low: close - 2.0,
            close,
            volume: 1.0,
        }
    }

    fn bar_for(symbol: &str, index: usize, close: f64) -> KlineBar {
        KlineBar {
            symbol: symbol.to_string(),
            open_time_ms: index as i64 * 60_000,
            open: close,
            high: close + 2.0,
            low: close - 2.0,
            close,
            volume: 1.0,
        }
    }

    fn assert_optional_close(actual: Option<f64>, expected: Option<f64>) {
        match (actual, expected) {
            (Some(actual), Some(expected)) => {
                assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
            }
            (None, None) => {}
            other => panic!("optional values differ: {other:?}"),
        }
    }

    #[test]
    fn cached_expression_indicators_match_batch_calculations() {
        let closes = vec![
            100.0, 101.0, 99.5, 102.0, 104.0, 103.0, 105.0, 108.0, 107.0, 109.0, 111.0, 110.0,
            112.0, 114.0, 113.0, 115.0, 118.0, 117.0, 119.0, 121.0,
        ];
        let expected_sma = sma(&closes, 5);
        let expected_ema = ema(&closes, 5);
        let expected_rsi = rsi(&closes, 5);
        let expected_bb = bollinger(&closes, 5, 2.0);

        let mut context = IndicatorRuntimeContext::default();
        for (index, close) in closes.iter().copied().enumerate() {
            context.push_bar(&bar(index, close));

            assert_optional_close(context.latest_sma("BTCUSDT", 5), expected_sma[index]);
            assert_optional_close(context.latest_ema("BTCUSDT", 5), expected_ema[index]);
            assert_optional_close(context.latest_rsi("BTCUSDT", 5), expected_rsi[index]);

            let actual_bb = context.latest_bollinger("BTCUSDT", 5, 2.0);
            match (actual_bb, expected_bb[index]) {
                (Some(actual), Some(expected)) => {
                    assert!((actual.middle - expected.middle).abs() < 1e-9);
                    assert!((actual.upper - expected.upper).abs() < 1e-9);
                    assert!((actual.lower - expected.lower).abs() < 1e-9);
                    assert!((actual.bandwidth - expected.bandwidth).abs() < 1e-9);
                }
                (None, None) => {}
                other => panic!("bollinger values differ: {other:?}"),
            }
        }
    }

    #[test]
    fn split_symbol_prefix_parses_cross_symbol_indicator() {
        // No dot → no override.
        assert_eq!(split_symbol_prefix("ema"), ("ema".to_string(), None));
        assert_eq!(
            split_symbol_prefix("bb_bandwidth"),
            ("bb_bandwidth".to_string(), None)
        );
        // Dot → override upper-cased.
        assert_eq!(
            split_symbol_prefix("btcusdt.ema"),
            ("ema".to_string(), Some("BTCUSDT".to_string()))
        );
        assert_eq!(
            split_symbol_prefix("BTCUSDT.bb_bandwidth"),
            ("bb_bandwidth".to_string(), Some("BTCUSDT".to_string()))
        );
        assert_eq!(
            split_symbol_prefix("ethusdt.close"),
            ("close".to_string(), Some("ETHUSDT".to_string()))
        );
        // Empty sides → no override (defensive).
        assert_eq!(split_symbol_prefix(".ema"), (".ema".to_string(), None));
        assert_eq!(
            split_symbol_prefix("btcusdt."),
            ("btcusdt.".to_string(), None)
        );
    }

    #[test]
    fn evaluate_expression_resolves_cross_symbol_indicator_reference() {
        // ALT strategy references BTC's ema(2). Bars for both BTCUSDT and
        // ALTUSDT live in the same context. ALT closes are flat at 100; BTC
        // closes trend up so BTC ema(2) rises above 100 by bar 2.
        let mut context = IndicatorRuntimeContext::default();
        // BTCUSDT: 100, 102, 104 → ema(2) at bar 2 ≈ 103.33
        context.push_bar(&bar_for("BTCUSDT", 0, 100.0));
        context.push_bar(&bar_for("BTCUSDT", 1, 102.0));
        context.push_bar(&bar_for("BTCUSDT", 2, 104.0));
        // ALTUSDT: 100, 100, 100 → own ema(2) = 100
        context.push_bar(&bar_for("ALTUSDT", 0, 100.0));
        context.push_bar(&bar_for("ALTUSDT", 1, 100.0));
        context.push_bar(&bar_for("ALTUSDT", 2, 100.0));

        // Cross-symbol reference: ALT close (100) < BTC ema(2) (~103.33) → true.
        let result = context
            .evaluate_expression("ALTUSDT", "close < btcusdt.ema(2)")
            .unwrap();
        assert_eq!(result, Some(true));

        // Same reference, upper-case symbol literal, must match.
        let result_upper = context
            .evaluate_expression("ALTUSDT", "close < BTCUSDT.ema(2)")
            .unwrap();
        assert_eq!(result_upper, Some(true));

        // Cross-symbol OHLC: ALT close (100) < BTC close (104) → true.
        let result_ohlc = context
            .evaluate_expression("ALTUSDT", "close < btcusdt.close")
            .unwrap();
        assert_eq!(result_ohlc, Some(true));
    }

    #[test]
    fn evaluate_expression_cross_symbol_reference_returns_none_when_symbol_missing() {
        // No BTCUSDT bars in context → the indicator lookup yields None (no
        // data to compute ema), and the evaluator returns Ok(None), which the
        // live path treats as entry-suppressing (do not enter). This is the
        // safe default when a referenced symbol has no bars yet.
        let mut context = IndicatorRuntimeContext::default();
        context.push_bar(&bar_for("ALTUSDT", 0, 100.0));
        let result = context
            .evaluate_expression("ALTUSDT", "close < btcusdt.ema(2)")
            .unwrap();
        assert_eq!(result, None);
    }

    // --- P0.2a: extract_symbol_dependencies ---

    use rust_decimal::Decimal;
    use shared_domain::martingale::{
        MartingaleDirection, MartingaleDirectionMode, MartingaleEntryTrigger, MartingaleMarketKind,
        MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel,
        MartingaleSpacingModel, MartingaleStopLossModel, MartingaleStrategyConfig,
        MartingaleTakeProfitModel,
    };

    fn dep_strategy(symbol: &str, entry: &str) -> MartingaleStrategyConfig {
        MartingaleStrategyConfig {
            strategy_id: format!("{symbol}-dep"),
            symbol: symbol.to_string(),
            market: MartingaleMarketKind::Spot,
            direction: MartingaleDirection::Long,
            direction_mode: MartingaleDirectionMode::LongOnly,
            margin_mode: None,
            leverage: None,
            spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
            sizing: MartingaleSizingModel::Multiplier {
                first_order_quote: Decimal::new(10, 0),
                multiplier: Decimal::new(2, 0),
                max_legs: 2,
            },
            take_profit: MartingaleTakeProfitModel::Percent { bps: 100 },
            stop_loss: None,
            indicators: Vec::new(),
            entry_triggers: vec![MartingaleEntryTrigger::IndicatorExpression {
                expression: entry.to_string(),
            }],
            risk_limits: MartingaleRiskLimits::default(),
        }
    }

    fn portfolio_with(strategies: Vec<MartingaleStrategyConfig>) -> MartingalePortfolioConfig {
        MartingalePortfolioConfig {
            direction_mode: MartingaleDirectionMode::LongOnly,
            strategies,
            risk_limits: MartingaleRiskLimits::default(),
        }
    }

    #[test]
    fn extract_dependencies_finds_cross_symbol_indicator_reference() {
        // SOL strategy gates entries on BTC's ema(50): BTCUSDT is a dependency.
        let config = portfolio_with(vec![dep_strategy("SOLUSDT", "close > BTCUSDT.ema(50)")]);
        assert_eq!(extract_symbol_dependencies(&config), vec!["BTCUSDT"]);
    }

    #[test]
    fn extract_dependencies_finds_cross_symbol_ohlc_reference() {
        // Cross-symbol OHLC reference (btcusdt.close) is also a dependency.
        let config = portfolio_with(vec![dep_strategy("SOLUSDT", "close < btcusdt.close")]);
        assert_eq!(extract_symbol_dependencies(&config), vec!["BTCUSDT"]);
    }

    #[test]
    fn extract_dependencies_excludes_strategy_own_symbol() {
        // A strategy referencing its OWN symbol is NOT a dependency.
        let config = portfolio_with(vec![dep_strategy("BTCUSDT", "close > BTCUSDT.ema(50)")]);
        assert!(extract_symbol_dependencies(&config).is_empty());
    }

    #[test]
    fn extract_dependencies_dedups_across_strategies_and_operands() {
        // Two strategies both referencing BTCUSDT (and one also referencing
        // ETHUSDT) → deps = [BTCUSDT, ETHUSDT], deduped and sorted.
        let config = portfolio_with(vec![
            dep_strategy("SOLUSDT", "close > BTCUSDT.ema(50) and 1 < 2 or BTCUSDT.close > 1"),
            dep_strategy("ADAUSDT", "close < ETHUSDT.ema(30)"),
        ]);
        // Note: only the first comparison operand pair is parsed per expression
        // (split_comparison takes the first operator), so the union is BTC + ETH.
        let deps = extract_symbol_dependencies(&config);
        assert_eq!(deps, vec!["BTCUSDT", "ETHUSDT"]);
    }

    #[test]
    fn extract_dependencies_picks_up_indicator_stop_loss_expression() {
        // A strategy with an Indicator stop-loss referencing BTCUSDT.
        let mut strategy = dep_strategy("SOLUSDT", "close > BTCUSDT.ema(50)");
        strategy.stop_loss = Some(MartingaleStopLossModel::Indicator {
            expression: "BTCUSDT.atr_percent(14) > 5.0".to_string(),
        });
        let config = portfolio_with(vec![strategy]);
        // BTCUSDT referenced in both the entry trigger and the stop loss → still
        // one entry after dedup.
        assert_eq!(extract_symbol_dependencies(&config), vec!["BTCUSDT"]);
    }

    #[test]
    fn extract_dependencies_returns_empty_when_no_expressions() {
        // Strategies with only Immediate triggers and no indicator stop → no deps.
        let mut strategy = dep_strategy("SOLUSDT", "close > BTCUSDT.ema(50)");
        strategy.entry_triggers = vec![MartingaleEntryTrigger::Immediate];
        let config = portfolio_with(vec![strategy]);
        assert!(extract_symbol_dependencies(&config).is_empty());
    }
}
