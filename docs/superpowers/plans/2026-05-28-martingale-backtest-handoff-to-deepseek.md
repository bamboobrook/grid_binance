# Martingale Backtest Handoff To Deepseek Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 mixed 马丁回测资金曲线/组合曲线真实性问题，并在真实 1m K 线、手续费、滑点成本约束下继续寻找更高年化、更低回撤的 7/18 币种 mixed 组合。

**Architecture:** 回测 worker 当前分为“参数筛选”和“最终精测/组合”两层。筛选阶段可以继续使用代表性抽样 K 线提高速度；最终候选、组合资金曲线、回撤曲线、发布实盘参数必须使用全量 1m K 线和真实成本。组合器必须按时间戳对齐成员曲线，不允许按数组下标合并，也不允许把 500 点预览曲线当作全量曲线。

**Tech Stack:** Rust workspace (`backtest-engine`, `backtest-worker`), PostgreSQL `backtest_tasks`, Docker Compose workers, Next.js 前端验证契约测试。

---

## 当前进展

### 已确认的问题

1. **资金曲线“大部分时间直线”不是单纯前端问题。**
   - 旧任务 `validation-18-mixed-tailfix3-202605271850` 的组合资金曲线只有 500 点，且出现 `577 天`、`514.5 天`的大时间断点。
   - 这会让前端看起来大段水平/直线，且组合回撤和视觉判断不可靠。

2. **组合曲线存在两个后端根因。**
   - 根因 A：`apps/backtest-engine/src/portfolio_search.rs` 的 `combine_equity_curves()` 原本按成员曲线数组下标合并，而不是按 `timestamp_ms` 对齐。
   - 根因 B：`apps/backtest-worker/src/main.rs` 的 `portfolio_candidates_from_outputs()` 原本优先读取 `summary.equity_curve`，而 summary 中是 `sampled_preview(..., 500)`，不是全量曲线。
   - 根因 C：`run_candidate_kline_screening()` 用了 `screening_bars_for_candidate()`，最终候选精测也在用代表性抽样 K 线，而不是全量 1m K 线。

3. **当前有未提交本地修改。**
   - `apps/backtest-engine/src/portfolio_search.rs`
   - `apps/backtest-engine/src/search.rs`
   - `apps/backtest-worker/src/main.rs`
   - `apps/web/components/backtest/backtest-professional-panel.tsx`
   - `apps/web/components/backtest/backtest-wizard.tsx`
   - `tests/verification/backtest_console_contract.test.mjs`

4. **已经取消了部分旧验证任务。**
   - 因为它们基于旧 worker 逻辑运行，曲线仍不可信。
   - 已完成但仍不可信的对照任务：`validation-7-mixed-aggressive-curvefix-202605281210`，结果年化约 `37.69%`、回撤约 `15.42%`，但曲线仍有 `577 天`断点，因为最终精测仍用了抽样 K 线。

### 已经做过但需要 deepseek 复核/完成的代码方向

1. `apps/backtest-engine/src/portfolio_search.rs`
   - `combine_equity_curves()` 已改成按所有成员 `timestamp_ms` 的并集对齐。
   - 对缺失时间点用该成员最近一次权益值 forward-fill。
   - 已加测试：`weighted_portfolio_aligns_member_equity_by_timestamp_not_index`。

2. `apps/backtest-worker/src/main.rs`
   - `portfolio_candidates_from_outputs()` 已改成优先使用 `output.equity_curve` / `output.drawdown_curve` / `output.trades_preview`，只有 output 为空时才回退 summary。
   - 已加测试：`portfolio_candidates_prefer_full_output_curve_over_summary_preview`。
   - `run_candidate_kline_screening()` 已开始改成 `bars_for_candidate()`，即最终精测用全量 K 线。
   - 已加测试：`final_kline_refinement_uses_full_symbol_history_not_screening_sample`。
   - 需要 deepseek 继续完整验证、部署、重跑任务。

