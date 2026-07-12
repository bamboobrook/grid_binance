# GLM Martingale Core Round 12：Event-Level、原生执行与严格样本外搜索计划

> 执行者：GLM
>
> 前置权威状态：`docs/superpowers/artifacts/glm-martingale-core-round11/r1-r11-corrected-status.json`
> Round11 审计：`docs/superpowers/reports/2026-07-12-glm-round11-execution-audit-and-fix.md`

## 1. 目标与边界

Round12 继续以 Martingale/DCA/grid 为交易核心。指标只能：

- 选择 long/short Martingale 方向；
- gate 新 cycle；
- 延迟或缩放 safety order；
- 调整 DCA spacing、TP、partial reduction 和 cycle exit；
- 分配 Martingale sleeves 的预算或 active 状态。

以下不得计入目标：纯趋势、纯 breakout、funding carry、pair-neutral、stat-arb、
buy-and-hold、独立非马丁收益 sleeve。

硬目标不变：

| Tier | Annualized | Max DD | 分段 | 其他 |
|---|---:|---:|---:|---|
| Conservative | >=50% | <=10% | >=4/5 正收益 | `<5000U`、多币种、OOS、live |
| Balanced | >=90% | <=20% | >=4/5 正收益 | `<5000U`、多币种、OOS、live |
| Aggressive | >=110% | <=30% | >=3/5 正收益 | `<5000U`、多币种、OOS、live |

任何 target hit 必须同时满足：

1. event-level shared-budget replay，不接受 LP/equity-curve 拼接；
2. 至少 5 个真实交易 symbols、至少 3 个独立 base assets；
3. 单 symbol configured budget share <=35%；
4. 单 symbol realized gross PnL contribution <=35%；
5. 资金费、fee、slippage、min-notional、quantity precision 全部生效；
6. cold-start 5 segments + nested walk-forward + untouched holdout；
7. 1000/2000/3000/4000/4999 budget ladder；
8. Rust backtest/live semantics + DB-backed reconcile + order trace parity；
9. 参数邻域稳定、成本压力和 symbol holdout 通过；
10. 所有探索写入 registry，不能只记录赢家。

## 2. Round11 后的新事实

执行前必须接受以下事实，禁止回退到旧 handoff：

- R9 `64.4196/18.2111` 是 curve-reuse diagnostic，不是 event-level 候选；
- 补齐 ANKR funding 后，R9 curve diagnostic 为 `62.7845/18.3840`；
- 补齐 ANKR funding 后，R7 event-level 为 `62.3718/28.6873`；
- R7 在 2000U 只有 `5.4232/45.5823`，不能用 5000U 单点代表小资金；
- P1 只有 helper/static-start gate，started executor 绕过 allocator；
- P3 没有原生 minigrid engine/live semantics；
- P4 只否定 partial-TP approximation；
- P5 的 8100 labels 中 7996 为 fixed TP；真实 ATR spacing 未测试；
- P6 历史 ADX/DD-scale 两维 inert，脚本已修复但未重跑；
- P7 没有新 sleeves，且 weight 字段不是实际 fractional weights。

## 3. 外部检索转化为可测试假设

外部资料只用于定义机制，不可当作收益证据：

1. 3Commas condition-based averaging orders：价格最小偏离与技术指标同时满足，
   支持 base-order/last-executed-order 计算模式。Round12 只测试 soft scale/delay，
   不重复 Round10 strict all-or-nothing block。
   https://help.3commas.io/en/articles/9663694-dca-bot-averaging-orders-by-technical-indicators
2. Gainium DCA Minigrids：每个 DCA order range 内存在多个 mini-grid levels；
   这与 Round11 的全仓 partial TP approximation 不同。
   https://gainium.io/help/minigrids-dca
3. Freqtrade position adjustment：DCA/partial exit 必须保留资金，频繁调整会显著
   增加运行成本。Round12 的 native minigrid 必须限制 active levels 和 order churn。
   https://www.freqtrade.io/en/stable/strategy-callbacks/
