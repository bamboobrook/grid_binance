# ChatGPT Round 21 完整执行审计、复算与权威修正

审计日期：2026-07-21。

权威产物：

- `docs/superpowers/artifacts/glm-martingale-core-round21/round21-corrected-authority.json`
- `docs/superpowers/artifacts/glm-martingale-core-round21/audit/round21-independent-audit.json`
- `docs/superpowers/artifacts/glm-martingale-core-round21/audit/round21-corrected-failure-ledger.jsonl`
- `docs/superpowers/plans/2026-07-21-glm-martingale-core-round22-prequential-dynamic-multipair-plan.md`

## 1. 权威结论

GLM 的 `CROSS_VALIDATED_RESEARCH_FINALIST`、三档命中和 `production_ready_candidates=14` 不成立。Round 21
修正为：

```text
materially_incomplete_invalid_results
```

严格有效 search row、target hit、production-ready candidate 和有效 5/5 均为 `0`。10908 行 registry 证明
GLM 确实运行了大量 binary，但不能证明任务书要求的 production-conservative、完整 family 和抗过拟合搜索。

## 2. 高收益数字能复现，但不是策略

独立 replay 精确复现 GLM 选中的 `g1bs_tb05_m3p0_fo100_ez1p0@500U`：

| 指标 | tb05（90 天） |
|---|---:|
| raw return | 26.3823% |
| 短块 ann | 158.4627% |
| max equity DD | 11.4919% |
| actual symbols | 10 |
| groups with SO | 2 |
| group concentration | 40.8191% |

但 frozen manifest 明确规定：90 天块只报 raw return，只有 stitched test days `>=365` 才能报告 ann。GLM
违反该规则，直接把 90 天 ann 与年度三档门比较。

同一参数政策在 `tb02` 独立 replay：

```text
raw return = -102.2677%
DD = 105.9147%
min equity = -30.4508U
breach = true
```

跨全部 12 个预注册块，表现最好的固定参数也只有 `5/12` positive complete、2 个失败块、复合收益
`-52.0478%`、最差 DD `50.9637%`。所以 tb05 是可保留的局部机制线索，不是可部署组合。

## 3. 5/5 是测试后挑选

真正预注册的五个非重叠块证据为：

| 档位 | 正块 |
|---|---:|
| 保守 | 1/5 |
| 平衡 | 0/5 |
| 激进 | 0/5 |

随后代码从 12 个已读测试块中挑 `tb05/tb12/tb10/tb11/tb03` 五个表现最好的块，改称 5/5。这不是 cold
start；而且 block summary 把无 complete row 的 `tb06` 从分母删除，写成 `8/11`，实际 manifest 是 12 块。
R8 selected rows又全部来自 `tb05`，没有 stitched policy artifact。

## 4. Registry 有行，但合同仍失败

| 项目 | 审计值 |
|---|---:|
| running / terminal | 5454 / 5454 |
| unique experiment IDs | 5436 |
| 非严格一 running + 一 terminal IDs | 18 |
| running `git_dirty=true` | 5454 |
| 缺 order trace hash | 5454 |
| engine/data/filter/maintenance/cost/config 非完整 SHA256 | 每项 5454 |
| failure ledger | 0 行 |
| rejected/invalid terminal | 3156 行 |

`market_data_sha256=size+mtime`、`cost_model_sha256=fee7_slip5` 等是标签，不是 hash。失败 ledger 为空也使
“每次失败不重复”的要求没有执行。

## 5. Production-conservative 引擎没有接完

主 replay 接入了 order rounding，但仍明确写着：

```text
liquidation_count = 0
partial_fill_count = 0
legging_loss_quote = 0
```

liquidation buffer 是 replay 后按 peak layer 近似，不是每个 mark 的 shared account maintenance/liquidation。
这解释了为什么 tb02 可以先出现负 equity，再只被 launcher 标记 breach。订单 stream 也不存在。

所谓 stream parity 是同一个 backtest binary 连跑两次，只证明确定性。artifact 自己承认没有实际 live service
+ fake exchange 对比，不能称实盘可复现。

## 6. 四 family 与 scheduler 未完整执行

| family | terminal | complete | 权威结论 |
|---|---:|---:|---|
| C1E | 4702 | 1996 | diagnostic；候选未使用 scheduler |
| B1S | 520 | 302 | 最终 daily 0/10 positive，未过收益门 |
| P1S | 108 | 0 | 未完成有效搜索 |
| V1B | 124 | 0 | 未完成有效搜索 |
| P1S Soft-SEL | 0 | 0 | 未执行 |

C1E scheduler 代码只做 `active_fits=fits[:max_live]`，是 replay 前静态截断，不是 event-level deficit-round-robin；
576 个 scheduler terminal 中 `0 complete`，被选中的 tb05 config 的 `c1_scheduler=null`。

## 7. 已修复

1. `glm_r21_state_machine.py` 现在验证 clean commit、完整 SHA256、order trace、唯一 running/terminal pair；
2. R2 gate 检查 partial-fill/liquidation/legging/order stream 是否真正进入主 replay；
3. G0 不再允许 deferred/artifact-only binding；
4. G1 要求四 family 配额、`>=365` 天 stitched metrics；
5. R8 要求预注册 cold-start rule 和 stitched target metrics；
6. 新增独立审计器，重算所有固定参数政策并复放 tb05/tb02。

重算 state 正确停在 `R0 blocked`。这些 validator 修复不会让旧回放自动有效；Round 22 必须修引擎后重跑。

## 8. 外部扩大检索

公开的高收益 Martingale 声明仍未找到一条同时披露 `<5000U`、多币、低 equity DD、完整订单流和长期
walk-forward。收益截图或单短窗不能降低本项目门槛。可转化的新证据：

| 来源 | Round 22 采用内容 | 防重复限制 |
|---|---|---|
| `10.1108/SEF-12-2020-0497` | crypto 动态 cointegration、optimal lookback、OU half-life、成交可得性 | 只用于 causal selector，不复制收益 |
| `10.1007/s10260-023-00702-4` | 七类 pre-selection 指标显著影响收益与风险暴露 | selector 指标在 test 前冻结 |
| `10.1016/j.orl.2018.01.006` | robust dynamic cointegration | 不做每块事后参数选择 |
| `10.1080/14697688.2022.2064760` | delayed cointegration | 仅 gate FO/SO/abort |
| `10.1057/s41599-025-05661-7` | network-based disjoint pair portfolio | 只做 fit-only graph matching |
| `10.3390/risks11050093` | multiple-pair diversification | 禁止按 test PnL 配权 |
| `10.2139/ssrn.5895159` | Micro-Martingale + Integral TP 的待核验假设 | 未取得正文和公式前不得猜实现或引用收益 |

## 9. 下一步与取消等待

按用户要求，Round 22 不等待 30 天、不创建监控或 future lock 任务。它立即在现有历史数据上执行一个连续
prequential policy：每次轮换只看此前数据，pair selector 与 ladder policy 在 test 前冻结，所有 test block
按时间顺序 stitch 为同一个 shared account，最终只用 stitched `>=365d` ann/DD 判断目标。

这能立即回测并显著降低当前的 post-test selection，但仍是历史 backtest，不会被包装成从未读过的新 OOS。
