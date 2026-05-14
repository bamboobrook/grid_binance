use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleMarginMode, MartingaleMarketKind,
    MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel,
    MartingaleStopLossModel, MartingaleStrategyConfig, MartingaleTakeProfitModel,
};

#[derive(Debug, Clone, PartialEq)]
pub struct SearchSpace {
    pub symbols: Vec<String>,
    pub direction_mode: MartingaleDirectionMode,
    pub directions: Vec<MartingaleDirection>,
    pub market: Option<MartingaleMarketKind>,
    pub margin_mode: Option<MartingaleMarginMode>,
    pub step_bps: Vec<u32>,
    pub first_order_quote: Vec<Decimal>,
    pub multiplier: Vec<Decimal>,
    pub take_profit_bps: Vec<u32>,
    pub leverage: Vec<u32>,
    pub max_legs: Vec<u32>,
    pub dynamic_allocation_enabled: bool,
    pub short_stop_drawdown_pct_candidates: Vec<f64>,
    pub short_atr_stop_multiplier_candidates: Vec<f64>,
    pub allocation_cooldown_hours_candidates: Vec<u32>,
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
        let leverage = *pick(&space.leverage, &mut rng)?;
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
        let mut strategies = Vec::new();
        for direction in candidate_directions(space, &mut rng)? {
            let step_bps = *pick(&space.step_bps, &mut rng)?;
            let first_order_quote = *pick(&space.first_order_quote, &mut rng)?;
            let multiplier = *pick(&space.multiplier, &mut rng)?;
            let take_profit_bps = *pick(&space.take_profit_bps, &mut rng)?;
            let max_legs = *pick(&space.max_legs, &mut rng)?;
            let stop_loss = default_stop_loss(space, market, direction, &mut rng)?;
            strategies.push(MartingaleStrategyConfig {
                strategy_id: format!("candidate-{index}-{symbol}-{direction:?}"),
                symbol: symbol.clone(),
                market,
                direction,
                direction_mode: space.direction_mode,
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
                stop_loss,
                indicators: Vec::new(),
                entry_triggers: Vec::new(),
                risk_limits: MartingaleRiskLimits::default(),
            });
        }
        let config = MartingalePortfolioConfig {
            direction_mode: space.direction_mode,
            strategies,
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

fn default_stop_loss(
    space: &SearchSpace,
    market: MartingaleMarketKind,
    direction: MartingaleDirection,
    rng: &mut StdRng,
) -> Result<Option<MartingaleStopLossModel>, String> {
    if market != MartingaleMarketKind::UsdMFutures || direction != MartingaleDirection::Short {
        return Ok(None);
    }

    if !space.short_atr_stop_multiplier_candidates.is_empty()
        && (space.short_stop_drawdown_pct_candidates.is_empty() || rng.gen_bool(0.5))
    {
        let multiplier = *pick(&space.short_atr_stop_multiplier_candidates, rng)?;
        if let Some(multiplier) = decimal_from_f64(multiplier) {
            return Ok(Some(MartingaleStopLossModel::Atr { multiplier }));
        }
    }

    let pct = if space.short_stop_drawdown_pct_candidates.is_empty() {
        20.0
    } else {
        *pick(&space.short_stop_drawdown_pct_candidates, rng)?
    };
    Ok(Some(MartingaleStopLossModel::StrategyDrawdownPct {
        pct_bps: pct_to_bps(pct).unwrap_or(2_000),
    }))
}

fn decimal_from_f64(value: f64) -> Option<Decimal> {
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    Decimal::try_from(value).ok()
}

fn pct_to_bps(pct: f64) -> Option<u32> {
    if !pct.is_finite() || pct <= 0.0 {
        return None;
    }
    u32::try_from((pct * 100.0).round() as i64).ok()
}

fn candidate_directions(
    space: &SearchSpace,
    rng: &mut StdRng,
) -> Result<Vec<MartingaleDirection>, String> {
    match space.direction_mode {
        MartingaleDirectionMode::LongAndShort => {
            Ok(vec![MartingaleDirection::Long, MartingaleDirection::Short])
        }
        MartingaleDirectionMode::LongOnly => Ok(vec![MartingaleDirection::Long]),
        MartingaleDirectionMode::ShortOnly => Ok(vec![MartingaleDirection::Short]),
        MartingaleDirectionMode::IndicatorSelected => Ok(vec![*pick(&space.directions, rng)?]),
    }
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
