# GLM Martingale Core Round 11 Live Rebalance And Native Minigrid Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish true production live-readiness for the R9/R10 allocator, then test martingale-native architectures that Round10 did not actually implement.

**Architecture:** Martingale/DCA/grid remains the trading core. Signals may only gate entries, safety-order sizing/delay, exits, cash defense, or sleeve selection. No pure trend, breakout, funding-carry, pair-neutral, or stat-arb trades may count toward the user's targets.

**Tech Stack:** Rust `backtest-engine`, `trading-engine`, `shared-domain`, `shared-db`; Python grid scripts; `portfolio_budget_replay`; JSON/JSONL artifacts under `docs/superpowers/artifacts/glm-martingale-core-round11/`.

---

## 0. Non-Negotiable Gates

Targets remain unchanged:

| Tier | Ann | DD | Budget | Other gates |
|---|---:|---:|---:|---|
| Conservative | >=50% | <=10% | <5000U | 4/5 positive segments, multi-symbol, live reproducible |
| Balanced | >=90% | <=20% | <5000U | 4/5 positive segments, multi-symbol, live reproducible |
| Aggressive | >=110% | <=30% | <5000U | 3/5 positive segments, multi-symbol, live reproducible |

Promotion gates:

- Full-period replay plus `h1_2023`, `h2_2023`, `2024`, `2025`, `2026_ytd`.
- At least 5 traded symbols and at least 3 independent base assets.
- No symbol above 35% configured budget share.
- No symbol above 35% realized gross PnL contribution.
- Budget <=5000U. Higher-budget runs are diagnostic only.
- If a candidate uses new backtest behavior without `trading-engine` parity, set `research_only=true` and do not count it as a target hit.
- Every failed family must be appended to the Round11 registry before the next family starts.

## 1. External References And Interpretation

Use these sources only to shape martingale-native mechanics:

- 3Commas condition-based averaging orders: minimum deviation plus indicators can gate averaging orders. Round11 may use signals to size or delay safety orders, not to replace martingale entries.
  https://help.3commas.io/en/articles/9663694-dca-bot-averaging-orders-by-technical-indicators
- 3Commas trailing take profit: trailing is an exit mechanic. Round10's trailing_lock approximation failed, so do not repeat that exact grid.
  https://help.3commas.io/en/articles/3108981-how-take-profit-works-smarttrade-and-dca-bots-trailing-feature-explained
- Freqtrade position adjustment callbacks: DCA must be explicit and controlled for performance. Use this as a reminder that native minigrid must be implemented as position reduction, not as standalone exposure.
  https://www.freqtrade.io/en/stable/strategy-callbacks/
- Hummingbot DCA executor: DCA can be an executor workflow under controller decisions. Use this to separate allocator decisions from order execution.
  https://hummingbot.org/strategies/v2-strategies/executors/dcaexecutor/
- Gainium minigrids DCA: minigrids between DCA steps are a real DCA-grid hybrid. Round10 did not implement this natively.
  https://gainium.io/help/minigrids-dca
- Binance USD-M exchange info: every live candidate must respect filters, min notional, and order semantics.
  https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information

## 2. Task P0: Canonical Startup And Non-Repeat Lock

**Files:**

- Read: `docs/superpowers/artifacts/glm-martingale-core-round10/r1-r10-corrected-status.json`
- Read: `docs/superpowers/reports/2026-07-09-glm-round10-execution-audit-and-fix.md`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round11/exploration-registry.jsonl`
- Create: `docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round11/run-manifests/r11-data-sha256.txt`

- [ ] **Step 1: Start from synced Round10**

Run:

```bash
git fetch origin
git checkout glm-martingale-core-round10
git pull --ff-only
git checkout -b glm-martingale-core-round11
```

Expected: new branch `glm-martingale-core-round11` with no uncommitted files.

- [ ] **Step 2: Create evidence layout**

Run:

```bash
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round11/promising
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round11/rejected
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round11/run-manifests
printf '# GLM Martingale Round 11 Search Ledger\n\n' > docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md
touch docs/superpowers/artifacts/glm-martingale-core-round11/exploration-registry.jsonl
sha256sum data/market_data_full.db data/funding_rates.db data/premium_index.db > docs/superpowers/artifacts/glm-martingale-core-round11/run-manifests/r11-data-sha256.txt
```

- [ ] **Step 3: Append canonical carry-in**

Append this exact block:

```markdown
## Canonical Carry-In

