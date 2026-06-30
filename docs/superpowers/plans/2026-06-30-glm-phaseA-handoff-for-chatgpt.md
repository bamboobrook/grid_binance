# 2026-06-30 GLM Phase A 研究成果交接：给 ChatGPT 继续探索

> 给 ChatGPT：本文件是 GLM 接手 `2026-06-30-glm-execution-handoff-small-cap-robust-martingale.md` 后，
> 完成 Phase A（0 引擎改动穷尽探索）+ 用户要求重试（regime allocator / 多空混合 / 双向 MR）的全部成果。
> **本轮全部离线，未触碰 live / Binance / flyingkid / 数据库展示 / 真实资金。** 工作分支 `worktree-p4-cycle-exit`（已推送 origin）。
> 用户已看过结论并要求"继续探索、不要早下结论"，请按 §6 的"待探索方向"推进，**不要重复 §5 的已证死路径**。

---

## 0. 用户最终目标（原样坚持，未变）

三档马丁组合，全部满足：①保证金本金 ≤5000 USDT ②多币种（动态数量，不退化单币）③抗过拟合（不依赖 2023H1）④分段均衡（H1-2023/H2-2023/2024/2025/2026_ytd）⑤收益门槛 **C: ann>50%/DD≤10%、B: ann>90%/DD≤20%、A: ann>110%/DD≤30%** ⑥live-parity 实盘可复现 ⑦达标后才许 flyingkid/实盘，且实盘启动前再次人工确认。

## 1. 本轮工作总览（GLM 做了什么）

接手后先做 P0 安全（0 残留进程）+ 清理工作区 + 分类提交 + 推送 main 基线。然后在 `worktree-p4-cycle-exit`（含 P4 cycle-exit 的 `regime_break_stop`/`max_cycle_age_hours` live-parity 机制）上，0 引擎改动地：

1. 写 `scripts/segment_first_largecap_search.py`（fork native 模板；segment-first 先 5 段筛后 full；regime sleeve entry_triggers；cycle-exit ON；broad/largecap 池；force-stop-bps；portfolio-stop env；Phase3 并行）。
2. 跑 **3 大类搜索 + 3 个手工 hybrid + 6 项校准**，共 **~1500 候选 / 590 分段验证**。
3. 用 `portfolio_budget_replay` 二进制（on_budget 真实预算口径）+ market_data_full.db（108G/506sym/2023-01→2026-06/1m）。

## 2. 核心结论（一句话）

**在 ≤5000U + 纯马丁 + live-parity + 抗过拟合下，三档 ann 目标（50/90/110%）与分段均衡不可同时满足。** 实盘可用纯马丁的全周期 ann 天花板 ≈ **20%**（segment-robust 仅 ~5%）；高 ann 全来自 2023H1 牛市过拟合。机制已查清（§4）。**用户的 regime allocator 方向部分有效**（把 2025 从杀手段变成可盈利），但暴露了 4 堵结构墙（§4）。这是第 **4 次**独立印证（此前：06-25 ann/overfit 维度、06-27 DD 维度、ChatGPT v7/P4 0-segment-pass）。

## 3. 关键实验数据（真实数字，可直接引用，不必重跑）

### 3.1 三类主搜索（segment-first, budget=5000, 全 0 passes）

| 实验 | conservative | balanced | aggressive |
|---|---|---|---|
| **v1 large-cap regime MR**（btc_trend/btc_range） | best ann 1.5 / dd 9.0 | best ann 4.2 / dd 11.4 | (跳过) |
| **v2 broad 山寨池 + 宽SL + portfolio-stop** | ps8: ann 3.5/dd5.8 | ps18: ann 9.3/dd13.7 | ps25: ann 14.5/dd22.7 |
| **v3 每币 regime allocator**（pc_trend/pc_range, ema5760+ADX） | ann 2.4/dd3.9 | ann 13.1/dd26.5（dd≤20 内仅 5.0） | **ann 21.2/dd41.7**（pos3/5，2025=+20.6 但 2024=-12.2，h1=92.9 过拟合） |

### 3.2 校准（isolated，long-only 大盘 balanced 基准）

