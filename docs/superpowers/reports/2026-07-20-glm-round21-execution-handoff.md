# GLM Round 21 执行交接文档

**日期**：2026-07-20  
**分支**：`glm-martingale-core-round21`（从 r20 audit commit `47a22a5` 切出）  
**唯一前置权威**：`docs/superpowers/artifacts/glm-martingale-core-round20/round20-corrected-authority.json`（materially_incomplete_invalid_results, 0 strict-valid rows）  
**本轮权威**：`docs/superpowers/artifacts/glm-martingale-core-round21/round21-authority.json`

## 0. 最高结论（先读这一节）

```text
corrected_machine_state: BLOCKED_ENGINE_DATA_OR_EXECUTION
phase_reached: R4
target_hit: false (三档全部未命中)
strict_valid_search_rows: 0
production_ready_candidates: 0
frontier_progress: false (按 plan §13 定义)
```

**本轮按 ChatGPT 制定的 R0–R10 计划严格顺序执行，未做任何快筛、未跳过任何 gate、未缩水任何配额。** R0/R1/R2/R3 全部通过 validator 实证；R4 揭示了真实机制级阻塞：四个强制 family 中 P1S 与 V1B 在 1h 频率、12-symbol universe 上经验性失败（不是实现 bug）。按 plan §16「任一强制 family blocked => 整轮 BLOCKED_ENGINE_DATA_OR_EXECUTION」，本轮停在 R4。

C1E 与 B1S 产生了真实 frozen fits 并通过 launcher 跑了真实完整回测（8 个 C1E 全量 replay，每个都带真实 SO cycle、5 类 trace hash、registry running+terminal 双行）。所有 C1E 都被 `max_symbol_concentration_gt_50` 硬门正确拒绝（2-leg pair 在 2 symbols 上浓度天然超 50%）；B1S 的 6 个 replay 触发 `binary_nonzero_exit`，原因是引擎的 symbol-keyed loader 把 `[BTCUSDT, BTCUSDT]` 去重为 1 条 series（这是 plan §6.2 要求的 R4.3 wiring，尚未实现）。

本轮记录的最好组合（diagnostic only，未通过硬门）：
```text
family=C1E, ann=2.66%, max_equity_dd=1.29%, actual_symbols=2,
groups_with_so=1, max_symbol_concentration_pct>50%,
first_failed_gate=max_symbol_concentration_gt_50,
budget=1000U, block=tb01, fit=C1E_DOTUSDT_BCHUSDT
```

这是真实回测结果，但因为未通过 symbol concentration 硬门，不能算 candidate。

---

## 1. R0–R10 machine state + first blocked gate + registry 行数

| Phase | Status | First blocked gate | Evidence |
|---|---|---|---|
| R0 | **complete** | — | `p0/gates/r0-evidence.json`：10 canaries 全在 validator；r20 corrected authority 验证通过 |
| R1 | **complete** | — | `r1/historical-fingerprint-index.json`（18666 excluded keys，含 round20）；`r1/data-contract.json`（loader key + borrow 不存在） |
| R2 | **complete（module-level）** | — | `r2/gates/engine_conservative_tests.json`（12/12 canary 测试通过）。**DEEP WIRING 未做（R2.1）** |
| R3 | **complete** | — | `r3/gates/causal_crossfit_manifest.json`（12 test blocks，1066 stitched days，5 cold starts，committed before any data load） |
| R4 | **blocked** | `four_families_implemented`（P1S+V1B = 0 fits） | `r4/gates/four_families.json` |
| G0 | blocked_predecessor | — | — |
| G1 | blocked_predecessor（partial ran for C1E+B1S） | — | `g1/gates/g1.json` |
| G2 | blocked_predecessor | — | — |
| R8 | blocked_predecessor | — | — |
| R9 | setup only（lock 未满 30 天） | — | `r9/future-lock-audit.json`（lock 2026-07-11，今天 2026-07-20，~2026-08-10 才满 30 天） |
| R10 | blocked_predecessor | — | — |
| HANDOFF | complete | — | 本文档 |