- Source: `docs/superpowers/artifacts/glm-martingale-core-round10/r1-r10-corrected-status.json`
- Best research: R9/R10 allocator ann 64.4196% / DD 18.2111% / 5/5, module-ready plus static live gate, not fully live-ready.
- Best fully live-ready: R4-combo ann 34.7233% / DD 17.6843% / 4/5.
- Target hits through Round10: none.
- Do not repeat: Round8 leaky allocator timing, Round9 random symbol substitution, reserve-only DD compression, Round10 strict condition-gated SO, Round10 trailing_lock approximation, Round10 partial-TP minigrid approximation, static-gate-only live_ready claims.
```

- [ ] **Step 4: Register and commit P0**

Append to registry:

```json
{"id":"r11-P0-canonical-startup-001","status":"complete","family":"startup","source":"r1-r10-corrected-status","target_hit":false}
```

Run:

```bash
git add docs/superpowers/artifacts/glm-martingale-core-round11 docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md
git commit -m "docs: 修复思路 初始化Round11马丁校正台账"
```

## 3. Task P1: Finish True Production Live Allocator

**Files:**

- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Create: `apps/trading-engine/src/martingale_allocator_live.rs`
- Create: `apps/trading-engine/tests/martingale_allocator_live_production.rs`
- Read: `apps/backtest-engine/src/martingale/allocator_replay.rs`
- Read: `crates/shared-db/src/backtest.rs`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round11/r11-live-allocator-production-wiring.json`

- [ ] **Step 1: Write failing production tests**

Create `apps/trading-engine/tests/martingale_allocator_live_production.rs` with these test names and assertions:

```rust
#[test]
fn r11_allocator_rebalances_when_next_rebalance_due_in_main_reconcile() {
    // Build an ephemeral DB portfolio with allocator_config, strategy_to_sleeve_id,
    // and risk_summary.allocator_state.active_sleeve_id = "R4".
    // Insert completed allocator observations where "ANKR-q" has the higher score.
    // Run reconcile_running_martingale_portfolios at now_ms >= next_rebalance_ms.
    // Assert risk_summary.allocator_state.active_sleeve_id == "ANKR-q".
}

#[test]
fn r11_allocator_persists_state_and_blocks_new_cycles_after_switch() {
    // After the same rebalance, assert risk_summary.allocator_state.next_rebalance_ms
    // advanced by rebalance_days and cycle_results contain
    // martingale_allocator_blocked_new_cycle for an inactive sleeve.
}

#[test]
fn r11_allocator_uses_completed_observations_only() {
    // Add a current interval observation whose timestamp is greater than the
    // rebalance boundary and would make another sleeve win.
    // Assert the rebalance ignores that observation until the next boundary.
}

#[test]
fn r11_allocator_reads_risk_summary_state_before_config_fallback() {
    // Put config.portfolio_config.allocator_state.active_sleeve_id = "R4"
    // and risk_summary.allocator_state.active_sleeve_id = "ANKR-q".
    // Assert the main reconcile gate treats "ANKR-q" as active.
}
```

Run:

```bash
cargo test -p trading-engine r11_allocator_rebalances_when_next_rebalance_due_in_main_reconcile r11_allocator_persists_state_and_blocks_new_cycles_after_switch r11_allocator_uses_completed_observations_only r11_allocator_reads_risk_summary_state_before_config_fallback
```

Expected before implementation: tests fail because production rebalance and persistence are missing.

- [ ] **Step 2: Implement allocator live helpers**

Create `apps/trading-engine/src/martingale_allocator_live.rs` with these public functions:

