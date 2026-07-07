# GLM Martingale Core Round 8 Live-Parity And Regime Rescue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the Round 7 frontier into a reproducible martingale-only research pipeline, close live-parity gaps, and search only non-repeated mechanisms that may reach conservative/balanced/aggressive targets under a 5000U budget.

**Architecture:** Round 8 remains martingale/DCA/grid-core. Indicators and external signals may only control cycle entry, safety-order eligibility/scale, TP/exit, quarantine, sleeve allocation, cooldown, or cash risk budget. The round is split into validation repair, live parity, 2025 regime rescue, dynamic grid reset inside DCA, and final anti-overfit packaging.

**Tech Stack:** Rust backtest engine and trading engine, Python search scripts, `portfolio_budget_replay`, SQLite market/funding/premium data, JSON/JSONL artifacts under `docs/superpowers/artifacts/glm-martingale-core-round8/`.

---

## 0. Non-Negotiable Boundary

Allowed:

- Martingale cycle entry gates.
- Safety-order gates, scale, freeze, taper, cancel, reset, and cooldown.
- Partial TP, breakeven, trailing lock, cost-cover exit, and cycle close rules.
- Symbol/direction quarantine.
- Funding/fee/slippage gates for martingale cycles.
- Lagged allocation across martingale sleeves and cash.

Forbidden:

- Pure trend following.
- Pure breakout.
- Pure funding carry.
- Pure pair-neutral.
- Pure statistical arbitrage.
- Any primary position that is not opened and managed as a martingale/DCA/grid cycle.

Original targets remain unchanged:

| Profile | Required ann | Required DD | Required capital |
|---|---:|---:|---:|
| Conservative | >=50% | <=10% | <5000U |
| Balanced | >=90% | <=20% | <5000U |
| Aggressive | >=110% | <=30% | <5000U |

Additional anti-overfit gates:

- Full-period replay plus five segment replays: `h1_2023`, `h2_2023`, `2024`, `2025`, `2026_ytd`.
- At least 4/5 positive segments.
- Conservative candidate: no segment DD above 15%.
- Balanced candidate: no segment DD above 25%.
- Aggressive candidate: no segment DD above 35%.
- All allocation decisions must use lagged data only.
- Candidate is "live-ready" only if trading-engine parity tests cover every feature it uses.

## 1. External Search Signals To Use

Use these only as mechanism inspiration, not as permission to leave martingale:

1. 3Commas DCA docs describe base orders, safety orders, max active deals, take-profit-from-average/base choices, and trailing TP. Map these to martingale cycle controls only.
2. Binance funding docs define funding as a periodic transfer between long and short perpetual holders. Use it as a cost gate or side-quality veto, not as an independent return source.
3. Dynamic Grid Trading research argues static grids need dynamic adaptation to improve practical behavior. Use dynamic spacing/reset only inside DCA cycle rules, never as standalone grid trading.

Sources:

- https://help.3commas.io/en/articles/3108940-dca-bot-interface-and-main-settings
- https://help.3commas.io/en/articles/3108981-how-take-profit-works-smarttrade-and-dca-bots-trailing-feature-explained
- https://www.binance.com/en/support/faq/detail/360033525031
- https://www.binance.com/en/academy/glossary/funding-fees
- https://arxiv.org/abs/2506.11921

## 2. Branch, Evidence, And Registry Startup

- [ ] **Step 1: Start from Round 7 remote**

Run:

```bash
git fetch origin
git checkout glm-martingale-core-round7
git pull --ff-only
git checkout -b glm-martingale-core-round8
```

- [ ] **Step 2: Create evidence layout**

Run:

```bash
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round8/promising
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round8/rejected
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round8/run-manifests
printf '# GLM Martingale Round 8 Search Ledger\n\n' > docs/superpowers/reports/2026-07-07-glm-martingale-round8-search-ledger.md
touch docs/superpowers/artifacts/glm-martingale-core-round8/exploration-registry.jsonl
sha256sum data/market_data_full.db data/funding_rates.db data/premium_index.db > docs/superpowers/artifacts/glm-martingale-core-round8/run-manifests/r8-data-sha256.txt
```

- [ ] **Step 3: Commit startup**

Run:

```bash
git add docs/superpowers/artifacts/glm-martingale-core-round8 docs/superpowers/reports/2026-07-07-glm-martingale-round8-search-ledger.md
git commit -m "docs: 修复思路 初始化Round8马丁live-parity与regime-rescue台账"
git push -u origin glm-martingale-core-round8
```

Each exploration must append a start line and a completion line to `exploration-registry.jsonl`:

```json
{"exploration_id":"r8-P2-regime-rescue-001","started_at_utc":"2026-07-07T00:00:00Z","family":"regime_rescue_martingale_allocator","hypothesis":"Lagged DD/regime state can rotate among martingale sleeves and cash to repair 2025 without leaving DCA","new_vs_prior":"Round 7 dynamic blend used short-term performance only and did not enforce segment DD caps","martingale_core":true,"research_only":false,"input_signature":"base=R7-ANKR-q+R6-QB+R4-combo|budget=5000|segments=full+5","status":"running","result_path":"docs/superpowers/artifacts/glm-martingale-core-round8/r8-regime-rescue-grid.json"}
```

```json
{"exploration_id":"r8-P2-regime-rescue-001","completed_at_utc":"2026-07-07T00:00:00Z","status":"completed","decision":"rejected","non_repeat_key":"r8-lagged-dd-regime-rescue-v1","result":"0 target hits; best ann/DD/segments recorded in result artifact"}
```

## 3. Task P0: Repair Round 7 Evidence Before New Search

**Files:**

- Read: `docs/superpowers/reports/2026-07-07-glm-round7-execution-audit-and-recheck.md`
- Modify: `docs/superpowers/reports/2026-07-07-glm-martingale-round8-search-ledger.md`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round8/r8-starting-frontier.json`

- [ ] **Step 1: Create starting frontier JSON**

Write exact starting candidates:

```json
{
  "generated_from_audit": "docs/superpowers/reports/2026-07-07-glm-round7-execution-audit-and-recheck.md",
  "targets": {
    "conservative": {"ann": 50.0, "dd": 10.0},
    "balanced": {"ann": 90.0, "dd": 20.0},
    "aggressive": {"ann": 110.0, "dd": 30.0}
  },
  "frontier": [
    {"label": "R7-ANKR-q1w24p24", "config": "docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json", "ann": 63.51046213512199, "dd": 28.197233976815543, "pos_segments": 4, "live_ready": false},
    {"label": "R7-dynamic-blend", "config": "docs/superpowers/artifacts/glm-martingale-core-round7/r7-dynamic-cost-dd-blend.json", "ann": 53.93140498030886, "dd": 24.31334338215717, "pos_segments": null, "live_ready": false},
    {"label": "R6-QB-lowdd", "config": "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json", "ann": 34.0, "dd": 18.0, "pos_segments": 4, "live_ready": false},
    {"label": "R4-combo", "config": "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json", "ann": 34.7, "dd": 17.7, "pos_segments": 4, "live_ready": true}
  ],
  "open_blockers": [
    "No target pass through Round 7",
    "R5-R7 trading-engine live parity incomplete",
    "R7 best segment DD exceeds 33% in 2024 and 2025",
    "R7 best relies on cap-truncated static legs under 5000U budget"
  ]
}
```

- [ ] **Step 2: Commit P0 evidence**

Run:

```bash
git add docs/superpowers/artifacts/glm-martingale-core-round8/r8-starting-frontier.json docs/superpowers/reports/2026-07-07-glm-martingale-round8-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round8/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round8记录Round7复核前沿与未闭环缺口"
```

## 4. Task P1: Live-Parity And Semantic Tests Gate

Do not promote any Round 8 target hit until this task passes for the features it uses.

**Files:**

- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Modify: `apps/trading-engine/tests/martingale_runtime.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Test: `cargo test -p backtest-engine`
- Test: `cargo test -p trading-engine`

