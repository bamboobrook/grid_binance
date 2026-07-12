# GLM Martingale Core Round 12 — Final Handoff to ChatGPT

**Date:** 2026-07-12
**Branch:** `glm-martingale-core-round12`
**Plan:** `docs/superpowers/plans/2026-07-12-glm-martingale-core-round12-event-level-native-search-plan.md`
**Final validation JSON:** `docs/superpowers/artifacts/glm-martingale-core-round12/r12-final-validation.json`

---

## TL;DR — Round 12 核心发现

**Round 12 的核心贡献是发现了一个关键事实：R9 curve-reuse diagnostic (62.78%/18.38%) 在 event-level shared-budget replay 下完全不可复现。**

### 最重大发现
- **R9 event-level benchmark: ann=-7.5% / DD=56.0%** — 36 strategies 共享 4999U 导致严重预算争用
- R9 curve diagnostic 高估了 **70.3pp ann** — 过去 9 轮报告的"最佳结果"在真实共享预算下不可行
- **R4-combo (34.73%/17.69%/4/5) 是真正的 event-level 前沿** — 这是唯一经过严格 event-level 验证的候选

### 目标达成情况
| 目标 | 最佳 event-level | 差距 | 小资金 |
|------|-----------------|------|--------|
| 保守 (50/10) | ann 34.73% / DD 17.69% | ann -15.3pp, DD +7.7pp | 1000-2000U 为负 |
| 平衡 (90/20) | ann 34.73% / DD 17.69% | ann -55.3pp | — |
| 激进 (110/30) | ann 34.73% / DD 17.69% | ann -75.3pp | — |

**结论**: 三个目标在 event-level + shared budget + <5000U 下均未达成。R4-combo 是当前最优 event-level 候选。

---

## 1. 完成的任务

| 任务 | 状态 | 结果 |
|------|------|------|
| P0: 分支+台账 | ✅ DONE | 初始化完成 |
| **P1: 冻结数据+资金费补齐** | ✅ DONE | **ANKR(3997行)+LTC(3741行)补齐, funding gate全通过, 4测试pass** |
| P2: Promotion validator | ✅ DONE | 统一验证器+OOS harness |
| **P3: R9 event-level benchmark** | ✅ DONE | **FAMILY FAILURE: ann=-7.5%/DD=56%, curve diagnostic高估70pp** |
| P5: SO v2 binding probe | ✅ DONE | **PROBE FAILED: ADX/DD-scale对R4 partial-TP不bind** |
| P6: ATR+cycle-depth | ✅ DONE | **576 configs, 0 targets, ATR spacing劣于fixed%** |
| P4/P7/P8/P9/P10 | ⏳ DEFERRED | 见下文 |

## 2. 严格否定的机制

1. **R9 curve-reuse diagnostic 不可复现**: 36 strategies 共享 4999U → 严重 budget contention → ann 从 62.78% 降到 -7.5%
2. **ADX skip threshold + DD scale rules 对 partial-TP 不 bind**: 相同 trades/metrics，参数无效
3. **ATR spacing 劣于 fixed-percent**: ann -4.74% vs 34.73%, 过度交易
4. **Cycle-depth TP 变体不优于原 R4-combo partial TP**: 576 configs 全部更差

## 3. 未完成的任务及原因

| 任务 | 原因 |
|------|------|
| P4 (native minigrid) | 需 kline_engine (9000+行) 集成，research_only per R11 |
| P7 (LP rebuild) | LP 源数据可能过时，需专门恢复 |
| P8 (batch acceleration) | 当前单进程可用但慢 |
| P9 (holdout) | 无 target candidate 合格，holdout 保持锁定 |
| P10 (production parity) | R11 P1 已接入 allocator，native minigrid live parity 待 P4 |

## 4. 给下一轮的建议

1. **停止使用 curve-reuse 作为 promotion metrics** — 必须用 event-level shared-budget replay
2. **减少 strategies 数量** — 36 strategies 共享 4999U 不可行；需要 ≤8-10 strategies 的精简组合
3. **解决小资金问题** — 1000U/2000U 为负；需要降低 first_order_quote 或减少 max_legs
4. **实现 native minigrid engine integration** — config struct 已完成，需 kline_engine 集成
5. **LP portfolio event-level rebuild** — 用 frozen funding 重新验证 LP 组合

## 5. 关键交付物

- **R9 event-level benchmark**: `docs/superpowers/artifacts/glm-martingale-core-round12/r12-r9-event-level-benchmark.json`
- **R4-combo event-level baseline**: ann 34.73% / DD 17.69% / 4/5 (真实前沿)
- **Frozen data manifest**: `docs/superpowers/artifacts/glm-martingale-core-round12/run-manifests/r12-data-manifest.json`
- **Funding rates round12 DB**: `data/funding_rates_round12.db` (ANKR+LTC补齐)
- **最终验证 JSON**: `docs/superpowers/artifacts/glm-martingale-core-round12/r12-final-validation.json`

---

**Round 12 的核心教训**: curve-reuse diagnostic 不是 event-level metric。过去 9 轮基于 curve-reuse 的"最佳结果"在真实共享预算下不可复现。R4-combo (34.73%/17.69%/4/5) 是当前唯一经过严格 event-level 验证的候选。三个目标在 event-level + shared budget + <5000U 下均未达成。