```rust
use std::collections::HashMap;

use backtest_engine::martingale::allocator_replay::{
    AllocatorConfig, AllocatorScoreFunction, AllocatorState, RollingMetrics,
};
use serde_json::Value;

pub fn parse_allocator_config(value: &Value) -> Option<AllocatorConfig> {
    serde_json::from_value(value.clone()).ok()
}

pub fn parse_strategy_to_sleeve_id(value: &Value) -> HashMap<String, String> {
    value
        .as_object()
        .into_iter()
        .flat_map(|obj| obj.iter())
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
        .collect()
}

pub fn allocator_state_from_json(value: Option<&Value>, fallback_sleeve: &str, now_ms: i64) -> AllocatorState {
    let active = value
        .and_then(|v| v.get("active_sleeve_id"))
        .and_then(Value::as_str)
        .unwrap_or(fallback_sleeve)
        .to_owned();
    let next_rebalance_ms = value
        .and_then(|v| v.get("next_rebalance_ms"))
        .and_then(Value::as_i64)
        .unwrap_or(now_ms);
    let last_completed_interval_ms = value
        .and_then(|v| v.get("last_completed_interval_ms"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let mut state = AllocatorState::new(active, next_rebalance_ms);
    state.last_completed_interval_ms = last_completed_interval_ms;
    state
}

pub fn state_to_json(state: &AllocatorState) -> Value {
    serde_json::json!({
        "active_sleeve_id": state.active_sleeve_id,
        "next_rebalance_ms": state.next_rebalance_ms,
        "last_completed_interval_ms": state.last_completed_interval_ms,
        "rolling_metrics_by_sleeve": state.rolling_metrics_by_sleeve
    })
}
```

Also implement `completed_allocator_metrics(...) -> HashMap<String, RollingMetrics>` using only observations with `timestamp_ms <= rebalance_boundary_ms`. Store/read observations from `risk_summary.allocator_observations` with this shape:

```json
{"timestamp_ms": 1704067200000, "sleeve_id": "R4", "equity_quote": 5033.25}
```

- [ ] **Step 3: Wire production rebalance in `main.rs`**

Modify `runtime_with_allocator_state` so it:

1. Parses `allocator_config` as `AllocatorConfig`.
2. Reads allocator state in this priority:
   - `portfolio.risk_summary.allocator_state`
   - `portfolio.config.portfolio_config.allocator_state`
   - alphabetically first sleeve fallback.
3. If `now_ms >= state.next_rebalance_ms`, computes completed metrics and calls `runtime.rebalance_allocator(now_ms, &cfg, &metrics)`.
4. Writes the updated state into a mutable `risk_summary["allocator_state"]`.
5. Persists risk summary with `db.backtest_repo().update_martingale_portfolio_risk_summary(...)` before opening new cycles.

Required persisted JSON:

```json
{
  "allocator_state": {
    "active_sleeve_id": "ANKR-q",
    "next_rebalance_ms": 1704672000000,
    "last_completed_interval_ms": 1704067200000,
    "rolling_metrics_by_sleeve": {}
  }
}
```

- [ ] **Step 4: Run allocator tests**

Run:

```bash
cargo test -p trading-engine r11_allocator_rebalances_when_next_rebalance_due_in_main_reconcile r11_allocator_persists_state_and_blocks_new_cycles_after_switch r11_allocator_uses_completed_observations_only r11_allocator_reads_risk_summary_state_before_config_fallback
cargo test -p trading-engine r10_allocator_blocks_new_cycles_for_inactive_sleeves_in_main_reconcile r10_allocator_switch_does_not_cancel_existing_cycle r10_allocator_rebalance_uses_completed_equity_only_in_main_loop
cargo test -p backtest-engine allocator_switch_uses_completed_interval_only allocator_respects_max_high_ann_and_min_low_dd_weights
```

Expected: all listed tests pass.

- [ ] **Step 5: Write P1 evidence and commit**

Create `r11-live-allocator-production-wiring.json`:

```json
{
  "task": "P1",
  "full_live_ready_gap_from_round10": "closed_if_all_tests_pass",
  "production_rolling_metrics_wired": true,
  "dynamic_rebalance_wired": true,
  "state_persistence_wired": true,
  "risk_summary_state_priority": true,
  "existing_cycles_force_closed": false,
  "target_hit": false
}
```

Append registry:

```json
{"id":"r11-P1-live-allocator-production-wiring-001","status":"complete","family":"live_allocator_production_wiring","target_hit":false,"result":"Production rolling metrics, dynamic rebalance, and allocator_state persistence wired and tested."}
```

