# GLM Martingale Core Round 10 Live Allocator And New Sleeves Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Round 9 allocator fully live-reproducible, then search new martingale-native sleeve architectures that were not covered by the first nine rounds.

**Architecture:** Martingale/DCA/grid remains the only trading core. External signals may only gate entries, safety orders, exits, capital reserve, sleeve selection, or cash defense. They must not create pure trend, breakout, funding-carry, pair-neutral, or statistical-arbitrage trades.

**Tech Stack:** Rust `backtest-engine` and `trading-engine`, Python discovery scripts, `portfolio_budget_replay`, SQLite market/funding/premium data, JSON/JSONL artifacts under `docs/superpowers/artifacts/glm-martingale-core-round10/`.

---

## 0. Non-Negotiable Gates

Targets remain unchanged:

| Tier | Ann | DD | Budget | Other gates |
|---|---:|---:|---:|---|
| Conservative | >=50% | <=10% | <5000U | 4/5 positive segments, multi-symbol, live reproducible |
| Balanced | >=90% | <=20% | <5000U | 4/5 positive segments, multi-symbol, live reproducible |
| Aggressive | >=110% | <=30% | <5000U | 3/5 positive segments, multi-symbol, live reproducible |

Every promoted candidate must include:

- Full-period replay plus `h1_2023`, `h2_2023`, `2024`, `2025`, and `2026_ytd`.
- At least 5 traded symbols and at least 3 independent base assets.
- No traded symbol above 35% configured budget share.
- No traded symbol above 35% realized gross PnL contribution.
- Budget <=5000U. Higher-budget runs are diagnostic only.
- Live reproducibility evidence. If the candidate uses new backtest behavior without trading-engine parity, mark it `research_only=true`.

## 1. External Signals Used For Round 10

These sources motivate implementation shape only; they do not permit leaving martingale:

- 3Commas condition-based averaging orders: safety orders can require both a minimum deviation and indicator conditions, with modes from base order or last executed entry order.  
  https://help.3commas.io/en/articles/9663694-dca-bot-averaging-orders-by-technical-indicators
- 3Commas trailing TP / multiple TP: trailing is an exit mechanic and is typically applied to the final target.  
  https://help.3commas.io/en/articles/3108981-how-take-profit-works-smarttrade-and-dca-bots-trailing-feature-explained
- Freqtrade position adjustment: DCA/position adjustment is an explicit callback pattern but must be controlled for performance and risk.  
  https://www.freqtrade.io/en/stable/strategy-callbacks/
- Hummingbot executors: controller/executor separation supports the live allocator design; executors manage order workflows under controller decisions.  
  https://hummingbot.org/strategies/v2-strategies/executors/
- Hummingbot DCA executor: DCA is implemented as a discrete executor workflow.  
  https://hummingbot.org/strategies/v2-strategies/executors/dcaexecutor/
- Gainium DCA scaled mode: ATR/ADR/percentage spacing and custom orders motivate volatility-aware safety-order ladders.  
  https://gainium.io/help/dca-mode
- Gainium DCA walkthrough: step scale and volume scale are the core martingale parameters; total capital must be checked before running.  
  https://gainium.io/help/dca-bot-set-up-walkthrough
- Gainium minigrids DCA: minigrids between DCA steps motivate a DCA-grid hybrid while still using DCA safety orders as the core.  
  https://gainium.io/help/minigrids-dca
- Binance USD-M Futures exchange info and MIN_NOTIONAL: every live package must respect min-notional and filter constraints.  
  https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information

## 2. Task P0: Canonical Startup And Non-Repeat Lock

**Files:**

- Read: `docs/superpowers/artifacts/glm-martingale-core-round9/r1-r9-corrected-status.json`
- Read: `docs/superpowers/reports/2026-07-07-glm-round9-execution-audit-and-fix.md`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round10/exploration-registry.jsonl`
- Create: `docs/superpowers/reports/2026-07-07-glm-martingale-round10-search-ledger.md`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round10/run-manifests/r10-data-sha256.txt`