3. 风险档位已调整为用户当前要求。
   - 保守：最大回撤 `10%`
   - 平衡：最大回撤 `20%`
   - 激进：最大回撤 `30%`
   - 相关位置：`apps/backtest-engine/src/search.rs`、`apps/backtest-worker/src/main.rs`、`apps/web/components/backtest/backtest-wizard.tsx`、`apps/web/components/backtest/backtest-professional-panel.tsx`。

---

## 目标标准

1. **真实性优先。**
   - 必须使用 `/home/bumblebee/Project/discord_c2im/pipeline/data/market_data.db` 中 1m K 线。
   - 时间范围：`2023-01-01 00:00:00` 到当前日期上个月月末，例如当前应到 `2026-04-30 23:59:00`。
   - 成本：Binance futures taker fee 使用 `0.045%` 即 `4.5 bps`，滑点当前保留 `2.0 bps`。
   - 不允许为了提高收益而移除手续费/滑点/全量 1m 数据。

2. **收益/回撤目标。**
   - 用户理想目标：年化 `50%+`，回撤尽量小。
   - 三档约束：保守 `<=10%`、平衡 `<=20%`、激进 `<=30%`。
   - 如果真实全量数据无法达到 50%，必须如实说明，并给出最优结果和约束原因。

3. **mixed 模式优先。**
   - 后续搜索以 `direction_mode = mixed_best` 为主。
   - mixed 必须在 long_only、short_only、long_short 三类中比较并选择，不应固定单向。
   - long_short 中多空参数允许不同，包括间隔、倍率、层数、止盈、止损、多空权重。

4. **组合要求。**
   - 组合不是挑单个最优策略，而是按权重分配资金给多个策略后重新计算组合资金曲线/回撤曲线。
   - 组合展示 Top3 即可。
   - 单一币种权重不得超过 `40%`。
   - 对 18 币种任务，优先找更分散的组合；但不要为了凑数量牺牲过多收益。

---

## Task 1: 完成并验证全量曲线修复

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`
- Modify: `apps/backtest-worker/src/main.rs`
- Test: Rust unit tests in the same files

- [ ] **Step 1: 检查组合曲线时间对齐测试存在**

确认 `apps/backtest-engine/src/portfolio_search.rs` 中存在测试：

```rust
#[test]
fn weighted_portfolio_aligns_member_equity_by_timestamp_not_index() {
    // 应构造 BTC 有 ts=1,2,3；ETH 有 ts=1,3。
    // 组合曲线必须输出 [1,2,3]，ts=2 时 ETH 使用 ts=1 的权益 forward-fill。
}
```

- [ ] **Step 2: 跑组合曲线测试**

Run:

```bash
cargo test -p backtest-engine weighted_portfolio_aligns_member_equity_by_timestamp_not_index -- --nocapture
```

Expected: `test result: ok`。

- [ ] **Step 3: 检查候选转组合输入时优先使用 output 全量曲线**

确认 `apps/backtest-worker/src/main.rs` 的 `portfolio_candidates_from_outputs()` 中逻辑为：

```rust
equity_curve: if output.equity_curve.is_empty() {
    equity_curve
} else {
    output.equity_curve.clone()
},
drawdown_curve: if output.drawdown_curve.is_empty() {
    drawdown_curve
} else {
    output.drawdown_curve.clone()
},
trades: if output.trades_preview.is_empty() {
    trades
} else {
    output.trades_preview.clone()
},
```

- [ ] **Step 4: 跑 output 全量曲线优先测试**

Run:

```bash
cargo test -p backtest-worker portfolio_candidates_prefer_full_output_curve_over_summary_preview -- --nocapture
```

Expected: `test result: ok`。

- [ ] **Step 5: 检查最终精测使用全量 K 线**

确认 `run_candidate_kline_screening()` 使用 `bars_for_candidate()`，不是 `screening_bars_for_candidate()`：

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

保留 `screening_bars_for_candidate()` 只给粗筛阶段使用。

- [ ] **Step 6: 跑全量精测测试**

Run:

```bash
cargo test -p backtest-worker final_kline_refinement_uses_full_symbol_history_not_screening_sample -- --nocapture
```

Expected: `test result: ok`。

- [ ] **Step 7: 跑完整相关验证**

Run:

```bash
cargo test -p backtest-engine portfolio_search::tests::weighted_portfolio -- --nocapture
cargo test -p backtest-engine portfolio_search::tests::portfolio_v2 -- --nocapture
cargo test -p backtest-worker portfolio_candidates_ -- --nocapture
cargo test -p backtest-worker final_kline_refinement_uses_full_symbol_history_not_screening_sample -- --nocapture
cargo test -p backtest-worker long_short_uses_configured_risk_profile_drawdown_limits -- --nocapture
node --test tests/verification/backtest_console_contract.test.mjs
cargo check -p backtest-worker
```

Expected: all commands exit `0`。`cargo check` 当前可能有既有 dead_code warnings；不得有编译错误。

---

## Task 2: 部署 worker 并创建曲线修复验证任务

**Files:**
- No code files if Task 1 already completed
- Runtime: Docker Compose backtest-worker
- DB: `backtest_tasks`

- [ ] **Step 1: 重建并重启 worker**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml build backtest-worker

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml up -d --scale backtest-worker=4 backtest-worker
```

