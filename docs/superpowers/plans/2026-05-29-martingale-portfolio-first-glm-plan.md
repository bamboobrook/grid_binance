# Martingale Portfolio-First GLM Fix Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 从“单策略先达标”改为“组合先达标”：允许单个策略高收益高回撤或低收益低回撤，只要组合后满足保守/平衡/激进回撤限制，并尽力寻找年化超过 50% 的真实可发布组合。

**Architecture:** 保留单策略作为候选生成器，但组合器不再只从“单策略已满足回撤”的结果中选最优，而是构建多层候选池：高收益候选、低回撤稳定器、低相关候选、方向互补候选。组合层使用真实全量 1m equity curve 做权重搜索，直接以组合年化和组合最大回撤为目标函数，输出组合 Top3。

**Tech Stack:** Rust `backtest-engine`, Rust `backtest-worker`, PostgreSQL `backtest_tasks`, Docker Compose backtest workers, local market data SQLite `/home/bumblebee/Project/discord_c2im/pipeline/data/market_data.db`.

---

## Non-Negotiable Requirements

1. FlyingKid 旧任务已经由 Codex 删除：`DELETE 14`。GLM 不需要再删，除非新任务又产生脏数据。
2. 必须使用真实 1m K 线：`2023-01-01 00:00:00 UTC` 到上个月月底。
3. 成本必须保留：Binance futures fee `4.5 bps`，slippage `2.0 bps`。
4. 不允许通过缩短回测区间、移除成本、抽样最终曲线来制造高收益。
5. 目标是组合达标，不要求单策略达标：
   - 保守：组合最大回撤 `<= 10%`
   - 平衡：组合最大回撤 `<= 20%`
   - 激进：组合最大回撤 `<= 30%`
   - 年化目标：优先寻找 `>= 50%`
6. 初始阶段只跑 **7 币种 balanced mixed_best**，不要立即跑 7/18 × 3 档全矩阵。

---

## Diagnosis: Why Current Search Likely Fails

1. **候选池仍偏向单策略达标。**
   - `select_portfolio_pool_outputs_v2()` 虽然有 high-return tier，但 `portfolio_pool_quality_eligible()` 仍会过滤掉一批高收益高回撤候选。
   - 如果高收益候选被过滤，组合器没有足够“进攻资产”可组合。

2. **组合器排序仍可能过度依赖单策略分数。**
   - `build_ranked_portfolios_v2()` 使用 `portfolio_seed_indices_by_symbol(eligible, 12, 8, 8)`，入口 already biased by score/low-dd/high-return，但组合目标不是直接最大化组合收益/回撤比。

3. **组合枚举权重不够像真实组合优化。**
   - 当前有模板、barbell、stochastic，但还不够明确地搜索：
     - 高收益高回撤策略低权重 + 多个低回撤/低相关稳定器；
     - long 与 short 或不同币种之间低相关组合；
     - 在硬回撤限制内最大化年化。

4. **之前曲线完整性有问题。**
   - 必须先确认 `run_candidate_kline_screening()` 最终精测用全量 K 线；
   - `portfolio_candidates_from_outputs()` 用全量 `output.equity_curve`；
   - `combine_equity_curves()` 按 timestamp 对齐。

---

## Task 1: Verify Full-Curve Foundation First

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `apps/backtest-engine/src/portfolio_search.rs`

- [ ] **Step 1: Verify final refinement uses full bars**

Check `apps/backtest-worker/src/main.rs`:

```rust
fn run_candidate_kline_screening(
    candidate: &SearchCandidate,
    market_context: &MarketDataContext,
) -> Result<MartingaleBacktestResult, String> {
    let bars = bars_for_candidate(candidate, &market_context.bars);
    if bars.is_empty() {
        return Err(format!(
            "candidate {} has no matching kline bars",
            candidate.candidate_id
        ));
    }
    run_kline_screening(candidate.config.clone(), &bars)
}
```

If it uses `screening_bars_for_candidate()`, fix it.

- [ ] **Step 2: Verify final full-bar test passes**

Run:

```bash
cargo test -p backtest-worker final_kline_refinement_uses_full_symbol_history_not_screening_sample -- --nocapture
```

Expected: PASS.

- [ ] **Step 3: Verify portfolio curves align by timestamp**

Run:

