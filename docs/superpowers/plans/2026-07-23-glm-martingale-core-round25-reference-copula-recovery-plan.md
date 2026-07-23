# GLM Martingale Core Round 25：Reference-Asset Conditional-Copula 多对马丁恢复计划

制定日期：2026-07-23。

本文件是 Round 25 唯一任务书。直接运行历史 prequential backtest，不设置 30 天监控，不等待新增行情，
不创建 future lock。所有结果只能称 historical backtest。

## 0. 唯一权威与本轮边界

只允许继承以下结论：

1. `docs/superpowers/artifacts/glm-martingale-core-round24/round24-corrected-authority.json`
2. `docs/superpowers/artifacts/glm-martingale-core-round24/audit/round24-independent-audit.json`
3. `docs/superpowers/artifacts/glm-martingale-core-round24/audit/round24-corrected-failure-ledger.jsonl`
4. `docs/superpowers/artifacts/glm-martingale-core-round24/audit/round1-24-consolidated-frontier.json`
5. `docs/superpowers/artifacts/glm-martingale-core-round24/audit/round25-external-source-update.json`
6. `docs/superpowers/reports/2026-07-23-chatgpt-round24-execution-audit-frontier-and-round25-direction.md`

禁止继承为有效结论：

- Round 24 的 `F1 16/16 valid failures` 和 exact fingerprint closure；
- `R24-F1-11 -2.72%/7.73%`；
- Round 19 `41.43%/6.27%/5-of-5` 作为候选；
- Round 9 `62.78%/18.38%/5-of-5` finished-curve allocator；
- Round 14 被撤销的 `50.70%/20.29%` 和 `35.07%/13.56%`；
- Round 23 单 BTC 的 5/5 与 25.69% headline；
- 外部论文的 75.2% ann、3.77 Sharpe、选币或参数。

本轮唯一主 family：

```text
C1_REFERENCE_ASSET_CONDITIONAL_COPULA_MARTIN
```

唯一条件增强：

```text
C2_ASYMMETRIC_TAIL_SO_VETO
```

C2 只有 C1 parent 通过 G1 P-B 后才允许运行。不得新增第三个收益 family，不得退回普通 multiplier、
spacing、TP、EMA/ADX/RSI、PC1/VECM/Johansen、order-flow 或 allocator 大网格。

## 1. 不变目标

| 档位 | stitched annualized | max equity DD | five cold starts | effective gross leverage cap |
|---|---:|---:|---:|---:|
| 保守 | `>=50%` | `<=10%` | `>=4/5` 正，另报 5/5 | `<=2.0x` |
| 平衡 | `>=90%` | `<=20%` | `>=4/5` 正，另报 5/5 | `<=3.0x` |
| 激进 | `>=100%` | `<=30%` | `>=3/5` 正，另报 5/5 | `<=4.0x` |

不添加 Sharpe hard gate。必须报告 Sharpe、Sortino、Calmar、tail loss、equity DD、balance DD 和 DDR。

### 1.1 小资金与 Martin 唯一收益合同

1. principal 只允许 `500/750/1000/1500/2000/3000/4000/4999U`，严格 `<5000U`；
2. 所有 symbols/groups 共用一个连续 cash/margin/equity/reserve account；
3. 每个目标 policy 实际成交 base assets `>=5`，C1 机制门要求 `>=3` disjoint pairs、`>=6` assets；
4. 每个 scored timeline 必须有真实 `aggregate net loss -> larger filled SO`；
5. 每个 SO 前 group net PnL after estimated close cost `<0`；
6. 下一层 filled gross 严格大于上一层；
7. adverse distance 从上一次已成交 group fill 计算，未成交 bar 不移动基准；
8. 收益只来自同一 Martin group 的 FO/SO/TP/reduce/abort 和对应 funding cashflow；
9. copula、cointegration、tail dependence、scheduler 只能决定 FO/SO/TP/reduce/abort/freeze/admission；
10. BTCUSDT 只作 reference signal，不得成为隐藏收益腿；
11. 禁止独立 trend、carry、market-making、fixed-fractional、anti-Martingale、finished curve 或 theoretical PnL；
12. liquidation、equity `<=0`、future/stale signal、filter bypass、NaN、缺 order trace 立即淘汰；
13. exact minimum principal 覆盖 margin、fees、close、funding、maintenance、pending legs 和全部 active groups 的 next SO reserve；
14. block 边界不得重置 wallet、running peak 或 active cycle；active group 使用 open-time frozen pair/model/weights；
15. pair、marginal、copula、threshold、quota 和 risk profile 不得读取当前或未来 outer return；
16. stitched test days `<365` 不允许判断年化目标。

