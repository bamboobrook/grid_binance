# GLM Martingale Core Round 13：原生趋势方向、资金调度与真实组合计划

执行者：GLM

前置审计：`docs/superpowers/reports/2026-07-12-glm-round12-execution-audit-and-fix.md`

权威状态：`docs/superpowers/artifacts/glm-martingale-core-round12/r1-r12-corrected-status.json`

## 1. 唯一目标与硬边界

Round13 必须继续以 Martingale/DCA/grid cycle 为交易核心。指标、regime、allocator 只能：

- 决定 long/short Martingale 是否允许开新 cycle；
- 缩放 first order 或后续 safety orders；
- 冻结每一腿的 DCA spacing；
- 选择 active Martingale sleeves/symbols；
- 管理已有 Martingale inventory 的 TP、partial reduce、bounded recycle 和退出。

禁止计入收益：独立趋势仓、纯 breakout、funding carry、pair-neutral、buy-and-hold、
stat-arb 或任何非 Martingale sleeve。

| Tier | Annualized | Max DD | 正收益 cold segments | Launch budget |
|---|---:|---:|---:|---:|
| Conservative | >=50% | <=10% | >=4/5 | 3000/4000/4999 中明确一个 |
| Balanced | >=90% | <=20% | >=4/5 | 3000/4000/4999 中明确一个 |
| Aggressive | >=110% | <=30% | >=3/5 | 3000/4000/4999 中明确一个 |

所有候选还必须满足：

1. budget 严格 `<5000U`；1000/2000U 必须可执行且无 principal breach，但可标明不适用；
2. event-level shared account，不接受 curve sum、curve rotation 或 LP 指标；
3. 至少 5 个 traded symbols、至少 3 个独立 base assets；
4. 单 symbol configured capital <=35%，realized gross PnL contribution <=35%；
5. funding、fee、slippage、minNotional、tickSize、stepSize 全部实际生效；
6. nested WFO、cold-start、邻域、LOSO、成本压力、候选未见 holdout 全部通过；
7. backtest/live order trace、started executor、DB reconcile、restart 全部通过；
8. 每个失败和 skipped duplicate 都进入 registry，禁止只保存赢家。

## 2. Round12 后禁止再犯的错误

以下不是意见，而是执行 gate：

- `r12-r9-static-36-strategy-merge` 只否定 36 strategies 同时 active；不得写 dynamic allocator failed。
- `r12-partial-tp-stage-grid-576` 不是 cycle-depth TP；不得改标签后重跑。
- LP conservative symbols + 统一 R4 ladder 只是一份替代配置；不得写所有 LP family failed。
- 固定参数在 validation segment 重放不叫 anchored WFO。
- 没有 fee/slippage override 时不得重复 base replay冒充 stress。
- `fully_live_ready` 必须 fail-closed；已有 suite 通过不能替代本机制 DB-backed tests。
- manifest 必须包含 candidate 的每一个 traded/dependency symbol。
- holdout 数据必须先过完整 K 线、funding edge/internal-gap gate。

精确 non-repeat scopes 见第 14 节。

## 3. 外部检索转化出的可测试假设

外部资料只用于定义机制，不是收益证明：

1. 3Commas averaging orders 要求价格偏离和技术条件同时满足。Round13 测试 completed-HTF
   condition 下的 soft scale/delay，不重复 Round10 strict all-or-nothing block。
   `https://help.3commas.io/en/articles/9663694-dca-bot-averaging-orders-by-technical-indicators`
2. Gainium minigrids 在 DCA 区间内做局部 grid。只实现 inventory-reducing 和 bounded
   recycle，不允许借 minigrid 超过原 Martingale ladder exposure。
   `https://gainium.io/help/minigrids-dca`
3. Freqtrade position adjustment 文档明确频繁调整会取消/替换订单并要求预留 DCA stake。
   因此必须限制 churn、保证幂等并显式保留 existing-cycle safety budget。
   `https://www.freqtrade.io/en/stable/strategy-callbacks/`
4. Dynamic Grid Trading 论文指出简单静态 grid 在其假设下接近零期望，并提出动态 reset。
   Round13 只把 reset 用于 Martingale cycle 内的 anchor/spacing，不外推其 BTC/ETH
   2021-2024 回测收益。
   `https://arxiv.org/abs/2506.11921`
