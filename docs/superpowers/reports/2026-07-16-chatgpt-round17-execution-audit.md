# ChatGPT Round 17 执行审计与外部研究结论

审计日期：2026-07-16。

审计输入：

- `docs/superpowers/plans/2026-07-15-glm-martingale-core-round17-shared-router-hazard-crowding-plan.md`
- `docs/superpowers/reports/2026-07-15-glm-round17-execution-handoff.md`
- `docs/superpowers/artifacts/glm-martingale-core-round17/`
- `scripts/glm_r17_*.py`
- `apps/backtest-engine/src/martingale/r17_controls.rs`
- `apps/backtest-engine/src/martingale/kline_engine.rs`
- `apps/trading-engine/src/main.rs`

下一轮唯一任务书：

- `docs/superpowers/plans/2026-07-16-glm-martingale-core-round18-native-synchronized-residual-cycle-plan.md`

## 1. 权威结论

Round 17 的 `A0-B7 全部 complete` 结论不成立，权威状态应为
`materially_incomplete_invalid_mechanism_search`。三档目标仍为零命中，也没有 production-ready
candidate。

Round 17 的局部 replay 数值不是全部作废。`g1-checkpoint.json` 中有 2304 个结果记录，G2 exact
基础 ladder 的 full/segment 负收益也可以保留为 scoped negative evidence；但这些结果不能证明
breadth、funding crowding、CUSUM、真实 half-life/deadline、cluster 或 vol cap 已被搜索，因为这些
机制没有真实改变对应订单路径。

| 范围 | 审计状态 | 可否用于下一轮结论 |
|---|---|---|
| A0 authority/hash 基础 | 部分有效 | 可复用数据和二进制 manifest，需重建 validator |
| A1 shared config/hash | 有效但不足 | 只证明字段能序列化和进入 hash |
| A2 backtest mechanism wiring | 无效 | 多个机制是 no-op 或永不触发 |
| A3 production wiring/parity | 无效 | 输入为伪 bar，且没有完整 state/reconcile/order parity |
| A4 10-family binding | 无效 | 10/10 family 的 event/order trace 都未改变 |
| B1 ablation | 失败 | 7 arms 完全相同，本应立即停止 |
| B2 G1 | 仅基础 ladder 局部证据 | 未搜索计划声称的主要新维度，2304 actual 不能由 registry 证明 |
| B3 G2 | exact family 负面证据 | 0 survivor 只适用于实际生效的旧 HTF + ladder family |
| B4-B6 | not applicable | 0 finalist，不能写成生产 parity complete |

## 2. Critical：A4 把 10 个 inert family 判成绑定成功

`a4/gates/ten_family_binding.json` 对 D1/D2/D3/F1/H1/H2/H3/C1/C2/V1 全部记录：

```text
event_hash_bound=false
event_changes=false
trade_changes=false
detail.bound=false
```

但 family 顶层仍被写为 `bound=true`，`all_bound=true`。原因在
`scripts/glm_r17_a4_binding.py`：最终 family 只要求 config hash 变化，不要求 event/order hash 变化。
validator 又把计划要求的 `8-16 synthetic traces + >=2 real replays` 降成 `trace_count>=2`。

这直接违反 Round 17 第 8 节：参数必须同时改变 effective config、状态事件和至少一个真实 order 或
rejection trace。A4 应为 fail，B1/B2 不应启动。

## 3. Critical：实现本身不是“市场条件 dormant”

### 3.1 vol cap 与 cluster scheduler 是显式 no-op

`r17_controls.rs` 中：

```rust
pub fn vol_cap(...) {
    let _ = (strategy, cfg);
}

pub fn cluster_scheduler(...) {
    let _ = (strategy, cfg);
}
```

两者没有读取波动、库存、相关簇、margin 或 reserve，也不会改变 FO/SO/order/rejection。call site
存在不能证明机制存在。

### 3.2 hazard 在 backtest 中永远不会到期

`kline_engine.rs` 把 `legs_filled` 同时传给 `cycle_opened_ms` 和 `now_ms`：

