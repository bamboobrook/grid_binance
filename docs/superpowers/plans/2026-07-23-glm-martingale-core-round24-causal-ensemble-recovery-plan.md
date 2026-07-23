# GLM Martingale Core Round 24：因果残差组合、事件级共享账户与 Soft-SEL 恢复计划

制定日期：2026-07-23。

本文件是 Round 24 唯一任务书。直接运行历史 prequential 回测，不设置 30 天监控，不创建 future lock，
不等待新增行情。任何历史结果只能称 historical backtest，不能称 untouched future OOS。

## 0. 唯一前置权威

只允许读取：

1. `docs/superpowers/artifacts/glm-martingale-core-round23/round23-corrected-authority.json`
2. `docs/superpowers/artifacts/glm-martingale-core-round23/audit/round23-independent-audit.json`
3. `docs/superpowers/artifacts/glm-martingale-core-round23/audit/round1-23-consolidated-frontier.json`
4. `docs/superpowers/artifacts/glm-martingale-core-round23/audit/round23-corrected-failure-ledger.jsonl`
5. `docs/superpowers/artifacts/glm-martingale-core-round23/audit/round24-external-source-update.json`
6. `docs/superpowers/reports/2026-07-23-chatgpt-round23-execution-audit-correction-and-feasibility.md`

禁止继承为有效结论：

- Round 23 的 `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET`；
- BTC `25.69%/23.54%`、DSR `+12.23`、PBO `0.435`；
- `17.72%/29.52%` 的所谓 valid M1；
- 1471/1633 次“有效 backtests”；
- 540-cell MicroIntegral 对 SSRN 5895159 的关闭结论；
- Round 9 finished-curve allocator 的 5/5；
- Round 19 inner-train `41.43%/6.27%` 作为候选。

它们只能作为待修复 hypothesis 或反例，不能 seed promotion、trial pass、frontier 或“已穷尽”。

## 1. 不变目标与最新档位

| 档位 | stitched annualized | max equity DD | five cold starts | effective gross leverage cap |
|---|---:|---:|---:|---:|
| 保守 | `>=50%` | `<=10%` | `>=4/5` 正，另报 5/5 | `<=2.0x` |
| 平衡 | `>=90%` | `<=20%` | `>=4/5` 正，另报 5/5 | `<=3.0x` |
| 激进 | `>=100%` | `<=30%` | `>=3/5` 正，另报 5/5 | `<=4.0x` |

Round 24 不添加 `Sharpe>=2` 作为用户目标门。必须报告 Sharpe、Sortino、Calmar、tail loss，但不得偷偷改变
三档定义。

### 1.1 Martin 唯一收益合同

1. principal 只允许 `500/750/1000/1500/2000/3000/4000/4999U`，必须严格 `<5000U`；
2. 所有 symbols、groups 和 families 共用一个连续 cash/margin/equity/reserve account；
3. 每个目标 policy 实际成交 base assets `>=5`，configured/loaded assets 不计；
4. 单 symbol、单 group、单 family、单 test block 的正收益贡献均 `<=50%`，另报是否 `<=35%`；
5. 每个计分 timeline 必须存在真实 loss-after-add SO；
6. 每个 SO 前，group net PnL after estimated close cost 必须 `<0`；
7. 下一层 group gross 必须严格大于上一层；
8. SO adverse distance 从上一次**已成交 group fill**计算，未成交 bar 不改变基准；
9. 收益只能来自 Martin FO/SO/TP/reduce/abort 和持仓对应 funding/borrow cashflow；
10. residual、pair graph、OI、crowding、depth、flow、SEL、regime、risk allocator 只能决定
    FO/SO/TP/reduce/abort/freeze/admission/reserve；
11. 禁止独立 trend、carry、market-making、fixed-fractional、anti-Martingale 或 theoretical PnL sleeve；
12. liquidation、equity `<=0`、future/stale signal、filter bypass、NaN、缺 order trace 立即淘汰；
13. equity DD、balance DD、DDR 分开报告，目标只读 equity DD；
14. exact minimum principal 同时覆盖 spot cash、perp initial/maintenance margin、fees、close、funding/borrow
    和所有 active groups 的 next SO reserve；
