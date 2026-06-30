# 2026-06-30 GLM 执行交接：小资金、抗过拟合、实盘可复现的马丁组合探索

> 给 GLM：ChatGPT 已停止继续搜索。本文件是下一轮执行交接，不是最终结果报告。
>
> 当前结论：截至本交接，仍未找到满足用户最终目标的三档组合。不要把当前失败候选展示到 `flyingkid`，不要启动任何实盘，不要用本轮失败候选做 1000U/5000U 真实交易。

## 0. 用户最终目标（必须原样坚持）

必须找到三条最终马丁组合，且全部满足：

1. 小资金可运行：本金/保证金预算必须低于 5000 USDT。这里的预算是保证金本金，不是杠杆后的名义仓位。
2. 多币种组合：不再严格限制必须 8 个币种，可以根据预算动态调整币种数量，但最终组合不能退化成单币种押注。
3. 抗过拟合：不能依赖 2023H1 单段极端收益；各时间段表现必须相对均衡。
4. 各周期表现均衡：full period 达标之外，H1-2023、H2-2023、2024、2025、2026_ytd 等分段必须通过验证。
5. 收益门槛：
   - 保守：年化收益率 > 50%，最大回撤按保守模式控制，目标 DD <= 10%。
   - 平衡：年化收益率 > 90%，最大回撤按平衡模式控制，目标 DD <= 20%。
   - 激进：年化收益率 > 110%，最大回撤按激进模式控制，目标 DD <= 30%。
6. 实盘可真实运行：所有指标计算、跨币种信号、止盈止损、风控、价格统计、手续费、资金费率、预算、下单数量、逐仓/杠杆配置，都必须能在 trading-engine/live 中与回测一致复现。
7. 找到最终三组合以后，才允许展示到 `flyingkid`，并归档其他组合；正式实盘启动前必须再次向用户确认。

## 1. 当前安全状态

本轮 ChatGPT 只做了离线 replay/search，没有操作 Binance、live mode、`flyingkid`、数据库展示或真实资金。

结束时已检查无残留：

```bash
ps -C portfolio_budget_replay --no-headers | wc -l
ps -C search_small_capital_martingale --no-headers | wc -l
ps -ef | grep -E "p4_row_combo_search.py|native_small_portfolio_search.py|original_margin_pack_v7.py" | grep -v grep | wc -l
```

当时结果均为 0。GLM 继续前必须再次执行上述命令确认无残留。

远端路径：

- 主仓库：`/home/bumblebee/Project/grid_binance`
- P4 worktree：`/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit`
- 已有阶段性报告：`/home/bumblebee/Project/grid_binance/docs/superpowers/reports/2026-06-30-chatgpt-p4-search-verdict-and-next-plan-for-glm.md`

## 2. 本轮执行过程

### 2.1 v7 original-margin pack 搜索

产物：`/tmp/codex_origpack_v7`

目标：在修正后的保证金本金模型下，继续尝试原始候选池/组合池是否能满足小资金和抗过拟合要求。

结果：三档均没有通过 full gate，也没有通过 segment gate。

关键 frontier：

| 模式 | full/valid | full gate | segment pass | DD 约束内最高年化 | 年化约束内最低 DD |
|---|---:|---:|---:|---:|---:|
| Conservative | 319 / 307 | 0 | 0 | 30.35 / DD 8.90 | 55.70 / DD 21.17 |
| Balanced | 312 / 294 | 0 | 0 | 53.92 / DD 13.29 | 118.16 / DD 41.00 |
| Aggressive | 314 / 294 | 0 | 0 | 64.16 / DD 26.84 | 120.47 / DD 41.56 |

结论：高收益组合仍然明显依赖 2023H1，2024-2026 表现差；如果强行追求收益，DD 会明显超标。

### 2.2 P4 focused full-period 单腿搜索

脚本：

- `/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit/scripts/run_p4_parallel_search.py`

产物：`/tmp/codex_p4_focus_v4`

搜索范围：

- symbol：`BTC, INJ, AAVE, DOGE, SOL, LINK, TRX, APT, COMP, DOT`
- guards：`default`、`no_atr_pause`
- tiny grid
- P4 live-parity 机制：`regime_break_stop`、`max_cycle_age_hours`

