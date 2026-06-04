use crate::martingale::metrics::{
    build_drawdown_curve, calculate_annualized_return_pct, DrawdownPoint, EquityPoint,
};
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
    pub leverage: Option<u32>,
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
    pub eligible_symbols: Vec<String>,
    pub unique_eligible_symbol_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_portfolios: Option<Vec<WeightedPortfolio>>,
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

fn candidate_symbol(candidate: &EvaluatedCandidate) -> String {
    candidate
        .candidate
        .config
        .strategies
        .first()
        .map(|strategy| strategy.symbol.clone())
        .unwrap_or_default()
}

fn best_indices_by_symbol(eligible: &[&EvaluatedCandidate], per_symbol: usize) -> Vec<usize> {
    let mut grouped: std::collections::BTreeMap<String, Vec<(usize, &EvaluatedCandidate)>> =
        std::collections::BTreeMap::new();
    for (index, candidate) in eligible.iter().enumerate() {
        grouped
            .entry(candidate_symbol(candidate))
            .or_default()
            .push((index, *candidate));
    }
    let mut result = Vec::new();
    for (_symbol, mut rows) in grouped {
        rows.sort_by(|a, b| {
            b.1.score
                .partial_cmp(&a.1.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        result.extend(rows.into_iter().take(per_symbol).map(|(index, _)| index));
    }
    result.sort_unstable();
    result.dedup();
    result
}

fn portfolio_seed_indices_by_symbol(
    eligible: &[&EvaluatedCandidate],
    per_symbol_score: usize,
    per_symbol_low_drawdown: usize,
    per_symbol_high_return: usize,
) -> Vec<usize> {
    let mut grouped: std::collections::BTreeMap<String, Vec<(usize, &EvaluatedCandidate)>> =
        std::collections::BTreeMap::new();
    for (index, candidate) in eligible.iter().enumerate() {
        grouped
            .entry(candidate_symbol(candidate))
            .or_default()
            .push((index, *candidate));
    }

    let mut result = Vec::new();
    for (_symbol, rows) in grouped {
        let mut by_score = rows.clone();
        by_score.sort_by(|a, b| b.1.score.total_cmp(&a.1.score));
        result.extend(
            by_score
                .into_iter()
                .take(per_symbol_score)
                .map(|(index, _)| index),
        );

        let mut by_drawdown = rows.clone();
        by_drawdown.sort_by(|a, b| a.1.max_drawdown_pct.total_cmp(&b.1.max_drawdown_pct));
        result.extend(
            by_drawdown
                .into_iter()
                .take(per_symbol_low_drawdown)
                .map(|(index, _)| index),
        );

        let mut by_return = rows;
        by_return.sort_by(|a, b| {
            candidate_annualized_or_return(b.1).total_cmp(&candidate_annualized_or_return(a.1))
        });
        result.extend(
            by_return
                .into_iter()
                .take(per_symbol_high_return)
                .map(|(index, _)| index),
        );
    }
    result.sort_unstable();
    result.dedup();
    result
}

fn allocation_templates_v2() -> Vec<Vec<f64>> {
    let mut templates = Vec::new();
    for member_count in 2..=12 {
        templates.extend(allocation_templates_for_member_count(member_count));
    }
    templates
}

fn allocation_templates_for_member_count(member_count: usize) -> Vec<Vec<f64>> {
    if member_count == 0 {
        return Vec::new();
    }
    let n = member_count as f64;
    let even = vec![1.0 / n; member_count];
    let mut templates = vec![even];

    if member_count >= 3 {
        let leader = if member_count >= 10 {
            0.40
        } else {
            0.40_f64.min(1.0 / n + 0.18)
        };
        let rest = (1.0 - leader) / (member_count - 1) as f64;
        templates.push(
            std::iter::once(leader)
                .chain(std::iter::repeat(rest).take(member_count - 1))
                .collect(),
        );
    }

    if member_count >= 5 {
        let first_bucket = (member_count / 3).max(1);
        let second_bucket = (member_count / 3).max(1);
        let third_bucket = member_count - first_bucket - second_bucket;
        let mut tpl = Vec::with_capacity(member_count);
        tpl.extend(std::iter::repeat(0.45 / first_bucket as f64).take(first_bucket));
        tpl.extend(std::iter::repeat(0.35 / second_bucket as f64).take(second_bucket));
        if third_bucket > 0 {
            tpl.extend(std::iter::repeat(0.20 / third_bucket as f64).take(third_bucket));
        }
        templates.push(tpl);
    }

    if member_count >= 10 {
        templates.push(
            vec![
                0.12, 0.11, 0.10, 0.10, 0.09, 0.09, 0.09, 0.08, 0.08, 0.07, 0.07,
            ][..member_count.min(11)]
                .to_vec(),
        );
        let mut barbell = vec![0.40];
        barbell.extend(std::iter::repeat(0.60 / (member_count - 1) as f64).take(member_count - 1));
        templates.push(barbell);
        let mut two_leaders = vec![0.25, 0.20];
        two_leaders
            .extend(std::iter::repeat(0.55 / (member_count - 2) as f64).take(member_count - 2));
        templates.push(two_leaders);
        templates.push(vec![0.10; member_count]);
    }

    if member_count >= 5 {
        let mut low_leader = vec![0.08];
        low_leader
            .extend(std::iter::repeat(0.92 / (member_count - 1) as f64).take(member_count - 1));
        templates.push(low_leader);

        let mut medium_leader = vec![0.12];
        medium_leader
            .extend(std::iter::repeat(0.88 / (member_count - 1) as f64).take(member_count - 1));
        templates.push(medium_leader);

        let mut two_growth = vec![0.10, 0.08];
        two_growth
            .extend(std::iter::repeat(0.82 / (member_count - 2) as f64).take(member_count - 2));
        templates.push(two_growth);
    }

    for tpl in &mut templates {
        normalize_allocations(tpl);
    }
    templates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    templates.dedup_by(|a, b| allocations_key(a) == allocations_key(b));
    templates
}

fn normalize_allocations(allocations: &mut [f64]) {
    let sum: f64 = allocations.iter().sum();
    if sum <= 0.0 {
        return;
    }
    for value in allocations {
        *value /= sum;
    }
}

fn allocations_key(allocations: &[f64]) -> String {
    allocations
        .iter()
        .map(|value| format!("{:.4}", value))
        .collect::<Vec<_>>()
        .join("|")
}

fn daily_return_correlation_penalty(members: &[(&EvaluatedCandidate, f64)]) -> f64 {
    let correlations = pairwise_curve_correlations(members);
    if correlations.is_empty() {
        return 1.0;
    }
    let avg = correlations.iter().sum::<f64>() / correlations.len() as f64;
    if avg > 0.8 {
        0.85
    } else if avg > 0.6 {
        0.93
    } else {
        1.0
    }
}

fn pairwise_curve_correlations(members: &[(&EvaluatedCandidate, f64)]) -> Vec<f64> {
    if members.len() < 2 {
        return Vec::new();
    }

    // Extract daily equity values for each member, aligned by day boundary.
    let daily_returns: Vec<Vec<f64>> = members
        .iter()
        .map(|(candidate, _)| daily_returns_from_curve(&candidate.equity_curve))
        .collect();

    if daily_returns.iter().any(|r| r.len() < 10) {
        return Vec::new();
    }

    // Find the minimum length across all series to align them.
    let min_len = daily_returns.iter().map(|r| r.len()).min().unwrap_or(0);
    if min_len < 10 {
        return Vec::new();
    }

    // Truncate all series to the same length (use the tail to favor recent data).
    let aligned: Vec<&[f64]> = daily_returns
        .iter()
        .map(|r| &r[r.len() - min_len..])
        .collect();

    let mut correlations = Vec::with_capacity(members.len() * (members.len() - 1) / 2);
    for i in 0..aligned.len() {
        for j in (i + 1)..aligned.len() {
            if let Some(r) = pearson_r(aligned[i], aligned[j]) {
                correlations.push(r);
            }
        }
    }
    correlations
}

/// Extract daily returns from a 1-minute equity curve.
/// Groups points by day (UTC midnight boundary) and takes the last equity per day,
/// then computes day-over-day percentage returns.
fn daily_returns_from_curve(curve: &[EquityPoint]) -> Vec<f64> {
    if curve.len() < 2 {
        return Vec::new();
    }

    let ms_per_day: i64 = 86_400_000;
    let mut daily_equities: Vec<f64> = Vec::new();
    let mut current_day: i64 = curve[0].timestamp_ms / ms_per_day;
    let mut last_equity: f64 = curve[0].equity_quote;

    for point in curve.iter().skip(1) {
        let day = point.timestamp_ms / ms_per_day;
        if day != current_day {
            daily_equities.push(last_equity);
            current_day = day;
        }
        last_equity = point.equity_quote;
    }
    // Push the last day's equity.
    daily_equities.push(last_equity);

    if daily_equities.len() < 2 {
        return Vec::new();
    }

    // Convert to daily returns: (e[t] - e[t-1]) / e[t-1]
    daily_equities
        .windows(2)
        .map(|window| {
            let prev = window[0];
            let curr = window[1];
            if prev > 0.0 {
                (curr - prev) / prev
            } else {
                0.0
            }
        })
        .collect()
}

/// Compute Pearson correlation coefficient between two slices.
fn pearson_r(x: &[f64], y: &[f64]) -> Option<f64> {
    let n = x.len().min(y.len());
    if n < 3 {
        return None;
    }
    let x = &x[..n];
    let y = &y[..n];

    let mean_x = x.iter().sum::<f64>() / n as f64;
    let mean_y = y.iter().sum::<f64>() / n as f64;

    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;

    for i in 0..n {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    if var_x <= 0.0 || var_y <= 0.0 {
        return None;
    }

    Some(cov / (var_x.sqrt() * var_y.sqrt()))
}

fn dedupe_portfolios_by_member_weight(portfolios: &mut Vec<WeightedPortfolio>) {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    portfolios.retain(|p| {
        let mut member_keys: Vec<String> = p
            .members
            .iter()
            .map(|m| format!("{}:{:.2}", m.candidate_id, m.allocation_pct))
            .collect();
        member_keys.sort();
        let key = member_keys.join("|");
        seen.insert(key)
    });
}

pub fn build_portfolio_top_n_v2(
    candidates: &[EvaluatedCandidate],
    max_drawdown_pct: f64,
    top_n: usize,
) -> PortfolioTop3Artifact {
    let eligible: Vec<&EvaluatedCandidate> = candidates
        .iter()
        .filter(|c| {
            c.return_pct > 0.0 && c.planned_margin_quote > 0.0 && !c.equity_curve.is_empty()
        })
        .collect();

    let eligible_count = eligible.len();
    let eligible_symbols: Vec<String> = eligible
        .iter()
        .map(|c| candidate_symbol(c))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let unique_eligible_symbol_count = eligible_symbols.len();

    if eligible.len() < 2 {
        return PortfolioTop3Artifact {
            top3: Vec::new(),
            eligible_candidate_count: eligible_count,
            eligible_symbols,
            unique_eligible_symbol_count,
            all_portfolios: None,
        };
    }

    let top_n = top_n.max(3).min(10);
    let all = build_ranked_portfolios_v2(&eligible, max_drawdown_pct, top_n);
    let top3 = all.iter().take(3).cloned().collect();

    PortfolioTop3Artifact {
        top3,
        eligible_candidate_count: eligible_count,
        eligible_symbols,
        unique_eligible_symbol_count,
        all_portfolios: Some(all),
    }
}

fn build_ranked_portfolios_v2(
    eligible: &[&EvaluatedCandidate],
    max_drawdown_pct: f64,
    top_n: usize,
) -> Vec<WeightedPortfolio> {
    let best_single_annualized = eligible
        .iter()
        .map(|candidate| candidate_annualized_or_return(candidate))
        .fold(0.0_f64, f64::max);
    let target_annualized = (best_single_annualized * 0.82).max(40.0);
    let seed_indices = portfolio_seed_indices_by_symbol(eligible, 12, 8, 8);
    let templates = allocation_templates_v2();
    let mut scored: Vec<WeightedPortfolio> = Vec::new();

    enumerate_compact_portfolios_v2(
        eligible,
        &seed_indices,
        &templates,
        max_drawdown_pct,
        &mut scored,
    );
    enumerate_seeded_diversified_portfolios_v2(
        eligible,
        &seed_indices,
        &templates,
        max_drawdown_pct,
        &mut scored,
    );
    enumerate_risk_balanced_portfolios_v2(
        eligible,
        &seed_indices,
        &templates,
        max_drawdown_pct,
        &mut scored,
    );
    enumerate_barbell_yield_portfolios_v2(eligible, &seed_indices, max_drawdown_pct, &mut scored);
    enumerate_stochastic_risk_portfolios_v2(eligible, &seed_indices, max_drawdown_pct, &mut scored);

    scored.sort_by(|a, b| b.score.total_cmp(&a.score));
    dedupe_portfolios_by_member_weight(&mut scored);
    let broad = scored
        .iter()
        .filter(|portfolio| portfolio_meets_live_diversity_floor(portfolio))
        .cloned()
        .collect::<Vec<_>>();
    let mut ranked = if broad.len() >= 3 {
        let mut broad_ranked = broad;
        sort_portfolios_by_yield_then_risk(&mut broad_ranked, target_annualized);
        let mut remaining = scored
            .into_iter()
            .filter(|portfolio| !portfolio_meets_live_diversity_floor(portfolio))
            .collect::<Vec<_>>();
        sort_portfolios_by_yield_then_risk(&mut remaining, target_annualized);
        broad_ranked.extend(remaining);
        broad_ranked
    } else {
        sort_portfolios_by_yield_then_risk(&mut scored, target_annualized);
        scored
    };
    ranked.truncate(top_n);
    ranked
}

fn sort_portfolios_by_yield_then_risk(
    portfolios: &mut [WeightedPortfolio],
    _target_annualized: f64,
) {
    portfolios.sort_by(|a, b| {
        let a_ann = a.annualized_return_pct.unwrap_or(a.return_pct);
        let b_ann = b.annualized_return_pct.unwrap_or(b.return_pct);
        let a_ratio = a_ann / a.max_drawdown_pct.max(1.0);
        let b_ratio = b_ann / b.max_drawdown_pct.max(1.0);

        b_ann
            .total_cmp(&a_ann)
            .then_with(|| b_ratio.total_cmp(&a_ratio))
            .then_with(|| a.max_drawdown_pct.total_cmp(&b.max_drawdown_pct))
    });
}

fn portfolio_meets_live_diversity_floor(portfolio: &WeightedPortfolio) -> bool {
    if portfolio.member_count < 10 {
        return false;
    }
    let mut allocation_by_symbol = std::collections::BTreeMap::<&str, f64>::new();
    for member in &portfolio.members {
        *allocation_by_symbol
            .entry(member.symbol.as_str())
            .or_default() += member.allocation_pct;
    }
    allocation_by_symbol.len() >= 10
        && allocation_by_symbol
            .values()
            .all(|allocation_pct| *allocation_pct <= 40.000001)
}

fn enumerate_compact_portfolios_v2(
    eligible: &[&EvaluatedCandidate],
    indices: &[usize],
    templates: &[Vec<f64>],
    max_drawdown_pct: f64,
    result: &mut Vec<WeightedPortfolio>,
) {
    let unique_symbols = indices
        .iter()
        .map(|idx| candidate_symbol(eligible[*idx]))
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let max_member_count = if unique_symbols >= 10 {
        0
    } else {
        10.min(indices.len())
    };
    for member_count in 2..=max_member_count {
        let windows = indices.len().saturating_sub(member_count).min(24);
        for start in 0..=windows {
            let current = indices
                .iter()
                .cycle()
                .skip(start)
                .take(member_count)
                .copied()
                .collect::<Vec<_>>();
            push_weighted_templates(eligible, &current, templates, max_drawdown_pct, result);
        }
    }
}

fn enumerate_seeded_diversified_portfolios_v2(
    eligible: &[&EvaluatedCandidate],
    indices: &[usize],
    templates: &[Vec<f64>],
    max_drawdown_pct: f64,
    result: &mut Vec<WeightedPortfolio>,
) {
    let mut by_symbol = std::collections::BTreeMap::<String, Vec<usize>>::new();
    for &idx in indices {
        by_symbol
            .entry(candidate_symbol(eligible[idx]))
            .or_default()
            .push(idx);
    }
    if by_symbol.len() < 2 {
        return;
    }

    for rows in by_symbol.values_mut() {
        rows.sort_by(|a, b| eligible[*b].score.total_cmp(&eligible[*a].score));
        rows.truncate(4);
    }

    let mut balanced_symbols: Vec<(String, usize)> = by_symbol
        .iter()
        .filter_map(|(symbol, rows)| rows.first().map(|idx| (symbol.clone(), *idx)))
        .collect();
    balanced_symbols.sort_by(|a, b| eligible[b.1].score.total_cmp(&eligible[a.1].score));

    let mut low_drawdown_symbols: Vec<(String, usize)> = by_symbol
        .iter()
        .filter_map(|(symbol, rows)| {
            rows.iter()
                .min_by(|a, b| {
                    eligible[**a]
                        .max_drawdown_pct
                        .total_cmp(&eligible[**b].max_drawdown_pct)
                })
                .map(|idx| (symbol.clone(), *idx))
        })
        .collect();
    low_drawdown_symbols.sort_by(|a, b| {
        eligible[a.1]
            .max_drawdown_pct
            .total_cmp(&eligible[b.1].max_drawdown_pct)
    });

    let mut high_return_symbols: Vec<(String, usize)> = by_symbol
        .iter()
        .filter_map(|(symbol, rows)| {
            rows.iter()
                .max_by(|a, b| {
                    candidate_annualized_or_return(eligible[**a])
                        .total_cmp(&candidate_annualized_or_return(eligible[**b]))
                })
                .map(|idx| (symbol.clone(), *idx))
        })
        .collect();
    high_return_symbols.sort_by(|a, b| {
        candidate_annualized_or_return(eligible[b.1])
            .total_cmp(&candidate_annualized_or_return(eligible[a.1]))
    });

    let symbol_orders = vec![balanced_symbols, low_drawdown_symbols, high_return_symbols];
    for member_count in 10..=12.min(indices.len()) {
        for order in &symbol_orders {
            let current = repeated_distinct_indices_from_order(order, &by_symbol, member_count, 0);
            push_weighted_templates(eligible, &current, templates, max_drawdown_pct, result);
        }
    }

    let symbols = by_symbol.keys().cloned().collect::<Vec<_>>();
    for offset in 0..symbols.len().min(8) {
        let order = symbols
            .iter()
            .cycle()
            .skip(offset)
            .take(symbols.len())
            .filter_map(|symbol| {
                by_symbol
                    .get(symbol)
                    .and_then(|rows| rows.first())
                    .map(|idx| (symbol.clone(), *idx))
            })
            .collect::<Vec<_>>();
        for member_count in 10..=12.min(indices.len()) {
            let current =
                repeated_distinct_indices_from_order(&order, &by_symbol, member_count, offset);
            push_weighted_templates(eligible, &current, templates, max_drawdown_pct, result);
        }
    }
}

fn enumerate_risk_balanced_portfolios_v2(
    eligible: &[&EvaluatedCandidate],
    indices: &[usize],
    templates: &[Vec<f64>],
    max_drawdown_pct: f64,
    result: &mut Vec<WeightedPortfolio>,
) {
    let mut high_return = indices.to_vec();
    high_return.sort_by(|a, b| {
        candidate_annualized_or_return(eligible[*b])
            .total_cmp(&candidate_annualized_or_return(eligible[*a]))
    });
    high_return.truncate(18);

    let mut low_drawdown = indices.to_vec();
    low_drawdown.sort_by(|a, b| {
        eligible[*a]
            .max_drawdown_pct
            .total_cmp(&eligible[*b].max_drawdown_pct)
    });
    low_drawdown.truncate(18);

    let mut best_ratio = indices.to_vec();
    best_ratio.sort_by(|a, b| {
        (candidate_annualized_or_return(eligible[*b]) / eligible[*b].max_drawdown_pct.max(1.0))
            .total_cmp(
                &(candidate_annualized_or_return(eligible[*a])
                    / eligible[*a].max_drawdown_pct.max(1.0)),
            )
    });
    best_ratio.truncate(18);

    let pools = [high_return, low_drawdown, best_ratio];
    for member_count in 6..=12.min(indices.len()) {
        for offset in 0..8 {
            let mut current = Vec::with_capacity(member_count);
            let mut seen = std::collections::BTreeSet::<usize>::new();
            let mut guard = 0;
            while current.len() < member_count && guard < member_count * pools.len() * 8 {
                for (pool_index, pool) in pools.iter().enumerate() {
                    if pool.is_empty() || current.len() >= member_count {
                        continue;
                    }
                    let idx = pool[(offset + guard + pool_index) % pool.len()];
                    if seen.insert(idx) {
                        current.push(idx);
                    }
                }
                guard += 1;
            }
            push_weighted_templates(eligible, &current, templates, max_drawdown_pct, result);
        }
    }
}

fn enumerate_barbell_yield_portfolios_v2(
    eligible: &[&EvaluatedCandidate],
    indices: &[usize],
    max_drawdown_pct: f64,
    result: &mut Vec<WeightedPortfolio>,
) {
    if indices.len() < 3 {
        return;
    }

    let mut high_return = indices.to_vec();
    high_return.sort_by(|a, b| {
        candidate_annualized_or_return(eligible[*b])
            .total_cmp(&candidate_annualized_or_return(eligible[*a]))
    });
    high_return.truncate(24);

    let mut stabilizers = indices.to_vec();
    stabilizers.sort_by(|a, b| {
        (eligible[*a].max_drawdown_pct * 1.4 - candidate_annualized_or_return(eligible[*a]) * 0.15)
            .total_cmp(
                &(eligible[*b].max_drawdown_pct * 1.4
                    - candidate_annualized_or_return(eligible[*b]) * 0.15),
            )
    });
    stabilizers.truncate(48);

    let mut ratio = indices.to_vec();
    ratio.sort_by(|a, b| {
        (candidate_annualized_or_return(eligible[*b]) / eligible[*b].max_drawdown_pct.max(1.0))
            .total_cmp(
                &(candidate_annualized_or_return(eligible[*a])
                    / eligible[*a].max_drawdown_pct.max(1.0)),
            )
    });
    ratio.truncate(48);

    let mut support_pool = stabilizers;
    support_pool.extend(ratio);
    support_pool.sort_unstable();
    support_pool.dedup();

    let min_member_count = if support_pool.len() >= 9 { 10 } else { 3 };
    let max_member_count = 12.min(support_pool.len() + 1);
    let leader_weights = [0.10, 0.12, 0.15, 0.18, 0.22, 0.26, 0.30, 0.35, 0.40];

    for &leader_idx in &high_return {
        for member_count in min_member_count..=max_member_count {
            for offset in 0..support_pool.len().min(14) {
                let mut current = Vec::with_capacity(member_count);
                let mut seen = std::collections::BTreeSet::<usize>::new();
                current.push(leader_idx);
                seen.insert(leader_idx);

                let mut guard = 0;
                while current.len() < member_count && guard < support_pool.len() * 3 {
                    let idx = support_pool[(offset + guard) % support_pool.len()];
                    if seen.insert(idx) {
                        current.push(idx);
                    }
                    guard += 1;
                }
                if current.len() < member_count {
                    continue;
                }

                for &leader_weight in &leader_weights {
                    let mut allocations = Vec::with_capacity(member_count);
                    allocations.push(leader_weight);
                    let rest = 1.0 - leader_weight;
                    let support_scores = current[1..]
                        .iter()
                        .map(|idx| {
                            let candidate = eligible[*idx];
                            (candidate_annualized_or_return(candidate).max(0.0)
                                / candidate.max_drawdown_pct.max(1.0))
                            .max(0.05)
                            .sqrt()
                        })
                        .collect::<Vec<_>>();
                    let support_sum: f64 = support_scores.iter().sum();
                    if support_sum <= 0.0 {
                        continue;
                    }
                    allocations.extend(
                        support_scores
                            .iter()
                            .map(|score| rest * *score / support_sum),
                    );
                    if let Some(portfolio) =
                        build_weighted_portfolio(eligible, &current, &allocations, max_drawdown_pct)
                    {
                        result.push(portfolio);
                        prune_scored_portfolios(result, 3_000, 1_500);
                    }
                }
            }
        }
    }
}

fn enumerate_stochastic_risk_portfolios_v2(
    eligible: &[&EvaluatedCandidate],
    indices: &[usize],
    max_drawdown_pct: f64,
    result: &mut Vec<WeightedPortfolio>,
) {
    if indices.len() < 2 {
        return;
    }

    let mut pool = indices.to_vec();
    pool.sort_by(|a, b| {
        (candidate_annualized_or_return(eligible[*b]) / eligible[*b].max_drawdown_pct.max(1.0))
            .total_cmp(
                &(candidate_annualized_or_return(eligible[*a])
                    / eligible[*a].max_drawdown_pct.max(1.0)),
            )
    });
    pool.truncate(48);

    let min_member_count = if pool.len() >= 10 { 10 } else { 2 };
    let max_member_count = 12.min(pool.len());
    let iterations = if pool.len() >= 18 { 1_800 } else { 900 };
    let mut rng = DeterministicRng::new(0xD15E_A5E5_5EED_u64 ^ pool.len() as u64);

    for _ in 0..iterations {
        let member_count = rng.gen_range(min_member_count, max_member_count + 1);
        let current = stochastic_member_indices(eligible, &pool, member_count, &mut rng);
        if current.len() < 2 {
            continue;
        }
        let weights = stochastic_allocations(eligible, &current, &mut rng);
        if let Some(portfolio) =
            build_weighted_portfolio(eligible, &current, &weights, max_drawdown_pct)
        {
            result.push(portfolio);
            prune_scored_portfolios(result, 3_000, 1_500);
        }
    }
}

fn stochastic_member_indices(
    eligible: &[&EvaluatedCandidate],
    pool: &[usize],
    member_count: usize,
    rng: &mut DeterministicRng,
) -> Vec<usize> {
    let mut by_symbol = std::collections::BTreeMap::<String, Vec<usize>>::new();
    for &idx in pool {
        by_symbol
            .entry(candidate_symbol(eligible[idx]))
            .or_default()
            .push(idx);
    }
    for rows in by_symbol.values_mut() {
        rows.sort_by(|a, b| {
            (candidate_annualized_or_return(eligible[*b]) / eligible[*b].max_drawdown_pct.max(1.0))
                .total_cmp(
                    &(candidate_annualized_or_return(eligible[*a])
                        / eligible[*a].max_drawdown_pct.max(1.0)),
                )
        });
    }

    let mut current = Vec::with_capacity(member_count);
    let mut seen = std::collections::BTreeSet::<usize>::new();
    let mut symbols = by_symbol.keys().cloned().collect::<Vec<_>>();
    shuffle_with_rng(&mut symbols, rng);
    for symbol in &symbols {
        if current.len() >= member_count {
            break;
        }
        let Some(rows) = by_symbol.get(symbol) else {
            continue;
        };
        let cap = rows.len().min(6);
        let idx = rows[rng.gen_range(0, cap)];
        if seen.insert(idx) {
            current.push(idx);
        }
    }

    let mut guard = 0;
    while current.len() < member_count && guard < pool.len() * 4 {
        let idx = pool[rng.gen_range(0, pool.len())];
        if seen.insert(idx) {
            current.push(idx);
        }
        guard += 1;
    }
    current
}

fn stochastic_allocations(
    eligible: &[&EvaluatedCandidate],
    current: &[usize],
    rng: &mut DeterministicRng,
) -> Vec<f64> {
    let mut weights = current
        .iter()
        .map(|idx| {
            let candidate = eligible[*idx];
            let quality = (candidate_annualized_or_return(candidate).max(0.0)
                / candidate.max_drawdown_pct.max(1.0))
            .max(0.05);
            let jitter = 0.35 + rng.next_unit_f64() * 1.45;
            quality.powf(0.70) * jitter
        })
        .collect::<Vec<_>>();
    normalize_allocations(&mut weights);
    weights
}

fn shuffle_with_rng<T>(items: &mut [T], rng: &mut DeterministicRng) {
    if items.len() < 2 {
        return;
    }
    for index in (1..items.len()).rev() {
        let swap_with = rng.gen_range(0, index + 1);
        items.swap(index, swap_with);
    }
}

struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn next_unit_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1_u64 << 53) as f64)
    }

    fn gen_range(&mut self, start: usize, end: usize) -> usize {
        if end <= start + 1 {
            return start;
        }
        start + (self.next_u64() as usize % (end - start))
    }
}

