use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

const TOTAL_WEIGHT_PCT: u32 = 100;
const COARSE_STEP_PCT: u32 = 10;
const REFINEMENT_STEP_PCT: u32 = 5;
const REFINEMENT_RADIUS_PCT: u32 = 5;
const MIN_RETAINED_PORTFOLIOS: usize = 32;

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

    let filtered = eligible_candidates(candidates);
    if !can_reach_full_weight(&filtered, config) {
        return Ok(Vec::new());
    }

    let retain_limit = retained_limit(limit);
    let coarse_weights = search_coarse_weights(&filtered, config, retain_limit);
    if coarse_weights.is_empty() {
        return Ok(Vec::new());
    }

    let mut portfolios = BTreeMap::new();
    for weights in &coarse_weights {
        insert_portfolio(&filtered, config, weights, &mut portfolios);
    }
    refine_weights(
        &filtered,
        config,
        &coarse_weights,
        retain_limit,
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

fn eligible_candidates(candidates: &[OptimizerCandidate]) -> Vec<&OptimizerCandidate> {
    let mut filtered = candidates
        .iter()
        .filter(|candidate| is_eligible(candidate))
        .collect::<Vec<_>>();
    filtered.sort_by(compare_candidates);
    filtered
}

fn compare_candidates(left: &&OptimizerCandidate, right: &&OptimizerCandidate) -> Ordering {
    right
        .total_return_pct
        .partial_cmp(&left.total_return_pct)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.candidate_id.cmp(&right.candidate_id))
}

fn is_eligible(candidate: &OptimizerCandidate) -> bool {
    candidate.total_return_pct.is_finite()
        && candidate.max_drawdown_pct.is_finite()
        && candidate.max_drawdown_pct >= 0.0
        && candidate.total_return_pct >= 0.0
        && candidate.equity_curve.iter().all(|value| value.is_finite())
}

fn can_reach_full_weight(candidates: &[&OptimizerCandidate], config: &OptimizerConfig) -> bool {
    if candidates.is_empty() {
        return false;
    }
    let package_capacity = candidates.len() as f64 * config.max_package_weight_pct;
    let symbol_capacity = symbol_counts(candidates).len() as f64 * config.max_symbol_weight_pct;
    package_capacity + f64::EPSILON >= 100.0 && symbol_capacity + f64::EPSILON >= 100.0
}

fn symbol_counts(candidates: &[&OptimizerCandidate]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for candidate in candidates {
        *counts.entry(candidate.symbol.clone()).or_default() += 1;
    }
    counts
}

fn retained_limit(limit: usize) -> usize {
    limit.saturating_mul(8).max(MIN_RETAINED_PORTFOLIOS)
}

fn search_coarse_weights(
    candidates: &[&OptimizerCandidate],
    config: &OptimizerConfig,
    retain_limit: usize,
) -> Vec<Vec<u32>> {
    let mut search = WeightSearch::new(candidates, config, COARSE_STEP_PCT, retain_limit);
    search.run();
    search.into_weights()
}

fn refine_weights(
    candidates: &[&OptimizerCandidate],
    config: &OptimizerConfig,
    coarse_weights: &[Vec<u32>],
    retain_limit: usize,
    portfolios: &mut BTreeMap<String, PortfolioCandidate>,
) {
    let mut generated = BTreeMap::<String, Vec<u32>>::new();
    for weights in coarse_weights.iter().take(retain_limit) {
        generated.insert(weight_key(weights), weights.clone());
        for from_index in 0..weights.len() {
            if weights[from_index] < REFINEMENT_STEP_PCT {
                continue;
            }
            for to_index in 0..weights.len() {
                if from_index == to_index {
                    continue;
                }
                let mut refined = weights.clone();
                refined[from_index] -= REFINEMENT_STEP_PCT;
                refined[to_index] += REFINEMENT_STEP_PCT;
                if refined[from_index].abs_diff(weights[from_index]) > REFINEMENT_RADIUS_PCT
                    || refined[to_index].abs_diff(weights[to_index]) > REFINEMENT_RADIUS_PCT
                {
                    continue;
                }
                if is_valid_weight_set(candidates, config, &refined) {
                    generated.insert(weight_key(&refined), refined);
                }
            }
        }
    }

    let mut refined_weights = generated.into_values().collect::<Vec<_>>();
    refined_weights.sort_by(|left, right| compare_weight_sets(candidates, config, left, right));
    refined_weights.truncate(retain_limit);
    for weights in &refined_weights {
        insert_portfolio(candidates, config, weights, portfolios);
    }
}

