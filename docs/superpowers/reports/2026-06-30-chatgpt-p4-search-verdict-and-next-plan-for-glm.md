# 2026-06-30 ChatGPT P4 Search Verdict and Next Plan for GLM

> 目标仍未完成：小资金可运行（本金/保证金预算 <= 5000U）、多币种、抗过拟合、各周期表现均衡、保守年化 >50% 且 DD <=10%、平衡年化 >90% 且 DD <=20%、激进年化 >110% 且 DD <=30%，并且所有信号/止盈止损/预算模型必须能在实盘复现。
>
> 本轮没有找到最终可上线组合。不要展示到 flyingkid，不要启动实盘，不要用本轮失败候选做 1000U/5000U 实盘。

## 1. 安全状态

- 本轮只做离线搜索和 `portfolio_budget_replay`，没有操作 Binance、live mode、flyingkid、数据库展示或实盘任务。
- 结束检查：`portfolio_budget_replay=0`，`search_small_capital_martingale=0`，`p4_row_combo_search/native_small_portfolio_search/original_margin_pack_v7=0`。
- 远端主要目录：
  - 主仓库：`/home/bumblebee/Project/grid_binance`
  - P4 worktree：`/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit`

## 2. 新增/保留脚本

保留这两个脚本，后续可复用：

- `/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit/scripts/run_p4_parallel_search.py`
  - 按 symbol/guard 并行跑 `search_small_capital_martingale`。
  - 注意必须用 `nohup` 后台跑，否则本地 SSH 会话超时会杀掉父调度器，只留下孤儿子进程。
- `/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit/scripts/p4_row_combo_search.py`
  - 读取 P4 单腿搜索 JSON，重建 live-parity 策略配置，缩放首单到组合预算，再用 `portfolio_budget_replay` 做真实预算回放和分段验证。

## 3. 本轮已证伪路径

### 3.1 v7 original-margin pack

产物：`/tmp/codex_origpack_v7`

结论：三档均无 full gate，通过分段验证的也为 0。

关键 frontier：

| 模式 | full/valid | full gate | segment pass | DD 约束内最高年化 | 年化约束内最低 DD |
|---|---:|---:|---:|---:|---:|
| Conservative | 319 / 307 | 0 | 0 | 30.35 / DD 8.90 | 55.70 / DD 21.17 |
| Balanced | 312 / 294 | 0 | 0 | 53.92 / DD 13.29 | 118.16 / DD 41.00 |
| Aggressive | 314 / 294 | 0 | 0 | 64.16 / DD 26.84 | 120.47 / DD 41.56 |

分段问题仍然明显：高收益组合大多 H1-2023 贡献过高，2024-2026 合计为负，segment DD 超标。

### 3.2 P4 focused full-period single-row search

产物：`/tmp/codex_p4_focus_v4`

范围：关键标的 `BTC, INJ, AAVE, DOGE, SOL, LINK, TRX, APT, COMP, DOT`，`default/no_atr_pause` guards，tiny grid，P4 live-parity 机制（`regime_break_stop`、`max_cycle_age_hours`）。

结果：20 个 report，共 2005 rows，passes C/B/A = 0/0/0。

关键 frontier：

- DD <= 10：最好 `BTCUSDT long_only trend_rsi`，年化 11.97 / DD 9.79。
- DD <= 20：最好 `SOLUSDT long_only trend_rsi`，年化 30.68 / DD 19.57。
- DD <= 30：最好 `SOLUSDT long_only trend_rsi`，年化 39.48 / DD 22.92。
- 年化 >= 50：最低 DD 是 `INJUSDT long_only`，年化 51.77 / DD 34.62。
- 年化 >= 90：最低 DD 是 `INJUSDT long_only`，年化 93.73 / DD 48.97。
- 年化 >= 110：无。

结论：P4 出场机制没有把单腿 frontier 推到可组合达标区。低 DD 候选收益太低，高收益候选仍集中在 INJ/SOL/BTC long，回撤过高。