- [ ] **Step 1: Start branch from synced Round 9**

Run:

```bash
git fetch origin
git checkout glm-martingale-core-round9
git pull --ff-only
git checkout -b glm-martingale-core-round10
```

Expected: new branch `glm-martingale-core-round10` with no uncommitted files.

- [ ] **Step 2: Create evidence layout**

Run:

```bash
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round10/promising
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round10/rejected
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round10/run-manifests
printf '# GLM Martingale Round 10 Search Ledger\n\n' > docs/superpowers/reports/2026-07-07-glm-martingale-round10-search-ledger.md
touch docs/superpowers/artifacts/glm-martingale-core-round10/exploration-registry.jsonl
sha256sum data/market_data_full.db data/funding_rates.db data/premium_index.db > docs/superpowers/artifacts/glm-martingale-core-round10/run-manifests/r10-data-sha256.txt
```

- [ ] **Step 3: Copy canonical status into ledger**

Append this exact block to `docs/superpowers/reports/2026-07-07-glm-martingale-round10-search-ledger.md`:

```markdown
## Canonical Carry-In

- Source: `docs/superpowers/artifacts/glm-martingale-core-round9/r1-r9-corrected-status.json`
- Best research: R9 allocator ann 64.4196% / DD 18.2111% / 5/5, module-ready but not fully live-ready.
- Best fully live-ready: R4-combo ann 34.7233% / DD 17.6843% / 4/5.
- Target hits through Round 9: none.
- Do not repeat: Round8 leaky allocator timing, R9 random symbol substitution, reserve-only DD compression, stale handoff registry counts, module-only live_ready claims.
```

- [ ] **Step 4: Register and commit P0**

Append to `exploration-registry.jsonl`:

```json
{"id":"r10-P0-canonical-startup-001","status":"complete","family":"startup","source":"r1-r9-corrected-status","target_hit":false}
```

Run:

```bash
git add docs/superpowers/artifacts/glm-martingale-core-round10 docs/superpowers/reports/2026-07-07-glm-martingale-round10-search-ledger.md
git commit -m "docs: 修复思路 初始化Round10马丁校正台账"
```

## 3. Task P1: Wire Allocator Into Trading-Engine Main Loop

**Files:**

- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Modify: `apps/trading-engine/tests/martingale_runtime.rs`
- Create: `apps/trading-engine/tests/martingale_allocator_live_integration.rs`
- Read: `apps/backtest-engine/src/martingale/allocator_replay.rs`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round10/r10-live-allocator-mainloop-wiring.json`

- [ ] **Step 1: Add failing integration tests**

Create `apps/trading-engine/tests/martingale_allocator_live_integration.rs` with tests named:

```rust
#[test]
fn allocator_blocks_new_cycles_for_inactive_sleeves_in_main_reconcile() {
    // Build one portfolio with two sleeve IDs: R4 and ANKR.
    // Initialize allocator active_sleeve_id = "R4".
    // Feed a tick that would satisfy ANKR entry triggers.
    // Assert main reconcile does not call start_cycle for ANKR while R4 is active.
}

#[test]
fn allocator_switch_does_not_cancel_existing_cycle() {
    // Start an R4 cycle, then rebalance active sleeve to ANKR.
    // Assert the R4 cycle/order state remains present and no close order is created.
    // Assert only new R4 cycle opens are blocked after the switch.
}

