# GLM Martingale Core Round 9 Allocator Repair And Sleeve Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair Round 8 allocator evidence, implement live-reproducible portfolio allocation, and search broader multi-symbol martingale sleeve combinations under the unchanged 5000U budget.

**Architecture:** Round 9 keeps martingale/DCA/grid as the only trading core. The allocator may choose among martingale portfolios or cash using lagged data only. Indicators may control entry, safety-order eligibility, exit, quarantine, sleeve selection, and capital reserve; they may not create pure trend, breakout, funding-carry, or statistical-arbitrage trades.

**Tech Stack:** Rust backtest engine and trading engine, Python research scripts only for discovery, `portfolio_budget_replay`, SQLite market/funding/premium data, JSON/JSONL artifacts under `docs/superpowers/artifacts/glm-martingale-core-round9/`.

---

## 0. Hard Constraints

Targets remain unchanged:

| Tier | Ann | DD | Budget | Other gates |
|---|---:|---:|---:|---|
| Conservative | >=50% | <=10% | <5000U | 4/5 positive segments, live reproducible |
| Balanced | >=90% | <=20% | <5000U | 4/5 positive segments, live reproducible |
| Aggressive | >=110% | <=30% | <5000U | 4/5 positive segments, live reproducible |

Portfolio gates:

- At least 5 traded symbols.
- At least 3 independent base assets after removing indicator-only dependencies.
- No traded symbol above 35% configured budget share.
- No traded symbol above 35% realized gross PnL contribution for a promoted package.
- Single-symbol results are diagnostic only and cannot be target hits.
- Budget above 5000U is diagnostic only and cannot count toward a target.

Evidence gates:

- Full-period replay plus `h1_2023`, `h2_2023`, `2024`, `2025`, and `2026_ytd`.
- Allocator decisions must be computed from data strictly before the interval they affect.
- Every result row must include actual traded symbols, indicator-only dependencies, budget share, PnL share, segment metrics, live-ready flag, and non-repeat key.

## 1. External Signals Used For New Directions

These references motivate implementation shape only; they do not allow leaving martingale:

- 3Commas DCA docs: base order, safety orders, max active deals, cooldown and TP controls map to martingale cycle controls.
- 3Commas trailing TP docs: trailing is an exit control, not a standalone signal.
- Freqtrade DCA/position-adjustment docs: position adjustment and custom stake sizing motivate wallet reserve and late-safety sizing, while still being DCA-native.
- Hummingbot DCA executor/controller docs: controller/executor separation motivates a live allocator that selects among martingale sleeves.
- Gainium DCA docs: safety-order count/volume/step scaling and indicator conditions motivate bounded DCA parameter families.
- Binance Futures exchange info/min-notional rules: live packages must prove min-notional and cap-truncation behavior.

Source links:

- https://help.3commas.io/en/articles/3108940-dca-bot-interface-and-main-settings
- https://help.3commas.io/en/articles/3108981-how-take-profit-works-smarttrade-and-dca-bots-trailing-feature-explained
- https://www.freqtrade.io/en/stable/strategy-callbacks/
- https://hummingbot.org/v2-strategies/executors/dcaexecutor/
- https://gainium.io/help/dca
- https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information

## 2. Branch And Evidence Startup

- [ ] **Step 1: Start from current Round 8**

Run:

```bash
git fetch origin
git checkout glm-martingale-core-round8
git pull --ff-only
git checkout -b glm-martingale-core-round9
```

If `git pull --ff-only` reports local commits that are not on origin, push or reconcile them before creating Round 9. Do not start Round 9 on an unsynced branch.

- [ ] **Step 2: Create evidence layout**

Run:

```bash
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round9/promising
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round9/rejected
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round9/run-manifests
printf '# GLM Martingale Round 9 Search Ledger\n\n' > docs/superpowers/reports/2026-07-07-glm-martingale-round9-search-ledger.md
touch docs/superpowers/artifacts/glm-martingale-core-round9/exploration-registry.jsonl
sha256sum data/market_data_full.db data/funding_rates.db data/premium_index.db > docs/superpowers/artifacts/glm-martingale-core-round9/run-manifests/r9-data-sha256.txt
```

