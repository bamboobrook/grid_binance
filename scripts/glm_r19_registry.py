#!/usr/bin/env python3
"""Round 19 R0: append-only registry + failure ledger helpers.

Plan §3.1: each experiment has >=1 running row and exactly one terminal row.
Terminal rows must include raw_command, exit_code, all hashes, fingerprint.
Forbid overwrite/truncate. Process crashes => validator marks orphan running as
`interrupted`.
"""
import hashlib
import json
import os
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round19"
REGISTRY = os.path.join(ART, "exploration-registry.jsonl")
FAILURE_LEDGER = os.path.join(ART, "failure-ledger.jsonl")

ALLOWED_TERMINAL = {
    "complete", "rejected_gate", "invalid_mechanism", "invalid_data",
    "invalid_engine_bug", "invalid_results", "timeout", "skipped_duplicate",
    "blocked_predecessor", "blocked_parent_gate", "blocked_no_stable_groups",
    "complete_zero_survivors", "not_applicable_zero_survivors",
    "not_martingale_no_so", "interrupted",
}


def sha256_str(s):
    return hashlib.sha256(s.encode()).hexdigest()


def sha256_json(obj):
    return sha256_str(json.dumps(obj, sort_keys=True, separators=(",", ":")))


def canonical_fingerprint(*, engine_sha, market_data_sha, funding_data_sha, family,
                          cycle_topology, trigger_contract_sha, fit_contract_sha,
                          universe_group_weights_sha, resolved_config_sha,
                          effective_config_sha, cost_model_sha, window, budget,
                          fold, seed):
    """Plan §3 canonical fingerprint = sha256(engine+data+funding+topology+
    trigger+fit+universe/weights+resolved+effective+cost+window+budget+fold+seed)."""
    fp = {
        "engine_sha256": engine_sha,
        "market_data_sha256": market_data_sha,
        "funding_data_sha256": funding_data_sha,
        "family": family,
        "cycle_topology": cycle_topology,
        "trigger_contract_sha256": trigger_contract_sha,
        "fit_contract_sha256": fit_contract_sha,
        "universe_group_weights_sha256": universe_group_weights_sha,
        "resolved_config_sha256": resolved_config_sha,
        "effective_config_sha256": effective_config_sha,
        "cost_model_sha256": cost_model_sha,
        "window": window,
        "budget": budget,
        "fold": fold,
        "seed": seed,
    }
    fp["fingerprint_sha256"] = sha256_json(fp)
    return fp


def append_running(fingerprint, *, experiment_id, hypothesis_id, family, novelty_proof,
                   exact_difference_from_r1_r18, raw_command, pid=None):
    row = {
        **fingerprint,
        "experiment_id": experiment_id,
        "hypothesis_id": hypothesis_id,
        "family": family,
        "novelty_proof": novelty_proof,
        "exact_difference_from_r1_r18": exact_difference_from_r1_r18,
        "status": "running",
        "started_at": time.time(),
        "raw_command": raw_command,
        "pid": pid,
    }
    _append(row)
    return row


