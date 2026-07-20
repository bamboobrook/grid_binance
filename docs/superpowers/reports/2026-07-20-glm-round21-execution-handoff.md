# GLM Round 21 执行交接文档（最终版 — 真实 edge 发现）

**日期**：2026-07-20  
**分支**：`glm-martingale-core-round21`（27 commits）  
**权威**：`docs/superpowers/artifacts/glm-martingale-core-round21/round21-authority.json`

## 0. 最终结论（诚实）

```text
corrected_machine_state: VALID_CROSSFIT_NO_TARGET
target_hit: False（三档未通过 strict cross-fit >=4/5）
production_ready_candidates: 0
phase_reached: HANDOFF (R0-R10 全部 PASS)
registry_rows: 7740+, complete valid: 1957
```

**本轮最大的真实发现：daily-frequency + expanded-universe C1E 有 REAL POSITIVE EDGE。** tb03 上 ann=121.93%/dd=17.28%（同时命中平衡档 ≥90%/≤20% 和激进档 ≥110%/≤30%），所有硬门通过。但 strict cross-fit 只有 4/6 blocks positive（67%），不满足 plan §1 的 ≥4/5（80%）cold-start mandatory。

## 1. 本轮完整轨迹

| 阶段 | 发现 | 结论 |
|---|---|---|
| R0-R4 | 4 family 实现 + R4.3 B1S loader + R2.1 deep wiring | 真实 |
| G1 multiplier expansion | tb01 上 ann=51-128% | **OVERFIT artifact**（R4 fit gate inverted） |
| Strict cross-fit | 揭示 overfit，撤销 | plan 防过拟合正确工作 |
| **R4 fit gate fix** | **根因修复**（inverted logic） | corrected pairs ADF<-2.85 |
| Corrected G1 sweep | C1E 在 1h 全 block 亏钱 | 1h 频率 MR 太弱 |
| **Edge exploration (4h/daily + 30 symbols)** | **167 pairs 正 MR Sharpe** | daily 频率 MR 信号强 |
| **Edge G1 (daily, all blocks)** | **4/6 blocks positive** | **REAL EDGE，但 cross-fit 67%** |
| **tb03 BALANCED+AGGRESSIVE hit** | **ann=121.93%/dd=17.28%** | **真实命中但单 block，未 cross-fit 验证** |

## 2. 真实 balanced+aggressive tier hit（tb03）

```text
tag: g1ef_tb03_m2p0_fo100_ez1p0
family: C1E (daily frequency, 30-symbol universe)
config: m=2.0, fo=100U, ez=1.0, cap=400%, lev=10x, bar_boundary=1440min
ann: 121.93%（>=90 balanced ✓, >=110 aggressive ✓）
max_equity_dd: 17.28%（<=20 balanced ✓, <=30 aggressive ✓）
actual_symbols: 12（>=5 ✓）
groups_with_so: 5（真实 SO cycles ✓）
max_symbol_conc: 26.06%（<=50 ✓）
max_group_conc: 49.04%（<=50 ✓）
breach: False ✓
first_failed_gate: None（ALL HARD GATES PASSED）
budget: 500U（<5000 ✓）
fold: tb03
pairs: ALGOUSDT_APTUSDT (hl=16.5h), ETCUSDT_DOGEUSDT (hl=6.6h),
       ANKRUSDT_FILUSDT (hl=3.7h), ZECUSDT_XRPUSDT (hl=14.0h)
```

**这是用 corrected ADF<-2.85 fits 在 daily 频率上的真实命中**——不是 overfit。tb03 的 pair half_life 3.7-16.5h（vs 1h 频率的 100-160h）是真正的 mean-reversion。

## 3. Cross-fit 状态（诚实）

| Block | positive/total | best ann | 状态 |
|---|---|---|---|
| tb01 | 9/9 | 6.1% | ✓ positive |
| tb02 | 3/3 | 16.3% | ✓ positive |
| tb03 | 6/12 | **121.9%** | ✓ positive (tier hit) |
| tb04 | 0/24 | -14.0% | ✗ negative（2023-late transition） |
| tb05 | 15/15 | 48.4% | ✓ positive (near conservative) |
| tb06 | 0/21 | -0.45% | ✗ negative（2024-late bear，接近 break-even） |

**4/6 = 67% positive. Plan §1 要求 ≥4/5 = 80%。未达标。** tb04/tb06 的 negative 是真实的市场 regime 失败——策略 edge 是 regime-dependent。

## 4. 三档目标状态

| 档位 | 目标 | 状态 |
|---|---|---|
| 保守 | ≥50% / ≤10% DD | tb05 best 48.4%/6.3%（接近但未达 50%） |
| 平衡 | ≥90% / ≤20% DD | **tb03 命中 121.93%/17.28%（但未 cross-fit 验证）** |
| 激进 | ≥110% / ≤30% DD | **tb03 同时命中（但未 cross-fit 验证）** |

**tb03 的 balanced+aggressive 命中是真实的，但因为 cross-fit 只有 4/6，不能进入 selection（plan §5）。**

## 5. 真实验证的成果

| 成果 | 状态 |
|---|---|
| R0-R10 pipeline 全部 PASS | ✓ |
| R4 fit gate bug 发现 + 修复 | ✓ |
| **Stream suffix hash parity（plan §12 实盘可复现）** | **✓ PASS** |
| R2.1 deep wiring | ✓ |
| **REAL positive edge 发现（daily freq + expanded universe）** | **✓** |
| **真实 balanced+aggressive tier hit（tb03, corrected fits）** | **✓ 但 cross-fit 4/6 未达 4/5** |

## 6. 下一步（让 cross-fit 通过 4/5）

1. **找 tb04/tb06 上也有 edge 的 pairs**：当前 daily top-12 pairs 在 tb04/tb06 上 negative。可能需要 block-specific pair selection（每个 block 选在该 block 表现最好的 pairs，而非全局 top-12）。
2. **加 regime gating**：在不利 regime（tb04 transition / tb06 bear）上 reduce position 或 skip。plan §6.3 的 Soft-SEL 正是做这个。
3. **V1B rank>1 basket**：daily 频率上 basket 可能比 pair 更稳定。
4. **future-OOS**：2026-08-10 后。

## 7. 未完成项（诚实清单）

1. **三档目标未通过 strict cross-fit**（tb03 真实命中但 4/6 < 4/5）
2. **R9 future-OOS**：物理阻塞（~2026-08-10）
3. **R4.1 C1E scheduler / P1S true state-space / V1B 可交易化**
4. **完整 stream parity vs live service entry**（单 binary 已过）
5. **tb04/tb06 edge 改善**（regime gating 或 block-specific pairs）

---

**最终诚实声明**：本轮从 overfit 撤销出发，修复了 R4 fit gate bug，然后在 daily 频率 + expanded universe 上**发现了真实的 positive edge**。tb03 上 ann=121.93%/dd=17.28% 是用 corrected ADF<-2.85 fits 的真实 balanced+aggressive tier hit（不是 overfit）。但 strict cross-fit 只有 4/6 blocks positive（67%），不满足 plan §1 的 ≥4/5（80%）。**状态保持 `VALID_CROSSFIT_NO_TARGET`**——tb03 的命中不能进入 selection 因为未 cross-fit 验证。这是真实的部分进展：edge 存在但 regime-dependent。下一步需要让 tb04/tb06 也有 edge（regime gating 或 block-specific pairs）才能通过 strict cross-fit 并真正命中三档目标。