### 1.2 贡献度合同修正

上一轮的单-family 自锁必须删除：

- symbol、group、test block 的正收益贡献各 `<=50%`，另报是否 `<=35%`；
- 只有同时存在两个以上真实 PnL families 的 ensemble 才检查 family `<=50%`；
- C1 单-family replay 的 family contribution 必然是 100%，只作信息字段，不得成为 immediate fail；
- C2 不产生 PnL，不算独立 family。

### 1.3 进展门

```text
P-C = ann >=35%, equity DD <=20%, >=4/5 cold starts positive,
      >=6 actual assets, all concentration/account/anti-overfit/parity gates pass
```

只有 P-C 可写 `HISTORICAL_PREQUENTIAL_FRONTIER_PROGRESS`。正式 50/90/100 目标不降低。

## 2. 永久禁止重复

Round 25 不运行：

- ordinary multiplier/spacing/max-legs/TP/FO grid；
- generic EMA/ADX/RSI/HTF gate；
- cross-sectional momentum/reversal；
- same-symbol long+short hedge；
- standalone funding carry、spot-perp basis、DGT、breakout、minigrid/depth-TP；
- PC1、dynamic factor、Johansen、VECM、Kalman、partial cointegration 的旧 fingerprint；
- Round 23/24 OI、crowding、depth、aggTrades、flow exhaustion；
- finished equity curve blending、independent account summation；
- KSS family。外部论文 KSS 5m DD 超过 160%，本轮不重试；
- 外部论文的固定选币、20k sizing、三周/一周重叠收益或事后最优 alpha；
- Round 24 current-hour close、temporary reserve、0.5/-0.5 label-only weights 和 single-family 100% fail gate。

每次 launch 前计算：

```text
mechanism_fingerprint = SHA256(
  engine_semantics + completed_bar_contract + copula_formula_version +
  signal_lag + fit_protocol + pair_matching + Martin_layer_schedule +
  exit_abort_rule + cost_model + filter_maintenance_model + account_model
)
```

旧 ledger fingerprint 只有在 engine/data/protocol 都相同且历史结果有效时才拒绝。Round 24 的 16 个 invalid
F1 fingerprints 不得阻止 corrected-engine replay，但仍计入 global trial floor。

## 3. R0：先修 Round 24，禁止先看收益

### 3.1 Git 与 append-only evidence

1. 从包含 Round 24 corrected authority 的 clean pushed commit 创建 `glm-martingale-core-round25`；
2. 立即 `git push -u origin glm-martingale-core-round25`；
3. 每个 replay parent 必须 clean、已 push、等于 upstream commit；
4. 每 phase 独立 commit/push；
5. 每个 commit body 必须含 `问题描述`、`复现路径`、`修复思路`；
6. 原 Round 24 artifacts/traces 只读，不覆盖；Round 25 使用独立 artifact root；
7. 继续复用并修复现有 `r24-engine/r24-registry/r24-research`，增加 `r25_*` modules/binaries；无必要不得复制三套新 crate。

### 3.2 九个必须先失败再修复的 regression canaries

```text
same_hour_close_used_for_same_timestamp_fill_is_rejected
g0_so_helper_not_called_by_scored_replay_is_rejected
fo_without_persistent_next_so_reserve_is_rejected
unrounded_order_or_missing_filter_version_is_rejected
single_family_100pct_is_not_a_concentration_failure
static_block_snapshot_is_not_dynamic_contribution_freeze
active_pairs_sharing_symbol_across_fit_roll_are_rejected
close_funding_legging_costs_missing_from_cost_ratio_are_rejected
block_dd_global_delta_or_pre_end_close_snapshot_is_rejected
```

