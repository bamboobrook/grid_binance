# Martingale Risk Matrix Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. This plan starts from verified 7-symbol balanced success and expands to conservative/balanced/aggressive plus 18-symbol universe. Do not change backtest logic unless validation shows a root-cause bug.

**Goal:** 在 FlyingKid 账户下复现可查看的 7 币种 balanced 达标组合，并扩展测试 7/18 币种在保守、平衡、激进三档下的收益/回撤表现，优先寻找保守模式年化 `>50%` 且回撤 `<=10%` 的组合。

**Architecture:** 先把已达标的 7-symbol balanced 结果以 FlyingKid 账户重跑/展示，确认曲线连续；然后按风险矩阵分阶段创建任务，先 7-conservative，再 18-conservative/balanced，最后 aggressive 与 balanced 优化。每一步都用真实全量 1m K 线、真实手续费/滑点、组合 Top3，并通过 SQL 检查资金曲线连续性与组合成员结构。

**Tech Stack:** Rust `backtest-worker`, Rust `backtest-engine`, PostgreSQL `backtest_tasks`, Docker Compose workers, local market data SQLite `/home/bumblebee/Project/discord_c2im/pipeline/data/market_data.db`.

---

## Verified Current State

- 当前已完成的最好结果不在 FlyingKid 下，而在 `super-admin@example.com`。
- `flyingkid2022@outlook.com` 当前没有回测任务。
- 最佳已知 7 币种 balanced mixed 任务：`glm-7-wide-seed67-20260529`。
- 最佳组合：年化 `50.23%`，最大回撤 `19.82%`，总收益 `288.06%`，成员 `9`。
- 曲线预览点数：equity `5000`，drawdown `5000`。
- 曲线时间范围：`2023-01-01 00:00:00 UTC` 到 `2026-04-30 23:59:00 UTC`。
- 预览曲线最大 gap 约 `0.2438` 天，未出现几百天断层。
- 当前 18 币种默认集合来自 `apps/backtest-worker/src/main.rs`：

```text
BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT, DOGEUSDT, XRPUSDT, ADAUSDT,
ZECUSDT, DASHUSDT, NEARUSDT, BCHUSDT, LINKUSDT, AVAXUSDT, UNIUSDT,
FILUSDT, DOTUSDT, AAVEUSDT, INJUSDT
```

---

## Non-Negotiable Constraints

- Owner 必须使用 `flyingkid2022@outlook.com`，这样用户能在前端查看图表。
- 使用真实全量 1m K 线：`2023-01-01 00:00:00 UTC` 到上个月月底；当前为 `2026-04-30 23:59:00 UTC`。
- `market = usd_m_futures`，不要混入 spot。
- `direction_mode = mixed_best`，必须允许 long_only/short_only/long_short 共同竞争。
- 成本：`fee_bps = 4.5`，`slippage_bps = 2.0`。
- `portfolio_top_n = 3`。
- 组合必须重新计算组合资金曲线/回撤曲线，不是挑单个策略。
- 单一币种权重不得超过 `40%`。
- 不允许用缩短时间、移除成本、抽样最终曲线来制造收益。

---

## Risk Targets

| Profile | Max DD | Primary Goal |
| --- | ---: | --- |
| conservative | `<=10%` | 尽力寻找年化 `>50%`，若达不到，报告最优真实结果和瓶颈 |
| balanced | `<=20%` | 复现并超越当前 `50.23% / 19.82%` 基线 |
| aggressive | `<=30%` | 寻找更高年化，同时观察回撤效率是否值得实盘考虑 |

---

## Task 1: Snapshot Current Verified Results

**Files:**
- No code changes.

- [ ] **Step 1: Confirm services are running**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml ps
```

Expected:

- `api-server` healthy
- `web` healthy
- `postgres` healthy
- at least 4 `backtest-worker` containers up

- [ ] **Step 2: Confirm FlyingKid has no stale tasks or record current tasks**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -P pager=off -c \
"SELECT task_id, owner, status, created_at, summary->>'stage' AS stage FROM backtest_tasks WHERE owner='flyingkid2022@outlook.com' ORDER BY created_at DESC;"
```

Expected currently: `0 rows` or only tasks intentionally created by this plan.

