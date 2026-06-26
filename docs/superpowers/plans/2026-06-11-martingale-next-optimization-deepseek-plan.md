# Martingale Next Optimization DeepSeek Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:systematic-debugging` before fixes and `superpowers:executing-plans` or `superpowers:subagent-driven-development` to execute this plan task-by-task. Do not pause after starting backtests; monitor, estimate ETA, and continue to the next task until all acceptance criteria are met.

**Goal:** 在 FlyingKid 账户下完成三档风险组合搜索：conservative 尽力达到年化 `>50%` 且 DD `<=10%`，balanced 超过当前 FlyingKid `65.52%` 且 DD `<=20%`，aggressive 超过当前 FlyingKid `77.00%` 并挑战历史 `~100.5%` 且 DD `<=30%`。如果回测使用 ATR/ADX 等指标，实盘必须实现同等指标计算和同等运行语义，不能只阻止上线。

**Architecture:** 先收尾 DeepSeek 已做的归档/搜索中断状态，再把 ATR/ADX 指标能力做成 backtest/live 共用的语义；然后修复回测速度、ETA 和监控闭环；最后自动分批执行三档搜索、归档落选结果、输出最终可实盘评估报告。

**Tech Stack:** Rust `backtest-engine`, Rust `backtest-worker`, Rust `trading-engine`, `shared-binance`, PostgreSQL, Docker Compose, Binance USD-M Futures REST/User Data Stream, local SQLite market data.

---

## Current Verified State

- Codex 已按用户要求停止 DeepSeek 当前回测：
  - `fk-18-conservative-seed211-20260611` 已从 `running` 标记为 `cancelled`。
  - `grid-binance-backtest-worker-1` 已停止。
  - 当前没有 `queued/running/paused` 回测任务。
  - 所有 `grid-binance-backtest-worker-*` 当前均为 `Exited`。
- 当前内存约 `17Gi used / 178Gi available`，Swap 约 `6.8Gi / 100Gi`。
- DeepSeek 已做的有效部分：
  - 使用 owner rename 将旧 FlyingKid 非最佳/失败任务归档到 `archive+flyingkid2022@outlook.com`，未硬删除。
  - 当前 FlyingKid 可见三档结果：
    - conservative: `fk-18-conservative-baseline-from-v5-20260611`，年化 `40.69%`，DD `9.66%`。
    - balanced: `fk-18-bal-v2-seed53-20260601`，年化 `65.52%`，DD `19.32%`。
    - aggressive: `fk-18-agg-v2-seed173-20260601`，年化 `77.00%`，DD `28.03%`。
  - 将 backtest worker 默认线程从 `24` 降到 `12`。
  - 为 worker 增加了部分阶段 timing log。
  - 将组合优化从过度分散方向调整为更允许 2-6 成员紧凑组合。
- DeepSeek 已做但需要修正/补完的部分：
  - `fk-18-conservative-baseline-from-v5-20260611` 只复制了 `backtest_tasks.summary`，没有复制 `backtest_candidate_summaries`，前端候选/发布链路可能不完整。
  - 当前新增的 `fk-18-conservative-seed211-20260611` 是 `cancelled`，应归档，避免 FlyingKid 可见列表超过每档一个最佳任务。
  - 报告中说 `backtest_candidate_summaries` 不会级联/会 orphan 的描述不准确；实际 schema 中 `backtest_candidate_summaries.task_id` 是 `ON DELETE CASCADE`。
  - `log_stage_timing()` 对 `rss_mb` 的打印会对 JSON number 调 `as_str()`，日志显示 `rss_mb=N/A`，需要修。
  - `evaluate_refinement_candidates_parallel()` 目前只打印 chunk 完成日志，没有写 DB progress/ETA。
  - DeepSeek 当前把 ATR spacing 从搜索中禁用，并在实盘 preflight 中拒绝 ATR spacing / ATR TP / ATR SL / IndicatorExpression。这只能防错，不满足用户目标，必须改成实现指标计算与回测/实盘 parity。

## Non-Negotiable Constraints