结果：20 个 report，共 2005 rows，passes C/B/A = 0/0/0。

关键 frontier：

- DD <= 10：最好 `BTCUSDT long_only trend_rsi`，年化 11.97 / DD 9.79。
- DD <= 20：最好 `SOLUSDT long_only trend_rsi`，年化 30.68 / DD 19.57。
- DD <= 30：最好 `SOLUSDT long_only trend_rsi`，年化 39.48 / DD 22.92。
- 年化 >= 50：最低 DD 是 `INJUSDT long_only`，年化 51.77 / DD 34.62。
- 年化 >= 90：最低 DD 是 `INJUSDT long_only`，年化 93.73 / DD 48.97。
- 年化 >= 110：无。

结论：P4 的出场机制没有把单腿收益/回撤 frontier 推到目标区域。低回撤候选收益太低，高收益候选回撤过高。

### 2.3 P4 row combo budget replay

脚本：

- `/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit/scripts/p4_row_combo_search.py`

产物：`/tmp/codex_p4_combo_seq`

目标：把 P4 单腿结果组合成多币种组合，并用真实 `portfolio_budget_replay` 验证预算、缩放、曲线、分段。

结果：

| 模式 | full | segment | passes | DD 约束内最高年化 | 最高年化 |
|---|---:|---:|---:|---:|---:|
| Conservative | 260 | 20 | 0 | 11.02 / DD 8.42 | 39.72 / DD 22.48 |
| Balanced | 260 | 20 | 0 | 45.88 / DD 17.74 | 81.31 / DD 41.27 |
| Aggressive | 260 | 20 | 0 | 74.31 / DD 25.78 | 90.13 / DD 43.45 |

关键事实：

- Top rows 的 `budget_blocked = 0`。
- 失败不是预算太小导致截腿，也不是保证金预算模型把交易挡掉。
- 失败原因是策略本身的收益/回撤/分段稳定性不够。

分段代表问题：

- Conservative top segment：年化 33.09 / DD 17.08，positive segments=3，H1 contribution=1.0，2024-2026 合计 -16.6%。
- Balanced top segment：年化 75.48 / DD 26.70，positive segments=2，2024-2026 合计 -41.27%。
- Aggressive top segment：年化 84.77 / DD 34.50，positive segments=2，max segment DD 51.03%，2024-2026 合计 -54.64%。

结论：静态多币种组合无法解决当前候选池的结构性问题。高收益来自少数强趋势 long 候选，加入低 DD 或 short 腿后收益被显著稀释。

### 2.4 Direct native portfolio generator 探针

产物：`/tmp/codex_native_combo_probe`

只做 aggressive 小探针：60 full + 8 segment，passes=0。

最好结果：

- 年化 16.46 / DD 39.91
- 年化 14.36 / DD 64.10
- 年化 8.67 / DD 12.68

结论：当前 direct native generator 比 P4 row combo 更弱，不建议按原样扩大。

## 3. 遇到的核心问题

### 3.1 不是“资金缩放”本身导致失败

用户提出得很对：如果模型正确，1000U、5000U、100000U 理论上应主要是仓位大小缩放，收益率和回撤率不应大幅变形。

本轮 P4 row combo 中，top rows 的 `budget_blocked = 0`，说明在 5000U 保证金本金预算下没有因为预算不足截腿。因此当前失败不是预算缩放错误，而是候选策略质量不足。

但是后续仍必须验证更小预算下的交易所约束：

- Binance 最小下单数量和最小名义价值。
- 数量 step size / tick size 舍入。
- 多币种分配后单币种首单是否过小。
- 每腿保证金、手续费、资金费率是否与回测完全一致。

也就是说，预算不是这次 frontier 失败的主因，但最终上线前仍必须做预算矩阵验证。

### 3.2 收益/回撤 Pareto 瓶颈明显

当前搜索池里存在很清楚的 tradeoff：

- DD 达标的候选，年化远低于目标。
- 年化接近目标的候选，DD 明显超标。
- 组合后可以平滑一部分曲线，但无法把 40%+ DD 的高收益腿变成 10%-20% DD 的稳健组合，同时还保持目标收益。

