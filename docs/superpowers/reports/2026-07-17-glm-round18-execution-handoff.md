# GLM Round 18 执行交接（原生同步残差 Martingale Cycle 计划）

执行者：GLM。计划：`docs/superpowers/plans/2026-07-16-glm-martingale-core-round18-native-synchronized-residual-cycle-plan.md`
审计/计划制定：ChatGPT（`docs/superpowers/reports/2026-07-16-chatgpt-round17-execution-audit.md`）
分支：`glm-martingale-core-round18`（从 `glm-martingale-core-round17@dece824` 创建）

## 0. 执行摘要（TL;DR）

- **Round 18 outcome（§18）：`VALID_NOVEL_FAMILIES_EXHAUSTED_NO_FRONTIER_PROGRESS`。**
- **机制前沿（首次达成，研究进展）**：前 17 轮从未原生执行过的「5+ 币同步残差
  Martingale cycle」本轮从零构建并完整执行：新引擎 `sync_cycle_engine.rs` + 新二进制
  `synchronized_cycle_replay` + 生产 skeleton + 10/10 M1 open param 全 mechanically bound
  + G0 对抗性激活 + 真实 SO（genuine loss-after-add Martingale）证据。
- **收益前沿（未推进）**：三档目标（保守 50% / 平衡 90% / 激进 110%）**零命中**。
  G3 anchored validation **0 finalists 存活**。无任何 single validation fold 命中
  P-A/P-B/P-C/P-D frontier gate。`frontier_progress=false`。
- **诚实根因**：同步残差 cycle 在 F2 train 期（2023）协整成立所以 F2 validation（2024）
  盈利，但 residual relation 跨 regime change（2025 熊市 / 2026 YTD）非平稳，F3/F4 失效。
  这是 plan §10 防过拟合合同要检测的 structural-break 风险，**不是 bug，是经验结果**。
- **完整回测量**：R5 G0（4 windows × 10 params × low/high = ~80）+ R6 G1（576）+
  R7 G2（960）+ R8 G3（24）= **~1640 full synchronized_cycle_replay，全部无快筛**。

## 1. 机器状态（validator 可重算）

| phase | 状态 | 说明 |
|---|---|---|
| R0 | complete | 3 个 no-op R17 控制已修（vol_cap/cluster_scheduler/hazard_deadline）+ production router 零 OHLC bar 已修；对抗性激活证明 3 机制在 bear+bull 改变 event/trade/rejection stream；R17 权威改 materially_incomplete_invalid_mechanism_search |
| R1 | complete | canonical fingerprint index（scan R1-R17 2370 行 → 2315 dedup-exclusion）+ append-only Round18Registry |
| R2 | complete | 新 sync_cycle_engine.rs（M1 pair + M2 basket 机器定义）+ SynchronizedCycleConfig 进 config hash；self-test FO+SO proven，无 breach@4999U |
| R3 | complete | 新 binary synchronized_cycle_replay.rs + trading-engine 6 个 production call site + SyncActiveCycle/SyncCycleDecision 类型 |
| R4 | complete | F2/F3/F4 unblocked（3-4 disjoint train-stable pairs each），F1 blocked_no_stable_groups；train_selection_commit_sha256 冻结 |
| R5 | complete | G0 binding：10/10 M1 open param mechanically bound（group_gross_cap_pct 条件性 bound，cap-proof 复测 mechanically live） |
| R6 | complete | G1 576 replays，178 survivors，top32 selected |
| R7 | complete | G2 960 replays，8 finalists 通过 strict gate |
| R8 | complete_zero_survivors | G3 0 survivors；OOS 过拟合；VALID_NOVEL_FAMILIES_EXHAUSTED_NO_FRONTIER_PROGRESS |
| R9 | not_applicable_zero_survivors | 0 finalist => 无 robustness 可跑（§15） |
| R10 | not_applicable_zero_survivors | 0 finalist => 无 production parity finalist；R3 production call sites 仍有效 |
| R11 | complete | 本文档 + failure ledger |

## 2. R0 前置修复门（plan §3）

