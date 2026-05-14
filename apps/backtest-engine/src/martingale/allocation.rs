use crate::martingale::metrics::{
    AllocationAction, AllocationCurvePoint, MarketRegimeLabel,
};

const HOUR_MS: i64 = 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq)]
pub struct AllocationConfig {
    pub cooldown_hours: i64,
    pub forced_exit_loss_pct: f64,
}

impl AllocationConfig {
    pub fn conservative() -> Self {
        Self {
            cooldown_hours: 24,
            forced_exit_loss_pct: 20.0,
        }
    }

    pub fn balanced() -> Self {
        Self {
            cooldown_hours: 16,
            forced_exit_loss_pct: 25.0,
        }
    }

    pub fn aggressive() -> Self {
        Self {
            cooldown_hours: 12,
            forced_exit_loss_pct: 30.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AllocationState {
    pub last_change_ms: Option<i64>,
    pub long_weight_pct: f64,
    pub short_weight_pct: f64,
}

impl Default for AllocationState {
    fn default() -> Self {
        Self {
            last_change_ms: None,
            long_weight_pct: 60.0,
            short_weight_pct: 40.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AllocationDecision {
    pub point: AllocationCurvePoint,
    pub long_weight_pct: f64,
    pub short_weight_pct: f64,
    pub action: AllocationAction,
    pub force_exit_long: bool,
    pub force_exit_short: bool,
    pub in_cooldown: bool,
}

pub fn decide_allocation(
    timestamp_ms: i64,
    symbol: &str,
    btc_regime: MarketRegimeLabel,
    symbol_regime: MarketRegimeLabel,
    adverse_direction_loss_pct: f64,
    config: &AllocationConfig,
    state: &AllocationState,
) -> AllocationDecision {
    let (target_long_weight_pct, target_short_weight_pct, base_action, regime_reason) =
        target_weights(&btc_regime, &symbol_regime);
    let extreme_strong_up = btc_regime == MarketRegimeLabel::StrongUptrend
        && symbol_regime == MarketRegimeLabel::StrongUptrend;
    let extreme_strong_down = btc_regime == MarketRegimeLabel::StrongDowntrend
        && symbol_regime == MarketRegimeLabel::StrongDowntrend;
    let loss_forced_exit = adverse_direction_loss_pct >= config.forced_exit_loss_pct;
    let extreme_risk = extreme_strong_up || extreme_strong_down || loss_forced_exit;
    let force_exit_short = extreme_strong_up && target_short_weight_pct == 0.0;
    let force_exit_long = extreme_strong_down && target_long_weight_pct == 0.0;

    let cooldown_ms = config.cooldown_hours * HOUR_MS;
    let in_cooldown = state
        .last_change_ms
        .map(|last_change_ms| timestamp_ms.saturating_sub(last_change_ms) < cooldown_ms)
        .unwrap_or(false)
        && !extreme_risk;

    let (long_weight_pct, short_weight_pct, action, reason) = if in_cooldown {
        (
            state.long_weight_pct,
            state.short_weight_pct,
            AllocationAction::None,
            format!(
                "cooldown active: keeping {:.1}/{:.1} instead of {:.1}/{:.1}",
                state.long_weight_pct,
                state.short_weight_pct,
                target_long_weight_pct,
                target_short_weight_pct
            ),
        )
    } else {
        let action = if force_exit_long || force_exit_short || loss_forced_exit {
            AllocationAction::DirectionForcedExit
        } else if base_action == AllocationAction::DirectionPaused {
            AllocationAction::DirectionPaused
        } else if weights_changed(
            state.long_weight_pct,
            state.short_weight_pct,
            target_long_weight_pct,
            target_short_weight_pct,
        ) || state.last_change_ms.is_none()
        {
            AllocationAction::Rebalance
        } else {
            AllocationAction::None
        };

        let reason = if loss_forced_exit {
            format!(
                "forced exit loss {:.2}% reached threshold {:.2}%: {regime_reason}",
                adverse_direction_loss_pct, config.forced_exit_loss_pct
            )
        } else {
            regime_reason
        };

        (
            target_long_weight_pct,
            target_short_weight_pct,
            action,
            reason,
        )
    };

    AllocationDecision {
        point: AllocationCurvePoint {
            timestamp_ms,
            symbol: symbol.to_string(),
            long_weight_pct,
            short_weight_pct,
            action: action.clone(),
            reason,
            in_cooldown,
        },
        long_weight_pct,
        short_weight_pct,
        action,
        force_exit_long,
        force_exit_short,
        in_cooldown,
    }
}

fn target_weights(
    btc_regime: &MarketRegimeLabel,
    symbol_regime: &MarketRegimeLabel,
) -> (f64, f64, AllocationAction, String) {
    if *btc_regime == MarketRegimeLabel::StrongUptrend
        && *symbol_regime == MarketRegimeLabel::StrongUptrend
    {
        return (
            100.0,
            0.0,
            AllocationAction::DirectionForcedExit,
            "btc and symbol are both strong uptrend".to_string(),
        );
    }

    if *btc_regime == MarketRegimeLabel::StrongDowntrend
        && *symbol_regime == MarketRegimeLabel::StrongDowntrend
    {
        return (
            0.0,
            100.0,
            AllocationAction::DirectionForcedExit,
            "btc and symbol are both strong downtrend".to_string(),
        );
    }

    if *btc_regime == MarketRegimeLabel::HighVolatility
        || *symbol_regime == MarketRegimeLabel::HighVolatility
    {
        return (
            60.0,
            40.0,
            AllocationAction::DirectionPaused,
            "high volatility pauses directional expansion".to_string(),
        );
    }

    if *btc_regime == MarketRegimeLabel::Uptrend
        && *symbol_regime == MarketRegimeLabel::Uptrend
    {
        return (
            80.0,
            20.0,
            AllocationAction::Rebalance,
            "btc and symbol are both uptrend".to_string(),
        );
    }

    if *btc_regime == MarketRegimeLabel::Downtrend
        && *symbol_regime == MarketRegimeLabel::Downtrend
    {
        return (
            20.0,
            80.0,
            AllocationAction::Rebalance,
            "btc and symbol are both downtrend".to_string(),
        );
    }

    (
        60.0,
        40.0,
        AllocationAction::None,
        "range or mixed regime keeps balanced allocation".to_string(),
    )
}

fn weights_changed(
    current_long: f64,
    current_short: f64,
    target_long: f64,
    target_short: f64,
) -> bool {
    (current_long - target_long).abs() > f64::EPSILON
        || (current_short - target_short).abs() > f64::EPSILON
}
