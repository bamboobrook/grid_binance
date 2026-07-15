# GLM Martingale Core Round 16：非对称状态路由、首达风险与相关簇库存执行计划

执行者：GLM

本计划只能从 ChatGPT Round 15 审计修正提交创建新分支后执行。Round 15 原 handoff、原
`round15-execution-state.json@76bd8ad` 和任意 `passed:true` 自报文件不得作为完成依据。

唯一 authority：

- `docs/superpowers/reports/2026-07-15-chatgpt-round15-execution-audit-and-fix.md`
- `docs/superpowers/artifacts/glm-martingale-core-round15/round15-independent-recheck.json`
- `docs/superpowers/artifacts/glm-martingale-core-round15/r1-r15-corrected-status.json`
- 本计划冻结后的 SHA256

## 1. 目标和不可变边界

Martingale/DCA cycle 是唯一收益来源。HTF、downside、half-life、cluster 等状态只能：

1. 决定哪个 symbol/direction 可以开启下一条 Martingale cycle；
2. 调整该 cycle 的 FO、SO multiplier、spacing、TP、deadline、reserve；
3. 对已有 cycle 执行 freeze-SO 或真实 aggregate reduce-only；
4. 在同一 `<5000U` shared account 内调度多个 Martingale sleeves。

禁止独立 trend/breakout 仓、buy-and-hold、funding carry、pair-neutral、stat-arb、curve PnL、
shadow PnL 或事后 benchmark 收益进入 equity。

| 档位 | 年化收益 | 最大回撤 | cold-start 正收益 |
|---|---:|---:|---:|
| 保守 | >=50% | <=10% | >=4/5 |
| 平衡 | >=90% | <=20% | >=4/5 |
| 激进 | >=110% | <=30% | >=3/5 |

共同硬门：

- launch budget 从 `1000/2000/3000/4000/4999U` 在 train 内选择，严格 `<5000U`；
- 五档预算均无 principal breach，输出 min equity、blocked/rejected orders、最大 margin/notional；
- 单一 event-level account，至少 5 个真实成交 symbols、至少 3 个 base assets；
- configured capital、gross profit、positive net PnL 单币集中度均 `<=35%`；
- fee、slippage、funding、rounding、minNotional、leverage、liquidation buffer 生效；
- nested WFO、平台、LOSO/LOCO、成本/延迟、production trace 全通过才可叫 candidate；
- 30 天未来 paper 通过前绝不写 `production-ready`。

## 2. v3 机器状态，禁止伪完成

启动时创建：

```text
docs/superpowers/artifacts/glm-martingale-core-round16/round16-execution-state.json
docs/superpowers/artifacts/glm-martingale-core-round16/exploration-registry.jsonl
docs/superpowers/artifacts/glm-martingale-core-round16/bootstrap-required-gates.json
scripts/glm_r16_validate_execution_state.py
```

validator 和 `bootstrap-required-gates.json` 在 bootstrap commit 后冻结 hash。GLM 不得在后续
phase 修改 validator；如发现 validator bug，停止并交回 ChatGPT 审批。

状态只能是：

```text
running / complete / complete_zero_survivors / blocked /
not_applicable / waiting_future_data
```

规则：

- gate builder 的 `passed=true` 没有权威性，validator 必须重算 raw rows/hash/count；
- required gate 未执行时只能是 `blocked` 或 `not_applicable`，不能伪写 passed；
- `complete_zero_survivors` 只在该 phase 的全部输入配额已执行且每个淘汰原因可重算时使用；
- candidate-only 后续 phase 可为 `not_applicable`，P9 可以在合法零 survivor 路径结束；
- `binary_replays` 是实际启动总数，`unique_execution_keys` 单独统计，重复不得从实际数中删除；
- registry 非法行、终态缺 raw command/exit/hash、running 无终态均 fail-closed。

validator 必须先通过以下 mutation regressions：

```text
bare_passed_true_is_rejected
22_of_128_cannot_pass_g1
helper_only_production_trace_is_rejected
zero_finalists_cannot_fake_required_robustness_pass
duplicate_launch_counts_as_actual_replay_and_duplicate
g2_dd_over_35_or_segment_breach_cannot_promote
phase_cannot_skip_blocked_predecessor
validator_or_required_gates_hash_change_blocks
```

固定 phase：

| phase | 内容 |
|---|---|
| R0 | validator、数据、真实 Batch/CLI parity 修复 |
| R1 | production/event 状态路由、hazard、scheduler 实现 |
| R2 | D/H/C 全参数 binding + restart trace |
| R3 | 固定 tracks/universe/parameter contracts |
| R4 | G1 train-only cheap stress |
| R5 | G2 full train + G3 nested WFO |
| R6 | finalist platform/LOSO/LOCO/cost/delay/budgets |
| R7 | production parity + exchange contract |
| R8 | future-lock 或 waiting data |
| R9 | machine-recomputed final handoff |