15. 只有连续 stitched test days `>=365` 可判断年化目标；
16. block 边界不得重置资金或抹掉 active cycle；selector 变化时已有 group 使用 open-time frozen legs/weights；
17. fit、threshold、pair、weight、risk quota 和 model 都不得读取当前或未来 outer block return。

### 1.2 本轮进展门，不等于目标命中

```text
P-C = ann >=35%, DD <=20%, >=4/5 cold starts,
      >=5 actual assets, all contribution gates,
      shared-account execution, anti-overfit gates,
      independent adapter parity
```

只有 P-C 可以写 `HISTORICAL_PREQUENTIAL_FRONTIER_PROGRESS`。任何单纯 ann 提高、DD 降低、5/5 train 或
短窗年化都不能写 progress。

## 2. 永久禁止重复

Round 24 不再运行：

- 普通 multiplier/spacing/max-legs/TP/FO 大网格；
- generic EMA/ADX/RSI/HTF gate；
- ordinary cross-sectional momentum/reversal；
- same-symbol long+short hedge grid；
- standalone funding carry/reversal；
- DGT、breakout、fixed-fractional、普通 DD scaling；
- 旧 first-passage/hazard、last-executed-SO、minigrid/depth-TP 原 fingerprint；
- finished equity curve blending 或“每 sleeve 独立本金后相加”；
- 每 block 选赢家、事后挑 5 个正窗口、90 天年化；
- `n_trials=1`、把 symbols 当 policies、事后选 8 个配置算 PBO；
- 从标题、摘要或论坛猜 SSRN 5895159；
- Round 22 Python PnL engine；
- Round 23 RelaxedM1、1/2% 冒充 0.2/1.0%、current-bucket M2；
- 无 running 行的 summary terminal、标签 hash 冒充 Git commit、空流 SHA256 冒充 trace。

每次运行前计算：

```text
mechanism_fingerprint = SHA256(
  engine_semantics + family_formula + signal_lag + fit_protocol +
  group_construction + layer_schedule + exit_rule + cost_model + account_model
)
```

fingerprint 已在 corrected failure ledger 中关闭且 engine/data/protocol 未变化时，launcher 必须拒绝。

## 3. R0：先建立不可绕过的真实证据链

### 3.1 Git

1. 从包含 Round 23 corrected authority 的 clean/pushed commit 创建 `glm-martingale-core-round24`；
2. 立即 `git push -u origin glm-martingale-core-round24`；
3. 每个 replay 的 parent commit 必须 clean、已 push，且等于 upstream remote commit；
4. Git commit 是真实 40-char hex；artifact/trace 才是 64-char SHA256，schema 必须区分；
5. 每 phase 单独 commit/push；
6. 每个 commit body 必须同时包含：`问题描述`、`复现路径`、`修复思路`。

### 3.2 Launcher/registry schema

每个 experiment ID 必须正好一行 `running` 和一行 terminal。launcher 必须从真实进程采集：

```text
experiment_id / parent_experiment_id / phase / mechanism_fingerprint
real git commit / dirty / upstream commit
binary/source/data/filter/maintenance/funding/borrow/cost/config/fit/protocol SHA256
nonempty raw argv / pid / exit / start / end / wall seconds / peak RSS
fit cutoff / purge / signal-ready / replay timestamps
event/trade/order/equity/funding/rejection/signal/margin trace file paths + SHA256
```

约束：

- `start < end`、`wall_seconds>0`、`rss_kb>0`；
- running row 的 terminal-only 字段用 null，不用空流 digest；
- terminal complete 的八类 trace 必须是存在、非空、内容可解析的 immutable 文件；
- non-complete terminal 与 failure ledger 同一事务写入；
- registry/ledger/trace 任一写失败，实验 terminal 必须 failed；
- summary 必须由 registry + traces 生成，禁止手写覆盖。

### 3.3 必须在真实 validator 上注入的 canaries

除 Round 23 的 15 项外，新增：