validator 必须检查真实 traces/call-bound evidence，不接受 `passed=true`、字段存在或 synthetic helper 输出代替。

### 3.3 Registry 新增不可伪造字段

每个 experiment 仍必须正好一行 `running` 和一行 terminal，并从真实进程采集 Round 24 全字段，另加：

```text
signal_bar_open / signal_bar_close / signal_ready / earliest_fill / actual_fill
fit_cutoff / model_version / pair_graph_hash / copula_parameter_hash
raw_qty / rounded_qty / raw_price / rounded_price / filter_version
maintenance_model_version / maintenance / liquidation_buffer
reserved_next_so_by_group / reserved_close / pending_leg_reserve
gross_profit / all_in_cost / cost_components
family_count / family_gate_applicable
scored_call_path_hash / G0-to-G1 binding hash
```

terminal complete/failed gate 都必须有八类存在、非空、可解析 traces。失败 terminal 与 exact failure ledger
同一事务落盘；write failure 必须产生可恢复的 failed terminal，不得只剩 running。

## 4. R1：production-conservative shared account 修复

Python 只允许 fit copula/生成 immutable signal snapshot，不能计算 PnL、DD、margin 或 promotion metrics。
收益只由 Rust event engine计算。

### 4.1 Completed-bar 与成交边界

1. 1m bar 明确保存 `open_time` 和 `close_time`；
2. 5m/1h signal 只在最后一根 1m bar close 后 ready；
3. 最早在下一根 1m event 的 open 成交，再加 taker fee 与 adverse slippage；
4. signal timestamp、ready timestamp、order timestamp、fill timestamp 必须严格递增；
5. current 5m/1h bucket、future shifted bucket、用 hourly close 标 hour open 均 fail；
6. funding 按真实 timestamp 结算，不 round 到一个可能早于事件的 hour。

### 4.2 订单、filters 与维护保证金

1. 使用冻结的 Binance exchangeInfo 解析 tickSize、stepSize、minQty、maxQty、minNotional；
2. 每条 order 先 price/qty rounding，再重新计算 resolved gross、margin 和 concentration；
3. 历史 filter snapshot 不可获得时，不伪称完整历史。主回放使用 current snapshot 的保守约束，G2 加
   `2x minNotional + coarser step/tick` stress；
4. 主回放 maintenance 使用 `max(current symbol rate, 2.5%)` 的静态保守模型，G2 使用 5%；
5. partial fill、leg delay、one-leg reject、forced close 与 liquidation fee 真实改变 wallet/position/reserve；
6. paired legs 非原子成交时执行预注册 hedge-or-flatten，legging loss 进入 equity。

### 4.3 持续 reserve 与 concentration

每次 FO/SO fill 后立即重新预留：

```text
next SO initial margin
+ next SO entry cost
+ all-open-position close cost
+ maintenance buffer
+ pending delayed-leg reserve
```

任何新 FO 必须在上述所有 active groups reserves 后仍可支付。每个 event 强制：

```text
symbol reserved gross <=25% equity
group reserved gross <=25% equity
effective gross leverage <= active tier cap
```

positive contribution 实时超过 35% 时，该 symbol/group 只允许现有 cycle reduce/TP/abort，禁止新 FO；
降回阈值后按 lagged completed ledger 恢复。不得只在 block 开头拍一次 snapshot。

### 4.4 必过测试

除 Round 24 的 15 tests 外新增：

```text
completed_five_minute_close_fills_at_next_minute_open
funding_event_is_not_rounded_earlier
rounded_pair_gross_reconciles_to_trace
min_notional_rejects_both_group_legs_atomically
all_active_groups_keep_next_so_and_close_reserve
new_fo_cannot_consume_existing_next_so_reserve
dynamic_35pct_freeze_changes_real_order_hash
active_fit_roll_cannot_share_symbol_with_frozen_cycle
all_close_and_funding_costs_reconcile_to_wallet_delta
block_metrics_include_end_close_and_local_running_peak
single_family_contribution_is_informational
scored_replay_calls_same_so_guard_as_g0
```

