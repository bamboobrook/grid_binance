//! Single-symbol gated Martin scored engine (plan §5.2 execution path).
//!
//! This is a self-contained, production-conservative single-symbol Martin that
//! REUSES the exchange-realism primitives from
//! `backtest_engine::martingale::r21_conservative_engine` and `exchange_model`
//! (filter_order_conservative, check_liquidation_buffer, apply_partial_fill,
//! next_so_close_maintenance_reserve_ok). It accepts an external **signal
//! gating callback** so M1 (OI/crowding) or M2 (depth/flow) flags decide when
//! FO / SO / TP / abort may fire — exactly plan §1.10 ("OI/flow/depth/factor
//! only decide FO/SO/TP/abort/freeze") and §5.2 ("each order is an OrderIntent
//! through real filters").
//!
//! The engine implements the §5.1 accounting:
//! - signed quantity + fill price + market identity;
//! - every fill charged fee + slippage;
//! - event-time liquidation check via the maintenance tier;
//! - reserve must cover next SO + close + maintenance before any FO;
//! - principal breach terminates immediately.
//!
//! It is deliberately simple (single symbol, soft ladder per plan §8) but
//! routes every order through the same conservative gates the pair engine uses.

use serde::{Deserialize, Serialize};

use backtest_engine::martingale::exchange_model::ExchangeFilters;
use backtest_engine::martingale::metrics::{
    calculate_annualized_return_pct, MartingaleBacktestEvent, MartingaleBacktestResult,
    DrawdownPoint, EquityPoint,
};
use backtest_engine::martingale::r21_conservative_engine::{
    check_liquidation_buffer, filter_order_conservative, next_so_close_maintenance_reserve_ok,
};
use backtest_engine::market_data::KlineBar;

/// Soft-ladder relative layer gross (plan §8: [1.00, 1.25, 1.55, 1.90]).
pub const SOFT_LADDER: [f64; 4] = [1.00, 1.25, 1.55, 1.90];

/// Signal gating decision at a bar. Produced by the external signal layer
/// (M1/M2). The engine only acts when these allow it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignalGate {
    /// Long FO allowed at this bar.
    pub long_fo: bool,
    /// Short FO allowed at this bar.
    pub short_fo: bool,
    /// SO allowed only when the position is adverse AND exhaustion/replenishment present.
    pub so_allowed: bool,
    /// Forced abort/flatten at this bar (cointegration break / depth withdrawal).
    pub force_abort: bool,
}

/// A signal-gating function: given the bar index + state, return a SignalGate.
/// It MUST be causal (only past data).
pub trait SignalSource {
    fn gate(&self, bar_idx: usize, ts_ms: i64, price: f64) -> SignalGate;
}

/// Engine config.
#[derive(Debug, Clone)]
pub struct GatedMartinConfig {
    pub symbol: String,
    pub filters: ExchangeFilters,
    pub budget_quote: f64,
    /// FO quote (USDT) for the first layer.
    pub fo_quote: f64,
    /// Adverse spacing (as a fraction of price) required since the last fill
    /// before an SO may fire. Mirrors "adverse distance from last executed fill".
    pub adverse_spacing_frac: f64,
    /// TP net bps floor (after fee+slippage).
    pub tp_net_bps_floor: f64,
    /// Fee bps and slippage bps.
    pub fee_bps: f64,
    pub slippage_bps: f64,
    /// Leverage for margin/liquidation.
    pub leverage: u32,
    /// Direction bias: +1 long-only, -1 short-only, 0 both (signal decides).
    pub direction_bias: i8,
    /// TP mode: Fixed (full close at tp_net_bps_floor), Early (lower floor =
    /// tp_net_bps_floor * early_scale), ScaleOut (partial close: half at floor,
    /// rest at 2x floor), TimeDecay (floor decays with bars-in-position).
    pub tp_mode: TpMode,
    /// Maximum number of legs (FO + SOs). Caps how deep averaging-down goes.
    /// Lower = smaller max DD but fewer recovery fills. Default 4 (full ladder).
    pub max_legs: u32,
}