1. 64-char label 作为 git commit；
2. empty-stream SHA256 作为 complete trace；
3. terminal-only summary row；
4. start=end、wall=0、RSS=0、argv empty；
5. authority 与 registry replay count 不一致；
6. state machine 输出与手写 state 不一致；
7. symbol 数等于 loaded 数但 actual fill 数 `<5`；
8. independent accounts summed as portfolio；
9. selected eight-config PBO；
10. DSR trial count 小于 global ledger；
11. funding hash存在但 funding cashflow 恒为 0 的 perp position；
12. current snapshot/current bucket signal；
13. block 边界 cycle reset；
14. empty block 从 12-block 分母删除；
15. source PDF 实为 HTML/Cloudflare；
16. latest aggressive target被写回 110%；
17. 把 reporting Sharpe 变成用户 tier hard gate。

R0 未过，状态只能是 `BLOCKED_ENGINE_DATA_OR_EXECUTION`，禁止搜索。

## 4. R1：一个真实多币共享账户引擎

Round 24 scored PnL 只允许 production-conservative Rust runtime。Python 可生成 fit/signal，但不能计算收益。

### 4.1 账户与订单

1. 全部 market legs 进入一个 timestamp-ordered event queue；
2. position key 至少包含 `symbol + market_type + side/position_mode`；
3. wallet balance、spot cash/inventory、perp margin、unrealized、maintenance、funding/borrow 独立记账；
4. 每次 fill 后断言 rounded leg gross 与 resolved group gross 在 tolerance 内；
5. 每个 mark 后计算 shared equity、maintenance tier、liquidation buffer 和 leverage；
6. reserve 同时覆盖所有 active groups 的 next SO、close cost、maintenance 和 pending delayed legs；
7. FO/SO/TP/reduce/abort/rebalance/end-close 全收费并结算 funding/borrow；
8. liquidation fee 和 forced close 进入最终 equity；穿仓账户终止在 0，禁止继续回放；
9. active group 冻结 open-time legs/weights；block 新 fit 只影响新 group；
10. engine 必须输出 nonempty canonical order/fill/reject/margin/funding streams。

### 4.2 真实压力路径

- partial fill 25/50/75% 必须改变 order fill quantity、inventory、cash 和 reserve；
- leg delay 1/2/3 bar 必须在后腿实际时间成交并记 legging PnL；
- one-leg reject 执行预注册 hedge-or-flatten；
- filter/maintenance tier 变化发生在 event time；
- kill/restart 从 SQLite 恢复 wallet、positions、cycles、last fill、reserved margin、pending orders 和 fit version；
- backtest adapter 与 trading-service adapter 使用不同 fake exchange 实现，再比较 ack/reject/order suffix/equity hash。

### 4.3 必过测试

```text
soft_ladder_first_so_is_1p25_not_1p55
opening_and_closing_costs_gate_tp_and_so
forced_close_cost_is_in_final_equity
breached_account_terminates_at_zero_without_second_close
shared_account_rejects_individually_affordable_but_jointly_unreserved_orders
block_transition_preserves_wallet_positions_cycles_and_running_peak_dd
active_group_keeps_open_fit_after_selector_roll
five_loaded_symbols_with_four_filled_symbols_fails_asset_gate
partial_fill_changes_cash_inventory_margin_and_next_so_reserve
leg_delay_books_realized_legging_loss
one_leg_reject_executes_hedge_or_flatten
funding_cashflow_changes_equity_for_open_perp
maintenance_tier_change_can_liquidate_at_event_time
restart_reconcile_matches_uninterrupted_order_and_equity_hashes
independent_backtest_and_service_adapters_match
```

R1 先重放 R4、R7、R19 和 Round 23 BTC 作为**算术 canary**。指标变化是预期结果；它们不得进入 Round 24
排名或成为 target candidate。

## 5. R2：冻结数据与历史 prequential 协议

### 5.1 主时间协议

```text
fit start:       2023-01-01
tb01 test start: 2023-07-01
test end:        2026-05-31
12 contiguous test blocks
purge: >= max feature lookback + source publication latency + one execution bar
one continuous account across tb01..tb12
```

五个 cold starts 在任何 Round 24 收益 replay 前冻结：

```text
cs00  = 2023-07-01
cs30  = 2023-07-31
cs60  = 2023-08-30
cs90  = 2023-09-29
cs120 = 2023-10-29
```

每个 cold start 独立初始化同一 frozen policy，然后连续运行到 2026-05-31。不得筛选、替换、拼接或共享
最终 equity。