这说明不能继续只做静态组合排列。候选池本身需要更强的 regime/exit/risk 机制。

### 3.3 过拟合集中在 2023H1

此前很多看起来优秀的组合，本质上依赖 2023H1 的强趋势行情。分段验证一旦加入，2024-2026 往往转负。

下一轮必须把 segment validation 放进搜索阶段，而不是 full-period top 选完后再做否决。否则会反复选中同一类 2023H1 票据。

### 3.4 当前 live-parity 风控不足

已有机制如：

- `new_cycle_drawdown_pause_pct`
- `new_cycle_atr_pause_pct`
- `safety_skip_adx_threshold`
- `regime_break_stop`
- `max_cycle_age_hours`

可以减少部分坏周期，但不足以解决“已有 active cycle 扩大亏损”的组合级 DD 问题。特别是 `new_cycle_drawdown_pause_pct` 主要暂停新周期，不能主动平掉已有风险。

若要达到用户目标，很可能需要新增实盘可复现的机制：

- portfolio-level equity/drawdown stop：组合权益回撤达到阈值时，reduceOnly 平掉 active cycles，并进入 cooldown。
- dynamic regime allocator：牛市启用 long sleeve，熊市启用 short sleeve，震荡启用 mean-reversion sleeve。
- cycle trailing/profit-lock：周期曾经浮盈后，如果回吐超过阈值，则提前退出。

这些机制必须先 TDD，确保 backtest-engine 和 trading-engine 完全一致。

### 3.5 CPU 利用率观察问题

WSL 里机器是 30 核。之前用户看到 Windows 侧 CPU 只有约 10%，但 WSL `ps` 里多个 backtest/replay 进程可以接近 100%，load 到 25-31。

后续执行时不要只看 Windows 总 CPU。应同时看：

```bash
nproc
uptime
ps -eo pid,pcpu,pmem,cmd --sort=-pcpu | head -40
```

并发建议：

- `portfolio_budget_replay` 控制在 12-20 并发。
- 不要三档各 18 并发同时跑，30 核机器会过载且 IO/调度效率下降。
- 长任务必须用 `nohup`，否则 SSH 超时会杀掉父调度器，留下孤儿子进程。

## 4. 已证伪路径：不要重复浪费时间

GLM 下一轮不要再把主要时间花在以下路径：

1. 继续排列 `/tmp/codex_p4_focus_v4` 的 P4 单腿结果。
2. 继续排列 `/tmp/codex_origpack_v7` 的 original-margin pack。
3. 按 full-period 年化排序再分段验证。
4. 只靠降低币种数做静态组合。
5. 只靠 `new_cycle_drawdown_pause_pct`、`regime_break_stop`、`max_cycle_age_hours` 这几个已有参数扩大网格搜索。

这些方向已经显示出稳定瓶颈。

## 5. 下一轮总体方向

下一轮必须换成：

1. segment-first 单腿搜索。
2. 动态币种数量和预算分配。
3. 市场状态驱动的方向选择，而不是 always-long/always-short。
4. 可实盘复现的组合级风控和主动退出机制。
5. full period + segment + budget matrix + live parity 一体化验证。

## 6. GLM 下一步执行计划

### P0. 安全冻结

继续前先确认：

```bash
cd /home/bumblebee/Project/grid_binance
ps -C portfolio_budget_replay --no-headers | wc -l
ps -C search_small_capital_martingale --no-headers | wc -l
ps -ef | grep -E "p4_row_combo_search.py|native_small_portfolio_search.py|original_margin_pack_v7.py" | grep -v grep | wc -l
```

要求：

- 不启动 live mode。
- 不操作 Binance。
- 不写入 flyingkid 展示。
- 不做真实资金烟测。

直到最终候选离线验证通过为止。

### P1. 建立 segment-first 搜索框架

搜索循环不能只输出 full-period 结果。每个候选必须同步验证：

- Full：`2023-01-01` 到 `2026-05-31` 或当前统一回测范围。
- Segment A：`2023-01-01` 到 `2023-06-30`
- Segment B：`2023-07-01` 到 `2023-12-31`
- Segment C：`2024-01-01` 到 `2024-12-31`
- Segment D：`2025-01-01` 到 `2025-12-31`
- Segment E：`2026-01-01` 到 `2026-05-31`