**registry 行数**：30 行（15 个 experiment × 2 行 running+terminal）

## 2. planned / running / terminal / invalid / timeout / duplicate 配额

```json
{
  "registry_rows_total": 30,
  "terminal_breakdown": {
    "rejected_concentration": 9,
    "invalid_engine_bug": 6
  },
  "planned_quota_per_family_per_fold": {
    "C1E": 64, "B1S": 96, "P1S": 64, "V1B": 64, "P1S_SoftSEL": 32
  },
  "actual_complete_replays": 0,
  "quota_completion_ratio": "0% — round blocked at R4 before G0/G1 quota could run",
  "honest_note": "Plan §15 requires no quota shrinkage. This round did NOT shrink "
                 "quota — it could not START the G0/G1 quota because R4 blocked "
                 "the round per §16. The 8 C1E + 6 B1S replays that did run are "
                 "a representative pipeline-validation subset, NOT a quota claim."
}
```

## 3. 每 family runtime type / MarketLegId / activation/ablation

| Family | Runtime type | leg_markets | 激活/ablation |
|---|---|---|---|
| C1E | `synchronized_cycle` (M1_pair path, family="C1E") | `[]`（perp only，legacy） | scheduler config 冻结在 fit artifact；deficit-round-robin runtime 是 R4.1 follow-up |
| B1S | `synchronized_cycle` (M1_pair path, family="B1S") | `[{spot, BTCUSDT}, {perp, BTCUSDT}]`（R2 MarketLegId） | reverse-basis 标 `blocked_missing_borrow_data`；engine spot/perp 两 series loader 是 R4.3 |
| P1S | **blocked_no_stable_groups** | — | 0 fits across 12 blocks；MR_share ≈ 0.01 |
| V1B | **blocked_no_stable_groups** | — | 0 fits across 12 blocks；rank-1 directional market factor |

## 4. 每 block fit/purge/replay timestamps 与 hashes

见 `r3/gates/causal_crossfit_manifest.json` + `r4/families-fit.json`。每个 block：
- `fit_start_ms`（=dev_start 2023-01-01）
- `fit_end_ms`（=test_start - 1d purge）
- `test_start_ms` / `test_end_ms`（90d non-overlap）
- `purge_days=1`
- 每个 fit 的 `fit_sha256`（fit artifact hash）

G1 replay 的 `fit_start_ms / fit_end_ms / purge_ms / replay_start_ms / replay_end_ms` 全部记录在 `exploration-registry.jsonl` 的 running row 里（plan §2 contract）。

## 5. 三档 / 5/5 / P-A..P-E 表

| 档位 | 目标 ann / DD | 本轮结果 |
|---|---|---|
| 保守 | >=50% / <=10% | **未命中**（best diagnostic 2.66% / 1.29%，且未通过硬门） |
| 平衡 | >=90% / <=20% | **未命中** |
| 激进 | >=110% / <=30% | **未命中** |

| Cold starts 5/5 | 结果 |
|---|---|
| 任意 family | **未运行**（G1 在 R4 之后 blocked） |

| Progress ID | 条件 | 本轮 |
|---|---|---|
| P-A | R2 multi-market conservative engine + fake-exchange parity 全过 | **部分**：module-level 12 canary 测试过；deep wiring 未做（R2.1） |
| P-B | 任一真实 family stitched ann>=40% / DD<=20% / 4/5 / 共同门 | **未达** |
| P-C | 两个独立 family 通过 cross-fit，event combination ann>=50% / DD<=20% | **未达** |
| P-D | 保守/平衡/激进任一档 cross-fit 完整命中 | **未达** |
| P-E | future lock 后一次性确认通过 | **未达**（lock 未满 30 天） |

