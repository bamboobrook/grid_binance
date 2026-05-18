use crate::martingale::metrics::{build_drawdown_curve, calculate_annualized_return_pct, DrawdownPoint, EquityPoint};
use crate::search::SearchCandidate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortfolioMember {
    pub candidate_id: String,
    pub symbol: String,
    pub direction: String,
    pub allocation_pct: f64,
    pub return_pct: f64,
    pub max_drawdown_pct: f64,
    pub annualized_return_pct: Option<f64>,
    pub trade_count: u64,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightedPortfolio {
    pub members: Vec<PortfolioMember>,
    pub member_count: usize,
    pub allocation_pct: f64,
    pub return_pct: f64,
    pub max_drawdown_pct: f64,
    pub annualized_return_pct: Option<f64>,
    pub trade_count: u64,
    pub score: f64,
    pub equity_curve: Vec<EquityPoint>,
    pub drawdown_curve: Vec<DrawdownPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortfolioTop3Artifact {
    pub top3: Vec<WeightedPortfolio>,
    pub eligible_candidate_count: usize,
}

#[derive(Debug, Clone)]
pub struct EvaluatedCandidate {
    pub candidate: SearchCandidate,
    pub score: f64,
    pub return_pct: f64,
    pub max_drawdown_pct: f64,
    pub survival_passed: bool,
    pub planned_margin_quote: f64,
    pub trade_count: u64,
    pub annualized_return_pct: Option<f64>,
    pub equity_curve: Vec<EquityPoint>,
    pub drawdown_curve: Vec<DrawdownPoint>,
    pub trades: Vec<crate::martingale::metrics::MartingaleTradeDetail>,
}

pub fn build_portfolio_top3(candidates: &[EvaluatedCandidate], max_drawdown_pct: f64) -> PortfolioTop3Artifact {
    let eligible: Vec<&EvaluatedCandidate> = candidates
        .iter()
        .filter(|c| c.survival_passed && c.max_drawdown_pct <= max_drawdown_pct && c.return_pct > 0.0)
        .collect();

    let eligible_count = eligible.len();

    if eligible_count < 2 {
        return PortfolioTop3Artifact {
            top3: Vec::new(),
            eligible_candidate_count: eligible_count,
        };
    }

    let mut scored_portfolios: Vec<WeightedPortfolio> = Vec::new();
    let max_combos = 60usize;
    let mut combo_count = 0usize;

    for i in 0..eligible_count {
        for j in (i + 1)..eligible_count {
            for k in (j + 1)..eligible_count.min(j + 4) {
                let members_idx = if eligible_count >= 3 && k < eligible_count {
                    vec![i, j, k]
                } else {
                    vec![i, j]
                };
                let portfolio = build_weighted_portfolio(&eligible, &members_idx);
                if let Some(p) = portfolio {
                    scored_portfolios.push(p);
                }
                combo_count += 1;
                if combo_count >= max_combos {
                    break;
                }
            }
            if combo_count >= max_combos {
                break;
            }
        }
        if combo_count >= max_combos {
            break;
        }
    }

    scored_portfolios.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored_portfolios.truncate(3);

    PortfolioTop3Artifact {
        top3: scored_portfolios,
        eligible_candidate_count: eligible_count,
    }
}

fn build_weighted_portfolio(eligible: &[&EvaluatedCandidate], member_indices: &[usize]) -> Option<WeightedPortfolio> {
    let members_data: Vec<&EvaluatedCandidate> = member_indices.iter().map(|&i| eligible[i]).collect();

    let symbols: Vec<&str> = members_data
        .iter()
        .map(|c| c.candidate.config.strategies.first().map(|s| s.symbol.as_str()).unwrap_or(""))
        .collect();
    let unique_symbols: Vec<&str> = symbols.iter().copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
    if unique_symbols.len() < members_data.len().min(2) {
        return None;
    }

    let n = members_data.len() as f64;
    let equal_weight = 100.0 / n;

    let portfolio_members: Vec<PortfolioMember> = members_data
        .iter()
        .map(|c| {
            let symbol = c
                .candidate
                .config
                .strategies
                .first()
                .map(|s| s.symbol.clone())
                .unwrap_or_default();
            let direction = c
                .candidate
                .config
                .strategies
                .first()
                .map(|s| format!("{:?}", s.direction))
                .unwrap_or_default();
            PortfolioMember {
                candidate_id: c.candidate.candidate_id.clone(),
                symbol,
                direction,
                allocation_pct: equal_weight,
                return_pct: c.return_pct,
                max_drawdown_pct: c.max_drawdown_pct,
                annualized_return_pct: c.annualized_return_pct,
                trade_count: c.trade_count,
                score: c.score,
            }
        })
        .collect();

    let combined_curve = combine_equity_curves(&members_data, equal_weight / 100.0);

    let return_pct = if combined_curve.is_empty() {
        0.0
    } else {
        let first = combined_curve.first().map(|p| p.equity_quote).unwrap_or(1.0);
        let last = combined_curve.last().map(|p| p.equity_quote).unwrap_or(1.0);
        if first > 0.0 { (last / first - 1.0) * 100.0 } else { 0.0 }
    };

    let drawdown_curve = build_drawdown_curve(&combined_curve);
    let max_drawdown_pct = drawdown_curve.iter().map(|p| p.drawdown_pct).fold(0.0_f64, f64::max);

    let trade_count: u64 = members_data.iter().map(|c| c.trade_count).sum();
    let days = if !combined_curve.is_empty() {
        let first_ts = combined_curve.first().map(|p| p.timestamp_ms).unwrap_or(0);
        let last_ts = combined_curve.last().map(|p| p.timestamp_ms).unwrap_or(0);
        ((last_ts - first_ts) as f64) / 86_400_000.0
    } else {
        0.0
    };
    let initial = combined_curve.first().map(|p| p.equity_quote).unwrap_or(1.0);
    let ending = combined_curve.last().map(|p| p.equity_quote).unwrap_or(1.0);
    let annualized_return_pct = calculate_annualized_return_pct(initial, ending, days);

    let calmar = if max_drawdown_pct > 0.0 { return_pct / max_drawdown_pct } else { 0.0 };
    let diversification_bonus = 1.0 + (portfolio_members.len() as f64 - 1.0) * 0.05;
    let score = calmar * diversification_bonus;

    Some(WeightedPortfolio {
        members: portfolio_members,
        member_count: member_indices.len(),
        allocation_pct: 100.0,
        return_pct,
        max_drawdown_pct,
        annualized_return_pct,
        trade_count,
        score,
        equity_curve: combined_curve,
        drawdown_curve,
    })
}

fn combine_equity_curves(candidates: &[&EvaluatedCandidate], weight: f64) -> Vec<EquityPoint> {
    if candidates.is_empty() {
        return Vec::new();
    }
    if candidates.len() == 1 {
        return candidates[0].equity_curve.clone();
    }

    let min_len = candidates.iter().map(|c| c.equity_curve.len()).min().unwrap_or(0);
    if min_len == 0 {
        return Vec::new();
    }

    (0..min_len)
        .map(|i| {
            let timestamp_ms = candidates[0].equity_curve[i].timestamp_ms;
            let weighted_equity: f64 = candidates
                .iter()
                .map(|c| c.equity_curve.get(i).map(|p| p.equity_quote).unwrap_or(0.0) * weight)
                .sum();
            EquityPoint { timestamp_ms, equity_quote: weighted_equity }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::martingale::metrics::EquityPoint;
    use shared_domain::martingale::{
        MartingaleDirection, MartingaleDirectionMode, MartingaleMarketKind, MartingalePortfolioConfig,
        MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel, MartingaleStrategyConfig,
        MartingaleTakeProfitModel,
    };
    use rust_decimal::Decimal;

    fn fixture_candidate(id: &str, symbol: &str, return_pct: f64, dd: f64, score: f64) -> EvaluatedCandidate {
        let strategy = MartingaleStrategyConfig {
            strategy_id: format!("test-{}", id),
            symbol: symbol.to_owned(),
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
        let base_equity = 100.0;
        let equity_curve: Vec<EquityPoint> = (0..100)
            .map(|t| EquityPoint {
                timestamp_ms: 1672531200000 + t * 86400000,
                equity_quote: base_equity + (t as f64) * (return_pct / 100.0),
            })
            .collect();
        EvaluatedCandidate {
            candidate: SearchCandidate {
                candidate_id: id.to_owned(),
                config: MartingalePortfolioConfig {
                    direction_mode: MartingaleDirectionMode::LongOnly,
                    strategies: vec![strategy],
                    risk_limits: MartingaleRiskLimits::default(),
                },
            },
            score,
            return_pct,
            max_drawdown_pct: dd,
            survival_passed: true,
            planned_margin_quote: 150.0,
            trade_count: 100,
            annualized_return_pct: Some(return_pct / 2.0),
            equity_curve,
            drawdown_curve: Vec::new(),
            trades: Vec::new(),
        }
    }

    #[test]
    fn portfolio_top3_combines_multiple_members() {
        let candidates = vec![
            fixture_candidate("a", "BTCUSDT", 30.0, 10.0, 3.0),
            fixture_candidate("b", "ETHUSDT", 25.0, 12.0, 2.5),
            fixture_candidate("c", "SOLUSDT", 20.0, 8.0, 2.0),
            fixture_candidate("d", "BTCUSDT", 15.0, 15.0, 1.5),
        ];
        let artifact = build_portfolio_top3(&candidates, 20.0);
        assert!(!artifact.top3.is_empty());
        for portfolio in &artifact.top3 {
            assert!(portfolio.member_count >= 2);
            let allocation_sum: f64 = portfolio.members.iter().map(|m| m.allocation_pct).sum();
            assert!((allocation_sum - 100.0).abs() < 0.000001);
            assert!(!portfolio.equity_curve.is_empty());
            assert!(!portfolio.drawdown_curve.is_empty());
        }
    }

    #[test]
    fn portfolio_top3_returns_empty_when_insufficient_eligible() {
        let candidates = vec![fixture_candidate("a", "BTCUSDT", 30.0, 10.0, 3.0)];
        let artifact = build_portfolio_top3(&candidates, 20.0);
        assert!(artifact.top3.is_empty());
        assert_eq!(artifact.eligible_candidate_count, 1);
    }

    #[test]
    fn portfolio_top3_filters_by_survival_and_drawdown() {
        let candidates = vec![
            fixture_candidate("a", "BTCUSDT", 30.0, 10.0, 3.0),
            fixture_candidate("b", "ETHUSDT", 25.0, 25.0, 2.5), // exceeds max drawdown
            fixture_candidate("c", "SOLUSDT", -5.0, 8.0, 2.0),  // negative return
        ];
        let artifact = build_portfolio_top3(&candidates, 20.0);
        assert_eq!(artifact.eligible_candidate_count, 1);
        assert!(artifact.top3.is_empty());
    }
}