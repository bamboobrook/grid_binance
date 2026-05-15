# 马丁固定多空配比与止损重算 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 关闭默认动态多空，恢复可解释的固定组合级多空配比，并修正止损、评分、结果解释，使回测优先搜索“回撤限制内收益最高”的马丁组合。

**Architecture:** 后端以 `backtest-engine` 为权威计算层，`backtest-worker` 负责从任务 payload 生成搜索配置并持久化 summary，`api-server` 负责请求校验与发布风控，`web` 负责向导默认值、手动比例校验和结果解释。旧动态多空代码保留为 experimental，但默认路径必须禁用。

**Tech Stack:** Rust workspace (`api-server`, `backtest-worker`, `backtest-engine`), Next.js/React web app, Node contract tests, Cargo integration tests.

---

## File Structure

- Modify: `apps/backtest-engine/src/martingale/allocation.rs` — 增加固定风险档位配比与关闭动态分配的决策入口。
- Modify: `apps/backtest-engine/src/martingale/exit_rules.rs` — 增加层数止损 + ATR 止损边界，确保止损在最后一层之外。
- Modify: `apps/backtest-engine/src/martingale/scoring.rs` — 把显示 score 归一化为 `0..100`，并按年化收益/回撤/止损/成本/样本稳定性评分。
- Modify: `apps/backtest-engine/src/martingale/metrics.rs` — 确认或补齐 summary 所需字段：年化、止损损耗、费用、滑点、权重、实盘推荐状态。
- Modify: `apps/backtest-engine/src/martingale/portfolio_optimizer.rs` — 固定组合级 long/short 预算池，优先硬约束筛选，fallback 标记不建议实盘。
- Modify: `apps/backtest-worker/src/main.rs` — 任务 payload 默认 `dynamic_allocation_enabled=false`，生成固定配比、止损搜索范围和 summary 字段。
- Modify: `apps/api-server/src/routes/backtest.rs` — 创建任务请求校验 long/short 合计、最大回撤、目标年化和动态分配默认关闭。
- Modify: `apps/api-server/src/services/backtest_service.rs` — 后端任务默认值与结果 fallback 状态透传。
- Modify: `apps/api-server/src/services/martingale_publish_service.rs` — 禁止 `can_recommend_live=false` 或回撤不通过候选默认发布。
- Modify: `apps/api-server/tests/martingale_backtest_flow.rs` — API 与发布风控回归测试。
- Modify: `apps/web/components/backtest/backtest-wizard.tsx` — 风险档位默认多空比例、手动比例校验、动态多空关闭提示。
- Modify: `apps/web/components/backtest/search-config-editor.tsx` — 高级止损搜索范围编辑项。
- Modify: `apps/web/components/backtest/backtest-result-table.tsx` — score 100 分、年化/回撤/止损/费用/实盘建议展示。
- Modify: `apps/web/components/backtest/portfolio-candidate-review.tsx` — 组合候选 long/short 目标与实际分配、fallback 风险提示。
- Modify: `apps/web/components/backtest/martingale-risk-warning.tsx` — 负收益、超回撤、止损磨损、不建议实盘提示。
- Modify: `tests/verification/backtest_worker_contract.test.mjs` — worker contract 更新为固定配比与动态关闭。
- Modify: `tests/verification/martingale_backtest_rebuild_contract.test.mjs` — 前端/API contract 覆盖默认值、校验与结果展示。
- Modify: `tests/verification/martingale_portfolio_contract.test.mjs` — 发布 API 风控 contract。

---

### Task 1: Contract Tests First

**Files:**
- Modify: `tests/verification/backtest_worker_contract.test.mjs`
- Modify: `tests/verification/martingale_backtest_rebuild_contract.test.mjs`
- Modify: `tests/verification/martingale_portfolio_contract.test.mjs`

- [ ] **Step 1: Add worker contract expectations**

In `tests/verification/backtest_worker_contract.test.mjs`, update or add a test that reads `apps/backtest-worker/src/main.rs` and asserts these literal contracts exist:

```js
test("martingale worker defaults to fixed allocation and layer plus ATR stops", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /dynamic_allocation_enabled/);
  assert.match(worker, /false/);
  assert.match(worker, /fixed_by_risk_profile/);
  assert.match(worker, /layer_plus_atr/);
  assert.match(worker, /extra_stop_spacing_multipliers/);
  assert.match(worker, /atr_stop_multipliers/);
  assert.match(worker, /target_annualized_return_pct/);
  assert.match(worker, /can_recommend_live/);
});
```

