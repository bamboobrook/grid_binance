# GLM Martingale Core Round 23：真实执行修复、订单流拥挤反转与动态因子组合计划

制定日期：2026-07-22。

本文件是 Round 23 唯一任务书。GLM 必须直接历史回测，不设置 30 天自然日监控，不创建 future lock，
不等待新增行情。

## 0. 唯一前置权威

只允许读取以下 Round 22 修正文件：

- `docs/superpowers/artifacts/glm-martingale-core-round22/round22-corrected-authority.json`
- `docs/superpowers/artifacts/glm-martingale-core-round22/audit/round22-independent-audit.json`
- `docs/superpowers/artifacts/glm-martingale-core-round22/audit/round22-corrected-failure-ledger.jsonl`
- `docs/superpowers/artifacts/glm-martingale-core-round22/audit/round23-external-source-probe.json`
- `docs/superpowers/reports/2026-07-22-chatgpt-round22-execution-audit-and-correction.md`

禁止继承：

- `31.2%/9.9%` xsection 前沿；
- `5/5 cold starts`；
- `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET`；
- “21 mechanisms/all data sources exhausted”；
- Round 22 G1/G2 的 top10、positive count 或 complete phase。

它们都不是严格有效结果。

## 1. 不变目标与 Martin 硬合同

| 档位 | stitched annualized | max equity DD | 5 cold starts | effective gross leverage cap |
|---|---:|---:|---:|---:|
| 保守 | `>=50%` | `<=10%` | `>=4/5` 正，另报 5/5 | `<=2.0x` |
| 平衡 | `>=90%` | `<=20%` | `>=4/5` 正，另报 5/5 | `<=3.0x` |
| 激进 | `>=110%` | `<=30%` | `>=3/5` 正，另报 5/5 | `<=4.0x` |

共同 hard gates：

1. principal 只允许 `500/750/1000/1500/2000/3000/4000/4999U`，必须 `<5000U`；
2. 一个连续 shared cash/margin/equity account，block 边界不得重置本金；
3. 每个 target policy 实际成交 base assets `>=5`；
4. 单 symbol、group 和单 test block 的正收益贡献均 `<=50%`，另报是否 `<=35%`；
5. 每个计分 timeline 必须存在真实 loss-after-add SO；
6. 每个 SO 前 `group net PnL after estimated close cost < 0`；
7. 下一层 paired/basket gross 必须严格大于上一层；
8. SO adverse distance 必须从“上一次已成交 paired/basket fill”计算，不能每 bar 重置；
9. 所有收益只能来自 Martin cycle 的 FO/SO/TP/abort；
10. OI、long/short ratio、taker flow、depth、factor、Johansen、ML 只能决定 FO/SO/TP/abort/freeze；
11. 禁止独立 trend、carry、market-making、fixed-fractional、anti-Martingale 或理论 PnL sleeve；
12. 任一 liquidation、equity `<=0`、future/stale signal、filter bypass、NaN、缺 order trace 立即淘汰；
13. equity DD、balance DD、DDR 同报，目标只读 equity DD；
14. exact minimum principal 必须包含 spot cash、perp initial/maintenance margin、fees、close 和 next-SO reserve；
15. 只有连续 test days `>=365` 的 stitched metrics 可判断年化目标。

这里的“多空”是每个可执行 Martin group 内真实 signed legs；signal 本身不产生收益。

## 2. 本轮禁止重复

以下机制已失败或不合格，不得再次换名扫参数：

- 普通 multiplier/spacing/max-legs/TP grid；
- generic EMA/ADX/RSI/HTF trend gate；
- ordinary cross-sectional momentum/reversal；
- same-symbol long+short hedged grid；
- standalone funding carry/reversal；
- DGT、breakout、fixed-fractional、普通 DD scaling；
- 旧 first-passage/hazard、last-executed SO basis、minigrid partial TP；
- 单币 SSRN `5935414` 的标题推测实现；
- Round 22 Python PnL engine；
- 每 block 选赢家、事后挑五个正窗口、短窗年化；
- 用 zero-cost 或当前 block 结果证明 signal ceiling。

修复“从未产生严格有效行”的 PC1/V1B/Johansen 不算重复，但必须使用真实可执行 legs，不得复用旧结果。

