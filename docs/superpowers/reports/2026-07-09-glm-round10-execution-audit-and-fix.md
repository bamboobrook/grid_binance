# GLM Round 10 Execution Audit And Fix

**Date:** 2026-07-09
**Branch:** `glm-martingale-core-round10`
**Corrected status JSON:** `docs/superpowers/artifacts/glm-martingale-core-round10/r1-r10-corrected-status.json`
**Round10 final JSON:** `docs/superpowers/artifacts/glm-martingale-core-round10/r10-final-validation.json`

## Executive Verdict

This pass fixes Round10 records before issuing the next GLM plan. Round10 did run the main P3-P6 replay searches, but it overstated R9 allocator live-readiness.

Corrected frontier:

| Candidate | Status | Ann | DD | Pos segments | Live status |
|---|---|---:|---:|---:|---|
| `r9-P5-winner-lb60-rb7-calmar-hi0.2-lo0.2` | Best research allocator | 64.4196% | 18.2111% | 5/5 | module-ready + static gate only, not fully live-ready |
| `R4-combo` | Best fully live-ready sleeve | 34.7233% | 17.6843% | 4/5 | fully live-ready |

No original target is met:

| Tier | Requirement | Corrected status |
|---|---|---|
| Conservative | ann >=50%, DD <=10%, <5000U, multi-symbol, live reproducible | Fail. Best research DD is 18.2111%; best fully live-ready ann is 34.7233%. |
| Balanced | ann >=90%, DD <=20%, <5000U, multi-symbol, live reproducible | Fail. Best research ann is 64.4196% and not fully live-ready. |
| Aggressive | ann >=110%, DD <=30%, <5000U, multi-symbol, live reproducible | Fail. Best research ann is 64.4196% and not fully live-ready. |

## Fixes Applied

| Area | Problem | Fix |
|---|---|---|
| Round10 head commit | Handoff and final JSON still pointed to `40fe5df4a0fbd00d7e6aac7deae0c20098e627bf`; actual branch HEAD is `e2e58e55c4778e4f2eeda49ce39f4a383c01affb`. | Updated Round10 handoff and final validation JSON. |
| R9 allocator live-ready claim | P1 only installed static gate from existing `allocator_state`; production dynamic rebalance and state persistence are absent. | Corrected `fully_live_ready=true` to `false`; added `live_static_gate_ready=true`, `live_rebalance_ready=false`, `live_state_persistence_ready=false`. |
| P1 evidence | Tests are mostly `MartingaleRuntime` helper tests despite names mentioning main reconcile. | Marked P1 as partial and required DB-backed production tests in Round11. |
| P5 DCA minigrid | Script says the engine has no native minigrid executor and approximates minigrid with partial TP stages. | Corrected the failure family to `dca_minigrid_partial_tp_approx`; native inventory-reducing DCA minigrid remains untested. |
| P6 defensive allocator | Funding/chop/cash triggers were partly approximated through DD-style controls. | Corrected the conclusion to an approximation failure only. |
| Round1-10 authority | First nine rounds had a corrected JSON; Round10 needed to be folded into a single source. | Added `r1-r10-corrected-status.json`. |

## Code Evidence For P1 Correction

`apps/trading-engine/src/main.rs` now calls `runtime_with_allocator_state()` before `start_cycle_with_futures_preflight`. That helper:

- checks that `allocator_config` exists, but stores it as `_allocator_cfg_value` and does not parse it into an `AllocatorConfig`;
- builds `AllocatorState` from persisted JSON or a default first sleeve;
- calls `runtime.set_allocator_state_for_test(state, strategy_to_sleeve_id)`;
- returns only a snapshot for logging.

The production path does not:

- compute rolling per-sleeve metrics from completed live observations;
- call `rebalance_allocator(now_ms, cfg, metrics)` when `now_ms >= next_rebalance_ms`;
- persist updated `active_sleeve_id`, `last_rebalance_ms`, or `next_rebalance_ms`.

Therefore Round10 closes only the static inactive-sleeve new-cycle gate. It does not close the Round9 full live-readiness gap.

## Round10 Search Validation

| Task | Count | Target hits | Corrected use |
|---|---:|---:|---|
| P2 R9 parity replay | 1 replay plus 5 segment checks | 0 | Valid replay/module parity; not production live proof |
| P3 condition-triggered SO | 2880 configs | 0 | Valid failure for strict indicator-gated safety orders |
| P4 multi-TP trailing | 2304 configs | 0 | Valid failure for existing trailing_lock approximation |
| P5 DCA minigrid | 2592 configs | 0 | Valid failure only for partial-TP minigrid approximation |
| P6 defensive allocator V2 | 2304 configs | 0 | Valid failure for approximated defensive allocator |
| LOSO | 0 target candidates | 0 | Correctly not triggered |

## External Research Used For Round11 Direction

External references still point to martingale-native mechanics, not pure trend systems:

- 3Commas DCA docs: safety orders can be conditioned on technical indicators after minimum deviation. This supports conditional safety-order sizing/delay, but Round10 showed strict blocking is harmful.
  https://help.3commas.io/en/articles/9663694-dca-bot-averaging-orders-by-technical-indicators
- 3Commas trailing TP docs: trailing TP is an exit mechanic, usually after a profit target. Round10 trailing_lock approximation failed, so Round11 should avoid repeating that exact model.
  https://help.3commas.io/en/articles/3108981-how-take-profit-works-smarttrade-and-dca-bots-trailing-feature-explained
- Freqtrade strategy callbacks: position adjustment/DCA must be explicit and controlled. This supports implementing native DCA-minigrid as position adjustment, not a separate strategy.
  https://www.freqtrade.io/en/stable/strategy-callbacks/
- Hummingbot DCA executor: DCA can be modeled as an executor workflow under a controller. This supports finishing production allocator state ownership separately from order execution.
  https://hummingbot.org/strategies/v2-strategies/executors/dcaexecutor/
- Gainium minigrids DCA: minigrids between DCA steps are a distinct DCA-grid hybrid concept. Round10 did not implement this natively.
  https://gainium.io/help/minigrids-dca
- Binance USD-M exchange info: any live package must keep respecting exchange filters and notional constraints.
  https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information

## Corrected Authority Order For GLM

Use this order:

1. `docs/superpowers/artifacts/glm-martingale-core-round10/r1-r10-corrected-status.json`
2. `docs/superpowers/reports/2026-07-09-glm-round10-execution-audit-and-fix.md`
3. `docs/superpowers/artifacts/glm-martingale-core-round10/r10-final-validation.json`
4. `docs/superpowers/reports/2026-07-07-glm-round10-handoff-to-chatgpt.md` only after applying its correction notice
5. `docs/superpowers/artifacts/glm-martingale-core-round9/r1-r9-corrected-status.json` for Round1-9 details

## Remaining Gaps For Round11

1. Finish true production allocator: rolling metrics, dynamic rebalance, and persistence.
2. Run live/backtest parity again after true production allocator is wired.
3. Implement native inventory-reducing DCA-minigrid instead of partial-TP approximation.
4. Search martingale-native architectures that are not just R4-combo parameter variants.
5. Keep every failure in the non-repeat ledger before running the next grid.

## Corrected Conclusion

The first ten rounds are now usable as a corrected research ledger. The best research result is still below all target gates once live reproducibility and drawdown constraints are enforced. Round11 must first close production allocator live-readiness, then test native martingale/DCA-grid mechanics that Round10 did not actually implement.
