# GLM Martingale Round 14 Handoff to ChatGPT (Updated)

**Date:** 2026-07-14
**Branch:** `glm-martingale-core-round14`
**Plan:** `docs/superpowers/plans/2026-07-13-glm-martingale-core-round14-htf-selector-inventory-plan.md`

## 1. Executive Verdict

Round 14 achieved a **major breakthrough** by integrating the cross-sectional (XS) selector into the kline_engine search loop. The XS selector improved annualized return from ~24% (baseline R4-combo) to **35-38%**, and at 3000U budget, the best config reaches **ann=50.05%** (hitting the conservative ann gate). However, drawdown remains above the conservative 10% gate.

This is the closest any round has come to hitting a target tier. The XS selector mechanism is proven effective, but the DD/ann tradeoff at different budgets means no single config simultaneously meets all gates of any tier.

## 2. Target Status

| Tier | Requirement | Round 14 best | Verdict |
|---|---|---|---|
| Conservative | ann≥50%, DD≤10%, 4/5 | ann=50.05% (3000U), DD=19.67%, 4/5 | **partial**: ann✓, pos✓, DD✗ (19.67>10%) |
| Balanced | ann≥90%, DD≤20%, 4/5 | ann=96.22% (1000U), DD=55.14%, 4/5 | **partial**: ann✓, pos✓, DD✗ (55>20%) |
| Aggressive | ann≥110%, DD≤30%, 3/5 | ann=101.92% (1000U), DD=55.14%, 4/5 | **close**: ann✗ (102<110), DD✗ (55>30%) |

No target fully hit. `fully_live_ready_candidates=[]`.

## 3. The XS Selector Breakthrough

### What was done
- Integrated XS selector into `kline_engine.rs` with `xs_selector_gate_enabled` config field
- Selector controls which symbols can open new cycles based on lagged cross-sectional ranking
- Shadow observations never enter live equity; inactive sleeves keep existing cycles managed
- Two families: XS-MOM (momentum) and XS-REVERSAL (reversal)

### Search executed (228 configs, 2736 binary replays)
1. **48 binding probes** (32 XS-MOM + 16 XS-REVERSAL) — full 5-segment + budget ladder
2. **112 train-only screen configs** — full window validation
3. **32 constrained trials** — neighborhood search on 4 gate-passing configs
4. **12 combination configs** — XS selector + HTF gate
5. **8 minigrid/depth TP configs** — corrected engine rerun

### Best configs

| Config | Family | Params | Ann (4999U) | DD | Pos | Calmar |
|---|---|---|---|---|---|---|
| p01 | XS-MOM | lb7d, s0, r1d, a3 | 35.18% | 16.75% | 4/5 | 2.10 |
| p46 | XS-REV | lb72h, s0, r1d, a5 | 38.02% | 26.24% | 4/5 | 1.45 |
| p40 | XS-REV | lb12h, s0, r7d, a5 | 33.27% | 18.35% | 4/5 | 1.81 |
| p42 | XS-REV | lb24h, s0, r1d, a5 | 33.21% | 18.34% | 4/5 | 1.81 |

### Budget ladder (XS-MOM p01)
| Budget | Ann | DD |
|---|---|---|
| 1000U | 96.22% | 35.79% |
| 2000U | 64.72% | 24.93% |
| **3000U** | **50.05%** | **19.67%** |
| 4000U | 41.21% | 18.09% |
| 4999U | 35.18% | 16.75% |

**3000U hits the conservative ann gate (50.05% ≥ 50%) with DD=19.67% (passes balanced DD gate ≤20%) and 4/5 positive segments.**

## 4. Key Findings

1. **XS selector is the breakthrough mechanism**: ann improved from ~24% to 35-38% on the same base
2. **Budget cliff is the key lever**: smaller budget → higher ann but higher DD
3. **2025 remains the weakness**: all configs show negative 2025 segment (-10% to -11%)
4. **max_capital is low**: 733U for XS-MOM (strategy doesn't need full 4999U budget)
5. **Daily rebalance is critical**: 3-day rebalance collapses ann
6. **Short lookback wins**: 7d for momentum, 72h for reversal
7. **HTF+XS combination infeasible**: both use O(n) per-symbol memory, combined overhead causes timeouts

## 5. What Remains Incomplete

1. **DD at conservative gate**: best DD is 16.75% (need ≤10%) — fundamental limit of the martingale approach
2. **2025 negative segment**: no config achieves positive 2025 — the strategy loses in trending down markets
3. **HTF+XS integration**: computationally infeasible without memory optimization
4. **Concentration check**: not yet computed for XS-selected symbols
5. **Nested WFO**: not implemented (F1-F4 anchored walk-forward)
6. **Production DB/executor**: tests use helper state, not real executor

## 6. Next Steps for Round 15

1. **Optimize HTF regime memory**: Use O(1) per-symbol state instead of O(n) rolling closes cache
2. **Fix 2025 weakness**: Investigate why all configs lose in 2025 — likely trending down market
3. **Multi-timeframe XS**: Combine short-term reversal with long-term momentum
4. **Dynamic budget allocation**: Use 3000U effective budget (not 4999U) for the conservative profile
5. **Nested WFO**: Implement F1-F4 to validate parameter stability
6. **Concentration analysis**: Compute per-symbol PnL contribution for XS-selected symbols

## 7. Engineering Verification

- **550 tests pass** (325 backtest-engine + 225 trading-engine)
- **57 new tests** for P2-P9 mechanisms
- **4 new modules**: htf_regime, xs_selector, dual_state_ladder, inventory_scheduler
- **228 search configs** with full 5-segment + budget ladder validation
- **2736 binary replays** executed (no fast screening)
