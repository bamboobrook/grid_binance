use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{bail, Result};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::UNIVERSE;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourBar {
    pub timestamp_ms: i64,
    pub close: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairFit {
    pub y_symbol: String,
    pub x_symbol: String,
    pub alpha: f64,
    pub beta: f64,
    pub residual_mean: f64,
    pub residual_sigma: f64,
    pub adf_t: f64,
    pub kpss_stat: f64,
    pub half_life_hours: f64,
    pub crossing_count: u64,
    pub expected_round_trip_cost_bps: f64,
    pub break_frequency: f64,
    pub train_score: f64,
    pub fit_start_ms: i64,
    pub fit_end_ms: i64,
}

impl PairFit {
    pub fn residual(&self, y_price: f64, x_price: f64) -> f64 {
        y_price.ln() - self.alpha - self.beta * x_price.ln()
    }

    pub fn zscore(&self, y_price: f64, x_price: f64) -> f64 {
        (self.residual(y_price, x_price) - self.residual_mean) / self.residual_sigma
    }
}

pub fn load_hourly_perp(
    database: &Path,
    start_ms: i64,
    end_ms: i64,
) -> Result<BTreeMap<String, Vec<HourBar>>> {
    let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut output = BTreeMap::new();
    for symbol in UNIVERSE {
        let mut statement = connection.prepare("SELECT open_time,close FROM klines WHERE symbol=?1 AND market_type='futures_usdt_perp' AND timeframe='1m' AND open_time>=?2 AND open_time<?3 ORDER BY open_time")?;
        let mut rows = statement.query((symbol, start_ms, end_ms))?;
        let mut bars = Vec::new();
        let mut active_hour = None;
        let mut last_close = 0.0;
        while let Some(row) = rows.next()? {
            let timestamp: i64 = row.get(0)?;
            let close: f64 = row.get(1)?;
            let hour = timestamp / 3_600_000 * 3_600_000;
            if active_hour.is_some_and(|previous| previous != hour) {
                bars.push(HourBar {
                    timestamp_ms: active_hour.unwrap(),
                    close: last_close,
                });
            }
            active_hour = Some(hour);
            last_close = close;
        }
        if let Some(hour) = active_hour {
            bars.push(HourBar {
                timestamp_ms: hour,
                close: last_close,
            });
        }
        output.insert(symbol.into(), bars);
    }
    Ok(output)
}

pub fn fit_all_pairs(
    data: &BTreeMap<String, Vec<HourBar>>,
    fit_start_ms: i64,
    fit_end_ms: i64,
    expected_round_trip_cost_bps: f64,
) -> Vec<PairFit> {
    let mut fits = Vec::new();
    for y_index in 0..UNIVERSE.len() {
        for x_index in (y_index + 1)..UNIVERSE.len() {
            if let Some(fit) = fit_pair(
                data,
                UNIVERSE[y_index],
                UNIVERSE[x_index],
                fit_start_ms,
                fit_end_ms,
                expected_round_trip_cost_bps,
            ) {
                fits.push(fit);
            }
        }
    }
    fits
}

pub fn fit_pair(
    data: &BTreeMap<String, Vec<HourBar>>,
    y_symbol: &str,
    x_symbol: &str,
    fit_start_ms: i64,
    fit_end_ms: i64,
    expected_round_trip_cost_bps: f64,
) -> Option<PairFit> {
    let y = data.get(y_symbol)?;
    let x = data.get(x_symbol)?;
    let x_map = x
        .iter()
        .filter(|bar| bar.timestamp_ms >= fit_start_ms && bar.timestamp_ms <= fit_end_ms)
        .map(|bar| (bar.timestamp_ms, bar.close))
        .collect::<BTreeMap<_, _>>();
    let aligned = y
        .iter()
        .filter(|bar| bar.timestamp_ms >= fit_start_ms && bar.timestamp_ms <= fit_end_ms)
        .filter_map(|bar| {
            x_map
                .get(&bar.timestamp_ms)
                .map(|x_close| (bar.close.ln(), x_close.ln()))
        })
        .collect::<Vec<_>>();
    if aligned.len() < 24 * 30 {
        return None;
    }
    let y_values = aligned.iter().map(|value| value.0).collect::<Vec<_>>();
    let x_values = aligned.iter().map(|value| value.1).collect::<Vec<_>>();
    let x_mean = mean(&x_values);
    let y_mean = mean(&y_values);
    let variance = x_values
        .iter()
        .map(|value| (value - x_mean).powi(2))
        .sum::<f64>();
    if variance <= 1e-12 {
        return None;
    }
    let beta = x_values
        .iter()
        .zip(&y_values)
        .map(|(x, y)| (x - x_mean) * (y - y_mean))
        .sum::<f64>()
        / variance;
    if !(0.2..=5.0).contains(&beta) {
        return None;
    }
    let alpha = y_mean - beta * x_mean;
    let residuals = aligned
        .iter()
        .map(|(y, x)| y - alpha - beta * x)
        .collect::<Vec<_>>();
    let residual_mean = mean(&residuals);
    let residual_sigma = sample_std(&residuals, residual_mean);
    if residual_sigma <= 1e-8 {
        return None;
    }
    let adf_t = adf_t_stat(&residuals)?;
    let kpss_stat = kpss_stat(&residuals)?;
    let phi = ar1_phi(&residuals)?;
    let half_life_hours = if phi > 0.0 && phi < 1.0 {
        -2.0_f64.ln() / phi.ln()
    } else {
        f64::INFINITY
    };
    let crossing_count = residuals
        .windows(2)
        .filter(|pair| (pair[0] - residual_mean) * (pair[1] - residual_mean) < 0.0)
        .count() as u64;
    let break_frequency = residuals
        .iter()
        .filter(|value| (**value - residual_mean).abs() > 4.0 * residual_sigma)
        .count() as f64
        / residuals.len() as f64;
    if adf_t >= -2.86
        || kpss_stat >= 0.463
        || !(2.0..=24.0 * 30.0).contains(&half_life_hours)
        || crossing_count < 6
    {
        return None;
    }
    let train_score = crossing_count as f64
        / (half_life_hours.max(1.0)
            * (expected_round_trip_cost_bps.max(1.0) / 10.0)
            * (1.0 + break_frequency * 100.0));
    Some(PairFit {
        y_symbol: y_symbol.into(),
        x_symbol: x_symbol.into(),
        alpha,
        beta,
        residual_mean,
        residual_sigma,
        adf_t,
        kpss_stat,
        half_life_hours,
        crossing_count,
        expected_round_trip_cost_bps,
        break_frequency,
        train_score,
        fit_start_ms,
        fit_end_ms,
    })
}

pub fn maximum_disjoint_matching(fits: &[PairFit], minimum_pairs: usize) -> Result<Vec<PairFit>> {
    let eligible = fits.iter().cloned().collect::<Vec<_>>();
    let mut best: Option<(f64, Vec<PairFit>)> = None;
    search_matching(
        &eligible,
        0,
        &mut BTreeSet::new(),
        &mut Vec::new(),
        0.0,
        &mut best,
    );
    let result = best.map(|value| value.1).unwrap_or_default();
    if result.len() < minimum_pairs {
        bail!("no train-only disjoint matching with at least {minimum_pairs} pairs");
    }
    Ok(result)
}

fn search_matching(
    fits: &[PairFit],
    index: usize,
    used: &mut BTreeSet<String>,
    selected: &mut Vec<PairFit>,
    score: f64,
    best: &mut Option<(f64, Vec<PairFit>)>,
) {
    if index == fits.len() {
        if best.as_ref().is_none_or(|current| score > current.0) {
            *best = Some((score, selected.clone()));
        }
        return;
    }
    search_matching(fits, index + 1, used, selected, score, best);
    let fit = &fits[index];
    if !used.contains(&fit.y_symbol) && !used.contains(&fit.x_symbol) {
        used.insert(fit.y_symbol.clone());
        used.insert(fit.x_symbol.clone());
        selected.push(fit.clone());
        search_matching(
            fits,
            index + 1,
            used,
            selected,
            score + fit.train_score,
            best,
        );
        selected.pop();
        used.remove(&fit.y_symbol);
        used.remove(&fit.x_symbol);
    }
}

fn adf_t_stat(values: &[f64]) -> Option<f64> {
    let lagged = &values[..values.len() - 1];
    let delta = values
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect::<Vec<_>>();
    slope_t_stat(lagged, &delta)
}

fn slope_t_stat(x: &[f64], y: &[f64]) -> Option<f64> {
    let x_mean = mean(x);
    let y_mean = mean(y);
    let sxx = x.iter().map(|value| (value - x_mean).powi(2)).sum::<f64>();
    if sxx <= 1e-14 || x.len() < 4 {
        return None;
    }
    let slope = x
        .iter()
        .zip(y)
        .map(|(x, y)| (x - x_mean) * (y - y_mean))
        .sum::<f64>()
        / sxx;
    let intercept = y_mean - slope * x_mean;
    let residual_variance = x
        .iter()
        .zip(y)
        .map(|(x, y)| (y - intercept - slope * x).powi(2))
        .sum::<f64>()
        / (x.len() - 2) as f64;
    Some(slope / (residual_variance / sxx).sqrt())
}

fn kpss_stat(values: &[f64]) -> Option<f64> {
    if values.len() < 10 {
        return None;
    }
    let average = mean(values);
    let centered = values
        .iter()
        .map(|value| value - average)
        .collect::<Vec<_>>();
    let mut cumulative = 0.0;
    let eta = centered
        .iter()
        .map(|value| {
            cumulative += value;
            cumulative.powi(2)
        })
        .sum::<f64>()
        / (values.len() * values.len()) as f64;
    let lag = (4.0 * (values.len() as f64 / 100.0).powf(0.25)).floor() as usize;
    let mut long_run_variance =
        centered.iter().map(|value| value * value).sum::<f64>() / values.len() as f64;
    for offset in 1..=lag {
        let covariance = centered[offset..]
            .iter()
            .zip(&centered[..centered.len() - offset])
            .map(|(a, b)| a * b)
            .sum::<f64>()
            / values.len() as f64;
        long_run_variance += 2.0 * (1.0 - offset as f64 / (lag + 1) as f64) * covariance;
    }
    (long_run_variance > 1e-14).then_some(eta / long_run_variance)
}

fn ar1_phi(values: &[f64]) -> Option<f64> {
    let average = mean(values);
    let numerator = values
        .windows(2)
        .map(|pair| (pair[0] - average) * (pair[1] - average))
        .sum::<f64>();
    let denominator = values[..values.len() - 1]
        .iter()
        .map(|value| (value - average).powi(2))
        .sum::<f64>();
    (denominator > 1e-14).then_some(numerator / denominator)
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}
fn sample_std(values: &[f64], average: f64) -> f64 {
    (values
        .iter()
        .map(|value| (value - average).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64)
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stationary_residual_passes_adf_and_kpss() {
        let mut value = 0.0;
        let values = (0..4000)
            .map(|index| {
                value = 0.85 * value + ((index * 7919 % 101) as f64 - 50.0) / 1000.0;
                value
            })
            .collect::<Vec<_>>();
        assert!(adf_t_stat(&values).unwrap() < -2.86);
        assert!(kpss_stat(&values).unwrap() < 0.463);
    }

    #[test]
    fn maximum_matching_is_disjoint() {
        let fits = [
            ("A", "B", 4.0),
            ("A", "C", 9.0),
            ("B", "D", 8.0),
            ("E", "F", 7.0),
            ("G", "H", 6.0),
        ]
        .into_iter()
        .map(|(y, x, score)| PairFit {
            y_symbol: y.into(),
            x_symbol: x.into(),
            alpha: 0.0,
            beta: 1.0,
            residual_mean: 0.0,
            residual_sigma: 1.0,
            adf_t: -4.0,
            kpss_stat: 0.1,
            half_life_hours: 10.0,
            crossing_count: 10,
            expected_round_trip_cost_bps: 10.0,
            break_frequency: 0.0,
            train_score: score,
            fit_start_ms: 0,
            fit_end_ms: 1,
        })
        .collect::<Vec<_>>();
        let selected = maximum_disjoint_matching(&fits, 3).unwrap();
        let assets = selected
            .iter()
            .flat_map(|fit| [&fit.y_symbol, &fit.x_symbol])
            .collect::<BTreeSet<_>>();
        assert_eq!(assets.len(), selected.len() * 2);
        assert!(selected.len() >= 3);
    }
}
