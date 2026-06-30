# 2026-06-29 ChatGPT 复核：现有结果池结论、CPU 并发要求与 GLM 下一轮搜索计划

> 目标：在本金预算 `<= 5000 USDT`、多币种但实际交易币种数量受控、抗过拟合、各周期表现均衡、且实盘可复现的前提下，寻找三档马丁组合：
>
> - Conservative：年化 `> 50%`，最大回撤 `<= 10%`
> - Balanced：年化 `> 90%`，最大回撤 `<= 20%`
> - Aggressive：年化 `> 110%`，最大回撤 `<= 30%`
>
> 本文档只讨论回测/搜索/验证任务；不得触碰 Binance 实盘、挂单、仓位或 live trading。

## 1. CPU 低占用问题的确认

用户反馈“CPU 只有 10%，不像在跑满回测”。我已核实：

- WSL 机器核数：`nproc = 30`
- 检查时间：2026-06-29 16:36 左右
- 当时 `portfolio_budget_replay` 进程数：`0`
- 当时 `uptime` load：`1.97, 6.20, 11.91`

结论：用户看到的低 CPU 是真实的。当时不是 replay 正在慢跑，而是上一轮 replay 已经跑完，没有活跃 replay 进程。

上一轮 targeted balanced 任务证据：

- 文件：`/tmp/codex_targeted_balanced_v1/targeted_balanced_b5000_seed20260629.json`
- full replay：`120/120` 已完成
- segment validation：`16/16` 已完成
- passes：`0`
- 结束时间：2026-06-29 16:32 左右

后续所有重型回测必须用下面命令证明 CPU 并发，不允许只说“正在跑”：

```bash
nproc
uptime
pgrep -af portfolio_budget_replay | wc -l
ps -C portfolio_budget_replay -o pid,ppid,pcpu,pmem,etime,args --sort=-pcpu
```

执行标准：

- 真实 replay 阶段应至少保持 `20` 个左右 `portfolio_budget_replay` 进程，每个接近 `100%` CPU，除非已经进入尾段。
- 如果队列仍有大量任务，但 replay 进程长期低于 `8`，必须停止解释结果，先修调度并发。
- 分段验证不能串行跑；我已把 `scripts/targeted_family_search.py` 的 segment validation 改为 `ProcessPoolExecutor` 并行模式，后续要沿用这个模式。
- 每轮实验日志必须记录 `DONE x/y` 或 `SEG x/y`，并每 2-5 分钟记录一次 CPU 状态。

## 2. 现有结果池是否已经能组合出达标策略

我新增了一个只读扫描脚本用于复核现有结果池：

- 临时脚本：`/tmp/scan_replay_result_pool.py`
- 扫描范围：
  - `/tmp/codex_small_search`
  - `/tmp/codex_targeted_balanced_v1`
- 扫描结果：
  - JSON 文件：`34197`
  - 解析失败/非目标结构：`5448`
  - 识别出的结果行：`7351`
  - 满足 `budget <= 5000`、`budget_blocked_legs = 0`、`max_capital_used_quote <= budget` 的真实结果：`6409`

### 2.1 Conservative

`ann > 50 / DD <= 10`：

- pass 数：`0`
- `DD <= 10` 下最高年化：`17.72% / DD 9.16%`
  - 路径：`/tmp/codex_small_search/window_anchor_portfolio_results_min3_b5000_v1/wanch_00826_b5000_conservative_m3_a1_u70_anch25_aaveusdt-etcusdt-injusdt.json`
- `ann > 50` 下最低 DD：`57.79% / DD 14.21%`
  - 来源：`/tmp/codex_targeted_balanced_v1/targeted_balanced_b5000_seed20260629.json`

结论：现有真实 replay 池里没有 Conservative 达标组合。最接近的高收益候选仍超出 DD10，低 DD 候选收益远不够。

### 2.2 Balanced

`ann > 90 / DD <= 20`：

- pass 数：`0`
- `DD <= 20` 下最高年化：`67.25% / DD 18.54%`
  - 来源：`/tmp/codex_targeted_balanced_v1/targeted_balanced_b5000_seed20260629.json`
- `ann > 90` 下最低 DD：`93.53% / DD 26.08%`
  - 路径：`/tmp/codex_small_search/guard_threshold_replays/balanced_fixed_near__strict_dd4_atr15_adx40.json`

结论：现有真实 replay 池里没有 Balanced 达标组合。当前前沿仍是“收益够则 DD 约 26%+，DD 达 20% 则年化约 67%”。

### 2.3 Aggressive

`ann > 110 / DD <= 30`：

- 表面 pass 数：`17`
- 最好之一：`136.62% / DD 29.47%`
  - 路径：`/tmp/codex_small_search/guard_threshold_replays/aggressive_fixed_pass__strict_dd4_atr15_adx40.json`
  - symbols：`AAVEUSDT, INJUSDT, LINKUSDT`
  - `budget = 3250`
  - `max_capital_used_quote = 2735.2`
  - `budget_blocked_legs = 0`