每 block：

1. fit 只读 `<= test_start - purge`；
2. pair、weight、threshold、model、quota 只由 fit/inner blocks决定；
3. no-fit/no-signal/reject/timeout/breach/empty block 均保留在 12-block 分母；
4. 每块只报 raw return/DD/PF，不报 ann；
5. 最终 stitched `>=365d` 才报 ann/DD；
6. policy hash、fit cutoff、signal snapshot、pair/weight、transition cost 逐块落盘。

### 5.2 数据门

基础 universe 冻结：

```text
BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT,
XRPUSDT, DOGEUSDT, LINKUSDT, LTCUSDT
```

必须重建 manifest，不得复用 Round 23 的 duplicate/failed rows：

- futures/spot klines、funding、filters、maintenance：主 universe 完整协议窗口；
- metrics：8 币，2023-01-01..2026-05-31；
- bookDepth/aggTrades：至少 BTC/ETH/BNB/SOL/XRP/DOGE 六币同窗口；
- 每个 archive + sidecar CHECKSUM；
- 唯一 key、checksum pass、schema、row count、min/max timestamp、gap histogram、sample hash；
- signal source 至少延迟一个 completed snapshot/bucket；
- missing `>10m` 禁止 FO/SO，active cycle 按 frozen flatten/freeze 规则处理。

F1 residual 可以在基础数据通过后先跑；F3 order-flow 只有自己的 metrics/depth/aggTrades 数据门全部通过才可跑。
数据不完整时写 `blocked_incomplete_data`，不能缩短窗口后年化。

## 6. R3：预注册与全局 trial ledger

收益 replay 前提交并 push：

```text
round24-policy-manifest.json
round24-protocol.json
round24-data-manifest.json
round1-24-trial-ledger.json
round24-mechanism-fingerprints.jsonl
```

要求：

1. Round 24 每个 policy 在收益前获得 immutable ID；
2. 历史无法重建的 trial 数写 `unknown`，不得归零；
3. 至少记录 Round 23 可见 trial floor `1080`，并对 global trial count 做
   `10k/50k/100k/1m` DSR sensitivity；
4. DSR 使用经过公式单测的实现，不能把自定义 z-score写成 probability；
5. CSCV/PBO 使用**全部同层 Round 24 policy returns**，不使用 symbols、cold starts 或 selected subset；
6. 同时报 stationary/bootstrap CI、White/SPA-style reality check、neighbor stability、selection frequency；
7. 由于全历史已读，任何结果都只能称 historical prequential，不得称真正未见 OOS。

## 7. F1：Causal Residual Disjoint-Pair Martin

这是 Round 24 第一优先，也是 R19 唯一允许的修复路径。

### 7.1 Train-only group construction

每个 block 只用 fit 数据：

1. 对 8 币所有可交易 pair 做 liquidity/filter feasibility；
2. rolling log-price hedge ratio，只允许 completed bars；
3. ADF 与 KPSS 对 residual stationarity 必须一致；
4. half-life、crossing count、expected close cost 和 break frequency 只作 train score；
5. 构造 disjoint maximum matching，同一 symbol 不得同时出现在两个 active pair；
6. 至少 3 个 executable pairs、至少 6 个 assets；不足则该 block no-fit；
7. outer return 不得选 pair、rank、weight 或 threshold。

R19 `M1R_F3_028/044` 只作 regression canary；最终 pair graph 必须由每个 block 的 train 数据重建。

### 7.2 Martin cycle

```text
FO: residual reaches train-frozen entry boundary and stationarity/break/liquidity gates pass
SO: aggregate net<0 after close cost + residual adverse from last filled group level
    + stationarity still valid + shared reserve pass
TP: aggregate group net positive after all close/legging/funding/borrow costs
abort: break/deadline/stale/liquidation-buffer breach; realized loss enters equity
```

paired legs 同一 group，long/short gross 差 `<=5%`，所有层使用 frozen pair beta 并经过 rounding。

### 7.3 只开放的新结构维度

```text
fit lookback:       60d / 120d
entry z:            1.5 / 2.0
SO residual step:   0.50 / 0.75 train sigma from last fill
max live groups:    2 / 3
```

