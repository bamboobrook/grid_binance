# 马丁最优组合策略搜索 — 监控与迭代计划

> **目标：** 找到保守模式年化 >50%、平衡和激进模式年化更高的最优组合
> **创建时间：** 2026-06-10
> **状态：** 进行中 — 两个任务在 trade_refinement 阶段

---

## 一、当前进度总览

### 已完成的历史任务

| 任务 ID | 风险档位 | 状态 | 单策略最高年化 | 组合 Top1 年化 | 备注 |
|---------|---------|------|--------------|--------------|------|
| `validation-7-symbol-v2` | Aggressive | ✅ succeeded | 86.6% (BTCUSDT long) | ~100.5% (BTC+DOGE) | 7币种 |
| `validation-18-symbol-v2` | Aggressive | ✅ succeeded | 75.7% (BTCUSDT) | ~100.5% (DOGE+FIIL) | 18币种 |
| `search-conservative-18sym-v2` | Conservative | ❌ 超时 | - | - | long_short 方向 BTCUSDT 超时 |
| `search-balanced-18sym-v2` | Balanced | ❌ 超时 | - | - | long_short 方向 BTCUSDT 超时 |
| `search-conservative-18sym-v3` | Conservative | ❌ Worker崩溃 | - | - | Worker-2 在精测阶段崩溃重启 |
| `search-balanced-18sym-v3` | Balanced | ❌ 状态丢失 | 102.8% (BNBUSDT) | - | 69/123候选写入，组合阶段状态丢失 |

### 正在运行的任务（截至 2026-06-10 ~09:30 UTC+8）

| 任务 ID | 风险档位 | 阶段 | 候选数 | Worker | CPU | 运行时间 |
|---------|---------|------|--------|--------|-----|---------|
| `search-conservative-18sym-v4` | Conservative | trade_refinement (80%) | 122 | Worker-3 (健康) | 1220% | ~8.3h |
| `search-balanced-18sym-v4` | Balanced | trade_refinement (80%) | 123 | Worker-2 (健康) | 1350% | ~7.4h |

### 目标 vs 现状

| 风险档位 | 目标 | Aggressive 已达标 | Conservative 待验证 | Balanced 待验证 |
|---------|------|------------------|-------------------|----------------|
| Conservative | 组合年化 > 50% | - | 🔵 精测中 | - |
| Balanced | 组合年化 > Aggressive(~100.5%) | - | - | 🔵 精测中 |
| Aggressive | 更高年化 | ✅ ~100.5% | - | - |

---

## 二、关键经验教训

### 1. long_short 方向会超时
- **问题：** `long_short` 搜索空间翻倍，BTCUSDT 在 80 分钟 timeout 内超时
- **解决：** 改用 `long` 方向，aggressive 结果显示 long-only 已可达到优秀收益

### 2. Worker 可能崩溃导致任务卡死
- **问题：** Worker-2 在精测阶段崩溃重启，任务状态变为孤立（running 但无 worker 处理）
- **检测方法：** 检查 `docker stats` 中对应 worker CPU 是否为 0%
- **解决：** 手动标记任务为 failed，重新提交

### 3. 精测阶段不更新 DB
- **现象：** `evaluate_refinement_candidates_parallel` 函数并行处理所有候选，全部完成后才批量返回
- **影响：** 122-123 个候选精测可能需要 5-6 小时，期间 summary 不更新
- **判断标准：** Worker CPU 高负载 = 正常运行；CPU 0% = 可能卡死

### 4. 精测后组合阶段也可能出问题
- **现象：** balanced v3 完成精测后，在写入候选和组合阶段丢失状态
- **检测：** `backtest_candidate_summaries` 有候选但无 portfolio 结果

---

## 三、监控操作手册

### 检查任务状态

