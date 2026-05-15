use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleMarginMode, MartingaleMarketKind,
    MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel,
    MartingaleStrategyConfig, MartingaleTakeProfitModel,
};

#[derive(Debug, Clone, PartialEq)]
pub struct SearchSpace {
    pub symbols: Vec<String>,
    pub directions: Vec<MartingaleDirection>,
    pub market: Option<MartingaleMarketKind>,
    pub margin_mode: Option<MartingaleMarginMode>,
    pub step_bps: Vec<u32>,
    pub first_order_quote: Vec<Decimal>,
    pub multiplier: Vec<Decimal>,
    pub take_profit_bps: Vec<u32>,
    pub leverage: Vec<u32>,
    pub max_legs: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchCandidate {
    pub candidate_id: String,
    pub config: MartingalePortfolioConfig,
}

pub fn drawdown_limit_sequence(risk_profile: &str) -> Vec<f64> {
    match risk_profile {
        "conservative" => vec![20.0, 25.0],
        "balanced" => vec![25.0, 30.0],
        "aggressive" => vec![30.0],
        _ => vec![25.0, 30.0],
    }
}

pub fn random_search(
    space: &SearchSpace,
    count: usize,
    seed: u64,
) -> Result<Vec<SearchCandidate>, String> {
    validate_space(space)?;
    let mut rng = StdRng::seed_from_u64(seed);
    let mut candidates = Vec::with_capacity(count);

    for index in 0..count {
        let symbol = pick(&space.symbols, &mut rng)?.clone();
        let direction = *pick(&space.directions, &mut rng)?;
        let step_bps = *pick(&space.step_bps, &mut rng)?;
        let first_order_quote = *pick(&space.first_order_quote, &mut rng)?;
        let multiplier = *pick(&space.multiplier, &mut rng)?;
        let take_profit_bps = *pick(&space.take_profit_bps, &mut rng)?;
        let leverage = *pick(&space.leverage, &mut rng)?;
        let max_legs = *pick(&space.max_legs, &mut rng)?;

        let market = space.market.unwrap_or(if leverage > 1 {
            MartingaleMarketKind::UsdMFutures
        } else {
            MartingaleMarketKind::Spot
        });
        let (margin_mode, leverage) = match market {
            MartingaleMarketKind::Spot => (None, None),
            MartingaleMarketKind::UsdMFutures => (
                Some(space.margin_mode.unwrap_or(MartingaleMarginMode::Cross)),
                Some(leverage),
            ),
        };
        let direction_mode = match direction {
            MartingaleDirection::Long => MartingaleDirectionMode::LongOnly,
            MartingaleDirection::Short => MartingaleDirectionMode::ShortOnly,
        };
        let strategy = MartingaleStrategyConfig {
            strategy_id: format!("candidate-{index}-{symbol}-{direction:?}"),
            symbol: symbol.clone(),
            market,
            direction,
            direction_mode,
            margin_mode,
            leverage,
            spacing: MartingaleSpacingModel::FixedPercent { step_bps },
            sizing: MartingaleSizingModel::Multiplier {
                first_order_quote,
                multiplier,
                max_legs,
            },
            take_profit: MartingaleTakeProfitModel::Percent {
                bps: take_profit_bps,
            },
            stop_loss: None,
            indicators: Vec::new(),
            entry_triggers: Vec::new(),
            risk_limits: MartingaleRiskLimits::default(),
        };
        let config = MartingalePortfolioConfig {
            direction_mode,
            strategies: vec![strategy],
            risk_limits: MartingaleRiskLimits::default(),
        };
        config.validate()?;
        candidates.push(SearchCandidate {
            candidate_id: format!("seed-{seed}-{index}"),
            config,
        });
    }

    Ok(candidates)
}

fn validate_space(space: &SearchSpace) -> Result<(), String> {
    if space.symbols.is_empty()
        || space.directions.is_empty()
        || space.step_bps.is_empty()
        || space.first_order_quote.is_empty()
        || space.multiplier.is_empty()
        || space.take_profit_bps.is_empty()
        || space.leverage.is_empty()
        || space.max_legs.is_empty()
    {
        return Err("search space dimensions cannot be empty".to_string());
    }
    Ok(())
}

fn pick<'a, T>(values: &'a [T], rng: &mut StdRng) -> Result<&'a T, String> {
    if values.is_empty() {
        return Err("cannot sample empty search dimension".to_string());
    }
    Ok(&values[rng.gen_range(0..values.len())])
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegParameters {
    pub spacing_bps: u32,
    pub order_multiplier: f64,
    pub max_legs: u32,
    pub take_profit_bps: u32,
    pub tail_stop_bps: u32,
    pub weight_pct: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StagedMartingaleSearchSpace {
    pub leverage: Vec<u32>,
    pub spacing_bps: Vec<u32>,
    pub order_multiplier: Vec<f64>,
    pub max_legs: Vec<u32>,
    pub take_profit_bps: Vec<u32>,
    pub tail_stop_bps: Vec<u32>,
    pub long_short_weight_pct: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoarseParameterPoint {
    pub leverage: u32,
    pub spacing_bps: u32,
    pub order_multiplier: f64,
    pub max_legs: u32,
    pub take_profit_bps: u32,
    pub tail_stop_bps: u32,
    pub long_weight_pct: u32,
    pub short_weight_pct: u32,
}

impl StagedMartingaleSearchSpace {
    pub fn for_profile(risk_profile: &str, direction: &str) -> Self {
        let mut space = match risk_profile {
            "conservative" => Self {
                leverage: vec![2, 3, 4, 5, 6],
                spacing_bps: vec![120, 160, 220, 300, 420],
                order_multiplier: vec![1.25, 1.4, 1.6],
                max_legs: vec![3, 4, 5, 6],
                take_profit_bps: vec![60, 80, 100, 130],
                tail_stop_bps: vec![1500, 2000, 2500],
                long_short_weight_pct: vec![(80, 20), (70, 30), (60, 40)],
            },
            "aggressive" => Self {
                leverage: vec![3, 4, 5, 6, 8, 10],
                spacing_bps: vec![50, 80, 120, 160, 220],
                order_multiplier: vec![1.4, 1.6, 2.0, 2.4],
                max_legs: vec![4, 5, 6, 8],
                take_profit_bps: vec![80, 100, 130, 180, 240],
                tail_stop_bps: vec![2000, 2500, 3000],
                long_short_weight_pct: vec![(60, 40), (50, 50), (40, 60)],
            },
            _ => Self {
                leverage: vec![2, 3, 4, 5, 6, 8, 10],
                spacing_bps: vec![80, 120, 160, 220, 300],
                order_multiplier: vec![1.25, 1.4, 1.6, 2.0],
                max_legs: vec![4, 5, 6, 8],
                take_profit_bps: vec![80, 100, 130, 180],
                tail_stop_bps: vec![1800, 2200, 2600],
                long_short_weight_pct: vec![(80, 20), (70, 30), (60, 40), (50, 50)],
            },
        };

        if direction == "long" || direction == "long_only" {
            space.long_short_weight_pct = vec![(100, 0)];
        }
        if direction == "short" || direction == "short_only" {
            space.long_short_weight_pct = vec![(0, 100)];
        }
        space
    }
}

pub fn fine_space_around(winner: &CoarseParameterPoint) -> StagedMartingaleSearchSpace {
    let spacing_neighbors = [winner.spacing_bps.saturating_sub(20), winner.spacing_bps, winner.spacing_bps + 30];
    let max_legs_neighbors = [winner.max_legs.saturating_sub(1), winner.max_legs, winner.max_legs + 1];
    let tp_neighbors = [winner.take_profit_bps.saturating_sub(20), winner.take_profit_bps, winner.take_profit_bps + 30];
    let tail_neighbors = [winner.tail_stop_bps.saturating_sub(200), winner.tail_stop_bps, winner.tail_stop_bps + 200];
    let long_w = winner.long_weight_pct;
    let short_w = winner.short_weight_pct;
    let weight_neighbors = [
        (long_w.saturating_sub(5), short_w + 5),
        (long_w, short_w),
        (long_w + 5, short_w.saturating_sub(5)),
    ];

    StagedMartingaleSearchSpace {
        leverage: vec![winner.leverage.saturating_sub(1).max(1), winner.leverage, winner.leverage + 1],
        spacing_bps: spacing_neighbors.to_vec(),
        order_multiplier: vec![winner.order_multiplier - 0.2, winner.order_multiplier, winner.order_multiplier + 0.2],
        max_legs: max_legs_neighbors.to_vec(),
        take_profit_bps: tp_neighbors.to_vec(),
        tail_stop_bps: tail_neighbors.to_vec(),
        long_short_weight_pct: weight_neighbors.to_vec(),
    }
}

#[cfg(test)]
mod staged_tests {
    use super::*;

    #[test]
    fn staged_search_space_covers_required_futures_ranges() {
        let space = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");

        assert!(space.leverage.contains(&2));
        assert!(space.leverage.contains(&10));
        assert!(space.spacing_bps.iter().any(|value| *value <= 80));
        assert!(space.spacing_bps.iter().any(|value| *value >= 220));
        assert!(space.order_multiplier.contains(&1.4));
        assert!(space.order_multiplier.contains(&2.0));
        assert!(space.max_legs.contains(&4));
        assert!(space.max_legs.contains(&8));
        assert!(space.long_short_weight_pct.contains(&(80, 20)));
        assert!(space.long_short_weight_pct.contains(&(50, 50)));
    }

    #[test]
    fn fine_search_expands_around_coarse_winner() {
        let winner = CoarseParameterPoint {
            leverage: 4,
            spacing_bps: 120,
            order_multiplier: 1.6,
            max_legs: 5,
            take_profit_bps: 100,
            tail_stop_bps: 1800,
            long_weight_pct: 70,
            short_weight_pct: 30,
        };

        let fine = fine_space_around(&winner);

        assert!(fine.spacing_bps.contains(&100));
        assert!(fine.spacing_bps.contains(&120));
        assert!(fine.spacing_bps.contains(&150));
        assert!(fine.max_legs.contains(&4));
        assert!(fine.max_legs.contains(&6));
        assert!(fine.long_short_weight_pct.contains(&(65, 35)));
        assert!(fine.long_short_weight_pct.contains(&(75, 25)));
    }
}
