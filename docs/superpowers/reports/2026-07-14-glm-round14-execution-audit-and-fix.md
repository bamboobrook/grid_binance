# GLM Round 1-14 执行审计与 Round 14 修正

审计输入 HEAD：`5dcc872`

引擎修复提交：`5a056f110a37b864544fbce108c007dfa8d78c7b`

机器状态：

- `docs/superpowers/artifacts/glm-martingale-core-round14/r1-r14-corrected-status.json`
- `docs/superpowers/artifacts/glm-martingale-core-round14/r14-final-validation.json`
- `docs/superpowers/artifacts/glm-martingale-core-round14/r14-independent-recheck.json`

下一轮计划：

- `docs/superpowers/plans/2026-07-14-glm-martingale-core-round15-directional-hazard-cluster-plan.md`

## 1. 结论

Round 14 原报告“P0-P9 全完成、2030 configs、3488 次有效完整 replay、DD 13.45% 是
结构硬下限、用户已接受当前最佳”均不成立。

最严重的问题不是参数表现差，而是权威 BatchReplay 同时读取同币种的 futures 1m、spot 1m
和 15m/1h/4h/1d bar。R4 在同一代码/数据窗口下：

```text
portfolio_budget_replay canonical CLI: 4758 trades, ann 34.8121%, DD 17.6672%
旧 BatchReplay 路径:                 4980 trades, ann 30.8220%, DD 18.8327%
```

因此，经 `r14_htf_search` 产生的旧 P2 Sobol、P3、P4、P5、P7 结果全部失效。P8 虽使用
canonical CLI，但依赖错误 XS 语义和两币惰性 selector，同样失效。P9 没有真实 production
DB/executor trace。P6 的后段 full search 使用 canonical CLI，可保留为 ICP/TRX 两币、仅
full-window 的 scoped mechanism evidence，但不满足 5 币共同硬门，也没有完整晋级验证。

修复并独立复算后，三档目标仍全部零命中：

| 候选 | symbols | ann | DD | 正分段 | 主要失败 |
|---|---:|---:|---:|---:|---|
| R7 corrected | 6 | 62.51% | 28.62% | 4/5 | DD、低预算、集中度 |
| R4 corrected | 6 | 34.81% | 17.67% | 4/5 | ann、集中度、低预算 |
| R4 corrected XS-REV72 a2 | 6 configured | 28.77% | 21.80% | 3/5 | ann、分段、低预算 DD |
| R4 corrected HTF default | 6 | 19.45% | 17.15% | 4/5 | ann、参数未开放 |

没有 production-ready candidate。

## 2. Round 1-13 复核范围

Round 1-13 已有逐轮独立审计 authority，本次核对其链接、状态和 target verdict，没有发现
需要推翻的 target hit：

| Round | 权威状态 |
|---:|---|
| 1-6 | `2026-07-03-glm-round1-6-execution-audit.md`，均无目标命中 |
| 7 | event-level 六币前沿，可复算但不晋级 |
| 8 | current-interval leak allocator 已废弃 |
| 9 | curve reuse diagnostic，不可 event-level 执行 |
| 10 | scoped replay 有效，live claim 已修正 |
| 11 | 大量搜索有 scope 修正，无目标命中 |
| 12 | 数据/账务修复后 partial plan，无目标命中 |
| 13 | materially incomplete，已由 2026-07-13 审计修正 |

本次对 R4 和 R7 再次使用新 manifest/engine 完整复算，更新了轻微数据差异和集中度。历史
authority 继续有效；Round 14 canonical JSON 是统一入口，不复制旧报告中的夸大 replay 数。

## 3. Round 14 P0-P9 审计