- [ ] **Step 2: Add frontend contract expectations**

In `tests/verification/martingale_backtest_rebuild_contract.test.mjs`, add assertions that read `apps/web/components/backtest/backtest-wizard.tsx`, `backtest-result-table.tsx`, and `martingale-risk-warning.tsx`:

```js
test("martingale wizard exposes fixed long short defaults and manual validation", () => {
  const wizard = readFileSync("apps/web/components/backtest/backtest-wizard.tsx", "utf8");
  assert.match(wizard, /conservative[^\n]+80[^\n]+20|80[^\n]+20[^\n]+conservative/s);
  assert.match(wizard, /balanced[^\n]+60[^\n]+40|60[^\n]+40[^\n]+balanced/s);
  assert.match(wizard, /aggressive[^\n]+50[^\n]+50|50[^\n]+50[^\n]+aggressive/s);
  assert.match(wizard, /dynamic_allocation_enabled/);
  assert.match(wizard, /longWeightPct/);
  assert.match(wizard, /shortWeightPct/);
  assert.match(wizard, /100/);
});

test("martingale results explain score, annualized return, stops, and live recommendation", () => {
  const table = readFileSync("apps/web/components/backtest/backtest-result-table.tsx", "utf8");
  const warning = readFileSync("apps/web/components/backtest/martingale-risk-warning.tsx", "utf8");
  assert.match(table, /annualized_return_pct|annualizedReturnPct/);
  assert.match(table, /max_drawdown_pct|maxDrawdownPct/);
  assert.match(table, /stop_loss_count|stopLossCount/);
  assert.match(table, /fee_quote|feeQuote/);
  assert.match(table, /slippage_quote|slippageQuote/);
  assert.match(table, /can_recommend_live|canRecommendLive/);
  assert.match(warning, /收益为负|negative return/);
  assert.match(warning, /超过最大回撤|drawdown/i);
  assert.match(warning, /止损频率|stop/i);
  assert.match(warning, /不建议实盘|not recommend/i);
});
```

- [ ] **Step 3: Add publish guard contract**

In `tests/verification/martingale_portfolio_contract.test.mjs`, add a source assertion against `apps/api-server/src/services/martingale_publish_service.rs`:

```js
test("martingale publish service blocks non-recommended backtest candidates", () => {
  const service = readFileSync("apps/api-server/src/services/martingale_publish_service.rs", "utf8");
  assert.match(service, /can_recommend_live/);
  assert.match(service, /max_drawdown_limit_passed/);
  assert.match(service, /not recommended|不建议|cannot publish|risk/i);
});
```

- [ ] **Step 4: Run contract tests and confirm failure**

Run:

```bash
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
node tests/verification/martingale_portfolio_contract.test.mjs
```

Expected: at least one test fails because implementation still contains dynamic allocation defaults or missing fields.

---

### Task 2: Fixed Allocation Model

**Files:**
- Modify: `apps/backtest-engine/src/martingale/allocation.rs`
- Modify: `apps/backtest-engine/src/martingale/portfolio_optimizer.rs`
- Test: Rust unit tests in the same modules or existing integration tests under `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add fixed profile defaults**

Add a public fixed allocation profile helper in `allocation.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedAllocationProfile {
    Conservative,
    Balanced,
    Aggressive,
}

pub fn fixed_long_short_weights(profile: FixedAllocationProfile) -> (f64, f64) {
    match profile {
        FixedAllocationProfile::Conservative => (80.0, 20.0),
        FixedAllocationProfile::Balanced => (60.0, 40.0),
        FixedAllocationProfile::Aggressive => (50.0, 50.0),
    }
}

pub fn validate_long_short_weights(long_weight_pct: f64, short_weight_pct: f64) -> Result<(), String> {
    if !long_weight_pct.is_finite() || !short_weight_pct.is_finite() {
        return Err("long/short weights must be finite".to_string());
    }
    if long_weight_pct < 0.0 || short_weight_pct < 0.0 {
        return Err("long/short weights cannot be negative".to_string());
    }
    if (long_weight_pct + short_weight_pct - 100.0).abs() > 0.001 {
        return Err("long/short weights must sum to 100%".to_string());
    }
    Ok(())
}
```

- [ ] **Step 2: Add allocation unit tests**

Add tests in `allocation.rs`:

```rust
#[test]
fn fixed_allocation_profiles_match_risk_defaults() {
    assert_eq!(fixed_long_short_weights(FixedAllocationProfile::Conservative), (80.0, 20.0));
    assert_eq!(fixed_long_short_weights(FixedAllocationProfile::Balanced), (60.0, 40.0));
    assert_eq!(fixed_long_short_weights(FixedAllocationProfile::Aggressive), (50.0, 50.0));
}

