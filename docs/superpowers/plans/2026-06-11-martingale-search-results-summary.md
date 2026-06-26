# 马丁最优组合策略搜索 — 探索结果与问题总结

> **时间范围：** 2026-06-10 ~ 2026-06-11
> **目标：** 找到 Conservative 年化 >50%、Balanced 年化 >100.5% 的最优组合
> **计划来源：** `docs/superpowers/plans/2026-06-10-martingale-optimal-portfolio-search-plan.md`

---

## 一、最终结果概览

| 风险档位 | 目标年化 | 实际最佳年化 | 达标？ | 根因 |
|---------|---------|------------|--------|------|
| Conservative | >50% | **40.8%** | ❌ | 搜索空间天花板 |
| Balanced | >100.5% | **49.7%（组合）/ 103.0%（单策略BNB）** | ❌ | 组合过度分散 + Worker-2崩溃 |
| Aggressive | 已达标 | ~100.5% | ✅ | 之前已完成 |

---

## 二、Conservative 探索过程

### 任务迭代

| 迭代 | 任务ID | 配置 | Worker | 结果 | 年化 |
|------|--------|------|--------|------|------|
| v4 | `search-conservative-18sym-v4` | random=32, rounds=3, seed=99 | Worker-3 | ✅ succeeded | **40.8%** |
| v5 | `search-conservative-18sym-v5` | random=64, rounds=5, seed=23 | Worker-3 | ✅ succeeded | **40.8%** |

### 详细结果

**单策略 Top 5（v4 和 v5 完全相同）：**

| 币种 | 年化收益 | 总收益 | 最大回撤 |
|------|---------|--------|---------|
| AVAXUSDT | 101.5% | 411.9% | 47.6% |
| BTCUSDT | 90.5% | 349.1% | 40.5% |
| DOTUSDT | 77.8% | 282.3% | 48.7% |
| XRPUSDT | 74.5% | 266.1% | 41.4% |
| ZECUSDT | 64.1% | 217.4% | 59.8% |

**组合 Top1：** 121.6% 总收益 → **40.8% 年化**（12 成员，过于分散）

### 关键发现
- **不同 seed（99 vs 23）和不同深度（32+3 vs 64+5）产生完全相同结果**
- 表明 Conservative 风险约束下搜索空间已收敛到天花板
- 单策略可达 101.5% 年化，但组合优化将 12 个币种分散后稀释至 40.8%

---

## 三、Balanced 探索过程

### 任务迭代

| 迭代 | 任务ID | 种子 | Worker | 结果 | 详情 |
|------|--------|------|--------|------|------|
| v3 | `search-balanced-18sym-v3` | — | Worker-2 | ❌ 超时 | long_short 方向导致 |
| v4 | `search-balanced-18sym-v4` | 13 | Worker-2 | ❌ 崩溃 | 69/123 候选，Worker 重启 |
| v5 | `search-balanced-18sym-v5` | 17 | Worker-2 | ❌ 崩溃 | 69/123，内存 46%→OOM |
| v6 | `search-balanced-18sym-v6` | 31 | Worker-2 | ❌ 崩溃 | 69/123，内存 69%→OOM |
| v7 | `search-balanced-18sym-v7` | 41 | **Worker-5** | ✅ succeeded | 49.7% 年化 |

### v7 详细结果（唯一成功）

**单策略 Top 5：**

| 币种 | 年化收益 | 总收益 | 最大回撤 |
|------|---------|--------|---------|
| **BNBUSDT** | **103.0%** | 419.7% | 67.6% |
| AAVEUSDT | 97.0% | 384.2% | 69.9% |
| DOGEUSDT | 84.5% | 315.8% | 48.4% |
| BTCUSDT | 67.1% | 230.1% | 21.7% |
| AVAXUSDT | 67.1% | 230.1% | 71.7% |

**每币种最佳年化（全部 18 币种）：**

