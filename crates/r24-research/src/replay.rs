use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{bail, Result};
use chrono::NaiveDate;
use r24_engine::{
    EngineConfig, FillRequest, FrozenLeg, MarketType, MartinGroup, PositionKey, PositionMode,
    SharedAccount, SOFT_LADDER,
};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::residual::{fit_all_pairs, maximum_disjoint_matching, HourBar, PairFit};
use crate::{BLOCKS, UNIVERSE};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct F1Parameters {
    pub fit_lookback_days: i64,
    pub entry_z: f64,
    pub so_residual_sigma: f64,
    pub max_live_groups: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockResult {
    pub block: String,
    pub test_start: String,
    pub test_end: String,
    pub fit_cutoff: String,
    pub pair_count: usize,
    pub actual_assets_in_graph: usize,
    pub no_fit: bool,
    pub start_equity: f64,
    pub end_equity: f64,
    pub raw_return_pct: f64,
    pub max_equity_dd_pct: f64,
    pub fo_count: u64,
    pub so_count: u64,
    pub tp_count: u64,
    pub abort_count: u64,
    pub pair_graph: Vec<PairFit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    pub policy_id: String,
    pub parameters: F1Parameters,
    pub principal_u: f64,
    pub final_equity: f64,
    pub compounded_return_pct: f64,
    pub annualized_return_pct: f64,
    pub max_equity_drawdown_pct: f64,
    pub balance_drawdown_pct: f64,
    pub ddr: f64,
    pub actual_assets: Vec<String>,
    pub fo_count: u64,
    pub so_count: u64,
    pub loss_after_add_so_count: u64,
    pub tp_count: u64,
    pub abort_count: u64,
    pub liquidation: bool,
    pub positive_blocks: usize,
    pub blocks: Vec<BlockResult>,
    pub symbol_positive_contribution_pct: BTreeMap<String, f64>,
    pub group_positive_contribution_pct: BTreeMap<String, f64>,
    pub family_positive_contribution_pct: f64,
    pub max_symbol_contribution_pct: f64,
    pub max_group_contribution_pct: f64,
    pub max_block_contribution_pct: f64,
    pub cost_to_gross_profit_pct: f64,
    pub peak_effective_leverage: f64,
    pub p_a: bool,
    pub p_b: bool,
    pub immediate_fail_reasons: Vec<String>,
    pub trace_rows: Vec<r24_engine::TraceRecord>,
}

#[derive(Debug, Clone)]
struct ActiveGroup {
    id: String,
    fit: PairFit,
    long_y: bool,
    level: usize,
    last_filled_residual: f64,
    opened_at: i64,
    keys: [PositionKey; 2],
}

pub fn load_funding(
    database: &Path,
    start_ms: i64,
    end_ms: i64,
) -> Result<BTreeMap<i64, Vec<(String, f64)>>> {
    let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut output: BTreeMap<i64, Vec<(String, f64)>> = BTreeMap::new();
    for symbol in UNIVERSE {
        let mut statement = connection.prepare("SELECT funding_time,funding_rate FROM funding_rates WHERE symbol=?1 AND funding_time>=?2 AND funding_time<?3 ORDER BY funding_time")?;
        let mut rows = statement.query((symbol, start_ms, end_ms))?;
        while let Some(row) = rows.next()? {
            let timestamp: i64 = row.get(0)?;
            let rate: f64 = row.get(1)?;
            let hour = timestamp / 3_600_000 * 3_600_000;
            output.entry(hour).or_default().push((symbol.into(), rate));
        }
    }
    Ok(output)
}

pub fn run_f1_policy(
    policy_id: &str,
    parameters: F1Parameters,
    data: &BTreeMap<String, Vec<HourBar>>,
    funding: &BTreeMap<i64, Vec<(String, f64)>>,
) -> Result<ReplayResult> {
    let principal = 1000.0;
    let mut config = EngineConfig::default();
    config.maintenance_rate = 0.025;
    config.liquidation_fee_bps = 125.0;
    config.max_effective_leverage = 2.0;
    config.leverage = 2.0;
    let mut account = SharedAccount::new(principal, config)?;
    let price_maps = data
        .iter()
        .map(|(symbol, bars)| {
            (
                symbol.clone(),
                bars.iter()
                    .map(|bar| (bar.timestamp_ms, bar.close))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut active = BTreeMap::<String, ActiveGroup>::new();
    let mut actual_assets = BTreeSet::new();
    let mut contribution_by_symbol = BTreeMap::<String, f64>::new();
    let mut contribution_by_group = BTreeMap::<String, f64>::new();
    let mut block_positive = BTreeMap::<String, f64>::new();
    let mut total_gross_profit = 0.0;
    let mut total_cost = 0.0;
    let mut fo_count = 0;
    let mut so_count = 0;
    let mut loss_after_add_so_count = 0;
    let mut tp_count = 0;
    let mut abort_count = 0;
    let mut group_sequence = 0_u64;
    let mut peak_effective_leverage = 0.0_f64;
    let mut peak_balance = principal;
    let mut max_balance_drawdown_pct = 0.0_f64;
    let mut blocks = Vec::new();

    for (block_index, (test_start, test_end)) in BLOCKS.iter().enumerate() {
        let start_ms = date_ms(test_start);
        let end_exclusive = date_ms(test_end) + 86_400_000;
        let fit_cutoff_ms = start_ms - 121 * 86_400_000;
        let fit_start_ms = fit_cutoff_ms - parameters.fit_lookback_days * 86_400_000;
        let eligible = fit_all_pairs(data, fit_start_ms, fit_cutoff_ms, 16.0);
        let matching = maximum_disjoint_matching(&eligible, 3).unwrap_or_default();
        let no_fit = matching.len() < 3;
        let block_start_equity = account.equity();
        let block_start_dd = account.max_equity_drawdown_pct;
        let block_start_counts = (fo_count, so_count, tp_count, abort_count);
        account.transition_block(start_ms, &format!("tb{:02}", block_index + 1));
        let contribution_snapshot = contribution_by_group.clone();

        let mut timestamp = start_ms;
        while timestamp < end_exclusive && !account.terminated {
            let prices = UNIVERSE
                .iter()
                .filter_map(|symbol| {
                    price_maps[*symbol]
                        .get(&timestamp)
                        .copied()
                        .map(|price| ((*symbol).to_string(), price))
                })
                .collect::<BTreeMap<_, _>>();
            if prices.len() < UNIVERSE.len() {
                timestamp += 3_600_000;
                continue;
            }
            account.mark(timestamp, &prices)?;
            peak_balance = peak_balance.max(account.wallet_balance);
            if peak_balance > 0.0 {
                max_balance_drawdown_pct = max_balance_drawdown_pct
                    .max((peak_balance - account.wallet_balance) / peak_balance * 100.0);
            }
            peak_effective_leverage =
                peak_effective_leverage.max(account.gross_notional() / account.equity().max(1e-9));
            if let Some(events) = funding.get(&timestamp) {
                let keys = account.positions.keys().cloned().collect::<Vec<_>>();
                for (symbol, rate) in events {
                    for key in keys.iter().filter(|key| key.symbol == *symbol) {
                        account.apply_funding(timestamp, key, *rate)?;
                    }
                }
            }

            let active_ids = active.keys().cloned().collect::<Vec<_>>();
            for id in active_ids {
                let Some(group) = active.get(&id).cloned() else {
                    continue;
                };
                let y_price = prices[&group.fit.y_symbol];
                let x_price = prices[&group.fit.x_symbol];
                let residual = group.fit.residual(y_price, x_price);
                let z = (residual - group.fit.residual_mean) / group.fit.residual_sigma;
                let net = group_net_after_cost(&account, &group);
                if net > 0.0 {
                    let realized =
                        close_active_group(&mut account, &group, &prices, timestamp, "tp")?;
                    attribute_positive(
                        realized,
                        &group,
                        &mut contribution_by_symbol,
                        &mut contribution_by_group,
                        &mut block_positive,
                        test_start,
                    );
                    total_gross_profit += realized.max(0.0);
                    tp_count += 1;
                    active.remove(&id);
                    continue;
                }
                if z.abs() > 4.0 || timestamp - group.opened_at > 30 * 86_400_000 {
                    let realized =
                        close_active_group(&mut account, &group, &prices, timestamp, "abort")?;
                    attribute_positive(
                        realized,
                        &group,
                        &mut contribution_by_symbol,
                        &mut contribution_by_group,
                        &mut block_positive,
                        test_start,
                    );
                    abort_count += 1;
                    active.remove(&id);
                    continue;
                }
                let adverse = if group.long_y {
                    group.last_filled_residual - residual
                } else {
                    residual - group.last_filled_residual
                };
                if net < 0.0
                    && adverse >= parameters.so_residual_sigma * group.fit.residual_sigma
                    && group.level + 1 < SOFT_LADDER.len()
                {
                    let next_level = group.level + 1;
                    let layer_gross = 20.0 * SOFT_LADDER[next_level];
                    account
                        .groups
                        .get_mut(&id)
                        .unwrap()
                        .net_pnl_after_close_cost = net;
                    if account.request_next_so(&id, layer_gross).is_ok() {
                        account.consume_group_reserve(&id)?;
                        let (filled, cost) = fill_layer(
                            &mut account,
                            &group,
                            layer_gross,
                            &prices,
                            timestamp,
                            next_level,
                        )?;
                        total_cost += cost;
                        if filled {
                            let active_group = active.get_mut(&id).unwrap();
                            active_group.level = next_level;
                            active_group.last_filled_residual = residual;
                            so_count += 1;
                            loss_after_add_so_count += 1;
                        }
                    }
                }
            }

            if !no_fit && active.len() < parameters.max_live_groups {
                let mut candidates = matching
                    .iter()
                    .filter(|fit| {
                        !active.values().any(|group| {
                            group.fit.y_symbol == fit.y_symbol && group.fit.x_symbol == fit.x_symbol
                        })
                    })
                    .collect::<Vec<_>>();
                candidates.sort_by(|left, right| {
                    contribution_snapshot
                        .get(&pair_name(left))
                        .copied()
                        .unwrap_or(0.0)
                        .partial_cmp(
                            &contribution_snapshot
                                .get(&pair_name(right))
                                .copied()
                                .unwrap_or(0.0),
                        )
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                for fit in candidates {
                    if active.len() >= parameters.max_live_groups {
                        break;
                    }
                    let y_price = prices[&fit.y_symbol];
                    let x_price = prices[&fit.x_symbol];
                    let z = fit.zscore(y_price, x_price);
                    if z.abs() < parameters.entry_z {
                        continue;
                    }
                    group_sequence += 1;
                    let id = format!("{policy_id}-g{group_sequence:06}");
                    let long_y = z < 0.0;
                    let group = build_active_group(
                        &id,
                        fit.clone(),
                        long_y,
                        timestamp,
                        fit.residual(y_price, x_price),
                    );
                    account.add_group(MartinGroup {
                        group_id: id.clone(),
                        fit_version: format!("tb{:02}", block_index + 1),
                        frozen_legs: group
                            .keys
                            .iter()
                            .enumerate()
                            .map(|(index, key)| FrozenLeg {
                                key: key.clone(),
                                signed_weight: if index == 0 { 0.5 } else { -0.5 },
                            })
                            .collect(),
                        level: 0,
                        previous_level_gross: 0.0,
                        current_level_gross: 20.0,
                        last_filled_group_price: None,
                        net_pnl_after_close_cost: 0.0,
                        reserved_next_so: 0.0,
                    })?;
                    let (filled, cost) =
                        fill_layer(&mut account, &group, 20.0, &prices, timestamp, 0)?;
                    total_cost += cost;
                    if filled {
                        actual_assets.insert(fit.y_symbol.clone());
                        actual_assets.insert(fit.x_symbol.clone());
                        active.insert(id, group);
                        fo_count += 1;
                    } else {
                        account.remove_group(&id);
                    }
                }
            }
            timestamp += 3_600_000;
        }
        let end_equity = account.equity();
        blocks.push(BlockResult {
            block: format!("tb{:02}", block_index + 1),
            test_start: (*test_start).into(),
            test_end: (*test_end).into(),
            fit_cutoff: timestamp_date(fit_cutoff_ms),
            pair_count: matching.len(),
            actual_assets_in_graph: matching.len() * 2,
            no_fit,
            start_equity: block_start_equity,
            end_equity,
            raw_return_pct: (end_equity / block_start_equity.max(1e-9) - 1.0) * 100.0,
            max_equity_dd_pct: account.max_equity_drawdown_pct - block_start_dd,
            fo_count: fo_count - block_start_counts.0,
            so_count: so_count - block_start_counts.1,
            tp_count: tp_count - block_start_counts.2,
            abort_count: abort_count - block_start_counts.3,
            pair_graph: matching,
        });
    }

    let final_prices = UNIVERSE
        .iter()
        .filter_map(|symbol| {
            price_maps[*symbol]
                .range(..date_ms("2026-06-01"))
                .next_back()
                .map(|(_, price)| ((*symbol).to_string(), *price))
        })
        .collect::<BTreeMap<_, _>>();
    for group in active.values().cloned().collect::<Vec<_>>() {
        let realized = close_active_group(
            &mut account,
            &group,
            &final_prices,
            date_ms("2026-06-01") - 1,
            "end_close",
        )?;
        attribute_positive(
            realized,
            &group,
            &mut contribution_by_symbol,
            &mut contribution_by_group,
            &mut block_positive,
            "tb12_end",
        );
    }
    let final_equity = account.equity();
    let days = (date_ms("2026-06-01") - date_ms("2023-07-01")) as f64 / 86_400_000.0;
    let compounded = final_equity / principal - 1.0;
    let annualized = ((final_equity / principal).max(1e-9).powf(365.0 / days) - 1.0) * 100.0;
    let total_positive = contribution_by_group.values().sum::<f64>();
    let symbol_pct = contribution_percentages(&contribution_by_symbol, total_positive);
    let group_pct = contribution_percentages(&contribution_by_group, total_positive);
    let block_pct = contribution_percentages(&block_positive, total_positive);
    let max_symbol = symbol_pct.values().copied().fold(0.0, f64::max);
    let max_group = group_pct.values().copied().fold(0.0, f64::max);
    let max_block = block_pct.values().copied().fold(0.0, f64::max);
    let positive_blocks = blocks
        .iter()
        .filter(|block| block.raw_return_pct > 0.0)
        .count();
    let family_contribution = if total_positive > 0.0 { 100.0 } else { 0.0 };
    let mut immediate_fail_reasons = Vec::new();
    if account.terminated {
        immediate_fail_reasons.push("liquidation_or_principal_breach".into());
    }
    if actual_assets.len() < 5 {
        immediate_fail_reasons.push("actual_assets_below_5".into());
    }
    if loss_after_add_so_count == 0 {
        immediate_fail_reasons.push("no_loss_after_add_so".into());
    }
    if max_symbol > 50.0 {
        immediate_fail_reasons.push("symbol_positive_contribution_above_50pct".into());
    }
    if max_group > 50.0 {
        immediate_fail_reasons.push("group_positive_contribution_above_50pct".into());
    }
    if max_block > 50.0 {
        immediate_fail_reasons.push("block_positive_contribution_above_50pct".into());
    }
    if family_contribution > 50.0 {
        immediate_fail_reasons.push("family_positive_contribution_above_50pct".into());
    }
    let cost_ratio = if total_gross_profit > 0.0 {
        total_cost / total_gross_profit * 100.0
    } else {
        f64::INFINITY
    };
    if cost_ratio > 50.0 {
        immediate_fail_reasons.push("cost_to_gross_profit_above_50pct".into());
    }
    let p_a = !account.terminated;
    let p_b = p_a && compounded > 0.0 && positive_blocks >= 8 && immediate_fail_reasons.is_empty();
    Ok(ReplayResult {
        policy_id: policy_id.into(),
        parameters,
        principal_u: principal,
        final_equity,
        compounded_return_pct: compounded * 100.0,
        annualized_return_pct: annualized,
        max_equity_drawdown_pct: account.max_equity_drawdown_pct,
        balance_drawdown_pct: max_balance_drawdown_pct,
        ddr: if account.max_equity_drawdown_pct > 0.0 {
            annualized / account.max_equity_drawdown_pct
        } else {
            0.0
        },
        actual_assets: actual_assets.into_iter().collect(),
        fo_count,
        so_count,
        loss_after_add_so_count,
        tp_count,
        abort_count,
        liquidation: account.terminated,
        positive_blocks,
        blocks,
        symbol_positive_contribution_pct: symbol_pct,
        group_positive_contribution_pct: group_pct,
        family_positive_contribution_pct: family_contribution,
        max_symbol_contribution_pct: max_symbol,
        max_group_contribution_pct: max_group,
        max_block_contribution_pct: max_block,
        cost_to_gross_profit_pct: cost_ratio,
        peak_effective_leverage,
        p_a,
        p_b,
        immediate_fail_reasons,
        trace_rows: account.traces,
    })
}

fn build_active_group(
    id: &str,
    fit: PairFit,
    long_y: bool,
    timestamp: i64,
    last_filled_residual: f64,
) -> ActiveGroup {
    let y_mode = if long_y {
        PositionMode::Long
    } else {
        PositionMode::Short
    };
    let x_mode = if long_y {
        PositionMode::Short
    } else {
        PositionMode::Long
    };
    ActiveGroup {
        id: id.into(),
        last_filled_residual,
        fit: fit.clone(),
        long_y,
        level: 0,
        opened_at: timestamp,
        keys: [
            position_key(&fit.y_symbol, y_mode, id),
            position_key(&fit.x_symbol, x_mode, id),
        ],
    }
}

fn position_key(symbol: &str, mode: PositionMode, owner: &str) -> PositionKey {
    PositionKey {
        symbol: symbol.into(),
        market_type: MarketType::UsdMPerp,
        mode,
        owner_group: owner.into(),
    }
}

fn fill_layer(
    account: &mut SharedAccount,
    group: &ActiveGroup,
    gross: f64,
    prices: &BTreeMap<String, f64>,
    timestamp: i64,
    level: usize,
) -> Result<(bool, f64)> {
    let mut filled_keys = Vec::new();
    let mut cost = 0.0;
    let mut resolved = 0.0;
    for (index, key) in group.keys.iter().enumerate() {
        let price = prices[&key.symbol];
        let quantity = gross * 0.5 / price;
        let outcome = account.submit(FillRequest {
            timestamp,
            order_id: format!("{}-L{}-{}", group.id, level, index),
            group_id: group.id.clone(),
            key: key.clone(),
            requested_quantity: quantity,
            price,
            fill_fraction: 1.0,
            delayed_bars: 0,
            reject: false,
        })?;
        if !outcome.accepted {
            for prior in filled_keys {
                account.close_key(timestamp, &prior, prices[&prior.symbol], "hedge_or_flatten")?;
            }
            return Ok((false, cost));
        }
        resolved += outcome.filled_quantity * price;
        cost += outcome.fee;
        filled_keys.push(key.clone());
    }
    if (resolved - gross).abs() > 0.01 {
        bail!("resolved group gross mismatch");
    }
    Ok((true, cost))
}

fn group_net_after_cost(account: &SharedAccount, group: &ActiveGroup) -> f64 {
    group
        .keys
        .iter()
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

fn close_active_group(
    account: &mut SharedAccount,
    group: &ActiveGroup,
    prices: &BTreeMap<String, f64>,
    timestamp: i64,
    reason: &str,
) -> Result<f64> {
    let net = group_net_after_cost(account, group);
    for key in &group.keys {
        account.close_key(timestamp, key, prices[&key.symbol], reason)?;
    }
    account.remove_group(&group.id);
    Ok(net)
}

fn attribute_positive(
    value: f64,
    group: &ActiveGroup,
    symbols: &mut BTreeMap<String, f64>,
    groups: &mut BTreeMap<String, f64>,
    blocks: &mut BTreeMap<String, f64>,
    block: &str,
) {
    if value > 0.0 {
        *symbols.entry(group.fit.y_symbol.clone()).or_default() += value * 0.5;
        *symbols.entry(group.fit.x_symbol.clone()).or_default() += value * 0.5;
        *groups.entry(pair_name(&group.fit)).or_default() += value;
        *blocks.entry(block.into()).or_default() += value;
    }
}
fn pair_name(fit: &PairFit) -> String {
    format!("{}-{}", fit.y_symbol, fit.x_symbol)
}
fn contribution_percentages(values: &BTreeMap<String, f64>, total: f64) -> BTreeMap<String, f64> {
    values
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                if total > 0.0 {
                    value / total * 100.0
                } else {
                    0.0
                },
            )
        })
        .collect()
}
fn date_ms(value: &str) -> i64 {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}
fn timestamp_date(value: i64) -> String {
    chrono::DateTime::from_timestamp_millis(value)
        .unwrap()
        .date_naive()
        .to_string()
}