共 `16` 个结构 policies。layer schedule、cost、contribution cap 不开放搜索。G1 后才把 surviving policy
映射到三档 risk profiles，避免把普通 sizing grid 再跑一遍。

### 7.4 固定 event-level concentration repair

- new group admission 使用 lagged deficit round-robin；
- symbol reserved gross `<=25%`、group reserved gross `<=25%`；
- 正收益贡献实时超过 35% 时只允许已有 cycle 风险退出，禁止该 symbol/group 新 FO；
- 规则只控制 admission/reserve，不产生独立收益；
- 必须做 no-scheduler / static-first-N / deficit-round-robin 三臂 ablation，证明 order hash 改变且不是 label-only。

## 8. F2：Event-Level Martin Family Ensemble

这是 Round 9 allocator 的唯一合法重建，不是独立收益 family。

### 8.1 Parent gate

只有至少两个不同 mechanism 的 parent 各自通过 G1 P-B 才能启动。允许的 parents：

- F1 causal residual；
- changed-engine R4/R7 legacy Martin（必须重新通过本轮 G1，旧数值无效）；
- F3 strict M1 或 M2；
- F1/F3 的 Soft-SEL enhancement（若已通过）。

若只有一个 parent，F2 写 `not_applicable_single_parent`，不能用旧 finished curves补足。

### 8.2 Shared account allocator

1. 按 timestamp 合并各 parent 的真实 order intents；
2. 一个 shared reserve 决定 accept/reject；
3. inactive family 当期 PnL 必须为 0；
4. block start 权重只用 earlier fit/inner returns；
5. 只开放两个预注册 allocator：`equal_risk`、`capped_inverse_expected_shortfall`；
6. family gross `<=40%`、group gross `<=25%`、symbol gross `<=25%`；
7. active overlap 必须合并 margin、maintenance、pending legs 和 next SO reserve；
8. 最多 `12` 个 parent-set/allocator combinations；禁止按 outer Calmar 继续扩组合。

必须提交 `inactive_family_zero_pnl`、`no_current_interval_weight`、`finished_curve_injection_rejected` 三个 canary。

## 9. F3：Strict Lagged Order-Flow Martin

Round 23 RelaxedM1/M2 不算本 family 的有效试验。

### 9.1 F3-M1 strict OI/crowding exhaustion

Long FO 必须同时满足：price downside extension、OI expansion、taker sell extreme、top/all crowd short、
taker sell pressure deceleration；Short 完全镜像。SO 还要求 aggregate loss、last-fill adverse 和 flow exhaustion；
OI/taker 继续恶化时 freeze SO 或 abort。

只开放：

```text
robust window: 7d / 30d
tail quantile: 95% / 97.5%
adverse spacing: 1.0 / 1.5 train ATR
```

共 `8` policies。

### 9.2 F3-M2 available-level depth/aggressor flow

不再把 1/2% 冒充 0.2/1.0%。根据 Binance archive 的实际 level 预注册新 fingerprint：

```text
depth level: exact 1% only / median(exact 1%, exact 2%)
flow window: 15m / 60m
tail quantile: 95% / 97.5%
```

共 `8` policies。所有 depth 和 aggTrade 至少 lag 一个 completed bucket。Long FO：downside extension + lagged
sell-flow deceleration + lagged bid replenishment；Short 镜像。SO 同时要求 loss、last-fill adverse、replenishment、
flow 不再加速。depth 消失时 freeze/flatten，禁止 market-making PnL。

F3-M1/M2 每个 policy 必须在至少 5 个 actual assets 成交；逐币独立 engine 后相加立即 invalid。

## 10. E1：Exact Soft Martin + train-only SEL

来源：`10.3390/a19060442`，PDF SHA256：

```text
8df91d970f91610d318440c86108a5e33f8d515500f836c47ca727d2afdc3935
```

实现前提交 source map：PDF URL/hash、页码、原文公式/算法、字段映射、不能采用部分。

只有 F1 或 F3 parent 先通过 G1 P-B 才允许：