4. Dynamic Grid Trading 论文：传统静态 grid 在简单假设下接近零期望，动态 reset
   可改善；但 Round8 已测试过窄版 reset，Round12 不重复原参数，只研究
   Martingale cycle 内 ATR/depth adaptation。
   https://arxiv.org/abs/2506.11921
5. Optuna constrained multi-objective：约束目标比把所有 gate 混成一个 score 更合理；
   TPE/GP 在少于约 1000 trials 时更 sample-efficient。
   https://optuna.readthedocs.io/en/stable/tutorial/20_recipes/002_multi_objective.html
6. Backtest overfitting：报告 PBO/selection bias，并使用 Deflated Sharpe Ratio 作为
   辅助而非收益目标。
   https://doi.org/10.2139/ssrn.2568435
   https://doi.org/10.3905/jpm.2014.40.5.094

## 4. 分支、提交和持续记录规则

### P0.1 创建分支

```bash
git fetch origin
git checkout glm-martingale-core-round11
git pull --ff-only
git checkout -b glm-martingale-core-round12
```

### P0.2 创建目录

```text
docs/superpowers/artifacts/glm-martingale-core-round12/
  exploration-registry.jsonl
  run-manifests/
  checkpoints/
  rejected/
  promising/
docs/superpowers/reports/2026-07-12-glm-martingale-round12-search-ledger.md
```

### P0.3 每次探索必须先登记

每一条 JSONL 必须包含：

```json
{
  "experiment_id": "r12-Px-family-seq",
  "parent_experiment_id": null,
  "family": "...",
  "hypothesis": "...",
  "status": "planned|running|complete|failed|skipped_duplicate|promising",
  "effective_config_sha256": "...",
  "engine_semantics_version": "...",
  "code_commit": "...",
  "data_manifest_sha256": "...",
  "budget": 4999,
  "train_windows": [],
  "validation_windows": [],
  "untouched_holdout_opened": false,
  "actual_binary_replays": 0,
  "effective_unique_configs": 0,
  "metrics": null,
  "reject_reasons": [],
  "artifact": "...",
  "non_repeat_scope": null
}
```

规则：

- hash 必须在 defaults、field-level inheritance、env overrides 全部解析后计算；
- 同一 `effective_config_sha256 + engine_semantics_version + data_manifest_sha256 + window`
  已存在时，记录 `skipped_duplicate`，禁止重跑；
- 每 50 trials 或每 15 分钟 flush checkpoint，进程中断后必须 resume；
- `actual_binary_replays` 与 in-memory evaluations 分开；
- family 失败只能写精确参数范围，不能写“架构已穷尽”；
- 每个 phase 完成即 commit + push，不等整个 Round12 结束；
- Git log 必须包含“问题描述”或“复现路径”或“修复思路”。

## 5. Task P1：冻结数据与补齐资金费

### P1.1 不修改历史源库

创建：

- `scripts/glm_r12_build_frozen_data_manifest.py`
- `docs/superpowers/artifacts/glm-martingale-core-round12/run-manifests/r12-data-manifest.json`
- 本地只读副本或新 DB：`data/funding_rates_round12.db`，不覆盖旧文件。

### P1.2 覆盖范围

Development window：

```text
1672531200000 .. 1780271999999
2023-01-01 .. 2026-05-31 23:59:59.999 UTC
```

Untouched holdout：

```text
1780272000000 .. 1783727999999
2026-06-01 .. 2026-07-10 23:59:59.999 UTC
```

该 holdout 在 P2-P7 期间禁止读取 metrics。只有 family、参数和候选全部冻结后，
P8 才能打开一次。

### P1.3 Manifest 必须包含

对所有 candidate symbols：

- market DB/file hash；
- exact range row count/min/max；
- duplicate `(symbol, market_type, timeframe, open_time)` count；
- missing-minute count；
- canonical row-stream SHA256；
- funding count/min/max/canonical SHA256；
- funding gap 分布；
- premium index count/hash；
- fetch source、fetch time、HTTP response hash；
- engine binary SHA256 和 commit。

