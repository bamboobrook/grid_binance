# Round 1-23 统一审计、Round 23 修正与目标可行性判断

日期：2026-07-23

审计的 GLM HEAD：`9c8be3fab6aa359a27569d0628a2a2a31c8d7bf2`

最新用户目标：

| 档位 | stitched annualized | max equity DD |
|---|---:|---:|
| 保守 | `>=50%` | `<=10%` |
| 平衡 | `>=90%` | `<=20%` |
| 激进 | `>=100%` | `<=30%` |

共同要求仍为：Martin 是唯一收益引擎、本金严格 `<5000U`、实际成交至少 5 个基础资产、一个连续共享
cash/margin/equity/reserve 账户、防过拟合、完整成本和实盘适配器可复现。

## 1. 最终答案

Round 23 没有完整执行，原来的 `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET` 必须撤销。修正状态是：

```text
MATERIALLY_INCOMPLETE_INVALID_RESULTS
strict-valid experiments = 0
strict-valid policies = 0
strict-valid 5/5 policies = 0
target_hit = false
```

Round 1-23 在**最新共同合同**下也没有一条严格有效候选。这个结论不等于“所有回放都没价值”：

- R4、R7、R15、R18、R19、R20、R22 和 R23 的若干数字可以复现；
- R8/R9/R11 的曲线组合提供了分散化线索；
- 但“数字可复现”和“组合可晋级实盘”是两件事；
- 缺失的 argv、trace、fit cutoff、订单事件和历史 trial matrix 无法事后伪造，正确修正只能是隔离无效结果，
  不是把 23 轮全部改写成有效。

### 是否出现过 5/5 全正

出现过，但没有严格有效的 5/5：

| 结果 | 5/5 含义 | 为什么不能晋级 |
|---|---|---|
| R9 `64.42%/18.21%` | 5/5 curve slices | 成品曲线分配，不是事件级共享账户 |
| R11 `62.78%/18.38%` | 5/5 curve slices | 同上，只补了 funding |
| R15 source B | 5/5 legacy segments | 收益/DD 不达标，计划机制和生产接线不完整 |
| R19 `41.43%/6.27%` | 5/5 inner-train slices | inner-fit 泄漏、集中度约 56%、没有 validation |
| R23 BTC 修正版 | 5/5 cold starts | 单币、RelaxedM1、无 funding、抗过拟合不可审计 |

因此答案是：**观察层面有 5/5，严格共同合同下仍是 0 个。**

## 2. Round 1-23 统一结果

下表的“代表值”是每轮最值得保留的观察值，不代表有效晋级。机器明细见
`round1-23-consolidated-frontier.json`。

| Round | 代表 ann / DD | 稳定性 | 修正后的用途 |
|---:|---:|---|---|
| 1 | `22.2 / 26.1` | 3/5 | 旧基线；本金 5000U，不满足 `<5000U` |
| 2 | `28.6 / 22.5` | 4/5 | partial-TP/conditional-SO 研究线索 |
| 3 | `34.5 / 17.8` | 4/5 | 低 DD 旧前沿；计划与实盘证据不完整 |
| 4 | `34.81 / 17.67` | 4/5 | canonical event-level 可复算；低预算和集中度失败 |
| 5 | `59.5 / 32.1` | 4/5 | 旧 ANKR 前沿；DD 超限且后续账务已变化 |
| 6 | `55.8 / 25.0` | 4/5 | quarantine 线索；DD、状态机和实盘语义失败 |
| 7 | `62.51 / 28.62` | 4/5 | 六币可复算；1000-4000U ann 仅 1.28-5.51%，holdout 负 |
| 8 | `60.4 / 18.2` | 无有效 5 分段 | 修正时序后的 Python 曲线分配，仅研究 |
| 9 | `64.42 / 18.21` | 5/5 curve slices | 8 币曲线诊断；非事件级、非共享账户 |
| 10 | 沿用 R9 | inherited | 没有把 R9 转成可执行组合 |
| 11 | `62.78 / 18.38` | 5/5 curve slices | funding 修正曲线诊断；仍不可执行 |
| 12 | `34.72 / 17.68` | 4/5；holdout 全预算负 | R4 修正版；dynamic allocator 未完成 |
| 13 | `62.51 / 28.62` | 4/5 | 沿用 R7；新 `65.75/29.57` 只有两币且 TRX 89.58% |
| 14 | `28.77 / 21.80` | 3/5 | XS 修正版；原 `50.7`、`35.07` 均撤销 |
| 15 | `42.11 / 29.90` | cold starts 负且穿仓 | exact long-only scoped diagnostic |
| 16 | `-2.11 / 31.76` | 0/5 | 修正后 0 survivor；新机制没有进入 scored path |
| 17 | `-34.11 / 11.10` | 7 arms 完全相同 | 机制 inert，不能称完成搜索 |
| 18 | `5.21 / 1.29` | 14 条正诊断行 | 8 币共同门最好诊断，离目标很远 |
| 19 | `41.43 / 6.27` | 5/5 inner-train | 最重要线索；泄漏、集中度、交易所模型未过 |
| 20 | `17.72 / 4.55` | diagnostic | 可复算，但 registry、family、causality 失败 |
| 21 | `-52.05%` compounded / `50.96 DD` | 最多 5/12 | `158.46% ann` 是选中的 90 天短块，已撤销 |
| 22 | `31.19 / 9.87` | 6/12 | Python 引擎可复算但量纲/保证金错误、仅两币 |
| 23 | `19.59 / 20.82` | 5/5 cold starts | 修正 BTC 取证值；单币且抗过拟合不可审计 |

