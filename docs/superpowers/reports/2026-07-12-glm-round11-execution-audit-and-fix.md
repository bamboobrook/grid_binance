# GLM Round 1-11 完整执行审计与修正

**日期：** 2026-07-12

**审计分支：** `glm-martingale-core-round11`

**审计基线提交：** `4b912e9a73b682ac3f3cc4aeae21a93fae212a13`
**机器可读总状态：** `docs/superpowers/artifacts/glm-martingale-core-round11/r1-r11-corrected-status.json`

## 结论

Round11 的 P4-P7 均没有目标命中，这个“0 target”结论成立；但 GLM 关于
production live-ready、原生 minigrid、搜索覆盖范围和回测总量的描述不成立。

补齐缺失的 ANKRUSDT 资金费并独立复算后，当前有效前沿为：

| 层级 | 候选 | Ann | DD | 分段 | 状态 |
|---|---|---:|---:|---:|---|
| 曲线诊断 | R9 allocator | 62.7845% | 18.3840% | 5/5 curve slices | 非 event-level、非 live |
| Event-level backtest | R7-ANKR-q | 62.3718% | 28.6873% | 4/5 cold-start | 非 fully live-ready |
| 既有 live-ready sleeve | R4-combo | 34.7233% | 17.6843% | 4/5 | 收益不达标 |

三个目标仍全部失败：

- 保守：没有 `ann >=50% / DD <=10%` 的 production/event-level 候选；
- 平衡：修正后研究值只有 `62.78% / 18.38%`，低于 90%；
- 激进：修正后 event-level 候选只有 `62.37% / 28.69%`，低于 110%。

因此不能发布实盘候选，也不能宣称 11 轮已穷尽全部马丁可能性。

## Round1-11 完整性

| Round | 审计状态 | 可继续使用 | 未完成或已修正 |
|---:|---|---|---|
| 1 | baseline only | 初始基线 | 部分证据路径缺失/重命名 |
| 2 | partial research | 已保存失败族 | registry 数错误，Direction H 无独立 full-grid，非 live |
| 3 | partial research | 已完成分支 | P2/P5 未执行，多分支较窄 |
| 4 | valid scoped result | R4-combo 既有 live sleeve | premium/index/mark 分支不完整，不能证明上限 |
| 5 | valid backtest frontier | 已完成 event-level 搜索 | promoted features 非 live，部分 artifact 缺失 |
| 6 | partial DD research | attribution/risk-budget 结果 | safety freeze/dynamic blend/live parity 不完整 |
| 7 | corrected event-level | 六币种 R7 config 和 cold-start segments | ANKR funding 原缺失，非 live，2000U 失效 |
| 8 | superseded allocator | individual sleeves 和 scoped negatives | P2 timing leak，P3/P4 低于计划规模 |
| 9 | curve diagnostic only | forward-only curve calculation | 非共享资金/持仓连续，segment 是 curve slices |
| 10 | valid scoped negatives | SO/trailing/minigrid/defense 近似失败 | production allocator/native minigrid 仍缺失 |
| 11 | zero targets, material corrections | P4-P7 scoped negative evidence | P1-P3 未按计划完成，P5/P6/P7 覆盖被夸大 |

“前 11 轮结果可用”的正确含义是：每个已完成 family 只能在记录的精确范围内作为
负面证据；不能把 partial、approximation 或 curve recombination 视为架构结论。

## Round11 逐项验收

### P1：生产 allocator 未完成

`main.rs` 在 `risk_summary.live_executor_started=true` 后调用
`reconcile_martingale_executor_strategies(...)` 并立即 `continue`。Round11 的
`runtime_with_allocator_state(...)` 只位于初次创建 executor 的路径，因此实际运行后
不再动态 rebalance 或 gate。

另外：

- 全仓库只有 `allocator_observations` reader，没有 production writer；
- inactive sleeve 没有 shadow equity 和 position state；
- executor reconcile 不安装 allocator state；
- 7 个所谓 production tests 都是 helper/direct-runtime tests；
- 没有测试调用 DB-backed `reconcile_running_martingale_portfolios`。

