# Round 26 独立审计与 Round 27 方向

审计日期：2026-07-26。

## 1. 最终结论

Round 26 按任务书的字面停止条件跑到了终态，但没有达到用户目标，也不能关闭整个 weekly reference-Copula
Martin 方向。修正状态为：

```text
MATERIALLY_INCOMPLETE_INVALID_FAMILY_CLOSURE
strict valid candidates = 0
strict valid 5/5 candidates = 0
target hit = false
Martin return replays = 0
```

准确含义：`VALID_ACTIVATION_NO_REPLAY_CANDIDATE` 只证明 Round 26 的**精确六重联合准入门**产生零激活；
它没有测试 Copula pair、Martin 收益、三档、预算、cold starts 或实盘压力，不能外推为整个 family 无效。

修正机器权威：

```text
docs/superpowers/artifacts/glm-martingale-core-round26/audit/
  round26-corrected-authority.json
  round26-independent-gate-decomposition.json
  round26-corrected-failure-ledger.jsonl
  round27-external-source-update.json
```

## 2. 已完整执行的部分

| 项目 | 实际证据 | 结论 |
|---|---:|---|
| D0 | 29/29 alts 数据与 filters 通过 | `PASS` |
| snapshots | 608 = 2 frequency x 2 formation x 152 rolls | `PASS` |
| independent legs | 9,570，最大数值误差 `6.08e-13` | `PASS` |
| G1 activation | 8 configs x 152 rolls | `PASS` |
| G0 terminals | 8，合计 158,400 个 1m 静态账户行 | `PARTIAL` |
| account validator | 8/8 静态终态归零 | `PASS_FOR_ZERO_ORDER_TRACE` |
| tests | Rust `23+34` tests；Python compile；Rust fmt | `PASS` |
| Git | 原分支 clean，`f2896787` 已与远端同步 | `PASS` |

G2/G3 因零 activation survivor 未执行，符合原任务书停止规则；但这也表示收益、DD、`<5000U`、多币成交、
cold starts 和压力均为**未评估**，不是零收益或未命中后的有效回测。

## 3. 关键问题

### 3.1 CUSUM 单门清空 9,570 个模型

生产代码把以下条件一次性做 AND：

```text
coverage
half-life
KPSS
CUSUM p >= 0.05
beta drift <= 25%
leg tail excursions >= 20
EG RAW/FDR
```

独立分解显示：

```text
CUSUM pass = 0 / 9570
maximum CUSUM p = 0.012006575985620636
```

Round 25R 的失败台账已经记录过：对自相关残差使用未做 Newey-West 长期方差调整的 CUSUM 会清空拟合。
Round 26 又直接使用 `breaks_cusumolsresid` 的 Brownian-Bridge p-value作为硬门，形成已知回归。

这不表示 structural-break 风险不重要；正确做法是把 break 指标改为 ranker/runtime veto，或先完成 HAC/bootstrap
校准，不能未经全量普查直接成为准入硬门。

### 3.2 即使移除 CUSUM，其余联合门仍过稀疏

| config | EG+KPSS+HL legs | rolls >=2 / >=4 | 再加 drift+tail20 legs | 后者 rolls >=2 |
|---|---:|---:|---:|---:|
| 1h/14d RAW | 123 | 23 / 8 | 6 | 0 |
| 1h/21d RAW | 116 | 22 / 8 | 20 | 2 |
| 5m/14d RAW | 24 | 5 / 0 | 12 | 1 |
| 5m/21d RAW | 21 | 4 / 1 | 11 | 3 |

因此只修 CUSUM 仍不足以进入原 activity gate。`tail>=20` 是单 BTC-alt residual 的门，但真正交易的是两个
alt residual 的 Copula divergence；把它放在 pair formation 前，统计对象与最终信号不一致。beta drift 的分母
又是 first-half beta，接近零时会出现极端比率，观测最大值达 `128,718`。

Round 27 必须拆成：基础统计 admission、稳定性 ranking、pair-level cost feasibility、运行时 break veto。