但这些不能直接接受为最终激进组合。GLM 已发现上一批 aggressive 候选 2023H1 贡献极高，2024/2025/2026 常为负。Aggressive pass 必须重新做 5 段真实 replay 后才能判断。

## 3. targeted balanced 局部变异结果

上一轮 targeted balanced 搜索结果：

- full replay：`120/120`
- segment validation：`16/16`
- pass：`0`

关键前沿：

- `ann 67.25 / DD 18.54`：DD 接近平衡目标，但年化不足，且 `h1_contribution_ratio = 1.00`
- `ann 114.84 / DD 31.59`：收益达标但 DD 超，分段里 H1 仍占比约 `0.60`
- `ann 104.58 / DD 32.87`：收益达标但 DD 超，segment DD 最高约 `46.22`

结论：围绕 GLM 这三条 balanced 近似候选做局部变异，暂时无法找到 `90/DD20 + 抗过拟合` 的组合。

## 4. 不能误判的“曲线前沿”

存在一个看起来很诱人的文件：

- `/tmp/codex_small_search/parallel_curve_frontier_sampled_v1.json`

其中 curve-only conservative strict 第一条显示：

- full：`ann 52.69 / DD 8.58`
- segments：5 段全正
- symbols：`XRPUSDT, DOTUSDT, DYDXUSDT, SOLUSDT, SOLUSDT, UNIUSDT`

这只能作为 prefilter，不能当成功结果。原因：

- 它是用候选 equity curve 组合出来的，不是真实 `portfolio_budget_replay`。
- 源候选里有 planned margin 巨大、max capital 不可信或不可直接实盘缩放的问题。
- 组合后还没有经过 Binance minNotional、保证金 cap、budget blocked、per-strategy cap、手续费/滑点/资金费率的真实 replay。

下一轮可以优先把这些 curve frontier 组合转换成真实 config 做 replay，但未 replay 前不能对用户报告为达标。

## 5. 外部方法论结论

本轮外部检索得到的约束与当前问题一致：

- Bailey 等的 PBO/CSCV 框架强调，反复从同一历史样本中挑最优回测，很容易得到过拟合结果；必须做组合式交叉验证或分段/样本外验证。
  - 来源：https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2326253
- Deflated Sharpe Ratio 用于修正多次试验选择偏差和非正态收益，核心思想是不能只报告被大量搜索筛出来的最佳 Sharpe/收益。
  - 来源：https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2460551
- White Reality Check 明确讨论 data snooping：同一数据反复用于模型选择时，满意结果可能只是偶然。
  - 来源：https://www.ssc.wisc.edu/~bhansen/718/White2000.pdf
- Binance 对 futures grid 的公开说明也提醒：杠杆会同时放大收益和亏损，网格/马丁必须带明确风险边界。
  - 来源：https://www.binance.com/en/square/post/1301765210410

因此下一轮不能继续“全周期反复调参，挑最好的一条”。必须：

- 记录 trial count；
- 做 segment-first；
- 做真实 replay；
- 做预算矩阵；
- 做 live parity；
- 最终只接受能解释每段表现的组合。

## 6. GLM 下一轮执行路线

### P0：安全边界

- 不触碰 Binance。
- 不启动 trading-engine live mode。
- 不运行 50U/1000U 烟测。
- 不修改 flyingkid 展示结果。
- 只做离线回测、搜索、验证、文档。

### P1：先做 Aggressive 表面 pass 的分段验证

目的：确认现有 17 条 aggressive pass 是否有一条真正抗过拟合。

候选来源优先级：

1. `/tmp/codex_small_search/guard_threshold_replays/aggressive_fixed_pass__strict_dd4_atr15_adx40.json`
2. `/tmp/codex_small_search/fixed_exposure_cash_priority_results/0105_full_pool_b3000_top_12_fixed_cash_b3250.json`
3. `/tmp/codex_small_search/lp_top_replay_results/0178_full_pool_b2000_top_27.json`
4. 其余 scan 输出中 `ann > 110 / DD <= 30` 的候选

每条必须跑：

- full：2023-01-01 到 2026-05-31
- H1-2023
- H2-2023
- 2024
- 2025
- 2026_ytd

Aggressive 验收：

- full `ann > 110`，`DD <= 30`
- 至少 3/5 段非负
- 任一段 DD `<= 36`
- `2024-2026 combined return >= 0`
- H1-2023 不能独占收益
- `budget_blocked_legs = 0`
- `max_capital_used_quote <= budget`
- `principal_breached = false`
- 不依赖 research-only env gate

如果全失败，必须明确报告“现有 aggressive pass 是全周期过拟合/单段贡献，不可实盘”。

### P2：把 curve frontier 作为候选生成器，而不是结果

优先处理：

- `/tmp/codex_small_search/parallel_curve_frontier_sampled_v1.json`
- `/tmp/codex_small_search/segment_first_portfolio_search_v1.json`
- `/tmp/codex_small_search/full_period_candidates.csv.gz`

