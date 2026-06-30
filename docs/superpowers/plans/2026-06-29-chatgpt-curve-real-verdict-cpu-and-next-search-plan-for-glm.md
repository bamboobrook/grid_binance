# 2026-06-29 ChatGPT 复核：CPU 占用、curve-frontier 真实 replay 结论与 GLM 下一轮搜索计划

> 范围：本文只用于离线回测、搜索、验证和后续任务安排。不得触碰 Binance、实盘挂单/持仓、trading-engine live mode 或 flyingkid 展示，直到真实 replay + 分段验证找到最终三档候选。

## 0. 当前目标

在本金预算 `<= 5000 USDT`、多币种但实际交易币种数量受预算控制、抗过拟合、各周期表现均衡、且实盘可复现的前提下，寻找三档马丁组合：

| 模式 | 年化目标 | 最大回撤目标 | 额外要求 |
|---|---:|---:|---|
| Conservative | `> 50%` | `<= 10%` | 低回撤优先，不能依赖单一窗口 |
| Balanced | `> 90%` | `<= 20%` | 2024-2026 不能整体亏损 |
| Aggressive | `> 110%` | `<= 30%` | 不能只是 2023H1 过拟合 |

资金口径继续使用已修正的保证金本金：`first_order_quote` 是名义开仓额，单腿保证金约等于 `notional / leverage`，组合 budget 是保证金本金上限，不是杠杆后名义价值。

## 1. CPU 低占用问题：结论与后续执行标准

用户反馈看到 CPU 只有约 `10%`。这件事需要分两种状态判断：

1. 如果没有活跃 `portfolio_budget_replay` 进程，那么低 CPU 是正常的，说明任务已经结束或调度没启动。
2. 如果声称正在跑大规模 replay，却没有看到大量 `portfolio_budget_replay` 进程接近 `100%` 单核 CPU，则是不合格的调度。

本轮我实际核实到：

- WSL 核数：`nproc = 30`
- v2 replay 运行中负载示例：`load average: 22.96, 16.91, 8.67`、`24.72, 19.78, 11.01`、`24.39, 23.45, 16.47`
- 运行中 `portfolio_budget_replay` 进程数：`20`
- 典型进程 CPU：每个 replay 子进程约 `101%-102%`
- v2 完成后：`portfolio_budget_replay` 进程数回到 `0`

所以本轮 v2 并不是 CPU 只用 10%；它按 `--jobs 20` 使用了约 20 个核心。后续正式大搜索应把并发提高到 `26-28`，给系统/SQLite/SSH 留 2-4 核。

后续 GLM 每轮重型回测必须在日志或报告中贴下面证据，不允许只写“正在跑”：

```bash
nproc
uptime
ps -C portfolio_budget_replay --no-headers | wc -l
ps -C portfolio_budget_replay -o pid,ppid,pcpu,pmem,etime,args --sort=-pcpu | head -30
```

执行标准：

- 全周期 replay 队列未进入尾段时，活跃 `portfolio_budget_replay` 应稳定 `>= 20`，推荐 `26-28`。
- 如果队列仍有大量任务而 replay 进程长期 `< 8`，先停止分析结果，修并发调度。
- 长任务必须用 `nohup ... > run.log 2>&1 &`，避免 SSH stdout pipe 阻塞父进程。
- 每轮日志必须有 `DONE x/y`、分段验证必须有 `SEG x/y`。

## 2. 本轮新增验证：curve frontier 转真实 portfolio_budget_replay

### 2.1 背景

此前 `/tmp/codex_small_search/parallel_curve_frontier_sampled_v1.json` 里有一些 curve-only 结果看起来很诱人，例如 Conservative curve-only strict 显示约 `ann 52.69 / DD 8.58`，并且分段也看起来不错。但它只是把候选 equity curve 拼接，不是真实预算 replay。

我新增脚本把这些 curve frontier 候选转换为真实 portfolio config，并强制走 `portfolio_budget_replay`：

- 脚本：`/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit/scripts/replay_curve_frontier_real.py`
- 输入：`/tmp/codex_small_search/parallel_curve_frontier_sampled_v1.json`
- 候选池：`/tmp/codex_small_search/full_period_candidates.csv.gz`
- 输出目录：`/tmp/codex_curve_real_v2`
- 最终报告：`/tmp/codex_curve_real_v2/report.json`
- 运行日志：`/tmp/codex_curve_real_v2/run.log`