```bash
# 1. 查询任务状态
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -t -c "
SELECT task_id, status, COALESCE(summary->>'progress_pct','0') as pct, 
       COALESCE(summary->>'stage','') as stage, COALESCE(summary->>'current_symbol','-') as sym
FROM backtest_tasks 
WHERE task_id IN ('search-conservative-18sym-v4', 'search-balanced-18sym-v4');
"

# 2. 检查 Worker CPU（判断是否在运行）
docker stats --no-stream 2>&1 | grep backtest | awk '{print $2, $3}'

# 3. 检查 Worker 健康状态
for w in 2 3 4 5; do
    docker inspect grid-binance-backtest-worker-$w --format "worker-$w: restarts={{.RestartCount}}" 2>&1
done

# 4. 检查候选是否已写入
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -t -c "
SELECT task_id, count(*) FROM backtest_candidate_summaries 
WHERE task_id IN ('search-conservative-18sym-v4', 'search-balanced-18sym-v4')
GROUP BY task_id;
"
```

### 判断 Worker 是否卡死

```
CPU > 500%  → 正常运行，继续等待
CPU < 5%    → 可能卡死或完成，需要检查：
  1. 查任务状态是否变为 succeeded/failed
  2. 查 Worker logs: docker logs grid-binance-backtest-worker-X --tail 20
  3. 如果 running + CPU=0% 超过 10 分钟 → 标记 failed 并重新提交
```

### 标记卡死任务并重新提交

```bash
# 标记失败
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -c "
UPDATE backtest_tasks 
SET status = 'failed', error_message = 'Worker lost task state', completed_at = NOW()
WHERE task_id = '<STUCK_TASK_ID>';
"

# 重新提交（修改 task_id, random_seed, risk_profile）
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -c "
INSERT INTO backtest_tasks (task_id, owner, status, strategy_type, config, summary)
VALUES (
    '<NEW_TASK_ID>',
    'system@optimization',
    'queued',
    'martingale_grid',
    '<CONFIG_JSON>'::jsonb,
    '{}'::jsonb
);
"
```

---

## 四、任务完成后的结果分析

### 步骤 1：检查任务状态

```bash
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -t -c "
SELECT task_id, status, error_message FROM backtest_tasks 
WHERE task_id IN ('search-conservative-18sym-v4', 'search-balanced-18sym-v4');
"
```

### 步骤 2：查看单策略 Top 候选

```bash
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -t -c "
SELECT cs.rank,
       cs.config->'strategies'->0->>'symbol' as symbol,
       cs.config->'strategies'->0->>'direction' as direction,
       cs.summary->>'annualized_return_pct' as ann_ret,
       cs.summary->>'return_pct' as total_ret,
       cs.summary->>'max_drawdown_pct' as max_dd,
       cs.summary->>'trade_count' as trades
FROM backtest_candidate_summaries cs
WHERE cs.task_id = '<TASK_ID>'
ORDER BY (cs.summary->>'return_pct')::numeric DESC NULLS LAST
LIMIT 20;
"
```

### 步骤 3：查看每个币种最佳候选

```bash
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -t -c "
SELECT DISTINCT ON (config->'strategies'->0->>'symbol') 
       config->'strategies'->0->>'symbol' as symbol,
       summary->>'annualized_return_pct' as ann_ret,
       summary->>'return_pct' as total_ret,
       summary->>'max_drawdown_pct' as max_dd
FROM backtest_candidate_summaries 
WHERE task_id = '<TASK_ID>'
ORDER BY symbol, (summary->>'annualized_return_pct')::numeric DESC;
"
```

### 步骤 4：查看组合 Top3

```bash
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -t -c "
SELECT summary->'portfolio_top3'->0->>'return_pct' as top1_return,
       summary->'portfolio_top3'->0->>'score' as top1_score,
       summary->'portfolio_top3'->0->'members'->0->>'symbol' as m1_sym,
       summary->'portfolio_top3'->0->'members'->0->>'return_pct' as m1_ret,
       summary->'portfolio_top3'->0->'members'->0->>'allocation_pct' as m1_alloc,
       summary->'portfolio_top3'->0->'members'->1->>'symbol' as m2_sym,
       summary->'portfolio_top3'->0->'members'->1->>'return_pct' as m2_ret,
       summary->'portfolio_top3'->0->'members'->1->>'allocation_pct' as m2_alloc
FROM backtest_tasks WHERE task_id = '<TASK_ID>';
"
```

### 步骤 5：计算组合年化