#[test]
fn fixed_allocation_rejects_invalid_manual_weights() {
    assert!(validate_long_short_weights(70.0, 20.0).is_err());
    assert!(validate_long_short_weights(-1.0, 101.0).is_err());
    assert!(validate_long_short_weights(80.0, 20.0).is_ok());
}
```

- [ ] **Step 3: Update optimizer budget pools**

In `portfolio_optimizer.rs`, route `long_and_short` portfolio optimization through fixed long/short budget pools when `dynamic_allocation_enabled` is absent or false:

```rust
let target_long_budget_quote = total_budget_quote * (request.long_weight_pct / 100.0);
let target_short_budget_quote = total_budget_quote * (request.short_weight_pct / 100.0);
```

Use the long pool only for long candidates and short pool only for short candidates. If a pool cannot be fully allocated because candidates fail constraints, keep unused budget and include a warning string like `direction_budget_underutilized` in the portfolio summary.

- [ ] **Step 4: Run engine tests**

Run:

```bash
cargo test -p backtest-engine fixed_allocation -- --nocapture
cargo test -p backtest-engine portfolio -- --nocapture
```

Expected: fixed allocation tests pass; portfolio tests pass or reveal exact compile locations to adjust.

---

### Task 3: Stop-Loss Recalculation

**Files:**
- Modify: `apps/backtest-engine/src/martingale/exit_rules.rs`
- Modify: `apps/backtest-engine/src/martingale/trade_engine.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Test: module tests in `exit_rules.rs` and existing `backtest-engine` tests

- [ ] **Step 1: Add layer plus ATR stop config**

In `exit_rules.rs`, add or extend stop config types with:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct LayerPlusAtrStopConfig {
    pub extra_stop_spacing_multiplier: f64,
    pub atr_stop_multiplier: f64,
}
```

- [ ] **Step 2: Implement stop boundary helper**

Add a helper that never returns a stop inside the active martingale ladder:

```rust
pub fn layer_plus_atr_stop_price(
    direction: MartingaleDirection,
    last_layer_price: f64,
    spacing_pct: f64,
    latest_atr: Option<f64>,
    config: &LayerPlusAtrStopConfig,
) -> Result<f64, String> {
    if last_layer_price <= 0.0 || spacing_pct <= 0.0 {
        return Err("last layer price and spacing must be positive".to_string());
    }
    if config.extra_stop_spacing_multiplier <= 0.0 || config.atr_stop_multiplier <= 0.0 {
        return Err("stop multipliers must be positive".to_string());
    }
    let layer_distance = last_layer_price * spacing_pct * config.extra_stop_spacing_multiplier;
    let atr_distance = latest_atr.unwrap_or(0.0).max(0.0) * config.atr_stop_multiplier;
    let distance = layer_distance.max(atr_distance);
    match direction {
        MartingaleDirection::Long => Ok((last_layer_price - distance).max(0.00000001)),
        MartingaleDirection::Short => Ok(last_layer_price + distance),
    }
}
```

- [ ] **Step 3: Add stop boundary tests**

Add tests:

```rust
#[test]
fn layer_plus_atr_long_stop_is_beyond_last_layer() {
    let stop = layer_plus_atr_stop_price(
        MartingaleDirection::Long,
        90.0,
        0.01,
        Some(0.2),
        &LayerPlusAtrStopConfig { extra_stop_spacing_multiplier: 2.0, atr_stop_multiplier: 3.0 },
    ).unwrap();
    assert!(stop < 90.0);
    assert!(stop <= 88.2);
}