- [ ] **Step 3: Commit startup**

Run:

```bash
git add docs/superpowers/artifacts/glm-martingale-core-round9 docs/superpowers/reports/2026-07-07-glm-martingale-round9-search-ledger.md
git commit -m "docs: 修复思路 初始化Round9马丁分配器修复与组合扩展台账"
git push -u origin glm-martingale-core-round9
```

Each exploration must append start and completion records to `exploration-registry.jsonl`.

## 3. Task P0: Round 8 Evidence Repair Registry

**Files:**

- Read: `docs/superpowers/reports/2026-07-07-glm-round8-execution-audit-and-recheck.md`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round9/r9-round8-correction.json`
- Modify: `docs/superpowers/reports/2026-07-07-glm-martingale-round9-search-ledger.md`

- [ ] **Step 1: Create correction JSON**

Write:

```json
{
  "round8_corrected_status": {
    "best_research_allocator_leaky": {"ann": 62.0, "dd": 18.2, "live_ready": false},
    "best_research_allocator_no_current_interval_leak": {"ann": 60.4, "dd": 18.2, "live_ready": false},
    "best_live_ready": {"label": "R4-combo", "ann": 34.72332738972233, "dd": 17.68431043895811, "live_ready": true}
  },
  "round8_invalid_claims": [
    "P2 was not truly 5-segment validated per candidate",
    "P2 max_high_ann_weight and min_low_dd_weight were not used by the allocator",
    "P2 traded_symbols contained sleeve names rather than exchange symbols",
    "P1 tests were mostly config smoke tests and did not prove live parity",
    "P3 and P4 ran narrower grids than planned"
  ],
  "target_status": {
    "conservative": "not_met",
    "balanced": "not_met",
    "aggressive": "not_met"
  }
}
```

- [ ] **Step 2: Commit P0**

Run:

```bash
git add docs/superpowers/artifacts/glm-martingale-core-round9/r9-round8-correction.json docs/superpowers/reports/2026-07-07-glm-martingale-round9-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round9/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round9记录Round8审计修正结论"
```

## 4. Task P1: Repair Allocator Semantics And Segment Validation

**Files:**

- Modify: `scripts/glm_r8_regime_rescue_allocator.py`
- Create: `scripts/glm_r9_validate_allocator_semantics.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round9/r9-allocator-repair-grid.json`

- [ ] **Step 1: Add allocator semantic checks**

Create `scripts/glm_r9_validate_allocator_semantics.py` with synthetic equity curves that prove:

```text
case_no_current_interval_leak:
  A has a large gain only in the current interval.
  Correct allocator cannot switch to A until the next interval.

case_weight_params_bind:
  max_high_ann_weight and min_low_dd_weight must change output allocation.
  If output metrics are identical across all weight settings, mark parameter as inactive.

case_segment_metrics_present:
  Every output row must include full_metrics, segment_metrics, positive_segments, traded_symbols, symbol_count, max_symbol_budget_pct, max_symbol_gross_pnl_share_pct, portfolio_candidate.
