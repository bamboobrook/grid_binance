# GLM Martingale Round 1-13 完整执行审计与修正

日期：2026-07-13

审计输入 HEAD：`e320f29d1c983744ec8f589dd312607feccd2fb6`

机器可读权威状态：

- `docs/superpowers/artifacts/glm-martingale-core-round13/r1-r13-corrected-status.json`
- `docs/superpowers/artifacts/glm-martingale-core-round13/r13-final-validation.json`
- `docs/superpowers/artifacts/glm-martingale-core-round13/r13-independent-recheck.json`

## 1. 权威结论

Round 13 原 handoff 的“P0-P9 全部完成”不成立。P2 没有实现真实 HTF，P3 没有接入
event/production 主路径，P4/P5 的成交账务错误，P6 配置来源被误标，P7 不是 nested WFO，
P8 holdout 已污染，P9 测试没有调用真实 executor/DB。

同样不能诚实地宣称前 13 轮已 100% 按计划执行。修正后的“可用”含义是：

1. 每轮已完成实验只在精确 scope 内保留；
2. 缺原始命令、hash、正确引擎语义或生产路径的结论降级；
3. 错误语义运行的搜索不进入 non-repeat，必须在修复后重跑；
4. 不通过硬门的候选一律不发布实盘。

修复后仍无三档命中：

| 档位 | 要求 | 修复后结论 |
|---|---|---|
| 保守 | ann >=50%，DD <=10%，4/5 正，组合/小资金/实盘门 | 未达。低 DD 历史候选收益远低于 50%；当前组合前沿 DD >=17.67% |
| 平衡 | ann >=90%，DD <=20%，4/5 正，组合/小资金/实盘门 | 未达。修复后多币 event-level 最高 ann 62.51% |
| 激进 | ann >=110%，DD <=30%，3/5 正，组合/小资金/实盘门 | 未达。最高两币诊断 65.75%，六币前沿 62.51% |

`fully_live_ready_candidates=[]`。

## 2. 独立复算证据

复算使用：

```text
engine SHA256   ffeba0f7a01e98027be3b55c4238f49747c96dbae71beef81652ebc80ffc4d04
market SHA256   01e1de0d1d4d17889a4a318b81ace01980fe38519dd942302654ffde502bedd1
funding SHA256  4d77dbdeddc42f8bb800e4b784bc6eb3e77d5213212be4e8bd3226274a4e1114
manifest SHA256 a754cf69f53279d77caa06a03f24ee75ed8fed96c8776a47edce886e0d71dbc0
result SHA256   d609e5b83ad93791e4f645f751f1b12f1dfa0ce4e2b2bbfdd19001529cda85b8
```

31 个 symbols 的 development 均为 `1,795,680/1,795,680` 根 futures 1m K 线；holdout
均为 `57,600/57,600`，market/funding gate 全通过。新增数据有 Binance USD-M 官方响应
stream hash。历史 development HTTP 响应 hash 仍无法追溯。

5 个配置各运行 full、5 cold-start segments、5 budgets、opened holdout，共 `60/60`
实际 binary replays，错误数为 0：

| 候选 | Symbols | Ann | DD | 正分段 | 1000U ann | 2000U ann | Holdout return | 最大正收益贡献 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| R7 ANKR-q | 6 | 62.5129% | 28.6242% | 4/5 | 1.2847% | 5.5052% | -19.1888% | TRX 36.1596% |
| R4 corrected | 6 | 34.8121% | 17.6672% | 4/5 | -15.7107% | -7.0544% | -10.9593% | BNB 49.5777% |
| robust-pool ICP+TRX long | 2 | 36.3790% | 28.6797% | 4/5 | 59.0903% | 29.2767% | +10.2704% | ICP 58.2860% |
| R4-like BNB+TRX long | 2 | 65.7460% | 29.5741% | 3/5 | -12.7451% | 35.9590% | -35.8911% | TRX 89.5786% |
| robust-pool 5-long | 5 | 48.2836% | 28.6759% | 2/5 | 59.5558% | 66.1944% | -9.4740% | BTC 50.2420% |

“最大正收益贡献”是逐 event 的 realized positive net PnL share。计划要求的 gross profit share
仍缺独立字段，因此必须 fail-closed；不能把当前数值包装成 gross concentration pass。

### 2.1 R7 六币前沿

R7 不是单币，而是 BNB/TRX/ANKR long + AAVE/SOL/DOT short 六币组合。修复后 full 略变为
`62.5129% / 28.6242%`，仍为 4/5 正分段：

| Segment | Ann | DD | Return |
|---|---:|---:|---:|
| H1-2023 | 421.6213% | 28.6242% | 126.8445% |
| H2-2023 | 10.0981% | 25.2246% | 4.9691% |
| 2024 | 62.4481% | 33.6110% | 62.6640% |
| 2025 | -17.9725% | 33.3929% | -17.9725% |
| 2026-YTD | 40.5258% | 20.3773% | 15.1135% |

