# Martingale 优化：DeepSeek → GLM 交接文档

**时间:** 2026-06-12 08:55 UTC  
**交接原因:** DeepSeek 已完成代码修复 + conservative 首轮搜索 + seed 307 瓶颈分析，剩余搜索转 GLM 执行。  
**当前状态:** seed 521 正在 trade_refinement 阶段，CPU 1928%，RAM 18.4GB

---

## 一、已完成工作

### 1. Task 1: Cleanup DeepSeek Partial State ✅

| 操作 | 状态 |
|------|------|
| 归档 `fk-18-conservative-seed211-20260611` → `archive+flyingkid2022@outlook.com` | ✅ |
| 修复 `fk-18-conservative-baseline-from-v5-20260611` 候选行（59 candidates 从 `search-conservative-18sym-v5` 复制） | ✅ |
| 修正 cleanup 报告 cascade 描述（`backtest_candidate_summaries.task_id` 实际为 `ON DELETE CASCADE`） | ✅ |
| 报告位置: `docs/superpowers/reports/2026-06-11-martingale-flyingkid-cleanup-and-baseline.md` | ✅ |

### 2. Task 2: ATR/ADX Indicator Parity (backtest ↔ live) ✅

| 修改 | 文件 | 说明 |
|------|------|------|
| `add_leg()` 传入 `latest_atr` | `kline_engine.rs:549` | 修复 ATR spacing 在回测中始终失败的根本原因 |
| 搜索空间启用 ATR spacing | `search.rs:217,242,260` | 三个 risk profile 都恢复 `SpacingModelChoice::Atr` |
| `is_valid_spacing_for_model()` 校验 ATR | `search.rs:653` | 从 `false` 改为 `atr_spacing_multiplier_bps > 0 && <= 40000` |
| `build_spacing_model()` 生成真实 ATR | `search.rs:702` | 从 fallback FixedPercent 改为 `MartingaleSpacingModel::Atr` |
| 创建公共 `IndicatorRuntimeContext` | `indicator_runtime.rs` (新文件) | 从 kline_engine.rs 提取，供 backtest/live 共用 |
| `MartingaleRuntime` 加入 `indicator_context` | `martingale_runtime.rs:125` | 支持 warmup/latest_atr/evaluate_expression |
| 移除 live preflight ATR/ADX 拦截 | `martingale_runtime.rs:556-596` | ATR spacing/TP/SL/IndicatorExpression 不再被拒绝 |
| 更新 `leg_trigger_price()` 接受 `latest_atr` | `martingale_runtime.rs:649` | 实盘 ATR spacing 计算可用 |
| `spacing_distance_bps()` 更新错误信息 | `martingale_runtime.rs:697` | ATR spacing 需要 latest_atr，走 compute_leg_trigger_prices 路径 |

**测试结果:** 全部 292 个测试通过（backtest-engine 155 + trading-engine 137）

### 3. Task 3: Backtest Accuracy Gates ✅

- `DEFAULT_FEE_BPS = 4.5`，`DEFAULT_SLIPPAGE_BPS = 2.0` — 已有测试验证
- 年化使用实际天数（`annualized_return_uses_backtest_days` test）
- 组合 DD 使用合并 equity curve — 已有测试验证
- Funding fee: **未包含**，需在所有报告中标记 "excluding funding"

### 4. Task 4: Worker Speed / ETA / Monitoring ✅

| 修改 | 文件 |
|------|------|
| 修复 `rss_mb=N/A` bug | `main.rs:1709`（`serde_json::Number.as_str()` → `mb.to_string()`） |
| pre-refinement DB 写入 `processed_candidates/total_candidates/rss_mb/worker_threads` | `main.rs:1831` |
| 监控脚本 | `scripts/monitor_martingale_backtests.sh` |

**注:** 以上代码修改**未重新构建 Docker 镜像**（当前运行的 worker 仍是旧二进制），所以 rss_mb 仍显示 "N/A"、refinement 期间无 DB 进度更新。下次重建镜像后生效。

---

## 二、当前执行状态

### 运行中

| Task ID | 配置 | 当前阶段 | CPU | RAM |
|---------|------|----------|-----|-----|
| `fk-18-conservative-seed521-20260611` | seed=521, candidates=64, per_sym=20, portfolio=3 | trade_refinement (80%) | 1928% | 18.4GB |

Worker 以 24 线程运行（`BACKTEST_WORKER_MAX_THREADS=24`），机器 30 核。

### 已归档

| Task ID | 原因 |
|---------|------|
| `fk-18-conservative-seed307-20260611` | refinement 超时(6h+)，candidates=256/per_sym=40 过于激进 |
| `fk-18-conservative-seed211-20260611` | 被 DeepSeek cancel 后归档 |

### FlyingKid 可见（每档最佳）

