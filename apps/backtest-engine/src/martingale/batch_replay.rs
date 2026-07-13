//! Round 13 Task P1: Batch CPU replay infrastructure.
//!
//! Preloads market data (1m bars + funding rates) once into memory, then
//! runs multiple martingale configs in parallel using Rayon. Each config
//! gets its own independent runtime state — no shared mutable data.
//!
//! ## Parity guarantee
//!
//! Batch replay calls the same event engine with immutable shared data. Exact
//! CLI parity additionally requires the caller to apply the CLI's raw-JSON
//! portfolio-weight caps before passing the typed config.
//!
//! ## Usage
//!
//! ```no_run
//! use backtest_engine::martingale::batch_replay::BatchReplay;
//! use shared_domain::martingale::MartingalePortfolioConfig;
//!
//! # fn example(configs: Vec<(String, MartingalePortfolioConfig)>) -> Result<(), String> {
//! let start_ms = 1_672_531_200_000;
//! let end_ms = start_ms + 86_400_000;
//! let mut batch = BatchReplay::new(
//!     "data/market_data_full.db",
//!     "data/funding_rates_round12.db",
//! )?;
//! batch.preload_symbols(&["BNBUSDT", "ETHUSDT"], start_ms, end_ms)?;
//! let _results = batch.run_configs_parallel(configs, 4_999.0, start_ms, end_ms);
//! # Ok(())
//! # }
//! ```

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use rayon::prelude::*;

use crate::market_data::KlineBar;
use crate::martingale::indicator_runtime::extract_symbol_dependencies;
use crate::martingale::kline_engine::{run_kline_screening_with_funding, FundingRatePoint};
use crate::sqlite_market_data::load_funding_rates_readonly;
use rusqlite::Connection;
use shared_domain::martingale::MartingalePortfolioConfig;

/// Preloaded market data for batch replay.
pub struct BatchReplay {
    market_db_path: String,
    funding_db_path: String,
    /// Preloaded bars keyed by symbol.
    bars: HashMap<String, Vec<KlineBar>>,
    /// Preloaded funding rates keyed by symbol.
    funding: HashMap<String, Vec<FundingRatePoint>>,
}

/// Result of a single batch replay.
#[derive(Debug, Clone)]
pub struct BatchReplayResult {
    pub config_label: String,
    pub annualized_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub total_return_pct: f64,
    pub trade_count: u64,
    pub max_capital_used_quote: f64,
    pub budget_blocked_legs: u64,
    pub error: Option<String>,
}

#[derive(Debug)]
struct PreparedBatchData {
    bars: Vec<KlineBar>,
    funding: Vec<FundingRatePoint>,
}

impl BatchReplay {
    /// Create a new batch replay context.
    pub fn new(market_db_path: &str, funding_db_path: &str) -> Result<Self, String> {
        // Verify DBs exist
        if !std::path::Path::new(market_db_path).exists() {
            return Err(format!("market DB not found: {}", market_db_path));
        }
        if !std::path::Path::new(funding_db_path).exists() {
            return Err(format!("funding DB not found: {}", funding_db_path));
        }
        Ok(Self {
            market_db_path: market_db_path.to_string(),
            funding_db_path: funding_db_path.to_string(),
            bars: HashMap::new(),
            funding: HashMap::new(),
        })
    }

