# ChatGPT Round 18 执行审计、引擎修复与独立复算

审计日期：2026-07-17。

审计输入：

- `docs/superpowers/plans/2026-07-16-glm-martingale-core-round18-native-synchronized-residual-cycle-plan.md`
- `docs/superpowers/reports/2026-07-17-glm-round18-execution-handoff.md`
- `docs/superpowers/artifacts/glm-martingale-core-round18/`
- `scripts/glm_r18_*.py`
- `apps/backtest-engine/src/martingale/sync_cycle_engine.rs`
- `apps/trading-engine/src/main.rs`
- `apps/trading-engine/src/martingale_runtime.rs`

权威机器产物：

- `docs/superpowers/artifacts/glm-martingale-core-round18/r18-corrected-authority.json`
- `docs/superpowers/artifacts/glm-martingale-core-round18/audit/round18-independent-recheck.json`

下一轮唯一任务书：

- `docs/superpowers/plans/2026-07-17-glm-martingale-core-round19-valid-residual-recovery-plan.md`

## 1. 权威结论

GLM 的 `VALID_NOVEL_FAMILIES_EXHAUSTED_NO_FRONTIER_PROGRESS` 不成立。Round 18 权威状态修正为：

```text
materially_incomplete_invalid_results
```

原因不是单纯“收益未达标”，而是 M1 订单方向、Martingale PnL、平仓成本、group cap、分散度、nested
fold、M2 配额、registry 和 production parity 同时不符合任务书。原来的约 1640 次回放不能证明 M1/M2
有效穷尽，也不能用于收益前沿排名。

三档仍然零命中：

| 档位 | ann | DD | 稳定性 | Round 18 权威结论 |
|---|---:|---:|---:|---|
| 保守 | >=50% | <=10% | >=4/5 正 | 未命中 |
| 平衡 | >=90% | <=20% | >=4/5 正 | 未命中 |
| 激进 | >=110% | <=30% | >=3/5 正 | 未命中 |

没有 5/5 cold-start 证据，没有有效 finalist，没有 production-ready candidate。

Round 1-17 继续沿用各轮 ChatGPT corrected authority；本次没有恢复任何历史目标命中。Round 18 不能把
“旧结果无效”解释成历史结果已全部有效修复：必须由 Round 19 先修复执行器，再逐 fold 重跑被污染的
M1，并补跑从未执行的 M2。

## 2. Critical：M1 实际是双币同时做多

R4 对所有 M1 fit 写入：

```text
leg_direction_signs=[1,1]
```

旧引擎虽然计算 `residual_z`，但 FO、SO、funding 和 PnL 始终读取这两个静态 sign。正负 residual 只决定
是否开仓，不决定每条腿多空。因此实际订单不是：

```text
residual = dependent - beta * factor
q_dependent = -sign(residual)
q_factor    =  sign(residual) * beta
```

而是“双币同时 long 的同步 Martingale”。这直接推翻 M1 residual-pair 的机制声明、收益值和 G1/G2
selection。修复后方向在 cycle open 时由 residual sign 和 train-frozen beta 动态冻结，active cycle 中不
翻向。

## 3. Critical：账务和风险约束失真

### 3.1 Martingale 多层均价错误

旧实现把不同金额的 FO/SO 成交价做算术平均，再对总 notional 计算 PnL。Martingale 每层金额不同，必须
先算数量：

```text
quantity_k = notional_k / fill_price_k
pnl = sign * sum((mark - fill_price_k) * quantity_k)
```

旧算法会系统性改变加仓后的盈亏平衡价。

### 3.2 平仓没有 fee/slippage

旧实现只在 FO/SO 收开仓费和滑点，TP、deadline 和 end-of-window close 均免费。TP 判断也没有预留
平仓成本。修复后所有 reduce/close 收取真实 mark notional 的 fee/slippage，TP 必须在扣除估计平仓
成本后仍超过净收益 floor。

### 3.3 `group_gross_cap_pct` 比较对象错误

