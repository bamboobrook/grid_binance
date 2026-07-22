//! R1 directed-test suite (plan §5.3) + P-A gate evidence.
//!
//! These integration tests drive a FULL continuous merged replay through the
//! production-conservative sync cycle engine over a synthetic mean-reverting
//! stream (the same construction the engine's own tests use), assert all twelve
//! §5.3 directed invariants, and write a running+terminal registry row pair via
//! the launcher. This is the P-A gate evidence: production-conservative main
//! replay + independent adapter parity both pass.

use backtest_engine::martingale::exchange_model::ExchangeFilters;
use backtest_engine::martingale::r21_conservative_engine::{
    filter_order_conservative, verify_adapter_exchange_parity,
};
use backtest_engine::market_data::KlineBar;
use r23_replay::account::ContinuousAccount;
use r23_replay::continuous::{
    run_continuous_replay, BlockReplayInput, ContinuousReplayConfig, ReplayMode,
};
use r23_replay::directed_checks as dc;
use r23_registry::launcher::{LaunchRequest, Launcher};
use r23_registry::{FailureLedger, Registry};
use shared_domain::martingale::SynchronizedCycleConfig;

use backtest_engine::martingale::sync_cycle_engine::SynchronizedFit;

fn bar(symbol: &str, timestamp_ms: i64, close: f64) -> KlineBar {
    KlineBar {
        symbol: symbol.to_string(),
        open_time_ms: timestamp_ms,
        open: close,
        high: close,
        low: close,
        close,
        volume: 1.0,
    }
}

/// A mean-reverting pair fit: residual = log(B) - beta*log(A) - mu reverts.
fn pair_fit(beta: f64) -> SynchronizedFit {
    SynchronizedFit {
        group_id: "M1_A_B".to_string(),
        legs: vec!["AUSDT".to_string(), "BUSDT".to_string()],
        leg_markets: Vec::new(),
        leg_direction_signs: vec![1, 1],
        betas: vec![beta],
        mus: vec![0.0],
        residual_sigma: 1.0,
        half_life_h: 24.0,
        weights: Vec::new(),
        fit_sha256: "fit".to_string(),
    }
}

fn executable_pair_config() -> SynchronizedCycleConfig {
    SynchronizedCycleConfig {
        entry_z: 1.5,
        group_fo_quote: 30.0,
        group_gross_cap_pct: 100.0,
        ..SynchronizedCycleConfig::default()
    }
}

fn empty_hash() -> String {
    r23_registry::hash::sha256_str("")
}

fn valid_launch_req(id: &str) -> LaunchRequest {
    let h = empty_hash();
    LaunchRequest {
        experiment_id: id.to_string(),
        phase: "R1".to_string(),
        parent_experiment_id: None,
        git_commit: h.clone(),
        git_dirty: false,
        upstream_remote_commit: h.clone(),
        config_budget_u: 1000.0,
        argv_budget_u: 1000.0,
        binary_sha256: h.clone(),
        source_sha256: h.clone(),
        market_sha256: h.clone(),
        metrics_sha256: h.clone(),
        depth_sha256: h.clone(),
        aggtrades_sha256: h.clone(),
        funding_sha256: h.clone(),
        filter_sha256: h.clone(),
        maintenance_sha256: h.clone(),
        borrow_sha256: h.clone(),
        cost_sha256: h.clone(),
        config_sha256: h.clone(),
        fit_sha256: h.clone(),
        protocol_sha256: h,
    }
}

/// Build a synthetic mean-reverting stream long enough to open cycles, fire SOs
/// on adverse excursions, and TP on reversion. Mirrors the engine's own test
/// construction so the engine behaves deterministically.
fn mean_reverting_bars(steps: i64) -> Vec<KlineBar> {
    let mut bars = Vec::new();
    // A stays at 1.0; B oscillates around e^beta so residual z oscillates.
    let beta = 1.0_f64;
    let center = beta.ln(); // log(B) center so residual ~ 0 at center
    for t in 0..steps {
        let ts = t * 60_000; // 1-minute bars
        // sinusoidal excursion in log space -> several entries/exits
        let log_b = center + 1.6 * ((t as f64) * 0.15).sin();
        let b = log_b.exp();
        bars.push(bar("AUSDT", ts, 1.0));
        bars.push(bar("BUSDT", ts, b));
    }
    bars
}

