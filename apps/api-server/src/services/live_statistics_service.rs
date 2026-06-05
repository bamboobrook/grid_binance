use serde::Serialize;
use shared_db::{SharedDb, SharedDbError};
use trading_engine::statistics::{
    compute_live_statistics_from_db, compute_position_count_for_strategies,
    LiveStatisticsSnapshot,
};

#[derive(Debug, Clone, Serialize)]
pub struct LiveStatisticsResponse {
    pub open_order_count: usize,
    pub position_count: usize,
    pub realized_pnl: String,
    pub unrealized_pnl: String,
    pub fees_paid: String,
    pub funding_total: String,
    pub wallet_balance: String,
    pub last_user_stream_event_at: Option<String>,
    pub last_rest_reconcile_at: Option<String>,
    pub stats_stale: bool,
    pub computed_at: String,
}

impl From<LiveStatisticsSnapshot> for LiveStatisticsResponse {
    fn from(snapshot: LiveStatisticsSnapshot) -> Self {
        Self {
            open_order_count: snapshot.open_order_count,
            position_count: snapshot.position_count,
            realized_pnl: snapshot.realized_pnl,
            unrealized_pnl: snapshot.unrealized_pnl,
            fees_paid: snapshot.fees_paid,
            funding_total: snapshot.funding_total,
            wallet_balance: snapshot.wallet_balance,
            last_user_stream_event_at: snapshot.last_user_stream_event_at,
            last_rest_reconcile_at: snapshot.last_rest_reconcile_at,
            stats_stale: snapshot.stats_stale,
            computed_at: snapshot.computed_at,
        }
    }
}

#[derive(Clone)]
pub struct LiveStatisticsService {
    db: SharedDb,
    stale_threshold_secs: i64,
}

impl Default for LiveStatisticsService {
    fn default() -> Self {
        Self {
            db: SharedDb::ephemeral().expect("ephemeral live-stats db should initialize"),
            stale_threshold_secs: 600,
        }
    }
}

impl LiveStatisticsService {
    pub fn new(db: SharedDb, stale_threshold_secs: i64) -> Self {
        Self {
            db,
            stale_threshold_secs,
        }
    }

    #[allow(dead_code)]
    pub fn compute_live_stats(
        &self,
        email: &str,
    ) -> Result<LiveStatisticsResponse, SharedDbError> {
        let snapshot =
            compute_live_statistics_from_db(&self.db, email, None, self.stale_threshold_secs)?;
        let position_count = compute_position_count_for_strategies(&self.db, email, None)?;
        let mut response = LiveStatisticsResponse::from(snapshot);
        response.position_count = position_count;
        Ok(response)
    }

    pub fn compute_portfolio_live_stats(
        &self,
        email: &str,
        portfolio_id: &str,
    ) -> Result<LiveStatisticsResponse, SharedDbError> {
        let portfolio = self
            .db
            .backtest_repo()
            .get_martingale_portfolio(email, portfolio_id)?
            .ok_or_else(|| {
                SharedDbError::new(format!(
                    "portfolio {portfolio_id} not found or not owned by {email}"
                ))
            })?;

        let strategy_ids: Vec<String> = portfolio
            .config
            .get("portfolio_config")
            .and_then(|cfg| cfg.get("strategies"))
            .and_then(|strategies| strategies.as_array())
            .into_iter()
            .flatten()
            .filter_map(|strategy| {
                strategy
                    .get("strategy_id")
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        let snapshot = if strategy_ids.is_empty() {
            LiveStatisticsSnapshot::default()
        } else {
            compute_live_statistics_from_db(
                &self.db,
                email,
                Some(&strategy_ids),
                self.stale_threshold_secs,
            )?
        };
        let position_count =
            compute_position_count_for_strategies(&self.db, email, Some(&strategy_ids))?;
        let mut response = LiveStatisticsResponse::from(snapshot);
        response.position_count = position_count;

        // Augment with live-snapshot data when Strategy runtime is empty but
        // the portfolio executor has generated orders (reconcile_running_martingale_portfolios
        // writes order_count to risk_summary).
        if response.open_order_count == 0 {
            if let Some(live_orders) = portfolio
                .risk_summary
                .get("order_count")
                .and_then(|v| v.as_u64())
            {
                response.open_order_count = live_orders as usize;
            }
        }

        Ok(response)
    }
}