- [ ] **Step 1: Add missing backtest semantic tests**

Add tests with these exact names under `apps/backtest-engine/src/martingale/kline_engine.rs` test module:

```rust
#[test]
fn funding_cost_gate_blocks_long_when_expected_cost_exceeds_threshold() {
    // Build a one-symbol futures portfolio with max_expected_funding_cost_bps=1.0
    // and a positive funding point above the threshold. Assert no new cycle opens.
}

#[test]
fn funding_cost_gate_allows_short_when_positive_funding_is_receivable() {
    // Same data as above, short direction. Assert the short cycle can open because
    // positive funding is not an expected cost for the short side.
}

#[test]
fn taper_safety_scale_reduces_late_safety_order_margin() {
    // Configure taper_safety_after_leg=2 and taper_safety_scale=0.5.
    // Fill legs through the third safety order and assert the later planned margin
    // is half of the untapered multiplier margin.
}
```

Expected initial result before implementation fixes if behavior is missing: at least one of the tests fails. Expected final result: all three pass.

- [ ] **Step 2: Add trading-engine parity tests**

Add tests with these exact names under `apps/trading-engine/tests/martingale_runtime.rs`:

```rust
#[test]
fn live_runtime_uses_last_executed_order_for_safety_basis() {
    // Start a long cycle at 100, fill a safety order at 98, then assert the next
    // safety order anchor is based on 98 instead of the original 100.
}

#[test]
fn live_runtime_applies_vol_target_scale_to_planned_orders() {
    // Feed ATR context that implies 0.5 scale and assert first/safety order
    // notional uses the same scale as backtest.
}

#[test]
fn live_runtime_blocks_new_cycle_during_quarantine_pause() {
    // Record one stop inside a 24h window with trigger=1 and pause=24h.
    // Assert start_cycle returns blocked until pause expiry.
}

#[test]
fn live_runtime_tapers_safety_orders_after_configured_leg() {
    // Configure taper_safety_after_leg=2 and taper_safety_scale=0.5.
    // Assert orders after that leg use tapered quote.
}
```

If funding cost gate is used by any promoted candidate, add:

```rust
#[test]
fn live_runtime_blocks_entry_when_funding_cost_gate_fails() {
    // Provide current funding context and assert the live runtime blocks entry
    // when expected cost bps is above max_expected_funding_cost_bps.
}
```

- [ ] **Step 3: Run parity tests**

Run:

```bash
cargo test -p backtest-engine
cargo test -p trading-engine
```

Expected final result: both commands exit 0. If `trading-engine` needs local port binding and fails in sandbox, rerun with the approved elevated path and record the reason in the ledger.

- [ ] **Step 4: Commit P1**

Run:

```bash
git add apps/trading-engine/src/martingale_runtime.rs apps/trading-engine/tests/martingale_runtime.rs apps/backtest-engine/src/martingale/kline_engine.rs docs/superpowers/reports/2026-07-07-glm-martingale-round8-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round8/exploration-registry.jsonl
git commit -m "feat: 修复思路 Round8补齐马丁R5-R7 live-parity与语义测试"
```

## 5. Task P2: 2025 Regime-Rescue Martingale Allocator

This is the highest-value new search because Round 7 still fails mainly through 2025 loss and 2024/2025 segment DD.

**Files:**

- Create: `scripts/glm_r8_regime_rescue_allocator.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round8/r8-regime-rescue-grid.json`
- Modify: `docs/superpowers/reports/2026-07-07-glm-martingale-round8-search-ledger.md`

- [ ] **Step 1: Implement lagged allocator**

