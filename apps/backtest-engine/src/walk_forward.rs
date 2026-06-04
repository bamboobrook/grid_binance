use crate::market_data::KlineBar;
use crate::martingale::kline_engine::run_kline_screening;
use crate::martingale::metrics::{calculate_annualized_return_pct, MartingaleBacktestResult};
use crate::time_splits::{walk_forward_windows, WalkForwardConfig, WalkForwardWindow};
use serde::{Deserialize, Serialize};
use shared_domain::martingale::MartingalePortfolioConfig;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowBacktestResult {
    pub window_index: usize,
    pub window_name: String,
    pub train_start_ms: i64,
    pub train_end_ms: i64,
    pub test_start_ms: i64,
    pub test_end_ms: i64,
    pub train_total_return_pct: f64,
    pub train_max_drawdown_pct: f64,
    pub train_annualized_return_pct: Option<f64>,
    pub train_trade_count: u64,
    pub test_total_return_pct: f64,
    pub test_max_drawdown_pct: f64,
    pub test_annualized_return_pct: Option<f64>,
    pub test_trade_count: u64,
    pub wfe: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalkForwardValidationResult {
    pub candidate_id: String,
    pub windows: Vec<WindowBacktestResult>,
    pub avg_wfe: Option<f64>,
    pub median_wfe: Option<f64>,
    pub min_wfe: Option<f64>,
    pub max_wfe: Option<f64>,
    pub overfit_windows_count: usize,
    pub total_windows: usize,
    pub verdict: WalkForwardVerdict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalkForwardVerdict {
    Robust,
    Acceptable,
    Overfit,
    InsufficientData,
}

impl std::fmt::Display for WalkForwardVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalkForwardVerdict::Robust => write!(f, "robust"),
            WalkForwardVerdict::Acceptable => write!(f, "acceptable"),
            WalkForwardVerdict::Overfit => write!(f, "overfit"),
            WalkForwardVerdict::InsufficientData => write!(f, "insufficient_data"),
        }
    }
}

const MS_PER_DAY: f64 = 86_400_000.0;
const WFE_ROBUST_THRESHOLD: f64 = 0.5;
const WFE_OVERFIT_THRESHOLD: f64 = 0.3;

pub fn default_backtest_walk_forward_config(start_ms: i64, end_ms: i64) -> WalkForwardConfig {
    let total_ms = end_ms - start_ms;
    let train_ms = (total_ms as f64 * 0.60) as i64;
    let validate_ms = (total_ms as f64 * 0.05) as i64;
    let test_ms = (total_ms as f64 * 0.20) as i64;
    let step_ms = (total_ms as f64 * 0.20) as i64;
    WalkForwardConfig {
        start_ms,
        end_ms,
        train_ms,
        validate_ms,
        test_ms,
        step_ms,
    }
}

pub fn default_martingale_walk_forward_config() -> WalkForwardConfig {
    WalkForwardConfig {
        start_ms: 1_672_531_200_000,
        end_ms: 1_777_593_599_999,
        train_ms: 31_536_000_000,
        validate_ms: 7_776_000_000,
        test_ms: 7_776_000_000,
        step_ms: 15_552_000_000,
    }
}

fn filter_bars_by_range(bars: &[KlineBar], start_ms: i64, end_ms: i64) -> Vec<KlineBar> {
    bars.iter()
        .filter(|bar| bar.open_time_ms >= start_ms && bar.open_time_ms < end_ms)
        .cloned()
        .collect()
}