每 phase 独立 commit，commit body 必须含 `问题描述 / 复现路径 / 修复思路`，push 成功且远端
SHA 一致后才能推进。

## 3. R0：先修权威入口

### R0.1 真实 20-config Batch/CLI parity

`portfolio_budget_replay` 增加 canonical trace digest 输出，不输出完整百万事件数组：

```text
event_stream_sha256
trade_stream_sha256
equity_stream_sha256
funding_stream_sha256
rejection_stream_sha256
resolved_config_sha256
canonical_input_rows_sha256
```

序列化必须固定字段顺序、浮点表示和 timestamp 顺序。对 20 个历史 configs 逐个比较：

```text
release CLI subprocess vs BatchReplay
events / trades / realized PnL / equity / DD / funding / rejection reasons
tolerance = 1e-9 for metrics; hashes exact
```

禁止 direct helper vs Batch、Batch single vs Batch parallel 或“共享 loader 所以相等”代替 CLI。
修复后重建 release binary、manifest 和 SHA256；旧二进制只保留 regression，不再作为搜索引擎。

### R0.2 Round 15 反例

新引擎必须复现：

```text
source A 3000U 21.3373/36.3246; 4999U 36.3790/28.6797; 4/5
source B 3000U 18.6454/38.0651; 4999U 32.6729/29.4675; 5/5
T3-8 baseline 4999U 42.1070/29.9014
T3-8 2026 cold start -92.2840/DD105.0700/principal breach
```

容差 `0.02pp`。语义变化导致不一致时，必须解释 event-level delta，禁止直接改 expected。

## 4. R1：真实 production/event 接线

必须修改 `apps/backtest-engine/src`、`apps/trading-engine/src` 和 shared config；只新增 tests 不算。

production test 必须从真实 started executor/main service 入口运行，经过 temp SQLite writer/read、
reconcile、restart、order submission adapter。禁止调用 `set_*_for_test()` 伪装接线。

必须落盘并恢复：

```text
completed HTF boundary and router state
state enter/exit persistence and minimum dwell
active long/short sleeve set
cycle age, estimated half-life bucket and deadline
next SO trigger/size and aggregate average entry
symbol/cluster inventory and safety reserve
last admission timestamp and deterministic tie-break state
```

## 5. R1-A：非对称 Martingale regime router

Round 15 的有效事实是 long-only 在 2023-2024 强、2025-2026 失效；对称 short 在牛市爆仓。
因此不再搜索 symmetric long/short，而是同一状态机非对称调度：

```text
BULL:
  allow new long Martingale cycles
  block new short cycles

BEAR:
  block new long cycles
  allow only shallow short Martingale cycles

RANGE:
  allow displacement mean-reversion cycles in either direction
  same symbol still at most one live cycle

SHOCK or UNKNOWN:
  block every new cycle
  existing cycles continue TP; SO obeys hazard freeze/deadline
```

状态只使用 decision 前 completed bars：

```text
primary timeframe = 4h
confirmation timeframe = 1h
trend return horizon = 6 / 12 / 24 primary bars
EMA slope horizon = 12 / 24 bars
normalized trend boundary = 0.75 / 1.25 / 1.75 lagged sigma
enter persistence = 2 / 3 / 6 primary bars
exit persistence = 1 / 2 / 3 primary bars
minimum dwell = 12 / 24 / 48h
RANGE displacement boundary = 0.75 / 1.25 / 1.75 sigma
SHOCK = downside semivariance above train 90/95 percentile
```

direction flip 只影响下一 cycle，不能反转、复制、平移或强平已有 cycle。同一 symbol long/short
不能同时 live。状态 tie/UNKNOWN 必须 block admission。

short ladder 必须比 long 更浅，防 short squeeze：

```text
short multiplier <= long multiplier
short max_legs <= long max_legs
short spacing >= long spacing
short deadline <= long deadline
short aggregate notional <= 40% account budget
```

## 6. R1-B：首达/half-life deadline 与 reserve

用 completed 1h prices 估计 AR(1)/OU half-life；`beta<=0`、`beta>=1`、样本不足或不稳定时为
`UNKNOWN`。不允许对 current bar 或 validation 重估。

