# GLM Martingale Core Round 21：真实执行引擎、Cross-Fit 与四机制收敛计划

制定日期：2026-07-20。

唯一前置权威：

- `docs/superpowers/artifacts/glm-martingale-core-round20/round20-corrected-authority.json`
- `docs/superpowers/artifacts/glm-martingale-core-round20/audit/round20-independent-audit.json`
- `docs/superpowers/artifacts/glm-martingale-core-round20/audit/round20-corrected-failure-ledger.jsonl`
- `docs/superpowers/reports/2026-07-20-chatgpt-round20-execution-audit-and-correction.md`

本文件是 Round 21 唯一执行任务书。禁止从 GLM Round 20 handoff 继承 candidate、survivor、complete phase 或
`VALID_SEARCH_NO_FRONTIER_PROGRESS`。Round 20 所有结果只可作为 diagnostic/bug fixture。

## 0. 本轮目的与状态上限

本轮不是继续扩大普通 spacing/multiplier 网格，而是把 Round 20 实际未执行的机制做成真实 event path：

1. 建立 spot/perpetual 分腿、shared cash/margin、filters/liquidation/partial-fill/legging 的统一回放；
2. 实现四个运行时可区分、仍以亏损后加仓为核心的多币 Martingale family；
3. 使用 rolling causal fit、purged cross-fit 和完整 trial ledger 抗过拟合；
4. 在 `<5000U` 下寻找三档 research finalist，并等待未来窗口作一次性确认。

2023-01-01..2026-05-31 已被前 20 轮反复读取，不再是 untouched OOS。`2026-07-11+` 在数据满 30 天且
selection 已 commit/push 前继续封存。因此本轮当前最高状态是：

```text
CROSS_VALIDATED_RESEARCH_FINALIST
```

只有 future lock 满足后才允许 `TARGET_HIT_PROVISIONAL_FUTURE_OOS`。不得承诺本轮必然命中收益目标。

## 1. 不变目标与 Martingale 定义

| 档位 | portfolio ann | max equity DD | cold starts |
|---|---:|---:|---:|
| 保守 | >=50% | <=10% | >=4/5 正，另报 5/5 |
| 平衡 | >=90% | <=20% | >=4/5 正，另报 5/5 |
| 激进 | >=110% | <=30% | >=3/5 正，另报 5/5 |

共同硬门：

- principal 必须 `<5000U`，搜索 `500/750/1000/1500/2000/3000/4000/4999U`；
- 实际成交 base assets `>=5`，不是配置数量；
- max symbol/group abs net PnL share 均 `<=50%`，另报 `<=35%`；
- 每个 scored window 必须有真实 SO，最终报 SO-cycle 占比与 PnL attribution；
- equity DD、balance DD、DDR 同时报，equity DD 是唯一目标 DD；
- exact minimum executable principal 包含 spot cash、perp margin、maintenance、close cost 与 next-SO reserve；
- 无 liquidation、principal breach、stale leg、NaN、future bar、filter bypass 或未建模 PnL。

每个 family 必须满足：FO 后 aggregate cycle net PnL `<0`，价格/residual/basis 继续 adverse，且 reserve 与
liquidation gate 通过，才允许下一层；下一层 paired gross 必须大于上一层。所有收益只来自这些 Martingale
cycle 的真实成交，指标/模型只能 gate/route/size/abort，不能加入独立 alpha 曲线。

## 2. R0：中央证据链，未过禁止写策略

创建 `docs/superpowers/artifacts/glm-martingale-core-round21/`。中央 launcher 是唯一允许 spawn replay binary
的代码；CI 静态扫描其他 `glm_r21_*` 出现 `subprocess.run.*synchronized_cycle_replay` 必须失败。

每个 experiment 必须先写 running、后写 terminal：

```text
experiment_id / parent_id / family / fold / block / budget / seed
canonical fingerprint / raw argv / pid / start/end / exit code / wall/RSS
git commit + dirty flag
engine/data/funding/filter/maintenance/borrow/fit/config/cost hashes
fit_start/end + purge + replay_start/end
event/trade/order/equity/funding/rejection stream hashes
full metrics + segment metrics + first failed gate
```

validator 必须实际注入并通过以下 canary：

1. central 0 行 + checkpoint 1 行 => blocked；
2. terminal 无 running/raw argv/exit/hash => invalid；
3. config budget 与 argv budget 不同 => invalid_budget；
4. duplicate `(venue,market_type,symbol)` leg => invalid_market_identity；
5. fit_end >= replay_start-purge => invalid_data_leakage；
6. family label 与 runtime trace family 不同 => invalid_mechanism；
7. G2 ID 不在 committed G1 manifest => blocked；
8. validation query 时间不晚于 pushed selection commit => invalid_oos;
9. symbol pass 但 group concentration fail => rejected；
10. registry 配额少一行 => incomplete，不得 complete。

