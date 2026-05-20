use crate::martingale::metrics::{build_drawdown_curve, calculate_annualized_return_pct, DrawdownPoint, EquityPoint};
use crate::search::SearchCandidate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortfolioMember {
    pub candidate_id: String,
    pub symbol: String,
    pub direction: String,
    pub direction_mode: String,
    pub allocation_pct: f64,
    pub weight_pct: f64,
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
    pub trades_preview: Vec<crate::martingale::metrics::MartingaleTradeDetail>,
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

    let allocation_templates: Vec<Vec<f64>> = vec![
        vec![0.5, 0.5],
        vec![0.6, 0.4],
        vec![0.4, 0.6],
        vec![0.34, 0.33, 0.33],
        vec![0.5, 0.25, 0.25],
    ];

    let mut scored_portfolios: Vec<WeightedPortfolio> = Vec::new();
    let max_combos = 120usize;
    let mut combo_count = 0usize;

    for i in 0..eligible_count {
        for j in (i + 1)..eligible_count {
            // Try 2-member combination first
            let mut tried = false;
            for template in &allocation_templates {
                if template.len() != 2 {
                    continue;
                }
                if let Some(p) = build_weighted_portfolio(&eligible, &[i, j], template) {
                    scored_portfolios.push(p);
                }
                combo_count += 1;
                tried = true;
                if combo_count >= max_combos {
                    break;
                }
            }
            if combo_count >= max_combos || !tried {
                if combo_count >= max_combos { break; }
                continue;
            }

            // Try 3-member combinations
            for k in (j + 1)..eligible_count.min(j + 4) {
                if k >= eligible_count {
                    break;
                }
                for template in &allocation_templates {
                    if template.len() != 3 {
                        continue;
                    }
                    if let Some(p) = build_weighted_portfolio(&eligible, &[i, j, k], template) {
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

fn build_weighted_portfolio(eligible: &[&EvaluatedCandidate], member_indices: &[usize], allocations: &[f64]) -> Option<WeightedPortfolio> {
    let members_data: Vec<&EvaluatedCandidate> = member_indices.iter().map(|&i| eligible[i]).collect();

    // Require unique candidate IDs (allow same symbol with different strategies)
    let unique_ids: std::collections::HashSet<&str> = members_data
        .iter()
        .map(|c| c.candidate.candidate_id.as_str())
        .collect();
    if unique_ids.len() < 2 {
        return None;
    }

    let initial_portfolio_capital = 10_000.0;

    let member_pairs: Vec<(&EvaluatedCandidate, f64)> = members_data
        .iter()
        .zip(allocations.iter())
        .map(|(c, alloc)| (*c, *alloc * 100.0))
        .collect();

    let portfolio_members: Vec<PortfolioMember> = member_pairs
        .iter()
        .map(|(c, allocation_pct)| {
            let symbol = c
                .candidate
                .config
                .strategies
                .first()
                .map(|s| s.symbol.clone())
                .unwrap_or_default();
            let direction = match c.candidate.config.direction_mode {
                shared_domain::martingale::MartingaleDirectionMode::LongAndShort => "long_short".to_owned(),
                _ => c
                    .candidate
                    .config
                    .strategies
                    .first()
                    .map(|s| format!("{:?}", s.direction))
                    .unwrap_or_default(),
            };
            let direction_mode = format!("{:?}", c.candidate.config.direction_mode).to_lowercase();
            PortfolioMember {
                candidate_id: c.candidate.candidate_id.clone(),
                symbol,
                direction,
                direction_mode,
                allocation_pct: *allocation_pct,
                weight_pct: *allocation_pct,
                return_pct: c.return_pct,
                max_drawdown_pct: c.max_drawdown_pct,
                annualized_return_pct: c.annualized_return_pct,
                trade_count: c.trade_count,
                score: c.score,
            }
        })
        .collect();

    let combined_curve = combine_equity_curves(&member_pairs, initial_portfolio_capital);

    if combined_curve.len() < 2 {
        return None;
    }

    let return_pct = {
        let first = combined_curve.first().map(|p| p.equity_quote).unwrap_or(1.0);
        let last = combined_curve.last().map(|p| p.equity_quote).unwrap_or(1.0);
        if first > 0.0 { (last / first - 1.0) * 100.0 } else { 0.0 }
    };

    if return_pct <= 0.0 {
        return None;
    }

    let drawdown_curve = build_drawdown_curve(&combined_curve);
    let max_drawdown_pct = drawdown_curve.iter().map(|p| p.drawdown_pct).fold(0.0_f64, f64::max);

    let trade_count: u64 = members_data.iter().map(|c| c.trade_count).sum();
    let days = {
        let first_ts = combined_curve.first().map(|p| p.timestamp_ms).unwrap_or(0);
        let last_ts = combined_curve.last().map(|p| p.timestamp_ms).unwrap_or(0);
        ((last_ts - first_ts) as f64) / 86_400_000.0
    };
    let initial = combined_curve.first().map(|p| p.equity_quote).unwrap_or(1.0);
    let ending = combined_curve.last().map(|p| p.equity_quote).unwrap_or(1.0);
    let annualized_return_pct = calculate_annualized_return_pct(initial, ending, days);

    let mut trades_preview = members_data.iter().flat_map(|c| c.trades.clone()).collect::<Vec<_>>();
    trades_preview.sort_by_key(|trade| trade.timestamp_ms);
    trades_preview.truncate(200);

    let calmar = if max_drawdown_pct > 0.0 { return_pct / max_drawdown_pct } else { 0.0 };
    let diversification_bonus = 1.0 + (portfolio_members.len() as f64 - 1.0) * 0.05;
    let unique_symbol_count = portfolio_members.iter().map(|m| m.symbol.as_str()).collect::<std::collections::HashSet<_>>().len();
    let concentration_penalty = if unique_symbol_count == 1 { 0.85 } else { 1.0 };
    let score = calmar * diversification_bonus * concentration_penalty;

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
        trades_preview,
    })
}

fn combine_equity_curves(members: &[(&EvaluatedCandidate, f64)], initial_portfolio_capital: f64) -> Vec<EquityPoint> {
    if members.is_empty() {
        return Vec::new();
    }
    if members.len() == 1 {
        return members[0].0.equity_curve.clone();
    }

    let min_len = members.iter().map(|(c, _)| c.equity_curve.len()).min().unwrap_or(0);
    if min_len == 0 {
        return Vec::new();
    }

    (0..min_len)
        .map(|i| {
            let timestamp_ms = members[0].0.equity_curve[i].timestamp_ms;
            let equity_quote: f64 = members
                .iter()
                .map(|(candidate, allocation_pct)| {
                    let allocated_capital = initial_portfolio_capital * (*allocation_pct / 100.0);
                    let initial_candidate_margin = candidate.planned_margin_quote.max(0.000001);
                    let candidate_equity = candidate.equity_curve[i].equity_quote;
                    allocated_capital * candidate_equity / initial_candidate_margin
                })
                .sum();
            EquityPoint { timestamp_ms, equity_quote }
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

    fn candidate_with_curve(
        id: &str,
        symbol: &str,
        return_pct: f64,
        dd: f64,
        score: f64,
        planned_margin: f64,
        equity_values: Vec<f64>,
    ) -> EvaluatedCandidate {
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
        let equity_curve: Vec<EquityPoint> = equity_values
            .into_iter()
            .enumerate()
            .map(|(t, eq)| EquityPoint {
                timestamp_ms: 1672531200000 + t as i64 * 86400000,
                equity_quote: eq,
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
            planned_margin_quote: planned_margin,
            trade_count: 100,
            annualized_return_pct: Some(return_pct / 2.0),
            equity_curve,
            drawdown_curve: Vec::new(),
            trades: Vec::new(),
        }
    }

    #[test]
    fn portfolio_curve_normalizes_member_equity_by_planned_margin_and_allocation() {
        let a = candidate_with_curve("a", "BTCUSDT", 20.0, 5.0, 3.0, 100.0, vec![100.0, 120.0]);
        let b = candidate_with_curve("b", "ETHUSDT", 10.0, 5.0, 2.0, 200.0, vec![200.0, 220.0]);

        let artifact = build_portfolio_top3(&[a, b], 20.0);
        assert!(!artifact.top3.is_empty());

        let portfolio = &artifact.top3[0];
        assert_eq!(portfolio.member_count, 2);
        let allocation_sum: f64 = portfolio.members.iter().map(|m| m.allocation_pct).sum();
        assert!((allocation_sum - 100.0).abs() < 0.000001);

        // Portfolio equity must start at a meaningful initial capital (not raw sum of equity values)
        // With normalization: allocated_capital * equity / planned_margin
        // The initial point should reflect allocated capital, not raw 100+200=300
        let initial_equity = portfolio.equity_curve.first().map(|p| p.equity_quote).unwrap_or(0.0);
        // Raw sum would be 300.0; normalized should be different.
        assert!(initial_equity > 0.0);
        assert!(
            (initial_equity - 300.0).abs() > 1.0,
            "initial equity should be normalized by planned_margin, not raw sum; got {}",
            initial_equity
        );
    }

    #[test]
    fn portfolio_allows_multiple_strategies_on_same_symbol_when_candidate_ids_differ() {
        let a = candidate_with_curve("btc-fast", "BTCUSDT", 15.0, 5.0, 3.0, 100.0, vec![100.0, 115.0]);
        let b = candidate_with_curve("btc-slow", "BTCUSDT", 8.0, 5.0, 2.0, 100.0, vec![100.0, 108.0]);
        let c = candidate_with_curve("eth", "ETHUSDT", 4.0, 5.0, 1.5, 100.0, vec![100.0, 104.0]);

        let artifact = build_portfolio_top3(&[a, b, c], 20.0);

        assert!(
            !artifact.top3.is_empty(),
            "should produce at least one portfolio from same-symbol + cross-symbol candidates"
        );
        assert!(
            artifact.top3.iter().any(|p| {
                let btc_members = p.members.iter().filter(|m| m.symbol == "BTCUSDT").count();
                btc_members >= 2
            }),
            "at least one portfolio should combine two BTCUSDT strategies"
        );
    }

    #[test]
    fn portfolio_carries_combined_trade_preview() {
        use crate::martingale::metrics::MartingaleTradeDetail;

        let mut a = candidate_with_curve("a", "BTCUSDT", 20.0, 5.0, 3.0, 100.0, vec![100.0, 120.0]);
        a.trades = vec![MartingaleTradeDetail {
            timestamp_ms: 1672531200000,
            symbol: "BTCUSDT".to_owned(),
            direction: "Long".to_owned(),
            event_type: "take_profit".to_owned(),
            leg_index: Some(0),
            price: 30000.0,
            margin_quote: 100.0,
            notional_quote: 30000.0,
            leverage: 3.0,
            fee_quote: 0.9,
            slippage_quote: 0.0,
            realized_pnl_quote: 5.0,
            equity_after_quote: 105.0,
        }];

        let mut b = candidate_with_curve("b", "ETHUSDT", 10.0, 5.0, 2.0, 100.0, vec![100.0, 110.0]);
        b.trades = vec![MartingaleTradeDetail {
            timestamp_ms: 1672531260000,
            symbol: "ETHUSDT".to_owned(),
            direction: "Long".to_owned(),
            event_type: "take_profit".to_owned(),
            leg_index: Some(0),
            price: 2000.0,
            margin_quote: 100.0,
            notional_quote: 2000.0,
            leverage: 3.0,
            fee_quote: 0.6,
            slippage_quote: 0.0,
            realized_pnl_quote: 3.0,
            equity_after_quote: 103.0,
        }];

        let artifact = build_portfolio_top3(&[a, b], 20.0);
        let portfolio = artifact.top3.first().expect("at least one portfolio");

        assert!(
            !portfolio.trades_preview.is_empty(),
            "portfolio should carry combined trade details from members"
        );
    }

    #[test]
    fn weighted_portfolio_equity_curve_starts_near_initial_portfolio_capital() {
        let mut btc = fixture_candidate("btc", "BTCUSDT", 30.0, 10.0, 3.0);
        btc.planned_margin_quote = 500.0;
        btc.equity_curve = vec![
            EquityPoint { timestamp_ms: 1, equity_quote: 500.0 },
            EquityPoint { timestamp_ms: 2, equity_quote: 650.0 },
        ];

        let mut eth = fixture_candidate("eth", "ETHUSDT", 20.0, 8.0, 2.0);
        eth.planned_margin_quote = 250.0;
        eth.equity_curve = vec![
            EquityPoint { timestamp_ms: 1, equity_quote: 250.0 },
            EquityPoint { timestamp_ms: 2, equity_quote: 300.0 },
        ];

        let portfolio = build_weighted_portfolio(&[&btc, &eth], &[0, 1], &[0.6, 0.4])
            .expect("portfolio should build");

        let first = portfolio.equity_curve.first().unwrap().equity_quote;
        let last = portfolio.equity_curve.last().unwrap().equity_quote;
        assert!((first - 10_000.0).abs() < 0.0001, "first equity should equal initial portfolio capital, got {first}");
        assert!(last > first, "last equity should grow proportionally, first={first}, last={last}");
        assert!(last < 13_000.0, "last equity should be realistically scaled, got {last}");
    }

    #[test]
    fn weighted_portfolio_rejects_zero_or_missing_planned_margin() {
        let mut btc = fixture_candidate("btc", "BTCUSDT", 30.0, 10.0, 3.0);
        btc.planned_margin_quote = 0.0;
        btc.equity_curve = vec![
            EquityPoint { timestamp_ms: 1, equity_quote: 500.0 },
            EquityPoint { timestamp_ms: 2, equity_quote: 650.0 },
        ];

        let mut eth = fixture_candidate("eth", "ETHUSDT", 20.0, 8.0, 2.0);
        eth.planned_margin_quote = 250.0;
        eth.equity_curve = vec![
            EquityPoint { timestamp_ms: 1, equity_quote: 250.0 },
            EquityPoint { timestamp_ms: 2, equity_quote: 300.0 },
        ];

        assert!(build_weighted_portfolio(&[&btc, &eth], &[0, 1], &[0.6, 0.4]).is_none());
    }
}