5. Moreira/Muir 的 volatility-managed portfolios 支持“高波动降风险”作为风险假设；只用于
   first/SO risk scale，不新增独立收益 sleeve。
   `https://www.nber.org/papers/w22208`
6. Optuna 官方建议把硬条件作为 constraints；少于 1000 trials 优先 constrained TPE/GP。
   `https://optuna.readthedocs.io/en/stable/tutorial/20_recipes/002_multi_objective.html`
7. PBO/Deflated Sharpe 只做 multiple-testing/selection-bias 辅助 gate，不替代收益和 DD。
   `https://doi.org/10.2139/ssrn.2326253`
   `https://doi.org/10.3905/jpm.2014.40.5.094`
8. 数据只能来自 Binance 官方 USD-M kline/funding endpoints；保存原响应 stream SHA256。

## 4. Phase P0：分支、台账与不可变输入

### P0.1 分支

```bash
git fetch origin
git checkout glm-martingale-core-round12
git pull --ff-only
git checkout -b glm-martingale-core-round13
```

### P0.2 目录

```text
docs/superpowers/artifacts/glm-martingale-core-round13/
  exploration-registry.jsonl
  run-manifests/
  checkpoints/
  rejected/
  promising/
  traces/
docs/superpowers/reports/2026-07-12-glm-martingale-round13-search-ledger.md
```

### P0.3 启动 gate

必须先验证：

```bash
jq -e '.development_window.gate.passed and .holdout_window.gate.passed' \
  docs/superpowers/artifacts/glm-martingale-core-round12/run-manifests/r12-data-manifest.json
cargo test -p backtest-engine --test r12_funding_manifest
```

复制而不是重写 Round12 non-repeat registry。每条 Round13 记录必须含 normalized resolved
config hash、engine hash、data canonical hash、窗口、实际 binary replays、cache hits、wall time、
peak RSS、状态和精确 reject scope。先登记 `running`，完成后追加终态；每 50 trials checkpoint。

## 5. Phase P1：批量 CPU 回测与去重基础设施

5090 暂不进入事件引擎。先完成 Round12 未实现的 CPU batch：

建议文件：

```text
apps/backtest-engine/src/martingale/batch_replay.rs
apps/backtest-engine/src/bin/martingale_batch_search.rs
apps/backtest-engine/tests/martingale_batch_parity.rs
```

要求：

1. 一个进程只读取一次所需 1m bars/funding；
2. bars/funding 使用只读 `Arc`，每个 config 拥有独立 runtime state；
3. Rayon 并行 configs，线程数可配；禁止共享可变 indicator/cycle state；
4. normalized effective-config cache；相同 hash/window/data/engine 直接 `skipped_duplicate`；
5. checkpoint/resume 后 trial 顺序和结果不变；
6. 20 个历史 configs 对比 CLI subprocess：trade count、events、PnL、DD 容差 `1e-9`；
7. benchmark 至少 100 configs，吞吐 >= subprocess 基线 5x，否则 profiler 后继续修；
8. GPU 仅在 profiler 证明无分支 indicator matrix 占比 >60% 时做 optional prototype，且 CPU
   event trace 必须完全一致。

P1 未通过 parity 前，后续只能做 <=32 个 binding probes，禁止启动大搜索。

## 6. Phase P2：修复后的 HTF 趋势方向 Martingale（最高优先级）

这是 Round12 完全未执行、且最直接针对 2025/holdout 亏损的方向。

### P2.1 先完成语义与 live parity

新增 completed-boundary HTF state：

```text
LONG_TREND  -> 只允许 long Martingale 开新 cycle
SHORT_TREND -> 只允许 short Martingale 开新 cycle
NEUTRAL     -> 双向 first-order scale 0.25/0.5 或暂停
EXTREME     -> 不 orphan 旧 cycle；后续 SO scale 0.25/0.5/0.75，并扩大新腿 spacing
```

输入仅允许：per-symbol 1h/4h EMA position+slope、1h ADX、1h ATR percentile；RSI 15m
只能做 optional overextension confirmation。BTC state 只能做组合背景，不能替代 per-symbol state。

必须新增测试：

```text
htf_state_uses_only_completed_bars
same_incomplete_htf_bar_cannot_change_state
long_state_blocks_short_new_cycle_and_inverse
state_change_never_closes_or_copies_existing_cycle
extreme_state_scales_next_safety_quantity
drawdown_safety_order_scale_changes_event_quantity
adx_threshold_extremes_change_deterministic_event_trace
backtest_live_htf_state_trace_matches
restart_restores_htf_state_and_pending_cycle
```

