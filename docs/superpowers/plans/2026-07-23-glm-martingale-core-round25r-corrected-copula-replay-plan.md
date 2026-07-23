# GLM Martingale Core Round 25R：Corrected Copula Martin 唯一任务书

制定日期：2026-07-23。

本任务不是 Round 26 新搜索，而是对无效 Round 25 的强制修正与重放。必须先恢复可审计 baseline，再决定
收益与回撤是否有提升空间。禁止增加普通 multiplier、spacing、TP、EMA/ADX/RSI 或新收益 family。

## 0. 唯一权威与状态

只允许继承：

1. `docs/superpowers/reports/2026-07-23-chatgpt-round25-independent-audit-and-corrective-direction.md`
2. `docs/superpowers/artifacts/glm-martingale-core-round25-recovery/audit/round25-independent-audit.json`
3. `docs/superpowers/artifacts/glm-martingale-core-round25-recovery/audit/round25-corrected-failure-ledger.jsonl`
4. Round 24 corrected authority、Round 25 source map、data manifest 和原 16-policy manifest。

现有 Round 25 recovery authority 被审计结果替代：

```text
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

16 个 replay 计入 trial ledger，但不得作为 valid failures、frontier 或 fingerprint closure。

## 1. 目标与不可变约束

| 档位 | annualized | max equity DD | cold starts | leverage cap |
|---|---:|---:|---:|---:|
| 保守 | `>=50%` | `<=10%` | `>=4/5` 正，另报 5/5 | `<=2x` |
| 平衡 | `>=90%` | `<=20%` | `>=4/5` 正，另报 5/5 | `<=3x` |
| 激进 | `>=100%` | `<=30%` | `>=3/5` 正，另报 5/5 | `<=4x` |

硬约束：

- principal 严格 `<5000U`；
- 最终候选实际成交 `>=6` alts、`>=3` distinct pairs；
- 一个连续 shared account，不合并独立 finished curves；
- 收益只能来自 C1 Martin FO/loss-after-add SO/TP/reduce/abort/funding；
- BTC 只作 reference，不得产生 order/trade/PnL；
- completed signal 后下一 1m event 才能成交；
- fit、pair、copula、threshold 不读取 outer return；
- 所有无信号、拒单、no-fit、失败 block 保留在分母。

## 2. Git 与 artifact 协议

回放只要求 clean immutable local commit：记录 commit、tree、dirty=false。push 失败写 `PUSH_PENDING` 并继续，
不得再次把网络权限作为 G0/G1 blocker。

新 root：

```text
docs/superpowers/artifacts/glm-martingale-core-round25-corrected/
docs/superpowers/reports/2026-07-XX-glm-round25-corrected-handoff.md
```

raw traces 不直接提交 Git：

```text
local raw root: artifacts-local/round25-corrected/<source_commit>/
repo evidence: trace manifest + row count + bytes + SHA256 + schema
               + first/last rows + deterministic 100-row sample
               + independently recomputed metrics
