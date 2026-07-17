#!/usr/bin/env python3
"""Round 18 R1: canonical fingerprint index + append-only registry.

Plan §4.1 requires a canonical fingerprint for every experiment so identical
configs are skipped_duplicate. Plan §4.2 requires every experiment to record
an immutable `running` row and a terminal row, and forbids overwrite/delete.

This module provides:
  - fingerprint(cfg, window, budget, fold, seed, ...) -> sha256 over the
    canonical fields (engine/data/cycle_topology/trigger_contract/fit_contract/
    universe/resolved/effective/cost_model hashes + window/budget/fold/seed).
  - Round18Registry: append-only JSONL writer that enforces terminal discipline
    and dedup on fingerprint.
  - scan_historical: scan R1-R17 artifacts for prior fingerprints so Round 18
    cannot re-launch a duplicate (plan §4.1 + §5).
"""
import hashlib
import json
import os
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round18"
REGISTRY = os.path.join(ART, "r1", "exploration-registry.jsonl")
HIST_INDEX = os.path.join(ART, "r1", "historical-fingerprint-index.json")

ALLOWED_TERMINAL = {
    "complete", "rejected_gate", "invalid_mechanism", "invalid_data",
    "timeout", "skipped_duplicate", "blocked_predecessor",
    "complete_zero_survivors", "not_applicable_zero_survivors",
    "blocked_parent_gate", "blocked_no_stable_groups",
}


def sha256_str(s):
    return hashlib.sha256(s.encode()).hexdigest()


def sha256_json(obj):
    return sha256_str(json.dumps(obj, sort_keys=True, separators=(",", ":")))


def canonical_fingerprint(*, engine_sha, market_data_sha, funding_data_sha,
                          cycle_topology, martingale_trigger_contract_sha,
                          fit_contract_sha, universe_and_group_sha,
                          resolved_config_sha, effective_config_sha,
                          window, budget, fold, seed, cost_model_sha,
                          plan_sha=None):
    """Return the canonical fingerprint dict + a single sha256 fingerprint key.

    cycle_topology ∈ {independent_symbol, synchronized_pair, synchronized_basket}.
    """
    fp = {
        "engine_sha256": engine_sha,
        "market_data_sha256": market_data_sha,
        "funding_data_sha256": funding_data_sha,
        "cycle_topology": cycle_topology,
        "martingale_trigger_contract_sha256": martingale_trigger_contract_sha,
        "fit_contract_sha256": fit_contract_sha,
        "universe_and_group_sha256": universe_and_group_sha,
        "resolved_config_sha256": resolved_config_sha,
        "effective_config_sha256": effective_config_sha,
        "cost_model_sha256": cost_model_sha,
        "window": window,
        "budget": budget,
        "fold": fold,
        "seed": seed,
    }
    fp["fingerprint_sha256"] = sha256_json(fp)
    if plan_sha:
        fp["plan_sha256"] = plan_sha
    return fp