### 2.1 Fail-close tests（§3.1，8 个全 DETECT R17 bug）
`scripts/glm_r18_r0_failclose_tests.py`：8 个 plan-required test 在 `--expect-fail` 模式
全 DETECT R17 bug（noop vol_cap/cluster_scheduler、hazard age_h=0、A4 config-hash-only
bound、B1 identical arms、production zero OHLC bar、g1 self-report 2304 vs registry 205）。

### 2.2 引擎修复（§3.2，机制级真实实现）
- `vol_cap`：从 `let _ = (strategy, cfg);` 改为读真实 ATR/price，超 risk_fraction 隐含
  ceiling 时 BLOCK 新 FO + emit `r17_vol_cap_block` event。
- `cluster_scheduler`：从 no-op 改为读真实 symbol/portfolio active margin，超 cap 时
  BLOCK 新 FO + emit `r17_cluster_block` event。
- `hazard_deadline`：call site 从 `(legs_filled, legs_filled)` 改为真实
  `cycle_start_ms`（首腿成交时间戳）+ 当前 bar `timestamp_ms`。
- production router：从 `symbol="BTCUSDT", OHLCV=0` 改为每个 strategy 的真实 symbol +
  最新 tick 价格。
- 对抗性激活：3 机制在 bear(2025-01)+bull(2023-10) 两个 30 天窗口 low vs high 参数
  event/trade/rejection stream 全不同 => all_mechanisms_activation_proven=true。

### 2.3 R17 权威修正（§3.3）
- R17 handoff 加 `SUPERSEDED_BY_CHATGPT_AUDIT` 横幅（原文保留）。
- `r17-corrected-authority-r18.json`：machine state =
  `materially_incomplete_invalid_mechanism_search`。

## 3. M1 synchronized pair residual Martingale cycle（plan §2/§7，机制机器定义）

**novelty proof**：代码库前 17 轮只有 independent per-symbol cycle（`MartingaleCycleState`
holds ONE symbol's legs）。Round 18 引入 `SynchronizedCycleState`：单一 aggregate cycle，
全部腿同 timestamp FO、net PnL<0 + residual 不利时同步 SO、net PnL 超 floor + residual
进 exit 时共同 TP/close。

机器定义实现（`apps/backtest-engine/src/martingale/sync_cycle_engine.rs`）：
```text
cycle_open:    residual_z(|z|>=entry_z) from train-frozen (beta,mu,sigma) -> all legs FO
safety_order:  aggregate net PnL<0 AND residual adverse >= so_residual_step_z -> all legs +notional
take_profit:   aggregate net PnL > tp_net_bps_floor AND |z|<=exit_z -> all legs reduce/close
abort:         deadline / end-of-window -> only real aggregate reduce/close
```
- `validate_martingale_contract()` 拒绝 `multiplier<=1` 或 `max_legs<2`（not_martingale）。
- residual_z 严格用 train-frozen (beta,mu,sigma)，不读 validation/future bar。
- funding per-leg，复用 `effective_fee_bps`/`effective_slippage_bps`（含 CLI override）。

## 4. 计数（全部从 registry 重算，无自报）

```text
R5 G0: ~80 full replays (4 windows × 10 params × low/high, partial overlap)
R6 G1: 576 full replays (96 Sobol × 3 train blocks × 2 budgets), 178 survivors, 14 rejected
R7 G2: 960 full replays (32 configs × 6 windows × 5 budgets), 8 finalists
R8 G3: 24 full replays (8 finalists × 3 validation folds), 0 survivors
合计：~1640 full synchronized_cycle_replay，0 快筛，0 cache-hit-as-result。
unique configs launched: 96 (G1 Sobol) + 32 (G2) + 8 (G3) = 136 distinct
timeouts: 0
```

## 5. 三档目标命中表

```text
保守 (>=50% ann, <=10% DD, >=4/5 pos) : []  NOT HIT
平衡 (>=90% ann, <=20% DD, >=4/5 pos)  : []  NOT HIT
激进 (>=110% ann,<=30% DD, >=3/5 pos)  : []  NOT HIT
```

