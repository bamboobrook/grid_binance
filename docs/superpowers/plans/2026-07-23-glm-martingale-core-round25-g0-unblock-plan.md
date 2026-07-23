# GLM Martingale Core Round 25：G0 解锁与连续回放唯一任务书

制定日期：2026-07-23。

本文件是 Round 25 后续执行的唯一任务书，替代
`2026-07-23-glm-martingale-core-round25-reference-copula-recovery-plan.md`。旧任务书保留为预注册历史；除本文件明确覆盖的条款外，旧任务书中的目标、Martin 唯一收益合同、成本、因果、共享账户、
`<5000U`、防过拟合和 G2/R8 规则继续有效。

必须直接修复并继续历史回测，不设置观察期，不等待新增行情，不把首次实现错误写成最终交接。

## 0. 当前 G0 不是有效失败

当前提交 `0e64767c` 只能归类为 `MATERIALLY_INCOMPLETE_INVALID_RESULTS`，不能关闭 C1 fingerprint：

1. `r25_execute.rs` 在进入生产回放前直接构造 `blocked=true`；
2. `return_replays_run=0`，没有执行 real-window C1 replay；
3. `g0-c1-blocked/order.jsonl` 的 `actual_fill=null`，只是把同一 blocked payload 写入八类文件；
4. `g0_so_helper_not_called_by_scored_replay_is_rejected` 实际比较同一 hash 与自身，没有证明 scored path 调用；
5. `data_gate_for_return_replay=true` 被写死，但 LTCUSDT funding coverage 为 0 行；
6. 因此现有 `BLOCKED_ENGINE_DATA_OR_EXECUTION` 是脚手架状态，不是机制、数据或收益结论。

旧 artifact root 全部只读：

```text
docs/superpowers/artifacts/glm-martingale-core-round25/
```

本次继续执行使用：

```text
docs/superpowers/artifacts/glm-martingale-core-round25-recovery/
docs/superpowers/reports/2026-07-XX-glm-round25-recovery-handoff.md
```

不得覆盖、改写或删除旧 artifact。旧 blocked row 保留在全局 trial ledger，但不写 exact failure closure。

## 1. 不得放松的硬约束

1. 收益引擎只能是 C1/C2 中的 Martin FO、loss-after-add SO、TP/reduce/abort；
2. BTCUSDT 仅作 reference，订单、position、trade、PnL 中 BTC 数量必须为 0；
3. signal bar 完成后才能 ready，最早下一根 1m event 下单和成交；
4. 所有币、pair、cycle 共用连续 cash/margin/equity/reserve account；
5. FO/SO 后持续预留全部 active groups 的 next SO、close、maintenance 和 pending-leg 资金；
6. SO 前 group net PnL after estimated close cost 必须 `<0`，下一层实际成交 gross 严增；
7. filters、rounding、min notional、funding、partial fill、legging、maintenance 和 liquidation 必须进入事件账本；
8. 目标仍为 `50%/10%`、`90%/20%`、`100%/30%`，本金严格 `<5000U`；
9. 最终候选必须是共享账户多币组合，完整时间线实际成交 `>=6` 个 alt、`>=3` 个 distinct pairs；
10. 不扩普通 multiplier/spacing/TP/指标网格，不新增收益 family，不读取 outer returns 选参数。

## 2. 本次修订覆盖的旧条款

以下旧条款作废：

- G0 要求两个短 real windows 同时有 `>=3` pairs；
- 每个 outer block 少于三对就整体 `no_fit`；
- scheduler 三臂在候选不足时仍必须产生 real order hash delta；
- 每个 G0 参数都必须在每个短窗口产生 real order/rejection delta；
- 任何首次编译、数据或实现错误都可立即结束整轮；
- 没有真实事件时，为凑齐八类 traces 写入 placeholder 行。

替代规则：

