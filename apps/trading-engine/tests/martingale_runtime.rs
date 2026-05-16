use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleMarginMode, MartingaleMarketKind,
    MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel,
    MartingaleStrategyConfig, MartingaleTakeProfitModel,
};
use shared_domain::strategy::StrategyStatus;
use std::collections::HashMap;
use trading_engine::martingale_runtime::{
    FuturesExchangeSettings, FuturesSymbolSettings, MartingaleRuntime, MartingaleRuntimeConfig,
    MartingaleRuntimeContext, MartingaleRuntimeOrderStatus,
};

fn dec(value: i64) -> Decimal {
    Decimal::new(value, 0)
}

pub fn strategy(id: &str, direction: MartingaleDirection) -> MartingaleStrategyConfig {
    MartingaleStrategyConfig {
        strategy_id: id.to_string(),
        symbol: "BTCUSDT".to_string(),
        market: MartingaleMarketKind::UsdMFutures,
        direction,
        direction_mode: MartingaleDirectionMode::LongAndShort,
        margin_mode: Some(shared_domain::martingale::MartingaleMarginMode::Cross),
        leverage: Some(3),
        spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
        sizing: MartingaleSizingModel::Multiplier {
            first_order_quote: dec(100),
            multiplier: dec(2),
            max_legs: 3,
        },
        take_profit: MartingaleTakeProfitModel::Percent { bps: 100 },
        stop_loss: None,
        indicators: Vec::new(),
        entry_triggers: Vec::new(),
        risk_limits: MartingaleRiskLimits::default(),
    }
}

pub fn runtime_config(strategies: Vec<MartingaleStrategyConfig>) -> MartingaleRuntimeConfig {
    MartingaleRuntimeConfig {
        portfolio_id: "portfolio-a".to_string(),
        strategy_instance_id: "instance-a".to_string(),
        portfolio: MartingalePortfolioConfig {
            direction_mode: MartingaleDirectionMode::LongAndShort,
            strategies,
            risk_limits: MartingaleRiskLimits::default(),
        },
        portfolio_budget_quote: dec(10_000),
        exchange_min_notional: Decimal::ZERO,
    }
}

pub fn futures_settings(hedge_mode: bool) -> FuturesExchangeSettings {
    FuturesExchangeSettings {
        hedge_mode,
        symbols: HashMap::from([(
            "BTCUSDT".to_string(),
            FuturesSymbolSettings {
                margin_mode: MartingaleMarginMode::Cross,
                leverage: 3,
            },
        )]),
    }
}

pub fn start_cycle_ok(runtime: &mut MartingaleRuntime, strategy_id: &str, anchor_price: Decimal) {
    runtime
        .start_cycle_with_futures_preflight(
            &futures_settings(true),
            strategy_id,
            anchor_price,
            MartingaleRuntimeContext::default(),
        )
        .expect("futures preflight start should pass");
}

#[test]
fn long_cycle_places_first_order_then_safety_order() {
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![strategy(
        "long-btc",
        MartingaleDirection::Long,
    )]))
    .expect("runtime should build");
    start_cycle_ok(&mut runtime, "long-btc", dec(100));
    runtime
        .mark_leg_filled("long-btc", MartingaleDirection::Long, 0)
        .expect("first leg should fill");

    let orders = runtime.orders();
    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].side, "BUY");
    assert_eq!(orders[0].price, dec(100));
    assert_eq!(orders[0].status, MartingaleRuntimeOrderStatus::Filled);
    assert_eq!(orders[1].side, "BUY");
    assert_eq!(orders[1].price, dec(99));
    assert!(orders[1].client_order_id.contains("portfolio-a"));
    assert!(orders[1].client_order_id.contains("instance-a"));
    assert!(orders[1].client_order_id.contains("long"));
    assert!(orders[1].client_order_id.ends_with("-leg-1"));
}

#[test]
fn short_cycle_places_safety_order_above_anchor() {
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![strategy(
        "short-btc",
        MartingaleDirection::Short,
    )]))
    .expect("runtime should build");
    start_cycle_ok(&mut runtime, "short-btc", dec(100));
    runtime
        .mark_leg_filled("short-btc", MartingaleDirection::Short, 0)
        .expect("first leg should fill");

    let safety = runtime
        .orders()
        .iter()
        .find(|order| order.leg_index == 1)
        .expect("short safety order should exist");
    assert_eq!(safety.side, "SELL");
    assert_eq!(safety.price, dec(101));
}

#[test]
fn long_and_short_cycles_remain_independent() {
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![
        strategy("long-btc", MartingaleDirection::Long),
        strategy("short-btc", MartingaleDirection::Short),
    ]))
    .expect("runtime should build");
    start_cycle_ok(&mut runtime, "long-btc", dec(100));
    start_cycle_ok(&mut runtime, "short-btc", dec(100));
    runtime
        .mark_leg_filled("long-btc", MartingaleDirection::Long, 0)
        .expect("long first leg should fill");

    assert!(runtime
        .orders_for("long-btc", MartingaleDirection::Long)
        .iter()
        .any(|order| order.leg_index == 1 && order.price == dec(99)));
    assert!(!runtime
        .orders_for("short-btc", MartingaleDirection::Short)
        .iter()
        .any(|order| order.leg_index == 1));
}