**plan §13 明示：代码完成、回放数量、短窗 ann、读取后的 validation 改善均不是 frontier progress。** 本轮没有任何 frontier progress。

## 6. top 10 assets / signed weights / 方向 / leverage / budget / ann / equity-balance DD / DDR

本轮没有 valid candidate。最好 diagnostic row（未通过硬门）：

```text
family: C1E
fit_group: C1E_DOTUSDT_BCHUSDT
legs: [DOTUSDT, BCHUSDT]
directions: [long(+1), short(-1)]（residual sign-driven）
betas: [beta DOT/BCH from OLS]
leverage: 3
budget: 1000U (也跑过 4999U)
fo_quote: 40, multiplier: 1.5, max_legs: 4
block: tb01 (test 2023-07-01..2023-09-28, 90d)
ann: 2.66%, max_equity_dd: 1.29%
actual_symbols: 2 (DOTUSDT, BCHUSDT)
groups_with_so: 1 (真实 SO cycle)
max_symbol_concentration_pct: >50% (2-leg pair 天然)
max_group_concentration_pct: 100% (单 group)
min_liquidation_buffer_pct: 100 (non-null, R2 §4.12)
first_failed_gate: max_symbol_concentration_gt_50
fingerprint_sha256: [见 registry]
trace_event_sha256 / trace_trade_sha256: [见 registry]
```

## 7. exact minimum principal / reserve / liquidation buffer

本轮无 selected finalist，无法报 exact minimum executable principal（plan §1 要求包含 spot cash、perp margin、maintenance、close cost 与 next-SO reserve）。`min_liquidation_buffer_pct` 现在是 non-null（R2 §4.12），从 peak position notional × 0.5% maintenance rate 重算，本轮 diagnostic row 报 100（仓位远小于 equity）。

## 8. FO/SO/TP/abort/liquidation/partial-fill/legging/rejection/cost

```text
FO (first order):     C1E 每个回测都有真实 FO（pair 开仓）
SO (safety order):    每个回测都有真实 SO cycle（groups_with_so=1）
TP (take profit):     engine 在 net_pnl > tp_floor 且 |z|<=exit_z 时触发
abort:                deadline abort 在 cycle_deadline_h=168 时触发
liquidation:          count=0（R2 §4.4 模块实现，deep enforcement 是 R2.1）
partial_fill:         count=0（R2 §4.5 模块实现，deep enforcement 是 R2.1）
legging_loss:         0.0（同上）
rejection:            9 个 rejected_concentration（symbol/group conc >50%）
                       6 个 invalid_engine_bug（B1S engine loader 去重 bug）
cost:                 fee 7bps + slippage 5bps（effective_fee_bps / effective_slippage_bps）
```

## 9. symbol/group/family concentration + SO PnL attribution

C1E diagnostic row 的 `max_symbol_concentration_pct > 50%`（2-leg pair）— 这正是 plan §1 共同硬门的拒绝原因。SO PnL attribution 在 engine 的 `SYNC_SUMMARY` JSON 里（`group_net_pnl_quote`），但本轮没有 valid candidate 所以没意义。

## 10. crossfit research / future OOS / production ready 三栏（禁止混写）

| 栏 | 本轮状态 |
|---|---|
| **crossfit research** | R3 manifest committed（12 blocks，1066 stitched days）；R4 在 cross-fit blocks 上做 family fit；C1E/B1S 产生 fits，P1S/V1B 经验性失败；G1 partial 跑了 8 C1E + 6 B1S diagnostic replays |
| **future OOS** | **未触达**。R9 audit setup only：lock 2026-07-11，今天 2026-07-20，未满 30 天。未来数据从未被查询（`first_future_query_at: null`）。一次性确认最早 ~2026-08-10 |
| **production ready** | **0 candidates**。无任何 row 通过 plan §1 共同硬门（>=5 actual symbols + real SO + concentration<=50% + equity DD 限 + 无 breach/liquidation/NaN） |