Expected: four `grid-binance-backtest-worker-*` containers are `Up`。

- [ ] **Step 2: 取消仍在旧逻辑下 running/queued 的验证任务**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -c "
UPDATE backtest_tasks
SET status='cancelled', updated_at=now(), error_message='cancelled: full-curve fix redeploy required'
WHERE task_id LIKE 'validation-%mixed-%202605281040'
  AND status IN ('queued','running');
"
```

Expected: old queued/running validation tasks become `cancelled`。

- [ ] **Step 3: 创建低预算 7 币种 mixed 曲线验证任务**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -c "
INSERT INTO backtest_tasks (task_id, owner, status, strategy_type, config, summary, created_at, updated_at)
SELECT
  'validation-7-mixed-aggressive-fullcurve-20260528',
  owner,
  'queued',
  strategy_type,
  jsonb_set(jsonb_set(jsonb_set(config, '{random_candidates}', '64'::jsonb), '{per_symbol_top_n}', '10'::jsonb), '{priority}', '650'::jsonb),
  jsonb_build_object('stage_label','queued full curve validation','source_task_id', task_id),
  now(),
  now()
FROM backtest_tasks
WHERE task_id='validation-7-mixed-tailfix3-202605271850'
ON CONFLICT (task_id) DO UPDATE
SET status='queued', config=EXCLUDED.config, summary=EXCLUDED.summary,
    started_at=NULL, completed_at=NULL, error_message=NULL, updated_at=now();
"
```

Expected: task status is `queued` then `running`。

- [ ] **Step 4: 轮询任务完成**

Run repeatedly:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -c "
SELECT task_id,status,summary->>'stage_label' stage,
       round((jsonb_path_query_first(summary,'$.portfolio_top3[0].annualized_return_pct')::text)::numeric,2) ann,
       round((jsonb_path_query_first(summary,'$.portfolio_top3[0].max_drawdown_pct')::text)::numeric,2) dd,
       jsonb_path_query_first(summary,'$.portfolio_top3[0].member_count') members,
       error_message, now()-updated_at age
FROM backtest_tasks
WHERE task_id='validation-7-mixed-aggressive-fullcurve-20260528';
"
```

Expected: status eventually `succeeded`。如果 `failed`， read `error_message` and fix root cause before continuing。

- [ ] **Step 5: 验证资金曲线没有大断点**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -c "
WITH p AS (
  SELECT summary->'portfolio_top3'->0 AS pf
  FROM backtest_tasks
  WHERE task_id='validation-7-mixed-aggressive-fullcurve-20260528'
), pts AS (
  SELECT row_number() over () rn,
         (v->>'timestamp_ms')::bigint ts,
         (v->>'equity_quote')::numeric eq
  FROM p, jsonb_array_elements(pf->'equity_curve') v
), diffs AS (
  SELECT *, ts-lag(ts) over(order by rn) dts, eq-lag(eq) over(order by rn) deq
  FROM pts
)
SELECT count(*) points,
       to_timestamp(min(ts)/1000) start_at,
       to_timestamp(max(ts)/1000) end_at,
       count(*) filter(where abs(coalesce(deq,0)) > 0.000001) changed_points,
       round(avg(dts)/86400000.0,2) avg_gap_days,
       round(max(dts)/86400000.0,2) max_gap_days,
       round(min(eq),2) min_eq,
       round(max(eq),2) max_eq
FROM diffs;
"
```