```

Run:

```bash
python3 scripts/glm_r9_validate_allocator_semantics.py
```

Expected output:

```text
PASS case_no_current_interval_leak
PASS case_weight_params_bind
PASS case_segment_metrics_present
```

- [ ] **Step 2: Repair allocator script**

Modify `scripts/glm_r8_regime_rescue_allocator.py` so that:

- Rebalance selection at timestamp `ts` only affects intervals after `ts`.
- `max_high_ann_weight` caps the high-ann sleeve weight.
- `min_low_dd_weight` forces low-DD sleeve allocation or removes those parameters from the grid.
- `traded_symbols` contains actual symbols from underlying sleeve configs.
- Segment metrics are computed for each allocator candidate over all five segments.
- Candidate rows contain `target_profile_hit` only after segment and portfolio gates pass.

- [ ] **Step 3: Run repaired allocator grid**

Run:

```bash
python3 scripts/glm_r8_regime_rescue_allocator.py \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round9/r9-allocator-repair-grid.json
```

Minimum valid output:

- At least 864 non-duplicate allocator rows.
- Every row has full and five segment metrics.
- Every promoted row has `portfolio_candidate=true`, `symbol_count>=5`, and `live_ready=false` unless trading-engine allocator exists.

- [ ] **Step 4: Commit P1**

Run:

```bash
git add scripts/glm_r8_regime_rescue_allocator.py scripts/glm_r9_validate_allocator_semantics.py docs/superpowers/artifacts/glm-martingale-core-round9/r9-allocator-repair-grid.json docs/superpowers/reports/2026-07-07-glm-martingale-round9-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round9/exploration-registry.jsonl
git commit -m "fix: 修复思路 Round9修正马丁分配器时序与分段验证"
```

## 5. Task P2: Implement Live-Reproducible Portfolio Allocator

**Files:**

- Create or modify: `apps/backtest-engine/src/martingale/allocator_replay.rs`
- Modify: `apps/backtest-engine/src/lib.rs`
- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Modify: `apps/trading-engine/tests/martingale_runtime.rs`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round9/r9-live-allocator-parity.json`

- [ ] **Step 1: Add failing tests first**

Add tests named:

```rust
#[test]
fn allocator_switch_uses_completed_interval_only() {
    // Build two synthetic sleeve curves. A outperforms only in the interval ending at ts.
    // Assert allocation remains on previous sleeve for that interval and switches only after ts.
}

#[test]
fn allocator_respects_max_high_ann_and_min_low_dd_weights() {
    // Build high-ann and low-DD synthetic sleeves.
    // Assert configured caps change output allocation weights.
}

#[test]
fn live_runtime_persists_allocator_active_sleeve_until_next_rebalance() {
    // Start active sleeve R4, feed a rebalance event, and assert live runtime schedules
    // the next sleeve without rewriting already-open martingale cycles.
}
```

Run:

```bash
cargo test -p backtest-engine allocator_switch_uses_completed_interval_only
cargo test -p trading-engine live_runtime_persists_allocator_active_sleeve_until_next_rebalance
```

Expected before implementation: fail because allocator replay/runtime hooks do not exist.

- [ ] **Step 2: Implement allocator replay and runtime state**

Implement:

```text
AllocatorConfig:
  lookback_days
  rebalance_days
  score_function
  max_high_ann_weight
  min_low_dd_weight
  cash_trigger_rolling_dd_pct
  switch_hysteresis_score_gap
  sleeves[]

AllocatorState:
  active_sleeve_id
  next_rebalance_ms
  last_completed_interval_ms
  rolling_metrics_by_sleeve
```

Runtime rule:

- Existing martingale cycles are not force-closed on sleeve switch.
- New cycles open only for active sleeve strategies.
- Rebalance uses completed equity observations only.

- [ ] **Step 3: Run parity tests**

Run:

```bash
cargo test -p backtest-engine
cargo test -p trading-engine
```

Expected final: both commands pass. If trading-engine hits the local bind sandbox error, rerun outside sandbox and record the exact reason in the ledger.

- [ ] **Step 4: Commit P2**

Run:

```bash
git add apps/backtest-engine/src/martingale/allocator_replay.rs apps/backtest-engine/src/lib.rs apps/trading-engine/src/martingale_runtime.rs apps/trading-engine/tests/martingale_runtime.rs docs/superpowers/artifacts/glm-martingale-core-round9/r9-live-allocator-parity.json docs/superpowers/reports/2026-07-07-glm-martingale-round9-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round9/exploration-registry.jsonl
git commit -m "feat: 修复思路 Round9实现马丁组合分配器live parity"
```

## 6. Task P3: Expand Multi-Symbol Sleeve Library

**Files:**