审计已修 backtest `safety_order_scale`，GLM 必须补 trading-engine 同源 resolved config 和 trace。
ADX binding 用确定性 synthetic bars + 阈值 0/100，不能再用“35/70 历史结果相同”判断 inert。

### P2.2 Ablation

Bases：R4 corrected、R7 corrected；每个 base 独立记录：

1. direction gate only；
2. extreme SO scale only；
3. ATR risk scale only；
4. direction + SO；
5. direction + SO + ATR。

搜索范围保持可解释：

```text
HTF: 1h, 4h
EMA pair: 50/200, 100/300
slope lookback: 6, 12, 24 completed HTF bars
ADX trend: 20, 25, 30, 35
ADX extreme: 40, 50, 60
neutral FO scale: 0.25, 0.5
extreme SO scale: 0.25, 0.5, 0.75
ATR target percentile: 40, 60, 80
state hysteresis: 1, 2, 3 HTF bars
```

每个 ablation 最多 512 constrained TPE/Sobol trials；先在每 fold train 内搜索，最多 30 个
Pareto configs 进入 validation。禁止 full-period 排名选参。

继续条件：至少一个 ablation 在 4 个 validation folds 中达到 median ann >=35%、worst DD
<=30%、至少 3 folds 正收益。否则精确关闭该 ablation，不得关闭所有 trend-directed Martingale。

## 7. Phase P3：Event-Level Dynamic Allocator 与资金调度

### P3.1 必须实现真正双状态

```text
Shadow: 每个 sleeve 虚拟运行，仅产生 lagged score/observation
Live: active sleeve 才能开新 base cycle；所有存量 cycle 共享真实账户
```

测试必须覆盖 Round12 缺失的 10 项：

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

### P3.2 先跑唯一 R9 benchmark

使用原 5 sleeves，`lookback=60d/rebalance=7d/calmar-like/max_active=1/budget=4999`。
输出每次切换、live/shadow equity、存量 cycles、budget rejection 和 order trace。

- 若 ann <45% 或 DD >30%，只关闭这组 R9 dynamic baseline，不搜索 selector grid；
- 若通过，最多 384 configs 搜索 lookback 30/60/90/120、rebalance 3/7/14/30、
  hysteresis 0/2/5、max active 1/2、cash trigger none/8/10。

### P3.3 资金感知 active-cycle scheduler

不依赖 R9 curve score，直接在每个 completed HTF boundary 对可开 Martingale cycles 排队：

- max live cycles 2/3/4；
- 每 correlation cluster 最多 1/2 个；correlation 仅用过去 60/120 天 completed returns；
- existing cycles 优先保留下一腿 safety reserve；
- 新 cycle 按 per-symbol trend alignment、ATR risk、近期已实现 Martingale outcome 排序；
- configured symbol cap 25/30/35%；
- 禁止把 shadow position 或未来 safety fill 计入 live PnL。

这一路径重点解释 R4 `max_capital_used=707.6U` 但低预算失败的问题。先做 128 configs
mechanism screen；只有 `ann>=40%、DD<=25%、4/5 positive` 才扩到 512。

## 8. Phase P4：原生 Inventory-Reducing DCA Minigrid

先只实现 inventory-reducing variant：

- safety leg 成交后才创建 band；anchor 固定为该 safety fill；
- favorable bounce 只做 reduce-only partial close，不新增 exposure；
- 每 level 每 cycle 一次；partial fill/restart 幂等；
- total reduce 不得超过对应 safety-leg quantity；
- same-bar reduce/TP/SL 使用保守顺序；
- fee、slippage、minNotional、stepSize 生效；
- backtest/live order trace 一致。

必需测试沿用 Round12 P4 的 10 个 test names。通过后运行 512 Sobol configs：

```text
levels 1..5; spacing 15..150bps; close fraction 0.10..0.33;
min profit fee-cover+5..80bps; active bands 1..3; activate after safety leg 1..4
```

只有 inventory-reducing 达到 `ann>=40%、DD<=25%` 才实现 bounded recycle；recycle 总量不得
超过原 safety quantity，max recycle 1..3，cycle exposure 不得超过原 ladder。

## 9. Phase P5：真实 ATR Spacing 与 Safety-Depth TP