```python
# 回测期间：2023-01-01 ~ 2025-04-30 = 2.33 年
years = 850 / 365.25  # = 2.327
portfolio_total_return = float(top1_return) / 100  # e.g. 404.9% → 4.049
annualized = ((1 + portfolio_total_return) ** (1 / years) - 1) * 100
# e.g. 404.9% total → ~100.5% annualized
```

---

## 五、验收标准与迭代策略

### 验收标准

| 风险档位 | 目标组合年化 | 已有最佳 |
|---------|------------|---------|
| **Conservative** | **> 50%** | 待验证 |
| **Balanced** | **> 100.5%** (aggressive 基准) | 待验证 |
| **Aggressive** | 已达标 | ~100.5% |

### 迭代参数调整策略（按优先级）

#### 如果不达标 — 调整方案 A（增加搜索广度）

```json
{
    "random_seed": <换一个新值，如 17, 23, 31, 37, 41>,
    "random_candidates": 32,
    "intelligent_rounds": 3,
    "direction_mode": "long"
}
```

#### 如果不达标 — 调整方案 B（增加搜索深度）

```json
{
    "random_seed": <换新值>,
    "random_candidates": 64,
    "intelligent_rounds": 5,
    "direction_mode": "long"
}
```

⚠️ **注意：** `intelligent_rounds > 1` + `random_candidates > 16` 会导致精测阶段 5-6 小时。如果 worker 不稳定，优先用方案 A。

#### 如果不达标 — 调整方案 C（轻量快速验证）

```json
{
    "random_seed": <换新值>,
    "random_candidates": 16,
    "intelligent_rounds": 1,
    "direction_mode": "long"
}
```

这是与成功 aggressive 任务相同的参数，预计 2-3 小时完成。如果这个都达不到目标，说明参数空间本身可能限制收益，需要更深层的搜索空间调整。

### ⚠️ 关键约束

1. **不要用 `long_short` 方向** — 会导致超时
2. **18 个币种不能减少** — 这是基于成交量 Top50 + 2023年有数据的筛选逻辑
3. **币种池固定**：BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT, DOGEUSDT, XRPUSDT, ADAUSDT, ZECUSDT, DASHUSDT, NEARUSDT, BCHUSDT, LINKUSDT, AVAXUSDT, UNIUSDT, FILUSDT, DOTUSDT, AAVEUSDT, INJUSDT
4. **回测期间固定**：`start_ms: 1672531200000` (2023-01-01), `end_ms: 1746057599999` (2025-04-30)

---

## 六、完整任务配置模板

### Conservative 任务模板

```json
{
    "mode": "auto_search",
    "top_n": 10,
    "end_ms": 1746057599999,
    "market": "usd_m_futures",
    "symbols": [],
    "interval": "1m",
    "start_ms": 1672531200000,
    "margin_mode": "isolated",
    "random_seed": <SEED>,
    "search_mode": "profit_optimized_v2",
    "risk_profile": "conservative",
    "direction_mode": "long",
    "execution_model": "conservative_futures_isolated",
    "portfolio_top_n": 10,
    "time_range_mode": "auto_since_2023_to_last_month_end",
    "per_symbol_top_n": 10,
    "extended_universe": true,
    "random_candidates": <16|32|64>,
    "search_space_mode": "risk_profile_auto",
    "intelligent_rounds": <1|3|5>
}
```

### Balanced 任务模板

```json
{
    "mode": "auto_search",
    "top_n": 10,
    "end_ms": 1746057599999,
    "market": "usd_m_futures",
    "symbols": [],
    "interval": "1m",
    "start_ms": 1672531200000,
    "margin_mode": "isolated",
    "random_seed": <SEED>,
    "search_mode": "profit_optimized_v2",
    "risk_profile": "balanced",
    "direction_mode": "long",
    "execution_model": "conservative_futures_isolated",
    "portfolio_top_n": 10,
    "time_range_mode": "auto_since_2023_to_last_month_end",
    "per_symbol_top_n": 10,
    "extended_universe": true,
    "random_candidates": <16|32|64>,
    "search_space_mode": "risk_profile_auto",
    "intelligent_rounds": <1|3|5>
}
```