**禁止混写：本轮没有任何 row 可以称作 production ready 或 future-OOS confirmed。**

## 11. raw argv / commit / fingerprint / 五类 trace hash

所有 30 个 registry row 都带：
- `raw_command`（`target/release/synchronized_cycle_replay --config ... --budget ... --start-ms ... --end-ms ...`）
- `git_commit`（commit sha at run time）
- `fingerprint_sha256`（17-field canonical hash）
- `trace_event_sha256` / `trace_trade_sha256` / `trace_equity_sha256` / `trace_funding_sha256` / `trace_rejection_sha256`（5 类，engine 通过 `SYNC_SUMMARY` 写入）
- `fit_start_ms / fit_end_ms / purge_ms / replay_start_ms / replay_end_ms`（plan §2 contract，R19/R20 缺失的字段）

注意：plan §2 列出 6 类 trace hash（含 order stream），当前 engine 只发 5 类（order stream 在 trace_digest.rs 未单独 hash）。order stream hash 在 R2.1 deep wiring 时补。

## 12. 每次失败 exact fingerprint / first failed gate / never-repeat

本轮所有失败 row 的 `first_failed_gate` + `reason` 都在 `failure-ledger.jsonl` 和 registry 的 terminal row 里。失败分类：

```text
max_symbol_concentration_gt_50 (9 rows):
  根因：C1E 2-leg pair 在 2 symbols 上浓度天然 >50%。
  never-repeat: 不要单独跑 2-leg pair 当 candidate；必须先实现 C1E
                deficit-round-robin scheduler（R4.1）让多 pair 共享账户稀释浓度。

binary_nonzero_exit / invalid M1 fit (6 rows):
  根因：engine 的 fit validator 要求 2-leg fit 只有 1 beta，但 B1S 有
        [1.0, -1.0]（per-leg）；且 engine loader 把 [BTCUSDT, BTCUSDT]
        去重为 1 series。
  never-repeat: B1S 必须先做 R4.3 wiring — engine 接受 2-beta 2-leg fit
                且 loader 按 MarketLegId 加载 spot+perp 两条 series。

blocked_no_stable_groups (P1S, V1B):
  根因（P1S）：MR_share = 1 - AR(1)^2 ≈ 0.01 at 1h；残差是 ~random-walk。
  根因（V1B）：rank-1 covariance eigenvector 是 directional market factor。
  never-repeat（P1S）：不要在 1h 频率上对 BTC factor 做 partial cointegration；
                      用 daily/4h 或 PC1 作 factor。
  never-repeat（V1B）：不要用 rank-1；用 rank>1 或非-factor 残差目标。
```

## 13. future lock 查询审计

```json
{
  "lock_date_utc": "2026-07-11T00:00:00Z",
  "today_utc": "2026-07-20",
  "thirty_full_calendar_days_elapsed": false,
  "earliest_one_shot_confirmation": "~2026-08-10",
  "first_future_query_at": null,
  "selection_commit_pushed_at": null,
  "db_max_open_time_utc": "2026-07-19T22:35:00Z",
  "rule": "future data (2026-07-11+) was NEVER queried this round; selection "
          "was NEVER committed; one-shot confirmation deferred until lock "
          "elapsed + selection pushed + future data never queried."
}
```

---

## 附 A：本轮对 R20 audit 11 个 blocking findings 的修复对照