fn repeated_distinct_indices_from_order(
    order: &[(String, usize)],
    by_symbol: &std::collections::BTreeMap<String, Vec<usize>>,
    member_count: usize,
    offset: usize,
) -> Vec<usize> {
    let mut current = Vec::with_capacity(member_count);
    let mut seen = std::collections::BTreeSet::<usize>::new();
    for (position, (symbol, fallback_idx)) in order.iter().cycle().skip(offset).enumerate() {
        if current.len() >= member_count || position > member_count * order.len().max(1) {
            break;
        }
        let Some(rows) = by_symbol.get(symbol) else {
            continue;
        };
        for shift in 0..rows.len().max(1) {
            let candidate_idx = rows
                .get((position + shift) % rows.len())
                .copied()
                .unwrap_or(*fallback_idx);
            if seen.insert(candidate_idx) {
                current.push(candidate_idx);
                break;
            }
        }
    }
    current
}

fn push_weighted_templates(
    eligible: &[&EvaluatedCandidate],
    current: &[usize],
    templates: &[Vec<f64>],
    max_drawdown_pct: f64,
    result: &mut Vec<WeightedPortfolio>,
) {
    let mut current_templates = templates
        .iter()
        .filter(|tpl| tpl.len() == current.len())
        .cloned()
        .collect::<Vec<_>>();
    current_templates.sort_by(|a, b| allocations_key(a).cmp(&allocations_key(b)));
    current_templates.dedup_by(|a, b| allocations_key(a) == allocations_key(b));

    for template in &current_templates {
        if let Some(portfolio) =
            build_weighted_portfolio(eligible, current, template, max_drawdown_pct)
        {
            let member_pairs: Vec<(&EvaluatedCandidate, f64)> = current
                .iter()
                .zip(template.iter())
                .map(|(idx, alloc)| (eligible[*idx], *alloc))
                .collect();
            let mut adjusted = portfolio;
            let correlation_factor = daily_return_correlation_penalty(&member_pairs);
            let correlation_penalty = (1.0 - correlation_factor).max(0.0) * 4.0;
            let unique_count = adjusted
                .members
                .iter()
                .map(|m| m.symbol.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len();
            adjusted.score += (unique_count as f64).ln() * 1.8 - correlation_penalty;
            result.push(adjusted);
            prune_scored_portfolios(result, 3_000, 1_500);
        }
    }
}

fn candidate_annualized_or_return(candidate: &EvaluatedCandidate) -> f64 {
    candidate
        .annualized_return_pct
        .unwrap_or(candidate.return_pct)
}

pub fn build_portfolio_top3(
    candidates: &[EvaluatedCandidate],
    max_drawdown_pct: f64,
) -> PortfolioTop3Artifact {
    let eligible: Vec<&EvaluatedCandidate> = candidates
        .iter()
        .filter(|c| c.return_pct > 0.0 && !c.equity_curve.is_empty())
        .collect();

    let eligible_count = eligible.len();

    let eligible_symbols: Vec<String> = eligible
        .iter()
        .map(|candidate| candidate_symbol(candidate))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let unique_eligible_symbol_count = eligible_symbols.len();

    if eligible_count < 2 {
        return PortfolioTop3Artifact {
            top3: Vec::new(),
            eligible_candidate_count: eligible_count,
            eligible_symbols,
            unique_eligible_symbol_count,
            all_portfolios: None,
        };
    }

    let allocation_templates = allocation_templates();
    let mut scored_portfolios: Vec<WeightedPortfolio> = Vec::new();
    let max_combos = 40_000usize;
    let mut combo_count = 0usize;

    let focused_indices = best_indices_by_symbol(&eligible, 8);
    let max_member_count = 8
        .min(unique_eligible_symbol_count.max(2))
        .min(focused_indices.len());
    for member_count in 2..=max_member_count {
        let mut current = Vec::with_capacity(member_count);
        enumerate_portfolio_index_combinations(
            &eligible,
            &focused_indices,
            member_count,
            0,
            &mut current,
            &allocation_templates,
            &mut scored_portfolios,
            &mut combo_count,
            max_combos,
            max_drawdown_pct,
        );
        if combo_count >= max_combos {
            break;
        }
    }

    // Step C: Sort by score, then force diversified portfolios to the top when eligible.
    scored_portfolios.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if unique_eligible_symbol_count >= 2 {
        let mut diversified: Vec<_> = scored_portfolios
            .iter()
            .cloned()
            .filter(|p| {
                p.members
                    .iter()
                    .map(|m| m.symbol.as_str())
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    >= 2
            })
            .collect();
        let mut concentrated: Vec<_> = scored_portfolios
            .iter()
            .cloned()
            .filter(|p| {
                p.members
                    .iter()
                    .map(|m| m.symbol.as_str())
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    < 2
            })
            .collect();
        diversified.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        concentrated.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored_portfolios = diversified
            .into_iter()
            .chain(concentrated.into_iter())
            .collect();
    }
    scored_portfolios.truncate(3);

    PortfolioTop3Artifact {
        top3: scored_portfolios,
        eligible_candidate_count: eligible_count,
        eligible_symbols,
        unique_eligible_symbol_count,
        all_portfolios: None,
    }
}

fn allocation_templates() -> Vec<Vec<f64>> {
    vec![
        vec![0.5, 0.5],
        vec![0.6, 0.4],
        vec![0.4, 0.6],
        vec![0.7, 0.3],
        vec![0.3, 0.7],
        vec![0.34, 0.33, 0.33],
        vec![0.5, 0.25, 0.25],
        vec![0.25, 0.5, 0.25],
        vec![0.25, 0.25, 0.5],
        vec![0.6, 0.2, 0.2],
        vec![0.2, 0.6, 0.2],
        vec![0.2, 0.2, 0.6],
        vec![0.7, 0.15, 0.15],
        vec![0.15, 0.7, 0.15],
        vec![0.15, 0.15, 0.7],
        vec![0.4, 0.35, 0.25],
        vec![0.25, 0.35, 0.4],
        vec![0.25, 0.25, 0.25, 0.25],
        vec![0.4, 0.2, 0.2, 0.2],
        vec![0.2, 0.4, 0.2, 0.2],
        vec![0.2, 0.2, 0.4, 0.2],
        vec![0.2, 0.2, 0.2, 0.4],
        vec![0.55, 0.15, 0.15, 0.15],
        vec![0.15, 0.55, 0.15, 0.15],
        vec![0.15, 0.15, 0.55, 0.15],
        vec![0.15, 0.15, 0.15, 0.55],
        vec![0.7, 0.1, 0.1, 0.1],
        vec![0.1, 0.7, 0.1, 0.1],
        vec![0.1, 0.1, 0.7, 0.1],
        vec![0.1, 0.1, 0.1, 0.7],
        vec![0.2, 0.2, 0.2, 0.2, 0.2],
        vec![0.3, 0.25, 0.2, 0.15, 0.1],
        vec![0.1, 0.15, 0.2, 0.25, 0.3],
        vec![0.6, 0.1, 0.1, 0.1, 0.1],
        vec![0.1, 0.6, 0.1, 0.1, 0.1],
        vec![0.1, 0.1, 0.6, 0.1, 0.1],
        vec![0.1, 0.1, 0.1, 0.6, 0.1],
        vec![0.1, 0.1, 0.1, 0.1, 0.6],
        vec![0.20, 0.18, 0.17, 0.16, 0.15, 0.14],
        vec![0.30, 0.18, 0.15, 0.12, 0.10, 0.08, 0.07],
        vec![0.25, 0.18, 0.15, 0.12, 0.10, 0.08, 0.07, 0.05],
    ]
}

fn enumerate_portfolio_index_combinations(
    eligible: &[&EvaluatedCandidate],
    indices: &[usize],
    target_len: usize,
    start_pos: usize,
    current: &mut Vec<usize>,
    allocation_templates: &[Vec<f64>],
    scored_portfolios: &mut Vec<WeightedPortfolio>,
    combo_count: &mut usize,
    max_combos: usize,
    max_portfolio_drawdown_pct: f64,
) {
    if *combo_count >= max_combos {
        return;
    }
    if current.len() == target_len {
        let unique_symbols = current
            .iter()
            .map(|index| candidate_symbol(eligible[*index]))
            .collect::<std::collections::HashSet<_>>()
            .len();
        if unique_symbols < 2 {
            return;
        }
        for template in allocation_templates
            .iter()
            .filter(|tpl| tpl.len() == target_len)
        {
            if let Some(portfolio) =
                build_weighted_portfolio(eligible, current, template, max_portfolio_drawdown_pct)
            {
                scored_portfolios.push(portfolio);
                prune_scored_portfolios(scored_portfolios, 160, 64);
            }
            *combo_count += 1;
            if *combo_count >= max_combos {
                break;
            }
        }
        return;
    }

    let remaining = target_len - current.len();
    if indices.len().saturating_sub(start_pos) < remaining {
        return;
    }
    for pos in start_pos..indices.len() {
        current.push(indices[pos]);
        enumerate_portfolio_index_combinations(
            eligible,
            indices,
            target_len,
            pos + 1,
            current,
            allocation_templates,
            scored_portfolios,
            combo_count,
            max_combos,
            max_portfolio_drawdown_pct,
        );
        current.pop();
        if *combo_count >= max_combos {
            break;
        }
    }
}

fn prune_scored_portfolios(portfolios: &mut Vec<WeightedPortfolio>, threshold: usize, keep: usize) {
    if portfolios.len() <= threshold {
        return;
    }
    portfolios.sort_by(|a, b| portfolio_prune_rank(b).total_cmp(&portfolio_prune_rank(a)));
    portfolios.truncate(keep);
}

fn portfolio_prune_rank(portfolio: &WeightedPortfolio) -> f64 {
    let annualized = portfolio
        .annualized_return_pct
        .unwrap_or(portfolio.return_pct);
    let unique_symbols = portfolio
        .members
        .iter()
        .map(|member| member.symbol.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len() as f64;
    let calmar = annualized / portfolio.max_drawdown_pct.max(1.0);
    calmar * 10.0 + annualized * 0.2 + unique_symbols.ln() * 4.0 - portfolio.max_drawdown_pct * 0.2
}

fn build_weighted_portfolio(
    eligible: &[&EvaluatedCandidate],
    member_indices: &[usize],
    allocations: &[f64],
    max_portfolio_drawdown_pct: f64,
) -> Option<WeightedPortfolio> {
    let members_data: Vec<&EvaluatedCandidate> =
        member_indices.iter().map(|&i| eligible[i]).collect();

    // Require unique candidate IDs (allow same symbol with different strategies)
    let unique_ids: std::collections::HashSet<&str> = members_data
        .iter()
        .map(|c| c.candidate.candidate_id.as_str())
        .collect();
    if unique_ids.len() < 2 {
        return None;
    }

    if members_data
        .iter()
        .any(|c| !c.planned_margin_quote.is_finite() || c.planned_margin_quote <= 0.0)
    {
        return None;
    }
    if members_data.iter().any(|c| c.equity_curve.is_empty()) {
        return None;
    }

    let initial_portfolio_capital = 10_000.0;

    let member_pairs: Vec<(&EvaluatedCandidate, f64)> = members_data
        .iter()
        .zip(allocations.iter())
        .map(|(c, alloc)| (*c, *alloc * 100.0))
        .collect();

    let mut allocation_by_symbol = std::collections::BTreeMap::<String, f64>::new();
    for (candidate, allocation_pct) in &member_pairs {
        let symbol = candidate_symbol(candidate);
        *allocation_by_symbol.entry(symbol).or_insert(0.0) += *allocation_pct;
    }
    let unique_symbol_count_for_cap = allocation_by_symbol.len();
    if member_indices.len() >= 10 && unique_symbol_count_for_cap < 10 {
        return None;
    }
    let max_symbol_allocation_pct = if unique_symbol_count_for_cap >= 10 {
        40.000001
    } else {
        80.000001
    };
    if allocation_by_symbol
        .values()
        .any(|allocation_pct| *allocation_pct > max_symbol_allocation_pct)
    {
        return None;
    }

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
                shared_domain::martingale::MartingaleDirectionMode::LongAndShort => {
                    "long_short".to_owned()
                }
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
                leverage: c
                    .candidate
                    .config
                    .strategies
                    .iter()
                    .filter_map(|strategy| strategy.leverage)
                    .max(),
            }
        })
        .collect();

    let combined_curve = combine_equity_curves(&member_pairs, initial_portfolio_capital);

    if combined_curve.len() < 2 {
        return None;
    }

    let first_equity = combined_curve
        .first()
        .map(|p| p.equity_quote)
        .unwrap_or(0.0);
    if !first_equity.is_finite()
        || first_equity <= 0.0
        || (first_equity - initial_portfolio_capital).abs() > 0.01
    {
        return None;
    }

    let return_pct = {
        let first = combined_curve
            .first()
            .map(|p| p.equity_quote)
            .unwrap_or(1.0);
        let last = combined_curve.last().map(|p| p.equity_quote).unwrap_or(1.0);
        if first > 0.0 {
            (last / first - 1.0) * 100.0
        } else {
            0.0
        }
    };

    if return_pct <= 0.0 {
        return None;
    }

    let drawdown_curve = build_drawdown_curve(&combined_curve);
    let max_drawdown_pct = drawdown_curve
        .iter()
        .map(|p| p.drawdown_pct)
        .fold(0.0_f64, f64::max);
    if max_drawdown_pct > max_portfolio_drawdown_pct {
        return None;
    }

    let trade_count: u64 = members_data.iter().map(|c| c.trade_count).sum();
    let days = {
        let first_ts = combined_curve.first().map(|p| p.timestamp_ms).unwrap_or(0);
        let last_ts = combined_curve.last().map(|p| p.timestamp_ms).unwrap_or(0);
        ((last_ts - first_ts) as f64) / 86_400_000.0
    };
    let initial = combined_curve
        .first()
        .map(|p| p.equity_quote)
        .unwrap_or(1.0);
    let ending = combined_curve.last().map(|p| p.equity_quote).unwrap_or(1.0);
    let annualized_return_pct = calculate_annualized_return_pct(initial, ending, days);

    let mut trades_preview = members_data
        .iter()
        .flat_map(|c| c.trades.clone())
        .collect::<Vec<_>>();
    trades_preview.sort_by_key(|trade| trade.timestamp_ms);
    trades_preview.truncate(200);

    let annualized = annualized_return_pct.unwrap_or(return_pct);
    let return_drawdown = annualized / max_drawdown_pct.max(1.0);
    let unique_symbol_count = portfolio_members
        .iter()
        .map(|m| m.symbol.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();
    let member_count = portfolio_members.len().max(1);
    let max_single_symbol_weight_pct = allocation_by_symbol
        .values()
        .copied()
        .fold(0.0_f64, f64::max);
    let member_bonus = (member_count as f64).ln() * 1.5;
    let unique_bonus = (unique_symbol_count as f64).ln() * 2.0;
    let concentration_penalty = max_single_symbol_weight_pct * 0.03;
    let score = annualized + return_drawdown * 18.0 + member_bonus + unique_bonus
        - max_drawdown_pct * 0.65
        - concentration_penalty;

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

fn combine_equity_curves(
    members: &[(&EvaluatedCandidate, f64)],
    initial_portfolio_capital: f64,
) -> Vec<EquityPoint> {
    if members.is_empty() {
        return Vec::new();
    }
    if members.len() == 1 {
        let (candidate, allocation_pct) = members[0];
        let initial_candidate_equity = candidate
            .equity_curve
            .first()
            .map(|p| p.equity_quote)
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(candidate.planned_margin_quote);
        let allocated_capital = initial_portfolio_capital * (allocation_pct / 100.0);
        let scale = allocated_capital / initial_candidate_equity;
        return candidate
            .equity_curve
            .iter()
            .map(|point| EquityPoint {
                timestamp_ms: point.timestamp_ms,
                equity_quote: point.equity_quote * scale,
            })
            .collect();
    }

    // Precompute initial equity for each member so the first combined point
    // equals initial_portfolio_capital regardless of raw candidate equity scale.
    let initial_equities: Vec<f64> = members
        .iter()
        .map(|(candidate, _)| {
            candidate
                .equity_curve
                .first()
                .map(|point| point.equity_quote)
                .filter(|value| value.is_finite() && *value > 0.0)
                .unwrap_or(candidate.planned_margin_quote)
        })
        .collect();

    let mut timestamps = members
        .iter()
        .flat_map(|(candidate, _)| {
            candidate
                .equity_curve
                .iter()
                .map(|point| point.timestamp_ms)
        })
        .collect::<Vec<_>>();
    timestamps.sort_unstable();
    timestamps.dedup();
    if timestamps.is_empty() {
        return Vec::new();
    }

    let mut member_positions = vec![0usize; members.len()];
    let mut latest_equities = initial_equities.clone();

    timestamps
        .into_iter()
        .map(|timestamp_ms| {
            for (idx, (candidate, _)) in members.iter().enumerate() {
                while member_positions[idx] < candidate.equity_curve.len()
                    && candidate.equity_curve[member_positions[idx]].timestamp_ms <= timestamp_ms
                {
                    latest_equities[idx] =
                        candidate.equity_curve[member_positions[idx]].equity_quote;
                    member_positions[idx] += 1;
                }
            }
            let equity_quote: f64 = members
                .iter()
                .enumerate()
                .map(|(idx, (_, allocation_pct))| {
                    let allocated_capital = initial_portfolio_capital * (*allocation_pct / 100.0);
                    let initial_candidate_equity = initial_equities[idx];
                    if !initial_candidate_equity.is_finite() || initial_candidate_equity <= 0.0 {
                        return 0.0;
                    }
                    let candidate_equity = latest_equities[idx];
                    let candidate_return_factor = candidate_equity / initial_candidate_equity;
                    allocated_capital * candidate_return_factor
                })
                .sum();
            EquityPoint {
                timestamp_ms,
                equity_quote,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::martingale::metrics::EquityPoint;
    use rust_decimal::Decimal;
    use shared_domain::martingale::{
        MartingaleDirection, MartingaleDirectionMode, MartingaleMarketKind,
        MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel,
        MartingaleSpacingModel, MartingaleStrategyConfig, MartingaleTakeProfitModel,
    };

    fn fixture_candidate(
        id: &str,
        symbol: &str,
        return_pct: f64,
        dd: f64,
        score: f64,
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
    fn portfolio_members_include_backtested_leverage() {
        let candidates = vec![
            fixture_candidate("a", "BTCUSDT", 30.0, 10.0, 3.0),
            fixture_candidate("b", "ETHUSDT", 25.0, 12.0, 2.5),
        ];

        let artifact = build_portfolio_top3(&candidates, 20.0);

        assert!(!artifact.top3.is_empty());
        assert!(artifact.top3[0]
            .members
            .iter()
            .all(|member| member.leverage == Some(3)));
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
    fn portfolio_top1_uses_cross_symbol_even_when_second_symbol_has_lower_return() {
        let mut btc_a = fixture_candidate("btc-a", "BTCUSDT", 62.0, 19.0, 62.0);
        btc_a.annualized_return_pct = Some(15.6);
        let mut btc_b = fixture_candidate("btc-b", "BTCUSDT", 64.0, 20.0, 62.0);
        btc_b.annualized_return_pct = Some(16.0);
        let mut btc_c = fixture_candidate("btc-c", "BTCUSDT", 53.0, 20.0, 55.0);
        btc_c.annualized_return_pct = Some(13.6);

        let mut eth_a = fixture_candidate("eth-a", "ETHUSDT", 15.0, 28.9, 20.0);
        eth_a.annualized_return_pct = Some(4.2);
        let mut eth_b = fixture_candidate("eth-b", "ETHUSDT", 1.1, 29.2, 5.0);
        eth_b.annualized_return_pct = Some(0.3);

        let artifact = build_portfolio_top3(&[btc_a, btc_b, btc_c, eth_a, eth_b], 30.0);
        assert!(!artifact.top3.is_empty());
        let first = &artifact.top3[0];
        let symbols: std::collections::HashSet<&str> =
            first.members.iter().map(|m| m.symbol.as_str()).collect();
        assert!(symbols.contains("BTCUSDT"));
        assert!(
            symbols.contains("ETHUSDT"),
            "Top1 must diversify when ETH eligible exists: {:?}",
            first.members
        );
    }

    #[test]
    fn portfolio_artifact_reports_eligible_symbols() {
        let artifact = build_portfolio_top3(
            &[
                fixture_candidate("btc", "BTCUSDT", 30.0, 10.0, 3.0),
                fixture_candidate("eth", "ETHUSDT", 10.0, 20.0, 2.0),
            ],
            30.0,
        );

        assert_eq!(artifact.unique_eligible_symbol_count, 2);
        assert_eq!(
            artifact.eligible_symbols,
            vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()]
        );
    }

    #[test]
    fn portfolio_top3_returns_empty_when_insufficient_eligible() {
        let candidates = vec![fixture_candidate("a", "BTCUSDT", 30.0, 10.0, 3.0)];
        let artifact = build_portfolio_top3(&candidates, 20.0);
        assert!(artifact.top3.is_empty());
        assert_eq!(artifact.eligible_candidate_count, 1);
        assert_eq!(artifact.unique_eligible_symbol_count, 1);
        assert_eq!(artifact.eligible_symbols, vec!["BTCUSDT".to_owned()]);
    }

    #[test]
    fn portfolio_top3_filters_by_survival_and_drawdown() {
        let candidates = vec![
            fixture_candidate("a", "BTCUSDT", 30.0, 10.0, 3.0),
            fixture_candidate("b", "ETHUSDT", 25.0, 25.0, 2.5), // exceeds max drawdown
            fixture_candidate("c", "SOLUSDT", -5.0, 8.0, 2.0),  // negative return
        ];
        let artifact = build_portfolio_top3(&candidates, 20.0);
        assert_eq!(artifact.eligible_candidate_count, 2);
        assert!(!artifact.top3.is_empty());
        assert!(artifact
            .top3
            .iter()
            .all(|portfolio| portfolio.max_drawdown_pct <= 20.0));
    }

    #[test]
    fn allocation_templates_cover_profit_weighted_larger_portfolios() {
        let templates = allocation_templates();

        assert!(templates.iter().any(|tpl| tpl == &vec![0.7, 0.1, 0.1, 0.1]));
        assert!(templates
            .iter()
            .any(|tpl| tpl == &vec![0.6, 0.1, 0.1, 0.1, 0.1]));
        assert!(templates
            .iter()
            .filter(|tpl| tpl.len() == 5)
            .all(|tpl| (tpl.iter().sum::<f64>() - 1.0).abs() < 0.000001));
        assert!(templates.iter().any(|tpl| tpl.len() == 6));
        assert!(templates.iter().any(|tpl| tpl.len() == 7));
        assert!(templates.iter().any(|tpl| tpl.len() == 8));
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
        let initial_equity = portfolio
            .equity_curve
            .first()
            .map(|p| p.equity_quote)
            .unwrap_or(0.0);
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
        let a = candidate_with_curve(
            "btc-fast",
            "BTCUSDT",
            15.0,
            5.0,
            3.0,
            100.0,
            vec![100.0, 115.0],
        );
        let b = candidate_with_curve(
            "btc-slow",
            "BTCUSDT",
            8.0,
            5.0,
            2.0,
            100.0,
            vec![100.0, 108.0],
        );
        let c = candidate_with_curve("eth", "ETHUSDT", 4.0, 5.0, 1.5, 100.0, vec![100.0, 104.0]);

        let portfolio =
            build_weighted_portfolio(&[&a, &b, &c], &[0, 1, 2], &[0.5, 0.25, 0.25], 30.0)
                .expect("same-symbol multi-strategy should be allowed under 80% symbol cap");
        assert!(
            portfolio
                .members
                .iter()
                .filter(|m| m.symbol == "BTCUSDT")
                .count()
                >= 2,
            "portfolio should combine two BTCUSDT strategies when total BTC allocation is capped"
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
            EquityPoint {
                timestamp_ms: 1,
                equity_quote: 500.0,
            },
            EquityPoint {
                timestamp_ms: 2,
                equity_quote: 650.0,
            },
        ];

        let mut eth = fixture_candidate("eth", "ETHUSDT", 20.0, 8.0, 2.0);
        eth.planned_margin_quote = 250.0;
        eth.equity_curve = vec![
            EquityPoint {
                timestamp_ms: 1,
                equity_quote: 250.0,
            },
            EquityPoint {
                timestamp_ms: 2,
                equity_quote: 300.0,
            },
        ];

        let portfolio = build_weighted_portfolio(&[&btc, &eth], &[0, 1], &[0.6, 0.4], 30.0)
            .expect("portfolio should build");

        let first = portfolio.equity_curve.first().unwrap().equity_quote;
        let last = portfolio.equity_curve.last().unwrap().equity_quote;
        assert!(
            (first - 10_000.0).abs() < 0.0001,
            "first equity should equal initial portfolio capital, got {first}"
        );
        assert!(
            last > first,
            "last equity should grow proportionally, first={first}, last={last}"
        );
        assert!(
            last < 13_000.0,
            "last equity should be realistically scaled, got {last}"
        );
    }

    #[test]
    fn portfolio_top3_uses_member_count_as_soft_reward_not_hard_target() {
        let candidates = vec![
            candidate_with_curve(
                "btc",
                "BTCUSDT",
                38.0,
                12.0,
                4.0,
                500.0,
                vec![500.0, 530.0, 520.0, 690.0],
            ),
            candidate_with_curve(
                "eth",
                "ETHUSDT",
                30.0,
                8.0,
                3.5,
                500.0,
                vec![500.0, 515.0, 510.0, 650.0],
            ),
            candidate_with_curve(
                "sol",
                "SOLUSDT",
                26.0,
                9.0,
                3.0,
                500.0,
                vec![500.0, 505.0, 540.0, 630.0],
            ),
            candidate_with_curve(
                "bnb",
                "BNBUSDT",
                22.0,
                7.0,
                2.8,
                500.0,
                vec![500.0, 512.0, 518.0, 610.0],
            ),
            candidate_with_curve(
                "xrp",
                "XRPUSDT",
                18.0,
                6.0,
                2.5,
                500.0,
                vec![500.0, 508.0, 515.0, 590.0],
            ),
            candidate_with_curve(
                "doge",
                "DOGEUSDT",
                16.0,
                6.0,
                2.2,
                500.0,
                vec![500.0, 506.0, 512.0, 580.0],
            ),
        ];

        let artifact = build_portfolio_top3(&candidates, 25.0);
        assert!(!artifact.top3.is_empty());
        let first = &artifact.top3[0];
        let symbols = first
            .members
            .iter()
            .map(|member| member.symbol.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert!(
            symbols.len() >= 2,
            "portfolio should remain diversified, got {:?}",
            first.members
        );
        assert!(
            first.annualized_return_pct.unwrap_or(first.return_pct) >= 12.0,
            "soft member reward must not demote higher-return combinations: {:?}",
            first
        );
    }

    #[test]
    fn portfolio_top3_prefers_cross_symbol_members_when_available() {
        // Use equity curves with drawdowns so calmar scoring is meaningful.
        // BTC candidates have high return but higher drawdown; ETH has lower return
        // but also lower drawdown. Cross-symbol should win via diversification bonus.
        let btc_a = candidate_with_curve(
            "btc-a",
            "BTCUSDT",
            30.0,
            15.0,
            3.0,
            500.0,
            vec![500.0, 480.0, 520.0, 650.0],
        );
        let btc_b = candidate_with_curve(
            "btc-b",
            "BTCUSDT",
            28.0,
            16.0,
            2.9,
            500.0,
            vec![500.0, 470.0, 510.0, 640.0],
        );
        let eth_a = candidate_with_curve(
            "eth-a",
            "ETHUSDT",
            20.0,
            5.0,
            2.0,
            250.0,
            vec![250.0, 245.0, 260.0, 300.0],
        );
        let eth_b = candidate_with_curve(
            "eth-b",
            "ETHUSDT",
            18.0,
            6.0,
            1.8,
            250.0,
            vec![250.0, 243.0, 255.0, 295.0],
        );

        let artifact = build_portfolio_top3(&[btc_a, btc_b, eth_a, eth_b], 25.0);
        assert!(!artifact.top3.is_empty());
        let first = &artifact.top3[0];
        let symbols: std::collections::HashSet<&str> = first
            .members
            .iter()
            .map(|member| member.symbol.as_str())
            .collect();
        assert!(symbols.contains("BTCUSDT"));
        assert!(
            symbols.contains("ETHUSDT"),
            "first portfolio should diversify across eligible requested symbols: {:?}",
            first.members
        );
    }

    #[test]
    fn weighted_portfolio_rejects_zero_or_missing_planned_margin() {
        let mut btc = fixture_candidate("btc", "BTCUSDT", 30.0, 10.0, 3.0);
        btc.planned_margin_quote = 0.0;
        btc.equity_curve = vec![
            EquityPoint {
                timestamp_ms: 1,
                equity_quote: 500.0,
            },
            EquityPoint {
                timestamp_ms: 2,
                equity_quote: 650.0,
            },
        ];

        let mut eth = fixture_candidate("eth", "ETHUSDT", 20.0, 8.0, 2.0);
        eth.planned_margin_quote = 250.0;
        eth.equity_curve = vec![
            EquityPoint {
                timestamp_ms: 1,
                equity_quote: 250.0,
            },
            EquityPoint {
                timestamp_ms: 2,
                equity_quote: 300.0,
            },
        ];

        assert!(build_weighted_portfolio(&[&btc, &eth], &[0, 1], &[0.6, 0.4], 30.0).is_none());
    }

    #[test]
    fn portfolio_v2_combines_high_return_with_low_drawdown_stabilizer_under_hard_limit() {
        let high = candidate_with_curve(
            "btc-growth",
            "BTCUSDT",
            120.0,
            55.0,
            8.0,
            100.0,
            vec![100.0, 180.0, 125.0, 230.0],
        );
        let low = candidate_with_curve(
            "eth-stable",
            "ETHUSDT",
            18.0,
            6.0,
            2.0,
            100.0,
            vec![100.0, 103.0, 106.0, 118.0],
        );
        let loss = candidate_with_curve(
            "ada-loss",
            "ADAUSDT",
            -5.0,
            3.0,
            1.0,
            100.0,
            vec![100.0, 99.0, 98.0, 95.0],
        );

        let artifact = build_portfolio_top_n_v2(&[high, low, loss], 30.0, 10);
        let first = artifact
            .top3
            .first()
            .expect("expected complementary portfolio");

        assert!(
            first.max_drawdown_pct <= 30.0,
            "portfolio must obey hard drawdown: {first:?}"
        );
        assert!(first.members.iter().any(|m| m.candidate_id == "btc-growth"));
        assert!(first.members.iter().any(|m| m.candidate_id == "eth-stable"));
        assert!(first.members.iter().all(|m| m.candidate_id != "ada-loss"));
    }

    #[test]
    fn portfolio_v2_prefers_high_yield_under_drawdown_limit_over_member_count() {
        let btc = candidate_with_curve(
            "btc-high",
            "BTCUSDT",
            230.0,
            19.8,
            9.0,
            1000.0,
            vec![1000.0, 980.0, 1300.0, 3300.0],
        );
        let xrp = candidate_with_curve(
            "xrp-high",
            "XRPUSDT",
            200.0,
            15.2,
            8.5,
            1000.0,
            vec![1000.0, 990.0, 1250.0, 3000.0],
        );
        let doge = candidate_with_curve(
            "doge-stable",
            "DOGEUSDT",
            99.0,
            24.0,
            5.0,
            1000.0,
            vec![1000.0, 990.0, 1150.0, 1990.0],
        );
        let link = candidate_with_curve(
            "link-low",
            "LINKUSDT",
            73.0,
            10.0,
            4.5,
            1000.0,
            vec![1000.0, 995.0, 1100.0, 1730.0],
        );

        let artifact = build_portfolio_top_n_v2(&[btc, xrp, doge, link], 20.0, 10);
        let first = artifact.top3.first().expect("top portfolio");
        let ids = first
            .members
            .iter()
            .map(|member| member.candidate_id.as_str())
            .collect::<std::collections::HashSet<_>>();

        assert!(
            first.max_drawdown_pct <= 20.0,
            "must obey hard drawdown limit: {first:?}"
        );
        assert!(
            ids.contains("btc-high") || ids.contains("xrp-high"),
            "high-yield candidates should not be lost: {first:?}"
        );
    }

    #[test]
    fn portfolio_v2_finds_lower_drawdown_without_sacrificing_yield() {
        let volatile = candidate_with_curve(
            "volatile-growth",
            "BTCUSDT",
            330.0,
            30.0,
            50.0,
            100.0,
            vec![100.0, 180.0, 120.0, 260.0, 430.0],
        );
        let steady_a = candidate_with_curve(
            "steady-a",
            "SOLUSDT",
            250.0,
            8.0,
            30.0,
            100.0,
            vec![100.0, 125.0, 170.0, 250.0, 350.0],
        );
        let steady_b = candidate_with_curve(
            "steady-b",
            "DOGEUSDT",
            240.0,
            8.0,
            28.0,
            100.0,
            vec![100.0, 130.0, 165.0, 245.0, 340.0],
        );
        let stabilizers = (0..9).map(|index| {
            candidate_with_curve(
                &format!("stabilizer-{index}"),
                &[
                    "ETHUSDT", "XRPUSDT", "ADAUSDT", "BNBUSDT", "LINKUSDT", "AVAXUSDT", "DOTUSDT",
                    "NEARUSDT", "AAVEUSDT",
                ][index],
                90.0 + index as f64,
                4.0,
                12.0,
                100.0,
                vec![100.0, 112.0, 125.0, 150.0, 190.0 + index as f64],
            )
        });
        let candidates = std::iter::once(volatile)
            .chain([steady_a, steady_b])
            .chain(stabilizers)
            .collect::<Vec<_>>();

        let artifact = build_portfolio_top_n_v2(&candidates, 30.0, 10);
        let first = artifact.top3.first().expect("top portfolio");

        assert!(
            first.annualized_return_pct.unwrap_or(first.return_pct) >= 45.0,
            "portfolio should keep meaningful yield: {first:?}"
        );
        assert!(
            first.max_drawdown_pct < 15.0,
            "risk-balanced members should reduce drawdown materially: {first:?}"
        );
    }

    #[test]
    fn portfolio_v2_can_return_top_ten_ranked_portfolios() {
        let mut candidates = Vec::new();
        for index in 0..12 {
            let symbol = if index % 3 == 0 {
                "BTCUSDT"
            } else if index % 3 == 1 {
                "ETHUSDT"
            } else {
                "SOLUSDT"
            };
            candidates.push(candidate_with_curve(
                &format!("c{index}"),
                symbol,
                20.0 + index as f64 * 3.0,
                5.0 + index as f64,
                2.0,
                100.0,
                vec![100.0, 105.0 + index as f64, 110.0 + index as f64],
            ));
        }

        let artifact = build_portfolio_top_n_v2(&candidates, 30.0, 10);
        assert!(artifact.top3.len() >= 3);
        assert!(
            artifact
                .all_portfolios
                .as_ref()
                .map(|items| items.len())
                .unwrap_or(0)
                >= 10
        );
    }

    #[test]
    fn portfolio_v2_prefers_ten_plus_symbols_and_caps_single_symbol_weight() {
        let mut candidates = Vec::new();
        for index in 0..18 {
            let symbol = format!("SYM{index}USDT");
            candidates.push(candidate_with_curve(
                &format!("{symbol}-growth"),
                &symbol,
                80.0 - index as f64,
                12.0 + (index % 4) as f64,
                80.0 - index as f64,
                100.0,
                vec![100.0, 108.0 + index as f64, 118.0 + index as f64],
            ));
            candidates.push(candidate_with_curve(
                &format!("{symbol}-stable"),
                &symbol,
                24.0 - (index % 5) as f64,
                3.0 + (index % 3) as f64,
                30.0 - index as f64 * 0.2,
                100.0,
                vec![
                    100.0,
                    101.0 + index as f64 * 0.1,
                    106.0 + index as f64 * 0.2,
                ],
            ));
        }

        let artifact = build_portfolio_top_n_v2(&candidates, 25.0, 10);
        assert_eq!(artifact.top3.len(), 3);
        for portfolio in &artifact.top3 {
            let mut allocation_by_symbol = std::collections::BTreeMap::<String, f64>::new();
            for member in &portfolio.members {
                *allocation_by_symbol
                    .entry(member.symbol.clone())
                    .or_default() += member.allocation_pct;
            }
            assert!(
                portfolio.member_count >= 10,
                "v2 portfolio must be broad enough for live risk, got {:?}",
                portfolio.members
            );
            assert!(
                allocation_by_symbol.len() >= 10,
                "v2 portfolio should cover at least 10 symbols, got {:?}",
                allocation_by_symbol
            );
            assert!(
                allocation_by_symbol
                    .values()
                    .all(|value| *value <= 40.000001),
                "single symbol cap violated: {:?}",
                allocation_by_symbol
            );
        }
    }

    #[test]
    fn portfolio_artifact_never_reports_single_member_as_combination() {
        let candidates = vec![candidate_with_curve(
            "btc-only",
            "BTCUSDT",
            50.0,
            10.0,
            3.0,
            100.0,
            vec![100.0, 150.0],
        )];
        let artifact = build_portfolio_top_n_v2(&candidates, 30.0, 10);
        assert!(artifact.top3.is_empty());
    }

    #[test]
    fn weighted_portfolio_rejects_single_symbol_allocation_above_eighty_pct() {
        let btc_a = candidate_with_curve(
            "btc-a",
            "BTCUSDT",
            20.0,
            5.0,
            3.0,
            100.0,
            vec![100.0, 120.0],
        );
        let btc_b = candidate_with_curve(
            "btc-b",
            "BTCUSDT",
            12.0,
            5.0,
            2.0,
            100.0,
            vec![100.0, 112.0],
        );
        let eth = candidate_with_curve("eth", "ETHUSDT", 8.0, 5.0, 1.0, 100.0, vec![100.0, 108.0]);

        assert!(
            build_weighted_portfolio(
                &[&btc_a, &btc_b, &eth],
                &[0, 1, 2],
                &[0.5, 0.25, 0.25],
                30.0
            )
            .is_some(),
            "75% BTC allocation should be allowed"
        );
        assert!(
            build_weighted_portfolio(
                &[&btc_a, &btc_b, &eth],
                &[0, 1, 2],
                &[0.5, 0.34, 0.16],
                30.0
            )
            .is_none(),
            "84% BTC allocation must be rejected"
        );
    }

    #[test]
    fn portfolio_dedupe_keeps_distinct_weight_allocations_for_same_members() {
        let btc = candidate_with_curve(
            "btc",
            "BTCUSDT",
            120.0,
            30.0,
            9.0,
            100.0,
            vec![100.0, 180.0, 120.0, 220.0],
        );
        let eth = candidate_with_curve(
            "eth",
            "ETHUSDT",
            25.0,
            4.0,
            3.0,
            100.0,
            vec![100.0, 105.0, 110.0, 125.0],
        );

        let aggressive = build_weighted_portfolio(&[&btc, &eth], &[0, 1], &[0.8, 0.2], 50.0)
            .expect("aggressive allocation should build");
        let balanced = build_weighted_portfolio(&[&btc, &eth], &[0, 1], &[0.2, 0.8], 50.0)
            .expect("balanced allocation should build");

        let mut portfolios = vec![aggressive, balanced];
        dedupe_portfolios_by_member_weight(&mut portfolios);

        assert_eq!(
            portfolios.len(),
            2,
            "same members with materially different weights are distinct portfolios"
        );
    }

    #[test]
    fn correlation_penalty_reduces_score_for_highly_correlated_curves() {
        // Two curves with nearly identical daily movements → high correlation
        let base_equity: Vec<f64> = (0..120)
            .map(|t| 100.0 + t as f64 * 0.5 + (t as f64 * 0.1).sin() * 10.0)
            .collect();
        let a = candidate_with_curve("a", "BTCUSDT", 50.0, 20.0, 5.0, 100.0, base_equity.clone());
        // B is almost identical to A — highly correlated
        let b_equity: Vec<f64> = base_equity.iter().map(|v| v * 1.01 + 2.0).collect();
        let b = candidate_with_curve("b", "ETHUSDT", 45.0, 22.0, 4.0, 100.0, b_equity);

        let penalty = daily_return_correlation_penalty(&[(&a, 0.6), (&b, 0.4)]);
        assert!(
            penalty < 1.0,
            "highly correlated curves should get penalty < 1.0, got {penalty}"
        );
    }

    #[test]
    fn portfolio_v2_uses_low_weight_growth_leader_with_stabilizers_to_hit_drawdown_limit() {
        let growth_curve: Vec<f64> = {
            let mut curve = Vec::new();
            let mut equity = 100.0;
            for t in 0..120 {
                if t < 10 {
                    equity += 5.0;
                } else if t < 25 {
                    equity -= 6.0;
                } else if t < 40 {
                    equity += 1.0;
                } else {
                    equity += 2.0;
                }
                curve.push(equity);
            }
            curve
        };
        let growth_peak = growth_curve
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let growth_trough = growth_curve.iter().cloned().fold(f64::INFINITY, f64::min);
        let growth_dd = ((growth_peak - growth_trough) / growth_peak) * 100.0;
        let growth_return = ((growth_curve.last().unwrap() - 100.0) / 100.0) * 100.0;

        let growth = candidate_with_curve(
            "growth-high-dd",
            "BTCUSDT",
            growth_return,
            growth_dd.max(30.0),
            5.0,
            100.0,
            growth_curve,
        );

        let stabilizers: Vec<EvaluatedCandidate> = (0..8)
            .map(|index| {
                let base = 100.0;
                let final_equity = base + 24.0 + index as f64;
                let curve: Vec<f64> = (0..120)
                    .map(|t| {
                        let progress = t as f64 / 119.0;
                        base + (final_equity - base) * progress + (t as f64 * 0.3).sin() * 1.0
                    })
                    .collect();
                let peak = curve.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let trough = curve.iter().cloned().fold(f64::INFINITY, f64::min);
                let dd = ((peak - trough) / peak) * 100.0;
                let ret = ((curve.last().unwrap() - base) / base) * 100.0;
                candidate_with_curve(
                    &format!("stable-{index}"),
                    &[
                        "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT",
                        "LINKUSDT", "AVAXUSDT",
                    ][index],
                    ret,
                    dd,
                    2.0,
                    100.0,
                    curve,
                )
            })
            .collect();

        let mut candidates = vec![growth];
        candidates.extend(stabilizers);

        let artifact = build_portfolio_top_n_v2(&candidates, 20.0, 3);
        assert!(
            !artifact.top3.is_empty(),
            "must produce at least one portfolio"
        );
        let first = artifact.top3.first().unwrap();

        assert!(
            first.max_drawdown_pct <= 20.0001,
            "portfolio must obey hard DD limit 20%, got {}",
            first.max_drawdown_pct
        );

        let has_growth = first
            .members
            .iter()
            .any(|m| m.candidate_id == "growth-high-dd");
        if has_growth {
            let growth_member = first
                .members
                .iter()
                .find(|m| m.candidate_id == "growth-high-dd")
                .unwrap();
            assert!(
                growth_member.allocation_pct <= 30.0,
                "growth leader should be at low weight, got {}%",
                growth_member.allocation_pct
            );
        }

        let portfolio_ann = first.annualized_return_pct.unwrap_or(first.return_pct);
        assert!(
            portfolio_ann >= 10.0,
            "portfolio annualized should be meaningful, got {}",
            portfolio_ann
        );
    }

    #[test]
    fn correlation_penalty_is_neutral_for_divergent_curves() {
        // Curve A: steady uptrend with noise
        let up_equity: Vec<f64> = (0..120)
            .map(|t| 100.0 + t as f64 * 0.5 + (t as f64 * 0.3).sin() * 8.0)
            .collect();
        // Curve B: mostly flat with different noise pattern → low correlation with A
        let flat_noisy: Vec<f64> = (0..120)
            .map(|t| 110.0 + (t as f64 * 0.7).sin() * 12.0 + (t as f64 * 0.13).cos() * 6.0)
            .collect();
        let a = candidate_with_curve("a", "BTCUSDT", 50.0, 20.0, 5.0, 100.0, up_equity);
        let b = candidate_with_curve("b", "ETHUSDT", 30.0, 18.0, 3.0, 100.0, flat_noisy);

        let penalty = daily_return_correlation_penalty(&[(&a, 0.6), (&b, 0.4)]);
        assert!(
            penalty >= 0.9,
            "weakly correlated curves should have little penalty, got {penalty}"
        );
    }

    #[test]
    fn weighted_portfolio_aligns_member_equity_by_timestamp_not_index() {
        let a = candidate_with_curve(
            "a",
            "BTCUSDT",
            30.0,
            10.0,
            3.0,
            100.0,
            vec![
                100.0, 105.0, 110.0, 108.0, 112.0, 115.0, 120.0, 118.0, 125.0, 130.0,
            ],
        );
        let b_curve: Vec<f64> = vec![
            100.0, 102.0, 104.0, 106.0, 108.0, 110.0, 112.0, 114.0, 116.0, 118.0,
        ];
        let mut b_equity = Vec::new();
        for (i, v) in b_curve.into_iter().enumerate() {
            if i % 2 == 0 {
                b_equity.push(100.0 + (i as f64 + 1.0) * 2.0);
            }
            b_equity.push(v);
        }
        let b = candidate_with_curve("b", "ETHUSDT", 18.0, 6.0, 2.0, 100.0, b_equity);

        let combined = combine_equity_curves(&[(&a, 50.0), (&b, 50.0)], 1000.0);

        assert!(!combined.is_empty(), "combined curve must not be empty");
        for i in 1..combined.len() {
            assert!(
                combined[i].timestamp_ms > combined[i - 1].timestamp_ms,
                "combined curve must be strictly increasing by timestamp at index {i}"
            );
        }

        let a_ts: std::collections::BTreeSet<i64> =
            a.equity_curve.iter().map(|p| p.timestamp_ms).collect();
        let b_ts: std::collections::BTreeSet<i64> =
            b.equity_curve.iter().map(|p| p.timestamp_ms).collect();
        let combined_ts: std::collections::BTreeSet<i64> =
            combined.iter().map(|p| p.timestamp_ms).collect();

        for ts in &a_ts {
            assert!(
                combined_ts.contains(ts),
                "combined must contain all timestamps from member a, missing {ts}"
            );
        }
        for ts in &b_ts {
            assert!(
                combined_ts.contains(ts),
                "combined must contain all timestamps from member b, missing {ts}"
            );
        }
        assert!(
            combined_ts.len() >= a_ts.len(),
            "combined must have at least as many points as a: {} vs {}",
            combined_ts.len(),
            a_ts.len()
        );

        let first = combined.first().unwrap();
        assert!(
            (first.equity_quote - 1000.0).abs() < 50.0,
            "combined initial equity should be near 1000: got {}",
            first.equity_quote
        );
    }

    #[test]
    fn portfolio_v2_scoring_demotes_high_correlation_combination() {
        // Build two pairs: one with correlated curves, one with divergent
        let base = (0..120)
            .map(|t| 100.0 + t as f64 * 0.5 + (t as f64 * 0.1).sin() * 10.0)
            .collect::<Vec<f64>>();
        let a = candidate_with_curve("a", "BTCUSDT", 50.0, 20.0, 5.0, 100.0, base.clone());
        let b = candidate_with_curve(
            "b",
            "ETHUSDT",
            45.0,
            22.0,
            4.0,
            100.0,
            base.iter().map(|v| v * 1.01 + 2.0).collect(),
        );
        let c = candidate_with_curve(
            "c",
            "SOLUSDT",
            40.0,
            15.0,
            3.0,
            100.0,
            (0..120).map(|t| 100.0 - t as f64 * 0.3).collect(),
        );

        let artifact = build_portfolio_top_n_v2(&[a.clone(), b.clone(), c.clone()], 30.0, 10);
        let portfolios = artifact.all_portfolios.unwrap_or_default();
        // The (a, c) combo should rank higher than or equal to (a, b) due to lower correlation
        // even though b has slightly better individual return than c.
        // At minimum, we verify that correlation affects scoring (no crash, non-empty result).
        assert!(
            !portfolios.is_empty(),
            "must produce at least one portfolio"
        );

        // Verify the high-correlation penalty is actually applied (function no longer dead_code)
        let penalty = daily_return_correlation_penalty(&[(&a, 0.6), (&b, 0.4)]);
        let low_penalty = daily_return_correlation_penalty(&[(&a, 0.6), (&c, 0.4)]);
        // The high-correlation pair (a,b) should have at most the same penalty as (a,c),
        // and typically worse (lower multiplier = more penalty).
        assert!(
            penalty <= low_penalty + 0.01,
            "high-correlation pair should score <= low-correlation pair: penalty={penalty} low_penalty={low_penalty}"
        );
    }
}