R1 只跑 synthetic/accounting canaries，不重放旧 headline 当 candidate。

## 5. R2：冻结数据与历史 prequential 协议

### 5.1 Universe 与数据

固定：

```text
reference only: BTCUSDT
tradable alts: ETHUSDT, BNBUSDT, SOLUSDT, XRPUSDT,
               DOGEUSDT, LINKUSDT, LTCUSDT
market: Binance USD-M perpetual only
```

BTC 不计 actual traded assets。重建 8 币 1m perp klines、funding、current filter snapshot 和静态保守
maintenance manifest。每项记录 unique key、rows、min/max timestamp、gap histogram、sample hash、source URL、
archive/checksum。`missing >10m` 禁止新 FO/SO，active group 按 frozen stale rule reduce/abort。

### 5.2 主时间协议

```text
fit data start:  2023-01-01
tb01 test start: 2023-07-01
test end:        2026-05-31
12 contiguous outer test blocks
signal bars:     completed 5m / completed 1h
embargo:         one full signal bar plus next execution event
continuous account across tb01..tb12
```

本机制没有 forward label，禁止照抄 Round 24 的 121 天 purge。lookback 是向后读取范围，不等于 purge。
任何 fit/marginal/copula 只能读 `<= signal_ready - embargo`。

五个 cold starts 在收益 replay 前冻结：

```text
cs00=2023-07-01, cs30=2023-07-31, cs60=2023-08-30,
cs90=2023-09-29, cs120=2023-10-29
```

每个 cold start 独立初始化同一 frozen policy，连续运行到 2026-05-31。不得挑选或共享最终 equity。

每 block：

1. pair/model/threshold 只由 prior fit data确定；
2. no-fit/no-signal/reject/timeout/breach/empty 均保留在 12-block 分母；
3. 每块只报 raw return/local DD/PF，不报 ann；
4. selector roll 不改变 active group 的 open-time fit；
5. final stitched 才报 ann/DD；
6. policy、fit cutoff、pair graph、marginal、copula、transition cost 逐块落盘。

## 6. R3：来源映射、预注册和 trial ledger

任何收益 replay 前提交并 push：

```text
round25-source-map.json
round25-protocol.json
round25-data-manifest.json
round25-policy-manifest.json
round1-25-trial-ledger.json
round25-mechanism-fingerprints.jsonl
round25-cold-starts.json
```

来源 `10.1186/s40854-024-00702-7` 必须记录 PDF URL/hash、section、Eq.31-33、Tables 3/4/6 与本地字段映射。
来源 `10.1214/21-AOAS1568` 只映射 asymmetric-tail risk veto。不能采用的部分也必须列出。

trial 要求：

- Round 24 的 16 invalid replays 仍算历史选择尝试；visible floor 至少 `1096`；历史未知写 unknown；
- DSR sensitivity 使用 `10k/50k/100k/1m`；
- CSCV/PBO 使用全部 16 个同层 C1 policies，不用 symbols/cold starts/selected subset；
- 同时报 stationary/bootstrap CI、White/SPA-style reality check、neighbor stability、selection frequency；
- 公式单测必须对照独立 reference implementation。

## 7. C1：Reference-Asset Conditional-Copula Martin

### 7.1 Train-only group construction

每个 outer block：

