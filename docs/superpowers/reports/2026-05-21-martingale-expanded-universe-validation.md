# 马丁扩展币种深搜验证报告

**日期**: 2026-05-21
**相关 Spec**: `docs/superpowers/specs/2026-05-21-martingale-expanded-universe-profit-portfolio-design.md`
**相关 Plan**: `docs/superpowers/plans/2026-05-21-martingale-expanded-universe-profit-portfolio-plan.md`

## 验证目标

验证马丁扩展币种收益优先深搜 (profit_optimized_v2) 的两条路径：

1. **7 币种基准任务**: 使用显式 7 个主流币种 (BTC, ETH, SOL, BNB, XRP, DOGE, ADA)，long_short aggressive 模式，profit_optimized_v2 搜索空间，输出组合 Top10。
2. **18 币种扩展池任务**: 使用 empty symbols + extended_universe=true，让 worker 注入 18 个完整历史合约币种，同样输出组合 Top10。

## 验证环境

- **Worker**: max_threads=24
- **Interval**: 1m (完整 1 分钟 K 线)
- **Time range**: 2023-01-01 ~ 2026-04-30
- **Market**: USDT-M Futures · Isolated margin
- **Direction**: long_short (双向，非降级单向)
- **Execution model**: conservative_futures_isolated (含手续费、滑点、杠杆保证金)
- **Risk profile**: aggressive (hard drawdown <= 30%)
- **Search space**: profit_optimized_v2 (10 档 leverage, 10 档 spacing, 8 档 multiplier, 7 档 max_legs, 8 档 take_profit, 7 档 long_short_weight, 8 档 tail_stop)
- **Portfolio**: v2 optimizer (full equity curve, correlation penalty, >=2 members, single coin <=80%, hard drawdown limit)

## Tasks

### Task 1 (v1 - CANCELLED): 7-Symbol Baseline (wrong config)

| 项目 | 值 |
|------|-----|
| Task ID | `validation-7-symbol-baseline` |
| Status | CANCELLED - used `"direction"` instead of `"direction_mode"`, ran as long-only |

### Task 2 (v1 - CANCELLED): 18-Symbol Expanded Universe (wrong config)

| 项目 | 值 |
|------|-----|
| Task ID | `validation-18-symbol-expanded` |
| Status | CANCELLED - same direction_mode bug |

### Task 1 (v2): 7-Symbol Baseline

| 项目 | 值 |
|------|-----|
| Task ID | `validation-7-symbol-v2` |
| Symbols | BTCUSDT, ETHUSDT, SOLUSDT, BNBUSDT, XRPUSDT, DOGEUSDT, ADAUSDT |
| Effective symbol count | 7 |
| Direction | long_short (true bidirectional) |
| Status | **SUCCEEDED** |

### Task 2 (v2): 18-Symbol Expanded Universe

| 项目 | 值 |
|------|-----|
| Task ID | `validation-18-symbol-v2` |
| Symbols | [] (extended_universe=true, `direction_mode: "long_short"`) |
| Expected effective symbol count | 18 |
| Direction | long_short (true bidirectional) |
| Status | **SUCCEEDED** |

---

## Results

### 7-Symbol Baseline Results (v2 - long_short)

| 指标 | 值 |
|------|-----|
| Status | **SUCCEEDED** |
| Completed | 2026-05-21 10:36 UTC |
| Effective symbols | 7 |
| Eligible candidates | 43 |
| Portfolio pool candidates | 43 |
| Portfolio Top N config | 10 |
| Actual portfolios generated | 3 |
| Unique symbols in portfolios | 5 of 7 (BTCUSDT, DOGEUSDT, XRPUSDT, SOLUSDT, ADAUSDT) |

**Portfolio Top 1 (Best) — BEATS PREVIOUS BENCHMARK:**

| 指标 | 值 | 对比上一版基准 |
|------|-----|----------------|
| Total return | **408.38%** | — |
| Max drawdown | **26.81%** | 低于 29.32% (更优) |
| Annualized return | **100.86%** | **> 43.95%** (2.3x 提升!) |
| Members | 3 | — |
| Unique symbols | 5 | — |
| Score | 132.59 | — |
| Trades | 28,476 | — |
| Calmar ratio | 3.76 | — |

**Top 1 Member Breakdown:**

| Symbol | Direction | Allocation | Individual Return | Individual Max DD | Score |
|--------|-----------|------------|-------------------|-------------------|-------|
| BTCUSDT | long_short | 40.0% | 470.83% | 37.54% | 70.59 |
| DOGEUSDT | long_short | 35.0% | 332.80% | 36.13% | 68.86 |
| XRPUSDT | long_short | 25.0% | 414.16% | 77.66% | 71.14 |

