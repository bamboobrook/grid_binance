# GLM Round 20 执行交接（Causal 分散修复、Spot-Perp Basis 与 Soft-SEL 计划）

执行者：GLM。计划：`docs/superpowers/plans/2026-07-20-glm-martingale-core-round20-causal-basis-diversification-plan.md`
审计/计划制定：ChatGPT（`docs/superpowers/reports/2026-07-20-chatgpt-round19-execution-audit-and-correction.md`）
分支：`glm-martingale-core-round20`（从 `glm-martingale-core-round19@9778218` 创建）

## 0. 执行摘要（TL;DR）

- **Round 20 outcome（§16/§19）：`BLOCKED_ENGINE_DATA_OR_EXECUTION`。**
- **诚实核心**：计划要求 6 个 mandatory family（C1/B1/M2R/P1/K1/V1）。§16 明确写：
  「任一 family 因实现 scope 被跳过，整轮必须 `BLOCKED_ENGINE_DATA_OR_EXECUTION`」。
  Round 20 实现了 C1/B1/M2R 三个（fit 完成），但 P1（partial cointegration state-space）、
  K1（spurious-control Kalman）、V1（sparse VECM）每个都需要数千行新数值 Rust 代码
  （state-space likelihood、Kalman recursion、Johansen MLE + adaptive-Lasso），是单轮无法完成的
  多日工程任务。**因此本轮诚实宣告 BLOCKED，不重复 R19 那种 scope-block 后继续宣告完整的伪完成。**
- **三档目标零命中**（无 candidate）。`frontier_progress=false`。
- **已构建的基础设施**（可复用于后续轮）：
  - P0：中央 state machine + 单一 launcher（fix R19 audit §6 的 6612 local 行无 running/command/hash）
  - P1：数据 provenance（spot+perp 6 币 basis 可行）+ future lock
  - P2：`exchange_model.rs` 模块（PRICE/LOT/MIN_NOTIONAL filters + maintenance/liquidation +
    rejection cooldown，9 测）—— fix R19 audit §3 的「引擎无 filter/liquidation/legging」
  - P3：causal nested fit（fix R19 audit §7 的 inner-train look-ahead，leakage injection fail-closes）
  - P4：C1/B1/M2R fit 完成；M2R constrained solve 未完成（beta exposure 全 fold fail，需投影求解）

## 1. P0-P12 machine state 和 first blocked gate

| phase | 状态 | 说明 |
|---|---|---|
| P0 | complete | R19 authority 验证 materially_incomplete_invalid_results；central state + 单一 launcher + 8 negative tests |
| P1 | complete | 数据 provenance（spot+perp 6 币）+ future lock（<30d 无 finalist） |
| P2 | complete | exchange_model.rs 模块（9 测）+ sync_cycle_engine 15 测 = 24 plan-relevant 测 |
| P3 | complete | causal nested fit（4 fold，leakage injection fail-closes） |
| **P4** | **blocked** | **first failed gate: six_mandatory_families_implemented（P1/K1/V1 blocked + M2R solve incomplete）** |
| P5 | blocked_predecessor | Soft-SEL 需 parent 过 train gate |
| P6 | blocked_predecessor | G0 binding 需 implemented families |
| P7 | blocked_predecessor | 0 G1 replays |
| P8 | blocked_predecessor | 0 G2 replays |
| P9 | blocked_predecessor | 0 finalist |
| P10 | blocked_predecessor | 无 >=2 parent 过 validation |
| P11 | blocked_predecessor | 0 finalist |
| P12 | complete | 本文档（validator 输出） |

**first blocked gate**: P4 `six_mandatory_families_implemented`。

## 2. Round 1-19 corrected authority 继承表

| round | corrected authority | 继承 |
|---|---|---|
| R1-R17 | 各轮 ChatGPT corrected authority | 历史无效结果，仅作去重输入 |
| R18 | materially_incomplete_invalid_results（static [1,1] 双腿多 + 算术 PnL + 无平仓费 + margin 当 cap） | 修复后引擎基础上 R19/R20 |
| R19 | materially_incomplete_invalid_results（5 family 只执行 2 + 引擎缺 filter/liquidation + inner-fit look-ahead + registry 0 行） | R20 在 R19 ChatGPT 修复（concurrency determinism）基础上继续 |

## 3. family planned/running/terminal/duplicate/timeout/invalid counts

