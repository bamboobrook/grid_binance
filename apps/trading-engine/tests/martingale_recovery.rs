use rust_decimal::Decimal;
use shared_domain::martingale::MartingaleDirection;
use trading_engine::martingale_recovery::{
    recover_martingale_runtime, MartingaleRecoveryInput, RecoveryOpenOrder, RecoveryPosition,
    RecoveryTrade,
};
use trading_engine::martingale_runtime::{MartingaleRuntime, MartingaleRuntimeOrderStatus};

mod runtime_cases {
    include!("martingale_runtime.rs");
}

fn dec(value: i64) -> Decimal {
    Decimal::new(value, 0)
}

fn sample_runtime() -> MartingaleRuntime {
    MartingaleRuntime::new(runtime_cases::runtime_config(vec![
        runtime_cases::strategy("long-btc", MartingaleDirection::Long),
    ]))
    .expect("runtime should build")
}

#[test]
fn orphan_order_pauses_strategy() {
    let mut runtime = sample_runtime();
    runtime_cases::start_cycle_ok(&mut runtime, "long-btc", dec(100));

    let report = recover_martingale_runtime(
        &mut runtime,
        MartingaleRecoveryInput {
            open_orders: vec![RecoveryOpenOrder {
                symbol: "BTCUSDT".to_string(),
                exchange_order_id: "999".to_string(),
                client_order_id: Some("manual-order".to_string()),
                price: dec(95),
                quantity: dec(1),
            }],
            trades: Vec::new(),
            positions: Vec::new(),
        },
    );

    assert!(!report.complete);
    assert_eq!(report.orphan_orders.len(), 1);
    assert!(runtime.is_strategy_paused("long-btc"));
    assert!(runtime
        .start_cycle("long-btc", dec(101), Default::default())
        .expect_err("orphan should block new legs")
        .to_string()
        .contains("recovery incomplete"));
}

#[test]
fn recovery_matches_only_known_client_order_ids() {
    let mut runtime = sample_runtime();
    runtime_cases::start_cycle_ok(&mut runtime, "long-btc", dec(100));
    let known_client_order_id = runtime.orders()[0].client_order_id.clone();

    let report = recover_martingale_runtime(
        &mut runtime,
        MartingaleRecoveryInput {
            open_orders: vec![RecoveryOpenOrder {
                symbol: "BTCUSDT".to_string(),
                exchange_order_id: "123".to_string(),
                client_order_id: Some(known_client_order_id),
                price: dec(100),
                quantity: dec(1),
            }],
            trades: vec![RecoveryTrade {
                trade_id: "t1".to_string(),
                client_order_id: Some("unknown-client-id".to_string()),
                price: dec(100),
                quantity: dec(1),
            }],
            positions: Vec::new(),
        },
    );

    assert!(!report.complete);
    assert_eq!(report.matched_orders, 1);
    assert_eq!(report.orphan_trades.len(), 1);
    assert_eq!(
        runtime.orders()[0].exchange_order_id.as_deref(),
        Some("123")
    );
    assert_eq!(
        runtime.orders()[0].status,
        MartingaleRuntimeOrderStatus::Placed
    );
}

#[test]
fn recovery_records_non_empty_positions() {
    let mut runtime = sample_runtime();

    let report = recover_martingale_runtime(
        &mut runtime,
        MartingaleRecoveryInput {
            positions: vec![RecoveryPosition {
                symbol: "BTCUSDT".to_string(),
                quantity: dec(2),
                entry_price: dec(99),
            }],
            open_orders: Vec::new(),
            trades: Vec::new(),
        },
    );

    assert!(!report.complete);
    assert_eq!(report.positions.len(), 1);
    assert_eq!(runtime.recovered_positions().len(), 1);
    assert_eq!(runtime.recovered_positions()[0].entry_price, dec(99));
    assert!(runtime.is_strategy_paused("long-btc"));
    assert!(runtime
        .start_cycle("long-btc", dec(100), Default::default())
        .expect_err("unattributed position should block restart")
        .to_string()
        .contains("recovery incomplete"));
}

#[test]
fn ambiguous_known_order_is_orphan_semantics_and_pauses_affected_strategy() {
    let mut runtime = sample_runtime();
    runtime_cases::start_cycle_ok(&mut runtime, "long-btc", dec(100));
    let known_client_order_id = runtime.orders()[0].client_order_id.clone();
    runtime.mark_order_placed(&known_client_order_id, "old-exchange-id".to_string());

    let report = recover_martingale_runtime(
        &mut runtime,
        MartingaleRecoveryInput {
            open_orders: vec![RecoveryOpenOrder {
                symbol: "BTCUSDT".to_string(),
                exchange_order_id: "new-exchange-id".to_string(),
                client_order_id: Some(known_client_order_id),
                price: dec(100),
                quantity: dec(1),
            }],
            trades: Vec::new(),
            positions: Vec::new(),
        },
    );

    assert!(!report.complete);
    assert_eq!(report.ambiguous.len(), 1);
    assert_eq!(report.orphan_orders.len(), 1);
    assert!(runtime.is_strategy_paused("long-btc"));
}