#[test]
fn allocator_rebalance_uses_completed_equity_only_in_main_loop() {
    // Give ANKR a large gain only in the current interval.
    // Assert the rebalance decision at the interval boundary cannot use that gain
    // until the next interval, matching allocator_replay.rs forward-only invariant.
}
```

Run:

```bash
cargo test -p trading-engine allocator_blocks_new_cycles_for_inactive_sleeves_in_main_reconcile allocator_switch_does_not_cancel_existing_cycle allocator_rebalance_uses_completed_equity_only_in_main_loop
```

Expected before implementation: tests fail because `main.rs` does not own or consult `AllocatorState`.

- [ ] **Step 2: Add runtime state and config parsing**

Implement these concrete changes:

- In `apps/trading-engine/src/martingale_runtime.rs`, add optional allocator fields to `MartingaleRuntime`:
  - `allocator_state: Option<backtest_engine::martingale::allocator_replay::AllocatorState>`
  - `strategy_to_sleeve_id: HashMap<String, String>`
- Add public methods:
  - `set_allocator_state_for_test(state: AllocatorState, strategy_to_sleeve_id: HashMap<String, String>)`
  - `allocator_allows_new_cycle(strategy_id: &str, now_ms: i64) -> bool`
  - `rebalance_allocator(now_ms: i64, cfg: &AllocatorConfig, metrics: &HashMap<String, RollingMetrics>) -> Option<String>`
- In `apps/trading-engine/src/main.rs`, update `martingale_runtime_config_from_portfolio` parsing to read optional JSON keys:
  - `allocator_config`
  - `sleeve_id`
  - `strategy_to_sleeve_id`

- [ ] **Step 3: Wire allocator gate into the real new-cycle path**

In `reconcile_martingale_executor_strategies` and the martingale tick path before any `start_cycle_with_futures_preflight(...)` call:

```text
if runtime.allocator_allows_new_cycle(strategy_id, now_ms) == false:
  record a runtime event "martingale_allocator_blocked_new_cycle"
  skip opening the new cycle
else:
  continue existing entry trigger and preflight path
```

Rules:

- Allocator only blocks new cycles.
- Allocator never closes existing cycles.
- If `now_ms >= next_rebalance_ms`, compute rolling metrics from completed observations only, call `rebalance_allocator`, then apply the new active sleeve to future opens.

- [ ] **Step 4: Run P1 tests**

Run:

```bash
cargo test -p trading-engine allocator_blocks_new_cycles_for_inactive_sleeves_in_main_reconcile allocator_switch_does_not_cancel_existing_cycle allocator_rebalance_uses_completed_equity_only_in_main_loop
cargo test -p trading-engine r9_live_runtime_persists_allocator_active_sleeve_until_next_rebalance r9_allocator_state_default_persists_initial_sleeve
cargo test -p backtest-engine allocator_switch_uses_completed_interval_only allocator_respects_max_high_ann_and_min_low_dd_weights
```

Expected final: all listed tests pass.

- [ ] **Step 5: Write wiring evidence JSON**

Create `docs/superpowers/artifacts/glm-martingale-core-round10/r10-live-allocator-mainloop-wiring.json`:

```json
{
  "task": "P1",
  "full_live_ready_gap_from_round9": "closed",
  "main_rs_dispatch_wired": true,
  "allocator_blocks_new_cycles_only": true,
  "existing_cycles_force_closed": false,
  "forward_only_rebalance": true,
  "tests": [
    "allocator_blocks_new_cycles_for_inactive_sleeves_in_main_reconcile",
    "allocator_switch_does_not_cancel_existing_cycle",
    "allocator_rebalance_uses_completed_equity_only_in_main_loop",
    "r9_live_runtime_persists_allocator_active_sleeve_until_next_rebalance",
    "r9_allocator_state_default_persists_initial_sleeve"
  ]
}
```

- [ ] **Step 6: Commit P1**

Run:

```bash
git add apps/trading-engine/src/main.rs apps/trading-engine/src/martingale_runtime.rs apps/trading-engine/tests/martingale_runtime.rs apps/trading-engine/tests/martingale_allocator_live_integration.rs docs/superpowers/artifacts/glm-martingale-core-round10/r10-live-allocator-mainloop-wiring.json docs/superpowers/reports/2026-07-07-glm-martingale-round10-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round10/exploration-registry.jsonl
git commit -m "fix: 修复思路 Round10接入马丁allocator主循环"
```

## 4. Task P2: Exact R9 Winner Live/Backtest Parity Replay

**Files:**

- Create: `scripts/glm_r10_r9_allocator_live_parity_replay.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round10/r10-r9-winner-live-parity.json`
- Read: `docs/superpowers/artifacts/glm-martingale-core-round9/promising/r9-P5-winner.json`
- Read: `docs/superpowers/artifacts/glm-martingale-core-round9/r9-final-validation.json`

- [ ] **Step 1: Write parity replay script**

Create `scripts/glm_r10_r9_allocator_live_parity_replay.py` that:

- Loads the R9 winner config.
- Replays allocator decisions using completed observations only.
- Emits each rebalance decision with `timestamp_ms`, `active_sleeve_before`, `active_sleeve_after`, `metrics_cutoff_ms`, and `applies_from_ms`.
- Recomputes full and five-segment metrics.
- Fails if any decision uses data from the same interval it applies to.

- [ ] **Step 2: Run exact replay**

Run:

```bash
python3 scripts/glm_r10_r9_allocator_live_parity_replay.py \
  --config docs/superpowers/artifacts/glm-martingale-core-round9/promising/r9-P5-winner.json \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --budget 5000 \
  --out docs/superpowers/artifacts/glm-martingale-core-round10/r10-r9-winner-live-parity.json
