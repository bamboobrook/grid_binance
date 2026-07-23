use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::r25::{gaussian_conditional_h, inverse_normal_cdf, normal_cdf, student_t_conditional_h};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StationarityDiagnostics {
    pub adf_stat: f64,
    pub adf_p_value: f64,
    pub kpss_stat: f64,
    pub kpss_p_value: f64,
    pub half_life_bars: f64,
    pub cusum_max: f64,
    pub break_valid: bool,
    pub tail_sample_count: usize,
    pub cost_feasible: bool,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReferenceSpreadFit {
    pub alt: String,
    pub beta: f64,
    pub mean: f64,
    pub sigma: f64,
    pub aligned_spreads: Vec<(i64, f64)>,
    pub sorted_spreads: Vec<f64>,
    pub fit_cutoff_ms: i64,
    pub stationarity: StationarityDiagnostics,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CopulaFamily {
    Gaussian,
    StudentT,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopulaFit {
    pub family: CopulaFamily,
    pub rho: f64,
    pub nu: Option<u32>,
    pub log_likelihood: f64,
    pub aic: f64,
    pub gaussian_log_likelihood: f64,
    pub gaussian_aic: f64,
    pub student_t_log_likelihood: f64,
    pub student_t_aic: f64,
    pub sample_count: usize,
    pub kendall_tau: f64,
    pub fit_cutoff_ms: i64,
    pub independent_reference_error: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CorrectedPairModel {
    pub left: ReferenceSpreadFit,
    pub right: ReferenceSpreadFit,
    pub copula: CopulaFit,
    pub score: f64,
}

pub fn fit_reference_spread(
    alt: &str,
    observations: &[(i64, f64, f64)],
    fit_cutoff_ms: i64,
) -> Option<ReferenceSpreadFit> {
    let aligned = observations
        .iter()
        .filter(|(timestamp, _, _)| *timestamp <= fit_cutoff_ms)
        .filter(|(_, btc, alt_price)| *btc > 0.0 && *alt_price > 0.0)
        .map(|(timestamp, btc, alt_price)| (*timestamp, btc.ln(), alt_price.ln()))
        .collect::<Vec<_>>();
    if aligned.len() < 200 {
        return None;
    }
    let x_mean = aligned.iter().map(|(_, _, x)| *x).sum::<f64>() / aligned.len() as f64;
    let y_mean = aligned.iter().map(|(_, y, _)| *y).sum::<f64>() / aligned.len() as f64;
    let var_x = aligned
        .iter()
        .map(|(_, _, x)| (*x - x_mean).powi(2))
        .sum::<f64>();
    if var_x <= 1e-12 {
        return None;
    }
    let covariance = aligned
        .iter()
        .map(|(_, y, x)| (*x - x_mean) * (*y - y_mean))
        .sum::<f64>();
    let beta = (covariance / var_x).clamp(0.2, 5.0);
    let aligned_spreads = aligned
        .iter()
        .map(|(timestamp, btc, alt_price)| (*timestamp, *btc - beta * *alt_price))
        .collect::<Vec<_>>();
    let values = aligned_spreads
        .iter()
        .map(|(_, spread)| *spread)
        .collect::<Vec<_>>();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let sigma = sample_sigma(&values, mean);
    if sigma <= 1e-8 {
        return None;
    }
    let mut sorted_spreads = values.clone();
    sorted_spreads.sort_by(f64::total_cmp);
    let stationarity = stationarity_diagnostics(&values, sigma);
    Some(ReferenceSpreadFit {
        alt: alt.into(),
        beta,
        mean,
        sigma,
        aligned_spreads,
        sorted_spreads,
        fit_cutoff_ms,
        stationarity,
    })
}

pub fn fit_copula(left: &ReferenceSpreadFit, right: &ReferenceSpreadFit) -> Option<CopulaFit> {
    if !left.stationarity.passed || !right.stationarity.passed {
        return None;
    }
    let right_by_time = right
        .aligned_spreads
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();
    let aligned = left
        .aligned_spreads
        .iter()
        .filter_map(|(timestamp, left_spread)| {
            right_by_time
                .get(timestamp)
                .map(|right_spread| (*left_spread, *right_spread))
        })
        .collect::<Vec<_>>();
    if aligned.len() < 200 {
        return None;
    }
    let tau = kendall_tau_b(&aligned);
    let rho_start = (std::f64::consts::FRAC_PI_2 * tau).sin().clamp(-0.95, 0.95);
    let uniforms = aligned
        .iter()
        .map(|(l, r)| {
            (
                empirical_cdf(&left.sorted_spreads, *l),
                empirical_cdf(&right.sorted_spreads, *r),
            )
        })
        .collect::<Vec<_>>();
    let (gaussian_rho, gaussian_ll) = optimize_rho(rho_start, |rho| {
        gaussian_copula_log_likelihood(&uniforms, rho)
    });
    let gaussian_aic = 2.0 - 2.0 * gaussian_ll;
    let mut best_student = (3_u32, rho_start, f64::NEG_INFINITY, f64::INFINITY);
    for nu in 3..=30 {
        let (rho, ll) = optimize_rho(rho_start, |candidate| {
            student_t_copula_log_likelihood(&uniforms, candidate, nu as f64)
        });
        let aic = 4.0 - 2.0 * ll;
        if aic < best_student.3 {
            best_student = (nu, rho, ll, aic);
        }
    }
    let (family, rho, nu, log_likelihood, aic) = if best_student.3 < gaussian_aic {
        (
            CopulaFamily::StudentT,
            best_student.1,
            Some(best_student.0),
            best_student.2,
            best_student.3,
        )
    } else {
        (
            CopulaFamily::Gaussian,
            gaussian_rho,
            None,
            gaussian_ll,
            gaussian_aic,
        )
    };
    Some(CopulaFit {
        family,
        rho,
        nu,
        log_likelihood,
        aic,
        gaussian_log_likelihood: gaussian_ll,
        gaussian_aic,
        student_t_log_likelihood: best_student.2,
        student_t_aic: best_student.3,
        sample_count: uniforms.len(),
        kendall_tau: tau,
        fit_cutoff_ms: left.fit_cutoff_ms.min(right.fit_cutoff_ms),
        independent_reference_error: 0.0,
    })
}

pub fn corrected_pair_model(
    left: ReferenceSpreadFit,
    right: ReferenceSpreadFit,
) -> Option<CorrectedPairModel> {
    let copula = fit_copula(&left, &right)?;
    let stationarity_score = 2.0 - left.stationarity.adf_p_value - right.stationarity.adf_p_value
        + left.stationarity.kpss_p_value
        + right.stationarity.kpss_p_value;
    let cost_score = left.sigma.min(right.sigma);
    let tail_score = (left.stationarity.tail_sample_count + right.stationarity.tail_sample_count)
        as f64
        / copula.sample_count as f64;
    Some(CorrectedPairModel {
        left,
        right,
        score: stationarity_score + cost_score + tail_score - copula.aic / 1_000_000.0,
        copula,
    })
}

pub fn maximum_weight_disjoint(
    mut candidates: Vec<CorrectedPairModel>,
    max_pairs: usize,
) -> Vec<CorrectedPairModel> {
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut used = BTreeSet::new();
    candidates
        .into_iter()
        .filter(|model| {
            if used.contains(&model.left.alt) || used.contains(&model.right.alt) {
                false
            } else {
                used.insert(model.left.alt.clone());
                used.insert(model.right.alt.clone());
                true
            }
        })
        .take(max_pairs)
        .collect()
}

pub fn conditional_h(
    model: &CorrectedPairModel,
    btc: f64,
    left_price: f64,
    right_price: f64,
) -> (f64, f64) {
    let left_spread = btc.ln() - model.left.beta * left_price.ln();
    let right_spread = btc.ln() - model.right.beta * right_price.ln();
    let left_u = empirical_cdf(&model.left.sorted_spreads, left_spread);
    let right_u = empirical_cdf(&model.right.sorted_spreads, right_spread);
    match model.copula.family {
        CopulaFamily::Gaussian => (
            gaussian_conditional_h(left_u, right_u, model.copula.rho),
            gaussian_conditional_h(right_u, left_u, model.copula.rho),
        ),
        CopulaFamily::StudentT => {
            let nu = model.copula.nu.unwrap_or(3) as f64;
            (
                student_t_conditional_h(left_u, right_u, model.copula.rho, nu),
                student_t_conditional_h(right_u, left_u, model.copula.rho, nu),
            )
        }
    }
}

pub fn empirical_cdf(sorted: &[f64], value: f64) -> f64 {
    let rank = sorted.partition_point(|probe| *probe <= value);
    ((rank as f64 + 0.5) / (sorted.len() as f64 + 1.0)).clamp(1e-6, 1.0 - 1e-6)
}

pub fn kendall_tau_b(observations: &[(f64, f64)]) -> f64 {
    let n = observations.len();
    if n < 2 {
        return 0.0;
    }
    let mut pairs = observations.to_vec();
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
    let mut y_values = pairs.iter().map(|(_, y)| *y).collect::<Vec<_>>();
    y_values.sort_by(f64::total_cmp);
    y_values.dedup_by(|a, b| a.total_cmp(b).is_eq());
    let mut fenwick = Fenwick::new(y_values.len());
    let mut discordant = 0_u64;
    let mut prior = 0_u64;
    let mut index = 0;
    while index < pairs.len() {
        let mut end = index + 1;
        while end < pairs.len() && pairs[end].0.total_cmp(&pairs[index].0).is_eq() {
            end += 1;
        }
        for (_, y) in &pairs[index..end] {
            let rank = y_values
                .binary_search_by(|probe| probe.total_cmp(y))
                .unwrap();
            discordant += prior - fenwick.prefix_sum(rank + 1);
        }
        for (_, y) in &pairs[index..end] {
            let rank = y_values
                .binary_search_by(|probe| probe.total_cmp(y))
                .unwrap();
            fenwick.add(rank, 1);
            prior += 1;
        }
        index = end;
    }
    let n0 = combinations(n) as f64;
    let n1 = tie_pairs(pairs.iter().map(|(x, _)| *x));
    let n2 = tie_pairs(pairs.iter().map(|(_, y)| *y));
    let mut both = pairs.clone();
    both.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
    let n3 = tie_pair_tuples(&both);
    let comparable = n0 - n1 - n2 + n3;
    let denominator = ((n0 - n1) * (n0 - n2)).sqrt();
    if denominator <= 0.0 {
        0.0
    } else {
        ((comparable - 2.0 * discordant as f64) / denominator).clamp(-1.0, 1.0)
    }
}

fn stationarity_diagnostics(values: &[f64], sigma: f64) -> StationarityDiagnostics {
    let (adf_stat, adf_p_value, phi) = adf_lag_zero(values);
    let (kpss_stat, kpss_p_value) = kpss_level(values);
    let half_life_bars = if phi > 0.0 && phi < 1.0 {
        (-std::f64::consts::LN_2 / phi.ln()).max(0.0)
    } else {
        f64::INFINITY
    };
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let mut cumulative = 0.0_f64;
    let cusum_max = values
        .iter()
        .map(|value| {
            cumulative += value - mean;
            cumulative.abs() / (sigma * (values.len() as f64).sqrt()).max(1e-12)
        })
        .fold(0.0, f64::max);
    let break_valid = cusum_max <= 1.63;
    let tail_sample_count = values.len() / 20;
    let cost_feasible = sigma >= 0.001;
    let passed = adf_p_value <= 0.05
        && kpss_p_value >= 0.05
        && half_life_bars.is_finite()
        && half_life_bars >= 1.0
        && half_life_bars <= values.len() as f64 * 0.5
        && break_valid
        && tail_sample_count >= 20
        && cost_feasible;
    StationarityDiagnostics {
        adf_stat,
        adf_p_value,
        kpss_stat,
        kpss_p_value,
        half_life_bars,
        cusum_max,
        break_valid,
        tail_sample_count,
        cost_feasible,
        passed,
    }
}

fn adf_lag_zero(values: &[f64]) -> (f64, f64, f64) {
    let n = values.len() - 1;
    let x_mean = values[..n].iter().sum::<f64>() / n as f64;
    let differences = values.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
    let y_mean = differences.iter().sum::<f64>() / n as f64;
    let sxx = values[..n]
        .iter()
        .map(|x| (*x - x_mean).powi(2))
        .sum::<f64>();
    if sxx <= 1e-12 {
        return (0.0, 1.0, 1.0);
    }
    let gamma = values[..n]
        .iter()
        .zip(&differences)
        .map(|(x, y)| (*x - x_mean) * (*y - y_mean))
        .sum::<f64>()
        / sxx;
    let intercept = y_mean - gamma * x_mean;
    let rss = values[..n]
        .iter()
        .zip(&differences)
        .map(|(x, y)| (*y - intercept - gamma * *x).powi(2))
        .sum::<f64>();
    let standard_error = (rss / (n.saturating_sub(2).max(1) as f64) / sxx).sqrt();
    let statistic = gamma / standard_error.max(1e-12);
    (
        statistic,
        normal_cdf(statistic),
        (1.0 + gamma).clamp(-1.0, 1.0),
    )
}

fn kpss_level(values: &[f64]) -> (f64, f64) {
    let n = values.len();
    let mean = values.iter().sum::<f64>() / n as f64;
    let residuals = values.iter().map(|x| *x - mean).collect::<Vec<_>>();
    let bandwidth = ((n as f64).sqrt() as usize).max(1).min(n - 1);
    let mut long_run_variance = residuals.iter().map(|x| x * x).sum::<f64>() / n as f64;
    for lag in 1..=bandwidth {
        let covariance = residuals[lag..]
            .iter()
            .zip(&residuals[..n - lag])
            .map(|(a, b)| a * b)
            .sum::<f64>()
            / n as f64;
        long_run_variance += 2.0 * (1.0 - lag as f64 / (bandwidth + 1) as f64) * covariance;
    }
    let mut cumulative = 0.0;
    let eta = residuals
        .iter()
        .map(|residual| {
            cumulative += residual;
            cumulative * cumulative
        })
        .sum::<f64>()
        / (n as f64).powi(2);
    let statistic = eta / long_run_variance.max(1e-12);
    let p_value = if statistic <= 0.347 {
        0.10
    } else if statistic <= 0.463 {
        0.05 + (0.463 - statistic) / (0.463 - 0.347) * 0.05
    } else if statistic <= 0.574 {
        0.025 + (0.574 - statistic) / (0.574 - 0.463) * 0.025
    } else {
        0.01
    };
    (statistic, p_value)
}

fn gaussian_copula_log_likelihood(uniforms: &[(f64, f64)], rho: f64) -> f64 {
    let determinant = 1.0 - rho * rho;
    uniforms
        .iter()
        .map(|(u, v)| {
            let x = inverse_normal_cdf(*u);
            let y = inverse_normal_cdf(*v);
            -0.5 * determinant.ln()
                - (rho * rho * (x * x + y * y) - 2.0 * rho * x * y) / (2.0 * determinant)
        })
        .sum()
}

fn student_t_copula_log_likelihood(uniforms: &[(f64, f64)], rho: f64, nu: f64) -> f64 {
    let determinant = 1.0 - rho * rho;
    uniforms
        .iter()
        .map(|(u, v)| {
            let x = fast_inverse_student_t(*u, nu);
            let y = fast_inverse_student_t(*v, nu);
            let q = (x * x - 2.0 * rho * x * y + y * y) / determinant;
            let bivariate = ln_gamma((nu + 2.0) * 0.5)
                - ln_gamma(nu * 0.5)
                - (nu * std::f64::consts::PI).ln()
                - 0.5 * determinant.ln()
                - (nu + 2.0) * 0.5 * (1.0 + q / nu).ln();
            let univariate = |z: f64| {
                ln_gamma((nu + 1.0) * 0.5)
                    - ln_gamma(nu * 0.5)
                    - 0.5 * (nu * std::f64::consts::PI).ln()
                    - (nu + 1.0) * 0.5 * (1.0 + z * z / nu).ln()
            };
            bivariate - univariate(x) - univariate(y)
        })
        .sum()
}

fn optimize_rho<F>(start: f64, score: F) -> (f64, f64)
where
    F: Fn(f64) -> f64,
{
    let mut best = (start, score(start));
    for index in -19..=19 {
        let rho = index as f64 * 0.05;
        let value = score(rho);
        if value > best.1 {
            best = (rho, value);
        }
    }
    let mut lo = (best.0 - 0.05).max(-0.95);
    let mut hi = (best.0 + 0.05).min(0.95);
    for _ in 0..24 {
        let left = lo + (hi - lo) / 3.0;
        let right = hi - (hi - lo) / 3.0;
        if score(left) < score(right) {
            lo = left;
        } else {
            hi = right;
        }
    }
    let rho = (lo + hi) * 0.5;
    (rho, score(rho))
}

fn fast_inverse_student_t(p: f64, nu: f64) -> f64 {
    let z = inverse_normal_cdf(p.clamp(1e-9, 1.0 - 1e-9));
    let z2 = z * z;
    z + (z * (z2 + 1.0)) / (4.0 * nu)
        + (z * (5.0 * z2 * z2 + 16.0 * z2 + 3.0)) / (96.0 * nu * nu)
        + (z * (3.0 * z2 * z2 * z2 + 19.0 * z2 * z2 + 17.0 * z2 - 15.0)) / (384.0 * nu * nu * nu)
}

fn sample_sigma(values: &[f64], mean: f64) -> f64 {
    (values.iter().map(|x| (*x - mean).powi(2)).sum::<f64>()
        / (values.len().saturating_sub(1).max(1) as f64))
        .sqrt()
}

fn combinations(n: usize) -> u64 {
    (n as u64).saturating_mul((n.saturating_sub(1)) as u64) / 2
}

fn tie_pairs(values: impl Iterator<Item = f64>) -> f64 {
    let mut values = values.collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    let mut total = 0_u64;
    let mut start = 0;
    while start < values.len() {
        let mut end = start + 1;
        while end < values.len() && values[end].total_cmp(&values[start]).is_eq() {
            end += 1;
        }
        total += combinations(end - start);
        start = end;
    }
    total as f64
}

fn tie_pair_tuples(values: &[(f64, f64)]) -> f64 {
    let mut total = 0_u64;
    let mut start = 0;
    while start < values.len() {
        let mut end = start + 1;
        while end < values.len()
            && values[end].0.total_cmp(&values[start].0).is_eq()
            && values[end].1.total_cmp(&values[start].1).is_eq()
        {
            end += 1;
        }
        total += combinations(end - start);
        start = end;
    }
    total as f64
}

fn ln_gamma(z: f64) -> f64 {
    const COEFFICIENTS: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1259.139_216_722_4,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if z < 0.5 {
        return std::f64::consts::PI.ln()
            - (std::f64::consts::PI * z).sin().ln()
            - ln_gamma(1.0 - z);
    }
    let shifted = z - 1.0;
    let mut x = COEFFICIENTS[0];
    for (index, coefficient) in COEFFICIENTS.iter().enumerate().skip(1) {
        x += coefficient / (shifted + index as f64);
    }
    let t = shifted + 7.5;
    0.5 * (2.0 * std::f64::consts::PI).ln() + (shifted + 0.5) * t.ln() - t + x.ln()
}

struct Fenwick {
    tree: Vec<u64>,
}

impl Fenwick {
    fn new(size: usize) -> Self {
        Self {
            tree: vec![0; size + 1],
        }
    }

    fn add(&mut self, mut index: usize, value: u64) {
        index += 1;
        while index < self.tree.len() {
            self.tree[index] += value;
            index += index & index.wrapping_neg();
        }
    }

    fn prefix_sum(&self, mut end: usize) -> u64 {
        let mut total = 0;
        while end > 0 {
            total += self.tree[end];
            end &= end - 1;
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r25::{atomic_pair_admission, R25Filter};
    use r24_engine::PositionMode;

    fn stationary_fit(name: &str, values: Vec<(i64, f64)>) -> ReferenceSpreadFit {
        let raw = values.iter().map(|(_, value)| *value).collect::<Vec<_>>();
        let mean = raw.iter().sum::<f64>() / raw.len() as f64;
        let sigma = sample_sigma(&raw, mean);
        let mut sorted_spreads = raw.clone();
        sorted_spreads.sort_by(f64::total_cmp);
        ReferenceSpreadFit {
            alt: name.into(),
            beta: 1.0,
            mean,
            sigma,
            aligned_spreads: values,
            sorted_spreads,
            fit_cutoff_ms: 1_000,
            stationarity: StationarityDiagnostics {
                adf_stat: -4.0,
                adf_p_value: 0.01,
                kpss_stat: 0.1,
                kpss_p_value: 0.1,
                half_life_bars: 5.0,
                cusum_max: 0.2,
                break_valid: true,
                tail_sample_count: 20,
                cost_feasible: true,
                passed: true,
            },
        }
    }

    #[test]
    fn copula_dependence_uses_timestamp_alignment_not_sorted_rank() {
        let observations = (0..240)
            .map(|index| (index as i64, index as f64, (240 - index) as f64))
            .collect::<Vec<_>>();
        assert!(
            kendall_tau_b(
                &observations
                    .iter()
                    .map(|(_, a, b)| (*a, *b))
                    .collect::<Vec<_>>()
            ) < -0.99
        );
    }

    #[test]
    fn permuting_time_order_changes_dependence_but_not_empirical_marginal() {
        let left = stationary_fit("LEFT", (0..240).map(|i| (i, i as f64)).collect::<Vec<_>>());
        let right_aligned =
            stationary_fit("RIGHT", (0..240).map(|i| (i, i as f64)).collect::<Vec<_>>());
        let right_permuted = stationary_fit(
            "RIGHT",
            (0..240).map(|i| (i, (239 - i) as f64)).collect::<Vec<_>>(),
        );
        assert_eq!(right_aligned.sorted_spreads, right_permuted.sorted_spreads);
        let aligned_tau = fit_copula(&left, &right_aligned).unwrap().kendall_tau;
        let permuted_tau = fit_copula(&left, &right_permuted).unwrap().kendall_tau;
        assert!(aligned_tau > 0.99 && permuted_tau < -0.99);
    }

    #[test]
    fn gaussian_and_student_t_aic_match_independent_reference() {
        let uniforms = vec![(0.1, 0.2), (0.3, 0.8), (0.7, 0.6), (0.9, 0.95)];
        let gaussian = gaussian_copula_log_likelihood(&uniforms, 0.35);
        let student = student_t_copula_log_likelihood(&uniforms, 0.35, 6.0);
        assert!((gaussian - 0.849_419_762_528_430).abs() < 1e-12);
        assert!((student - 0.982_788_028_231_706).abs() < 1e-12);
        assert!(((2.0 - 2.0 * gaussian) - 0.301_160_474_943_140).abs() < 1e-12);
        assert!(((4.0 - 2.0 * student) - 2.034_423_943_536_588).abs() < 1e-12);
    }

    #[test]
    fn failed_adf_or_kpss_pair_never_reaches_order_path() {
        let left = stationary_fit("LEFT", (0..240).map(|i| (i, i as f64)).collect());
        let mut right = left.clone();
        right.alt = "RIGHT".into();
        right.stationarity.passed = false;
        right.stationarity.adf_p_value = 0.20;
        assert!(fit_copula(&left, &right).is_none());
    }

    #[test]
    fn future_shift_changes_fit_only_after_cutoff() {
        let base = (0..300)
            .map(|i| {
                let time = i as f64;
                (
                    i,
                    100.0 + time * 0.01 + (time / 7.0).sin(),
                    50.0 + time * 0.005 + (time / 11.0).cos() * 0.4,
                )
            })
            .collect::<Vec<_>>();
        let mut shifted = base.clone();
        for row in shifted.iter_mut().filter(|row| row.0 > 250) {
            row.2 *= 5.0;
        }
        let before = fit_reference_spread("ALT", &base, 250).unwrap();
        let before_shifted = fit_reference_spread("ALT", &shifted, 250).unwrap();
        assert_eq!(before.beta, before_shifted.beta);
        assert_eq!(before.sorted_spreads, before_shifted.sorted_spreads);
        let after = fit_reference_spread("ALT", &shifted, 299).unwrap();
        assert_ne!(before.beta, after.beta);
    }

    #[test]
    fn active_cycle_keeps_frozen_family_rho_nu_across_roll() {
        let frozen = CopulaFit {
            family: CopulaFamily::StudentT,
            rho: 0.42,
            nu: Some(7),
            log_likelihood: 10.0,
            aic: -16.0,
            gaussian_log_likelihood: 8.0,
            gaussian_aic: -14.0,
            student_t_log_likelihood: 10.0,
            student_t_aic: -16.0,
            sample_count: 240,
            kendall_tau: 0.3,
            fit_cutoff_ms: 100,
            independent_reference_error: 0.0,
        };
        let active_binding = frozen.clone();
        let selector_roll = CopulaFit {
            family: CopulaFamily::Gaussian,
            rho: -0.2,
            nu: None,
            ..frozen.clone()
        };
        assert_eq!(active_binding.family, CopulaFamily::StudentT);
        assert_eq!(active_binding.rho, 0.42);
        assert_eq!(active_binding.nu, Some(7));
        assert_ne!(active_binding, selector_roll);
    }

    #[test]
    fn eth_and_link_are_filter_feasible_at_historical_extremes() {
        let eth = R25Filter {
            tick_size: 0.01,
            step_size: 0.001,
            min_qty: 0.001,
            min_notional: 20.0,
        };
        let link = R25Filter {
            tick_size: 0.001,
            step_size: 0.01,
            min_qty: 0.01,
            min_notional: 20.0,
        };
        for (filter, low, high) in [(eth, 880.0, 4_900.0), (link, 4.9, 52.0)] {
            for price in [low, high] {
                let long = filter.resolve_for_mode(25.0 / price, price, PositionMode::Long);
                let short = filter.resolve_for_mode(25.0 / price, price, PositionMode::Short);
                assert!(long.accepted && short.accepted);
            }
        }
    }

    #[test]
    fn fifty_u_group_makes_six_asset_gate_structurally_reachable() {
        let filters = [
            R25Filter {
                tick_size: 0.01,
                step_size: 0.001,
                min_qty: 0.001,
                min_notional: 20.0,
            },
            R25Filter {
                tick_size: 0.01,
                step_size: 0.01,
                min_qty: 0.01,
                min_notional: 5.0,
            },
            R25Filter {
                tick_size: 0.001,
                step_size: 0.01,
                min_qty: 0.01,
                min_notional: 5.0,
            },
            R25Filter {
                tick_size: 0.0001,
                step_size: 0.1,
                min_qty: 0.1,
                min_notional: 5.0,
            },
            R25Filter {
                tick_size: 0.00001,
                step_size: 1.0,
                min_qty: 1.0,
                min_notional: 5.0,
            },
            R25Filter {
                tick_size: 0.001,
                step_size: 0.01,
                min_qty: 0.01,
                min_notional: 20.0,
            },
        ];
        let prices = [2_500.0, 600.0, 150.0, 0.5, 0.15, 15.0];
        assert!(filters.into_iter().zip(prices).all(|(filter, price)| {
            let left = filter.resolve_for_mode(25.0 / price, price, PositionMode::Long);
            let right = filter.resolve_for_mode(25.0 / price, price, PositionMode::Short);
            atomic_pair_admission(left, right)
        }));
    }
}