| Risk | Task ID | Ann% | DD% | 说明 |
|------|---------|------|-----|------|
| conservative | `fk-18-conservative-baseline-from-v5-20260611` | 40.69 | 9.66 | 从 search-conservative-18sym-v5 复制的 baseline |
| balanced | `fk-18-bal-v2-seed53-20260601` | 65.52 | 19.32 | 当前最佳 |
| aggressive | `fk-18-agg-v2-seed173-20260601` | 77.00 | 28.03 | 当前最佳 |

---

## 三、目标与决策树

### Conservative（当前正在执行）

**目标:** annualized_return_pct > 50%，max_drawdown_pct <= 10%

**Seeds 序列:** 307 → ~~521~~（当前）→ 887 → 1597

**Seed 521 完成后决策:**
```text
如果 seed 521 成功:
  - 提取最佳候选的 annualized_return_pct / max_drawdown_pct
  - 若 ann > 50 AND dd <= 10: 
    → 归档 fk-18-conservative-baseline-from-v5-20260611 到 archive+flyingkid2022@outlook.com
    → 标记 seed 521 为 FlyingKid 可见 conservative 最佳
    → 进入 Balanced 搜索
  - 若 ann 达标但 dd 超标: 调低 leverage 范围，继续 seed 887
  - 若均未达标: 继续 seed 887 → 1597
  - 若所有 seed 耗尽仍未达标: 
    → 扩展参数范围（降低 min_leverage, 放宽 spacing/max_legs）
    → 或接受最佳结果，标记瓶颈

如果 seed 521 失败/超时:
  - 归档到 archive+flyingkid2022@outlook.com
  - 自动提交 seed 887（相同轻量化配置）
```

**当前运行 seed 521 配置（推荐用于后续所有种子）:**
```json
{
    "risk_profile": "conservative",
    "direction_mode": "long",
    "random_seed": 521,
    "random_candidates": 64,
    "per_symbol_top_n": 20,
    "portfolio_top_n": 3,
    "extended_universe": true,
    "search_mode": "profit_optimized_v2",
    "search_space_mode": "risk_profile_auto",
    "intelligent_rounds": 5,
    "fee_bps": 4.5,
    "slippage_bps": 2.0,
    "start_ms": 1672531200000,
    "end_ms": 1780271999999,
    "time_range_mode": "auto_since_2023_to_last_month_end"
}
```

### Balanced

**目标:** 超过 `fk-18-bal-v2-seed53-20260601` rank1 (65.52%/19.32%)

**Seeds:** 67, 173, 307, 521

**配置:** 与 conservative 相同，`risk_profile: "balanced"`

### Aggressive

**目标:** 超过 `fk-18-agg-v2-seed173-20260601` (77.00%/28.03%)，挑战 ~100.5%

**Seeds:** 67, 211, 307, 521

**配置:** `risk_profile: "aggressive"`，DD <= 30%

---

## 四、监控命令

### 1. 检查当前任务状态
```bash
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -P pager=off -c "
SELECT task_id, owner, status,
       summary->>'stage' AS stage,
       summary->>'current_symbol' AS sym,
       summary->>'progress_pct' AS pct,
       updated_at,
       EXTRACT(EPOCH FROM now() - updated_at)::int / 60 AS stale_min
FROM backtest_tasks
WHERE owner='flyingkid2022@outlook.com' AND status IN ('queued','running','paused')
ORDER BY updated_at DESC;
"
```

### 2. 检查任务结果（任务完成后）
```bash
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -P pager=off -c "
SELECT task_id, status, error_message,
       summary->>'stage' AS stage
FROM backtest_tasks
WHERE task_id = 'fk-18-conservative-seed521-20260611';
"
```

### 3. 检查 Worker 状态
```bash
docker stats --no-stream grid-binance-backtest-worker-1
docker exec grid-binance-backtest-worker-1 cat /proc/1/io | grep -E "read_bytes|write_bytes"
```

### 4. 提交新任务（SQL 模板）

```sql
INSERT INTO backtest_tasks (task_id, owner, status, strategy_type, config, summary)
VALUES (
    'fk-18-REPLACE_WITH_TASK_ID',
    'flyingkid2022@outlook.com',
    'queued',
    'martingale_auto_search',
    '{
        "mode": "auto_search",
        "owner": "flyingkid2022@outlook.com",
        "top_n": 3,
        "market": "usd_m_futures",
        "symbols": [],
        "interval": "1m",
        "margin_mode": "isolated",
        "random_seed": REPLACE_WITH_SEED,
        "search_mode": "profit_optimized_v2",
        "risk_profile": "REPLACE_WITH_RISK",
        "direction_mode": "long",
        "execution_model": "conservative_futures_isolated",
        "portfolio_top_n": 3,
        "time_range_mode": "auto_since_2023_to_last_month_end",
        "per_symbol_top_n": 20,
        "extended_universe": true,
        "random_candidates": 64,
        "search_space_mode": "risk_profile_auto",
        "intelligent_rounds": 5,
        "fee_bps": 4.5,
        "slippage_bps": 2.0,
        "start_ms": 1672531200000,
        "end_ms": 1780271999999
    }'::jsonb,
    '{}'::jsonb
);
```