任务书要求 group gross notional cap；旧实现却拿 `projected margin` 与 gross cap 比较，杠杆越高越容易
越过名义风险上限。修复后 group cap 按 gross notional 判断，并另行检查 portfolio margin/equity。

### 3.4 分散度是占位值

旧输出的 symbol/group concentration 固定为 `0`，G1/G2/G3 也没有从成交重算。修复后从真实成交和
group realized PnL 输出：实际币数、单币 gross share、单币 abs net PnL share、单 group abs net PnL
share。

### 3.5 另外修复的回放边界

- 缺少任一腿同 timestamp completed bar 时禁止用 stale mark 伪造同步订单；
- deadline aggregate close 后允许后续新 cycle，避免该 group 永久停用；
- end-of-window 强平成本进入最终 equity/DD curve；
- funding 按当前 mark notional 和 cycle 冻结方向结算；
- M2 在动态方向和 factor-neutral notional 未实现前 fail closed。

当前修正版仍未实现完整 Binance maintenance margin/liquidation、symbol filter、quantity/price rounding、
partial fill 和 legging。复算行只能叫 corrected research diagnostic，不能叫 production-ready result。

## 4. High：计划执行不完整

### 4.1 M2 64 configs 被错误跳过

Round 18 计划明确写的是：

```text
M1 synchronized pair = 96
M2 residual basket    = 64
M3-M5 enhancement     = 32，仅父机制继续门通过后生成
```

只有 M3-M5 可写 `blocked_parent_gate`。M2 是独立主 family，不依赖 M1 通过。GLM 把 M2 和增强器一起标为
blocked，导致任务书要求的 64-config synchronized factor-residual basket 完全未执行。因此禁止使用
“valid novel families exhausted”。

### 4.2 G1/G2 不是逐 fold nested selection

`glm_r18_r6_g1_search.py` 和 `glm_r18_r7_g2_full_train.py` 都只加载 F2 fit，只在 2023 F2 train 中选参。
随后 G3 把这套 F2-selected 配置直接套到 F2/F3/F4 validation。任务书要求每个 anchored fold 独立完成：

```text
fit(train_f) -> G1(train_f) -> G2(train_f) -> commit selection_f -> validate_once(f)
```

所以原 576/960/24 计数不是完整 nested pipeline，且 M1 selection 还建立在错误方向/账务引擎上。

### 4.3 G1-G3 未执行完整 gate

- G1/G2 没有从结果校验 actual symbols 和 concentration；
- G3 没有输出/校验 symbol/group concentration；
- atomic reject 每分钟重复记录，最高单行超过 150 万次，没有 reason-specific cooldown；
- `groups_with_so=0` 的行仍可能进入普通收益比较，不是真实 Martingale；
- 没有五个 cold starts、DSR/PBO、selection frequency 或 rank degradation 的有效 finalist 结果。

## 5. High：ledger 和 production parity 不成立

### 5.1 历史去重只扫了 Round 17

`scan_historical()` 的注释声称扫描 Round 1-17，实际只读取：

```text
docs/superpowers/artifacts/glm-martingale-core-round17/exploration-registry.jsonl
```

没有扫描 R1-R16 的 registry、checkpoint、corrected authority 和 exact config，不能证明不重复。

### 5.2 设计好的 append-only registry 没被 runner 使用

`Round18Registry` 支持 `running -> terminal` 和 canonical fingerprint，但 G1/G2/G3 各自直接 append 简化
JSONL。终端行缺少 command、exit code、engine/data/plan/validator hash、完整 effective config 和五类
hash，也没有 immutable running 行。最后补的 gate JSON 是静态 stub，不是从中央 registry 重算。

### 5.3 production 只是未调用决策的 skeleton

`main.rs` 只执行 `sync_cycle_push_completed()` 和 `sync_cycle_residual_state()`，随后把 `_z` 丢弃；没有：

