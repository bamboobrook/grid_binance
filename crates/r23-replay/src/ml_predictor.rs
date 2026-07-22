//! ML logistic-regression next-bar-direction predictor (plan §3 — ML ensemble).
//!
//! Trains a logistic regression on M1 features (price_ext, oi_change, taker_flow,
//! top_crowd, all_crowd) to predict next-bar direction (up/down). Training is
//! prequential: at each test block, the model is fit only on past data.
//! Pure Rust (gradient descent with L2 regularization) — no external ML crate.

use serde::{Deserialize, Serialize};

const LR: f64 = 0.01;
const EPOCHS: usize = 50;
const L2: f64 = 0.001;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogisticModel {
    pub weights: Vec<f64>,
    pub bias: f64,
    pub n_features: usize,
}

impl LogisticModel {
    pub fn new(n_features: usize) -> Self {
        Self { weights: vec![0.0; n_features], bias: 0.0, n_features }
    }

    pub fn predict_proba(&self, features: &[f64]) -> f64 {
        let z = self.bias + features.iter().zip(&self.weights).map(|(x, w)| x * w).sum::<f64>();
        1.0 / (1.0 + (-z).exp())
    }

    /// Train on (features, label) pairs. label = 1.0 if next-bar up, 0.0 if down.
    pub fn train(&mut self, x: &[Vec<f64>], y: &[f64]) {
        let n = x.len();
        if n == 0 { return; }
        for _ in 0..EPOCHS {
            let mut grad_w = vec![0.0; self.n_features];
            let mut grad_b = 0.0;
            for i in 0..n {
                let p = self.predict_proba(&x[i]);
                let err = p - y[i]; // gradient of log-loss
                for j in 0..self.n_features {
                    grad_w[j] += err * x[i][j];
                }
                grad_b += err;
            }
            for j in 0..self.n_features {
                self.weights[j] -= LR * (grad_w[j] / n as f64 + L2 * self.weights[j]);
            }
            self.bias -= LR * (grad_b / n as f64);
        }
    }
}

/// Build training data from M1 states + aligned next-bar returns.
/// Features: [price_ext, oi_change, taker_flow, top_crowd, all_crowd].
/// Label: 1.0 if next-bar return > 0, else 0.0.
pub fn build_dataset(
    states: &[crate::m1_signal::M1State],
    closes: &[(i64, f64)],
) -> (Vec<Vec<f64>>, Vec<f64>, Vec<i64>) {
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut ts = Vec::new();
    let close_map: std::collections::BTreeMap<i64, f64> = closes.iter().cloned().collect();
    for s in states {
        if let (Some(cur), Some(next)) = (close_map.get(&s.ts_ms), close_map.get(&(s.ts_ms + 300_000))) {
            if *cur > 0.0 && *next > 0.0 {
                x.push(vec![s.price_ext, s.oi_change, s.taker_flow, s.top_crowd, s.all_crowd]);
                y.push(if next > cur { 1.0 } else { 0.0 });
                ts.push(s.ts_ms);
            }
        }
    }
    (x, y, ts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_predicts_after_training() {
        let mut m = LogisticModel::new(2);
        // feature [1, 0] → up; [0, 1] → down
        let x = vec![vec![1.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0], vec![0.0, 1.0]];
        let y = vec![1.0, 1.0, 0.0, 0.0];
        m.train(&x, &y);
        let p_up = m.predict_proba(&[1.0, 0.0]);
        let p_down = m.predict_proba(&[0.0, 1.0]);
        assert!(p_up > 0.5, "p_up={p_up}");
        assert!(p_down < 0.5, "p_down={p_down}");
    }
}
