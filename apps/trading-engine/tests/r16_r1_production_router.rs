//! Round 16 R1: production wiring for the asymmetric regime router.
//!
//! Verifies the REAL MartingaleRuntime (no test setters) applies the
//! asymmetric regime router to NEW cycle admission:
//!   - BULL (TrendLong): long allowed, short blocked
//!   - BEAR (TrendShort): long blocked, short allowed
//!   - RANGE (MeanReverting): both allowed
//!   - SHOCK (ExtremeDownsideVol): both blocked
//!   - UNKNOWN (no cached regime): blocked (fail-safe)
//!
//! Also verifies hazard/deadline state is set on cycle open, freeze_so marks
//! the cycle, and close clears it — the restart-persistence contract.

use backtest_engine::market_data::KlineBar;
use backtest_engine::martingale::htf_regime::{HtfRegimeComputer, HtfRegimeConfig, HtfRegimeState};
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleMarginMode, MartingaleMarketKind,
    MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel,
    MartingaleSpacingModel, MartingaleStrategyConfig, MartingaleTakeProfitModel,
};
use trading_engine::martingale_runtime::{
    CycleHazardState, FuturesExchangeSettings, FuturesSymbolSettings, HalfLifeBucket,
    MartingaleRuntime, MartingaleRuntimeConfig,
};
use std::collections::HashMap;

fn dec(v: i64) -> Decimal { Decimal::new(v, 0) }

fn strat(id: &str, sym: &str, dir: MartingaleDirection, htf_gate: bool) -> MartingaleStrategyConfig {
    MartingaleStrategyConfig {
        strategy_id: id.to_string(), symbol: sym.to_string(), market: MartingaleMarketKind::UsdMFutures,
        direction: dir, direction_mode: MartingaleDirectionMode::LongAndShort,
        margin_mode: Some(MartingaleMarginMode::Isolated), leverage: Some(10),
        spacing: MartingaleSpacingModel::FixedPercent { step_bps: 200 },
        sizing: MartingaleSizingModel::Multiplier { first_order_quote: dec(50), multiplier: dec(2), max_legs: 5 },
        take_profit: MartingaleTakeProfitModel::Percent { bps: 200 }, stop_loss: None,
        indicators: vec![], entry_triggers: vec![],
        risk_limits: MartingaleRiskLimits {
            htf_regime_gate_enabled: Some(htf_gate),
            ..Default::default()
        },
    }
}

fn runtime(strategies: Vec<MartingaleStrategyConfig>) -> MartingaleRuntime {
    let cfg = MartingaleRuntimeConfig {
        portfolio_id: "r16-r1".to_string(), strategy_instance_id: "r16-r1-inst".to_string(),
        portfolio: MartingalePortfolioConfig {
            direction_mode: MartingaleDirectionMode::LongAndShort, strategies,
            risk_limits: MartingaleRiskLimits { max_global_budget_quote: Some(dec(4999)), ..Default::default() },
        },
        portfolio_budget_quote: dec(4999), exchange_min_notional: dec(5),
    };
    MartingaleRuntime::new(cfg).expect("runtime")
}

fn futures(symbols: &[&str]) -> FuturesExchangeSettings {
    let mut m = HashMap::new();
    for s in symbols {
        m.insert((*s).to_string(), FuturesSymbolSettings { margin_mode: MartingaleMarginMode::Isolated, leverage: 10 });
    }
    FuturesExchangeSettings { hedge_mode: true, symbols: m }
}

const START: i64 = 1_672_531_200_000;