资金费硬 gate：

- 每个 futures traded symbol 必须至少 1 个 funding row；
- 不得把 missing funding 静默当 0；
- loader 遇到 traded symbol 无 funding coverage 时必须报错，除非 config 显式
  `allow_missing_funding_for_diagnostic=true`，该模式永远不能 promotion；
- ANKRUSDT 使用 Binance official `/fapi/v1/fundingRate` 补齐；
- 对原 30 symbols 也做官方 count/range spot-check，避免只修 ANKR。

### P1.4 测试

新增：

```text
replay_rejects_traded_futures_symbol_with_missing_funding
funding_manifest_detects_gap_duplicate_and_text_mark_price
market_manifest_is_stable_when_rows_after_end_ms_are_appended
canonical_range_hash_changes_when_in_range_row_changes
```

验收：manifest 可重复生成且 SHA256 一致；任何缺失不得进入 P2。

## 6. Task P2：统一 promotion validator 与真实 OOS 协议

创建：

- `scripts/glm_r12_validate_candidate.py`
- `apps/backtest-engine/src/bin/martingale_candidate_validate.rs`，优先 Rust；
- `docs/superpowers/artifacts/glm-martingale-core-round12/r12-validation-schema.json`。

validator 自动派生所有 pass/fail，禁止脚本手填 `live_ready=true`。

### P2.1 Cold-start segments

每个 finalist 独立启动 engine：

```text
h1_2023, h2_2023, 2024, 2025, 2026_ytd
```

不得切 full-period curve 后重归一化冒充 segment replay。

### P2.2 Anchored walk-forward

固定 folds：

```text
F1 train H1-2023                 validate H2-2023
F2 train 2023                    validate 2024
F3 train 2023-2024               validate 2025
F4 train 2023-2025               validate 2026-YTD
```

每 fold 参数只能由该 fold train 选择；validate 只执行一次。记录：

- fold train rank；
- validate ann/DD/return/trades/stops；
- parameter selection frequency；
- PBO proxy、Deflated Sharpe、worst-fold return；
- train/validate degradation。

### P2.3 Neighbor stability

对 target/near-target 的每个连续参数做 `-10%, -5%, +5%, +10%`，离散参数做相邻值。
至少 60% 邻域必须：

- 维持同 tier 的 DD gate；
- annualized 不低于中心值 70%；
- 仍达到最低正收益 segment 数；
- 不发生 principal breach。

### P2.4 Symbol robustness

- leave-one-symbol-out；
- leave-one-side-out diagnostic；
- realized gross PnL share；
- correlation cluster exposure；
- 不允许 ANKR/单个 alt 贡献 >35%。

### P2.5 Cost stress

至少：

```text
base fee/slippage/funding
fee x1.5
slippage x2
fee x1.5 + slippage x2
adverse funding shift +2 bps per funding event
one-bar delayed fill diagnostic
```

target promotion 要求 base 达标，stress 下无 principal breach，DD 不超过 tier limit +5pp，
annualized 仍为正。

## 7. Task P3：Event-Level Dynamic Sleeve Allocator

这是修复 R9 语义，不是继续 curve sweep。

### P3.1 双状态模型

实现两个完全分离的层：

1. **Shadow layer**：每个 sleeve 在虚拟账户中运行，用于 rolling score；
2. **Live layer**：只有 active sleeve 可开新 base cycle，共享真实 `<5000U` 账户。

硬语义：

- shadow position/PnL 永远不能进入 live equity；
- 切换 active sleeve 时不能复制 shadow positions；
- 旧 active sleeve 已存在的 cycle 继续管理 TP/SL/SO，不能 orphan；
- inactive sleeve 不能开新 base cycle；
- 所有决策在 completed boundary 后生效；
- shared budget 对所有 surviving cycles 同时约束；
- state restart 后决策、open cycles、next boundary 完全一致。

建议新文件：

```text
apps/backtest-engine/src/martingale/event_level_allocator.rs
apps/backtest-engine/src/bin/event_level_allocator_replay.rs
apps/trading-engine/src/martingale_allocator_service.rs
apps/trading-engine/tests/martingale_allocator_db_reconcile.rs
```

