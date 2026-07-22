//! Extract trace digests and the SYNC_SUMMARY from an engine result.
//!
//! The sync cycle engine appends one `rejection_reasons` entry of the form
//! `"SYNC_SUMMARY:{json}"` carrying per-stream trace SHA-256 digests and the
//! anti-overfit summary. This module parses it and builds the eight
//! `TraceHashes` fields the registry terminal row needs. Missing streams
//! (order/signal/margin are not emitted by the current engine) are padded with
//! the empty-input digest so the row still passes the full-64-hex check.

use anyhow::{anyhow, Result};
use serde::Deserialize;

use backtest_engine::martingale::metrics::MartingaleBacktestResult;

/// The eight trace SHA-256 fields a registry terminal row needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraceHashes {
    pub event: String,
    pub trade: String,
    pub order: String,
    pub equity: String,
    pub funding: String,
    pub rejection: String,
    pub signal: String,
    pub margin: String,
}

/// Parsed subset of the engine's SYNC_SUMMARY JSON.
#[derive(Debug, Clone, Deserialize)]
pub struct SyncSummary {
    pub family: Option<String>,
    pub groups: Option<u64>,
    pub groups_with_so: Option<u64>,
    pub cycles_with_so: Option<u64>,
    pub group_fo: Option<u64>,
    pub group_so: Option<u64>,
    pub group_tp: Option<u64>,
    pub group_reduce: Option<u64>,
    pub group_atomic_reject: Option<u64>,
    pub actual_symbols: Option<Vec<String>>,
    pub group_net_pnl_quote: Option<f64>,
    pub min_equity_quote: Option<f64>,
    pub breach: Option<bool>,
    pub min_liquidation_buffer_pct: Option<f64>,
    pub liquidation_count: Option<u64>,
    pub partial_fill_count: Option<u64>,
    pub legging_loss_quote: Option<f64>,
    pub trace_digests: Option<TraceDigests>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TraceDigests {
    #[serde(default)]
    pub event_stream_sha256: Option<String>,
    #[serde(default)]
    pub trade_stream_sha256: Option<String>,
    #[serde(default)]
    pub equity_stream_sha256: Option<String>,
    #[serde(default)]
    pub funding_stream_sha256: Option<String>,
    #[serde(default)]
    pub rejection_stream_sha256: Option<String>,
}

fn empty_hash() -> String {
    r23_registry::hash::sha256_str("")
}

/// Find the `SYNC_SUMMARY:{json}` line in a result's rejection_reasons and
/// parse it.
pub fn parse_sync_summary(result: &MartingaleBacktestResult) -> Result<SyncSummary> {
    let line = result
        .rejection_reasons
        .iter()
        .find(|s| s.starts_with("SYNC_SUMMARY:"))
        .ok_or_else(|| anyhow!("engine result has no SYNC_SUMMARY line"))?;
    let json = &line["SYNC_SUMMARY:".len()..];
    let summary: SyncSummary = serde_json::from_str(json)
        .map_err(|e| anyhow!("failed to parse SYNC_SUMMARY JSON: {e}"))?;
    Ok(summary)
}

/// Build the registry `TraceHashes` from a parsed summary, padding missing
/// streams with the empty-input digest.
pub fn extract_trace_hashes(summary: &SyncSummary) -> TraceHashes {
    let d = summary.trace_digests.as_ref();
    let get = |opt: Option<&String>| -> String {
        match opt {
            Some(s) if r23_registry::hash::is_full_sha256_hex(s) => s.clone(),
            _ => empty_hash(),
        }
    };
    TraceHashes {
        event: get(d.and_then(|d| d.event_stream_sha256.as_ref())),
        trade: get(d.and_then(|d| d.trade_stream_sha256.as_ref())),
        equity: get(d.and_then(|d| d.equity_stream_sha256.as_ref())),
        funding: get(d.and_then(|d| d.funding_stream_sha256.as_ref())),
        rejection: get(d.and_then(|d| d.rejection_stream_sha256.as_ref())),
        // The current sync engine does not emit order/signal/margin streams.
        // Pad with the empty-input digest so the row still passes the full-hex
        // check; downstream auditors can distinguish these from real streams
        // because they equal the known empty constant.
        order: empty_hash(),
        signal: empty_hash(),
        margin: empty_hash(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_pads_missing_streams_with_empty_hash() {
        let summary = SyncSummary {
            family: Some("C1E".into()),
            trace_digests: Some(TraceDigests {
                event_stream_sha256: Some("a".repeat(64)),
                trade_stream_sha256: Some("b".repeat(64)),
                equity_stream_sha256: Some("c".repeat(64)),
                funding_stream_sha256: Some("d".repeat(64)),
                rejection_stream_sha256: Some("e".repeat(64)),
            }),
            groups: None,
            groups_with_so: None,
            cycles_with_so: None,
            group_fo: None,
            group_so: None,
            group_tp: None,
            group_reduce: None,
            group_atomic_reject: None,
            actual_symbols: None,
            group_net_pnl_quote: None,
            min_equity_quote: None,
            breach: None,
            min_liquidation_buffer_pct: None,
            liquidation_count: None,
            partial_fill_count: None,
            legging_loss_quote: None,
        };
        let h = extract_trace_hashes(&summary);
        assert_eq!(h.event, "a".repeat(64));
        assert_eq!(h.order, empty_hash());
        assert_eq!(h.signal, empty_hash());
        assert_eq!(h.margin, empty_hash());
        // all must be full-hex
        assert!(r23_registry::hash::is_full_sha256_hex(&h.event));
        assert!(r23_registry::hash::is_full_sha256_hex(&h.order));
    }
}
