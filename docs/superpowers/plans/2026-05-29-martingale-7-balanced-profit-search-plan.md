# Martingale 7-Symbol Balanced Profit Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 先用 7 币种 mixed 平衡策略，在真实全量 1m K 线、手续费和滑点成本下，找出年化尽量接近或超过 50%、最大回撤不超过 20%、资金曲线和回撤曲线完整可信的组合。

**Architecture:** 先完成回测真实性修复：最终候选必须全量 1m K 线精测，组合曲线必须按时间戳对齐，前端/summary 只展示采样预览但组合计算不得使用预览曲线。然后只跑 7 币种 balanced mixed 多轮分阶段搜索：小预算验证曲线，中预算找方向，多 seed/扩参深搜，最后才扩展到 7/18 × 保守/平衡/激进。

**Tech Stack:** Rust workspace (`backtest-engine`, `backtest-worker`), PostgreSQL `backtest_tasks`, Docker Compose workers, Next.js verification tests, local SQLite market data `/home/bumblebee/Project/discord_c2im/pipeline/data/market_data.db`.

---

## Critical Constraints

- Do **not** run 7/18 × conservative/balanced/aggressive immediately. That is too expensive.
- First target only: **7 symbols + balanced + mixed_best**.
- Required 7 symbols: `BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT, XRPUSDT, DOGEUSDT, ADAUSDT`.
- Backtest range must be full: `2023-01-01 00:00:00 UTC` through previous month end, currently `2026-04-30 23:59:00 UTC`.
- Fee must remain `4.5 bps` (`0.045%`).
- Slippage must remain `2.0 bps` unless explicitly documented in result comparison.
- Balanced max drawdown target: `<= 20%`.
- Do not remove costs, shorten data, or use sampled final curves to manufacture high returns.
- If 50% annualized is not achievable after honest search, report the best real result and why.

---

## Current Known State

There are already local uncommitted changes from previous investigation. Deepseek must inspect them before editing:

```bash
git status --short
git diff -- apps/backtest-engine/src/portfolio_search.rs apps/backtest-engine/src/search.rs apps/backtest-worker/src/main.rs apps/web/components/backtest/backtest-wizard.tsx tests/verification/backtest_console_contract.test.mjs
```

Important previous findings:

1. Old portfolio curve had huge gaps (`577 days`, `514.5 days`). This was caused by sampled/preview curves and index-based merge.
2. `combine_equity_curves()` should align by `timestamp_ms`, not array index.
3. `portfolio_candidates_from_outputs()` should prefer full `output.equity_curve`, not `summary.equity_curve` preview.
4. `run_candidate_kline_screening()` must use full `bars_for_candidate()`, not `screening_bars_for_candidate()`, for final candidate refinement.
5. `screening_bars_for_candidate()` may remain only for coarse screening.

---

## Task 1: Lock Full-Curve Correctness Before Any Search

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`
- Modify: `apps/backtest-worker/src/main.rs`
- Test: inline Rust unit tests in both files

- [ ] **Step 1: Verify portfolio curve timestamp alignment test exists**

Ensure this test exists in `apps/backtest-engine/src/portfolio_search.rs`:

```rust
#[test]
fn weighted_portfolio_aligns_member_equity_by_timestamp_not_index() {
    let mut btc = fixture_candidate("btc", "BTCUSDT", 30.0, 10.0, 3.0);
    btc.planned_margin_quote = 100.0;
    btc.equity_curve = vec![
        EquityPoint { timestamp_ms: 1, equity_quote: 100.0 },
        EquityPoint { timestamp_ms: 2, equity_quote: 120.0 },
        EquityPoint { timestamp_ms: 3, equity_quote: 130.0 },
    ];

    let mut eth = fixture_candidate("eth", "ETHUSDT", 20.0, 8.0, 2.0);
    eth.planned_margin_quote = 100.0;
    eth.equity_curve = vec![
        EquityPoint { timestamp_ms: 1, equity_quote: 100.0 },
        EquityPoint { timestamp_ms: 3, equity_quote: 110.0 },
    ];

    let portfolio = build_weighted_portfolio(&[&btc, &eth], &[0, 1], &[0.5, 0.5], 30.0)
        .expect("portfolio should build");

    assert_eq!(
        portfolio.equity_curve.iter().map(|point| point.timestamp_ms).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );

    let by_ts = portfolio
        .equity_curve
        .iter()
        .map(|point| (point.timestamp_ms, point.equity_quote))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert!((by_ts[&1] - 10_000.0).abs() < 0.0001);
    assert!((by_ts[&2] - 11_000.0).abs() < 0.0001);
    assert!((by_ts[&3] - 12_000.0).abs() < 0.0001);
}
```

- [ ] **Step 2: Verify `combine_equity_curves()` uses timestamp union and forward-fill**

The function in `apps/backtest-engine/src/portfolio_search.rs` must:

```rust
let mut timestamps = members
    .iter()
    .flat_map(|(candidate, _)| candidate.equity_curve.iter().map(|point| point.timestamp_ms))
    .collect::<Vec<_>>();