每个 phase state 只由 validator 从 central rows 重算。GLM 不得手写 complete JSON。

## 3. R1：历史去重与数据合同

递归读取 Round 1-20 corrected ledger、config、checkpoint 和 registry，生成 canonical fingerprint index。必须
显式列出 Round 15-18 的 OU/hazard/first-passage、Round 19-20 residual/P1/K1/V1/basis 标签，分类为：

```text
valid_executed / invalid_mechanism / invalid_budget / invalid_leakage
planned_never_executed / exact_duplicate_forbidden / changed_engine_control
```

同一机制只有 engine/data/fit/market identity/cost/selection 合同实质改变才可重跑。普通 grid multiplier、
last-executed basis、HTF/ADX/EMA gate、DD scaling、DGT、breakout/trend 独立 sleeve、standalone funding carry、
curve allocator 全部禁止重复。

数据 loader key 必须是 `(venue, market_type, symbol, timeframe)`，至少区分：

```text
binance / spot / BTCUSDT / 1m
binance / futures_usdt_perp / BTCUSDT / 1m
```

冻结每个 source 的 schema、row count、min/max、duplicate、missing-minute、OHLC、funding settlement、premium、
borrow availability 和 provenance hash。缺 borrow 数据时 reverse basis 禁止；不得把缺值当 0 或 forward-fill 下单。

## 4. R2：真实 multi-market conservative engine

先把 `exchange_model.rs` 接入 synchronized replay 和 fake-exchange adapter。所有 family 共用一个账户状态：

```text
spot free/locked cash + inventory
perp wallet/equity/initial/maintenance margin
open orders + partial fills + fees + funding + borrow
shared reserve + liquidation buffer + terminal close
```

必须实现并测试：

1. spot/perp 同 symbol 是两个 MarketLegId，不被 set 去重；
2. PRICE_FILTER、LOT_SIZE、MARKET_LOT_SIZE、MIN_NOTIONAL/NOTIONAL 使用冻结 snapshot；
3. quantity/price rounding 后重新检查 paired hedge 与 reserve；
4. maintenance tier、mark-price floating equity 与 conservative liquidation；
5. 25/50/75% partial fill、后腿 delay/reject、hedge-or-flatten legging loss；
6. FO/SO/TP/abort/end-close 全收费，funding 在精确 settlement 计入对应 cycle；
7. next SO + close fee + maintenance 预留后才准新 FO；
8. temporary reject cooldown 与 permanent config freeze 进入持久状态；
9. kill/restart/reconcile 后 client order id、cycle depth、fills、reserve 幂等；
10. backtest adapter 与 fake exchange 的 order/rejection/equity suffix hash exact；
11. 64 并发同输入 trace hash 一致；
12. `min_liquidation_buffer_pct` 非 null，且由 event ledger 重算。

Ideal atomic path 只作为 upper-bound control，不得进入候选排名。R2 任一测试失败，整轮状态为
`BLOCKED_ENGINE_DATA_OR_EXECUTION`，禁止开始 family fit 或搜索。

## 5. R3：rolling causal fit 与 cross-fit

旧 outer validation 已污染，改用预注册 rolling-origin cross-fit。日期必须在第一次加载行情前写入 manifest 并
commit/push，之后禁止改：

```text
fit >=180d expanding/rolling
purge >= max(signal lookback, funding latency, 1 completed decision bar)
test 90d non-overlap blocks
每个 test block 单独 refit，fit_end < test_start-purge
stitch 只包含 test-block equity，不包含 fit data
```

用 2023-01-01..2026-05-31 形成尽可能多的 90d test block，再用 5 个预注册 cold-start offset 重放。任何
单 block `<180d` 不报告 ann，只报 raw return/DD/PF；仅 stitched test days `>=365` 才报告 portfolio ann。

fit artifact 必须包含训练行范围、代码/参数/hash。runner 不得加载 outer-train-end fit 回放早期 block。
注入 future fit 后必须 fail-close。所有模型超参数只按 fit likelihood、forecast/residual stability、cost boundary
或 blocked inner-CV 选择，不按策略 validation return 选择。

## 6. R4：四个真实 family

本轮只允许四个 family，减少 scope 伪完成。任一 family 没有独立 online state、trace activation 和完整配额，
整轮不能宣告完整搜索。

### 6.1 C1E：event-balanced residual Martingale

以 Round 20 C1 diagnostic 只作 changed-engine control。实现真实 deficit-round-robin scheduler：

```text
3-6 train-stable disjoint residual groups
train-frozen equal-risk group budget
activity deficit + max-live + shared next-SO reserve 决定 FO admission
active cycle 冻结 legs/beta/direction/ladder
event-level group gross <=25%，family gross <=40%
```

