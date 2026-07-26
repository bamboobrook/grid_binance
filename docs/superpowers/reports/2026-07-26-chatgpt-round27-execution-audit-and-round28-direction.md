# Round 27 独立审计、复算与 Round 28 方向

日期：2026-07-26  
审计对象：`glm-martingale-core-round27`，原始最终提交 `5feed6d3`  
原报告：`docs/superpowers/reports/2026-07-26-glm-round27-handoff.md`

## 1. 权威结论

Round 27 报告中的 8 组负收益、最终权益和回撤，能够在它实际输出的 trace 上精确复算；但这些 trace
由错误的交易方向、未来退出信息筛选和串行化账户回放生成。因此原状态
`VALID_HISTORICAL_PREQUENTIAL_NO_PB` 必须撤销，修正为：

```text
MATERIALLY_INCOMPLETE_INVALID_G2_AND_FAMILY_CLOSURE
strict_valid_candidates = 0
target_hit = false
round27_copula_family_closed = false
```

这不是把负结果改成正结果。正确含义是：**数值忠实描述了错误程序，不能评价预注册策略，也不能关闭
Copula/PBD 或多币共享账户 Martin 方向。** Round 27 的 G0/G1 模型快照可以作为待复核输入；G2、G3、
account validator、failure closure 和最终 authority 不能继承。

## 2. 审计范围与复算证据

```text
计划：docs/superpowers/plans/2026-07-26-glm-martingale-core-round27-staged-admission-persistence-recovery-plan.md
source commit：aaa02fda4e588ced0359318465ee682c677ae00c
source tree：42c605916e4419e4cd556125e82dc0609205f95f
raw root：artifacts-local/round27/aaa02fda4e588ced0359318465ee682c677ae00c
临时独立复算器 SHA256：8fa13359297f16622733cd4da583660eff81ca0d48d3edd77f65f7825de24b02
临时复算结果 SHA256：9f19fe70979c4a3c2e1ae66db74d7209a0089ca9c1a09dd54da6dc739fd6cd68
```

独立复算逐行读取 engine trace 和 risk RLE，不调用 Round 27 的收益/DD 函数。8 个 policy 均得到：

- `1,531,680` 个逻辑 1m 风险点；
- chronology、RLE 覆盖和终态权益一致；
- annualized return 误差 `<=2.3e-14` 个百分点；
- DD 误差为 0；
- trace 最终权益误差 `<=2.3e-13U`。

现有测试也已重跑：`r24-engine 23/23`、`r24-research 46/46`、doc tests 全通过。测试通过不抵消下列
语义错误，因为原测试没有覆盖 source direction、开放 censor、真实并发调度、calendar block 和 G3 survivor。

## 3. 阻断性问题

### 3.1 Copula 信号的交易方向反了

`r27_execute.rs:969-972` 把 `h_left` 低、`h_right` 高编码为 `direction=+1`；
`r27_execute.rs:1095-1103` 又把 `+1` 执行为 long left / short right。

