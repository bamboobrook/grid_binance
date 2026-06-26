use std::collections::BTreeMap;

use rust_decimal::prelude::ToPrimitive;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleEntryTrigger, MartingalePortfolioConfig,
    MartingaleStopLossModel, MartingaleStrategyConfig, MartingaleTakeProfitModel,
};

use crate::market_data::KlineBar;
use crate::martingale::exit_rules::{
    evaluate_exit_priority, take_profit_price, weighted_average_entry, ExitDecision,
};
use crate::martingale::capital::{effective_leverage, leg_notional_series};
use crate::martingale::indicator_runtime::{latest_atr_for_strategy, IndicatorRuntimeContext};
use crate::martingale::metrics::{
    build_drawdown_curve, calculate_annualized_return_pct, EquityPoint,
    MartingaleBacktestEvent, MartingaleBacktestResult, MartingaleMetrics, MartingaleTradeDetail,
};
use crate::martingale::rules::compute_leg_trigger_prices;
use crate::martingale::state::MartingaleLegState;

const DEFAULT_EXCHANGE_MIN_NOTIONAL: f64 = 0.0;
const DEFAULT_FEE_BPS: f64 = 4.5;
const DEFAULT_SLIPPAGE_BPS: f64 = 2.0;

#[derive(Debug, Clone, PartialEq)]
pub struct FundingRatePoint {
    pub symbol: String,
    pub funding_time_ms: i64,
    pub funding_rate: f64,
    pub mark_price: Option<f64>,
}

pub fn run_kline_screening(
    portfolio: MartingalePortfolioConfig,
    bars: &[KlineBar],
) -> Result<MartingaleBacktestResult, String> {
    run_kline_screening_with_funding(portfolio, bars, &[])
}

pub fn run_kline_screening_with_funding(
    portfolio: MartingalePortfolioConfig,
    bars: &[KlineBar],
    funding_rates: &[FundingRatePoint],
) -> Result<MartingaleBacktestResult, String> {
    portfolio.validate()?;

    let preflight_rejections = preflight_rejection_reasons(&portfolio);
    if !preflight_rejections.is_empty() {
        return Ok(rejected_result(preflight_rejections));
    }

    let mut strategy_states = portfolio
        .strategies
        .iter()
        .map(StrategyRuntime::new)
        .collect::<Result<Vec<_>, _>>()?;

    // Equity base and return denominator is planned MARGIN capital (sum of leg
    // margins across the portfolio) — not the leveraged notional and not the
    // raw max_global_budget allocation. See `martingale::capital`.
    let initial_margin_capital: f64 = strategy_states
        .iter()
        .map(|state| state.margins.iter().sum::<f64>())
        .sum();
    validate_positive_f64("initial_margin_capital", initial_margin_capital)?;

    let mut events = Vec::new();
    let mut equity_curve = Vec::new();
    let mut rejection_reasons = Vec::new();
    let mut trade_count = 0_u64;
    let mut stop_count = 0_u64;
    let mut realized_pnl_quote = 0.0_f64;
    let mut capital_used_quote = 0.0_f64;
    let mut max_capital_used_quote = 0.0_f64;
    let mut equity_peak_quote = initial_margin_capital;
    let mut max_drawdown_pct = 0.0_f64;
    let mut last_equity_quote = initial_margin_capital;
    let mut latest_close_by_symbol = BTreeMap::new();
    let mut indicator_context = IndicatorRuntimeContext::default();

    let mut total_fee_quote = 0.0_f64;
    let mut total_slippage_quote = 0.0_f64;
    let mut total_funding_quote = 0.0_f64;
    let mut sorted_funding_rates = funding_rates.to_vec();
    sorted_funding_rates.sort_by(|left, right| {
        left.funding_time_ms
            .cmp(&right.funding_time_ms)
            .then_with(|| left.symbol.cmp(&right.symbol))
    });
    let mut funding_index = 0_usize;

    let mut bar_index = 0;
    while bar_index < bars.len() {
        let timestamp_ms = bars[bar_index].open_time_ms;
        let group_start = bar_index;
        while bar_index < bars.len() && bars[bar_index].open_time_ms == timestamp_ms {
            validate_bar(&bars[bar_index])?;
            latest_close_by_symbol.insert(bars[bar_index].symbol.clone(), bars[bar_index].close);
            indicator_context.push_bar(&bars[bar_index]);
            bar_index += 1;
        }
        let group = &bars[group_start..bar_index];

        // 方向2: 组合级动态降仓 — 计算当前回撤（基于上一 bar 结束时的 equity）
        let portfolio_drawdown_pct = if equity_peak_quote > 0.0 {
            ((equity_peak_quote - last_equity_quote) / equity_peak_quote * 100.0).max(0.0)
        } else {
            0.0
        };

        while funding_index < sorted_funding_rates.len()
            && sorted_funding_rates[funding_index].funding_time_ms <= timestamp_ms
        {
            let funding = &sorted_funding_rates[funding_index];
            if funding.funding_rate.is_finite() {
                let funding_quote = apply_funding_rate(
                    &mut strategy_states,
                    funding,
                    &latest_close_by_symbol,
                    &mut events,
                )?;
                if funding_quote != 0.0 {
                    total_funding_quote += funding_quote;
                    realized_pnl_quote += funding_quote;
                }
            }
            funding_index += 1;
        }

        for bar in group {
            let mut state_index = 0;
            while state_index < strategy_states.len() {
                if strategy_states[state_index].strategy.symbol != bar.symbol
                    || strategy_states[state_index].new_legs_blocked
                {
                    state_index += 1;
                    continue;
                }

                if strategy_states[state_index].legs.is_empty() {
                    // 方向2: 组合级动态降仓 — 回撤超 6% 时暂停新 cycle
                    if portfolio_drawdown_pct > 6.0 {
                        state_index += 1;
                        continue;
                    }
                    // 方向3: 波动率过滤 — ATR > 2% of price 时暂停新 cycle（高波动期风险大）
                    {
                        let atr_val = latest_atr_for_strategy(
                            &mut indicator_context,
                            &strategy_states[state_index].strategy,
                        );
                        if let Some(atr) = atr_val {
                            let atr_pct = atr / bar.close * 100.0;
                            if atr_pct > 2.0 {
                                state_index += 1;
                                continue;
                            }
                        }
                    }
                    if !entry_triggers_allow_entry(
                        &strategy_states[state_index],
                        bar,
                        &mut indicator_context,
                    )? {
                        state_index += 1;
                        continue;
                    }

                    let margin = strategy_states[state_index].margins[0];
                    let notional = strategy_states[state_index].notionals[0];
                    let entry_cost = trading_cost_quote(notional);
                    total_fee_quote += entry_cost.fee_quote;
                    total_slippage_quote += entry_cost.slippage_quote;
                    let capital_required = margin + entry_cost.total();
                    if let Some(reason) = budget_rejection_reason(
                        &portfolio,
                        &strategy_states,
                        state_index,
                        capital_used_quote,
                        capital_required,
                    )? {
                        reject_budget(
                            &mut rejection_reasons,
                            &mut events,
                            bar,
                            &mut strategy_states[state_index],
                            reason,
                        );
                        state_index += 1;
                        continue;
                    }
                    let latest_atr = latest_atr_for_strategy(
                        &mut indicator_context,
                        strategy_states[state_index].strategy,
                    );
                    add_leg(
                        &mut strategy_states[state_index],
                        0,
                        bar.open,
                        margin,
                        notional,
                        latest_atr,
                    )?;
                    capital_used_quote += capital_required;
                    trade_count += 1;
                    max_capital_used_quote = max_capital_used_quote.max(capital_used_quote);
                    events.push(event(
                        bar,
                        &strategy_states[state_index],
                        "entry",
                        format!(
                            "margin_quote={margin};notional_quote={notional};leverage={};fee_quote={};slippage_quote={}",
                            strategy_states[state_index].strategy.leverage.unwrap_or(1),
                            entry_cost.fee_quote,
                            entry_cost.slippage_quote
                        ),
                    ));
                }

                if let Some(next_leg_index) = strategy_states[state_index].next_leg_index() {
                    let trigger_price =
                        strategy_states[state_index].trigger_prices[next_leg_index - 1];
                    if safety_order_triggered(
                        strategy_states[state_index].strategy.direction,
                        bar,
                        trigger_price,
                    ) {
                        // B-2 趋势过滤：ADX > 45%（极端趋势）时跳过加仓。
                        // 仅在极端趋势中跳过（保留大部分平均加仓机会），避免 B-2 v1（用入场阈值18-30%）太激进。
                        // 仅对有 ADX indicator 的策略生效；无 ADX 的策略不受影响。
                        {
                            let strategy = &strategy_states[state_index].strategy;
                            let adx_period = strategy.indicators.iter().find_map(|i| match i {
                                shared_domain::martingale::MartingaleIndicatorConfig::Adx {
                                    period,
                                } => Some(*period as usize),
                                _ => None,
                            });
                            if let Some(period) = adx_period {
                                if let Some(adx) =
                                    indicator_context.latest_adx(&strategy.symbol, period)
                                {
                                    if adx > 45.0 {
                                        // ADX > 45% = 极端趋势 → 跳过此 safety order
                                        state_index += 1;
                                        continue;
                                    }
                                }
                            }
                        }
                        let margin = strategy_states[state_index].margins[next_leg_index];
                        let notional = strategy_states[state_index].notionals[next_leg_index];
                        let entry_cost = trading_cost_quote(notional);
                        total_fee_quote += entry_cost.fee_quote;
                        total_slippage_quote += entry_cost.slippage_quote;
                        let capital_required = margin + entry_cost.total();
                        if let Some(reason) = budget_rejection_reason(
                            &portfolio,
                            &strategy_states,
                            state_index,
                            capital_used_quote,
                            capital_required,
                        )? {
                            reject_budget(
                                &mut rejection_reasons,
                                &mut events,
                                bar,
                                &mut strategy_states[state_index],
                                reason,
                            );
                            state_index += 1;
                            continue;
                        }
                        let latest_atr = latest_atr_for_strategy(
                            &mut indicator_context,
                            strategy_states[state_index].strategy,
                        );
                        add_leg(
                            &mut strategy_states[state_index],
                            next_leg_index,
                            trigger_price,
                            margin,
                            notional,
                            latest_atr,
                        )?;
                        capital_used_quote += capital_required;
                        trade_count += 1;
                        max_capital_used_quote = max_capital_used_quote.max(capital_used_quote);
                        events.push(event(
                            bar,
                            &strategy_states[state_index],
                            "safety_order",
                            format!(
                                "leg_index={next_leg_index};margin_quote={margin};notional_quote={notional};leverage={};fee_quote={};slippage_quote={}",
                                strategy_states[state_index].strategy.leverage.unwrap_or(1),
                                entry_cost.fee_quote,
                                entry_cost.slippage_quote
                            ),
                        ));
                    }
                }

                state_index += 1;
            }
        }

        let exit_decisions = exit_decision_snapshot(
            &mut strategy_states,
            group,
            &latest_close_by_symbol,
            &mut indicator_context,
        )?;

        for exit in exit_decisions {
            let state_index = exit.state_index;
            if strategy_states[state_index].legs.is_empty() {
                continue;
            }

            match exit.decision {
                ExitDecision::GlobalStop | ExitDecision::SymbolStop => {
                    let close_price = latest_close_by_symbol
                        .get(&strategy_states[state_index].strategy.symbol)
                        .copied()
                        .unwrap_or(exit.bar.close);
                    let close_gross_pnl = close_pnl(
                        strategy_states[state_index].strategy.direction,
                        &strategy_states[state_index].legs,
                        close_price,
                    )?;
                    let entry_cost = entry_cost_quote(&strategy_states[state_index].legs);
                    let exit_cost =
                        exit_cost_quote(&strategy_states[state_index].legs, close_price);
                    total_fee_quote += exit_cost.fee_quote;
                    total_slippage_quote += exit_cost.slippage_quote;
                    let pnl = close_gross_pnl - entry_cost - exit_cost.total();
                    realized_pnl_quote += pnl;
                    capital_used_quote -= strategy_states[state_index].active_capital_used_quote();
                    let event_type = if matches!(exit.decision, ExitDecision::GlobalStop) {
                        "global_stop_loss"
                    } else {
                        "symbol_stop_loss"
                    };
                    let state = &mut strategy_states[state_index];
                    state.realized_pnl_quote += pnl;
                    events.push(event(
                        &exit.bar,
                        state,
                        event_type,
                        format!(
                            "price={close_price};pnl_quote={pnl};exit_fee_quote={};exit_slippage_quote={}",
                            exit_cost.fee_quote, exit_cost.slippage_quote
                        ),
                    ));
                    state.reset_cycle(exit.bar.open_time_ms);
                    stop_count += 1;
                    trade_count += 1;
                }
                ExitDecision::StrategyStop => {
                    let stop_price = exit.stop_price.unwrap_or_else(|| {
                        latest_close_by_symbol
                            .get(&strategy_states[state_index].strategy.symbol)
                            .copied()
                            .unwrap_or(exit.bar.close)
                    });
                    let close_gross_pnl = close_pnl(
                        strategy_states[state_index].strategy.direction,
                        &strategy_states[state_index].legs,
                        stop_price,
                    )?;
                    let entry_cost = entry_cost_quote(&strategy_states[state_index].legs);
                    let exit_cost = exit_cost_quote(&strategy_states[state_index].legs, stop_price);
                    total_fee_quote += exit_cost.fee_quote;
                    total_slippage_quote += exit_cost.slippage_quote;
                    let pnl = close_gross_pnl - entry_cost - exit_cost.total();
                    realized_pnl_quote += pnl;
                    capital_used_quote -= strategy_states[state_index].active_capital_used_quote();
                    let state = &mut strategy_states[state_index];
                    state.realized_pnl_quote += pnl;
                    events.push(event(
                        &exit.bar,
                        state,
                        "stop_loss",
                        format!(
                            "price={stop_price};pnl_quote={pnl};exit_fee_quote={};exit_slippage_quote={}",
                            exit_cost.fee_quote, exit_cost.slippage_quote
                        ),
                    ));
                    state.reset_cycle(exit.bar.open_time_ms);
                    stop_count += 1;
                    trade_count += 1;
                }
                ExitDecision::TakeProfit => {
                    let tp_price = exit.exit_price;
                    let close_gross_pnl = close_pnl(
                        strategy_states[state_index].strategy.direction,
                        &strategy_states[state_index].legs,
                        tp_price,
                    )?;
                    let entry_cost = entry_cost_quote(&strategy_states[state_index].legs);
                    let exit_cost = exit_cost_quote(&strategy_states[state_index].legs, tp_price);
                    total_fee_quote += exit_cost.fee_quote;
                    total_slippage_quote += exit_cost.slippage_quote;
                    let pnl = close_gross_pnl - entry_cost - exit_cost.total();
                    realized_pnl_quote += pnl;
                    capital_used_quote -= strategy_states[state_index].active_capital_used_quote();
                    let state = &mut strategy_states[state_index];
                    state.realized_pnl_quote += pnl;
                    events.push(event(
                        &exit.bar,
                        state,
                        "take_profit",
                        format!(
                            "price={tp_price};pnl_quote={pnl};exit_fee_quote={};exit_slippage_quote={}",
                            exit_cost.fee_quote, exit_cost.slippage_quote
                        ),
                    ));
                    state.reset_cycle(exit.bar.open_time_ms);
                    trade_count += 1;
                }
                ExitDecision::None => {}
            }
        }

        let equity_quote = initial_margin_capital
            + realized_pnl_quote
            + unrealized_pnl(&strategy_states, &latest_close_by_symbol)?;
        if equity_quote.is_finite() {
            equity_peak_quote = equity_peak_quote.max(equity_quote);
            if equity_peak_quote > 0.0 {
                let drawdown_pct =
                    ((equity_peak_quote - equity_quote) / equity_peak_quote * 100.0).max(0.0);
                max_drawdown_pct = max_drawdown_pct.max(drawdown_pct);
            }
            last_equity_quote = equity_quote;
            equity_curve.push(EquityPoint {
                timestamp_ms,
                equity_quote,
            });
        }
    }

    let total_planned_margin: f64 = strategy_states
        .iter()
        .map(|s| s.margins.iter().sum::<f64>())
        .sum();
    let total_planned_notional: f64 = strategy_states
        .iter()
        .map(|s| s.notionals.iter().sum::<f64>())
        .sum();

    let final_equity_quote = equity_curve
        .last()
        .map(|point| point.equity_quote)
        .unwrap_or(initial_margin_capital);
    let total_return_pct = if initial_margin_capital > 0.0 {
        (final_equity_quote - initial_margin_capital) / initial_margin_capital * 100.0
    } else {
        0.0
    };

    let annualized_return_pct = {
        let first = equity_curve.first();
        let last = equity_curve.last();
        match (first, last) {
            (Some(f), Some(l)) => {
                let days = ((l.timestamp_ms - f.timestamp_ms) as f64) / 86_400_000.0;
                calculate_annualized_return_pct(f.equity_quote, l.equity_quote, days)
            }
            _ => None,
        }
    };

    let trades = trade_details_from_events(&events, &equity_curve);

    Ok(MartingaleBacktestResult {
        metrics: MartingaleMetrics {
            total_return_pct: finite_or_zero(total_return_pct),
            annualized_return_pct,
            max_drawdown_pct: finite_or_zero(max_drawdown_pct),
            global_drawdown_pct: Some(finite_or_zero(max_drawdown_pct)),
            max_strategy_drawdown_pct: Some(finite_or_zero(max_drawdown_pct)),
            monthly_win_rate_pct: None,
            max_leverage_used: strategy_states
                .iter()
                .filter_map(|state| state.strategy.leverage.map(|value| value as f64))
                .max_by(f64::total_cmp),
            min_liquidation_buffer_pct: None,
            total_fee_quote: Some(total_fee_quote),
            total_slippage_quote: Some(total_slippage_quote),
            total_funding_quote: Some(total_funding_quote),
            planned_margin_quote: Some(total_planned_margin),
            planned_notional_quote: Some(total_planned_notional),
            return_drawdown_ratio: if max_drawdown_pct > 0.0 {
                Some(total_return_pct / max_drawdown_pct)
            } else {
                None
            },
            data_quality_score: Some(1.0),
            trade_count,
            stop_count,
            max_capital_used_quote: finite_or_zero(max_capital_used_quote),
            survival_passed: rejection_reasons.is_empty(),
        },
        events,
        drawdown_curve: build_drawdown_curve(&equity_curve),
        equity_curve,
        trades,
        rejection_reasons,
    })
}