- Create: `scripts/glm_r9_multi_symbol_sleeve_search.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round9/r9-multi-symbol-sleeve-library.json`
- Create configs under: `docs/superpowers/artifacts/glm-martingale-core-round9/promising/`

- [ ] **Step 1: Build diagnostic symbol universe**

Use local market data only. Score symbols by:

```text
coverage from 2023-01-01 to 2026-06-01
median quote volume rank
2025 standalone DCA diagnostic return
2025 standalone DCA diagnostic DD
correlation to existing sleeves
funding drag as percentage of 5000U budget
```

Standalone diagnostics cannot be target hits. They only feed multi-symbol sleeve construction.

- [ ] **Step 2: Construct portfolio sleeves**

Generate only portfolios satisfying:

```text
symbols_per_sleeve: 6, 8, 10
long_count: 3, 4, 5
short_count: 3, 4, 5
max_symbol_budget_pct: 20, 25, 30
max_pairwise_corr_allowed: 0.70, 0.80
include_existing_anchor: none, R4, ANKR-q, QB
```

Minimum valid search size: 1500 multi-symbol portfolios. Every portfolio must run full + five segments.

- [ ] **Step 3: Run search**

Run:

```bash
python3 scripts/glm_r9_multi_symbol_sleeve_search.py \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round9/r9-multi-symbol-sleeve-library.json
```

Promotion gates:

- Full ann improves by at least 5pp over R4-combo at DD <=20, or DD improves by at least 3pp over R4-combo with ann >=30.
- 2025 segment return is above -5% or 2025 DD below 15%.
- No single symbol PnL contribution above 35%.

- [ ] **Step 4: Commit P3**

Run:

```bash
git add scripts/glm_r9_multi_symbol_sleeve_search.py docs/superpowers/artifacts/glm-martingale-core-round9/r9-multi-symbol-sleeve-library.json docs/superpowers/artifacts/glm-martingale-core-round9/promising docs/superpowers/reports/2026-07-07-glm-martingale-round9-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round9/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round9完成多币种马丁sleeve库扩展搜索"
```

## 7. Task P4: DCA Reserve And Dynamic Stake Buffer

This direction is inspired by DCA position adjustment and custom stake reserve. It remains martingale-native because it changes base/safety order sizing and reserve only.

**Files:**

- Create: `scripts/glm_r9_dca_reserve_stake_buffer.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round9/r9-dca-reserve-stake-buffer.json`

- [ ] **Step 1: Search reserve rules**

Use:

```text
bases: R4-combo, R6-QB, R7-ANKR-q, best sleeves from P3
reserve_pct: 10, 20, 30, 40
first_order_scale_when_atr_gt_2pct: 0.5, 0.7, 1.0
late_safety_reserve_release_after_leg: 2, 3, 4
release_condition: rsi_reclaim_35, adx_falling_below_25, volatility_contracting_30pct
max_active_symbols: 3, 4, 5
```

Minimum valid search size: 720 configs. Every config must preserve at least 5 traded symbols and run full + five segments.

- [ ] **Step 2: Run reserve search**

Run:

```bash
python3 scripts/glm_r9_dca_reserve_stake_buffer.py \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round9/r9-dca-reserve-stake-buffer.json
```

Reject if DD improves by less than 3pp while ann falls by more than 5pp.

- [ ] **Step 3: Commit P4**

Run:

```bash
git add scripts/glm_r9_dca_reserve_stake_buffer.py docs/superpowers/artifacts/glm-martingale-core-round9/r9-dca-reserve-stake-buffer.json docs/superpowers/reports/2026-07-07-glm-martingale-round9-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round9/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round9完成马丁DCA资金reserve与动态stake搜索"
```

## 8. Task P5: Combine Repaired Allocator With Expanded Sleeves

**Files:**

- Create: `scripts/glm_r9_allocator_with_expanded_sleeves.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round9/r9-expanded-allocator-grid.json`
- Create configs under: `docs/superpowers/artifacts/glm-martingale-core-round9/promising/`