| 币种 | 年化 | 总收益 | 最大回撤 |
|------|------|--------|---------|
| BNBUSDT | 102.8% | 419.7% | 67.6% |
| AAVEUSDT | 96.7% | 384.2% | 69.9% |
| DOGEUSDT | 84.3% | 315.8% | 48.4% |
| AVAXUSDT | 66.9% | 230.1% | 71.7% |
| BTCUSDT | 66.9% | 230.1% | 21.7% |
| BCHUSDT | 57.1% | 186.8% | 29.5% |
| DOTUSDT | 48.6% | 151.9% | 75.1% |
| SOLUSDT | 41.5% | 124.6% | 49.6% |
| LINKUSDT | 39.7% | 118.1% | 46.3% |
| XRPUSDT | 35.2% | 102.0% | 32.2% |
| ADAUSDT | 31.3% | 88.8% | 66.9% |
| INJUSDT | 31.3% | 88.6% | 38.7% |
| DASHUSDT | 22.5% | 60.4% | 80.0% |
| UNIUSDT | 18.5% | 48.5% | 30.0% |
| ETHUSDT | 12.1% | 30.5% | 15.7% |
| FILUSDT | 8.3% | 20.5% | 20.2% |
| NEARUSDT | 4.3% | 10.3% | 3.6% |
| ZECUSDT | 1.3% | 3.1% | 5.2% |

**组合 Top1：** 155.9% 总收益 → **49.7% 年化**（11 成员）
- 主要成员：AAVEUSDT(13.9%), BTCUSDT(24.7%) + 9 个其他币种

### 关键发现
- **BNBUSDT 单策略 103.0% 年化** — 已超过 Aggressive 基准（100.5%）和 Balanced 目标
- **组合过度分散** — 11 成员将 103% 稀释到 49.7%
- **Worker-2 连续 4 次在相同位置崩溃** — 见下文详细分析

---

## 四、关键问题

### 问题 1：Worker-2 持续崩溃（严重BUG）

**现象：** Balanced 任务 v3/v4/v5/v6 全部在 Worker-2 上崩溃

| 迭代 | 崩溃点 | 候选数 | 内存峰值 | Worker重启 |
|------|--------|--------|---------|-----------|
| v4 | post-refinement portfolio | 69/123 | — | 1→2 |
| v5 | post-refinement portfolio | 69/123 | 46% | 2→3 |
| v6 | post-refinement portfolio | 69/123 | 69% | 3→4 |

**精确位置：** `evaluate_refinement_candidates_parallel()` 返回后，进入 `build_portfolio_top_n_v2()` 组合优化阶段，内存飙升导致 Worker-2 崩溃重启。

**对比：**
- Worker-2: 4次崩溃，restart count = 4
- Worker-3: 0次崩溃，成功完成 Conservative v4/v5
- Worker-5: 0次崩溃，成功完成 Balanced v7

**推测根因：** Worker-2 容器内存配额不足（可能 < Worker-3/5），在组合优化加载候选数据时触发 OOM。

**影响：** 任何被 Worker-2 拾取的 Balanced 任务都会失败。只有 Worker-3 或 Worker-5 能完成。

### 问题 2：Conservative 搜索空间天花板

**现象：** v4 (rand=32, rounds=3) 和 v5 (rand=64, rounds=5) 结果完全相同

| 参数 | v4 | v5 | 结果 |
|------|-----|-----|------|
| random_candidates | 32 | 64 | 相同 |
| intelligent_rounds | 3 | 5 | 相同 |
| random_seed | 99 | 23 | 相同 |
| 最佳年化 | 40.8% | 40.8% | **完全一致** |
| 候选数 | 59 | 59 | 相同 |
| Top 策略 | AVAX 101.5%, BTC 90.5% | 完全相同 | 相同 |

**推测根因：** 保守模式的回撤约束非常严格（max_drawdown_limit），参数空间中只有少数组合满足约束，且这些组合早已被 v4 穷尽。

### 问题 3：组合优化过度分散

**现象（两种风险档位均有）：**

| 风险档位 | 最强单策略年化 | 组合年化 | 成员数 | 稀释率 |
|---------|-------------|---------|--------|--------|
| Conservative | 101.5% (AVAX) | 40.8% | 12 | -60% |
| Balanced | 103.0% (BNB) | 49.7% | 11 | -52% |

**推测根因：** `build_portfolio_top_n_v2()` 为了风险分散将资金分配给 11-12 个币种，导致收益被大规模稀释。更集中的分配（如 2-3 个币种）可能产生更高年化。

### 问题 4：精测阶段极长（5-10小时）

**现象：** 122-123 个候选的 `evaluate_refinement_candidates_parallel()` 需要 5-10 小时

