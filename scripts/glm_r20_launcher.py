#!/usr/bin/env python3
"""Round 20 P0: the SINGLE central launcher + append-only registry.

Plan §2.2: all runners must call this one launcher. No runner may build its own
simplified registry. Each experiment has >=1 running row + 1 terminal row with
the full hash contract.

Usage from a runner:
  from glm_r20_launcher import Launcher
  ln = Launcher()
  fp = ln.fingerprint(family="C1", cycle_topology="synchronized_pair", ...)
  ln.start(fp, experiment_id="...", raw_command="...", git_commit="...")
  result = ln.run_synchronized_cycle(...)  # executes + appends terminal
"""
import hashlib
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round20"
REGISTRY = os.path.join(ART, "exploration-registry.jsonl")
FAILURE_LEDGER = os.path.join(ART, "failure-ledger.jsonl")

ALLOWED_TERMINAL = {
    "complete", "rejected_gate", "invalid_mechanism", "invalid_data",
    "invalid_engine_bug", "invalid_results", "invalid_mechanism_or_overfit",
    "invalid_data_leakage", "timeout", "skipped_duplicate",
    "blocked_predecessor", "blocked_parent_gate", "blocked_no_stable_groups",
    "blocked_missing_borrow_data", "blocked_implementation_scope",
    "complete_zero_survivors", "not_applicable_zero_survivors",
    "not_martingale_no_so", "interrupted", "control_not_new_search",
}


def sha256_str(s):
    return hashlib.sha256(s.encode()).hexdigest()


def sha256_json(obj):
    return sha256_str(json.dumps(obj, sort_keys=True, separators=(",", ":")))


def sha256_file(path):
    if not os.path.exists(path):
        return None
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        while chunk := fh.read(8 << 20):
            h.update(chunk)
    return h.hexdigest()


def git_commit_sha():
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], stderr=subprocess.DEVNULL).decode().strip()
    except Exception:
        return None