```

Expected valid output:

```text
PASS forward_only_decisions
PASS full_metrics_tolerance ann_abs_diff<=0.2 dd_abs_diff<=0.2
PASS segment_metrics_present 5/5
```

- [ ] **Step 3: Promotion rule**

If P1 and P2 both pass, update the R9 allocator status only in Round10 artifacts:

```json
{
  "candidate": "r9-P5-winner",
  "round10_live_ready_after_wiring": true,
  "annualized_return_pct": 64.4196,
  "max_drawdown_pct": 18.2111,
  "target_hit": false
}
```

Do not edit Round9 historical metrics again.

- [ ] **Step 4: Commit P2**

Run:

```bash
git add scripts/glm_r10_r9_allocator_live_parity_replay.py docs/superpowers/artifacts/glm-martingale-core-round10/r10-r9-winner-live-parity.json docs/superpowers/reports/2026-07-07-glm-martingale-round10-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round10/exploration-registry.jsonl
git commit -m "test: 修复思路 Round10验证R9马丁allocator实盘回放一致性"
```

## 5. Task P3: Condition-Triggered Safety Order Ladders

**Files:**

- Create: `scripts/glm_r10_condition_triggered_so_ladder.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round10/r10-condition-triggered-so-ladder.json`
- Create promising configs under: `docs/superpowers/artifacts/glm-martingale-core-round10/promising/`

- [ ] **Step 1: Implement only martingale-native variants**

Use existing martingale cycle logic. Indicators may only decide whether a safety order is allowed after minimum deviation is reached.

Grid:

```text
bases: R4-combo, R7-ANKR-q, R9-r4-sleeve, R9-ankr-sleeve
min_price_calc_mode: from_base_order, from_last_executed_entry_order
min_deviation_bps: 80, 120, 180, 250
step_scale: 1.15, 1.35, 1.60
volume_scale: 1.05, 1.20, 1.35
max_legs: 4, 5, 6
condition_family:
  rsi_reclaim_35
  rsi_oversold_28_and_adx_falling
  atr_contracting_30pct
  bb_lower_reclaim_for_longs_upper_reject_for_shorts
  funding_abs_below_0p03_and_rsi_reclaim
