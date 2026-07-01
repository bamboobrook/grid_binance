# GLM Martingale Core — Stop Report (2026-07-01)

Per plan section 8, this report is written because search condition 3 is met:
"搜索 5 条方向后仍无 near-miss，且证据显示收益/回撤 frontier 没有改善" — more
precisely, the ann/DD frontier has a confirmed empty middle that no lever
(regime gate, ATR/ADX, single-strategy SL, portfolio DD stop) breaks.

## 1. 达标候选路径
None. No candidate satisfies all six gates (return + DD + budget + multi-symbol +
segment-stability + live-parity) for any profile. See `2026-07-01-glm-martingale-core-final-candidates.md`.

## 2. 未达标 frontier (best confirmed, segment-first validated)
- Aggressive: ann 73.5% / DD 45.1% / 2/5 pos / agg24-26 +65.5% (high-TP+strict-gate).
- Aggressive segment-stable: ann 2.9% / DD 23.6% / 3/5 pos / agg24-26 +18.8%.
- Conservative low-DD: ann −2.8% / DD 9.4% / 1/5 pos.
- 8 portfolio candidates made 2024+2025 both-positive (first time ever, vs 0/590 historically).

## 3. 每条方向的投入、行数、最好结果和失败原因

| 方向 (plan section) | 行数/候选 | 最好结果 | 失败原因 |
|---|---|---|---|
| A: per-symbol regime gate (单策略) | 3276 候选 | 0 过 2024+2025 段门 | regime gate 过严→几乎不交易(近零); 过松→亏损。单symbol无法双段正 |
| B: ATR/ADX 效率 (高TP+高multiplier) | 1800+ 候选 | ann 73.5% / DD 45.1% | 收益突破但 DD 45% 远超 30% 门禁 |
| C: active-cycle 风控 (portfolio DD stop) | 1260+ 候选 | DD 5.75% (新低) / ann 3.1% | DD stop 把 DD 从 37%→5.75%，但收益同时被砍; DD stop 不能制造收益 |
| D: segment-first walk-forward | 全部搜索基础 | 8/2592 portfolio 双段正 | 2024/2025 反相关结构性冲突 |
| E: 动态多币种组合 | 2592+224 候选 | 3/5 段正 / agg24-26 +18.8% | 组合改善了段稳定性，但收益天花板仍低 |
| 紧单策略 SL 扫描 (附加) | 1008 候选 | DD 9.4% / ann −2.8% | 紧SL降DD但杀收益；无 ann>50 & DD≤30 的 sweet spot |

## 4. 下一步是否仍保持马丁核心
**是，仍保持马丁核心。** 收益突破(73.5%)证明马丁核心在小资金+多币种+segment-first
下能产生高收益。问题不是马丁核心本身，而是 ann/DD 悬崖——这是马丁的结构特性(收益来自
价格回归平均入场，需承受浮亏)。突破悬崖需要：ATR-adaptive TP、恢复重入 DD stop、
或熊市 regime 倾斜(short sleeve 捕捉更多 2025/2026 崩盘)。

## 5. 是否需要用户批准新的范围
**需要。** 两个决策点：
1. 收益目标(50/90/110% ann)与 DD 目标(10/20/30%)在 segment-first + 小资金 + 多币种
   约束下经本轮验证不可同时达成。是否：(a) 接受 ~10-20% ann 的现实 frontier，
   (b) 放宽 DD 门禁(如 aggressive DD≤40%)，或 (c) 继续投入 ATR-adaptive/恢复重入
   方向尝试突破悬崖。
2. 是否继续下一轮搜索(ATR-adaptive TP + live DD stop 恢复重入)，还是先让用户决策。

## 本轮已交付的工程价值(无论收益目标如何)
- portfolio equity stop 从 research-only 提升为 **完整 live-parity** (config + backtest +
  trading-engine close/cooldown + 185 tests)。这是实盘可复现的实质进步。
- segment-first 验证框架(glm_segment_validator.py)和搜索脚本可复用于下一轮。
- 所有 5 段证据已记录,每次最佳结果都已提交(11 commits, 全部推送)。
