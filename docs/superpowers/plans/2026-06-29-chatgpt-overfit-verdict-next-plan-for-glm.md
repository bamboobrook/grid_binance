# 2026-06-29 ChatGPT 复核结论与 GLM 下一轮探索计划

> 任务目标：在本金预算低于 5000U、可多币种但实际交易币种数量受限、抗过拟合、各周期表现均衡、且实盘可复现的前提下，寻找三档马丁组合：保守 `ann > 50% / DD <= 10%`，平衡 `ann > 90% / DD <= 20%`，激进 `ann > 110% / DD <= 30%`。
>
> 复核范围：GLM 报告 `docs/superpowers/reports/2026-06-28-small-cap-optimization-progress.md`，三个 balanced 改进候选，P4 cycle-exit worktree，P4 2025 搜索全量结果，以及我额外做的组合混合探针。
>
> 结论先行：**按现有候选和 P4 已搜索结果，暂时不能组合出同时满足目标收益、目标回撤、抗过拟合和实盘可复现的三档组合。** 下一轮不能再只做“现有候选重新加权”，必须改成 segment-first + live-parity 新机制搜索。

## 1. 本轮我复核和新增验证了什么

### 1.1 已读取的 GLM/历史材料

- `docs/superpowers/reports/2026-06-28-small-cap-optimization-progress.md`
- `docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_floor1500_b5000.json`
- `docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_floor1500_legs5_b5000.json`
- `docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_l5_robust_b5000.json`
- `docs/superpowers/plans/2026-06-29-p4-handoff-for-chatgpt.md`
- `docs/superpowers/reports/2026-06-29-p2-2025-segment-search-findings.md`
- `docs/superpowers/plans/2026-06-28-glm-comprehensive-handoff-to-chatgpt.md`

### 1.2 新增验证产物

P4 worktree：

- Worktree：`/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit`
- P4 全量 2025 搜索：`/tmp/2025_p4_3000_allrows.json`
- P4 分段验证：`docs/superpowers/reports/2025_p4_segments.json`
- floor1500 混合探针：`docs/superpowers/reports/2026-06-29-p4-combo-targeted-probe.json`
- l5_robust 混合探针：`docs/superpowers/reports/2026-06-29-p4-combo-l5-only-probe.json`
- 探针脚本：
  - `scripts/probe_p4_combo_mix.py`
  - `scripts/probe_p4_combo_targeted.py`
  - `scripts/probe_p4_combo_l5_only.py`

说明：这些脚本是 research probe，只用于证明当前候选池的组合边界，不是产品代码；未触碰 Binance、实盘、挂单或仓位。

## 2. 关键结论

### 2.1 现有候选不能直接组合达标

现有结果形成了两个互斥前沿：

| 类型 | 典型结果 | 问题 |
|---|---:|---|
| 高收益腿 | `floor1500` 原样 `99.43% / 24.24%`，去旧 short 后 `125.20% / 44.57%` | 收益够，但 DD 和分段风险不达标 |
| 防守腿 | 加入 2025 low-DD short 后 DD 可到 `15-17%` | 年化掉到 `33-39%`，远低于平衡/激进门槛 |
| 抗过拟合基线 | `l5_robust` 原样 `70.48% / 26.40%` | 分段健康改善，但收益和 DD 都不达平衡 |
| 低 DD short 混合 | `l5_robust + low3 short 15%` 得到 `33.00% / 16.79%` | 风险降了，收益坍塌 |

这不是简单权重没有调好，而是收益来源和防守来源来自不同 regime，且通过 per-strategy cap/minNotional/几何阶梯共同作用后，组合曲线不能线性叠加。

### 2.2 P4 cycle-exit 没有突破前沿

P4 全量 2025 搜索配置：

- budget：3000U
- symbols：BCH/DOT/APT/ETC/NEAR/COMP/GALA/ICP
- modes：short_only、long_and_short
- filters：rsi_moderate、bb_moderate、trend_rsi、none
- P4 参数：`regime_break ema50/ema100/none`，`max_cycle_age 48/120/none`
- 结果：`candidate_count=2624`，live parity `2624/2624 pass`，但 `pass_candidates=0`

2025 单段前沿：

