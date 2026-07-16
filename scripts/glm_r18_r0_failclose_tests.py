#!/usr/bin/env python3
"""Round 18 R0 fail-close tests.

Plan §3.1 requires these NEW fail-close tests. They MUST first fail on the
current (Round 17) code, proving the bug exists, then pass only after the
Round 18 fix. This is test-driven: run with `--expect-fail` to confirm the
tests detect the R17 bugs, then after the fix run normally.

Each test reads RAW evidence (Rust source, registry rows, gate evidence) and
recomputes the verdict. It never trusts self-reported passed=true/all_bound=true
/used_real_service=true as fact.

Plan-required test names (§3.1):
  r17_family_config_hash_only_is_not_bound
  r17_family_two_traces_cannot_satisfy_eight_to_sixteen
  r17_identical_ablation_event_hash_blocks_b2
  r17_noop_vol_cap_is_rejected
  r17_noop_cluster_scheduler_is_rejected
  r17_hazard_same_open_and_now_never_counts_as_active
  r17_production_zero_ohlc_bar_is_rejected
  r17_checkpoint_without_command_and_hash_is_not_actual_registry_replay

Run:
  python3 scripts/glm_r18_r0_failclose_tests.py            # after fix: all pass
  python3 scripts/glm_r18_r0_failclose_tests.py --expect-fail  # before fix: detect bugs
"""
import argparse
import json
import os
import re
import sys

R17_CONTROLS = "apps/backtest-engine/src/martingale/r17_controls.rs"
KLINE_ENGINE = "apps/backtest-engine/src/martingale/kline_engine.rs"
PROD_MAIN = "apps/trading-engine/src/main.rs"
R17_ART = "docs/superpowers/artifacts/glm-martingale-core-round17"
R17_REGISTRY = os.path.join(R17_ART, "exploration-registry.jsonl")
A4_BINDING = os.path.join(R17_ART, "a4", "gates", "ten_family_binding.json")
B1_ABLATION = os.path.join(R17_ART, "b1", "gates", "mechanism_ablation_executed.json")


def read(path):
    try:
        with open(path, errors="replace") as fh:
            return fh.read()
    except FileNotFoundError:
        return ""


def read_json(path):
    try:
        with open(path) as fh:
            return json.load(fh)
    except (FileNotFoundError, json.JSONDecodeError):
        return None


# ---------------------------------------------------------------------------
# Each test returns (passed: bool, detail: dict). "passed" means the FIX is in
# place (i.e. the bug is GONE). On current R17 code these return False.
# ---------------------------------------------------------------------------

def _extract_fn_body(src, fn_name):
    """Locate `pub fn <fn_name>(...) [-> Ret] {` allowing multi-line sigs,
    return the body text between the opening brace and the next `pub fn`/`///`."""
    m = re.search(r"pub fn " + re.escape(fn_name) + r"\s*\([^)]*\)(?:\s*->\s*[^{]*)?\s*\{", src, re.DOTALL)
    if not m:
        return None
    start = m.end()
    # next top-level fn or doc-comment start
    nxt = re.search(r"\n(?:pub fn |/// )", src[start:])
    end = start + (nxt.start() if nxt else len(src) - start)
    return src[start:end]


def t_noop_vol_cap_is_rejected():
    """vol_cap must read vol/reserve and change a real FO/SO quote or rejection.
    A body of `let _ = (strategy, cfg);` is a no-op and must be rejected."""
    src = read(R17_CONTROLS)
    body = _extract_fn_body(src, "vol_cap")
    if body is None:
        return False, {"reason": "vol_cap function not found"}
    stripped = re.sub(r"//[^\n]*", "", body)
    stripped = stripped.replace(" ", "").replace("\n", "")
    is_noop = ("let_=(strategy,cfg);" in stripped
               or "let_=(strategy,cfg)" in stripped
               or len(stripped.replace("{", "").replace("}", "")) < 5)
    return (not is_noop), {"is_noop": is_noop, "body_snippet": body.strip()[:160]}


def t_noop_cluster_scheduler_is_rejected():
    """cluster_scheduler must read real open cycles / next-SO reserve / margin
    and change a real order/rejection. No-op body rejected."""
    src = read(R17_CONTROLS)
    body = _extract_fn_body(src, "cluster_scheduler")
    if body is None:
        return False, {"reason": "cluster_scheduler function not found"}
    stripped = re.sub(r"//[^\n]*", "", body)
    stripped = stripped.replace(" ", "").replace("\n", "")
    is_noop = ("let_=(strategy,cfg);" in stripped
               or "let_=(strategy,cfg)" in stripped
               or len(stripped.replace("{", "").replace("}", "")) < 5)
    return (not is_noop), {"is_noop": is_noop, "body_snippet": body.strip()[:160]}


