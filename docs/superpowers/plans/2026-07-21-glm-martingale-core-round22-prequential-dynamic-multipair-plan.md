# GLM Martingale Core Round 22：Prequential 动态多 Pair 与连续账户回测计划

制定日期：2026-07-21。

唯一前置权威：

- `docs/superpowers/artifacts/glm-martingale-core-round21/round21-corrected-authority.json`
- `docs/superpowers/artifacts/glm-martingale-core-round21/audit/round21-independent-audit.json`
- `docs/superpowers/artifacts/glm-martingale-core-round21/audit/round21-corrected-failure-ledger.jsonl`
- `docs/superpowers/reports/2026-07-21-chatgpt-round21-execution-audit-and-correction.md`

本文件是 Round 22 唯一任务书。禁止继承 Round 21 的 target hit、5/5、production-ready candidate、complete
phase 或 selected-configs。Round 21 的 tb05 仅作机制线索，tb02 是同政策破产反例。

## 0. 用户指令：立即回测，不等待

本轮不设置 30 天自然日监控、不创建 future lock 自动任务、不等待新增行情。直接使用现有历史数据完成
严格的 nested/prequential 回测。结论必须标记为 historical backtest，不得把已读历史包装为 pristine future OOS。

最终状态只能是：

```text
HISTORICAL_PREQUENTIAL_TARGET_HIT
HISTORICAL_PREQUENTIAL_FRONTIER_PROGRESS
VALID_HISTORICAL_PREQUENTIAL_NO_TARGET
BLOCKED_ENGINE_DATA_OR_EXECUTION
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

## 1. 不变目标与共同门

| 档位 | stitched ann | stitched max equity DD | 5 cold starts |
|---|---:|---:|---:|
| 保守 | >=50% | <=10% | >=4/5 正，另报 5/5 |
| 平衡 | >=90% | <=20% | >=4/5 正，另报 5/5 |
| 激进 | >=110% | <=30% | >=3/5 正，另报 5/5 |

共同 hard gates：

- principal `500/750/1000/1500/2000/3000/4000/4999U`，必须 `<5000U`；
- 一个连续 shared cash/margin/equity account，block 边界不重置本金；
- selector 换 pair 时先保守平仓旧 cycle，成本进入连续 equity；active cycle 禁止中途换腿；
- 实际成交 base assets `>=5`；symbol/group PnL concentration 均 `<=50%`，另报 `<=35%`；
- 每个计分窗口有真实 loss-after-add SO；下一层 paired gross `>` 上一层；
- equity DD、balance DD、DDR 同报；equity DD 是目标门；
- exact minimum principal 含 spot cash、perp margin、fees、maintenance、close 与 next-SO reserve；
- 任一 liquidation、equity<=0、stale/future leg、filter bypass、NaN、缺 trace 立即淘汰；
- 目标只能由连续 stitched test days `>=365` 的 metrics 命中，90 天块禁止输出/比较 ann。

所有 PnL 必须来自 Martingale cycle。selector、KSS/OU、graph、copula、volatility、delayed-cointegration 只能决定
pair、FO/SO/TP/abort，不得加入独立 sleeve 或理论 PnL。

## 2. R0：先修证据链，未过禁止搜索

中央 launcher 必须满足：

```text
one immutable experiment_id = exactly one running + one terminal
git_dirty=false; commit 已 push
full 64-char SHA256: binary/source/data/funding/filter/maintenance/borrow/cost/config/fit
non-null 64-char traces: event/trade/order/equity/funding/rejection
raw argv / pid / exit / wall / RSS / fit-purge-replay timestamps
```

launcher 对每个 non-complete terminal 同步 append failure ledger：fingerprint、first gate、full metrics、trace hashes、
exact config、never-repeat。registry 或 ledger append 失败则 experiment 失败。

canary 必须注入：dirty commit、短 hash、null order hash、复用 experiment ID、缺 failure row、config/argv budget
不一致、G2 非 committed parent、90 天 ann 进入 target、manifest block 被删除、test 后选 top cold starts，全部 fail。

## 3. R1：主 replay 的 production-conservative 修复

在任何收益回放前完成：

1. 每个 order 生成 `OrderIntent`，经过 filter rounding、reserve、margin 后写 order trace；
2. spot/perp cash、inventory、perp wallet、initial/maintenance margin 共用一个账户；
3. 每个 mark 按 Binance maintenance tier 检查 liquidation，触发强平费并终止组合；
4. base path 使用保守 deterministic fill；stress path 强制 25/50/75% partial fill；
5. 后腿 delay `1/2/3 bar`、reject、hedge-or-flatten，legging loss 入账；
6. FO/SO/TP/abort/rebalance/end-close 全收费；funding/borrow 精确结算；
7. reserve 包含所有 active cycle 的 next SO、close fee、maintenance buffer；
8. temporary reject cooldown、permanent freeze、kill/restart/reconcile 持久化；
9. `liquidation_count/partial_fill_count/legging_loss` 从事件重算，不得硬编码；
10. min liquidation buffer 是 event-time minimum，不是 replay 后 peak-layer 近似；
11. backtest adapter 与真实 trading service adapter 分别连接同一 fake exchange，比较 order/ack/reject/equity hash；
12. 64 并发确定性测试与所有 R21 regression 通过。

R1 任一项失败，状态 `BLOCKED_ENGINE_DATA_OR_EXECUTION`，禁止运行 selector 或参数搜索。

## 4. R2：冻结连续 prequential 协议

沿用 12 个 90 天 test block，但改为一个连续 replay：

```text
fit_t: only data <= block_t_start - purge
select pairs and risk schedule
close/transition with full cost at rebalance boundary
replay block_t without reading its result for current decision
append equity to same account
after block closes, its data may enter next fit
```

`tb01..tb12` 全部在分母；no-fit、reject、timeout、breach 都是失败，不能删除。输出每块 raw return/DD/PF 和最终
连续约 1080 天 stitched ann/DD。另建 5 个预注册 cold-start offsets，全部使用同一个 online policy；不得看
结果后挑五个。

Nested 参数选择：评估 block `t` 时，selector/execution policy 只能由 block `<t` 的 inner records 决定；前四块
作为 warm-up/evaluation，禁止用当前或未来 block 收益选 policy。每个 block terminal 记录 `policy_fit_cutoff` 和
`policy_hash`。

## 5. R3：外部机制提取与历史 exact 去重

递归索引 Round 1-21。普通 multiplier/spacing/legs、HTF trend、ADX/EMA、DD scaling、last-executed basis、旧
first-passage/hazard、DGT/breakout、standalone funding、旧 C1 block-specific per-test tuning 禁止重复。

来源转为可证伪任务：

| 来源 | 采用机制 | 禁止外推 |
|---|---|---|
| `10.1108/SEF-12-2020-0497` | dynamic crypto cointegration、optimal lookback、OU half-life、成交可得性 | 不复制论文收益 |
| `10.1007/s10260-023-00702-4` | 多 pre-selection 指标与风险暴露 | 只用 fit window |
| `10.1016/j.orl.2018.01.006` | robust dynamic cointegration | 不按 test PnL 重估 |
| `10.1080/14697688.2022.2064760` | delayed cointegration | 只 gate cycle |
| `10.1057/s41599-025-05661-7` | network disjoint-pair matching | 不使用 GNN 黑箱预测收益 |
| `10.3390/risks11050093` | multiple-pair diversification | 不按 test 收益配权 |
| `10.1080/00036846.2022.2103506` | fractional-OU persistence veto | 不作独立收益 |
| `10.2139/ssrn.5895159` | Micro-Martingale/Integral TP 待核验 | 正文公式未取得前禁止猜实现 |

对 SSRN 论文，GLM 必须先提交 source URL、下载 hash、公式页码和伪代码。无法取得则标
`blocked_unverified_source`，不得根据标题自行发明“exact”机制。

## 6. R4：三个 selector，一个连续 Martingale runtime

本轮不再造多个仅标签不同的 family。只实现一个 `PrequentialMultiPairMartingale` runtime，selector 是可消融组件。

### S0：Round 21 causal baseline control

- 30-symbol liquid universe；daily fit；ADF/half-life；top-6 disjoint；
- 只作为 changed-engine control；一套冻结 policy 跑完整时间轴；
- 不允许每 block 选择不同 ladder 参数。

### S1：Dynamic-Coint Ensemble

fit-only pre-selection 候选指标：distance/correlation、Engle-Granger、Johansen rank、KSS nonlinear stationarity、
OU half-life、fractional persistence/Hurst、spread turnover、estimated all-in cost、liquidity availability。仅开放三种
预注册规则：intersection、rank-vote、stability-first。选择阈值由 earlier-block nested fit 决定。

### S2：Robust Network Matching

以 train-only residual dependence 建 graph，边分数使用 bootstrap selection frequency、robust beta drift、ADF/KSS
stability、half-life 和 cost。用 deterministic maximum-weight matching 选 3-6 个 disjoint pairs；max cluster gross
25%，不得按 test PnL 加权。

### S3：Delayed/Break-Robust Selector

在 S1/S2 parent 上增加 delayed cointegration/change-point gate：break 时 block FO、freeze SO 或 aggregate abort；
恢复必须经过 train-frozen hysteresis。不得复用旧 generic trend/hazard gate。

## 7. R5：Martingale execution enhancement

开放的执行合同只有以下两类，避免再扫普通倍数：

### E0：冻结 Soft Ladder

```text
relative layers = [1.00, 1.25, 1.55, 1.90]
SO = aggregate net loss + residual adverse from last paired fill + reserve pass
TP = aggregate close net positive after estimated close cost
```

### E1：Micro-Martingale + Integral TP

仅在成功取得 `10.2139/ssrn.5895159` 正文、公式和可审计定义后实现。必须逐字段映射公式，且仍满足下一层
gross 大于上一层。Integral TP 必须是 active Martingale groups 的 event-level aggregate close，不允许 curve-level
利润搬运。若来源不可核验，本 family 不执行，且不阻塞 S0-S3/E0 主搜索。

selector daily 更新，signal 可测试 `4h/daily` 两档，订单/风险在 completed `1h` bar 执行；active cycle 冻结 pair。
这是新的 multi-timescale residual execution，不等同于前轮 HTF directional gate。

## 8. R6：G0 activation 与配额冻结

每个 `Sx × Ex × signal frequency` 参数必须在 synthetic mean-revert、random walk、structural break、jump、stale、
partial fill、liquidation 和 bull/bear/range 真实窗口产生预期 state/order delta。任一 inert/deferred 参数删除或 block。

冻结最多：

| mechanism | policy count | seed |
|---|---:|---:|
| S0×E0 | 8 controls | 20261201 |
| S1×E0 | 48 | 20261202 |
| S2×E0 | 48 | 20261203 |
| S3 parent enhancements | <=32 | 20261204 |
| verified E1 | <=24 | 20261205 |

config manifest commit/push 后再运行。不得看到 block 结果后扩 grid；新假设进入下一轮 ledger。

## 9. R7：两阶段立即回测

### G1：政策级 prequential replay

每个 policy 在 `500/1000/4999U` 跑完整连续 12-block 时间轴，不做 block-level top-N。Immediate fail：任一 block
breach/liquidation、actual assets<5、无 SO、concentration>50%、缺 trace、cost/gross profit>50%、连续账户状态重置。

只按 stitched metrics 排名：ann、equity DD、positive blocks、worst raw block、cost、SO attribution、selector turnover。
每机制最多 8 个 policy 进入 G2。G1 survivor manifest commit/push。

### G2：本金平台与压力

对 survivor 跑全部 `<5000U` budgets、5 cold starts、fee/slippage `1/1.5/2x`、partial fill `25/50/75%`、leg delay
`1/2/3`、reject、filter/maintenance stress、LOSO/LOGO。至少两个相邻本金通过；任何压力 liquidation 淘汰。

输出 DSR、CSCV/PBO、trial count（合并 Round 1-22）、policy selection frequency。未过 trial correction 不得 selection。

## 10. R8：三档与组合

单个 policy 本身必须多币。只有至少两个 selector mechanism 在 G2 独立通过，才允许 event-level 组合，最多 24 个：

- shared cash/margin/reserve；
- family/selector gross `<=40%`，group `<=25%`；
- fit-only equal-risk，不按 test PnL 配权；
- active pair overlap 合并风险，不能重复占用同 symbol reserve。

三档判断只读取 stitched full policy 和五个预注册 cold starts。报告 top 10 的每期 pair、weights、方向、leverage、
预算、FO/SO/TP/abort、成本、equity/balance DD、DDR、concentration 和 exact minimum principal。

## 11. 进展门

| ID | 条件 |
|---|---|
| P-A | production-conservative main replay + real service/fake exchange parity 全过 |
| P-B | 同一冻结 online policy 12-block compounded return >0、无 breach、>=8/12 正 |
| P-C | stitched ann>=35%、DD<=20%、4/5 cold starts、trial correction 通过 |
| P-D | 任一三档 historical prequential 完整命中 |

单个 90 天高 ann、每块各挑赢家、任何 block-positive、代码完成和回放数量都不是进展。

## 12. Handoff 与 Git

Handoff 必须由 validator 生成并包含：

1. R0-R8 first blocked gate；
2. registry/failure ledger 与完整 hash/trace 统计；
3. planned/running/terminal/complete 配额；
4. 每 block 的 selector cutoff、policy hash、pair rotation 与 rebalance cost；
5. 连续 stitched equity hash、ann/DD 与所有 raw block；
6. 预注册 5 cold starts，不得筛选；
7. 三档、P-A..P-D、trial correction；
8. production parity、partial fill/liquidation/legging；
9. 全部失败 fingerprint + never-repeat；
10. 明确 `historical_backtest_only`，不写 future monitor。

每 phase 单独 commit/push；commit body 必须含 `问题描述`、`复现路径`、`修复思路`。运行前 clean/pushed，结束时
工作区干净。不提交 DB、target、cache 或大 stdout。
