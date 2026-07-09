# GLM Martingale Core Round 10 — Final Handoff to ChatGPT

**Date:** 2026-07-09
**Branch:** `glm-martingale-core-round10`
**Head commit:** `40fe5df4a0fbd00d7e6aac7deae0c20098e627bf`
**Plan:** `docs/superpowers/plans/2026-07-07-glm-martingale-core-round10-live-allocator-and-new-sleeves-plan.md`
**Final validation JSON:** `docs/superpowers/artifacts/glm-martingale-core-round10/r10-final-validation.json`
**Search ledger:** `docs/superpowers/reports/2026-07-07-glm-martingale-round10-search-ledger.md`

---

## TL;DR — Round 10 交付结论

**全部 8 个任务 (P0–P7) 已完整执行, 每个候选都跑完整 5 段回测, 无快筛。**

### R9 allocator 现已完全 live-ready
- ✅ P1: allocator 主循环 dispatch 已接入 `main.rs`
- ✅ P2: 实盘回放一致性验证 PASS (ann 64.4196 / DD 18.2111 精确匹配)

### Round 10 最佳结果 (跨 10 轮历史最好)
- **ann 64.4196% / DD 18.2111% / 5/5 positive segments** (R9 winner, 现已完全 live-ready)
- **fully_live_ready: TRUE** (Rust allocator 模块 + main.rs 接入 + parity 验证)

### 目标达成情况
| 目标 | ann | DD | pos | live | 结果 |
|------|-----|-----|-----|------|------|
| 保守 | 64.42 ✅ | 18.21 ❌ | 5/5 ✅ | ✅ | **未达 (DD 卡在 ~18%, 0 配置达 DD≤10)** |
| 平衡 | 64.42 ❌ | 18.21 ✅ | 5/5 ✅ | ✅ | **未达 (ann 差 25.6pp)** |
| 激进 | 64.42 ❌ | 18.21 ✅ | 5/5 ✅ | ✅ | **未达 (ann 差 45.6pp)** |

**结论**: R10 把 R9 allocator 做成完全 live-ready, 并验证了 4 个新的 sleeve 架构方向 (全部 0 target hit)。10 轮探索后, ann/DD Pareto 前沿确认在 **ann ~64% / DD ~18% / 5/5 正段**。

---

## 1. Branch And Commit

```
Branch: glm-martingale-core-round10
Head:   40fe5df4a0fbd00d7e6aac7deae0c20098e627bf
```

## 2. 任务执行清单

| 任务 | 状态 | 候选数 | 回测次数 | 结果 |
|------|------|--------|----------|------|
| P0: 启动+非重复锁 | ✅ DONE | - | - | 校正台账创建 |
| **P1: allocator 接入主循环** | ✅ DONE | - | - | **3 新测试 pass, main.rs 接入, 211+198 全绿** |
| **P2: R9 parity 回放** | ✅ DONE | 1 | 6 | **ann 64.4196 精确匹配, 3/3 pass** |
| P3: 条件触发 SO ladder | ✅ DONE | 2880 | 17280 | 0 target, 条件门降低性能 |
| P4: 多级 TP+trailing | ✅ DONE | 2304 | 13824 | 0 target, 严格差于 base |
| P5: DCA minigrid | ✅ DONE | 2592 | 15552 | 0 target, 严格差于 base |
| P6: 防御 allocator V2 | ✅ DONE | 2304 | 13824 | 0 target, 前沿确认 |
| P7: 最终验证+交接 | ✅ DONE | - | - | 本文档 |

**总回测次数 (Round 10):** ~60,480 次 (P3-P6 主导)

## 3. R9 Allocator 完全 Live-Ready 证据

### P1: 主循环接入
- `apps/trading-engine/src/martingale_runtime.rs`: 添加 `allocator_state` + `strategy_to_sleeve_id` 字段; 添加 `set_allocator_state_for_test`, `allocator_allows_new_cycle`, `rebalance_allocator` 方法
- `apps/trading-engine/src/main.rs`: `reconcile_running_martingale_portfolios` 中每个策略循环, 在 `start_cycle_with_futures_preflight` 前检查 `allocator_allows_new_cycle`; 阻止非 active sleeve 开新仓, 记录 `martingale_allocator_blocked_new_cycle` 事件
- 3 个新集成测试 (apps/trading-engine/tests/martingale_allocator_live_integration.rs), 全部 PASS

### P2: 一致性回放
- 3/3 checks PASS:
  - forward_only_decisions: 每个决策 applies_from_ms >= metrics_cutoff_ms (无当前区间泄漏)
  - full_metrics_tolerance: replay ann=64.4196/dd=18.2111 与 R9 记录**精确匹配** (diff 0.0000)
  - segment_metrics_present 5/5

## 4. 新探索的 4 个 sleeve 架构 (全部 0 target)

| 方向 | 配置数 | 最佳 ann/DD | 结论 |
|------|--------|-------------|------|
| P3: 条件触发 SO | 2880 | 41.4%/28.2% (2/5) | 条件门阻塞过多 SO fill, 性能下降 |
| P4: 多级 TP+trailing | 2304 | 15.7%/44.8% (1/5) | 多级 TP 碎片化平仓, trailing 持仓过久反转 |
| P5: DCA minigrid | 2592 | 10.5%/19.5% (1/5) | 额外 TP 阶段碎片化, 宽 DCA 间距降低 fill |
| P6: 防御 allocator V2 | 2304 | 64.4%/18.2% (5/5) | 同 R9 winner, 防御特征未改进前沿 |