Expected:
- `start_at` = `2023-01-01 00:00:00+00`
- `end_at` = `2026-04-30 23:59:00+00`
- `max_gap_days` 应接近 `2.5` 天左右（因为最终 summary 展示仍采样 500 点），但不应再出现 `577` 天或 `514` 天这种跨年断点。
- 如果仍出现巨大断点，继续追 `CandidateOutput.equity_curve` 是否仍来自抽样数据。

---

## Task 3: 重跑三档 7/18 mixed 深搜

**Files:**
- No code files if Task 1/2 passed
- Runtime: `backtest_tasks`

- [ ] **Step 1: 创建 7/18 币种 × 三档 mixed 任务**

Use these task IDs:

```text
validation-7-mixed-conservative-fullcurve-20260528
validation-7-mixed-balanced-fullcurve-20260528
validation-7-mixed-aggressive-fullcurve-20260528
validation-18-mixed-conservative-fullcurve-20260528
validation-18-mixed-balanced-fullcurve-20260528
validation-18-mixed-aggressive-fullcurve-20260528
```

Base configs:
- 7 symbols from `validation-7-mixed-tailfix3-202605271850`
- 18 symbols from `validation-18-mixed-tailfix3-202605271850`

Recommended budgets:
- 7 symbols: `random_candidates=256`, `per_symbol_top_n=20`
- 18 symbols: `random_candidates=384`, `per_symbol_top_n=20`
- `direction_mode=mixed_best`
- `portfolio_top_n=3`
- `leverage_range=[1,20]`
- `fee_bps=4.5`
- `slippage_bps=2.0`

- [ ] **Step 2: 监控任务状态**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -c "
SELECT task_id,status,summary->>'stage_label' stage,
       summary->>'portfolio_pool_candidate_count' pool,
       round((jsonb_path_query_first(summary,'$.portfolio_top3[0].annualized_return_pct')::text)::numeric,2) ann,
       round((jsonb_path_query_first(summary,'$.portfolio_top3[0].max_drawdown_pct')::text)::numeric,2) dd,
       jsonb_path_query_first(summary,'$.portfolio_top3[0].member_count') members,
       jsonb_path_query_first(summary,'$.portfolio_top3[0].portfolio_unique_symbol_count') uniq,
       error_message, now()-updated_at age
