use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleEntryTrigger,
    MartingaleIndicatorConfig, MartingaleMarginMode, MartingaleMarketKind,
    MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel,
    MartingaleStrategyConfig, MartingaleTakeProfitModel,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpacingModelChoice {
    FixedPercent,
    Atr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TakeProfitModelChoice {
    Percent,
    Atr,
}

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
            entry_triggers: vec![MartingaleEntryTrigger::Cooldown { seconds: 21_600 }],
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
    pub spacing_model: SpacingModelChoice,
    pub take_profit_model: TakeProfitModelChoice,
    pub atr_period: u32,
    pub atr_spacing_multiplier_bps: u32,
    pub atr_tp_multiplier_bps: u32,
    pub adx_filter_enabled: bool,
    pub adx_threshold_bps: u32,
    pub adx_period: u32,
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
    pub spacing_model: Vec<SpacingModelChoice>,
    pub take_profit_model: Vec<TakeProfitModelChoice>,
    pub atr_period: Vec<u32>,
    pub atr_spacing_multiplier_bps: Vec<u32>,
    pub atr_tp_multiplier_bps: Vec<u32>,
    pub adx_filter_enabled: Vec<bool>,
    pub adx_threshold_bps: Vec<u32>,
    pub adx_period: Vec<u32>,
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
    pub spacing_model: SpacingModelChoice,
    pub take_profit_model: TakeProfitModelChoice,
    pub atr_period: u32,
    pub atr_spacing_multiplier_bps: u32,
    pub atr_tp_multiplier_bps: u32,
    pub adx_filter_enabled: bool,
    pub adx_threshold_bps: u32,
    pub adx_period: u32,
}