struct WeightSearch<'a> {
    candidates: &'a [&'a OptimizerCandidate],
    config: &'a OptimizerConfig,
    step_pct: u32,
    retain_limit: usize,
    max_package_weight_pct: u32,
    max_symbol_weight_pct: u32,
    weights: Vec<u32>,
    symbol_weights: HashMap<&'a str, u32>,
    symbol_packages: HashMap<&'a str, usize>,
    best_weights: Vec<Vec<u32>>,
}

impl<'a> WeightSearch<'a> {
    fn new(
        candidates: &'a [&'a OptimizerCandidate],
        config: &'a OptimizerConfig,
        step_pct: u32,
        retain_limit: usize,
    ) -> Self {
        Self {
            candidates,
            config,
            step_pct,
            retain_limit,
            max_package_weight_pct: config.max_package_weight_pct.floor() as u32,
            max_symbol_weight_pct: config.max_symbol_weight_pct.floor() as u32,
            weights: vec![0; candidates.len()],
            symbol_weights: HashMap::new(),
            symbol_packages: HashMap::new(),
            best_weights: Vec::new(),
        }
    }

    fn run(&mut self) {
        self.assign(0, TOTAL_WEIGHT_PCT);
    }

    fn into_weights(self) -> Vec<Vec<u32>> {
        self.best_weights
    }

    fn assign(&mut self, index: usize, remaining_pct: u32) {
        if index == self.candidates.len() {
            if remaining_pct == 0 {
                self.push_current();
            }
            return;
        }

        if !self.remaining_capacity_can_fill(index, remaining_pct) {
            return;
        }

        let candidate = self.candidates[index];
        let current_symbol_weight = self
            .symbol_weights
            .get(candidate.symbol.as_str())
            .copied()
            .unwrap_or_default();
        let symbol_room = self
            .max_symbol_weight_pct
            .saturating_sub(current_symbol_weight);
        let max_weight = remaining_pct
            .min(self.max_package_weight_pct)
            .min(symbol_room);

        for weight in stepped_weights(max_weight, self.step_pct) {
            if remaining_pct < weight {
                break;
            }
            if self.can_assign(candidate, weight) {
                self.apply_weight(candidate, weight);
                self.weights[index] = weight;
                self.assign(index + 1, remaining_pct - weight);
                self.weights[index] = 0;
                self.remove_weight(candidate, weight);
            }
        }
    }

    fn remaining_capacity_can_fill(&self, index: usize, remaining_pct: u32) -> bool {
        let mut total_capacity = 0_u32;
        let mut symbol_capacity = self.symbol_weights.clone();
        for candidate in &self.candidates[index..] {
            let symbol = candidate.symbol.as_str();
            let symbol_room = self
                .max_symbol_weight_pct
                .saturating_sub(symbol_capacity.get(symbol).copied().unwrap_or_default());
            let capacity = self.max_package_weight_pct.min(symbol_room);
            if capacity > 0 {
                total_capacity += capacity;
                *symbol_capacity.entry(symbol).or_default() += capacity;
            }
        }
        total_capacity >= remaining_pct
    }

    fn can_assign(&self, candidate: &OptimizerCandidate, weight: u32) -> bool {
        if weight == 0 {
            return true;
        }
        let symbol = candidate.symbol.as_str();
        let package_count = self
            .symbol_packages
            .get(symbol)
            .copied()
            .unwrap_or_default();
        let symbol_weight = self.symbol_weights.get(symbol).copied().unwrap_or_default();
        package_count < self.config.max_packages_per_symbol
            && symbol_weight + weight <= self.max_symbol_weight_pct
            && weight <= self.max_package_weight_pct
    }

    fn apply_weight(&mut self, candidate: &'a OptimizerCandidate, weight: u32) {
        if weight == 0 {
            return;
        }
        *self
            .symbol_weights
            .entry(candidate.symbol.as_str())
            .or_default() += weight;
        *self
            .symbol_packages
            .entry(candidate.symbol.as_str())
            .or_default() += 1;
    }

    fn remove_weight(&mut self, candidate: &'a OptimizerCandidate, weight: u32) {
        if weight == 0 {
            return;
        }
        let symbol = candidate.symbol.as_str();
        if let Some(symbol_weight) = self.symbol_weights.get_mut(symbol) {
            *symbol_weight -= weight;
            if *symbol_weight == 0 {
                self.symbol_weights.remove(symbol);
            }
        }
        if let Some(package_count) = self.symbol_packages.get_mut(symbol) {
            *package_count -= 1;
            if *package_count == 0 {
                self.symbol_packages.remove(symbol);
            }
        }
    }

    fn push_current(&mut self) {
        if !is_valid_weight_set(self.candidates, self.config, &self.weights) {
            return;
        }
        if build_portfolio(self.candidates, self.config, &self.weights).is_none() {
            return;
        }
        self.best_weights.push(self.weights.clone());
        self.best_weights
            .sort_by(|left, right| compare_weight_sets(self.candidates, self.config, left, right));
        self.best_weights.dedup();
        self.best_weights.truncate(self.retain_limit);
    }
}