FROM backtest_tasks
WHERE task_id LIKE 'validation-%mixed-%fullcurve-20260528'
ORDER BY task_id;
"
```

Expected: all eventually `succeeded` or actionable `failed` with root cause。

- [ ] **Step 3: 汇总结果**

For each task, record:
- annualized_return_pct
- max_drawdown_pct
- member_count
- portfolio_unique_symbol_count
- top portfolio members: symbol, direction, allocation_pct, leverage, return_pct, max_drawdown_pct, trade_count
- curve validation `max_gap_days`

- [ ] **Step 4: 判断是否达到目标**

Criteria:
- conservative: `dd <= 10%`
- balanced: `dd <= 20%`
- aggressive: `dd <= 30%`
- target annualized: ideal `>=50%`

If no task reaches 50%, do not claim success. Report best observed portfolio and exact constraints.

---

## Task 4: 如果收益仍低于 50%，按以下方向继续优化

**Files likely touched:**
- `apps/backtest-engine/src/search.rs`
- `apps/backtest-worker/src/main.rs`
- `apps/backtest-engine/src/portfolio_search.rs`

- [ ] **Step 1: 增加更多 random seeds，而不是盲目扩大单任务预算**

Rationale: 当前 long_short 生成中存在随机采样。多个 seed 更容易找到不同区域的高收益候选。

Create tasks with same config but seeds:

```text
17, 29, 43, 71, 101
```

Then aggregate best candidates or compare task-level portfolios。

- [ ] **Step 2: 扩大 mixed 参数空间但保持风险约束**

In `apps/backtest-engine/src/search.rs`, consider adding:

```rust
space.spacing_bps = vec![25, 35, 50, 70, 90, 120, 160, 220, 300, 420, 600, 800];
space.take_profit_bps = vec![25, 30, 45, 60, 80, 100, 140, 200, 300, 450];
space.tail_stop_bps = vec![600, 800, 1200, 1800, 2400, 3000, 4000, 5500, 7000, 9000];
space.max_legs = vec![3, 4, 5, 6, 7, 8, 9, 10];
space.order_multiplier = vec![1.1, 1.15, 1.25, 1.4, 1.6, 1.8, 2.0, 2.2, 2.4, 2.8];
space.leverage = vec![2, 3, 4, 5, 6, 8, 10, 12, 15, 20];
```

But every expansion must be verified by tests and should not make final精测 impossible to finish。

- [ ] **Step 3: 优化组合器，不要只奖励成员数量**

In `apps/backtest-engine/src/portfolio_search.rs`, portfolio rank should prefer:

```text
1. annualized / max_drawdown ratio
2. lower drawdown at similar annualized
3. lower member correlation
4. symbol diversification
5. no single symbol >40%
```

Do not allow低收益币种只为凑数量拖累组合。对于 7 币种任务，不强制 10 unique symbols；对于 18 币种任务，优先 8-12 unique symbols but not at any cost。

- [ ] **Step 4: 观察 short/long_short 是否真实贡献收益**

For every mixed task, query top members:

```sql
SELECT member->>'symbol', member->>'direction', member->>'allocation_pct', member->>'leverage', member->>'return_pct', member->>'max_drawdown_pct'
FROM backtest_tasks t,
     jsonb_array_elements(t.summary->'portfolio_top3'->0->'members') member
WHERE t.task_id = '<TASK_ID>';
```

If all winners are Long, that may be真实行情结果，也可能是 short/long_short 参数空间或评分被压制。只有在 full-curve 修复后才能判断。

---

## 需要重点观察的指标

1. **曲线质量**
   - `max_gap_days` 不应出现几百天。
   - `changed_points` 应接近 `points - 1`。
   - 曲线 start/end 必须覆盖完整回测区间。

2. **收益真实性**
   - 年化收益不要只看单策略，要看组合权益曲线重新计算后的年化。
   - 成本必须包含 `fee_bps=4.5` 和 `slippage_bps=2.0`。
   - 回撤必须按组合总资金权益曲线计算，不按首单金额计算。

3. **交易密度**
   - 如果交易明细仍只集中在 2023-01，需要确认是否只是 `trades_preview` 截断为前 100/200 条。
   - 可以后续改成“首尾混合预览”或 artifact 提供全量交易明细下载。

4. **组合分散度**
   - 单一币种占比 `<=40%`。
   - 18 币种任务中，若 top portfolio 只有 2-3 个币种，需要检查组合器是否过度追收益或候选池过窄。

5. **收益目标可达性**
   - 旧的 50%+ 结果可能来自抽样/曲线问题，不能直接沿用。
   - full-curve 修复后如果最佳仍在 35%-40%，应诚实报告“真实全量约束下暂未达到 50%”。

---

## 交付要求

- [ ] 所有代码测试通过。
- [ ] Docker worker 已部署。
- [ ] 至少一个低预算 fullcurve 任务证明资金曲线无跨年断点。
- [ ] 完整 7/18 三档 mixed 任务已创建并完成。
- [ ] 给出结果表：任务、风险档位、年化、回撤、成员数、币种数、曲线最大间隔。
- [ ] 如达到 50% 年化，标记候选组合并复核成员参数能否发布实盘。
- [ ] 如未达到 50%，给出下一轮 seed/参数空间/组合器优化建议。