1. exact ten-layer relative schedule `linspace(1,5,10)`；
2. 每层仍必须满足 next-SO/close/maintenance reserve 和 leverage cap；
3. SEL label 只来自 inner-train 的完整 forward Martin path simulation；
4. label window 与 outer block purge/embargo 不重叠；
5. 只允许 regularized logistic 或 GAM 两类低容量模型；
6. SEL 只能 veto/side-select FO/SO，不能创建独立 trade；
7. permuted-label、feature ablation、calibration、neighbor stability 全部执行；
8. permuted label 仍赚钱、模型无 order delta、SO 消失或小资金放不下 10 层时淘汰；
9. 最多 2 parents、每 parent `<=4` SEL policies，总计 `<=8`。

必须同时报告 balance DD、equity DD 和 DDR。论文的单 EUR/USD、1:500、442.6% ann、79.97% equity DD
不得作为参数先验、收益证据或目标命中。

## 11. G0：机制真实性与因果激活

每个 family/开放参数至少：

```text
8 synthetic adversarial traces
4 real causal windows: bull / bear / range / jump-liquidity shock
low/high effective config hash delta
family state delta
order or rejection delta
one-bar future shift rejection
label-swap rejection
```

额外必须证明：

- long/short 对称；
- 无 net loss 不出现 SO；
- adverse 但 residual/flow 继续恶化时不盲目 SO；
- last-fill basis 未成交不移动；
- next layer gross 严格增加；
- group/symbol quota 在事件发生时生效；
- stale/future fit、depth、flow、weight 立即 fail；
- source/model label 本身不改变 PnL。

参数 inert 时从 manifest 删除并写 failure ledger。G0 未过禁止 G1。

## 12. G1：冻结 policy 的连续 12-block 回放

每个 policy 先在 `1000U` 运行完整 timeline。每 family 最多 4 个进入 G2。

Immediate fail：

- 任一 block 删除、empty block 跳过、资金/cycle/running peak 重置；
- liquidation/principal breach；
- actual assets `<5`；
- 无 loss-after-add SO；
- symbol/group/family/block positive contribution `>50%`；
- cost/gross profit `>50%`；
- stale/future fit/signal；
- 缺 trace/hash/order/funding/margin；
- resolved gross/leverage/reserve 不可从 traces 独立复算。

进展门：

| Gate | 条件 |
|---|---|
| P-A | production-conservative main replay + independent adapter parity + evidence chain 全过 |
| P-B | 12-block compounded `>0`、无 breach、`>=8/12` 正、贡献门通过 |
| P-C | ann `>=35%`、DD `<=20%`、至少 4/5 cold starts 正、anti-overfit 全过 |
| P-D | 任一 50/90/100 档位和全部 hard gates 同时命中 |

只按 frozen stitched policy 排名，不按 block 选 top-N。P-B 全部失败时停止该 family，不看结果扩 grid。

## 13. G2：预算、五起点、压力和防过拟合

对 committed survivors 运行：

1. 全部 8 个 `<5000U` budgets；
2. 五个预注册 cold starts；
3. fee/slippage `1x/1.5x/2x`；
4. real partial fills `25/50/75%`；
5. real leg delays `1/2/3 bars`；
6. one-leg reject + hedge-or-flatten；
7. filter/maintenance tier change；
8. funding/borrow `1x/1.5x/2x` 和 timestamp gap；
9. metrics/depth/flow missing/stale；
10. LOSO（删一个 symbol）与 LOGO（删一个 group）；
11. kill/restart/reconcile；
12. min-notional/qty/price rounding；
13. exact minimum principal binary search，并在上下相邻本金复验。

至少两个相邻 budgets 通过；任一压力 liquidation 淘汰。

防过拟合输出：

```text
exact Round 24 policy count
historical global-trial sensitivity 10k/50k/100k/1m
Deflated Sharpe implementation/version/tests
full-matrix CSCV/PBO
stationary/bootstrap confidence interval
White/SPA-style reality check
neighbor stability and selection frequency
block/symbol/group/family concentration
permuted-label controls for SEL
```

缺任一项写 `anti_overfit_not_auditable`，不得进 R8。

## 14. R8：三档与组合最终判断

只有 committed G2 survivors 可判断。每个 top candidate 必须列出：

