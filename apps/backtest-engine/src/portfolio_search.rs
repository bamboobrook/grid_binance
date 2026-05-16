use crate::search::SearchCandidate;

#[derive(Debug, Clone)]
pub struct EvaluatedCandidate {
    pub candidate: SearchCandidate,
    pub score: f64,
    pub return_pct: f64,
    pub max_drawdown_pct: f64,
    pub survival_passed: bool,
}

#[derive(Debug, Clone)]
pub struct PortfolioArtifact {
    pub top3: Vec<EvaluatedCandidate>,
    pub total_candidates: usize,
    pub survivors: usize,
}

pub fn build_portfolio_top3(
    candidates: &[EvaluatedCandidate],
    max_drawdown_pct: f64,
) -> PortfolioArtifact {
    let survivors: Vec<&EvaluatedCandidate> = candidates
        .iter()
        .filter(|c| c.survival_passed && c.max_drawdown_pct <= max_drawdown_pct)
        .collect();

    let mut ranked: Vec<&EvaluatedCandidate> = survivors.clone();
    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    let top3: Vec<EvaluatedCandidate> = ranked.iter().take(3).cloned().cloned().collect();

    PortfolioArtifact {
        top3,
        total_candidates: candidates.len(),
        survivors: survivors.len(),
    }
}

pub fn score_candidate(return_pct: f64, max_drawdown_pct: f64, trade_count: u64) -> f64 {
    if max_drawdown_pct <= 0.0 {
        return 0.0;
    }
    let calmar = return_pct / max_drawdown_pct;
    let activity_bonus = if trade_count > 0 { 1.0 } else { 0.0 };
    calmar * activity_bonus
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::SearchCandidate;
    use shared_domain::martingale::{
        MartingaleDirection, MartingaleDirectionMode, MartingaleMarketKind, MartingalePortfolioConfig,
        MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel, MartingaleStrategyConfig,
        MartingaleTakeProfitModel,
    };
    use rust_decimal::Decimal;

    fn dummy_candidate(id: &str) -> SearchCandidate {
        let strategy = MartingaleStrategyConfig {
            strategy_id: id.to_owned(),
            symbol: "BTCUSDT".to_owned(),
            market: MartingaleMarketKind::UsdMFutures,
            direction: MartingaleDirection::Long,
            direction_mode: MartingaleDirectionMode::LongOnly,
            margin_mode: None,
            leverage: Some(3),
            spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
            sizing: MartingaleSizingModel::Multiplier {
                first_order_quote: Decimal::new(100, 0),
                multiplier: Decimal::new(15, 1),
                max_legs: 5,
            },
            take_profit: MartingaleTakeProfitModel::Percent { bps: 100 },
            stop_loss: None,
            indicators: Vec::new(),
            entry_triggers: Vec::new(),
            risk_limits: MartingaleRiskLimits::default(),
        };
        SearchCandidate {
            candidate_id: id.to_owned(),
            config: MartingalePortfolioConfig {
                direction_mode: MartingaleDirectionMode::LongOnly,
                strategies: vec![strategy],
                risk_limits: MartingaleRiskLimits::default(),
            },
        }
    }

    #[test]
    fn portfolio_top3_picks_best_three_by_score() {
        let candidates = vec![
            EvaluatedCandidate {
                candidate: dummy_candidate("a"),
                score: 1.5,
                return_pct: 30.0,
                max_drawdown_pct: 20.0,
                survival_passed: true,
            },
            EvaluatedCandidate {
                candidate: dummy_candidate("b"),
                score: 3.0,
                return_pct: 60.0,
                max_drawdown_pct: 20.0,
                survival_passed: true,
            },
            EvaluatedCandidate {
                candidate: dummy_candidate("c"),
                score: 2.0,
                return_pct: 40.0,
                max_drawdown_pct: 20.0,
                survival_passed: true,
            },
            EvaluatedCandidate {
                candidate: dummy_candidate("d"),
                score: 0.5,
                return_pct: 10.0,
                max_drawdown_pct: 20.0,
                survival_passed: true,
            },
        ];

        let portfolio = build_portfolio_top3(&candidates, 25.0);
        assert_eq!(portfolio.top3.len(), 3);
        assert_eq!(portfolio.top3[0].candidate.candidate_id, "b");
        assert_eq!(portfolio.top3[1].candidate.candidate_id, "c");
        assert_eq!(portfolio.top3[2].candidate.candidate_id, "a");
        assert_eq!(portfolio.total_candidates, 4);
        assert_eq!(portfolio.survivors, 4);
    }

    #[test]
    fn portfolio_filters_by_drawdown_limit() {
        let candidates = vec![
            EvaluatedCandidate {
                candidate: dummy_candidate("a"),
                score: 3.0,
                return_pct: 60.0,
                max_drawdown_pct: 15.0,
                survival_passed: true,
            },
            EvaluatedCandidate {
                candidate: dummy_candidate("b"),
                score: 2.0,
                return_pct: 40.0,
                max_drawdown_pct: 25.0,
                survival_passed: true,
            },
        ];

        let portfolio = build_portfolio_top3(&candidates, 20.0);
        assert_eq!(portfolio.top3.len(), 1);
        assert_eq!(portfolio.top3[0].candidate.candidate_id, "a");
        assert_eq!(portfolio.survivors, 1);
    }

    #[test]
    fn score_candidate_penalizes_high_drawdown() {
        let good = score_candidate(30.0, 10.0, 10);
        let bad = score_candidate(30.0, 30.0, 10);
        assert!(good > bad);
    }
}