1. 每个 block 可形成 `0..3` 个 disjoint pairs；只有 0 对才是该 block `no_fit`；
2. `>=6 actual assets`、`>=3 distinct traded pairs` 只在完整 G1 stitched timeline 判断；
3. G0 验证机制、因果和参数绑定，不用短窗口证明最终多币收益；
4. no-fit/no-signal/reject 仍保留在完整 12-block 分母，不得删除困难时期；
5. 当前实现错误必须先修复并重跑同一 gate，不能直接生成最终 handoff。

## 3. U0：先把脚手架变成生产执行器

### 3.1 必须实现，不得再写死状态

重构 `r25_execute` 为可恢复 orchestrator，至少支持：

```text
--phase preflight
--phase g0
--phase g1
--phase all
--resume
--artifact-root <path>
```

执行状态只能由真实 phase result 派生。生产路径禁止出现无条件：

```text
"blocked": true
"return_replays_run": 0
"real_window_order_delta_passed": false
```

允许这些值出现在失败测试 fixture，不允许 orchestrator 在未调用 replay 时把它们当 G0 结论。

### 3.2 复用同一生产调用路径

必须新增一个真实 C1 replay API，G0 和 G1 调用同一函数，例如：

```rust
run_r25_c1_replay(config, interval, mode) -> ReplayEvidence
```

`mode` 只能控制输出范围或是否计算收益指标，不得切换信号、订单、账户或 SO 实现。G0 与 G1 必须共同调用：

- completed-bar aggregator；
- train-only reference spread/copula fitter；
- pair matcher/admission scheduler；
- shared account submit/fill/funding/close；
- 同一个 `allow_r25_so` production function；
- 同一个 filter/reserve/concentration evaluator。

禁止用 `hash == same_hash` 证明绑定。每次 production SO decision 必须写入输入、输出、call-site version 和
event sequence；测试通过 replay trace 证明该函数实际被调用。

### 3.3 真实 trace 规则

1. order trace 只允许真实 submit attempt；`actual_fill=null` 不能算成交；
2. rejection trace 只允许真实 signal/admission/order rejection，并记录 reason code；
3. 没有事件的 stream 写 `row_count=0` 到 manifest，文件可为空，不得复制 phase payload 充数；
4. order/rejection hash 从 canonical event rows 计算；
5. validator 必须拒绝八个 stream 内容完全相同、placeholder 或未来时间戳；
6. registry terminal row 引用 trace manifest、真实进程 exit code 和 replay count。

## 4. U1：修复数据门

数据门按 symbol 计算，不再使用全局常量：

```text
tradable = complete 1m klines
           AND complete funding or explicit conservative missing-event rule
           AND valid filter snapshot
           AND maintenance model available
```

本轮主回放不允许用“funding=0”填补 LTCUSDT。先尝试补齐并校验 LTC funding；若来源确实不可获得，则把
LTC 标记 `ineligible_for_trade`，保留在 manifest，不得静默交易。

继续 G0 的最低数据条件是 BTC reference 加至少 2 个 eligible alts；继续 G1 的最低数据条件是 BTC 加至少
6 个 eligible alts。G1 启动前不足 6 个时，才允许声明 `BLOCKED_DATA`，并必须列出逐 symbol 缺口和已执行的
补数命令。数据门结果由 coverage rows 的 `all(...)` 计算，禁止 hard-code true。

## 5. G0-S：生产函数的确定性机制门

先运行 synthetic/accounting gate，旧 9 个 regression canaries 与新增 12 个测试全部必须通过，但每项必须：

1. 先构造会失败的反例；
2. 调用生产 replay/account/order function；
3. 断言 trace 或 wallet/reserve delta，不接受常量布尔值；
4. 记录 test name、command、exit code、production symbol 和 evidence hash。

额外必须证明：