fn trade_details_from_events(
    events: &[MartingaleBacktestEvent],
    equity_curve: &[EquityPoint],
) -> Vec<MartingaleTradeDetail> {
    events
        .iter()
        .filter_map(|event| {
            let event_type = match event.event_type.as_str() {
                "entry" => "open_leg",
                "safety_order" => "open_leg",
                "take_profit" => "close_cycle",
                "stop_loss" | "global_stop_loss" | "symbol_stop_loss" => "stop_loss",
                "funding_fee" => "funding_fee",
                _ => return None,
            };
            let detail = parse_event_detail(&event.detail);
            let price = detail
                .get("price")
                .copied()
                .or_else(|| detail.get("mark_price").copied())
                .or_else(|| {
                    equity_curve
                        .iter()
                        .find(|point| point.timestamp_ms == event.timestamp_ms)
                        .map(|_| 0.0)
                })
                .unwrap_or(0.0);
            let notional_quote = detail.get("notional_quote").copied().unwrap_or(0.0);
            let fee_quote = detail
                .get("fee_quote")
                .copied()
                .or_else(|| detail.get("exit_fee_quote").copied())
                .unwrap_or(0.0);
            let slippage_quote = detail
                .get("slippage_quote")
                .copied()
                .or_else(|| detail.get("exit_slippage_quote").copied())
                .unwrap_or(0.0);
            let realized_pnl_quote = detail.get("pnl_quote").copied().unwrap_or(0.0);
            let realized_pnl_quote = detail
                .get("funding_quote")
                .copied()
                .unwrap_or(realized_pnl_quote);
            let equity_after_quote = equity_curve
                .iter()
                .rev()
                .find(|point| point.timestamp_ms <= event.timestamp_ms)
                .or_else(|| equity_curve.first())
                .map(|point| point.equity_quote)
                .unwrap_or(0.0);
            Some(MartingaleTradeDetail {
                timestamp_ms: event.timestamp_ms,
                symbol: event.symbol.clone(),
                direction: event
                    .strategy_instance_id
                    .split('-')
                    .next_back()
                    .unwrap_or_default()
                    .to_owned(),
                event_type: event_type.to_owned(),
                leg_index: detail.get("leg_index").map(|value| *value as u32),
                price,
                margin_quote: detail
                    .get("margin_quote")
                    .copied()
                    .unwrap_or(notional_quote),
                notional_quote,
                leverage: detail.get("leverage").copied().unwrap_or(1.0),
                fee_quote,
                slippage_quote,
                realized_pnl_quote,
                equity_after_quote,
            })
        })
        .collect()
}

fn parse_event_detail(detail: &str) -> BTreeMap<String, f64> {
    detail
        .split(';')
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            let parsed = value.parse::<f64>().ok()?;
            Some((key.to_owned(), parsed))
        })
        .collect()
}

struct StrategyRuntime<'a> {
    strategy: &'a MartingaleStrategyConfig,
    margins: Vec<f64>,
    notionals: Vec<f64>,
    trigger_prices: Vec<f64>,
    legs: Vec<MartingaleLegState>,
    cycle_seq: u64,
    cycle_id: String,
    new_legs_blocked: bool,
    realized_pnl_quote: f64,
    last_cycle_closed_at_ms: Option<i64>,
    trailing_anchor_price: Option<f64>,
}

impl<'a> StrategyRuntime<'a> {
    fn new(strategy: &'a MartingaleStrategyConfig) -> Result<Self, String> {
        // Canonical capital model (see `martingale::capital`): the sizing
        // geometric series is the LEVERAGED ORDER NOTIONAL (position size sent
        // to the exchange); leg margin is `notional / effective_leverage`.
        // Fees, PnL and quantity use notional; capital budgets and returns use
        // margin.
        let notionals = leg_notional_series(&strategy.sizing, DEFAULT_EXCHANGE_MIN_NOTIONAL)?;
        if notionals.is_empty() {
            return Err(format!(
                "strategy {} has no notionals",
                strategy.strategy_id
            ));
        }
        let leverage = effective_leverage(strategy.market, strategy.leverage);
        let margins = notionals
            .iter()
            .map(|notional| notional / leverage)
            .collect::<Vec<_>>();

        Ok(Self {
            strategy,
            trigger_prices: Vec::new(),
            margins,
            notionals,
            legs: Vec::new(),
            cycle_seq: 1,
            cycle_id: format!("{}-cycle-1", strategy.strategy_id),
            new_legs_blocked: false,
            realized_pnl_quote: 0.0,
            last_cycle_closed_at_ms: None,
            trailing_anchor_price: None,
        })
    }