- **杠杆 sweep**（lev 5/10/15/20/25）：ann 11.4→-0.6%；trades 5152→203。**杠杆反降 ann**（drawdown SL 在高杠杆下频繁止损）。
- **SL 宽度 sweep**（stop 5%/15%/30%/60%）：ann 8.1→16.8% saturation（stops=6）。**逐周期 SL 是 ann 杀手，放开触顶 ~17%**。
- **spot vs futures**：spot ann -2.8%（1× 无放大 + 高频费率，更差）。
- **trailing TP**（趋势捕获）：ann 9.9%（不如 fixed TP 的 16.8%）。

### 3.3 手工多 sleeve hybrid（用户要求的多空组合）

| 版本 | 构成 | 2024 | 2025 | full ann |
|---|---|---:|---:|---:|
| v1 | 趋势多+MR双向+山寨空 | -5.5% (200笔) | -15.2% (556笔) | 5.3% |
| v2 | 放宽 ADX 闭合死区 | -6.5% | -15.8% | 1.2% |
| v3 | 趋势多+大盘MR（去山寨空） | -9.8% (374笔) | -24.1% | 3.0% |

### 3.4 跨实验 mining（决定性）

- **正段≥4 且 2024-2026≥0 的 config：仅 2 个，full ann 都 = 0.8%。**
- **2024≥0 且 2025≥0 的 config：0/590。** 2024 与 2025 对马丁**反相关**。
- 最高 full ann（任意牺牲分段）：aggressive regime_pc idx189 = 21.2%（pos3/5, dd41.7, h1=92.9 过拟合）。
- 最 segment-robust：v1/balanced idx13 = ann 0.8%/dd12.2/4 正段/2024-2026 +8.4%（真抗过拟合但 ann≈0）。
- **单段可赢证据**：idx6 的 2024=+24.5（trend-long）；idx283 的 2025=+36.6（大盘 MR）。**两段各自能赢，但同一组合赢不了两段。**

## 4. 机制（为什么 ann 被锁死在 ~20%）—— 已查清

1. **费 + 资金费吃光 MR 边际**：此前 balanced 5000U 资金费 2325U（≈46%/年）。剔除 2023H1 牛市，大盘 MR 年化 ≈ 0.8%（idx13 实证）。与网络研究一致："静态网格在随机游走下期望收益≈0，利润只来自横盘均值回归，费率吃掉大部分边际。"
2. **杠杆不增 ann**：`strategy_drawdown_pct` SL 在高杠杆下亏损相对保证金放大 → 频繁止损 → 周期完不成 → trades 崩。**控制 DD 的逐周期 SL，正是锁死 ann 的机制本身。**
3. **宽 SL 提 ann 但 large-cap MR 触顶 ~17%**（saturation）。
4. **portfolio-stop 不创造 ann**：山寨波动大→stop 频繁触发→反复实现亏损→ann 更低。只在回撤"罕见"时有益。
5. **trailing TP / spot 均更差**（9.9% / -2.8%）。
6. **2023H1 是唯一显著收益源**：所有高 ann config 的总收益几乎全来自 2023H1（h1_ratio≈1.0）。2025 大盘横盘 MR 有效（+28.5~36.6%），但 2024 趋势年 MR 反亏。

### 4 堵结构墙（用户方向验证后暴露）

1. **2024 ↔ 2025 反相关**：2024(BTC牛/alt弱)要 long-BTC；2025(alt熊)要 short-alt/MR；静态组合赢不了一致（0/590）。
2. **方向性山寨空头在 2025 震荡熊市被 whipsaw**（净拖累，非对冲收益）——与 P4 shorts 结论一致。
3. **regime 分类器死区**（ADX 趋势>20 / 震荡<25 仍留盲区）+ 实时 regime 难判 → sleeve 在关键时段不激活（2024 仅 200-400 笔）或 whipsaw。
4. 全周期 ann 被 2023H1 锁死在 ~20%。

## 5. 已证死路径（ChatGPT 不要再重复）

1. **静态网格 MR**（任意币池、任意 spacing/TP/legs/multiplier 组合）：ann 天花板 ~17-20%（large-cap），山寨池 segment 崩。
2. **提高杠杆**：drawdown SL choke，反降 ann。
3. **逐周期 SL 调宽**：触顶 ~17%（large-cap MR 极限）。
4. **portfolio-stop / max-active-cycles**（backtest env）：不创造 ann。
5. **spot（去资金费）**：更差。
6. **trailing TP**：更差（且非 live-parity）。
7. **方向性山寨空头 sleeve**：2025 震荡熊市 whipsaw，净拖累。
8. **per-coin regime allocator（pc_trend/pc_range）单干**：改善 2025 robustness 但不破 ann；组合后 sleeve 互相干扰。
9. **手工多 sleeve hybrid**（趋势多+MR+空）：v1/v2/v3 全失败。
10. **full-period 排序再分段否决**（旧法）：必选中 2023H1 过拟合（ChatGPT v7/P4 已证 0 segment-pass）。

