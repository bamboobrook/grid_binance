use crate::indicators::{adx, atr, ema, IndicatorCandle};
use crate::market_data::KlineBar;
use crate::martingale::metrics::MarketRegimeLabel;

#[derive(Debug, Clone, PartialEq)]
pub struct RegimeConfig {
    pub fast_ema_period: usize,
    pub slow_ema_period: usize,
    pub adx_period: usize,
    pub atr_period: usize,
    pub strong_adx: f64,
    pub high_volatility_atr_pct: f64,
    pub slope_bps: f64,
}

impl Default for RegimeConfig {
    fn default() -> Self {
        Self {
            fast_ema_period: 12,
            slow_ema_period: 26,
            adx_period: 14,
            atr_period: 14,
            strong_adx: 25.0,
            high_volatility_atr_pct: 5.0,
            slope_bps: 20.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegimeSnapshot {
    pub timestamp_ms: i64,
    pub label: MarketRegimeLabel,
    pub ema_spread_bps: f64,
    pub adx: f64,
    pub atr_pct: f64,
}

pub fn classify_regime(
    bars: &[KlineBar],
    config: &RegimeConfig,
) -> Result<RegimeSnapshot, String> {
    validate_bars(bars)?;
    let latest = bars.last().expect("validated non-empty bars");
    validate_config(config)?;

    let closes: Vec<f64> = bars.iter().map(|bar| bar.close).collect();
    let candles: Vec<IndicatorCandle> = bars
        .iter()
        .map(|bar| IndicatorCandle {
            high: bar.high,
            low: bar.low,
            close: bar.close,
        })
        .collect();

    let fast_ema = latest_required_indicator(
        &ema(&closes, config.fast_ema_period),
        "fast EMA indicator unavailable: insufficient bars or invalid latest value",
    )?;
    let slow_ema = latest_required_indicator(
        &ema(&closes, config.slow_ema_period),
        "slow EMA indicator unavailable: insufficient bars or invalid latest value",
    )?;
    let adx_value = latest_required_indicator(
        &adx(&candles, config.adx_period),
        "ADX indicator unavailable: insufficient bars or invalid latest value",
    )?;
    let atr_value = latest_required_indicator(
        &atr(&candles, config.atr_period),
        "ATR indicator unavailable: insufficient bars or invalid latest value",
    )?;

    if latest.close == 0.0 || !latest.close.is_finite() || !slow_ema.is_finite() || slow_ema == 0.0
    {
        return Err("latest close and slow EMA must be finite non-zero values".to_string());
    }

    let ema_spread_bps = ((fast_ema - slow_ema) / slow_ema) * 10_000.0;
    let atr_pct = (atr_value / latest.close) * 100.0;

    if !ema_spread_bps.is_finite() || !atr_pct.is_finite() || !adx_value.is_finite() {
        return Err("regime indicators must be finite".to_string());
    }

    let label = if atr_pct >= config.high_volatility_atr_pct {
        MarketRegimeLabel::HighVolatility
    } else if ema_spread_bps >= config.slope_bps && adx_value >= config.strong_adx {
        MarketRegimeLabel::StrongUptrend
    } else if ema_spread_bps <= -config.slope_bps && adx_value >= config.strong_adx {
        MarketRegimeLabel::StrongDowntrend
    } else if ema_spread_bps >= config.slope_bps {
        MarketRegimeLabel::Uptrend
    } else if ema_spread_bps <= -config.slope_bps {
        MarketRegimeLabel::Downtrend
    } else {
        MarketRegimeLabel::Range
    };

    Ok(RegimeSnapshot {
        timestamp_ms: latest.open_time_ms,
        label,
        ema_spread_bps,
        adx: adx_value,
        atr_pct,
    })
}

fn latest_required_indicator(values: &[Option<f64>], message: &str) -> Result<f64, String> {
    values
        .last()
        .copied()
        .flatten()
        .filter(|value| value.is_finite())
        .ok_or_else(|| message.to_string())
}

fn validate_bars(bars: &[KlineBar]) -> Result<(), String> {
    if bars.is_empty() {
        return Err("bars must not be empty".to_string());
    }

    for (index, bar) in bars.iter().enumerate() {
        validate_price(bar.open, index, "open")?;
        validate_price(bar.high, index, "high")?;
        validate_price(bar.low, index, "low")?;
        validate_price(bar.close, index, "close")?;

        if bar.high < bar.low {
            return Err(format!("bar {index} high must be greater than or equal to low"));
        }
    }

    Ok(())
}

fn validate_price(value: f64, index: usize, field: &str) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!("bar {index} {field} must be finite and greater than zero"))
    }
}

fn validate_config(config: &RegimeConfig) -> Result<(), String> {
    if config.fast_ema_period == 0
        || config.slow_ema_period == 0
        || config.adx_period == 0
        || config.atr_period == 0
    {
        return Err("indicator periods must be greater than zero".to_string());
    }

    if !config.strong_adx.is_finite()
        || !config.high_volatility_atr_pct.is_finite()
        || !config.slope_bps.is_finite()
    {
        return Err("regime thresholds must be finite".to_string());
    }

    Ok(())
}
