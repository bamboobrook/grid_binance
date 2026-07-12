# GLM Martingale Round 12 执行审计与修正

日期：2026-07-12

审计输入 HEAD：`5fa73efb5ab0b4b546a4c70d9dd43b9808b0e3dd`

原计划：`docs/superpowers/plans/2026-07-12-glm-martingale-core-round12-event-level-native-search-plan.md`

## 1. 权威结论

Round 12 原 handoff 的“P0-P10 全部闭合”不成立。P3、P4、P5、P6、P7、P8、P9、P10
均有执行偏离或缺项；P1、P2 也存在会导致错误 promotion 的实现问题。

审计修复后仍无三档命中：

| 档位 | 要求 | 当前可复现前沿 | 结论 |
|---|---|---|---|
| 保守 | ann >=50%，DD <=10% | R4 `34.7284% / 17.6868%` | 未达 |
| 平衡 | ann >=90%，DD <=20% | R4 `34.7284% / 17.6868%` | 未达 |
| 激进 | ann >=110%，DD <=30% | R7 corrected `62.3718% / 28.6873%` | 未达 |

没有任何候选同时通过 nested WFO、邻域、symbol holdout、成本压力、完整生产 DB reconcile
和 backtest/live order trace，因此 `fully_live_ready` 候选为空。

## 2. 独立复算结果

复算配置：`r4-combo-best.json`

最终 release binary SHA256：
`afb70e9aedba2cd974733206fb468c9a27e8f1943d81a28904933c3dafc0a755`

### 2.1 Development full 与预算阶梯

| Budget | Annualized | Max DD | Total return | Principal breach |
|---:|---:|---:|---:|---|
| 1000 | -15.7285% | 51.0143% | -44.2695% | 否 |
| 2000 | -7.0614% | 27.1500% | -22.1348% | 否 |
| 3000 | 49.4656% | 24.7951% | 294.7379% | 否 |
| 4000 | 40.6943% | 20.6445% | 221.0534% | 否 |
| 4999 | 34.7284% | 17.6868% | 176.8781% | 否 |

4999U full replay 为 4758 trades、19 个 budget-blocked legs，最大实际 capital used
仅 `707.6238U`。低预算失败与高预算利用率偏低同时存在，说明下一轮应优先研究真实
active-cycle 调度和资金预留语义，而不是继续盲扫 TP 数字。

### 2.2 五个独立 cold-start segments

| Segment | Annualized | Max DD | Return |
|---|---:|---:|---:|
| H1-2023 | 58.9053% | 17.6868% | 25.8178% |
| H2-2023 | 7.0830% | 14.2970% | 3.5100% |
| 2024 | 31.8525% | 22.1915% | 31.9523% |
| 2025 | -8.7067% | 21.1678% | -8.7067% |
| 2026-YTD | 18.2295% | 8.9780% | 7.1733% |

结果为 `4/5` 正收益，但这不是 anchored WFO：没有在每个 train fold 内重新选参。

### 2.3 修复后的 holdout

原 holdout 数据在 15-symbol 冻结宇宙中缺少 36,351 根 futures 1m K 线，其中 R4
实际使用的 6 symbols 缺 13,563 根；R4 资金费仅到 2026-06-22。审计从 Binance
官方 USD-M endpoints 补齐，并保存每个 HTTP response stream SHA256。修复后每个
symbol 都为 `57,600/57,600` 根 K 线，funding 边界和内部 gap gate 均通过。

| Budget | Holdout return | Max DD | Trades | Principal breach |
|---:|---:|---:|---:|---|
| 1000 | -18.5035% | 21.8036% | 429 | 否 |
| 2000 | -9.2517% | 11.1271% | 429 | 否 |
| 3000 | -18.2618% | 21.2208% | 408 | 否 |
| 4000 | -13.6963% | 16.0609% | 408 | 否 |
| 4999 | -10.9593% | 12.9221% | 408 | 否 |

旧数值恰好接近修复值，但旧执行缺数据完整性证明。修复后的结论才有效：R4 的该次
holdout 明确失败。该窗口已经打开，后续不能再对 R4 调参后称其为 untouched holdout。

## 3. P0-P10 完整性审计