single committed file hard cap: 10MB
```

raw root 必须可由 source/data/policy hash 重建。为 `artifacts-local/` 增加 `.gitignore`。旧 oversized commit 只作
本地审计证据，不作为 corrected branch 的远端 parent；不得删除用户现有 branch 或 raw files。

## 3. R0：先写失败测试，再修账户

以下测试必须先在旧实现失败，修复后通过：

```text
closed_group_releases_all_owned_reserve_exactly
one_hundred_close_cycles_leave_reserved_quote_zero
so_consumes_old_reserve_then_replaces_next_layer_reserve
last_layer_keeps_close_reserve_only
failed_fo_releases_group_and_pending_leg_reserve
end_close_leaves_zero_positions_groups_pending_and_reserve
replay_restart_preserves_component_reserve_ledger
```

把单一 `reserved_quote` 扩展为可复算 owner/component ledger，至少区分：

```text
next_so_initial_margin / next_so_entry_cost / open_close_cost
maintenance_buffer / pending_leg / hedge_or_flatten
```

规则：

1. FO 成功后持有下一层 reserve；
2. SO 前原子消费当前 next-SO reserve，成交后立即计算下一层 reserve；
3. SO 失败必须恢复正确 reserve 或按预注册规则 paired close；
4. TP/abort/end-close/liquidation 后释放该 group 全部 reserve；
5. 每个 event 强制 `reserved_quote == sum(component ledger)`；
6. final close 后 positions/groups/pending/reserve 全为 0；
7. reserve 变化必须有真实 timestamp，禁止 timestamp=0 placeholder。

## 4. R1：修复 filter-feasible 小资金 sizing

旧 20U group FO 作废，因为无法交易 ETH/LINK。corrected G1 calibration：

```text
principal: 2000U
group FO target gross: 50U
relative layers: [1.00, 1.25, 1.55, 1.90]
target layer gross: [50.0, 62.5, 77.5, 95.0]U
max active groups: 3
leverage cap: 2x
```

50U 是由 `2 * max eligible minNotional(20U) * 1.25 buffer` 得到的可执行性修正，不是收益调参。

每层先解析两腿 filter：price 向不利方向取整，quantity 按 step 得到可成交值，resolved leg gross 必须
`>=minNotional`，两腿 gross mismatch `<=5%`，且总 resolved gross 不突破 group/account cap。不得用 theoretical
50/50 权重替代真实 quantities。

必须测试 ETH、LINK 在历史高低价下都存在可成交数量，并证明六币 gate 不再结构性不可达。低于可执行
minimum principal 的 budget 允许 valid reject，但最终至少两个相邻 `<5000U` budgets 通过。

## 5. R2：正确实现 reference-spread Copula

每个 outer block 只用 fit window：

1. 对每个 alt 拟合 `S_i=log(BTC)-beta_i*log(alt_i)`；
2. EG-ADF `p<=0.05` 且 KPSS `p>=0.05`；
3. half-life、CUSUM/break、tail sample 和 round-trip cost feasibility 全通过；
4. empirical CDF 可使用排序副本，但 dependence 必须使用 timestamp-aligned unsorted observations；
5. 用 aligned spreads 计算 Kendall tau；Gaussian rho 使用 `sin(pi*tau/2)` 作为起点；
6. Gaussian 与 Student-t (`nu=3..30`) 只按 train log-likelihood/AIC 选择；
7. persist family、rho、nu、loglik、AIC、sample count、fit cutoff 和 independent reference error；
8. maximum-weight disjoint matching 只使用 train stationarity、copula fit、tail sample 和 cost score；
9. active group 始终使用 open-time frozen model，selector roll 不改变旧 cycle；
10. sorted marginal values的排列变化不得改变 dependence estimate。

必过测试：

```text
copula_dependence_uses_timestamp_alignment_not_sorted_rank
permuting_time_order_changes_dependence_but_not_empirical_marginal
gaussian_and_student_t_aic_match_independent_reference
failed_adf_or_kpss_pair_never_reaches_order_path
future_shift_changes_fit_only_after_cutoff
active_cycle_keeps_frozen_family_rho_nu_across_roll
```

## 6. R3：SO、退出与 1m 实盘路径

SO production state 不得有常量字段：

```text
group_net_after_close_cost < 0
signed adverse distance from last filled layer >= frozen step
same tail from frozen family/rho/nu
one-bar adverse increment no longer worsening
rolling completed-bar stationarity valid
next actual resolved layer gross > previous
reserve/filter/concentration pass
```

`worsening` 从连续 completed signal observations 计算；`rolling_stationarity` 从冻结规则与过去窗口计算。
每个 decision trace 保存输入和结果。

信号仍为 completed 5m/1h，但 account 必须处理每根 1m mark、funding、pending leg、maintenance 和 liquidation。
使用 1m high/low 的预注册不利路径检测 bar 内 liquidation，不能只看 signal close。TP 只在 all-in group net positive
且 conditional probabilities 回到 neutral band 时执行。

一个 pair 只有发生 `neutral reset -> fresh tail crossing` 才能新开下一 cycle，禁止持续 tail 每根 bar 重复提交。
这属于 live order-state 修复，不是 alpha 参数。

## 7. R4：完整指标与独立 validator

每个 policy 必须持久化：

```text
principal/start/end/days/final wallet/final equity
compounded return/annualized return
equity DD/balance DD/DDR/Sharpe/Sortino/Calmar/tail loss
12 block start/end/raw return/local DD/PF/positive flag
FO/SO/TP/reduce/abort/reject/partial/legging/liquidation counts
actual symbols/pairs/signed legs/rounded quantities
gross profit and fee/slippage/funding/close/legging cost components
cost-to-gross-profit
symbol/group/block positive contribution and dynamic freeze events
peak gross/effective leverage/maintenance/all reserve components
final positions/groups/pending/reserved_quote
```

P-B 必须同时满足：

- production/account/trace parity 全过；
- compounded return `>0` 且 `>=8/12` positive blocks；
- actual assets `>=6`、distinct pairs `>=3`；
- 至少两个 groups 有真实 loss-after-add SO；
- symbol/group/block positive contribution 各 `<=50%`；
- all-in cost/gross profit `<=50%`；
- 无 liquidation、principal breach、BTC order、future/stale signal 或 unreconciled field。

validator 独立读取 raw traces，不调用 replay summary 的指标函数。至少手工复算 wallet、reserve、cost、funding、
DD、block return、concentration 和 annualization。registry wall time/RSS 从真实进程采集，不得写常量。

## 8. G0：corrected production gate

G0-S 必须调用与 G1 相同的 production functions。除旧 canaries 外必须通过 R0–R3 全部新增测试。

G0-R 运行旧四个 fixed windows，至少证明：

- 真实 pair/model 含 family/rho/nu/fit cutoff；
- ETH 或 LINK 的 filter-feasible submit/reject path 被真实调用；
- reserve 在成功 cycle 关闭后回到前值；
- SO state 没有 hard-coded rho/worsening/stationarity；
- BTC order/trade 为 0；
- 1m risk path 行数与数据时间范围一致；
- raw trace manifest 与本地文件 SHA256 一致。

实现失败先修复，不得用 `blocked=true` 或常量 canary 提前结束。

## 9. G1：原 16 policies corrected replay

保持原四维 population，不扩参数：

```text
frequency: 5m / 1h
lookback: 60d / 120d
entry alpha: 0.05 / 0.10
SO step: 0.50 / 0.75 train sigma
```

先跑完整 12-block activation census，不设 20,000-step 截断；再跑 16 个 continuous shared-account replays。
每 policy 独立 checkpoint，支持 resume。16 个都必须结束为 valid survivor 或 exact valid failure，不能首个失败即停。

若 0 个 P-B：结束为 `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET`，但只关闭 corrected exact fingerprints。
若有 P-B：最多四个按冻结 P-B/DDR/concentration 规则晋级，不按目标收益事后挑选。

## 10. G2：仅对 P-B survivors

依次执行：

- budgets `500/750/1000/1500/2000/3000/4000/4999U`；
- cold starts `0/30/60/90/120d`；
- fee/slippage/funding `1x/1.5x/2x`；
- partial fill、1/2/3 bar leg delay、one-leg reject；
- minNotional 2x、coarser tick/step、maintenance 5%；
- signal missing/stale、kill/restart/reconcile；
- LOSO/LOGO、worst-combined stress、exact minimum principal；
- exact 16-policy CSCV/PBO、DSR global trials、SPA/reality check、bootstrap CI、neighbor stability。

三档 sizing 只对同一 survivor 做预注册比例映射，不改变 entry/exit/pair model：

```text
conservative: corrected baseline, cap 2x
balanced:     all layers x1.5, cap 3x
aggressive:   all layers x2.0, cap 4x
```

每档重新跑完整 account，不得线性放大 equity curve。

## 11. 条件增强

只有 corrected C1 parent 通过 P-B，才运行：

```text
C2 asymmetric lower/upper-tail FO/SO veto
deficit scheduler versus static scheduler
```

C2/scheduler 只能 veto、排序或冻结已有 Martin actions，不得产生独立 PnL、加 leverage 或改变方向。对照臂与
增强臂都必须完整重放，真实 order/rejection hash 必须不同。没有 P-B 时全部 `not_applicable`。

## 12. 执行命令

允许按 crate 最小调整 binary 名，但 phase 语义不可省略：

```bash
cargo fmt --package r24-engine --package r24-research -- --check
cargo test -p r24-engine
cargo test -p r24-research
cargo run --release -p r24-research --bin r25_corrected_execute -- --phase g0 --resume
cargo run --release -p r24-research --bin r25_corrected_execute -- --phase g1 --resume
cargo run --release -p r24-research --bin r25_corrected_validate
```

有 P-B 才继续：

```bash
cargo run --release -p r24-research --bin r25_corrected_execute -- --phase g2 --resume
cargo run --release -p r24-research --bin r25_corrected_validate
```

## 13. 必交产物

```text
round25-corrected-authority.json
round25-corrected-execution-state.json
round25-corrected-protocol.json
round25-corrected-policy-manifest.json
round25-corrected-data-manifest.json
round25-corrected-trial-ledger.json
round25-corrected-failure-ledger.jsonl
exploration-registry.jsonl
gates/r0-r4.json
gates/g0.json
gates/g1.json
gates/g2.json (conditional)
trace-manifests/**
replay-results/**
independent-validator.json
2026-07-XX-glm-round25-corrected-handoff.md
```

每次 commit body 必须包含 `问题描述`、`复现路径`、`修复思路`。本地 commit 即可启动回放；push 失败只记
pending。最终 worktree 必须 clean。

## 14. 停止条件

只有以下情况可停止：

1. 16 个 corrected G1 policies 全部形成可独立复算的 terminal；或
2. 同一外部数据/硬件 blocker 经三次可复现修复仍无法解除。

编译错误、运行慢、no-fit、无收益、push 失败、oversized trace、某个 policy 失败都不能提前结束。禁止在
reserve reconciliation、filter feasibility、Copula alignment、1m risk path 或 validator 任一未通过时发布收益结论。