### 5. 归档任务
```sql
UPDATE backtest_tasks
SET owner = 'archive+flyingkid2022@outlook.com',
    summary = summary || jsonb_build_object(
        'archived_from_owner', 'flyingkid2022@outlook.com',
        'archived_at', now()::text,
        'archive_reason', 'REPLACE_WITH_REASON'
    ),
    updated_at = now()
WHERE task_id = 'REPLACE_WITH_TASK_ID'
  AND owner = 'flyingkid2022@outlook.com';
```

### 6. 重启 Worker（需要时）
```bash
docker stop grid-binance-backtest-worker-1
docker rm grid-binance-backtest-worker-1
docker run -d \
    --name grid-binance-backtest-worker-1 \
    --network grid-binance_default \
    -e DATABASE_URL="postgres://postgres:postgres@postgres:5432/grid_binance" \
    -e REDIS_URL="redis://redis:6379/0" \
    -e BACKTEST_ARTIFACT_ROOT="/var/lib/grid-binance/backtest-artifacts" \
    -e BACKTEST_MARKET_DATA_DB_PATH="/market-data/market_data.db" \
    -e BACKTEST_WORKER_MAX_THREADS=24 \
    -e BACKTEST_WORKER_POLL_MS=5000 \
    -v grid-binance_backtest-artifacts:/var/lib/grid-binance/backtest-artifacts \
    -v /home/bumblebee/Project/discord_c2im/pipeline/data:/market-data:ro \
    -e APP_NAME=backtest-worker \
    grid-binance-backtest-worker:latest
```

---

## 五、监控节奏

### 搜索阶段 (search_symbol, progress=35%)

**节奏:** 每 10-15 分钟检查一次  
**关注:** `current_symbol` 是否在推进（18 symbols 全部完成约需 6-8 小时）

### 精测阶段 (trade_refinement, progress=80%)

**节奏:** 每 20-30 分钟检查一次  
**关注:** `updated_at` 是否变化（当前二进制无 DB 进度更新，需看 worker CPU/IO）  
**超时判定:** 若 CPU 持续 < 5% 或 `updated_at` 超过 2 小时无变化，则可能卡死  
**正常现象:** CPU 1800-2000%，RAM 18GB+，read_bytes=0 但 write_bytes 在增长

### 组合优化阶段 (portfolio 相关)

进度会跳到 ~90%+，出现 `trade_refinement_top_N` 心跳

### 任务完成

- `status = 'succeeded'`: 提取 summary 中的 `portfolio_summary` 或 candidate 信息
- `status = 'failed'`: 检查 `error_message`

---

## 六、Known Issues / 待优化

### 1. 精测无 DB 进度（代码已修，未部署）
**问题:** refinement 期间 updated_at 数小时不更新  
**修复:** 代码已在 `main.rs` 中增加了 `processed_candidates/total_candidates` 字段，但需重建 Docker 镜像才能生效  
**当前判断方法:** 检查 worker CPU（是否 1800%+）和 I/O（write_bytes 是否增长）

### 2. seed 307 精测超时分析
**结论:** `random_candidates=256, per_symbol_top_n=40` 对 18-symbol extended universe 过于激进
- 筛选产生 697 个候选
- 精测每个候选需完整回测 3.5 年数据
- 24 线程仍无法在合理时间内完成

**解决:** 后续统一使用 `random_candidates=64, per_symbol_top_n=20`（seed 521 配置）

### 3. Docker 日志缓冲区太小
Docker json-file log driver 只保留 6 行日志，TIMING 日志丢失。建议增加 Docker 日志配置。

### 4. 代码未部署
以下代码修改已提交但未重建 Docker 镜像：
- `rss_mb` 修复
- 精测 DB 进度更新
- ATR/ADX parity 系列修复

重建命令:
```bash
cd deploy/docker && docker compose build backtest-worker
```

---

## 七、验证命令总结

```bash
# 运行所有相关测试
cargo test -p backtest-engine --lib -- --nocapture
cargo test -p backtest-engine --test search_scoring_time_splits -- --nocapture
cargo test -p backtest-worker --lib -- --nocapture
cargo test -p trading-engine --test martingale_runtime -- --nocapture
cargo test -p trading-engine --test order_sync -- --nocapture
cargo test -p trading-engine --test trade_sync -- --nocapture
```