    fn next_leg_index(&self) -> Option<usize> {
        let next = self.legs.len();
        (next < self.notionals.len()).then_some(next)
    }

    fn capital_used_quote(&self) -> f64 {
        self.legs.iter().map(|leg| leg.margin_quote).sum()
    }

    fn active_capital_used_quote(&self) -> f64 {
        self.legs
            .iter()
            .map(|leg| leg.margin_quote + leg.fee_quote + leg.slippage_quote)
            .sum()
    }

    fn reset_cycle(&mut self, closed_at_ms: i64) {
        self.legs.clear();
        self.trigger_prices.clear();
        self.new_legs_blocked = false;
        self.trailing_anchor_price = None;
        self.last_cycle_closed_at_ms = Some(closed_at_ms);
        self.cycle_seq += 1;
        self.cycle_id = format!("{}-cycle-{}", self.strategy.strategy_id, self.cycle_seq);
    }
}

fn add_leg(
    state: &mut StrategyRuntime<'_>,
    leg_index: usize,
    price: f64,
    margin_quote: f64,
    notional_quote: f64,
    latest_atr: Option<f64>,
) -> Result<(), String> {
    validate_positive_f64("price", price)?;
    validate_positive_f64("margin_quote", margin_quote)?;
    validate_positive_f64("notional_quote", notional_quote)?;
    let quantity = notional_quote / price;
    validate_positive_f64("quantity", quantity)?;
    let entry_cost = trading_cost_quote(notional_quote);

    if leg_index == 0 {
        state.trigger_prices = compute_leg_trigger_prices(
            price,
            state.strategy.direction,
            &state.strategy.spacing,
            latest_atr,
            state.notionals.len().saturating_sub(1) as u32,
        )?;
    }

    state.legs.push(MartingaleLegState {
        leg_index: leg_index as u32,
        price,
        quantity,
        margin_quote,
        notional_quote,
        fee_quote: entry_cost.fee_quote,
        slippage_quote: entry_cost.slippage_quote,
    });
    Ok(())
}

fn preflight_rejection_reasons(portfolio: &MartingalePortfolioConfig) -> Vec<String> {
    let mut reasons = Vec::new();

    for strategy in &portfolio.strategies {
        if let Some(error) = kline_stop_loss_support_error(strategy.stop_loss.as_ref()) {
            reasons.push(format!("{error} for strategy {}", strategy.strategy_id));
        }

        if let Some(error) = kline_take_profit_support_error(&strategy.take_profit) {
            reasons.push(format!("{error} for strategy {}", strategy.strategy_id));
        }

        for trigger in &strategy.entry_triggers {
            if let Some(error) = kline_entry_trigger_support_error(trigger) {
                reasons.push(format!("{error} for strategy {}", strategy.strategy_id));
            }
        }
    }

    reasons
}

fn kline_stop_loss_support_error(
    stop_loss: Option<&MartingaleStopLossModel>,
) -> Option<&'static str> {
    match stop_loss {
        _ => None,
    }
}

fn kline_take_profit_support_error(
    take_profit: &MartingaleTakeProfitModel,
) -> Option<&'static str> {
    match take_profit {
        MartingaleTakeProfitModel::Percent { .. }
        | MartingaleTakeProfitModel::Amount { .. }
        | MartingaleTakeProfitModel::Atr { .. }
        | MartingaleTakeProfitModel::Trailing { .. } => None,
        MartingaleTakeProfitModel::Mixed { phases } => {
            phases.iter().find_map(kline_take_profit_support_error)
        }
    }
}

fn kline_entry_trigger_support_error(trigger: &MartingaleEntryTrigger) -> Option<&'static str> {
    match trigger {
        _ => None,
    }
}