fn stepped_weights(max_weight: u32, step_pct: u32) -> Vec<u32> {
    let mut weights = Vec::new();
    let mut weight = 0;
    while weight <= max_weight {
        weights.push(weight);
        weight += step_pct;
    }
    if weights.last().copied() != Some(max_weight) {
        weights.push(max_weight);
    }
    weights
}

fn is_valid_weight_set(
    candidates: &[&OptimizerCandidate],
    config: &OptimizerConfig,
    weights: &[u32],
) -> bool {
    if weights.iter().sum::<u32>() != TOTAL_WEIGHT_PCT {
        return false;
    }

    let mut symbol_weights: HashMap<&str, f64> = HashMap::new();
    let mut symbol_packages: HashMap<&str, usize> = HashMap::new();

    for (candidate, weight) in candidates.iter().zip(weights.iter().copied()) {
        if weight == 0 {
            continue;
        }
        let weight = weight as f64;
        if weight > config.max_package_weight_pct + f64::EPSILON {
            return false;
        }
        *symbol_weights.entry(candidate.symbol.as_str()).or_default() += weight;
        *symbol_packages
            .entry(candidate.symbol.as_str())
            .or_default() += 1;
    }

    for (symbol, weight) in &symbol_weights {
        if *weight > config.max_symbol_weight_pct + f64::EPSILON {
            return false;
        }
        if symbol_packages.get(symbol).copied().unwrap_or_default() > config.max_packages_per_symbol
        {
            return false;
        }
    }

    true
}

fn compare_weight_sets(
    candidates: &[&OptimizerCandidate],
    config: &OptimizerConfig,
    left: &[u32],
    right: &[u32],
) -> Ordering {
    match (
        build_portfolio(candidates, config, left),
        build_portfolio(candidates, config, right),
    ) {
        (Some(left), Some(right)) => compare_portfolios(&left, &right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left.cmp(right),
    }
}

fn insert_portfolio(
    candidates: &[&OptimizerCandidate],
    config: &OptimizerConfig,
    weights: &[u32],
    portfolios: &mut BTreeMap<String, PortfolioCandidate>,
) {
    if let Some(portfolio) = build_portfolio(candidates, config, weights) {
        portfolios.insert(weight_key(weights), portfolio);
    }
}

fn build_portfolio(
    candidates: &[&OptimizerCandidate],
    config: &OptimizerConfig,
    weights: &[u32],
) -> Option<PortfolioCandidate> {
    if !is_valid_weight_set(candidates, config, weights) {
        return None;
    }

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
        .filter(|(_, weight)| *weight > 0)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refinement_only_expands_retained_coarse_neighbors() {
        let candidates = vec![
            OptimizerCandidate::new("coarse-a", "BTCUSDT", 100.0, 1.0, Vec::new()),
            OptimizerCandidate::new("coarse-b", "ETHUSDT", 90.0, 1.0, Vec::new()),
            OptimizerCandidate::new("global-five-pct-best", "SOLUSDT", 1_000.0, 1.0, Vec::new()),
        ];
        let candidate_refs = candidates.iter().collect::<Vec<_>>();
        let config = OptimizerConfig {
            max_drawdown_pct: 100.0,
            max_packages_per_symbol: 1,
            max_symbol_weight_pct: 100.0,
            max_package_weight_pct: 100.0,
        };
        let coarse_weights = vec![vec![50, 50, 0]];
        let mut portfolios = BTreeMap::new();

        refine_weights(&candidate_refs, &config, &coarse_weights, 32, &mut portfolios);

        assert!(portfolios.contains_key("55-45-0"));
        assert!(portfolios.contains_key("45-50-5"));
        assert!(!portfolios.contains_key("0-0-100"));
    }
}
