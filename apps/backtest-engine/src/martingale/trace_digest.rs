//! Canonical trace digests for Batch/CLI parity (Round 16 R0.1).
//!
//! Produces stable SHA256 of the event/trade/equity/funding/rejection streams
//! plus the resolved config. Field order, float formatting and timestamp order
//! are fixed so the release CLI and BatchReplay produce identical hashes when
//! they share data + config + window + budget.

use crate::martingale::kline_engine::FundingRatePoint;
use crate::martingale::metrics::MartingaleBacktestResult;
use sha2::{Digest, Sha256};

/// Round an f64 to a fixed number of decimal digits for stable hash output.
pub fn round_digits(v: f64, digits: i32) -> f64 {
    let factor = 10f64.powi(digits);
    (v * factor).round() / factor
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceDigests {
    pub event_stream_sha256: String,
    pub trade_stream_sha256: String,
    pub equity_stream_sha256: String,
    pub funding_stream_sha256: String,
    pub rejection_stream_sha256: String,
}

/// Compute the four stream digests from a backtest result + the funding input.
pub fn compute_trace_digests(
    result: &MartingaleBacktestResult,
    funding: &[FundingRatePoint],
) -> TraceDigests {
    let mut h = Sha256::new();
    for ev in &result.events {
        h.update(ev.timestamp_ms.to_string().as_bytes());
        h.update(b"|");
        h.update(ev.event_type.as_bytes());
        h.update(b"|");
        h.update(ev.symbol.as_bytes());
        h.update(b"|");
        h.update(ev.strategy_instance_id.as_bytes());
        h.update(b"|");
        h.update(ev.cycle_id.as_deref().unwrap_or("").as_bytes());
        h.update(b"|");
        h.update(ev.detail.as_bytes());
        h.update(b"\n");
    }
    let event_stream_sha256 = format!("{:x}", h.finalize());

    let mut h = Sha256::new();
    for t in &result.trades {
        h.update(
            format!(
                "{}|{}|{}|{:?}|{:?}|{}|{}|{}|{}|{}|{}|{}\n",
                t.timestamp_ms,
                t.symbol,
                t.direction,
                t.event_type,
                t.leg_index,
                round_digits(t.price, 12),
                round_digits(t.notional_quote, 12),
                round_digits(t.margin_quote, 12),
                t.leverage,
                round_digits(t.fee_quote, 12),
                round_digits(t.slippage_quote, 12),
                round_digits(t.realized_pnl_quote, 12),
            )
            .as_bytes(),
        );
    }
    let trade_stream_sha256 = format!("{:x}", h.finalize());

    let mut h = Sha256::new();
    for p in &result.equity_curve {
        h.update(format!("{}|{}\n", p.timestamp_ms, round_digits(p.equity_quote, 12)).as_bytes());
    }
    let equity_stream_sha256 = format!("{:x}", h.finalize());

    let mut keyed: Vec<(i64, &str, &FundingRatePoint)> = funding
        .iter()
        .map(|f| (f.funding_time_ms, f.symbol.as_str(), f))
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    let mut h = Sha256::new();
    for (t, sym, f) in keyed {
        let mp = f.mark_price.map(|v| round_digits(v, 12)).unwrap_or(-1.0);
        h.update(
            format!(
                "{}|{}|{}|{}\n",
                t,
                sym,
                round_digits(f.funding_rate, 12),
                mp
            )
            .as_bytes(),
        );
    }
    let funding_stream_sha256 = format!("{:x}", h.finalize());

    let mut h = Sha256::new();
    for r in &result.rejection_reasons {
        h.update(r.as_bytes());
        h.update(b"\n");
    }
    let rejection_stream_sha256 = format!("{:x}", h.finalize());

    TraceDigests {
        event_stream_sha256,
        trade_stream_sha256,
        equity_stream_sha256,
        funding_stream_sha256,
        rejection_stream_sha256,
    }
}