它不能晋级：

- TRX `36.1596%`，BNB `35.6693%`，均触及或超过 35% 附近，TRX 已明确超限；
- opened holdout 为 `-19.1888% / DD20.0352%`；
- 1000/2000/3000/4000U ann 分别仅 `1.28/5.51/3.75/2.84%`，4999U 才跳到 62.51%；
- 这种预算悬崖不符合“小资金可稳定复现”；
- nested WFO、生产 DB/executor/order trace 仍缺失。

### 2.2 Round 13 新组合

ICP+TRX 与五币 long 配置不是原 LP portfolio。它们从 `glm_robust_pool.json` 抽取单个
long leg 后静态合并：

- ICP+TRX 只有 2 symbols，且 ICP 正收益贡献 58.29%；
- 五币配置虽然满足数量，但只有 2/5 正分段、holdout 为负、BTC 贡献 50.24%；
- BNB+TRX 两币 ann 最高，但 TRX 贡献 89.58%，holdout -35.89%。

这些配置可作 corrected diagnostics，不能称多币风险分散成功。

## 3. 前 13 轮完整性分类

| Round | 修正状态 | 可继续使用 | 不能继续声称 |
|---:|---|---|---|
| 1 | baseline only | 初始马丁基线 | 完整覆盖所有计划 |
| 2 | partial research | 有 artifact 的精确失败族 | Direction H 独立全网格、live-ready |
| 3 | partial research | 已完成分支 | P2/P5 已执行、全方向闭合 |
| 4 | corrected baseline | R4 event-level 与修复后复算 | holdout/集中度/目标通过 |
| 5 | stale frontier | 历史方向和配置可定位 | vol-target 旧数值完全有效 |
| 6 | partial DD research | 风险归因和精确搜索 | 所有 state/freeze/live 语义闭合 |
| 7 | corrected event-level | 六币 config 与本次 60 replay 中的结果 | 小资金稳定、集中度通过、live-ready |
| 8 | superseded | 单 sleeves 和精确窄失败 | 当前区间 allocator 无泄漏 |
| 9 | curve diagnostic | forward-only curve 诊断 | shared-account event-level/live 结果 |
| 10 | scoped negatives | strict SO/trailing 等精确近似范围 | 原生 minigrid、完整 production allocator |
| 11 | zero target + corrected scope | P4-P7 精确负证据 | 159k 都是 binary replay、P1-P3 完成 |
| 12 | corrected partial | R4、静态 merge、普通 partial grid | dynamic allocator、真实 depth/minigrid 完成 |
| 13 | materially incomplete + corrected | 本报告列出的数据、代码回归和 60 replays | P0-P9 全完成或 13,488 次均可审计 |

Round 1-12 的详细 authority 继续分别由既有 audit 报告承担；本次 canonical JSON 统一了最终
状态和优先级，没有删除历史探索记录。

## 4. Round 13 P0-P9 审计

| Task | GLM 声称 | 修正状态 | 证据与缺口 |
|---|---|---|---|
| P0 | complete | 已修正 | registry 第 21 行非法 JSON；manifest 只是 Round12 副本，漏 ICP 等 16 币 |
| P1 | complete | partial | 原实现非 Rayon；现已真并行，但缺 cache/resume、20-config CLI parity、5x benchmark |
| P2 | 1176 HTF configs | wrong mechanism | 脚本只是当前周期 BTC close/EMA expression，无 1h/4h completed state |
| P3 | allocator/scheduler complete | helpers only | helper 未控制 replay、started executor、writer/reconcile；参数“不 bind”不构成 family 失败 |
| P4 | 512 minigrid failed | invalidated | 原引擎删 safety leg，不记真实 PnL/成本/资本；也不按聚合持仓均价结算 |
| P5 | 432 depth TP failed | invalidated | reduce pct 用错、partial entry cost 重复扣、depth 状态不完整 |
| P6 | original LP recovered | misprovenanced | 28 configs 来自 robust pool；仅 LTC candidate ID 与旧 LP 报告一致 |
| P7 | strict validation complete | partial | 固定参数分段重放不是 nested WFO；gross concentration、delay/funding stress 不完整 |
| P8 | holdout complete | contaminated | 窗口已打开，原 manifest 又缺候选币；只能作 opened diagnostic |
| P9 | production parity complete | not executed | 同名 tests 只测 helper/config/serialization，未调用 executor 或 DB |

原 `total_search_configs=2248`、`total_replays=13488` 缺完整 raw command、resolved hash、失败结果
和 checkpoint accounting，不能作为可审计 replay 总量。它们可以表示脚本标签规模，不能表示
13,488 个独立有效实验。

## 5. 已实施代码与数据修复