- 新回测 owner 必须是 `flyingkid2022@outlook.com`。
- FlyingKid 可见回测最终只保留每档风险最佳一个；其他失败、取消、过期、被超越任务归档到 `archive+flyingkid2022@outlook.com`，不要硬删除，除非用户明确授权。
- 目标币种池只能来自：
  - 指定 7 币种：`BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT, XRPUSDT, DOGEUSDT, ADAUSDT`
  - 18 币种：`BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT, DOGEUSDT, XRPUSDT, ADAUSDT, ZECUSDT, DASHUSDT, NEARUSDT, BCHUSDT, LINKUSDT, AVAXUSDT, UNIUSDT, FILUSDT, DOTUSDT, AAVEUSDT, INJUSDT`
  - 或成交量 Top50 且 `2023-01-01` 起有完整数据的前 18 个币种。若与上述 18 币种不同，必须输出筛选 SQL 和结果。
- 回测窗口从 `2023-01-01 00:00:00 UTC` 到上个月月底；不要缩短窗口制造收益。
- 必须使用真实 1m K 线、手续费 `4.5 bps`、滑点 `2.0 bps`。如 config 未显式写 `fee_bps`，必须证明引擎默认就是 `4.5 bps`。
- 不能移除成本、不能采样最终曲线、不能后验挑窗口。
- 回测使用的指标，实盘也必须能计算；回测怎样决定入场、加仓、止盈、止损，实盘必须有同等语义。禁止把“阻止上线”当最终方案。
- 执行回测后必须估算 ETA、定时监控、自动进入下一步。不要启动回测后停下来等用户指示。
- 不要干扰 Claude 或其他 agent 正在执行的任务；启动/停止 worker 前必须确认没有其他 owner 的 `queued/running` 回测任务。

---

### Task 1: Cleanup DeepSeek Partial State

**Files:**
- Update report: `docs/superpowers/reports/2026-06-11-martingale-flyingkid-cleanup-and-baseline.md`
- Runtime DB only.

- [ ] **Step 1: Confirm no active backtests**

Run:

```bash
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -P pager=off -c "
SELECT task_id, owner, status, config->>'risk_profile' AS risk, summary->>'stage' AS stage
FROM backtest_tasks
WHERE status IN ('queued','running','paused')
ORDER BY updated_at DESC;
"
docker ps -a --format '{{.Names}} {{.Status}}' | grep 'grid-binance-backtest-worker'
free -h
```

Expected: no active tasks; workers stopped unless intentionally restarted later.

- [ ] **Step 2: Archive the cancelled DeepSeek task**

Archive `fk-18-conservative-seed211-20260611`:

```sql
UPDATE backtest_tasks
SET owner='archive+flyingkid2022@outlook.com',
    summary = summary || jsonb_build_object(
      'archived_from_owner','flyingkid2022@outlook.com',
      'archived_at', now()::text,
      'archive_reason','cancelled before plan update'
    ),
    updated_at=now()
WHERE task_id='fk-18-conservative-seed211-20260611'
  AND owner='flyingkid2022@outlook.com';
```

- [ ] **Step 3: Repair conservative baseline completeness**

`fk-18-conservative-baseline-from-v5-20260611` currently has summary but no candidate rows. Either:

1. Recopy `search-conservative-18sym-v5` candidates into the FlyingKid copy with new unique `candidate_id` values and rewritten member candidate references if needed.
2. Or rerun the conservative baseline under FlyingKid and let the worker generate full task/candidates/artifacts naturally.

Acceptance query:

```sql
SELECT task_id, count(*) AS candidates
FROM backtest_candidate_summaries
WHERE task_id IN ('fk-18-conservative-baseline-from-v5-20260611','search-conservative-18sym-v5')
GROUP BY task_id;
```

Expected: the FlyingKid conservative baseline has candidate rows sufficient for frontend candidate inspection and publish flow.

- [ ] **Step 4: Correct cleanup report**

Update `docs/superpowers/reports/2026-06-11-martingale-flyingkid-cleanup-and-baseline.md`:

```text
1. Mention fk-18-conservative-seed211-20260611 was cancelled and archived.
2. Correct FK cascade statement: backtest_candidate_summaries has ON DELETE CASCADE.
3. Record whether conservative candidate rows were copied or baseline was rerun.
4. Record exact SQL and verification queries.
```

---

### Task 2: Replace Blocking With Indicator Parity