1. 对每个 alt 拟合 `S_i = log(BTC) - beta_i*log(alt_i)`；
2. beta、均值、尺度只用 completed fit bars；
3. EG-ADF 与 KPSS 必须一致，另检查 half-life、break frequency、tail event count 和交易成本；
4. 对两条 stationary `S_i,S_j` 用 empirical CDF marginals 生成 `U_i,U_j`；
5. 只在 Gaussian 与 Student-t 两个低容量 copula 中按 train AIC 选择；t degrees-of-freedom 约束 `[3,30]`；
6. 计算 conditional probabilities `h(i|j), h(j|i)`；
7. 用 train-only stationarity、AIC、tail sample 与 cost feasibility 构造 maximum-weight disjoint matching；
8. 至少三组 disjoint alt pairs、六个 tradable assets，否则该 block `no_fit`；
9. BTC 只出现在 signal trace，order/trade/PnL trace 出现 BTC 立即 invalid；
10. outer returns 不得选 pair、copula family、frequency 或 threshold。

### 7.2 明确的 order sizing

为避免 Round 24 beta/weight 歧义：

- beta 只定义 reference-spread signal；
- 实际交易两只 alt，long/short quote gross 各占 group layer 的 50%；
- rounding 后两侧 gross 差 `<=5%`，否则整层 atomic reject；
- 一个 symbol 同时最多属于一个 active group；
- 每层使用 open-time frozen pair、direction、copula、threshold 和 equal-dollar order contract。

### 7.3 Martin cycle

设 alpha 为 entry tail，direction 为 `long i / short j` 或反向：

```text
FO:
  h(i|j) <= alpha AND h(j|i) >= 1-alpha
  OR exact mirror
  AND pair/model/stale/cost/reserve/concentration gates pass

SO:
  aggregate group net after estimated close cost < 0
  AND signed relative-spread adverse distance from last filled layer >= frozen SO step
  AND conditional probabilities remain in the same tail
  AND current one-bar adverse increment is no longer worsening
  AND rolling completed-bar break/stationarity gate remains valid
  AND shared reserve/filter/concentration gates pass

TP:
  aggregate group net after all fees/slippage/funding/legging/estimated close > 0
  AND both conditional probabilities return to frozen neutral band

ABORT:
  model break/stale >10m/deadline/liquidation buffer/one-leg failure
  -> paired reduce or close, realized loss enters equity
```

SO worsening guard 必须与 G0 调用同一 production function，不能复制两份逻辑。

### 7.4 固定 layer 与唯一开放维度

G1 `1000U` 固定 group FO gross `20U`，layer quote schedule：

```text
[20.0, 25.0, 31.0, 38.0] U
relative = [1.00, 1.25, 1.55, 1.90]
```

只开放：

```text
signal frequency:  completed 5m / completed 1h
fit lookback:      60d / 120d
entry alpha:       0.05 / 0.10
SO adverse step:   0.50 / 0.75 train sigma from last fill
```

共 `16` 个 policies。max groups=3、neutral exit band、deadline=7d、copula candidate set、layer、cost、
quota 都冻结，不搜索。不得根据 G1 结果再追加 alpha、copula family 或 TP。

### 7.5 三档 risk mapping

只有 G1 P-B survivors 才映射：

| profile | group FO gross | group gross cap | leverage cap |
|---|---:|---:|---:|
| conservative | `2% current equity` | `15% equity` | `2x` |
| balanced | `3% current equity` | `22.5% equity` | `3x` |
| aggressive | `4% current equity` | `25% equity` | `4x` |

这是三个预注册 profiles，不是 post-hoc sizing grid。rounding 后低于 min-notional 时整组不成交并影响 exact
minimum principal。

## 8. G0：机制真实性、公式与订单绑定

C1 在收益搜索前必须通过：

1. Gaussian/Student-t conditional CDF 与独立 reference 数值一致；
2. empirical CDF 只使用 fit values；
3. 8 synthetic traces：双尾、镜像、独立、强相关、tail event不足、stale、future shift、label swap；
4. 4 real causal windows：bull、bear、range、jump/liquidity shock；
5. 至少两个 real windows 有 `>=3` pairs 和真实 FO/rejection；纯 no-fit state delta 不算通过；
6. low/high config 必须改变 real order 或 real rejection hash；
7. signal close < order < fill；
8. 无 net loss 不出现 SO；worsening 时不 SO；last-fill 未成交不移动；
9. next layer gross 严增；BTC order count=0；
10. pair disjoint、dynamic 35% freeze、persistent reserve 在 event time 生效；
11. no-scheduler/static-first-N/deficit-round-robin 三臂必须改变真实候选 admission/order hash；
12. source/model label 本身不改变 PnL。