| 任务 | 候选数 | 精测耗时 |
|------|--------|---------|
| Conservative v4 | 122 | ~10h |
| Conservative v5 | 122 | ~6h |
| Balanced v5 | 123 | ~6h |
| Balanced v7 | 69 | ~3h |

**根因：** 并行函数是黑盒原子操作（零日志、零进度更新），所有候选完成才返回。函数内部使用 K 线级别回测 (`run_candidate_kline_screening`)，对 2.3 年 × 18 币种的 1 分钟数据计算量极大。

**影响：** 无法观测进度，只能通过 CPU 下降间接判断。长时间运行增加了 Worker 崩溃的概率窗口。

---

## 五、搜索配置总结

### 基础约束（所有任务共用）

```json
{
    "mode": "auto_search",
    "market": "usd_m_futures",
    "interval": "1m",
    "start_ms": 1672531200000,       // 2023-01-01
    "end_ms": 1746057599999,         // 2025-04-30
    "margin_mode": "isolated",
    "search_mode": "profit_optimized_v2",
    "direction_mode": "long",
    "execution_model": "conservative_futures_isolated",
    "extended_universe": true,
    "search_space_mode": "risk_profile_auto",
    "portfolio_top_n": 10,
    "per_symbol_top_n": 10,
    "top_n": 10,
    "time_range_mode": "auto_since_2023_to_last_month_end"
}
```

### 币种池（18 个，固定）

```
BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT, DOGEUSDT, XRPUSDT,
ADAUSDT, ZECUSDT, DASHUSDT, NEARUSDT, BCHUSDT, LINKUSDT,
AVAXUSDT, UNIUSDT, FILUSDT, DOTUSDT, AAVEUSDT, INJUSDT
```

### 迭代参数对比

| 档位 | 迭代 | random | rounds | seed | 结果 |
|------|------|--------|--------|------|------|
| Cons | v4 | 32 | 3 | 99 | 40.8% |
| Cons | v5 | 64 | 5 | 23 | 40.8% (相同) |
| Bal | v4 | 16 | 1 | 13 | 崩溃 |
| Bal | v5 | 16 | 1 | 17 | 崩溃 |
| Bal | v6 | 16 | 1 | 31 | 崩溃 |
| Bal | v7 | 16 | 1 | 41 | 49.7% |

### 未尝试的方向
- `direction_mode: "long_short"` — 之前确认会导致超时（搜索空间翻倍），已被排除
- Conservative 的方案 C（random=16, rounds=1）— v4/v5 结果相同，预测方案 C 也是 40.8%

---

## 六、给下一步优化的建议方向

### 方案 A：修复 Worker-2（运维）
- 增加 Worker-2 容器内存限制
- 或从 Worker-2 移除 backtest 任务调度

### 方案 B：调整组合优化参数（代码）
- 修改 `portfolio_top_n` 从 10 降到 3-5，降低分散度
- 或修改 `per_symbol_top_n` 让更少的币种进入组合池
- 目标：让 BNBUSDT 103% 年化不只占 13.9% 权重

### 方案 C：放宽 Conservative 约束（参数）
- 当前 `risk_profile: "conservative"` 回撤限制可能过严
- 尝试自定义 `max_drawdown_pct` 或 `drawdown_limit_sequence`
- 或尝试 `risk_profile: "moderate"` 作为中间档位

### 方案 D：单策略代替组合（简化）
- Balanced BNBUSDT 单策略 103% 年化已超过 Aggressive 100.5%
- 如果单策略回撤可控（67.6%），可考虑直接用最优单策略

### 方案 E：增加搜索深度（Conservative）
- 当前 `random=64, rounds=5` 已是最大搜索
- 可考虑增加 `random_candidates` 到 128 或使用不同 `search_space_mode`

---

## 七、相关文件

| 文件 | 说明 |
|------|------|
| `apps/backtest-worker/src/main.rs` | Worker 主循环（evaluate_refinement_candidates_parallel @ L2467） |
| `apps/backtest-engine/src/search.rs` | 搜索空间定义 |
| `apps/backtest-engine/src/portfolio_search.rs` | 组合优化器 |
| `docs/superpowers/plans/2026-06-10-martingale-optimal-portfolio-search-plan.md` | 原始 GLM 计划 |