Commit:

```bash
git add apps/trading-engine/src apps/trading-engine/tests docs/superpowers/artifacts/glm-martingale-core-round11 docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md
git commit -m "feat: 修复思路 完成马丁allocator生产动态rebalance"
```

## 4. Task P2: R9 Winner Production Parity After True Live Rebalance

**Files:**

- Create: `scripts/glm_r11_r9_allocator_production_parity.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round11/r11-r9-production-live-parity.json`
- Modify: `docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md`

- [ ] **Step 1: Build parity replay**

The script must replay the R9 winner using the same `AllocatorConfig` as Round9/Round10:

```json
{"lookback_days":60,"rebalance_days":7,"score_function":"calmar_like","max_high_ann_weight":0.2,"min_low_dd_weight":0.2}
```

It must output:

- full metrics;
- five segment metrics;
- every rebalance decision with `timestamp_ms`, `metrics_cutoff_ms`, `applies_from_ms`, `active_sleeve_before`, `active_sleeve_after`, and scores;
- `production_live_ready_after_p1: true` only if P1 evidence exists and all P1 tests pass.

- [ ] **Step 2: Run parity**

Run:

```bash
python3 scripts/glm_r11_r9_allocator_production_parity.py --out docs/superpowers/artifacts/glm-martingale-core-round11/r11-r9-production-live-parity.json --budget 5000
```

Expected:

```text
ann_abs_diff <= 0.2
dd_abs_diff <= 0.2
forward_only_decisions_pass = true
segment_metrics_present_pass = true
```

- [ ] **Step 3: Registry and commit**

If parity passes but targets still fail, append:

```json
{"id":"r11-P2-r9-winner-production-parity-001","status":"complete","family":"live_parity_replay","target_hit":false,"result":"R9 winner production parity PASS after true live allocator wiring; still misses Conservative DD and Balanced/Aggressive ann gates."}
```

Commit:

```bash
git add scripts/glm_r11_r9_allocator_production_parity.py docs/superpowers/artifacts/glm-martingale-core-round11/r11-r9-production-live-parity.json docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md
git commit -m "test: 修复思路 验证R9 allocator生产实盘一致性"
```

## 5. Task P3: Native Inventory-Reducing DCA Minigrid

**Files:**

- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Create: `apps/backtest-engine/tests/martingale_native_minigrid.rs`
- Create: `apps/trading-engine/tests/martingale_native_minigrid_live.rs`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round11/r11-native-minigrid-engine-parity.json`

- [ ] **Step 1: Add failing backtest tests**

Create tests with these names:

```rust
#[test]
fn native_minigrid_after_safety_fill_reduces_inventory_only() {
    // Long cycle: base order fills, safety order fills lower, price bounces to
    // minigrid level. Assert event_type == "dca_minigrid_take_profit" and
    // total leg quantity decreases. Assert no new exposure leg is added.
}

#[test]
fn native_minigrid_never_opens_without_active_safety_leg() {
    // Price reaches minigrid level before any safety order exists.
    // Assert no minigrid event and no position change.
}

