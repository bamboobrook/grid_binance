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

### Task 1: 7-Symbol Baseline

| 项目 | 值 |
|------|-----|
| Task ID | `validation-7-symbol-baseline` |
| Symbols | BTCUSDT, ETHUSDT, SOLUSDT, BNBUSDT, XRPUSDT, DOGEUSDT, ADAUSDT |
| Effective symbol count | 7 |
| Status | RUNNING (started 2026-05-21 09:55 UTC) |

### Task 2: 18-Symbol Expanded Universe

| 项目 | 值 |
|------|-----|
| Task ID | `validation-18-symbol-expanded` |
| Symbols | [] (extended_universe=true) |
| Expected effective symbol count | 18 (BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT, DOGEUSDT, XRPUSDT, ADAUSDT, ZECUSDT, DASHUSDT, NEARUSDT, BCHUSDT, LINKUSDT, AVAXUSDT, UNIUSDT, FILUSDT, DOTUSDT, AAVEUSDT, INJUSDT) |
| Status | QUEUED |

---

## Results

> Results will be filled in once tasks complete.

### 7-Symbol Baseline Results

| 指标 | 值 |
|------|-----|
| Status | **SUCCEEDED** |
| Completed | 2026-05-21 ~10:00 UTC |
| Effective symbols | 7 |
| Eligible candidates | 11 |
| Portfolio pool candidates | 11 |
| Portfolio Top N config | 10 |
| Actual portfolios generated | 3 |
| Unique symbols in portfolios | 3 (ADAUSDT, ETHUSDT, XRPUSDT) |

**Portfolio Top 1 (Best):**

| 指标 | 值 |
|------|-----|
| Total return | 26.94% |
| Max drawdown | 29.69% |
| Annualized return | 10.77% |
| Members | 5 |
| Unique symbols | 3 |
| Score | 14.07 |
| Trades | 22,815 |

**Portfolio Top 2:**

| 指标 | 值 |
|------|-----|
| Total return | 22.68% |
| Max drawdown | 25.09% |
| Annualized return | 9.16% |
| Members | 5 |

**Portfolio Top 3:**

| 指标 | 值 |
|------|-----|
| Total return | 22.66% |
| Max drawdown | 26.46% |
| Annualized return | 9.16% |
| Members | 5 |

**Top Individual Candidates (eligible for portfolio):**

| Symbol | Return | Max DD | Annualized | Score | Trades |
|--------|--------|--------|------------|-------|--------|
| BTCUSDT | 255.04% | 27.78% | 72.21% | 55.32 | 687 |
| BTCUSDT | 252.62% | 26.22% | 71.73% | 69.06 | 582 |
| BTCUSDT | 145.16% | 21.93% | 46.93% | 41.09 | 1070 |
| BTCUSDT | 130.10% | 17.11% | 42.97% | 45.70 | 838 |
| SOLUSDT | 65.47% | 21.87% | 24.11% | 27.87 | 3436 |
| XRPUSDT | 51.00% | 16.82% | 19.34% | 33.92 | 2772 |
| BNBUSDT | 47.50% | 29.72% | 18.14% | 54.17 | 535 |

**Comparison vs Previous Benchmark (43.95% annualized / 29.32% max DD):**

- Portfolio annualized (10.77%) is significantly **lower** than previous 43.95%
- However, individual BTCUSDT candidates (72.21% annualized) exceed the benchmark
- Portfolio combination degrades individual performance: best candidates (BTCUSDT) are not selected for final portfolios
- Only 3 portfolios pass the 30% hard drawdown limit, all with 3 unique symbols each
- Root cause: Cross-symbol equity curve combinations likely exceed the 30% hard drawdown when high-return BTCUSDT candidates are included

### 18-Symbol Expanded Universe Results

| 指标 | 值 |
|------|-----|
| Status | RUNNING (35% progress, search_symbol stage) |

---

## 校验检查清单

- [x] `cargo test -p backtest-engine --lib` — 102 passed (2026-05-21 verified)
- [x] `cargo test -p backtest-worker` — 46 passed (2026-05-21 verified)
- [x] `cargo test -p api-server --lib` — 34 passed (2026-05-21 verified)
- [x] Frontend build passes (2026-05-21 verified)
- [x] 7-symbol task succeeds (portfolio max DD 29.69% <= 30%)
- [ ] 18-symbol task succeeds (>=18 effective symbols, portfolio Top10)
- [x] long_short outputs contain both long and short legs (verified: Short ETHUSDT and Long XRPUSDT in Top1)
- [x] Portfolio results have >=2 members (5 members in Top1)
- [x] Single-symbol allocation <=80% (max single symbol 55% ADAUSDT across 2 candidates)
- [x] Portfolio max drawdown <= risk profile hard limit (29.69% <= 30%)
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