**Files:**
- Modify:
  - `apps/backtest-engine/src/indicators.rs`
  - `apps/backtest-engine/src/martingale/kline_engine.rs`
  - `apps/backtest-engine/src/search.rs`
  - `apps/trading-engine/src/martingale_runtime.rs`
  - `apps/trading-engine/src/main.rs`
  - `apps/trading-engine/tests/martingale_runtime.rs`
  - `crates/shared-binance/src/client.rs` only if existing `fetch_klines` needs small extension.
- Prefer creating a shared indicator module if needed, but do not add new services unless the current module boundaries require it.

- [ ] **Step 1: Remove DeepSeek's final blocking stance**

Do not leave these as final behavior:

```text
ATR spacing disabled forever in search
ATR TP/SL rejected forever in live preflight
IndicatorExpression/ADX rejected forever in live preflight
```

Temporary guards may remain only while implementing, but final acceptance requires real support.

- [ ] **Step 2: Define exact shared semantics**

Write this behavior into code comments/tests:

```text
ATR/ADX are computed from completed 1m candles only.
Entry trigger for bar N uses indicators from candles <= N-1, not the unfinished current candle.
ATR spacing safety-order trigger prices are computed once at cycle start and frozen for that cycle.
ATR take-profit/stop-loss may be recalculated on each completed candle only if backtest uses the same rule; otherwise freeze the same snapshot.
Live runtime stores indicator snapshot used for each cycle in runtime summary/events for audit.
```

- [ ] **Step 3: Fix backtest ATR spacing**

Current backtest bug: `add_leg()` calls:

```rust
compute_leg_trigger_prices(price, direction, spacing, None, ...)
```

For `MartingaleSpacingModel::Atr`, pass the latest ATR available before cycle start. Add tests:

```text
atr_spacing_cycle_start_uses_previous_completed_candle
atr_spacing_trigger_prices_are_frozen_for_cycle
adx_entry_filter_does_not_use_future_or_current_unclosed_bar
```

- [ ] **Step 4: Re-enable ATR spacing search**

After Step 3 passes, revert DeepSeek's search disabling:

```text
StagedMartingaleSearchSpace.spacing_model includes FixedPercent and Atr again.
is_valid_spacing_for_model(Atr, ...) validates bounds instead of always false.
build_spacing_model(Atr, ...) emits MartingaleSpacingModel::Atr.
```

Run:

```bash
cargo test -p backtest-engine --lib staged_tests -- --nocapture
cargo test -p backtest-engine martingale::kline_engine -- --nocapture
```

- [ ] **Step 5: Implement live indicator runtime**

Use existing `shared-binance::BinanceClient::fetch_klines()`; it already maps USD-M to `/fapi/v1/klines`.

Implement for live martingale:

```text
warmup: fetch enough 1m klines per symbol and period, at least max(period)*3 or 200 candles
cache: maintain completed candles per symbol
refresh: poll REST periodically or consume kline stream if available; ignore unfinished candle
compute: ATR/ADX with same functions as backtest
evaluate: IndicatorExpression like "adx(14) > 25" before starting a new cycle
snapshot: persist atr/adx value, candle timestamp, expression result, and cycle id
```

- [ ] **Step 6: Support live ATR spacing**

Change `MartingaleRuntime` APIs as needed so `start_cycle` and `mark_leg_filled` can receive indicator snapshots.

Expected behavior:

```text
start_cycle(strategy_id, anchor_price, context, indicator_snapshot)
compute safety orders using latest ATR snapshot
store frozen trigger prices in cycle state
placing later legs uses frozen trigger prices, not None/latest ad hoc value
```

- [ ] **Step 7: Support live ATR TP/SL and ADX entry**

Implement:

```text
ATR take-profit price using the same latest/completed candle rule as backtest.
ATR stop-loss using same rule as backtest if strategy uses MartingaleStopLossModel::Atr.
IndicatorExpression entry triggers, including ADX, evaluated before opening a cycle.
Clear error only if indicator warmup is not ready, not because feature is unsupported.
```

- [ ] **Step 8: Live parity tests**

Add tests that compare backtest vs live runtime calculations on the same synthetic candles:

```bash
cargo test -p trading-engine --test martingale_runtime atr -- --nocapture
cargo test -p trading-engine --test martingale_runtime adx -- --nocapture
cargo test -p trading-engine --test order_sync -- --nocapture
cargo test -p trading-engine --test trade_sync -- --nocapture
cargo test -p backtest-engine martingale::kline_engine -- --nocapture
```

