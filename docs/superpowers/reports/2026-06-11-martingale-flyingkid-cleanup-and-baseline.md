# Martingale FlyingKid Cleanup & Baseline Report

**Date:** 2026-06-11
**Executor:** DeepSeek (via Claude Code agent)
**Status:** ✅ Complete

---

## Pre-Cleanup State

| Metric | Value |
|--------|-------|
| Workers running | 0 (all Exited 137) |
| Queued/running tasks | 0 |
| Memory | 18Gi used / 178Gi available |
| Swap | 6.8Gi / 100Gi |
| FlyingKid tasks | 8 (6 succeeded, 2 failed) |
| FlyingKid portfolios | 0 (no cascade risk) |

---

## Archive Policy Applied

**Method:** Owner rename to `archive+flyingkid2022@outlook.com` (no hard delete, no cascade)

**Archived tasks (7):**

| Task ID | Risk | Status | Ann% | DD% | Reason |
|---------|------|--------|------|-----|--------|
| `fk-18-balanced-wide-seed127-20260530` | balanced | succeeded | 59.92 | 19.59 | Superseded by 65.52% |
| `fk-18-balanced-atradx-seed307-20260605` | balanced | succeeded | 47.61 | 19.37 | ATR/ADX — accuracy rules pending; superseded by 65.52% |
| `fk-18-balanced-atradx-seed521-20260608` | balanced | failed | — | — | Timeout: NEARUSDT long_short |
| `fk-18-aggressive-atradx-seed521-20260608` | aggressive | succeeded | 63.06 | 29.75 | ATR/ADX — accuracy rules pending; superseded by 77.00% |
| `fk-18-aggressive-atradx-seed307-20260608` | aggressive | succeeded | 61.56 | 29.28 | ATR/ADX — accuracy rules pending; superseded by 77.00% |
| `fk-18-conservative-atradx-seed211-20260608` | conservative | failed | — | — | Timeout: DASHUSDT long_short |
| `fk-18-conservative-seed211-20260611` | conservative | cancelled | — | — | Cancelled before completion by plan update (2026-06-11) |

---

## Kept Visible (FlyingKid)

| Risk | Task ID | Annualized% | Max DD% | Total Return% |
|------|---------|------------|---------|---------------|
| **conservative** | `fk-18-conservative-baseline-from-v5-20260611` | 40.69 | 9.66 | 121.64 |
| **balanced** | `fk-18-bal-v2-seed53-20260601` | 65.52 | 19.32 | 435.96 |
| **aggressive** | `fk-18-agg-v2-seed173-20260601` | 77.00 | 28.03 | 570.05 |

> Conservative baseline copied from `search-conservative-18sym-v5` (originally `system@optimization`). Original task preserved.

---

## Cascade Risk Assessment

| Related Table | FK Constraint | Rows Linked | Risk |
|---------------|---------------|-------------|------|
| `martingale_portfolios` | `source_task_id → backtest_tasks(task_id) ON DELETE CASCADE` | **0** | None |
| `martingale_portfolio_items` | via `martingale_portfolios` cascade | **0** | None |
| `backtest_candidate_summaries` | `task_id → backtest_tasks(task_id) ON DELETE CASCADE` | **59 (conservative baseline)** / 342 (all) | CASCADE — hard delete would remove all linked candidates |
| `backtest_task_events` | `task_id → backtest_tasks(task_id) ON DELETE CASCADE` | 1061 | CASCADE — hard delete would remove all linked events |

**Conclusion:** No hard deletes were performed. Owner rename preserves all related rows.

---

## SQL Executed

```sql
-- Archive cancelled DeepSeek task (2026-06-11 follow-up)
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

-- Archive superseded/failed tasks
UPDATE backtest_tasks
SET owner = 'archive+flyingkid2022@outlook.com',
    summary = summary || jsonb_build_object(
      'archived_from_owner', 'flyingkid2022@outlook.com',
      'archived_at', now()::text,
      'archive_reason', CASE
        WHEN status = 'failed' THEN 'failed martingale exploration'
        WHEN task_id LIKE '%atradx%' THEN 'ATR/ADX invalid under pending accuracy rules'
        ELSE 'superseded by better result in same risk tier'
      END
    ),
    updated_at = now()
WHERE owner = 'flyingkid2022@outlook.com'
  AND task_id NOT IN ('fk-18-bal-v2-seed53-20260601', 'fk-18-agg-v2-seed173-20260601');

-- Copy conservative baseline
INSERT INTO backtest_tasks (task_id, owner, status, strategy_type, config, summary, created_at, updated_at)
SELECT 'fk-18-conservative-baseline-from-v5-20260611',
       'flyingkid2022@outlook.com',
       status, strategy_type,
       config || jsonb_build_object('owner', 'flyingkid2022@outlook.com', 'copied_from', 'search-conservative-18sym-v5', 'copied_at', now()::text),
       summary || jsonb_build_object('copied_from_task', 'search-conservative-18sym-v5', 'copied_at', now()::text, 'copy_note', 'Conservative baseline copied for FlyingKid visibility'),
       now(), now()
FROM backtest_tasks WHERE task_id = 'search-conservative-18sym-v5';

-- Fix missing candidate rows for conservative baseline (2026-06-11 follow-up)
INSERT INTO backtest_candidate_summaries (candidate_id, task_id, status, rank, config, summary, created_at, updated_at)
SELECT
  'bl_' || candidate_id AS candidate_id,
  'fk-18-conservative-baseline-from-v5-20260611' AS task_id,
  status,
  rank,
  config,
  summary || jsonb_build_object('copied_from_candidate', candidate_id, 'copied_from_task', 'search-conservative-18sym-v5'),
  now(),
  now()
FROM backtest_candidate_summaries
WHERE task_id='search-conservative-18sym-v5'
ORDER BY rank;
```

**Verification (2026-06-11):**

```sql
SELECT task_id, count(*) AS candidates
FROM backtest_candidate_summaries
WHERE task_id IN ('fk-18-conservative-baseline-from-v5-20260611','search-conservative-18sym-v5')
GROUP BY task_id;
-- Result: both tasks have 59 candidates
```

## Candidate Rows Fix Record

- **Problem:** `fk-18-conservative-baseline-from-v5-20260611` was created by copying only `backtest_tasks` row from `search-conservative-18sym-v5`, without `backtest_candidate_summaries` rows. The frontend candidate inspection and publish flow relied on these rows.
- **Fix:** Copied 59 candidate rows from `search-conservative-18sym-v5` to the conservative baseline task with new unique `candidate_id` values (prefixed with `bl_`). Each row carries a `copied_from_candidate` and `copied_from_task` marker in its summary JSON.
- **Status:** Verified both tasks have 59 candidates each.

---

## Snapshots Saved

- `/tmp/flyingkid-backtest-tasks-20260611.json` — Full task export (pre-archive)
- `/tmp/flyingkid-backtest-portfolios-20260611.json` — Portfolio rows (empty, 0 rows)
