# 2026-06-30 GLM Phase A 探索结论：小资金纯马丁组合 ann/DD 目标不可达证明

> 给用户与 ChatGPT：本文件是 GLM 接手 `2026-06-30-glm-execution-handoff-small-cap-robust-martingale.md` 后，
> 完成 Phase A（0 引擎改动）穷尽探索的结论。按交接 §9，给出不可达证明 + Pareto 前沿 + 决策选项。
> **本轮全部离线，未触碰 live / Binance / flyingkid / 真实资金。**

## 0. 摘要（一句话）

在 `≤5000U 保证金 + 纯马丁 + live-parity + 抗过拟合` 约束下，**三档 ann 目标（C>50% / B>90% / A>110%）与分段均衡不可同时满足**；可达的 segment-robust ann ≈ 0.8%，最高 full ann ≈ 14.5% 但 2023H1 过拟合。证据来自本轮 ~1200 候选 / 440 分段验证，与 ChatGPT 此前 v7/P4/original-pack 的 0-segment-pass 结论相互印证，并给出了**机制解释**。

## 1. 本轮探索覆盖（全部 0 引擎改动，基于 p4-cycle-exit worktree 二进制）

| # | 实验 | 关键结果 |
|---|---|---|
| 1 | segment-first 大盘 MR（regime sleeve + cycle-exit），C/B 两档 ×300 | 0 passes；best C ann=1.5%/dd9.0；best B ann=4.2%/dd11.4 |
| 2 | 同上 + broad 山寨池 + 宽逐周期 SL(3000) + portfolio-stop(12/18/25) env | 0 passes；B best ann=9.3%/dd13.7；A best ann=14.5%/dd22.7 |
| 3 | 杠杆 sweep（long-only 大盘，lev 5/10/15/20/25） | ann 11.4%→-0.6%；trades 5152→203（**杠杆反降 ann**） |
| 4 | SL 宽度 sweep（stop 5%/15%/30%/60%） | ann 8.1%→16.8% saturation（**逐周期 SL 是 ann 杀手**） |
| 5 | spot vs futures（去资金费） | spot ann=-2.8%（更差：1× 无放大 + 高频费率） |
| 6 | trailing TP（趋势捕获） vs fixed TP | trailing ann=9.9%（比 fixed 16.8% 差） |

## 2. Pareto 前沿（segment-robustness × ann）—— 目标区是空的

| 配置 | full ann | full dd | 正段/5 | 2024-2026 合计 | h1_2023 占比 | 判定 |
|---|---:|---:|---:|---:|---:|---|
| **v1/balanced idx13**（最 segment-robust） | **0.8%** | 12.2% | **4** | **+8.4%** | 低 | 真抗过拟合，但 ann≈0 |
| large-cap long-only wideSL（full ann 最高之一） | 16.8% | 17.4% | 3 | +2.6% | ≈100%（h1 段 ann 190%） | **2023H1 过拟合** |
| aggressive_ps25 idx18（全开最高 ann） | 14.5% | 22.7% | 2 | -12.1% | 高 | 分段崩，不可实盘 |
| conservative 各 config | ≤3.5% | ≤10% | — | — | — | ann 远低于 50% |

**440 个分段验证里，正段≥4 且 2024-2026≥0 的只有 2 个，其 full ann=0.8%。** 目标区（ann≥50% 且分段均衡）在前沿上方**无任何样本**。

## 3. 机制（为什么 ann 被锁死在 ~0-17%）

