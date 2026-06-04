# Martingale Speed Accuracy Profit Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:systematic-debugging for accuracy checks, superpowers:writing-plans before code changes, and superpowers:executing-plans for implementation. Do not start long deep-search jobs until cleanup, baseline preservation, and accuracy gates are complete.

**Goal:** 清理 FlyingKid 回测结果，只保留 18 币种 balanced 最佳结果；在不降低回测准确性的前提下提升回测速度，并继续寻找低回撤、高年化的马丁组合策略。

**Architecture:** 先做数据保全与清理，再建立准确性验收门禁；之后分析 CPU/GPU/内存可利用的性能瓶颈，优化并发、缓存、数据读取、候选筛选和组合搜索；最后在保守/平衡/激进三档重新进行收益最大化搜索，并输出可复核的结果表。

**Tech Stack:** Rust `backtest-engine`, Rust `backtest-worker`, PostgreSQL `backtest_tasks`, Docker Compose, local SQLite market data `/home/bumblebee/Project/discord_c2im/pipeline/data/market_data.db`, hardware target AMD 9950X CPU, RTX 5090 GPU, 200G RAM.

---

## Final Target

最终目标必须明确：

1. **低回撤优先，高收益次之，但最终排名要同时看收益/回撤效率。**
2. 在真实全量 1m K 线、真实手续费、真实滑点下，分别寻找：
   - conservative：最大回撤 `<=10%`，尽量年化 `>50%`；
   - balanced：最大回撤 `<=20%`，年化越高越好；
   - aggressive：最大回撤 `<=30%`，年化越高越好。
3. 不能通过移除手续费/滑点、缩短时间、抽样最终曲线、忽略止盈止损来制造收益。
4. 回测逻辑必须能解释并验证：止盈、止损、加仓、强平/尾部止损、手续费、滑点、杠杆本金计算都正确执行。
5. 后续若推实盘，在行情路径相同、成交假设相同的情况下，回测的低回撤应具备可复现性。

---

## Task 1: Preserve Best FlyingKid 18-Balanced Result And Clean Others

**Files:**
- Runtime DB only, unless API delete endpoint is available and safer.

- [ ] **Step 1: Find FlyingKid completed tasks**

Query all `flyingkid2022@outlook.com` tasks and summarize `risk_profile`, universe size, status, best portfolio annualized/DD.

- [ ] **Step 2: Identify best 18-symbol balanced result**

Criteria:

1. owner = `flyingkid2022@outlook.com`
2. risk = `balanced`
3. universe is 18 symbols or expanded-universe task
4. status = `succeeded`
5. portfolio Top3 exists
6. choose rank 1 by highest annualized return under DD `<=20%`; tie-break by lower DD, then more complete curve.

- [ ] **Step 3: Back up the keep task metadata**

Before deletion, export keep task summary and member table to a local artifact, e.g. `/tmp/flyingkid-keep-18-balanced-<task_id>.json`.

- [ ] **Step 4: Delete other FlyingKid backtest tasks**

Delete all other FlyingKid backtest tasks except the selected keep task. Because FK constraints cascade candidates/artifacts, verify only one FlyingKid task remains.

- [ ] **Step 5: Report keep task**

Report:

```text
Kept Task ID | Annualized % | Max DD % | Return % | Members | Unique Symbols | Max Symbol Weight % | Eq Points | Max Gap Days
```

---

## Task 2: Backtest Accuracy Audit

**Files:**
- Inspect/modify only if a bug is proven:
  - `apps/backtest-engine/src/martingale/**`
  - `apps/backtest-engine/src/search.rs`
  - `apps/backtest-worker/src/main.rs`
  - existing tests under `apps/backtest-engine/tests/**`

- [ ] **Step 1: Trace order lifecycle**

Trace exact code path for:

1. initial order
2. add-position order
3. take-profit trigger
4. stop-loss/tail-stop trigger
5. long_short simultaneous legs
6. fee/slippage deduction
7. leverage and margin calculation
8. drawdown curve calculation

- [ ] **Step 2: Add or confirm deterministic unit tests**

Tests must cover:

- price hits take-profit exactly -> position closes and PnL includes fee/slippage;
- price hits stop-loss before next add -> stop-loss closes and DD updates;
- price hits add-level then take-profit -> average entry and realized PnL correct;
- long_short has independent long and short TP/SL logic;
- leverage affects position notional but return/DD denominator uses actual allocated margin/principal correctly;
- combined portfolio DD is computed from combined equity curve, not average member DD.

- [ ] **Step 3: Compare synthetic hand-calculated cases**

Create tiny 1m synthetic price path and manually compute expected trades/equity. Test engine output must match within tolerance.

- [ ] **Step 4: Verify real task curves**

For kept FlyingKid 18-balanced task, verify:

- equity curve first timestamp = `2023-01-01`;
- last timestamp = previous month end;
- max gap `< 1 day` for preview curve;
- drawdown curve exists and aligns with equity timestamps;
- trades preview/artifact exists;
- all members have leverage, direction, allocation, candidate ID.

---

## Task 3: Performance Profiling And Speed Plan