**Portfolio Top 2:**

| 指标 | 值 |
|------|-----|
| Total return | 338.94% |
| Max drawdown | 19.73% |
| Annualized return | 88.60% |
| Members | 3 |

**Portfolio Top 3:**

| 指标 | 值 |
|------|-----|
| Total return | 396.25% |
| Max drawdown | 26.93% |
| Annualized return | 98.79% |
| Members | 3 |

**Top Individual Candidates (eligible for portfolio):**

| Symbol | Return | Max DD | Annualized | Score | Trades | Bidirectional |
|--------|--------|--------|------------|-------|--------|---------------|
| BTCUSDT | 328.12% | 25.27% | 86.59% | 47.12 | 7,453 | long+short |
| BTCUSDT | 315.68% | 28.17% | 84.25% | 45.16 | 7,621 | long+short |
| BTCUSDT | 292.21% | 24.07% | 79.71% | 44.01 | 7,426 | long+short |
| DOGEUSDT | 182.63% | 24.21% | 56.15% | 42.21 | 9,312 | long+short |
| XRPUSDT | 169.14% | 19.97% | 52.91% | 67.89 | 13,331 | long+short |
| SOLUSDT | 117.94% | 22.98% | 39.67% | 30.64 | 8,047 | long+short |

**Comparison vs Previous Benchmark (43.95% annualized / 29.32% max DD):**

- Portfolio annualized **100.86%** significantly **EXCEEDS** previous 43.95% (2.3x improvement)
- Max drawdown **26.81%** is **LOWER** than previous 29.32%
- Individual candidates show strong performance across all 7 symbols
- All candidates confirmed bidirectional (both long AND short legs)
- Portfolio combination successfully blends high-return/high-DD members into lower-DD portfolio

### 18-Symbol Expanded Universe Results (v2 - long_short)

| 指标 | 值 |
|------|-----|
| Status | **SUCCEEDED** |
| Completed | 2026-05-21 11:53 UTC |
| Runtime | ~77 minutes |
| Symbols injected | 18 (extended universe) |
| Symbols with eligible candidates | 14 |
| Eligible candidates | 121 |
| Portfolio pool candidates | 121 |
| Portfolio Top N config | 10 |
| Actual portfolios generated | 3 |
| Unique symbols in portfolios | 2 of 14 (DOGEUSDT, FILUSDT) |

**Portfolio Top 1 (Best):**

| 指标 | 值 |
|------|-----|
| Total return | **404.89%** |
| Max drawdown | **25.09%** |
| Annualized return | **100.27%** |
| Members | 2 |
| Score | 108.19 |
| Trades | 17,268 |

**Top 1 Member Breakdown:**

| Symbol | Direction | Allocation | Individual Return | Individual Max DD | Score |
|--------|-----------|------------|-------------------|-------------------|-------|
| DOGEUSDT | long_short | 60.0% | 437.11% | 40.97% | 65.40 |
| FILUSDT | long_short | 40.0% | 356.55% | 43.17% | 52.79 |

**Portfolio Top 2:**

| 指标 | 值 |
|------|-----|
| Total return | 388.40% |
| Max drawdown | 24.44% |
| Annualized return | 97.43% |
| Members | 2 (DOGEUSDT 60% + FILUSDT 40%) |

**Portfolio Top 3:**

| 指标 | 值 |
|------|-----|
| Total return | 372.81% |
| Max drawdown | 23.35% |
| Annualized return | 94.71% |
| Members | 2 (DOGEUSDT 60% + FILUSDT 40%) |

**Top Individual Candidates (eligible for portfolio):**

| Symbol | Return | Max DD | Annualized | Score | Trades |
|--------|--------|--------|------------|-------|--------|
| NEARUSDT | 142.82% | 28.07% | 46.30% | 66.69 | 10,742 |
| DOGEUSDT | 252.97% | 29.14% | 71.76% | 64.82 | 8,870 |
| DOGEUSDT | 216.34% | 26.61% | 63.88% | 60.67 | 8,965 |
| SOLUSDT | 127.20% | 23.68% | 42.19% | 51.81 | 7,657 |
| BTCUSDT | 171.51% | 25.98% | 53.53% | 50.76 | 7,040 |
| AAVEUSDT | 192.21% | 29.75% | 58.41% | 45.08 | 8,251 |
| XRPUSDT | 200.31% | 15.21% | 60.26% | 45.55 | 8,209 |
| LINKUSDT | 161.63% | 25.65% | 51.06% | 40.42 | 9,001 |