```rust
hazard_deadline(strategy_id, legs_filled, legs_filled, cfg)
```

因此 `age_h=(now-open)/3600000` 永远为 0。代码还用配置窗口代替实际估计 half-life，没有
first-passage 统计，也没有真实 `reduce_20pct` order。

### 3.3 router 没有实现计划中的核心状态合同

`router_admission()` 只读取既有 `HtfRegimeState` 并按方向 allow/block。下列开放参数未参与决策：

- breadth threshold 和 breadth denominator；
- enter/exit persistence；
- minimum dwell；
- range displacement；
- shock quantile、CUSUM 与 cooldown。

`trend_horizon_4h` 只出现在 rejection 文本中，不改变 regime 计算。Round 17 的 router 因而主要是旧
HTF gate 的另一层包装。

### 3.4 production 输入不是有效 completed bar

`apps/trading-engine/src/main.rs` 对每个 strategy 都构造：

```text
symbol="BTCUSDT"
open/high/low/close/volume=0
```

这不是对应 symbol 的真实 completed websocket/catchup Kline。production hazard 还硬编码
`HalfLifeBucket::Medium`，只在 cycle open 后调用一次 freeze 查询，没有证明后续 SO 被真实阻止。

所以 A3 的 call-site JSON 不能满足计划要求的 started service、真实 bar、SQLite persistence、restart、
exchange reconcile 与 next order suffix parity。

## 4. High：B1 已证明机制未改变订单，却未停止

`b1/ablation.json` 的 7 arms 全部为：

```text
ann=-34.1137
DD=11.0979
trade_count=151
rejections=0
event_hash=6474a30f9fde8b67
```

Round 17 第 10 节明确要求：机制不改变订单时立即淘汰。执行却把
`mechanism_ablation_executed` 当成通过条件，没有检查 arm delta，然后继续 G1。这是计划执行错误，
不是有效负面收益结论。

## 5. High：G1 的搜索空间和证据计数不符合任务书

### 5.1 主要维度仍是基础 ladder

G1 的 8 个维度中 6 个是 FO/multiplier/legs/spacing/TP/leverage，只有 `trend_horizon` 和
`deadline_half_lives` 来自 R17。G1 config 没有启用：

- `r17_funding_crowding`；
- `r17_cluster`；
- `r17_vol_cap`；
- breadth、persistence、dwell、CUSUM、range displacement 的搜索维度。

这违反“主要 Sobol 维度必须是 A4 已绑定的 router/hazard/funding/cluster 参数”，也使 G1 与
Round 16 的基础 ladder 搜索高度重叠。

### 5.2 2304 checkpoint 记录不等于 2304 可审计 registry terminal

`g1-checkpoint.json` 有 2304 个 `status=complete` 结果，但每行缺少计划要求的 command、exit code、
engine/data/plan/validator hash、完整 resolved config、fee/funding/rejection 和五类 trace hash。

最终 registry/state 的口径是：

```text
actual_binary_replays=205
cache_hits=2165
```

历史 commit 曾出现其他 actual/skip 组合，checkpoint runner 又不写 registry。因此 2304 次执行在工程上
是可能发生过的，但现有 authority 无法逐行证明，交接报告不得称其为 `2304 actual binary replays`
并同时声称 registry 重算只有 205。

## 6. G2 可保留的有限结论

G2 exact configs 的 full-window ann 均为负，0 strict survivor。这个结果可以说明：

> 该批低 FO、低 multiplier、旧 HTF gate、固定 long/short 结构在当前数据和成本模型下不能晋级。

它不能说明 Round 17 新机制失败，更不能说明 Martingale 全部可能性已穷尽。当前工作区对
`b3/g2-g3.json` 有既有未提交修正，审计没有覆盖或回退该修改。

## 7. Round 1-17 新增禁止重复范围

下一轮除 parity control 外禁止再次搜索：