#[test]
fn native_minigrid_respects_min_notional_and_close_fraction() {
    // If close notional is below 5 USDT, no minigrid event is emitted.
    // If above 5 USDT, closed fraction equals close_fraction_num/close_fraction_den.
}
```

Run:

```bash
cargo test -p backtest-engine native_minigrid_after_safety_fill_reduces_inventory_only native_minigrid_never_opens_without_active_safety_leg native_minigrid_respects_min_notional_and_close_fraction
```

Expected before implementation: tests fail because native minigrid does not exist.

- [ ] **Step 2: Add config struct**

Add to `crates/shared-domain/src/martingale.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingaleDcaMiniGridConfig {
    pub levels_per_band: u32,
    pub spacing_bps: u32,
    pub close_fraction_num: u32,
    pub close_fraction_den: u32,
    pub min_profit_bps: u32,
    pub max_active_levels: u32,
}
```

Add this field to `MartingaleRiskLimits`:

```rust
#[serde(default)]
pub dca_minigrid: Option<MartingaleDcaMiniGridConfig>,
```

Validation rules:

- `levels_per_band` in `1..=5`.
- `spacing_bps` in `10..=150`.
- `close_fraction_num > 0`.
- `close_fraction_den >= close_fraction_num`.
- `min_profit_bps >= 5`.
- `max_active_levels in 1..=5`.

- [ ] **Step 3: Implement backtest semantics**

In `kline_engine.rs`, after a safety order fill and before final full TP:

- Create minigrid levels only when the active cycle has at least one safety leg.
- Long level price: `last_safety_fill_price * (1.0 + (min_profit_bps + level_index * spacing_bps) / 10000.0)`.
- Short level price: `last_safety_fill_price * (1.0 - (min_profit_bps + level_index * spacing_bps) / 10000.0)`.
- When a level is touched, close `close_fraction_num / close_fraction_den` of remaining quantity across all legs.
- Emit event `dca_minigrid_take_profit`.
- Do not add exposure, do not reset the cycle, do not count it as a safety order.
- Skip if closed notional is below 5 USDT.

- [ ] **Step 4: Add live semantics or mark research-only**

In `trading-engine`, implement reduce-only partial closes for minigrid levels. If the existing order path cannot submit partial reduce-only quantities safely, stop P3 promotion and write:

```json
{"native_minigrid_backtest_ready":true,"native_minigrid_live_ready":false,"research_only":true}
```

If live semantics are implemented, add tests:

```rust
#[test]
fn live_minigrid_uses_reduce_only_partial_close_quantity() {
    // Assert generated close order quantity is a fraction of the active position
    // and reduce_only is true for one-way mode or positionSide is preserved for hedge mode.
}