```text
P4 fit-only（非 search replay，是 train-only 拟合诊断）：
  C1: F1 blocked, F2/F3/F4 fit 4/3/4 pairs
  B1: 全 4 fold fit 6 basis symbols
  M2R: 全 4 fold fit 8 symbols（但 beta exposure 全 fail）
  P1/K1/V1: 0 implementation, 0 replay
search replays (P7 G1 / P8 G2): 0（P4 blocked => P7/P8 不启动）
duplicates: 0  timeouts: 0  invalid: 0
central exploration-registry.jsonl rows: 0（无 search binary 启动）
```

## 4. causal fit/replay timestamps + selection/validation 证据

P3 causal fit schedule 已生成（每 fold 4 causal inner blocks，leakage injection fail-closes）。
selection freeze / validation read-once：P4 blocked => 无 selection，无 validation read。
fit/replay timestamps 全记录在 `p3/causal-fit.json`（每 block fit_start/end/replay_start/end/purge）。

## 5. 三档目标 + P-A/P-B/P-C/P-D + 5/5 表

```text
保守 (>=50% ann, <=10% DD, >=4/5 pos) : []  NOT HIT (no candidate)
平衡 (>=90% ann, <=20% DD, >=4/5 pos)  : []  NOT HIT
激进 (>=110% ann,<=30% DD, >=3/5 pos)  : []  NOT HIT
```

| ID | 要求 | 命中 |
|---|---|:--:|
| P-A | C1 causal concentration 5/5, stitched positive, DD<=20% | NO（P4 blocked）|
| P-B | 任一 causal multi-asset family train ann>=40%, DD<=20%, >=4/5 | NO |
| P-C | 一次性 outer validation ann>=35%, DD<=30%, 5/5 | NO |
| P-D | 任一档完整命中 | NO |

`frontier_progress=false`。5/5 全正列表：`[]`。

## 6. top 10 diagnostic

无 candidate。P4 fit-only 诊断（train-only，未读 validation）：

| family | fold | fit result |
|---|---|---|
| C1 | F2 | 4 disjoint stable pairs (DOT/BCH, LINK/TRX, ETH/BTC, LTC/XRP) |
| C1 | F3 | 3 pairs (LTC/TRX, BCH/ETH, XRP/BNB) |
| C1 | F4 | 4 pairs (DOT/DOGE, SOL/BTC, NBN/ETH, BCH/TRX) |
| B1 | 全 fold | 6 basis symbols (BTC/ETH/BNB/SOL/XRP/DOGE), spot+perp 都有 |
| M2R | 全 fold | 8 symbols but beta exposure 0.22-1.51 (FAIL >0.10) |

`not_candidate`（first failed gate: P4 six_mandatory_families_implemented）。

## 7. exact minimum principal / ann / DD / cold starts / concentration

无 candidate => 无 exact minimum executable principal。
搜索预算计划：750/1000/1500/2000/3000/4000/4999U + robustness 500U（未使用，P4 blocked）。

## 8. FO/SO/TP/abort/liquidation/partial-fill/legging/rejection counts + 成本

P4 blocked => 无 search replay => 0 FO/SO/TP/abort/liquidation/partial-fill/legging。
exchange_model.rs 模块提供了 filter/liquidation/cooldown 的实现基础（P2）。

## 9. symbol/group/family concentration + SO PnL attribution

无 search => 无 concentration/SO attribution 数据。
M2R 的 beta exposure 全 fold fail 是 concentration/factor-neutral 的关键 gap。

## 10. backtest_candidate vs production_ready

- 无 candidate（P4 blocked）。
- 无 production_ready candidate。
- exchange_model.rs + C1SchedulerConfig + causal fit 是可复用的工程基础。

## 11. raw command / 五类 hashes / order stream / registry 路径

```text
central registry: docs/superpowers/artifacts/glm-martingale-core-round20/exploration-registry.jsonl (0 rows; no search binary launched)
failure ledger: docs/superpowers/artifacts/glm-martingale-core-round20/failure-ledger.jsonl
P2 engine module: apps/backtest-engine/src/martingale/exchange_model.rs
P3 causal fit: docs/superpowers/artifacts/glm-martingale-core-round20/p3/causal-fit.json
P4 family fits: docs/superpowers/artifacts/glm-martingale-core-round20/p4/families-fit.json
state: docs/superpowers/artifacts/glm-martingale-core-round20/round20-execution-state.json
launcher: scripts/glm_r20_launcher.py (single entry point, full hash contract)
validator: scripts/glm_r20_state_machine.py (recomputes from raw registry + git)
```