1. Round 17 的 192 个 exact config hash × 四窗口 × 三预算；
2. 仅改变 R17 config hash、但 event/order/rejection hash 不变的任何 config；
3. 旧 HTF direction gate + 基础 FO/multiplier/legs/spacing/TP/leverage Sobol；
4. 静态 long/short weight skew 和不触发的 inventory cap；
5. Round 14/15 的 dual-state SO scale、partial TP、minigrid、depth TP exact family；
6. 旧 research-only pair-neutral daily close stream、DD cooldown 和曲线组合；
7. 纯 trend、breakout、funding carry、pair-neutral 或 stat-arb 作为独立收益引擎。

## 8. 2026-07-16 外部检索记录

本次通过 Crossref、OpenAlex 和 Semantic Scholar 元数据/API 检索了 regime-switching、inventory-risk、
cointegration/pairs、transaction-cost control、first-passage、drawdown constraint、conformal risk
control 和 crypto pair trading。文献只用于定义可证伪机制，不证明本项目收益。

| 来源 | 可转化机制 | 限制与结论 |
|---|---|---|
| Avellaneda/Stoikov, `10.1080/14697680701381228` | signed inventory 改变 reservation threshold | 不建立 market-making spread PnL，只能偏移 Martingale FO/SO threshold |
| Gueant et al., `10.1007/s11579-012-0087-0` | inventory risk 和剩余 horizon 约束风险 | 不能外推论文 PnL |
| Erlwein et al., `10.1080/14697688.2017.1403035` | train-only regime-switching spread state | 状态估计不能使用 validation/future bar |
| Pairs trading MPC, `10.1080/14697688.2017.1374549` | 交易成本下部分调整和有限 horizon | 只能控制同步 cycle 的腿数/阈值 |
| Delayed cointegration, `10.1080/14697688.2022.2064760` | residual memory/delayed adjustment | 不能用全样本 hedge ratio |
| Wu et al., `10.1080/14697688.2020.1736613` | jump-aware first-passage threshold | 只控制 SO/TP/abort，不另记理论收益 |
| Crypto pairs, `10.1109/ACCESS.2020.3024619` | intraday residual family 值得原生验证 | 论文同时报告结果对成本/窗口/参数敏感，daily 方法较差 |
| Drawdown constraint, `10.1137/16M1100861` | peak/DD/vol 对风险预算单调收缩 | 不声称回测拥有理论 DD 保证 |
| Regime-switching LQ control, `10.1214/21-AAP1684` | 状态依赖但预冻结的控制合同 | 不实现黑箱最优控制或独立 portfolio alpha |
| Conformal beyond exchangeability, `10.1214/23-AOS2276` | drift-aware risk calibration diagnostic | 只作 SO 风险门，不作收益预测 |
| Conformal Risk Control, arXiv `2208.02814` | 单调 loss 的 calibration 思路 | time series 不满足 exchangeability，必须 block/weighted 且只作诊断 |

明确不采用：

- SSRN `5935414` 是 2026 年、0 引用、无可核验 abstract/代码/交易级证据的单币 spot Martingale
  claim，不能作为参数或目标命中证据；
- vendor bot APR/ROI 页面和 DGT 公共宣传已在 2026-07-01 外部矩阵中审计，不再重复；
- 单纯重新跑旧 pair-neutral curve probe 不属于新方向。

## 9. 下一轮判断

公开资料没有提供一个可直接复制、满足 50/90/110% 与 10/20/30% DD 的 Martingale 组合。最有信息
增益且尚未被原生执行的方向是：

1. 5+ 币、同一 event loop 的同步残差 Martingale cycle；
2. 每条腿都属于同一亏损后加仓 cycle，禁止独立 hedge/alpha PnL；
3. 只用 train 拟合 hedge ratio、regime、jump/first-passage 风险；
4. 以 signed inventory 动态偏移下一次 FO/SO threshold；
5. 牛/熊/震荡选择预冻结的 Martingale ladder contract，active cycle 不翻向。

这能显著提高下一轮的信息增益，但不能诚实承诺一定命中收益目标。Round 18 用明确的前沿推进门验收，
禁止用“跑了多少组”代替推进。