Acceptance: ATR/ADX configs that pass backtest can be started live after indicator warmup and exchange preflight.

---

### Task 3: Backtest Accuracy And Cost Gates

**Files:**
- Modify only if tests prove gaps:
  - `apps/backtest-engine/src/martingale/kline_engine.rs`
  - `apps/backtest-engine/src/martingale/metrics.rs`
  - `apps/backtest-engine/src/portfolio_search.rs`
  - `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Verify fee/slippage math**

Tests must prove:

```text
entry fee = entry notional * 4.5 / 10000
entry slippage = entry notional * 2.0 / 10000
exit fee = exit notional * 4.5 / 10000
exit slippage = exit notional * 2.0 / 10000
realized PnL subtracts entry and exit costs
```

- [ ] **Step 2: Verify annualized/DD**

Run:

```bash
cargo test -p backtest-engine --test search_scoring_time_splits annualized_return_uses_backtest_days -- --nocapture
cargo test -p backtest-engine portfolio -- --nocapture
```

Acceptance: annualized return uses actual days, combined portfolio DD uses combined equity curve.

- [ ] **Step 3: Funding fee policy**

Backtest currently clearly tracks fee/slippage; funding fee integration is not proven. Decide and implement one:

```text
Preferred: include historical funding if available in local data and document formula.
Fallback: explicitly label all backtest results "excluding funding" and include expected live funding risk in final report.
```

Do not silently compare funding-excluding backtest returns to live funding-including stats.

---

### Task 4: Worker Speed, ETA, And Monitoring Loop

**Files:**
- Modify:
  - `apps/backtest-worker/src/main.rs`
  - `deploy/docker/docker-compose.yml`
  - optional helper script: `scripts/monitor_martingale_backtests.sh`

- [ ] **Step 1: Fix timing log bug**

Current `rss_mb` prints `N/A`. Print numeric RSS correctly.

Expected log:

```text
TIMING task_id=... stage=kline_load rss_mb=6585 extra={...}
```

- [ ] **Step 2: Persist progress and ETA into DB**

Every stage heartbeat must include:

```json
{
  "stage": "search_symbol|trade_refinement|portfolio|completed",
  "progress_pct": 0,
  "current_symbol": "BTCUSDT",
  "processed_symbols": 0,
  "total_symbols": 18,
  "processed_candidates": 0,
  "total_candidates": 0,
  "started_at_ms": 0,
  "elapsed_secs": 0,
  "eta_secs": 0,
  "eta_text": "2h 15m",
  "rss_mb": 0,
  "worker_threads": 12
}
```

For refinement, chunk completion must update DB, not only stderr.

- [ ] **Step 3: Add monitor loop**

Create or document a monitor loop that runs every 5 minutes while tasks are active:

```bash
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -P pager=off -c "
SELECT task_id, owner, status,
       summary->>'stage' AS stage,
       summary->>'progress_pct' AS pct,
       summary->>'current_symbol' AS symbol,
       summary->>'eta_text' AS eta,
       summary->>'rss_mb' AS rss_mb,
       updated_at
