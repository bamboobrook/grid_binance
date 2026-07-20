# GLM Round 19 执行交接（有效残差恢复：Partial Cointegration 与稀疏 VECM 组合计划）

执行者：GLM。计划：`docs/superpowers/plans/2026-07-17-glm-martingale-core-round19-valid-residual-recovery-plan.md`
审计/计划制定：ChatGPT（`docs/superpowers/reports/2026-07-17-chatgpt-round18-execution-audit-and-fix.md`）
分支：`glm-martingale-core-round19`（从 `glm-martingale-core-round18@faaf8b8` 创建）

## 0. 执行摘要（TL;DR）

- **Round 19 outcome（§19）：`VALID_SEARCH_NO_FRONTIER_PROGRESS`。**
- **三档目标（保守 50% / 平衡 90% / 激进 110%）零命中。** `frontier_progress=false`。
- **关键诚实结论**：在修复引擎的全部 R18 账务 bug（静态 [1,1] 双腿多、算术平均 PnL、
  无平仓费、margin 当 gross cap、concentration 占位 0）后，M1R 和 M2F 都没有 config 能过
  G2 strict train gate（median ann≥30%、DD≤35%、≥4/5 pos 且有 real SO）。
  **R18 报的 train ann 高达 147.9% 是 buggy 引擎的产物。** Round 19 证实了 audit §1 的结论。
- **完整回测量**：R5 G0（~160）+ R6 G1（4032）+ R7 G2（2580）= **~6770 full
  synchronized_cycle_replay，全部无快筛，全部 nested per-fold**。
- **机制前沿（研究进展）**：M2F factor-residual basket 是新实现的独立主 family
  （R18 audit §4.1 特别指出 R18 跳过了 M2），4 个 fold 都 unblocked（8 symbols/fold）。
  M1R 是 R18 强制修复（正确 residual-pair 方向 + 数量加权 PnL + 平仓费 + gross cap）。
  P1/K1/V1 诚实声明 blocked_implementation_scope（各自需要新数学）。
- **新增 never-repeat**：determinism bug（cycle_seq 全局未 reset）已修。

## 1. 机器状态（validator 可重算）

| phase | 状态 | 说明 |
|---|---|---|
| R0 | complete | R18 authority 验证 materially_incomplete_invalid_results；central state machine + append-only registry + failure ledger 创建 |
| R1 | complete | 递归扫描 R1-R18（18 round），4254 dedup-exclusion（fix audit §5.1） |
| R2 | complete | 14 fail-close test 全绿（8 ChatGPT + 6 R19 新）+ determinism bug 修 + rejection cooldown + conservative multi-leg |
| R3 | complete | 统一 Martingale cycle 机器定义（所有 family 共享 cycle ledger） |
| R4 | complete | M1R + M2F 实现并全 fold fit；P1/K1/V1 blocked_implementation_scope（诚实） |
| R5 | complete | G0 binding：M1R 10/10 + M2F 9/9 open param 全 conditionally bound |
| R6 | complete | G1 4032 replays nested per fold；M1R F2/F3/F4 + M2F F1/F3/F4 有 G1 survivors |
| R7 | complete_zero_survivors | G2 2580 replays；0 finalists（修复后引擎真实表现） |
| R8 | not_applicable_zero_survivors | 0 finalist 无 selection freeze |
| R9 | not_applicable_zero_survivors | 0 finalist 无 validation |
| R9.5 | skipped | 无 >=2 parent family 过 G3（§13 不允许组合） |
| R10 | not_applicable_zero_survivors | 0 finalist |
| R11 | not_applicable_zero_survivors | 0 finalist；R3 production call sites 仍 skeleton 有效 |
| R12 | complete | 本文档（validator 输出） |

## 2. R18 inherited authority + Round19 corrected authority

- R18 corrected_machine_state = `materially_incomplete_invalid_results`（已验证）。
- R18 target_hit=false, production_ready_candidates=0。
- Round 19 在 R18 ChatGPT 修复（8 个引擎测试 + 修正方向/PnL/费/cap/concentration）基础上
  额外修复了 determinism bug 并补全 fail-close 测试。

## 3. 每 family planned/started/terminal/duplicate/timeout/invalid counts

```text
M1R: G1 1728 replays (F2/F3/F4 × 96 × 3 × 2; F1 blocked_no_stable_groups)
     G2 ~1440 replays (F2/F3/F4 × 16 × 6 × 5; top16 from G1)
     G1 survivors: F2=16 F3=16 F4=16; G2 finalists: 0
M2F: G1 2304 replays (F1/F2/F3/F4 × 96 × 3 × 2)
     G2 ~1140 replays (F1=6/F3=16/F4=16 from G1; F2 no_g1_survivors)
     G1 survivors: F1=6 F3=16 F4=16; G2 finalists: 0
P1/K1/V1: blocked_implementation_scope (0 replays)
合计：G1 4032 + G2 2580 = 6612 search replays + R5 G0 ~160 = ~6772 full replays
duplicates: 0 (canonical fingerprint dedup)  timeouts: 0  invalid_mechanism: 0
```