校正：`fully_live_ready=false`，仅 `helper/static-start-gate-ready=true`。

### P2：不是 production parity

`glm_r11_r9_allocator_production_parity.py` 只重建 Python sleeve curves 并调用
`run_allocator_repaired`。历史脚本：

- 把 `forward_only_decisions_pass` 硬编码为 true；
- 只要 P1 evidence JSON 存在，就可设置 production-ready；
- 没有启动 Rust service、DB、executor 或真实 order trace；
- 没有输出逐次 rebalance 决策供校验。

审计已修改脚本：production 字段固定为 false，并明确输出 scope 为
`python_research_curve_replay_only`。

### P3/P4：不是原生 minigrid

P3 只实现 config/validation/math helpers。`kline_engine.rs` 没有读取
`dca_minigrid` 并产生真实 partial reduce event，trading-engine 也没有 reduce-only
partial close order。

P4 脚本明确通过额外 partial TP stages 模拟 minigrid。因此 6912 个 0 target 只能拒绝：

```text
r11-partial-tp-minigrid-approx-6912-no-target
```

原生 minigrid 仍未执行，不能放入 non-repeat。

### P5：99% 是 fixed TP

独立计数：

```json
{
  "fixed_tp": 2304,
  "fixed_tp_v3": 5692,
  "low_mult": 36,
  "vol_ladder": 4,
  "asymmetric": 64
}
```

所谓 4 个 vol_ladder 只改变 ATR pause，不改变 spacing。正确分段分布为：

```text
0/5=4217, 1/5=2707, 2/5=982, 3/5=167, 4/5=27, 5/5=0
```

旧台账使用了 P4 的分布，现已修正。P5 不能用于拒绝真实 ATR ladder 或全部
non-R4 架构。

### P6：两维参数无效

历史脚本把 ADX threshold 和 drawdown state rules 写到 strategy-level；engine 从
portfolio-level 读取。因此 4608 labels 只有 217 组不同 full metrics。

正确分布：

```text
0/5=816, 1/5=1072, 2/5=1472, 3/5=880, 4/5=368, 5/5=0
```

审计已修复脚本字段层级，并增加 `semantics_version=2`；旧 4608 artifact 未重跑，
Round12 必须补跑。rebound/basis/taper/step/FOQ 的旧结果仍可作为精确范围负面证据。

### P7：没有新异构 sleeves

P7 仍使用 R9 的同一组五个 sleeves，没有采用 P4/P5/P6 结果。6912 labels 中：

- 真正非 weight 参数组合只有 768；
- `max_high_ann_weight` 所有正值等价；
- `min_low_dd_weight` 所有正值也只是 eligibility switch；
- 只有 72 组不同 full metrics、316 组不同 full+segments；
- 只运行 5 次底层 binary curve build，41472 是内存 curve calculations。

更重要的是，inactive sleeve 在预计算曲线中继续产生持仓/PnL。切换后直接使用其下一段
PnL，相当于继承未实盘建立的持仓，无法由 static new-cycle gate 复现。

## 数据完整性修正

Round11 manifest：

```text
market_data_full.db cc5b88c113b83d055a88222405cc9d4770eb837524981bf8e5f98bf47392d018
funding_rates.db   356e270dacce36b4703364545beeafe402f1aa55aa11caebc5708f5f3b6767a1
premium_index.db   78bd01280bbb349383e090b9576cfa543865a5680997329c33f2e05c089a2627
```

当前 market DB hash 为
`be2514c88620642538ec38882bb08080995bc5d26e51aafe3012bfed8050f98f`，说明整库
后来追加。目标区间检查显示 R9 的 8 个交易币各有 `1,795,680` 条 futures 1m bar，
覆盖 `1672531200000..1780271940000`，且历史 R9 输出精确复现。因此本次目标区间
bar 数据可继续使用，但没有 immutable range snapshot。

资金费数据库只有 30 个 symbols，完全缺失 ANKRUSDT。通过 Binance 官方：

