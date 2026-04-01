use rust_decimal::Decimal;
use shared_domain::analytics::{AnalyticsReport, TradeFillInput};
use trading_engine::statistics::{
    compute_analytics_report, compute_fill_views, export_fill_views_csv,
};

#[derive(Clone, Default)]
pub struct AnalyticsService;

impl AnalyticsService {
    pub fn report(&self) -> AnalyticsReport {
        compute_analytics_report(&sample_fills())
    }

    pub fn export_csv(&self) -> String {
        let fills = compute_fill_views(&sample_fills());
        export_fill_views_csv(&fills)
    }
}

fn sample_fills() -> Vec<TradeFillInput> {
    vec![
        TradeFillInput {
            strategy_id: "strategy-1".to_string(),
            user_id: "user-1".to_string(),
            symbol: "BTCUSDT".to_string(),
            quantity: decimal(1, 0),
            entry_price: decimal(100, 0),
            exit_price: decimal(110, 0),
            fee: decimal(1, 0),
            funding: Decimal::ZERO,
        },
        TradeFillInput {
            strategy_id: "strategy-1".to_string(),
            user_id: "user-1".to_string(),
            symbol: "BTCUSDT".to_string(),
            quantity: decimal(2, 0),
            entry_price: decimal(110, 0),
            exit_price: decimal(105, 0),
            fee: decimal(5, 1),
            funding: decimal(-2, 1),
        },
        TradeFillInput {
            strategy_id: "strategy-2".to_string(),
            user_id: "user-1".to_string(),
            symbol: "ETHUSDT".to_string(),
            quantity: decimal(3, 0),
            entry_price: decimal(50, 0),
            exit_price: decimal(55, 0),
            fee: decimal(75, 2),
            funding: decimal(1, 1),
        },
    ]
}

fn decimal(value: i64, scale: u32) -> Decimal {
    Decimal::new(value, scale)
}