本脚本只做离线研究，不触碰 DB live 状态、Binance、挂单或实盘。

启动命令：

```bash
cd /home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit
mkdir -p /tmp/codex_curve_real_v2
nohup python3 scripts/replay_curve_frontier_real.py \
  --profiles conservative balanced aggressive \
  --budgets 5000 \
  --top-per-bucket 8 \
  --jobs 20 \
  --top-segment 20 \
  --out-dir /tmp/codex_curve_real_v2 \
  --replay-bin target/release/portfolio_budget_replay \
  --market-data /home/bumblebee/Project/grid_binance/data/market_data_full.db \
  --funding-data /home/bumblebee/Project/grid_binance/data/funding_rates.db \
  > /tmp/codex_curve_real_v2/run.log 2>&1 &
```

### 2.2 最终结果

最终完成状态：

- full replay：`385/385`
- segment validation：`60/60`
- passes：`0`
- replay 完成后进程数：`0`
- report finished_at：`1782735258.2217205`

按模式统计：

| 模式 | full rows | DD 门槛内最高年化 | 年化达标时最低 DD | 最佳年化 | pass |
|---|---:|---:|---:|---:|---:|
| Conservative | 102 | `6.99% / DD 9.89%` | `65.07% / DD 44.02%` | `78.90% / DD 58.62%` | 0 |
| Balanced | 138 | `16.18% / DD 16.29%` | 无 `ann >= 90` | `55.47% / DD 32.80%` | 0 |
| Aggressive | 145 | `27.23% / DD 15.61%` | `110.07% / DD 42.41%` | `110.07% / DD 42.41%` | 0 |

分段验证结论：

- Top 60 候选全部 segment pass 失败。
- Aggressive 最高年化候选 `110.07 / DD 42.41`，full DD 已经超 `30`，分段也不能接受。
- 多个 aggressive 候选年化在 `99-105` 附近，但 DD 在 `38-45`，同样不可用。
- Balanced 最高年化只有 `55.47 / DD 32.80`，离 `90 / 20` 很远。
- Conservative 在 DD10 内最高年化仅 `6.99`，说明 curve-only 的 `52 / DD8` 转真实预算后完全失效。

### 2.3 判定

这批 curve frontier 不能直接组合出符合目标的三档策略。原因不是 CPU 没跑，也不是少跑几条，而是 curve-only 拼接在真实预算约束下失真：

- 转真实 portfolio 后必须满足 Binance `minNotional`、保证金 cap、`budget_blocked_legs = 0`、手续费/滑点/资金费率、逐腿下单和止盈止损路径。
- 缩放 `first_order_quote` 后，马丁阶梯的补仓频率、TP 触发、SL 触发、资金费率暴露时长都变了，收益/回撤不能线性缩放。
- 小资金场景还有离散约束：5U 最小名义开仓、多腿几何级数、币种数量、同时开 cycle 数，这些都会导致 `1000` 和 `5000` 不是简单等比例关系。

因此，后续不要再把 curve-only 结果当作候选成功，只能当作 prefilter。

## 3. 结合现有结果池的总体结论

此前现有真实 replay 池扫描结果已经显示：

- 扫描 JSON：`34197`
- 识别结果行：`7351`
- 满足 `budget <= 5000`、`budget_blocked_legs = 0`、`max_capital_used_quote <= budget` 的真实结果：`6409`

结果池结论：

| 模式 | 结果池 pass | 关键前沿 |
|---|---:|---|
| Conservative `50/DD10` | 0 | `DD<=10` 下最高约 `17.72%`；`ann>50` 下最低 DD 约 `14.21%` |
| Balanced `90/DD20` | 0 | `DD<=20` 下最高约 `67.25%`；`ann>90` 下最低 DD 约 `26.08%` |
| Aggressive `110/DD30` | 表面有 | 需分段验证；已有证据显示很多是 2023H1 过拟合 |

叠加本轮 curve-real v2 后，可以判定：现有结果池和 curve frontier 暂时不能直接拼出三档合格组合。下一轮必须换搜索目标与搜索结构。

## 4. 外部方法论对下一轮的约束