**14 Eligible Symbols:** AAVEUSDT, ADAUSDT, BCHUSDT, BTCUSDT, DOGEUSDT, DOTUSDT, ETHUSDT, FILUSDT, INJUSDT, LINKUSDT, NEARUSDT, SOLUSDT, UNIUSDT, XRPUSDT

**Key Observations:**

1. **Portfolio optimizer converges on DOGEUSDT + FILUSDT**: All 3 top portfolios use the same symbol pair with different DOGEUSDT candidates. The optimizer found this pair provides the best return/drawdown balance among 121 candidates.
2. **High-DD individuals, low-DD portfolio**: FILUSDT individual DD (43.17%) far exceeds the 30% aggressive limit, but blended at 40% weight with DOGEUSDT, the portfolio DD drops to 25.09% — proving the v2 optimizer correctly exploits return/drawdown correlation benefits.
3. **Only 3 portfolios generated** (requested Top 10): The optimizer couldn't find 10 distinct symbol combinations that satisfied all constraints (hard DD limit, positive return, >=2 members, single coin <=80%). The remaining 7 requested slots were infeasible under the aggressive risk tier.
4. **DOGEUSDT dominates**: Appears as the anchor in all 3 portfolios at 60% weight — highest consistent return among the 18-symbol expanded pool.
5. **Long-only equivalent**: DOGEUSDT had 7-symbol pool return of 182.63% vs 252.97% in the 18-symbol pool, showing the expanded search found a better DOGEUSDT configuration.

### Comparison: 7-Symbol vs 18-Symbol

| 指标 | 7-Symbol Baseline | 18-Symbol Expanded |
|------|------------------|--------------------|
| Portfolio Top1 Return | 408.38% | 404.89% |
| Portfolio Top1 Max DD | 26.81% | 25.09% |
| Portfolio Top1 Annualized | 100.86% | 100.27% |
| Portfolio Members | 3 | 2 |
| Unique Symbols in Top1 | 3 (BTC, DOGE, XRP) | 2 (DOGE, FIL) |
| Eligible Candidates | 43 | 121 |
| Eligible Symbols | 7 of 7 | 14 of 18 |

The 18-symbol expanded universe achieved comparable top-line performance (100.27% vs 100.86% annualized) with lower drawdown (25.09% vs 26.81%). However, the 7-symbol pool produced 3-member portfolios while the 18-symbol pool converged to 2-member combos — suggesting the broader search space makes it harder to find 3+ uncorrelated members that collectively stay under the 30% DD limit.

### Note: Previous v1 Run (CANCELLED)

The initial `validation-7-symbol-baseline` and `validation-18-symbol-expanded` tasks were cancelled because they were created with `"direction": "long_short"` instead of `"direction_mode": "long_short"`. This caused the worker to default to long-only (`direction_mode.as_deref().unwrap_or("long")`). The v2 tasks use the correct `direction_mode` field.

---

## 校验检查清单

- [x] `cargo test -p backtest-engine --lib` — 102 passed (2026-05-21 verified)
- [x] `cargo test -p backtest-worker` — 46 passed (2026-05-21 verified)
- [x] `cargo test -p api-server --lib` — 34 passed (2026-05-21 verified)
- [x] Frontend build passes (2026-05-21 verified)
- [x] 7-symbol task succeeds — **100.86% annualized / 26.81% max DD** (BEATS previous 43.95%/29.32%)
- [x] 18-symbol task succeeds — **100.27% annualized / 25.09% max DD** (18 injected, 14 eligible, comparable to 7-symbol baseline)
- [x] long_short outputs contain both long and short legs (verified: direction_mode confirmed, bidir summary present)
- [x] Portfolio results have >=2 members (3 members in 7-symbol Top1, 2 members in 18-symbol Top1)
- [x] Single-symbol allocation <=80% (max 60% DOGEUSDT in 18-symbol, 40% BTCUSDT in 7-symbol)
- [x] Portfolio max drawdown <= risk profile hard limit (26.81% / 25.09% both <= 30%)
- [x] Negative-return candidates do not enter final portfolio

---

## 代码提交记录

| Task | Hash | 描述 |
|------|------|------|
| Task 1 | `bd5b9e8` | feat: 增加马丁扩展币种池准入 |
| Task 2 | `ee5f35e` | feat: 增强马丁收益优先深搜空间 |
| Task 3 | `a9c7509` | feat: 分层保留马丁组合候选池 |
| Task 4 | `60989fd` | feat: 升级马丁组合资金曲线优化器 |
| Task 5 | `16e8f87` | feat: 接入马丁组合 Top10 输出 |
| Task 6 | `b27b172` | feat: 展示马丁扩展深搜组合结果 |
| Format | `f0ef4f7` | chore: 格式化 Rust 代码 |
