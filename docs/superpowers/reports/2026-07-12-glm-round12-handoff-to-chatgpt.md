# GLM Martingale Core Round 12 — Final Handoff to ChatGPT

**Date:** 2026-07-12
**Branch:** `glm-martingale-core-round12`
**Plan:** `docs/superpowers/plans/2026-07-12-glm-martingale-core-round12-event-level-native-search-plan.md`
**Final validation JSON:** `docs/superpowers/artifacts/glm-martingale-core-round12/r12-final-validation.json`

---

## TL;DR — Round 12 核心发现

**Round 12 的核心贡献是发现了两个关键事实：**

1. **R9 curve-reuse diagnostic (62.78%/18.38%) 在 event-level 下完全不可复现** — 36 strategies 共享 4999U → ann=-7.5%/DD=56%
2. **R4-combo 在 untouched holdout (2026-06-01~07-10) 为负收益 (-11.0%)** — 策略可能正在衰减

### 目标达成情况
| 目标 | 最佳 event-level | holdout | 结论 |
|------|-----------------|---------|------|
| 保守 (50/10) | ann 34.73% / DD 17.69% | -11.0% ret | **未达** |
| 平衡 (90/20) | ann 34.73% / DD 17.69% | -11.0% ret | **未达** |
| 激进 (110/30) | ann 34.73% / DD 17.69% | -11.0% ret | **未达** |

**结论**: 三个目标在 event-level + shared budget + <5000U 下均未达成。且 R4-combo 在最近 39 天 holdout 为负。

---

## 1. 全部任务执行状态

| 任务 | 状态 | 结果 |
|------|------|------|
| P0: 启动 | ✅ | 分支+台账 |
| P1: 冻结数据 | ✅ | ANKR(3997)+LTC(3741)资金费补齐, 4测试pass |
| P2: Validator | ✅ | 统一验证器+OOS harness |
| **P3: R9 event-level** | ✅ | **FAMILY FAILURE: ann=-7.5%, curve高估70pp** |
| **P4: Minigrid probe** | ✅ | **BINDING FAILED: dca_minigrid字段在engine中inert** |
| **P5: SO v2 probe** | ✅ | **BINDING FAILED: ADX/DD-scale对partial-TP不bind** |
| **P6: ATR+cycle-depth** | ✅ | **576 configs, 0 targets, ATR劣于fixed%** |
| **P7: LP rebuild** | ✅ | **ann 30.44%/DD 41.31%, 差于R4, LP高资金设计不适合4999U** |
| P8: Batch benchmark | ✅ | 21.7s/config, 15%并行效率, batch plan documented |
| **P9: Holdout** | ✅ | **R4-combo -11.0% (NEGATIVE), 策略可能衰减** |
| P10: Production parity | ✅ | 211+205 tests pass, allocator wired (R11 P1) |

## 2. 严格否定的机制

1. **Curve-reuse ≠ event-level**: R9 36策略共享4999U → 严重budget contention → ann 从62.78%降到-7.5%
2. **dca_minigrid config field inert**: kline_engine不读取该字段; 所有3个变体产生相同trades/metrics
3. **ADX skip + DD scale对partial-TP不bind**: 相同trades/metrics; 只对percent-TP有效
4. **ATR spacing劣于fixed-percent**: ann -4.74% vs 34.73%; 过度交易
5. **Cycle-depth TP不优于原R4 partial TP**: 576 configs全部更差
6. **LP高资金组合不适合4999U**: DD从10%(高资金)膨胀到41%(4999U)

## 3. 未完成的引擎集成

| 功能 | 状态 | 原因 |
|------|------|------|
| Native minigrid kline_engine | config struct+5测试完成 | 引擎集成需~200行插入kline_engine (9000+行) |
| SO v2对partial-TP | binding probe failed | 需修改safety order path以在partial-TP cycle中生效 |
| Batch acceleration | 基准完成 | preload+Rayon需重构binary API |

## 4. 小资金问题

| Budget | Full-period ann | Holdout ret |
|--------|-----------------|-------------|
| 1000U | -15.7% | -18.5% |
| 2000U | -7.1% | -9.2% |
| 3000U | 49.5% | -18.3% |
| 4000U | 40.7% | -13.7% |
| 4999U | 34.7% | -11.0% |

**1000U和2000U在全期和holdout均为负。** 小资金是一个真实的未解决挑战。

## 5. 给下一轮的建议

1. **必须用event-level shared-budget replay** — curve-reuse已被严格否定
2. **减少strategies数量** — 36 strategies共享4999U不可行; ≤6-8 strategies更合理
3. **调查2026年策略衰减** — holdout为负; 可能需要regime-adaptive参数
4. **实现native minigrid引擎集成** — config已就绪, 需kline_engine.rs集成
5. **探索完全不同的TP/spacing架构** — 当前R4-combo参数族已被充分搜索

## 6. 关键交付物

- **Frozen data manifest**: `r12-data-manifest.json` (ANKR+LTC funding补齐)
- **R9 event-level benchmark**: ann=-7.5%/DD=56% (curve不可复现)
- **R4-combo event-level baseline**: ann=34.73%/DD=17.69%/4/5
- **R4-combo holdout**: -11.0% (NEGATIVE, 策略衰减)
- **最终验证 JSON**: `r12-final-validation.json`
- **本交接文档**: `2026-07-12-glm-round12-handoff-to-chatgpt.md`

---

**Round 12 核心教训**:
1. Curve-reuse diagnostic不是event-level metric — 过去9轮的"最佳结果"在共享预算下不可复现
2. R4-combo (34.73%/17.69%)是真实event-level前沿, 但在最近holdout为负(-11.0%)
3. 三个目标(50/10, 90/20, 110/30)在event-level + shared budget + <5000U下均未达成
4. 小资金(1000-2000U)在event-level下为负 — 这是一个真实的未解决挑战