fn window_result_from_backtest(
    window: &WalkForwardWindow,
    window_index: usize,
    train_result: &MartingaleBacktestResult,
    test_result: &MartingaleBacktestResult,
) -> WindowBacktestResult {
    let train_days = (window.train.end_ms - window.train.start_ms) as f64 / MS_PER_DAY;
    let test_days = (window.test.end_ms - window.test.start_ms) as f64 / MS_PER_DAY;

    let train_ann = if train_result.metrics.total_return_pct.is_finite()
        && train_days > 0.0
        && train_result.metrics.max_capital_used_quote > 0.0
    {
        let initial = train_result.metrics.max_capital_used_quote;
        let ending = initial * (1.0 + train_result.metrics.total_return_pct / 100.0);
        calculate_annualized_return_pct(initial, ending, train_days)
    } else {
        None
    };

    let test_ann = if test_result.metrics.total_return_pct.is_finite()
        && test_days > 0.0
        && test_result.metrics.max_capital_used_quote > 0.0
    {
        let initial = test_result.metrics.max_capital_used_quote;
        let ending = initial * (1.0 + test_result.metrics.total_return_pct / 100.0);
        calculate_annualized_return_pct(initial, ending, test_days)
    } else {
        None
    };

    let train_return_dd = if train_result.metrics.max_drawdown_pct > 0.0 {
        train_result.metrics.total_return_pct / train_result.metrics.max_drawdown_pct
    } else if train_result.metrics.total_return_pct > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };

    let test_return_dd = if test_result.metrics.max_drawdown_pct > 0.0 {
        test_result.metrics.total_return_pct / test_result.metrics.max_drawdown_pct
    } else if test_result.metrics.total_return_pct > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };

    let wfe = if train_return_dd.is_finite() && train_return_dd > 0.0 {
        let ratio = test_return_dd / train_return_dd;
        if ratio.is_finite() {
            Some(ratio)
        } else {
            None
        }
    } else {
        None
    };

    WindowBacktestResult {
        window_index,
        window_name: window.train.name.replace("train", "wf"),
        train_start_ms: window.train.start_ms,
        train_end_ms: window.train.end_ms,
        test_start_ms: window.test.start_ms,
        test_end_ms: window.test.end_ms,
        train_total_return_pct: train_result.metrics.total_return_pct,
        train_max_drawdown_pct: train_result.metrics.max_drawdown_pct,
        train_annualized_return_pct: train_ann,
        train_trade_count: train_result.metrics.trade_count,
        test_total_return_pct: test_result.metrics.total_return_pct,
        test_max_drawdown_pct: test_result.metrics.max_drawdown_pct,
        test_annualized_return_pct: test_ann,
        test_trade_count: test_result.metrics.trade_count,
        wfe,
    }
}