def t_hazard_same_open_and_now_never_counts_as_active():
    """hazard_deadline is called with (cycle_opened_ms, now_ms). If both are the
    SAME value (e.g. legs_filled passed as both), age_h is always 0 and the
    deadline can never fire. The call site must pass a real cycle-open timestamp
    distinct from the current bar timestamp."""
    src = read(KLINE_ENGINE)
    m = re.search(r"hazard_deadline\(([^;]*?)\)\s*;", src, re.DOTALL)
    if not m:
        return False, {"reason": "hazard_deadline call site not found"}
    call = m.group(1)
    args = [a.strip() for a in call.split(",")]
    # args layout: strategy_id, cycle_opened_ms, now_ms, cfg
    if len(args) < 4:
        return False, {"reason": "hazard_deadline call has <4 args", "call": call[:120]}
    opened = args[1]
    now = args[2]
    same_value = (opened == now)
    # legs_filled is an i64 leg count, not a timestamp. Reject if either is
    # `legs_filled` (a leg-count proxy) rather than a real ms timestamp.
    uses_leg_count_proxy = ("legs_filled" in opened) or ("legs_filled" in now)
    passed = (not same_value) and (not uses_leg_count_proxy)
    return passed, {"opened_arg": opened, "now_arg": now,
                    "same_value": same_value, "uses_leg_count_proxy": uses_leg_count_proxy}


def t_family_config_hash_only_is_not_bound():
    """A4 binding must require event_hash OR order/rejection hash change, not
    config-hash change alone. Reject any family whose 'bound' is true while
    event_hash_bound=false AND trade/order_hash_bound=false."""
    ev = read_json(A4_BINDING)
    if not isinstance(ev, dict):
        return False, {"reason": f"no A4 binding evidence at {A4_BINDING}"}
    fams = ev.get("families")
    if not isinstance(fams, list):
        return False, {"reason": "no families list"}
    bad = []
    for f in fams:
        bound = f.get("bound") is True
        event_bound = f.get("event_hash_bound") is True
        order_bound = f.get("order_hash_bound") is True or f.get("trade_hash_bound") is True
        if bound and not (event_bound or order_bound):
            bad.append(f.get("name"))
    return (len(bad) == 0), {"families_bound_without_event_or_order_hash": bad}


def t_family_two_traces_cannot_satisfy_eight_to_sixteen():
    """Plan §11 G0 requires 8-16 synthetic traces per family. A family whose
    trace_count < 8 must NOT be accepted as G0-bound even if config-hash changes."""
    ev = read_json(A4_BINDING)
    if not isinstance(ev, dict):
        return False, {"reason": "no A4 binding evidence"}
    fams = ev.get("families")
    if not isinstance(fams, list):
        return False, {"reason": "no families list"}
    under = [f.get("name") for f in fams if int(f.get("trace_count", 0) or 0) < 8]
    # Test passes only if EVERY family has >=8 traces (the corrected requirement)
    return (len(under) == 0), {"families_with_trace_count_below_8": under}


def t_identical_ablation_event_hash_blocks_b2():
    """If all B1 ablation arms share one event_hash (mechanism inert), B2 must
    NOT launch. Reject ablation evidence where arm event_hash set size == 1."""
    ev = read_json(B1_ABLATION)
    if not isinstance(ev, dict):
        return False, {"reason": "no B1 ablation evidence"}
    arms = ev.get("ablation_arms")
    if not isinstance(arms, list):
        return False, {"reason": "no ablation_arms list"}
    hashes = {a.get("event_hash") for a in arms if isinstance(a, dict)}
    distinct = len(hashes)
    # Passed means: the evidence shows divergence (distinct > 1) OR, if it shows
    # degeneracy, a 'blocked_inert' status was recorded so B2 did not launch.
    blocked = ev.get("blocked_inert") is True or ev.get("status") in (
        "blocked_inert", "rejected_gate")
    passed = (distinct > 1) or blocked
    return passed, {"distinct_event_hashes": distinct, "blocked_inert_flag": blocked}


def t_production_zero_ohlc_bar_is_rejected():
    """Production router must receive each strategy's REAL completed Kline, not
    a hardcoded symbol=BTCUSDT with all OHLCV=0. Reject the pattern where a
    single hardcoded bar is pushed for every strategy."""
    src = read(PROD_MAIN)
    if not src:
        return False, {"reason": f"no {PROD_MAIN}"}
    # Detect the literal zero-OHLCV construction feeding router_push_completed_1m.
    zero_bar = bool(re.search(r"symbol\s*[:=]\s*\"BTCUSDT\"[^}]{0,200}open\s*[:=,]\s*0(\.0)?[^}]{0,400}close\s*[:=,]\s*0(\.0)?", src, re.DOTALL))
    # A more lenient detect: router_push_completed_1m called with a hardcoded
    # BTCUSDT and a bar literal where high/low/open/close/volume are all zero.
    all_zero = bool(re.search(r"open:\s*0(?:\.0)?[,\s]+high:\s*0(?:\.0)?[,\s]+low:\s*0(?:\.0)?[,\s]+close:\s*0(?:\.0)?[,\s]+volume:\s*0(?:\.0)?", src))
    return (not (zero_bar or all_zero)), {"zero_ohlc_bar_detected": zero_bar or all_zero}