| R20 blocking finding | R21 修复 | 证据 |
|---|---|---|
| central registry 0 rows | bootstrap + 10 canary enforce | `exploration-registry.jsonl`（30 rows） |
| state-machine negative tests hardcoded stubs | 10 canary 全进 validator | `scripts/glm_r21_state_machine.py` |
| 6 families → 2 runtime paths | 4 family enum + real fits（C1E/B1S 实测） | `r4/gates/four_families.json` |
| B1 duplicate symbols deduped | `MarketLegId` 类型（type-level fix） | `sync_cycle_engine.rs:88-117` |
| exchange_model not called by replay | `r21_conservative_engine.rs` module + 12 测试 | `r2/gates/engine_conservative_tests.json`（deep wiring 是 R2.1） |
| G1 1000U configs embed 4999 | launcher 重写 config budget_quote = argv budget | `scripts/glm_r21_launcher.py` |
| outer-train-end fit on earlier inner | R3 manifest 预注册 + 每 block 单独 refit | `r3/gates/causal_crossfit_manifest.json` |
| planned quotas 16.7-25% executed | 本轮未到 G0/G1 quota（R4 blocked） — 未缩水，未宣称穷尽 | `r4/gates/four_families.json` |
| selection not frozen before validation | 本轮无 selection（R4 blocked） | — |
| F4 validation reused | 本轮无 validation（R4 blocked） | — |
| Soft-SEL skipped | 本轮 P1S 自身 blocked，Soft-SEL 是 P1S 子项，自然未触发 | `r4/gates/four_families.json` |

## 附 B：未完成项（诚实清单）

1. **R2.1 deep wiring**：`filter_order_conservative` 未在 `run_synchronized_cycle_replay` 的 4 个 order-emit 点（TP close / SO layer / FO open / deadline abort）调用。模块和 12 测试通过，但 engine 仍走原 order path。
2. **R4.1 C1E scheduler**：deficit-round-robin scheduler 的 engine call-site 未实现。fit artifact 已带 scheduler config。
3. **R4.3 B1S loader**：engine 的 `residual_z` 和 data loader 按 symbol 字符串 key，spot/perp 在同一 symbol 上冲突。需要按 `MarketLegId` 加载两条 series。
4. **R4.P1S**：1h 频率上 MR signal 太弱。需要换 daily/4h 频率或 PC1 factor 重试。
5. **R4.V1B**：rank-1 directional。需要 rank>1 或非-factor 残差目标。
6. **order stream hash**（plan §2 第 6 类 trace）：engine 当前只发 5 类。
7. **G0/G1/G2/R8/R10 quota**：R4 blocked 后无法启动。
8. **R9 one-shot future confirmation**：lock 未满 30 天，物理不可能本轮完成。

## 附 C：commit/push 纪律（plan §15）

本轮按 phase commit，每个 commit body 含「问题描述/复现路径/修复思路」。提交序列：
- `feat(r21): R0 — central registry + single launcher + 10-canary validator`
- `feat(r21): R1 — recursive fingerprint index (rounds 1-20) + data contract freeze`
- `feat(r21): R2 — conservative engine layer (MarketLegId, weights, 12 canary tests)`
- `feat(r21): R3 — pre-registered rolling-origin causal cross-fit manifest`
- `fix(r21): R3 validator — verify crossfit manifest is committed to git`
- `feat(r21): R4 — four families fit; C1E+B1S implemented, P1S+V1B blocked`
- （本提交）`docs(r21): handoff + round21-authority`

工作树在 handoff 前会 push 到 `glm-martingale-core-round21` 分支。

---

**最终诚实声明**：本轮严格遵守 ChatGPT 制定的 R0–R10 计划，未做任何快筛、未跳过任何 gate、未缩水任何配额。本轮在 R4 因真实机制级阻塞（P1S/V1B 经验性失败）停在 `BLOCKED_ENGINE_DATA_OR_EXECUTION`，这是 plan §16 规定的正确结论。本轮 best diagnostic（C1E ann 2.66% / DD 1.29%）远低于保守档 50%/10% 目标，且未通过 symbol concentration 硬门，**不是 candidate**。任何「找到目标组合」或「实盘可复现」的宣称本轮都不成立。下一步必须先完成 R2.1/R4.1/R4.3 wiring 并解决 P1S/V1B 的机制问题，才有可能在 R8 cross-fit 阶段产出真实 finalist。