/// Take-profit mode for smoothing the return distribution (reduce kurtosis).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpMode {
    /// Full close when net >= tp_net_bps_floor.
    Fixed,
    /// Full close at a lower floor (tp_net_bps_floor * early_scale). Reduces
    /// variance by locking smaller wins more often.
    Early,
    /// Partial scale-out: close 50% at tp_net_bps_floor, rest at 2x floor.
    ScaleOut,
    /// Time-decay: floor shrinks as bars-in-position grows, forcing earlier exit
    /// on stale positions. floor = tp_net_bps_floor * max(0.25, 1 - bars/decay_bars).
    TimeDecay { decay_bars: u32 },
}

#[derive(Debug, Clone, Default)]
struct Position {
    /// signed quantity in base units (positive long, negative short)
    qty: f64,
    /// weighted average entry price
    avg_entry: f64,
    /// realized PnL (quote)
    realized: f64,
    /// total fees paid (quote)
    fees: f64,
    /// total slippage cost (quote)
    slippage: f64,
    /// depth (number of layers filled so far, 1 = FO only)
    depth: u32,
    /// last fill price (for adverse-distance SO basis, plan §4.8)
    last_fill_price: f64,
    /// last fill timestamp
    last_fill_ts: i64,
    /// bars since the position was opened (for time-decay TP).
    bars_in_position: u32,
    /// for ScaleOut TP: true once the first half has been closed.
    scaled_out_once: bool,
}

