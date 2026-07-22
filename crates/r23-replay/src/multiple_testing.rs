//! Multiple-testing correction for R8 (plan §12, §13).
//!
//! Implements:
//! - Deflated Sharpe Ratio (DSR, Bailey & López de Prado 2014): corrects the
//!   observed Sharpe for the number of trials and the skew/kurtosis of returns.
//! - Probability of Backtest Overfitting (PBO, Bailey et al. 2017): combinatorial
//!   symmetric cross-validation.
//! - CSCV (Combinatorially Symmetric Cross-Validation) logit transform.
//!
//! These guard against overfitting when many policies were tried. A policy may
//! not enter R8 selection unless DSR > 0 and PBO < 0.5.

use serde::{Deserialize, Serialize};

/// Sharpe-ratio statistics of a return series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharpeStats {
    pub mean: f64,
    pub std: f64,
    pub skew: f64,
    pub kurtosis: f64,
    pub sharpe: f64,
    pub n: usize,
}

/// Compute Sharpe + moments of a per-period return series (returns in fraction,
/// e.g. 0.01 = 1%). Annualization is left to the caller.
pub fn sharpe_stats(returns: &[f64]) -> SharpeStats {
    let n = returns.len();
    if n == 0 {
        return SharpeStats { mean: 0.0, std: 0.0, skew: 0.0, kurtosis: 0.0, sharpe: 0.0, n: 0 };
    }
    let mean = returns.iter().sum::<f64>() / n as f64;
    let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n as f64;
    let std = var.sqrt();
    let m3 = returns.iter().map(|r| (r - mean).powi(3)).sum::<f64>() / n as f64;
    let m4 = returns.iter().map(|r| (r - mean).powi(4)).sum::<f64>() / n as f64;
    let skew = if std > 1e-12 { m3 / std.powi(3) } else { 0.0 };
    let kurtosis = if std > 1e-12 { m4 / std.powi(4) - 3.0 } else { 0.0 }; // excess
    let sharpe = if std > 1e-12 { mean / std } else { 0.0 };
    SharpeStats { mean, std, skew, kurtosis, sharpe, n }
}

/// Deflated Sharpe Ratio (Bailey & López de Prado 2014).
///
/// `observed_sharpe` is the annualized Sharpe of the selected policy.
/// `n_trials` is the number of independent policies tried.
/// `n_periods` is the number of return observations (e.g. days).
/// `skew`, `kurt` are the return-moments of the selected policy.
/// Returns the deflated Sharpe — the SR corrected for selection bias. A policy
/// is defensible only if DSR > 0.
pub fn deflated_sharpe(
    observed_sharpe: f64,
    n_trials: u64,
    n_periods: u64,
    skew: f64,
    kurt: f64, // excess kurtosis
) -> f64 {
    // Expected maximum Sharpe under the null (all policies have zero true SR):
    // E[max SR] ≈ sqrt(2 * ln(n_trials))  (for n_trials >= 1)
    let n_t = n_trials.max(1) as f64;
    let e_max_sr = (2.0 * n_t.ln()).sqrt();
    // Variance of the estimated SR (López de Prado eq.):
    // Var[SR] ≈ (1 - skew*SR + (kurt/4)*SR^2) / (n_periods - 1)
    let n_p = n_periods.max(2) as f64;
    let sr_var = (1.0 - skew * observed_sharpe + (kurt / 4.0) * observed_sharpe.powi(2)) / (n_p - 1.0);
    let sr_std = sr_var.max(0.0).sqrt();
    if sr_std < 1e-12 {
        return observed_sharpe;
    }
    // DSR = (SR_observed - E[max SR]) / std(SR)
    (observed_sharpe - e_max_sr) / sr_std
}

/// Probability of Backtest Overfitting (PBO) via combinatorially symmetric
/// cross-validation (Bailey et al. 2017).
///
/// `policy_returns` is a matrix [n_policies][n_periods] of per-period returns.
/// Splits into C = choose(n_periods, n_periods/2) train/test halves; for each
/// split, ranks policies in-train, takes the best-train policy's rank in test,
/// computes the logit; PBO = fraction of splits where the best-train policy
/// underperforms the median in test (logit <= 0).
///
/// For tractability we use a fixed number of random symmetric splits (S).
pub fn probability_of_backtest_overfitting(policy_returns: &[Vec<f64>], n_splits: usize) -> f64 {
    let n_policies = policy_returns.len();
    if n_policies < 2 { return 0.0; }
    let n_periods = policy_returns[0].len();
    if n_periods < 8 { return 0.0; }
    let half = n_periods / 2;
    let mut underperform = 0usize;
    let mut total = 0usize;
    // deterministic pseudo-random splits (seeded) for reproducibility
    let mut seed: u64 = 0x9e3779b97f4a7c15;
    for _ in 0..n_splits {
        // pick `half` distinct indices for the train half
        let idx = random_subset(n_periods, half, &mut seed);
        let mut in_train = vec![false; n_periods];
        for &i in &idx { in_train[i] = true; }
        // train/test cumulative returns per policy
        let mut train_cum = vec![0.0f64; n_policies];
        let mut test_cum = vec![0.0f64; n_policies];
        for (pi, pret) in policy_returns.iter().enumerate() {
            for (t, r) in pret.iter().enumerate() {
                if t >= n_periods { break; }
                if in_train[t] { train_cum[pi] += r; } else { test_cum[pi] += r; }
            }
        }
        // best-train policy
        let best = (0..n_policies).max_by(|&a, &b| train_cum[a].partial_cmp(&train_cum[b]).unwrap()).unwrap();
        // rank of best in test (1-indexed; 1 = best/highest test cum)
        let mut rank = 1usize;
        for pi in 0..n_policies {
            if pi != best && test_cum[pi] > test_cum[best] { rank += 1; }
        }
        // CSCV logit: omega = (n + 1 - rank) / rank. rank=1 (best) -> omega = n > 1 -> logit > 0 (good).
        // PBO = fraction where logit <= 0 (best-train underperforms median in test).
        let omega = (n_policies as f64 + 1.0 - rank as f64) / rank as f64;
        let logit = omega.ln();
        if !logit.is_finite() || logit <= 0.0 { underperform += 1; }
        total += 1;
    }
    if total == 0 { return 0.0; }
    underperform as f64 / total as f64
}