这次外部检索支持以下约束，不能继续“全周期反复调参挑最好”：

- Bailey 等的 PBO/CSCV 框架指出，在投资回测中反复从同一历史样本挑最优策略，很容易产生 backtest overfitting，需要组合式交叉验证。参考：https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2326253
- Deflated Sharpe Ratio 强调要修正多次试验选择偏差和非正态收益，否则最优回测很可能只是筛选膨胀。参考：https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2460551
- White Reality Check 明确讨论 data snooping：同一数据反复用于模型选择时，“好结果”可能只是偶然。参考：https://www.ssc.wisc.edu/~bhansen/718/White2000.pdf
- Moreira/Muir 的 volatility-managed portfolios 说明“高波动时降低风险暴露”有经验依据，但这不是简单暂停开仓，必须作为可实盘复现的风险缩放/暴露管理来实现。参考：https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2773438
- Binance futures grid 风险说明也提醒：杠杆会放大亏损，价格离开网格且不恢复时存在清算/亏损风险。参考：https://www.binance.com/en-IN/support/faq/detail/f4c453bab89648beb722aa26634120c3

落地含义：下一轮必须 `segment-first`、记录 trial count、做预算矩阵、做 live parity，不得报告 curve-only 或 research-only env 结果为成功。

## 5. 下一轮搜索路线：从结果拟合改为 segment-first + live-parity 原生搜索

### P0：安全边界

- 不触碰 Binance。
- 不启动 trading-engine live mode。
- 不运行 50U/1000U 烟测。
- 不修改 flyingkid 展示。
- 只做离线回测、搜索、验证、文档。

### P1：先保留并复核 P4 能力

P4 worktree 已实现：

- `max_cycle_age_hours`
- `regime_break_stop`
- backtest/trading-engine/live_parity_check 对齐

这两个机制方向仍然是必要的，因为 GLM 已证明单纯暂停新 cycle 无法处理已有深套仓位。下一步要继续用 P4，但必须用真实 replay 和分段验证证明是否突破。

建议先跑 P4 交接文档里的 2025 short/regime 验证，但要提高并发并记录 CPU：

```bash
cd /home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit
PATH=$HOME/.cargo/bin:$PATH cargo build -p backtest-engine --bin search_small_capital_martingale --bin portfolio_budget_replay --release

nohup ./target/release/search_small_capital_martingale \
  --budgets 3000,5000 \
  --symbols BCHUSDT,DOTUSDT,APTUSDT,ETCUSDT,NEARUSDT,COMPUSDT,GALAUSDT,ICPUSDT,BTCUSDT,ETHUSDT,SOLUSDT,ADAUSDT,TRXUSDT \
  --direction-modes short_only,long_and_short,long_only \
  --entry-filters rsi_moderate,bb_moderate,trend_rsi,btc_trend_rsi,none \
  --regime-break ema50,ema100,none \
  --max-cycle-age 24,48,96,168,none \
  --start-ms 1672531200000 --end-ms 1780271999999 \
  --market-data /home/bumblebee/Project/grid_binance/data/market_data_full.db \
  --funding-data /home/bumblebee/Project/grid_binance/data/funding_rates.db \
  --output /tmp/p4_fullcycle_native_search_20260629.json \
  --top-n 200 --grid small --max-params-per-symbol-budget 80 \
  > /tmp/p4_fullcycle_native_search_20260629.log 2>&1 &
```

如果该 Rust 搜索本身不能多核，必须改调度层，把不同 symbol/budget/profile 拆成多进程并行，不能让 30 核机器只用 1 核。

### P2：改造 native search，使其按预算动态控制币种数量

用户提出“根据本金动态调整组合中币种数量”，这是正确方向，但要按可执行约束实现：

| 预算 | 建议实际交易币种数 | 原因 |
|---:|---:|---|
| `<= 1000` | 2-3 | 5U minNotional + 多腿保证金会让太多币种被迫截断 |
| `1500-3000` | 3-5 | 可以做核心+卫星，但不能平均摊太薄 |
| `3000-5000` | 4-6 | 才有足够空间分散同时保留完整阶梯 |

不要固定 8 币种。币种数量应是搜索维度，并且每个组合必须输出：

- `strategy_count`
- 实际交易 symbols
- 每个 symbol 的 planned_margin
- `max_capital_used_quote`
- `budget_blocked_legs`
- 是否因 minNotional 被迫裁腿