### P3.2 先写失败测试

必须包含：

```text
inactive_shadow_profit_never_enters_live_equity
switch_does_not_copy_shadow_open_positions
inactive_sleeve_cannot_open_new_base_cycle
existing_inactive_cycle_can_exit_and_manage_safety_orders
shared_budget_counts_cycles_from_all_sleeves
rebalance_uses_observations_at_or_before_boundary_only
started_executor_path_runs_allocator_gate
production_writer_persists_every_shadow_sleeve_observation
restart_restores_allocator_and_open_cycle_state
db_reconcile_switch_matches_backtest_decision_trace
```

### P3.3 R9 benchmark

先只运行 R9 五 sleeves 和单一历史 winner 参数：

```text
lookback=60d, rebalance=7d, calmar-like, no cash
budget=4999
funding=frozen Round12 DB
```

输出：

- curve diagnostic versus event-level delta；
- every rebalance decision；
- live/shadow equity separate；
- open cycles at switch；
- budget rejection attribution；
- five cold-start segments；
- budget ladder。

若 event-level ann <45% 或 DD >30%，记录本 family benchmark failure 后停止扩大
R9 selector 参数，不再浪费时间做 6912 curve-style labels。

若仍有 frontier 价值，只搜索真实有效维度：

```text
lookback: 30, 60, 90, 120
rebalance: 3, 7, 14, 30
score: return-dd, calmar, downside-capture
hysteresis: 0, 2, 5, 8
cash trigger: none, 6, 8, 10
max simultaneous active sleeves: 1, 2
```

删除或重命名 legacy `max_high_ann_weight/min_low_dd_weight`；如需 weights，必须实现
真实 `sleeve_budget_weight` 并在 shared account 中约束，不得用 eligibility 冒充。

## 8. Task P4：原生 DCA Minigrid

Round11 P4 不得复用。先冻结两种明确语义：

### P4.1 Inventory-reducing minigrid

- 只有 safety leg fill 后创建 band；
- favorable bounce 触发 partial reduce；
- 不新增 exposure；
- 以 last safety fill 为 anchor；
- 每 level 只执行一次；
- min notional、step size、fee、slippage 全部生效；
- partial reduction 后重算 remaining quantity 和 weighted entry；
- final TP/SL 与 minigrid priority 明确。

### P4.2 Bounded recycle minigrid

- partial reduce 后允许在同 band 更低/更高价买回/卖回；
- recycled quantity 累计不得超过该 safety leg 原始已成交 quantity；
- 全 cycle exposure 不得超过标准 Martingale ladder 原计划；
- max active bands 和 max churn 均受限；
- 此 variant 单独登记，不能与 inventory-reducing 混为一个 family。

### P4.3 Bar ambiguity

同一 1m bar 同时触及 buyback/reduce/TP/SL 时，使用保守 worst-case order；另做
1s/5s data spot-check。不能默认有利成交顺序。

### P4.4 Backtest/live tests

```text
native_minigrid_emits_engine_event_after_safety_fill
native_minigrid_never_opens_before_safety_fill
partial_reduce_changes_real_position_quantity
recycle_never_exceeds_original_safety_quantity
same_bar_ambiguity_uses_conservative_order
min_notional_and_step_size_are_enforced
live_order_is_reduce_only_with_correct_position_side
duplicate_tick_does_not_duplicate_minigrid_order
partial_fill_restart_is_idempotent
backtest_and_live_minigrid_order_trace_match
```

### P4.5 Search

Bases：corrected R4、R7、P5 best fixed-TP diagnostic 中最多 1 个，仅 multi-symbol。

先用 512 Sobol configs/family：

```text
levels_per_band: 1..5
spacing: 15..150 bps OR 0.25..1.5 ATR
close_fraction: 1/10..1/3
min_profit: fee-cover + 5..80 bps
max_active_bands: 1..3
cycle depth activation: after leg 1..4
max recycle count: 0..3
```