开放维度仅 scheduler `max_live=2/3/4`、quota window `7/30d`、reserve `next/all`、group cap `15/20/25%`，
ladder 只用预注册三个相邻 schedule。目标是降低真实 concentration，不得添加亏损组稀释 share。

### 6.2 B1S：真实多币 spot-perpetual basis Martingale

至少 BTC/ETH/BNB/SOL/XRP/DOGE 六币共享账户：

```text
basis = log(perp fresh mark / spot fresh close)
positive basis FO: long spot + short perp
negative basis FO: short spot + long perp，仅在 borrow snapshot 可用
SO: paired aggregate net<0 + basis adverse from last paired fill + reserve pass
TP: paired close after all close/legging costs remains positive
abort: break/deadline/liquidation buffer breach，损失必须入账
```

funding 是 cycle cashflow，不是独立 carry sleeve。fit lookback `30/60/120/240d`、entry z `1/1.5/2/2.5`、
SO step `0.25/0.5/0.75`、paired layers `3/4`、deadline `1/3/7/14d`、两条 soft schedule。每个 order trace 必须
显示不同 spot/perp MarketLegId。

### 6.3 P1S：partial-cointegration + exact Soft-SEL Martingale

实现 `residual = random_walk + mean_reverting` state-space，在线只用 MR component。K1 不再伪装为独立 family，
而是 P1 必过的 spurious control：independent random walks、symbol permutation、fixed cointegration、synthetic
time-varying beta。失败即 block P1S。

父 P1 通过 G1 后必须执行论文 `10.3390/a19060442` 的 exact enhancement：

- `linspace(1,5,10)` 相对层级，prefix `4/6/10`；
- label 来自完整 forward Martingale path，不是固定 horizon 涨跌；
- regularized logistic/GAM，nested blocked fit、permuted-label、calibration、ablation；
- SEL 只能 veto/side-select FO/SO，不能生成独立 trade；
- permuted label 仍有收益、模型无 order delta、SO 消失或邻域不稳即 invalid overfit。

论文单 EUR/USD、1:500、79.97% equity DD 的收益不得作为先验或目标证据。

### 6.4 V1B：budget-constrained sparse VECM aggregate Martingale

实现真实 6-10 leg weights，`SynchronizedFit` 必须包含并消费 signed weights：

```text
sum(abs(w))=1
abs(sum(w*BTC_beta))<=0.10
max abs(w)<=0.25
long gross>=0.35 / short gross>=0.35
actual non-zero legs>=5
actual residual=sum(w_i*log(price_i))
```

Johansen rank/adaptive sparsity只按 blocked fit stability 选；增加 `10.1109/TSP.2018.2799193` 的 budget constraint，
把 exchange min-notional、next-SO reserve 和每腿 rounding 直接放入 fit feasibility。交易成本 boundary 可参考
`10.3390/jrfm15040147`/`10.1080/14697688.2017.1374549`，但只预冻结 FO/SO/TP 区域。

Round 15-18 已出现 OU/first-passage/hazard。除非历史 index 证明 exact fingerprint 从未有效执行，禁止另建
first-passage family；最多作为 V1B/P1S 的预冻结 cost/deadline enhancement，且需单独 activation/ablation。

## 7. R5：G0 activation、机制真实性与配额冻结

每个 family 每个开放参数至少跑：8 synthetic adversarial traces + bull/bear/range/jump 四个 causal short windows。
low/high 必须同时有 effective hash delta、family state delta、order/rejection delta。另做 label-swap：把 family label
改掉而状态不变必须被 validator 捕获。

G0 全过后冻结并 commit/push config manifest：

| family | configs/fold | seed |
|---|---:|---:|
| C1E | 64 | 20261101 |
| B1S | 96 | 20261102 |
| P1S parent | 64 | 20261103 |
| P1S Soft-SEL | <=32/parent | 20261104 |
| V1B | 64 | 20261105 |

配额可运行很久，但不得缩水。无法完成就报告 blocked/incomplete，不能用“时间有限”宣告穷尽。

## 8. R6：G1 全量 causal cross-fit

每 config/fold/block 在 `1000/4999U` 全量 event replay。立即淘汰：

```text
breach/liquidation/filter bypass/stale or duplicate market leg
any scored block actual assets <5 or no real SO
symbol/group concentration >50%
equity DD >45% or DDR >3（equity DD<=5% 可单列）
cost/gross profit >50%
partial-fill/legging failure >10%
fit leakage / budget mismatch / missing trace hash
>=2 negative test blocks
```

每 family 最多 12 个 config 进入 G2。G1 survivor manifest 必须 commit/push，G2 只能读取 manifest exact ID。

## 9. R7：G2 预算平台、局部稳健与 trial correction

对 G1 survivor 执行全部 `<5000U` budgets、12 个局部邻域、LOSO/LOGO、fee/slippage `1/1.5x`、partial-fill
`50%` 和 one-leg delay。要求：