任一参数 inert 或 G0 只剩 no-fit，删除该 parameter/family 并写 failure ledger，禁止进入 G1。

## 9. G1：16 个冻结 policies 的连续回放

每个 policy 在 `1000U` 跑完整 12 blocks，独立进程、独立 shared account、八类 traces。

Immediate fail：

- current/future signal 或同 bar fill；
- block/cycle/account/running peak reset；
- liquidation/principal breach；
- actual assets `<6` 或 actual pairs `<3`；
- 无 loss-after-add SO，或少于两个 groups 出现 SO；
- symbol/group/block positive contribution `>50%`；
- all-in cost/gross profit `>50%`；
- family=100% 被错误当作 fail；
- BTC 产生 order/PnL；
- missing/stale fit、filter、funding、margin、reserve 或 trace；
- rounded gross、wallet delta、funding、cost ratio、DD 无法从 traces 独立复算。

进展门：

| Gate | 条件 |
|---|---|
| P-A | production-conservative replay + independent adapter parity + evidence chain 全过 |
| P-B | 12-block compounded `>0`、无 breach、`>=8/12` 正、全部 immediate gates 通过 |
| P-C | ann `>=35%`、DD `<=20%`、至少 4/5 cold starts 正、anti-overfit 全过 |
| P-D | 任一 50/90/100 档位与全部 hard gates 同时命中 |

只按 frozen stitched policy 排名。P-B 全失败立即停止 C1，不扩 grid，C2/G2 不适用。

## 10. C2：Asymmetric-Tail SO Veto，仅条件执行

只有 C1 parent 通过 P-B 才执行。C2 不创建订单，只 veto C1 的 FO/SO：

1. fit 内固定 `q=0.05`；
2. 分别估计 lower/upper empirical tail dependence；
3. 用 fit subwindows 冻结 relevant-tail stress threshold；
4. runtime 只用 completed rolling observations；
5. long leg 看 lower tail，short leg 看 upper tail；
6. stress 超阈值时 block FO、freeze SO 或 paired reduce，不能翻方向、加 leverage 或生成 PnL；
7. 只运行 `parent control` 与 `tail-veto on` 两臂，每个 parent 最多两个实验；
8. 必须有真实 order/rejection delta，若无则 `inert_enhancement` 并关闭 fingerprint。

C2 若降低 ann 但改善 DD，仍须独立通过同一 P-B/P-C/P-D，不能和 parent 事后拼曲线。

## 11. G2：budgets、cold starts、压力与防过拟合

最多四个 committed G1 survivors，加符合条件的 C2 variants。每个运行：

1. 全部 8 个 `<5000U` budgets；
2. 五个预注册 cold starts；
3. fee/slippage `1x/1.5x/2x`；
4. partial fills `25/50/75%`；
5. leg delays `1/2/3` 个 1m bars；
6. one-leg reject + hedge-or-flatten；
7. minNotional `1x/2x`、coarser rounding；
8. maintenance `2.5%/5%`；
9. funding `1x/1.5x/2x` 与 timestamp gaps；
10. signal missing/stale；
11. LOSO：删一 symbol 后 train-only refit，仍需三对六币；
12. LOGO：删一 group 只作 concentration diagnostic，不伪称满足六币 promotion gate；
13. kill/restart/reconcile；
14. exact minimum principal binary search，并复验上下相邻本金；
15. 单项 stress 后再跑一个预注册 worst-combined stress，禁止结果后组合压力。

至少两个相邻 budgets 通过；任一必测压力 liquidation 淘汰。

防过拟合输出必须包括：

```text
exact 16-policy population and global trial ledger
DSR sensitivity 10k/50k/100k/1m
full-matrix CSCV/PBO
stationary/bootstrap confidence intervals
White/SPA-style reality check
neighbor stability and selection frequency
12 block + symbol + group concentration
copula-family selection frequency and parameter drift
signal-frequency ablation
```