| 条件 | 最好结果 |
|---|---:|
| DD <= 10 | DOT short `6.33% / 7.90%` |
| DD <= 15 | APT short `30.06% / 12.65%` |
| DD <= 20 | APT short `30.06% / 12.65%` |
| DD <= 25 | APT short `70.13% / 23.89%` |
| DD <= 30 | APT short `70.13% / 23.89%` |
| ann >= 50 的最低 DD | COMP short `69.24% / 20.55%` |
| ann >= 90 的最低 DD | COMP short `118.70% / 30.00%` |
| ann >= 110 的最低 DD | COMP short `118.70% / 30.00%` |

更重要的是，全周期分段验证后，高收益 2025 short 在 full-period 上仍然失败：

- APT short：2025 `170.32% / 42.57%`，full `-15.39% / 45.87%`
- COMP short：2025 `168.16% / 37.61%`，full `-17.24% / 50.62%`
- DOT short：2025 `145.51% / 41.74%`，full `-14.96% / 42.58%`
- 低 DD short 也只是“少亏”：APT low-DD full `-2.31% / 7.96%`，COMP low-DD full `-2.54% / 9.37%`
- long_and_short 变体 full 最好只有约 `2.67% / 11.44%`，且 `2024-2026` 合计仍为负

因此 P4 代码是一个有价值的能力补充，但**当前 P4 搜索结果不是最终突破**。如果后续继续使用 P4，需要扩大和重设搜索空间，而不是把 P4 当前结果合并当成解法。

### 2.3 组合混合探针也没有发现甜点

我做了两个定向混合实验：把 `floor1500` 和 `l5_robust` 作为基线，加入 P4 2025 正收益 short 腿。

#### `floor1500` 相关结果

| 实验 | full ann/DD | 备注 |
|---|---:|---|
| `floor1500__as_is` | `99.43 / 24.24` | 收益达标，DD 超平衡门槛 |
| `floor1500__no_old_shorts` | `125.20 / 44.57` | 去掉旧 short 后收益更高但 DD 爆炸 |
| `floor1500 + low3 short 20%` | `38.74 / 15.80` | DD 达平衡，但收益坍塌 |
| `floor1500 + mid3 short 20%` | `38.73 / 15.68` | 同上 |
| `floor1500 + low3 short 35%` | `36.71 / 16.22` | 同上 |
| `floor1500 drop old shorts + low3 short 35%` | `68.70 / 30.96` | 仍不达标 |

#### `l5_robust` 相关结果

| 实验 | full ann/DD | 备注 |
|---|---:|---|
| `l5_robust__as_is` | `70.48 / 26.40` | 当前最健康基线，但不达平衡 |
| `l5_robust__no_old_shorts` | `65.41 / 28.19` | 去旧 short 没改善 |
| `l5_robust + low3 short 15%` | `33.00 / 16.79` | 风险降低，收益不足 |
| `l5_robust + low3 short 25%` | `63.10 / 31.43` | 收益仍不足，DD 变差 |
| `l5_robust + low3 short 35%` | `62.68 / 31.63` | 同上 |

结论：P4 short 腿能降低某些 DD，但会严重稀释或破坏收益；加大 short 权重又带来另一侧风险。现有候选池没有出现 `ann > 90 / DD <= 20` 的组合甜点，更不用说保守 `>50 / <=10` 和抗过拟合激进。

## 3. 为什么“多币种动态组合”这条思路暂时没有直接成功

用户提出“根据本金动态调整组合中币种数量”是正确方向，但不能只靠增加或减少币种数量解决。

原因：

1. **现有收益高度 regime-specific。**
   - 多头核心收益集中在 2023H1。
   - 2025 short 腿在 2023 牛市会亏。
   - 2024-2026 很多候选不是稳定赚钱，而是在不同阶段互相抵消。

2. **资金预算不是线性缩放。**
   - `portfolio_budget_replay` 会按 `portfolio_weight_pct` 应用 per-strategy margin cap。
   - Binance minNotional、首单地板、几何阶梯、budget-blocked legs 会让 1000/3000/5000 的结果跳变。
   - 因此不能假设“5000U 好，1000U 只是等比例缩小版”。

3. **几何马丁存在相变悬崖。**
   - 自截断区：DD 可控，但 ann 低。
   - 完整阶梯区：ann 高，但 DD 大。
   - 当前搜索反复落在两个峰之间，缺少中间甜点。

