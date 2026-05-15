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

const VALID_RANK_EPSILON: f64 = f64::EPSILON;

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
    if metrics.total_return_pct <= 0.0 {
        push_reason(&mut reasons, "negative_return");
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
    let trade_stability = (metrics.trade_count as f64 / 30.0).clamp(0.0, 1.0);

    if !survival_valid {
        return CandidateScore {
            survival_valid,
            rank_score: 0.0,
            raw_score: 0.0,
            rejection_reasons: reasons,
        };
    }

    let ratio = return_drawdown_ratio(metrics.total_return_pct, drawdown);
    let annualized = metrics
        .annualized_return_pct
        .unwrap_or(metrics.total_return_pct);
    let stop_penalty = stop_frequency * 20.0;
    let leverage_penalty = metrics.max_leverage_used.unwrap_or(1.0).max(1.0).ln() * 4.0;
    let liquidation_penalty = if metrics.min_liquidation_buffer_pct.unwrap_or(100.0) < 15.0 {
        20.0
    } else {
        0.0
    };

    let return_points = weighted_points(config.weight_return, 35.0, (ratio / 4.0).clamp(0.0, 1.0));
    let annualized_points = weighted_points(
        config.weight_calmar,
        25.0,
        (annualized / 80.0).clamp(0.0, 1.0),
    );
    let drawdown_points = weighted_points(
        config.weight_drawdown,
        20.0,
        ((100.0 - drawdown) / 100.0).clamp(0.0, 1.0),
    );
    let monthly_win_points = weighted_points(
        config.weight_sortino,
        10.0,
        (metrics.monthly_win_rate_pct.unwrap_or(50.0) / 100.0).clamp(0.0, 1.0),
    );
    let trade_stability_points =
        weighted_points(config.weight_trade_stability, 10.0, trade_stability);
    let positive_weight = positive_weight_sum(&[
        (config.weight_return, 35.0),
        (config.weight_calmar, 25.0),
        (config.weight_drawdown, 20.0),
        (config.weight_sortino, 10.0),
        (config.weight_trade_stability, 10.0),
    ]);
    let positive_score = if positive_weight > 0.0 {
        (return_points
            + annualized_points
            + drawdown_points
            + monthly_win_points
            + trade_stability_points)
            / positive_weight
            * 100.0
    } else {
        0.0
    };
    let capital_bonus = if config.max_budget_quote.is_finite() && config.max_budget_quote > 0.0 {
        (1.0 - (metrics.max_capital_used_quote / config.max_budget_quote).clamp(0.0, 1.0))
            * 10.0
            * config.weight_capital_utilization.max(0.0)
    } else {
        0.0
    };

    let raw_score = positive_score + capital_bonus
        - config.weight_stop_frequency.max(0.0) * stop_penalty
        - leverage_penalty
        - liquidation_penalty;
    let raw_score = clamp_score(raw_score);
    let rank_score = (raw_score + VALID_RANK_EPSILON).min(100.0);

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

fn clamp_score(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 100.0)
    } else {
        0.0
    }
}

fn return_drawdown_ratio(total_return_pct: f64, drawdown_pct: f64) -> f64 {
    if total_return_pct <= 0.0 {
        0.0
    } else {
        total_return_pct / drawdown_pct.max(1.0)
    }
}

fn weighted_points(weight: f64, points: f64, factor: f64) -> f64 {
    weight.max(0.0) * points * factor
}

fn positive_weight_sum(weights: &[(f64, f64)]) -> f64 {
    weights
        .iter()
        .map(|(weight, points)| weight.max(0.0) * points)
        .sum()
}