batch_mode: one_eligible_so_per_signal, all_eligible_sos_capped_at_2
```

Minimum valid search: 2500 configs, full + five segments for each config.

- [ ] **Step 2: Run search**

Run:

```bash
python3 scripts/glm_r10_condition_triggered_so_ladder.py \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --premium-data data/premium_index.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round10/r10-condition-triggered-so-ladder.json
```

Promotion gates:

- Conservative candidate: ann >=50 and DD <=10.
- Balanced candidate: ann >=90 and DD <=20.
- Aggressive candidate: ann >=110 and DD <=30.
- Near-frontier candidate: ann >=65 and DD <=18, or ann >=75 and DD <=22, with 4/5 positive segments and no symbol PnL share above 35%.

- [ ] **Step 3: Failure recording**

If no target or near-frontier appears, append this non-repeat key:

```json
{"key":"r10-condition-triggered-safety-orders-no-target","reason":"indicator-gated safety orders did not improve ann/DD frontier after full 5-segment validation"}
```

- [ ] **Step 4: Commit P3**

Run:

```bash
git add scripts/glm_r10_condition_triggered_so_ladder.py docs/superpowers/artifacts/glm-martingale-core-round10/r10-condition-triggered-so-ladder.json docs/superpowers/artifacts/glm-martingale-core-round10/promising docs/superpowers/reports/2026-07-07-glm-martingale-round10-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round10/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round10回测条件触发马丁安全单"
```

## 6. Task P4: Multi-TP With Final Trailing Runner

**Files:**

- Modify if needed: `apps/backtest-engine/src/martingale/exit_rules.rs`
- Modify if needed: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Modify if needed: `apps/trading-engine/src/main.rs`
- Create: `scripts/glm_r10_multi_tp_trailing_runner.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round10/r10-multi-tp-trailing-runner.json`

- [ ] **Step 1: Add tests before new exit behavior**

If final-stage trailing is not already implemented, add tests:

```rust
#[test]
fn final_tp_stage_can_trail_without_closing_earlier_stages() {
    // First TP stages close fixed fractions.
    // Final stage activates trailing only after target is reached.
}

#[test]
fn trailing_runner_never_increases_loss_after_breakeven_activation() {
    // Once breakeven is active, trailing stop cannot be below breakeven for longs
    // or above breakeven for shorts.
}
```

Run:

```bash
cargo test -p backtest-engine final_tp_stage_can_trail_without_closing_earlier_stages trailing_runner_never_increases_loss_after_breakeven_activation
```

Expected before implementation: fail if the feature is missing.

- [ ] **Step 2: Search TP/trailing variants**

Grid:

```text
bases: R4-combo, R7-ANKR-q, R9-P5 component sleeves
tp_stage_sets:
  30/40/30 at 45/90/final_trailing_bps
  25/25/25/25 at 40/75/110/final_trailing_bps
  50/25/25 at 55/100/final_trailing_bps