初筛软门槛建议：

| 模式 | full ann | full DD | segment 要求 |
|---|---:|---:|---|
| Conservative | >= 30 | <= 13 | 至少 4/5 段非负，2024-2026 合计 >= 0，max segment DD <= 18 |
| Balanced | >= 60 | <= 24 | 至少 4/5 段非负，2024-2026 合计 >= 0，max segment DD <= 30 |
| Aggressive | >= 80 | <= 36 | 至少 3/5 段非负，2024-2026 合计 >= 0，max segment DD <= 42 |

这些是进入组合器前的候选池软门槛，不是最终验收门槛。最终仍按用户目标验收。

### P2. 搜索实盘可复现的 regime entry

优先把方向切换写进 entry/filter，而不是固定 long 或固定 short。

候选表达式方向：

Long sleeve：

- `symbol.close > symbol.ema(200)` AND `BTCUSDT.close > BTCUSDT.ema(200)`
- `symbol.close > symbol.ema(100)` AND `BTCUSDT.close > BTCUSDT.ema(100)` AND `rsi(14) < 65`
- `symbol.close > symbol.ema(100)` AND `ETHUSDT.close > ETHUSDT.ema(100)`

Short sleeve：

- `symbol.close < symbol.ema(200)` AND `BTCUSDT.close < BTCUSDT.ema(200)`
- `symbol.close < symbol.ema(100)` AND `BTCUSDT.close < BTCUSDT.ema(100)` AND `rsi(14) > 35`
- 只在 symbol 自身弱势且 BTC/ETH 市场弱势时 short，禁止 always-short 盲目常驻。

Range/mean-reversion sleeve：

- 只在 ATR/ADX 较低、趋势过滤不强时启用。
- 必须限制补仓腿数、周期年龄和组合 DD。

注意：如果使用跨币种指标，必须确认回测和实盘都能按同一根 K 线、同一闭合时点计算，不能引入未来函数或实盘不可取数据。

### P3. 如 P2 仍无法达标，新增 live-parity 风控机制

不要直接上更复杂搜索。先补齐可实盘复现机制并写测试。

建议按以下顺序实现：

1. Portfolio-level drawdown stop
   - 回测：组合权益从峰值回撤超过阈值，平掉所有 active cycles，进入 cooldown。
   - 实盘：trading-engine 使用 reduceOnly 市价/限价保护平仓，取消冲突挂单，记录 flatten reason。
   - 指标：手续费、资金费率、实现盈亏、未实现盈亏必须进入曲线。

2. Cycle trailing/profit-lock
   - 当单 cycle 曾经达到浮盈阈值后，回吐超过阈值则退出。
   - 防止“盈利周期变亏损周期”扩大 DD。

3. Regime allocator / sleeve enable
   - 组合内配置 long sleeve、short sleeve、range sleeve。
   - 由可计算指标自动启停，不靠人工切换。
   - 回测和实盘使用同一配置结构。

必须补的测试：

- backtest-engine 单测：触发 portfolio stop 后不再开新腿，equity/DD 计算正确。
- trading-engine 单测：触发 stop 时取消挂单、reduceOnly 平仓、不会重复开单。
- parity 测试：同一段 K 线回测和 runtime 决策一致。
- budget 测试：保证金预算使用 `notional / leverage`，不是名义仓位。

### P4. 动态币种数量与预算搜索

用户允许不严格限制 8 币种。下一轮应按预算动态搜索：

| 总保证金预算 | 候选币种数 |
|---:|---|
| <= 1000U | 2-4 个币种 |
| 1000-3000U | 3-6 个币种 |
| 3000-5000U | 4-8 个币种 |

关键要求：

- 多币种，但不要为了凑数量加入负贡献腿。
- 每个 symbol 必须满足 Binance min notional / qty step / tick size。
- 组合器要同时搜索 symbol count、weight、per-symbol budget floor。
- `budget_blocked` 必须为 0 或可解释；如果被预算截腿，该结果不能作为最终结果。

预算矩阵建议至少验证：

- 1000U
- 2000U
- 3000U
- 5000U

如果某组合只在 5000U 可运行，必须写明最小启动本金，不能假装 1000U 可运行。

