use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq)]
pub struct OptimizerCandidate {
    pub candidate_id: String,
    pub symbol: String,
    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub equity_curve: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortfolioItem {
    pub candidate_id: String,
    pub symbol: String,
    pub weight_pct: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortfolioCandidate {
    pub items: Vec<PortfolioItem>,
    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub return_drawdown_ratio: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct OptimizerConfig {
    pub max_drawdown_pct: f64,
    pub max_packages_per_symbol: usize,
    pub max_symbol_weight_pct: f64,
    pub max_package_weight_pct: f64,
}

impl OptimizerConfig {
    pub fn conservative(max_drawdown_pct: f64) -> Self {
        Self {
            max_drawdown_pct,
            max_packages_per_symbol: 2,
            max_symbol_weight_pct: 25.0,
            max_package_weight_pct: 10.0,
        }
    }

    pub fn balanced(max_drawdown_pct: f64) -> Self {
        Self {
            max_drawdown_pct,
            max_packages_per_symbol: 2,
            max_symbol_weight_pct: 35.0,
            max_package_weight_pct: 15.0,
        }
    }

    pub fn aggressive(max_drawdown_pct: f64) -> Self {
        Self {
            max_drawdown_pct,
            max_packages_per_symbol: 3,
            max_symbol_weight_pct: 50.0,
            max_package_weight_pct: 25.0,
        }
    }
}

impl OptimizerCandidate {
    pub fn new(
        candidate_id: impl Into<String>,
        symbol: impl Into<String>,
        total_return_pct: f64,
        max_drawdown_pct: f64,
        equity_curve: Vec<f64>,
    ) -> Self {
        Self {
            candidate_id: candidate_id.into(),
            symbol: symbol.into(),
            total_return_pct,
            max_drawdown_pct,
            equity_curve,
        }
    }
}

impl PortfolioCandidate {
    pub fn total_weight_pct(&self) -> f64 {
        round_weight(self.items.iter().map(|item| item.weight_pct).sum())
    }

    pub fn weight_by_symbol(&self, symbol: &str) -> f64 {
        round_weight(
            self.items
                .iter()
                .filter(|item| item.symbol == symbol)
                .map(|item| item.weight_pct)
                .sum(),
        )
    }
}

pub fn optimize_portfolios(
    candidates: &[OptimizerCandidate],
    config: &OptimizerConfig,
    limit: usize,
) -> Result<Vec<PortfolioCandidate>, String> {
    validate_config(config)?;
    if limit == 0 {
        return Ok(Vec::new());
    }

    let filtered = eligible_candidates(candidates, config.max_packages_per_symbol);
    if filtered.is_empty() {
        return Ok(Vec::new());
    }

    let symbol_cap_is_feasible =
        unique_symbol_count(&filtered) as f64 * config.max_symbol_weight_pct >= 100.0;
    let package_cap = if filtered.len() as f64 * config.max_package_weight_pct >= 100.0 {
        config.max_package_weight_pct
    } else {
        100.0
    };

    let mut portfolios = BTreeMap::new();
    search_weights(
        &filtered,
        config,
        package_cap,
        symbol_cap_is_feasible,
        10,
        &mut portfolios,
    );
    search_weights(
        &filtered,
        config,
        package_cap,
        symbol_cap_is_feasible,
        5,
        &mut portfolios,
    );

    let mut portfolios = portfolios.into_values().collect::<Vec<_>>();
    portfolios.sort_by(compare_portfolios);
    portfolios.truncate(limit);
    Ok(portfolios)
}

fn validate_config(config: &OptimizerConfig) -> Result<(), String> {
    if !config.max_drawdown_pct.is_finite() || config.max_drawdown_pct < 0.0 {
        return Err("max_drawdown_pct must be finite and non-negative".to_owned());
    }
    if config.max_packages_per_symbol == 0 {
        return Err("max_packages_per_symbol must be positive".to_owned());
    }
    validate_pct("max_symbol_weight_pct", config.max_symbol_weight_pct)?;
    validate_pct("max_package_weight_pct", config.max_package_weight_pct)?;
    Ok(())
}

fn validate_pct(name: &str, value: f64) -> Result<(), String> {
    if !value.is_finite() || value <= 0.0 || value > 100.0 {
        return Err(format!("{name} must be finite and in (0, 100]"));
    }
    Ok(())
}

fn eligible_candidates<'a>(
    candidates: &'a [OptimizerCandidate],
    max_packages_per_symbol: usize,
) -> Vec<&'a OptimizerCandidate> {
    let mut by_symbol: HashMap<&str, Vec<&OptimizerCandidate>> = HashMap::new();
    for candidate in candidates.iter().filter(|candidate| is_eligible(candidate)) {
        by_symbol
            .entry(candidate.symbol.as_str())
            .or_default()
            .push(candidate);
    }

    let mut filtered = Vec::new();
    for candidates in by_symbol.values_mut() {
        candidates.sort_by(|left, right| {
            right
                .total_return_pct
                .partial_cmp(&left.total_return_pct)
                .unwrap_or(Ordering::Equal)
        });
        filtered.extend(candidates.iter().take(max_packages_per_symbol).copied());
    }
    filtered.sort_by(|left, right| {
        right
            .total_return_pct
            .partial_cmp(&left.total_return_pct)
            .unwrap_or(Ordering::Equal)
    });
    filtered
}

fn is_eligible(candidate: &OptimizerCandidate) -> bool {
    candidate.total_return_pct.is_finite()
        && candidate.max_drawdown_pct.is_finite()
        && candidate.max_drawdown_pct >= 0.0
        && candidate.total_return_pct >= 0.0
        && candidate.equity_curve.iter().all(|value| value.is_finite())
}

fn unique_symbol_count(candidates: &[&OptimizerCandidate]) -> usize {
    candidates
        .iter()
        .map(|candidate| candidate.symbol.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

fn search_weights(
    candidates: &[&OptimizerCandidate],
    config: &OptimizerConfig,
    package_cap: f64,
    symbol_cap_is_feasible: bool,
    step_pct: u32,
    portfolios: &mut BTreeMap<String, PortfolioCandidate>,
) {
    let mut weights = vec![0_u32; candidates.len()];
    assign_weight(
        candidates,
        config,
        package_cap,
        symbol_cap_is_feasible,
        step_pct,
        0,
        100,
        &mut weights,
        portfolios,
    );
}

#[allow(clippy::too_many_arguments)]
fn assign_weight(
    candidates: &[&OptimizerCandidate],
    config: &OptimizerConfig,
    package_cap: f64,
    symbol_cap_is_feasible: bool,
    step_pct: u32,
    index: usize,
    remaining_pct: u32,
    weights: &mut [u32],
    portfolios: &mut BTreeMap<String, PortfolioCandidate>,
) {
    if index == candidates.len() - 1 {
        weights[index] = remaining_pct;
        if is_valid_weight_set(
            candidates,
            config,
            package_cap,
            symbol_cap_is_feasible,
            weights,
        ) {
            if let Some(portfolio) = build_portfolio(candidates, config, weights) {
                portfolios.insert(weight_key(weights), portfolio);
            }
        }
        weights[index] = 0;
        return;
    }

    let max_weight = remaining_pct.min(package_cap.round() as u32);
    let mut weight = 0;
    while weight <= max_weight {
        weights[index] = weight;
        assign_weight(
            candidates,
            config,
            package_cap,
            symbol_cap_is_feasible,
            step_pct,
            index + 1,
            remaining_pct - weight,
            weights,
            portfolios,
        );
        weight += step_pct;
    }
    weights[index] = 0;
}

fn is_valid_weight_set(
    candidates: &[&OptimizerCandidate],
    config: &OptimizerConfig,
    package_cap: f64,
    symbol_cap_is_feasible: bool,
    weights: &[u32],
) -> bool {
    let mut symbol_weights: HashMap<&str, f64> = HashMap::new();
    let mut symbol_packages: HashMap<&str, usize> = HashMap::new();
    let mut available_packages: HashMap<&str, usize> = HashMap::new();

    for candidate in candidates {
        *available_packages
            .entry(candidate.symbol.as_str())
            .or_default() += 1;
    }

    for (candidate, weight) in candidates.iter().zip(weights.iter().copied()) {
        if weight == 0 {
            continue;
        }
        let weight = weight as f64;
        if weight > package_cap + f64::EPSILON {
            return false;
        }
        *symbol_weights.entry(candidate.symbol.as_str()).or_default() += weight;
        *symbol_packages
            .entry(candidate.symbol.as_str())
            .or_default() += 1;
    }

    for (symbol, weight) in &symbol_weights {
        let package_count = symbol_packages.get(symbol).copied().unwrap_or_default();
        let available_package_count = available_packages.get(symbol).copied().unwrap_or_default();
        if package_count > config.max_packages_per_symbol {
            return false;
        }
        if (symbol_cap_is_feasible || available_package_count > 1)
            && *weight > config.max_symbol_weight_pct + f64::EPSILON
        {
            return false;
        }
    }

    true
}

fn build_portfolio(
    candidates: &[&OptimizerCandidate],
    config: &OptimizerConfig,
    weights: &[u32],
) -> Option<PortfolioCandidate> {
    let items = candidates
        .iter()
        .zip(weights.iter().copied())
        .filter(|(_, weight)| *weight > 0)
        .map(|(candidate, weight)| PortfolioItem {
            candidate_id: candidate.candidate_id.clone(),
            symbol: candidate.symbol.clone(),
            weight_pct: weight as f64,
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        return None;
    }

    let total_return_pct = candidates
        .iter()
        .zip(weights.iter().copied())
        .map(|(candidate, weight)| candidate.total_return_pct * weight as f64 / 100.0)
        .sum::<f64>();
    let max_drawdown_pct = portfolio_drawdown_pct(candidates, weights);
    if max_drawdown_pct > config.max_drawdown_pct + f64::EPSILON {
        return None;
    }

    Some(PortfolioCandidate {
        items,
        total_return_pct,
        max_drawdown_pct,
        return_drawdown_ratio: if max_drawdown_pct <= f64::EPSILON {
            total_return_pct
        } else {
            total_return_pct / max_drawdown_pct
        },
    })
}

fn portfolio_drawdown_pct(candidates: &[&OptimizerCandidate], weights: &[u32]) -> f64 {
    if let Some(drawdown) = equity_curve_drawdown_pct(candidates, weights) {
        return drawdown;
    }

    candidates
        .iter()
        .zip(weights.iter().copied())
        .map(|(candidate, weight)| candidate.max_drawdown_pct * weight as f64 / 100.0)
        .sum()
}

fn equity_curve_drawdown_pct(candidates: &[&OptimizerCandidate], weights: &[u32]) -> Option<f64> {
    let active = candidates
        .iter()
        .zip(weights.iter().copied())
        .filter(|(candidate, weight)| *weight > 0 && !candidate.equity_curve.is_empty())
        .collect::<Vec<_>>();
    if active.is_empty() {
        return None;
    }
    let len = active[0].0.equity_curve.len();
    if len == 0
        || !active
            .iter()
            .all(|(candidate, _)| candidate.equity_curve.len() == len)
    {
        return None;
    }

    let mut peak = f64::NEG_INFINITY;
    let mut max_drawdown = 0.0;
    for point_index in 0..len {
        let value = active
            .iter()
            .map(|(candidate, weight)| candidate.equity_curve[point_index] * *weight as f64 / 100.0)
            .sum::<f64>();
        peak = peak.max(value);
        if peak > f64::EPSILON {
            max_drawdown = f64::max(max_drawdown, (peak - value) / peak * 100.0);
        }
    }
    Some(max_drawdown)
}

fn compare_portfolios(left: &PortfolioCandidate, right: &PortfolioCandidate) -> Ordering {
    right
        .return_drawdown_ratio
        .partial_cmp(&left.return_drawdown_ratio)
        .unwrap_or(Ordering::Equal)
        .then_with(|| {
            right
                .total_return_pct
                .partial_cmp(&left.total_return_pct)
                .unwrap_or(Ordering::Equal)
        })
}

fn weight_key(weights: &[u32]) -> String {
    weights
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join("-")
}

fn round_weight(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}
