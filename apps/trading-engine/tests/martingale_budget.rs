use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleMarketKind, MartingalePortfolioConfig,
    MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel, MartingaleStrategyConfig,
    MartingaleTakeProfitModel,
};
use std::collections::HashMap;
use trading_engine::martingale_budget::{apply_global_budget_allocations, cap_strategy_budget};

fn dec(value: i64) -> Decimal {
    Decimal::new(value, 0)
}

fn strategy(
    id: &str,
    market: MartingaleMarketKind,
    leverage: Option<u32>,
    first_order_quote: Decimal,
) -> MartingaleStrategyConfig {
    MartingaleStrategyConfig {
        strategy_id: id.to_string(),
        symbol: "BTCUSDT".to_string(),
        market,
        direction: MartingaleDirection::Long,
        direction_mode: MartingaleDirectionMode::LongAndShort,
        margin_mode: None,
        leverage,
        spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
        sizing: MartingaleSizingModel::Multiplier {
            first_order_quote,
            multiplier: dec(2),
            max_legs: 4,
        },
        take_profit: MartingaleTakeProfitModel::Percent { bps: 100 },
        stop_loss: None,
        indicators: Vec::new(),
        entry_triggers: Vec::new(),
        risk_limits: MartingaleRiskLimits::default(),
    }
}

#[test]
fn cap_uses_first_leg_margin_not_notional_for_futures() {
    // Futures leverage 5, foq=250 -> first-leg MARGIN = 250/5 = 50.
    // budget_cap=10 (margin) -> effective = max(10, 50) = 50. NOT 250.
    let mut s = strategy("s1", MartingaleMarketKind::UsdMFutures, Some(5), dec(250));
    cap_strategy_budget(&mut s, dec(10));
    assert_eq!(s.risk_limits.max_strategy_budget_quote, Some(dec(50)));
}

#[test]
fn cap_keeps_budget_cap_when_above_first_leg_margin() {
    // budget_cap=100, first-leg margin=50 -> effective = max(100, 50) = 100.
    let mut s = strategy("s1", MartingaleMarketKind::UsdMFutures, Some(5), dec(250));
    cap_strategy_budget(&mut s, dec(100));
    assert_eq!(s.risk_limits.max_strategy_budget_quote, Some(dec(100)));
}

#[test]
fn cap_first_leg_margin_spot_is_unleveraged() {
    // Spot: leverage None/ignored -> first-leg margin = foq = 250 (no leverage).
    // budget_cap=10 -> effective = max(10, 250) = 250.
    let mut s = strategy("s1", MartingaleMarketKind::Spot, None, dec(250));
    cap_strategy_budget(&mut s, dec(10));
    assert_eq!(s.risk_limits.max_strategy_budget_quote, Some(dec(250)));
}

#[test]
fn cap_does_not_shrink_below_first_leg_margin() {
    // cap_optional_budget_limit semantics: min(existing>0, effective).
    // existing=300, effective=50 (first-leg margin) -> min(300, 50) = 50.
    let mut s = strategy("s1", MartingaleMarketKind::UsdMFutures, Some(5), dec(250));
    s.risk_limits.max_strategy_budget_quote = Some(dec(300));
    cap_strategy_budget(&mut s, dec(10));
    assert_eq!(s.risk_limits.max_strategy_budget_quote, Some(dec(50)));
}

#[test]
fn apply_global_budget_allocations_uses_weight_factors() {
    // global_budget=100 (margin). weights {"long":0.5, "short":0.5} -> 50 each.
    // foq=100 lev=4 -> first-leg margin=25 <= 50 -> cap=50 for both.
    // Proves weights sum to 100% of MARGIN, not 200% of notional.
    let long = strategy("long", MartingaleMarketKind::UsdMFutures, Some(4), dec(100));
    let short = strategy(
        "short",
        MartingaleMarketKind::UsdMFutures,
        Some(4),
        dec(100),
    );
    let mut config = MartingalePortfolioConfig {
        direction_mode: MartingaleDirectionMode::LongAndShort,
        strategies: vec![long, short],
        risk_limits: MartingaleRiskLimits {
            max_global_budget_quote: Some(dec(100)),
            ..MartingaleRiskLimits::default()
        },
    };
    let mut weights = HashMap::new();
    weights.insert("long".to_string(), Decimal::new(5, 1)); // 0.5
    weights.insert("short".to_string(), Decimal::new(5, 1)); // 0.5
    apply_global_budget_allocations(&mut config, &weights);
    let caps: Vec<Option<Decimal>> = config
        .strategies
        .iter()
        .map(|s| s.risk_limits.max_strategy_budget_quote)
        .collect();
    assert_eq!(caps, vec![Some(dec(50)), Some(dec(50))]);
}