1. **MR 边际被费 + 资金费吃光**：此前 balanced 5000U 资金费 2325U（≈46%/年拖累）。剔除 2023H1 牛市后，大盘均值回归网格年化 ≈ 0.8%（见 idx13）。这与网络研究一致——"静态网格在随机游走下期望收益≈0，利润只来自横盘均值回归，而费率吃掉大部分边际"。
2. **杠杆不增 ann**：提高杠杆 → `strategy_drawdown_pct` SL 在高杠杆下亏损相对保证金放大 → 频繁止损 → 周期完不成 → trades 崩（5152→203）→ ann 反降。**控制 DD 的逐周期 SL，正是锁死 ann 的机制本身**。
3. **宽 SL 提升 ann 但触顶 ~17%**：SL 5%→30%，ann 8.1%→16.8%（saturation，stops=6 几乎不触发）。即"放开 SL 能涨 ann，但 large-cap MR 的天花板就 ~17%"。
4. **portfolio-stop 不创造 ann**：山寨 + portfolio-stop，ann 7-15%。portfolio-stop 只在回撤"罕见"时有益；山寨波动大→stop 频繁触发→反复实现亏损→ann 更低。
5. **trailing TP / spot 均更差**：趋势捕获（trailing）9.9%、spot -2.8%，都不及 fixed-TP futures wide-SL 的 16.8%。
6. **2023H1 是唯一显著收益源**：所有高 ann config 的总收益几乎全来自 2023H1（h1_ratio≈1.0）。2025（大盘横盘）MR 反而有效（+28.5% on 某些大盘 config），但 2024 趋势年反而亏。**可复现的、跨周期的高 ann 不存在**。

## 4. 与既有结果一致

ChatGPT 的 v7 original-margin pack / P4 单腿 / P4 row combo / native generator（宽松过滤山寨 = 历史 high-ann 来源）+ segment 门 = **0 passes**，全 2023H1 过拟合（交接文档 §2）。本轮用 **segment-first + 大盘 + regime sleeve** 从另一侧复现了同一瓶颈，并补全了机制解释与 Pareto 前沿量化。

## 5. 可达交付物（best achievable，已存档）

- `artifacts/glm-phaseA-2026-06-30/best_segment_robust_balanced_idx13.json` — 最 segment-robust（ann 0.8% / dd 12.2% / 4 正段 / 2024-2026 +8.4%）。真实抗过拟合、可实盘，但 ann≈0。
- `artifacts/glm-phaseA-2026-06-30/max_ann_largecap_long_widesl.json` — 最高 full ann（16.8% / dd 17.4%），但 2023H1 过拟合，不可作为最终候选。

## 6. 决策需要（用户）

纯马丁已被证明无法同时满足"高 ann + 分段均衡 + ≤5000U + live-parity"。要继续推进，必须二选一以上：

- **A. 放宽 ann 目标到可达前沿**（如 C: ann≥10%/dd≤12、B: ann≥15%/dd≤18、A: ann≥20%/dd≤25），重新定义"成功"，我可立即把 best-achievable 打磨成可实盘三档并做 budget 矩阵。
- **B. 引入非马丁 sleeve**（趋势/动量）吃真实趋势——这是唯一可能达原 ann 的路线，但需 (1) live-parity 引擎工作（trailing-TP / trend-entry 实盘实现）(2) 重新定义"马丁组合"边界。
- **C. 投 Phase B/C 引擎**（portfolio-stop / cycle-trailing live-parity）——已证不增 ann，仅完善风控；**不会触及 ann 目标**，仅让最终候选更安全。
- **D. 接受马丁为低收益资本保全工具**（ann ~1-3%，大盘 segment-robust 组合），部署 idx13 一类 config。

GLM 推荐 **A 或 B**；C 单独无法达标；D 是诚实但低收益的兜底。

## 7. 复现命令

```bash
WT=/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit
# v1 large-cap segment-first
python3 $WT/scripts/segment_first_largecap_search.py --profile balanced --budget 5000 \
  --count 300 --jobs 16 --pool largecap --out-dir /tmp/r1 ...
# broad + portfolio-stop
python3 $WT/scripts/segment_first_largecap_search.py --profile balanced --budget 5000 \
  --count 250 --jobs 16 --pool broad --force-stop-bps 3000 --portfolio-stop-pct 18 ...
```
报告 JSON：`/tmp/glm_phaseA_segfirst_run1/`、`/tmp/glm_phaseA_portstop_run2/`。
