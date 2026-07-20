# ChatGPT Round 20 完整执行审计、复算与权威修正

审计日期：2026-07-20。

权威产物：

- `docs/superpowers/artifacts/glm-martingale-core-round20/round20-corrected-authority.json`
- `docs/superpowers/artifacts/glm-martingale-core-round20/audit/round20-independent-audit.json`
- `docs/superpowers/artifacts/glm-martingale-core-round20/audit/round20-corrected-failure-ledger.jsonl`
- `docs/superpowers/plans/2026-07-20-glm-martingale-core-round21-real-execution-crossfit-plan.md`

## 1. 权威结论

GLM 的 `VALID_SEARCH_NO_FRONTIER_PROGRESS` 不成立。Round 20 修正为：

```text
materially_incomplete_invalid_results
```

3946 次 checkpoint 记录不是全部数值错误，但没有一条同时满足 registry、真实 family、causal fit、预算、
production-conservative execution 和 selection/OOS 合同，因此严格有效 search row、candidate、production-ready
candidate 均为 `0`。保守 50%/10%、平衡 90%/20%、激进 110%/30% 全部未命中，也不存在有效 5/5。

## 2. 独立复算

审计器完整重放了 GLM 报告的最佳 continued config：

| 项目 | 复算结果 | 结论 |
|---|---:|---|
| ann | 17.7183078% | 精确复现 |
| max equity DD | 4.5490963% | 精确复现 |
| actual symbols | 8 | 通过 |
| groups with SO | 4 | 通过 |
| max symbol concentration | 27.9720% | 通过 |
| max group concentration | 60.3617% | **失败，要求 <=50%** |

严格共同门下，24 个 continued rows 仅 `cap30_mult170_fo60@1000U` 通过表面指标：ann 13.5291%、DD
3.7266%、group concentration 44.0673%。但它是在读取 F4 validation 后直接调参再读同一窗口，且仍使用
无真实 scheduler/exchange model 的 M1 path，所以只能记为污染后的 diagnostic。

G1 预算缺陷也被独立复现：同一 `g1_C1_F4_008_ib2_1000.json` 分别传 `--budget 1000` 与 `--budget 4999`，
两次输出都声明 `budget_quote=4999`，stdout SHA256 完全相同。Round 20 G1 的 1000U 证据无效。

## 3. 执行完整性失败

中央 `exploration-registry.jsonl` 和 `failure-ledger.jsonl` 均为 0 行，但 G1/G2/P9 checkpoint 分别有
1408/2350/20 行。没有 running/terminal pair、raw command、exit code、canonical fingerprint、engine/data/cost
hash 或五类 trace hash。原 state machine 的多个 negative test 被硬编码为 `True`，空 registry 仍能 P12
complete。本次已修复 validator，重算状态停在 `P0 blocked`。

计划配额与实际 config/fold：

| family | 计划 | 实际 | 完成率 |
|---|---:|---:|---:|
| C1 | 64 | 16 | 25.0% |
| B1 | 96 | 16 | 16.7% |
| M2R | 64 | 16 | 25.0% |
| P1 | 64 | 16 | 25.0% |
| K1 | 48 | 12 | 25.0% |
| V1 | 64 | 16 | 25.0% |

## 4. 六个 family 没有按机器定义实现

运行时实际映射只有两种：

```text
C1/P1/K1 -> M1_pair
B1/M2R/V1 -> M2F
```

- C1 fit 的 `c1_scheduler` 四折全为 null，引擎没有 scheduler call site；
- P1/K1 没有各自的在线 component/Kalman state；
- V1 的 `weights` 不在 `SynchronizedFit`，引擎使用通用 basket 路径；
- B1 的 24 个 fit 全是 `[BTCUSDT, BTCUSDT]` 一类重复字符串，CLI 的 `BTreeSet` 去重后只有一条 market
  series，不是 long spot + short perp；
- `exchange_model.rs` 有局部测试，但 replay/engine 均不调用，`min_liquidation_buffer_pct` 仍为 null。

因此不能把 fit 脚本生成了不同字段等同于 family 已实现。

## 5. causal 与 OOS 合同失败

P3 只生成了 causal schedule。G1/G2 实际仍加载各 outer fold 的 train-end frozen fit 回放更早 inner block；
审计可构造的 8 个 inner block 为 `0 causal / 8 leaky`。

没有 `selected-configs.json`，P8 与 P9 同属 commit `5adf50d`。读取 validation 后又执行 148 个 neighborhood
replay 和 24 个 `direct F4 validation`。所以 12.7% 与 17.7% 都不是 one-shot OOS。2023-01 至 2026-05 的
窗口今后只能作为已读 cross-fit 数据，不能再次宣传为 untouched OOS。

## 6. 已修复代码

1. `glm_r20_state_machine.py`：空 central registry、checkpoint/local-only evidence、无 committed G1 selection、
   无 registry-backed family/causal row 均 fail closed；
2. `glm_r20_r6r7_g0_g1.py`：resolved config 的 `budget_quote` 改为当前实验 budget，禁止 1000U 标签下跑 4999U；
3. `chatgpt_r20_independent_audit.py`：可重复生成全部机器审计和两条独立 replay 证据。

这些修复不会把历史 3946 行自动变有效。真正 spot/perp identity、exchange model call site、family online state、
rolling refit 和 production adapter 必须在 Round 21 搜索前实现。

## 7. 外部扩大检索

网络检索没有找到一条公开策略同时披露多币、`<5000U`、低 equity DD、完整成本和可复现订单流。高收益
Martingale 案例不能直接作为本目标已可达的证据。新增可证伪方向如下：

| 来源 | 可采用机制 | 限制 |
|---|---|---|
| `10.3390/a19060442` | exact Soft Martingale + path-level SEL + DDR | 论文 442.6% 使用单 EUR/USD、1:500，equity DD 79.97% |
| `10.1080/14697688.2017.1403035` | train-only regime-switching spread state | 不得读取未来 state |
| `10.3390/jrfm15040147` | 含交易成本的 pairs singular control boundary | 只控制 Martingale FO/SO/TP，不添加理论 PnL |
| `10.1080/14697688.2017.1374549` | transaction-cost-aware model predictive boundary | 必须预冻结且进入订单 trace |
| `10.1109/TSP.2018.2799193` | budget-constrained mean-reverting portfolio fit | 权重只能由 train stability/cost 选择 |
| `10.1080/00036846.2022.2103506` | fractional-OU persistence veto | 用作均值回归失效 gate，不做独立收益 |
| `10.3390/risks11050093` | multiple-pair diversification | 禁止按已读 validation PnL 配权 |

Round 18 曾规划 first-passage/hazard，因此不能把它重新包装为新 family；Round 21 只允许 transaction-cost
boundary 作为 changed-mechanism enhancement，并须先通过历史 fingerprint 去重。

## 8. 下一步

Round 21 先实现一个真实 multi-market、shared-cash、production-conservative event engine，再运行 C1 scheduler、
B1 spot-perp、P1+Soft-SEL、budget-constrained VECM residual 四个不同运行时机制。旧验证窗口只做预注册 cross-fit；
`2026-07-11+` 满 30 天前，最高只能称 `CROSS_VALIDATED_RESEARCH_FINALIST`，不能称已找到实盘组合。
