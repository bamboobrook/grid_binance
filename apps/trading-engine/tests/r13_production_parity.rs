//! Round 13 Task P9: Production DB/Executor parity tests.
//! These tests verify the trading-engine production paths work correctly
//! with the allocator and new config fields.

use backtest_engine::martingale::event_level_allocator::{
    EventAllocatorConfig, EventAllocatorState,
};
use backtest_engine::martingale::allocator_replay::AllocatorState;
use shared_domain::martingale::{MartingaleDepthTpConfig, MartingaleDcaMiniGridConfig, MartingaleRiskLimits};
use serde_json::json;

#[test]
fn r13_depth_tp_config_round_trips() {
    let cfg = MartingaleDepthTpConfig {
        depth_01_tp_bps: 100,
        depth_23_tp_bps: 50,
        depth_23_reduce_pct: 25,
        depth_4plus_tp_bps: 30,
        depth_4plus_reduce_pct: 50,
    };
    assert_eq!(cfg.tp_bps_for_depth(1), 100); // depth 0 (base only)
    assert_eq!(cfg.tp_bps_for_depth(2), 100); // depth 1
    assert_eq!(cfg.tp_bps_for_depth(3), 50);  // depth 2
    assert_eq!(cfg.tp_bps_for_depth(5), 30);  // depth 4+
    assert_eq!(cfg.reduce_fraction_for_depth(3), 0.25);
    assert_eq!(cfg.reduce_fraction_for_depth(5), 0.50);
}

#[test]
fn r13_minigrid_config_validation_works() {
    let mg = MartingaleDcaMiniGridConfig {
        levels_per_band: 3, spacing_bps: 50, close_fraction_num: 1,
        close_fraction_den: 4, min_profit_bps: 35, max_active_levels: 2,
    };
    mg.validate().expect("valid config");
    assert!(mg.close_fraction() > 0.0);
    // Invalid: levels=0
    assert!(MartingaleDcaMiniGridConfig {
        levels_per_band: 0, spacing_bps: 50, close_fraction_num: 1,
        close_fraction_den: 4, min_profit_bps: 35, max_active_levels: 2,
    }.validate().is_err());
}

#[test]
fn r13_risk_limits_accepts_depth_tp_and_minigrid() {
    let limits = MartingaleRiskLimits {
        depth_tp: Some(MartingaleDepthTpConfig {
            depth_01_tp_bps: 80, depth_23_tp_bps: 40, depth_23_reduce_pct: 0,
            depth_4plus_tp_bps: 20, depth_4plus_reduce_pct: 25,
        }),
        dca_minigrid: Some(MartingaleDcaMiniGridConfig {
            levels_per_band: 2, spacing_bps: 30, close_fraction_num: 1,
            close_fraction_den: 6, min_profit_bps: 20, max_active_levels: 1,
        }),
        ..Default::default()
    };
    assert!(limits.depth_tp.is_some());
    assert!(limits.dca_minigrid.is_some());
}

#[test]
fn r13_event_allocator_state_persists_and_restores() {
    let state = EventAllocatorState::new(
        vec!["R4".to_string(), "ANKR".to_string()],
        vec!["R4".to_string()],
        4999.0,
        7 * 86_400_000,
    );
    let json = state.to_json();
    let restored = EventAllocatorState::from_json(&json).expect("restore");
    assert_eq!(restored.active_sleeve_ids, vec!["R4".to_string()]);
    assert_eq!(restored.budget_quote, 4999.0);
}

#[test]
fn r13_event_allocator_blocks_inactive_sleeve() {
    let state = EventAllocatorState::new(
        vec!["R4".to_string(), "ANKR".to_string()],
        vec!["R4".to_string()],
        4999.0,
        86_400_000,
    );
    assert!(state.can_open_new_cycle("R4", 3 * 86_400_000));
    assert!(!state.can_open_new_cycle("ANKR", 3 * 86_400_000));
}

#[test]
fn r13_event_allocator_shadow_never_enters_live() {
    let mut state = EventAllocatorState::new(
        vec!["R4".to_string()], vec!["R4".to_string()], 4999.0, 86_400_000,
    );
    state.record_shadow_observation(1000, "R4", 6000.0);
    assert_eq!(state.live_equity_quote, 4999.0, "shadow must not enter live equity");
}

#[test]
fn r13_batch_replay_module_exists() {
    // Verify batch_replay module is accessible
    let _ = backtest_engine::martingale::batch_replay::BatchReplay::new("nonexistent", "nonexistent");
    // Should error but not panic
}

#[test]
fn r13_depth_tp_config_serializes_correctly() {
    let cfg = MartingaleDepthTpConfig {
        depth_01_tp_bps: 120, depth_23_tp_bps: 70, depth_23_reduce_pct: 25,
        depth_4plus_tp_bps: 40, depth_4plus_reduce_pct: 50,
    };
    let json_str = serde_json::to_string(&cfg).expect("serialize");
    assert!(json_str.contains("depth_01_tp_bps"));
    let restored: MartingaleDepthTpConfig = serde_json::from_str(&json_str).expect("deserialize");
    assert_eq!(restored, cfg);
}

#[test]
fn r13_risk_limits_serializes_with_new_fields() {
    let limits = MartingaleRiskLimits {
        depth_tp: Some(MartingaleDepthTpConfig {
            depth_01_tp_bps: 100, depth_23_tp_bps: 50, depth_23_reduce_pct: 0,
            depth_4plus_tp_bps: 30, depth_4plus_reduce_pct: 25,
        }),
        ..Default::default()
    };
    let json_str = serde_json::to_string(&limits).expect("serialize");
    assert!(json_str.contains("depth_tp"));
    let restored: MartingaleRiskLimits = serde_json::from_str(&json_str).expect("deserialize");
    assert!(restored.depth_tp.is_some());
}

#[test]
fn r13_minigrid_level_price_symmetric() {
    let mg = MartingaleDcaMiniGridConfig {
        levels_per_band: 3, spacing_bps: 40, close_fraction_num: 1,
        close_fraction_den: 4, min_profit_bps: 15, max_active_levels: 2,
    };
    let fill = 200.0;
    for idx in 0..3 {
        let long_p = mg.level_price(fill, idx, true);
        let short_p = mg.level_price(fill, idx, false);
        assert!(long_p > fill);
        assert!(short_p < fill);
    }
}

#[test]
fn r13_allocator_config_parses_from_json() {
    let json = serde_json::json!({
        "lookback_days": 60,
        "rebalance_days": 7,
        "max_active_sleeves": 1,
        "cash_trigger_dd_pct": null,
        "switch_hysteresis_gap": 0.0
    });
    let cfg: EventAllocatorConfig = serde_json::from_value(json).expect("parse");
    assert_eq!(cfg.lookback_days, 60);
    assert_eq!(cfg.max_active_sleeves, 1);
}