执行方式：

1. 从 curve frontier 中提取 conservative/balanced/aggressive 各前 `20` 个组合。
2. 根据 `ids` 回溯原始 candidate config。
3. 转成真实 portfolio config。
4. 强制 budget 为 `1000, 1500, 2000, 3000, 5000` 分别 replay。
5. 每个预算下跑 full + 5 segment。
6. 只保留真实 replay 通过的组合。

注意：如果转换后出现 planned margin 过大、minNotional 不可执行、budget blocked 或 cap 超预算，直接淘汰。

### P3：新搜索空间要从 segment-first 开始

不要继续围绕 INJ/FIL/AAVE 近似解局部变异。新搜索应先找跨周期存活的单策略，再组合。

单策略筛选建议：

- full ann 可以不高，但至少不能依赖单段。
- `2024-2026 combined return >= 0`
- 至少 3/5 段非负，Conservative 需要 4/5。
- 每段 DD 不能远高于目标 profile 的 segment gate。
- 2025 必须有可解释的收益源或低亏损机制。

组合维度：

- symbols：允许 2-6 个，实际交易币种数量可随预算动态减少。
- budget：
  - `<= 1000`：优先 2-3 symbols
  - `1500-3000`：优先 3-5 symbols
  - `3000-5000`：优先 4-6 symbols
- 不要求固定 8 币种。
- 相同 symbol 的 long/short 可同时存在，但必须证明 live parity 和订单不会互相冲突。

### P4：参数方向

优先探索可实盘复现的结构化参数：

- `max_cycle_age_hours`
- `regime_break_stop`
- `strategy_drawdown_pct`
- per-symbol `close > ema(...)` / `close < ema(...)`
- `adx(...)` 趋势/震荡过滤
- ATR/fixed spacing
- soft martingale：`multiplier 1.15-1.60`
- `max_legs 3-6`
- TP 较低但不能被手续费磨损：优先 `80-250 bps`
- cooldown：`3h, 6h, 12h, 24h`

不要作为最终成功条件：

- `MARTINGALE_BT_MAX_PORTFOLIO_ACTIVE_CYCLES`
- portfolio equity stop/cooldown env
- 只存在于回测、不存在于 live parity 的 ATR pause/env gate
- 任意无法映射到 trading-engine 的表达式或指标

### P5：并发与监控要求

30 核机器建议：

- replay workers：`20-24`
- segment validation workers：`20`
- 保留部分 CPU 给 DB/系统

每轮搜索必须输出：

```text
run_id
worker_count
trial_count
queued_count
completed_count
passes
start_time
finish_time
```

运行中每 2-5 分钟记录：

```bash
uptime
pgrep -af portfolio_budget_replay | wc -l
ps -C portfolio_budget_replay -o pid,ppid,pcpu,pmem,etime,args --sort=-pcpu | head -30
tail -50 <run_log>
```

如果用户看到 CPU 低，要能用日志说明是哪一种情况：

- replay 已跑完；
- 尾段剩余任务少；
- 当前处在轻量筛选阶段；
- 调度器坏了，需要修并发。

### P6：最终候选交付格式

每个最终候选必须给：

```text
profile
portfolio_id
budget matrix: 1000/1500/2000/3000/5000
symbols
strategy_count
annualized_return_pct
max_drawdown_pct
total_return_pct
max_capital_used_quote
budget_blocked_legs
principal_breached
trade_count
stop_count
total_fee_quote
total_slippage_quote
total_funding_quote
H1-2023 ret/DD
H2-2023 ret/DD
2024 ret/DD
2025 ret/DD
2026_ytd ret/DD
2024-2026 combined return
H1 contribution ratio
live parity result
config path
replay result path
```

最终只有满足以下条件才可以进入实盘测试计划：

- full gate 达标；
- segment gate 达标；
- budget matrix 至少在目标预算内可运行；
- `budget_blocked_legs = 0`；
- `max_capital_used_quote <= budget`；
- 订单指标、TP、SL、价格统计、手续费、资金费率均能在实盘复现；
- 不依赖任何 research-only 开关。

## 7. 当前结论

现有真实 replay 池里：

- Conservative：没有达标组合。
- Balanced：没有达标组合。
- Aggressive：有表面达标组合，但尚未通过抗过拟合分段验证，不能交付。

下一轮最可能的突破路径不是继续调旧候选，而是：

1. 先分段验证 aggressive 表面 pass；
2. 把 curve frontier 组合转成真实 config 做 replay；
3. 以 segment-first 重新搜索跨周期存活单策略；
4. 再用真实 replay 做组合验证；
5. 全程记录 CPU 并发，避免低利用率空转。

如果上述路线仍无法找到 Conservative/Balanced 达标组合，GLM 必须输出失败证明：搜索空间、trial count、Pareto 前沿、最接近结果、失败原因，以及是否需要用户调整目标或允许混合非马丁策略族。