## 3. 外部来源与采用边界

| 来源 | 本轮采用 | 禁止外推 |
|---|---|---|
| Binance Public Data：`https://github.com/binance/binance-public-data` | 官方日/月 archive、sidecar CHECKSUM | 不因官方数据就假设 signal 有收益 |
| `10.1093/jjfinec/nbt003` | depth-normalized order-flow state | contemporaneous impact 不是未来收益 |
| `10.1142/S2382626619500114` | multi-level depth/flow vector | 股票结论不直接搬到 crypto |
| `10.2139/ssrn.6938742` | aggressor-side crypto flow、OOS/cost/data-snooping protocol | 该文是 review/design，没有新收益结果 |
| `10.1007/s10203-021-00318-x` | 仅第二动态因子平稳时启用多对组合 | 不复制四币论文收益 |
| `10.1108/SEF-08-2018-0264` | Johansen/EG 多币 executable basket | 不按 outer PnL 选 basket |
| `10.1007/s10614-025-10958-5` | train-only threshold range classifier | outer return 不得作为 label/feature |
| `10.2139/ssrn.5895159` | Micro-Martingale/Integral TP 待公式核验 | 无正文页码和公式不得猜实现 |

对每篇正文必须提交：URL、文件 SHA256、公式/算法页码、字段映射、不可采用部分。只有 metadata 的来源只能
登记为 hypothesis，不能称 exact implementation。

## 4. R0：先建立不可绕过的证据链

### 4.1 Git

1. 从包含 Round 22 corrected authority 的 clean/pushed commit 创建 `glm-martingale-core-round23`；
2. 立即 `git push -u origin glm-martingale-core-round23`；
3. 每次 replay 的 parent commit 必须已经在远端；
4. dirty tree、无 upstream、短 hash 或未 push commit 均禁止运行；
5. 每 phase 单独 commit/push；
6. 每个 commit body 必须同时包含：`问题描述`、`复现路径`、`修复思路`。

### 4.2 Central launcher/registry

每个 immutable `experiment_id` 必须正好有一行 `running` 和一行 terminal：

```text
git_commit/git_dirty/upstream_remote_commit
full SHA256: binary/source/market/metrics/depth/aggTrades/funding/filter/
             maintenance/borrow/cost/config/fit/protocol
raw argv/pid/exit/wall/RSS/start/end/fit/purge/replay timestamps
full trace SHA256: event/trade/order/equity/funding/rejection/signal/margin
```

non-complete terminal 必须原子写 failure ledger。registry、ledger 或 trace append 失败时，实验本身失败。

### 4.3 必须真的注入失败 canary

不能只写检查函数。自动测试必须注入并断言拒绝：

1. empty registry + downstream result；
2. dirty/unpushed commit；
3. 63-char hash；
4. reused experiment ID；
5. 缺 running 或 terminal；
6. null order/signal/margin trace；
7. config/argv budget mismatch；
8. G2 无 committed G1 parent；
9. 90-day ann 进入 target；
10. 删除一个 manifest block；
11. test 后选择 cold starts；
12. summary replay count 与 registry 不同；
13. selector 状态为 `planned`；
14. future/stale metrics/depth；
15. terminal 无 failure row。

R0 未过，状态必须是 `BLOCKED_ENGINE_DATA_OR_EXECUTION`，禁止搜索。

## 5. R1：修 scored replay，禁止 Python 自算 PnL

本轮 signal/fit 可由 Python 生成，但 scored PnL 必须由同一个 production-conservative Rust runtime 产生。

### 5.1 必修账务

1. position 只存 signed quantity、fill price、market identity；
2. 每次 fill 后断言 `sum(abs(qty*price)) == resolved_group_gross`（允许 exchange rounding tolerance）；
3. beta/factor weights 先归一化，再按 quote gross 分配，禁止 `FO * price_a/price_b`；
4. spot cash、perp wallet、inventory、initial/maintenance margin 共用一个账户；
5. event-time 每个 mark 检查 Binance maintenance tier 和 liquidation fee；
6. reserve 同时覆盖所有 active groups 的 next SO、close cost、maintenance buffer；
7. FO/SO/TP/abort/rebalance/end-close 全部收费并结算 funding/borrow；
8. balance 与 mark-to-market equity 分开；
9. block 换 selector 时保守平仓并计成本；active group 中途不得换 legs/weights；
10. 任何 principal breach 立即终止，不允许 replay-end 才检查。