- [ ] **Step 1: Use repaired allocator only**

Input sleeves:

```text
R4-combo
R6-QB
R7-ANKR-q
R5-fine
all promoted P3 sleeves
all promoted P4 reserve variants
```

Allocator grid:

```text
lookback_days: 30, 60, 90, 120
rebalance_days: 7, 14, 30
score: ann_minus_1dd, ann_minus_2dd, ann_minus_cost, calmar_like
max_high_ann_weight: 0.20, 0.30, 0.40
min_low_dd_weight: 0.20, 0.35, 0.50
cash_trigger_rolling_dd_pct: none, 8, 12
switch_hysteresis_score_gap: 0, 3, 6
```

Every candidate must run full + five true segment allocator replays.

- [ ] **Step 2: Run expanded allocator**

Run:

```bash
python3 scripts/glm_r9_allocator_with_expanded_sleeves.py \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round9/r9-expanded-allocator-grid.json
```

Stop early only if a full target hit appears and then immediately run 20 nearby robustness configs plus leave-one-segment-out validation.

- [ ] **Step 3: Commit P5**

Run:

```bash
git add scripts/glm_r9_allocator_with_expanded_sleeves.py docs/superpowers/artifacts/glm-martingale-core-round9/r9-expanded-allocator-grid.json docs/superpowers/artifacts/glm-martingale-core-round9/promising docs/superpowers/reports/2026-07-07-glm-martingale-round9-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round9/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round9完成扩展sleeve马丁分配器搜索"
```

## 9. Task P6: Final Validation And Handoff

**Files:**

- Create: `docs/superpowers/artifacts/glm-martingale-core-round9/r9-final-validation.json`
- Create: `docs/superpowers/reports/2026-07-07-glm-round9-handoff-to-chatgpt.md`

- [ ] **Step 1: Validate targets**

Final JSON shape:

```json
{
  "target_hits": {
    "conservative": [],
    "balanced": [],
    "aggressive": []
  },
  "best_research": [],
  "best_live_ready": [],
  "failed_families": [],
  "do_not_repeat_new_keys": [],
  "round8_repairs_closed": []
}
```

- [ ] **Step 2: Handoff content**

The handoff must include:

- Exact branch and commit hash.
- Candidate counts per task.
- Actual tests run and pass/fail counts.
- Corrected allocator timing result.
- Full + five segment table for every promoted candidate.
- Live-ready status for every candidate.
- Symbol list, symbol count, max symbol budget share, max symbol gross PnL share.
- Explicit target verdict for conservative, balanced, and aggressive.

- [ ] **Step 3: Verification**

Run:

```bash
rg -n "T[B]D|T[O]DO|P[L]ACEHOLDER|待[补]|后续[补]" docs/superpowers/reports/2026-07-07-glm-round9-handoff-to-chatgpt.md docs/superpowers/plans/2026-07-07-glm-martingale-core-round9-allocator-repair-and-sleeve-expansion-plan.md
git diff --check
cargo test -p backtest-engine
cargo test -p trading-engine
git status --short --branch
```

Commit and push:

```bash
git add docs/superpowers/artifacts/glm-martingale-core-round9/r9-final-validation.json docs/superpowers/reports/2026-07-07-glm-round9-handoff-to-chatgpt.md
git commit -m "docs: 修复思路 Round9最终验证马丁组合目标"
git push
```

## 10. Stop Conditions

Stop the round and hand off immediately if any candidate satisfies all gates:

- Conservative: ann >=50, DD <=10, budget <5000U, 4/5 positive segments, multi-symbol portfolio gate, live reproducible.
- Balanced: ann >=90, DD <=20, budget <5000U, 4/5 positive segments, multi-symbol portfolio gate, live reproducible.
- Aggressive: ann >=110, DD <=30, budget <5000U, 4/5 positive segments, multi-symbol portfolio gate, live reproducible.

If no target hit appears, hand off the repaired frontier and all failed families. Do not claim architecture impossibility unless P1-P5 are complete and independently verified.