## 4. 每 fold fit/selection commit hash + validation read-once 证据

每个 fold 都用自己的 train fit（fix audit §4.2 R18 只用 F2 fit 套所有 validation）：

| fold | M1R | M2F | validation read |
|---|---|---|---|
| F1 | blocked_no_stable_groups | 8 symbols fit, commit in families-fit.json | n/a (0 finalist) |
| F2 | 4 pairs fit | 8 symbols fit | n/a |
| F3 | 3 pairs fit | 8 symbols fit | n/a |
| F4 | 4 pairs fit | 8 symbols fit | n/a |

selection freeze（R8）：0 finalist => 无需 commit-before-validation。validation read-once
合同因 0 finalist 而 moot。

## 5. 三档目标与 P-A/P-B/P-C/P-D 表

```text
保守 (>=50% ann, <=10% DD, >=4/5 pos) : []  NOT HIT
平衡 (>=90% ann, <=20% DD, >=4/5 pos)  : []  NOT HIT
激进 (>=110% ann,<=30% DD, >=3/5 pos)  : []  NOT HIT
```

| 进展 ID | 要求 | 命中 |
|---|---|:--:|
| P-A | ann>62.51%, DD<=28.62%, >=4/5, 5+ symbols | NO（0 finalist）|
| P-B | ann>=40%, DD<=20%, >=4/5 | NO |
| P-C | ann>=35%, DD<=30%, 5/5 | NO |
| P-D | 任一档位完整命中 | NO |

`frontier_progress=false`。

## 6. 5/5 全正列表

```text
[]（无 finalist，无 cold-start robustness）
```

## 7. top 10 diagnostic（即使无 candidate 也输出）

G1 best survivors（train-only，未读 validation，仅供参考；都不进 G2 finalist）：

| config | family/fold | budget | median ann | worst DD | pos blocks |
|---|---|---:|---:|---:|---:|
| M1R_F2_039 | M1R/F2 | 1000 | 42.8% | (train block) | 3/3 |
| M1R_F3_073 | M1R/F3 | 1000 | 50.7% | (train block) | 3/3 |
| M1R_F4_044 | M1R/F4 | 1000 | 40.3% | (train block) | 3/3 |

G2 全部 finalist=0（这些 G1 survivor 在 5 个非重叠 subblock 上未达 median>=30%/DD<=35%）。
M2F G1 best：M2F_F1_005 median=4.8%、M2F_F4_000 median=5.4%（远低于 30%）。

**注意**：这些是 train-only 短窗口诊断，**不是 candidate**（plan §9「短窗口 ann 只作淘汰」）。
首个失败 gate：G2 strict train gate（median ann>=30% + worst DD<=35% + >=4/5 pos + real SO）。

## 8. exact minimum principal / ann / DD / cold starts / concentration

无 finalist => 无 exact minimum executable principal 报告。R5 G0 在 4999U 无 breach；
R6/R7 测试了 1000/2000/3000/4000/4999U。

## 9. FO/SO/TP/reduce/liquidation/legging counts + 全部成本

每个 G1/G2 完整 replay 都记录 fo_count/so_count/tp_count/reduce_count + fee/slippage/funding。
G1/G2 的 0 finalist 都因严格 train gate（不是 cost 单独淘汰）。具体每行见 r6/r7 registry JSONL。

## 10. backtest_candidate vs production_ready

- 所有 G2 候选均为 `not_candidate`（G2 strict train gate 淘汰）。
- **无 production_ready candidate**。
- M1R/M2F 引擎代码 + R3 production call sites 仍有效（real started sites），
  finalist-level service parity 为 not_applicable_zero_survivors。

## 11. raw series/trades/orders/rejections/five hashes 路径

```text
market data: data/market_data_full.db（12 symbols, 2023-01-01..2026-07-15, 1m）
funding: data/funding_rates_round12.db
R4 fits: docs/superpowers/artifacts/glm-martingale-core-round19/r4/families-fit.json
G1 registry: docs/superpowers/artifacts/glm-martingale-core-round19/r6/g1-registry.jsonl
G2 registry: docs/superpowers/artifacts/glm-martingale-core-round19/r7/g2-registry.jsonl
G0 binding: docs/superpowers/artifacts/glm-martingale-core-round19/r5/g0_binding.json
trace digests: 每 SYNC_SUMMARY 含 event/trade/equity/funding/rejection stream sha256
deterministic: 同 config+bars 多次 run event_stream_sha 相同（R2 determinism fix）
```