### 3.3 P4 row combo budget replay

产物：`/tmp/codex_p4_combo_seq`

脚本：`scripts/p4_row_combo_search.py`

每档 260 个组合 full replay + 20 个分段验证，全部 budget blocked = 0，说明预算缩放没有截腿；失败是策略 frontier 问题，不是预算回放错误。

| 模式 | full | segment | passes | DD 约束内最高年化 | 最高年化 |
|---|---:|---:|---:|---:|---:|
| Conservative | 260 | 20 | 0 | 11.02 / DD 8.42 | 39.72 / DD 22.48 |
| Balanced | 260 | 20 | 0 | 45.88 / DD 17.74 | 81.31 / DD 41.27 |
| Aggressive | 260 | 20 | 0 | 74.31 / DD 25.78 | 90.13 / DD 43.45 |

分段代表问题：

- Conservative top segment：`33.09 / DD 17.08`，positive segments=3，H1 contribution=1.0，2024-2026 合计 -16.6%。
- Balanced top segment：`75.48 / DD 26.70`，positive segments=2，2024-2026 合计 -41.27%。
- Aggressive top segment：`84.77 / DD 34.50`，positive segments=2，max segment DD 51.03%，2024-2026 合计 -54.64%。

结论：静态组合无法靠分散化解决收益/回撤矛盾。组合收益一旦靠 INJ/SOL long 拉高，DD 和分段过拟合立刻失控；加入低 DD/short 腿会显著压低收益。

### 3.4 Direct native portfolio generator probe

产物：`/tmp/codex_native_combo_probe`

只做了 aggressive 小探针：60 full + 8 segment，passes=0。

最好结果：

- 年化 16.46 / DD 39.91
- 年化 14.36 / DD 64.10
- 年化 8.67 / DD 12.68

结论：当前 direct native generator 比 P4 row combo 更弱，不应继续按原样扩大。

## 4. 核心判断

当前 live-parity 机制下，已有搜索空间存在清晰 Pareto 瓶颈：

1. DD 合格的候选几乎没有足够收益。
2. 收益接近目标的候选高度依赖 INJ/SOL/BTC long，且 DD 大幅超标。
3. 高收益组合在分段上仍偏 2023H1，2024-2026 往往为负。
4. P4 `regime_break_stop + max_cycle_age_hours` 有助于控制部分极端持仓，但不足以同时满足收益、DD、小资金、分段均衡。
5. 继续排列 `/tmp/codex_p4_focus_v4` 或 `/tmp/codex_origpack_v7` 里的候选，预计不会突破；这两批池子的 frontier 已经被 budget replay 验证过。

## 5. 下一步必须换方向

### P1. 做 segment-first 单腿搜索，而不是 full-period top 再分段否决

搜索阶段就必须加入分段约束：

- H1-2023/H2-2023/2024/2025/2026_ytd 分段全部 replay。
- 初筛要求：
  - Conservative：full DD <= 13，ann >= 30，至少 4/5 段非负，2024-2026 合计 >= 0。
  - Balanced：full DD <= 24，ann >= 60，至少 4/5 段非负，2024-2026 合计 >= 0。
  - Aggressive：full DD <= 36，ann >= 80，至少 3/5 段非负，2024-2026 合计 >= 0。
- 只有通过上述软门槛的单腿才进入组合器。

不要再先按 full 年化排序，因为会反复选中 2023H1 票据。

### P2. 搜索“熊市也能活”的 entry regime

当前 short 在 2025 有收益，但全周期拖累；long 在全周期有收益，但 2025/山寨熊市拖累。下一轮要把方向切换条件做进 entry，而不是用 always-long 或 always-short。

优先尝试可实盘复现的表达式：

