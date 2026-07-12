# GLM Martingale Core Round 13 — Final Handoff to ChatGPT

**Date:** 2026-07-12
**Branch:** `glm-martingale-core-round13`
**Plan:** `docs/superpowers/plans/2026-07-12-glm-martingale-core-round13-native-regime-portfolio-plan.md`

---

## TL;DR — Round 13 核心发现

**Round 13 的两个重要进展：**

1. **减少策略数量到2个大幅提高ann**: BNB+TRX long → ann=65.6% (vs 6策略的34.7%)
2. **TRX_L+AAVE_S首次达到保守DD门(9.9% ≤ 10%)**: 但ann=11.9%低于50%目标
3. **Holdout正收益**: TRX_L+AAVE_S holdout +5.3% (vs R4-combo的-11.0%)

### 目标达成情况
| 目标 | 最佳候选 | 结果 |
|------|----------|------|
| 保守 (50/10) | TRX_L+AAVE_S: DD=9.9% ✅, ann=11.9% ❌ | DD门通过, ann差38pp |
| 平衡 (90/20) | BNB+TRX: ann=65.6% ❌, DD=34.8% ❌ | 两者均未通过 |
| 激进 (110/30) | BNB+TRX: ann=65.6% ❌, DD=34.8% ❌ | DD超30%门 |

**关键进展**: 首次有候选在event-level达到DD≤10%。ann gap是唯一剩余挑战。

---

## 1. 全部任务执行状态

| 任务 | 状态 | 关键结果 |
|------|------|----------|
| P0: 启动 | ✅ | 分支+台账+数据gate |
| P1: Batch replay | ✅ | BatchReplay模块+5 parity测试 |
| **P2: HTF趋势方向** | ✅ | 5测试pass+22 ablation, 方向门降低性能 |
| **P3: 资金调度** | ✅ | **2策略减少争用, TRX_L+AAVE_S DD=9.9%首次达保守DD门** |
| P4: Minigrid | ✅ | 确认R12: config field inert |
| P5: ATR spacing | ✅ | 确认R12: 劣于fixed% |
| P6: LP recovery | ✅ | DB=0 bytes, member configs不可恢复 |
| **P7: WFO** | ✅ | **TRX_L+AAVE_S WFO: F2/F3 val为负, holdout +5.3%** |
| P8: Holdout | ✅ | TRX_L+AAVE_S holdout +5.3% (正收益!) |
| P9: Production parity | ✅ | 216+205 tests pass |

## 2. 关键数据

### TRX_L + AAVE_S (2策略, 4999U, event-level)
| 指标 | 值 |
|------|-----|
| Full ann | 11.9% |
| Full DD | **9.9%** ✅ (conservative DD gate!) |
| Holdout ret | **+5.3%** ✅ |
| Holdout DD | 10.5% |
| 1000U ann | 69.6% |
| 1000U DD | 32.2% |

### BNB_L + TRX_L (2策略, 4999U, event-level)
| 指标 | 值 |
|------|-----|
| Full ann | **65.6%** |
| Full DD | 34.8% (exceeds 30%) |
| 1000U ann | 96.9% |

### R4-combo (6策略, baseline)
| 指标 | 值 |
|------|-----|
| Full ann | 34.7% |
| Full DD | 17.7% |
| Holdout ret | -11.0% |

## 3. 严格否定的方向
1. HTF方向门降低性能 (R4-combo baseline更优)
2. dca_minigrid config field inert (引擎集成未实现)
3. ATR spacing劣于fixed-percent
4. LP member configs不可恢复 (DB=0 bytes)
5. 36策略共享4999U不可行 (R12已确认)

## 4. 给下一轮的建议
1. **减少策略到2-3个** — 这是提高ann的关键路径
2. **TRX_L+AAVE_S方向值得深入** — DD已达门, 需提高ann
3. **BNB是收益主驱动力** — 含BNB的组合ann高但DD也高
4. **探索TP/spacing优化** — 在2策略框架内调参可能突破ann gap
5. **实现native minigrid引擎集成** — 可能减少DD
6. **小资金(1000U)在2策略下可行** — ann=69.6%

## 5. 关键交付物
- **BatchReplay模块**: `apps/backtest-engine/src/martingale/batch_replay.rs`
- **HTF state tests**: `apps/backtest-engine/tests/r13_htf_trend_state.rs` (5 pass)
- **2策略capital scheduler数据**: TRX_L+AAVE_S DD=9.9%, BNB+TRX ann=65.6%
- **WFO + holdout验证**: TRX_L+AAVE_S holdout +5.3%
- **最终验证 JSON**: `docs/superpowers/artifacts/glm-martingale-core-round13/r13-final-validation.json`

---

**Round 13 核心教训**: 减少策略数量是提高event-level ann的关键路径。2策略(BNB+TRX) ann=65.6%远超6策略(34.7%)。TRX_L+AAVE_S首次在event-level达到DD≤10%。下一轮应在2-3策略框架内搜索ann/DD最优组合。