### P3：单策略先做 segment-first 筛选，再组合

不要继续从全周期最优组合局部变异。先建立“跨周期能活”的单策略池。

单策略筛选门槛建议：

- Full 结果无需一开始达到最终年化，但不能依赖单一窗口。
- 至少 `3/5` 段非负；Conservative 候选优先 `4/5` 段非负。
- `2024-2026 combined return >= 0`，否则大概率是 2023H1 票据。
- 每段 DD 不能远超对应 profile：Conservative 单段 DD 尽量 `<= 15`，Balanced `<= 30`，Aggressive `<= 45`。
- `h1_contribution_ratio`：Conservative `<= 0.35`，Balanced `<= 0.45`，Aggressive `<= 0.55`。
- `budget_blocked_legs = 0`，`principal_breached = false`，`max_capital_used_quote <= budget`。

组合时再按相关性、窗口互补、资金占用和交易方向做选择，而不是按 full ann 排名。

### P4：搜索参数必须 live-parity

允许进入最终候选的机制：

- Take profit：`percent`。
- Stop loss：`strategy_drawdown_pct` 或 P4 的 `regime_break_stop`。
- Risk limit：P4 `max_cycle_age_hours`。
- Entry filter：当前 backtest/trading-engine 共用 indicator runtime 支持的表达式。
- Spacing：优先 `fixed_percent`；如使用 ATR spacing，必须确认 trading-engine 同路径支持且实盘预热逻辑一致。
- Cross-symbol 指标：只允许在实盘 K 线依赖能被明确加载并 live_parity_check 通过时使用。

不得进入最终候选的机制：

- 只通过 research-only env 生效的阈值。
- 回测有但 trading-engine 没实现的 trailing TP、ATR TP、partial exit、动态调仓。
- curve-only 组合。
- 依赖人工平仓或事后筛选的规则。

如果下一轮发现必须使用新机制才能突破，例如 volatility-managed exposure、partial cycle exit、drawdown-triggered de-risking，则必须先做代码 parity：backtest-engine、trading-engine、live_parity_check、发布预检、测试全部补齐，再重新回测。

### P5：优先搜索的结构

#### 结构 A：稳健多币种低倍率软马丁

目的：突破 Conservative。

参数方向：

- 2-6 symbols，预算动态决定数量。
- multiplier `1.15-1.60`。
- max_legs `3-6`。
- step_bps `60-180`。
- tp_bps `20-70`。
- SL：`strategy_drawdown_pct 250-800 bps` 或 `regime_break_stop`。
- max_cycle_age：`24/48/96/168h`。
- entry：per-symbol trend/RSI/BB/ADX + BTC regime，但必须分段验证。

#### 结构 B：核心+卫星，但核心不固定 INJ

GLM 的核心+卫星让 Conservative DD 从约 18 降到约 10.7，这是有价值的方向，但 INJ/H1 依赖太强。

下一轮核心候选要从 robust pool 里选，不固定 INJ：

- BTC/ETH/SOL/BNB/TRX/ADA/LTC 等低尾部风险 long。
- DOT/APT/COMP/NEAR/GALA/ETC/BCH/ICP 等只在 regime 条件满足时 short。
- 卫星权重不是平均摊薄，而是按段贡献、max segment DD、资金占用排序。

#### 结构 C：趋势/波动 regime 下的方向切换

目的：解决 2025 山寨熊市和 2023 牛市的方向冲突。

可用规则必须是 live parity：

- long only when `symbol.close > symbol.ema(N)` and BTC not in down regime。
- short only when `symbol.close < symbol.ema(N)` or BTC/symbol down regime。
- `regime_break_stop` 用于已有 cycle 退出，而不是只暂停新 cycle。

#### 结构 D：如需 volatility-managed exposure，先补代码再搜索

外部资料支持高波动降风险暴露的思想，但当前“ATR pause”实验证明只暂停新 cycle 反而可能加深已有仓位 DD。真正需要的是 exposure scaling：

- 高 ATR/高 realized vol 时降低 `first_order_quote`、降低 max active exposure 或缩短 max_cycle_age。
- 低 ATR/趋势友好时允许恢复风险。
- 该机制如果现有实盘没有同路径实现，必须先做 backtest/trading-engine parity。

