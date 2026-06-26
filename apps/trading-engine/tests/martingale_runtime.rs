use backtest_engine::market_data::KlineBar;
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleEntryTrigger, MartingaleIndicatorConfig,
    MartingaleMarginMode, MartingaleMarketKind, MartingalePortfolioConfig, MartingaleRiskLimits,
    MartingaleSizingModel, MartingaleSpacingModel, MartingaleStopLossModel, MartingaleStrategyConfig,
    MartingaleTakeProfitModel,
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
    assert!(orders[1].client_order_id.starts_with("mg-"));
    assert!(orders[1].client_order_id.contains("-long-"));
    assert!(orders[1].client_order_id.ends_with("-leg-1"));
    assert!(orders[1].client_order_id.len() <= 36);
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

#[test]
fn preflight_accepts_atr_spacing() {
    let mut strat = strategy("atr-spacing-btc", MartingaleDirection::Long);
    strat.spacing = MartingaleSpacingModel::Atr {
        multiplier: dec(1),
        min_step_bps: 50,
        max_step_bps: 300,
    };
    let mut runtime =
        MartingaleRuntime::new(runtime_config(vec![strat])).expect("runtime should build");
    runtime
        .start_cycle_with_futures_preflight(
            &futures_settings(true),
            "atr-spacing-btc",
            dec(100),
            MartingaleRuntimeContext::default(),
        )
        .expect("ATR spacing must be accepted by preflight");
    let orders = runtime.orders();
    assert_eq!(orders.len(), 1);
    assert!(orders[0].client_order_id.contains("leg-0"));
}

#[test]
fn preflight_accepts_atr_take_profit() {
    let mut strat = strategy("atr-tp-btc", MartingaleDirection::Long);
    strat.take_profit = MartingaleTakeProfitModel::Atr { multiplier: dec(2) };
    let mut runtime =
        MartingaleRuntime::new(runtime_config(vec![strat])).expect("runtime should build");
    runtime
        .start_cycle_with_futures_preflight(
            &futures_settings(true),
            "atr-tp-btc",
            dec(100),
            MartingaleRuntimeContext::default(),
        )
        .expect("ATR take-profit must be accepted by preflight");
    let orders = runtime.orders();
    assert_eq!(orders.len(), 1);
    assert!(orders[0].client_order_id.contains("leg-0"));
}

#[test]
fn preflight_accepts_atr_stop_loss() {
    let mut strat = strategy("atr-sl-btc", MartingaleDirection::Long);
    strat.stop_loss = Some(MartingaleStopLossModel::Atr { multiplier: dec(2) });
    let mut runtime =
        MartingaleRuntime::new(runtime_config(vec![strat])).expect("runtime should build");
    runtime
        .start_cycle_with_futures_preflight(
            &futures_settings(true),
            "atr-sl-btc",
            dec(100),
            MartingaleRuntimeContext::default(),
        )
        .expect("ATR stop-loss must be accepted by preflight");
    let orders = runtime.orders();
    assert_eq!(orders.len(), 1);
    assert!(orders[0].client_order_id.contains("leg-0"));
}

#[test]
fn indicator_expression_blocks_entry_until_warmup_satisfies_condition() {
    let mut strat = strategy("adx-btc", MartingaleDirection::Long);
    strat.entry_triggers = vec![MartingaleEntryTrigger::IndicatorExpression {
        expression: "close > sma(3)".to_string(),
    }];
    let mut runtime =
        MartingaleRuntime::new(runtime_config(vec![strat])).expect("runtime should build");

    let error = runtime
        .start_cycle_with_futures_preflight(
            &futures_settings(true),
            "adx-btc",
            dec(100),
            MartingaleRuntimeContext::default(),
        )
        .expect_err("indicator expression without warmup must block entry");
    assert!(error.to_string().contains("entry triggers"));
    assert!(runtime.orders().is_empty());

    // Low-volatility warmup: ATR(2)/close ≈ 0.5% < 2% guard, and close(101) > sma(3)=100.33
    // keeps the indicator-expression path satisfied. The original [100,100,105] bars produced
    // ATR/close ≈ 2.38% which trips the ATR>2% new-cycle guard ported from backtest.
    runtime.warmup_indicators_from_bars(vec![
        kline("BTCUSDT", 0, 100.0),
        kline("BTCUSDT", 3_600_000, 100.0),
        kline("BTCUSDT", 7_200_000, 101.0),
    ]);
    runtime
        .start_cycle_with_futures_preflight(
            &futures_settings(true),
            "adx-btc",
            dec(100),
            MartingaleRuntimeContext::default(),
        )
        .expect("satisfied indicator expression should allow entry");
    assert_eq!(runtime.orders().len(), 1);
}

fn kline(symbol: &str, open_time_ms: i64, close: f64) -> KlineBar {
    KlineBar {
        symbol: symbol.to_owned(),
        open_time_ms,
        open: close,
        high: close,
        low: close,
        close,
        volume: 0.0,
    }
}

#[test]
fn backtest_cost_constants_are_importable_for_live_stop_loss() {
    use trading_engine::martingale_runtime as _;
    use backtest_engine::martingale::kline_engine::{DEFAULT_FEE_BPS, DEFAULT_SLIPPAGE_BPS};
    assert_eq!(DEFAULT_FEE_BPS, 4.5);
    assert_eq!(DEFAULT_SLIPPAGE_BPS, 2.0);
}