- 从配置或 SQLite load frozen fit；
- 调用 `sync_cycle_decide()`；
- 生成 N-leg order intent；
- partial fill/legging policy；
- SQLite cycle/group/leg persistence；
- restart + exchange reconcile + next-order suffix parity。

`sync_cycle_persist_state()` 也明确只是内存 HashMap。R10 的 production parity stub 不能替代 started-service
证据。

## 6. 审计修复与独立复算

修复文件：

- `apps/backtest-engine/src/martingale/sync_cycle_engine.rs`
- `crates/shared-domain/src/martingale.rs`
- `scripts/glm_r18_r4_fold_fit_freeze.py`
- `scripts/glm_r18_r2_selftest.py`
- `scripts/chatgpt_r18_independent_recheck.py`

新增并通过 8 个同步引擎测试，其中 8/8 通过：

```text
pair_entry_directions_follow_residual_sign_and_beta
pair_notionals_follow_absolute_hedge_beta
martingale_pnl_uses_fill_quantity_not_arithmetic_price_average
close_charges_fee_and_slippage
m2_fails_closed_until_factor_neutral_orders_are_implemented
stale_leg_marks_cannot_create_synchronized_orders
deadline_abort_does_not_disable_group_forever
forced_close_cost_is_in_final_equity_curve
```

验证命令：

```text
cargo test -p backtest-engine sync_cycle_engine --lib
cargo build --release -p backtest-engine --bin synchronized_cycle_replay
python3 scripts/chatgpt_r18_independent_recheck.py
```

使用修正后的 release binary 独立重跑全部 24 个旧 G3 行：

| 范围 | 数量/结果 |
|---|---:|
| completed | 24/24 |
| ann > 0 的诊断行 | 14 |
| 多币+分散+无 breach+真实 SO | 2 |
| 三档命中 | 0 |

关键行：

| key | ann | DD | symbols | max symbol | max group | 结论 |
|---|---:|---:|---:|---:|---:|---|
| `M1_007|2000|F4` | 10.9567% | 4.7377% | 8 | 27.03% | 69.81% | 单 group 超 50%，淘汰 |
| `M1_067|1000|F4` | 5.2086% | 1.2947% | 8 | 20.09% | 46.16% | 修正后共同门最佳，远低于目标 |
| `M1_044|1000|F3` | 3.1480% | 18.1831% | 6 | 32.84% | 46.98% | 过共同门，远低于目标 |
| `M1_085|1000|F4` | 4.4513% | 1.1427% | 8 | 30.28% | 42.88% | `groups_with_so=0`，不是有效 Martingale |

旧值被实质推翻的例子：

```text
M1_068|1000|F2
旧：ann=81.9612%, DD=62.3977%
新：ann=-20.2272%, DD=43.7747%
新 concentration：max symbol=50.08%, max group=67.56%
atomic rejects：1,527,926
```

这 24 行仍不是有效 finalist：它们由错误 G1/G2 selection 选出，且没有五 cold starts、liquidation、
exchange filters 或 live parity。其用途仅是证明旧收益值不可靠并确定修正后量级。

## 7. R0-R11 阶段审计

| phase | 权威状态 | 可保留范围 |
|---|---|---|
| R0 | partial_valid | R17 三个 no-op 修复及局部 activation 可保留 |
| R1 | invalid_scope | 只扫 R17；中央 registry 未被后续 runner 使用 |
| R2 | invalid_then_corrected | M1 原方向/账务无效；审计后 research engine 已修，M2 仍未实现 |
| R3 | materially_incomplete | 只有内存 helper/call site，无真实 order/persistence/reconcile |
| R4 | partial_valid | train-only pair fit 数据可作输入；静态 signs 已修为运行时动态 |
| R5 | invalid_for_search | 在错误成交路径上做 binding，不能证明修正后参数绑定 |
| R6 | invalid_results | 仅 F2、错误引擎、无中央 ledger、M2 缺失 |
| R7 | invalid_results | 仅 F2、错误引擎、未校验实际分散度 |
| R8 | invalid_results | 选自无效 G1/G2；24 行已作诊断性复算 |
| R9 | not_applicable | 无有效 finalist |
| R10 | not_applicable | 无 finalist，且 production skeleton 不满足 parity |
| R11 | superseded | 原 handoff 结论无效，由本文替代 |