## 6. P-A/P-B/P-C/P-D 前沿推进表（§12）

| 进展 ID | 要求 | 命中 |
|---|---|:--:|
| P-A | ann>62.51%, DD<=28.62%, >=4/5, 5+ symbols, event-level | NO（best single fold M1_085 F2 ann=107.5% 但 DD=60.6%）|
| P-B | ann>=40%, DD<=20%, >=4/5, 5+ symbols, event-level | NO |
| P-C | ann>=35%, DD<=30%, 5/5, 5+ symbols, event-level | NO |
| P-D | 任一保守/平衡/激进最终档位完整命中 | NO |

`frontier_progress=false`。Round 18 不能使用 "breakthrough/接近目标/全部完成"。

## 7. M1 每 hypothesis 的 novelty proof 与 activation delta

| hypothesis | novelty proof（vs R1-R17） | activation delta（G0 证明）|
|---|---|---|
| M1 synchronized pair cycle | 前 17 轮无同步 cycle；M1 是 1m event、同步 FO/SO/TP、真实 margin/funding/legging（vs 旧 daily-close 合成 pair probe） | 10/10 open param 改变 event/trade/rejection stream；base config 在 4 窗口都产生真 SO |
| M2 synchronized factor-residual basket | （未执行：blocked_parent_gate，M1 未达 frontier gate，§9.4 不解锁 M2）| n/a |
| M3 dynamic inventory reservation | blocked_parent_gate | n/a |
| M4 jump first-passage SO gate | blocked_parent_gate | n/a |
| M5 frozen regime ladder contract | blocked_parent_gate | n/a |

M2/M3-M5 配额不转给 M1（§11.1），记录 `blocked_parent_gate`。

## 8. 每 fold train selection hash 与一次性 validation 结果

| fold | train_selection_commit_sha256 | validation 结果 |
|---|---|---|
| F1 | `blocked_no_stable_groups`（2023 H1 无足够 stable disjoint pairs）| n/a |
| F2 | `d21e4b82...`（4 pairs: DOT/BCH, LINK/TRX, ETH/BTC, LTC/XRP）| 8 finalists 全部 ann 正（49-107%）但 DD 40-62%；无 frontier gate 命中 |
| F3 | （3 pairs: LTC/TRX, BCH/ETH, XRP/BNB）| 全部 ann 接近零/负（-0.8--6.6%）|
| F4 | （4 pairs: DOT/DOGE, SOL/BTC, BNB/ETH, BCH/TRX）| 全部 ann 深度负（-35--77%）|

validation 每配置每 fold 只读一次（§10.3），无回 train 改参数。

## 9. 失败 exact fingerprint 与 never-repeat 原因（§4.2 `new_failures_to_never_repeat`）

```text
1. M1_pair F2-train-fit 在 F3(2025)/F4(2026YTD) OOS 全负：
   根因 = 60d train 协整 relation 跨 regime change 非平稳。
   never-repeat: 不要用单 fold train fit 直接跑多年 OOS；需要 regime-conditioned 或
   rolling-re-fit（但后者违反 frozen-at-commit）。
2. 8 finalists 全部 DD 40-62%（远超 frontier DD 限 20-30%）：
   根因 = synchronized cycle 在不利 residual 移动时 aggregate SO 把 DD 放大。
   never-repeat: aggregate SO 的 group_gross_cap_pct 必须在约束边界（非 slack）才控 DD。
3. F1 blocked_no_stable_groups：
   根因 = 2023 H1（牛市启动）无足够 stable 协整 pair。
   never-repeat: 不要用收益补齐 fold，记 blocked（§7.1/§18）。
```

## 10. backtest_candidate vs production_ready

- 所有 G3 候选均为 `backtest_candidate`（G3 OOS 淘汰）。
- **无 production_ready candidate**（无 finalist）。
- R3 production call sites（synchronized-cycle skeleton）仍有效（real started sites），
  但 finalist-level service parity 为 not_applicable_zero_survivors。