### 3.3 GO 普查不是 production-equivalent

Round 26 GO 报告的 1h 14d `36/14`、21d `31/13` 是全 29-alt 的 EG+KPSS+finite-half-life 普查；生产则使用
weekly Top-20，并增加 CUSUM、drift、tail20。production-equivalent 基础值实际是 `23/8` 和 `22/8`。

后续任何 GO census 必须由 production 消费同一个 manifest/hash，不能用较宽普查为较窄联合门背书。

### 3.4 Validator 与 G0 只验证了零订单路径

Model validator 数值复算有效，但 `checked_pair_count=0`。因此 Copula、pair cost、matching 没有真实数据 canary。
G0 的 8 个账户 trace 全是常数 2000U、零订单、零成交；它能证明空账户对账，不能证明 selector 到 Martin
order 的生产绑定。`production_parity=true` 目前只是结果字段。

Round 27 G0 必须同时包含一个确定性 synthetic Martin 路径和按时间选出的第一个 return-blind real pair 路径。

### 3.5 closure 范围过宽

可以关闭：

```text
EG + KPSS + half-life + raw CUSUM + beta drift + tail20 的精确联合 admission
```

不能关闭：

```text
weekly Top-20 reference-Copula Martin family
staged EG/KPSS/half-life admission
pair-level fixed-threshold cost-aware Martin
PBD finite-persistence Martin
```

## 4. 用户目标判定

| 目标 | Round 26 结果 |
|---|---|
| 防过拟合 | return-blind snapshots 有效；没有收益 population，无法做 PBO/DSR/SPA |
| `<5000U` | 未跑 budget replay |
| 多币组合 | 数据池 29 币；实际成交 0 币/0 pair |
| 保守 `50%/10%` | 未评估，未命中 |
| 平衡 `90%/20%` | 未评估，未命中 |
| 激进 `100%/30%` | 未评估，未命中 |
| 实盘复现 | 空账户 trace 可复算；真实订单路径未验证 |

## 5. 外部扩大检索后的取舍

1. `10.1186/s40854-024-00702-7` 仍支持 3-week formation、1-week trading、reference-Copula 的
   source-faithful control。论文没有 Round 26 的六重硬门。
2. `10.1108/CFRI-11-2024-0727` 在 top-50 Binance crypto、牛/震荡/熊三段中比较 distance、cointegration
   和 hybrid，报告固定 threshold 优于 dynamic threshold。Round 27 保留固定 alpha，不按结果调 threshold。
3. `10.1007/s10614-023-10539-4` 支持用预期 convergence time 排除慢回归 spread；Round 27 只把它作为
   ranker/veto，不重跑已关闭 Johansen/VECM family。
4. `10.7494/manage.2019.20.2.151` 的 PBD 把永久、有限持久和微观噪声冲击分开，熊市表现更稳，属于尚未
   执行的明确机制。但其真实等待、成交量、成本约束下只有 `9.16% ann / 46.37% MDD`；移除执行限制后
   `138.6%` 被作者明确解释为 delay-zero 偏差。它只能是条件 activation arm，不能继承高收益。
5. structural-break RL、DNN/LSTM ensemble、GA triple-barrier 均扩大过拟合面，且部分来源不含完整成本；
   Round 27 拒绝这些黑箱方向。

外部搜索仍没有找到一条同时证明 `<5000U`、多币共享账户、Martin-only、真实成交和三档收益/DD 的公开策略。
所以目标保持不变，但任何 agent 都不得承诺下一轮必然命中。

## 6. 下一步

Round 27 只执行：

```text
docs/superpowers/plans/2026-07-26-glm-martingale-core-round27-staged-admission-persistence-recovery-plan.md
```

核心顺序：先复现上述门槛分解并撤销过宽 closure；再跑 source-faithful EG control、staged robust arm 和条件
PBD arm；任何 arm 有真实 pair activity 才进入同一个 production-conservative shared-account Martin replay；
最后才跑三档、budgets、cold starts、stress 和 anti-overfit。禁止 30 天监控，直接历史 prequential 回测。