#[test]
fn layer_plus_atr_short_stop_is_beyond_last_layer() {
    let stop = layer_plus_atr_stop_price(
        MartingaleDirection::Short,
        110.0,
        0.01,
        Some(0.2),
        &LayerPlusAtrStopConfig { extra_stop_spacing_multiplier: 2.0, atr_stop_multiplier: 3.0 },
    ).unwrap();
    assert!(stop > 110.0);
    assert!(stop >= 112.2);
}
```

- [ ] **Step 4: Wire helper into trade simulation**

In `trade_engine.rs` or `kline_engine.rs`, replace any stop that can trigger before final martingale layer with `layer_plus_atr_stop_price(...)`. Preserve trailing take-profit behavior so trailing drawdown activates only after take-profit threshold has been reached.

- [ ] **Step 5: Run stop-loss tests**

Run:

```bash
cargo test -p backtest-engine layer_plus_atr -- --nocapture
cargo test -p backtest-engine trailing -- --nocapture
```

Expected: stop tests pass; trailing tests confirm no pre-TP stop behavior.

---

### Task 4: Score and Recommendation Semantics

**Files:**
- Modify: `apps/backtest-engine/src/martingale/scoring.rs`
- Modify: `apps/backtest-engine/src/martingale/metrics.rs`
- Modify: `apps/backtest-worker/src/main.rs`
- Test: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Replace display score with 0..100 score**

In `scoring.rs`, keep internal sorting stable if needed, but expose a `display_score` or make `raw_score` bounded `0.0..100.0` using the spec weights:

```rust
let annualized_component = normalize_positive(metrics.annualized_return_pct.unwrap_or(metrics.total_return_pct), 50.0) * 35.0;
let drawdown_component = normalize_inverse(drawdown, config.max_global_drawdown_pct.max(1.0)) * 30.0;
let stop_component = normalize_inverse(stop_frequency * 100.0, 10.0) * 15.0;
let cost_pct = if metrics.total_return_quote.abs() > 0.0 {
    ((metrics.fee_quote + metrics.slippage_quote + metrics.stop_loss_cost_quote).abs() / metrics.max_capital_used_quote.max(1.0)) * 100.0
} else {
    100.0
};
let cost_component = normalize_inverse(cost_pct, 10.0) * 10.0;
let stability_component = trade_stability * 10.0;
let display_score = (annualized_component + drawdown_component + stop_component + cost_component + stability_component).clamp(0.0, 100.0);
```

If current `MartingaleMetrics` uses different field names, map to existing equivalents and add missing optional fields only when necessary.

- [ ] **Step 2: Penalize negative returns and drawdown failures**

Ensure:

```rust
if metrics.annualized_return_pct.unwrap_or(metrics.total_return_pct) < 0.0 {
    display_score = display_score.min(35.0);
    push_reason(&mut reasons, "negative_return");
}
if global_drawdown_pct > config.max_global_drawdown_pct {
    display_score = display_score.min(50.0);
    push_reason(&mut reasons, "global_drawdown_exceeded");
}
```

- [ ] **Step 3: Set live recommendation flags in worker summary**

In `apps/backtest-worker/src/main.rs`, when building candidate summary, set:

```rust
let target_annualized_return_passed = annualized_return_pct >= target_annualized_return_pct;
let max_drawdown_limit_passed = max_drawdown_pct <= max_drawdown_limit_pct;
let can_recommend_live = survival_valid
    && target_annualized_return_passed
    && max_drawdown_limit_passed
    && annualized_return_pct > 0.0;
```

If product decision allows low-return but drawdown-compliant candidates to be manually reviewed, keep `can_recommend_live=false` and show `warning_reason="target_annualized_return_not_met"`.

- [ ] **Step 4: Add scoring tests**

Add tests proving:

```rust
#[test]
fn score_is_bounded_to_zero_one_hundred() { /* build metrics with extreme values and assert 0 <= score <= 100 */ }

#[test]
fn negative_return_candidate_is_not_live_recommended() { /* build negative annualized return result and assert reason contains negative_return */ }