```text
half-life windows = 72 / 168 / 336h
deadline = 2 / 3 / 5 half-lives
deadline cap = 24 / 72 / 168h
after deadline = freeze SO or 10/20/30% aggregate reduce-only
reserve = existing cycles next 1/2 SO legs + fees + liquidation buffer
risk fraction = 0.20 / 0.35 / 0.50 of remaining free capital
```

vol、downside、cycle age、depth 任一上升时，admission risk cap 不得上升。SO 仍是亏损后加仓，
但 cap 不足时必须拒绝真实 order，不能缩小账面成本后按原 notional 记收益。

必须输出 empirical first-passage diagnostic：train 内按 state/displacement/depth bucket 统计在
deadline 前到达 TP 的比例和置信区间。它只能用于 gate/reserve，不得产生预测 PnL。

## 7. R1-C：train-only correlation cluster scheduler

每个 WFO fold 仅用 train completed 1h returns 聚类，validation 冻结。预声明：

```text
C0 no cluster cap control
C1 abs-correlation threshold = 0.55 / 0.70 / 0.85
C2 MST communities target = 3 / 4 / 5 clusters
```

真实 event scheduler：

```text
symbol live margin cap = 20/25/30% budget
cluster live margin cap = 30/35/40% budget
cluster downside contribution cap = 35/45%
admissions per cluster per completed minute = 1/2
existing safety reserve before any new FO
tie = frozen symbol id
inactive sleeve continues SO/TP/deadline management
```

## 8. 外部研究转化边界

| 来源 | 只允许转化为 |
|---|---|
| Dobrynskaya, DOI `10.3905/JAI.2023.1.189` | 预声明不同 horizon 的 momentum/reversal admission family；不复制论文收益 |
| Foroni/Merlo/Petrella, DOI `10.1007/S11222-023-10377-2` | downside/return state 稳定性诊断；Round 16 主线仍用可解释 deterministic hysteresis，不上复杂 HMM |
| Iqbal/Zahid/Koutmos, DOI `10.3390/RISKS11070122` | downside state 与 tail stress；不把 downside signal 作为独立仓 |
| Kitapbayev/Leung, DOI `10.1142/S0219024918500048` | sequential deadline、freeze/reduce contract |
| Leung/Li, DOI `10.1142/S021902491550020X` | 带成本/止损的 mean-reversion boundary diagnostic |
| Busseti/Ryu/Boyd, DOI `10.3905/JOI.2016.25.3.118` | FO/SO/reserve 上限，不用 Kelly 预测放大杠杆 |

文献定义可证伪机制，不是收益证明。

## 9. R2：参数绑定硬门

family：`D1 BULL/BEAR`、`D2 RANGE displacement`、`D3 SHOCK`、`H1 spacing`、`H2 deadline`、
`H3 reserve`、`C1 correlation`、`C2 MST`。

每 family 运行 `8-16` synthetic traces。每个开放参数必须同时改变：

```text
resolved config hash
effective config hash
state/admission/deadline/reserve event hash
至少一个真实 order/rejection trace
```

inactive、仅改 label、只改 JSON 未进 engine、只改 helper 的参数判 `parameter_inert`，停止 family。

## 10. R3：固定 tracks 和风险包络

universe：

```text
T1 diagnostic R7 exact universe; 永不晋级
T2 BTC ETH BNB SOL XRP DOGE ADA TRX LINK LTC BCH DOT
T3-8 BTC ETH BNB SOL XRP DOGE ADA TRX
T3-12 same as T2
```

每币同时配置 long/short sleeve，但由 router admission；全期至少 5 symbols 实际成交。

风险包络只定义搜索边界：

| 档 | leverage | long FO | long mult | long legs | short mult | short legs |
|---|---|---|---|---|---|---|
| 保守 | 3/5 | 10/15/20 | 1.25/1.40/1.55 | 4/5 | 1.20/1.30 | 3/4 |
| 平衡 | 5/8 | 15/20/30 | 1.40/1.60/1.80 | 5/6 | 1.25/1.40 | 3/4 |
| 激进 | 8/10 | 20/30/45 | 1.60/1.90/2.20 | 5/6/7 | 1.30/1.50/1.70 | 3/4/5 |

spacing `80/120/180/250/400bps`，TP `80/120/180/250/350bps`。worst-case reserved margin
在生成 config 时先验检查，超预算不进入 sampler。

## 11. R4-R5：固定搜索与 nested WFO

### G1 cheap stress

全局生成 128 个 unique Sobol effective configs：

```text
T1=12 seed 20260715
T2=72 seed 20260716
T3-8=22 seed 20260717
T3-12=22 seed 20260718
```

共同的四个 earliest-train 30 天窗口，禁止后验选行情：