### 提交新任务 SQL

```sql
INSERT INTO backtest_tasks (task_id, owner, status, strategy_type, config, summary)
VALUES (
    '<TASK_ID>',           -- 如: search-conservative-18sym-v5
    'system@optimization',
    'queued',
    'martingale_grid',
    '<CONFIG_JSON>'::jsonb,
    '{}'::jsonb
);
```

---

## 七、Balanced V3 中间结果参考

Balanced v3 任务虽然最终失败，但已产出了 69 个精测候选，数据可作为参考：

| 币种 | 最佳年化 | 总收益 | 最大回撤 |
|------|---------|--------|---------|
| **BNBUSDT** | **102.8%** | 419.7% | 67.6% |
| AAVEUSDT | 96.7% | 384.2% | 69.9% |
| DOGEUSDT | 84.3% | 315.8% | 48.4% |
| BTCUSDT | 66.9% | 230.1% | 21.7% |
| AVAXUSDT | 66.9% | 230.1% | 71.7% |
| BCHUSDT | 57.1% | 186.8% | 29.5% |
| SOLUSDT | 41.5% | 124.6% | 49.6% |
| LINKUSDT | 39.7% | 118.1% | 46.3% |
| XRPUSDT | 35.2% | 102.0% | 32.2% |
| ADAUSDT | 31.3% | 88.8% | 66.9% |
| INJUSDT | 31.3% | 88.6% | 38.7% |
| DOTUSDT | 48.6% | 151.9% | 75.1% |
| UNIUSDT | 18.5% | 48.5% | 30.0% |
| DASHUSDT | 22.5% | 60.4% | 80.0% |
| ETHUSDT | 12.1% | 30.5% | 15.7% |
| FILUSDT | 8.3% | 20.5% | 20.2% |
| NEARUSDT | 4.3% | 10.3% | 3.6% |
| ZECUSDT | 1.3% | 3.1% | 5.2% |

**关键发现：** Balanced 模式下 6 个币种年化 >50%，组合潜力极大。组合优化后年化很可能超过 aggressive 的 100.5%。

---

## 八、监控循环（建议每 30 分钟检查一次）

```
循环开始:
  1. 查询任务状态
  2. 如果 status = succeeded:
     a. 分析单策略 Top 候选年化
     b. 分析组合 Top3 年化
     c. 判断是否达标
     d. 达标 → 完成，报告结果
     e. 不达标 → 选择调整方案重新提交
  3. 如果 status = failed:
     a. 检查 error_message
     b. 如果超时 → 改用 long 方向或减少参数
     c. 如果 Worker 崩溃 → 重新提交相同配置
     d. 重新提交后继续监控
  4. 如果 status = running:
     a. 检查 Worker CPU
     b. CPU > 500% → 正常，继续等待
     c. CPU < 5% 超过 10 分钟 → 标记 failed，重新提交
     d. 检查 Worker 重启次数
  5. 等待 30 分钟
  6. 回到步骤 1
```

---

## 九、文件位置与相关代码

| 文件 | 作用 |
|------|------|
| `apps/backtest-engine/src/search.rs` | 搜索空间定义、staged search |
| `apps/backtest-engine/src/portfolio_search.rs` | 组合优化器 |
| `apps/backtest-worker/src/main.rs` | Worker 主循环、精测逻辑 |
| `apps/api-server/src/services/backtest_service.rs` | API 任务创建 |
| `docs/superpowers/specs/2026-05-20-martingale-search-portfolio-final-design.md` | 搜索与组合最终设计 |
| `docs/superpowers/plans/2026-05-21-martingale-expanded-universe-profit-portfolio-plan.md` | 扩展币种深搜计划 |

### 关键函数

- `StagedMartingaleSearchSpace::profit_optimized_v2(risk_profile, direction_mode)` — 搜索空间定义
- `evaluate_refinement_candidates_parallel()` — 并行精测（所有候选全部完成才返回）
- `select_portfolio_pool_outputs_v2()` — 组合候选池选择
- `long_short_search_timeout_secs()` — 超时计算 `(batches * 240).clamp(600, 14400)`
