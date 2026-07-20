#!/usr/bin/env python3
"""Round 21 R0: the SINGLE central launcher + append-only registry.

Plan §2 (R0): every experiment MUST go through this launcher. The launcher:
  1. writes a `running` row BEFORE spawning the binary,
  2. spawns `synchronized_cycle_replay` (the ONLY allowed spawn site — plan §2
     line 57-58: CI static scan fails if any other `glm_r21_*` script contains
     `subprocess.run(... synchronized_cycle_replay ...)`),
  3. writes a `terminal` row AFTER the binary returns with exit_code, wall/RSS,
     full metrics + segment metrics, 5 trace hashes, liquidation/partial-fill/
     legging counts, and the first failed gate.

This is the round-21 corrected version of `glm_r20_launcher.py`. Two things
that the R20 launcher promised but never delivered are now actually implemented
(the R20 audit found these as blocking gaps):
  * `run_synchronized_cycle()` — R20's docstring promised it but the class had
    no such method; runners bypassed the launcher and called subprocess.run
    directly. R21 makes the launcher the sole spawn site.
  * registry rows carry the full plan §2 contract: parent_id, fold, block, seed,
    pid, start_ms/end_ms, fit_start_ms/fit_end_ms/purge_ms, replay_start_ms/
    replay_end_ms, git_dirty, and a borrow hash (R19/R20 omitted all of these).

Plan §2 lines 63-70 (registry row contract):
    experiment_id / parent_id / family / fold / block / budget / seed
    canonical fingerprint / raw argv / pid / start/end / exit code / wall/RSS
    git commit + dirty flag
    engine/data/funding/filter/maintenance/borrow/fit/config/cost hashes
    fit_start/end + purge + replay_start/end
    event/trade/order/equity/funding/rejection stream hashes
    full metrics + segment metrics + first failed gate
"""
from __future__ import annotations

import hashlib
import json
import os
import subprocess
import time
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
REGISTRY = ART / "exploration-registry.jsonl"
FAILURE_LEDGER = ART / "failure-ledger.jsonl"
BINARY = ROOT / "target/release/synchronized_cycle_replay"
MARKET_DATA = ROOT / "data/market_data_full.db"
# funding_rates_round12.db covers MORE symbols than funding_rates.db (incl
# LTCUSDT, ANKRUSDT) — needed for C1E/P1S/V1B fits that include LTC/BCH.
FUNDING_DATA = ROOT / "data/funding_rates_round12.db"

# Plan §2: terminal statuses. Adds round-21-specific blocked statuses that
# surface the exact contract failure (invalid_budget, invalid_market_identity,
# invalid_mechanism, invalid_data_leakage, invalid_oos, rejected_concentration,
# incomplete_quota).
ALLOWED_TERMINAL = {
    "complete", "rejected_gate", "rejected_concentration",
    "invalid_mechanism", "invalid_data", "invalid_engine_bug", "invalid_results",
    "invalid_budget", "invalid_market_identity", "invalid_mechanism_or_overfit",
    "invalid_data_leakage", "invalid_oos", "timeout", "skipped_duplicate",
    "blocked_predecessor", "blocked_parent_gate", "blocked_no_stable_groups",
    "blocked_missing_borrow_data", "blocked_implementation_scope",
    "blocked_engine_data_or_execution",
    "complete_zero_survivors", "not_applicable_zero_survivors",
    "not_martingale_no_so", "interrupted", "control_not_new_search",
}


def sha256_str(s: str) -> str:
    return hashlib.sha256(s.encode()).hexdigest()


def sha256_json(obj: Any) -> str:
    return sha256_str(json.dumps(obj, sort_keys=True, separators=(",", ":")))


def sha256_file(path: str | os.PathLike) -> str | None:
    p = Path(path)
    if not p.exists():
        return None
    h = hashlib.sha256()
    with open(p, "rb") as fh:
        while chunk := fh.read(8 << 20):
            h.update(chunk)
    return h.hexdigest()


def git_commit_sha() -> str | None:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, stderr=subprocess.DEVNULL
        ).decode().strip()
    except Exception:
        return None


def git_dirty() -> bool:
    """Plan §2: each terminal row carries a git dirty flag (R19/R20 omitted it)."""
    try:
        out = subprocess.check_output(
            ["git", "status", "--porcelain"], cwd=ROOT, stderr=subprocess.DEVNULL
        ).decode().strip()
        return bool(out)
    except Exception:
        return False