不能复用 Round12 的 partial-stage 实现。

### P5.1 Engine 状态

- `filled_safety_leg_count` 是唯一 depth source；
- depth 0-1、2-3、4+ 使用不同 TP/de-risk rule；
- 每次 cycle start 或 safety fill 时冻结下一腿 ATR spacing，旧 trigger 不随 bar 漂移；
- max-age 只能在 fee-cover 或显式 bounded-loss 下退出，不得无条件按 close 强平；
- partial reduce 后重算真实 quantity、margin、SO reserve；
- live reduce-only/positionSide 与 backtest 一致。

必需 synthetic tests：depth transition、no-future ATR、same-bar conservative order、SO budget、
partial quantity、restart、live trace。

### P5.2 搜索

```text
ATR HTF: 15m, 1h, 4h
k: 0.5, 0.75, 1.0, 1.25, 1.5
min/max spacing: 60/250, 100/400, 150/600, 150/900 bps
depth0-1 TP: 80, 120, 180 bps
depth2-3 TP: 40, 70, 100 bps; reduce 0/25%
depth4+ TP: 20, 40, 70 bps; reduce 25/50%
max age: disabled, 48, 72, 120h
```

先 768 constrained trials/base；最多 40 Pareto configs/base 进入 WFO。Round12 的 576 labels
不得算入该 family trial count。

## 10. Phase P6：原 LP Member Config 恢复 Gate

当前 `data/martingale_portfolios.db` 为 0 bytes，Round12 没有恢复原 configs。先按 candidate IDs
搜索 git history、任务 DB、artifact 和远端分支：

```text
btc_1782397542840937284
btc_1782366734837215913
btc_1782397541205906740
...报告中的全部 24 个 member IDs
```

必须恢复每个 member 的完整 direction、spacing、sizing、TP/SL、trigger 和 risk config，并核对
source task hash。找不到就记录 `skipped_missing_source`，禁止再次用统一 R4 ladder替代。

恢复后：

1. 扩展 frozen manifest 到全部 LP symbols；
2. member 单体 replay；
3. 2/4/6/8-member ablation；
4. 5-8 symbols、cap25%、FO scale0.1..1.0、max legs3..8、live cycles1..4；
5. conservative/balanced/aggressive 各 600 constrained trials；
6. 只有真实 event-level `ann>=40%、DD<=25%` sleeves 才能进入 P3 allocator。

## 11. Phase P7：严格选择、抗过拟合和组合 Gate

### P7.1 真 anchored nested WFO

固定 folds，但每 fold 必须在 train 内独立搜索/选参，validation 只运行一次：

```text
F1 train H1-2023       validate H2-2023
F2 train 2023          validate 2024
F3 train 2023-2024     validate 2025
F4 train 2023-2025     validate 2026-YTD
```

train/validate 边界加 7 天 purge，validation cold-start，不继承 train position。记录 train rank、
selection frequency、validation degradation、worst fold、PBO proxy、Deflated Sharpe、trial count。

### P7.2 邻域与 symbol robustness

- 连续参数 +/-5%、+/-10%，离散参数相邻值；>=60% 邻域保持 tier DD、ann>=中心70%；
- leave-one-symbol-out、leave-one-side-out；
- realized gross/net PnL、capital、DD contribution；
- 任一 symbol gross PnL >35% 或 configured capital >35% 直接拒绝；
- rolling correlation cluster 不得由未来窗口计算。

### P7.3 成本/延迟压力

先给 replay binary 加真实 override，再运行：

```text
fee x1.5
slippage x2
fee x1.5 + slippage x2
funding adverse +2bps/event
one-bar delayed fill
minNotional/tick/step boundary
```

stress 必须改变 event/metric hash；相同结果先判 binding failure。要求无 principal breach、ann>0、
DD <= tier limit+5pp。

## 12. Phase P8：候选未见 Holdout 与未来 paper

`2026-06-01..2026-07-10` 已对 R4 打开，不能再称全局 untouched。对 Round13 新 family 可作为
candidate-unseen locked holdout，但必须：

1. 每 profile 最多 3 个 config；
2. config hash、engine hash、参数和选择理由先 commit；
3. validator 只有显式 `--open-holdout <frozen-commit>` 才能读取；
4. 看到结果后不得在同 family 调参再重测；
5. return>0、无 breach、DD<=tier、无 symbol dominance>50%。