#[test]
fn live_minigrid_does_not_cancel_existing_safety_orders() {
    // Assert safety orders remain unless the full cycle closes.
}
```

Run:

```bash
cargo test -p trading-engine live_minigrid_uses_reduce_only_partial_close_quantity live_minigrid_does_not_cancel_existing_safety_orders
```

- [ ] **Step 5: Evidence and commit**

Create `r11-native-minigrid-engine-parity.json`:

```json
{
  "task": "P3",
  "native_minigrid_backtest_ready": true,
  "native_minigrid_live_ready": false,
  "research_only_until_live_tests_pass": true,
  "target_hit": false
}
```

Commit:

```bash
git add crates/shared-domain/src/martingale.rs apps/backtest-engine/src/martingale/kline_engine.rs apps/trading-engine/src apps/backtest-engine/tests apps/trading-engine/tests docs/superpowers/artifacts/glm-martingale-core-round11
git commit -m "feat: 修复思路 增加马丁原生DCA minigrid语义"
```

## 6. Task P4: Native Minigrid Parameter Search

**Files:**

- Create: `scripts/glm_r11_native_minigrid_search.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round11/r11-native-minigrid-search.json`
- Modify: `docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md`

- [ ] **Step 1: Generate grid**

Use only multi-symbol bases:

- `R4-combo`
- `R7-ANKR-q1w24p24`
- `r9-P5-winner` sleeves where config is reconstructable

Grid:

```text
bases: 3
levels_per_band: 1,2,3,5
spacing_bps: 15,30,50,80
close_fraction: 1/8,1/6,1/4
min_profit_bps: 10,20,35,55
max_active_levels: 1,2,3
dca_step_bps: keep_base,180,250,350
```

Minimum valid configs: `3*4*4*3*4*3*4 = 6912`.

- [ ] **Step 2: Run full plus five segments**

Run:

```bash
python3 scripts/glm_r11_native_minigrid_search.py --out docs/superpowers/artifacts/glm-martingale-core-round11/r11-native-minigrid-search.json --budget 5000 --workers 26
```

Each candidate must include:

- full metrics;
- segment metrics;
- traded symbols;
- symbol_count;
- max_symbol_budget_pct;
- realized PnL contribution by symbol;
- `research_only` copied from P3 live readiness.

- [ ] **Step 3: Registry**

Append one of:

```json
{"id":"r11-P4-native-minigrid-search-001","status":"complete","family":"native_dca_minigrid","target_hit":false,"result":"6912+ configs; 0 target hits; do not repeat exact grid."}
```

or, if a live-ready target hit exists:

```json
{"id":"r11-P4-native-minigrid-search-001","status":"promising","family":"native_dca_minigrid","target_hit":true,"result":"Candidate(s) met target gates; copied to promising with full metrics and live parity evidence."}
```

Commit:

```bash
git add scripts/glm_r11_native_minigrid_search.py docs/superpowers/artifacts/glm-martingale-core-round11 docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md
git commit -m "test: 修复思路 搜索马丁原生DCA minigrid参数"
```

## 7. Task P5: Non-R4 Martingale Architecture Expansion

**Files:**

- Create: `scripts/glm_r11_non_r4_martingale_architecture_search.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round11/r11-non-r4-architecture-search.json`
- Modify: `docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md`

- [ ] **Step 1: Search only martingale-native families**

Families:

1. Fixed percent TP, no partial TP:
   - `take_profit_bps`: 45,60,80,100,130,180
   - `first_order_quote`: 8,12,18,25
   - `multiplier`: 1.15,1.35,1.55,1.8
   - `max_legs`: 3,4,5,6
2. Low multiplier high frequency:
   - `step_bps`: 60,90,120,160
   - `multiplier`: 1.05,1.15,1.25
   - `max_active_cycles`: 1,2,3
3. Volatility ladder:
   - `spacing`: ATR or mixed fixed/ATR already supported by engine
   - `new_cycle_atr_pause_pct`: 1.2,1.6,2.0,2.6
4. Asymmetric long/short:
   - long and short use independent `step_bps`, `tp_bps`, `multiplier`
   - same symbol budget caps

Minimum valid configs: 8000 full plus segment replays.

- [ ] **Step 2: Run**

```bash
python3 scripts/glm_r11_non_r4_martingale_architecture_search.py --out docs/superpowers/artifacts/glm-martingale-core-round11/r11-non-r4-architecture-search.json --budget 5000 --workers 26
```

Promotion rule:

- Multi-symbol only.
- No `ANKR`-only or single-symbol candidate can be promoted.
- Any target hit must be rerun with a distinct `portfolio-id` and identical metrics tolerance <=0.2 ann/DD.

- [ ] **Step 3: Commit**

```bash
git add scripts/glm_r11_non_r4_martingale_architecture_search.py docs/superpowers/artifacts/glm-martingale-core-round11 docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md
git commit -m "test: 修复思路 搜索非R4参数族马丁架构"
```

## 8. Task P6: Conditional Safety Order V2 Without Strict Blocking

**Files:**

- Create: `scripts/glm_r11_conditional_so_v2_search.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round11/r11-conditional-so-v2-search.json`
- Modify: `docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md`

- [ ] **Step 1: Avoid Round10 repeat**

Do not use strict `safety_order_condition` as a hard all-or-nothing gate. Use existing martingale-native risk controls instead:

- `safety_order_rebound_bps`: 0,20,40,70
- `safety_order_basis`: `base_order`, `last_executed_order`
- `safety_skip_adx_threshold`: 35,45,55,70
- drawdown-state `safety_order_scale`: 0.5,0.7,0.9,1.0
- late-leg cap: reduce only legs 4+; never block legs 1-2.

Minimum valid configs: 4000.

- [ ] **Step 2: Run**

```bash
python3 scripts/glm_r11_conditional_so_v2_search.py --out docs/superpowers/artifacts/glm-martingale-core-round11/r11-conditional-so-v2-search.json --budget 5000 --workers 26
```

- [ ] **Step 3: Commit**

```bash
git add scripts/glm_r11_conditional_so_v2_search.py docs/superpowers/artifacts/glm-martingale-core-round11 docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md
git commit -m "test: 修复思路 搜索非硬阻塞安全单V2"
```

## 9. Task P7: Heterogeneous Sleeve Allocator

**Files:**

- Create: `scripts/glm_r11_heterogeneous_allocator.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round11/r11-heterogeneous-allocator.json`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round11/r11-leave-one-segment-out-validation.json`

- [ ] **Step 1: Build sleeves only from validated candidates**

Allowed sleeves:

- R4-combo.
- R9/R10 allocator component sleeves.
- Best live-ready native minigrid sleeve only if P3 live tests pass.
- Best P5 non-R4 martingale sleeve if full plus five segments are present.
- Best P6 conditional SO v2 sleeve if it is not a strict-block repeat.

- [ ] **Step 2: Run allocator grid**

Grid:

```text
lookback_days: 30,45,60,90,120
rebalance_days: 3,7,14,30
score_function: ann_minus_1dd, ann_minus_2dd, calmar_like, down_capture_penalty
switch_hysteresis_score_gap: 0,2,5,8
cash_trigger_rolling_dd_pct: none,6,8,10
min_low_dd_weight: 0,0.2,0.4
max_high_ann_weight: 0.2,0.5,1.0
```

Run:

```bash
python3 scripts/glm_r11_heterogeneous_allocator.py --out docs/superpowers/artifacts/glm-martingale-core-round11/r11-heterogeneous-allocator.json --budget 5000
```

- [ ] **Step 3: LOSO validation**

For every target candidate, rerun leave-one-segment-out:

```bash
python3 scripts/glm_r11_heterogeneous_allocator.py --loso --out docs/superpowers/artifacts/glm-martingale-core-round11/r11-leave-one-segment-out-validation.json --budget 5000
```

Promotion requires:

- target gate on full period;
- target gate remains in at least 4/5 LOSO folds for conservative/balanced;
- target gate remains in at least 3/5 LOSO folds for aggressive;
- `fully_live_ready=true`.

- [ ] **Step 4: Commit**

```bash
git add scripts/glm_r11_heterogeneous_allocator.py docs/superpowers/artifacts/glm-martingale-core-round11
git commit -m "test: 修复思路 搜索异构马丁sleeve allocator组合"
```

## 10. Task P8: Final Validation And Handoff

**Files:**

- Create: `docs/superpowers/artifacts/glm-martingale-core-round11/r11-final-validation.json`
- Create: `docs/superpowers/reports/2026-07-09-glm-round11-handoff-to-chatgpt.md`
- Modify: `docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md`

- [ ] **Step 1: Run full verification**

Run:

```bash
jq empty docs/superpowers/artifacts/glm-martingale-core-round11/*.json
cargo test -p backtest-engine
cargo test -p trading-engine
git diff --check
```

Expected: JSON valid, tests pass, diff check clean.

- [ ] **Step 2: Final validation JSON**

`r11-final-validation.json` must include:

```json
{
  "round": 11,
  "target_hits": {"conservative": [], "balanced": [], "aggressive": []},
  "best_research": [],
  "best_fully_live_ready": [],
  "new_failed_families": [],
  "do_not_repeat_new_keys": [],
  "live_allocator_status": {},
  "native_minigrid_status": {},
  "source_of_truth": "docs/superpowers/artifacts/glm-martingale-core-round11/r11-final-validation.json"
}
```

Fill every list with actual values from P1-P7. Do not leave empty lists unless there were truly zero entries.

- [ ] **Step 3: Handoff**

The handoff must answer explicitly:

- Did any of Conservative/Balanced/Aggressive pass?
- Is the passing candidate multi-symbol?
- Is it under 5000U?
- Is it fully live-ready?
- What exact failure keys should never be repeated?
- What exact artifact contains the best candidate config?

- [ ] **Step 4: Commit and push**

```bash
git add docs/superpowers/artifacts/glm-martingale-core-round11 docs/superpowers/reports/2026-07-09-glm-round11-handoff-to-chatgpt.md docs/superpowers/reports/2026-07-09-glm-martingale-round11-search-ledger.md
git commit -m "docs: 修复思路 完成Round11马丁组合验证交接"
git push -u origin glm-martingale-core-round11
```

## 11. Stop Conditions

Stop immediately and hand back to ChatGPT if any of these happens:

- A candidate appears to hit a target but is single-symbol or fewer than 5 traded symbols.
- A candidate hits in backtest but lacks trading-engine parity.
- Any script uses current-interval data in allocator decisions.
- Any target hit disappears in segment or LOSO validation.
- Any new behavior is implemented only in Python without Rust backtest and live parity.