## 8. 新增 never-repeat ledger

1. 禁止静态 `[1,1]` sign 伪装 residual pair；方向必须从 residual sign + beta 生成并进入 order hash。
2. 禁止用成交价算术平均计算多层 Martingale PnL。
3. 禁止免收平仓 fee/slippage，禁止 terminal close 不进入 DD。
4. 禁止把 margin 当 group gross notional cap。
5. 禁止 concentration 占位 `0` 或用配置币数代替实际成交币数。
6. 禁止只在 F2 train 选参再称为逐 fold nested WFO。
7. 禁止把独立主 family M2 写成 `blocked_parent_gate`。
8. 禁止每分钟重复永久不可能成功的 minNotional/gross-cap rejection。
9. 禁止用 gate JSON stub 代替中央 append-only registry validator 重算。
10. 禁止把只 push bar/read z、未 decide/order/persist/reconcile 的 call site 称为 production parity。

## 9. 2026-07-17 外部检索结论

本次通过 Crossref、Semantic Scholar 和 OpenAlex API 扩大检索。文献只用于定义可证伪机制，不用于
外推收益，更不能证明 50/90/110% 可实现。

| 来源 | Round 19 可转化机制 | 防误用限制 |
|---|---|---|
| Gregory/Hansen, `10.1016/0304-4076(69)41685-7` | train/online break diagnostic；level/slope break 后 block FO、freeze SO | break test 不产生独立收益 |
| Eroglu/Miller/Yigit, `10.1080/07474938.2020.1861776` | 区分 no/fixed/time-varying cointegration | 论文明确警告 Kalman state-space 易受伪回归；未过 spurious-control 不得运行 |
| Clegg/Krauss, `10.1080/14697688.2017.1370122` | partial cointegration：把 residual 分为 random-walk 与 mean-reverting component | 只交易 MR component；random-walk share 超阈值 fail closed |
| d'Aspremont, `10.1080/14697688.2010.481634` | sparse canonical-correlation mean-reverting portfolio | 必须 5+ 实际币，禁止稀疏成单币/双币 |
| `10.1111/anzs.12304` | sparse VECM/adaptive-Lasso cointegrating vector，多币 market-neutral basket | 只在 train 拟合 penalty；validation 不按收益调权 |
| `10.1016/j.automatica.2019.108651` | penalized-likelihood sparse mean-reverting portfolio | 只作为 M2B 权重候选，不照搬论文收益 |
| `10.1016/j.orl.2018.01.006` | robust dynamic cointegration 作为时变参数对照 family | 必须和 fixed-beta control 同 fold 比较并经过成本/断点 gate |

最有信息增益的新方向不是再次扩大 FO/multiplier 网格，而是：

1. 修正后的 static M1 作为 control，完整逐 fold 重跑；
2. 补执行从未跑过的 M2 factor-residual basket；
3. partial-cointegration residual Martingale；
4. spurious-control Kalman + break-aware online state；
5. sparse VECM 5+ 币 aggregate Martingale basket。

所有方向中，方向和 residual 只负责判断 FO/SO/TP；收益必须来自真实 aggregate Martingale cycle，禁止
独立 stat-arb sleeve、shadow curve 或非 Martingale alpha。

## 10. 下一步

Round 19 必须先通过 engine/accounting/exchange/registry fail-close，再执行每 fold 独立 selection。不得复用
Round 18 的 checkpoint 作为结果；只能把 exact fingerprint 写入失败 ledger 用于避免无意义重复。

公开研究没有提供可直接复制、同时满足小资金、低 DD 和 50/90/110% 的保证。Round 19 的设计目标是
修复有效性并提高命中概率，不承诺一定命中；如果仍未命中，必须输出可信失败边界，而不是新的伪完成。