```text
symbols and per-block signed weights
long/short legs and market identities
FO/SO/TP/reduce/abort counts and attribution
layer gross schedule
effective leverage and peak gross
exact minimum principal
fees/slippage/funding/borrow/legging/liquidation buffer
equity DD/balance DD/DDR
12 raw block returns
5 cold-start returns
symbol/group/family/block concentration
DSR sensitivity/PBO/SPA/trial count
live adapter parity trace hashes
```

缺任一字段 candidate invalid。三档使用最新 `50/10`、`90/20`、`100/30`，不使用旧 110%。

## 15. 状态机与 GLM handoff 格式

最终状态只能是：

```text
HISTORICAL_PREQUENTIAL_TARGET_HIT
HISTORICAL_PREQUENTIAL_FRONTIER_PROGRESS
VALID_HISTORICAL_PREQUENTIAL_NO_TARGET
BLOCKED_ENGINE_DATA_OR_EXECUTION
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

约束：

- `TARGET_HIT` 只在 P-D 与全部 hard gates 通过；
- `FRONTIER_PROGRESS` 只在 P-C 通过；
- `VALID_NO_TARGET` 要求 R0-R8 全部按配额完成且证据完整；
- engine/data/registry/source 前置失败必须 blocked/invalid；
- `0 finalist` 不会自动把未执行的后续任务标 complete；
- handoff 必须由 validator 从 registry/traces 生成；
- 禁止写“全部可能性耗尽”，只能关闭 exact fingerprint；
- 每个失败必须有 never-repeat row。

最终 handoff 第一页必须是一张机器表：

| 字段 | 必填 |
|---|---|
| audited commit/upstream/dirty | 是 |
| registry running/terminal/unique/violations | 是 |
| exact executed policy/replay counts | 是 |
| phase complete/blocked/invalid reasons | 是 |
| strict survivors/P-C/P-D counts | 是 |
| best strict candidate或 null | 是 |
| latest three-tier verdict | 是 |
| unresolved evidence/data/source blockers | 是 |

## 16. 必交产物

```text
docs/superpowers/artifacts/glm-martingale-core-round24/round24-authority.json
docs/superpowers/artifacts/glm-martingale-core-round24/round24-execution-state.json
docs/superpowers/artifacts/glm-martingale-core-round24/exploration-registry.jsonl
docs/superpowers/artifacts/glm-martingale-core-round24/failure-ledger.jsonl
docs/superpowers/artifacts/glm-martingale-core-round24/round24-policy-manifest.json
docs/superpowers/artifacts/glm-martingale-core-round24/round1-24-trial-ledger.json
docs/superpowers/artifacts/glm-martingale-core-round24/gates/*.json
docs/superpowers/artifacts/glm-martingale-core-round24/traces/**
docs/superpowers/reports/2026-07-XX-glm-round24-execution-handoff.md
```

raw market archives 不进 Git；URL/size/SHA/checksum/time/schema manifest 必须提交。trace 可压缩，但 validator
必须能解压、解析、重算关键账务。

## 17. GLM 严格执行顺序

- [ ] R0 branch/upstream/real launcher/registry/validator canaries
- [ ] R1 shared-account engine/accounting/order/funding/restart/independent adapters
- [ ] R1 R4/R7/R19/R23 arithmetic canaries，全部标 non-candidate
- [ ] R2 protocol/cold starts/data manifests freeze + commit/push
- [ ] R3 policy manifest/global trial ledger/fingerprints freeze + commit/push
- [ ] F1 causal residual disjoint-pair implementation
- [ ] F3 strict M1/M2 implementation；M2 数据不全则明确 blocked
- [ ] G0 activation/causality/order-delta gate
- [ ] G1 exact quotas and continuous 12-block replays
- [ ] G1 survivors commit/push
- [ ] E1 Soft-SEL only for eligible parents
- [ ] F2 event-level ensemble only with at least two eligible parents
- [ ] G2 all budgets/cold starts/real stresses/LOSO/LOGO/trial correction
- [ ] R8 latest three-tier judgment
- [ ] validator-generated authority/state/handoff
- [ ] final tests/commit/push/clean worktree

任何前置项失败时，先修复并重跑该 gate。禁止越过失败项继续搜索，再用 handoff 文字宣布 complete。