```bash
cargo test -p backtest-engine weighted_portfolio_aligns_member_equity_by_timestamp_not_index -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Verify candidate conversion uses full output curve**

Run:

```bash
cargo test -p backtest-worker portfolio_candidates_prefer_full_output_curve_over_summary_preview -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Deploy worker after full-curve fixes**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml build backtest-worker

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml up -d --scale backtest-worker=4 backtest-worker
```

---

## Task 2: Change Candidate Pool To Portfolio-First Admission

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Tests: same file

### Desired Behavior

The portfolio candidate pool must include:

1. **Qualified candidates:** positive return and max drawdown within risk limit.
2. **Growth candidates:** high annualized return, even if single-strategy drawdown exceeds risk limit.
3. **Stabilizer candidates:** very low drawdown, even if return is modest.
4. **Low-correlation candidates:** candidates whose daily returns diversify existing leaders.
5. **Direction-diverse candidates:** long_only, short_only, long_short if profitable enough.

Single strategy drawdown may exceed target, but candidate must not be obviously unusable:

```text
annualized > 0
trade_count >= 5
max_drawdown <= 65% for balanced/aggressive search pool
max_drawdown <= 45% for conservative search pool
```

Final portfolio still enforces hard drawdown target.

- [ ] **Step 1: Add test for high-return/high-drawdown candidate admission**

Add test in `apps/backtest-worker/src/main.rs`:

```rust
#[test]
fn portfolio_pool_admits_growth_candidates_above_single_strategy_drawdown_limit() {
    let outputs = vec![
        candidate_output_fixture("btc-growth", "BTCUSDT", 180.0, 48.0, 100.0),
        candidate_output_fixture("eth-stable", "ETHUSDT", 18.0, 4.0, 100.0),
        candidate_output_fixture("sol-balanced", "SOLUSDT", 55.0, 18.0, 100.0),
    ];

    let pool = select_portfolio_pool_outputs_v2(outputs, 20.0, 10, 10, 10);
    let ids = pool.iter().map(|o| o.candidate_id.as_str()).collect::<std::collections::BTreeSet<_>>();

    assert!(ids.contains("btc-growth"), "growth candidate must remain available for low-weight portfolio allocation");
    assert!(ids.contains("eth-stable"), "stabilizer candidate must remain available");
    assert!(ids.contains("sol-balanced"), "qualified candidate must remain available");
}
```

- [ ] **Step 2: Update `portfolio_pool_quality_eligible()`**

Use this logic:

```rust
fn portfolio_pool_quality_eligible(output: &CandidateOutput, drawdown_limit_pct: f64) -> bool {
    if output.total_return_pct <= 0.0 || output.equity_curve.is_empty() || output.trade_count < 5 {
        return false;
    }
    let annualized = output.annualized_return_pct.unwrap_or(output.total_return_pct);
    if annualized <= 0.0 || !annualized.is_finite() || !output.max_drawdown_pct.is_finite() {
        return false;
    }

    if output.max_drawdown_pct <= drawdown_limit_pct {
        return true;
    }

    let hard_candidate_dd_cap = if drawdown_limit_pct <= 10.0 { 45.0 } else { 65.0 };
    if output.max_drawdown_pct > hard_candidate_dd_cap {
        return false;
    }

    let calmar = annualized / output.max_drawdown_pct.max(1.0);
    let high_growth = annualized >= 35.0 && calmar >= 0.75;
    let exceptional_growth = annualized >= 60.0 && calmar >= 0.55;
    let useful_stabilizer = output.max_drawdown_pct <= drawdown_limit_pct * 1.5 && annualized >= 5.0;

    high_growth || exceptional_growth || useful_stabilizer
}
```

- [ ] **Step 3: Run test**

```bash
cargo test -p backtest-worker portfolio_pool_admits_growth_candidates_above_single_strategy_drawdown_limit -- --nocapture
```

Expected: PASS.

---

## Task 3: Add Portfolio-First Optimizer Mode

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`

### Desired Behavior

Portfolio search must directly optimize:

```text
maximize annualized_return_pct
subject to max_drawdown_pct <= risk limit
secondary: maximize annualized / max_drawdown
secondary: lower correlation
secondary: reasonable diversification
```

It must be able to build a valid portfolio from:

```text
1-3 growth leaders with limited weight
+ 3-9 stabilizers/diversifiers
```

- [ ] **Step 1: Add test for portfolio combining high growth and stabilizers**

Add or update test in `apps/backtest-engine/src/portfolio_search.rs`:

```rust
#[test]
fn portfolio_v2_uses_low_weight_growth_leader_with_stabilizers_to_hit_drawdown_limit() {
    let growth = candidate_with_curve(
        "growth-high-dd",
        "BTCUSDT",
        220.0,
        55.0,
        50.0,
        100.0,
        vec![100.0, 160.0, 108.0, 210.0, 320.0],
    );
    let stabilizers = (0..8).map(|index| {
        candidate_with_curve(
            &format!("stable-{index}"),
            &["ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "LINKUSDT", "AVAXUSDT"][index],
            24.0 + index as f64,
            4.0 + (index % 3) as f64,
            12.0,
            100.0,
            vec![100.0, 103.0, 108.0, 116.0, 128.0 + index as f64],
        )
    });
    let candidates = std::iter::once(growth).chain(stabilizers).collect::<Vec<_>>();

    let artifact = build_portfolio_top_n_v2(&candidates, 20.0, 3);
    let first = artifact.top3.first().expect("portfolio expected");

    assert!(first.max_drawdown_pct <= 20.0, "portfolio must obey hard DD limit: {first:?}");
    assert!(first.members.iter().any(|m| m.candidate_id == "growth-high-dd"), "growth leader should be included at low weight");
    assert!(first.annualized_return_pct.unwrap_or(first.return_pct) >= 35.0, "portfolio should keep meaningful yield");
}
```

- [ ] **Step 2: Add allocation templates that support low-weight leaders**

In `allocation_templates_for_member_count()`, for `member_count >= 5`, include templates like:

```rust
let mut low_leader = vec![0.08];
low_leader.extend(std::iter::repeat(0.92 / (member_count - 1) as f64).take(member_count - 1));
templates.push(low_leader);

let mut medium_leader = vec![0.12];
medium_leader.extend(std::iter::repeat(0.88 / (member_count - 1) as f64).take(member_count - 1));
templates.push(medium_leader);

let mut two_growth = vec![0.10, 0.08];
two_growth.extend(std::iter::repeat(0.82 / (member_count - 2) as f64).take(member_count - 2));
templates.push(two_growth);
```

- [ ] **Step 3: Improve ranking to portfolio-first**

Update `sort_portfolios_by_yield_then_risk()` so portfolios under hard DD limit are sorted by:

```rust
let a_ann = a.annualized_return_pct.unwrap_or(a.return_pct);
let b_ann = b.annualized_return_pct.unwrap_or(b.return_pct);
let a_ratio = a_ann / a.max_drawdown_pct.max(1.0);
let b_ratio = b_ann / b.max_drawdown_pct.max(1.0);

b_ann.total_cmp(&a_ann)
    .then_with(|| b_ratio.total_cmp(&a_ratio))
    .then_with(|| a.max_drawdown_pct.total_cmp(&b.max_drawdown_pct))
```

Do not require every member to be individually low drawdown.

- [ ] **Step 4: Run portfolio tests**

```bash
cargo test -p backtest-engine portfolio_v2_uses_low_weight_growth_leader_with_stabilizers_to_hit_drawdown_limit -- --nocapture
cargo test -p backtest-engine portfolio_search::tests::portfolio_v2 -- --nocapture
```

Expected: PASS.

---

## Task 4: Fast Validation Before Deep Runs

**Runtime only.**

Use only 7 symbols and balanced risk first.

- [ ] **Step 1: Create smoke task**

Task ID:

```text
glm-7-balanced-portfolio-first-smoke-20260529
```

Config:

```json
{
  "symbols": ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT"],
  "risk_profile": "balanced",
  "direction_mode": "mixed_best",
  "search_mode": "profit_optimized_v2",
  "market": "usd_m_futures",
  "margin_mode": "isolated",
  "leverage_range": [1, 20],
  "random_candidates": 96,
  "per_symbol_top_n": 20,
  "portfolio_top_n": 3,
  "fee_bps": 4.5,
  "slippage_bps": 2.0,
  "random_seed": 17,
  "start_ms": 1672531200000,
  "end_ms": 1777593599999
}
```

- [ ] **Step 2: Verify curve and portfolio**

After completion, run curve gap SQL. Result is invalid if `max_gap_days` is hundreds of days.

- [ ] **Step 3: If smoke annualized < 35%, inspect candidate pool before deep run**

Query candidate pool count and top candidates:

```sql
SELECT candidate_id,
       summary->>'symbol' symbol,
       summary->>'direction_mode' direction_mode,
       round((summary->>'annualized_return_pct')::numeric,2) ann,
       round((summary->>'max_drawdown_pct')::numeric,2) dd,
       summary->>'trade_count' trades,
       summary->>'max_leverage_used' lev
FROM backtest_candidate_summaries
WHERE task_id='glm-7-balanced-portfolio-first-smoke-20260529'
ORDER BY (summary->>'annualized_return_pct')::numeric DESC NULLS LAST
LIMIT 30;
```

If there are not enough high-return candidates, widen search space before deep run.

---

## Task 5: Deep Search Strategy For 50% Annualized Target

Only run after smoke proves full curve and candidate pool is meaningful.

- [ ] **Step 1: Run multiple seeds medium budget**

Task IDs:

```text
glm-7-balanced-portfolio-first-seed17-20260529
glm-7-balanced-portfolio-first-seed29-20260529
glm-7-balanced-portfolio-first-seed43-20260529
glm-7-balanced-portfolio-first-seed71-20260529
glm-7-balanced-portfolio-first-seed101-20260529
```

Config:

```json
{
  "random_candidates": 192,
  "per_symbol_top_n": 30,
  "portfolio_top_n": 3,
  "risk_profile": "balanced",
  "direction_mode": "mixed_best",
  "fee_bps": 4.5,
  "slippage_bps": 2.0
}
```

- [ ] **Step 2: If best < 45%, widen parameter space**

In `apps/backtest-engine/src/search.rs`, widen `profit_optimized_v2()`:

```rust
space.leverage = vec![2, 3, 4, 5, 6, 8, 10, 12, 15, 20];
space.spacing_bps = vec![25, 35, 50, 70, 90, 120, 160, 220, 300, 420, 600, 800];
space.order_multiplier = vec![1.1, 1.15, 1.25, 1.4, 1.6, 1.8, 2.0, 2.2, 2.4, 2.8];
space.max_legs = vec![3, 4, 5, 6, 7, 8, 9, 10];
space.take_profit_bps = vec![25, 30, 45, 60, 80, 100, 140, 200, 300, 450];
space.tail_stop_bps = vec![600, 800, 1200, 1800, 2400, 3000, 4000, 5500, 7000, 9000];
space.long_short_weight_pct = vec![(90,10),(80,20),(70,30),(60,40),(50,50),(40,60),(30,70),(20,80),(10,90)];
```

- [ ] **Step 3: Run one deep task with best seed**

Task ID:

```text
glm-7-balanced-portfolio-first-deep-best-20260529
```

Config:

```json
{
  "random_candidates": 512,
  "per_symbol_top_n": 40,
  "portfolio_top_n": 3,
  "risk_profile": "balanced",
  "direction_mode": "mixed_best",
  "fee_bps": 4.5,
  "slippage_bps": 2.0
}
```

- [ ] **Step 4: Report honestly**

Report table:

```text
Task ID | Annualized % | Max DD % | Members | Unique Symbols | Curve Max Gap Days | Candidate Pool Count | Status
```

Best portfolio members:

```text
Symbol | Direction | Allocation % | Leverage | Return % | Annualized % | Max DD % | Trades
```

Conclusion:

```text
Reached >=50% annualized under <=20% DD: Yes/No
If No: best real annualized/DD and what blocks it.
Next recommended expansion: 18-symbol balanced or more seeds/space.
```

---

## Task 6: Only After 7-Balanced, Expand Matrix

Do not start until 7-balanced has a validated full-curve best result.

Order:

1. 18-symbol balanced with same portfolio-first optimizer.
2. 7-symbol conservative/aggressive.
3. 18-symbol conservative/aggressive.

Never run all six at once before validating the 7-balanced foundation.

---

## External Research Summary To Guide Design

General trading-system portfolio research supports the following principles:

1. Combining strategies can reduce drawdown when return streams are not perfectly correlated.
2. Walk-forward and multi-seed validation reduce overfitting risk.
3. Position sizing/portfolio allocation often matters more than finding a single perfect strategy.
4. Martingale/grid strategies need strict tail-risk controls because rare trending moves dominate losses.
5. A high-return/high-drawdown strategy can be useful at low portfolio weight if paired with low-correlated stabilizers.

Apply these principles in the optimizer. Do not discard all high-drawdown single strategies before portfolio construction.