    /// Preload bars and funding for the given symbols within [start_ms, end_ms].
    pub fn preload_symbols(
        &mut self,
        symbols: &[&str],
        start_ms: i64,
        end_ms: i64,
    ) -> Result<(), String> {
        let conn =
            Connection::open(&self.market_db_path).map_err(|e| format!("open market DB: {}", e))?;

        for symbol in symbols {
            // Load bars
            let mut stmt = conn
                .prepare(
                    "SELECT symbol, open_time, open, high, low, close, volume
                     FROM klines WHERE symbol = ? AND open_time >= ? AND open_time <= ?
                     ORDER BY open_time",
                )
                .map_err(|e| format!("prepare bars query: {}", e))?;

            let bars: Vec<KlineBar> = stmt
                .query_map(rusqlite::params![symbol, start_ms, end_ms], |row| {
                    Ok(KlineBar {
                        symbol: row.get(0)?,
                        open_time_ms: row.get(1)?,
                        open: row.get(2)?,
                        high: row.get(3)?,
                        low: row.get(4)?,
                        close: row.get(5)?,
                        volume: row.get(6)?,
                    })
                })
                .map_err(|e| format!("query bars for {}: {}", symbol, e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("decode bars for {}: {}", symbol, e))?;

            self.bars.insert(symbol.to_string(), bars);

            // Load funding for this symbol
            let symbols_vec = vec![symbol.to_string()];
            let funding_all =
                load_funding_rates_readonly(&self.funding_db_path, &symbols_vec, start_ms, end_ms)
                    .map_err(|e| format!("load funding for {}: {}", symbol, e))?;
            self.funding.insert(symbol.to_string(), funding_all);
        }

        Ok(())
    }

    fn symbol_sets(config: &MartingalePortfolioConfig) -> (Vec<String>, Vec<String>) {
        let traded = config
            .strategies
            .iter()
            .map(|strategy| strategy.symbol.trim().to_uppercase())
            .collect::<BTreeSet<_>>();
        let all = traded
            .iter()
            .cloned()
            .chain(extract_symbol_dependencies(config))
            .collect::<BTreeSet<_>>();
        (traded.into_iter().collect(), all.into_iter().collect())
    }

    fn prepare_data(
        &self,
        config: &MartingalePortfolioConfig,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<PreparedBatchData, String> {
        let (traded_symbols, all_symbols) = Self::symbol_sets(config);
        let mut all_bars = Vec::new();
        let mut all_funding = Vec::new();

        for symbol in &all_symbols {
            let bars = self
                .bars
                .get(symbol)
                .ok_or_else(|| format!("symbol {symbol} was not preloaded"))?;
            all_bars.extend(
                bars.iter()
                    .filter(|bar| bar.open_time_ms >= start_ms && bar.open_time_ms <= end_ms)
                    .cloned(),
            );
        }
        for symbol in &traded_symbols {
            let funding = self
                .funding
                .get(symbol)
                .ok_or_else(|| format!("funding for {symbol} was not preloaded"))?;
            all_funding.extend(
                funding
                    .iter()
                    .filter(|point| {
                        point.funding_time_ms >= start_ms && point.funding_time_ms <= end_ms
                    })
                    .cloned(),
            );
        }

        all_bars.sort_by(|left, right| {
            left.open_time_ms
                .cmp(&right.open_time_ms)
                .then_with(|| left.symbol.cmp(&right.symbol))
        });
        all_funding.sort_by(|left, right| {
            left.funding_time_ms
                .cmp(&right.funding_time_ms)
                .then_with(|| left.symbol.cmp(&right.symbol))
        });
        Ok(PreparedBatchData {
            bars: all_bars,
            funding: all_funding,
        })
    }

    /// Run a single already-resolved config using preloaded data.
    pub fn run_single(
        &self,
        config: &MartingalePortfolioConfig,
        budget: f64,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<BatchReplayResult, String> {
        let data = self.prepare_data(config, start_ms, end_ms)?;
        let result =
            run_kline_screening_with_funding(config.clone(), &data.bars, &data.funding, budget)?;

        Ok(BatchReplayResult {
            config_label: String::new(), // caller fills
            annualized_return_pct: result.metrics.annualized_return_pct.unwrap_or(-999.0),
            max_drawdown_pct: result.metrics.max_drawdown_pct,
            total_return_pct: result.metrics.total_return_pct,
            trade_count: result.metrics.trade_count,
            max_capital_used_quote: result.metrics.max_capital_used_quote,
            budget_blocked_legs: result.rejection_reasons.len() as u64,
            error: None,
        })
    }

    /// Run multiple configs in parallel using Rayon. Configs with the same
    /// traded/dependency symbols share one immutable merged bar/funding slice.
    pub fn run_configs_parallel(
        &self,
        configs: Vec<(String, MartingalePortfolioConfig)>,
        budget: f64,
        start_ms: i64,
        end_ms: i64,
    ) -> Vec<BatchReplayResult> {
        let mut data_by_symbols = HashMap::new();
        for (_, config) in &configs {
            let key = Self::symbol_sets(config);
            data_by_symbols
                .entry(key)
                .or_insert_with(|| self.prepare_data(config, start_ms, end_ms).map(Arc::new));
        }

        configs
            .into_par_iter()
            .map(|(label, config)| {
                let key = Self::symbol_sets(&config);
                let data = data_by_symbols.get(&key).expect("prepared key must exist");
                let data = match data {
                    Ok(data) => data,
                    Err(error) => {
                        return BatchReplayResult {
                            config_label: label,
                            annualized_return_pct: -999.0,
                            max_drawdown_pct: 999.0,
                            total_return_pct: -999.0,
                            trade_count: 0,
                            max_capital_used_quote: 0.0,
                            budget_blocked_legs: 0,
                            error: Some(error.clone()),
                        };
                    }
                };
                match run_kline_screening_with_funding(config, &data.bars, &data.funding, budget) {
                    Ok(result) => BatchReplayResult {
                        config_label: label,
                        annualized_return_pct: result
                            .metrics
                            .annualized_return_pct
                            .unwrap_or(-999.0),
                        max_drawdown_pct: result.metrics.max_drawdown_pct,
                        total_return_pct: result.metrics.total_return_pct,
                        trade_count: result.metrics.trade_count,
                        max_capital_used_quote: result.metrics.max_capital_used_quote,
                        budget_blocked_legs: result.rejection_reasons.len() as u64,
                        error: None,
                    },
                    Err(e) => BatchReplayResult {
                        config_label: label,
                        annualized_return_pct: -999.0,
                        max_drawdown_pct: 999.0,
                        total_return_pct: -999.0,
                        trade_count: 0,
                        max_capital_used_quote: 0.0,
                        budget_blocked_legs: 0,
                        error: Some(e),
                    },
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_replay_result_is_debug_clone() {
        let r = BatchReplayResult {
            config_label: "test".to_string(),
            annualized_return_pct: 10.0,
            max_drawdown_pct: 5.0,
            total_return_pct: 100.0,
            trade_count: 100,
            max_capital_used_quote: 5000.0,
            budget_blocked_legs: 0,
            error: None,
        };
        let cloned = r.clone();
        assert_eq!(cloned.config_label, "test");
        assert!((cloned.annualized_return_pct - 10.0).abs() < 1e-9);
    }

    #[test]
    fn batch_replay_rejects_missing_db() {
        let result = BatchReplay::new("nonexistent.db", "also_nonexistent.db");
        assert!(result.is_err());
    }
}