#[test]
fn drawdown_failure_keeps_fallback_but_blocks_live_recommendation() { /* drawdown > limit => max_drawdown_limit_passed false */ }
```

- [ ] **Step 5: Run scoring tests**

Run:

```bash
cargo test -p backtest-engine score -- --nocapture
cargo test -p backtest-worker -- --nocapture
```

Expected: scores are bounded and summary flags are correct.

---

### Task 5: API Validation and Publish Guard

**Files:**
- Modify: `apps/api-server/src/routes/backtest.rs`
- Modify: `apps/api-server/src/services/backtest_service.rs`
- Modify: `apps/api-server/src/services/martingale_publish_service.rs`
- Modify: `apps/api-server/tests/martingale_backtest_flow.rs`

- [ ] **Step 1: Add request validation**

In `routes/backtest.rs` or service-level request normalization, enforce:

```rust
if request.direction == "long_and_short" {
    validate_long_short_weights(request.long_weight_pct, request.short_weight_pct)?;
}
if request.max_drawdown_limit_pct <= 0.0 || request.max_drawdown_limit_pct > 100.0 {
    return Err(ApiError::bad_request("max_drawdown_limit_pct must be between 0 and 100"));
}
if request.dynamic_allocation_enabled.unwrap_or(false) {
    return Err(ApiError::bad_request("dynamic allocation is disabled for this revision"));
}
```

Use the project’s existing error type and request structs instead of inventing a new one.

- [ ] **Step 2: Add API default normalization**

When fields are omitted, normalize:

```rust
allocation_mode = "fixed_by_risk_profile";
dynamic_allocation_enabled = false;
(long_weight_pct, short_weight_pct) = risk_profile_default_weights(risk_profile);
target_annualized_return_pct = 50.0;
```

- [ ] **Step 3: Block unsafe publish**

In `martingale_publish_service.rs`, before creating a live portfolio from a candidate summary, reject when:

```rust
if !summary.can_recommend_live || !summary.max_drawdown_limit_passed {
    return Err(ServiceError::bad_request("candidate is not recommended for live publishing because risk checks failed"));
}
```

Keep a future extension point for explicit manual override, but do not implement override this round.

- [ ] **Step 4: Add API flow tests**

In `apps/api-server/tests/martingale_backtest_flow.rs`, add tests for:

```rust
#[tokio::test]
async fn create_backtest_rejects_invalid_long_short_weights() { /* 70/20 returns 400 */ }

#[tokio::test]
async fn create_backtest_defaults_fixed_allocation_by_risk_profile() { /* conservative returns/stores 80/20 and dynamic false */ }