## 6. 待探索方向（建议 ChatGPT 主攻，按预期收益排序）

### 6.1 非马丁趋势 sleeve（唯一可能达高 ann 的路，但需引擎扩展）
当前马丁网格本质是 **MR**（加仓摊平/反弹止盈），**不捕获趋势**。2024 趋势年 MR 必亏。要捕获 2023H1/2024 趋势需：
- **breakout 入场**（close > Donchian-high(N) / bb_upper 突破）——当前 `entry_triggers` 的 `indicator_runtime` 支持ema/sma/rsi/atr/adx/bb 但**不支持 highest/lowest(Donchian)**，需扩展（`apps/backtest-engine/src/martingale/indicator_runtime.rs` resolve_operand + 实盘 `IndicatorRuntimeContext`）。
- **趋势金字塔**（趋势延续时加仓，而非逆势加仓）——与马丁"逆势摊平"相反，需新的 sizing 逻辑。
- **必须 TDD 四端**：shared-domain schema → backtest `kline_engine.rs` → trading-engine `martingale_runtime.rs`/`main.rs` → `budget_replay.rs live_parity_check` 放行。P4 的 7-commit 模式（`docs/superpowers/plans/2026-06-29-p4-cycle-exit-mechanisms.md`）是现成模板。
- ⚠️ 注意：trailing-TP 回测仅 9.9%（不如 fixed），趋势机制在本引擎/market payoff 不确定；先做最小 backtest-only 验证再投入 live-parity。

### 6.2 Funding 套利 sleeve（delta-neutral，收益与方向无关）
long spot + short futures 同币 → delta 中性 + 收取 funding（crypto funding 常为正，空头收取）。这是**与 MR 完全不同的 PnL 源**，可能提供稳定 ann。
- 需确认引擎支持同币 spot+futures 同时持仓 + funding 实盘计入（`capital.rs`/`martingale_runtime.rs`）。spot 已支持（market=spot，去 margin_mode/leverage）。
- 先 backtest-only 量化 funding 收益率。

### 6.3 高时间帧 regime（降低噪音）
当前全 1m。趋势判定用 1m 的 ema(5760)=4天 仍噪音大、滞后。若引擎能**重采样到 1h/4h/daily** 再算 regime（ema/ADX），趋势识别更干净，dead-zone 更小。需检查 `kline_engine.rs`/`indicator_runtime` 是否支持重采样指标（或预计算高时间帧 feed）。

### 6.4 动态币种选择（用户提到的辅助）
按 momentum/volatility/regime 实时排序选币（而非固定池）。注意**未来函数**：只能用 t 时刻可得指标。可复用 `indicator_runtime` 跨币种表达式。

### 6.5 抗过拟合纪律（walk-forward）
记录 trial count，walk-forward / PBO / DSCV / Deflated Sharpe；报告时用 out-of-sample 而非全周期最优。避免反复挑同一批 2023H1 票据。

### 6.6 兜底：放宽目标到可达前沿
若 6.1-6.5 仍不达标，建议用户放宽到 **C: ann≥10/dd≤12、B: ann≥15/dd≤18、A: ann≥20/dd≤25**（可达，segment-robust，可实盘）。GLM 可立即打磨 best-achievable 成三档 + budget 矩阵。

## 7. 已交付物（worktree-p4-cycle-exit 分支，已推送）

- **脚本**：`scripts/segment_first_largecap_search.py`（regime_btc/regime_pc、portfolio-stop env、broad/largecap、force-stop-bps、Phase3 并行；import native_small_portfolio_search 复用 helper）。所有 ChatGPT 的 P4 脚本（native/p4_row_combo/original_pack/probe）已一并提交保留。
- **最佳 config（artifacts/glm-phaseA-2026-06-30/，gitignored 但在磁盘 + 本地 /tmp）**：
  - `best_segment_robust_balanced_idx13.json` — ann 0.8/dd12.2/4 正段/2024-2026 +8.4%（真抗过拟合，可实盘，ann 低）。
  - `max_ann_largecap_long_widesl.json` — full ann 16.8/dd17.4/2025 +28.5（最高可达 ann，但 2024 -23、2023H1 依赖）。
  - `hybrid_v2_multi_sleeve.json` — 多 sleeve hybrid 参考样例。