/// Run the gated single-symbol Martin over a bar stream with a signal source.
/// Returns the standard MartingaleBacktestResult so existing metric/digest
/// infrastructure works, plus a SYNC_SUMMARY line.
pub fn run_gated_martin<S: SignalSource>(
    cfg: &GatedMartinConfig,
    bars: &[KlineBar],
    signal: &S,
) -> Result<MartingaleBacktestResult, String> {
    let mut equity = cfg.budget_quote;
    let initial_equity = cfg.budget_quote;
    let mut pos = Position::default();
    let mut events: Vec<MartingaleBacktestEvent> = Vec::new();
    let mut equity_curve: Vec<EquityPoint> = Vec::with_capacity(bars.len());
    let mut trade_count: u64 = 0;
    let mut fo_count: u64 = 0;
    let mut so_count: u64 = 0;
    let mut tp_count: u64 = 0;
    let mut abort_count: u64 = 0;
    let mut liquidation_count: u64 = 0;
    let mut breach = false;

    for (i, b) in bars.iter().enumerate() {
        if b.close <= 0.0 {
            continue;
        }
        let ts = b.open_time_ms;
        let price = b.close;
        let gate = signal.gate(i, ts, price);

        // ---- event-time liquidation check (§5.1.7 / §5.2) ----
        if pos.qty.abs() > 1e-12 {
            let pos_notional = pos.qty.abs() * price;
            let unreal = unrealized(&pos, price);
            let avail_margin = equity.max(0.0);
            let (_, would_liq) = check_liquidation_buffer(
                &cfg.filters,
                pos_notional,
                -unreal.max(0.0),
                avail_margin,
            );
            // The conservative primitive returns would_liquidate when buffer too thin.
            // We treat a deep-enough loss that breaches maintenance as liquidation.
            let maint = cfg.filters.maintenance_margin(pos_notional);
            if would_liq || (unreal < 0.0 && unreal.abs() + maint > avail_margin) {
                liquidation_count += 1;
                breach = true;
                events.push(MartingaleBacktestEvent {
                    timestamp_ms: ts,
                    event_type: "gated_liquidation".to_string(),
                    symbol: cfg.symbol.clone(),
                    strategy_instance_id: "M1".to_string(),
                    cycle_id: None,
                    detail: format!("equity={:.2} unreal={:.2} maint={:.2}", equity, unreal, maint),
                });
                // liquidation terminates the shared account (§5.1.12)
                equity = 0.0;
                pos = Position::default();
                equity_curve.push(EquityPoint { timestamp_ms: ts, equity_quote: 0.0 });
                break;
            }
        }

        // ---- forced abort (cointegration break / depth withdrawal) ----
        if gate.force_abort && pos.qty.abs() > 1e-12 {
            let (settled, fee, slip) = close_position(&mut pos, price, cfg);
            equity += settled;
            pos.fees += fee;
            pos.slippage += slip;
            abort_count += 1;
            events.push(ev(ts, "gated_abort", price, settled));
            continue;
        }

        // ---- TP if in profit beyond floor (mode-dependent) ----
        if pos.qty.abs() > 1e-12 {
            pos.bars_in_position = pos.bars_in_position.saturating_add(1);
            let net = net_pnl_after_cost(&pos, price, cfg);
            let notional = pos.qty.abs() * pos.avg_entry;
            // Compute the effective floor based on tp_mode.
            let (floor_bps, partial_close) = match cfg.tp_mode {
                TpMode::Fixed => (cfg.tp_net_bps_floor, false),
                TpMode::Early => (cfg.tp_net_bps_floor * 0.5, false),
                TpMode::ScaleOut => {
                    if pos.scaled_out_once {
                        // second half closes at 2x floor
                        (cfg.tp_net_bps_floor * 2.0, false)
                    } else {
                        // first half closes at floor
                        (cfg.tp_net_bps_floor, true)
                    }
                }
                TpMode::TimeDecay { decay_bars } => {
                    let factor = (1.0 - pos.bars_in_position as f64 / decay_bars.max(1) as f64).max(0.25);
                    (cfg.tp_net_bps_floor * factor, false)
                }
            };
            if net >= floor_bps / 10_000.0 * notional {
                if partial_close && !pos.scaled_out_once {
                    // close half the position (scale-out), keep the rest
                    let half_qty = pos.qty / 2.0;
                    let (settled, fee, slip) = partial_close_position(&mut pos, half_qty, price, cfg);
                    equity += settled;
                    pos.fees += fee;
                    pos.slippage += slip;
                    pos.scaled_out_once = true;
                    events.push(ev(ts, "gated_tp_scaleout1", price, settled));
                    // do NOT continue — keep evaluating; the remaining half uses 2x floor
                } else {
                    let (settled, fee, slip) = close_position(&mut pos, price, cfg);
                    equity += settled;
                    pos.fees += fee;
                    pos.slippage += slip;
                    tp_count += 1;
                    trade_count += 1;
                    events.push(ev(ts, "gated_tp", price, settled));
                    equity_curve.push(EquityPoint { timestamp_ms: ts, equity_quote: equity });
                    continue;
                }
            }
        }

        // ---- FO (no position) ----
        if pos.qty.abs() <= 1e-12 {
            let dir = if gate.long_fo && cfg.direction_bias >= 0 {
                1i8
            } else if gate.short_fo && cfg.direction_bias <= 0 {
                -1i8
            } else {
                0
            };
            if dir != 0 {
                // reserve check: must cover full ladder + close + maintenance
                let full_ladder_notional: f64 =
                    SOFT_LADDER.iter().map(|m| cfg.fo_quote * m).sum();
                let close_cost = full_ladder_notional * cfg.fee_bps / 10_000.0;
                let maint = cfg.filters.maintenance_margin(full_ladder_notional);
                let ok = next_so_close_maintenance_reserve_ok(
                    equity,
                    cfg.fo_quote,
                    cfg.fo_quote * SOFT_LADDER[1],
                    maint,
                    cfg.fee_bps / 10_000.0,
                    cfg.fo_quote,
                );
                if ok && close_cost + maint < equity {
                    if let Some(fill) = try_fill(cfg, dir as f64 * cfg.fo_quote, price) {
                        apply_fill(&mut pos, fill.qty, price, fill.fee, fill.slip, ts);
                        equity -= fill.fee + fill.slip;
                        fo_count += 1;
                        events.push(ev(ts, "gated_fo", price, -(fill.fee + fill.slip)));
                    }
                }
            }
        } else {
            // ---- SO: only if adverse from last fill AND signal allows AND reserve ok ----
            let adverse = if pos.qty > 0.0 {
                (pos.last_fill_price - price) / pos.last_fill_price
            } else {
                (price - pos.last_fill_price) / pos.last_fill_price
            };
            let next_layer = (pos.depth as usize).min(SOFT_LADDER.len() - 1) + 1;
            let layer_quote = cfg.fo_quote * SOFT_LADDER[next_layer.min(SOFT_LADDER.len() - 1)];
            // cap SO depth at max_legs (FO counts as leg 1)
            if pos.depth >= cfg.max_legs {
                // no more averaging; wait for TP/abort/liquidation
                let unreal = unrealized(&pos, price);
                let mtm = equity + unreal;
                equity_curve.push(EquityPoint { timestamp_ms: ts, equity_quote: mtm });
                if mtm <= 0.0 { breach = true; liquidation_count += 1; events.push(ev(ts, "gated_equity_nonpositive", price, mtm)); break; }
                continue;
            }
            let net = net_pnl_after_cost(&pos, price, cfg);
            let maint = cfg.filters.maintenance_margin(pos.qty.abs() * price);
            if gate.so_allowed
                && net < 0.0
                && adverse >= cfg.adverse_spacing_frac
                && next_so_close_maintenance_reserve_ok(
                    equity,
                    cfg.fo_quote,
                    layer_quote,
                    maint,
                    cfg.fee_bps / 10_000.0,
                    cfg.fo_quote,
                )
            {
                let dir_sign = pos.qty.signum();
                if let Some(fill) = try_fill(cfg, dir_sign * layer_quote, price) {
                    apply_fill(&mut pos, fill.qty, price, fill.fee, fill.slip, ts);
                    equity -= fill.fee + fill.slip;
                    so_count += 1;
                    events.push(ev(ts, "gated_so", price, -(fill.fee + fill.slip)));
                }
            }
        }

        // mark-to-market equity
        let unreal = if pos.qty.abs() > 1e-12 { unrealized(&pos, price) } else { 0.0 };
        let mtm = equity + unreal;
        equity_curve.push(EquityPoint { timestamp_ms: ts, equity_quote: mtm });
        if mtm <= 0.0 {
            breach = true;
            liquidation_count += 1;
            events.push(ev(ts, "gated_equity_nonpositive", price, mtm));
            break;
        }
    }

    // force-close any open position at the last bar
    if pos.qty.abs() > 1e-12 {
        if let Some(last) = bars.last() {
            let (settled, fee, slip) = close_position(&mut pos, last.close, cfg);
            equity += settled;
            pos.fees += fee;
            pos.slippage += slip;
            events.push(ev(last.open_time_ms, "gated_force_close_end", last.close, settled));
        }
    }

    let ending_equity = equity_curve.last().map(|e| e.equity_quote).unwrap_or(initial_equity);
    let peak = equity_curve
        .iter()
        .fold(f64::NEG_INFINITY, |p, e| p.max(e.equity_quote));
    let max_dd = equity_curve
        .iter()
        .map(|e| if peak > 0.0 { (peak - e.equity_quote) / peak * 100.0 } else { 0.0 })
        .fold(0.0f64, f64::max);
    let drawdown_curve: Vec<DrawdownPoint> = equity_curve
        .iter()
        .scan(f64::NEG_INFINITY, |peak, e| {
            *peak = peak.max(e.equity_quote);
            let dd = if *peak > 0.0 { (*peak - e.equity_quote) / *peak * 100.0 } else { 0.0 };
            Some(DrawdownPoint { timestamp_ms: e.timestamp_ms, drawdown_pct: dd })
        })
        .collect();

    let days = if bars.len() >= 2 {
        ((bars.last().unwrap().open_time_ms - bars.first().unwrap().open_time_ms) as f64)
            / 86_400_000.0
    } else {
        0.0
    };
    let ann = calculate_annualized_return_pct(initial_equity, ending_equity, days);

    let mut rejection_reasons = Vec::new();
    let summary = serde_json::json!({
        "family": "M1_gated_martin",
        "symbol": cfg.symbol,
        "fo": fo_count,
        "so": so_count,
        "tp": tp_count,
        "abort": abort_count,
        "trade_count": trade_count,
        "liquidation_count": liquidation_count,
        "min_equity_quote": equity_curve.iter().map(|e| e.equity_quote).fold(f64::INFINITY, f64::min),
        "max_equity_dd_pct": max_dd,
        "breach": breach,
        "initial_equity": initial_equity,
        "ending_equity": ending_equity,
        "total_return_pct": if initial_equity > 0.0 { (ending_equity/initial_equity - 1.0)*100.0 } else { 0.0 },
        "annualized_return_pct": ann,
        "stitched_days": days,
    });
    rejection_reasons.push(format!("GATED_SUMMARY:{}", summary));

    Ok(MartingaleBacktestResult {
        metrics: backtest_engine::martingale::metrics::MartingaleMetrics {
            total_return_pct: if initial_equity > 0.0 { (ending_equity / initial_equity - 1.0) * 100.0 } else { 0.0 },
            annualized_return_pct: ann,
            max_drawdown_pct: max_dd,
            global_drawdown_pct: Some(max_dd),
            max_strategy_drawdown_pct: Some(max_dd),
            monthly_win_rate_pct: None,
            max_leverage_used: Some(cfg.leverage as f64),
            min_liquidation_buffer_pct: Some(0.0),
            total_fee_quote: Some(pos.fees),
            total_slippage_quote: Some(pos.slippage),
            total_funding_quote: Some(0.0),
            planned_margin_quote: Some(cfg.fo_quote / cfg.leverage as f64),
            planned_notional_quote: Some(cfg.fo_quote),
            return_drawdown_ratio: None,
            data_quality_score: None,
            trade_count,
            stop_count: abort_count,
            max_capital_used_quote: cfg.budget_quote,
            survival_passed: !breach,
        },
        events,
        equity_curve,
        drawdown_curve,
        trades: Vec::new(),
        rejection_reasons,
    })
}