class Launcher:
    """The single entry point for running experiments. Every runner must use
    this; no bespoke JSONL appends."""

    def __init__(self, registry_path=REGISTRY, failure_ledger=FAILURE_LEDGER):
        self.registry = registry_path
        self.failure_ledger = failure_ledger
        os.makedirs(os.path.dirname(self.registry), exist_ok=True)
        # touch empty registry if missing
        if not os.path.exists(self.registry):
            open(self.registry, "a").close()
        if not os.path.exists(self.failure_ledger):
            open(self.failure_ledger, "a").close()
        self._seen = self._load_seen()

    def _load_seen(self):
        seen = set()
        with open(self.registry) as fh:
            for ln in fh:
                ln = ln.strip()
                if not ln:
                    continue
                try:
                    r = json.loads(ln)
                except json.JSONDecodeError:
                    continue
                fp = r.get("fingerprint_sha256")
                st = r.get("status")
                if fp and st in ALLOWED_TERMINAL and st not in (
                        "rejected_gate", "interrupted", "invalid_engine_bug",
                        "invalid_data_leakage", "control_not_new_search"):
                    seen.add(fp)
        return seen

    def fingerprint(self, *, family, cycle_topology, trigger_contract_sha,
                    fit_contract_sha, universe_group_weights_sha, scheduler_sha,
                    resolved_config_sha, effective_config_sha, cost_model_sha,
                    engine_sha, market_data_sha, funding_data_sha,
                    exchange_filter_snapshot_sha, maintenance_tiers_sha,
                    window, budget, fold, seed):
        """Canonical fingerprint per plan §2.3."""
        fp = {
            "engine_sha256": engine_sha,
            "market_data_sha256": market_data_sha,
            "funding_data_sha256": funding_data_sha,
            "exchange_filter_snapshot_sha256": exchange_filter_snapshot_sha,
            "maintenance_tiers_sha256": maintenance_tiers_sha,
            "family": family,
            "cycle_topology": cycle_topology,
            "trigger_contract_sha256": trigger_contract_sha,
            "fit_contract_sha256": fit_contract_sha,
            "universe_group_weights_sha256": universe_group_weights_sha,
            "scheduler_sha256": scheduler_sha,
            "resolved_config_sha256": resolved_config_sha,
            "effective_config_sha256": effective_config_sha,
            "cost_model_sha256": cost_model_sha,
            "window": window, "budget": budget, "fold": fold, "seed": seed,
        }
        fp["fingerprint_sha256"] = sha256_json(fp)
        return fp

    def is_duplicate(self, fingerprint):
        return fingerprint["fingerprint_sha256"] in self._seen

    def start(self, fingerprint, *, experiment_id, parent_id=None, raw_command, pid=None):
        """Append a `running` row. Must be called BEFORE the binary runs."""
        row = {
            **fingerprint,
            "experiment_id": experiment_id,
            "parent_id": parent_id,
            "status": "running",
            "started_at": time.time(),
            "raw_command": raw_command,
            "pid": pid,
            "git_commit": git_commit_sha(),
        }
        self._append(row)
        return row

    def terminal(self, fingerprint, *, experiment_id, status, raw_command, exit_code,
                 wall_s=None, rss_mb=None, metrics=None, segment_metrics=None,
                 rejection_reason=None, trace_hashes=None, liquidation_count=None,
                 partial_fill_count=None, legging_loss_quote=None, reason=None,
                 terminal_note=None):
        if status not in ALLOWED_TERMINAL:
            raise ValueError(f"illegal terminal status {status!r}; allowed: {sorted(ALLOWED_TERMINAL)}")
        m = metrics or {}
        row = {
            **fingerprint,
            "experiment_id": experiment_id, "status": status,
            "finished_at": time.time(), "raw_command": raw_command, "exit_code": exit_code,
            "wall_s": wall_s, "rss_mb": rss_mb,
            "metrics": m, "segment_metrics": segment_metrics,
            "rejection_reason": rejection_reason,
            "trace_event_sha256": (trace_hashes or {}).get("event_stream_sha256"),
            "trace_trade_sha256": (trace_hashes or {}).get("trade_stream_sha256"),
            "trace_equity_sha256": (trace_hashes or {}).get("equity_stream_sha256"),
            "trace_funding_sha256": (trace_hashes or {}).get("funding_stream_sha256"),
            "trace_rejection_sha256": (trace_hashes or {}).get("rejection_stream_sha256"),
            "liquidation_count": liquidation_count,
            "partial_fill_count": partial_fill_count,
            "legging_loss_quote": legging_loss_quote,
            "actual_binary_replays": 1 if status == "complete" else 0,
            "reason": reason, "terminal_note": terminal_note,
        }
        self._append(row)
        self._seen.add(fingerprint["fingerprint_sha256"])
        return row

    def skip_duplicate(self, fingerprint, *, experiment_id, original_experiment_id=None):
        row = {
            **fingerprint, "experiment_id": experiment_id, "status": "skipped_duplicate",
            "finished_at": time.time(), "raw_command": "cache:skipped_duplicate",
            "exit_code": 0, "actual_binary_replays": 0,
            "terminal_note": f"fingerprint matches prior terminal; original={original_experiment_id}",
        }
        self._append(row)
        return row

    def failure(self, *, experiment_id, family, fingerprint_sha256, failure_type, reason,
                first_failed_gate=None, exact_config=None, metrics=None,
                trace_hashes=None, never_repeat_rule=None):
        row = {
            "experiment_id": experiment_id, "family": family,
            "fingerprint_sha256": fingerprint_sha256, "failure_type": failure_type,
            "first_failed_gate": first_failed_gate, "reason": reason,
            "exact_config": exact_config, "metrics": metrics, "trace_hashes": trace_hashes,
            "never_repeat_rule": never_repeat_rule, "recorded_at": time.time(),
        }
        with open(self.failure_ledger, "a") as fh:
            fh.write(json.dumps(row, sort_keys=True) + "\n")
        return row

    def _append(self, row):
        with open(self.registry, "a") as fh:
            fh.write(json.dumps(row, sort_keys=True) + "\n")