FROM backtest_tasks
WHERE owner='flyingkid2022@outlook.com'
ORDER BY updated_at DESC
LIMIT 20;
"
docker stats --no-stream | grep backtest || true
```

Stall detection:

```text
If updated_at unchanged for >20 minutes and worker CPU <5%, inspect logs.
If task failed/cancelled, archive it and automatically submit next seed.
If task succeeded, summarize result and automatically decide whether to continue or promote winner.
```

- [ ] **Step 4: Worker resource limits**

Keep:

```text
BACKTEST_WORKER_MAX_THREADS=12 initially
scale backtest-worker=1 until first full task runtime is known
scale to 2 only if memory and CPU remain stable
```

Do not start all seeds at once.

---

### Task 5: Automated Search Execution

**Files:**
- Runtime DB/API.
- Final report:
  - `docs/superpowers/reports/2026-06-11-martingale-three-risk-search-report.md`

- [ ] **Step 1: Before starting each task**

Run:

```sql
SELECT task_id, owner, status FROM backtest_tasks WHERE status IN ('queued','running','paused');
```

If any non-DeepSeek/Claude task is active, do not stop it. Coordinate by waiting or using a separate worker only if safe.

- [ ] **Step 2: Conservative search**

Goal: `annualized_return_pct > 50` and `max_drawdown_pct <= 10`.

Start with one task:

```text
fk-18-conservative-seed307-20260611
```

Use 18-symbol universe, full window from `2023-01-01`, cost `fee_bps=4.5`, `slippage_bps=2.0`, `portfolio_top_n=3`, `per_symbol_top_n=40`, `random_candidates=256`.

After it completes:

```text
If best ann >50 and DD<=10: promote and archive old conservative baseline.
If not: run next seed automatically: 521, 887, 1597.
If all fail: expand parameter ranges or member allocation templates, then continue.
```

- [ ] **Step 3: Balanced search**

Goal: beat `fk-18-bal-v2-seed53-20260601` rank1 `65.52% / 19.32%`.

Seeds:

```text
67, 173, 307, 521
```

Use compact portfolio optimizer and ATR/ADX only after parity tests pass.

- [ ] **Step 4: Aggressive search**

Goal: beat `fk-18-agg-v2-seed173-20260601` rank1 `77.00% / 28.03%`, then challenge `~100.5%`.

Seeds:

```text
67, 211, 307, 521
```

DD must remain `<=30%`.

- [ ] **Step 5: Never stop waiting for user mid-search**

DeepSeek must keep a monitor loop running and proceed automatically:

```text
task running -> monitor ETA every 5 minutes
task succeeded -> extract result, compare target, promote/archive or submit next seed
task failed/timed out -> archive failed task and submit next seed with adjusted params
all seed plan exhausted -> inspect bottleneck, adjust search space, run next batch
```

Only stop if:

```text
critical code/test failure blocks correctness
machine memory pressure risks system stability
Claude/another owner has active tasks and coordination is required
all three risk goals are met and final report is written
```

- [ ] **Step 6: Final report tables**

Report:

```text
Risk | Task ID | Annualized % | Max DD % | Return % | Ann/DD | Members | Unique Symbols | Max Symbol Weight % | Eq Points | Max Gap Days | Cost/Funding Notes
Symbol | Direction | Allocation % | Leverage | Candidate Ann % | Candidate DD % | Spacing | TP | ATR | ADX | Trades
```

---

### Task 6: Verification And Commit Guidance

**Files:**
- All modified Rust files.
- Reports under `docs/superpowers/reports/`.

- [ ] **Step 1: Required tests**

Run at minimum:

```bash
cargo check --workspace --lib
cargo test -p backtest-engine --lib -- --nocapture
cargo test -p backtest-engine --test search_scoring_time_splits -- --nocapture
cargo test -p backtest-worker --lib -- --nocapture
cargo test -p trading-engine --test martingale_runtime -- --nocapture
cargo test -p trading-engine --test order_sync -- --nocapture
cargo test -p trading-engine --test trade_sync -- --nocapture
cargo test -p api-server --lib live -- --nocapture
cd apps/web && ./node_modules/.bin/tsc --noEmit --incremental false
```

- [ ] **Step 2: Commit structure**

Commit in small groups:

```text
1. fix(backtest): ATR/ADX indicator parity and accuracy gates
2. feat(trading-engine): live martingale ATR/ADX indicator runtime
3. feat(backtest-worker): ETA/progress monitoring for martingale search
4. chore(backtest): FlyingKid cleanup and reports
```

Each commit log must include:

```text
问题描述:
修复思路:
验证:
```

---

## Final Acceptance Checklist

- [ ] No active unmanaged backtest worker remains.
- [ ] FlyingKid visible tasks include exactly one best conservative, balanced, aggressive result.
- [ ] Cancelled task `fk-18-conservative-seed211-20260611` archived.
- [ ] Conservative baseline has candidate rows or has been rerun naturally.
- [ ] ATR spacing, ATR TP/SL, and ADX entry triggers are implemented in live if used by backtest.
- [ ] Backtest and live share equivalent completed-candle indicator semantics.
- [ ] Backtest ETA and DB progress update during long refinement.
- [ ] Search runs automatically through seeds and next steps without waiting for user after each run.
- [ ] Conservative target `>50%` is met or final report proves exact bottleneck and next expansion path.
- [ ] Balanced/aggressive improved over FlyingKid baselines or final report proves why not.
- [ ] Final report and verification commands are complete.