def t_checkpoint_without_command_and_hash_is_not_actual_registry_replay():
    """A registry terminal row is an actual replay only if it has a real
    raw_command (the binary invocation, not 'cache') and exit_code is present.
    skipped_duplicate rows must NOT be counted as actual_binary_replays."""
    if not os.path.exists(R17_REGISTRY):
        return False, {"reason": "no registry"}
    rows = []
    with open(R17_REGISTRY) as fh:
        for ln in fh:
            ln = ln.strip()
            if ln:
                try:
                    rows.append(json.loads(ln))
                except json.JSONDecodeError:
                    pass
    miscounted = 0
    actual = 0
    for r in rows:
        st = r.get("status")
        if st == "skipped_duplicate":
            # if a skipped_duplicate row reports actual_binary_replays > 0 it is miscounted
            if int(r.get("actual_binary_replays", 0) or 0) > 0:
                miscounted += 1
        if st in ("complete", "complete_zero_survivors"):
            actual += 1
            if r.get("raw_command") in (None, "", "cache") or "exit_code" not in r:
                miscounted += 1
    # The validator's gate-evidence self-report claimed 2304 actual; reality is 'actual'.
    self_report = None
    b2g = os.path.join(R17_ART, "b2", "gates", "g1_192_sobol_2304_replays.json")
    ev = read_json(b2g)
    if isinstance(ev, dict):
        self_report = ev.get("actual_replays")
    consistent = (self_report is None) or (self_report == actual) or (abs((self_report or 0) - actual) <= 50)
    return (miscounted == 0 and consistent), {
        "actual_complete_rows": actual, "miscounted_rows": miscounted,
        "self_reported_actual_replays": self_report, "consistent_with_registry": consistent,
    }


TESTS = [
    ("r17_noop_vol_cap_is_rejected", t_noop_vol_cap_is_rejected),
    ("r17_noop_cluster_scheduler_is_rejected", t_noop_cluster_scheduler_is_rejected),
    ("r17_hazard_same_open_and_now_never_counts_as_active", t_hazard_same_open_and_now_never_counts_as_active),
    ("r17_family_config_hash_only_is_not_bound", t_family_config_hash_only_is_not_bound),
    ("r17_family_two_traces_cannot_satisfy_eight_to_sixteen", t_family_two_traces_cannot_satisfy_eight_to_sixteen),
    ("r17_identical_ablation_event_hash_blocks_b2", t_identical_ablation_event_hash_blocks_b2),
    ("r17_production_zero_ohlc_bar_is_rejected", t_production_zero_ohlc_bar_is_rejected),
    ("r17_checkpoint_without_command_and_hash_is_not_actual_registry_replay", t_checkpoint_without_command_and_hash_is_not_actual_registry_replay),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--expect-fail", action="store_true",
                    help="before-fix mode: assert all tests FAIL (proving bugs exist)")
    ap.add_argument("--json", action="store_true", help="emit machine-readable json")
    args = ap.parse_args()

    results = []
    for name, fn in TESTS:
        passed, detail = fn()
        results.append({"name": name, "passed": passed, "detail": detail})

    if args.expect_fail:
        all_fail = all(not r["passed"] for r in results)
        if args.json:
            print(json.dumps({"mode": "expect-fail", "all_bugs_detected": all_fail, "results": results}, indent=2))
        else:
            for r in results:
                tag = "DETECTED" if not r["passed"] else "NOT-DETECTED"
                print(f"  [{tag}] {r['name']}")
            print(f"\nexpect-fail: all_bugs_detected={all_fail}")
        sys.exit(0 if all_fail else 1)

    all_pass = all(r["passed"] for r in results)
    if args.json:
        print(json.dumps({"mode": "post-fix", "all_pass": all_pass, "results": results}, indent=2))
    else:
        for r in results:
            tag = "PASS" if r["passed"] else "FAIL"
            print(f"  [{tag}] {r['name']}  {json.dumps(r['detail'])}")
        print(f"\npost-fix: all_pass={all_pass}")
    sys.exit(0 if all_pass else 1)


if __name__ == "__main__":
    main()
