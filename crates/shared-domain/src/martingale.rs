use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleMarketKind {
    Spot,
    UsdMFutures,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleDirection {
    Long,
    Short,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleDirectionMode {
    LongOnly,
    ShortOnly,
    LongAndShort,
    IndicatorSelected,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleMarginMode {
    Isolated,
    Cross,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleSpacingModel {
    FixedPercent {
        step_bps: u32,
    },
    Multiplier {
        first_step_bps: u32,
        multiplier: Decimal,
    },
    Atr {
        multiplier: Decimal,
        min_step_bps: u32,
        max_step_bps: u32,
    },
    CustomSequence {
        steps_bps: Vec<u32>,
    },
    Mixed {
        phases: Vec<MartingaleSpacingModel>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleSizingModel {
    Multiplier {
        first_order_quote: Decimal,
        multiplier: Decimal,
        max_legs: u32,
    },
    CustomSequence {
        notionals: Vec<Decimal>,
    },
    BudgetScaled {
        first_order_quote: Decimal,
        multiplier: Decimal,
        max_legs: u32,
        max_budget_quote: Decimal,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleTakeProfitModel {
    Percent {
        bps: u32,
    },
    Amount {
        quote: Decimal,
    },
    Atr {
        multiplier: Decimal,
    },
    Trailing {
        activation_bps: u32,
        callback_bps: u32,
    },
    Mixed {
        phases: Vec<MartingaleTakeProfitModel>,
    },
    /// Partial TP ladder with breakeven stop migration (Round 2 Direction A).
    /// Closes a FRACTION of the position at each TP stage (not the whole
    /// position), advances to the next stage, and after `breakeven_after_stage`
    /// migrates the stop to weighted-average-entry + `breakeven_buffer_bps`.
    /// Unused safety orders can be canceled after the configured stage.
    /// `stages` are (close_fraction_num/close_fraction_den, tp_bps) pairs;
    /// fractions are applied to the REMAINING position at each stage, so the
    /// final stage closes whatever remains.
    Partial {
        /// TP stage definitions: (numerator, denominator, tp_bps). The fraction
        /// `num/den` of the REMAINING position is closed at each stage's tp_bps
        /// from the weighted average entry. The last stage closes the rest.
        stages: Vec<(u32, u32, u32)>,
        /// After this stage index (0-based) fires, the stop migrates to
        /// breakeven (avg entry + buffer). Set to a large value to disable.
        breakeven_after_stage: u32,
        /// Breakeven buffer in bps added to avg entry (covers fees).
        breakeven_buffer_bps: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleStopLossModel {
    PriceRange { lower: Decimal, upper: Decimal },
    Atr { multiplier: Decimal },
    Indicator { expression: String },
    StrategyDrawdownPct { pct_bps: u32 },
    SymbolDrawdownAmount { quote: Decimal },
    GlobalDrawdownAmount { quote: Decimal },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleIndicatorConfig {
    Atr {
        period: u32,
    },
    Sma {
        period: u32,
    },
    Ema {
        period: u32,
    },
    Rsi {
        period: u32,
        overbought: Decimal,
        oversold: Decimal,
    },
    Bollinger {
        period: u32,
        std_dev: Decimal,
    },
    Adx {
        period: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleEntryTrigger {
    Immediate,
    IndicatorExpression { expression: String },
    PriceRange { lower: Decimal, upper: Decimal },
    TimeWindow { start: String, end: String },
    Cooldown { seconds: u64 },
    Capacity { max_active_cycles: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MartingaleRiskLimits {
    pub max_active_cycles: Option<u32>,
    #[serde(alias = "max_budget_quote")]
    pub max_global_budget_quote: Option<Decimal>,
    #[serde(alias = "max_symbol_exposure_quote")]
    pub max_symbol_budget_quote: Option<Decimal>,
    pub max_direction_budget_quote: Option<Decimal>,
    pub max_strategy_budget_quote: Option<Decimal>,
    pub max_global_drawdown_quote: Option<Decimal>,
    /// Pause opening new cycles when portfolio drawdown (peak→current) exceeds
    /// this percent. `None` = use engine default (6.0%). Parity-structured
    /// replacement for the `MARTINGALE_BT_NEW_CYCLE_DD_PAUSE_PCT` research env.
    #[serde(default)]
    pub new_cycle_drawdown_pause_pct: Option<f64>,
    /// Pause opening new cycles when ATR/close*100 exceeds this percent.
    /// `None` = use engine default (2.0%). Parity-structured replacement for
    /// the `MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT` research env.
    #[serde(default)]
    pub new_cycle_atr_pause_pct: Option<f64>,
    /// Skip averaging-down (safety) legs when ADX exceeds this value.
    /// `None` = use engine default (45.0). Parity-structured replacement for
    /// the `MARTINGALE_BT_SAFETY_SKIP_ADX` research env.
    #[serde(default)]
    pub safety_skip_adx_threshold: Option<f64>,
    /// When portfolio drawdown (peak→current equity) reaches this percent,
    /// close ALL active martingale cycles via reduceOnly and enter a cooldown
    /// (`portfolio_stop_cooldown_hours`). `None`/`0.0` = disabled (default).
    /// Parity-structured replacement for the
    /// `MARTINGALE_BT_PORTFOLIO_EQUITY_STOP_PCT` research env switch.
    #[serde(default)]
    pub portfolio_equity_stop_pct: Option<f64>,
    /// Cooldown duration (hours) after a portfolio equity stop fires, during
    /// which no new cycles may open. `None`/`0.0` = no cooldown (default).
    /// Parity-structured replacement for the
    /// `MARTINGALE_BT_PORTFOLIO_STOP_COOLDOWN_HOURS` research env switch.
    #[serde(default)]
    pub portfolio_stop_cooldown_hours: Option<f64>,
    /// Round 2 Direction B: an optional indicator expression that must evaluate
    /// TRUE before a safety (averaging) order is added, in addition to the
    /// price-deviation trigger. `None`/empty = safety orders trigger on
    /// deviation only (engine default). Example: `"rsi(14) < 45"` only adds
    /// safety when RSI confirms oversold.
    #[serde(default)]
    pub safety_order_condition: Option<String>,
    /// Round 2 Direction F: when set (0.0-1.0), the portfolio equity-stop
    /// cooldown ends EARLY once portfolio equity recovers this fraction of the
    /// drawdown that triggered the stop. E.g. 0.5 = re-enter when equity
    /// recovers half the stopped drawdown. `None`/0.0 = use the full calendar
    /// cooldown (engine default).
    #[serde(default)]
    pub reentry_equity_reclaim_fraction: Option<f64>,
    /// Round 4 P4: maximum age of an active cycle in hours. If a cycle is
    /// older than this, it is force-closed at the current price. `None`/0.0 =
    /// disabled (cycles can live indefinitely).
    #[serde(default)]
    pub max_cycle_age_hours: Option<f64>,
    /// Round 4 P4: if a cycle has not reached this much favorable excursion
    /// (in bps from average entry) within `no_progress_exit_hours`, it is
    /// force-closed. `None` = disabled.
    #[serde(default)]
    pub no_progress_exit_hours: Option<f64>,
    /// Round 4 P4: MFE threshold in bps for the no-progress exit.
    #[serde(default)]
    pub no_progress_mfe_bps: Option<u32>,
    /// Round 4 P5: Rebound confirmation in bps. When set, a safety (averaging)
    /// order is only placed after price deviates to the trigger level AND then
    /// rebounds by this many bps from the local extreme since the trigger was
    /// reached. `None`/0 = no rebound confirmation (place immediately on
    /// deviation, existing behavior).
    #[serde(default)]
    pub safety_order_rebound_bps: Option<u32>,
    /// Round 5 Task B: Safety order deviation basis. `BaseOrder` = current
    /// behavior (triggers measured from base order price). `LastExecutedOrder`
    /// = next safety trigger measured from the last filled leg price, so
    /// successive safety orders are closer together in choppy markets.
    #[serde(default)]
    pub safety_order_basis: Option<MartingaleSafetyOrderBasis>,
    /// Round 5 Task C: After N consecutive losing cycles, reduce first_order_quote
    /// by this percentage for subsequent cycles until recovery wins. `None`/0 = disabled.
    #[serde(default)]
    pub loss_streak_risk_reduction_pct: Option<f64>,
    /// Round 5 Task C: Number of consecutive losing cycles to trigger risk reduction.
    #[serde(default)]
    pub loss_streak_trigger_count: Option<u32>,
    /// Round 5 Task C: Number of winning cycles to recover from risk reduction.
    #[serde(default)]
    pub loss_streak_recovery_win_count: Option<u32>,
    /// Round 5 Task D: Volatility-targeted ATR percent. When set, first order
    /// size is scaled by (target_atr_pct / current_atr_pct), clamped to
    /// [vol_target_min_scale, vol_target_max_scale]. This reduces exposure in
    /// high-volatility regimes and increases it in low-volatility regimes.
    #[serde(default)]
    pub vol_target_atr_pct: Option<f64>,
    /// Round 5 Task D: Minimum scale factor for vol targeting.
    #[serde(default)]
    pub vol_target_min_scale: Option<f64>,
    /// Round 5 Task D: Maximum scale factor for vol targeting.
    #[serde(default)]
    pub vol_target_max_scale: Option<f64>,
    /// Round 6 Task B: Staged portfolio drawdown state machine rules.
    /// When portfolio DD exceeds a rule's trigger, that rule's actions apply
    /// (scale entries, freeze safety orders, extend cooldown). Higher DD
    /// rules override lower ones. Recovery requires reclaiming
    /// `drawdown_state_recovery_pct` of peak-to-trough loss.
    #[serde(default)]
    pub drawdown_state_rules: Vec<MartingaleDrawdownStateRule>,
    /// Round 6 Task B: Recovery reclaim fraction for DD state machine.
    #[serde(default)]
    pub drawdown_state_recovery_pct: Option<f64>,
    /// Round 6 Task D: Arm trailing profit lock after this partial TP stage.
    #[serde(default)]
    pub trailing_lock_after_stage: Option<u32>,
    /// Round 6 Task D: Trailing lock activation in bps from avg entry.
    #[serde(default)]
    pub trailing_lock_activation_bps: Option<u32>,
    /// Round 6 Task D: Trailing lock callback in bps.
    #[serde(default)]
    pub trailing_lock_callback_bps: Option<u32>,
    /// Round 6 Task D: Trailing lock floor in bps (cannot be worse than BE + floor).
    #[serde(default)]
    pub trailing_lock_floor_bps: Option<u32>,
    /// Round 6 Task E: Freeze safety orders after this partial TP stage fires.
    #[serde(default)]
    pub freeze_safety_after_partial_tp_stage: Option<u32>,
    /// Round 6 Task C: Quarantine a symbol/direction after N stop-losses in
    /// a rolling window. When quarantined, no new cycles open for that scope.
    #[serde(default)]
    pub quarantine_stop_count_trigger: Option<u32>,
    /// Round 6 Task C: Rolling window in hours for stop-count quarantine.
    #[serde(default)]
    pub quarantine_stop_window_hours: Option<f64>,
    /// Round 6 Task C: Pause duration in hours after quarantine triggers.
    #[serde(default)]
    pub quarantine_pause_hours: Option<f64>,
    /// Round 7 Task C: Block new cycle if expected funding cost in the lookback
    /// window exceeds this many bps. None = disabled.
    #[serde(default)]
    pub max_expected_funding_cost_bps: Option<f64>,
    /// Round 7 Task C: Mode for funding/fee cost gate.
    /// "entry_only" = block new cycles, "entry_and_safety" = also block safety orders.
    #[serde(default)]
    pub funding_side_bias_mode: Option<String>,
    /// Round 7 Task F: Taper safety orders after this leg index (scale by taper_safety_scale).
    #[serde(default)]
    pub taper_safety_after_leg: Option<u32>,
    /// Round 7 Task F: Scale factor for tapered safety orders.
    #[serde(default)]
    pub taper_safety_scale: Option<f64>,
    /// Round 11 Task P3: Native inventory-reducing DCA minigrid config.
    /// When present, minigrid levels open between safety order fills and
    /// reduce inventory only (never add exposure).
    #[serde(default)]
    pub dca_minigrid: Option<MartingaleDcaMiniGridConfig>,
    /// Round 13 P5: Depth-dependent TP config. When present, the TP target
    /// adapts based on the number of filled safety legs (cycle depth).
    #[serde(default)]
    pub depth_tp: Option<MartingaleDepthTpConfig>,
    /// Round 14 P2: Enable completed-HTF regime gate. When true, new cycle
    /// entry is blocked if the per-symbol 1h/4h completed-bar regime state
    /// doesn't allow the cycle direction. State never affects existing cycles.
    #[serde(default)]
    pub htf_regime_gate_enabled: Option<bool>,
    /// Round 14 P3: Enable cross-sectional selector gate. When true, new cycle
    /// entry is blocked if the symbol is not in the active set for this direction.
    #[serde(default)]
    pub xs_selector_gate_enabled: Option<bool>,
    /// Round 14 P3: Cross-sectional selector configuration.
    #[serde(default)]
    pub xs_selector_config: Option<MartingaleXsSelectorConfig>,
}

/// Round 14 P3: Cross-sectional selector configuration.
/// Controls which symbols can open new Martingale cycles based on
/// lagged cross-sectional momentum or reversal ranking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingaleXsSelectorConfig {
    /// "momentum" or "reversal".
    #[serde(default)]
    pub family: String,
    /// Lookback window in days for momentum, in hours for reversal.
    pub lookback_periods: usize,
    /// Skip recent N periods before computing score.
    #[serde(default)]
    pub skip_recent_periods: usize,
    /// Rebalance frequency in 1m bars.
    #[serde(default = "default_rebalance_period")]
    pub rebalance_period_bars: usize,
    /// Number of top-ranked symbols to activate for long.
    #[serde(default = "default_active_count")]
    pub active_long_count: usize,
    /// Number of bottom-ranked symbols to activate for short.
    #[serde(default = "default_active_count")]
    pub active_short_count: usize,
    /// Single symbol capital cap (fraction of budget, 0-1).
    #[serde(default = "default_symbol_cap")]
    pub symbol_cap: f64,
    /// Cluster cap (fraction of budget, 0-1).
    #[serde(default = "default_cluster_cap")]
    pub cluster_cap: f64,
    /// Minimum symbols that must actually trade.
    #[serde(default = "default_min_symbols")]
    pub min_active_symbols: usize,
}

fn default_rebalance_period() -> usize {
    7 * 24 * 60
}
fn default_active_count() -> usize {
    3
}
fn default_symbol_cap() -> f64 {
    0.25
}
fn default_cluster_cap() -> f64 {
    0.35
}
fn default_min_symbols() -> usize {
    5
}

impl Default for MartingaleXsSelectorConfig {
    fn default() -> Self {
        Self {
            family: "momentum".to_string(),
            lookback_periods: 14,
            skip_recent_periods: 1,
            rebalance_period_bars: default_rebalance_period(),
            active_long_count: default_active_count(),
            active_short_count: default_active_count(),
            symbol_cap: default_symbol_cap(),
            cluster_cap: default_cluster_cap(),
            min_active_symbols: default_min_symbols(),
        }
    }
}

/// Round 6 Task B: A single drawdown state rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingaleDrawdownStateRule {
    pub trigger_drawdown_pct: f64,
    #[serde(default)]
    pub first_order_scale: Option<f64>,
    #[serde(default)]
    pub safety_order_scale: Option<f64>,
    #[serde(default)]
    pub cooldown_multiplier: Option<f64>,
    #[serde(default)]
    pub freeze_safety_orders: Option<bool>,
}

/// Round 11 Task P3: Native inventory-reducing DCA minigrid config.
/// A minigrid opens local reduce-only take-profit levels between safety
/// order fills. It can only reduce inventory (close fractions of existing
/// legs); it never adds new exposure or resets the cycle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingaleDcaMiniGridConfig {
    /// Number of minigrid levels per band (1..=5).
    pub levels_per_band: u32,
    /// Spacing between minigrid levels in bps (10..=150).
    pub spacing_bps: u32,
    /// Numerator of the close fraction (close_fraction_num/close_fraction_den).
    pub close_fraction_num: u32,
    /// Denominator of the close fraction (>= close_fraction_num).
    pub close_fraction_den: u32,
    /// Minimum profit in bps from last safety fill to place a minigrid level (>= 5).
    pub min_profit_bps: u32,
    /// Maximum concurrent active minigrid levels (1..=5).
    pub max_active_levels: u32,
}

impl MartingaleDcaMiniGridConfig {
    /// Validate minigrid config per plan rules.
    pub fn validate(&self) -> Result<(), String> {
        if self.levels_per_band < 1 || self.levels_per_band > 5 {
            return Err(format!(
                "levels_per_band must be in 1..=5, got {}",
                self.levels_per_band
            ));
        }
        if self.spacing_bps < 10 || self.spacing_bps > 150 {
            return Err(format!(
                "spacing_bps must be in 10..=150, got {}",
                self.spacing_bps
            ));
        }
        if self.close_fraction_num == 0 {
            return Err("close_fraction_num must be > 0".to_string());
        }
        if self.close_fraction_den < self.close_fraction_num {
            return Err(format!(
                "close_fraction_den ({}) must be >= close_fraction_num ({})",
                self.close_fraction_den, self.close_fraction_num
            ));
        }
        if self.min_profit_bps < 5 {
            return Err(format!(
                "min_profit_bps must be >= 5, got {}",
                self.min_profit_bps
            ));
        }
        if self.max_active_levels < 1 || self.max_active_levels > 5 {
            return Err(format!(
                "max_active_levels must be in 1..=5, got {}",
                self.max_active_levels
            ));
        }
        Ok(())
    }

    /// Compute the minigrid level price for a given level index, direction, and
    /// last safety fill price.
    /// Long: price = last_fill * (1 + (min_profit + idx*spacing) / 10000)
    /// Short: price = last_fill * (1 - (min_profit + idx*spacing) / 10000)
    pub fn level_price(&self, last_safety_fill_price: f64, level_index: u32, is_long: bool) -> f64 {
        let offset_bps = self.min_profit_bps + level_index * self.spacing_bps;
        let factor = 1.0 + (offset_bps as f64) / 10_000.0;
        if is_long {
            last_safety_fill_price * factor
        } else {
            last_safety_fill_price * (2.0 - factor)
        }
    }

    /// The close fraction as a float (num/den).
    pub fn close_fraction(&self) -> f64 {
        self.close_fraction_num as f64 / self.close_fraction_den as f64
    }
}

/// Round 13 Task P5: Depth-dependent TP config.
/// Adapts TP target based on the number of filled safety legs (cycle depth).
/// Depth 0-1 = normal TP, depth 2-3 = lower TP + optional partial reduce,
/// depth 4+ = lowest TP + larger partial reduce + breakeven.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingaleDepthTpConfig {
    /// TP bps for depth 0-1 (1-2 legs total including base).
    pub depth_01_tp_bps: u32,
    /// TP bps for depth 2-3 (3-4 legs total).
    pub depth_23_tp_bps: u32,
    /// Partial reduce fraction for depth 2-3 (0 = no reduce, e.g. 25 = 25%).
    pub depth_23_reduce_pct: u32,
    /// TP bps for depth 4+ (5+ legs total).
    pub depth_4plus_tp_bps: u32,
    /// Partial reduce fraction for depth 4+ (e.g. 50 = 50%).
    pub depth_4plus_reduce_pct: u32,
}

impl MartingaleDepthTpConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.depth_01_tp_bps == 0 || self.depth_23_tp_bps == 0 || self.depth_4plus_tp_bps == 0 {
            return Err("depth TP targets must be greater than 0 bps".to_string());
        }
        if self.depth_23_reduce_pct > 100 || self.depth_4plus_reduce_pct > 100 {
            return Err("depth TP reduce percentages must be in 0..=100".to_string());
        }
        Ok(())
    }

    /// Get the effective TP bps for a given number of filled legs.
    /// filled_legs includes the base order (leg 0), so depth = filled_legs - 1.
    pub fn tp_bps_for_depth(&self, filled_legs: usize) -> u32 {
        if filled_legs <= 2 {
            self.depth_01_tp_bps
        } else if filled_legs <= 4 {
            self.depth_23_tp_bps
        } else {
            self.depth_4plus_tp_bps
        }
    }

    /// Get the partial reduce fraction for the current depth.
    pub fn reduce_fraction_for_depth(&self, filled_legs: usize) -> f64 {
        if filled_legs <= 2 {
            0.0
        } else if filled_legs <= 4 {
            self.depth_23_reduce_pct as f64 / 100.0
        } else {
            self.depth_4plus_reduce_pct as f64 / 100.0
        }
    }
}

/// Round 5 Task B: Safety order trigger basis.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleSafetyOrderBasis {
    #[default]
    BaseOrder,
    LastExecutedOrder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingaleStrategyConfig {
    pub strategy_id: String,
    pub symbol: String,
    pub market: MartingaleMarketKind,
    pub direction: MartingaleDirection,
    pub direction_mode: MartingaleDirectionMode,
    pub margin_mode: Option<MartingaleMarginMode>,
    pub leverage: Option<u32>,
    pub spacing: MartingaleSpacingModel,
    pub sizing: MartingaleSizingModel,
    pub take_profit: MartingaleTakeProfitModel,
    pub stop_loss: Option<MartingaleStopLossModel>,
    pub indicators: Vec<MartingaleIndicatorConfig>,
    pub entry_triggers: Vec<MartingaleEntryTrigger>,
    pub risk_limits: MartingaleRiskLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingalePortfolioConfig {
    pub direction_mode: MartingaleDirectionMode,
    pub strategies: Vec<MartingaleStrategyConfig>,
    pub risk_limits: MartingaleRiskLimits,
}

impl MartingaleStrategyConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.symbol.trim().is_empty() {
            return Err("symbol cannot be empty".to_string());
        }
        if self.symbol.trim() != self.symbol {
            return Err(format!(
                "symbol {} cannot contain outer whitespace",
                self.symbol
            ));
        }

        match self.market {
            MartingaleMarketKind::Spot => {
                if self.margin_mode.is_some() || self.leverage.is_some() {
                    return Err("spot strategy cannot use margin_mode or leverage".to_string());
                }
            }
            MartingaleMarketKind::UsdMFutures => {
                if self.margin_mode.is_none() {
                    return Err(format!(
                        "USDT-M futures strategy {} requires margin_mode",
                        self.symbol
                    ));
                }
                match self.leverage {
                    Some(0) => return Err(format!("{} leverage cannot be 0", self.symbol)),
                    Some(_) => {}
                    None => {
                        return Err(format!(
                            "USDT-M futures strategy {} requires leverage",
                            self.symbol
                        ));
                    }
                }
            }
        }

        if let Some(config) = &self.risk_limits.dca_minigrid {
            config.validate()?;
        }
        if let Some(config) = &self.risk_limits.depth_tp {
            config.validate()?;
        }

        Ok(())
    }
}

impl MartingalePortfolioConfig {
    pub fn validate(&self) -> Result<(), String> {
        use std::collections::HashMap;

        let mut futures_by_symbol: HashMap<String, MartingaleMarginMode> = HashMap::new();

        for strategy in &self.strategies {
            strategy.validate()?;

            if strategy.market == MartingaleMarketKind::UsdMFutures {
                let margin_mode = strategy
                    .margin_mode
                    .expect("validated futures strategy must have margin_mode");
                let symbol_key = strategy.symbol.trim().to_uppercase();
                if let Some(existing_margin_mode) = futures_by_symbol.get(&symbol_key) {
                    if *existing_margin_mode != margin_mode {
                        return Err(format!(
                            "{} margin_mode conflict for USDT-M futures strategies",
                            strategy.symbol
                        ));
                    }
                } else {
                    futures_by_symbol.insert(symbol_key, margin_mode);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl MartingaleStrategyConfig {
        fn example_spot_long(symbol: &str) -> Self {
            Self {
                strategy_id: format!("{symbol}-spot-long"),
                symbol: symbol.to_string(),
                market: MartingaleMarketKind::Spot,
                direction: MartingaleDirection::Long,
                direction_mode: MartingaleDirectionMode::LongOnly,
                margin_mode: None,
                leverage: None,
                spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
                sizing: MartingaleSizingModel::Multiplier {
                    first_order_quote: Decimal::new(100, 0),
                    multiplier: Decimal::new(2, 0),
                    max_legs: 5,
                },
                take_profit: MartingaleTakeProfitModel::Percent { bps: 100 },
                stop_loss: None,
                indicators: Vec::new(),
                entry_triggers: vec![MartingaleEntryTrigger::Immediate],
                risk_limits: MartingaleRiskLimits::default(),
            }
        }
    }

    impl MartingalePortfolioConfig {
        fn example_futures_long_short(symbol: &str) -> Self {
            let base = MartingaleStrategyConfig {
                strategy_id: format!("{symbol}-futures-long"),
                symbol: symbol.to_string(),
                market: MartingaleMarketKind::UsdMFutures,
                direction: MartingaleDirection::Long,
                direction_mode: MartingaleDirectionMode::LongAndShort,
                margin_mode: Some(MartingaleMarginMode::Cross),
                leverage: Some(3),
                spacing: MartingaleSpacingModel::Multiplier {
                    first_step_bps: 100,
                    multiplier: Decimal::new(12, 1),
                },
                sizing: MartingaleSizingModel::BudgetScaled {
                    first_order_quote: Decimal::new(100, 0),
                    multiplier: Decimal::new(2, 0),
                    max_legs: 6,
                    max_budget_quote: Decimal::new(10_000, 0),
                },
                take_profit: MartingaleTakeProfitModel::Percent { bps: 80 },
                stop_loss: Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps: 2_000 }),
                indicators: Vec::new(),
                entry_triggers: vec![MartingaleEntryTrigger::Immediate],
                risk_limits: MartingaleRiskLimits::default(),
            };

            let short = MartingaleStrategyConfig {
                strategy_id: format!("{symbol}-futures-short"),
                direction: MartingaleDirection::Short,
                ..base.clone()
            };

            Self {
                direction_mode: MartingaleDirectionMode::LongAndShort,
                strategies: vec![base, short],
                risk_limits: MartingaleRiskLimits::default(),
            }
        }
    }

    #[test]
    fn futures_long_short_portfolio_round_trips() {
        let portfolio = MartingalePortfolioConfig::example_futures_long_short("BTCUSDT");
        let encoded = serde_json::to_string(&portfolio).expect("serialize portfolio");
        assert!(encoded.contains("BTCUSDT"));
        assert!(encoded.contains("long_and_short"));
        let decoded: MartingalePortfolioConfig =
            serde_json::from_str(&encoded).expect("deserialize portfolio");
        assert_eq!(decoded.strategies.len(), 2);
        assert_eq!(decoded.validate().unwrap(), ());
    }

    #[test]
    fn same_symbol_futures_leverage_difference_is_allowed() {
        let mut portfolio = MartingalePortfolioConfig::example_futures_long_short("BTCUSDT");
        portfolio.strategies[1].leverage = Some(5);
        assert_eq!(portfolio.validate().unwrap(), ());
    }

    #[test]
    fn same_symbol_futures_margin_mode_conflict_is_rejected() {
        let mut portfolio = MartingalePortfolioConfig::example_futures_long_short("BTCUSDT");
        portfolio.strategies[1].margin_mode = Some(MartingaleMarginMode::Isolated);
        let error = portfolio
            .validate()
            .expect_err("conflicting margin mode must fail");
        assert!(error.contains("BTCUSDT"));
        assert!(error.contains("margin_mode"));
    }

    #[test]
    fn spot_rejects_futures_only_fields() {
        let mut strategy = MartingaleStrategyConfig::example_spot_long("ETHUSDT");
        strategy.margin_mode = Some(MartingaleMarginMode::Isolated);
        strategy.leverage = Some(2);
        let error = strategy
            .validate()
            .expect_err("spot cannot use futures fields");
        assert!(error.contains("spot"));
    }

    #[test]
    fn symbol_with_outer_whitespace_is_rejected() {
        let strategy = MartingaleStrategyConfig::example_spot_long(" ETHUSDT ");
        let error = strategy
            .validate()
            .expect_err("symbol with outer whitespace must fail");
        assert!(error.contains("symbol"));
    }

    #[test]
    fn risk_limits_support_canonical_budget_field_names() {
        let json = r#"{
            "max_global_budget_quote":"1000",
            "max_symbol_budget_quote":"500",
            "max_direction_budget_quote":"400",
            "max_strategy_budget_quote":"300",
            "max_global_drawdown_quote":"50"
        }"#;

        let limits: MartingaleRiskLimits = serde_json::from_str(json).unwrap();

        assert_eq!(limits.max_global_budget_quote, Some(Decimal::new(1000, 0)));
        assert_eq!(limits.max_symbol_budget_quote, Some(Decimal::new(500, 0)));
        assert_eq!(
            limits.max_direction_budget_quote,
            Some(Decimal::new(400, 0))
        );
        assert_eq!(limits.max_strategy_budget_quote, Some(Decimal::new(300, 0)));
        assert_eq!(limits.max_global_drawdown_quote, Some(Decimal::new(50, 0)));
    }

    #[test]
    fn risk_limits_support_legacy_budget_aliases() {
        let json = r#"{
            "max_budget_quote":"1000",
            "max_symbol_exposure_quote":"500"
        }"#;

        let limits: MartingaleRiskLimits = serde_json::from_str(json).unwrap();

        assert_eq!(limits.max_global_budget_quote, Some(Decimal::new(1000, 0)));
        assert_eq!(limits.max_symbol_budget_quote, Some(Decimal::new(500, 0)));
    }

    #[test]
    fn risk_limits_guard_thresholds_round_trip_with_defaults() {
        // Structured parity replacements for the research env switches.
        let json = r#"{
            "new_cycle_drawdown_pause_pct": 8.0,
            "new_cycle_atr_pause_pct": 1.5,
            "safety_skip_adx_threshold": 50.0
        }"#;

        let limits: MartingaleRiskLimits = serde_json::from_str(json).unwrap();
        assert_eq!(limits.new_cycle_drawdown_pause_pct, Some(8.0));
        assert_eq!(limits.new_cycle_atr_pause_pct, Some(1.5));
        assert_eq!(limits.safety_skip_adx_threshold, Some(50.0));

        // Round-trip preserves the values.
        let encoded = serde_json::to_string(&limits).unwrap();
        let decoded: MartingaleRiskLimits = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, limits);
    }

    #[test]
    fn risk_limits_default_omits_guard_thresholds() {
        // Old configs without these fields must deserialize cleanly to None,
        // so historical candidates keep working after the schema extension.
        let json = r#"{"max_global_budget_quote":"1000"}"#;
        let limits: MartingaleRiskLimits = serde_json::from_str(json).unwrap();
        assert_eq!(limits.new_cycle_drawdown_pause_pct, None);
        assert_eq!(limits.new_cycle_atr_pause_pct, None);
        assert_eq!(limits.safety_skip_adx_threshold, None);
    }

    #[test]
    fn depth_tp_validation_rejects_zero_targets_and_over_close() {
        let valid = MartingaleDepthTpConfig {
            depth_01_tp_bps: 120,
            depth_23_tp_bps: 70,
            depth_23_reduce_pct: 25,
            depth_4plus_tp_bps: 40,
            depth_4plus_reduce_pct: 50,
        };
        valid.validate().expect("valid depth TP");
        assert!(MartingaleDepthTpConfig {
            depth_01_tp_bps: 0,
            ..valid.clone()
        }
        .validate()
        .is_err());
        assert!(MartingaleDepthTpConfig {
            depth_4plus_reduce_pct: 101,
            ..valid
        }
        .validate()
        .is_err());
    }
}
