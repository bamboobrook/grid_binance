use rust_decimal::Decimal;
use shared_db::{BillingOrderRecord, SharedDb, SharedDbError};
use shared_domain::analytics::{AnalyticsReport, StrategyProfitSummary, TradeFillInput};
use trading_engine::statistics::{
    compute_analytics_report, compute_fill_views, compute_strategy_summaries,
};

#[derive(Clone)]
pub struct AnalyticsService {
    db: SharedDb,
}

impl Default for AnalyticsService {
    fn default() -> Self {
        Self::new(SharedDb::ephemeral().expect("ephemeral analytics db should initialize"))
    }
}

impl AnalyticsService {
    pub fn new(db: SharedDb) -> Self {
        Self { db }
    }

    pub fn report_for_user(&self, user_email: &str) -> Result<AnalyticsReport, SharedDbError> {
        let fills = self.trade_fill_inputs(user_email)?;
        Ok(compute_analytics_report(&fills))
    }

    pub fn export_orders_csv(&self, user_email: &str) -> Result<String, SharedDbError> {
        let mut csv = String::from(
            "order_id,strategy_id,symbol,side,order_type,price,quantity,status
",
        );
        for strategy in self.user_strategies(user_email)? {
            for order in strategy.runtime.orders {
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{},{}
",
                    order.order_id,
                    strategy.id,
                    strategy.symbol,
                    order.side,
                    order.order_type,
                    order.price.map(format_decimal).unwrap_or_default(),
                    format_decimal(order.quantity),
                    order.status,
                ));
            }
        }
        Ok(csv)
    }

    pub fn export_fills_csv(&self, user_email: &str) -> Result<String, SharedDbError> {
        let mut csv = String::from(
            "fill_id,strategy_id,order_id,symbol,price,quantity,realized_pnl,fee_amount,fee_asset,fill_type
",
        );
        for strategy in self.user_strategies(user_email)? {
            for fill in strategy.runtime.fills {
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{}
",
                    fill.fill_id,
                    strategy.id,
                    fill.order_id.unwrap_or_default(),
                    strategy.symbol,
                    format_decimal(fill.price),
                    format_decimal(fill.quantity),
                    fill.realized_pnl.map(format_decimal).unwrap_or_default(),
                    fill.fee_amount.map(format_decimal).unwrap_or_default(),
                    fill.fee_asset.unwrap_or_default(),
                    fill.fill_type,
                ));
            }
        }
        Ok(csv)
    }

    pub fn export_strategy_stats_csv(&self, user_email: &str) -> Result<String, SharedDbError> {
        let fills = compute_fill_views(&self.trade_fill_inputs(user_email)?);
        let strategies = compute_strategy_summaries(&fills);
        let mut csv = String::from(
            "strategy_id,user_id,symbol,realized_pnl,unrealized_pnl,fees_paid,funding_total,net_pnl
",
        );
        for strategy in strategies {
            csv.push_str(&strategy_summary_row(&strategy));
        }
        Ok(csv)
    }

    pub fn export_payments_csv(&self, user_email: &str) -> Result<String, SharedDbError> {
        let mut csv = String::from(
            "order_id,email,chain,asset,plan_code,amount,status,address,requested_at,paid_at,tx_hash
",
        );
        let mut orders = self.db.list_billing_orders()?;
        orders.retain(|order| order.email.eq_ignore_ascii_case(user_email));
        for order in orders {
            csv.push_str(&payment_row(&order));
        }
        Ok(csv)
    }

    fn trade_fill_inputs(&self, user_email: &str) -> Result<Vec<TradeFillInput>, SharedDbError> {
        let mut fills = Vec::new();
        for strategy in self.user_strategies(user_email)? {
            for fill in strategy.runtime.fills {
                fills.push(TradeFillInput {
                    strategy_id: strategy.id.clone(),
                    user_id: strategy.owner_email.clone(),
                    symbol: strategy.symbol.clone(),
                    quantity: fill.quantity,
                    entry_price: derive_entry_price(fill.price, fill.quantity, fill.realized_pnl),
                    exit_price: fill.price,
                    fee: fill.fee_amount.unwrap_or(Decimal::ZERO),
                    funding: Decimal::ZERO,
                });
            }
        }
        Ok(fills)
    }

    fn user_strategies(
        &self,
        user_email: &str,
    ) -> Result<Vec<shared_domain::strategy::Strategy>, SharedDbError> {
        self.db.list_strategies(user_email)
    }
}

fn derive_entry_price(
    exit_price: Decimal,
    quantity: Decimal,
    realized_pnl: Option<Decimal>,
) -> Decimal {
    if quantity.is_zero() {
        return exit_price;
    }

    match realized_pnl {
        Some(realized_pnl) => exit_price - (realized_pnl / quantity),
        None => exit_price,
    }
}

fn strategy_summary_row(strategy: &StrategyProfitSummary) -> String {
    format!(
        "{},{},{},{},{},{},{},{}
",
        strategy.strategy_id,
        strategy.user_id,
        strategy.symbol,
        format_decimal(strategy.realized_pnl),
        format_decimal(strategy.unrealized_pnl),
        format_decimal(strategy.fees_paid),
        format_decimal(strategy.funding_total),
        format_decimal(strategy.net_pnl),
    )
}

fn payment_row(order: &BillingOrderRecord) -> String {
    format!(
        "{},{},{},{},{},{},{},{},{},{},{}
",
        order.order_id,
        order.email,
        order.chain,
        order.asset,
        order.plan_code,
        order.amount,
        order.status,
        order
            .assignment
            .as_ref()
            .map(|assignment| assignment.address.clone())
            .unwrap_or_default(),
        order.requested_at.to_rfc3339(),
        order
            .paid_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_default(),
        order.tx_hash.clone().unwrap_or_default(),
    )
}

fn format_decimal(value: Decimal) -> String {
    value.normalize().to_string()
}
