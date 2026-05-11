use crate::martingale::metrics::MartingaleBacktestResult;

#[derive(Debug, Clone, PartialEq)]
pub struct ScoringConfig {
    pub max_global_drawdown_pct: f64,
    pub max_strategy_drawdown_pct: f64,
    pub max_budget_quote: f64,
    pub max_stop_count: u64,
    pub min_trade_count: u64,
    pub min_data_quality_score: f64,
    pub weight_return: f64,
    pub weight_calmar: f64,
    pub weight_sortino: f64,
    pub weight_drawdown: f64,
    pub weight_stop_frequency: f64,
    pub weight_capital_utilization: f64,
    pub weight_trade_stability: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateScore {
    pub survival_valid: bool,
    pub rank_score: f64,
    pub raw_score: f64,
    pub rejection_reasons: Vec<String>,
}

const VALID_RANK_BASE: f64 = 1.0e12;
const INVALID_RANK_BASE: f64 = -1.0e12;
const RANK_SCORE_SPREAD: f64 = 1.0e9;

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            max_global_drawdown_pct: 40.0,
            max_strategy_drawdown_pct: 40.0,
            max_budget_quote: f64::MAX,
            max_stop_count: u64::MAX,
            min_trade_count: 1,
            min_data_quality_score: 0.95,
            weight_return: 1.0,
            weight_calmar: 0.8,
            weight_sortino: 0.5,
            weight_drawdown: 0.8,
            weight_stop_frequency: 0.5,
            weight_capital_utilization: 0.3,
            weight_trade_stability: 0.3,
        }
    }
}

pub fn score_candidate(
    result: &MartingaleBacktestResult,
    config: &ScoringConfig,
) -> CandidateScore {
    let mut reasons = result.rejection_reasons.clone();
    let metrics = &result.metrics;

    if !metrics.survival_passed {
        push_reason(&mut reasons, "survival_failed");
    }
    if reasons.iter().any(|reason| reason.contains("liquidation")) {
        push_reason(&mut reasons, "liquidation_hit");
    }
    let global_drawdown_pct = metrics
        .global_drawdown_pct
        .unwrap_or(metrics.max_drawdown_pct);
    let max_strategy_drawdown_pct = metrics
        .max_strategy_drawdown_pct
        .unwrap_or(metrics.max_drawdown_pct);
    let data_quality_score = metrics.data_quality_score.unwrap_or(1.0);

    if global_drawdown_pct > config.max_global_drawdown_pct {
        push_reason(&mut reasons, "global_drawdown_exceeded");
    }
    if max_strategy_drawdown_pct > config.max_strategy_drawdown_pct {
        push_reason(&mut reasons, "strategy_drawdown_exceeded");
    }
    if metrics.max_capital_used_quote > config.max_budget_quote {
        push_reason(&mut reasons, "budget_exceeded");
    }
    if metrics.stop_count > config.max_stop_count {
        push_reason(&mut reasons, "excessive_stop_count");
    }
    if metrics.trade_count < config.min_trade_count
        || data_quality_score < config.min_data_quality_score
    {
        push_reason(&mut reasons, "insufficient_data_quality");
    }

    let survival_valid = reasons.is_empty();
    let stop_frequency = if metrics.trade_count > 0 {
        metrics.stop_count as f64 / metrics.trade_count as f64
    } else {
        1.0
    };
    let drawdown = global_drawdown_pct.max(max_strategy_drawdown_pct).max(0.0);
    let calmar = metrics.total_return_pct / drawdown.max(1.0);
    let sortino = if metrics.total_return_pct >= 0.0 {
        metrics.total_return_pct / (drawdown / 2.0).max(1.0)
    } else {
        metrics.total_return_pct
    };
    let capital_utilization =
        if config.max_budget_quote.is_finite() && config.max_budget_quote > 0.0 {
            (metrics.max_capital_used_quote / config.max_budget_quote).clamp(0.0, 1.0)
        } else {
            0.5
        };
    let trade_stability = (metrics.trade_count as f64 / 30.0).clamp(0.0, 1.0);

    let raw_score = config.weight_return * metrics.total_return_pct
        + config.weight_calmar * calmar
        + config.weight_sortino * sortino
        - config.weight_drawdown * drawdown
        - config.weight_stop_frequency * stop_frequency * 100.0
        + config.weight_capital_utilization * capital_utilization * 100.0
        + config.weight_trade_stability * trade_stability * 100.0;
    let raw_score = finite_or(raw_score, f64::MIN / 4.0);
    let bounded_score = raw_score.clamp(-RANK_SCORE_SPREAD, RANK_SCORE_SPREAD);
    let rank_score = if survival_valid {
        VALID_RANK_BASE + bounded_score
    } else {
        INVALID_RANK_BASE + bounded_score
    };

    CandidateScore {
        survival_valid,
        rank_score,
        raw_score,
        rejection_reasons: reasons,
    }
}

fn push_reason(reasons: &mut Vec<String>, reason: &str) {
    if !reasons.iter().any(|existing| existing == reason) {
        reasons.push(reason.to_string());
    }
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}