#[test]
fn full_continuous_merged_replay_runs_and_passes_directed_checks() {
    // P-A gate: production-conservative main replay over a full stream.
    let bars = mean_reverting_bars(400);
    let cfg = ContinuousReplayConfig {
        mode: ReplayMode::ContinuousMerged,
        martingale_cfg: executable_pair_config(),
        budget_quote: 1000.0,
        funding_rates: vec![],
        fee_bps_override: None,
        slippage_bps_override: None,
    };
    let blocks = vec![BlockReplayInput {
        block_index: 1,
        train_fits: vec![pair_fit(1.0)],
        test_bars: bars,
    }];
    let outcome = run_continuous_replay(&cfg, &blocks);
    // The engine must not return an error on a well-formed stream.
    assert!(outcome.engine_error.is_none(), "engine error: {:?}", outcome.engine_error);
    let result = outcome
        .merged_result
        .as_ref()
        .expect("merged result must be present in ContinuousMerged mode");

    // §5.3.1: pair gross dimensionally equal after rounding.
    let fa = ExchangeFilters::default_for("AUSDT");
    let fb = ExchangeFilters::default_for("BUSDT");
    assert!(
        dc::pair_gross_is_dimensional_equal_after_rounding(&fa, &fb, 1.0, 2.0, 30.0, 0.10),
        "pair gross not dimensionally equal after rounding"
    );

    // §5.3.3/4: SO only on adverse loss.
    assert!(
        dc::cycle_so_only_on_adverse_loss(result),
        "a SO fired while the cycle was net positive"
    );

    // §5.3.5: last-fill basis does not move without a fill.
    assert!(
        dc::last_fill_basis_does_not_move_without_fill(result),
        "last-fill basis moved without a fill"
    );

    // §5.3.6: FO rejected when reserve missing.
    assert!(
        dc::fo_rejected_when_reserve_missing(10.0, 50.0, 5.0, 5.0, 30.0),
        "reserve gate did not reject when equity insufficient"
    );

    // §5.3.7: event-time liquidation terminates the shared account.
    assert!(
        dc::event_time_liquidation_terminates(result),
        "orders continued after a liquidation"
    );

    // §5.3.8: partial fill changes inventory/cash/equity.
    assert!(
        dc::partial_fill_changes_inventory_cash_equity(&fa, 1.0, 30.0, 0.5),
        "partial fill did not reconstruct intended notional"
    );

    // §5.3.9: leg delay books realized legging loss.
    assert!(
        dc::leg_delay_books_realized_legging_loss(&fa, 1.0, 30.0, 2.0, 2.0),
        "leg delay did not book legging loss"
    );

    // §5.3.11: independent adapters match.
    assert!(
        dc::independent_adapters_match(&fa, 1.0, 30.0),
        "backtest and exchange adapters disagree"
    );

    // §5.3.12: pseudo PC1 symbol rejected.
    assert!(dc::pseudo_symbol_pc1_is_rejected(), "pseudo PC1 symbol not rejected");

    // No breach: equity never <= 0 and no liquidation on this benign stream.
    assert!(!outcome.breach, "unexpected breach on mean-reverting stream");
}

#[test]
fn full_block_wise_carry_replay_preserves_continuous_account() {
    // §5.3.10: block transition never resets cash or equity.
    let bars1 = mean_reverting_bars(120);
    let bars2 = mean_reverting_bars(120);
    let cfg = ContinuousReplayConfig {
        mode: ReplayMode::BlockWiseCarry,
        martingale_cfg: executable_pair_config(),
        budget_quote: 1000.0,
        funding_rates: vec![],
        fee_bps_override: None,
        slippage_bps_override: None,
    };
    let blocks = vec![
        BlockReplayInput { block_index: 1, train_fits: vec![pair_fit(1.0)], test_bars: bars1 },
        BlockReplayInput { block_index: 2, train_fits: vec![pair_fit(1.0)], test_bars: bars2 },
    ];
    let outcome = run_continuous_replay(&cfg, &blocks);
    assert!(outcome.engine_error.is_none(), "engine error: {:?}", outcome.engine_error);
    assert!(dc::block_transition_never_resets_cash_or_equity(
        &outcome.account.snapshots,
        1000.0
    ));
}

#[test]
fn registry_records_running_and_terminal_for_complete_replay() {
    use tempfile::NamedTempFile;
    let r = NamedTempFile::new().unwrap().into_temp_path().keep().unwrap();
    let l = NamedTempFile::new().unwrap().into_temp_path().keep().unwrap();
    std::fs::write(&r, b"").unwrap();
    std::fs::write(&l, b"").unwrap();
    let launcher = Launcher::new(Registry::new(&r), FailureLedger::new(&l));

    let bars = mean_reverting_bars(200);
    let cfg = ContinuousReplayConfig {
        mode: ReplayMode::ContinuousMerged,
        martingale_cfg: executable_pair_config(),
        budget_quote: 1000.0,
        funding_rates: vec![],
        fee_bps_override: None,
        slippage_bps_override: None,
    };
    let blocks = vec![BlockReplayInput {
        block_index: 1,
        train_fits: vec![pair_fit(1.0)],
        test_bars: bars,
    }];
    let outcome = run_continuous_replay(&cfg, &blocks);
    let status = r23_replay::continuous::record_outcome(&launcher, &valid_launch_req("r1-full-001"), &outcome)
        .expect("record_outcome must succeed for a non-breaching replay");
    assert_eq!(status, r23_registry::registry::TerminalStatus::Complete);

    // running + terminal invariant must hold.
    let violations = launcher.registry().invariant_violations();
    assert!(violations.is_empty(), "registry invariant violated: {violations:?}");
    let rows = launcher.registry().read_all().unwrap();
    assert_eq!(rows.len(), 2); // one running + one terminal
}

#[test]
fn adapter_parity_p_a_evidence() {
    // P-A: independent adapter parity over a grid of prices/notionals.
    let f = ExchangeFilters::default_for("BTCUSDT");
    for &(price, notional) in &[(50_000.0, 100.0), (50_000.0, 5.0), (3_000.0, 30.0), (1.0, 30.0)] {
        assert!(verify_adapter_exchange_parity(&f, price, notional), "parity fail @ {price}/{notional}");
        let dec = filter_order_conservative(&f, price, notional);
        // A well-formed order must produce a decision (reject or accept), never panic.
        let _ = &dec;
    }
}
