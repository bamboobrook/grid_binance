# GLM Martingale Round 6 DD Compression Search Ledger

Branch: `glm-martingale-core-round6`
Plan: `docs/superpowers/plans/2026-07-03-glm-martingale-core-round6-dd-compression-plan.md`
Round 5 best: `r5-G-best-ANKRUSDT` ann 59.5%, DD 32.1%, 4/5 pos, 2025 -17.5%.
Goal: compress DD to ≤10% (conservative) / ≤20% (balanced) / ≤30% (aggressive).
Order: A(attribution) → B(DD state machine) → C(quarantine) → D(trailing lock) → E(safety freeze) → F(blend) → G(parity) → H(ANKR closeout).


## r6-A-dd-attribution-001 (Task A: Drawdown-Window Attribution)
- Segment DD: 2024=15.7%, h1_2023=12.3%, 2025=6.0%, h2_2023=5.3%, 2026=3.4%
- Overall 32.1% DD includes startup effect (trough in Jan 2023 before gains compound)
- Funding drag: 677 USDT on 5000U budget (13.5% annual)
- Routes: B(DD state machine), F(blend), D(trailing lock - low stop rate)

## r6-B-dd-state-machine-001 (Task B: Portfolio DD State Machine — FULL 14-candidate grid)
- Implemented MartingaleDrawdownStateRule config + engine logic. 208 tests pass.
- Grid: 4 state-threshold sets × 3 recoveries + 2 baselines = 14 × 6 replays, 89s.
- **RESULT: no effect.** ALL candidates identical (ann59.5/dd32.1). The DD state machine rules don't compress DD because: the engine's portfolio_drawdown_pct uses margin-based equity (lower running DD), and the first_order_scale from rules isn't applied to base order sizing in the current implementation.
- Non-repeat key: staged-dd-state-machine (needs engine refactor to apply scaling to base order + use on_budget DD metric)

## r6-F-blend-001 (Task F: Risk-Budget Blend — FULL 31-candidate grid)
- Grid: 31 candidates (ANKR × R4/fine/XRP/DOGE blends at various ratios) × 6 replays, 289s.
- **RESULT: frontier improvement!** Best blend: ankr20_r460 = ann 34.9%, **DD 19.4%**, 4/5 pos, 2025 -11.7%.
  - DD compressed from 32.1% (ANKR100) to **19.4%** (ankr20_r460). This is within Balanced DD ≤20%!
  - But ann dropped from 59.5% to 34.9%.
  - 6 frontier improvements.
- Pure XRP: ann 50.6%/DD 25.4%/4/5 (known from R5).
- Many high-ratio blends give ~0% ann (budget exhaustion from 12+ strategies).
- Best DD-ann tradeoff: ankr20_r460 (ann 34.9%/DD 19.4%) or xrp100 (ann 50.6%/DD 25.4%).

## r6-D-trailing-lock-001 (Task D: Partial-TP Trailing Lock)
- Config fields added (trailing_lock_after_stage/activation_bps/callback_bps/floor_bps). 208 tests pass.
- Grid: 18 candidates (6 trailing configs × 3 freeze configs) × 6 replays, 107s.
- **RESULT: no effect.** All identical to baseline. Engine doesn't implement trailing lock exit logic yet — only state fields exist.
- Non-repeat key: trailing-lock-config-only-needs-engine-logic

## r6-E-safety-freeze-001 (Task E: Safety Freeze After Partial TP)
- Same grid as Task D. freeze_safety_after_partial_tp_stage field parses but engine doesn't check it.
- **RESULT: no effect.** Same as D.
- Non-repeat key: safety-freeze-config-only-needs-engine-logic

## r6-C-quarantine-001 (Task C: Symbol Quarantine) — DEFERRED
- Needs MartingaleQuarantineRule struct + engine entry-path logic. Deferred.

## r6-G-parity-001 (Task G: Trading-Engine Parity) — DOCUMENTED
- R5 features (last-exec SO, vol-target, risk reduction) + R6 features (DD state machine, trailing lock, safety freeze) all backtest-only.
- Trading-engine has 4/4 R4 parity features (187 tests).

## r6-H-ankr-closeout-001 (Task H: ANKR Low-DD Closeout) — SUPERSEDED
- Superseded by Task F blend (ankr20_r460 = ann34.9%/DD19.4%).

## r6-B-dd-state-machine-002 (Task B FIXED: Budget-based DD + engine logic)
- Implemented budget_based_dd_pct + first_order_scale + freeze_safety_orders engine logic. 208 tests pass.
- Smoke test confirmed DD state machine fires (dd8_16_25 changed trades from 367 to 4409).
- Full grid: 34 candidates × 6 replays, 200s.
- **RESULT: marginal effect.** DD state machine fires but cannot compress DD below ~30%.
  - Scaling-only: ann 59.4/DD 32.1 (no DD change — DD from unrealized PnL, not new entries)
  - Freeze variants: DD 30.5% (from 32.1%) but ann drops to 33.3%, pos to 2/5
  - Root cause: DD comes from EXISTING position unrealized PnL, not new entry sizing
- The best DD compression remains Task F blend (DD 19.4% via portfolio blending).
