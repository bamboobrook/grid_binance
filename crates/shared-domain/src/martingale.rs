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

        Ok(())
    }
}

impl MartingalePortfolioConfig {
    pub fn validate(&self) -> Result<(), String> {
        use std::collections::HashMap;

        let mut futures_by_symbol: HashMap<String, (MartingaleMarginMode, u32)> = HashMap::new();

        for strategy in &self.strategies {
            strategy.validate()?;

            if strategy.market == MartingaleMarketKind::UsdMFutures {
                let margin_mode = strategy
                    .margin_mode
                    .expect("validated futures strategy must have margin_mode");
                let leverage = strategy
                    .leverage
                    .expect("validated futures strategy must have leverage");

                let symbol_key = strategy.symbol.trim().to_uppercase();
                if let Some((existing_margin_mode, existing_leverage)) =
                    futures_by_symbol.get(&symbol_key)
                {
                    if *existing_margin_mode != margin_mode {
                        return Err(format!(
                            "{} margin_mode conflict for USDT-M futures strategies",
                            strategy.symbol
                        ));
                    }
                    if *existing_leverage != leverage {
                        return Err(format!(
                            "{} leverage conflict for USDT-M futures strategies",
                            strategy.symbol
                        ));
                    }
                } else {
                    futures_by_symbol.insert(symbol_key, (margin_mode, leverage));
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
    fn same_symbol_futures_margin_or_leverage_conflict_is_rejected() {
        let mut portfolio = MartingalePortfolioConfig::example_futures_long_short("BTCUSDT");
        portfolio.strategies[1].leverage = Some(5);
        let error = portfolio
            .validate()
            .expect_err("conflicting leverage must fail");
        assert!(error.contains("BTCUSDT"));
        assert!(error.contains("leverage"));
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
}