impl StagedMartingaleSearchSpace {
    pub fn for_profile(risk_profile: &str, direction: &str) -> Self {
        let mut space = match risk_profile {
            "conservative" => Self {
                leverage: vec![2, 3, 4, 5, 6, 7, 8],
                spacing_bps: vec![80, 120, 160, 220, 300, 420, 600],
                order_multiplier: vec![1.2, 1.4, 1.6, 1.8, 2.2],
                max_legs: vec![3, 4, 5, 6, 7, 8],
                take_profit_bps: vec![60, 80, 100, 130, 160, 200],
                tail_stop_bps: vec![300, 500, 800, 1000, 1500, 2000, 2500, 3500],
                long_short_weight_pct: vec![(80, 20), (70, 30), (60, 40), (50, 50)],
                spacing_model: vec![SpacingModelChoice::FixedPercent, SpacingModelChoice::Atr],
                take_profit_model: vec![TakeProfitModelChoice::Percent, TakeProfitModelChoice::Atr],
                atr_period: vec![7, 14, 21],
                atr_spacing_multiplier_bps: vec![10000, 15000, 20000],
                atr_tp_multiplier_bps: vec![10000, 15000, 20000],
                adx_filter_enabled: vec![true, false],
                adx_threshold_bps: vec![800, 1200, 1500, 1800, 2000, 2500, 3000],
                adx_period: vec![14, 21],
            },
            "aggressive" => Self {
                leverage: vec![2, 3, 4, 5, 6, 8, 10],
                spacing_bps: vec![35, 50, 70, 90, 120, 160, 220, 300, 420],
                order_multiplier: vec![1.2, 1.35, 1.5, 1.7, 2.0, 2.4, 2.8],
                max_legs: vec![3, 4, 5, 6, 8, 10],
                take_profit_bps: vec![60, 80, 120, 180, 240, 320, 450],
                tail_stop_bps: vec![1800, 2400, 3000, 3600, 4500, 6000],
                long_short_weight_pct: vec![
                    (80, 20),
                    (70, 30),
                    (60, 40),
                    (50, 50),
                    (40, 60),
                    (30, 70),
                ],
                spacing_model: vec![SpacingModelChoice::FixedPercent, SpacingModelChoice::Atr],
                take_profit_model: vec![TakeProfitModelChoice::Percent, TakeProfitModelChoice::Atr],
                atr_period: vec![7, 14, 21, 28],
                atr_spacing_multiplier_bps: vec![5000, 8000, 12000, 16000, 20000, 30000],
                atr_tp_multiplier_bps: vec![5000, 8000, 12000, 16000, 20000, 30000],
                adx_filter_enabled: vec![false],
                adx_threshold_bps: vec![1500, 2000],
                adx_period: vec![14],
            },
            _ => Self {
                leverage: vec![2, 3, 4, 5, 6, 7, 8, 10],
                spacing_bps: vec![80, 120, 160, 220, 300, 420, 600],
                order_multiplier: vec![1.25, 1.4, 1.6, 2.0, 2.4],
                max_legs: vec![4, 5, 6, 7, 8],
                take_profit_bps: vec![80, 100, 130, 180, 220, 280],
                tail_stop_bps: vec![1800, 2200, 2600, 3500],
                long_short_weight_pct: vec![(80, 20), (70, 30), (60, 40), (50, 50), (40, 60)],
                spacing_model: vec![SpacingModelChoice::FixedPercent, SpacingModelChoice::Atr],
                take_profit_model: vec![TakeProfitModelChoice::Percent, TakeProfitModelChoice::Atr],
                atr_period: vec![7, 14, 21, 28],
                atr_spacing_multiplier_bps: vec![8000, 12000, 16000, 20000],
                atr_tp_multiplier_bps: vec![8000, 12000, 16000, 20000],
                adx_filter_enabled: vec![true, false],
                adx_threshold_bps: vec![1500, 2000, 2500, 3000],
                adx_period: vec![14, 21],
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

    pub fn profit_optimized_v2(risk_profile: &str, direction_mode: &str) -> Self {
        let mut space = Self::for_profile(risk_profile, direction_mode);
        space.leverage = vec![2, 3, 4, 5, 6, 8, 10, 12, 15, 20];
        space.spacing_bps = vec![25, 35, 50, 70, 90, 120, 160, 220, 300, 420, 600, 800];
        space.order_multiplier = vec![1.1, 1.15, 1.25, 1.4, 1.6, 1.8, 2.0, 2.2, 2.4, 2.8];
        space.max_legs = vec![3, 4, 5, 6, 7, 8, 9, 10];
        space.take_profit_bps = vec![25, 30, 45, 60, 80, 100, 140, 200, 300, 450];
        if direction_mode == "long_short" {
            space.long_short_weight_pct = vec![
                (90, 10),
                (80, 20),
                (70, 30),
                (60, 40),
                (50, 50),
                (40, 60),
                (30, 70),
                (20, 80),
                (10, 90),
            ];
        }
        space.tail_stop_bps = vec![600, 800, 1200, 1800, 2400, 3000, 4000, 5500, 7000, 9000];
        space
    }
}

pub fn fine_space_around(winner: &CoarseParameterPoint) -> StagedMartingaleSearchSpace {
    let spacing_neighbors = [
        winner.spacing_bps.saturating_sub(20),
        winner.spacing_bps,
        winner.spacing_bps + 30,
    ];
    let max_legs_neighbors = [
        winner.max_legs.saturating_sub(1),
        winner.max_legs,
        winner.max_legs + 1,
    ];
    let tp_neighbors = [
        winner.take_profit_bps.saturating_sub(20),
        winner.take_profit_bps,
        winner.take_profit_bps + 30,
    ];
    let tail_neighbors = [
        winner.tail_stop_bps.saturating_sub(200),
        winner.tail_stop_bps,
        winner.tail_stop_bps + 200,
    ];
    let long_w = winner.long_weight_pct;
    let short_w = winner.short_weight_pct;
    let weight_neighbors = [
        (long_w.saturating_sub(5), short_w + 5),
        (long_w, short_w),
        (long_w + 5, short_w.saturating_sub(5)),
    ];

    StagedMartingaleSearchSpace {
        leverage: vec![
            winner.leverage.saturating_sub(1).max(1),
            winner.leverage,
            winner.leverage + 1,
        ],
        spacing_bps: spacing_neighbors.to_vec(),
        order_multiplier: vec![
            winner.order_multiplier - 0.2,
            winner.order_multiplier,
            winner.order_multiplier + 0.2,
        ],
        max_legs: max_legs_neighbors.to_vec(),
        take_profit_bps: tp_neighbors.to_vec(),
        tail_stop_bps: tail_neighbors.to_vec(),
        long_short_weight_pct: weight_neighbors.to_vec(),
        spacing_model: vec![winner.spacing_model],
        take_profit_model: vec![winner.take_profit_model],
        atr_period: vec![winner.atr_period],
        atr_spacing_multiplier_bps: vec![
            winner
                .atr_spacing_multiplier_bps
                .saturating_sub(2000)
                .max(5000),
            winner.atr_spacing_multiplier_bps,
            winner.atr_spacing_multiplier_bps + 2000,
        ],
        atr_tp_multiplier_bps: vec![
            winner.atr_tp_multiplier_bps.saturating_sub(2000).max(5000),
            winner.atr_tp_multiplier_bps,
            winner.atr_tp_multiplier_bps + 2000,
        ],
        adx_filter_enabled: vec![winner.adx_filter_enabled, !winner.adx_filter_enabled],
        adx_threshold_bps: vec![
            winner.adx_threshold_bps.saturating_sub(500).max(1000),
            winner.adx_threshold_bps,
            winner.adx_threshold_bps + 500,
        ],
        adx_period: vec![winner.adx_period],
    }
}

pub fn generate_staged_candidates_for_symbol(
    symbol: &str,
    direction: &str,
    space: &StagedMartingaleSearchSpace,
    limit: usize,
) -> Result<Vec<SearchCandidate>, String> {
    let mut candidates = Vec::new();
    let mut id_counter = 0usize;

    if direction == "long_short" || direction == "long_and_short" {
        let mut rng = rand::thread_rng();
        let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();

        while candidates.len() < limit {
            let li = rng.gen_range(0..space.leverage.len());
            let lsi = rng.gen_range(0..space.spacing_bps.len());
            let mi = rng.gen_range(0..space.order_multiplier.len());
            let mli = rng.gen_range(0..space.max_legs.len());
            let ltpi = rng.gen_range(0..space.take_profit_bps.len());
            let tsi = rng.gen_range(0..space.tail_stop_bps.len());
            let wi = rng.gen_range(0..space.long_short_weight_pct.len());
            let smi = rng.gen_range(0..space.order_multiplier.len());
            let smli = rng.gen_range(0..space.max_legs.len());
            let ssi = rng.gen_range(0..space.spacing_bps.len());
            let stpi = rng.gen_range(0..space.take_profit_bps.len());
            let stsi = rng.gen_range(0..space.tail_stop_bps.len());
            let spmi = rng.gen_range(0..space.spacing_model.len());
            let tpmi = rng.gen_range(0..space.take_profit_model.len());
            let api = rng.gen_range(0..space.atr_period.len());
            let asmi = rng.gen_range(0..space.atr_spacing_multiplier_bps.len());
            let atpmi = rng.gen_range(0..space.atr_tp_multiplier_bps.len());
            let afei = rng.gen_range(0..space.adx_filter_enabled.len());
            let athi = rng.gen_range(0..space.adx_threshold_bps.len());

            let key = fold_indices(&[
                li, lsi, mi, mli, ltpi, tsi, wi, smi, smli, ssi, stpi, stsi, spmi, tpmi, api, asmi,
                atpmi, afei, athi,
            ]);
            if !seen.insert(key) {
                if seen.len()
                    >= space.leverage.len()
                        * space.spacing_bps.len()
                        * space.order_multiplier.len()
                        * space.max_legs.len()
                        * space.take_profit_bps.len()
                        * space.tail_stop_bps.len()
                        * space.long_short_weight_pct.len()
                        * space.order_multiplier.len()
                        * space.max_legs.len()
                        * space.spacing_bps.len()
                        * space.take_profit_bps.len()
                        * space.tail_stop_bps.len()
                        * space.spacing_model.len()
                        * space.take_profit_model.len()
                        * space.atr_period.len()
                        * space.atr_spacing_multiplier_bps.len()
                        * space.atr_tp_multiplier_bps.len()
                        * space.adx_filter_enabled.len()
                        * space.adx_threshold_bps.len()
                {
                    break;
                }
                continue;
            }

            let leverage = space.leverage[li];
            let long_spacing_bps = space.spacing_bps[lsi];
            let multiplier = space.order_multiplier[mi];
            let max_legs = space.max_legs[mli];
            let long_take_profit_bps = space.take_profit_bps[ltpi];
            let tail_stop_bps = space.tail_stop_bps[tsi];
            let (long_weight_pct, short_weight_pct) = space.long_short_weight_pct[wi];
            let short_multiplier = space.order_multiplier[smi];
            let short_max_legs = space.max_legs[smli];
            let short_spacing_bps = space.spacing_bps[ssi];
            let short_take_profit_bps = space.take_profit_bps[stpi];
            let short_tail_stop_bps = space.tail_stop_bps[stsi];
            let spacing_model = space.spacing_model[spmi];
            let take_profit_model = space.take_profit_model[tpmi];
            let atr_period = space.atr_period[api];
            let atr_spacing_multiplier_bps = space.atr_spacing_multiplier_bps[asmi];
            let atr_tp_multiplier_bps = space.atr_tp_multiplier_bps[atpmi];
            let adx_filter_enabled = space.adx_filter_enabled[afei];
            let adx_threshold_bps = space.adx_threshold_bps[athi];
            let adx_period = *space.adx_period.first().unwrap_or(&14);

            let indicator_context = IndicatorSamplingContext {
                spacing_model,
                take_profit_model,
                atr_period,
                atr_spacing_multiplier_bps,
                atr_tp_multiplier_bps,
                adx_filter_enabled,
                adx_threshold_bps,
                adx_period,
            };

            if !is_valid_spacing_for_model(
                spacing_model,
                MartingaleDirection::Long,
                long_spacing_bps,
                max_legs,
                atr_spacing_multiplier_bps,
            ) || !is_valid_spacing_for_model(
                spacing_model,
                MartingaleDirection::Short,
                short_spacing_bps,
                short_max_legs,
                atr_spacing_multiplier_bps,
            ) {
                continue;
            }

            let long_params = LegParameters {
                spacing_bps: long_spacing_bps,
                order_multiplier: multiplier,
                max_legs,
                take_profit_bps: long_take_profit_bps,
                tail_stop_bps,
                weight_pct: long_weight_pct,
                spacing_model,
                take_profit_model,
                atr_period,
                atr_spacing_multiplier_bps,
                atr_tp_multiplier_bps,
                adx_filter_enabled,
                adx_threshold_bps,
                adx_period,
            };
            let short_params = LegParameters {
                spacing_bps: short_spacing_bps,
                order_multiplier: short_multiplier,
                max_legs: short_max_legs,
                take_profit_bps: short_take_profit_bps,
                tail_stop_bps: short_tail_stop_bps,
                weight_pct: short_weight_pct,
                spacing_model,
                take_profit_model,
                atr_period,
                atr_spacing_multiplier_bps,
                atr_tp_multiplier_bps,
                adx_filter_enabled,
                adx_threshold_bps,
                adx_period,
            };
            candidates.push(build_long_short_candidate_from_legs(
                symbol,
                leverage,
                long_params,
                short_params,
                &indicator_context,
                &mut id_counter,
            )?);
        }
        return Ok(candidates);
    }

    let single_direction = match direction {
        "long" | "long_only" => MartingaleDirection::Long,
        "short" | "short_only" => MartingaleDirection::Short,
        other => return Err(format!("unsupported direction: {other}")),
    };
    let mut rng = StdRng::seed_from_u64(staged_candidate_seed(symbol, direction, limit));
    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let total_combinations = space.leverage.len()
        * space.spacing_bps.len()
        * space.order_multiplier.len()
        * space.max_legs.len()
        * space.take_profit_bps.len()
        * space.tail_stop_bps.len()
        * space.spacing_model.len()
        * space.take_profit_model.len()
        * space.atr_period.len()
        * space.atr_spacing_multiplier_bps.len()
        * space.atr_tp_multiplier_bps.len()
        * space.adx_filter_enabled.len()
        * space.adx_threshold_bps.len();
    while candidates.len() < limit && seen.len() < total_combinations {
        let li = rng.gen_range(0..space.leverage.len());
        let si = rng.gen_range(0..space.spacing_bps.len());
        let mi = rng.gen_range(0..space.order_multiplier.len());
        let mli = rng.gen_range(0..space.max_legs.len());
        let tpi = rng.gen_range(0..space.take_profit_bps.len());
        let tsi = rng.gen_range(0..space.tail_stop_bps.len());
        let spmi = rng.gen_range(0..space.spacing_model.len());
        let tpmi = rng.gen_range(0..space.take_profit_model.len());
        let api = rng.gen_range(0..space.atr_period.len());
        let asmi = rng.gen_range(0..space.atr_spacing_multiplier_bps.len());
        let atpmi = rng.gen_range(0..space.atr_tp_multiplier_bps.len());
        let afei = rng.gen_range(0..space.adx_filter_enabled.len());
        let athi = rng.gen_range(0..space.adx_threshold_bps.len());
        if !seen.insert(fold_indices(&[
            li, si, mi, mli, tpi, tsi, spmi, tpmi, api, asmi, atpmi, afei, athi,
        ])) {
            continue;
        }
        let spacing_bps = space.spacing_bps[si];
        let max_legs = space.max_legs[mli];
        let spacing_model = space.spacing_model[spmi];
        let atr_spacing_multiplier_bps = space.atr_spacing_multiplier_bps[asmi];

        if !is_valid_spacing_for_model(
            spacing_model,
            single_direction,
            spacing_bps,
            max_legs,
            atr_spacing_multiplier_bps,
        ) {
            continue;
        }
        candidates.push(build_single_direction_candidate(
            symbol,
            single_direction,
            space.leverage[li],
            spacing_bps,
            space.order_multiplier[mi],
            max_legs,
            space.take_profit_bps[tpi],
            space.tail_stop_bps[tsi],
            100,
            &IndicatorSamplingContext {
                spacing_model,
                take_profit_model: space.take_profit_model[tpmi],
                atr_period: space.atr_period[api],
                atr_spacing_multiplier_bps,
                atr_tp_multiplier_bps: space.atr_tp_multiplier_bps[atpmi],
                adx_filter_enabled: space.adx_filter_enabled[afei],
                adx_threshold_bps: space.adx_threshold_bps[athi],
                adx_period: space.adx_period[0],
            },
            &mut id_counter,
        )?);
    }
    Ok(candidates)
}

fn staged_candidate_seed(symbol: &str, direction: &str, limit: usize) -> u64 {
    let mut seed = 0xA11C_E5E5_D15C_0DE5_u64 ^ limit as u64;
    for byte in symbol.bytes().chain(direction.bytes()) {
        seed = seed
            .wrapping_mul(1_099_511_628_211)
            .wrapping_add(byte as u64);
    }
    seed
}

fn fold_indices(indices: &[usize]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for &i in indices {
        std::hash::Hash::hash(&i, &mut hasher);
    }
    std::hash::Hasher::finish(&hasher)
}

fn is_valid_fixed_percent_spacing(
    direction: MartingaleDirection,
    spacing_bps: u32,
    max_legs: u32,
) -> bool {
    let max_distance_bps = spacing_bps.saturating_mul(max_legs);
    match direction {
        MartingaleDirection::Long => max_distance_bps < 9_500,
        MartingaleDirection::Short => max_distance_bps <= 30_000,
    }
}

fn is_valid_spacing_for_model(
    spacing_model: SpacingModelChoice,
    direction: MartingaleDirection,
    spacing_bps: u32,
    max_legs: u32,
    atr_spacing_multiplier_bps: u32,
) -> bool {
    match spacing_model {
        SpacingModelChoice::FixedPercent => {
            is_valid_fixed_percent_spacing(direction, spacing_bps, max_legs)
        }
        SpacingModelChoice::Atr => {
            atr_spacing_multiplier_bps > 0 && atr_spacing_multiplier_bps <= 40_000
        }
    }
}

struct IndicatorSamplingContext {
    spacing_model: SpacingModelChoice,
    take_profit_model: TakeProfitModelChoice,
    atr_period: u32,
    atr_spacing_multiplier_bps: u32,
    atr_tp_multiplier_bps: u32,
    adx_filter_enabled: bool,
    adx_threshold_bps: u32,
    adx_period: u32,
}

fn build_indicators_and_triggers(
    ctx: &IndicatorSamplingContext,
) -> (Vec<MartingaleIndicatorConfig>, Vec<MartingaleEntryTrigger>) {
    let mut indicators = Vec::new();
    let mut triggers = vec![MartingaleEntryTrigger::Cooldown { seconds: 21_600 }];

    indicators.push(MartingaleIndicatorConfig::Atr {
        period: ctx.atr_period.max(2),
    });

    if ctx.adx_filter_enabled {
        indicators.push(MartingaleIndicatorConfig::Adx {
            period: ctx.adx_period.max(2),
        });
        let threshold = ctx.adx_threshold_bps as f64 / 100.0;
        // 方向1: 低 ADX（threshold < 15%）= 震荡均值回归入场（adx < threshold）
        //        高 ADX（threshold >= 15%）= 趋势入场（adx > threshold，原行为）
        // 马丁/网格策略在震荡市盈利最优 → 低 ADX 入场可能大幅提升 ann
        let operator = if threshold < 15.0 { "<" } else { ">" };
        triggers.push(MartingaleEntryTrigger::IndicatorExpression {
            expression: format!("adx({}) {} {}", ctx.adx_period.max(2), operator, threshold),
        });
    }

    (indicators, triggers)
}

fn build_spacing_model(ctx: &IndicatorSamplingContext, spacing_bps: u32) -> MartingaleSpacingModel {
    match ctx.spacing_model {
        SpacingModelChoice::FixedPercent => MartingaleSpacingModel::FixedPercent {
            step_bps: spacing_bps,
        },
        SpacingModelChoice::Atr => MartingaleSpacingModel::Atr {
            multiplier: rust_decimal::Decimal::from(ctx.atr_spacing_multiplier_bps)
                / rust_decimal::Decimal::from(10_000u32),
            min_step_bps: 0,
            max_step_bps: 30_000,
        },
    }
}

fn build_take_profit_model(
    ctx: &IndicatorSamplingContext,
    take_profit_bps: u32,
) -> MartingaleTakeProfitModel {
    match ctx.take_profit_model {
        TakeProfitModelChoice::Percent => MartingaleTakeProfitModel::Percent {
            bps: take_profit_bps,
        },
        TakeProfitModelChoice::Atr => {
            let multiplier = rust_decimal::Decimal::from(ctx.atr_tp_multiplier_bps)
                / rust_decimal::Decimal::from(10000u32);
            MartingaleTakeProfitModel::Atr { multiplier }
        }
    }
}

fn build_single_direction_candidate(
    symbol: &str,
    direction: MartingaleDirection,
    leverage: u32,
    spacing_bps: u32,
    multiplier: f64,
    max_legs: u32,
    take_profit_bps: u32,
    tail_stop_bps: u32,
    _weight_pct: u32,
    ctx: &IndicatorSamplingContext,
    id_counter: &mut usize,
) -> Result<SearchCandidate, String> {
    let direction_mode = match direction {
        MartingaleDirection::Long => MartingaleDirectionMode::LongOnly,
        MartingaleDirection::Short => MartingaleDirectionMode::ShortOnly,
    };
    let market = if leverage > 1 {
        MartingaleMarketKind::UsdMFutures
    } else {
        MartingaleMarketKind::Spot
    };
    let (margin_mode, leverage_val) = match market {
        MartingaleMarketKind::Spot => (None, None),
        MartingaleMarketKind::UsdMFutures => (Some(MartingaleMarginMode::Isolated), Some(leverage)),
    };

    let multiplier_decimal = Decimal::from_f64_retain(multiplier).unwrap_or(Decimal::new(15, 1));
    let (indicators, entry_triggers) = build_indicators_and_triggers(ctx);
    let strategy = MartingaleStrategyConfig {
        strategy_id: format!("staged-{}", *id_counter),
        symbol: symbol.to_owned(),
        market,
        direction,
        direction_mode,
        margin_mode,
        leverage: leverage_val,
        spacing: build_spacing_model(ctx, spacing_bps),
        sizing: MartingaleSizingModel::Multiplier {
            first_order_quote: Decimal::new(100, 0),
            multiplier: multiplier_decimal,
            max_legs,
        },
        take_profit: build_take_profit_model(ctx, take_profit_bps),
        stop_loss: Some(
            shared_domain::martingale::MartingaleStopLossModel::StrategyDrawdownPct {
                pct_bps: tail_stop_bps,
            },
        ),
        indicators,
        entry_triggers,
        risk_limits: MartingaleRiskLimits::default(),
    };
    *id_counter += 1;
    let config = MartingalePortfolioConfig {
        direction_mode,
        strategies: vec![strategy],
        risk_limits: MartingaleRiskLimits::default(),
    };
    config.validate()?;
    Ok(SearchCandidate {
        candidate_id: format!("staged-cand-{}", *id_counter),
        config,
    })
}

// --- per-leg builder for asymmetric long_short candidates ---

fn build_long_short_candidate_from_legs(
    symbol: &str,
    leverage: u32,
    long_params: LegParameters,
    short_params: LegParameters,
    ctx: &IndicatorSamplingContext,
    id_counter: &mut usize,
) -> Result<SearchCandidate, String> {
    let market = if leverage > 1 {
        MartingaleMarketKind::UsdMFutures
    } else {
        MartingaleMarketKind::Spot
    };
    let (margin_mode, leverage_val) = match market {
        MartingaleMarketKind::Spot => (None, None),
        MartingaleMarketKind::UsdMFutures => (Some(MartingaleMarginMode::Isolated), Some(leverage)),
    };

    let long_strategy = strategy_from_leg_params(
        symbol,
        MartingaleDirection::Long,
        market,
        margin_mode,
        leverage_val,
        long_params,
        ctx,
        *id_counter,
    )?;
    let short_strategy = strategy_from_leg_params(
        symbol,
        MartingaleDirection::Short,
        market,
        margin_mode,
        leverage_val,
        short_params,
        ctx,
        *id_counter,
    )?;

    *id_counter += 1;
    let config = MartingalePortfolioConfig {
        direction_mode: MartingaleDirectionMode::LongAndShort,
        strategies: vec![long_strategy, short_strategy],
        risk_limits: MartingaleRiskLimits::default(),
    };
    config.validate()?;
    Ok(SearchCandidate {
        candidate_id: format!("staged-cand-{}", *id_counter),
        config,
    })
}

fn strategy_from_leg_params(
    symbol: &str,
    direction: MartingaleDirection,
    market: MartingaleMarketKind,
    margin_mode: Option<MartingaleMarginMode>,
    leverage: Option<u32>,
    params: LegParameters,
    ctx: &IndicatorSamplingContext,
    id_counter: usize,
) -> Result<MartingaleStrategyConfig, String> {
    let multiplier = Decimal::from_f64_retain(params.order_multiplier)
        .ok_or_else(|| format!("invalid multiplier {}", params.order_multiplier))?;
    let first_order_quote =
        Decimal::new(100, 0) * Decimal::from(params.weight_pct) / Decimal::from(100u32);
    let (indicators, entry_triggers) = build_indicators_and_triggers(ctx);
    Ok(MartingaleStrategyConfig {
        strategy_id: format!("staged-{id_counter}-{direction:?}"),
        symbol: symbol.to_owned(),
        market,
        direction,
        direction_mode: MartingaleDirectionMode::LongAndShort,
        margin_mode,
        leverage,
        spacing: build_spacing_model(ctx, params.spacing_bps),
        sizing: MartingaleSizingModel::Multiplier {
            first_order_quote,
            multiplier,
            max_legs: params.max_legs,
        },
        take_profit: build_take_profit_model(ctx, params.take_profit_bps),
        stop_loss: Some(
            shared_domain::martingale::MartingaleStopLossModel::StrategyDrawdownPct {
                pct_bps: params.tail_stop_bps,
            },
        ),
        indicators,
        entry_triggers,
        risk_limits: MartingaleRiskLimits::default(),
    })
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
    fn aggressive_search_space_covers_wide_profit_seeking_ranges() {
        let space = StagedMartingaleSearchSpace::for_profile("aggressive", "long_short");

        assert!(space.spacing_bps.contains(&35));
        assert!(space.spacing_bps.contains(&420));
        assert!(space.order_multiplier.contains(&1.2));
        assert!(space.order_multiplier.contains(&2.8));
        assert!(space.max_legs.contains(&3));
        assert!(space.max_legs.contains(&10));
        assert!(space.take_profit_bps.contains(&60));
        assert!(space.take_profit_bps.contains(&450));
        assert!(space.tail_stop_bps.contains(&1800));
        assert!(space.tail_stop_bps.contains(&6000));
        assert!(space.long_short_weight_pct.contains(&(80, 20)));
        assert!(space.long_short_weight_pct.contains(&(30, 70)));
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
            spacing_model: SpacingModelChoice::FixedPercent,
            take_profit_model: TakeProfitModelChoice::Percent,
            atr_period: 14,
            atr_spacing_multiplier_bps: 15000,
            atr_tp_multiplier_bps: 15000,
            adx_filter_enabled: false,
            adx_threshold_bps: 2500,
            adx_period: 14,
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

    #[test]
    fn aggressive_profit_search_v2_covers_wide_spacing_and_profit_targets() {
        let space = StagedMartingaleSearchSpace::profit_optimized_v2("aggressive", "long_short");
        assert!(space.leverage.contains(&10));
        assert!(space.spacing_bps.iter().any(|v| *v <= 35));
        assert!(space.spacing_bps.iter().any(|v| *v >= 600));
        assert!(space.take_profit_bps.iter().any(|v| *v <= 30));
        assert!(space.take_profit_bps.iter().any(|v| *v >= 300));
        assert!(space.max_legs.contains(&9));
        assert!(space.order_multiplier.iter().any(|v| *v <= 1.15));
        assert!(space.order_multiplier.iter().any(|v| *v >= 2.4));
    }

    #[test]
    fn single_direction_staged_candidates_sample_tail_parameters() {
        let space = StagedMartingaleSearchSpace::profit_optimized_v2("balanced", "long_only");

        let candidates = generate_staged_candidates_for_symbol("BTCUSDT", "long_only", &space, 96)
            .expect("single-direction candidates should generate");

        assert!(
            candidates.iter().any(|candidate| candidate
                .config
                .strategies
                .iter()
                .any(|strategy| strategy.leverage.unwrap_or(1) >= 10)),
            "single-direction search must not only emit low-leverage prefix candidates"
        );
        assert!(
            candidates.iter().any(
                |candidate| candidate.config.strategies.iter().any(|strategy| {
                    match strategy.spacing {
                        MartingaleSpacingModel::FixedPercent { step_bps } => step_bps >= 420,
                        _ => false,
                    }
                })
            ),
            "single-direction search must cover wide spacing tail candidates"
        );
        assert!(
            candidates.iter().any(
                |candidate| candidate.config.strategies.iter().any(|strategy| {
                    match strategy.take_profit {
                        MartingaleTakeProfitModel::Percent { bps } => bps >= 200,
                        _ => false,
                    }
                })
            ),
            "single-direction search must cover higher take-profit tail candidates"
        );
    }

    #[test]
    fn long_short_staged_candidates_include_asymmetric_leg_parameters() {
        let space = StagedMartingaleSearchSpace {
            leverage: vec![2],
            spacing_bps: vec![120, 240],
            order_multiplier: vec![1.10, 1.25],
            max_legs: vec![2, 3],
            take_profit_bps: vec![60, 120],
            tail_stop_bps: vec![2000, 3000],
            long_short_weight_pct: vec![(60, 40), (50, 50)],
            spacing_model: vec![SpacingModelChoice::FixedPercent],
            take_profit_model: vec![TakeProfitModelChoice::Percent],
            atr_period: vec![14],
            atr_spacing_multiplier_bps: vec![15000],
            atr_tp_multiplier_bps: vec![15000],
            adx_filter_enabled: vec![false],
            adx_threshold_bps: vec![2500],
            adx_period: vec![14],
        };

        let candidates =
            generate_staged_candidates_for_symbol("BTCUSDT", "long_short", &space, 256)
                .expect("long_short candidates should generate");

        assert!(candidates.iter().all(|candidate| {
            candidate.config.direction_mode == MartingaleDirectionMode::LongAndShort
                && candidate.config.strategies.len() == 2
                && candidate
                    .config
                    .strategies
                    .iter()
                    .any(|s| s.direction == MartingaleDirection::Long)
                && candidate
                    .config
                    .strategies
                    .iter()
                    .any(|s| s.direction == MartingaleDirection::Short)
        }));

        let has_asymmetric_spacing = candidates.iter().any(|candidate| {
            let long = candidate
                .config
                .strategies
                .iter()
                .find(|s| s.direction == MartingaleDirection::Long)
                .unwrap();
            let short = candidate
                .config
                .strategies
                .iter()
                .find(|s| s.direction == MartingaleDirection::Short)
                .unwrap();
            match (&long.spacing, &short.spacing) {
                (
                    MartingaleSpacingModel::FixedPercent {
                        step_bps: long_step,
                    },
                    MartingaleSpacingModel::FixedPercent {
                        step_bps: short_step,
                    },
                ) => long_step != short_step,
                _ => false,
            }
        });
        assert!(
            has_asymmetric_spacing,
            "long_short search must include different long/short spacing combinations"
        );

        let has_asymmetric_tp = candidates.iter().any(|candidate| {
            let long = candidate
                .config
                .strategies
                .iter()
                .find(|s| s.direction == MartingaleDirection::Long)
                .unwrap();
            let short = candidate
                .config
                .strategies
                .iter()
                .find(|s| s.direction == MartingaleDirection::Short)
                .unwrap();
            match (&long.take_profit, &short.take_profit) {
                (
                    MartingaleTakeProfitModel::Percent { bps: long_tp },
                    MartingaleTakeProfitModel::Percent { bps: short_tp },
                ) => long_tp != short_tp,
                _ => false,
            }
        });
        assert!(
            has_asymmetric_tp,
            "long_short search must include different long/short take-profit combinations"
        );

        let has_asymmetric_multiplier_or_depth = candidates.iter().any(|candidate| {
            let long = candidate
                .config
                .strategies
                .iter()
                .find(|s| s.direction == MartingaleDirection::Long)
                .unwrap();
            let short = candidate
                .config
                .strategies
                .iter()
                .find(|s| s.direction == MartingaleDirection::Short)
                .unwrap();
            match (&long.sizing, &short.sizing) {
                (
                    MartingaleSizingModel::Multiplier {
                        multiplier: long_multiplier,
                        max_legs: long_max_legs,
                        ..
                    },
                    MartingaleSizingModel::Multiplier {
                        multiplier: short_multiplier,
                        max_legs: short_max_legs,
                        ..
                    },
                ) => long_multiplier != short_multiplier || long_max_legs != short_max_legs,
                _ => false,
            }
        });
        assert!(
            has_asymmetric_multiplier_or_depth,
            "long_short search must include different long/short multiplier or depth combinations"
        );
    }

    #[test]
    fn atr_spacing_generates_candidates_with_atr_spacing_model() {
        let space = StagedMartingaleSearchSpace {
            leverage: vec![4],
            spacing_bps: vec![120],
            order_multiplier: vec![1.5],
            max_legs: vec![4],
            take_profit_bps: vec![80],
            tail_stop_bps: vec![2000],
            long_short_weight_pct: vec![(100, 0)],
            spacing_model: vec![SpacingModelChoice::Atr],
            take_profit_model: vec![TakeProfitModelChoice::Percent],
            atr_period: vec![14],
            atr_spacing_multiplier_bps: vec![15000],
            atr_tp_multiplier_bps: vec![15000],
            adx_filter_enabled: vec![false],
            adx_threshold_bps: vec![2500],
            adx_period: vec![14],
        };

        let candidates = generate_staged_candidates_for_symbol("BTCUSDT", "long_only", &space, 10)
            .expect("candidates should generate with ATR spacing");
        assert!(!candidates.is_empty());

        let has_atr = candidates.iter().any(|c| {
            matches!(
                c.config.strategies[0].spacing,
                MartingaleSpacingModel::Atr { .. }
            )
        });
        assert!(has_atr, "ATR spacing candidates must be generated");
    }

    #[test]
    fn atr_take_profit_candidate_generates_atr_tp_model() {
        let space = StagedMartingaleSearchSpace {
            leverage: vec![4],
            spacing_bps: vec![120],
            order_multiplier: vec![1.5],
            max_legs: vec![4],
            take_profit_bps: vec![80],
            tail_stop_bps: vec![2000],
            long_short_weight_pct: vec![(100, 0)],
            spacing_model: vec![SpacingModelChoice::FixedPercent],
            take_profit_model: vec![TakeProfitModelChoice::Atr],
            atr_period: vec![14],
            atr_spacing_multiplier_bps: vec![15000],
            atr_tp_multiplier_bps: vec![20000],
            adx_filter_enabled: vec![false],
            adx_threshold_bps: vec![2500],
            adx_period: vec![14],
        };

        let candidates = generate_staged_candidates_for_symbol("BTCUSDT", "long_only", &space, 10)
            .expect("candidates should generate");

        assert!(!candidates.is_empty());
        let strategy = &candidates[0].config.strategies[0];
        assert!(
            matches!(strategy.take_profit, MartingaleTakeProfitModel::Atr { .. }),
            "expected Atr take profit model, got {:?}",
            strategy.take_profit
        );
    }

    #[test]
    fn adx_filter_candidate_has_indicator_expression_trigger() {
        let space = StagedMartingaleSearchSpace {
            leverage: vec![4],
            spacing_bps: vec![120],
            order_multiplier: vec![1.5],
            max_legs: vec![4],
            take_profit_bps: vec![80],
            tail_stop_bps: vec![2000],
            long_short_weight_pct: vec![(100, 0)],
            spacing_model: vec![SpacingModelChoice::FixedPercent],
            take_profit_model: vec![TakeProfitModelChoice::Percent],
            atr_period: vec![14],
            atr_spacing_multiplier_bps: vec![15000],
            atr_tp_multiplier_bps: vec![15000],
            adx_filter_enabled: vec![true],
            adx_threshold_bps: vec![2500],
            adx_period: vec![14],
        };

        let candidates = generate_staged_candidates_for_symbol("BTCUSDT", "long_only", &space, 10)
            .expect("candidates should generate");

        assert!(!candidates.is_empty());
        let strategy = &candidates[0].config.strategies[0];
        assert!(
            strategy
                .entry_triggers
                .iter()
                .any(|t| matches!(t, MartingaleEntryTrigger::IndicatorExpression { .. })),
            "expected IndicatorExpression trigger in entry_triggers"
        );
        assert!(
            strategy
                .indicators
                .iter()
                .any(|i| matches!(i, MartingaleIndicatorConfig::Adx { .. })),
            "expected Adx indicator in indicators list"
        );
    }

    #[test]
    fn atr_spacing_enabled_both_fixed_percent_and_atr_candidates_appear() {
        let space = StagedMartingaleSearchSpace {
            leverage: vec![4],
            spacing_bps: vec![120],
            order_multiplier: vec![1.5],
            max_legs: vec![4],
            take_profit_bps: vec![80],
            tail_stop_bps: vec![2000],
            long_short_weight_pct: vec![(100, 0)],
            spacing_model: vec![SpacingModelChoice::FixedPercent, SpacingModelChoice::Atr],
            take_profit_model: vec![TakeProfitModelChoice::Percent, TakeProfitModelChoice::Atr],
            atr_period: vec![14],
            atr_spacing_multiplier_bps: vec![15000],
            atr_tp_multiplier_bps: vec![15000],
            adx_filter_enabled: vec![false],
            adx_threshold_bps: vec![2500],
            adx_period: vec![14],
        };

        let candidates = generate_staged_candidates_for_symbol("BTCUSDT", "long_only", &space, 64)
            .expect("candidates should generate");

        let has_fixed = candidates.iter().any(|c| {
            matches!(
                c.config.strategies[0].spacing,
                MartingaleSpacingModel::FixedPercent { .. }
            )
        });
        let has_atr = candidates.iter().any(|c| {
            matches!(
                c.config.strategies[0].spacing,
                MartingaleSpacingModel::Atr { .. }
            )
        });
        assert!(has_fixed, "must include FixedPercent spacing candidates");
        assert!(has_atr, "must include ATR spacing candidates");

        let has_pct_tp = candidates.iter().any(|c| {
            matches!(
                c.config.strategies[0].take_profit,
                MartingaleTakeProfitModel::Percent { .. }
            )
        });
        let has_atr_tp = candidates.iter().any(|c| {
            matches!(
                c.config.strategies[0].take_profit,
                MartingaleTakeProfitModel::Atr { .. }
            )
        });
        assert!(has_pct_tp, "must include Percent take profit candidates");
        assert!(has_atr_tp, "must include Atr take profit candidates");
    }
}