### 5.2 必修执行路径

1. 每个 order 生成 `OrderIntent`，经过真实 filter rounding 后进入 order trace；
2. base fill path 与 25/50/75% partial-fill stress path 都调用主账户代码；
3. 后腿 delay `1/2/3 bar`、reject、hedge-or-flatten，legging loss 入 equity；
4. temporary cooldown、permanent freeze、kill/restart/reconcile 持久化；
5. backtest adapter 与 trading-service adapter 分别连接独立 fake exchange；
6. 禁止两个 adapter 调同一 helper 后把 hash 相等称为 parity；
7. liquidation/partial-fill/legging count 必须从 canonical events 重算。

### 5.3 R1 定向测试

至少包含：

```text
pair_gross_is_dimensionally_equal_after_rounding
four_leg_factor_basket_gross_equals_resolved_gross
long_cycle_so_only_on_adverse_loss
short_cycle_so_only_on_adverse_loss
last_fill_basis_does_not_move_without_fill
fo_rejected_when_next_so_close_maintenance_reserve_missing
event_time_liquidation_terminates_shared_account
partial_fill_changes_inventory_cash_and_equity
leg_delay_books_realized_legging_loss
block_transition_never_resets_cash_or_equity
independent_adapters_match_order_ack_reject_equity_hashes
pseudo_symbol_pc1_is_rejected
```

再把 Round 22 `budget500/m3/fo120` 作为**非候选算术 canary**重跑：只用于证明旧 `31.2/9.9` 被撤销，
不得进入搜索、排名或 trial winner。

## 6. R2：冻结数据与连续 prequential 协议

沿用 12-block 主协议：

```text
fit start: 2023-01-01
tb01 test start: 2023-07-01
test end: 2026-05-31
purge: >= max feature lookback + source publication latency + one completed execution bar
one continuous account across tb01..tb12
```

五个 cold starts 在任何收益 replay 前冻结为：

```text
cs00 = 2023-07-01
cs30 = 2023-07-31
cs60 = 2023-08-30
cs90 = 2023-09-29
cs120 = 2023-10-29
```

每个 cold start 都独立初始化同一 policy，并连续运行至 2026-05-31；不得筛选、替换或共用最终 equity。

每个 block：

1. fit 只读 `<= test_start - purge`；
2. block 当前/未来 return 不得进入 threshold、pair、weight 或 risk schedule；
3. no-fit/no-signal/reject/timeout/breach 保留在 12-block 分母；
4. 每块只报 raw return/DD/PF，不报 ann；
5. 最终 stitched test days `>=365` 才报 ann/DD；
6. policy hash、fit cutoff、signal snapshot hash、pair/basket weights、transition cost 全部逐块落盘。

## 7. R3：获取新历史数据，解决旧 30 天阻塞

### 7.1 冻结 universe

主 universe：

```text
BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT,
XRPUSDT, DOGEUSDT, LINKUSDT, LTCUSDT
```

`aggTrades` 高体量子集固定为：

```text
BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT, XRPUSDT, DOGEUSDT
```

不得根据 outer return 换币。任一文件缺失保留 missing 状态；不得事后用表现更好的币替代。

### 7.2 Binance Vision 数据

下载 `2023-01-01..2026-05-31`：

```text
data/futures/um/daily/metrics/<SYMBOL>/
data/futures/um/daily/bookDepth/<SYMBOL>/
data/futures/um/daily/aggTrades/<SYMBOL>/   # 仅固定 6 币
```

已验证的 schema：

```text
metrics:
create_time,symbol,sum_open_interest,sum_open_interest_value,
count_toptrader_long_short_ratio,sum_toptrader_long_short_ratio,
count_long_short_ratio,sum_taker_long_short_vol_ratio

bookDepth:
timestamp,percentage,depth,notional
```

要求：