```text
stitched cross-fit ann>=30%（test days>=365）
worst equity DD<=35%
>=4/5 cold starts positive
neighbor >=8/12 保留 center return 的 80%，DD<=center+3pp
至少两个相邻 budget 通过，不接受孤立本金点
每个 scored block assets>=5、real SO、concentration<=50%
```

合并 Round 1-21 全部 trial count，输出 DSR、CSCV/PBO、selection frequency 和 parameter rank degradation。
未过 trial correction 的高 ann 行不得进入 selection。

## 10. R8：selection freeze 与旧数据 cross-fit 结论

每 family 最多 2 个 config+budget。先生成 `selected-configs.json`，含 engine/data/fit/market-leg/config/cost hashes、
全部 cross-fit metrics 和 first failed gate；commit 并 push。旧数据不能再做 one-shot validation，只能根据预注册
cross-fit 汇总出：

```text
CROSS_VALIDATED_RESEARCH_FINALIST
VALID_CROSSFIT_NO_TARGET
NO_VALID_CROSSFIT_CANDIDATE
BLOCKED_ENGINE_DATA_OR_EXECUTION
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

看到结果后禁止增补 neighborhood 或改 window。任何修 engine bug 都改变 fingerprint，受影响 family 从 G0 重跑。

## 11. R9：future lock 与一次性确认

`2026-07-11+` 满 30 个完整自然日、selection commit 已 push、未来数据从未被查询后，才允许每个 selected row
一次 replay。运行前 validator 记录 DB max timestamp 和首次 query 时间。失败后不得调参重读。

future OOS 必须同时过 survival、assets、SO、concentration、成本与对应档位 ann/DD；不足 180 天不允许用短窗 ann
宣布目标命中，只能报 raw return 和持续收集。真正收益结论至少需要 365 天或多个独立 future blocks。

## 12. R10：组合与实盘复现

只有至少两个 family 通过 R8 cross-fit，才允许最多 24 个 event-level combination：shared cash、max family gross
40%、group gross 25%、train-frozen equal-risk，先 reserve 已开 cycle 的 next SO 再开 FO。禁止 curve sum、按旧
validation 收益配权或 rotation。

每档最多 2 个 finalist，运行：五 cold starts、所有 budgets、2x cost、25/50/75% fill、1/2/3 bar delay、reject、
maintenance/filter stress、kill/restart/reconcile。输出 exact minimum principal、订单意图、weights、方向、leverage、
FO/SO/TP/abort、funding/borrow/fees、equity/balance DD/DDR 与 trace hashes。

backtest adapter 与真实 service entry + fake exchange 的订单/拒绝/equity suffix hash 必须一致。否则只能叫
backtest research，不得叫实盘可复现。

## 13. 进展定义

| ID | 条件 |
|---|---|
| P-A | R2 multi-market conservative engine 与 fake-exchange parity 全过 |
| P-B | 任一真实 family stitched ann>=40%、DD<=20%、4/5、共同门全过 |
| P-C | 两个独立 family 通过 cross-fit，event combination ann>=50%、DD<=20% |
| P-D | 保守/平衡/激进任一档 cross-fit 完整命中 |
| P-E | future lock 后一次性确认通过 |

代码完成、回放数量、短窗 ann、读取后的 validation 改善均不是 frontier progress。

## 14. Handoff 必填

1. R0-R10 machine state、first blocked gate、中央 registry 行数；
2. planned/running/terminal/invalid/timeout/duplicate 配额；
3. 每 family runtime type、MarketLegId、activation/ablation；
4. 每 block fit/purge/replay timestamps 与 hashes；
5. 三档、5/5、P-A..P-E 表；
6. top 10 assets、signed weights、方向、leverage、budget、ann、equity/balance DD、DDR；
7. exact minimum principal、reserve/liquidation buffer；
8. FO/SO/TP/abort/liquidation/partial-fill/legging/rejection/cost；
9. symbol/group/family concentration 与 SO PnL attribution；
10. crossfit research / future OOS / production ready 三栏，禁止混写；
11. raw argv、commit、fingerprint、五类 trace hash；
12. 每次失败 exact fingerprint、first failed gate、never-repeat；
13. future lock 查询审计。

## 15. Git 与执行纪律

1. 从包含本计划的远端 commit 建 Round 21 分支；
2. R0/R1/R2/R3/R4/G0/G1/G2/selection/future/handoff 分 phase commit/push；
3. 每个 commit body 必须含 `问题描述`、`复现路径`、`修复思路`；
4. registry 实时 append，checkpoint 至少每 30 分钟；
5. selection commit/push 前禁止 future query；
6. 不提交 DB、target、cache、大 stdout；
7. 不覆盖历史 artifact，修正只用 additive authority；
8. handoff 前工作区干净并 push 当前 branch。