- [ ] **Step 3: Record current best baseline**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -P pager=off -c \
"WITH p AS (SELECT task_id, owner, ord, elem FROM backtest_tasks, jsonb_array_elements(summary->'portfolio_top3') WITH ORDINALITY x(elem, ord) WHERE task_id LIKE 'glm-7-wide-%20260529') SELECT task_id, owner, ord AS rank, round((elem->>'annualized_return_pct')::numeric,2) ann, round((elem->>'max_drawdown_pct')::numeric,2) dd, round((elem->>'return_pct')::numeric,2) ret, elem->>'member_count' members, jsonb_array_length(elem->'equity_curve') eq_points FROM p ORDER BY ann DESC NULLS LAST LIMIT 10;"
```

Expected best known: `glm-7-wide-seed67-20260529`, rank `1`, ann about `50.23`, dd about `19.82`.

---

## Task 2: Create FlyingKid 7-Symbol Balanced Replication

**Files:**
- Runtime DB/API only. Prefer API if available; direct DB insert is allowed only if it matches existing task schema exactly.

- [ ] **Step 1: Create one FlyingKid replication task**

Task ID:

```text
fk-7-balanced-wide-seed67-20260530
```

Config must match known successful shape:

```json
{
  "strategy_type": "martingale",
  "owner": "flyingkid2022@outlook.com",
  "symbols": ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT"],
  "risk_profile": "balanced",
  "direction_mode": "mixed_best",
  "market": "usd_m_futures",
  "margin_mode": "isolated",
  "search_mode": "profit_optimized_v2",
  "random_candidates": 256,
  "per_symbol_top_n": 30,
  "portfolio_top_n": 3,
  "fee_bps": 4.5,
  "slippage_bps": 2.0,
  "random_seed": 67,
  "time_mode": "auto_recent",
  "start_ms": 1672531200000,
  "end_ms": 1777593540000
}
```

- [ ] **Step 2: Poll until completed**

Run every few minutes:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -P pager=off -c \
"SELECT task_id, owner, status, updated_at, summary->>'stage' stage, summary->>'progress_pct' progress, summary->>'portfolio_pool_candidate_count' pool FROM backtest_tasks WHERE task_id='fk-7-balanced-wide-seed67-20260530';"
```

Expected final: `status=succeeded`, `stage=completed`.

- [ ] **Step 3: Verify curve continuity**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -P pager=off -c \
"WITH p AS (SELECT elem FROM backtest_tasks, jsonb_array_elements(summary->'portfolio_top3') WITH ORDINALITY x(elem, ord) WHERE task_id='fk-7-balanced-wide-seed67-20260530' AND ord=1), pts AS (SELECT row_number() over (ORDER BY (pt->>'timestamp_ms')::bigint) rn, (pt->>'timestamp_ms')::bigint ts, (pt->>'equity_quote')::numeric eq FROM p, jsonb_array_elements(elem->'equity_curve') pt), gaps AS (SELECT ts, ts - lag(ts) over (ORDER BY ts) AS gap_ms FROM pts) SELECT count(*) points, to_timestamp(min(ts)/1000) first_ts, to_timestamp(max(ts)/1000) last_ts, round(max(gap_ms)/86400000.0,4) max_gap_days, round(min(eq),2) min_eq, round(max(eq),2) max_eq FROM pts LEFT JOIN gaps USING(ts);"
```

Expected:

- `points >= 5000`
- first timestamp = `2023-01-01`
- last timestamp = `2026-04-30 23:59`
- `max_gap_days < 1`

---

## Task 3: Run 7-Symbol Conservative Search

**Files:**
- Runtime DB/API only.

- [ ] **Step 1: Create initial conservative seeds**

Create these FlyingKid tasks:

```text
fk-7-conservative-wide-seed67-20260530
fk-7-conservative-wide-seed83-20260530
fk-7-conservative-wide-seed127-20260530
fk-7-conservative-wide-seed199-20260530
```

Use same 7 symbols, but:

```json
{
  "risk_profile": "conservative",
  "scoring": { "max_drawdown_pct": 10 },
  "random_candidates": 256,
  "per_symbol_top_n": 30,
  "portfolio_top_n": 3
}
```

Expected: all tasks should enforce portfolio DD `<=10%`.

- [ ] **Step 2: Summarize conservative results**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -P pager=off -c \
"WITH p AS (SELECT task_id, ord, elem FROM backtest_tasks, jsonb_array_elements(summary->'portfolio_top3') WITH ORDINALITY x(elem, ord) WHERE task_id LIKE 'fk-7-conservative-wide-%20260530' AND status='succeeded') SELECT task_id, ord rank, round((elem->>'annualized_return_pct')::numeric,2) ann, round((elem->>'max_drawdown_pct')::numeric,2) dd, round((elem->>'return_pct')::numeric,2) ret, elem->>'member_count' members, jsonb_array_length(elem->'equity_curve') eq_points FROM p ORDER BY ann DESC NULLS LAST LIMIT 12;"
```

If best conservative ann `<50%`, do not declare failure yet. Proceed to Task 5 for targeted expansion.

---

## Task 4: Run 18-Symbol Conservative And Balanced Search

**Files:**
- Runtime DB/API only.

- [ ] **Step 1: Use this 18-symbol universe**

```text
BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT, DOGEUSDT, XRPUSDT, ADAUSDT,
ZECUSDT, DASHUSDT, NEARUSDT, BCHUSDT, LINKUSDT, AVAXUSDT, UNIUSDT,
FILUSDT, DOTUSDT, AAVEUSDT, INJUSDT
```

- [ ] **Step 2: Create 18 conservative seeds**

Task IDs:

```text
fk-18-conservative-wide-seed67-20260530
fk-18-conservative-wide-seed83-20260530
fk-18-conservative-wide-seed127-20260530
fk-18-conservative-wide-seed199-20260530
```

Config changes:

```json
{
  "risk_profile": "conservative",
  "scoring": { "max_drawdown_pct": 10 },
  "symbols": [18 symbols above],
  "random_candidates": 384,
  "per_symbol_top_n": 30,
  "portfolio_top_n": 3
}
```

- [ ] **Step 3: Create 18 balanced seeds**

Task IDs:

```text
fk-18-balanced-wide-seed67-20260530
fk-18-balanced-wide-seed83-20260530
fk-18-balanced-wide-seed127-20260530
fk-18-balanced-wide-seed199-20260530
```

Config changes:

```json
{
  "risk_profile": "balanced",
  "scoring": { "max_drawdown_pct": 20 },
  "symbols": [18 symbols above],
  "random_candidates": 384,
  "per_symbol_top_n": 30,
  "portfolio_top_n": 3
}
```

- [ ] **Step 4: Summarize 18-symbol results**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -P pager=off -c \
"WITH p AS (SELECT task_id, ord, elem FROM backtest_tasks, jsonb_array_elements(summary->'portfolio_top3') WITH ORDINALITY x(elem, ord) WHERE task_id LIKE 'fk-18-%wide-%20260530' AND status='succeeded') SELECT task_id, ord rank, round((elem->>'annualized_return_pct')::numeric,2) ann, round((elem->>'max_drawdown_pct')::numeric,2) dd, round((elem->>'return_pct')::numeric,2) ret, elem->>'member_count' members, (SELECT count(DISTINCT m->>'symbol') FROM jsonb_array_elements(elem->'members') m) unique_symbols, round((SELECT max(sum_alloc) FROM (SELECT m->>'symbol' symbol, sum((m->>'allocation_pct')::numeric) sum_alloc FROM jsonb_array_elements(elem->'members') m GROUP BY m->>'symbol') s),2) max_symbol_weight, jsonb_array_length(elem->'equity_curve') eq_points FROM p ORDER BY ann DESC NULLS LAST LIMIT 20;"
```

Expected:

- conservative rows must have dd `<=10`
- balanced rows must have dd `<=20`
- top portfolios should not exceed single-symbol weight `40%`

---

## Task 5: Conservative >50% Targeted Expansion

**Files:**
- Modify code only if Task 3/4 proves search space or pool is the bottleneck.

- [ ] **Step 1: If conservative best <50%, inspect candidate pool**

Run for the best conservative task:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -P pager=off -c \
"SELECT candidate_id, summary->>'symbol' symbol, summary->>'direction_mode' direction_mode, round((summary->>'annualized_return_pct')::numeric,2) ann, round((summary->>'max_drawdown_pct')::numeric,2) dd, summary->>'trade_count' trades, summary->>'max_leverage_used' lev FROM backtest_candidate_summaries WHERE task_id='<BEST_CONSERVATIVE_TASK_ID>' ORDER BY (summary->>'annualized_return_pct')::numeric DESC NULLS LAST LIMIT 40;"
```

Decision rule:

- If there are high-ann candidates but portfolio result is weak: improve portfolio weight search, not candidate generation.
- If no high-ann candidates exist: increase seeds/random candidates and widen search.
- If high-ann candidates are filtered before portfolio pool: fix pool admission.

- [ ] **Step 2: Run deeper conservative search only after diagnosis**

Use:

```json
{
  "random_candidates": 512,
  "per_symbol_top_n": 40,
  "portfolio_top_n": 3,
  "seeds": [17, 23, 37, 53, 67, 83, 127, 151, 173, 199]
}
```

Start with 7-symbol conservative first. If still below target, run 18-symbol conservative.

---

## Task 6: Aggressive Matrix After Conservative/Balanced

Only after Tasks 2-5 complete:

Create:

```text
fk-7-aggressive-wide-seed67-20260530
fk-7-aggressive-wide-seed83-20260530
fk-18-aggressive-wide-seed67-20260530
fk-18-aggressive-wide-seed83-20260530
```

Config:

```json
{
  "risk_profile": "aggressive",
  "scoring": { "max_drawdown_pct": 30 },
  "random_candidates": 256 for 7 symbols,
  "random_candidates": 384 for 18 symbols,
  "per_symbol_top_n": 30,
  "portfolio_top_n": 3
}
```

Purpose: compare risk efficiency. Aggressive is not the immediate priority.

---

## Final Report Required

Report table:

```text
Task ID | Owner | Risk | Universe | Annualized % | Max DD % | Return % | Members | Unique Symbols | Max Symbol Weight % | Eq Points | Max Gap Days | Status
```

Best portfolio member table:

```text
Symbol | Direction | Allocation % | Leverage | Candidate Annualized % | Candidate DD % | Trades | Candidate ID
```

Conclusion:

```text
1. FlyingKid 是否能在前端看到 7-balanced 达标结果？
2. conservative 是否达到年化 >50% 且 DD <=10%？
3. 18-symbol 是否优于 7-symbol？
4. balanced 是否超过当前 50.23% / 19.82% 基线？
5. 下一步应扩大搜索、优化组合器，还是进入实盘前验证？
```