- low/high alpha、lookback、frequency、SO step 在 synthetic boundary fixture 中改变 decision hash；
- scheduler 三臂在 `eligible candidates > available slots` fixture 中改变 admission hash；
- one-leg reject 触发 hedge-or-flatten 且损失进 wallet；
- shared reserve 会阻止本可单独成交的新 FO；
- family=100% 在单 family 情况只作信息字段；
- dynamic 35% freeze 在 event time 改变后续 FO hash；
- block DD 使用 local running peak，但 stitched DD 使用连续 global peak。

G0-S 失败必须修代码并重跑，不得写 Round 25 最终 handoff。

## 6. G0-R：真实数据烟测，不再要求短窗三对

### 6.1 固定窗口与扩展扫描

先运行四个固定 causal windows：

```text
range: 2023-08-01..2023-09-30
bull:  2024-02-01..2024-03-31
shock: 2024-04-01..2024-05-31
bear:  2025-02-01..2025-03-31
```

这些窗口只验证 activation/causality/order binding，不比较 PnL、不选最赚钱窗口。若四窗全部无可测试事件，
按日历顺序扫描从 2023-07-01 开始的非重叠 30 天窗口，直到：

- 找到可测试事件；或
- 扫完 2026-05-31。

扫描顺序和全部 no-fit/no-signal 结果必须落盘，禁止只保留命中窗口。

### 6.2 G0-R 通过条件

聚合全部已扫描 real windows 后满足：

1. 至少一个真实 fitted pair；
2. 至少一个真实 FO submit 或有信号后的真实 order/admission rejection；
3. `signal_close < signal_ready < order_time <= fill_time`，拒单则没有伪 fill；
4. BTC order/trade/PnL count 为 0；
5. shared reserve、filter version、rounded quantities、maintenance 可从 trace 复算；
6. production SO guard 至少被真实事件路径调用一次，允许结果为 false；
7. 在全部窗口聚合后，至少一个预注册 low/high config pair 改变真实 fit/admission/order/rejection hash。

此处不要求真实 SO fill、不要求单窗三对、不要求 scheduler 在候选不足时产生差异。真实 loss-after-add SO 和
六币三对留到完整 G1 判定。

若 scheduler 在所有真实窗口都没有竞争场景，写 `not_testable_in_g0_real`；G0-S 已证明绑定后可进入 G1。
若某个参数 synthetic 有效、但完整 real-window scan 中始终不改变任何 fit/admission/order/rejection，标为
`real_data_inert`，在看收益前删除该维度、重建 policy manifest 和 fingerprints；原 16 次仍进入 trial ledger。

### 6.3 G0 允许的终态

```text
PASS
PASS_WITH_REAL_SCHEDULER_NOT_TESTABLE
VALID_NO_ACTIVATION
IMPLEMENTATION_OR_DATA_BLOCKED
```

- `PASS*`：必须立即进入 G1，不得停下来只写 handoff；
- `VALID_NO_ACTIVATION`：仅在完整日历扫描没有 fitted pair 或没有任何 signal 后成立，关闭 exact C1 fingerprint；
- `IMPLEMENTATION_OR_DATA_BLOCKED`：仅在实际命令失败、记录错误、完成至少三次针对性修复仍无法运行时使用；
- 禁止再次用预先构造的 blocked artifact 进入任何终态。

## 7. G1：先做激活普查，再跑完整 16 policies

### 7.1 Activation census

对冻结 policy population 的全部 12 blocks 运行 train-only fit/signal census，不计算或排名收益，输出：

- 每 block eligible alts、pair candidates、matching 和 no-fit reason；
- 每 policy FO candidate、admission、reject 和 SO-decision counts；
- 参数 decision hashes；
- 完整分母，包括 no-fit/no-signal/reject blocks。

不得因单个 block 只有 1 或 2 对而跳过该 block；当时有几对就由共享账户运行几对，最多 3 个 active groups。

### 7.2 Full replay

对保留的全部非 inert policies 独立、连续运行 tb01..tb12：