4. **P4 exit 是退出机制，不是入场 regime 选择。**
   - 这次高收益 short 多数还是 `entry_filter=none`。
   - regime_break 只有在已经持仓且出现浮亏/EMA 条件后才退出，不能阻止牛市里错误开空。

5. **旧 BTC filter 对山寨独立熊市不够。**
   - GLM 已发现 BTC 2025 跌幅小，而山寨跌幅大。
   - 只用 BTC 下跌过滤无法捕捉 per-symbol 熊市。

## 4. GLM 下一轮必须切换到的新路线

下一轮不要再以 full-period 年化最高为第一目标，也不要继续围绕旧 INJ/FIL/AAVE 核心微调。应改成：

```text
先找跨 segment 存活的单策略
→ 再按 segment 曲线组合
→ 再跑 full-period
→ 最后做预算矩阵和 live parity
```

### P0：安全边界

- 不触碰 Binance、实盘、挂单、仓位。
- 不运行烟测，不启动 live trading。
- 所有候选先通过 full + segment + budget + live parity，才允许进入后续实盘验证文档。

### P1：保留当前最有价值代码和证据

建议保留：

- P0 live-parity 能力：跨币种指标、`atr_percent`、ATR 间距预热修复。
- P4 cycle-exit 能力：`max_cycle_age_hours`、`regime_break_stop`。
- `l5_robust` 作为抗过拟合基线，而不是最终候选。

暂不建议：

- 不要把当前 P4 搜索结果当成最终策略。
- 不要因为 P4 代码通过测试就直接 merge 到 main；先决定后续搜索是否继续依赖 P4。若要合并，必须做 whole-branch review。
- 不要把我新增的 `probe_p4_combo_*.py` 当产品代码，它们只是证据脚本。

### P2：建立“抗过拟合优先”的验证器

验证器必须输出：

- full-period：ann、DD、total return、max_capital_used、budget_blocked_legs、principal_breached、fee/slippage/funding。
- 五段：
  - H1-2023
  - H2-2023
  - 2024
  - 2025
  - 2026_ytd
- 额外风控：
  - `2024-2026 combined return`
  - `H1-2023 contribution ratio`
  - 每段 DD
  - 每段 trade_count
  - 每段 max_capital_used
- 试验次数记录：
  - 每一轮搜索的 trial count
  - 参数空间
  - 选中候选的 rank

建议 gate：

| Profile | full gate | segment gate |
|---|---|---|
| Conservative | `ann > 50`, `DD <= 10` | 至少 4/5 段非负；任一段 DD <= 12；2024-2026 合计 >= 0；H1 贡献不能独占 |
| Balanced | `ann > 90`, `DD <= 20` | 至少 3/5 段非负；任一段 DD <= 24；2024-2026 合计 >= 0；2025 不允许大亏 |
| Aggressive | `ann > 110`, `DD <= 30` | 至少 3/5 段非负；任一段 DD <= 36；2024-2026 合计 >= 0；不得本金击穿 |

外部方法论依据：

- Bailey/Borwein/López de Prado/Zhu 的 PBO/CSCV 框架用于估计回测过拟合概率：https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2326253
- Bailey/López de Prado 的 Deflated Sharpe Ratio 用于修正多重试验选择偏差和非正态收益：https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2460551
- White Reality Check 用于处理 data snooping 偏差：https://www.ssc.wisc.edu/~bhansen/718/White2000.pdf

不需要一开始就完整实现学术版 DSR/PBO，但必须至少做到：

- 记录总 trial 数。
- 使用 segment-first / walk-forward。
- 不能在同一全周期上反复调参后只报告最佳点。
- 每个最终候选必须给出样本外/分段证据。

### P3：重新搜索单策略，先找“跨周期存活”的收益源

#### 3.1 搜索目标

不要只找 2025 正收益，也不要只找 full ann 高。先找单策略满足：

- full ann 为正。
- H2-2023、2024、2025、2026_ytd 中至少 3 段不亏，或亏损极小。
- 任一段 DD 不超过 profile 对应上限的 1.2 倍。
- H1-2023 贡献不能超过 full return 的 50-60%。
- budget_blocked_legs 尽量为 0；若不为 0，必须解释是否是安全截断还是执行失真。

#### 3.2 入场 regime 要改，不要只靠 exit