final_trailing_activation_bps: 80, 120, 180, 250
final_trailing_deviation_bps: 25, 40, 65, 90
breakeven_after_stage: 1, 2
breakeven_buffer_bps: 5, 15, 30
time_limit_days: none, 3, 7, 14
```

Run:

```bash
python3 scripts/glm_r10_multi_tp_trailing_runner.py \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round10/r10-multi-tp-trailing-runner.json
```

Minimum valid search: 1200 configs, full + five segments for each config.

- [ ] **Step 3: Commit P4**

Run:

```bash
git add apps/backtest-engine/src/martingale/exit_rules.rs apps/backtest-engine/src/martingale/kline_engine.rs apps/trading-engine/src/main.rs scripts/glm_r10_multi_tp_trailing_runner.py docs/superpowers/artifacts/glm-martingale-core-round10/r10-multi-tp-trailing-runner.json docs/superpowers/reports/2026-07-07-glm-martingale-round10-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round10/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round10回测马丁多级止盈尾仓追踪"
```

## 7. Task P5: DCA Minigrid Hybrid Sleeves

**Files:**

- Create: `scripts/glm_r10_dca_minigrid_hybrid.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round10/r10-dca-minigrid-hybrid.json`
- Create promising configs under: `docs/superpowers/artifacts/glm-martingale-core-round10/promising/`

- [ ] **Step 1: Implement DCA-first minigrid rules**

Rules:

- A safety order fill opens a local minigrid inside the distance to the next safety order.
- The minigrid can only reduce inventory or realize partial profit; it cannot open standalone trend/grid exposure.
- All minigrid orders must pass exchange min-notional, using 5 USDT as the USD-M default from Binance docs.
- If minigrid logic requires order notional below 5 USDT, reject the config before replay.

Grid:

```text
bases: R4-combo, R7-ANKR-q, best P3 if promoted, best P4 if promoted
dca_step_bps: 180, 250, 350, 500
dca_step_scale: 1.1, 1.35, 1.6
dca_volume_scale: 1.0, 1.1, 1.25
minigrid_levels_per_band: 2, 3, 5
minigrid_spacing_bps: 20, 40, 70
minigrid_profit_take_bps: 20, 35, 55
max_active_minigrids: 1, 2, 3
```

Minimum valid search: 1500 configs, full + five segments for each config.

- [ ] **Step 2: Run search**

Run:

```bash
python3 scripts/glm_r10_dca_minigrid_hybrid.py \
  --budget 5000 \
  --exchange-min-notional 5 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round10/r10-dca-minigrid-hybrid.json
```

- [ ] **Step 3: Commit P5**

Run:

```bash
git add scripts/glm_r10_dca_minigrid_hybrid.py docs/superpowers/artifacts/glm-martingale-core-round10/r10-dca-minigrid-hybrid.json docs/superpowers/artifacts/glm-martingale-core-round10/promising docs/superpowers/reports/2026-07-07-glm-martingale-round10-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round10/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round10回测DCA小网格马丁组合"
```

## 8. Task P6: Regime-Defensive Martingale Allocator V2

**Files:**

- Create: `scripts/glm_r10_regime_defensive_allocator_v2.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round10/r10-regime-defensive-allocator-v2.json`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round10/r10-leave-one-segment-out-validation.json`

- [ ] **Step 1: Freeze features before fitting**

Allowed lagged features:

```text
rolling_7d_return
rolling_30d_return
rolling_30d_realized_vol
rolling_30d_max_dd
rolling_30d_funding_drag_pct_of_budget
btc_ema50_ema200_distance
portfolio_chop_score = realized_vol / abs(rolling_return)
symbol_breadth_above_ema50
```

Forbidden:

```text
current_interval_return
future segment label
full-period rank
same-segment optimized threshold without leave-one-segment-out validation
```

- [ ] **Step 2: Search allocator rules**

Allocator can select only among martingale sleeves or cash.

Grid:

```text
lookback_days: 30, 60, 90, 120
rebalance_days: 3, 7, 14
score: calmar_like, ann_minus_2dd, ann_minus_cost, 2025_defensive_score
cash_trigger:
  none
  rolling_dd_gt_6
  chop_score_gt_4_and_return_lt_0
  funding_drag_30d_gt_1pct_budget
defensive_sleeve_floor: 0.0, 0.25, 0.50
switch_hysteresis_score_gap: 0, 2, 5
```

Run:

```bash
python3 scripts/glm_r10_regime_defensive_allocator_v2.py \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --premium-data data/premium_index.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round10/r10-regime-defensive-allocator-v2.json \
  --loso-out docs/superpowers/artifacts/glm-martingale-core-round10/r10-leave-one-segment-out-validation.json
```

Minimum valid search: 2000 configs, full + five true segment allocator replays.

- [ ] **Step 3: Leave-one-segment-out gate**

For any candidate passing a target in full-period:

- Refit/select thresholds on four segments.
- Test on the held-out segment.
- Repeat for all five held-out segments.
- Reject if any held-out result has DD above the tier gate by more than 20% relative or ann below zero.

- [ ] **Step 4: Commit P6**