## 11. 数据 / raw series / 五类 hash 路径

```text
market data: data/market_data_full.db（110GB, 12 symbols, 2023-01-01..2026-07-15, 1m）
funding: data/funding_rates_round12.db（12 symbols, 2023-01-01..2026-07-10, 8h）
R4 fold fits: docs/superpowers/artifacts/glm-martingale-core-round18/r4/fold-fit-freeze.json
G1 registry: docs/superpowers/artifacts/glm-martingale-core-round18/r6/g1-registry.jsonl
G2 registry: docs/superpowers/artifacts/glm-martingale-core-round18/r7/g2-registry.jsonl
G3 registry: docs/superpowers/artifacts/glm-martingale-core-round18/r8/g3-registry.jsonl
trace digests: 每个 SYNC_SUMMARY 含 event/trade/equity/funding/rejection stream sha256
```

## 12. 2026-07-11+ future lock 状态

数据实际到 2026-07-15（OHLCV）/ 2026-07-10（funding）。plan §1 hard gate 9 要求
2026-07-11 之后数据锁定，至少连续 30 天后才允许一次性 future check。Round 18 所有
validation 窗口 ≤ 2026-05-31，候选最多 `provisional_backtest_candidate`。**无 candidate
进入 future check**（0 finalist）。future lock 状态：`not_applicable_zero_finalists`。

## 13. 关键工程产物（代码 + 脚本 + artifacts）

**代码（已 commit）**：
- `crates/shared-domain/src/martingale.rs`：SynchronizedCycleConfig
- `apps/backtest-engine/src/martingale/sync_cycle_engine.rs`（新引擎，~600 行）
- `apps/backtest-engine/src/bin/synchronized_cycle_replay.rs`（新二进制）
- `apps/backtest-engine/src/martingale/r17_controls.rs`（vol_cap/cluster/hazard 真实化）
- `apps/backtest-engine/src/martingale/kline_engine.rs`（hazard call site + pub fee/slip accessor + margin helpers）
- `apps/trading-engine/src/main.rs`（production router 真实 bar + sync cycle call sites）
- `apps/trading-engine/src/martingale_runtime.rs`（6 个 sync cycle 方法 + SyncActiveCycle/SyncCycleDecision）

**脚本（已 commit）**：
- `scripts/glm_r18_r0_failclose_tests.py`、`scripts/glm_r18_r0_adversarial_activation.py`
- `scripts/glm_r18_r1_fingerprint_registry.py`
- `scripts/glm_r18_r2_selftest.py`
- `scripts/glm_r18_r4_fold_fit_freeze.py`
- `scripts/glm_r18_r5_g0_binding.py`
- `scripts/glm_r18_r6_g1_search.py`、`scripts/glm_r18_r7_g2_full_train.py`、`scripts/glm_r18_r8_g3_validation.py`

**artifacts**：`docs/superpowers/artifacts/glm-martingale-core-round18/r0/..r8/` 每 phase
gate evidence + registries + checkpoints + configs。

## 14. 诚实结论

Round 18 把「同步残差 Martingale cycle」这个前 17 轮从未原生执行过的机制从零构建、
完整绑定、完整回测（~1640 full replay，无快筛），并诚实报告：**机制有效但收益前沿
未推进**。三档目标零命中，0 finalist，`VALID_NOVEL_FAMILIES_EXHAUSTED_NO_FRONTIER_PROGRESS`。

过拟合诊断清晰：F2 train 协整在 F2 validation 仍成立所以盈利，但跨 2025 熊市 / 2026 YTD
非平稳，OOS 全负。这是 plan §10 防过拟合合同设计要检测的 structural-break，本轮诚实
捕获并报告，未用「跑了多少组」代替推进。

下一轮可能方向（不承诺命中）：regime-conditioned cointegration（每 regime 独立 fit）、
rolling re-fit with embargo（违反 frozen-at-commit 需新合同）、或 M2 factor-residual
basket（6+ leg aggregate 可能比 pair 更稳）。但这些都需新 plan，不能在 Round 18 偷偷做。