按 train folds 做 constrained Pareto；每 family 最多 40 finalists 进入 full cold-start/WFO。
只有 binding tests 证明每个参数改变 event trace 后，才计入 `effective_unique_configs`。

## 9. Task P5：修复后的 SO V2 与趋势方向门控

### P5.1 补跑 Round11 inert dimensions

使用已修正 `scripts/glm_r11_conditional_so_v2_search.py`，先做 12 个 binding probes：

- ADX 35 与 70 必须产生不同 safety-order event count；
- DD scale 0.5 与 1.0 必须产生不同 quantity/exposure；
- 字段必须位于 portfolio risk limits；
- backtest/live 读取同一 resolved config。

若 probe 不 bind，立即停止，不得再跑 4608。

probe 通过后，重新运行旧 grid 并保存为新的 Round12 artifact；旧 R11 artifact 不覆盖。

### P5.2 新方向：HTF trend-directed Martingale

目标是用指标判断做多/做空，但交易和加仓仍由 Martingale cycle 完成。

只使用少量可解释状态：

```text
EMA slope/price position: 1h 或 4h
ADX: 1h，区分 trend strength
ATR percentile: 1h，决定 spacing/risk state
optional RSI: 15m，只做 overextension/rebound confirmation
```

状态机：

```text
LONG_TREND: 只允许 long Martingale 开新 cycle
SHORT_TREND: 只允许 short Martingale 开新 cycle
NEUTRAL: 允许双向低仓或只运行 mean-reversion sleeve
EXTREME_TREND: 不禁用已有 cycle；只降低后续 SO scale/扩大 spacing
```

禁止：指标本身直接开非 Martingale order；禁止用 full-period 选择阈值。

搜索维度保持少量：

```text
EMA: 50/200, 100/300
slope lookback: 6, 12, 24 HTF bars
ADX trend threshold: 20, 25, 30, 35
ADX extreme threshold: 40, 50, 60
neutral first-order scale: 0.25, 0.5, 0.75
extreme SO scale: 0.25, 0.5, 0.75
rebound confirmation: 0, 20, 40, 70 bps
```

先按 mechanism blocks 做 ablation：direction only、SO scale only、两者组合。每块失败
单独登记，不能只保存组合赢家。

## 10. Task P6：真实 ATR Spacing 与 Cycle-Depth TP

这是 P5 未覆盖的新 Martingale-native family。

### P6.1 ATR spacing

使用 engine 原生 `MartingaleSpacingModel::Atr` 或补齐其 live parity：

```text
next_step_bps = clamp(ATR_pct * k, min_bps, max_bps)
k: 0.5, 0.75, 1.0, 1.25, 1.5
min_bps: 60, 100, 150
max_bps: 250, 400, 600, 900
ATR timeframe: 15m, 1h, 4h
```

触发价只能在 cycle start 或上一个 leg fill 后冻结；不得每根 bar 移动旧订单造成
lookahead-like chasing。

### P6.2 Cycle-depth-aware TP/de-risk

```text
depth 0-1: normal TP 80..180 bps
depth 2-3: fee-cover TP 40..100 bps + optional 25% partial reduce
depth 4+: fee-cover TP 20..70 bps + 25/50% partial reduce + breakeven stop
max cycle age: 24/48/72/120h，仅在 cost-cover 或 bounded loss 退出
```

深 cycle 降 TP 是退出机制，不可改变 Martingale sizing 核心。

测试必须证明：

- depth transition 只由 filled safety legs 驱动；
- TP 不使用未来 ATR；
- partial reduce 后 SO budget 正确；
- live order quantity/reduce-only 与 backtest 一致；
- 2025 loss 和 2024/2025 DD attribution 明确。

搜索先做 1024 constrained Sobol configs，最多 60 finalists 进入严格验证。

## 11. Task P7：旧 LP 成员池的 4999U Event-Level 重建

这是尚未严格测试的新组合方向，不得复用旧 LP 指标作为命中。

来源：