#[test]
fn global_drawdown_pauses_new_entries() {
    let mut config = runtime_config(vec![strategy("long-btc", MartingaleDirection::Long)]);
    config.portfolio.risk_limits.max_global_drawdown_quote = Some(dec(50));
    let mut runtime = MartingaleRuntime::new(config).expect("runtime should build");
    let error = runtime
        .start_cycle_with_futures_preflight(
            &futures_settings(true),
            "long-btc",
            dec(100),
            MartingaleRuntimeContext {
                global_drawdown_quote: dec(51),
                ..MartingaleRuntimeContext::default()
            },
        )
        .expect_err("drawdown should block entries");

    assert!(error.to_string().contains("global drawdown"));
    assert!(runtime.orders().is_empty());
}

#[test]
fn strategy_pause_resume_stop_controls_new_entries() {
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![strategy(
        "long-btc",
        MartingaleDirection::Long,
    )]))
    .expect("runtime should build");
    let error = runtime
        .start_cycle_with_futures_preflight(
            &futures_settings(true),
            "long-btc",
            dec(100),
            MartingaleRuntimeContext {
                strategy_status: StrategyStatus::Paused,
                ..MartingaleRuntimeContext::default()
            },
        )
        .expect_err("paused strategy should block entries");
    assert!(error.to_string().contains("paused"));

    runtime
        .start_cycle_with_futures_preflight(
            &futures_settings(true),
            "long-btc",
            dec(100),
            MartingaleRuntimeContext {
                strategy_status: StrategyStatus::Running,
                ..MartingaleRuntimeContext::default()
            },
        )
        .expect("running strategy should resume entries");
    runtime.stop_strategy("long-btc");
    assert!(runtime
        .start_cycle_with_futures_preflight(
            &futures_settings(true),
            "long-btc",
            dec(101),
            MartingaleRuntimeContext::default(),
        )
        .expect_err("stopped strategy should block entries")
        .to_string()
        .contains("stopped"));
}

#[test]
fn stopped_strategy_does_not_place_safety_leg_after_fill() {
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![strategy(
        "long-btc",
        MartingaleDirection::Long,
    )]))
    .expect("runtime should build");
    start_cycle_ok(&mut runtime, "long-btc", dec(100));
    runtime.stop_strategy("long-btc");

    let error = runtime
        .mark_leg_filled_with_context(
            "long-btc",
            MartingaleDirection::Long,
            0,
            MartingaleRuntimeContext::default(),
        )
        .expect_err("stopped strategy should block safety legs");

    assert!(error.to_string().contains("stopped"));
    assert_eq!(runtime.orders().len(), 1);
}

#[test]
fn recovery_incomplete_does_not_place_safety_leg_after_fill() {
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![strategy(
        "long-btc",
        MartingaleDirection::Long,
    )]))
    .expect("runtime should build");
    start_cycle_ok(&mut runtime, "long-btc", dec(100));
    runtime.pause_strategy_for_recovery("long-btc");

    let error = runtime
        .mark_leg_filled_with_context(
            "long-btc",
            MartingaleDirection::Long,
            0,
            MartingaleRuntimeContext::default(),
        )
        .expect_err("recovery pause should block safety legs");

    assert!(error.to_string().contains("recovery incomplete"));
    assert_eq!(runtime.orders().len(), 1);
}

#[test]
fn futures_preflight_rejects_missing_hedge_mode_before_start() {
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![
        strategy("long-btc", MartingaleDirection::Long),
        strategy("short-btc", MartingaleDirection::Short),
    ]))
    .expect("runtime should build");
    let settings = FuturesExchangeSettings {
        hedge_mode: false,
        symbols: HashMap::from([(
            "BTCUSDT".to_string(),
            FuturesSymbolSettings {
                margin_mode: MartingaleMarginMode::Cross,
                leverage: 3,
            },
        )]),
    };

    let error = runtime
        .start_cycle_with_futures_preflight(
            &settings,
            "long-btc",
            dec(100),
            MartingaleRuntimeContext::default(),
        )
        .expect_err("missing hedge mode must reject start");

    assert!(error.to_string().contains("Hedge Mode"));
}

#[test]
fn futures_preflight_rejects_same_symbol_margin_leverage_conflict() {
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![strategy(
        "long-btc",
        MartingaleDirection::Long,
    )]))
    .expect("runtime should build");
    let settings = FuturesExchangeSettings {
        hedge_mode: true,
        symbols: HashMap::from([(
            "BTCUSDT".to_string(),
            FuturesSymbolSettings {
                margin_mode: MartingaleMarginMode::Isolated,
                leverage: 5,
            },
        )]),
    };

    let error = runtime
        .start_cycle_with_futures_preflight(
            &settings,
            "long-btc",
            dec(100),
            MartingaleRuntimeContext::default(),
        )
        .expect_err("conflicting symbol settings must reject start");

    assert!(error.to_string().contains("margin mode") || error.to_string().contains("leverage"));
}

#[test]
fn futures_start_cycle_requires_successful_preflight() {
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![strategy(
        "long-btc",
        MartingaleDirection::Long,
    )]))
    .expect("runtime should build");

    let error = runtime
        .start_cycle("long-btc", dec(100), MartingaleRuntimeContext::default())
        .expect_err("futures start must require preflight");

    assert!(error.to_string().contains("preflight"));
}