缺任一项写 `anti_overfit_not_auditable`，不得进 R8。

## 12. R8：三档最终判断

只有 committed G2 survivors 可判断。每个候选必须列：

```text
reference symbol and traded symbols
per-block pair graph/copula/parameters/signed legs
FO/SO/TP/reduce/abort counts and attribution
layer schedule and actual rounded quantities
effective leverage/peak gross/all reserves
exact minimum principal
fees/slippage/funding/legging/liquidation buffer
equity DD/balance DD/DDR
12 raw block returns and 5 cold-start returns
symbol/group/block concentration
DSR/PBO/SPA/global trials
adapter parity and trace hashes
```

三档只使用 `50/10`、`90/20`、`100/30`。不使用旧 110%，不把 reporting Sharpe 变成 hard gate。

## 13. 状态机

最终状态只能是：

```text
HISTORICAL_PREQUENTIAL_TARGET_HIT
HISTORICAL_PREQUENTIAL_FRONTIER_PROGRESS
VALID_HISTORICAL_PREQUENTIAL_NO_TARGET
BLOCKED_ENGINE_DATA_OR_EXECUTION
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

- TARGET_HIT 只在 P-D 与全部 hard gates 通过；
- FRONTIER_PROGRESS 只在 P-C 通过；
- VALID_NO_TARGET 要求所有适用 phases、配额和 evidence 完成；
- R0/R1/G0 失败为 invalid/blocked，禁止继续收益搜索；
- 0 survivor 不把条件 phases 写 complete，只能 not_applicable；
- handoff 必须由 validator 从 registry/traces 生成；
- 禁止写“所有马丁可能性耗尽”，只能关闭 exact valid fingerprint；
- 每个失败必须有 exact never-repeat row。

## 14. 必交产物

```text
docs/superpowers/artifacts/glm-martingale-core-round25/round25-authority.json
docs/superpowers/artifacts/glm-martingale-core-round25/round25-execution-state.json
docs/superpowers/artifacts/glm-martingale-core-round25/exploration-registry.jsonl
docs/superpowers/artifacts/glm-martingale-core-round25/failure-ledger.jsonl
docs/superpowers/artifacts/glm-martingale-core-round25/round25-source-map.json
docs/superpowers/artifacts/glm-martingale-core-round25/round25-protocol.json
docs/superpowers/artifacts/glm-martingale-core-round25/round25-data-manifest.json
docs/superpowers/artifacts/glm-martingale-core-round25/round25-policy-manifest.json
docs/superpowers/artifacts/glm-martingale-core-round25/round1-25-trial-ledger.json
docs/superpowers/artifacts/glm-martingale-core-round25/gates/*.json
docs/superpowers/artifacts/glm-martingale-core-round25/traces/**
docs/superpowers/reports/2026-07-XX-glm-round25-execution-handoff.md
```

raw market archives 和论文 PDF 不进 Git；URL/bytes/SHA256/schema/section map 必须提交。

## 15. GLM 严格执行顺序

- [ ] R0 branch/upstream/append-only registry/domain canaries
- [ ] R1 completed-bar/filter/rounding/reserve/cost/concentration engine repairs
- [ ] R1 old 15 + new 12 tests and independent adapter parity
- [ ] R2 data/protocol/cold starts freeze
- [ ] R3 source map/16 policies/global trial ledger/fingerprints freeze + push
- [ ] C1 conditional-copula fit/signal implementation
- [ ] G0 formula/causality/real order-delta/binding gate
- [ ] G1 exact 16 continuous 12-block replays
- [ ] commit/push G1 survivors
- [ ] C2 only for eligible P-B parents
- [ ] G2 all budgets/cold starts/stresses/anti-overfit/min principal
- [ ] R8 latest three-tier judgment
- [ ] validator-generated authority/state/handoff
- [ ] final tests/commit/push/clean worktree

前置失败时先修复并重跑同一 gate。禁止越过失败项、扩参数、换 family 或用 handoff 文字宣布 complete。