这 23 行不能直接互相排名，因为早期“5 分段”、后期 cold start、90 天 block、finished equity curve 和
continuous event account 的统计定义不同。统一合同只给出一个可比较结论：`strict_valid=false` 共 23/23。

## 3. Round 23 完整性审计

### 3.1 Registry 与 authority

独立读取当前 141 行 registry：

| 项目 | 审计值 |
|---|---:|
| running / terminal | `66 / 75` |
| unique experiment IDs | `75` |
| 只有 terminal、没有 running | `9` |
| `raw_argv=[]` | `141` |
| `wall_seconds<=0` / `rss_kb<=0` | `141 / 141` |
| 空 `fit_cutoff_utc` | `141` |
| 非空且 start=end | `132` |
| 含已知空流 SHA256 | `91` |
| 任一 trace hash 等于所谓 git commit | `35` 个 terminal |
| 缺失/非法必需 hash | `9` |
| 64 字符“git commit”标签 hash | `132` |
| 真实 40 字符 Git commit | `9` |
| 仓库内真实 trace/stdout 文件 | `0 / 0` |

另外：

- authority 写 `git_dirty=true`；
- execution state 仍列 `R2/R5/G0` 未完成；
- state/authority/handoff 分别使用 132/141 行、1471/1633 次；
- authority 最优是 `25.69/23.54`，state 最优是 `17.72/29.52`；
- 实际 state machine 在 R0 后固定返回 `BLOCKED`，所谓 valid 状态不是由它推导。

所以“invariant-clean、全阶段完成、validator-generated handoff”均不成立。

### 3.2 数据覆盖

| 数据 | 记录 | 失败/重复 | 实际覆盖 | 结论 |
|---|---:|---:|---|---|
| metrics | 15122 | 45 checksum fail；6532 duplicate groups | 2023-07-01..2026-05-31 | 缺 2023H1，不符合协议 |
| bookDepth | 2121 | 12 checksum fail；307 duplicate groups | 5 币，主 manifest 到 2024-06 | 不完整 |
| bookDepth extension | 1817 | 9 checksum fail | 5 币，2024-07..2025-06 | 仍不到 2026-05 |
| aggTrades | 90 | 1 checksum fail | 6 币，仅 2023-07-01..07-15 | 不能称 1066 天 M2 |

Round 23 M2 从未拥有完整、干净、可去重的订单流数据。

### 3.3 实现偏差

1. M1 的 scored binary 明确使用 `RelaxedM1`，只看 price extension/OI；计划要求的 taker extreme、
   top/all crowd 和 deceleration 没有同时进入 FO，SO 也被无条件放开。
2. M2 把计划的 `0.2%/1.0%` 深度擅自映射为 `1%/2%`，current aggTrade bucket 直接进入信号。
3. 多币 grid 给每币独立账户和 reserve，最后相加 equity；这不是共享 cash/margin/equity。
4. `max_sym_gross` 实际是期末 equity share；PBO 把 symbols 当 policies；多处 DSR 用 `n_trials=1`。
5. G1 每 block 新建 engine，cycle 被重置；空 block 被跳过；DD 是各 block DD 的最大值。
6. partial-fill/delay/reject stress 只是抬高 slippage 或 filter，不产生真实订单事件。
7. LOSO 是逐币单独跑，不是从多币 policy 删除一个币；funding 没进入 scored gated-Martin。
8. 540 个 MicroIntegral cells 来自 SSRN 标题/摘要重建，违反“无 PDF hash/公式页码禁止实现”。

## 4. 已复算和修正

### 4.1 账务修正

`gated_martin.rs` 已修复：

- SO ladder 不再跳过 `1.25x`；
- opening/closing fee 与 slippage 进入 cycle net PnL；
- reserve 覆盖 margin、maintenance、close cost 和下一层 SO；
- leverage cap 实际约束 projected notional；
- event-time maintenance/liquidation；
- forced close 写入最终 equity；穿仓终止在 0，不再二次平仓；
- max DD 使用连续 running peak；
- 报告实际 peak leverage 和累计成本。

### 4.2 BTC 独立复算

旧引擎可精确复现 GLM 的 `25.69% / 23.54%`。修正引擎结果：

| 指标 | 修正值 |
|---|---:|
| annualized | `19.5927%` |
| max equity DD | `20.8213%` |
| ending equity | `1686.3U` |
| ann Sharpe（只报告） | `0.9560` |
| cold starts | `5/5` 正 |
| cold-start ann range | `19.59%-20.86%` |
| cold-start DD range | `20.82%-21.40%` |

即便先忽略单币问题，抗过拟合也没有通过：