fn budget_rejection_reason(
    portfolio: &MartingalePortfolioConfig,
    states: &[StrategyRuntime<'_>],
    state_index: usize,
    current_global_capital: f64,
    additional_capital: f64,
) -> Result<Option<String>, String> {
    let state = &states[state_index];

    if let Some(limit) = optional_decimal_to_f64(
        "max_global_budget_quote",
        portfolio.risk_limits.max_global_budget_quote,
    )? {
        if current_global_capital + additional_capital > limit {
            return Ok(Some(format!(
                "global budget exceeded for strategy {}; current_capital_quote={current_global_capital}; next_capital_quote={additional_capital}; budget_quote={limit}",
                state.strategy.strategy_id
            )));
        }
    }

    if let Some(limit) = optional_decimal_to_f64(
        "max_symbol_budget_quote",
        portfolio.risk_limits.max_symbol_budget_quote,
    )? {
        let current = symbol_active_capital(states, &state.strategy.symbol);
        if current + additional_capital > limit {
            return Ok(Some(format!(
                "symbol budget exceeded for strategy {}; symbol={}; current_capital_quote={current}; next_capital_quote={additional_capital}; budget_quote={limit}",
                state.strategy.strategy_id, state.strategy.symbol
            )));
        }
    }

    if let Some(limit) = optional_decimal_to_f64(
        "max_direction_budget_quote",
        portfolio.risk_limits.max_direction_budget_quote,
    )? {
        let current = direction_active_capital(states, state.strategy.direction);
        if current + additional_capital > limit {
            return Ok(Some(format!(
                "direction budget exceeded for strategy {}; direction={:?}; current_capital_quote={current}; next_capital_quote={additional_capital}; budget_quote={limit}",
                state.strategy.strategy_id, state.strategy.direction
            )));
        }
    }

    let strategy_budget = state
        .strategy
        .risk_limits
        .max_strategy_budget_quote
        .or(portfolio.risk_limits.max_strategy_budget_quote);
    if let Some(limit) = optional_decimal_to_f64("max_strategy_budget_quote", strategy_budget)? {
        let current = state.active_capital_used_quote();
        if current + additional_capital > limit {
            return Ok(Some(format!(
                "strategy budget exceeded for strategy {}; current_capital_quote={current}; next_capital_quote={additional_capital}; budget_quote={limit}",
                state.strategy.strategy_id
            )));
        }
    }

    Ok(None)
}

fn symbol_active_capital(states: &[StrategyRuntime<'_>], symbol: &str) -> f64 {
    states
        .iter()
        .filter(|state| state.strategy.symbol == symbol)
        .map(StrategyRuntime::active_capital_used_quote)
        .sum()
}

fn direction_active_capital(states: &[StrategyRuntime<'_>], direction: MartingaleDirection) -> f64 {
    states
        .iter()
        .filter(|state| state.strategy.direction == direction)
        .map(StrategyRuntime::active_capital_used_quote)
        .sum()
}

fn rejected_result(rejection_reasons: Vec<String>) -> MartingaleBacktestResult {
    MartingaleBacktestResult {
        metrics: MartingaleMetrics {
            total_return_pct: 0.0,
            annualized_return_pct: None,
            max_drawdown_pct: 0.0,
            global_drawdown_pct: Some(0.0),
            max_strategy_drawdown_pct: Some(0.0),
            monthly_win_rate_pct: None,
            max_leverage_used: None,
            min_liquidation_buffer_pct: None,
            total_fee_quote: None,
            total_slippage_quote: None,
            total_funding_quote: None,
            planned_margin_quote: None,
            planned_notional_quote: None,
            return_drawdown_ratio: None,
            data_quality_score: Some(1.0),
            trade_count: 0,
            stop_count: 0,
            max_capital_used_quote: 0.0,
            survival_passed: false,
        },
        events: Vec::new(),
        equity_curve: Vec::new(),
        drawdown_curve: Vec::new(),
        trades: Vec::new(),
        rejection_reasons,
    }
}

fn reject_budget(
    rejection_reasons: &mut Vec<String>,
    events: &mut Vec<MartingaleBacktestEvent>,
    bar: &KlineBar,
    state: &mut StrategyRuntime<'_>,
    reason: String,
) {
    rejection_reasons.push(reason.clone());
    events.push(event(bar, state, "rejected", reason));
    state.new_legs_blocked = true;
}

fn apply_funding_rate(
    states: &mut [StrategyRuntime<'_>],
    funding: &FundingRatePoint,
    latest_close_by_symbol: &BTreeMap<String, f64>,
    events: &mut Vec<MartingaleBacktestEvent>,
) -> Result<f64, String> {
    let mut total = 0.0;
    for state in states.iter_mut() {
        if state.strategy.symbol != funding.symbol || state.legs.is_empty() {
            continue;
        }
        let mark_price = funding
            .mark_price
            .filter(|price| price.is_finite() && *price > 0.0)
            .or_else(|| latest_close_by_symbol.get(&funding.symbol).copied())
            .ok_or_else(|| {
                format!(
                    "missing mark/close price for funding symbol {} at {}",
                    funding.symbol, funding.funding_time_ms
                )
            })?;
        validate_positive_f64("funding.mark_price", mark_price)?;
        let notional = state
            .legs
            .iter()
            .map(|leg| leg.quantity * mark_price)
            .sum::<f64>();
        if notional <= 0.0 {
            continue;
        }
        let direction_sign = match state.strategy.direction {
            MartingaleDirection::Long => 1.0,
            MartingaleDirection::Short => -1.0,
        };
        let funding_quote = -direction_sign * notional * funding.funding_rate;
        if !funding_quote.is_finite() {
            return Err(format!(
                "non-finite funding quote for {} at {}",
                funding.symbol, funding.funding_time_ms
            ));
        }
        state.realized_pnl_quote += funding_quote;
        total += funding_quote;
        events.push(MartingaleBacktestEvent {
            timestamp_ms: funding.funding_time_ms,
            event_type: "funding_fee".to_string(),
            symbol: state.strategy.symbol.clone(),
            strategy_instance_id: state.strategy.strategy_id.clone(),
            cycle_id: Some(state.cycle_id.clone()),
            detail: format!(
                "funding_rate={};mark_price={mark_price};notional_quote={notional};funding_quote={funding_quote}",
                funding.funding_rate
            ),
        });
    }
    Ok(total)
}

fn event(
    bar: &KlineBar,
    state: &StrategyRuntime<'_>,
    event_type: &str,
    detail: String,
) -> MartingaleBacktestEvent {
    MartingaleBacktestEvent {
        timestamp_ms: bar.open_time_ms,
        event_type: event_type.to_string(),
        symbol: state.strategy.symbol.clone(),
        strategy_instance_id: state.strategy.strategy_id.clone(),
        cycle_id: Some(state.cycle_id.clone()),
        detail,
    }
}

fn safety_order_triggered(
    direction: MartingaleDirection,
    bar: &KlineBar,
    trigger_price: f64,
) -> bool {
    match direction {
        MartingaleDirection::Long => bar.low <= trigger_price,
        MartingaleDirection::Short => bar.high >= trigger_price,
    }
}

fn take_profit_triggered(direction: MartingaleDirection, bar: &KlineBar, price: f64) -> bool {
    match direction {
        MartingaleDirection::Long => bar.high >= price,
        MartingaleDirection::Short => bar.low <= price,
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct TakeProfitSignal {
    triggered: bool,
    price: Option<f64>,
}

fn take_profit_signal(
    state: &mut StrategyRuntime<'_>,
    bar: &KlineBar,
    indicator_context: &mut IndicatorRuntimeContext,
) -> Result<TakeProfitSignal, String> {
    let model = state.strategy.take_profit.clone();
    take_profit_signal_for_model(state, bar, &model, indicator_context)
}

fn take_profit_signal_for_model(
    state: &mut StrategyRuntime<'_>,
    bar: &KlineBar,
    model: &MartingaleTakeProfitModel,
    indicator_context: &mut IndicatorRuntimeContext,
) -> Result<TakeProfitSignal, String> {
    let average_entry = weighted_average_entry(&state.legs)?;
    match model {
        MartingaleTakeProfitModel::Amount { quote } => {
            let threshold = decimal_to_positive_f64("take_profit.amount_quote", quote)?;
            let price = amount_take_profit_price(state, threshold)?;
            Ok(TakeProfitSignal {
                triggered: take_profit_triggered(state.strategy.direction, bar, price),
                price: Some(price),
            })
        }
        MartingaleTakeProfitModel::Atr { multiplier } => {
            let Some(latest_atr) = latest_atr_for_strategy(indicator_context, state.strategy)
            else {
                return Ok(TakeProfitSignal::default());
            };
            let price = take_profit_price(
                average_entry,
                state.strategy.direction,
                &MartingaleTakeProfitModel::Atr {
                    multiplier: *multiplier,
                },
                Some(latest_atr),
            )?;
            Ok(TakeProfitSignal {
                triggered: take_profit_triggered(state.strategy.direction, bar, price),
                price: Some(price),
            })
        }
        MartingaleTakeProfitModel::Trailing {
            activation_bps,
            callback_bps,
        } => trailing_take_profit_signal(state, bar, average_entry, *activation_bps, *callback_bps),
        MartingaleTakeProfitModel::Mixed { phases } => {
            if phases.is_empty() {
                return Err("mixed take profit requires at least one phase".to_string());
            }
            let mut any_supported = false;
            for phase in phases {
                let signal = take_profit_signal_for_model(state, bar, phase, indicator_context)?;
                any_supported = true;
                if signal.triggered {
                    return Ok(signal);
                }
            }
            Ok(TakeProfitSignal {
                triggered: false,
                price: if any_supported { Some(bar.close) } else { None },
            })
        }
        MartingaleTakeProfitModel::Percent { .. } => {
            let price = take_profit_price(average_entry, state.strategy.direction, model, None)?;
            Ok(TakeProfitSignal {
                triggered: take_profit_triggered(state.strategy.direction, bar, price),
                price: Some(price),
            })
        }
    }
}

fn trailing_take_profit_signal(
    state: &mut StrategyRuntime<'_>,
    bar: &KlineBar,
    average_entry: f64,
    activation_bps: u32,
    callback_bps: u32,
) -> Result<TakeProfitSignal, String> {
    if callback_bps == 0 {
        return Err("callback_bps must be > 0 for trailing take profit".to_string());
    }
    let activation_offset = average_entry * activation_bps as f64 / 10_000.0;
    let callback_multiplier = callback_bps as f64 / 10_000.0;
    match state.strategy.direction {
        MartingaleDirection::Long => {
            let activation = average_entry + activation_offset;
            if bar.high >= activation {
                let anchor = state
                    .trailing_anchor_price
                    .map(|current| current.max(bar.high))
                    .unwrap_or(bar.high);
                state.trailing_anchor_price = Some(anchor);
                let stop_price = anchor * (1.0 - callback_multiplier);
                Ok(TakeProfitSignal {
                    triggered: bar.low <= stop_price,
                    price: Some(stop_price),
                })
            } else {
                Ok(TakeProfitSignal::default())
            }
        }
        MartingaleDirection::Short => {
            let activation = average_entry - activation_offset;
            if bar.low <= activation {
                let anchor = state
                    .trailing_anchor_price
                    .map(|current| current.min(bar.low))
                    .unwrap_or(bar.low);
                state.trailing_anchor_price = Some(anchor);
                let stop_price = anchor * (1.0 + callback_multiplier);
                Ok(TakeProfitSignal {
                    triggered: bar.high >= stop_price,
                    price: Some(stop_price),
                })
            } else {
                Ok(TakeProfitSignal::default())
            }
        }
    }
}

fn amount_take_profit_price(
    state: &StrategyRuntime<'_>,
    threshold_quote: f64,
) -> Result<f64, String> {
    validate_positive_f64("take_profit.amount_quote", threshold_quote)?;
    let total_quantity = state.legs.iter().map(|leg| leg.quantity).sum::<f64>();
    validate_positive_f64("total_quantity", total_quantity)?;
    let entry_notional = state
        .legs
        .iter()
        .map(|leg| leg.price * leg.quantity)
        .sum::<f64>();
    validate_positive_f64("entry_notional", entry_notional)?;
    let entry_cost = entry_cost_quote(&state.legs);
    let exit_cost_rate = (DEFAULT_FEE_BPS + DEFAULT_SLIPPAGE_BPS) / 10_000.0;
    let price = match state.strategy.direction {
        MartingaleDirection::Long => {
            (threshold_quote + entry_notional + entry_cost)
                / (total_quantity * (1.0 - exit_cost_rate))
        }
        MartingaleDirection::Short => {
            (entry_notional - entry_cost - threshold_quote)
                / (total_quantity * (1.0 + exit_cost_rate))
        }
    };
    validate_positive_f64("take_profit.amount_price", price)?;
    Ok(price)
}

#[derive(Debug, Clone, Copy, Default)]
struct StopSignal {
    global_stop: bool,
    symbol_stop: bool,
    strategy_stop: bool,
    price: Option<f64>,
}

#[derive(Debug, Clone)]
struct ExitSnapshot {
    state_index: usize,
    decision: ExitDecision,
    exit_price: f64,
    stop_price: Option<f64>,
    bar: KlineBar,
}

fn exit_decision_snapshot(
    states: &mut [StrategyRuntime<'_>],
    bars: &[KlineBar],
    latest_close_by_symbol: &BTreeMap<String, f64>,
    indicator_context: &mut IndicatorRuntimeContext,
) -> Result<Vec<ExitSnapshot>, String> {
    let mut snapshots = Vec::new();

    for state_index in 0..states.len() {
        let symbol = states[state_index].strategy.symbol.clone();
        let Some(bar) = bars.iter().find(|bar| bar.symbol == symbol) else {
            continue;
        };
        if states[state_index].legs.is_empty() {
            continue;
        }

        let stop = triggered_stop(
            &states[state_index],
            bar,
            states,
            latest_close_by_symbol,
            indicator_context,
        )?;
        let take_profit = take_profit_signal(&mut states[state_index], bar, indicator_context)?;
        let decision = evaluate_exit_priority(
            stop.global_stop,
            stop.symbol_stop,
            stop.strategy_stop,
            take_profit.triggered,
        );

        snapshots.push(ExitSnapshot {
            state_index,
            decision,
            exit_price: take_profit.price.unwrap_or(bar.close),
            stop_price: stop.price,
            bar: bar.clone(),
        });
    }

    Ok(snapshots)
}

fn triggered_stop(
    state: &StrategyRuntime<'_>,
    bar: &KlineBar,
    states: &[StrategyRuntime<'_>],
    latest_close_by_symbol: &BTreeMap<String, f64>,
    indicator_context: &mut IndicatorRuntimeContext,
) -> Result<StopSignal, String> {
    let Some(stop_loss) = &state.strategy.stop_loss else {
        return Ok(StopSignal::default());
    };

    match stop_loss {
        MartingaleStopLossModel::PriceRange { .. } => Ok(StopSignal {
            strategy_stop: triggered_price_range_stop_price(state.strategy, bar)?.is_some(),
            price: triggered_price_range_stop_price(state.strategy, bar)?,
            ..StopSignal::default()
        }),
        MartingaleStopLossModel::StrategyDrawdownPct { pct_bps } => {
            let invested = state.capital_used_quote();
            if invested <= 0.0 {
                return Ok(StopSignal::default());
            }
            let current_price = latest_close_by_symbol
                .get(&state.strategy.symbol)
                .copied()
                .unwrap_or(bar.close);
            let pnl = strategy_net_pnl(state, current_price)?;
            let drawdown_pct = (-pnl).max(0.0) / invested * 100.0;
            Ok(StopSignal {
                strategy_stop: drawdown_pct >= *pct_bps as f64 / 100.0,
                ..StopSignal::default()
            })
        }
        MartingaleStopLossModel::SymbolDrawdownAmount { quote } => {
            let threshold = decimal_to_positive_f64("stop_loss.symbol_drawdown_quote", quote)?;
            let symbol_pnl = symbol_pnl(states, latest_close_by_symbol, &state.strategy.symbol)?;
            Ok(StopSignal {
                symbol_stop: -symbol_pnl >= threshold,
                ..StopSignal::default()
            })
        }
        MartingaleStopLossModel::GlobalDrawdownAmount { quote } => {
            let threshold = decimal_to_positive_f64("stop_loss.global_drawdown_quote", quote)?;
            let global_pnl = portfolio_pnl(states, latest_close_by_symbol)?;
            Ok(StopSignal {
                global_stop: -global_pnl >= threshold,
                ..StopSignal::default()
            })
        }
        MartingaleStopLossModel::Atr { multiplier } => {
            let multiplier = decimal_to_positive_f64("stop_loss.atr_multiplier", multiplier)?;
            let Some(latest_atr) = latest_atr_for_strategy(indicator_context, state.strategy)
            else {
                return Ok(StopSignal::default());
            };
            let average_entry = weighted_average_entry(&state.legs)?;
            let stop_price = match state.strategy.direction {
                MartingaleDirection::Long => average_entry - latest_atr * multiplier,
                MartingaleDirection::Short => average_entry + latest_atr * multiplier,
            };
            validate_positive_f64("stop_loss.atr_price", stop_price)?;
            let triggered = match state.strategy.direction {
                MartingaleDirection::Long => bar.low <= stop_price,
                MartingaleDirection::Short => bar.high >= stop_price,
            };
            Ok(StopSignal {
                strategy_stop: triggered,
                price: triggered.then_some(stop_price),
                ..StopSignal::default()
            })
        }
        MartingaleStopLossModel::Indicator { expression } => Ok(StopSignal {
            strategy_stop: indicator_context
                .evaluate_expression(&state.strategy.symbol, expression)?
                .unwrap_or(false),
            price: Some(bar.close),
            ..StopSignal::default()
        }),
    }
}

fn triggered_price_range_stop_price(
    strategy: &MartingaleStrategyConfig,
    bar: &KlineBar,
) -> Result<Option<f64>, String> {
    match &strategy.stop_loss {
        Some(MartingaleStopLossModel::PriceRange { lower, upper }) => match strategy.direction {
            MartingaleDirection::Long => {
                let lower = decimal_to_positive_f64("stop_loss.lower", lower)?;
                Ok((bar.low <= lower).then_some(lower))
            }
            MartingaleDirection::Short => {
                let upper = decimal_to_positive_f64("stop_loss.upper", upper)?;
                Ok((bar.high >= upper).then_some(upper))
            }
        },
        _ => Ok(None),
    }
}

fn entry_triggers_allow_entry(
    state: &StrategyRuntime<'_>,
    bar: &KlineBar,
    indicator_context: &mut IndicatorRuntimeContext,
) -> Result<bool, String> {
    if state.strategy.entry_triggers.is_empty() {
        return Ok(true);
    }

    for trigger in &state.strategy.entry_triggers {
        match trigger {
            MartingaleEntryTrigger::Immediate => {}
            MartingaleEntryTrigger::PriceRange { lower, upper } => {
                let lower = decimal_to_positive_f64("entry_trigger.lower", lower)?;
                let upper = decimal_to_positive_f64("entry_trigger.upper", upper)?;
                if lower > upper {
                    return Err("entry_trigger.lower must be <= entry_trigger.upper".to_string());
                }
                if bar.close < lower || bar.close > upper {
                    return Ok(false);
                }
            }
            MartingaleEntryTrigger::Capacity { max_active_cycles } => {
                if !capacity_allows_entry(*max_active_cycles, active_cycle_count(state)) {
                    return Ok(false);
                }
            }
            MartingaleEntryTrigger::IndicatorExpression { expression } => {
                if !indicator_context
                    .evaluate_expression(&state.strategy.symbol, expression)?
                    .unwrap_or(false)
                {
                    return Ok(false);
                }
            }
            MartingaleEntryTrigger::TimeWindow { start, end } => {
                if !timestamp_in_time_window(bar.open_time_ms, start, end)? {
                    return Ok(false);
                }
            }
            MartingaleEntryTrigger::Cooldown { seconds } => {
                if let Some(closed_at_ms) = state.last_cycle_closed_at_ms {
                    let elapsed_ms = bar.open_time_ms.saturating_sub(closed_at_ms);
                    if elapsed_ms < (*seconds as i64).saturating_mul(1_000) {
                        return Ok(false);
                    }
                }
            }
        }
    }

    Ok(true)
}
fn timestamp_in_time_window(timestamp_ms: i64, start: &str, end: &str) -> Result<bool, String> {
    let start_ms = parse_time_of_day_ms(start)?;
    let end_ms = parse_time_of_day_ms(end)?;
    let day_ms = 86_400_000_i64;
    let time_ms = timestamp_ms.rem_euclid(day_ms);
    if start_ms <= end_ms {
        Ok(time_ms >= start_ms && time_ms <= end_ms)
    } else {
        Ok(time_ms >= start_ms || time_ms <= end_ms)
    }
}

fn parse_time_of_day_ms(value: &str) -> Result<i64, String> {
    if let Ok(ms) = value.trim().parse::<i64>() {
        if (0..86_400_000).contains(&ms) {
            return Ok(ms);
        }
        return Err(format!(
            "time window millisecond value out of range: {value}"
        ));
    }

    let parts = value.trim().split(':').collect::<Vec<_>>();
    if !(2..=3).contains(&parts.len()) {
        return Err(format!("time window must be HH:MM or HH:MM:SS: {value}"));
    }
    let hour = parts[0]
        .parse::<i64>()
        .map_err(|_| format!("invalid time window hour: {value}"))?;
    let minute = parts[1]
        .parse::<i64>()
        .map_err(|_| format!("invalid time window minute: {value}"))?;
    let second = if parts.len() == 3 {
        parts[2]
            .parse::<i64>()
            .map_err(|_| format!("invalid time window second: {value}"))?
    } else {
        0
    };
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) || !(0..60).contains(&second) {
        return Err(format!("time window value out of range: {value}"));
    }
    Ok(((hour * 60 + minute) * 60 + second) * 1_000)
}

fn active_cycle_count(state: &StrategyRuntime<'_>) -> u32 {
    u32::from(!state.legs.is_empty())
}

fn capacity_allows_entry(max_active_cycles: u32, active_cycle_count: u32) -> bool {
    active_cycle_count < max_active_cycles
}

fn close_pnl(
    direction: MartingaleDirection,
    legs: &[MartingaleLegState],
    close_price: f64,
) -> Result<f64, String> {
    validate_positive_f64("close_price", close_price)?;
    let pnl = legs
        .iter()
        .map(|leg| match direction {
            MartingaleDirection::Long => (close_price - leg.price) * leg.quantity,
            MartingaleDirection::Short => (leg.price - close_price) * leg.quantity,
        })
        .sum::<f64>();
    Ok(finite_or_zero(pnl))
}

fn strategy_net_pnl(state: &StrategyRuntime<'_>, close_price: f64) -> Result<f64, String> {
    Ok(
        state.realized_pnl_quote + close_pnl(state.strategy.direction, &state.legs, close_price)?
            - entry_cost_quote(&state.legs)
            - exit_cost_quote(&state.legs, close_price).total(),
    )
}

#[derive(Debug, Clone, Copy)]
struct TradingCost {
    fee_quote: f64,
    slippage_quote: f64,
}

impl TradingCost {
    fn total(self) -> f64 {
        self.fee_quote + self.slippage_quote
    }
}

fn trading_cost_quote(notional_quote: f64) -> TradingCost {
    TradingCost {
        fee_quote: notional_quote * DEFAULT_FEE_BPS / 10_000.0,
        slippage_quote: notional_quote * DEFAULT_SLIPPAGE_BPS / 10_000.0,
    }
}

fn exit_cost_quote(legs: &[MartingaleLegState], close_price: f64) -> TradingCost {
    let close_notional = legs
        .iter()
        .map(|leg| leg.quantity * close_price)
        .sum::<f64>();
    trading_cost_quote(close_notional)
}

fn entry_cost_quote(legs: &[MartingaleLegState]) -> f64 {
    legs.iter()
        .map(|leg| leg.fee_quote + leg.slippage_quote)
        .sum()
}

fn unrealized_pnl(
    states: &[StrategyRuntime<'_>],
    latest_close_by_symbol: &BTreeMap<String, f64>,
) -> Result<f64, String> {
    states.iter().try_fold(0.0, |acc, state| {
        let Some(close) = latest_close_by_symbol.get(&state.strategy.symbol) else {
            return Ok(acc);
        };
        Ok(
            acc + close_pnl(state.strategy.direction, &state.legs, *close)?
                - entry_cost_quote(&state.legs)
                - exit_cost_quote(&state.legs, *close).total(),
        )
    })
}

fn symbol_pnl(
    states: &[StrategyRuntime<'_>],
    latest_close_by_symbol: &BTreeMap<String, f64>,
    symbol: &str,
) -> Result<f64, String> {
    states.iter().try_fold(0.0, |acc, state| {
        if state.strategy.symbol != symbol {
            return Ok(acc);
        }
        let unrealized = latest_close_by_symbol
            .get(&state.strategy.symbol)
            .map(|close| close_pnl(state.strategy.direction, &state.legs, *close))
            .transpose()?
            .unwrap_or(0.0);
        let close = latest_close_by_symbol.get(&state.strategy.symbol).copied();
        let exit_cost = close
            .map(|price| exit_cost_quote(&state.legs, price).total())
            .unwrap_or(0.0);
        Ok(acc + state.realized_pnl_quote + unrealized - entry_cost_quote(&state.legs) - exit_cost)
    })
}

fn portfolio_pnl(
    states: &[StrategyRuntime<'_>],
    latest_close_by_symbol: &BTreeMap<String, f64>,
) -> Result<f64, String> {
    states.iter().try_fold(0.0, |acc, state| {
        let unrealized = latest_close_by_symbol
            .get(&state.strategy.symbol)
            .map(|close| close_pnl(state.strategy.direction, &state.legs, *close))
            .transpose()?
            .unwrap_or(0.0);
        let close = latest_close_by_symbol.get(&state.strategy.symbol).copied();
        let exit_cost = close
            .map(|price| exit_cost_quote(&state.legs, price).total())
            .unwrap_or(0.0);
        Ok(acc + state.realized_pnl_quote + unrealized - entry_cost_quote(&state.legs) - exit_cost)
    })
}

fn validate_bar(bar: &KlineBar) -> Result<(), String> {
    validate_positive_f64("bar.open", bar.open)?;
    validate_positive_f64("bar.high", bar.high)?;
    validate_positive_f64("bar.low", bar.low)?;
    validate_positive_f64("bar.close", bar.close)?;
    if bar.high < bar.low {
        return Err("bar.high must be >= bar.low".to_string());
    }
    Ok(())
}

fn validate_positive_f64(name: &str, value: f64) -> Result<(), String> {
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("{name} must be finite and > 0"));
    }
    Ok(())
}

fn decimal_to_positive_f64(name: &str, value: &rust_decimal::Decimal) -> Result<f64, String> {
    let value = value
        .to_f64()
        .ok_or_else(|| format!("{name} cannot be represented as f64"))?;
    validate_positive_f64(name, value)?;
    Ok(value)
}

fn optional_decimal_to_f64(
    name: &str,
    value: Option<rust_decimal::Decimal>,
) -> Result<Option<f64>, String> {
    value
        .map(|value| {
            let value = value
                .to_f64()
                .ok_or_else(|| format!("{name} cannot be represented as f64"))?;
            validate_positive_f64(name, value)?;
            Ok(value)
        })
        .transpose()
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use shared_domain::martingale::{
        MartingaleDirection, MartingaleDirectionMode, MartingaleEntryTrigger, MartingaleMarginMode,
        MartingaleMarketKind, MartingalePortfolioConfig, MartingaleRiskLimits,
        MartingaleSizingModel, MartingaleSpacingModel, MartingaleStopLossModel,
        MartingaleStrategyConfig, MartingaleTakeProfitModel,
    };

    use crate::market_data::KlineBar;
    use crate::martingale::kline_engine::{
        capacity_allows_entry, run_kline_screening, run_kline_screening_with_funding,
        FundingRatePoint,
    };

    fn single_strategy_portfolio(budget_quote: i64) -> MartingalePortfolioConfig {
        portfolio_with_direction(MartingaleDirection::Long, budget_quote)
    }

    fn portfolio_with_direction(
        direction: MartingaleDirection,
        budget_quote: i64,
    ) -> MartingalePortfolioConfig {
        let direction_mode = match direction {
            MartingaleDirection::Long => MartingaleDirectionMode::LongOnly,
            MartingaleDirection::Short => MartingaleDirectionMode::ShortOnly,
        };

        MartingalePortfolioConfig {
            direction_mode,
            strategies: vec![MartingaleStrategyConfig {
                strategy_id: format!("{direction:?}-grid"),
                symbol: "BTCUSDT".to_string(),
                market: MartingaleMarketKind::Spot,
                direction,
                direction_mode,
                margin_mode: None,
                leverage: None,
                spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
                sizing: MartingaleSizingModel::CustomSequence {
                    notionals: vec![Decimal::new(100, 0), Decimal::new(200, 0)],
                },
                take_profit: MartingaleTakeProfitModel::Percent { bps: 100 },
                stop_loss: None,
                indicators: Vec::new(),
                entry_triggers: vec![MartingaleEntryTrigger::Immediate],
                risk_limits: MartingaleRiskLimits::default(),
            }],
            risk_limits: MartingaleRiskLimits {
                max_global_budget_quote: Some(Decimal::new(budget_quote, 0)),
                ..MartingaleRiskLimits::default()
            },
        }
    }

    fn bar(open_time_ms: i64, open: f64, high: f64, low: f64, close: f64) -> KlineBar {
        KlineBar {
            symbol: "BTCUSDT".to_string(),
            open_time_ms,
            open,
            high,
            low,
            close,
            volume: 1.0,
        }
    }

    fn falling_bars() -> Vec<KlineBar> {
        vec![
            bar(1_000, 100.0, 100.2, 100.0, 100.0),
            bar(2_000, 100.0, 100.1, 98.9, 99.0),
            bar(3_000, 99.0, 101.2, 99.0, 101.0),
        ]
    }

    fn rising_bars() -> Vec<KlineBar> {
        vec![
            bar(1_000, 100.0, 100.0, 99.8, 100.0),
            bar(2_000, 100.0, 101.1, 99.9, 101.0),
            bar(3_000, 101.0, 101.0, 98.8, 99.0),
        ]
    }

    fn stop_loss_portfolio() -> MartingalePortfolioConfig {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0].stop_loss = Some(MartingaleStopLossModel::PriceRange {
            lower: Decimal::new(99, 0),
            upper: Decimal::new(120, 0),
        });
        portfolio
    }

    fn portfolio_with_stop_loss(stop_loss: MartingaleStopLossModel) -> MartingalePortfolioConfig {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0].stop_loss = Some(stop_loss);
        portfolio
    }

    fn two_symbol_portfolio() -> MartingalePortfolioConfig {
        let mut portfolio = single_strategy_portfolio(1_000);
        let mut second = portfolio.strategies[0].clone();
        second.strategy_id = "eth-grid".to_string();
        second.symbol = "ETHUSDT".to_string();
        portfolio.strategies.push(second);
        portfolio
    }

    fn symbol_bar(
        symbol: &str,
        open_time_ms: i64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
    ) -> KlineBar {
        KlineBar {
            symbol: symbol.to_string(),
            open_time_ms,
            open,
            high,
            low,
            close,
            volume: 1.0,
        }
    }

    #[test]
    fn long_cycle_adds_safety_order_and_takes_profit() {
        let result =
            run_kline_screening(single_strategy_portfolio(1_000), &falling_bars()).unwrap();

        assert!(result.metrics.survival_passed);
        assert_eq!(result.metrics.trade_count, 3);
        assert_eq!(result.metrics.stop_count, 0);
        assert!(result.metrics.max_capital_used_quote > 300.0);
        assert!(result.metrics.total_return_pct > 0.0);
        assert!(result.rejection_reasons.is_empty());
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "safety_order"));
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "take_profit"));
        assert_eq!(result.equity_curve.len(), 3);
    }

    #[test]
    fn global_budget_blocks_new_leg() {
        let result = run_kline_screening(single_strategy_portfolio(150), &falling_bars()).unwrap();

        assert!(!result.metrics.survival_passed);
        assert_eq!(result.metrics.trade_count, 2);
        assert!(result.metrics.max_capital_used_quote > 100.0);
        assert!(result.metrics.max_capital_used_quote < 101.0);
        assert!(result
            .rejection_reasons
            .iter()
            .any(|reason| reason.contains("budget")));
    }

    #[test]
    fn strategy_budget_blocks_new_leg_before_global_budget() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0]
            .risk_limits
            .max_strategy_budget_quote = Some(Decimal::new(150, 0));

        let result = run_kline_screening(portfolio, &falling_bars()).unwrap();

        assert!(!result.metrics.survival_passed);
        assert!(result
            .rejection_reasons
            .iter()
            .any(|reason| reason.contains("strategy budget")));
    }

    #[test]
    fn symbol_budget_blocks_new_leg() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.risk_limits.max_symbol_budget_quote = Some(Decimal::new(150, 0));

        let result = run_kline_screening(portfolio, &falling_bars()).unwrap();

        assert!(!result.metrics.survival_passed);
        assert!(result
            .rejection_reasons
            .iter()
            .any(|reason| reason.contains("symbol budget")));
    }

    #[test]
    fn direction_budget_blocks_new_leg() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.risk_limits.max_direction_budget_quote = Some(Decimal::new(150, 0));

        let result = run_kline_screening(portfolio, &falling_bars()).unwrap();

        assert!(!result.metrics.survival_passed);
        assert!(result
            .rejection_reasons
            .iter()
            .any(|reason| reason.contains("direction budget")));
    }

    #[test]
    fn amount_take_profit_closes_when_net_pnl_reaches_quote_target() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0].take_profit = MartingaleTakeProfitModel::Amount {
            quote: Decimal::new(1, 0),
        };

        let result = run_kline_screening(
            portfolio,
            &[
                bar(1_000, 100.0, 100.0, 100.0, 100.0),
                bar(2_000, 100.0, 101.5, 100.0, 101.2),
            ],
        )
        .unwrap();

        assert!(result.metrics.survival_passed);
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "take_profit"));
    }

    #[test]
    fn atr_take_profit_uses_latest_warmed_atr() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0].take_profit = MartingaleTakeProfitModel::Atr {
            multiplier: Decimal::new(1, 0),
        };

        let result = run_kline_screening(
            portfolio,
            &[
                bar(1_000, 100.0, 101.0, 99.0, 100.0),
                bar(2_000, 100.0, 101.0, 99.0, 100.0),
                bar(3_000, 100.0, 102.5, 100.0, 102.2),
            ],
        )
        .unwrap();

        assert!(result.metrics.survival_passed);
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "take_profit"));
    }

    #[test]
    fn trailing_take_profit_waits_for_callback_after_activation() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0].take_profit = MartingaleTakeProfitModel::Trailing {
            activation_bps: 100,
            callback_bps: 50,
        };

        let result = run_kline_screening(
            portfolio,
            &[
                bar(1_000, 100.0, 100.0, 100.0, 100.0),
                bar(2_000, 100.0, 101.2, 100.8, 101.1),
                bar(3_000, 101.1, 101.1, 100.4, 100.6),
            ],
        )
        .unwrap();

        assert!(result.metrics.survival_passed);
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "take_profit"));
    }

    #[test]
    fn mixed_take_profit_supports_amount_or_percent_phase() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0].take_profit = MartingaleTakeProfitModel::Mixed {
            phases: vec![
                MartingaleTakeProfitModel::Amount {
                    quote: Decimal::new(1, 0),
                },
                MartingaleTakeProfitModel::Percent { bps: 200 },
            ],
        };

        let result = run_kline_screening(
            portfolio,
            &[
                bar(1_000, 100.0, 100.0, 100.0, 100.0),
                bar(2_000, 100.0, 101.5, 100.0, 101.2),
            ],
        )
        .unwrap();

        assert!(result.metrics.survival_passed);
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "take_profit"));
    }

    #[test]
    fn short_cycle_adds_safety_order_and_takes_profit() {
        let result = run_kline_screening(
            portfolio_with_direction(MartingaleDirection::Short, 1_000),
            &rising_bars(),
        )
        .unwrap();

        assert!(result.metrics.survival_passed);
        assert_eq!(result.metrics.trade_count, 3);
        assert!(result.metrics.max_capital_used_quote > 300.0);
        assert!(result.metrics.total_return_pct > 0.0);
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "safety_order"));
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "take_profit"));
    }

    #[test]
    fn stop_loss_has_priority_over_take_profit_in_same_bar() {
        let bars = vec![
            bar(1_000, 100.0, 100.0, 100.0, 100.0),
            bar(2_000, 100.0, 102.0, 98.0, 99.0),
        ];

        let result = run_kline_screening(stop_loss_portfolio(), &bars).unwrap();

        assert_eq!(result.metrics.stop_count, 1);
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "stop_loss"));
        assert!(!result
            .events
            .iter()
            .any(|event| event.event_type == "take_profit"));
    }

    #[test]
    fn strategy_drawdown_stop_uses_net_pnl_including_costs() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0].stop_loss =
            Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps: 10 });

        let result =
            run_kline_screening(portfolio, &[bar(1_000, 100.0, 100.0, 99.98, 99.98)]).unwrap();

        assert_eq!(result.metrics.stop_count, 1);
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "stop_loss"));
    }

    #[test]
    fn multi_symbol_equity_keeps_other_symbol_unrealized_pnl() {
        let bars = vec![
            symbol_bar("BTCUSDT", 1_000, 100.0, 100.0, 100.0, 100.0),
            symbol_bar("BTCUSDT", 2_000, 100.0, 100.0, 99.0, 99.0),
            symbol_bar("ETHUSDT", 3_000, 50.0, 50.0, 50.0, 50.0),
        ];

        let result = run_kline_screening(two_symbol_portfolio(), &bars).unwrap();

        assert!(result.equity_curve[1].equity_quote < 999.0);
        assert!(result.equity_curve[2].equity_quote < 999.0);
    }

    #[test]
    fn atr_stop_loss_triggers_after_warmup() {
        let mut portfolio = portfolio_with_stop_loss(MartingaleStopLossModel::Atr {
            multiplier: Decimal::new(1, 0),
        });
        portfolio.strategies[0].take_profit = MartingaleTakeProfitModel::Percent { bps: 9_000 };
        let result = run_kline_screening(
            portfolio,
            &[
                bar(1_000, 100.0, 101.0, 99.0, 100.0),
                bar(2_000, 100.0, 101.0, 99.0, 100.0),
                bar(3_000, 100.0, 100.2, 95.0, 97.0),
            ],
        )
        .unwrap();

        assert!(result.metrics.survival_passed);
        assert_eq!(result.metrics.stop_count, 1);
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "stop_loss"));
    }

    #[test]
    fn indicator_stop_loss_triggers_when_expression_is_true() {
        let result = run_kline_screening(
            portfolio_with_stop_loss(MartingaleStopLossModel::Indicator {
                expression: "close < sma(2)".to_string(),
            }),
            &[
                bar(1_000, 100.0, 100.0, 100.0, 100.0),
                bar(2_000, 100.0, 100.0, 100.0, 100.0),
                bar(3_000, 100.0, 100.0, 98.0, 98.0),
            ],
        )
        .unwrap();

        assert!(result.metrics.survival_passed);
        assert_eq!(result.metrics.stop_count, 1);
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "stop_loss"));
    }

    #[test]
    fn total_return_includes_final_unrealized_loss() {
        let result = run_kline_screening(
            single_strategy_portfolio(1_000),
            &[
                bar(1_000, 100.0, 100.0, 100.0, 100.0),
                bar(2_000, 100.0, 100.0, 99.0, 99.0),
            ],
        )
        .unwrap();

        assert!(result.metrics.total_return_pct < 0.0);
        assert!(result.metrics.total_return_pct < -0.1);
    }

    #[test]
    fn price_range_entry_trigger_blocks_first_order_when_close_outside_range() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0].entry_triggers = vec![MartingaleEntryTrigger::PriceRange {
            lower: Decimal::new(90, 0),
            upper: Decimal::new(95, 0),
        }];

        let result =
            run_kline_screening(portfolio, &[bar(1_000, 100.0, 100.0, 100.0, 100.0)]).unwrap();

        assert!(result.metrics.survival_passed);
        assert_eq!(result.metrics.trade_count, 0);
        assert!(result.events.is_empty());
    }

    #[test]
    fn cooldown_entry_trigger_blocks_reentry_until_elapsed() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0].take_profit = MartingaleTakeProfitModel::Amount {
            quote: Decimal::new(1, 0),
        };
        portfolio.strategies[0].entry_triggers =
            vec![MartingaleEntryTrigger::Cooldown { seconds: 60 }];

        let result = run_kline_screening(
            portfolio,
            &[
                bar(1_000, 100.0, 101.5, 100.0, 101.2),
                bar(30_000, 101.2, 101.2, 101.2, 101.2),
                bar(70_000, 101.2, 101.2, 101.2, 101.2),
            ],
        )
        .unwrap();

        assert!(result.metrics.survival_passed);
        assert_eq!(
            result
                .events
                .iter()
                .filter(|event| event.event_type == "entry")
                .count(),
            2
        );
    }

    #[test]
    fn time_window_entry_trigger_allows_only_inside_window() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0].entry_triggers = vec![MartingaleEntryTrigger::TimeWindow {
            start: "00:01".to_string(),
            end: "00:02".to_string(),
        }];

        let result = run_kline_screening(
            portfolio,
            &[
                bar(30_000, 100.0, 100.0, 100.0, 100.0),
                bar(70_000, 100.0, 100.0, 100.0, 100.0),
            ],
        )
        .unwrap();

        assert!(result.metrics.survival_passed);
        assert_eq!(result.events[0].event_type, "entry");
        assert_eq!(result.events[0].timestamp_ms, 70_000);
    }

    #[test]
    fn indicator_expression_entry_trigger_waits_for_true_expression() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0].entry_triggers =
            vec![MartingaleEntryTrigger::IndicatorExpression {
                expression: "close > sma(2)".to_string(),
            }];

        let result = run_kline_screening(
            portfolio,
            &[
                bar(1_000, 100.0, 100.0, 100.0, 100.0),
                bar(2_000, 100.0, 100.0, 100.0, 100.0),
                bar(3_000, 101.0, 101.0, 101.0, 101.0),
            ],
        )
        .unwrap();

        assert!(result.metrics.survival_passed);
        assert_eq!(result.events[0].event_type, "entry");
        assert_eq!(result.events[0].timestamp_ms, 3_000);
    }

    #[test]
    fn conservative_costs_reduce_take_profit_return() {
        let result = run_kline_screening(
            single_strategy_portfolio(1_000),
            &[
                bar(1_000, 100.0, 100.0, 100.0, 100.0),
                bar(2_000, 100.0, 101.1, 100.0, 101.0),
            ],
        )
        .unwrap();

        let entry = result
            .events
            .iter()
            .find(|event| event.event_type == "entry")
            .expect("entry event should exist");
        assert!(entry.detail.contains("fee_quote="));
        assert!(entry.detail.contains("slippage_quote="));
        assert!(result.metrics.total_return_pct > 0.0);
        // Return is normalized by planned margin capital (here 300 for the spot
        // [100,200] leg sequence), not the 1000 budget allocation, so the net
        // return after costs is small but above the old budget-based bound.
        assert!(result.metrics.total_return_pct < 0.5);
    }

    #[test]
    fn default_cost_model_charges_0045_percent_fee_and_002_percent_slippage_per_fill() {
        let result = run_kline_screening(
            single_strategy_portfolio(1_000),
            &[
                bar(1_000, 100.0, 100.0, 100.0, 100.0),
                bar(2_000, 100.0, 101.1, 100.0, 101.0),
            ],
        )
        .unwrap();

        let entry = result
            .trades
            .iter()
            .find(|trade| trade.event_type == "open_leg")
            .expect("entry trade should exist");
        assert!((entry.notional_quote - 100.0).abs() < 1e-9);
        assert!((entry.fee_quote - 0.045).abs() < 1e-9);
        assert!((entry.slippage_quote - 0.02).abs() < 1e-9);

        let exit = result
            .trades
            .iter()
            .find(|trade| trade.event_type == "close_cycle")
            .expect("exit trade should exist");
        assert!(exit.fee_quote > 0.045, "exit fee uses close notional");
        assert!(
            exit.slippage_quote > 0.02,
            "exit slippage uses close notional"
        );
        assert!(
            result.metrics.total_fee_quote.unwrap() > entry.fee_quote,
            "metrics include entry and exit fees"
        );
        assert!(
            result.metrics.total_slippage_quote.unwrap() > entry.slippage_quote,
            "metrics include entry and exit slippage"
        );
    }

    #[test]
    fn completed_cycle_releases_entry_cost_capital_before_next_cycle() {
        let result = run_kline_screening(
            single_strategy_portfolio(101),
            &[
                bar(1_000, 100.0, 101.1, 100.0, 101.0),
                bar(2_000, 100.0, 101.1, 100.0, 101.0),
            ],
        )
        .unwrap();

        assert!(result.metrics.survival_passed);
        assert_eq!(
            result
                .events
                .iter()
                .filter(|event| event.event_type == "entry")
                .count(),
            2
        );
        assert!(result.rejection_reasons.is_empty());
    }

    #[test]
    fn global_drawdown_stop_snapshot_stops_all_matching_strategies_same_bar() {
        let mut portfolio = two_symbol_portfolio();
        portfolio.risk_limits.max_global_drawdown_quote = Some(Decimal::new(1_000, 0));
        for strategy in &mut portfolio.strategies {
            strategy.stop_loss = Some(MartingaleStopLossModel::GlobalDrawdownAmount {
                quote: Decimal::new(1, 0),
            });
        }

        let result = run_kline_screening(
            portfolio,
            &[
                symbol_bar("BTCUSDT", 1_000, 100.0, 100.0, 100.0, 100.0),
                symbol_bar("ETHUSDT", 1_000, 100.0, 100.0, 100.0, 100.0),
                symbol_bar("BTCUSDT", 2_000, 100.0, 100.0, 98.0, 98.0),
                symbol_bar("ETHUSDT", 2_000, 100.0, 100.0, 98.0, 98.0),
            ],
        )
        .unwrap();

        assert_eq!(result.metrics.stop_count, 2);
        assert_eq!(
            result
                .events
                .iter()
                .filter(|event| event.event_type == "global_stop_loss")
                .count(),
            2
        );
    }

    #[test]
    fn budget_rejected_safety_order_still_allows_existing_cycle_take_profit() {
        let result = run_kline_screening(
            single_strategy_portfolio(150),
            &[
                bar(1_000, 100.0, 100.0, 100.0, 100.0),
                bar(2_000, 100.0, 100.0, 98.9, 99.0),
                bar(3_000, 99.0, 101.2, 99.0, 101.0),
                bar(4_000, 101.0, 101.0, 101.0, 101.0),
            ],
        )
        .unwrap();

        assert!(!result.metrics.survival_passed);
        assert!(result
            .rejection_reasons
            .iter()
            .any(|reason| reason.contains("budget")));
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "take_profit"));
        assert_eq!(
            result
                .events
                .iter()
                .filter(|event| event.event_type == "entry")
                .count(),
            2
        );
        assert!(result.events.iter().any(|event| {
            event.event_type == "entry" && event.cycle_id.as_deref() == Some("Long-grid-cycle-2")
        }));
    }

    #[test]
    fn long_position_pays_positive_funding_in_equity_curve() {
        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.strategies[0].take_profit = MartingaleTakeProfitModel::Percent { bps: 9_000 };
        let bars = vec![
            bar(1_000, 100.0, 100.0, 100.0, 100.0),
            bar(2_000, 100.0, 100.0, 100.0, 100.0),
        ];
        let with_funding = run_kline_screening_with_funding(
            portfolio.clone(),
            &bars,
            &[FundingRatePoint {
                symbol: "BTCUSDT".to_string(),
                funding_time_ms: 2_000,
                funding_rate: 0.01,
                mark_price: Some(100.0),
            }],
        )
        .unwrap();
        let without_funding = run_kline_screening(portfolio, &bars).unwrap();

        assert_eq!(with_funding.metrics.total_funding_quote, Some(-1.0));
        assert!(with_funding
            .events
            .iter()
            .any(|event| event.event_type == "funding_fee"));
        let funded_equity = with_funding.equity_curve.last().unwrap().equity_quote;
        let plain_equity = without_funding.equity_curve.last().unwrap().equity_quote;
        assert!((plain_equity - funded_equity - 1.0).abs() < 0.000001);
    }

    #[test]
    fn short_position_receives_positive_funding_in_equity_curve() {
        let mut portfolio = portfolio_with_direction(MartingaleDirection::Short, 1_000);
        portfolio.strategies[0].take_profit = MartingaleTakeProfitModel::Percent { bps: 9_000 };
        let bars = vec![
            bar(1_000, 100.0, 100.0, 100.0, 100.0),
            bar(2_000, 100.0, 100.0, 100.0, 100.0),
        ];
        let with_funding = run_kline_screening_with_funding(
            portfolio.clone(),
            &bars,
            &[FundingRatePoint {
                symbol: "BTCUSDT".to_string(),
                funding_time_ms: 2_000,
                funding_rate: 0.01,
                mark_price: Some(100.0),
            }],
        )
        .unwrap();
        let without_funding = run_kline_screening(portfolio, &bars).unwrap();

        assert_eq!(with_funding.metrics.total_funding_quote, Some(1.0));
        let funded_equity = with_funding.equity_curve.last().unwrap().equity_quote;
        let plain_equity = without_funding.equity_curve.last().unwrap().equity_quote;
        assert!((funded_equity - plain_equity - 1.0).abs() < 0.000001);
    }

    #[test]
    fn global_drawdown_uses_same_timestamp_cross_symbol_prices() {
        let mut portfolio = two_symbol_portfolio();
        portfolio.risk_limits.max_global_drawdown_quote = Some(Decimal::new(1_000, 0));
        for strategy in &mut portfolio.strategies {
            strategy.stop_loss = Some(MartingaleStopLossModel::GlobalDrawdownAmount {
                quote: Decimal::new(3, 0),
            });
        }

        let result = run_kline_screening(
            portfolio,
            &[
                symbol_bar("BTCUSDT", 1_000, 100.0, 100.0, 100.0, 100.0),
                symbol_bar("ETHUSDT", 1_000, 100.0, 100.0, 100.0, 100.0),
                symbol_bar("BTCUSDT", 2_000, 100.0, 100.0, 98.0, 98.0),
                symbol_bar("ETHUSDT", 2_000, 100.0, 100.0, 98.0, 98.0),
            ],
        )
        .unwrap();

        assert_eq!(result.metrics.stop_count, 2);
        assert_eq!(
            result
                .events
                .iter()
                .filter(|event| event.event_type == "global_stop_loss")
                .count(),
            2
        );
    }

    #[test]
    fn capacity_trigger_compares_runtime_active_cycle_count() {
        assert!(capacity_allows_entry(1, 0));
        assert!(!capacity_allows_entry(1, 1));
        assert!(!capacity_allows_entry(1, 2));
        assert!(!capacity_allows_entry(0, 0));
    }

    #[test]
    fn same_bar_safety_order_is_added_before_stop_for_conservative_path() {
        let bars = vec![
            bar(1_000, 100.0, 100.0, 100.0, 100.0),
            bar(2_000, 100.0, 101.0, 98.0, 99.0),
        ];

        let result = run_kline_screening(stop_loss_portfolio(), &bars).unwrap();

        assert!(result.metrics.max_capital_used_quote > 300.0);
        assert_eq!(result.metrics.stop_count, 1);
    }

    // --- Task 0: deterministic correctness tests ---

    fn test_bar(open_time_ms: i64, open: f64, high: f64, low: f64, close: f64) -> KlineBar {
        bar(open_time_ms, open, high, low, close)
    }

    #[test]
    fn deterministic_long_martingale_cycle_adds_safety_order_and_exits_positive_after_fees() {
        let bars = vec![
            test_bar(0, 100.0, 100.0, 100.0, 100.0),
            test_bar(60_000, 100.0, 100.0, 98.8, 99.0),
            test_bar(120_000, 99.0, 101.0, 99.0, 100.8),
        ];

        let mut portfolio = single_strategy_portfolio(1_000);
        let strategy = &mut portfolio.strategies[0];
        strategy.direction = MartingaleDirection::Long;
        strategy.spacing = MartingaleSpacingModel::FixedPercent { step_bps: 100 };
        strategy.sizing = MartingaleSizingModel::Multiplier {
            first_order_quote: Decimal::new(10, 0),
            multiplier: Decimal::new(2, 0),
            max_legs: 2,
        };
        strategy.take_profit = MartingaleTakeProfitModel::Percent { bps: 60 };
        strategy.stop_loss = Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps: 2_000 });

        let result = run_kline_screening(portfolio, &bars).expect("long cycle should run");
        assert!(
            result.metrics.total_return_pct > 0.0,
            "expected positive long martingale cycle after fees: {:?}",
            result.metrics
        );
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "safety_order"));
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "take_profit"));
        assert!(result
            .trades
            .iter()
            .any(|trade| trade.event_type == "open_leg"));
        assert!(result
            .trades
            .iter()
            .any(|trade| trade.event_type == "close_cycle"));
        assert!(
            result.metrics.trade_count <= 10,
            "trade count should count orders/exits, not churn: {}",
            result.metrics.trade_count
        );
    }

    #[test]
    fn deterministic_short_martingale_cycle_adds_safety_order_and_exits_positive_after_fees() {
        let bars = vec![
            test_bar(0, 100.0, 100.0, 100.0, 100.0),
            test_bar(60_000, 100.0, 101.2, 100.0, 101.0),
            test_bar(120_000, 101.0, 101.0, 99.0, 99.2),
        ];

        let mut portfolio = single_strategy_portfolio(1_000);
        let strategy = &mut portfolio.strategies[0];
        strategy.direction = MartingaleDirection::Short;
        strategy.direction_mode = MartingaleDirectionMode::ShortOnly;
        strategy.spacing = MartingaleSpacingModel::FixedPercent { step_bps: 100 };
        strategy.sizing = MartingaleSizingModel::Multiplier {
            first_order_quote: Decimal::new(10, 0),
            multiplier: Decimal::new(2, 0),
            max_legs: 2,
        };
        strategy.take_profit = MartingaleTakeProfitModel::Percent { bps: 60 };
        strategy.stop_loss = Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps: 2_000 });

        let result = run_kline_screening(portfolio, &bars).expect("short cycle should run");
        assert!(
            result.metrics.total_return_pct > 0.0,
            "expected positive short martingale cycle after fees: {:?}",
            result.metrics
        );
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "safety_order"));
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == "take_profit"));
        assert!(
            result.metrics.trade_count <= 10,
            "trade count should count orders/exits, not churn: {}",
            result.metrics.trade_count
        );
    }

    #[test]
    fn deterministic_long_short_allows_independent_leg_parameters_and_positive_survivor() {
        let bars = vec![
            test_bar(0, 100.0, 100.0, 100.0, 100.0),
            test_bar(60_000, 100.0, 100.0, 98.8, 99.0),
            test_bar(120_000, 99.0, 101.0, 99.0, 100.8),
            test_bar(180_000, 100.8, 100.9, 100.5, 100.7),
        ];

        let mut portfolio = single_strategy_portfolio(1_000);
        portfolio.direction_mode = MartingaleDirectionMode::LongAndShort;

        let mut long_strategy = portfolio.strategies[0].clone();
        long_strategy.strategy_id = "long-leg".to_owned();
        long_strategy.direction = MartingaleDirection::Long;
        long_strategy.direction_mode = MartingaleDirectionMode::LongAndShort;
        long_strategy.spacing = MartingaleSpacingModel::FixedPercent { step_bps: 100 };
        long_strategy.take_profit = MartingaleTakeProfitModel::Percent { bps: 60 };
        long_strategy.stop_loss =
            Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps: 2_000 });

        let mut short_strategy = long_strategy.clone();
        short_strategy.strategy_id = "short-leg".to_owned();
        short_strategy.direction = MartingaleDirection::Short;
        short_strategy.spacing = MartingaleSpacingModel::FixedPercent { step_bps: 300 };
        short_strategy.take_profit = MartingaleTakeProfitModel::Percent { bps: 140 };
        short_strategy.stop_loss =
            Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps: 800 });

        portfolio.strategies = vec![long_strategy, short_strategy];

        let result = run_kline_screening(portfolio, &bars).expect("long_short cycle should run");
        assert!(result.metrics.trade_count > 0);
        assert!(
            result.metrics.trade_count < 10,
            "long_short deterministic test should not churn: {}",
            result.metrics.trade_count
        );
        assert!(result
            .events
            .iter()
            .any(|event| event.strategy_instance_id == "long-leg"));
        assert!(result
            .events
            .iter()
            .any(|event| event.strategy_instance_id == "short-leg"));
        assert!(result.metrics.total_return_pct.is_finite());
        assert!(result.metrics.max_drawdown_pct.is_finite());
    }

    #[test]
    fn long_short_return_and_drawdown_use_planned_margin_not_notional_or_first_order_only() {
        let bars = vec![
            test_bar(0, 100.0, 100.0, 100.0, 100.0),
            test_bar(60_000, 100.0, 100.0, 98.8, 99.0),
            test_bar(120_000, 99.0, 101.0, 99.0, 100.8),
        ];

        let mut portfolio = MartingalePortfolioConfig {
            direction_mode: MartingaleDirectionMode::LongAndShort,
            strategies: vec![MartingaleStrategyConfig {
                strategy_id: "long-leg".to_string(),
                symbol: "BTCUSDT".to_string(),
                market: MartingaleMarketKind::UsdMFutures,
                direction: MartingaleDirection::Long,
                direction_mode: MartingaleDirectionMode::LongAndShort,
                margin_mode: Some(MartingaleMarginMode::Isolated),
                leverage: Some(2),
                spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
                sizing: MartingaleSizingModel::Multiplier {
                    first_order_quote: Decimal::new(10, 0),
                    multiplier: Decimal::new(2, 0),
                    max_legs: 3,
                },
                take_profit: MartingaleTakeProfitModel::Percent { bps: 60 },
                stop_loss: Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps: 2_000 }),
                indicators: Vec::new(),
                entry_triggers: vec![MartingaleEntryTrigger::Immediate],
                risk_limits: MartingaleRiskLimits {
                    max_strategy_budget_quote: Some(Decimal::new(70, 0)),
                    ..MartingaleRiskLimits::default()
                },
            }],
            risk_limits: MartingaleRiskLimits {
                max_global_budget_quote: Some(Decimal::new(100, 0)),
                ..MartingaleRiskLimits::default()
            },
        };

        let mut short_strategy = portfolio.strategies[0].clone();
        short_strategy.strategy_id = "short-leg".to_owned();
        short_strategy.direction = MartingaleDirection::Short;
        short_strategy.risk_limits.max_strategy_budget_quote = Some(Decimal::new(30, 0));
        portfolio.strategies.push(short_strategy);

        let result = run_kline_screening(portfolio, &bars).expect("screening should run");
        assert!(
            result.metrics.planned_margin_quote.is_some_and(|m| m > 0.0),
            "planned_margin should be positive"
        );
        assert!(result.metrics.total_return_pct.is_finite());
        assert!(result.metrics.max_drawdown_pct.is_finite());
        assert!(
            result.metrics.total_return_pct.abs() < 100.0,
            "return should be normalized by planned margin, got {}",
            result.metrics.total_return_pct
        );
        assert!(
            result.metrics.max_drawdown_pct < 100.0,
            "drawdown should be normalized by planned margin, got {}",
            result.metrics.max_drawdown_pct
        );
    }

    #[test]
    fn futures_leverage_reduces_margin_while_notional_stays_fixed() {
        let bars = vec![
            test_bar(0, 100.0, 100.0, 100.0, 100.0),
            test_bar(60_000, 100.0, 101.0, 100.0, 101.0),
        ];

        let mut two_x = single_strategy_portfolio(1_000);
        two_x.strategies[0].market = MartingaleMarketKind::UsdMFutures;
        two_x.strategies[0].margin_mode = Some(MartingaleMarginMode::Isolated);
        two_x.strategies[0].leverage = Some(2);
        two_x.strategies[0].sizing = MartingaleSizingModel::Multiplier {
            first_order_quote: Decimal::new(10, 0),
            multiplier: Decimal::new(1, 0),
            max_legs: 1,
        };
        two_x.strategies[0].take_profit = MartingaleTakeProfitModel::Percent { bps: 60 };
        two_x.strategies[0].entry_triggers = vec![MartingaleEntryTrigger::Immediate];

        let mut four_x = two_x.clone();
        four_x.strategies[0].leverage = Some(4);

        let two_x_result = run_kline_screening(two_x, &bars).expect("2x run should succeed");
        let four_x_result = run_kline_screening(four_x, &bars).expect("4x run should succeed");

        // Notional is the leveraged position size and is independent of leverage.
        assert_eq!(two_x_result.metrics.planned_notional_quote, Some(10.0));
        assert_eq!(four_x_result.metrics.planned_notional_quote, Some(10.0));
        // Margin = notional / leverage: 2x => 5, 4x => 2.5.
        assert_eq!(two_x_result.metrics.planned_margin_quote, Some(5.0));
        assert_eq!(four_x_result.metrics.planned_margin_quote, Some(2.5));
        // Same PnL over a smaller margin base => higher return at higher leverage.
        assert!(
            four_x_result.metrics.total_return_pct > two_x_result.metrics.total_return_pct * 1.5,
            "higher leverage should materially increase return on the same notional: 2x={}, 4x={}",
            two_x_result.metrics.total_return_pct,
            four_x_result.metrics.total_return_pct
        );
        // Trade details expose margin (= notional/leverage) and notional separately.
        assert!(
            four_x_result
                .trades
                .iter()
                .any(|trade| (trade.margin_quote - 2.5).abs() < 0.000001
                    && (trade.notional_quote - 10.0).abs() < 0.000001),
            "expected trade details to expose margin and notional separately: {:?}",
            four_x_result.trades
        );
    }

    fn trending_bars(
        symbol: &str,
        start_ms: i64,
        count: usize,
        start_price: f64,
        end_price: f64,
    ) -> Vec<KlineBar> {
        (0..count)
            .map(|index| {
                let t = index as f64 / (count.saturating_sub(1).max(1)) as f64;
                let close = start_price + (end_price - start_price) * t;
                KlineBar {
                    symbol: symbol.to_owned(),
                    open_time_ms: start_ms + index as i64 * 60_000,
                    open: close,
                    high: close * 1.001,
                    low: close * 0.999,
                    close,
                    volume: 1.0,
                }
            })
            .collect()
    }

    #[test]
    fn long_short_cooldown_entry_trigger_prevents_every_bar_churn() {
        let bars = trending_bars("BTCUSDT", 1_672_531_200_000, 1_000, 20_000.0, 20_500.0);
        let mut portfolio = portfolio_with_direction(MartingaleDirection::Long, 10_000);
        portfolio.direction_mode = MartingaleDirectionMode::LongAndShort;

        let mut long_strategy = portfolio.strategies[0].clone();
        long_strategy.direction = MartingaleDirection::Long;
        long_strategy.direction_mode = MartingaleDirectionMode::LongAndShort;
        long_strategy.entry_triggers = vec![MartingaleEntryTrigger::Cooldown { seconds: 21_600 }];

        let mut short_strategy = long_strategy.clone();
        short_strategy.strategy_id = "short".to_owned();
        short_strategy.direction = MartingaleDirection::Short;
        short_strategy.entry_triggers = vec![MartingaleEntryTrigger::Cooldown { seconds: 21_600 }];

        portfolio.strategies = vec![long_strategy, short_strategy];

        let result = run_kline_screening(portfolio, &bars).expect("screening should run");
        assert!(result.metrics.trade_count > 0);
        assert!(
            result.metrics.trade_count < 400,
            "trade count should not churn every bar: {}",
            result.metrics.trade_count
        );
    }
}