Inputs:

```text
R7-ANKR-q1w24p24 = docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json
R6-QB-lowdd       = docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json
R4-combo          = docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json
R5-fine           = docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json
```

Allocator states must be based only on data strictly before the rebalance timestamp:

```text
lookback_days: 14, 30, 60, 90
rebalance_days: 7, 14, 30
score: rolling_return_minus_1x_dd, rolling_return_minus_2x_dd, rolling_return_minus_cost, recovery_speed
max_high_ann_weight: 0.25, 0.35, 0.50
min_low_dd_weight: 0.20, 0.35, 0.50
cash_weight_when_rolling_dd_gt: none, 8, 12, 16
switch_hysteresis_score_gap: 0, 3, 6
```

The output JSON per candidate must include:

```json
{
  "label": "r8_rr_l30_r14_dd2_hi35_low35_cash12_hys3",
  "params": {
    "lookback_days": 30,
    "rebalance_days": 14,
    "score": "rolling_return_minus_2x_dd",
    "max_high_ann_weight": 0.35,
    "min_low_dd_weight": 0.35,
    "cash_weight_when_rolling_dd_gt": 12,
    "switch_hysteresis_score_gap": 3
  },
  "full_metrics": {"ann": 0.0, "dd": 0.0, "ret": 0.0},
  "segment_metrics": {},
  "positive_segments": 0,
  "target_profile_hit": null,
  "live_ready": false
}
```

- [ ] **Step 2: Run full grid**

Run:

```bash
python3 scripts/glm_r8_regime_rescue_allocator.py \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round8/r8-regime-rescue-grid.json
```

Expected minimum search size: 864 allocator candidates. Every candidate must include full + five segment metrics.

- [ ] **Step 3: Promote or reject**

Promotion gates:

- Conservative: ann >=50, DD <=10, segment DD <=15, positive segments >=4.
- Balanced: ann >=90, DD <=20, segment DD <=25, positive segments >=4.
- Aggressive: ann >=110, DD <=30, segment DD <=35, positive segments >=4.
- Live-ready flag remains false until P1 parity covers all used features.

Commit:

```bash
git add scripts/glm_r8_regime_rescue_allocator.py docs/superpowers/artifacts/glm-martingale-core-round8/r8-regime-rescue-grid.json docs/superpowers/reports/2026-07-07-glm-martingale-round8-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round8/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round8完成2025 regime-rescue马丁分配搜索"
```

## 6. Task P3: Dynamic Grid Reset Inside Martingale

This task reopens DGT only as DCA cycle spacing/reset logic. It must not create a standalone grid sleeve.

**Files:**

