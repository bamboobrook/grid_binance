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
    pub step_bps: Vec<u32>,
    pub first_order_quote: Vec<Decimal>,
    pub take_profit_bps: Vec<u32>,
    pub leverage: Vec<u32>,
    pub max_legs: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchCandidate {
    pub candidate_id: String,
    pub config: MartingalePortfolioConfig,
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
        let take_profit_bps = *pick(&space.take_profit_bps, &mut rng)?;
        let leverage = *pick(&space.leverage, &mut rng)?;
        let max_legs = *pick(&space.max_legs, &mut rng)?;

        let market = if leverage > 1 {
            MartingaleMarketKind::UsdMFutures
        } else {
            MartingaleMarketKind::Spot
        };
        let (margin_mode, leverage) = match market {
            MartingaleMarketKind::Spot => (None, None),
            MartingaleMarketKind::UsdMFutures => {
                (Some(MartingaleMarginMode::Cross), Some(leverage))
            }
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
                multiplier: Decimal::new(15, 1),
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