当前 P4 失败的一个核心原因是 short 腿入场太宽。下一轮需要 side-specific regime entry：

- long entry：
  - `symbol.close > symbol.ema(50/100/200)`
  - 或 `market breadth / BTC / ETH` 风险开启
  - 配合 RSI/BB 低吸触发
- short entry：
  - `symbol.close < symbol.ema(50/100/200)`
  - 或 `symbol.ema(50) < symbol.ema(200)`
  - 禁止 `entry_filter=none` 的高杠杆 short 作为最终候选
- regime_break exit：
  - 不只扫 `stop_loss_bps=5000` 这种太宽的阈值
  - 增加 `300/500/800/1200/1500/2000 bps`
  - `max_cycle_age_hours` 增加 `12/24/48/72/120/168`
- regime_break 后增加 re-entry cooldown，避免牛市震荡反复开平。

若这些表达式或 re-entry cooldown 当前没有 live parity，先补 backtest + trading-engine + live_parity_check，再进入最终搜索。

### P4：突破几何马丁相变悬崖

当前几何阶梯让结果在“低收益低 DD”和“高收益高 DD”之间跳变。下一轮要测试新的 sizing，但必须保持实盘可实现：

1. **等保证金阶梯 / arithmetic ladder**
   - 每腿保证金不按几何爆炸，而是固定或线性增长。
   - 目标是减少深套时最后几腿对 DD 的支配。
   - 如果 shared-domain 还没有该 sizing model，先实现结构化配置和 live parity。

2. **ATR/波动率自适应间距**
   - 使用已修复的 ATR spacing。
   - 高波动时扩大 step，低波动时保持交易频率。
   - 不能用 env-only 开关作为最终策略。

3. **ATR TP / trailing TP**
   - 当前 final live parity 只允许 Percent TP。
   - 如果引入 ATR TP 或 trailing TP，必须同步实现 trading-engine reduceOnly 下单逻辑、止盈撤改单、订单统计和 live parity gate。

4. **cycle-level partial de-risk**
   - 不是直接全平，而是在 regime break 时停止补仓或减仓。
   - 这比单纯止损可能更适合马丁，但必须实盘可下单。

### P5：预算自适应 K，但先用静态组合，不要先做动态调度

用户提出“预算小于 5000U 时减少币种数量”是正确方向。下一轮建议：

| 本金预算 | active symbol 上限 | 说明 |
|---:|---:|---|
| 1000U | 1-2 | 避免 minNotional 让权重失真 |
| 2000U | 2-3 | 只保留最高质量单策略 |
| 3000U | 3-4 | 可以加入一个防守腿 |
| 5000U | 4-6 | 再考虑多币种分散 |

先为每个预算生成独立静态 config，不要依赖未实盘实现的动态 active-cycle 调度。

只有在静态 K 仍然失败时，再实现 live-parity 的：

- `max_portfolio_active_cycles`
- per-symbol active cycle cap
- capital allocator / symbol scheduler

如果实现这些动态机制，必须同步：

- backtest-engine
- trading-engine
- API 发布预检
- live_parity_check
- 持仓/挂单恢复逻辑

### P6：组合构造要用 equity curve，不要只用 summary metrics

下一轮组合搜索应先保存单策略的 segment equity curve，然后组合。

组合器约束：

- 总本金 <= 5000U。
- 每个策略 `max_capital_used <= allocated_cap`。
- no principal breach。
- no unimplemented TP/SL/filter。
- 每个 symbol 权重上限，例如 35%。
- 每个方向权重上限，例如 long <= 75%、short <= 45%。
- active symbols 按预算 K 控制。

评分建议：

```text
score =
  full_ann
  - 3.5 * full_dd
  + 0.8 * min(segment_returns)
  + 0.5 * combined_return_2024_2026
  - 3.0 * max(0, segment_dd_2025 - segment_dd_limit)
  - 2.0 * max(0, h1_contribution_ratio - 0.55)
  - 1.5 * budget_blocked_penalty
  - live_parity_penalty
```

注意：如果某组合只是把 2023H1 收益拉高，一律淘汰。

### P7：三档要分开搜索，不能再共享同一核心

#### Conservative