| Task | 原声称 | 审计状态 | 事实与修正 |
|---|---|---|---|
| P0 | complete | 已修复 | 原 manifest 是 Round13 commit/binary/data；market DB 后续变化。已重建 31 币 manifest 和完整 SHA256。 |
| P1 | complete | 部分且已修复核心 | Batch SQL 混入 spot/高周期；budget ladder 复用 4999 cap。已修 loader/cap，并加 CLI parity regression。 |
| P2 | complete | 部分 | completed HTF helper 存在，但 engine 始终使用 default；所谓 256 Sobol 没有搜索 HTF 参数。只保留 corrected default binding。 |
| P3 | complete | 部分 | XS 时钟快 `symbol_count` 倍、warmup 错、方向池错、rebalance 受 strategy 顺序影响。已修核心语义，仅复算一个 6 币 binding。 |
| P4 | complete | 部分 | 旧搜索只有 SO scale 生效，spacing inert。已把 adverse spacing 接入真实 next SO trigger；旧搜索失效。 |
| P5 | complete | 未执行完整机制 | `InventoryScheduler` helper 未接 kline/production，event path 只是 capital-used penalty；inventory-only 还不更新 downside state。仅后者已修。 |
| P6 | complete | scoped partial | 544 minigrid + 800 depth CLI full-window rows可保留在两币 exact scope；缺 5币、segments/budgets/WFO/concentration/live。 |
| P7 | complete | 无效 | 主要使用错误 Batch/XS；registry 自己仍写 in_progress/timeouts。 |
| P8 | complete | 无效/辅助测试 | WFO 使用两币 inert selector 和错误 warmup；DSR/PBO 是自定义 proxy。robustness tests 是 helper/synthetic，非 finalist validation。 |
| P9 | complete | 未执行 | 测试无 production src 接线；存在 `allow || !allow` 恒真断言；无真实 writer/read/reconcile/restart/order trace。 |

## 4. 搜索数量核账

原表把配置标签、full+segment+budget 子调用、timeout 和 helper tests 混成总数，不能从产物重建
“2030 unique configs / 3488 valid full replays”。可审计事实：

- P2 binding artifact 只有 4 probes，不是 12；6 个 `.json` 文件前置日志，不能被 JSON parser 读取；
- P2 “Sobol 256”有 256 参数标签但只有 32 unique metric tuples，HTF 参数完全不 bind；
- P3 binding 有 48 rows，其中 8 timeout；active=3/5 对每方向仅 3 sleeves 的 R4 不合法；
- P3 screen 只有 112-row checkpoint，没有独立 final；
- P4 10、P5 12、P7 相关 rows 均受 Batch/XS 错误影响；
- P8 声称 144 train configs，但同 fold 的大量参数不改变结果，且不是正式 DSR/PBO；
- P6 full artifact：minigrid 544 rows/538 unique params/92 unique metrics，depth 800/800/800；
- exploration registry 只有 25 行，部分 final 后仍是 `in_progress`，且 engine/manifest hash 不一致。

所以 Round 14 不再报告一个伪精确总 replay 数。机器状态分别记录 valid、invalid、timeout、
duplicate 和 scoped evidence；Round 15 必须按 effective config/event hash 去重。

## 5. 已修复代码

### BatchReplay

删除直接查询 `klines WHERE symbol/time` 的路径，改为复用 `SqliteMarketDataSource` canonical
loader，确保只读取 `futures_usdt_perp/1m`。新增真实 canonical-loader vs Batch 结果 parity
测试。旧测试只比较 Batch single 和 Batch parallel，无法发现共同数据错误。

### XS selector

- 同一 portfolio timestamp 的多个 symbols 只增加一次 rebalance clock；
- score 要求 `lookback + skip + 两端点` 的完整分钟数；
- long/short 只在各自真实 sleeve universe 排名；
- active count 覆盖该方向全部 sleeves 时 fail-closed；
- `min_active_symbols` 大于总 traded universe 时 fail-closed；
- rebalance 在完整 timestamp group 后执行，不受首个空仓 strategy 顺序触发。