1. 下载每个 `.zip` 和 `.CHECKSUM`；
2. sidecar 校验通过后再写 immutable manifest；
3. raw archive 不进 Git，只提交 URL/size/SHA/time range/schema manifest；
4. archive 更新导致 hash 变化时整轮 invalid，不可静默覆盖；
5. metrics/bookDepth 至少延迟一个完整 snapshot；aggTrades 至少延迟一个 completed aggregation bar；
6. duplicate/out-of-order/future timestamp 立即 fail；
7. missing `>10m` 时禁止 FO/SO；active cycle 按 frozen conservative flatten rule 处理；
8. ingestion 前后 row count、min/max timestamp、gap histogram、sample hash 全部提交。

R3 只验证数据与 causal alignment，不看策略 return。

## 8. R4：三个新的 Martin family

三个 family 共用同一 Martin runtime 和 Soft Ladder：

```text
relative layer gross = [1.00, 1.25, 1.55, 1.90]
SO = group net loss + adverse distance from last executed fill + family confirmation + reserve pass
TP = aggregate group net positive after estimated close cost
```

任何代码仍出现 `multiplier ** depth`，E0 直接失败。

三档 sizing 不再开放普通 FO grid：

| profile | resolved FO basket gross | max live groups | leverage cap |
|---|---:|---:|---:|
| conservative | `max(sum leg min-notional, 6% * budget)` | 3 | 2x |
| balanced | `max(sum leg min-notional, 10% * budget)` | 3 | 3x |
| aggressive | `max(sum leg min-notional, 14% * budget)` | 4 | 4x |

`sum leg min-notional` 必须基于当时 exchange filter 和 rounding 逐腿计算。若完整 ladder、close 和 maintenance reserve
放不下，拒绝该 FO，禁止缩短已冻结 ladder 或偷偷提高 leverage。

### M1：OI/Crowding Exhaustion Martingale

只用已完成 5m snapshot，计算 fit-frozen rolling robust state：

```text
oi_change   = delta(log(sum_open_interest_value))
taker_flow  = log(sum_taker_long_short_vol_ratio)
top_crowd   = log(sum_toptrader_long_short_ratio)
all_crowd   = log(count_long_short_ratio)
price_ext   = log_price - rolling_median(log_price), scaled by MAD
```

所有 z/quantile 只由过去窗口计算。

Long FO 必须同时满足：

1. price downside extension；
2. OI expansion；
3. taker sell imbalance extreme；
4. top/all crowd 偏 short；
5. taker sell pressure 已开始衰减，而不是仍加速。

Short FO 完全镜像。SO 仍必须在持仓亏损且价格继续 adverse 后触发，并要求 flow/crowd 出现 exhaustion；若 OI 和
taker flow 继续同向恶化，则 freeze SO 或 aggregate abort，禁止盲目摊平。

只开放：

```text
robust window = 7d / 30d
tail quantile = 95% / 97.5%
adverse spacing = 1.0 / 1.5 train ATR
risk profile = conservative / balanced / aggressive
```

去重后的 policy quota `<=32`。

### M2：Depth Replenishment + Aggressor Flow Martingale

depth imbalance：

```text
I(level) = (bid_notional(-level) - ask_notional(+level)) /
           (bid_notional(-level) + ask_notional(+level))
levels = 0.2%, 1.0%
```

aggTrades signed flow 使用 aggressor side，不得从当前 bar future close 推断。

Long FO：价格 downside extension + sell aggressor flow 衰减 + bid depth replenishment；Short 镜像。SO 必须同时满足：

- group net loss；
- 从 last fill 起达到 adverse spacing；
- 对应 side depth replenishment；
- aggressor flow 不再加速 adverse。

若 depth 消失或 spread/depth stress 超门，禁止 add 并 flatten。该 family 不做独立 market making，也不把
book impact 当作收益。

只开放：

```text
depth state = 0.2% only / median(0.2%,1.0%)
flow window = 15m / 60m
tail quantile = 95% / 97.5%
risk profile = conservative / balanced / aggressive
```

policy quota `<=24`。

### M3：Executable Stationary Dynamic-Factor / Johansen Basket Martin

先从论文正文提取公式并提交 source map，然后实现两个预注册 sibling：

#### M3-DFA