**Files:**
- Likely inspect/modify:
  - `apps/backtest-worker/src/main.rs`
  - `apps/backtest-engine/src/sqlite_market_data.rs`
  - `apps/backtest-engine/src/search.rs`
  - `apps/backtest-engine/src/portfolio_search.rs`

- [ ] **Step 1: Measure before optimizing**

Profile a representative 7-symbol and 18-symbol task:

- total wall time;
- per-symbol candidate generation time;
- kline loading time;
- candidate evaluation time;
- final refinement time;
- portfolio combination time;
- CPU utilization;
- memory utilization;
- worker concurrency;
- DB/SQLite IO wait.

- [ ] **Step 2: Use 9950X CPU effectively**

Evaluate and implement safe improvements:

- increase worker count only if CPU/IO can sustain it;
- parallelize per-symbol and per-candidate evaluation with bounded rayon/thread pools;
- avoid oversubscription: total active compute threads should not exceed practical 9950X capacity;
- keep one task from starving all workers if many tasks are queued.

- [ ] **Step 3: Use 200G RAM effectively**

Evaluate and implement safe improvements:

- cache 1m kline arrays per symbol/date range in worker memory;
- avoid repeatedly decoding the same SQLite rows per candidate;
- keep data immutable and shared by reference/Arc;
- precompute common arrays such as close/high/low/timestamp for faster loops.

- [ ] **Step 4: Evaluate RTX 5090 GPU realistically**

Research whether current Rust stack can benefit from GPU. Do not force GPU if transfer overhead or rewrite risk is too high.

Potential GPU-suitable work:

- massive independent candidate scoring kernels;
- parameter grid batch evaluation;
- vectorized equity path simulation.

GPU is optional. CPU/RAM optimization is priority unless GPU proof-of-concept shows clear speedup without changing accuracy.

- [ ] **Step 5: Keep accuracy unchanged**

Every performance optimization must run accuracy regression tests. Results for a fixed seed/config must match previous output within an explicitly documented tolerance.

---

## Task 4: External Research For Better Martingale/Grid Strategy Ideas

**Files:**
- Create research note if useful:
  - `docs/superpowers/research/2026-06-01-martingale-grid-optimization-research.md`

- [ ] **Step 1: Search external sources**

Research recent and practical sources on crypto martingale/grid strategy optimization, including:

- volatility-adaptive grid spacing;
- ATR/Bollinger/realized-volatility based spacing;
- trend filters to disable adverse direction;
- regime detection;
- dynamic TP/SL based on volatility;
- portfolio allocation and correlation controls;
- drawdown-constrained optimization;
- walk-forward validation to reduce overfitting.

- [ ] **Step 2: Convert research to testable parameters**

Do not just summarize. Convert ideas into parameter families that can be backtested honestly, e.g.:

- ATR-scaled spacing ranges;
- volatility percentile filters;
- trend regime on/off rules;
- max stop-loss frequency constraints;
- low-correlation portfolio scoring;
- rolling-window validation score.

---

## Task 5: Profit Maximization Search

**Files:**
- Modify only after Tasks 2-4 establish safe improvements.

- [ ] **Step 1: Define ranking function**

Ranking must prefer:

1. satisfies risk DD limit;
2. higher annualized return;
3. lower max drawdown;
4. higher annualized/DD ratio;
5. smoother equity curve;
6. lower stop-loss frequency and lower churn when returns are similar.

- [ ] **Step 2: Expand parameter search safely**

Search more combinations only after speed improvements or batching are in place. Keep all costs and full 1m data.

Parameters to investigate:

- leverage range;
- spacing bps;
- order multiplier;
- max legs;
- take profit bps;
- stop/tail stop bps;
- long/short weights;
- volatility-adaptive spacing;
- symbol allocation weights;
- portfolio member count and max symbol weight.

- [ ] **Step 3: Run tiered search**

Order:

1. 18-symbol balanced improvement over kept baseline;
2. 18-symbol conservative target `>50%` annualized and `<=10%` DD;
3. 7-symbol conservative as comparison;
4. aggressive only after conservative/balanced improvements.

- [ ] **Step 4: Stop only when target is met or bottleneck is proven**

Do not stop after one mediocre result. If target is not met, report bottleneck with evidence:

- insufficient high-return candidates;
- stop-loss too frequent;
- portfolio optimizer too conservative;
- parameter space too narrow;
- true market data under constraints cannot support target.

---

## Final Report Required

Report must include:

```text
1. FlyingKid cleanup result and kept task ID
2. Accuracy audit result: TP/SL/add-order/fee/slippage/leverage/DD correctness
3. Performance baseline and optimized speed comparison
4. Hardware usage: CPU threads/workers, memory cache, GPU feasibility
5. External research summary and which ideas were converted into tests
6. Best conservative/balanced/aggressive results
7. Whether conservative >50% annualized under <=10% DD was achieved
8. If not achieved, exact bottleneck and next expansion path
```

Result table:

```text
Risk | Universe | Task ID | Annualized % | Max DD % | Return % | Ann/DD | Members | Unique Symbols | Max Symbol Weight % | Eq Points | Max Gap Days | Notes
```

Member table:

```text
Symbol | Direction | Allocation % | Leverage | Candidate Ann % | Candidate DD % | Trades | TP/SL Params | Candidate ID
```
