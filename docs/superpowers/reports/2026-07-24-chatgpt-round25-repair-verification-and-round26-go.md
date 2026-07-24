# Round 25 修复完成度复核与 Round 26 GO 决策

复核日期：2026-07-24。

## 1. 结论

截至本次复核，没有发现晚于 `12631058` 的 Round 25 修复提交、交接报告或回测产物。已检查：

```text
当前 worktree 与 git status
全部本地和远端 branches
git fetch --all --prune 后的远端提交
已登记 worktrees，包括 /tmp/grid-binance-round25r
docs/superpowers/reports、plans、artifacts 的更新时间
artifacts-local 的新增文件
```

因此，不能把“修复任务完成”解释为已经产生了一轮新的有效回测。当前权威状态仍是：

```text
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

Round 25R 已完成且可以继承的部分：reserve lifecycle、filter-resolved sizing、completed-bar 后下一 1m 成交、
1m high/low/funding/maintenance 路径、终态归零和账户 trace 复算。

尚未完成且会使 16 个结果失效的部分：正确 EG/MacKinnon p-value、独立 KPSS/model parity、真实 cost gate、
exact matching、模型 validator、相邻 budgets、三档、cold starts 和实盘压力。独立复算仅 `1/5` selected pairs
通过 EG+KPSS，所以原 16 个 terminal 不能作为 valid failures 或收益上限。

## 2. 目标覆盖复核

下一步仍执行 Round 26，不另写重复的 Round 27。唯一任务书已逐项覆盖用户要求：

| 用户要求 | Round 26 机器门 |
|---|---|
| 防过拟合 | weekly prequential、RAW/FDR 预注册双臂、global trial ledger、CSCV/PBO、DSR、SPA、bootstrap、LOSO/LOPO、neighbor stability、5 cold starts |
| `<5000U` | `500/750/1000/1500/2000/3000/4000/4999U` 全量 replay、exact minimum principal、至少两个相邻 budget 通过 |
| 保守 | `ann>=50%`、`DD<=10%`、`>=4/5` cold starts 正，另报 5/5，cap `2x` |
| 平衡 | `ann>=90%`、`DD<=20%`、`>=4/5` cold starts 正，另报 5/5，cap `3x` |
| 激进 | `ann>=100%`、`DD<=30%`、`>=3/5` cold starts 正，另报 5/5，cap `4x` |
| 多币组合 | 29-alt pool、weekly Top-20、实际 `>=6` alts、`>=3` pairs、贡献集中度 `<=50%` |
| 实盘复现 | 真实 filters、fees/slippage/funding、1m risk path、partial fill、leg delay、one-leg reject、restart/reconcile、双 validator |
| 马丁核心 | 所有 PnL 只能来自 FO/loss-after-add SO/TP/reduce/abort/funding；selector/Copula/Hurst 只能 gate Martin action |

## 3. 为什么仍值得执行

return-blind census 显示，原 6-alt quarterly selector 结构性过稀疏；扩大到 29-alt weekly formation 后，1h RAW
在 152 个 weekly rolls 中：

```text
14d: 36 rolls >=2 stationary alts, 14 rolls >=4, 9 rolls >=6
21d: 31 rolls >=2 stationary alts, 13 rolls >=4, 9 rolls >=6
```

这不是收益证据，但证明新 fingerprint 有足够激活可能性，值得进入严格 production replay。FDR 结果仍稀疏，
所以 RAW 与 FDR 必须同时冻结，不能看完收益后选择门槛。主臂为 1h；5m 只有通过 activity gate 才能完整回放。

## 4. GO 决策

执行：

```text
docs/superpowers/plans/2026-07-23-glm-martingale-core-round26-weekly-expanded-copula-martin-plan.md
```

状态：`GO_READY_NOT_EXECUTED`。

Round 26 必须先生成独立 statsmodels snapshots，再做 8 个 return-blind activation configs；只有通过 activity
gate 的配置才能跑 Martin shared account，只有 P-B survivor 才能进入三档、budgets、cold starts 与压力测试。

不能承诺一定命中 50%/90%/100% 目标，但该计划不会重复已关闭 fingerprint，也不会用无效收益继续放大风险。