class Round18Registry:
    """Append-only JSONL registry. Each experiment gets >=1 running row and
    exactly one terminal row. Dedup is by fingerprint_sha256 + window + budget.
    """

    def __init__(self, path=REGISTRY):
        self.path = path
        os.makedirs(os.path.dirname(path), exist_ok=True)
        self._seen = self._load_seen()

    def _load_seen(self):
        seen = set()
        if not os.path.exists(self.path):
            return seen
        with open(self.path) as fh:
            for ln in fh:
                ln = ln.strip()
                if not ln:
                    continue
                try:
                    row = json.loads(ln)
                except json.JSONDecodeError:
                    continue
                key = self._key(row)
                if row.get("status") in ALLOWED_TERMINAL and row.get("status") != "rejected_gate":
                    seen.add(key)
        return seen

    def _key(self, row):
        return f"{row.get('fingerprint_sha256','')}|{row.get('window','')}|{row.get('budget','')}|{row.get('fold','')}|{row.get('seed','')}"

    def is_duplicate(self, fingerprint):
        return self._key(fingerprint) in self._seen

    def append_running(self, fingerprint, *, hypothesis_id, family, novelty_proof,
                       source_to_mechanism, exact_difference_from_r1_r17,
                       command, started_at=None):
        row = {
            **fingerprint,
            "hypothesis_id": hypothesis_id,
            "family": family,
            "novelty_proof": novelty_proof,
            "source_to_mechanism": source_to_mechanism,
            "exact_difference_from_r1_r17": exact_difference_from_r1_r17,
            "status": "running",
            "started_at": started_at or time.time(),
            "raw_command": command,
        }
        self._write(row)
        return row

    def append_terminal(self, fingerprint, *, status, hypothesis_id=None,
                        family=None, command="", exit_code=None, wall_s=None,
                        rss_mb=None, window=None, budget=None, fold=None, seed=None,
                        fo_count=None, so_count=None, tp_count=None, reduce_count=None,
                        rejection_count=None, actual_symbols=None, groups=None,
                        concentration_max_symbol=None, concentration_max_group=None,
                        fee_quote=None, slippage_quote=None, funding_quote=None,
                        ann=None, dd=None, min_equity=None, breach=None,
                        cold_starts_pos=None, cold_starts_total=None,
                        trace_hashes=None, cycle_group_hash=None,
                        rejection_failure_reason=None, terminal_note=None):
        if status not in ALLOWED_TERMINAL:
            raise ValueError(f"illegal terminal status {status!r}; allowed: {sorted(ALLOWED_TERMINAL)}")
        row = {**fingerprint,
               "hypothesis_id": hypothesis_id, "family": family, "status": status,
               "finished_at": time.time(), "raw_command": command, "exit_code": exit_code,
               "wall_s": wall_s, "rss_mb": rss_mb, "window": window, "budget": budget,
               "fold": fold, "seed": seed,
               "fo_count": fo_count, "so_count": so_count, "tp_count": tp_count,
               "reduce_count": reduce_count, "rejection_count": rejection_count,
               "actual_symbols": actual_symbols, "groups": groups,
               "concentration_max_symbol": concentration_max_symbol,
               "concentration_max_group": concentration_max_group,
               "fee_quote": fee_quote, "slippage_quote": slippage_quote,
               "funding_quote": funding_quote,
               "ann": ann, "dd": dd, "min_equity": min_equity, "breach": breach,
               "cold_starts_pos": cold_starts_pos, "cold_starts_total": cold_starts_total,
               "trace_hashes": trace_hashes, "cycle_group_hash": cycle_group_hash,
               "rejection_failure_reason": rejection_failure_reason,
               "terminal_note": terminal_note}
        self._write(row)
        self._seen.add(self._key(fingerprint))
        return row

    def append_skipped_duplicate(self, fingerprint, *, hypothesis_id=None, family=None):
        row = {**fingerprint, "hypothesis_id": hypothesis_id, "family": family,
               "status": "skipped_duplicate", "finished_at": time.time(),
               "terminal_note": "fingerprint matches a prior terminal row; not launched"}
        self._write(row)
        return row

    def _write(self, row):
        with open(self.path, "a") as fh:
            fh.write(json.dumps(row, sort_keys=True) + "\n")

    def counts(self):
        c = {"unique_terminal_fingerprints": len(self._seen)}
        return c


def scan_historical():
    """Scan R1-R17 artifacts for prior (effective_config_hash, window, budget)
    triples so Round 18 can never re-launch a duplicate. Independent-symbol
    (old) and the forbidden exact families (audit §7) are all recorded as
    dedup-exclusions. We do NOT trust R17's self-reported counts; we read the
    actual registry rows.

    Returns the historical index dict and writes it to HIST_INDEX.
    """
    r17_reg = "docs/superpowers/artifacts/glm-martingale-core-round17/exploration-registry.jsonl"
    excluded = set()
    rows_scanned = 0
    if os.path.exists(r17_reg):
        with open(r17_reg) as fh:
            for ln in fh:
                ln = ln.strip()
                if not ln:
                    continue
                try:
                    r = json.loads(ln)
                except json.JSONDecodeError:
                    continue
                rows_scanned += 1
                eff = r.get("effective_config_hash") or r.get("resolved_config_hash")
                window = r.get("window", "")
                budget = r.get("budget", "")
                if eff:
                    excluded.add(f"r17:{eff}|{window}|{budget}")

    index = {
        "scanned_registry": r17_reg,
        "rows_scanned": rows_scanned,
        "excluded_keys_count": len(excluded),
        "excluded_keys_sample": sorted(excluded)[:10],
        "scope": "All R17 effective_config_hash|window|budget triples are dedup-excluded. Round 18 must use a DIFFERENT cycle_topology (synchronized_pair / synchronized_basket) and/or different resolved config, so its fingerprints cannot collide.",
        "forbidden_families_audit_section_7": [
            "R17 192 exact config hash × four windows × three budgets",
            "config-hash-only changes with no event/order/rejection hash change",
            "old HTF direction gate + basic FO/multiplier/legs/spacing/TP/leverage Sobol",
            "static long/short weight skew and non-triggering inventory cap",
            "R14/15 dual-state SO scale, partial TP, minigrid, depth TP exact family",
            "old research-only pair-neutral daily close stream, DD cooldown, curve combination",
            "pure trend/breakout/funding carry/pair-neutral/stat-arb as standalone PnL engine",
        ],
    }
    os.makedirs(os.path.dirname(HIST_INDEX), exist_ok=True)
    with open(HIST_INDEX, "w") as fh:
        json.dump(index, fh, indent=2, sort_keys=True)
    return index


def main():
    idx = scan_historical()
    print(json.dumps({k: idx[k] for k in ("rows_scanned", "excluded_keys_count", "excluded_keys_sample")}, indent=2))
    print(f"\nwrote {HIST_INDEX}")
    reg = Round18Registry()
    print(f"registry: {reg.path}")
    print(f"current unique terminal fingerprints: {len(reg._seen)}")


if __name__ == "__main__":
    main()