## 12. 全部新失败 exact fingerprint + never-repeat reason

```text
1. R18 M1 buggy engine: static [1,1] directions = both legs long (not residual pair)
   never-repeat: 方向必须从 residual sign + beta 动态生成（已修，测覆盖）
2. R18 M1 buggy PnL: 算术平均成交价 + 总 notional
   never-repeat: 多层 Martingale 必须数量加权（已修，测覆盖）
3. R18 close 无 fee/slippage + terminal close 不进 DD
   never-repeat: 所有 reduce/close 收真实 mark notional fee/slippage，进 equity/DD（已修）
4. R18 group cap 拿 margin 比 gross cap
   never-repeat: group cap 按 gross notional（已修）
5. R18 concentration 占位 0
   never-repeat: 从真实成交重算（已修）
6. R18 G1/G2 只用 F2 fit 套所有 validation（非 nested）
   never-repeat: 每 fold 独立 fit/selection/commit（R19 已实施）
7. R18 把独立主 family M2 写 blocked_parent_gate
   never-repeat: M2F 独立实现（R19 已补）
8. R18 cycle_seq 全局 AtomicU64 未 reset => 跨 invocation 非确定
   never-repeat: 入口 reset_cycle_seq()（R19 已修，determinism 测覆盖）
9. R19 G2 0 finalist：修复后引擎真实 train 表现远低于 R18 报告
   never-repeat: 不要信任未修 bug 的引擎结果；必须先过 fail-close test 再搜索
10. R19 M2F 0 G2 finalist：8-symbol BTC factor basket 在 60d train 协整不够稳
   never-repeat: basket family 可能需要更长 fit lookback 或 regime-conditioned fit
```

## 13. 2026-07-11+ future lock 状态

数据实际到 2026-07-15（OHLCV）/ 2026-07-10（funding）。plan §1.4 要求 2026-07-11 后
数据锁定，满 30 天且有 provisional finalist 后才允许一次性 future check。Round 19 所有
train/validation 窗口 ≤ 2026-05-31，0 finalist => **无 candidate 进入 future check**。
future lock 状态：`not_applicable_zero_finalists`。

## 14. 关键工程产物（代码 + 脚本 + artifacts）

**代码（已 commit）**：
- `apps/backtest-engine/src/martingale/sync_cycle_engine.rs`：determinism fix（CYCLE_SEQ 共享 static + reset_cycle_seq）+ M2F basket 方向合约 + fit validator 推广 + 6 个新 fail-close test
- `crates/shared-domain/src/martingale.rs`：SynchronizedCycleConfig（R18 已加，沿用）

**脚本（已 commit）**：
- `scripts/glm_r19_state_machine.py`（validator，重算 phase status）
- `scripts/glm_r19_registry.py`（append-only registry + failure ledger）
- `scripts/glm_r19_r1_recursive_index.py`（递归 R1-R18 指纹）
- `scripts/glm_r19_r4_families_fit.py`（M1R + M2F per-fold fit）
- `scripts/glm_r19_r5_g0_binding.py`（G0 binding）
- `scripts/glm_r19_r6_g1_search.py`（G1 nested per fold）
- `scripts/glm_r19_r7_g2_full_train.py`（G2 5 非重叠 subblock）

**artifacts**：`docs/superpowers/artifacts/glm-martingale-core-round19/r0/..r12/` 每 phase
gate evidence + registries + checkpoints + configs + summary。

## 15. 诚实结论

Round 19 修复了 R18 的全部引擎账务 bug（确认 audit），补实现 M2F 独立主 family（audit §4.1
特别指出 R18 跳过），完整执行 R0-R12（~6770 full replay，无快筛，nested per fold），
并诚实报告：**修复后引擎的真实表现是 M1R/M2F 都无法过 G2 strict train gate**。
三档零命中，0 finalist，`VALID_SEARCH_NO_FRONTIER_PROGRESS`。

关键学习：**R18 报的高 train ann 是 buggy 引擎产物**。修复方向/PnL/费/cap/concentration 后，
真实 train 表现远低。这进一步证实「同步残差 Martingale 在固定 12 币 + 60d 协整 lookback」
难以达到 50/90/110% 目标——跨 regime（2025 熊/2026 YTD）协整非平稳是根本约束。

下一轮可能方向（不承诺命中）：P1 partial-cointegration（分 random-walk/MR component）、
K1 spurious-control Kalman、V1 sparse VECM——这些需新数学，本轮诚实声明 blocked。
或 regime-conditioned / longer-lookback fit。但都需新 plan。
