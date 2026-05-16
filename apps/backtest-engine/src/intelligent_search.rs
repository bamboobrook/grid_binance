use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleSizingModel, MartingaleSpacingModel, MartingaleTakeProfitModel,
};

use crate::martingale::metrics::MartingaleBacktestResult;
use crate::martingale::scoring::{score_candidate, CandidateScore, ScoringConfig};
use crate::search::{random_search, SearchCandidate, SearchSpace};

#[derive(Debug, Clone)]
pub struct IntelligentSearchConfig {
    pub seed: u64,
    pub random_round_size: usize,
    pub max_rounds: usize,
    pub max_candidates: usize,
    pub survivor_percentile: f64,
    pub timeout: Option<Duration>,
    pub scoring: ScoringConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedCandidate {
    pub candidate: SearchCandidate,
    pub score: CandidateScore,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntelligentSearchResult {
    pub candidates: Vec<EvaluatedCandidate>,
    pub rounds_completed: usize,
    pub stopped_reason: String,
}

impl Default for IntelligentSearchConfig {
    fn default() -> Self {
        Self {
            seed: 1,
            random_round_size: 32,
            max_rounds: 3,
            max_candidates: 128,
            survivor_percentile: 0.2,
            timeout: None,
            scoring: ScoringConfig::default(),
        }
    }
}

pub fn intelligent_search<F>(
    space: &SearchSpace,
    config: &IntelligentSearchConfig,
    cancel: Option<&AtomicBool>,
    mut evaluate: F,
) -> Result<IntelligentSearchResult, String>
where
    F: FnMut(&SearchCandidate) -> Result<MartingaleBacktestResult, String>,
{
    let started = Instant::now();
    let mut rng = StdRng::seed_from_u64(config.seed);
    let mut all = Vec::new();
    let mut current = random_search(space, config.random_round_size, config.seed)?;
    let mut stopped_reason = "max_rounds".to_string();
    let mut rounds_completed = 0;

    'rounds: for round in 0..config.max_rounds {
        if cancelled(cancel) {
            stopped_reason = "cancelled".to_string();
            break;
        }
        if timed_out(started, config.timeout) {
            stopped_reason = "timeout".to_string();
            break;
        }

        let remaining = config.max_candidates.saturating_sub(all.len());
        if remaining == 0 {
            stopped_reason = "max_candidates".to_string();
            break;
        }

        for candidate in current.iter().take(remaining) {
            if cancelled(cancel) {
                stopped_reason = "cancelled".to_string();
                break 'rounds;
            }
            if timed_out(started, config.timeout) {
                stopped_reason = "timeout".to_string();
                break 'rounds;
            }
            let result = evaluate(candidate)?;
            all.push(EvaluatedCandidate {
                candidate: candidate.clone(),
                score: score_candidate(&result, &config.scoring),
            });
            if cancelled(cancel) {
                stopped_reason = "cancelled".to_string();
                break 'rounds;
            }
            if timed_out(started, config.timeout) {
                stopped_reason = "timeout".to_string();
                break 'rounds;
            }
        }
        rounds_completed += 1;
        all.sort_by(|left, right| right.score.rank_score.total_cmp(&left.score.rank_score));

        if all.len() >= config.max_candidates {
            stopped_reason = "max_candidates".to_string();
            break;
        }
        if round + 1 == config.max_rounds {
            break;
        }

        let survivor_count = ((all.len() as f64 * config.survivor_percentile.clamp(0.01, 1.0))
            .ceil() as usize)
            .max(1);
        let survivors: Vec<SearchCandidate> = all
            .iter()
            .filter(|candidate| candidate.score.survival_valid)
            .take(survivor_count)
            .map(|candidate| candidate.candidate.clone())
            .collect();
        current = if survivors.is_empty() {
            random_search(space, config.random_round_size, rng.gen())?
        } else {
            mutate_candidates(&survivors, config.random_round_size, &mut rng)?
        };
    }

    all.sort_by(|left, right| right.score.rank_score.total_cmp(&left.score.rank_score));
    Ok(IntelligentSearchResult {
        candidates: all,
        rounds_completed,
        stopped_reason,
    })
}

fn mutate_candidates(
    winners: &[SearchCandidate],
    target_count: usize,
    rng: &mut StdRng,
) -> Result<Vec<SearchCandidate>, String> {
    let mut mutated = Vec::with_capacity(target_count);
    for index in 0..target_count {
        let mut candidate = winners[rng.gen_range(0..winners.len())].clone();
        candidate.candidate_id = format!("{}-mut-{index}", candidate.candidate_id);
        for strategy in &mut candidate.config.strategies {
            if let MartingaleSpacingModel::FixedPercent { step_bps } = &mut strategy.spacing {
                *step_bps = mutate_u32(*step_bps, 20, rng).max(1);
            }
            if let MartingaleSizingModel::Multiplier {
                first_order_quote,
                multiplier,
                ..
            } = &mut strategy.sizing
            {
                let value = decimal_to_f64(*first_order_quote)?;
                *first_order_quote =
                    Decimal::from_f64_retain((value * rng.gen_range(0.8..=1.2)).max(1.0))
                        .ok_or_else(|| "mutated first_order_quote is invalid".to_string())?;
                let multiplier_value = decimal_to_f64(*multiplier)?;
                *multiplier = Decimal::from_f64_retain(
                    (multiplier_value * rng.gen_range(0.9..=1.1)).max(1.0),
                )
                .ok_or_else(|| "mutated multiplier is invalid".to_string())?;
            }
            if let MartingaleTakeProfitModel::Percent { bps } = &mut strategy.take_profit {
                *bps = mutate_u32(*bps, 15, rng).max(1);
            }
            if let Some(leverage) = &mut strategy.leverage {
                *leverage = mutate_u32(*leverage, 1, rng).clamp(1, 125);
            }
        }
        candidate.config.validate()?;
        mutated.push(candidate);
    }
    Ok(mutated)
}

fn mutate_u32(value: u32, spread: u32, rng: &mut StdRng) -> u32 {
    let delta = rng.gen_range(-(spread as i32)..=(spread as i32));
    value.saturating_add_signed(delta)
}

fn decimal_to_f64(value: Decimal) -> Result<f64, String> {
    use rust_decimal::prelude::ToPrimitive;
    value
        .to_f64()
        .ok_or_else(|| "decimal cannot convert to f64".to_string())
}

fn cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|flag| flag.load(Ordering::Relaxed))
}

fn timed_out(started: Instant, timeout: Option<Duration>) -> bool {
    timeout.is_some_and(|timeout| started.elapsed() >= timeout)
}