- Create: `scripts/glm_r8_dca_dynamic_grid_reset.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round8/r8-dca-dynamic-grid-reset.json`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs` only if new config fields are required
- Modify: `crates/shared-domain/src/martingale.rs` only if new config fields are required

- [ ] **Step 1: Add bounded config fields if missing**

Use these field names if implementation is needed:

```rust
pub dynamic_grid_reset_mode: Option<String>, // "cycle_close_only" or "adverse_excursion"
pub dynamic_grid_atr_period: Option<u32>,
pub dynamic_grid_min_step_bps: Option<u32>,
pub dynamic_grid_max_step_bps: Option<u32>,
pub dynamic_grid_adverse_reset_bps: Option<u32>,
```

Rules:

- Reset spacing only at cycle close or after a realized adverse excursion threshold.
- Never reset based on future bars.
- Clamp spacing between min and max bps.
- Safety orders still belong to the original martingale cycle.

- [ ] **Step 2: Search bounded grid**

Search only these combinations:

```text
base: R7-ANKR-q1w24p24, R5-fine, R6-QB-lowdd, R4-combo
reset_mode: cycle_close_only, adverse_excursion
atr_period: 14, 28
min_step_bps: 120, 150
max_step_bps: 210, 240, 300
adverse_reset_bps: 450, 600, 900
safety_order_condition: existing, existing_and_adx_below_30, existing_and_rsi_below_40
```

Expected minimum search size: 432 candidates. Do not expand until this complete grid is recorded.

- [ ] **Step 3: Validate full and segments**

For every candidate, run full + five segments using the same data and budget:

```bash
python3 scripts/glm_r8_dca_dynamic_grid_reset.py \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round8/r8-dca-dynamic-grid-reset.json
```

Commit:

```bash
git add scripts/glm_r8_dca_dynamic_grid_reset.py apps/backtest-engine/src/martingale/kline_engine.rs crates/shared-domain/src/martingale.rs docs/superpowers/artifacts/glm-martingale-core-round8/r8-dca-dynamic-grid-reset.json docs/superpowers/reports/2026-07-07-glm-martingale-round8-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round8/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round8完成马丁内部动态网格reset搜索"
```

## 7. Task P4: Cost-Cover TP And Rescue Exit

Previous trailing lock and static TP sweeps are do-not-repeat. This task is new only if it explicitly uses accumulated cycle cost and late-leg risk state.

**Files:**

- Create: `scripts/glm_r8_cost_cover_rescue_exit.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round8/r8-cost-cover-rescue-exit.json`

- [ ] **Step 1: Implement cost-cover rules**

Allowed rules:

```text
after_leg: 2, 3, 4
cost_cover_buffer_bps: 30, 60, 100
rescue_tp_compress_bps: 80, 120, 180
breakeven_floor_after_partial_stage: 1, 2
max_cycle_age_hours: 72, 168, 336
```

Definition:

- After the selected leg count, TP may compress only to recover average entry plus fees, slippage, and funding plus buffer.
- If cycle age exceeds `max_cycle_age_hours`, exit only at cost-cover or better; do not hard-stop below cost unless existing strategy stop-loss fires.
- This remains a martingale cycle exit rule.

- [ ] **Step 2: Run grid**

Run:

```bash
python3 scripts/glm_r8_cost_cover_rescue_exit.py \
  --bases docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round8/r8-cost-cover-rescue-exit.json
```

Expected minimum search size: 324 candidates. Reject the family if it improves DD by less than 3pp while reducing ann by more than 5pp.

Commit:

```bash
git add scripts/glm_r8_cost_cover_rescue_exit.py docs/superpowers/artifacts/glm-martingale-core-round8/r8-cost-cover-rescue-exit.json docs/superpowers/reports/2026-07-07-glm-martingale-round8-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round8/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round8完成成本覆盖TP与rescue退出搜索"
```

## 8. Task P5: Small-Capital Live-Executable Packaging

This task is mandatory before claiming any target hit.

**Files:**

- Create: `scripts/glm_r8_package_small_capital_candidates.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round8/r8-small-capital-packages.json`
- Create candidate configs under `docs/superpowers/artifacts/glm-martingale-core-round8/promising/`

- [ ] **Step 1: Package only candidate families with target or near-target evidence**

Near-target gates:

```text
conservative_near: ann >=45 and DD <=12
balanced_near: ann >=75 and DD <=22
aggressive_near: ann >=90 and DD <=32
```

Each package must include:

```json
{
  "label": "r8-package-aggressive-001",
  "source_family": "r8-regime-rescue",
  "config_path": "docs/superpowers/artifacts/glm-martingale-core-round8/promising/r8-package-aggressive-001.json",
  "budget": 5000,
  "exchange_min_notional": 5,
  "max_capital_used": 0.0,
  "budget_blocked_legs": 0,
  "min_first_order_quote": 0.0,
  "live_ready": false,
  "full_metrics": {},
  "segment_metrics": {},
  "target_profile_hit": null
}
```

If `budget_blocked_legs > 0`, the package may remain research-valid but must be marked `live_ready=false` until trading-engine parity proves identical rejection behavior.

- [ ] **Step 2: Replay each package independently**

Run for each package:

```bash
target/release/portfolio_budget_replay \
  --config docs/superpowers/artifacts/glm-martingale-core-round8/promising/<candidate>.json \
  --budget 5000 \
  --start-ms 1672531200000 \
  --end-ms 1780271999999 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --profile aggressive \
  --portfolio-id <candidate> \
  --exchange-min-notional 5