1. 账户、wallet、running peak、active cycle 跨 block 不重置；
2. 每 policy/block checkpoint，失败后 `--resume`，不得从头丢失 ledger；
3. 完整时间线必须实际成交 `>=6` alts、`>=3` distinct pairs；
4. 必须至少两个 groups 出现真实 loss-after-add SO；
5. 六币、三对、SO 或收益不达标属于该 policy 的 `valid_failure`，不是 engine blocked；
6. 16 个 policy 全部 valid failure 后才能关闭 C1，不能因第一个失败提前停止；
7. P-B survivor 立即继续旧任务书 C2/G2/R8；无 P-B 才结束条件阶段。

旧任务书的 P-A/P-B/P-C/P-D、concentration、cost ratio、cold starts、budgets、stress 和 anti-overfit 门保持不变。

## 8. 失败恢复与禁止提前交接

遇到错误按以下顺序处理：

1. 保存命令、stderr、input hash、checkpoint 和最小复现；
2. 判断 `implementation_bug`、`data_gap`、`valid_no_fit` 或 `valid_policy_failure`；
3. implementation/data 问题先修复并重跑同一 phase；
4. 至少三次独立修复尝试后仍被同一外部条件阻塞，才允许 blocked；
5. blocked 不能写 `valid failure`，也不能关闭 mechanism fingerprint；
6. 编译失败、运行慢、窗口 no-fit 或某 policy 无收益都不是提前结束整轮的理由。

每次失败追加 exact row：

```text
failure_id / phase / policy / fingerprint / class / input_hash
command / exit_code / stderr_hash / repair_attempt / resolution
safe_to_skip_exact_fingerprint / never_repeat
```

## 9. 执行命令与验收

实现后至少依次运行：

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo run --release -p r24-research --bin r25_execute -- \
  --phase g0 --artifact-root docs/superpowers/artifacts/glm-martingale-core-round25-recovery
cargo run --release -p r24-research --bin r25_execute -- \
  --phase g1 --resume --artifact-root docs/superpowers/artifacts/glm-martingale-core-round25-recovery
```

如果 G1 有 P-B survivor，再自动继续 `--phase all --resume`，完成 C2/G2/R8。命令名称可以按现有 crate
做最小调整，但 phase、resume 和独立 artifact root 能力不可省略。

提交前 validator 必须确认：

- G0 real replay count `>0`；
- 不存在 placeholder trace；
- data gate 与逐 symbol coverage 一致；
- registry 每个 experiment 正好一行 running 和一行 terminal；
- G1 policy count 等于冻结后的 non-inert population；
- 所有失败都可区分 invalid blocked 与 valid strategy failure；
- authority、state、registry、traces 和 report 的状态一致；
- tests 通过、commit 已 push、worktree clean。

Git 每次提交 body 必须包含：

```text
问题描述:
复现路径:
修复思路:
```

## 10. GLM 唯一执行清单

- [ ] 保留旧 blocked artifacts，只读并登记为 invalid attempt-00
- [ ] 删除生产 orchestrator 的 hard-coded G0 blocker
- [ ] 实现 G0/G1 共用的 C1 production replay API
- [ ] 修复 trace schema、真实 call binding 和 validator
- [ ] 修复逐 symbol data gate；补 LTC funding 或显式禁用
- [ ] G0-S 全部 production-bound canaries 通过
- [ ] G0-R 四窗 + 必要时完整日历扫描
- [ ] G0 PASS 后不停止，立即 activation census
- [ ] 跑完冻结后的全部 G1 policies、12 blocks 和真实共享账户
- [ ] 有 P-B 则继续 C2/G2/R8；无 P-B 才形成 valid no-target
- [ ] 生成 recovery authority/state/registry/ledger/traces/handoff
- [ ] tests、validator、commit、push、clean worktree

**停止条件**：只有完成全部适用阶段，或同一外部阻塞经过至少三次可复现修复仍无法解除，才能停止并交接。
不得再次在 `return_replays_run=0` 且生产 replay 从未调用的状态下宣布 Round 25 执行完成。