timestamps.sort_unstable();
timestamps.dedup();

let mut member_positions = vec![0usize; members.len()];
let mut latest_equities = initial_equities.clone();

for timestamp_ms in timestamps {
    for (idx, (candidate, _)) in members.iter().enumerate() {
        while member_positions[idx] < candidate.equity_curve.len()
            && candidate.equity_curve[member_positions[idx]].timestamp_ms <= timestamp_ms
        {
            latest_equities[idx] = candidate.equity_curve[member_positions[idx]].equity_quote;
            member_positions[idx] += 1;
        }
    }
    // combine allocated capital using latest_equities[idx] / initial_equities[idx]
}
```

- [ ] **Step 3: Verify candidate-to-portfolio conversion prefers full output curves**

`apps/backtest-worker/src/main.rs` in `portfolio_candidates_from_outputs()` must use:

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

- [ ] **Step 4: Verify final kline refinement uses full bars**

`apps/backtest-worker/src/main.rs` must have:

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

Do not use `screening_bars_for_candidate()` in final refinement.

- [ ] **Step 5: Run correctness tests**

Run:

```bash
cargo test -p backtest-engine weighted_portfolio_aligns_member_equity_by_timestamp_not_index -- --nocapture
cargo test -p backtest-worker portfolio_candidates_prefer_full_output_curve_over_summary_preview -- --nocapture
cargo test -p backtest-worker final_kline_refinement_uses_full_symbol_history_not_screening_sample -- --nocapture
cargo test -p backtest-worker long_short_uses_configured_risk_profile_drawdown_limits -- --nocapture
cargo check -p backtest-worker
```

Expected: all exit `0`; existing dead_code warnings are acceptable, errors are not.

---

## Task 2: Deploy Fixed Worker And Run One Cheap 7-Balanced Curve Validation

**Files:**
- No source change expected
- Runtime: Docker Compose
- DB: Postgres `backtest_tasks`

- [ ] **Step 1: Rebuild and restart workers**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml build backtest-worker

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml up -d --scale backtest-worker=4 backtest-worker
```

- [ ] **Step 2: Create cheap validation task**

Create only one task first: `validation-7-mixed-balanced-fullcurve-smoke-20260529`.

Use base config from `validation-7-mixed-tailfix3-202605271850`, but override:

```json
{
  "risk_profile": "balanced",
  "direction_mode": "mixed_best",
  "random_candidates": 64,
  "per_symbol_top_n": 10,
  "portfolio_top_n": 3,
  "fee_bps": 4.5,
  "slippage_bps": 2.0,
  "priority": 700
}
```