### P5. 组合器验收

最终候选必须全部满足：

Full-period：

- Conservative：ann > 50，DD <= 10。
- Balanced：ann > 90，DD <= 20。
- Aggressive：ann > 110，DD <= 30。

Segment：

- 不能只有 2023H1 赚钱。
- 2024-2026 合计必须为正或接近正且原因可解释。
- Conservative 至少 4/5 段非负；Balanced 至少 4/5 段非负；Aggressive 至少 3/5 段非负。
- 任一单段 DD 不能远超对应档位，例如 Conservative 不应出现 20%+ segment DD。

预算：

- 总保证金本金 <= 5000U。
- 输出 `planned_margin_quote`、`planned_notional_quote`、`min_start_capital_quote`。
- 明确首单名义金额、杠杆、每腿保证金、最大腿数。

实盘复现：

- 所有 signal/filter/exit/stop/trailing/regime 都必须在 trading-engine 中有实现。
- 回测与实盘使用同一个参数含义和同一个资金模型。
- 不能使用 research-only 脚本里才有的逻辑作为最终策略。

### P6. 找到最终候选后的流程

只有在 P5 通过后才执行：

1. 写最终结果报告，列出三档组合完整参数、full + segment + budget matrix 结果。
2. 将三条组合展示到 `flyingkid` 账户。
3. 归档其他组合，只保留最终三条。
4. 写实盘 runbook：
   - 预配置逐仓/杠杆。
   - 启动前检查已有持仓和挂单。
   - 若已有冲突挂单/错误成交，先取消挂单、清仓、清理旧运行数据。
   - 小额 smoke 测试计划。
   - 1000U 或用户确认的正式本金启动计划。
   - 监控指标和错误处理流程。
5. 正式实盘启动前必须再次向用户确认。没有用户确认，不允许启动真实资金。

## 7. 后续 smoke/live 验证思路

离线候选通过后，才进入 smoke。

Smoke 资金说明：

- 用户之前强调 50U/1000U 指的是保证金本金，不是杠杆后名义仓位。
- 若杠杆 5x，50U 保证金可对应约 250U 名义仓位，但仍必须受交易所最小下单限制、手续费、资金费率和安全 buffer 限制。

Smoke 必须覆盖：

1. 指标计算：所有 entry/filter/regime/trailing 在实盘数据流中可计算。
2. 下单：首单、补仓腿、TP、SL、reduceOnly 平仓。
3. 预算：保证金本金 cap 生效，不按名义仓位错误阻挡或错误放大。
4. 恢复：已有持仓/挂单时不会重复开单。
5. 统计：交易记录、持仓、手续费、资金费率、实现盈亏、未实现盈亏准确。
6. 网格兼容：普通网格交易路径不被马丁改造破坏。

错误处理：

- 下单失败：停止该执行策略，记录原因，不重试无限开单。
- 指标不可计算：禁止开仓。
- 持仓和状态不一致：进入 `NeedsManualReview`，不自动加仓。
- 触发组合 DD stop：取消挂单、reduceOnly 平仓、cooldown。
- Binance 返回异常：记录原始响应，停止自动交易，等待人工确认。

## 8. 复核命令

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

观察 CPU：

```bash
nproc
uptime
ps -eo pid,pcpu,pmem,cmd --sort=-pcpu | head -40
```

## 9. 给 GLM 的明确结论

当前任务不是“再多排列几组旧候选”，而是要补出真正能跨周期工作的 live-parity 策略机制。

最可能的突破方向：

1. segment-first 搜索，避免 2023H1 过拟合候选进入组合器。
2. regime-aware direction switching，让 long/short/range sleeve 根据市场状态自动启停。
3. portfolio-level drawdown stop，主动处理已有风险，而不是只暂停新周期。
4. cycle trailing/profit-lock，减少盈利周期转亏损。
5. 动态币种数量和预算分配，在 <=5000U 保证金本金下找到可运行规模。

如果这些机制仍无法达到三档门槛，GLM 必须明确报告原因：在当前交易周期、币种池、马丁结构、费用/资金费率/交易所约束下，目标收益和目标 DD 可能不可同时满足，不能通过放宽验证或忽略分段来伪造结果。