- `docs/superpowers/reports/2026-06-26-margin-v2-lp-portfolios.md`
- `scripts/optimize_margin_v2_lp_portfolios.py`
- conservative/balanced/aggressive 的原 8-symbol members；
- 相关 DB candidate configs。

旧值 `79.36/10`、`108.06/20`、`128.96/30` 只用于建立候选池；它们基于高资金
curve LP，永远不能直接报告。

### P7.1 重建规则

- 从 DB/配置恢复每个 member 的原始 Martingale ladder；
- 统一 frozen funding/market data；
- 只用 event-level joint replay；
- shared budget 固定 4999U；
- first order 必须 >= exchange min notional；
- sizing 只能按明确 scale family 缩放，保留 ladder ratio；
- 不允许 18k-144k planned margin 分母；
- 不允许每个 member 各自拥有 4999U 后再线性相加。

### P7.2 静态组合搜索

先做 member 单体与 2/4/6/8-member ablation，再做组合：

```text
portfolio size: 5..8 symbols
weight floor: 5%
weight cap: 25%
long/short gross balance: 40/60..60/40
first-order scale: 0.1..1.0
max legs: 3..8
global active cycles: 1..4
```

训练目标使用 constrained TPE/GP 或 Sobol，不使用 full-period target score：

- constraints：budget、DD、symbol count、PnL share、no breach；
- objectives：train median return 最大、worst-fold DD 最小、cost turnover 最小；
- 先 1000 trials/profile，Pareto top 50/profile 进入 cold-start/WFO；
- normalized effective-config hash 去重。

### P7.3 Dynamic 组合

只有静态 event-level 组合产生 `ann >=40%` 且 `DD <=25%` 的 robust sleeves 后，才允许
进入 P3 event-level allocator；否则不做动态组合，避免再次 curve 拼接。

## 12. Task P8：高效回测实现

5090 不作为 Round12 关键路径。当前 engine 有大量事件分支、SQLite IO、订单状态和
Decimal 逻辑，直接 CUDA 化成本高且难保证 parity。

优先优化：

1. 一个 Rust 进程预载目标 symbols 的 1m bars/funding；
2. 预计算 deterministic HTF indicators；
3. Rayon 批量并行 configs，避免每 config 重读 110GB DB；
4. normalized config cache + result cache；
5. checkpoint/resume；
6. 先 binding probes，再 Sobol/TPE，再局部邻域；
7. 记录 wall time、CPU time、peak RSS、bars/sec、configs/hour。

创建 benchmark：

```text
r12 baseline: current one-process-per-config
r12 batch: preload + Rayon
target: >=5x configs/hour, metrics bitwise/tolerance identical
```

只有 indicator matrix 或无分支评分明确占主要耗时，且 CPU batch 完成 parity 后，才可
做 optional CUDA prototype；GPU 结果必须与 CPU event trace 精确一致，否则停止。

## 13. Task P9：Untouched Holdout 与最终三档验证

P3-P7 完成、family/params 冻结并 commit 后，才能打开
`2026-06-01..2026-07-10` holdout。

每 profile 最多 3 个 frozen candidates 进入 holdout，禁止看到结果后调参。

### P9.1 Budget ladder

```text
1000, 2000: diagnostic runnable gate，无 principal breach/invalid order
3000, 4000, 4999: promotion stability gate
```

目标 metrics 以声明的 launch budget（必须 <=4999）计算。若只在 4999 达标：

- 4000U annualized 至少为 4999U 的 70%；
- 4000U DD 不得超过 tier limit +5pp；
- 3000U 不得 principal breach，annualized 必须为正；
- 报告必须明确“不适用于 2000U”，不能笼统写“小资金”。

### P9.2 Target pass contract

每个 target candidate 输出：

```text
full development replay
5 cold-start segments
4 anchored WFO folds
untouched holdout
budget ladder
neighbor grid
leave-one-symbol-out
cost/funding/latency stress
per-symbol budget/PnL attribution
actual binary replay count
effective unique config count
engine/data/config hashes
backtest/live order trace diff
```