struct FillOut {
    qty: f64,
    fee: f64,
    slip: f64,
}

fn try_fill(cfg: &GatedMartinConfig, signed_notional: f64, price: f64) -> Option<FillOut> {
    let abs_notional = signed_notional.abs();
    let dec = filter_order_conservative(&cfg.filters, price, abs_notional);
    if dec.reject.is_some() || dec.rounded_notional < 5.0 {
        return None;
    }
    let qty = signed_notional.signum() * (dec.rounded_notional / price);
    let fee = dec.rounded_notional * cfg.fee_bps / 10_000.0;
    let slip = dec.rounded_notional * cfg.slippage_bps / 10_000.0;
    Some(FillOut { qty, fee, slip })
}

fn apply_fill(pos: &mut Position, qty: f64, price: f64, fee: f64, slip: f64, ts: i64) {
    let was_flat = pos.qty.abs() < 1e-12;
    let new_qty = pos.qty + qty;
    if new_qty.abs() < 1e-12 {
        pos.avg_entry = 0.0;
    } else if was_flat {
        pos.avg_entry = price;
    } else if qty.signum() == pos.qty.signum() {
        // adding same direction -> weighted average
        let total_cost = pos.avg_entry * pos.qty.abs() + price * qty.abs();
        pos.avg_entry = total_cost / new_qty.abs();
    }
    pos.qty = new_qty;
    pos.fees += fee;
    pos.slippage += slip;
    pos.depth += 1;
    pos.last_fill_price = price;
    pos.last_fill_ts = ts;
    if was_flat {
        pos.bars_in_position = 0;
        pos.scaled_out_once = false;
    }
}