1. partial/depth proportional reduce 同比缩减剩余 entry fee/slippage，避免后续重复扣费；
2. minigrid 变为真实 reduce-only event，记录 PnL、fee、slippage、资本释放和 creation-bar guard；
3. minigrid 按交易所聚合持仓均价结算，不再假装可单独卖出盈利 safety tranche；
4. 每 safety band 的 reduce cap 不超过对应 safety quantity；
5. depth TP 使用自身 reduce 参数，每个 depth 只触发一次；
6. 被 budget 拒绝的 orders 不再虚增 fee/slippage；
7. vol-target 后的 notional 同时用于 budget、cost、capital 和 event；
8. rolling stop/quarantine 历史不再被普通 cycle reset 清空；
9. engine 默认 minNotional 从 0 修正为 5U；
10. replay 新增逐币 realized positive PnL contribution；
11. BatchReplay 使用 Rayon，合并只读数据共享，SQLite decode 错误 fail-closed；
12. 新增执行级 minigrid、depth TP、费用和 batch parity 回归测试；
13. manifest 扩展到 31 币，补齐官方 holdout K 线/资金费和响应 hash。

## 6. 有效与失效的 Non-Repeat

可以精确跳过：

- Round12 的静态 36-strategy simultaneous merge；
- Round12 普通 partial-stage + max-age 576 grid；
- Round10 strict all-or-nothing conditional SO；
- Round11 fixed-TP dominated ranges；
- Round13 当前周期 BTC EMA pseudo-HTF 表达式；
- Round13 未接 event loop 的 R4 scheduler helper screen；
- 本次 5 个 exact config hashes 在相同 engine/data/window 上的 60 replays。

必须重跑，不能登记为“已穷尽”：

- aggregate-position 语义的 native minigrid；
- corrected reduce/cost 语义的 depth TP；
- 受 vol-target capital/cost mismatch 影响的历史 families；
- 真 completed 1h/4h per-symbol HTF regime；
- event-level cross-sectional Martingale selector；
- production-integrated allocator/scheduler；
- original LP configs（若未来找到真实 source）。

## 7. 外部检索与 Round 14 方向

本次通过 Crossref/DOI 元数据核实了以下研究：

- Time-series momentum：`10.1016/j.jfineco.2011.11.003`；
- Crypto cross-sectional momentum：`10.2139/ssrn.4322637`；
- Cryptocurrency momentum/reversal：`10.3905/jai.2023.1.189`；
- Volatility-managed portfolios：`10.1111/jofi.12513`；
- Downside volatility management：`10.3905/jpm.2020.1.162`；
- Variance ratio：`10.1093/rfs/1.1.41`；
- Adaptive crypto markets：`10.1016/j.irfa.2019.05.008`；
- Inventory risk：`10.1007/s11147-009-9036-3`；
- PBO/Deflated Sharpe：`10.21314/jcf.2016.322`、`10.3905/jpm.2014.40.5.094`。

它们只被转化为 Martingale cycle 的方向、symbol 选择、SO/spacing、库存和风险控制；禁止
独立趋势收益。下一轮详细计划：

`docs/superpowers/plans/2026-07-13-glm-martingale-core-round14-htf-selector-inventory-plan.md`

Round 14 的主线为：

1. 真 completed-HTF time-series regime；
2. event-level cross-sectional momentum/reversal Martingale selector；
3. mean-reversion/trend 双状态 safety ladder；
4. inventory-aware reserve + downside-vol scaling；
5. corrected minigrid/depth TP 重跑；
6. nested WFO、PBO/DSR、LOSO/LOCO、压力和生产 DB trace。

## 8. 工程验证

```text
cargo test -p shared-domain                         PASS 10
cargo test -p backtest-engine                       PASS 262
cargo test -p trading-engine (sandbox)              1 local-bind PermissionDenied
cargo test -p trading-engine (outside sandbox)      PASS 215
python3 -m py_compile (3 modified scripts)           PASS
jq empty corrected JSON files                        PASS
jq -s exploration-registry.jsonl                     PASS 46 records
rustfmt --check (7 modified Rust files)               PASS
git diff --check                                     PASS
```

`cargo fmt --all -- --check` 仍失败，差异来自 Round 13 基线中大量未格式化的 allocator、
budget replay、trading tests 等文件；本次 7 个 Rust 改动文件单独检查全部通过。没有对全仓执行
机械格式化，以免混入与审计无关的上千行 churn。

## 9. 最终判断

修复后的证据否定的是当前候选，不是全部 Martingale 可能性。最有价值的新事实不是“继续
调 TP”，而是：

- 4999U 与 1000-4000U 之间存在严重 budget cliff；
- 多数收益仍集中于 BNB/TRX/ANKR；
- 2025 和 opened holdout 继续暴露逆趋势加仓问题；
- 真 HTF、横截面选择和 event-level allocator 实际尚未探索。

因此应执行 Round 14 计划，但任何收益目标都不能被预先保证。只有通过全部硬门的真实组合
才可报告目标达成。