SQL template:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -c "
INSERT INTO backtest_tasks (task_id, owner, status, strategy_type, config, summary, created_at, updated_at)
SELECT
  'validation-7-mixed-balanced-fullcurve-smoke-20260529',
  owner,
  'queued',
  strategy_type,
  jsonb_set(
    jsonb_set(
      jsonb_set(
        jsonb_set(
          jsonb_set(
            jsonb_set(config, '{risk_profile}', to_jsonb('balanced'::text)),
            '{random_candidates}', '64'::jsonb
          ),
          '{per_symbol_top_n}', '10'::jsonb
        ),
        '{portfolio_top_n}', '3'::jsonb
      ),
      '{priority}', '700'::jsonb
    ),
    '{direction_mode}', to_jsonb('mixed_best'::text)
  ),
  jsonb_build_object('stage_label','queued 7-symbol balanced fullcurve smoke','source_task_id', task_id),
  now(),
  now()
FROM backtest_tasks
WHERE task_id='validation-7-mixed-tailfix3-202605271850'
ON CONFLICT (task_id) DO UPDATE
SET status='queued', config=EXCLUDED.config, summary=EXCLUDED.summary,
    started_at=NULL, completed_at=NULL, error_message=NULL, updated_at=now();
"
```

- [ ] **Step 3: Wait for smoke task completion**

Poll:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -c "
SELECT task_id,status,summary->>'stage_label' stage,
       round((jsonb_path_query_first(summary,'$.portfolio_top3[0].annualized_return_pct')::text)::numeric,2) ann,
       round((jsonb_path_query_first(summary,'$.portfolio_top3[0].max_drawdown_pct')::text)::numeric,2) dd,
       error_message, now()-updated_at age
FROM backtest_tasks
WHERE task_id='validation-7-mixed-balanced-fullcurve-smoke-20260529';
"
```

Expected: status `succeeded`.