pub fn run_walk_forward_validation(
    candidate_id: &str,
    portfolio_config: MartingalePortfolioConfig,
    bars: &[KlineBar],
    wf_config: WalkForwardConfig,
) -> Result<WalkForwardValidationResult, String> {
    let windows = walk_forward_windows(wf_config)?;

    if windows.is_empty() {
        return Ok(WalkForwardValidationResult {
            candidate_id: candidate_id.to_string(),
            windows: vec![],
            avg_wfe: None,
            median_wfe: None,
            min_wfe: None,
            max_wfe: None,
            overfit_windows_count: 0,
            total_windows: 0,
            verdict: WalkForwardVerdict::InsufficientData,
        });
    }

    let symbols: Vec<String> = portfolio_config
        .strategies
        .iter()
        .map(|s| s.symbol.clone())
        .collect();

    let candidate_bars: Vec<KlineBar> = bars
        .iter()
        .filter(|bar| symbols.iter().any(|s| s.eq_ignore_ascii_case(&bar.symbol)))
        .cloned()
        .collect();

    let mut window_results = Vec::with_capacity(windows.len());

    for (idx, window) in windows.iter().enumerate() {
        let train_bars =
            filter_bars_by_range(&candidate_bars, window.train.start_ms, window.train.end_ms);
        let test_bars =
            filter_bars_by_range(&candidate_bars, window.test.start_ms, window.test.end_ms);

        if train_bars.is_empty() || test_bars.is_empty() {
            continue;
        }

        let train_result = match run_kline_screening(portfolio_config.clone(), &train_bars) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if train_result
            .rejection_reasons
            .iter()
            .any(|r| r.contains("preflight"))
        {
            continue;
        }

        let test_result = match run_kline_screening(portfolio_config.clone(), &test_bars) {
            Ok(r) => r,
            Err(_) => continue,
        };

        window_results.push(window_result_from_backtest(
            window,
            idx,
            &train_result,
            &test_result,
        ));
    }

    let valid_wfes: Vec<f64> = window_results
        .iter()
        .filter_map(|w| w.wfe)
        .filter(|w| w.is_finite())
        .collect();

    let (avg_wfe, median_wfe, min_wfe, max_wfe) = if valid_wfes.is_empty() {
        (None, None, None, None)
    } else {
        let mut sorted = valid_wfes.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let avg = valid_wfes.iter().sum::<f64>() / valid_wfes.len() as f64;
        let median = if sorted.len() % 2 == 0 {
            (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
        } else {
            sorted[sorted.len() / 2]
        };
        (
            Some(avg),
            Some(median),
            Some(sorted[0]),
            Some(sorted[sorted.len() - 1]),
        )
    };

    let overfit_count = valid_wfes
        .iter()
        .filter(|wfe| **wfe < WFE_OVERFIT_THRESHOLD)
        .count();

    let verdict = if window_results.is_empty() {
        WalkForwardVerdict::InsufficientData
    } else if let Some(med) = median_wfe {
        if med >= WFE_ROBUST_THRESHOLD {
            WalkForwardVerdict::Robust
        } else if med >= WFE_OVERFIT_THRESHOLD {
            WalkForwardVerdict::Acceptable
        } else {
            WalkForwardVerdict::Overfit
        }
    } else {
        WalkForwardVerdict::InsufficientData
    };

    Ok(WalkForwardValidationResult {
        candidate_id: candidate_id.to_string(),
        windows: window_results,
        avg_wfe,
        median_wfe,
        min_wfe,
        max_wfe,
        overfit_windows_count: overfit_count,
        total_windows: windows.len(),
        verdict,
    })
}

pub fn wfe_penalty_factor(result: &WalkForwardValidationResult) -> f64 {
    match result.verdict {
        WalkForwardVerdict::Robust => 1.0,
        WalkForwardVerdict::Acceptable => {
            if let Some(med) = result.median_wfe {
                0.7 + 0.3 * (med / WFE_ROBUST_THRESHOLD).min(1.0)
            } else {
                0.7
            }
        }
        WalkForwardVerdict::Overfit => 0.5,
        WalkForwardVerdict::InsufficientData => 0.9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wfe_robust_when_test_matches_train() {
        let result = WalkForwardValidationResult {
            candidate_id: "test-1".to_string(),
            windows: vec![WindowBacktestResult {
                window_index: 0,
                window_name: "wf-1".to_string(),
                train_start_ms: 0,
                train_end_ms: 1000,
                test_start_ms: 1000,
                test_end_ms: 2000,
                train_total_return_pct: 50.0,
                train_max_drawdown_pct: 10.0,
                train_annualized_return_pct: Some(50.0),
                train_trade_count: 100,
                test_total_return_pct: 40.0,
                test_max_drawdown_pct: 10.0,
                test_annualized_return_pct: Some(40.0),
                test_trade_count: 50,
                wfe: Some(0.8),
            }],
            avg_wfe: Some(0.8),
            median_wfe: Some(0.8),
            min_wfe: Some(0.8),
            max_wfe: Some(0.8),
            overfit_windows_count: 0,
            total_windows: 1,
            verdict: WalkForwardVerdict::Robust,
        };
        assert_eq!(result.verdict, WalkForwardVerdict::Robust);
        assert_eq!(wfe_penalty_factor(&result), 1.0);
    }

    #[test]
    fn wfe_overfit_when_test_degrades_badly() {
        let result = WalkForwardValidationResult {
            candidate_id: "test-2".to_string(),
            windows: vec![WindowBacktestResult {
                window_index: 0,
                window_name: "wf-1".to_string(),
                train_start_ms: 0,
                train_end_ms: 1000,
                test_start_ms: 1000,
                test_end_ms: 2000,
                train_total_return_pct: 100.0,
                train_max_drawdown_pct: 10.0,
                train_annualized_return_pct: Some(100.0),
                train_trade_count: 200,
                test_total_return_pct: 5.0,
                test_max_drawdown_pct: 20.0,
                test_annualized_return_pct: Some(5.0),
                test_trade_count: 10,
                wfe: Some(0.025),
            }],
            avg_wfe: Some(0.025),
            median_wfe: Some(0.025),
            min_wfe: Some(0.025),
            max_wfe: Some(0.025),
            overfit_windows_count: 1,
            total_windows: 1,
            verdict: WalkForwardVerdict::Overfit,
        };
        assert_eq!(result.verdict, WalkForwardVerdict::Overfit);
        assert_eq!(wfe_penalty_factor(&result), 0.5);
    }

    #[test]
    fn wfe_acceptable_in_between() {
        let result = WalkForwardValidationResult {
            candidate_id: "test-3".to_string(),
            windows: vec![WindowBacktestResult {
                window_index: 0,
                window_name: "wf-1".to_string(),
                train_start_ms: 0,
                train_end_ms: 1000,
                test_start_ms: 1000,
                test_end_ms: 2000,
                train_total_return_pct: 60.0,
                train_max_drawdown_pct: 10.0,
                train_annualized_return_pct: Some(60.0),
                train_trade_count: 100,
                test_total_return_pct: 20.0,
                test_max_drawdown_pct: 12.0,
                test_annualized_return_pct: Some(20.0),
                test_trade_count: 40,
                wfe: Some(0.278),
            }],
            avg_wfe: Some(0.278),
            median_wfe: Some(0.278),
            min_wfe: Some(0.278),
            max_wfe: Some(0.278),
            overfit_windows_count: 1,
            total_windows: 1,
            verdict: WalkForwardVerdict::Acceptable,
        };
        assert_eq!(result.verdict, WalkForwardVerdict::Acceptable);
        let penalty = wfe_penalty_factor(&result);
        assert!(penalty > 0.7 && penalty < 1.0);
    }

    #[test]
    fn wfe_insufficient_data_when_no_windows() {
        let result = WalkForwardValidationResult {
            candidate_id: "test-4".to_string(),
            windows: vec![],
            avg_wfe: None,
            median_wfe: None,
            min_wfe: None,
            max_wfe: None,
            overfit_windows_count: 0,
            total_windows: 0,
            verdict: WalkForwardVerdict::InsufficientData,
        };
        assert_eq!(result.verdict, WalkForwardVerdict::InsufficientData);
        assert_eq!(wfe_penalty_factor(&result), 0.9);
    }

    #[test]
    fn default_config_generates_windows() {
        let config = default_martingale_walk_forward_config();
        let windows = walk_forward_windows(config).unwrap();
        assert!(!windows.is_empty(), "should produce at least one window");
        for w in &windows {
            assert!(w.train.end_ms > w.train.start_ms);
            assert!(w.test.end_ms > w.test.start_ms);
            assert_eq!(w.validate.start_ms, w.train.end_ms);
            assert_eq!(w.test.start_ms, w.validate.end_ms);
        }
    }

    #[test]
    fn filter_bars_by_range_works() {
        let bars = vec![
            KlineBar {
                symbol: "BTCUSDT".to_string(),
                open_time_ms: 100,
                open: 1.0,
                high: 2.0,
                low: 0.5,
                close: 1.5,
                volume: 10.0,
            },
            KlineBar {
                symbol: "BTCUSDT".to_string(),
                open_time_ms: 200,
                open: 1.0,
                high: 2.0,
                low: 0.5,
                close: 1.5,
                volume: 10.0,
            },
            KlineBar {
                symbol: "BTCUSDT".to_string(),
                open_time_ms: 300,
                open: 1.0,
                high: 2.0,
                low: 0.5,
                close: 1.5,
                volume: 10.0,
            },
            KlineBar {
                symbol: "BTCUSDT".to_string(),
                open_time_ms: 400,
                open: 1.0,
                high: 2.0,
                low: 0.5,
                close: 1.5,
                volume: 10.0,
            },
        ];
        let filtered = filter_bars_by_range(&bars, 150, 350);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].open_time_ms, 200);
        assert_eq!(filtered[1].open_time_ms, 300);
    }

    #[test]
    fn wfe_penalty_acceptable_scales_linearly() {
        let r1 = WalkForwardValidationResult {
            candidate_id: "t".into(),
            windows: vec![],
            avg_wfe: None,
            median_wfe: Some(0.3),
            min_wfe: None,
            max_wfe: None,
            overfit_windows_count: 0,
            total_windows: 1,
            verdict: WalkForwardVerdict::Acceptable,
        };
        let p1 = wfe_penalty_factor(&r1);
        let r2 = WalkForwardValidationResult {
            candidate_id: "t".into(),
            windows: vec![],
            avg_wfe: None,
            median_wfe: Some(0.5),
            min_wfe: None,
            max_wfe: None,
            overfit_windows_count: 0,
            total_windows: 1,
            verdict: WalkForwardVerdict::Acceptable,
        };
        let p2 = wfe_penalty_factor(&r2);
        assert!(
            p2 > p1,
            "higher median WFE should give higher penalty factor"
        );
        assert!(p1 >= 0.7);
        assert!(p2 <= 1.0);
    }
}