#[tokio::test]
async fn publish_rejects_candidate_that_failed_drawdown_limit() { /* summary max_drawdown_limit_passed=false => 400 */ }
```

- [ ] **Step 5: Run API tests**

Run:

```bash
cargo test -p api-server --test martingale_backtest_flow -- --nocapture
```

Expected: all martingale backtest flow tests pass.

---

### Task 6: Frontend Wizard and Results UX

**Files:**
- Modify: `apps/web/components/backtest/backtest-wizard.tsx`
- Modify: `apps/web/components/backtest/search-config-editor.tsx`
- Modify: `apps/web/components/backtest/backtest-result-table.tsx`
- Modify: `apps/web/components/backtest/portfolio-candidate-review.tsx`
- Modify: `apps/web/components/backtest/martingale-risk-warning.tsx`

- [ ] **Step 1: Add fixed default mapping**

In `backtest-wizard.tsx`, add a single mapping:

```ts
const RISK_PROFILE_DEFAULTS = {
  conservative: { longWeightPct: 80, shortWeightPct: 20, maxDrawdownLimitPct: 20 },
  balanced: { longWeightPct: 60, shortWeightPct: 40, maxDrawdownLimitPct: 25 },
  aggressive: { longWeightPct: 50, shortWeightPct: 50, maxDrawdownLimitPct: 30 },
} as const;
```

Track whether the user manually edited allocation/drawdown so risk-profile changes do not overwrite manual input.

- [ ] **Step 2: Add visible long/short controls**

Only for `direction === "long_and_short"`, render two numeric inputs or sliders:

```tsx
<label>Long 资金比例</label>
<input value={longWeightPct} onChange={...} />
<label>Short 资金比例</label>
<input value={shortWeightPct} onChange={...} />
```

Show validation text when total is not 100:

```tsx
{Math.abs(longWeightPct + shortWeightPct - 100) > 0.001 ? (
  <p role="alert">Long 与 Short 比例合计必须等于 100%</p>
) : null}
```

Disable submit while invalid.

- [ ] **Step 3: Send fixed allocation payload**

Ensure create payload contains:

```ts
allocation_mode: "fixed_by_risk_profile",
dynamic_allocation_enabled: false,
long_weight_pct: longWeightPct,
short_weight_pct: shortWeightPct,
target_annualized_return_pct: 50,
stop_model: {
  kind: "layer_plus_atr",
  extra_stop_spacing_multipliers: [1, 1.5, 2, 2.5, 3],
  atr_stop_multipliers: [2, 2.5, 3, 3.5, 4],
},
```

- [ ] **Step 4: Update result table columns**

In `backtest-result-table.tsx`, show columns/cards for:

```ts
score
annualized_return_pct
max_drawdown_pct
long_weight_pct
short_weight_pct
actual_long_weight_pct
actual_short_weight_pct
stop_loss_count
stop_loss_cost_quote
fee_quote
slippage_quote
forced_exit_count
total_trades
can_recommend_live
```

Format score as `score.toFixed(1) + "/100"`.

- [ ] **Step 5: Update risk warnings**

In `martingale-risk-warning.tsx`, render explicit human-readable warnings for:

```ts
annualized_return_pct < 0
!max_drawdown_limit_passed
stop_loss_count high or warning_reason includes stop
!target_annualized_return_passed
!can_recommend_live
```

Use Chinese copy matching the spec:

```tsx
"该候选收益为负，不建议实盘"
"该候选超过最大回撤限制，不建议实盘"
"该候选止损频率过高，可能被手续费和滑点磨损"
"未找到同时满足年化目标和回撤限制的组合"
```

- [ ] **Step 6: Run frontend contracts/build**

Run:

```bash
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
npm run build --workspace apps/web
```

Expected: contract test passes; web build passes. If workspace script name differs, use the existing repo build command from `package.json`.

---

### Task 7: Full Verification and Real Smoke Backtest

**Files:**
- No required code files unless verification reveals a bug.

- [ ] **Step 1: Run focused verification**

Run:

```bash
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
node tests/verification/martingale_portfolio_contract.test.mjs
cargo test -p api-server --test martingale_backtest_flow -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p backtest-engine -- --nocapture
```

Expected: all pass. Existing unrelated warnings may remain, but no failures.

- [ ] **Step 2: Run BTCUSDT + ETHUSDT conservative smoke backtest**

Create or use the existing API/worker entrypoint to run:

- symbols: `BTCUSDT`, `ETHUSDT`
- market: USDT-M futures
- direction: `long_and_short`
- risk profile: `conservative`
- max drawdown limit: `20%`
- target annualized return: `50%`
- allocation: fixed `80/20`
- dynamic allocation: disabled
- date range: `2023-01-01` to last month end

Expected result:

- Task completes or gives a clear failure reason.
- Result contains Top10 or fallback Top10.
- Each candidate has annualized return, max drawdown, score `/100`, stop count, fee, slippage, live recommendation state.
- If all returns are negative, result explains whether losses come from stops, costs, short side, leverage, or drawdown failures.

- [ ] **Step 3: Restart only project frontend if needed**

If UI changes need local verification, restart only this project’s frontend on port `8080`. Do not touch unrelated `3000` service.

Use existing repo scripts/process notes; verify with:

```bash
ss -ltnp | rg ':8080'
curl -sS http://127.0.0.1:8080/ | head
```

- [ ] **Step 4: Final status and commit**

Check:

```bash
git status --short
git diff --stat
```

Commit with a message containing problem/fix approach, for example:

```bash
git add docs/superpowers/specs/2026-05-15-martingale-fixed-allocation-stoploss-revision-design.md docs/superpowers/plans/2026-05-15-martingale-fixed-allocation-stoploss-revision-plan.md apps/backtest-engine apps/backtest-worker apps/api-server apps/web tests/verification
git commit -m "fix: stabilize martingale fixed allocation backtests" -m "Problem: dynamic long/short allocation produced negative, hard-to-explain martingale backtest results. Fix: disable dynamic allocation by default, use fixed risk-profile weights, recalculate layer+ATR stops, normalize score to 100, and block unsafe live publish."
```

Push only after user confirms remote target, unless this session already has explicit push permission.

---

## Self-Review

- Spec coverage: fixed allocation defaults, manual validation, stop model, drawdown priority, 100-point score, result explanation, publish guard, and BTC/ETH smoke verification are all mapped to tasks.
- Placeholder scan: no `TBD`, no generic “add tests” without concrete expected behavior.
- Type consistency: uses `dynamic_allocation_enabled`, `allocation_mode`, `long_weight_pct`, `short_weight_pct`, `max_drawdown_limit_pct`, `target_annualized_return_pct`, `can_recommend_live`, `max_drawdown_limit_passed` consistently across API, worker, web and tests.