```

Then run the same candidate over all five segment windows.

Commit:

```bash
git add scripts/glm_r8_package_small_capital_candidates.py docs/superpowers/artifacts/glm-martingale-core-round8/promising docs/superpowers/artifacts/glm-martingale-core-round8/r8-small-capital-packages.json docs/superpowers/reports/2026-07-07-glm-martingale-round8-search-ledger.md docs/superpowers/artifacts/glm-martingale-core-round8/exploration-registry.jsonl
git commit -m "docs: 修复思路 Round8封装小资金马丁候选并复算"
```

## 9. Task P6: Final Validation And Handoff

**Files:**

- Create: `docs/superpowers/reports/2026-07-07-glm-round8-handoff-to-chatgpt.md`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round8/r8-final-validation.json`

- [ ] **Step 1: Final validation script or jq summary**

Final JSON must include:

```json
{
  "target_hits": {
    "conservative": [],
    "balanced": [],
    "aggressive": []
  },
  "best_frontier": [],
  "failed_families": [],
  "live_ready_candidates": [],
  "research_only_candidates": [],
  "do_not_repeat_new_keys": []
}
```

- [ ] **Step 2: Handoff requirements**

The handoff must state:

- Exact branch and latest commit hash.
- Candidate counts per family.
- Best full-period ann/DD/return.
- Segment table for every promoted candidate.
- Whether each candidate is live-ready or research-only.
- Whether it uses cap-truncated legs under 5000U.
- Every failed family and non-repeat key.
- Clear answer for each original target.

- [ ] **Step 3: Verification before final commit**

Run:

```bash
rg -n "T[B]D|T[O]DO|P[L]ACEHOLDER|待[补]|后续[补]" docs/superpowers/reports/2026-07-07-glm-round8-handoff-to-chatgpt.md docs/superpowers/plans/2026-07-07-glm-martingale-core-round8-live-parity-and-regime-rescue-plan.md
git diff --check
git status --short --branch
```

Commit and push:

```bash
git add docs/superpowers/reports/2026-07-07-glm-round8-handoff-to-chatgpt.md docs/superpowers/artifacts/glm-martingale-core-round8/r8-final-validation.json
git commit -m "docs: 修复思路 Round8最终交接马丁目标验证"
git push
```

## 10. Strict Do-Not-Repeat List

Do not rerun these unless the registry line names a materially new mechanism:

- Plain tight stop-loss sweeps.
- Static portfolio DD stop sweeps.
- Static spacing or ATR spacing around prior ranges.
- Same-symbol hedge that cancels martingale return.
- Pure funding carry or funding as a standalone sleeve.
- Pure trend/breakout strategy.
- XRP-only quarantine/trailing lock repeats.
- Cost gate thresholds without implemented engine behavior.
- Python-only allocator claimed as live-ready.
- Symbol swap sweeps without a new cost/DD/regime mechanism.

## 11. Stop Conditions

Stop the round and hand off immediately if any candidate satisfies a full target gate after independent replay:

- Conservative: ann >=50, DD <=10, segment DD <=15, positive segments >=4, capital <5000U.
- Balanced: ann >=90, DD <=20, segment DD <=25, positive segments >=4, capital <5000U.
- Aggressive: ann >=110, DD <=30, segment DD <=35, positive segments >=4, capital <5000U.

If no target hit appears after P2-P5, hand off the best frontier and all rejected families. Do not keep expanding blind grids without a new hypothesis and registry key.
