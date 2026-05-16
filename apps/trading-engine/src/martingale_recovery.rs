use rust_decimal::Decimal;

use crate::martingale_runtime::{MartingaleRecoveredPosition, MartingaleRuntime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryOpenOrder {
    pub symbol: String,
    pub exchange_order_id: String,
    pub client_order_id: Option<String>,
    pub price: Decimal,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryPosition {
    pub symbol: String,
    pub quantity: Decimal,
    pub entry_price: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryTrade {
    pub trade_id: String,
    pub client_order_id: Option<String>,
    pub price: Decimal,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MartingaleRecoveryInput {
    pub positions: Vec<RecoveryPosition>,
    pub open_orders: Vec<RecoveryOpenOrder>,
    pub trades: Vec<RecoveryTrade>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MartingaleRecoveryReport {
    pub complete: bool,
    pub matched_orders: usize,
    pub positions: Vec<RecoveryPosition>,
    pub orphan_orders: Vec<RecoveryOpenOrder>,
    pub orphan_trades: Vec<RecoveryTrade>,
    pub ambiguous: Vec<String>,
}

pub fn recover_martingale_runtime(
    runtime: &mut MartingaleRuntime,
    input: MartingaleRecoveryInput,
) -> MartingaleRecoveryReport {
    let known_client_order_ids = runtime.known_client_order_ids();
    let mut report = MartingaleRecoveryReport {
        complete: true,
        positions: input.positions.clone(),
        ..MartingaleRecoveryReport::default()
    };
    runtime.replace_recovered_positions(
        input
            .positions
            .into_iter()
            .map(|position| MartingaleRecoveredPosition {
                symbol: position.symbol,
                quantity: position.quantity,
                entry_price: position.entry_price,
            })
            .collect(),
    );
    if !report.positions.is_empty() {
        report.complete = false;
        runtime.pause_all_for_recovery();
    }

    for open_order in input.open_orders {
        let Some(client_order_id) = open_order.client_order_id.as_deref() else {
            report.complete = false;
            report.orphan_orders.push(open_order);
            continue;
        };
        if !known_client_order_ids.contains(client_order_id) {
            report.complete = false;
            report.orphan_orders.push(open_order);
            continue;
        }
        if known_client_order_ids.contains(client_order_id)
            && runtime.orders().iter().any(|order| {
                order.client_order_id == client_order_id
                    && order.exchange_order_id.as_deref()
                        != Some(open_order.exchange_order_id.as_str())
                    && order.exchange_order_id.is_some()
            })
        {
            report.complete = false;
            report.ambiguous.push(client_order_id.to_string());
            report.orphan_orders.push(open_order);
            continue;
        }
        if runtime.mark_order_placed(client_order_id, open_order.exchange_order_id.clone()) {
            report.matched_orders += 1;
        } else {
            report.complete = false;
            report.ambiguous.push(client_order_id.to_string());
            report.orphan_orders.push(open_order);
        }
    }

    for trade in input.trades {
        let Some(client_order_id) = trade.client_order_id.as_deref() else {
            report.complete = false;
            report.orphan_trades.push(trade);
            continue;
        };
        if !known_client_order_ids.contains(client_order_id) {
            report.complete = false;
            report.orphan_trades.push(trade);
        }
    }

    if !report.complete {
        let affected_strategy_ids = report
            .ambiguous
            .iter()
            .filter_map(|client_order_id| runtime.strategy_id_for_client_order(client_order_id))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if affected_strategy_ids.is_empty() {
            runtime.pause_all_for_recovery();
        } else {
            for strategy_id in affected_strategy_ids {
                runtime.pause_strategy_for_recovery(&strategy_id);
            }
        }
    }

    report
}