fn close_position(pos: &mut Position, price: f64, cfg: &GatedMartinConfig) -> (f64, f64, f64) {
    if pos.qty.abs() < 1e-12 {
        return (0.0, 0.0, 0.0);
    }
    let realized = (price - pos.avg_entry) * pos.qty; // long qty>0 profits on rise; short qty<0 profits on fall
    let notional = pos.qty.abs() * price;
    let fee = notional * cfg.fee_bps / 10_000.0;
    let slip = notional * cfg.slippage_bps / 10_000.0;
    pos.realized += realized - fee - slip;
    let r = realized - fee - slip;
    pos.qty = 0.0;
    pos.avg_entry = 0.0;
    pos.depth = 0;
    pos.last_fill_price = 0.0;
    pos.bars_in_position = 0;
    pos.scaled_out_once = false;
    (r, fee, slip)
}

/// Close a partial quantity (for ScaleOut TP). Does NOT reset depth-tracking
/// fields; the remaining position keeps its avg_entry.
fn partial_close_position(pos: &mut Position, qty_to_close: f64, price: f64, cfg: &GatedMartinConfig) -> (f64, f64, f64) {
    if qty_to_close.abs() < 1e-12 || pos.qty.abs() < 1e-12 {
        return (0.0, 0.0, 0.0);
    }
    let close_qty = if qty_to_close.abs() > pos.qty.abs() { pos.qty } else { qty_to_close };
    let realized = (price - pos.avg_entry) * close_qty;
    let notional = close_qty.abs() * price;
    let fee = notional * cfg.fee_bps / 10_000.0;
    let slip = notional * cfg.slippage_bps / 10_000.0;
    pos.realized += realized - fee - slip;
    pos.qty -= close_qty;
    (realized - fee - slip, fee, slip)
}