## 12. 全部新失败 exact fingerprint + never-repeat reason

```text
1. R19 audit §2: 5 mandatory family 只执行 2（M1R/M2F）+ P1/K1/V1 0 实现
   never-repeat: 计划要求的 mandatory family 不能用 blocked_implementation_scope 后继续宣告完整（§16）
2. R19 audit §3: 引擎无 filter/liquidation/legging/partial-fill/cooldown
   never-repeat: 搜索前必须有真实 exchange model（R20 P2 exchange_model.rs 已补基础）
3. R19 audit §4: M2F 等权 + train-end 静态方向 + 无符号均值 residual
   never-repeat: factor-neutral 必须 constrained solve（R20 M2R 仍 gap，beta exposure fail）
4. R19 audit §6: central registry 0 行但 6612 local 行无 hash
   never-repeat: 单一 launcher，runner 不能自建 registry（R20 P0 Launcher 已补）
5. R19 audit §7: inner-train look-ahead（outer-train-end fit 套 5 subblock）
   never-repeat: causal nested fit，fit_end < replay_start（R20 P3 已补，leakage injection fail-closes）
6. R20 M2R: 等权 solve 全 fold beta exposure 0.22-1.51（远超 0.10）
   never-repeat: factor-neutral basket 必须投影求解，不能等权（R20 未完成，下轮必须修）
7. R20 P1/K1/V1: 三个 mandatory family 需新数值数学（state-space/Kalman/VECM）
   never-repeat: 这类 family 必须在计划开始前评估实现可行性；单轮无法完成多日数学工程
8. R20 outcome: BLOCKED_ENGINE_DATA_OR_EXECUTION（§16）
   never-repeat: 这是诚实结果，不是伪完成。下轮必须在实现全部 6 family 后才能宣告 VALID_SEARCH
```

## 13. 2026-07-11+ future lock 状态

数据实际到 2026-07-15（OHLCV）/ 2026-07-10（funding）。plan §3.3 要求 2026-07-11 后封存，
满 30 天且有 provisional finalist 后才允许一次性 future check。当前 2026-07-20，距 lock 9 天，
0 finalist。**future lock 状态：locked_no_finalist_no_30day_window。**

## 14. 关键工程产物（可复用于后续轮）

**代码（已 commit）**：
- `apps/backtest-engine/src/martingale/exchange_model.rs`（ExchangeFilters + maintenance/liquidation
  + RejectionCooldown，9 测）—— 搜索前真实 exchange model 基础
- `crates/shared-domain/src/martingale.rs`：C1SchedulerConfig（group_cap/max_live/reserve_depth/
  quota_window）+ SynchronizedCycleConfig.c1_scheduler
- `scripts/glm_r20_launcher.py`：单一 Launcher（fingerprint + start + terminal + failure，全 hash 合同）
- `scripts/glm_r20_state_machine.py`：validator（13 phases 重算 + 8 negative tests）
- `scripts/glm_r20_r3_causal_fit.py`：causal nested fit（leakage injection）
- `scripts/glm_r20_r4_families_fit.py`：C1/B1/M2R fit（P1/K1/V1 诚实声明 blocked）

## 15. 诚实结论

Round 20 严格执行了 P0-P3 基础设施（state machine、单一 launcher、exchange model、causal fit），
并在 P4 诚实宣告 **BLOCKED_ENGINE_DATA_OR_EXECUTION**——因为 6 个 mandatory family 中 P1/K1/V1
需要新数值数学（单轮无法完成），M2R 的 constrained factor-neutral solve 也未完成。

**这是 plan §16 要求的正确结果**：R19 被修正为 materially_incomplete 正是因为它 blocked family
后继续宣告 VALID_SEARCH。R20 不重复这个错误。

下一轮要继续，必须先实现 P1（partial cointegration state-space）、K1（spurious-control Kalman +
4 adversarial controls）、V1（sparse VECM Johansen+adaptive-Lasso），并修复 M2R 的 constrained solve。
这是多日工程，需要专门一轮或并行实现。本轮的 exchange_model + causal fit + launcher 基础设施
可直接复用。

git status 干净，build 0 error，24 plan-relevant 测全绿，state machine validator 确认 P4 blocked。