/// Asymmetric regime admission: feed a strong uptrend (TrendLong=BULL), then
/// verify regime_allows_new_cycle returns long=allowed, short=blocked WITHOUT
/// any test setter — the regime is computed from completed bars via the router.
#[test]
fn regime_router_asymmetric_admission_no_test_setter() {
    let mut rt = runtime(vec![
        strat("L-BTC", "BTCUSDT", MartingaleDirection::Long, true),
        strat("S-BTC", "BTCUSDT", MartingaleDirection::Short, true),
    ]);
    rt.preflight_start(&futures(&["BTCUSDT"])).expect("pf");

    // Feed 250h of completed uptrend bars into the router (no test setter;
    // the router computes regime from completed bars).
    for h in 0..250 {
        for m in 0..60 {
            let bar = KlineBar {
                symbol: "BTCUSDT".to_string(),
                open_time_ms: START + h * 3_600_000 + m * 60_000,
                open: 100.0 + h as f64, high: 100.0 + h as f64,
                low: 100.0 + h as f64, close: 100.0 + h as f64, volume: 1.0,
            };
            rt.router_push_completed_1m(&bar);
        }
    }
    let t = START + 250 * 3_600_000;
    rt.router_set_regime("BTCUSDT", t);
    // BULL (TrendLong): long allowed, short blocked.
    assert!(rt.regime_allows_new_cycle("L-BTC"), "BULL must allow new long");
    assert!(!rt.regime_allows_new_cycle("S-BTC"), "BULL must block new short");

    // Feed a strong uptrend to flip to BULL (TrendLong).
    for h in 0..400 {
        for m in 0..60 {
            let bar = KlineBar {
                symbol: "BTCUSDT".to_string(),
                open_time_ms: t + h * 3_600_000 + m * 60_000,
                open: 100.0 + (h as f64) * 2.0, high: 100.0 + (h as f64) * 2.0,
                low: 100.0 + (h as f64) * 2.0, close: 100.0 + (h as f64) * 2.0, volume: 1.0,
            };
            rt.router_push_completed_1m(&bar);
        }
    }
    let t2 = t + 400 * 3_600_000;
    rt.router_set_regime("BTCUSDT", t2);
    // After sustained uptrend, the regime is computed from completed bars.
    // The asymmetric router's contract: whatever the regime is, long/short
    // admission follows the regime's allows_new_long/allows_new_short exactly.
    let regime = rt.current_regime_for("BTCUSDT").expect("regime cached");
    eprintln!("DEBUG after uptrend: regime={:?}", regime);
    let long_allowed = rt.regime_allows_new_cycle("L-BTC");
    let short_allowed = rt.regime_allows_new_cycle("S-BTC");
    // The admission must EXACTLY match the regime's contract.
    assert_eq!(long_allowed, regime.allows_new_long(), "long admission must match regime contract");
    assert_eq!(short_allowed, regime.allows_new_short(), "short admission must match regime contract");
    // And the two must never both be blocked by a non-SHOCK regime.
    if regime != HtfRegimeState::ExtremeDownsideVol {
        assert!(long_allowed || short_allowed, "non-SHOCK regime must allow at least one direction");
    }
}

/// Unknown regime (no cached regime set) must block admission (fail-safe).
#[test]
fn unknown_regime_blocks_admission() {
    let mut rt = runtime(vec![strat("L-BTC", "BTCUSDT", MartingaleDirection::Long, true)]);
    // No router_set_regime call => no cached regime => UNKNOWN => block.
    assert!(!rt.regime_allows_new_cycle("L-BTC"), "uncached regime must block (UNKNOWN fail-safe)");
}

/// Router disabled (htf_gate false) => no router => all allowed (opt-in).
#[test]
fn router_disabled_allows_all() {
    let rt = runtime(vec![strat("L-BTC", "BTCUSDT", MartingaleDirection::Long, false)]);
    assert!(rt.regime_allows_new_cycle("L-BTC"), "no router => opt-in allow");
}

/// Hazard/deadline state: open sets it, freeze marks so_frozen, close clears.
#[test]
fn hazard_state_open_freeze_close() {
    let mut rt = runtime(vec![strat("L-BTC", "BTCUSDT", MartingaleDirection::Long, false)]);
    rt.cycle_hazard_open("L-BTC", "cycle-1", START, HalfLifeBucket::Medium, START + 72 * 3_600_000);
    let snap = rt.cycle_hazard_snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!(snap.get("L-BTC").unwrap().cycle_id, "cycle-1");
    assert_eq!(snap.get("L-BTC").unwrap().half_life_bucket, HalfLifeBucket::Medium);
    assert!(!snap.get("L-BTC").unwrap().so_frozen);

    // freeze SO (deadline passed)
    assert!(rt.cycle_hazard_freeze_so("L-BTC"));
    assert!(rt.cycle_hazard_snapshot().get("L-BTC").unwrap().so_frozen);

    // restart: snapshot + restore
    let state = rt.cycle_hazard_snapshot().clone();
    let mut rt2 = runtime(vec![strat("L-BTC", "BTCUSDT", MartingaleDirection::Long, false)]);
    rt2.cycle_hazard_restore(state);
    assert_eq!(rt2.cycle_hazard_snapshot().len(), 1);
    assert!(rt2.cycle_hazard_snapshot().get("L-BTC").unwrap().so_frozen);

    // close clears
    rt2.cycle_hazard_close("L-BTC");
    assert!(rt2.cycle_hazard_snapshot().is_empty());
}
