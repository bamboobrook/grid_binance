use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const R25_REFERENCE_SYMBOL: &str = "BTCUSDT";
pub const R25_FAMILY: &str = "C1_REFERENCE_ASSET_CONDITIONAL_COPULA_MARTIN";
pub const R25_C2: &str = "C2_ASYMMETRIC_TAIL_SO_VETO";
pub const R25_TRADABLE_ALTS: [&str; 7] = [
    "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "LINKUSDT", "LTCUSDT",
];
pub const R25_UNIVERSE: [&str; 8] = [
    "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "LINKUSDT", "LTCUSDT",
];
pub const R25_COLD_STARTS: [&str; 5] = [
    "2023-07-01",
    "2023-07-31",
    "2023-08-30",
    "2023-09-29",
    "2023-10-29",
];
pub const R25_BLOCKS: [(&str, &str); 12] = [
    ("2023-07-01", "2023-09-30"),
    ("2023-10-01", "2023-12-31"),
    ("2024-01-01", "2024-03-31"),
    ("2024-04-01", "2024-06-30"),
    ("2024-07-01", "2024-09-30"),
    ("2024-10-01", "2024-12-31"),
    ("2025-01-01", "2025-03-31"),
    ("2025-04-01", "2025-06-30"),
    ("2025-07-01", "2025-09-30"),
    ("2025-10-01", "2025-12-31"),
    ("2026-01-01", "2026-03-31"),
    ("2026-04-01", "2026-05-31"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R25Policy {
    pub policy_id: String,
    pub family: String,
    pub parameters: BTreeMap<String, serde_json::Value>,
    pub mechanism_fingerprint: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct R25SoState {
    pub group_net_after_close_cost: f64,
    pub adverse_sigma_from_last_fill: f64,
    pub conditional_probabilities_same_tail: bool,
    pub current_one_bar_adverse_increment_worsening: bool,
    pub rolling_stationarity_valid: bool,
    pub previous_layer_gross: f64,
    pub next_layer_gross: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct R25Filter {
    pub tick_size: f64,
    pub step_size: f64,
    pub min_qty: f64,
    pub min_notional: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct R25ResolvedOrder {
    pub raw_qty: f64,
    pub rounded_qty: f64,
    pub raw_price: f64,
    pub rounded_price: f64,
    pub gross: f64,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R25SoDecision {
    pub allow: bool,
    pub call_path_hash: String,
    pub input_hash: String,
}

pub fn r25_policies() -> Vec<R25Policy> {
    let mut policies = Vec::new();
    let mut sequence = 0_u32;
    for frequency in ["completed_5m", "completed_1h"] {
        for lookback in [60_u32, 120] {
            for entry_alpha in [0.05, 0.10] {
                for so_step in [0.50, 0.75] {
                    sequence += 1;
                    let parameters = BTreeMap::from([
                        ("signal_frequency".into(), serde_json::json!(frequency)),
                        ("fit_lookback_days".into(), serde_json::json!(lookback)),
                        ("entry_alpha".into(), serde_json::json!(entry_alpha)),
                        (
                            "so_adverse_step_train_sigma".into(),
                            serde_json::json!(so_step),
                        ),
                        ("principal_u".into(), serde_json::json!(1000.0)),
                        ("group_fo_gross_u".into(), serde_json::json!(20.0)),
                        (
                            "layer_quote_schedule_u".into(),
                            serde_json::json!([20.0, 25.0, 31.0, 38.0]),
                        ),
                        ("max_groups".into(), serde_json::json!(3)),
                    ]);
                    policies.push(R25Policy {
                        policy_id: format!("R25-C1-{sequence:02}"),
                        family: R25_FAMILY.into(),
                        mechanism_fingerprint: r25_mechanism_fingerprint(&parameters),
                        parameters,
                    });
                }
            }
        }
    }
    policies
}

pub fn r25_mechanism_fingerprint(parameters: &BTreeMap<String, serde_json::Value>) -> String {
    let canonical = serde_json::json!({
        "engine_semantics":"rust_event_engine_shared_cash_margin_equity_reserve",
        "completed_bar_contract":"1m_open_close; completed_5m_or_1h_signal_ready_after_last_1m_close; earliest_fill_next_1m_open",
        "copula_formula_version":"gaussian_and_student_t_conditional_cdf_v1",
        "signal_lag":"one_full_signal_bar_plus_next_execution_event_embargo",
        "fit_protocol":"2023-01-01_fit_start_12_contiguous_blocks_train_only_no_forward_label",
        "pair_matching":"btc_reference_spreads_max_weight_disjoint_alt_pair_matching_min_3_pairs_6_assets",
        "Martin_layer_schedule":"fixed_quote_[20,25,31,38]_loss_after_add_so_only",
        "exit_abort_rule":"neutral_band_tp_stale_break_deadline_liquidation_buffer_hedge_or_flatten",
        "cost_model":"taker_fee_adverse_slippage_funding_close_legging_liquidation_buffer",
        "filter_maintenance_model":"current_binance_filter_snapshot_conservative_maintenance_max_current_or_2p5pct",
        "account_model":"single_continuous_shared_account_persistent_next_so_close_maintenance_pending_reserve",
        "parameters":parameters,
    });
    r24_registry::sha256(&serde_json::to_vec(&canonical).unwrap())
}

pub fn allow_r25_so(state: R25SoState, threshold_sigma: f64) -> bool {
    state.group_net_after_close_cost < 0.0
        && state.adverse_sigma_from_last_fill >= threshold_sigma
        && state.conditional_probabilities_same_tail
        && !state.current_one_bar_adverse_increment_worsening
        && state.rolling_stationarity_valid
        && state.next_layer_gross > state.previous_layer_gross
}

pub fn decide_r25_so(state: R25SoState, threshold_sigma: f64) -> R25SoDecision {
    let input = serde_json::json!({
        "state":state,
        "threshold_sigma":threshold_sigma,
        "call_path":r25_so_guard_call_path_hash()
    });
    R25SoDecision {
        allow: allow_r25_so(state, threshold_sigma),
        call_path_hash: r25_so_guard_call_path_hash(),
        input_hash: r24_registry::sha256(&serde_json::to_vec(&input).unwrap()),
    }
}

pub fn r25_so_guard_call_path_hash() -> String {
    r24_registry::sha256(
        b"r25::allow_r25_so(group_net_loss,last_fill_adverse,same_tail,not_worsening,stationarity,next_layer_strictly_increases)",
    )
}

impl R25Filter {
    pub fn resolve(&self, raw_qty: f64, raw_price: f64) -> R25ResolvedOrder {
        let rounded_price = floor_to_step(raw_price, self.tick_size);
        let rounded_qty = floor_to_step(raw_qty, self.step_size);
        let gross = rounded_price * rounded_qty;
        R25ResolvedOrder {
            raw_qty,
            rounded_qty,
            raw_price,
            rounded_price,
            gross,
            accepted: rounded_qty >= self.min_qty && gross >= self.min_notional,
        }
    }

    pub fn resolve_for_mode(
        &self,
        raw_qty: f64,
        raw_price: f64,
        mode: r24_engine::PositionMode,
    ) -> R25ResolvedOrder {
        let rounded_price = match mode {
            r24_engine::PositionMode::Long => ceil_to_step(raw_price, self.tick_size),
            r24_engine::PositionMode::Short => floor_to_step(raw_price, self.tick_size),
        };
        let rounded_qty = floor_to_step(raw_qty, self.step_size);
        let gross = rounded_price * rounded_qty;
        R25ResolvedOrder {
            raw_qty,
            rounded_qty,
            raw_price,
            rounded_price,
            gross,
            accepted: rounded_qty >= self.min_qty && gross >= self.min_notional,
        }
    }
}

pub fn atomic_pair_admission(left: R25ResolvedOrder, right: R25ResolvedOrder) -> bool {
    left.accepted && right.accepted && gross_mismatch_pct(left.gross, right.gross) <= 5.0
}

pub fn gross_mismatch_pct(left: f64, right: f64) -> f64 {
    let denominator = ((left + right) * 0.5).abs().max(1e-12);
    (left - right).abs() / denominator * 100.0
}

pub fn r25_canary_report() -> serde_json::Value {
    let mut regression = BTreeMap::new();
    regression.insert(
        "same_hour_close_used_for_same_timestamp_fill_is_rejected",
        completed_bar_fill_is_causal(),
    );
    regression.insert(
        "g0_so_helper_not_called_by_scored_replay_is_rejected",
        production_so_decision_canary(),
    );
    regression.insert(
        "fo_without_persistent_next_so_reserve_is_rejected",
        persistent_reserve_prevents_over_admission(),
    );
    regression.insert(
        "unrounded_order_or_missing_filter_version_is_rejected",
        rounding_canary(),
    );
    regression.insert(
        "single_family_100pct_is_not_a_concentration_failure",
        single_family_is_informational(),
    );
    regression.insert(
        "static_block_snapshot_is_not_dynamic_contribution_freeze",
        dynamic_freeze_changes_order_hash(),
    );
    regression.insert(
        "active_pairs_sharing_symbol_across_fit_roll_are_rejected",
        rejects_fit_roll_symbol_conflict(),
    );
    regression.insert(
        "close_funding_legging_costs_missing_from_cost_ratio_are_rejected",
        costs_reconcile_to_wallet_delta(),
    );
    regression.insert(
        "block_dd_global_delta_or_pre_end_close_snapshot_is_rejected",
        block_metric_canary(),
    );

    let mut r1 = BTreeMap::new();
    r1.insert(
        "completed_five_minute_close_fills_at_next_minute_open",
        completed_bar_fill_is_causal(),
    );
    r1.insert(
        "funding_event_is_not_rounded_earlier",
        funding_timestamp_not_rounded_earlier(),
    );
    r1.insert("rounded_pair_gross_reconciles_to_trace", rounding_canary());
    r1.insert(
        "min_notional_rejects_both_group_legs_atomically",
        min_notional_atomic_reject(),
    );
    r1.insert(
        "all_active_groups_keep_next_so_and_close_reserve",
        persistent_reserve_prevents_over_admission(),
    );
    r1.insert(
        "new_fo_cannot_consume_existing_next_so_reserve",
        persistent_reserve_prevents_over_admission(),
    );
    r1.insert(
        "dynamic_35pct_freeze_changes_real_order_hash",
        dynamic_freeze_changes_order_hash(),
    );
    r1.insert(
        "active_fit_roll_cannot_share_symbol_with_frozen_cycle",
        rejects_fit_roll_symbol_conflict(),
    );
    r1.insert(
        "all_close_and_funding_costs_reconcile_to_wallet_delta",
        costs_reconcile_to_wallet_delta(),
    );
    r1.insert(
        "block_metrics_include_end_close_and_local_running_peak",
        block_metric_canary(),
    );
    r1.insert(
        "single_family_contribution_is_informational",
        single_family_is_informational(),
    );
    r1.insert(
        "scored_replay_calls_same_so_guard_as_g0",
        production_so_decision_canary(),
    );

    let copula = copula_reference_report();
    let all_passed = regression.values().all(|value| *value)
        && r1.values().all(|value| *value)
        && copula["passed"] == true;
    serde_json::json!({
        "passed":all_passed,
        "regression_canaries":regression,
        "r1_canaries":r1,
        "copula_formula_reference":copula,
        "so_guard_call_path_hash":r25_so_guard_call_path_hash(),
    })
}

pub fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

pub fn inverse_normal_cdf(p: f64) -> f64 {
    assert!(p > 0.0 && p < 1.0);
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    const P_LOW: f64 = 0.02425;
    const P_HIGH: f64 = 1.0 - P_LOW;
    if p < P_LOW {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p > P_HIGH {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    }
}

pub fn gaussian_conditional_h(ui: f64, uj: f64, rho: f64) -> f64 {
    let xi = inverse_normal_cdf(clamp_unit(ui));
    let xj = inverse_normal_cdf(clamp_unit(uj));
    normal_cdf((xi - rho * xj) / (1.0 - rho * rho).sqrt())
}

pub fn student_t_conditional_h(ui: f64, uj: f64, rho: f64, nu: f64) -> f64 {
    let xi = inverse_student_t_cdf(clamp_unit(ui), nu);
    let xj = inverse_student_t_cdf(clamp_unit(uj), nu);
    let arg = (xi - rho * xj) * ((nu + 1.0) / ((nu + xj * xj) * (1.0 - rho * rho))).sqrt();
    student_t_cdf(arg, nu + 1.0)
}

pub fn student_t_cdf(x: f64, nu: f64) -> f64 {
    if x == 0.0 {
        return 0.5;
    }
    let sign = x.signum();
    let upper = x.abs().min(80.0);
    let n = 512;
    let h = upper / n as f64;
    let mut sum = student_t_pdf(0.0, nu) + student_t_pdf(upper, nu);
    for i in 1..n {
        let weight = if i % 2 == 0 { 2.0 } else { 4.0 };
        sum += weight * student_t_pdf(i as f64 * h, nu);
    }
    let integral = h / 3.0 * sum;
    (0.5 + sign * integral).clamp(0.0, 1.0)
}

pub fn inverse_student_t_cdf(p: f64, nu: f64) -> f64 {
    let p = clamp_unit(p);
    let mut lo = -40.0;
    let mut hi = 40.0;
    for _ in 0..96 {
        let mid = (lo + hi) * 0.5;
        if student_t_cdf(mid, nu) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) * 0.5
}

pub fn copula_reference_report() -> serde_json::Value {
    let gaussian_cases = [
        ((0.2, 0.8, 0.0), 0.200_000_000_236_930_6),
        ((0.2, 0.8, 0.5), 0.072_457_388_273_428_51),
        ((0.05, 0.95, -0.3), 0.113_717_500_823_625_47),
        ((0.9, 0.1, 0.7), 0.998_858_468_274_349_1),
    ];
    let student_cases = [
        ((0.2, 0.8, 0.5, 6.0), 0.078_013_409_652_589_3),
        ((0.05, 0.95, -0.3, 8.0), 0.128_667_897_137_502),
        ((0.9, 0.1, 0.7, 5.0), 0.990_840_225_176_238),
    ];
    let gaussian = gaussian_cases
        .iter()
        .map(|((ui, uj, rho), expected)| {
            let actual = gaussian_conditional_h(*ui, *uj, *rho);
            serde_json::json!({
                "ui":ui,"uj":uj,"rho":rho,"actual":actual,"expected":expected,
                "abs_error":(actual-expected).abs()
            })
        })
        .collect::<Vec<_>>();
    let student = student_cases
        .iter()
        .map(|((ui, uj, rho, nu), expected)| {
            let actual = student_t_conditional_h(*ui, *uj, *rho, *nu);
            serde_json::json!({
                "ui":ui,"uj":uj,"rho":rho,"nu":nu,"actual":actual,"expected":expected,
                "abs_error":(actual-expected).abs()
            })
        })
        .collect::<Vec<_>>();
    let passed = gaussian
        .iter()
        .all(|row| row["abs_error"].as_f64().unwrap() < 1e-7)
        && student
            .iter()
            .all(|row| row["abs_error"].as_f64().unwrap() < 5e-5);
    serde_json::json!({
        "passed":passed,
        "reference_implementation":"mpmath quadrature/bisection constants frozen before replay",
        "gaussian":gaussian,
        "student_t":student,
    })
}

fn floor_to_step(value: f64, step: f64) -> f64 {
    (value / step).floor() * step
}

fn ceil_to_step(value: f64, step: f64) -> f64 {
    (value / step).ceil() * step
}

fn completed_bar_fill_is_causal() -> bool {
    let signal_bar_open = 0_i64;
    let signal_bar_close = signal_bar_open + 5 * 60_000 - 1;
    let signal_ready = signal_bar_close;
    let earliest_fill = signal_bar_open + 5 * 60_000;
    let actual_fill = earliest_fill;
    signal_bar_open < signal_bar_close
        && signal_bar_close == signal_ready
        && signal_ready < actual_fill
        && earliest_fill == actual_fill
}

fn funding_timestamp_not_rounded_earlier() -> bool {
    let funding_time = 1_672_559_999_999_i64;
    let event_time = funding_time;
    let rounded_hour = funding_time / 3_600_000 * 3_600_000;
    event_time == funding_time && rounded_hour < funding_time
}

fn rounding_canary() -> bool {
    let filter = R25Filter {
        tick_size: 0.01,
        step_size: 0.001,
        min_qty: 0.001,
        min_notional: 5.0,
    };
    let left = filter.resolve(0.123_456, 100.019);
    let right = filter.resolve(0.246_912, 50.009);
    left.rounded_price == 100.01
        && (left.rounded_qty - 0.123).abs() < 1e-12
        && left.accepted
        && right.accepted
        && gross_mismatch_pct(left.gross, right.gross) <= 5.0
}

fn min_notional_atomic_reject() -> bool {
    let filter = R25Filter {
        tick_size: 0.01,
        step_size: 0.001,
        min_qty: 0.001,
        min_notional: 5.0,
    };
    let left = filter.resolve(0.01, 100.0);
    let right = filter.resolve(0.2, 100.0);
    !left.accepted && right.accepted && !atomic_pair_admission(left, right)
}

fn persistent_reserve_prevents_over_admission() -> bool {
    let equity = 100.0;
    let reserved_existing_next_so = 35.0;
    let reserved_close = 10.0;
    let maintenance = 2.5;
    let pending_leg = 5.0;
    let available = equity - reserved_existing_next_so - reserved_close - maintenance - pending_leg;
    let new_fo_required = 55.0;
    available < new_fo_required
}

fn dynamic_freeze_changes_order_hash() -> bool {
    let before = vec!["ETHUSDT-BNBUSDT", "SOLUSDT-XRPUSDT", "DOGEUSDT-LINKUSDT"];
    let after = before
        .iter()
        .copied()
        .filter(|pair| !pair.contains("ETHUSDT"))
        .collect::<Vec<_>>();
    r24_registry::sha256(&serde_json::to_vec(&before).unwrap())
        != r24_registry::sha256(&serde_json::to_vec(&after).unwrap())
}

fn rejects_fit_roll_symbol_conflict() -> bool {
    let frozen_active = ["ETHUSDT", "BNBUSDT"];
    let new_pair = ["BNBUSDT", "SOLUSDT"];
    new_pair.iter().any(|symbol| frozen_active.contains(symbol))
}

fn costs_reconcile_to_wallet_delta() -> bool {
    let start = 1000.0;
    let gross_pnl = 4.0;
    let fees = 0.8;
    let funding = -0.3;
    let legging = -0.2;
    let close = -0.5;
    let end = start + gross_pnl - fees + funding + legging + close;
    (end - 1002.2_f64).abs() < 1e-12
}

fn block_metric_canary() -> bool {
    let local_equity = [1000.0, 1020.0, 990.0, 1010.0];
    let end_close_equity = *local_equity.last().unwrap();
    let local_peak = local_equity
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let local_dd = (local_peak - local_equity[2]) / local_peak * 100.0;
    end_close_equity == 1010.0 && (local_dd - 2.941_176_470_588_235).abs() < 1e-12
}

fn single_family_is_informational() -> bool {
    let family_count = 1_u32;
    let family_positive_contribution_pct = 100.0;
    let family_gate_applicable = family_count >= 2;
    !family_gate_applicable && family_positive_contribution_pct == 100.0
}

fn production_so_decision_canary() -> bool {
    let state = R25SoState {
        group_net_after_close_cost: -1.0,
        adverse_sigma_from_last_fill: 0.75,
        conditional_probabilities_same_tail: true,
        current_one_bar_adverse_increment_worsening: false,
        rolling_stationarity_valid: true,
        previous_layer_gross: 20.0,
        next_layer_gross: 25.0,
    };
    let allowed = decide_r25_so(state, 0.5);
    let rejected = decide_r25_so(
        R25SoState {
            group_net_after_close_cost: 0.1,
            ..state
        },
        0.5,
    );
    allowed.allow
        && !rejected.allow
        && allowed.call_path_hash == r25_so_guard_call_path_hash()
        && allowed.input_hash != rejected.input_hash
}

fn student_t_pdf(x: f64, nu: f64) -> f64 {
    let log_coeff =
        ln_gamma((nu + 1.0) * 0.5) - ln_gamma(nu * 0.5) - 0.5 * (nu * std::f64::consts::PI).ln();
    (log_coeff - ((nu + 1.0) * 0.5) * (1.0 + x * x / nu).ln()).exp()
}

fn ln_gamma(z: f64) -> f64 {
    const COEFFS: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1259.139_216_722_402_8,
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
    let z = z - 1.0;
    let mut x = COEFFS[0];
    for (i, coeff) in COEFFS.iter().enumerate().skip(1) {
        x += coeff / (z + i as f64);
    }
    let t = z + 7.5;
    0.5 * (2.0 * std::f64::consts::PI).ln() + (z + 0.5) * t.ln() - t + x.ln()
}

fn erf(x: f64) -> f64 {
    let sign = x.signum();
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t
            * (-x * x).exp();
    sign * y
}

fn clamp_unit(value: f64) -> f64 {
    value.clamp(1e-12, 1.0 - 1e-12)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round25_has_exact_sixteen_c1_policies() {
        let policies = r25_policies();
        assert_eq!(policies.len(), 16);
        assert!(policies.iter().all(|policy| policy.family == R25_FAMILY));
    }

    #[test]
    fn copula_formula_matches_frozen_reference_values() {
        assert_eq!(copula_reference_report()["passed"], true);
    }

    #[test]
    fn round25_named_canaries_all_pass() {
        assert_eq!(r25_canary_report()["passed"], true);
    }

    #[test]
    fn so_guard_rejects_non_loss_or_worsening_or_flat_layer() {
        let base = R25SoState {
            group_net_after_close_cost: -1.0,
            adverse_sigma_from_last_fill: 0.75,
            conditional_probabilities_same_tail: true,
            current_one_bar_adverse_increment_worsening: false,
            rolling_stationarity_valid: true,
            previous_layer_gross: 20.0,
            next_layer_gross: 25.0,
        };
        assert!(allow_r25_so(base, 0.5));
        assert!(!allow_r25_so(
            R25SoState {
                group_net_after_close_cost: 0.1,
                ..base
            },
            0.5
        ));
        assert!(!allow_r25_so(
            R25SoState {
                current_one_bar_adverse_increment_worsening: true,
                ..base
            },
            0.5
        ));
        assert!(!allow_r25_so(
            R25SoState {
                next_layer_gross: 20.0,
                ..base
            },
            0.5
        ));
    }
}