Untouched holdout 时间较短，不单独年化作为主要 pass；要求：

- return >0；
- no principal breach；
- DD <= tier limit；
- order/rejection behavior 无异常；
- 不出现单 symbol PnL dominance >50%。

## 14. Task P10：Production DB/Executor Parity

任何 research near-target 在写入 `promising/` 前，必须补齐：

```text
production DB-backed reconcile
live_executor_started path allocator gate
shadow observation writer
restart/state persistence
partial fill/idempotency
reduce-only/positionSide
exchange minNotional/tickSize/stepSize
fee/funding settlement
backtest versus live order trace
```

至少新增测试：

```text
r12_db_reconcile_rebalances_started_executor
r12_db_reconcile_persists_shadow_observations
r12_db_reconcile_blocks_inactive_new_cycle_after_restart
r12_existing_inactive_cycle_remains_managed
r12_native_minigrid_live_trace_matches_backtest
r12_budget_rejection_trace_matches_backtest
```

测试必须创建 ephemeral DB record 并调用真实 reconcile entry point，禁止手工构造预期 JSON
冒充 persistence test。

## 15. Phase 顺序与停止条件

严格顺序：

1. P0 registry/branch；
2. P1 frozen data + ANKR/all-symbol funding gate；
3. P2 validator/OOS harness；
4. P3 R9 event-level benchmark；
5. P4/P5/P6/P7 可并行研究，但各自独立 registry；
6. P8 batch acceleration；
7. P9 frozen finalists only；
8. P10 production parity；
9. final handoff。

立即停止某 family 并记录失败，如果：

- 参数 binding probe 不改变 event trace；
- 数据缺失或 manifest 不一致；
- 使用 curve slicing/LP recombination 作为 promotion metrics；
- 单 symbol 或 PnL share >35%；
- target 只在 full period 命中而 WFO/cold segments 失败；
- target 在 cost stress 下 principal breach；
- live semantics 缺失；
- effective config hash 已存在；
- holdout 已被提前读取。

发现 target/near-target 时不要继续扩大参数搜索，先执行 P9/P10；验证失败后记录原因，
再决定是否回到下一 family。

## 16. 最终产物

必须提交：

```text
docs/superpowers/artifacts/glm-martingale-core-round12/r12-final-validation.json
docs/superpowers/artifacts/glm-martingale-core-round12/r1-r12-corrected-status.json
docs/superpowers/artifacts/glm-martingale-core-round12/exploration-registry.jsonl
docs/superpowers/artifacts/glm-martingale-core-round12/run-manifests/r12-data-manifest.json
docs/superpowers/reports/2026-07-12-glm-martingale-round12-search-ledger.md
docs/superpowers/reports/2026-07-12-glm-round12-handoff-to-chatgpt.md
```

Final JSON 必须分别回答：

- conservative/balanced/aggressive 是否通过；
- 哪个 launch budget 通过；
- 1000/2000/3000/4000/4999 各自结果；
- 是否 event-level；
- 是否 multi-symbol/PnL 分散；
- 是否通过 WFO/holdout/neighbor/stress；
- 是否 fully live-ready；
- 实际 binary replay 与 effective unique config 数；
- 所有新 non-repeat keys 的精确范围；
- 最佳 config、decision trace、order trace、data manifest 的路径。

若仍无命中，必须给出“哪些机制被严格否定、哪些仍未完成”，禁止写“全部可能性已穷尽”。

## 17. 验证与推送

每个 phase 至少运行：

```bash
jq empty docs/superpowers/artifacts/glm-martingale-core-round12/*.json
cargo test -p backtest-engine
cargo test -p trading-engine
git diff --check
git status --short
```

最终：

```bash
git add apps crates scripts docs/superpowers
git commit -m "docs: 问题描述 Round12马丁event-level与严格OOS交接"
git push -u origin glm-martingale-core-round12
```

提交前确认工作区只包含 Round12/必要修正文件，不提交 `/tmp` 数据、临时 DB、下载 JSON、
target binaries 或缓存。