- 先锁 DD <= 10，再看 ann 上限。
- 优先使用低波动/低 DD 单策略，不要为了收益拉 INJ/AAVE/FIL 权重。
- 若 best frontier 长期停在 `ann 30-40 / DD 10-12`，要明确报告“当前 pure martingale/live-parity 模型下保守目标不可达”，并说明需要新 sizing 或新 TP/SL。

#### Balanced

- 以 `l5_robust` 为 baseline，但不要只围绕它加权。
- 目标是补一个真正 full-period 正收益的 bear/regime engine，而不是只加 2025 short。
- 必须同时满足 `ann > 90 / DD <= 20` 和 segment gate。

#### Aggressive

- 旧 aggressive `133.5 / 29.9` 因 2023H1 过拟合，不能作为最终候选。
- 新 aggressive 可以允许更高波动，但必须：
  - full `ann > 110 / DD <= 30`
  - 任一段 DD <= 36
  - 2024-2026 合计 >= 0
  - 2025 不能出现 50%+ segment DD

### P8：最终候选必须做预算矩阵

每个候选至少跑：

- 1000U
- 2000U
- 3000U
- 4000U
- 5000U

输出：

- full ann/DD
- total return
- max_capital_used
- budget_blocked_legs
- min_exact_scaled_executable_principal_quote
- first_order 是否低于 minNotional 安全边界
- per-strategy effective cap

如果一个组合只能在 5000U 附近运行，不能宣称“适应多种预算”；只能说“最低启动本金约 X”。

## 5. 建议的下一轮执行顺序

1. **整理当前 P4 worktree**
   - 保留 P4 代码和我新增的 evidence scripts。
   - 不要立即 merge。
   - 将 `scripts/validate_2025_single_strategy_segments.py` 中我修过的 dedup key 保留，避免 P4 参数被错误去重。

2. **扩展 P4 搜索，但改变重点**
   - 加入 tighter regime-break thresholds。
   - 加入 side-specific trend entry。
   - 加入 shorter max_cycle_age。
   - 禁止最终候选使用 `short entry_filter=none`，除非 full/segment 证明完全健康。

3. **做 robust single-strategy pool**
   - 每个候选跑 full + 5 段。
   - 只保留 full 正收益且 2024-2026 不亏的候选。
   - 先产出 Conservative/Balanced/Aggressive 三个候选池。

4. **做 segment equity-curve portfolio assembly**
   - 先组合 equity curve，而不是组合 summary metrics。
   - K 按预算限制。
   - 对每个 profile 单独组合。

5. **只对 shortlist 跑完整 Rust replay**
   - 每档保留前 20 个组合跑 full + 5 段。
   - 每档前 5 个跑预算矩阵。

6. **若仍失败，输出失败证明**
   - 搜索空间、trial count、Pareto frontier。
   - `DD<=10` 最高 ann。
   - `DD<=20` 最高 ann。
   - `ann>=50/90/110` 的最低 DD。
   - 是否存在 live-parity full-period bear engine。
   - 明确说明需要的新机制，而不是继续调旧参数。

## 6. 最终上线前仍必须补的实盘要求

任何最终候选进入实盘前必须保证：

- backtest-engine 与 trading-engine 使用同一指标计算。
- 跨币种指标引用时，被引用 symbol 的 K 线在 live runtime 中一定加载和更新。
- TP/SL 在 live 中可下单，可恢复，可撤单，可 reduceOnly。
- restart 时能识别现有仓位和挂单，不重复开单。
- 交易记录、手续费、滑点、资金费率、持仓、已实现/未实现 PnL 统计一致。
- 普通网格交易路径不能被马丁改动破坏。
- 1000U 正式运行前必须先做小额 smoke；正式启动前必须让用户确认。

## 7. 当前应告知用户的真实状态

可以这样汇报：

> 当前已有候选和 P4 搜索结果中，没有找到能直接组合出保守、平衡、激进三档都达标且抗过拟合的组合。P4 证明 2025 short 收益源存在，但这些 short 腿在全周期仍然不稳；与现有 `floor1500` / `l5_robust` 混合后，要么收益降到 30-40%，要么回撤升到 26-45%。下一轮必须改为 segment-first 搜索、side-specific regime entry、非几何/波动自适应 sizing，以及预算自适应 K 的静态组合验证。如果这些新机制仍无法突破，应输出不可达前沿，而不是继续报告过拟合候选。