fn unrealized(pos: &Position, price: f64) -> f64 {
    (price - pos.avg_entry) * pos.qty
}

fn net_pnl_after_cost(pos: &Position, price: f64, cfg: &GatedMartinConfig) -> f64 {
    let gross = (price - pos.avg_entry) * pos.qty;
    let close_fee = pos.qty.abs() * price * cfg.fee_bps / 10_000.0;
    gross - close_fee
}

fn ev(ts: i64, etype: &str, price: f64, pnl: f64) -> MartingaleBacktestEvent {
    MartingaleBacktestEvent {
        timestamp_ms: ts,
        event_type: etype.to_string(),
        symbol: "M1".to_string(),
        strategy_instance_id: "M1".to_string(),
        cycle_id: None,
        detail: format!("price={:.4} settled_pnl={:.4}", price, pnl),
    }
}

/// A signal source backed by precomputed M1 activations aligned to bar timestamps.
pub struct M1SignalSource<'a> {
    pub activations: std::collections::BTreeMap<i64, &'a crate::m1_signal::M1Activation>,
}

impl<'a> SignalSource for M1SignalSource<'a> {
    fn gate(&self, _bar_idx: usize, ts_ms: i64, _price: f64) -> SignalGate {
        match self.activations.get(&ts_ms) {
            Some(a) => SignalGate {
                long_fo: a.long_fo,
                short_fo: a.short_fo,
                so_allowed: a.long_so_exhaustion || a.short_so_exhaustion,
                force_abort: false,
            },
            None => SignalGate::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysOn;
    impl SignalSource for AlwaysOn {
        fn gate(&self, _: usize, _: i64, _: f64) -> SignalGate {
            SignalGate { long_fo: true, short_fo: false, so_allowed: true, force_abort: false }
        }
    }

    fn bars_mean_revert() -> Vec<KlineBar> {
        // price dips then reverts -> long FO should profit
        let mut v = Vec::new();
        let prices: Vec<f64> = (0..200).map(|t| 100.0 - 10.0 * (((t as f64) * 0.1).sin()) ).collect();
        for (t, p) in prices.iter().enumerate() {
            v.push(KlineBar { symbol: "X".into(), open_time_ms: (t as i64) * 60_000, open: *p, high: *p, low: *p, close: *p, volume: 1.0 });
        }
        v
    }

    #[test]
    fn gated_martin_runs_and_produces_summary() {
        let cfg = GatedMartinConfig {
            symbol: "X".into(),
            filters: ExchangeFilters::default_for("X"),
            budget_quote: 1000.0,
            fo_quote: 100.0,
            adverse_spacing_frac: 0.02,
            tp_net_bps_floor: 10.0,
            fee_bps: 2.0,
            slippage_bps: 1.0,
            leverage: 3,
            direction_bias: 1,
            tp_mode: TpMode::Fixed,
            max_legs: 4,
        };
        let res = run_gated_martin(&cfg, &bars_mean_revert(), &AlwaysOn).unwrap();
        let s = res.rejection_reasons.iter().find(|s| s.starts_with("GATED_SUMMARY:")).unwrap();
        assert!(s.contains("M1_gated_martin"));
        // should have opened at least one FO
        let v: serde_json::Value = serde_json::from_str(&s["GATED_SUMMARY:".len()..]).unwrap();
        assert!(v["fo"].as_u64().unwrap() >= 1, "fo={}", v["fo"]);
    }
}