任何“可实盘”结论还要增加 2026-07-11 之后至少 30 天 forward paper observation；这不阻止
报告 backtest target hit，但没有 forward paper 不能标 production-ready。

## 13. Phase P9：Production DB/Executor Parity

每个 target/near-target 必须新增真实 ephemeral DB tests：

```text
r13_started_executor_applies_htf_direction_gate
r13_db_reconcile_persists_shadow_observations
r13_restart_restores_allocator_and_open_cycles
r13_existing_inactive_cycle_remains_managed
r13_drawdown_so_scale_matches_backtest
r13_native_minigrid_trace_matches_backtest
r13_depth_tp_reduce_only_matches_backtest
r13_budget_rejection_trace_matches_backtest
r13_partial_fill_restart_is_idempotent
r13_exchange_filters_match_replay
```

测试必须调用 production reconcile/executor entry point，禁止只构造 helper JSON。

## 14. 精确 Non-Repeat Registry

Round13 启动时至少导入：

```text
r12-r9-static-36-strategy-merge:
  exact five source configs, all 36 strategies simultaneously active, shared 1000..4999U
r12-partial-tp-stage-grid-576:
  fixed spacing {120,150,180,220}, three ordinary TP stages, age {none,48,120,240}
r12-lp-conservative-symbols-r4like-one-config:
  LTC/DYDX/INJ/FIL/ICP/XRP/UNI/BTC + FOQ15 + 2.8x + 8 legs + copied R4-like TP
r12-r4-corrected-baseline-and-holdout:
  exact R4 config under corrected manifest/binary; do not rerun unless engine/data hash changes
r10-strict-condition-so:
  hard all-or-nothing condition family
r11-fixed-tp-dominated-grid:
  exact historical fixed-TP ranges
```

不得导入以下过宽 scope：`all dynamic allocators`、`all cycle-depth TP`、`all LP portfolios`、
`all ATR spacing`、`all minigrids`。

## 15. 执行顺序与停止规则

严格顺序：

1. P0 data/registry；
2. P1 batch parity；
3. P2 HTF trend-directed；
4. P3 event allocator + capital scheduler；
5. P4/P5 原生 minigrid 与 depth/ATR；
6. P6 仅在原 member configs 恢复后执行；
7. P7 strict validation；
8. P8 locked holdout；
9. P9 production parity；
10. final handoff。

立即停止当前 config/family 并登记，如果：

- 参数不改变 resolved config 或 deterministic event trace；
- 数据/资金费/依赖 symbol 不在 manifest；
- 使用 full period 选参；
- shared account、min notional、funding 或 costs 未生效；
- symbol/PnL concentration 超限；
- WFO、邻域或 stress 失败；
- live semantics 缺失；
- hash 已存在；
- 读取 holdout 前未冻结 config。

发现 near-target（ann 距目标 <=15pp 且 DD 已过 gate）后停止扩大搜索，先完成 P7-P9；验证
失败后记录原因，再决定是否恢复 family。

## 16. 最终交付物

```text
docs/superpowers/artifacts/glm-martingale-core-round13/r13-final-validation.json
docs/superpowers/artifacts/glm-martingale-core-round13/exploration-registry.jsonl
docs/superpowers/artifacts/glm-martingale-core-round13/run-manifests/r13-data-manifest.json
docs/superpowers/reports/2026-07-12-glm-martingale-round13-search-ledger.md
docs/superpowers/reports/2026-07-12-glm-round13-handoff-to-chatgpt.md
```

Final JSON 必须逐档回答：target 是否命中、launch budget、五档预算、symbols、PnL concentration、
WFO/neighbor/LOSO/stress/holdout、production tests、actual replay count、unique hash count、全部
失败 scopes。没有命中时只写精确未通过项，禁止写“所有可能性已穷尽”。

## 17. 测试、提交与推送

每 phase：

```bash
python3 -m py_compile scripts/glm_r13_*.py
jq empty docs/superpowers/artifacts/glm-martingale-core-round13/**/*.json
cargo test -p backtest-engine
cargo test -p trading-engine
git diff --check
git status --short
```

每 phase 独立 commit + push；Git log 必须包含“问题描述”“复现路径”或“修复思路”。示例：

```bash
git commit -m "fix: 修复思路 Round13完成HTF方向马丁backtest-live语义"
git push -u origin glm-martingale-core-round13
```