```text
GET https://fapi.binance.com/fapi/v1/fundingRate
symbol=ANKRUSDT
range=1672531200000..1780271999999
points=3997
json_sha256=599b8be47f42580eb2ad51d0d774424ba8461ca63aee6f81497fda2d93e73997
```

数据仅加入 `/tmp/funding_rates_with_ankr.db` 进行审计，没有覆盖原数据库。

## 独立复算

### R7 full + cold-start segments

| Window | Ann | DD | Return | Funding | Trades |
|---|---:|---:|---:|---:|---:|
| Full | 62.3718% | 28.6873% | 423.8373% | 219.7704U | 377 |
| H1 2023 | 419.3114% | 28.6873% | 126.3458% | 259.1756U | 145 |
| H2 2023 | 9.9197% | 25.2267% | 4.8833% | 42.3235U | 567 |
| 2024 | 62.3234% | 33.6500% | 62.5388% | 155.5788U | 1315 |
| 2025 | -17.9985% | 33.3953% | -17.9985% | -9.7532U | 1983 |
| 2026 YTD | 40.3457% | 20.3781% | 15.0524% | 45.4472U | 488 |

仍为 4/5 正收益，但 2024/2025 segment DD 均超过 33%。

### R9 curve diagnostic

| Input | Ann | DD | Return | Segments |
|---|---:|---:|---:|---:|
| Historical DB | 64.4196% | 18.2111% | 446.7542% | 5/5 curve slices |
| ANKR funding corrected | 62.7845% | 18.3840% | 428.3994% | 5/5 curve slices |

这仍不是 event-level portfolio result。

### Budget ladder spot check

| Budget | Ann | DD | Return | Principal breached |
|---:|---:|---:|---:|---|
| 4999U | 62.3795% | 28.6894% | 423.9221% | false |
| 2000U | 5.4232% | 45.5823% | 19.7734% | false |

这证明 `max_capital_used=1729.28U` 不能替代真实小预算 replay。

## 修复清单

本次已完成：

1. 修正 R9/R11 parity 脚本的伪 production flag 与硬编码 forward-only pass。
2. 修正 Python/Rust allocator 文档，明确 curve reuse、binary eligibility 和非 event-level。
3. 修正 P6 脚本 ADX/DD rules 的 portfolio-level 字段位置。
4. 重命名误导性的 production tests，明确它们不调用 DB reconcile。
5. 修正 Round11 P1/P2/P3 evidence JSON、final validation、ledger 和 registry。
6. 修正 R9 winner 的 total return 与 promotion/live 状态，并记录 funding-corrected metrics。
7. 新增 Round1-11 canonical status，统一每轮可用范围和 non-repeat keys。
8. 新增 Round12 严格 event-level/native/OOS 计划。

未在本次直接实现：production shadow sleeves、native minigrid 和 event-level dynamic
allocator。这三项涉及共享资金、持仓生命周期和真实订单语义，必须按 Round12 的先测试后
实现顺序完成，不能再用 Python curve approximation 代替。

## 工程验证

```text
python3 -m py_compile ...                         PASS
patched R9 research replay                       PASS 64.4196/18.2111
patched script production_live_ready             false
cargo test -p backtest-engine                    PASS 219 tests
cargo test -p trading-engine (outside sandbox)   PASS 205 tests
git diff --cached --check                        PASS
```

`trading-engine` 在 workspace sandbox 内唯一失败是本地测试 HTTP server 无权 bind；
同一命令在沙箱外重跑 205 tests 全部通过。

`cargo fmt --all -- --check` 仍失败，但差异遍布 Round11 基线中既有未格式化文件；本次
没有执行全仓 format，避免引入与审计无关的大量机械变更。

## 下一步

GLM 必须执行：

`docs/superpowers/plans/2026-07-12-glm-martingale-core-round12-event-level-native-search-plan.md`

任何候选只有通过 event-level shared-budget replay、cold-start/WFO、完整 funding、
1000/2000/3000/4000/4999 budget ladder 和 DB-backed live parity 后，才能计入三档目标。