- **完整证明报告**：`docs/superpowers/reports/2026-06-30-glm-phaseA-infeasibility-proof.md`（含 Phase A + A2 全部数据/机制）。
- **网络研究**：本轮做了两轮——马丁理论（grid O(n²) 损失、随机游走期望≈0、cycle-exit 必要、大盘币 pooling 系统性风险）+ 实战 regime allocator（多空按 regime 切换、delta-neutral 对冲、ADX 区分趋势/震荡）。结论与本轮数据一致。

## 8. 复现命令

```bash
WT=/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit
MAIN=/home/bumblebee/Project/grid_binance
BIN=$WT/target/release/portfolio_budget_replay   # 含 P4 cycle-exit，已编译
MD=$MAIN/data/market_data_full.db
FD=$MAIN/data/funding_rates.db
# 每币 regime allocator 搜索（用户方向）
python3 $WT/scripts/segment_first_largecap_search.py --profile balanced --budget 5000 \
  --count 300 --jobs 12 --screen-min -30 --top-full 50 \
  --pool broad --filters regime_pc --force-stop-bps 2500 \
  --out-dir /tmp/r3 --replay-bin $BIN --market-data $MD --funding-data $FD \
  --timeout 900 --segment-timeout 600
# 单 config 5 段复核
for SEG in "h1_2023:1672531200000:1688169599999" "h2_2023:1688169600000:1704067199999" \
  "2024:1704067200000:1735689599999" "2025:1735689600000:1767225599999" \
  "2026_ytd:1767225600000:1780271999999" "FULL:1672531200000:1780271999999"; do
  name=${SEG%%:*}; rest=${SEG#*:}; s=${rest%%:*}; e=${rest#*:}
  $BIN --config <cfg.json> --budget 5000 --profile balanced --start-ms $s --end-ms $e \
    --market-data $MD --funding-data $FD --exchange-min-notional 5 --equity-curve-points 16
done
```
报告 JSON（本地）：`/tmp/glm_phaseA_segfirst_run1/`（v1）、`/tmp/glm_phaseA_portstop_run2/`（v2）、`/tmp/glm_phaseA_regimepc_run3/`（v3）。

## 9. 安全状态与 CPU 注意

- 本轮结束已确认 0 残留 `portfolio_budget_replay`/`search_small_capital_martingale`/脚本进程。
- 30 核 WSL2。`portfolio_budget_replay` 单段（5 币+BTC dep，1 年，315 万 bar）≈ **3.6s**；full period ≈ 12s。
- 并发建议：`jobs 12-16`（20 会超订到 33-41 进程，效率下降）。长任务用 `nohup`。
- live-parity 允许清单（`budget_replay.rs:549`）：TP 仅 `Percent`，SL 仅 `StrategyDrawdownPct` 或 `RegimeBreakStop`（P4）。新机制须先补 live-parity 再进最终候选。
- **禁止**：live mode、Binance 操作、flyingkid 展示、真实资金烟测，直到 P5 离线全过 + 用户再次确认。

## 10. 给 ChatGPT 的明确建议

1. **不要重复 §5 的 10 条已证死路径**——它们是稳定的结构瓶颈，不是调参不够。
2. **主攻 §6.1（趋势 sleeve 引擎扩展）**——这是唯一可能把 ann 从 ~20% 推向 50%+ 的路。先做 backtest-only 最小验证（breakout 入场能否在 2024 趋势段显著盈利），再决定是否投入 live-parity TDD。
3. **并行试 §6.2（funding 套利）**——收益与方向无关，可能是稳定的增量 ann 源。
4. 若穷尽 §6.1-6.5 仍不达标，按 §6.6 向用户报告"目标需放宽到可达前沿"，不要伪造结果（交接 §9）。
5. 用户对"早下结论"敏感——每条结论必须有数据支撑（本文件 §3 的数字可直接引用）。