1. train-only dynamic factor fit；
2. ADF 与 KPSS 对第二 common factor 的平稳性结论必须一致；
3. 只有 factor stationary/stable 时允许 FO；
4. factor 必须映射成 4-6 个真实 Binance legs，不允许伪 `PC1` symbol；
5. `sum long gross` 与 `sum short gross` 差 `<=5%`；
6. 单 leg abs weight `<=25%`；
7. SO/TP 对整组同一 frozen weights 执行。

#### M3-JOH

1. train-only Johansen rank + sparse executable vector；
2. penalty 只能由 blocked inner CV 的 residual stability/cost 选择；
3. outer PnL 不得选 rank/weights；
4. 至少 4 个非零 legs，最多 6 个；
5. break 时禁止 FO、freeze SO，并按预注册 aggregate abort。

只开放 factor rank、4/6 legs、entry z `1.5/2.0` 和三档 risk profile，合计 policy quota `<=24`。

## 9. R5：G0 activation，未过禁止收益搜索

每个 family/参数必须在以下 synthetic paths 产生预期 order/state delta：

```text
range mean reversion
one-way trend
OI expansion then unwind
OI expansion without unwind
sell-flow exhaustion / buy-flow exhaustion
bid/ask depth replenishment
depth withdrawal
stationary factor / integrated factor / rank break
stale/future metrics
partial fill / leg reject / liquidation
```

必须证明：

- M1/M2 long/short 对称激活；
- 无 loss 不出现 SO；
- adverse 持续但无 exhaustion/replenishment 时不出现 SO；
- next layer gross 严格增加；
- factor pseudo leg 被拒绝；
- 每个开放参数至少改变一个 canonical signal/order hash；
- signal 整体向未来平移一 bar 必须被 causal gate 拒绝。

inert、label-only 或 deferred 参数从 manifest 删除并写 failure ledger，不能进入 G1。

## 10. R6：可选 threshold transfer 与 E1

### 10.1 M4 threshold-range controller

只有 M1 或 M2 parent 已在冻结 G1 manifest 前通过 `P-B`，才允许执行。使用
`10.1007/s10614-025-10958-5` 的方法框架，把 inner-only 的 mean/variance/skew/kurtosis/VaR/correlation
映射到三个预注册 SO/TP threshold bins。

- label 只能来自 earlier inner blocks；
- 当前/未来 outer block 不能训练；
- classifier 失败时回退到 frozen parent，不得按 outer winner 选 bin；
- quota `<=12`，并计入全部 trial correction。

### 10.2 E1 Micro-Martingale + Integral TP

GLM 可尝试取得 `10.2139/ssrn.5895159` 正文。只有同时提交：

```text
source URL + PDF SHA256 + formula page + verbatim variable map + pseudocode
```

才允许 `<=12` 个 policies。否则记 `blocked_unverified_source`，不得根据标题或摘要发明实现，也不阻塞 M1-M3。

## 11. G1：立即连续历史回测

冻结并 push manifest 后，每个 policy 在 `500/1000/4999U` 上运行完整 12-block continuous timeline。

Immediate fail：

- 任一 block 被删除或账户重置；
- liquidation/principal breach；
- actual assets `<5`；
- 无 loss-after-add SO；
- symbol/group/block positive contribution `>50%`；
- cost/gross profit `>50%`；
- 缺 trace/hash/policy cutoff；
- stale/future metrics/depth；
- resolved gross/leverage/reserve 不可复算。

只按 stitched policy 排名，不得按 block 选 top-N。每 family 最多 4 个 policy 进入 G2。

进展门：

| Gate | 条件 |
|---|---|
| P-A | production-conservative main replay + independent adapter parity 全过 |
| P-B | 同一 policy 12-block compounded return `>0`、无 breach、`>=8/12` 正、贡献门通过 |
| P-C | stitched ann `>=35%`、DD `<=20%`、至少 4/5 cold starts 正、trial correction 通过 |
| P-D | 任一用户三档完整命中 |

G1 不过 P-B 时，禁止看结果后扩 grid；只记录失败并进入 handoff。

## 12. G2：全部本金、五起点与实盘压力

对 committed survivors 运行：