来源 [Copula-based trading of cointegrated cryptocurrency pairs](https://doi.org/10.1186/s40854-024-00702-7)
Table 4 的规则相反：

```text
h_left low,  h_right high -> short left, long right
h_left high, h_right low  -> long left,  short right
```

同一来源还使用 beta-weighted 两腿。Round 27 在 `r27_execute.rs:1110-1118` 和 `1156-1161` 固定
equal-dollar 50/50，因此报告所称 `source-faithful C0` 不成立。

### 3.2 入场样本被未来退出结果筛选

`excursions_from_points()` 只有看到未来 neutral 或 deadline 才把 active signal 写入输出
（`r27_execute.rs:698-726`）。每个 weekly snapshot 只产生一周 signal points，而 Copula deadline 也是
一周；周末尚未 neutral 的入场不会进入 G2。

这等价于在入场前先问“本周内是否出现可识别退出”，属于 lookahead。正确实现必须在 signal onset
立即产生 causal intent，由账户 scheduler 此后逐分钟决定 TP、SO、deadline 或数据末端强平。

### 3.3 多币共享账户被串行化

`replay_policy()` 先完整跑完一个 excursion，再处理下一个，并在
`r27_execute.rs:758-760` 丢弃所有 `entry_ms < last_processed` 的重叠信号。结果是：

- 任意时刻最多只有一个 active group；
- 计划要求的 `max_active_groups=3` 从未被真实回放；
- pair intents 没有按时间合并；
- 账户 reserve、margin、concentration 和冲突拒单没有在并发场景受测。

活动统计也直接暴露了丢弃：C0 1h alpha 0.20 有 108 crossings，但只执行 46 FO；C0 5m alpha 0.20
有 180 crossings，只执行 91 FO。不能把这种串行结果称为多 pair shared-account portfolio。

### 3.4 12 个 calendar blocks 索引错误

`r27_execute.rs:1434-1439` 使用：

```text
((year - 2023) * 4 + zero_based_quarter).min(11)
```

外层从 2023Q3 开始，正确索引必须再减 2，且越界应报错而不是 clamp。现实现令 block 0/1 永远为空，
并把 2025Q4、2026Q1、2026Q2 挤入 block 11。

修正 calendar mapping 后，8 个 policy 的 positive-block 数恰好未变，但 1h alpha 0.10 的 block
concentration 从报告的 `85.91%` 变为 `97.21%`。这说明原 block PnL 和 concentration 不能继承。

### 3.5 Account validator 并不独立验证账户

`r27_validate.rs:67-149` 实际只检查 trace hash/row count、RLE chronology、DD、annualization 和终态字段。
它没有独立复算计划要求的：

```text
side/mode/order/fill、wallet ledger、fee/slippage/funding、FO/SO/TP/abort、
pair/group/block PnL、concentration、reserve/margin、P-A/P-B
```

engine trace 又缺少完整 side/mode/unit fill price 等字段，所以即使 validator 想检查，也无法由 raw evidence
重建方向和订单。`account-independent-validator = PASS` 因而不能证明执行正确。

### 3.6 G3 只写了占位符

`r27_execute.rs:596-610` 在没有 P-B 时写 `NOT_APPLICABLE_NO_PB`；一旦真的出现 P-B，程序直接：

```text
bail!("G3 tier replay implementation required for P-B survivors")
```

所以 Round 27 并不具备三档、8 budgets、cold starts、stress 或 anti-overfit 的完整执行能力。负收益碰巧
隐藏了这个阻断点。

### 3.7 Martin 机制几乎没有被检验

8 个 policy 合计只有 6 次 SO；其中 4 个 policy 为 0 SO。最佳原始结果
`C0-1h-21d-RAW-A0.10-SO0.50/0.75` 的 34 个 cycle 全部没有 SO，本质上主要测试了错误方向的首单
pair trade，而不是 Martin recovery。

## 4. 错误 trace 的机械复算

以下只能用于确认报告抄数无误，不能作为有效策略排名：

| policy | report/recalc ann | report/recalc DD | corrected positive blocks | FO/SO/TP/abort |
|---|---:|---:|---:|---:|
| C0-1h-A0.10-SO0.50 | -0.4768% / -0.4768% | 1.8444% / 1.8444% | 2/12 | 34/0/8/26 |
| C0-1h-A0.10-SO0.75 | -0.4768% / -0.4768% | 1.8444% / 1.8444% | 2/12 | 34/0/8/26 |
| C0-5m-A0.10-SO0.75 | -0.6357% / -0.6357% | 2.2124% / 2.2124% | 0/12 | 59/0/8/51 |
| C0-1h-A0.20-SO0.75 | -0.7434% / -0.7434% | 2.6551% / 2.6551% | 1/12 | 46/1/6/40 |
| C0-5m-A0.10-SO0.50 | -0.7525% / -0.7525% | 2.5471% / 2.5471% | 0/12 | 59/2/8/51 |
| C0-1h-A0.20-SO0.50 | -0.8419% / -0.8419% | 2.9375% / 2.9375% | 1/12 | 46/2/6/40 |
| C0-5m-A0.20-SO0.75 | -0.8733% / -0.8733% | 2.6569% / 2.6569% | 1/12 | 91/0/16/75 |
| C0-5m-A0.20-SO0.50 | -0.9048% / -0.9048% | 2.7470% / 2.7470% | 1/12 | 91/1/16/75 |

仅把同一错误退出路径的 gross PnL 反号，得到约 `0.33%-0.63% ann`。这不是合法 counterfactual：正确方向
会改变 SO、TP、deadline 和并发占用，只能作为量级诊断。它提示 corrected C0 单独达到 50% 的先验很低，
但不能替代 causal concurrent replay。

## 5. 外部检索后的新信息

| 来源 | 核验到的信息 | Round 28 决定 |
|---|---|---|
| [10.1186/s40854-024-00702-7](https://doi.org/10.1186/s40854-024-00702-7) | Table 4 证明方向反转；两腿按 beta 配置 | 必须做 corrected beta-weighted recovery；equal-dollar 仅作为预注册 ablation |
| [10.3934/QFE.2026016](https://doi.org/10.3934/QFE.2026016) | 2026 Binance perpetual Copula：成本后 market-neutral 为负；directional overlay 虽增益但 DD 超 80% | 不采用 directional PnL sleeve，也不继承论文收益 |
| [10.1198/073500101316970395](https://doi.org/10.1198/073500101316970395) | Enders-Siklos TAR/MTAR 对正负偏离估计不同 error-correction speed | 新增一个低容量、train-only 的非对称 Martin admission/SO-freeze family |
| [10.1080/14697688.2022.2064760](https://doi.org/10.1080/14697688.2022.2064760) | delayed cointegration 可描述滞后调整 | Round 22 的 variance-ratio 标签不算 exact 实现，但本轮先不扩成高容量模型 |
| [10.3390/a19060442](https://doi.org/10.3390/a19060442) | Soft-Martingale 高收益来自单 EUR/USD、1:500 和约 80% equity DD | 已测试，不重复 |
| [10.2139/ssrn.5895159](https://doi.org/10.2139/ssrn.5895159) | Micro-Martingale/Integral TP 已在前轮做过来源审计与大量近似测试 | 不重复 |

没有找到公开研究同时证明 `<5000U`、真实成本、多币共享账户、Martin-only、`50/90/100% ann` 和
`10/20/30% DD`。目标可以继续作为硬验收线，但不能承诺下一轮一定命中。

## 6. 不得重复与允许重开

继续关闭：普通 multiplier/spacing/TP 网格、ATR/ADX/EMA/RSI/Donchian、first-passage/hazard、
Micro/Integral TP、Soft-Martingale、funding standalone、trend/breakout、curve allocator、旧 finished-curve
拼接、旧 PC1/Johansen/VECM/Kalman/partial-cointegration exact fingerprints。

允许重开且必须重跑：

```text
R27-C0 1h/5m RAW alpha 0.10/0.20 SO 0.50/0.75
```

原因不是结果差，而是其 G2 fingerprint 由 reversed direction + future-filtered excursions + serial account
共同污染。修正后的每个 fingerprint 必须加上 `R28-CORRECTED-DIRECTION-CAUSAL-CONCURRENT`，不得覆盖
Round 27 原失败记录。

## 7. Round 28 方向

Round 28 固定为两步：

1. `R28-C0 recovery`：先修方向、beta sizing、开放 censor、并发 scheduler、calendar blocks、完整 trace、
   独立 account validator 和 G3。recovery gate 未通过，不得发布任何策略结论。
2. `R28-T1 TAR/MTAR`：只用 formation data 估计正负偏离的 adjustment speed；可靠方向允许 Martin FO，
   弱/发散方向禁止 FO，并在 active cycle 中 freeze SO/按 deadline abort。它只能控制 Martin group，不能产生
   独立仓位或收益。

唯一执行任务书：

```text
docs/superpowers/plans/2026-07-26-glm-martingale-core-round28-corrected-concurrent-asymmetric-plan.md
```