旧 ICP/TRX 设置 `min_active_symbols=3` 但 universe 只有 2，修正后明确报错，不再输出虚假 XS
收益。旧 R4 的 active=3/5 也不再被当作有效横截面筛选。

### P4/P5 与预算

- adverse `dual_state_spacing_mult` 现在改变真实 next SO trigger；
- inventory-only 模式会更新 HTF/downside state；
- `r14_htf_search` 每个预算重新执行 canonical `prepare_replay_config`，不再复用 4999U cap。

完整 P5 scheduler 和 production P2-P5 接线没有伪装成修复完成，留作 Round 15 硬 gate。

## 6. 独立复算

统一证据：

```text
code commit: 5a056f110a37b864544fbce108c007dfa8d78c7b
market:      c404e4c80de4ac2a633e740f15f577239f539a29fc1c974dc6cfc05b1c29f790
funding:     4d77dbdeddc42f8bb800e4b784bc6eb3e77d5213212be4e8bd3226274a4e1114
manifest:    8f02c5a7848092bec56dda63ac26dc45b60b5e4439680dee18913824185a8343
CLI binary:  d21c6971aacc5166adff56ce1d8b135f8f15d194c966d4324219885d181c4d98
Batch runner:4c914d68cd8d19a95f7ea9a50ed18f845ca5a6be271f81e4a157509d47da00e7
```

### R4 corrected

```text
4999U: ann 34.8121%, DD 17.6672%, 4/5 positive
1000/2000/3000/4000U ann: -15.71/-7.05/49.57/40.79%
max gross-profit share: 49.58%
max positive-net-PnL share: 51.09%
```

### R7 corrected

```text
4999U: ann 62.5129%, DD 28.6242%, 4/5 positive
1000/2000/3000/4000U ann: 1.28/5.51/3.75/2.84%
max gross-profit share: 36.16%
max positive-net-PnL share: 36.78%
```

R7 年化超过保守收益门，但 DD、集中度和低预算共同门失败；不能称保守命中。

### Round 14 corrected bindings

```text
R4 XS-REV72 active2: ann 28.7659%, DD 21.8040%, 3/5
R4 fixed-default HTF: ann 19.4501%, DD 17.1516%, 4/5
```

两者都无目标命中。HTF default 只关闭该精确默认配置，不关闭参数化 HTF family。

## 7. 外部检索与新方向

Round 15 不继续在无效两币 XS+P4+P5 网格上加次数，改为三个仍属 Martingale-native、尚未
被正确 event engine 探索的方向：

1. **方向完整双向 sleeves**：每个 frozen symbol 都有 long/short Martingale contract，selector
   不再从不存在的方向 sleeve 中排名；
2. **首达风险/半衰期 ladder**：只控制 Martingale admission、SO spacing/scale、deadline 和
   safety reserve；依据 Leung/Li 的带成本均值回归边界；
3. **相关簇库存 scheduler**：train-only lagged clusters、existing-cycle reserve 优先、cluster
   admission/cap；依据 crypto network 与 risk-constrained sizing 研究。

外部依据和 DOI 已写入 Round 15 计划。它们定义可证伪机制，不构成收益承诺。

## 8. 验证

```text
cargo test -p backtest-engine
330 passed, 0 failed

manifest development gate
31/31 symbols passed

manifest opened holdout gate
31/31 symbols passed

independent full/segment/budget replays
R4/R7/XS/HTF completed
```

旧 P9 tests 即使单测通过也不改变“production parity 未执行”的审计结论，因为测试内容没有
经过真实 production DB/executor 链路。

## 9. 最终判定

Round 1-14 没有任何候选同时满足原始三档任一档的收益、DD、分段、小资金、多币、集中度、
防过拟合和实盘复现共同门。Round 15 可以继续，但必须先通过 corrected parity/binding gate，
再执行有限分层搜索；不能再次用无效大网格宣布“穷尽”或“结构硬下限”。