#[test]
fn live_stop_loss_matches_backtest_margin_drawdown() {
    use trading_engine::martingale_exit::martingale_strategy_drawdown_pct;
    use rust_decimal::Decimal;

    // 10x leverage, entry 100, pct_bps 1200 -> backtest stops at ~1.2% adverse price
    // (12% of margin). The OLD live SL (price distance, pct_bps/10000) stopped at 12% adverse.
    let mut strat = strategy("long-btc", MartingaleDirection::Long);
    strat.leverage = Some(10);
    strat.stop_loss = Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps: 1200 });

    let threshold = 1200_f64 / 100.0; // 12.0% margin drawdown

    // 1.0% adverse (price 99.0) -> ~10% margin DD -> must NOT trigger
    let dd_below = martingale_strategy_drawdown_pct(
        &strat, dec(1), dec(100), Decimal::new(9900, 2), Decimal::ZERO, Decimal::ZERO,
    ).expect("some drawdown");
    assert!(dd_below < threshold, "1% adverse should not stop, got {dd_below}");

    // 1.3% adverse (price 98.70) -> ~13% margin DD -> must trigger
    let dd_above = martingale_strategy_drawdown_pct(
        &strat, dec(1), dec(100), Decimal::new(9870, 2), Decimal::ZERO, Decimal::ZERO,
    ).expect("some drawdown");
    assert!(dd_above >= threshold, "1.3% adverse should stop, got {dd_above}");

    // short side symmetric
    let mut short_strat = strategy("short-btc", MartingaleDirection::Short);
    short_strat.leverage = Some(10);
    short_strat.stop_loss = Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps: 1200 });
    let dd_short = martingale_strategy_drawdown_pct(
        &short_strat, dec(1), dec(100), Decimal::new(10130, 2), Decimal::ZERO, Decimal::ZERO,
    ).expect("some drawdown");
    assert!(dd_short >= threshold, "1.3% adverse short should stop, got {dd_short}");
}

#[test]
fn atr_above_two_percent_pauses_new_cycle() {
    let mut strat = strategy("long-btc", MartingaleDirection::Long);
    strat.indicators = vec![MartingaleIndicatorConfig::Atr { period: 2 }];
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![strat])).expect("runtime");
    // bars with a wide range -> atr/close > 2%
    runtime.warmup_indicators_from_bars(vec![
        kline("BTCUSDT", 0,       100.0),
        kline("BTCUSDT", 60_000,   80.0), // 20% drop -> true range huge vs close
        kline("BTCUSDT", 120_000,  80.0),
    ]);
    let err = runtime
        .start_cycle_with_futures_preflight(
            &futures_settings(true), "long-btc", dec(80),
            MartingaleRuntimeContext::default(),
        )
        .expect_err("high ATR should pause new cycle");
    assert!(err.to_string().contains("atr"), "expected atr pause, got: {err}");
    assert!(runtime.orders().is_empty());
}

#[test]
fn atr_within_limit_allows_new_cycle() {
    let mut strat = strategy("long-btc", MartingaleDirection::Long);
    strat.indicators = vec![MartingaleIndicatorConfig::Atr { period: 2 }];
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![strat])).expect("runtime");
    runtime.warmup_indicators_from_bars(vec![
        kline("BTCUSDT", 0,       100.0),
        kline("BTCUSDT", 60_000,  100.5), // tiny range -> atr/close << 2%
        kline("BTCUSDT", 120_000, 100.5),
    ]);
    runtime
        .start_cycle_with_futures_preflight(
            &futures_settings(true), "long-btc", dec(100),
            MartingaleRuntimeContext::default(),
        )
        .expect("low ATR should allow entry");
    assert_eq!(runtime.orders().len(), 1);
}

#[test]
fn adx_above_45_skips_safety_leg() {
    let mut strat = strategy("long-btc", MartingaleDirection::Long);
    strat.indicators = vec![
        MartingaleIndicatorConfig::Atr { period: 2 },
        MartingaleIndicatorConfig::Adx { period: 14 },
    ];
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![strat])).expect("runtime");
    // Consistent tiny uptrend -> directional movement dominates (DX~100) so ADX > 45,
    // while absolute ranges stay small so ATR/close << 2% (new-cycle ATR guard passes).
    // ADX(period=14) needs ~2*period+1 bars to seed Wilder smoothing + the DX average.
    let mut bars = Vec::new();
    for i in 0..30 {
        bars.push(kline("BTCUSDT", i * 60_000, 100.0 + i as f64 * 0.10));
    }
    runtime.warmup_indicators_from_bars(bars);
    start_cycle_ok(&mut runtime, "long-btc", dec(100));
    // first leg filled; safety leg should be skipped because ADX > 45
    runtime
        .mark_leg_filled_with_context(
            "long-btc", MartingaleDirection::Long, 0, MartingaleRuntimeContext::default(),
        )
        .expect("ok (safety leg skipped silently)");
    assert_eq!(runtime.orders().len(), 1, "safety leg must not be placed in strong trend");
}

#[test]
fn adx_guard_ignored_when_strategy_has_no_adx_indicator() {
    let mut strat = strategy("long-btc", MartingaleDirection::Long);
    strat.indicators = vec![MartingaleIndicatorConfig::Atr { period: 2 }]; // no ADX
    let mut runtime = MartingaleRuntime::new(runtime_config(vec![strat])).expect("runtime");
    runtime.warmup_indicators_from_bars(vec![
        kline("BTCUSDT", 0, 100.0), kline("BTCUSDT", 60_000, 100.0),
    ]);
    start_cycle_ok(&mut runtime, "long-btc", dec(100));
    runtime
        .mark_leg_filled_with_context(
            "long-btc", MartingaleDirection::Long, 0, MartingaleRuntimeContext::default(),
        )
        .expect("ok");
    assert_eq!(runtime.orders().len(), 2, "safety leg placed when no ADX configured");
}