- 可见 R23 单币 648 cells + multi 432 cells，试验数下限已是 `1080`；
- 项目本地近似 DSR 在该下限为 `-44.2264`，不是原来的正值；
- 这个 DSR 实现本身不是正式统计 authority，只能说明 `n_trials=1` 明显错误；
- PBO `0.48` 只来自事后选出的 8 个配置，不是完整 policy-return matrix；
- 所以正式结论是 `anti_overfit=not_auditable`，不是 pass。

同时它仍然只有 BTC、没有 funding、没有 shared multi-symbol account，且信号是 RelaxedM1。因此这个
`5/5` 只能保留为单币取证结果。

## 5. 目标是否过高

不能据当前无效搜索证明“数学上不可能”，但当前没有严格证据支持 50/90/100。目标隐含的最低
ann/DD 比率为：

| 档位 | 最低 ann/DD（近似 Calmar 门） | 判断 |
|---|---:|---|
| 50% / 10% | `5.0` | 对长期多币 Martin 极高；当前无严格证据 |
| 90% / 20% | `4.5` | 需要独立 alpha、杠杆和很强尾部控制 |
| 100% / 30% | `3.33` | 理论上可达，但实盘尾部风险、成本和参数选择风险很高 |

公开高收益 Martin 案例通常至少占一项：单品种、1:500 一类高杠杆、短窗口、只报 balance DD、
equity DD 极高，或不披露订单/成本。不能用这些案例证明本项目的小资金、多币、低 equity DD 可以同时成立。

基于可复算证据，合理的分层预期应是：

| 历史研究区间 | 定位 |
|---|---|
| `15%-30% ann / 10%-25% DD` | 第一条可信严格组合应先达到这里；目前仍未证明 live |
| `30%-45% ann / 15%-30% DD` | 进取前沿；R4/R19 说明值得继续修复，但还没有有效候选 |
| `>=50% ann / <=10% DD` | 继续保留为正式保守目标，但应承认是 Calmar>=5 的高难度目标 |

建议**不降低用户的正式三档目标**，但 Round 24 增加一个不等于目标命中的进展门：

```text
P-C progress = ann >=35%, DD <=20%, >=4/5 cold starts,
               actual assets >=5, concentration pass, shared account,
               trial correction and production adapter parity pass
```

这能区分“确有推进”和“又跑一轮无效大网格”。

## 6. 下一步最有希望的方向

### 第一优先：R19 causal residual multi-pair Martin

R19 `41.43%/6.27%` 是 23 轮中最接近保守目标形状的线索。它不是候选，但失败点是可修的：

- rolling fit 必须只读 cutoff 前数据；
- train-only disjoint pair graph，避免同一资产重复占用多个 group；
- chronological event-level group quota，解决 56% 集中度；
- 完整 shared margin/reserve/filter/liquidation/funding；
- outer block 不得选 pair、阈值或预算。

如果修正后还能保留 `>=35%/<=20%`，就是一个真实推进；若完全消失，也能有效关闭这个 exact fingerprint。

### 第二优先：R9 allocator 事件级重建

R9 的 `64.42%/18.21%` 说明“不同 Martin family 的时序互补”可能有效，但不能再组合 finished curves。
Round 24 必须把所有 FO/SO/TP/abort/order/funding 事件按时间合并到一个账户；inactive family 收益必须为 0，
权重只能来自 lagged train equal-risk，不能按 outer PnL 调权。

### 第三优先：Exact Soft Martin + SEL

现已取得可核验来源 `10.3390/a19060442`，PDF SHA256：

```text
8df91d970f91610d318440c86108a5e33f8d515500f836c47ca727d2afdc3935
```

只采用 `linspace(1,5,10)` Soft Martin 和 inner-train 完整 Martin path label。论文的 EUR/USD、1:500、
`442.6% ann / 79.97% equity DD` 不能搬到 crypto，也不能作为收益先验。只有父 Martin family 先过 G1 才允许
启用 SEL，防止再产生一个“模型本身赚钱”的非 Martin sleeve。

### 第四优先：严格 M1/M2

Round 23 实际只测试了 RelaxedM1 和残缺 M2。补齐数据后，严格 lagged 五条件 M1、completed-bucket M2 可以
作为独立 Martin admission family 小配额测试；禁止继续普通 grid。

## 7. 权威文件

从现在起只读：

- `docs/superpowers/artifacts/glm-martingale-core-round23/round23-corrected-authority.json`
- `docs/superpowers/artifacts/glm-martingale-core-round23/audit/round23-independent-audit.json`
- `docs/superpowers/artifacts/glm-martingale-core-round23/audit/round1-23-consolidated-frontier.json`
- `docs/superpowers/artifacts/glm-martingale-core-round23/audit/round23-corrected-failure-ledger.jsonl`
- `docs/superpowers/artifacts/glm-martingale-core-round23/audit/round24-external-source-update.json`

Round 24 唯一任务书：

`docs/superpowers/plans/2026-07-23-glm-martingale-core-round24-causal-ensemble-recovery-plan.md`

不创建 30 天监控，不等待未来数据；但所有结论只能称历史 prequential backtest，不能包装成未读 future OOS。
