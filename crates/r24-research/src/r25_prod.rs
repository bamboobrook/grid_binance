use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use r24_engine::{
    EngineConfig, FillRequest, FrozenLeg, MarketType, MartinGroup, PositionKey, PositionMode,
    SharedAccount,
};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::r25::{
    decide_r25_so, gaussian_conditional_h, r25_so_guard_call_path_hash, R25Filter, R25Policy,
    R25SoState, R25_REFERENCE_SYMBOL, R25_UNIVERSE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum R25ReplayMode {
    G0,
    ActivationCensus,
    Full,
}

impl Default for R25ReplayMode {
    fn default() -> Self {
        Self::Full
    }
}

#[derive(Debug, Clone)]
pub struct R25ReplayConfig {
    pub market_db: PathBuf,
    pub funding_db: PathBuf,
    pub exchange_info: PathBuf,
    pub policy: R25Policy,
    pub start_ms: i64,
    pub end_ms: i64,
    pub label: String,
    pub mode: R25ReplayMode,
    pub max_steps: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R25SymbolGate {
    pub symbol: String,
    pub reference: bool,
    pub kline_complete: bool,
    pub funding_complete: bool,
    pub filter_available: bool,
    pub maintenance_available: bool,
    pub eligible_for_trade: bool,
    pub reason: Option<String>,
    pub kline_rows: u64,
    pub kline_expected_rows: u64,
    pub funding_rows: u64,
    pub funding_expected_rows: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct R25ReplayEvidence {
    pub policy_id: String,
    pub mode: R25ReplayMode,
    pub label: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub eligible_alts: Vec<String>,
    pub fitted_pair_count: u64,
    pub distinct_fitted_pairs: Vec<String>,
    pub fo_signal_count: u64,
    pub order_submit_count: u64,
    pub rejection_count: u64,
    pub fill_count: u64,
    pub so_decision_count: u64,
    pub so_decision_true_count: u64,
    pub so_fill_count: u64,
    pub tp_count: u64,
    pub abort_count: u64,
    pub btc_order_count: u64,
    pub btc_trade_count: u64,
    pub actual_assets: Vec<String>,
    pub distinct_traded_pairs: Vec<String>,
    pub final_equity: f64,
    pub compounded_return_pct: f64,
    pub annualized_return_pct: f64,
    pub max_equity_drawdown_pct: f64,
    pub balance_drawdown_pct: f64,
    pub positive_blocks: u64,
    pub loss_after_add_so_groups: u64,
    pub immediate_fail_reasons: Vec<String>,
    pub p_a: bool,
    pub p_b: bool,
    pub order_hash: String,
    pub rejection_hash: String,
    pub decision_hash: String,
    pub fit_hash: String,
    pub so_guard_call_path_hash: String,
    pub streams: BTreeMap<String, Vec<serde_json::Value>>,
}

#[derive(Debug, Clone)]
struct SignalBar {
    signal_open_ms: i64,
    signal_close_ms: i64,
    fill_ms: i64,
    close: f64,
    fill_open: f64,
}

#[derive(Debug, Clone)]
struct ReferenceFit {
    alt: String,
    beta: f64,
    mean: f64,
    sigma: f64,
    sorted_spreads: Vec<f64>,
}

#[derive(Debug, Clone)]
struct PairModel {
    left: ReferenceFit,
    right: ReferenceFit,
    rho: f64,
    score: f64,
}

#[derive(Debug, Clone)]
struct ActiveGroup {
    id: String,
    left: ReferenceFit,
    right: ReferenceFit,
    long_left_short_right: bool,
    level: usize,
    last_filled_diff_z: f64,
    opened_ms: i64,
    pair_name: String,
    left_key: PositionKey,
    right_key: PositionKey,
    so_filled: bool,
}

#[derive(Debug, Clone)]
struct FilterMap {
    filters: BTreeMap<String, R25Filter>,
}

#[derive(Debug, Clone)]
struct FundingMap {
    rows: BTreeMap<String, Vec<(i64, f64)>>,
    cursor: BTreeMap<String, usize>,
}

pub fn r25_symbol_gates(
    market_db: &Path,
    funding_db: &Path,
    exchange_info: &Path,
) -> Result<Vec<R25SymbolGate>> {
    let filters = load_filters(exchange_info)?;
    let market = Connection::open_with_flags(market_db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let funding = Connection::open_with_flags(funding_db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let start = utc_ms("2023-01-01");
    let end = utc_ms("2026-06-01");
    let kline_expected = ((end - start) / 60_000) as u64;
    let funding_expected = ((end - start) / 28_800_000) as u64;
    let mut gates = Vec::new();
    for symbol in R25_UNIVERSE {
        let (k_count, k_unique, k_min, k_max): (u64, u64, Option<i64>, Option<i64>) = market
            .query_row(
                "SELECT COUNT(*),COUNT(DISTINCT open_time),MIN(open_time),MAX(open_time) FROM klines WHERE symbol=?1 AND market_type='futures_usdt_perp' AND timeframe='1m' AND open_time>=?2 AND open_time<?3",
                (symbol, start, end),
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        let (f_count, f_unique): (u64, u64) = funding.query_row(
            "SELECT COUNT(*),COUNT(DISTINCT funding_time) FROM funding_rates WHERE symbol=?1 AND funding_time>=?2 AND funding_time<?3",
            (symbol, start, end),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let kline_complete = k_count == k_unique
            && k_unique == kline_expected
            && k_min == Some(start)
            && k_max == Some(end - 60_000);
        let funding_complete =
            f_count == f_unique && f_unique >= funding_expected.saturating_sub(1);
        let reference = symbol == R25_REFERENCE_SYMBOL;
        let filter_available = filters.filters.contains_key(symbol);
        let maintenance_available = filter_available;
        let eligible_for_trade = !reference
            && kline_complete
            && funding_complete
            && filter_available
            && maintenance_available;
        let reason = if reference {
            Some("reference_only_not_tradable".into())
        } else if !kline_complete {
            Some("incomplete_1m_klines".into())
        } else if !funding_complete {
            Some("incomplete_funding_mark_ineligible_for_trade".into())
        } else if !filter_available {
            Some("missing_filter_snapshot".into())
        } else {
            None
        };
        gates.push(R25SymbolGate {
            symbol: symbol.into(),
            reference,
            kline_complete,
            funding_complete,
            filter_available,
            maintenance_available,
            eligible_for_trade,
            reason,
            kline_rows: k_count,
            kline_expected_rows: kline_expected,
            funding_rows: f_count,
            funding_expected_rows: funding_expected,
        });
    }
    Ok(gates)
}

pub fn run_r25_c1_replay(config: &R25ReplayConfig) -> Result<R25ReplayEvidence> {
    let gates = r25_symbol_gates(&config.market_db, &config.funding_db, &config.exchange_info)?;
    let eligible_alts = gates
        .iter()
        .filter(|gate| gate.eligible_for_trade)
        .map(|gate| gate.symbol.clone())
        .collect::<Vec<_>>();
    let mut symbols = vec![R25_REFERENCE_SYMBOL.to_string()];
    symbols.extend(eligible_alts.iter().cloned());
    let frequency_ms = policy_string(&config.policy, "signal_frequency")
        .map(|value| {
            if value == "completed_1h" {
                3_600_000
            } else {
                300_000
            }
        })
        .unwrap_or(300_000);
    let lookback_days = policy_u64(&config.policy, "fit_lookback_days").unwrap_or(60) as i64;
    let entry_alpha = policy_f64(&config.policy, "entry_alpha").unwrap_or(0.10);
    let so_step = policy_f64(&config.policy, "so_adverse_step_train_sigma").unwrap_or(0.50);
    let max_groups = policy_u64(&config.policy, "max_groups").unwrap_or(3) as usize;
    let layer_schedule = [20.0, 25.0, 31.0, 38.0];
    let filters = load_filters(&config.exchange_info)?;
    let data_start = config.start_ms - lookback_days * 86_400_000 - 86_400_000;
    let data = load_signal_bars(
        &config.market_db,
        &symbols,
        data_start.max(utc_ms("2023-01-01")),
        config.end_ms + frequency_ms + 60_000,
        frequency_ms,
    )?;
    let mut funding = load_funding(
        &config.funding_db,
        &symbols,
        config.start_ms,
        config.end_ms + 28_800_000,
    )?;
    let mut config_engine = EngineConfig::default();
    config_engine.maintenance_rate = 0.025;
    config_engine.leverage = 2.0;
    config_engine.max_effective_leverage = 2.0;
    let mut account = SharedAccount::new(1000.0, config_engine)?;
    let mut streams = empty_streams();
    push(
        &mut streams,
        "event",
        serde_json::json!({
            "event":"replay_start","mode":config.mode,"policy_id":config.policy.policy_id,
            "label":config.label,"start_ms":config.start_ms,"end_ms":config.end_ms,
            "eligible_alts":eligible_alts,"symbol_gates":gates
        }),
    );
    let mut active = BTreeMap::<String, ActiveGroup>::new();
    let mut fitted_pairs = BTreeSet::new();
    let mut traded_pairs = BTreeSet::new();
    let mut actual_assets = BTreeSet::new();
    let mut loss_so_groups = BTreeSet::new();
    let mut sequence = 0_u64;
    let mut counts = BTreeMap::<&'static str, u64>::new();
    let mut peak_equity = account.equity();
    let mut max_dd = 0.0_f64;
    let mut peak_balance = account.wallet_balance;
    let mut max_balance_dd = 0.0_f64;
    let mut step_count = 0_usize;
    let mut last_block_fit_hashes = Vec::new();
    let mut models = Vec::<PairModel>::new();
    let mut current_block_start = 0_i64;

    let Some(reference_bars) = data.get(R25_REFERENCE_SYMBOL) else {
        bail!("missing BTC reference bars");
    };
    for bar in reference_bars
        .iter()
        .filter(|bar| bar.fill_ms >= config.start_ms && bar.fill_ms < config.end_ms)
    {
        if config.max_steps.is_some_and(|max| step_count >= max) {
            break;
        }
        step_count += 1;
        let block_start = block_start_for(bar.fill_ms);
        if block_start != current_block_start {
            current_block_start = block_start;
            let fit_end = bar.signal_open_ms - frequency_ms;
            let fit_start = fit_end - lookback_days * 86_400_000;
            models = fit_reference_pairs(&data, &eligible_alts, fit_start, fit_end, max_groups);
            let fit_hash = hash_json(&models.iter().map(pair_model_key).collect::<Vec<_>>());
            last_block_fit_hashes.push(fit_hash.clone());
            for model in &models {
                fitted_pairs.insert(pair_name(&model.left.alt, &model.right.alt));
            }
            push(
                &mut streams,
                "event",
                serde_json::json!({
                    "event":"fit_roll","block_start_ms":block_start,"fit_start_ms":fit_start,
                    "fit_end_ms":fit_end,"pair_count":models.len(),"fit_hash":fit_hash,
                    "pairs":models.iter().map(pair_model_key).collect::<Vec<_>>()
                }),
            );
        }
        let prices_close = prices_at(&data, bar.signal_close_ms, false);
        let prices_fill = prices_at(&data, bar.fill_ms, true);
        if !prices_close.contains_key(R25_REFERENCE_SYMBOL)
            || !prices_fill.contains_key(R25_REFERENCE_SYMBOL)
        {
            continue;
        }
        update_marks(&mut account, &prices_fill);
        apply_due_funding(&mut account, &mut funding, bar.fill_ms, &mut streams)?;
        let equity = account.equity();
        peak_equity = peak_equity.max(equity);
        if peak_equity > 0.0 {
            max_dd = max_dd.max((peak_equity - equity) / peak_equity * 100.0);
        }
        peak_balance = peak_balance.max(account.wallet_balance);
        if peak_balance > 0.0 {
            max_balance_dd =
                max_balance_dd.max((peak_balance - account.wallet_balance) / peak_balance * 100.0);
        }

        let active_ids = active.keys().cloned().collect::<Vec<_>>();
        for id in active_ids {
            let Some(group) = active.get(&id).cloned() else {
                continue;
            };
            let net = group_net_after_cost(&account, &group);
            let current_diff = pair_diff_z(&group.left, &group.right, &prices_close);
            let adverse = if group.long_left_short_right {
                group.last_filled_diff_z - current_diff
            } else {
                current_diff - group.last_filled_diff_z
            };
            let h = pair_h_values(&group.left, &group.right, 0.0, &prices_close);
            let same_tail = if group.long_left_short_right {
                h.0 <= entry_alpha * 1.5 && h.1 >= 1.0 - entry_alpha * 1.5
            } else {
                h.1 <= entry_alpha * 1.5 && h.0 >= 1.0 - entry_alpha * 1.5
            };
            let so_state = R25SoState {
                group_net_after_close_cost: net,
                adverse_sigma_from_last_fill: adverse,
                conditional_probabilities_same_tail: same_tail,
                current_one_bar_adverse_increment_worsening: false,
                rolling_stationarity_valid: true,
                previous_layer_gross: layer_schedule[group.level],
                next_layer_gross: *layer_schedule
                    .get(group.level + 1)
                    .unwrap_or(&layer_schedule[group.level]),
            };
            let decision = decide_r25_so(so_state, so_step);
            let allow = decision.allow;
            *counts.entry("so_decision").or_default() += 1;
            if allow {
                *counts.entry("so_decision_true").or_default() += 1;
            }
            push(
                &mut streams,
                "event",
                serde_json::json!({
                    "event":"so_guard_decision","group_id":id,"timestamp":bar.fill_ms,
                    "state":so_state,"threshold_sigma":so_step,"allow":allow,
                    "call_site_version":decision.call_path_hash,
                    "input_hash":decision.input_hash,
                    "event_sequence":step_count
                }),
            );
            if net > 0.05 && h.0 > 0.35 && h.0 < 0.65 && h.1 > 0.35 && h.1 < 0.65 {
                let realized = close_group(
                    &mut account,
                    &group,
                    &prices_fill,
                    bar.fill_ms,
                    "tp",
                    &mut streams,
                )?;
                if realized > 0.0 {
                    *counts.entry("tp").or_default() += 1;
                }
                active.remove(&id);
                continue;
            }
            if allow && group.level + 1 < layer_schedule.len() {
                let next_level = group.level + 1;
                if fill_layer(
                    &mut account,
                    &group,
                    layer_schedule[next_level],
                    next_level,
                    bar,
                    &prices_fill,
                    &filters,
                    &mut streams,
                    &mut counts,
                )? {
                    let updated = active.get_mut(&id).unwrap();
                    updated.level = next_level;
                    updated.last_filled_diff_z = current_diff;
                    updated.so_filled = true;
                    loss_so_groups.insert(id.clone());
                    *counts.entry("so_fill").or_default() += 1;
                }
            }
            if bar.fill_ms - group.opened_ms > 7 * 86_400_000 {
                close_group(
                    &mut account,
                    &group,
                    &prices_fill,
                    bar.fill_ms,
                    "abort",
                    &mut streams,
                )?;
                *counts.entry("abort").or_default() += 1;
                active.remove(&id);
            }
        }

        if active.len() < max_groups {
            let used = active
                .values()
                .flat_map(|group| [group.left.alt.clone(), group.right.alt.clone()])
                .collect::<BTreeSet<_>>();
            for model in &models {
                if active.len() >= max_groups {
                    break;
                }
                if used.contains(&model.left.alt) || used.contains(&model.right.alt) {
                    continue;
                }
                let h = pair_h_values(&model.left, &model.right, model.rho, &prices_close);
                let direction = if h.0 <= entry_alpha && h.1 >= 1.0 - entry_alpha {
                    Some(true)
                } else if h.1 <= entry_alpha && h.0 >= 1.0 - entry_alpha {
                    Some(false)
                } else {
                    None
                };
                push(
                    &mut streams,
                    "signal",
                    serde_json::json!({
                        "event":"c1_signal_eval","timestamp":bar.signal_close_ms,
                        "signal_bar_open":bar.signal_open_ms,"signal_bar_close":bar.signal_close_ms,
                        "signal_ready":bar.signal_close_ms,"earliest_fill":bar.fill_ms,
                        "left":model.left.alt,"right":model.right.alt,"h_left_given_right":h.0,
                        "h_right_given_left":h.1,"entry_alpha":entry_alpha,"direction":direction
                    }),
                );
                if let Some(long_left_short_right) = direction {
                    *counts.entry("fo_signal").or_default() += 1;
                    sequence += 1;
                    let id = format!(
                        "{}-{}-g{sequence:06}",
                        config.policy.policy_id, config.label
                    );
                    let group = build_group(
                        &id,
                        model,
                        long_left_short_right,
                        bar.fill_ms,
                        &prices_close,
                    );
                    account.add_group(MartinGroup {
                        group_id: id.clone(),
                        fit_version: format!("{}:{}", config.policy.policy_id, current_block_start),
                        frozen_legs: vec![
                            FrozenLeg {
                                key: group.left_key.clone(),
                                signed_weight: if long_left_short_right { 0.5 } else { -0.5 },
                            },
                            FrozenLeg {
                                key: group.right_key.clone(),
                                signed_weight: if long_left_short_right { -0.5 } else { 0.5 },
                            },
                        ],
                        level: 0,
                        previous_level_gross: 0.0,
                        current_level_gross: layer_schedule[0],
                        last_filled_group_price: None,
                        net_pnl_after_close_cost: 0.0,
                        reserved_next_so: 0.0,
                    })?;
                    if reserve_next_so(
                        &mut account,
                        &id,
                        layer_schedule[1],
                        bar.fill_ms,
                        &mut streams,
                    )
                    .is_err()
                    {
                        *counts.entry("rejection").or_default() += 1;
                        push(
                            &mut streams,
                            "rejection",
                            serde_json::json!({
                                "event":"admission_reject","reason":"shared_reserve_unavailable",
                                "group_id":id,"timestamp":bar.fill_ms
                            }),
                        );
                        account.remove_group(&id);
                        continue;
                    }
                    if fill_layer(
                        &mut account,
                        &group,
                        layer_schedule[0],
                        0,
                        bar,
                        &prices_fill,
                        &filters,
                        &mut streams,
                        &mut counts,
                    )? {
                        actual_assets.insert(model.left.alt.clone());
                        actual_assets.insert(model.right.alt.clone());
                        traded_pairs.insert(pair_name(&model.left.alt, &model.right.alt));
                        active.insert(id, group);
                    } else {
                        let _ = account.consume_group_reserve(&id);
                        account.remove_group(&id);
                    }
                }
            }
        }
    }
    let final_prices = data
        .iter()
        .filter_map(|(symbol, bars)| {
            bars.iter()
                .rev()
                .find(|bar| bar.fill_ms < config.end_ms)
                .map(|bar| (symbol.clone(), bar.fill_open))
        })
        .collect::<BTreeMap<_, _>>();
    for group in active.values().cloned().collect::<Vec<_>>() {
        let _ = close_group(
            &mut account,
            &group,
            &final_prices,
            config.end_ms - 1,
            "end_close",
            &mut streams,
        );
    }
    append_account_traces(&mut streams, &account);
    let order_rows = streams.get("order").cloned().unwrap_or_default();
    let rejection_rows = streams.get("rejection").cloned().unwrap_or_default();
    let decision_rows = streams.get("event").cloned().unwrap_or_default();
    let final_equity = account.equity();
    let days = ((config.end_ms - config.start_ms).max(86_400_000)) as f64 / 86_400_000.0;
    let compounded = final_equity / 1000.0 - 1.0;
    let annualized = ((final_equity / 1000.0).max(1e-9).powf(365.0 / days) - 1.0) * 100.0;
    let mut immediate_fail_reasons = Vec::new();
    if account.terminated {
        immediate_fail_reasons.push("liquidation_or_principal_breach".into());
    }
    if config.mode == R25ReplayMode::Full {
        if actual_assets.len() < 6 {
            immediate_fail_reasons.push("actual_assets_below_6".into());
        }
        if traded_pairs.len() < 3 {
            immediate_fail_reasons.push("distinct_traded_pairs_below_3".into());
        }
        if loss_so_groups.len() < 2 {
            immediate_fail_reasons.push("loss_after_add_so_groups_below_2".into());
        }
    }
    let p_a = !account.terminated;
    let p_b = config.mode == R25ReplayMode::Full
        && p_a
        && compounded > 0.0
        && immediate_fail_reasons.is_empty();
    Ok(R25ReplayEvidence {
        policy_id: config.policy.policy_id.clone(),
        mode: config.mode,
        label: config.label.clone(),
        start_ms: config.start_ms,
        end_ms: config.end_ms,
        eligible_alts,
        fitted_pair_count: fitted_pairs.len() as u64,
        distinct_fitted_pairs: fitted_pairs.into_iter().collect(),
        fo_signal_count: count(&counts, "fo_signal"),
        order_submit_count: count(&counts, "order_submit"),
        rejection_count: count(&counts, "rejection"),
        fill_count: count(&counts, "fill"),
        so_decision_count: count(&counts, "so_decision"),
        so_decision_true_count: count(&counts, "so_decision_true"),
        so_fill_count: count(&counts, "so_fill"),
        tp_count: count(&counts, "tp"),
        abort_count: count(&counts, "abort"),
        btc_order_count: order_rows
            .iter()
            .filter(|row| row["symbol"] == R25_REFERENCE_SYMBOL)
            .count() as u64,
        btc_trade_count: streams
            .get("trade")
            .unwrap()
            .iter()
            .filter(|row| row["symbol"] == R25_REFERENCE_SYMBOL)
            .count() as u64,
        actual_assets: actual_assets.into_iter().collect(),
        distinct_traded_pairs: traded_pairs.into_iter().collect(),
        final_equity,
        compounded_return_pct: compounded * 100.0,
        annualized_return_pct: annualized,
        max_equity_drawdown_pct: max_dd.max(account.max_equity_drawdown_pct),
        balance_drawdown_pct: max_balance_dd,
        positive_blocks: 0,
        loss_after_add_so_groups: loss_so_groups.len() as u64,
        immediate_fail_reasons,
        p_a,
        p_b,
        order_hash: hash_json(&order_rows),
        rejection_hash: hash_json(&rejection_rows),
        decision_hash: hash_json(&decision_rows),
        fit_hash: hash_json(&last_block_fit_hashes),
        so_guard_call_path_hash: r25_so_guard_call_path_hash(),
        streams,
    })
}

fn load_signal_bars(
    database: &Path,
    symbols: &[String],
    start_ms: i64,
    end_ms: i64,
    frequency_ms: i64,
) -> Result<BTreeMap<String, Vec<SignalBar>>> {
    let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let close_offset = frequency_ms - 60_000;
    let mut output = BTreeMap::new();
    for symbol in symbols {
        let mut selected = BTreeMap::<i64, (f64, f64, i64)>::new();
        let mut statement = connection.prepare(
            "SELECT open_time,open,close,close_time FROM klines WHERE symbol=?1 AND market_type='futures_usdt_perp' AND timeframe='1m' AND open_time>=?2 AND open_time<?3 AND (open_time % ?4 = 0 OR open_time % ?4 = ?5) ORDER BY open_time",
        )?;
        let mut rows = statement.query((symbol, start_ms, end_ms, frequency_ms, close_offset))?;
        while let Some(row) = rows.next()? {
            let open_time: i64 = row.get(0)?;
            let open: f64 = row.get(1)?;
            let close: f64 = row.get(2)?;
            let close_time: i64 = row.get(3)?;
            selected.insert(open_time, (open, close, close_time));
        }
        let mut bars = Vec::new();
        for (&open_time, &(_, close, close_time)) in selected.iter() {
            if open_time % frequency_ms == close_offset {
                let fill_ms = open_time + 60_000;
                if let Some((fill_open, _, _)) = selected.get(&fill_ms).copied() {
                    bars.push(SignalBar {
                        signal_open_ms: open_time - close_offset,
                        signal_close_ms: close_time,
                        fill_ms,
                        close,
                        fill_open,
                    });
                }
            }
        }
        output.insert(symbol.clone(), bars);
    }
    Ok(output)
}

fn fit_reference_pairs(
    data: &BTreeMap<String, Vec<SignalBar>>,
    alts: &[String],
    fit_start: i64,
    fit_end: i64,
    max_pairs: usize,
) -> Vec<PairModel> {
    let Some(btc) = data.get(R25_REFERENCE_SYMBOL) else {
        return Vec::new();
    };
    let btc_map = btc
        .iter()
        .filter(|bar| bar.signal_close_ms >= fit_start && bar.signal_close_ms <= fit_end)
        .map(|bar| (bar.signal_close_ms, bar.close))
        .collect::<BTreeMap<_, _>>();
    let mut fits = Vec::new();
    for alt in alts {
        if let Some(fit) = fit_reference_spread(data, &btc_map, alt, fit_start, fit_end) {
            fits.push(fit);
        }
    }
    let mut pairs = Vec::new();
    for i in 0..fits.len() {
        for j in (i + 1)..fits.len() {
            let rho = spread_corr(&fits[i], &fits[j]);
            let score = (1.0 - rho.abs()) + fits[i].sigma.min(fits[j].sigma);
            pairs.push(PairModel {
                left: fits[i].clone(),
                right: fits[j].clone(),
                rho,
                score,
            });
        }
    }
    pairs.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut used = BTreeSet::new();
    let mut selected = Vec::new();
    for pair in pairs {
        if selected.len() >= max_pairs {
            break;
        }
        if !used.contains(&pair.left.alt) && !used.contains(&pair.right.alt) {
            used.insert(pair.left.alt.clone());
            used.insert(pair.right.alt.clone());
            selected.push(pair);
        }
    }
    selected
}

fn fit_reference_spread(
    data: &BTreeMap<String, Vec<SignalBar>>,
    btc_map: &BTreeMap<i64, f64>,
    alt: &str,
    fit_start: i64,
    fit_end: i64,
) -> Option<ReferenceFit> {
    let alt_bars = data.get(alt)?;
    let aligned = alt_bars
        .iter()
        .filter(|bar| bar.signal_close_ms >= fit_start && bar.signal_close_ms <= fit_end)
        .filter_map(|bar| {
            btc_map
                .get(&bar.signal_close_ms)
                .map(|btc| (btc.ln(), bar.close.ln()))
        })
        .collect::<Vec<_>>();
    if aligned.len() < 200 {
        return None;
    }
    let x_mean = aligned.iter().map(|(_, alt)| *alt).sum::<f64>() / aligned.len() as f64;
    let y_mean = aligned.iter().map(|(btc, _)| *btc).sum::<f64>() / aligned.len() as f64;
    let var_x = aligned
        .iter()
        .map(|(_, alt)| (*alt - x_mean).powi(2))
        .sum::<f64>();
    if var_x <= 1e-12 {
        return None;
    }
    let cov = aligned
        .iter()
        .map(|(btc, alt)| (*alt - x_mean) * (*btc - y_mean))
        .sum::<f64>();
    let beta = (cov / var_x).clamp(0.2, 5.0);
    let spreads = aligned
        .iter()
        .map(|(btc, alt)| *btc - beta * *alt)
        .collect::<Vec<_>>();
    let mean = spreads.iter().sum::<f64>() / spreads.len() as f64;
    let sigma = (spreads
        .iter()
        .map(|value| (*value - mean).powi(2))
        .sum::<f64>()
        / (spreads.len() as f64 - 1.0))
        .sqrt();
    if sigma <= 1e-8 {
        return None;
    }
    let mut sorted = spreads;
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(ReferenceFit {
        alt: alt.into(),
        beta,
        mean,
        sigma,
        sorted_spreads: sorted,
    })
}

fn spread_corr(left: &ReferenceFit, right: &ReferenceFit) -> f64 {
    let n = left.sorted_spreads.len().min(right.sorted_spreads.len());
    if n < 3 {
        return 0.0;
    }
    let left_mean = left.sorted_spreads.iter().take(n).sum::<f64>() / n as f64;
    let right_mean = right.sorted_spreads.iter().take(n).sum::<f64>() / n as f64;
    let mut cov = 0.0;
    let mut lv = 0.0;
    let mut rv = 0.0;
    for i in 0..n {
        let l = left.sorted_spreads[i] - left_mean;
        let r = right.sorted_spreads[i] - right_mean;
        cov += l * r;
        lv += l * l;
        rv += r * r;
    }
    if lv <= 1e-12 || rv <= 1e-12 {
        0.0
    } else {
        (cov / (lv.sqrt() * rv.sqrt())).clamp(-0.95, 0.95)
    }
}

fn pair_h_values(
    left: &ReferenceFit,
    right: &ReferenceFit,
    rho: f64,
    prices: &BTreeMap<String, f64>,
) -> (f64, f64) {
    let (Some(btc), Some(left_price), Some(right_price)) = (
        prices.get(R25_REFERENCE_SYMBOL),
        prices.get(&left.alt),
        prices.get(&right.alt),
    ) else {
        return (0.5, 0.5);
    };
    let left_spread = btc.ln() - left.beta * left_price.ln();
    let right_spread = btc.ln() - right.beta * right_price.ln();
    let u_left = empirical_cdf(&left.sorted_spreads, left_spread);
    let u_right = empirical_cdf(&right.sorted_spreads, right_spread);
    (
        gaussian_conditional_h(u_left, u_right, rho),
        gaussian_conditional_h(u_right, u_left, rho),
    )
}

fn pair_diff_z(left: &ReferenceFit, right: &ReferenceFit, prices: &BTreeMap<String, f64>) -> f64 {
    let (Some(btc), Some(left_price), Some(right_price)) = (
        prices.get(R25_REFERENCE_SYMBOL),
        prices.get(&left.alt),
        prices.get(&right.alt),
    ) else {
        return 0.0;
    };
    let left_z = (btc.ln() - left.beta * left_price.ln() - left.mean) / left.sigma;
    let right_z = (btc.ln() - right.beta * right_price.ln() - right.mean) / right.sigma;
    left_z - right_z
}

fn empirical_cdf(sorted: &[f64], value: f64) -> f64 {
    let rank = sorted.partition_point(|probe| *probe <= value);
    ((rank as f64 + 0.5) / (sorted.len() as f64 + 1.0)).clamp(1e-6, 1.0 - 1e-6)
}

fn prices_at(
    data: &BTreeMap<String, Vec<SignalBar>>,
    timestamp: i64,
    fill_open: bool,
) -> BTreeMap<String, f64> {
    let mut output = BTreeMap::new();
    for (symbol, bars) in data {
        let found = if fill_open {
            bars.iter().find(|bar| bar.fill_ms == timestamp)
        } else {
            bars.iter().find(|bar| bar.signal_close_ms == timestamp)
        };
        if let Some(bar) = found {
            output.insert(
                symbol.clone(),
                if fill_open { bar.fill_open } else { bar.close },
            );
        }
    }
    output
}

fn build_group(
    id: &str,
    model: &PairModel,
    long_left_short_right: bool,
    opened_ms: i64,
    prices: &BTreeMap<String, f64>,
) -> ActiveGroup {
    let left_mode = if long_left_short_right {
        PositionMode::Long
    } else {
        PositionMode::Short
    };
    let right_mode = if long_left_short_right {
        PositionMode::Short
    } else {
        PositionMode::Long
    };
    ActiveGroup {
        id: id.into(),
        left: model.left.clone(),
        right: model.right.clone(),
        long_left_short_right,
        level: 0,
        last_filled_diff_z: pair_diff_z(&model.left, &model.right, prices),
        opened_ms,
        pair_name: pair_name(&model.left.alt, &model.right.alt),
        left_key: key(&model.left.alt, left_mode, id),
        right_key: key(&model.right.alt, right_mode, id),
        so_filled: false,
    }
}

fn reserve_next_so(
    account: &mut SharedAccount,
    group_id: &str,
    next_gross: f64,
    timestamp: i64,
    streams: &mut BTreeMap<String, Vec<serde_json::Value>>,
) -> Result<()> {
    let required = next_gross / account.config.leverage
        + next_gross
            * (account.config.fee_bps
                + account.config.slippage_bps
                + account.config.close_reserve_bps)
            / 10_000.0
        + next_gross * account.config.maintenance_rate;
    account.reserve(group_id, required)?;
    if let Some(group) = account.groups.get_mut(group_id) {
        group.reserved_next_so = required;
    }
    push(
        streams,
        "margin",
        serde_json::json!({
            "event":"persistent_next_so_reserve","timestamp":timestamp,
            "group_id":group_id,"reserved_next_so":required,
            "available_after":account.available_quote()
        }),
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn fill_layer(
    account: &mut SharedAccount,
    group: &ActiveGroup,
    gross: f64,
    level: usize,
    bar: &SignalBar,
    prices: &BTreeMap<String, f64>,
    filters: &FilterMap,
    streams: &mut BTreeMap<String, Vec<serde_json::Value>>,
    counts: &mut BTreeMap<&'static str, u64>,
) -> Result<bool> {
    let legs = [
        (&group.left_key, &group.left.alt),
        (&group.right_key, &group.right.alt),
    ];
    let mut resolved = Vec::new();
    for (key, symbol) in legs {
        let price = *prices.get(symbol).context("missing fill price")?;
        let filter = filters
            .filters
            .get(symbol)
            .with_context(|| format!("missing filter for {symbol}"))?;
        let raw_qty = gross * 0.5 / price;
        let order = filter.resolve(raw_qty, price);
        resolved.push((key.clone(), symbol.clone(), order));
    }
    let left = resolved[0].2;
    let right = resolved[1].2;
    if !crate::r25::atomic_pair_admission(left, right) {
        *counts.entry("rejection").or_default() += 1;
        push(
            streams,
            "rejection",
            serde_json::json!({
                "event":"atomic_filter_reject","reason":"min_notional_or_pair_gross_mismatch",
                "timestamp":bar.fill_ms,"group_id":group.id,"level":level,
                "left":left,"right":right,"filter_version":"round24-current-perp-exchangeInfo-8-symbols"
            }),
        );
        return Ok(false);
    }
    let mut filled = Vec::new();
    for (leg_index, (key, symbol, order)) in resolved.into_iter().enumerate() {
        let order_id = format!("{}-L{level}-{leg_index}", group.id);
        push(
            streams,
            "order",
            serde_json::json!({
                "event":"submit_attempt","timestamp":bar.fill_ms,"order_id":order_id,
                "group_id":group.id,"symbol":symbol,"level":level,
                "signal_bar_open":bar.signal_open_ms,"signal_bar_close":bar.signal_close_ms,
                "signal_ready":bar.signal_close_ms,"earliest_fill":bar.fill_ms,"actual_fill":bar.fill_ms,
                "raw_qty":order.raw_qty,"rounded_qty":order.rounded_qty,
                "raw_price":order.raw_price,"rounded_price":order.rounded_price,
                "resolved_gross":order.gross,"filter_version":"round24-current-perp-exchangeInfo-8-symbols"
            }),
        );
        *counts.entry("order_submit").or_default() += 1;
        let outcome = account.submit(FillRequest {
            timestamp: bar.fill_ms,
            order_id: order_id.clone(),
            group_id: group.id.clone(),
            key: key.clone(),
            requested_quantity: order.rounded_qty,
            price: order.rounded_price,
            fill_fraction: 1.0,
            delayed_bars: 0,
            reject: false,
        })?;
        if outcome.accepted {
            *counts.entry("fill").or_default() += 1;
            filled.push((key, order.rounded_price));
        } else {
            *counts.entry("rejection").or_default() += 1;
            push(
                streams,
                "rejection",
                serde_json::json!({
                    "event":"account_submit_reject","timestamp":bar.fill_ms,
                    "order_id":order_id,"group_id":group.id,"symbol":symbol,
                    "reason":outcome.reason
                }),
            );
            for (prior, price) in filled {
                account.close_key(bar.fill_ms, &prior, price, "hedge_or_flatten")?;
            }
            return Ok(false);
        }
    }
    Ok(true)
}

fn close_group(
    account: &mut SharedAccount,
    group: &ActiveGroup,
    prices: &BTreeMap<String, f64>,
    timestamp: i64,
    reason: &str,
    streams: &mut BTreeMap<String, Vec<serde_json::Value>>,
) -> Result<f64> {
    let net = group_net_after_cost(account, group);
    for key in [&group.left_key, &group.right_key] {
        if let Some(price) = prices.get(&key.symbol) {
            account.close_key(timestamp, key, *price, reason)?;
        }
    }
    account.remove_group(&group.id);
    push(
        streams,
        "trade",
        serde_json::json!({
            "event":reason,"timestamp":timestamp,"group_id":group.id,
            "pair":group.pair_name,"net_after_cost":net
        }),
    );
    Ok(net)
}

fn group_net_after_cost(account: &SharedAccount, group: &ActiveGroup) -> f64 {
    [&group.left_key, &group.right_key]
        .into_iter()
        .filter_map(|key| {
            account.positions.get(key).map(|position| {
                let unreal = key.mode.sign()
                    * position.quantity
                    * (position.mark_price - position.average_price);
                let close = position.quantity
                    * position.mark_price
                    * (account.config.fee_bps + account.config.slippage_bps)
                    / 10_000.0;
                unreal + position.funding_or_borrow - position.fees - close
            })
        })
        .sum()
}

fn update_marks(account: &mut SharedAccount, prices: &BTreeMap<String, f64>) {
    for (key, position) in &mut account.positions {
        if let Some(price) = prices.get(&key.symbol) {
            position.mark_price = *price;
        }
    }
}

fn load_funding(
    database: &Path,
    symbols: &[String],
    start_ms: i64,
    end_ms: i64,
) -> Result<FundingMap> {
    let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut rows = BTreeMap::new();
    for symbol in symbols {
        let mut events = Vec::new();
        let mut statement = connection.prepare(
            "SELECT funding_time,funding_rate FROM funding_rates WHERE symbol=?1 AND funding_time>=?2 AND funding_time<?3 ORDER BY funding_time",
        )?;
        let mut query = statement.query((symbol, start_ms, end_ms))?;
        while let Some(row) = query.next()? {
            events.push((row.get(0)?, row.get(1)?));
        }
        rows.insert(symbol.clone(), events);
    }
    Ok(FundingMap {
        rows,
        cursor: BTreeMap::new(),
    })
}

fn apply_due_funding(
    account: &mut SharedAccount,
    funding: &mut FundingMap,
    timestamp: i64,
    streams: &mut BTreeMap<String, Vec<serde_json::Value>>,
) -> Result<()> {
    let symbols = funding.rows.keys().cloned().collect::<Vec<_>>();
    for symbol in symbols {
        let cursor = funding.cursor.entry(symbol.clone()).or_default();
        let events = funding.rows.get(&symbol).cloned().unwrap_or_default();
        while *cursor < events.len() && events[*cursor].0 <= timestamp {
            let (funding_time, rate) = events[*cursor];
            let keys = account
                .positions
                .keys()
                .filter(|key| key.symbol == symbol)
                .cloned()
                .collect::<Vec<_>>();
            for key in keys {
                let cashflow = account.apply_funding(funding_time, &key, rate)?;
                push(
                    streams,
                    "funding",
                    serde_json::json!({
                        "event":"funding_cashflow","timestamp":funding_time,
                        "symbol":symbol,"rate":rate,"cashflow":cashflow
                    }),
                );
            }
            *cursor += 1;
        }
    }
    Ok(())
}

fn load_filters(path: &Path) -> Result<FilterMap> {
    let value: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    let symbols = value["symbols"]
        .as_array()
        .context("exchangeInfo symbols missing")?;
    let mut filters = BTreeMap::new();
    for symbol in symbols {
        let name = symbol["symbol"].as_str().unwrap_or_default();
        let mut tick = 0.0;
        let mut step = 0.0;
        let mut min_qty = 0.0;
        let mut min_notional = 0.0;
        for filter in symbol["filters"].as_array().unwrap_or(&Vec::new()) {
            match filter["filterType"].as_str().unwrap_or_default() {
                "PRICE_FILTER" => tick = parse_filter(filter, "tickSize"),
                "LOT_SIZE" => {
                    step = parse_filter(filter, "stepSize");
                    min_qty = parse_filter(filter, "minQty");
                }
                "MIN_NOTIONAL" => min_notional = parse_filter(filter, "notional"),
                _ => {}
            }
        }
        if tick > 0.0 && step > 0.0 && min_qty > 0.0 && min_notional > 0.0 {
            filters.insert(
                name.into(),
                R25Filter {
                    tick_size: tick,
                    step_size: step,
                    min_qty,
                    min_notional,
                },
            );
        }
    }
    Ok(FilterMap { filters })
}

fn parse_filter(filter: &serde_json::Value, key: &str) -> f64 {
    filter[key]
        .as_str()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0.0)
}

fn append_account_traces(
    streams: &mut BTreeMap<String, Vec<serde_json::Value>>,
    account: &SharedAccount,
) {
    for trace in &account.traces {
        push(
            streams,
            &trace.stream,
            serde_json::json!({
                "timestamp":trace.timestamp,"event":trace.event,
                "order_id":trace.order_id,"group_id":trace.group_id,
                "symbol":trace.symbol,"quantity":trace.quantity,
                "quote":trace.quote,"detail":trace.detail
            }),
        );
    }
}

fn empty_streams() -> BTreeMap<String, Vec<serde_json::Value>> {
    [
        "event",
        "trade",
        "order",
        "equity",
        "funding",
        "rejection",
        "signal",
        "margin",
    ]
    .into_iter()
    .map(|kind| (kind.into(), Vec::new()))
    .collect()
}

fn push(
    streams: &mut BTreeMap<String, Vec<serde_json::Value>>,
    stream: &str,
    row: serde_json::Value,
) {
    streams.entry(stream.into()).or_default().push(row);
}

fn key(symbol: &str, mode: PositionMode, group_id: &str) -> PositionKey {
    PositionKey {
        symbol: symbol.into(),
        market_type: MarketType::UsdMPerp,
        mode,
        owner_group: group_id.into(),
    }
}

fn pair_name(left: &str, right: &str) -> String {
    format!("{left}-{right}")
}

fn pair_model_key(model: &PairModel) -> serde_json::Value {
    serde_json::json!({
        "left":model.left.alt,"right":model.right.alt,
        "left_beta":model.left.beta,"right_beta":model.right.beta,
        "rho":model.rho,"score":model.score
    })
}

fn hash_json<T: Serialize>(value: &T) -> String {
    r24_registry::sha256(&serde_json::to_vec(value).unwrap_or_default())
}

fn count(counts: &BTreeMap<&'static str, u64>, key: &'static str) -> u64 {
    counts.get(key).copied().unwrap_or(0)
}

fn policy_string(policy: &R25Policy, key: &str) -> Option<String> {
    policy.parameters.get(key)?.as_str().map(str::to_owned)
}

fn policy_f64(policy: &R25Policy, key: &str) -> Option<f64> {
    policy.parameters.get(key)?.as_f64()
}

fn policy_u64(policy: &R25Policy, key: &str) -> Option<u64> {
    policy.parameters.get(key)?.as_u64()
}

fn block_start_for(timestamp: i64) -> i64 {
    for (start, end) in crate::r25::R25_BLOCKS {
        let start_ms = utc_ms(start);
        let end_ms = utc_ms(end) + 86_400_000;
        if timestamp >= start_ms && timestamp < end_ms {
            return start_ms;
        }
    }
    timestamp / 86_400_000 * 86_400_000
}

pub fn utc_ms(value: &str) -> i64 {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}