1. 全部 8 个 `<5000U` budgets；
2. 预注册 5 cold starts；
3. fee/slippage `1x/1.5x/2x`；
4. partial fills `25/50/75%`；
5. leg delay `1/2/3 bars`；
6. one-leg reject + hedge-or-flatten；
7. filter/maintenance tier change；
8. metrics/depth missing/stale；
9. LOSO（逐 symbol）与 LOGO（逐 group）；
10. kill/restart/reconcile；
11. 订单最小名义金额和数量 rounding；
12. exact minimum principal binary search 后再以相邻本金复验。

至少两个相邻 budgets 通过。任何压力 liquidation 淘汰。

防过拟合必须输出：

```text
all Round 1-23 trial count
Deflated Sharpe Ratio
CSCV/PBO
stationary/bootstrap confidence interval
White/SPA-style reality check or documented equivalent
neighbor stability
policy selection frequency
block/symbol/group contribution concentration
```

未过 multiple-testing correction，不得进入 R8 selection。

## 13. R8：三档与组合

单个 policy 本身必须是多币组合。只有至少两个不同 mechanism 的 policy 独立通过 G2，才允许最多 12 个
event-level combinations：

- 一个 shared cash/margin/reserve；
- family gross `<=40%`；
- group gross `<=25%`；
- symbol gross `<=25%`；
- fit-only equal-risk，不按 outer PnL 配权；
- active overlap 合并风险和 reserve；
- 一个 family 失败不得由另一个 family 的 theoretical PnL 填补。

三档 target 必须由完整 stitched timeline + 五个预注册 cold starts + G2 stress 同时判定。

每个 top candidate 必须列出：

```text
symbols and per-block signed weights
long/short legs
FO/SO/TP/abort counts and attribution
layer gross schedule
effective leverage and peak gross
exact minimum principal
fees/slippage/funding/legging/liquidation buffer
equity DD/balance DD/DDR
12 raw block returns
5 cold-start returns
symbol/group/block concentration
DSR/PBO/trial count
live adapter parity hashes
```

缺任一字段，candidate 无效。

## 14. 状态机与 handoff

最终状态只能是：

```text
HISTORICAL_PREQUENTIAL_TARGET_HIT
HISTORICAL_PREQUENTIAL_FRONTIER_PROGRESS
VALID_HISTORICAL_PREQUENTIAL_NO_TARGET
BLOCKED_ENGINE_DATA_OR_EXECUTION
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

约束：

- `TARGET_HIT` 只在 P-D 和全部 hard gates 通过时使用；
- `FRONTIER_PROGRESS` 至少通过 P-C；
- `VALID_NO_TARGET` 只能描述本轮 committed manifest，不得写“所有可能性耗尽”；
- engine/data/registry 未过必须 blocked/invalid；
- handoff 必须由 validator 从 registry/traces 自动生成；
- 所有失败同步进入 never-repeat ledger；
- 不创建 30 天监控，不把 historical backtest 写成 live/future OOS。

最终文件：

```text
docs/superpowers/artifacts/glm-martingale-core-round23/round23-execution-state.json
docs/superpowers/artifacts/glm-martingale-core-round23/round23-authority.json
docs/superpowers/artifacts/glm-martingale-core-round23/exploration-registry.jsonl
docs/superpowers/artifacts/glm-martingale-core-round23/failure-ledger.jsonl
docs/superpowers/reports/2026-07-XX-glm-round23-execution-handoff.md
```

## 15. GLM 执行顺序

- [ ] R0 branch/upstream/launcher/injected canaries
- [ ] R1 Rust main replay 账务与 independent parity 修复
- [ ] R2 protocol + five cold starts freeze/commit/push
- [ ] R3 metrics/bookDepth/aggTrades ingestion + checksummed manifest
- [ ] R4 source maps + M1/M2/M3 exact implementations
- [ ] R5 G0 synthetic/real activation and inert-parameter removal
- [ ] G1 frozen 12-block continuous replays
- [ ] G1 survivor manifest commit/push
- [ ] G2 budgets/cold starts/stress/LOSO/LOGO/trial correction
- [ ] R8 target/combination judgment
- [ ] validator-generated handoff
- [ ] final commit/push and clean worktree

任何前置项失败时，先修复并重跑对应 gate；禁止跳过后用 handoff 文字宣布 complete。