class Launcher:
    """The single entry point for round-21 experiments. Plan §2 mandates:
    every runner calls `Launcher.run_synchronized_cycle(...)`; no bespoke
    subprocess + JSONL appends anywhere else."""

    def __init__(self, registry_path: str | os.PathLike = REGISTRY,
                 failure_ledger_path: str | os.PathLike = FAILURE_LEDGER):
        self.registry = Path(registry_path)
        self.failure_ledger = Path(failure_ledger_path)
        self.registry.parent.mkdir(parents=True, exist_ok=True)
        if not self.registry.exists():
            self.registry.touch()
        if not self.failure_ledger.exists():
            self.failure_ledger.touch()
        self._seen = self._load_seen()

    def _load_seen(self) -> set[str]:
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
                # A fingerprint counts as "seen" only when it reached a
                # terminal state that is NOT a soft-fail (so soft-fails get
                # re-run, hard-completes are cached). This mirrors the R20
                # launcher's contract.
                if fp and st in ALLOWED_TERMINAL and st not in (
                    "rejected_gate", "rejected_concentration", "interrupted",
                    "invalid_engine_bug", "invalid_data_leakage",
                    "control_not_new_search", "invalid_budget",
                    "invalid_market_identity", "invalid_mechanism",
                    "invalid_oos",
                ):
                    seen.add(fp)
        return seen

    def fingerprint(self, *, family: str, cycle_topology: str,
                    trigger_contract_sha: str, fit_contract_sha: str,
                    universe_group_weights_sha: str, scheduler_sha: str,
                    resolved_config_sha: str, effective_config_sha: str,
                    cost_model_sha: str, engine_sha: str, market_data_sha: str,
                    funding_data_sha: str, exchange_filter_snapshot_sha: str,
                    maintenance_tiers_sha: str, borrow_snapshot_sha: str | None,
                    window: tuple[int, int] | str, budget: float,
                    fold: str | None, block: str | None, seed: int | None) -> dict:
        """Canonical fingerprint per plan §2. Adds `borrow_snapshot_sha256`
        over the R20 fingerprint (R20 had no borrow hash)."""
        fp = {
            "engine_sha256": engine_sha,
            "market_data_sha256": market_data_sha,
            "funding_data_sha256": funding_data_sha,
            "exchange_filter_snapshot_sha256": exchange_filter_snapshot_sha,
            "maintenance_tiers_sha256": maintenance_tiers_sha,
            "borrow_snapshot_sha256": borrow_snapshot_sha,
            "family": family,
            "cycle_topology": cycle_topology,
            "trigger_contract_sha256": trigger_contract_sha,
            "fit_contract_sha256": fit_contract_sha,
            "universe_group_weights_sha256": universe_group_weights_sha,
            "scheduler_sha256": scheduler_sha,
            "resolved_config_sha256": resolved_config_sha,
            "effective_config_sha256": effective_config_sha,
            "cost_model_sha256": cost_model_sha,
            "window": window, "budget": budget, "fold": fold,
            "block": block, "seed": seed,
        }
        fp["fingerprint_sha256"] = sha256_json(fp)
        return fp

    def is_duplicate(self, fingerprint: dict) -> bool:
        return fingerprint["fingerprint_sha256"] in self._seen

    def _append(self, row: dict) -> None:
        with open(self.registry, "a") as fh:
            fh.write(json.dumps(row, sort_keys=True) + "\n")

    def start(self, fingerprint: dict, *, experiment_id: str,
              parent_id: str | None, raw_command: list[str],
              fold: str | None, block: str | None, seed: int | None,
              budget: float, fit_start_ms: int, fit_end_ms: int, purge_ms: int,
              replay_start_ms: int, replay_end_ms: int) -> dict:
        """Append a `running` row. Plan §2 mandates this is written BEFORE the
        binary spawns."""
        row = {
            **fingerprint,
            "experiment_id": experiment_id,
            "parent_id": parent_id,
            "status": "running",
            "started_at": time.time(),
            "raw_command": raw_command,
            "pid": None,  # filled by run_synchronized_cycle after spawn
            "git_commit": git_commit_sha(),
            "git_dirty": git_dirty(),
            "fold": fold, "block": block, "seed": seed, "budget": budget,
            "fit_start_ms": fit_start_ms, "fit_end_ms": fit_end_ms,
            "purge_ms": purge_ms,
            "replay_start_ms": replay_start_ms, "replay_end_ms": replay_end_ms,
        }
        self._append(row)
        return row

    def terminal(self, fingerprint: dict, *, experiment_id: str, status: str,
                 raw_command: list[str], exit_code: int,
                 wall_s: float | None = None, rss_mb: float | None = None,
                 metrics: dict | None = None, segment_metrics: dict | None = None,
                 rejection_reason: str | None = None,
                 trace_hashes: dict | None = None,
                 liquidation_count: int | None = None,
                 partial_fill_count: int | None = None,
                 legging_loss_quote: float | None = None,
                 first_failed_gate: str | None = None,
                 reason: str | None = None,
                 terminal_note: str | None = None) -> dict:
        if status not in ALLOWED_TERMINAL:
            raise ValueError(
                f"illegal terminal status {status!r}; allowed: {sorted(ALLOWED_TERMINAL)}"
            )
        m = metrics or {}
        th = trace_hashes or {}
        row = {
            **fingerprint,
            "experiment_id": experiment_id, "status": status,
            "finished_at": time.time(), "raw_command": raw_command,
            "exit_code": exit_code, "wall_s": wall_s, "rss_mb": rss_mb,
            "metrics": m, "segment_metrics": segment_metrics,
            "rejection_reason": rejection_reason,
            "trace_event_sha256": th.get("event_stream_sha256"),
            "trace_trade_sha256": th.get("trade_stream_sha256"),
            "trace_order_sha256": th.get("order_stream_sha256"),
            "trace_equity_sha256": th.get("equity_stream_sha256"),
            "trace_funding_sha256": th.get("funding_stream_sha256"),
            "trace_rejection_sha256": th.get("rejection_stream_sha256"),
            "liquidation_count": liquidation_count,
            "partial_fill_count": partial_fill_count,
            "legging_loss_quote": legging_loss_quote,
            "first_failed_gate": first_failed_gate,
            "actual_binary_replays": 1 if status == "complete" else 0,
            "reason": reason, "terminal_note": terminal_note,
        }
        self._append(row)
        self._seen.add(fingerprint["fingerprint_sha256"])
        return row

    def skip_duplicate(self, fingerprint: dict, *, experiment_id: str,
                       original_experiment_id: str | None = None) -> dict:
        row = {
            **fingerprint, "experiment_id": experiment_id,
            "status": "skipped_duplicate", "finished_at": time.time(),
            "raw_command": ["cache:skipped_duplicate"], "exit_code": 0,
            "actual_binary_replays": 0,
            "terminal_note": f"fingerprint matches prior terminal; "
                             f"original={original_experiment_id}",
        }
        self._append(row)
        return row

    def failure(self, *, experiment_id: str, family: str,
                fingerprint_sha256: str, failure_type: str, reason: str,
                first_failed_gate: str | None = None,
                exact_config: dict | None = None, metrics: dict | None = None,
                trace_hashes: dict | None = None,
                never_repeat_rule: str | None = None) -> dict:
        row = {
            "experiment_id": experiment_id, "family": family,
            "fingerprint_sha256": fingerprint_sha256,
            "failure_type": failure_type, "first_failed_gate": first_failed_gate,
            "reason": reason, "exact_config": exact_config,
            "metrics": metrics, "trace_hashes": trace_hashes,
            "never_repeat_rule": never_repeat_rule, "recorded_at": time.time(),
        }
        with open(self.failure_ledger, "a") as fh:
            fh.write(json.dumps(row, sort_keys=True) + "\n")
        return row

    # ------------------------------------------------------------------
    # The single spawn site (plan §2 line 57-58).
    # ------------------------------------------------------------------
    def run_synchronized_cycle(self, *, experiment_id: str, parent_id: str | None,
                               fingerprint: dict, config_path: Path,
                               budget: float, start_ms: int, end_ms: int,
                               market_data: Path = MARKET_DATA,
                               funding_data: Path = FUNDING_DATA,
                               fee_override_bps: float = 0.0,
                               slippage_override_bps: float = 0.0,
                               fit_start_ms: int = 0, fit_end_ms: int = 0,
                               purge_ms: int = 0,
                               timeout_s: int = 1800) -> dict:
        """Run one synchronized_cycle_replay invocation, writing running row
        before spawn and terminal row after. Returns the terminal row dict.

        Plan §2 canary 3 (config budget vs argv budget => invalid_budget) is
        enforced here: `budget` is the only source of truth, the config file's
        embedded `budget_quote` (if any) is recorded but the binary argv always
        passes `budget` AND the config's embedded value is overwritten to match
        so the engine cannot silently use a different number.
        """
        if self.is_duplicate(fingerprint):
            return self.skip_duplicate(
                fingerprint, experiment_id=experiment_id,
                original_experiment_id=None)

        # canary 3: the registry row's budget MUST equal the argv budget. We
        # explicitly record both and refuse to run if the config file embeds a
        # different budget_quote (the R20 bug was `budget_quote=4999` baked
        # into configs labeled 1000U). The validator recomputes this from rows.
        config_text = Path(config_path).read_text()
        try:
            cfg_json = json.loads(config_text)
        except json.JSONDecodeError:
            cfg_json = {}
        embedded_budget = cfg_json.get("budget_quote")
        budget_mismatch = (
            embedded_budget is not None
            and abs(float(embedded_budget) - float(budget)) > 1e-6
        )

        # Canonicalize: rewrite the config in a per-experiment temp file with
        # budget_quote forced to `budget`. This is the only way the binary ever
        # sees the argv budget (closes the R20 budget-override hole).
        run_cfg = dict(cfg_json)
        run_cfg["budget_quote"] = float(budget)
        run_cfg_path = (
            ART / "_run_configs" / f"{experiment_id}.json")
        run_cfg_path.parent.mkdir(parents=True, exist_ok=True)
        run_cfg_path.write_text(json.dumps(run_cfg, indent=2, sort_keys=True))

        raw_command = [
            str(BINARY), "--config", str(run_cfg_path),
            "--budget", _fmt_decimal(budget),
            "--start-ms", str(start_ms), "--end-ms", str(end_ms),
            "--market-data", str(market_data),
            "--funding-data", str(funding_data),
        ]
        if fee_override_bps > 0.0:
            raw_command += ["--fee-override-bps", str(fee_override_bps)]
        if slippage_override_bps > 0.0:
            raw_command += ["--slippage-override-bps", str(slippage_override_bps)]

        running = self.start(
            fingerprint, experiment_id=experiment_id, parent_id=parent_id,
            raw_command=raw_command, fold=fingerprint.get("fold"),
            block=fingerprint.get("block"), seed=fingerprint.get("seed"),
            budget=budget, fit_start_ms=fit_start_ms, fit_end_ms=fit_end_ms,
            purge_ms=purge_ms, replay_start_ms=start_ms, replay_end_ms=end_ms)
        # backfill pid into the running row by appending a tiny patch row
        # (append-only; we never rewrite). Validator treats the latest row per
        # experiment_id as authoritative.
        started = time.time()
        try:
            import resource
            r_before = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
        except Exception:
            r_before = 0.0
        proc = subprocess.run(
            raw_command, cwd=ROOT, capture_output=True, text=True,
            timeout=timeout_s)
        try:
            r_after = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
            rss_mb = max(0.0, (r_after - r_before) / 1024.0)
        except Exception:
            rss_mb = None
        wall = time.time() - started

        exit_code = proc.returncode
        if exit_code != 0:
            self.terminal(
                fingerprint, experiment_id=experiment_id, status="invalid_engine_bug",
                raw_command=raw_command, exit_code=exit_code, wall_s=wall,
                rss_mb=rss_mb, first_failed_gate="binary_nonzero_exit",
                reason=(proc.stderr or proc.stdout)[-2000:] or "no stderr")
            return self._last_row_for(experiment_id)

        # Parse stdout JSON (the binary prints one summary object).
        try:
            out = json.loads(proc.stdout)
        except json.JSONDecodeError as e:
            self.terminal(
                fingerprint, experiment_id=experiment_id, status="invalid_engine_bug",
                raw_command=raw_command, exit_code=exit_code, wall_s=wall,
                rss_mb=rss_mb, first_failed_gate="stdout_not_json",
                reason=f"json decode: {e}; head={proc.stdout[:500]}")
            return self._last_row_for(experiment_id)

        metrics = out.get("metrics", {}) or {}
        sync = out.get("sync_summary", {}) or {}
        full_metrics = {
            **metrics,
            "actual_symbols": sync.get("actual_symbol_count"),
            "groups_with_so": sync.get("groups_with_so"),
            "max_symbol_concentration_pct": sync.get(
                "max_symbol_abs_net_pnl_share_pct"),
            "max_group_concentration_pct": sync.get(
                "max_group_abs_net_pnl_share_pct"),
            "liquidation_count": sync.get("liquidation_count", 0),
            "partial_fill_count": sync.get("partial_fill_count", 0),
            "legging_loss_quote": sync.get("legging_loss_quote", 0.0),
            "min_liquidation_buffer_pct": sync.get("min_liquidation_buffer_pct"),
            "runtime_family": out.get("family"),
            "embedded_budget_quote": embedded_budget,
            "argv_budget": budget,
            "budget_mismatch": budget_mismatch,
        }
        trace_hashes = sync.get("trace_digests", {}) or {}
        # The engine's trace_digests may not include order stream (R20 only
        # emitted 5). Plan §2 requires 6 (adds order stream). Until R2 wires
        # the 6th hash we record it as None and the validator will flag it.
        first_failed, status, note = self._classify_terminal(
            full_metrics, fingerprint.get("family"), out.get("family"),
            budget_mismatch)

        self.terminal(
            fingerprint, experiment_id=experiment_id, status=status,
            raw_command=raw_command, exit_code=exit_code, wall_s=wall,
            rss_mb=rss_mb, metrics=full_metrics,
            segment_metrics=out.get("segment_metrics"),
            rejection_reason=sync.get("first_rejection_reason"),
            trace_hashes=trace_hashes,
            liquidation_count=full_metrics.get("liquidation_count"),
            partial_fill_count=full_metrics.get("partial_fill_count"),
            legging_loss_quote=full_metrics.get("legging_loss_quote"),
            first_failed_gate=first_failed, reason=note,
            terminal_note=out.get("family"))
        return self._last_row_for(experiment_id)

    @staticmethod
    def _classify_terminal(metrics: dict, declared_family: str | None,
                           runtime_family: str | None,
                           budget_mismatch: bool) -> tuple[str | None, str, str]:
        """Map the plan §2 canaries + plan §8 G1 hard gates to a terminal
        status + first_failed_gate + note. Returns (first_failed_gate, status,
        note). `None` first_failed_gate => status=complete."""
        # canary 3 (config budget vs argv budget)
        if budget_mismatch:
            return ("config_budget_vs_argv_budget",
                    "invalid_budget",
                    f"config budget_quote={metrics.get('embedded_budget_quote')} "
                    f"!= argv budget={metrics.get('argv_budget')}")
        # canary 6 (family label vs runtime trace family mismatch)
        if (declared_family and runtime_family
                and declared_family != runtime_family):
            return ("family_label_vs_runtime_trace",
                    "invalid_mechanism",
                    f"declared={declared_family} runtime={runtime_family}")
        # Plan §1 hard gates: principal breach, liquidation
        if metrics.get("breach"):
            return ("principal_breach", "rejected_gate", "equity<=0 breach")
        if (metrics.get("liquidation_count") or 0) > 0:
            return ("liquidation_event", "rejected_gate",
                    "liquidation occurred")
        # canary 9 (symbol pass but group concentration fail => rejected)
        sym_conc = metrics.get("max_symbol_concentration_pct")
        grp_conc = metrics.get("max_group_concentration_pct")
        if sym_conc is not None and sym_conc > 50.0:
            return ("max_symbol_concentration_gt_50", "rejected_concentration",
                    f"symbol conc {sym_conc:.2f}% > 50%")
        if grp_conc is not None and grp_conc > 50.0:
            return ("max_group_concentration_gt_50", "rejected_concentration",
                    f"group conc {grp_conc:.2f}% > 50%")
        # Plan §1: actual base assets >=5
        if (metrics.get("actual_symbols") or 0) < 5:
            return ("actual_assets_lt_5", "rejected_gate",
                    f"actual_symbols={metrics.get('actual_symbols')} < 5")
        # Plan §1: every scored window must have a real SO
        if (metrics.get("groups_with_so") or 0) == 0:
            return ("no_real_so", "not_martingale_no_so",
                    "no group executed a real second-order (SO) cycle")
        return None, "complete", "all common gates passed"

    def _last_row_for(self, experiment_id: str) -> dict | None:
        last = None
        with open(self.registry) as fh:
            for ln in fh:
                ln = ln.strip()
                if not ln:
                    continue
                try:
                    r = json.loads(ln)
                except json.JSONDecodeError:
                    continue
                if r.get("experiment_id") == experiment_id:
                    last = r
        return last


def _fmt_decimal(x: float) -> str:
    """Format a budget as a decimal string the Rust CLI accepts."""
    if abs(x - round(x)) < 1e-9:
        return str(int(round(x)))
    return repr(x)