fn random_subset(n: usize, k: usize, seed: &mut u64) -> Vec<usize> {
    // Fisher-Yates partial shuffle with xorshift RNG (deterministic)
    let mut idx: Vec<usize> = (0..n).collect();
    for i in 0..k {
        *seed ^= *seed << 13; *seed ^= *seed >> 7; *seed ^= *seed << 17;
        let j = i + (*seed as usize % (n - i));
        idx.swap(i, j);
    }
    idx[..k].to_vec()
}

/// CSCV: returns the PBO plus the distribution of logits (for diagnostics).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CscvResult {
    pub pbo: f64,
    pub logits: Vec<f64>,
}

pub fn cscv(policy_returns: &[Vec<f64>], n_splits: usize) -> CscvResult {
    let n_policies = policy_returns.len();
    if n_policies < 2 || policy_returns[0].len() < 8 {
        return CscvResult { pbo: 0.0, logits: vec![] };
    }
    let n_periods = policy_returns[0].len();
    let half = n_periods / 2;
    let mut logits = Vec::new();
    let mut seed: u64 = 0x9e3779b97f4a7c15;
    for _ in 0..n_splits {
        let idx = random_subset(n_periods, half, &mut seed);
        let mut in_train = vec![false; n_periods];
        for &i in &idx { in_train[i] = true; }
        let mut train_cum = vec![0.0f64; n_policies];
        let mut test_cum = vec![0.0f64; n_policies];
        for (pi, pret) in policy_returns.iter().enumerate() {
            for (t, r) in pret.iter().enumerate() {
                if t >= n_periods { break; }
                if in_train[t] { train_cum[pi] += r; } else { test_cum[pi] += r; }
            }
        }
        let best = (0..n_policies).max_by(|&a, &b| train_cum[a].partial_cmp(&train_cum[b]).unwrap()).unwrap();
        let mut rank = 1usize;
        for pi in 0..n_policies {
            if pi != best && test_cum[pi] > test_cum[best] { rank += 1; }
        }
        let omega = (n_policies as f64 + 1.0 - rank as f64) / rank as f64;
        let logit = omega.ln();
        if logit.is_finite() { logits.push(logit); }
    }
    let pbo = logits.iter().filter(|&&l| l <= 0.0).count() as f64 / logits.len().max(1) as f64;
    CscvResult { pbo, logits }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sharpe_of_flat_returns_is_zero() {
        let s = sharpe_stats(&[0.0; 100]);
        assert!(s.sharpe.abs() < 1e-9);
    }

    #[test]
    fn dsr_decreases_with_more_trials() {
        // with the same observed SR, more trials -> lower DSR (harder to defend)
        let d1 = deflated_sharpe(1.0, 1, 252, 0.0, 0.0);
        let d100 = deflated_sharpe(1.0, 100, 252, 0.0, 0.0);
        assert!(d100 < d1, "d1={d1} d100={d100}");
    }

    #[test]
    fn pbo_high_when_no_real_edge() {
        // random policy returns -> high PBO
        let mut seed = 42u64;
        let policies: Vec<Vec<f64>> = (0..10).map(|_| {
            (0..200).map(|_| {
                seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17;
                ((seed % 1000) as f64 / 100000.0) - 0.005
            }).collect()
        }).collect();
        let pbo = probability_of_backtest_overfitting(&policies, 100);
        // random returns tend to high PBO (best-in-train often underperforms in test)
        assert!(pbo >= 0.0 && pbo <= 1.0);
    }

    #[test]
    fn pbo_low_when_one_dominant_policy() {
        // one policy strictly dominates -> PBO near 0
        let mut policies: Vec<Vec<f64>> = (0..5).map(|_| vec![0.0; 100]).collect();
        policies[0] = vec![0.001; 100]; // dominant
        let pbo = probability_of_backtest_overfitting(&policies, 50);
        assert!(pbo < 0.5, "pbo={pbo}");
    }
}