- long entry：
  - `close > ema(200)` AND `BTCUSDT.close > BTCUSDT.ema(200)`
  - `close > ema(100)` AND `rsi(14) < 65`
  - `BTCUSDT.close > BTCUSDT.ema(100)` AND `ETHUSDT.close > ETHUSDT.ema(100)`
- short entry：
  - `close < ema(200)` AND `BTCUSDT.close < BTCUSDT.ema(200)`
  - `close < ema(100)` AND `rsi(14) > 35`
  - 只在 symbol 自身弱势时 short，不再用 always-short。

注意：如果跨币种 `indicator_expression` 已在 P0/P4 中支持，必须用 `live_parity_check` 和 trading-engine 测试确认实盘也能计算同样依赖。

### P3. 如果 P2 仍失败，需要新增 live-parity 机制

当前 `new_cycle_drawdown_pause_pct` 只暂停新周期，不能平掉已有亏损 cycle；这对控制 DD 不够。

建议新增并 TDD：

1. Portfolio-level equity/drawdown stop：
   - 当组合 on-budget equity DD 超过阈值，flatten 所有 active cycles，并进入 cooldown。
   - backtest 和 trading-engine 必须一致，实盘必须 reduceOnly 平仓。
2. Regime allocator / sleeve enable：
   - 牛市只启用 long sleeve，熊市只启用 short sleeve，震荡启用 mean-reversion sleeve。
   - 不能靠人工切换，必须在配置和实盘中可复现。
3. Cycle trailing stop 或 profit-lock：
   - 当 cycle 曾经浮盈后回撤超过阈值，提前退出。
   - 必须实现 backtest/trading-engine/live parity。

如果不新增“会主动退出已有风险”的机制，仅靠暂停新单和静态权重，当前证据显示达不到目标。

### P4. 搜索执行方式

避免一口气跑大 full grid：

1. 先用小窗口/分段筛掉明显过拟合参数。
2. 对每个 symbol 保留：
   - low DD top N
   - ann/DD top N
   - 2024-2026 positive top N
   - 2025 positive top N
3. 再用 `portfolio_budget_replay` 做组合 full + segments。
4. 并发控制：
   - `portfolio_budget_replay` 建议 12-20 并发，不要三档各 18 并发同时跑，30 核机器会过载。
   - 长任务必须 `nohup`，否则 SSH 超时会杀父进程。

## 6. 可复核命令

检查无残留：

```bash
ps -C portfolio_budget_replay --no-headers | wc -l
ps -C search_small_capital_martingale --no-headers | wc -l
ps -ef | grep -E "p4_row_combo_search.py|native_small_portfolio_search.py|original_margin_pack_v7.py" | grep -v grep | wc -l
```

复核 P4 row combo 结论：

```bash
python3 - <<'PY'
import json
for profile in ["conservative","balanced","aggressive"]:
    p=f"/tmp/codex_p4_combo_seq/p4row_combo_{profile}_b5000_seed20260630.json"
    d=json.load(open(p))
    rows=[r for r in d["full_results"] if r["full"].get("ok")]
    print(profile, "full", len(rows), "seg", len(d["segment_validations"]), "passes", len(d["passes"]))
    for r in sorted(rows, key=lambda r: float(r["full"].get("ann") or -999), reverse=True)[:3]:
        f=r["full"]
        print(" ", r["idx"], round(float(f["ann"]),2), round(float(f["dd"]),2), r.get("meta",{}).get("symbols"))
PY
```

## 7. 当前结论

本轮不能交付最终三组合。当前最接近的只是“研究前沿”，不是可上线结果：

- Conservative：DD 合格时最高年化只有 11.02%，收益合格的组合不存在。
- Balanced：DD 合格时最高年化 45.88%，收益最高 81.31 但 DD 41.27。
- Aggressive：DD 合格时最高年化 74.31%，收益最高 90.13，仍低于 110 且 DD 43.45。

下一轮应停止在现有候选池上做静态组合排列，转为 segment-first + regime allocator/portfolio stop 这类实盘可复现机制探索。