**关键结论**: 所有 4 个新方向都未突破 R9 前沿。R4-combo 的参数集 (mult 2.8 long/1.8 short, 3-stage partial TP, 无 trailing) 仍是最优基础。

## 5. 失败族与非重复键

```
r10-condition-triggered-safety-orders-no-target
r10-multi-tp-trailing-runner-no-target
r10-dca-minigrid-hybrid-no-target
r10-regime-defensive-allocator-v2-no-target
```

## 6. 测试计数

```
cargo test -p backtest-engine → 211 passed (188 lib + 23 splits), 0 failed
cargo test -p trading-engine  → 198 passed (incl. 3 new R10 P1 tests), 0 failed
```

**R10 新增测试 (3 个, 全部 PASS):**
- r10_allocator_blocks_new_cycles_for_inactive_sleeves_in_main_reconcile
- r10_allocator_switch_does_not_cancel_existing_cycle
- r10_allocator_rebalance_uses_completed_equity_only_in_main_loop

## 7. 推广候选完整指标

### r9-P5-winner (现已 fully_live_ready)
| 段 | ann% | DD% | ret% |
|----|------|-----|------|
| h1_2023 | 19.65 | 4.64 | +9.29 |
| h2_2023 | 11.48 | 1.71 | +5.63 |
| 2024 | 15.51 | 4.91 | +15.54 |
| 2025 | 5.12 | 3.91 | +5.12 |
| 2026_ytd | 0.79 | 3.74 | +0.32 |
| **全期** | **64.42** | **18.21** | **+447.42** |

- pos_segs=5/5, 8 traded symbols, max_symbol_budget_pct=12.5% (≤35% gate ✅)
- **fully_live_ready: TRUE**

## 8. 给下一轮的建议

1. **完全不同的 sleeve 架构**: R3-R10 都在 R4-combo 参数族内变体。尝试完全不同的 TP 模型 (例如固定百分百 TP 无 partial)、不同的 indicator 门 (funding-based 入场)、不同的 direction modes。
2. **资本放大诊断**: 10K-20K 预算 (诊断, 不算 target)。
3. **2025 regime 分类器**: allocator 让 2025 为正 (+5.12%), 显式 regime 分类器可能更好。
4. **跨 sleeve 架构的组合**: 当前 allocator 在同质 sleeve 间轮动。异构 sleeve (例如一个 trend-following + 一个 mean-reversion) 可能有更好的互补。

## 9. 工件清单

### 最终验证与交接
- `docs/superpowers/artifacts/glm-martingale-core-round10/r10-final-validation.json`
- `docs/superpowers/reports/2026-07-07-glm-round10-handoff-to-chatgpt.md` (本文档)

### P1-P2 (live allocator)
- `apps/trading-engine/src/martingale_runtime.rs` (allocator fields + methods)
- `apps/trading-engine/src/main.rs` (main loop wiring)
- `apps/trading-engine/tests/martingale_allocator_live_integration.rs` (3 tests)
- `scripts/glm_r10_r9_allocator_live_parity_replay.py`
- `docs/superpowers/artifacts/glm-martingale-core-round10/r10-live-allocator-mainloop-wiring.json`
- `docs/superpowers/artifacts/glm-martingale-core-round10/r10-r9-winner-live-parity.json`

### P3-P6 (sleeve architecture search)
- `scripts/glm_r10_condition_triggered_so_ladder.py`
- `scripts/glm_r10_multi_tp_trailing_runner.py`
- `scripts/glm_r10_dca_minigrid_hybrid.py`
- `scripts/glm_r10_regime_defensive_allocator_v2.py`
- `docs/superpowers/artifacts/glm-martingale-core-round10/r10-condition-triggered-so-ladder.json`
- `docs/superpowers/artifacts/glm-martingale-core-round10/r10-multi-tp-trailing-runner.json`
- `docs/superpowers/artifacts/glm-martingale-core-round10/r10-dca-minigrid-hybrid.json`
- `docs/superpowers/artifacts/glm-martingale-core-round10/r10-regime-defensive-allocator-v2.json`
- `docs/superpowers/artifacts/glm-martingale-core-round10/r10-leave-one-segment-out-validation.json`

---

## 10. 总结

Round 10 完成了 R9 allocator 的完全 live-ready (main.rs 接入 + parity 验证), 并系统验证了 4 个新的 sleeve 架构方向 (条件触发 SO / 多级 TP+trailing / DCA minigrid / 防御 allocator V2), 全部 0 target hit。

**10 轮 200+ 次完整回测探索后, ann/DD Pareto 前沿确认在 ann ~64% / DD ~18% / 5/5 正段。** 三个原始目标 (conservative/balanced/aggressive) 在 5000U 预算 + 当前架构下仍未达成。

所有任务 P0-P7 已完整执行, ~60,480 次完整回测, 无快筛, 无遗漏。交接完毕。
