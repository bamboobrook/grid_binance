# GLM Martingale Core Round 11：审计后交接

> 本文件已于 2026-07-12 被独立审计修正。原版关于“完全 production live-ready”、
> “原生 minigrid 已失败”和“159192 次真实回测”的结论不再有效。

**Round11 原始分支：** `glm-martingale-core-round11`

**审计基线：** `4b912e9a73b682ac3f3cc4aeae21a93fae212a13`

**完整审计：** `docs/superpowers/reports/2026-07-12-glm-round11-execution-audit-and-fix.md`

**机器状态：** `docs/superpowers/artifacts/glm-martingale-core-round11/r1-r11-corrected-status.json`
**下一轮计划：** `docs/superpowers/plans/2026-07-12-glm-martingale-core-round12-event-level-native-search-plan.md`

## 最终结论

三个目标均未达成：

| Tier | 要求 | 审计后结果 |
|---|---|---|
| Conservative | ann >=50%, DD <=10% | FAIL |
| Balanced | ann >=90%, DD <=20% | FAIL |
| Aggressive | ann >=110%, DD <=30% | FAIL |

所有 tier 还必须同时满足 `<5000U`、多币种、抗过拟合和 fully live-ready；当前没有
任何候选通过完整 contract。

## 修正后的前沿

| 类型 | 候选 | Ann | DD | 分段 | 可实盘 |
|---|---|---:|---:|---:|---|
| Curve diagnostic | R9 allocator + corrected ANKR funding | 62.7845% | 18.3840% | 5/5 curve slices | 否 |
| Event-level backtest | R7-ANKR-q + corrected funding | 62.3718% | 28.6873% | 4/5 cold-start | 否 |
| 已接受 live-ready sleeve | R4-combo | 34.7233% | 17.6843% | 4/5 | 是，但收益不达标 |

R7-ANKR-q 是 6 个真实交易币的组合，不是单币；但 2000U 复算只有
`5.4232% ann / 45.5823% DD`，因此不能笼统宣称“小资金可跑”。4999U 结果为
`62.3795% / 28.6894%`，仍不命中任何 tier。

## Round11 执行修正

- **P1 partial：** allocator helper 和初始 gate 存在；started executor 绕过动态
  rebalance，且无 shadow observation writer。
- **P2 research-only：** Python curve replay，不是 trading-engine/DB production parity；
  原 forward-only pass 为硬编码。
- **P3 config-only：** 只有 minigrid config/math helper，无 kline/live order semantics。
- **P4 scoped fail：** 6912 个 partial-TP approximation，0 target；原生 minigrid 未测。
- **P5 narrow fail：** 8100 个中 7996 个为 fixed TP；真实 ATR spacing 未测。
- **P6 partial：** 4608 labels 中 ADX/DD-scale 两维 inert，只有 217 组不同 full metrics；
  脚本字段层级已修复，必须重跑。
- **P7 legacy curve grid：** 同一组 R9 sleeves，72 组不同 full metrics；不是异构
  event-level allocator。

实际估计 `portfolio_budget_replay` 进程约 117730，而不是 159192；差额主要是把 P7
内存 curve calculations 算成真实 backtests。

## 数据修正

历史 `funding_rates.db` 完全没有 ANKRUSDT。审计从 Binance 官方接口取得 3997 个
funding points，在临时 DB 中复算：

- R7：`63.5105/28.1972` -> `62.3718/28.6873`；
- R9 curve：`64.4196/18.2111` -> `62.7845/18.3840`。

原数据库没有被覆盖。Round12 必须先生成 frozen range manifest，并让 futures replay
在 traded symbol 缺 funding 时 fail closed。

## Non-Repeat Keys

只允许使用以下精确范围：

```text
r11-partial-tp-minigrid-approx-6912-no-target
r11-fixed-tp-dominated-7996-no-target
r11-so-v2-effective-controls-historical-grid-no-target
r11-r9-same-sleeve-curve-allocator-grid-no-target
```

禁止继续使用过宽旧键：

```text
r11-native-minigrid-no-target
r11-non-r4-architecture-no-target
r11-conditional-so-v2-no-target
r11-heterogeneous-allocator-no-target
```

## GLM 下一步

直接执行：

`docs/superpowers/plans/2026-07-12-glm-martingale-core-round12-event-level-native-search-plan.md`

优先级：

1. 冻结数据并补齐全部 traded symbols funding；
2. 建立 event-level shared-budget allocator，先重新衡量 R9；
3. 实现原生 minigrid backtest/live order semantics；
4. 补跑已修复 ADX/DD-scale，并测试 HTF 趋势方向门控；
5. 搜索真实 ATR spacing + cycle-depth TP；
6. 把旧 LP member pool 重新做 4999U event-level 组合，不复用旧 LP 指标；
7. 执行 WFO、untouched 2026-06/07 holdout、budget ladder、stress 和 DB parity。

Round12 只有通过上述完整链路的候选才能写入 `promising/` 或报告为目标命中。
