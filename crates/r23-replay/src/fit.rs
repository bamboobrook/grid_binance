//! Train-only OLS cointegration fit for a two-leg pair.
//!
//! Matches the engine's residual definition exactly: residual =
//! log(legs[1]) - betas[0]*log(legs[0]) - mus[0], where legs[0] is the
//! independent variable. The fit uses ONLY train bars (anti-overfit contract);
//! the caller is responsible for passing train-only closes.

use anyhow::{anyhow, Result};

use backtest_engine::martingale::sync_cycle_engine::SynchronizedFit;

/// A fitted pair with diagnostic stats.
pub struct PairFit {
    pub fit: SynchronizedFit,
    pub beta: f64,
    pub mu: f64,
    pub residual_sigma: f64,
    pub half_life_h: Option<f64>,
    pub n_train: usize,
}

/// Fit `log(b_closes) ~ beta * log(a_closes) + mu` on the train window and
/// compute the residual standard deviation. `a_closes` and `b_closes` must be
/// the same length and time-aligned (both sampled at the same bar boundary).
pub fn fit_pair_ols(
    group_id: &str,
    leg_a: &str,
    leg_b: &str,
    a_closes: &[f64],
    b_closes: &[f64],
    bar_minutes: u32,
) -> Result<PairFit> {
    if a_closes.len() != b_closes.len() {
        return Err(anyhow!("length mismatch: {} vs {}", a_closes.len(), b_closes.len()));
    }
    let n = a_closes.len();
    if n < 30 {
        return Err(anyhow!("need >=30 train bars, got {n}"));
    }
    // log prices, skip non-positive
    let mut xa: Vec<f64> = Vec::with_capacity(n);
    let mut yb: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        if a_closes[i] > 0.0 && b_closes[i] > 0.0 {
            xa.push(a_closes[i].ln());
            yb.push(b_closes[i].ln());
        }
    }
    let m = xa.len();
    if m < 30 {
        return Err(anyhow!("need >=30 valid train bars after log filter, got {m}"));
    }
    let mean_x = xa.iter().sum::<f64>() / m as f64;
    let mean_y = yb.iter().sum::<f64>() / m as f64;
    let mut sxx = 0.0;
    let mut sxy = 0.0;
    for i in 0..m {
        sxx += (xa[i] - mean_x) * (xa[i] - mean_x);
        sxy += (xa[i] - mean_x) * (yb[i] - mean_y);
    }
    if sxx <= 0.0 {
        return Err(anyhow!("zero variance in independent leg"));
    }
    let beta = sxy / sxx;
    let mu = mean_y - beta * mean_x;
    // residual sigma
    let mut resid: Vec<f64> = Vec::with_capacity(m);
    for i in 0..m {
        resid.push(yb[i] - beta * xa[i] - mu);
    }
    let rmean = resid.iter().sum::<f64>() / m as f64;
    let var = resid.iter().map(|r| (r - rmean).powi(2)).sum::<f64>() / m as f64;
    let sigma = var.sqrt();
    if !sigma.is_finite() || sigma <= 0.0 {
        return Err(anyhow!("degenerate residual sigma"));
    }
    // half-life via AR(1) on residuals: r_t = phi * r_{t-1} + e; HL = -ln(2)/ln(phi)
    let half_life_h = half_life_hours(&resid, bar_minutes);
    let fit_sha256 = format!(
        "{}|{}|{}|beta={:.10}|mu={:.10}|sigma={:.10}|n={}",
        group_id, leg_a, leg_b, beta, mu, sigma, m
    );
    let fit = SynchronizedFit {
        group_id: group_id.to_string(),
        legs: vec![leg_a.to_string(), leg_b.to_string()],
        leg_markets: Vec::new(),
        leg_direction_signs: vec![1, 1],
        betas: vec![beta],
        mus: vec![mu],
        residual_sigma: sigma,
        half_life_h: half_life_h.unwrap_or(24.0),
        weights: Vec::new(),
        fit_sha256: r23_registry::hash::sha256_str(&fit_sha256),
    };
    Ok(PairFit { fit, beta, mu, residual_sigma: sigma, half_life_h, n_train: m })
}

fn half_life_hours(resid: &[f64], bar_minutes: u32) -> Option<f64> {
    if resid.len() < 10 || bar_minutes == 0 {
        return None;
    }
    let n = resid.len() - 1;
    let mean = resid.iter().sum::<f64>() / resid.len() as f64;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    for i in 0..n {
        let x = resid[i] - mean;
        let y = resid[i + 1] - mean;
        sxy += x * y;
        sxx += x * x;
    }
    if sxx <= 0.0 {
        return None;
    }
    let phi = sxy / sxx;
    if phi <= 0.0 || phi >= 1.0 {
        return None;
    }
    let hl_bars = -((2.0f64).ln()) / phi.ln();
    Some(hl_bars * bar_minutes as f64 / 60.0)
}