| Task | 原声称 | 审计状态 | 可用范围 / 缺口 |
|---|---|---|---|
| P0 | 完成 | 部分 | registry 后 3 行缺 14-15 个必填字段；多项记录无 resolved hash/checkpoint |
| P1 | 完成 | 已修正可用 | 原 manifest 混入 spot/higher TF、伪造大文件 hash、无 missing-minute；loader 静默接受缺 funding。现已修复。历史 development HTTP hash 仍无法追溯 |
| P2 | 完成 | 未完成 | 原脚本没有 neighbor/symbol robustness；固定参数 segment 被叫作 WFO；四次 base replay 被叫作 cost stress；曾可错误派生 live-ready。现改为 fail-closed |
| P3 | family failed | 偏离计划 | 实际只把 5 sleeves 的 36 strategies 静态同时运行。`-7.5209/56.0007` 只否定该静态 merge，不否定 shadow/live dynamic allocator |
| P4 | 完成 | 未执行 | 只证明 `dca_minigrid` 字段在当前 kline engine inert；未实现 inventory reduce/recycle、live order 或 trace parity |
| P5 | 完成 | 未执行 | ADX 35/70 同结果没有原始 artifact，且 ADX 路径实际与 TP 类型无关；`safety_order_scale` 确实未被 engine 读取，审计已修复 backtest 路径。HTF trend-directed Martingale 完全未跑 |
| P6 | 576 depth configs | 错误命名、窄结果有效 | partial TP stage 由 TP fill 推进，不由 safety-leg depth 推进。576 个普通 partial-stage + max-age configs 为有效负证据；真实 ATR/depth-aware family 未跑 |
| P7 | LP rebuild 完成 | 严重缩减 | 只把 R4 参数复制到 conservative 的 8 个 symbols；未恢复 LP member 原 ladder，未做 2/4/6/8 ablation 或 1000 trials/profile。该单配置 `30.4412/41.3107` 可复现 |
| P8 | 完成 | 部分 | 只做 benchmark；preload+Rayon、>=5x 和 parity benchmark 均未实现。15% efficiency 的瓶颈结论也没有 profiler artifact |
| P9 | 完成 | 部分且已修复 | 原数据不完整、verdict 文本与负收益矛盾；现 R4 holdout 已有效复算。未执行 WFO/neighbor/LOSO/stress，也没有合格 finalists |
| P10 | 完成 | 未执行 | 所有 Round12 指定的 DB-backed allocator/minigrid 测试均缺失；已有 suite 通过不能证明新机制 production parity |

## 4. 已实施修复

1. `load_funding_rates_readonly` 现在拒绝 missing symbol、前后覆盖不足及区间内部 >9h gap。
2. 真实 SQLite loader 测试替换了手工构造预期 JSON 的伪 missing-funding test。
3. manifest 严格过滤 `futures_usdt_perp/1m`，计算缺分钟、重复键、off-grid、内部 gap、
   canonical row hash、完整 DB SHA256、funding/premium coverage 和 engine SHA256。
4. 新增官方数据修复脚本与 provenance artifact；development 和 holdout data gate 均通过。
5. validator 改为 fail-closed；不再伪报 WFO、cost stress 或 `fully_live_ready=true`，并检查
   candidate symbols 必须属于 manifest。
6. 补上 backtest engine 的 `drawdown_state_rules.safety_order_scale` 实际执行路径和确定性测试；
   trading-engine parity 仍须 Round13 完成。
7. P3/P6/P7/P8/P9 的产物和 non-repeat scope 已收窄，不再误封未实现机制。

## 5. 可继续使用的 Round12 负证据

- 精确静态组合：5 个 R9 configs、36 strategies、同时 active、shared 4999U，结果
  `-7.5209% / DD56.0007%`。禁止原样重复该 merge。
- 精确 partial-TP grid：576 个 `spacing x TP-stage0/1/2 x max-age` 配置，0 个 ann >=50%，
  0 个达到 4/5 正收益；最佳 ann `25.9807% / DD28.5702% / 2/5`。
- 精确 LP 替代配置：8 个 conservative symbols + 统一 R4-like ladder
  `(FOQ15, 2.8x, 8 legs)`，`30.4412% / DD41.3107%`。禁止只换 symbols 后重复。
- R4 corrected full、cold-start、budget ladder 和 funding-complete holdout。

## 6. 不能继续沿用的结论

- “R9 dynamic allocator family failed”。动态 allocator 尚未实现。
- “cycle-depth TP 576 configs failed”。运行的是 TP-stage，不是 cycle depth。
- “ADX 对 partial TP 不 bind”。代码不支持该归因，且无原始 probe artifact。
- “所有 LP-derived <5000U 都会 DD inflation”。只测试了一个伪重建配置。
- “R4 fully live-ready”。严格 P9/P10 证据缺失且 holdout 失败。
- “Round12 P0-P10 全部闭合”。事实不成立。

## 7. Round 1-12 总状态

Round1-11 继续以
`docs/superpowers/artifacts/glm-martingale-core-round11/r1-r11-corrected-status.json`
为审计基础；本轮没有推翻其中任何 target verdict。Round7/Round9 的 corrected funding
结果仍是研究前沿，但都不是可上线目标候选。

新的总权威状态：
`docs/superpowers/artifacts/glm-martingale-core-round12/r1-r12-corrected-status.json`。

下一轮必须优先完成未执行机制，而不是继续在旧 R4 参数附近做大网格：
`docs/superpowers/plans/2026-07-12-glm-martingale-core-round13-native-regime-portfolio-plan.md`。