```text
2023-01-01..2023-01-30
2023-02-15..2023-03-16
2023-04-01..2023-04-30
2023-05-25..2023-06-23
```

每 config × 4 windows × `1000/3000/4999U`，必须是 `1536` actual binary replays。重复 key
必须 cache hit，不能算执行。G1 只淘汰，不发布短窗口“年化”。全局最多 24 Pareto configs。

### G2/G3 anchored nested WFO

```text
F1 train H1-2023             purge 7d -> validate H2-2023
F2 train 2023                purge 7d -> validate 2024
F3 train 2023-2024           purge 7d -> validate 2025
F4 train 2023-2025           purge 7d -> validate 2026-01-01..2026-05-31
```

每 fold：

1. 24 configs 只读 train，重新估计 router boundaries、half-life buckets、clusters；
2. launch budget 也只在 train 选，写入 hash 后冻结；
3. strict G2：train median ann `>=30%`、worst train-block DD `<=35%`、>=3/4 blocks 正、无 breach；
4. 最多 8 configs 读取 validation 一次；
5. 两个 validation folds ann 为负、任一 DD `>45%` 或 breach，停止该 family；
6. 最多 3 finalists 进入 R6。

4 folds 至少 3 folds 选择相同或相邻 budget，否则 `budget_unstable`。禁止看 validation 后改
budget、state label、参数范围或 universe。

保存所有 train trials 和 validation traces，使用完整 return series 计算真实 CSCV/PBO、DSR、
selection frequency 和 rank degradation；禁止 top-5/ann mean proxy。

## 12. R6：finalist robustness

每个 finalist 必须完成：

- 五预算 full + 5 cold starts；
- 参数平台 12 neighbors，至少 8/12 ann 保留中心 `>=80%` 且 DD `<=center+3pp`；
- LOSO 每次删 1 symbol；LOCO 每次删 1 frozen cluster；
- fee/slippage `1.0x/1.5x/2.0x`，adverse funding；
- entry/SO/TP delay `1/2 completed bars`；
- same-bar conservative order；
- start/restart at every cold boundary；
- 三种 concentration、min equity、liquidation distance、rejections。

0 finalists 时这些 phase 标 `not_applicable`，绝不能写 passed。

## 13. R7：production parity

必须通过真实 started service：

```text
started_executor_applies_router_hazard_cluster_state
real_sqlite_writer_read_restores_completed_state
exchange_reconcile_does_not_duplicate_cycle_or_so
restart_restores_router_half_life_deadline_reserve_cluster
backtest_live_order_and_rejection_trace_hash_match
direction_flip_affects_next_cycle_only
same_symbol_never_has_two_live_directions
inactive_cycle_continues_so_tp_deadline
same_timestamp_strategy_order_invariant
exchange_rounding_min_notional_liquidation_parity
```

禁止 test-only setter、mock-only writer、JSON-only roundtrip、手动 helper gate 或恒真断言。

## 14. R8 future lock

`2026-06-01..2026-07-10` 已打开，只能诊断。`2026-07-11` 后数据在达到连续 30 天前不得读取
收益指标；最早可用日为 `2026-08-10` 之后。到时先冻结 candidate/config/engine/data hash 并
commit，再读取一次。数据不足则 `waiting_future_data`，不能伪写 passed。

## 15. Registry、停止与交接

每次运行前写 `running`，终态至少：

```text
experiment_id/family/track/hypothesis
resolved/effective config JSON and hash
engine/data/manifest/plan/validator hashes
window/budget/seed/sampler/fold
raw command/exit code/wall/RSS
actual replay count/cache/duplicate/timeout
ann/DD/min equity/breach/rejections/max capital
three concentration metrics/event/trade/equity/funding hashes
reject reason/exact_non_repeat_scope/status
```

立即停止 family：参数 inert、duplicate rate `>25%`、非 Martingale PnL、两 folds 负、DD>45%、
principal breach、实际 symbols<5、集中度>50%、Batch/CLI/production trace mismatch。

最终 handoff 必须由 validator 从 registry/raw artifacts 重算：

1. 三档命中列表或空数组；
2. unique keys、actual replays、duplicates、timeouts；
3. 每 phase 的 `complete/zero/not_applicable/blocked`，不得混写；
4. 每个 finalist 的 budgets/WFO/platform/LOSO/LOCO/stress/production；
5. 所有失败的 exact non-repeat scope；
6. `backtest_candidate` 与 `production_ready` 分开。

没有候选同时通过全部门就输出零命中，不降低目标；但只允许对“已完整执行的 exact family”写
失败，不得再把局部 long-only 结果外推成 Martingale 全部可能性已穷尽。