Run:

```bash
git add scripts/glm_r10_regime_defensive_allocator_v2.py docs/superpowers/artifacts/glm-martingale-core-round10/r10-regime-defensive-allocator-v2.json docs/superpowers/artifacts/glm-martingale-core-round10/r10-leave-one-segment-out-validation.json docs/superpowers/reports/2026-07-07-glm-martingale-round10-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round10/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round10回测防御型马丁分配器"
```

## 9. Task P7: Final Validation And Handoff

**Files:**

- Create: `docs/superpowers/artifacts/glm-martingale-core-round10/r10-final-validation.json`
- Create: `docs/superpowers/reports/2026-07-07-glm-round10-handoff-to-chatgpt.md`

- [ ] **Step 1: Build final JSON**

Create `r10-final-validation.json` with:

```json
{
  "round": 10,
  "target_hits": {"conservative": [], "balanced": [], "aggressive": []},
  "best_research": [],
  "best_fully_live_ready": [],
  "new_failed_families": [],
  "do_not_repeat_new_keys": [],
  "live_allocator_wiring": {},
  "source_of_truth": "docs/superpowers/artifacts/glm-martingale-core-round9/r1-r9-corrected-status.json"
}
```

- [ ] **Step 2: Required final checks**

Run:

```bash
jq '.target_hits, .best_research, .best_fully_live_ready' docs/superpowers/artifacts/glm-martingale-core-round10/r10-final-validation.json
rg -n "T[B]D|T[O]DO|P[L]ACEHOLDER|待[补]|后续[补]" docs/superpowers/reports/2026-07-07-glm-round10-handoff-to-chatgpt.md docs/superpowers/artifacts/glm-martingale-core-round10/r10-final-validation.json
git diff --check
cargo test -p backtest-engine
cargo test -p trading-engine
git status --short --branch
```

If `cargo test -p trading-engine` fails only because the sandbox cannot bind a local test server, rerun outside the sandbox and record the exact reason.

- [ ] **Step 3: Handoff requirements**

The handoff must include:

- Exact branch and head commit.
- Exact candidate counts and replay counts per task.
- Every failure family and its non-repeat key.
- Whether R9 allocator became fully live-ready after P1/P2.
- Full + five segment table for every promoted candidate.
- Symbol list, budget share, realized PnL share, max capital used, blocked orders, funding, fees, slippage.
- Clear target verdict for conservative, balanced, and aggressive.

- [ ] **Step 4: Commit and push**

Run:

```bash
git add docs/superpowers/artifacts/glm-martingale-core-round10/r10-final-validation.json docs/superpowers/reports/2026-07-07-glm-round10-handoff-to-chatgpt.md docs/superpowers/reports/2026-07-07-glm-martingale-round10-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round10/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round10最终验证马丁组合目标"
git push -u origin glm-martingale-core-round10
```

## 10. Performance Rules

Do not start with GPU work. First make CPU replay cheaper:

- Cache sleeve equity curves once, then run allocator searches on cached curves.
- Hash config JSON and skip duplicate metric tuples.
- Reuse segment slices instead of replaying the same sleeve repeatedly.
- Use process-level parallelism for independent configs.
- Only consider GPU for frozen feature-matrix experiments in P6. GPU results are diagnostic until exact replay passes through the Rust/Python backtest path.

## 11. Stop Conditions

Stop immediately and hand off if any candidate satisfies a full tier:

- Conservative: ann >=50, DD <=10, budget <5000U, 4/5 positive segments, multi-symbol, live reproducible.
- Balanced: ann >=90, DD <=20, budget <5000U, 4/5 positive segments, multi-symbol, live reproducible.
- Aggressive: ann >=110, DD <=30, budget <5000U, 3/5 positive segments, multi-symbol, live reproducible.

If no target hit appears, hand off the corrected live-ready status, all near-frontier candidates, and every failed family so Round 11 does not repeat them.