def append_terminal(fingerprint, *, experiment_id, family, status, raw_command,
                    exit_code, wall_s=None, rss_mb=None, fo_count=None, so_count=None,
                    tp_count=None, reduce_count=None, rejection_count=None,
                    actual_symbols=None, groups=None, max_symbol_concentration_pct=None,
                    max_group_concentration_pct=None, max_symbol_abs_net_pnl_share_pct=None,
                    max_group_abs_net_pnl_share_pct=None, fee_quote=None, slip_quote=None,
                    funding_quote=None, ann=None, dd=None, min_equity=None, breach=None,
                    cold_starts_pos=None, cold_starts_total=None, groups_with_so=None,
                    trace_event_sha=None, trace_trade_sha=None, trace_equity_sha=None,
                    trace_funding_sha=None, trace_rejection_sha=None,
                    reason=None, terminal_note=None, liquidation_count=None):
    if status not in ALLOWED_TERMINAL:
        raise ValueError(f"illegal terminal status {status!r}; allowed: {sorted(ALLOWED_TERMINAL)}")
    row = {**fingerprint,
           "experiment_id": experiment_id, "family": family, "status": status,
           "finished_at": time.time(), "raw_command": raw_command, "exit_code": exit_code,
           "wall_s": wall_s, "rss_mb": rss_mb,
           "fo_count": fo_count, "so_count": so_count, "tp_count": tp_count,
           "reduce_count": reduce_count, "rejection_count": rejection_count,
           "actual_symbols": actual_symbols, "groups": groups,
           "max_symbol_concentration_pct": max_symbol_concentration_pct,
           "max_group_concentration_pct": max_group_concentration_pct,
           "max_symbol_abs_net_pnl_share_pct": max_symbol_abs_net_pnl_share_pct,
           "max_group_abs_net_pnl_share_pct": max_group_abs_net_pnl_share_pct,
           "fee_quote": fee_quote, "slippage_quote": slip_quote, "funding_quote": funding_quote,
           "ann": ann, "dd": dd, "min_equity": min_equity, "breach": breach,
           "cold_starts_pos": cold_starts_pos, "cold_starts_total": cold_starts_total,
           "groups_with_so": groups_with_so,
           "trace_event_sha256": trace_event_sha, "trace_trade_sha256": trace_trade_sha,
           "trace_equity_sha256": trace_equity_sha, "trace_funding_sha256": trace_funding_sha,
           "trace_rejection_sha256": trace_rejection_sha,
           "liquidation_count": liquidation_count,
           "reason": reason, "terminal_note": terminal_note,
           "actual_binary_replays": 1}
    _append(row)
    return row


def append_skipped_duplicate(fingerprint, *, experiment_id, family, original_experiment_id=None):
    row = {**fingerprint, "experiment_id": experiment_id, "family": family,
           "status": "skipped_duplicate", "finished_at": time.time(),
           "raw_command": "cache:skipped_duplicate", "exit_code": 0,
           "terminal_note": f"fingerprint matches prior terminal; original={original_experiment_id}",
           "actual_binary_replays": 0}
    _append(row)
    return row


def append_failure(*, experiment_id, family, fingerprint_sha256, failure_type, reason,
                   exact_config=None, trace_hashes=None, never_repeat_rule=None):
    """Plan §4.2 / §17: every failure must be logged with exact fingerprint and
    a never-repeat reason."""
    row = {
        "experiment_id": experiment_id,
        "family": family,
        "fingerprint_sha256": fingerprint_sha256,
        "failure_type": failure_type,
        "reason": reason,
        "exact_config": exact_config,
        "trace_hashes": trace_hashes,
        "never_repeat_rule": never_repeat_rule,
        "recorded_at": time.time(),
    }
    os.makedirs(os.path.dirname(FAILURE_LEDGER), exist_ok=True)
    with open(FAILURE_LEDGER, "a") as fh:
        fh.write(json.dumps(row, sort_keys=True) + "\n")
    return row


def _append(row):
    os.makedirs(os.path.dirname(REGISTRY), exist_ok=True)
    with open(REGISTRY, "a") as fh:
        fh.write(json.dumps(row, sort_keys=True) + "\n")


def load_seen_fingerprints():
    """Return set of fingerprint_sha256 that have a non-rejected terminal row
    (so duplicates can be skipped)."""
    seen = set()
    if not os.path.exists(REGISTRY):
        return seen
    with open(REGISTRY) as fh:
        for ln in fh:
            ln = ln.strip()
            if not ln:
                continue
            try:
                r = json.loads(ln)
            except json.JSONDecodeError:
                continue
            st = r.get("status")
            fp = r.get("fingerprint_sha256")
            if not fp:
                continue
            if st in ALLOWED_TERMINAL and st not in ("rejected_gate", "interrupted", "invalid_engine_bug"):
                seen.add(fp)
    return seen