## 6. 验收 gate

每个最终候选必须交付以下证据。

### 6.1 Full gate

- Conservative：`ann > 50`，`DD <= 10`。
- Balanced：`ann > 90`，`DD <= 20`。
- Aggressive：`ann > 110`，`DD <= 30`。
- `budget <= 5000`。
- `max_capital_used_quote <= budget`。
- `budget_blocked_legs = 0`。
- `principal_breached = false`。
- `gate.passed = true` 或说明 replay gate 与上述人工 gate 的差异。

### 6.2 Segment gate

必须跑：

- H1-2023：`1672531200000` 到 `1688169599999`
- H2-2023：`1688169600000` 到 `1704067199999`
- 2024：`1704067200000` 到 `1735689599999`
- 2025：`1735689600000` 到 `1767225599999`
- 2026_ytd：`1767225600000` 到 `1780271999999`

最低要求：

| 模式 | 正收益段数 | max segment DD | H1 贡献 | 2024-2026 combined |
|---|---:|---:|---:|---:|
| Conservative | `>= 4/5` | `<= 15%` 优先，硬上限 `18%` | `<= 35%` | `>= 0` |
| Balanced | `>= 3/5` | `<= 30%` | `<= 45%` | `>= 0` |
| Aggressive | `>= 3/5` | `<= 45%` | `<= 55%` | `>= 0` |

如果某候选 full 达标但 segment gate 失败，只能列为“过拟合/待研究”，不得提交为最终组合。

### 6.3 Budget matrix

每个最终候选必须跑：

- `1000`
- `1500`
- `2000`
- `3000`
- `5000`

如果只在 `5000` 成立，需要给出最小启动本金，不得宣称“适配多种预算”。

### 6.4 Live parity

每个最终候选必须通过：

- live_parity_check。
- 指标依赖清单，包括 cross-symbol K 线依赖。
- TP/SL/spacing/risk_limits 在 trading-engine 中有同路径实现。
- 发布预检可计算 planned margin、max budget、可用 USDT、手续费缓冲。

## 7. GLM 下一步具体执行顺序

1. 在 P4 worktree 上保留当前代码，不要合并 main，不要触碰实盘。
2. 用 `git status --short` 记录工作树；把本轮 `replay_curve_frontier_real.py` 和 `/tmp/codex_curve_real_v2/report.json` 作为失败证据引用。
3. 先跑 P4 full-cycle native search，必须用 `nohup` 和 CPU 证据。
4. 若 P4 原生搜索没有 full gate pass，则扩展 `scripts/native_small_portfolio_search.py`：
   - 支持预算动态 symbol count。
   - 支持 robust pool 输入。
   - 支持 segment-first scoring。
   - 并发提高到 `26-28`。
   - 每 `10` 个 full replay 落盘一次 report。
5. 对 Conservative/Balanced/Aggressive 分别跑不少于 `1000-3000` 个真实 portfolio replay，而不是只跑曲线拼接。
6. 对每个 profile 的 top `30-50` 做 full + 5 segment。
7. 对所有 segment pass 的候选做 budget matrix。
8. 若找到候选，再检查 live parity；如使用新机制，先实现实盘 parity 再回测确认。
9. 若仍找不到，输出 Pareto frontier：
   - DD 门槛内最高年化。
   - 年化门槛上最低 DD。
   - 各预算最小可运行资金。
   - trial count。
   - 失败原因：minNotional、budget blocked、H1 过拟合、2025 熊市、segment DD、live parity 缺口。

## 8. 当前可直接告诉用户的结论

- 这次不是 CPU 没跑。v2 真实 replay 使用了 20 个核心左右，完成 `385/385` full replay 和 `60/60` segment validation。
- 这批 curve frontier 候选真实 replay 后 `passes=0`，不能作为最终组合。
- 现有结果池也还不能直接拼出 Conservative/Balanced 达标组合；Aggressive 表面结果仍需严格分段，现有证据显示过拟合风险高。
- 下一轮突破点不是继续曲线拼接，而是 `segment-first` 原生搜索、动态币种数量、P4 cycle exit、可实盘复现的 regime/volatility 风险管理。
- 在找到真实候选前，不得展示到 flyingkid，不得烟测，不得启动 1000U 实盘。