- [ ] **Step 4: Verify curve integrity**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -c "
WITH p AS (
  SELECT summary->'portfolio_top3'->0 AS pf
  FROM backtest_tasks
  WHERE task_id='validation-7-mixed-balanced-fullcurve-smoke-20260529'
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
- full range from `2023-01-01` to `2026-04-30`
- no giant `577`/`514` day gaps
- `max_gap_days` should be around normal preview sampling interval, not hundreds of days

If this fails, stop. Do not run deep search until curve is fixed.

---

## Task 3: 7-Symbol Balanced Profit Search Ladder

**Files:**
- Possibly modify: `apps/backtest-engine/src/search.rs`
- Possibly modify: `apps/backtest-engine/src/portfolio_search.rs`
- Runtime: `backtest_tasks`

### Stage A: Multi-seed medium-budget search

- [ ] **Step 1: Create 5 medium tasks**

Task IDs:

```text
validation-7-mixed-balanced-seed17-20260529
validation-7-mixed-balanced-seed29-20260529
validation-7-mixed-balanced-seed43-20260529
validation-7-mixed-balanced-seed71-20260529
validation-7-mixed-balanced-seed101-20260529
```

Config overrides:

```json
{
  "risk_profile": "balanced",
  "direction_mode": "mixed_best",
  "random_candidates": 192,
  "per_symbol_top_n": 20,
  "portfolio_top_n": 3,
  "leverage_range": [1, 20],
  "fee_bps": 4.5,
  "slippage_bps": 2.0,
  "random_seed": 17/29/43/71/101
}
```

- [ ] **Step 2: Compare results**

For each task, record:

```sql
SELECT task_id,status,
       round((jsonb_path_query_first(summary,'$.portfolio_top3[0].annualized_return_pct')::text)::numeric,2) ann,
       round((jsonb_path_query_first(summary,'$.portfolio_top3[0].max_drawdown_pct')::text)::numeric,2) dd,
       jsonb_path_query_first(summary,'$.portfolio_top3[0].member_count') members,
       jsonb_path_query_first(summary,'$.portfolio_top3[0].portfolio_unique_symbol_count') uniq
FROM backtest_tasks
WHERE task_id LIKE 'validation-7-mixed-balanced-seed%-20260529'
ORDER BY ann DESC NULLS LAST;
```

Proceed to Stage B only after at least one task succeeds and curve integrity passes.

### Stage B: Wider parameter search if annualized < 45%

If best annualized is below `45%`, widen search in `apps/backtest-engine/src/search.rs` inside `StagedMartingaleSearchSpace::profit_optimized_v2()`.

Add/ensure these tails:

```rust
space.leverage = vec![2, 3, 4, 5, 6, 8, 10, 12, 15, 20];
space.spacing_bps = vec![25, 35, 50, 70, 90, 120, 160, 220, 300, 420, 600, 800];
space.order_multiplier = vec![1.1, 1.15, 1.25, 1.4, 1.6, 1.8, 2.0, 2.2, 2.4, 2.8];
space.max_legs = vec![3, 4, 5, 6, 7, 8, 9, 10];
space.take_profit_bps = vec![25, 30, 45, 60, 80, 100, 140, 200, 300, 450];
space.tail_stop_bps = vec![600, 800, 1200, 1800, 2400, 3000, 4000, 5500, 7000, 9000];
```

For `long_short`, include asymmetric weights:

```rust
space.long_short_weight_pct = vec![
    (90, 10), (80, 20), (70, 30), (60, 40), (50, 50),
    (40, 60), (30, 70), (20, 80), (10, 90),
];
```

Then run tests:

```bash
cargo test -p backtest-engine aggressive_profit_search_v2_covers_wide_spacing_and_profit_targets -- --nocapture
cargo test -p backtest-worker profit_optimized_v2_merges_narrow_user_presets_with_wide_tail -- --nocapture
cargo check -p backtest-worker
```

Deploy worker again before rerunning searches.

### Stage C: Deep budget only for best seed/space

After Stage A/B identifies the best direction, run one deep task only:

Task ID:

```text
validation-7-mixed-balanced-deep-best-20260529
```

Config:

```json
{
  "risk_profile": "balanced",
  "direction_mode": "mixed_best",
  "random_candidates": 512,
  "per_symbol_top_n": 30,
  "portfolio_top_n": 3,
  "random_seed": <best seed from Stage A>,
  "leverage_range": [1, 20],
  "fee_bps": 4.5,
  "slippage_bps": 2.0
}
```

Success target:
- annualized ideally `>= 50%`
- max drawdown `<= 20%`
- curve integrity passes
- portfolio members and leverage displayed

---

## Task 4: Only After 7-Balanced Is Stable, Expand To Larger Matrix

Do not start this task until Task 3 has a validated best 7-balanced portfolio.

- [ ] **Step 1: Run 7-symbol conservative/aggressive only after balanced baseline**

Use the best parameter/search settings found in Task 3.

- [ ] **Step 2: Run 18-symbol balanced next**

Do not run all 18 × 3 at once. First run:

```text
validation-18-mixed-balanced-bestspace-20260529
```

Use same best search space and seed strategy.

- [ ] **Step 3: If 18-balanced improves diversification or yield, then run 18-conservative and 18-aggressive**

Only then run:

```text
validation-18-mixed-conservative-bestspace-20260529
validation-18-mixed-aggressive-bestspace-20260529
```

---

## Result Report Format

Deepseek must report using this table:

```text
Task ID | Risk | Symbols | Seed | Random Candidates | Annualized % | Max DD % | Members | Unique Symbols | Curve Max Gap Days | Status
```

And for best portfolio:

```text
Symbol | Direction Mode | Allocation % | Leverage | Return % | Annualized % | Max DD % | Trades
```

Also include:

```text
Conclusion:
- 是否达到年化 50%: Yes/No
- 是否满足平衡回撤 <=20%: Yes/No
- 资金曲线是否完整: Yes/No, max_gap_days=...
- 是否可以进入 18 币种扩展: Yes/No
- 下一步建议: ...
```

---

## Final Guardrails

- If curve max gap is hundreds of days, stop and fix curve before interpreting return.
- If annualized is high but curve is incomplete, treat result as invalid.
- If annualized is below 50 but data is honest, do not fake it; propose seeds/search-space/portfolio optimizer improvements.
- Do not run 18-symbol or three risk profiles until 7-balanced is validated.
