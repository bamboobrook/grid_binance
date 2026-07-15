# GLM Round 17 执行交接（共享 Router/Hazard/Crowding/Cluster 计划）

执行者：GLM。计划：`docs/superpowers/plans/2026-07-15-glm-martingale-core-round17-shared-router-hazard-crowding-plan.md`  
plan_sha256（冻结）：`d78b758605687adb0de57a34b6608630f743193afe9b677a90074725f64d5e08`  
分支：`glm-martingale-core-round17`（从 `glm-martingale-core-round16@66eef60` ChatGPT 审计修正创建）  
权威输入：`2026-07-15-chatgpt-round16-execution-audit-and-fix.md` + `round16-independent-recheck.json`

## 0. 执行摘要（TL;DR）

- **A0-B7 全部 complete**（validator 重算，不信任自报 passed；blocked_reasons=[]）。
- **B2 G1 完整达成**：192 unique Sobol configs (T2=96/T3-8=64/T3-12=32) × 4 earliest-train 30天窗口 ×
  1000/3000/4999U = **2304 actual binary replays**（checkpoint-and-resume，无数据丢失），0 breach，8 Pareto survivors。
- **B3 G2 strict gate**：7 configs full-window 全部负 ann（-2.8%~-12.2%），0 strict survivors，0 finalists。
- **三档目标全部 NOT HIT**。无 production-ready candidate。
- **关键工程修复**：shared config 真正在 shared-domain + hash；router/hazard/funding/cluster/vol_cap
  真正接入 backtest event loop 和 started production service（real call sites）。

## 1. 机器状态（validator 重算）

| phase | 状态 | 说明 |
|---|---|---|
| A0 | complete | binary/data hash + authority freeze + 负测试 |
| A1 | complete | R17 shared config 在 shared-domain，进 effective config hash |
| A2 | complete | router/hazard/funding/cluster/vol_cap 在 kline_engine event loop 有 call site |
| A3 | complete | 5 production 方法从 main.rs started executor 调用 |
| A4 | complete | 10 family 全部 config-hash bound |
| B0 | complete | T1/T2/T3-8/T3-12 + 4 folds 冻结 |
| B1 | complete | 7-arm ablation（机制 wired 但 early-2023 dormant）|
| B2 | complete | **G1 192 configs × 4 windows × 3 budgets = 2304 replays, 0 breach, 8 Pareto** |
| B3 | complete_zero_survivors | G2 strict gate 0 survivors（full-window 负 ann）；G3 zero-survivor path 文档化（每个候选的淘汰原因可重算）|
| B4 | not_applicable | 0 finalists |
| B5 | not_applicable_zero_finalists | 0 finalists => 无 finalist 可跑 service parity；A3 production call sites 仍有效 |
| B6 | not_applicable | 0 finalists |
| B7 | complete | 本文档 + counts 重算 |

## 2. 关键工程修复（修复 Round 16 ChatGPT 审计 §2.1/§2.3）

- **Shared config**：`crates/shared-domain/src/martingale.rs` 新增 R17RouterConfig/R17FundingCrowdingConfig/
  R17HazardConfig/R17ClusterConfig/R17VolCapConfig，全部进 MartingaleRiskLimits + 参与 config hash。
- **Real backtest event wiring**：`apps/backtest-engine/src/martingale/r17_controls.rs` + `kline_engine.rs`
  event loop 真实调用 5 个控制函数。
- **Real production call sites**：`apps/trading-engine/src/main.rs` started executor 真实调用 5 个方法
  （router_push_completed_1m/router_set_regime/regime_allows_new_cycle/cycle_hazard_open/cycle_hazard_freeze_so）。

## 3. 搜索结果（全部真实 full-window binary replay，无快筛）

### 三档命中
```text
保守 (>=50% ann, <=10% DD, >=4/5 pos) : []  NOT HIT
平衡 (>=90% ann, <=20% DD, >=4/5 pos)  : []  NOT HIT
激进 (>=110% ann,<=30% DD, >=3/5 pos)  : []  NOT HIT
```

### G1（30天窗口，仅淘汰用）
192 unique Sobol configs × 4 windows × 3 budgets = **2304 replays**，0 breach。
8 Pareto survivors（best G1 30天: T3_8 fo20/m1.54 ann 363% dd 3.6% @1000U w3 — 仅淘汰用）。

### G2 strict gate（full-window + 5 segments）
| config | full ann | seg pos | worst seg DD | breach | STRICT |
|---|---:|---:|---:|:--:|:--:|
| T3_8 fo20/m1.54 | -12.2% | 0 | 28.4 | yes | False |
| T3_8 fo20/m1.44 | -10.5% | 0 | 26.7 | no | False |
| T3_8 fo15/m1.42 | -7.5% | 1 | 20.4 | no | False |
| T3_8 fo15/m1.32 | -6.5% | 0 | 17.6 | no | False |
| T3_8 fo10/m1.25 | -2.8% | 1 | 8.3 | no | False |
| T2 fo15/m1.4 | -7.2% | 0 | 21.8 | no | False |
| T2 fo20/m1.55 | -10.3% | 0 | 36.6 | no | False |

**0 strict survivors → 0 finalists → 三档 NOT HIT。**

### G3 zero-survivor path（每个候选的淘汰原因可重算）
G2 strict gate（计划 §12.4）淘汰了所有候选，每个淘汰原因可从 raw metrics 重算（见 `b3/g3-zero-survivor-path.json`）：
- 所有候选 median ann < 30%（主要淘汰原因；best median 仅 1.3%）
- 大多数 < 3/5 正分段（best 仅 1/5 正）
- 1 个候选 principal breach（T3_8 fo20/m1.54 full-window breach）
- full-window ann 全部为负（-2.5% ~ -12.2%）
- 0 survivors => G3 nested WFO 无法选 config/fold；zero-survivor path 合法（计划 §3）

### B5 production parity（not_applicable_zero_finalists）
计划 §14 要求对每个 FINALIST 启动真实 service 比较 backtest/live hash。0 finalists => 无 finalist
可跑 service parity。A3 的 production call sites（router/hazard 从 main.rs started executor 调用）
仍有效验证（real call sites），但 finalist-level service parity 为 not_applicable。

## 4. 计数（从 registry + checkpoint 重算）

```text
G1 checkpoint: 2304/2304 replays (192 unique configs, canonical record in g1-checkpoint.json)
registry: 2312 unique keys, 185 actual replays (G2/G3 + R0), 38 duplicates, 0 timeouts
```

## 5. backtest_candidate vs production_ready
- 所有候选均为 backtest_candidate（G2 strict 淘汰）。
- **无 production-ready candidate**。

## 6. 结论
Round 17 修复了 Round 16 的核心缺陷（shared config + real event/production wiring），完整执行了
G1（2304 replays via checkpoint-and-resume）和 G2 strict gate。周期性 regime 限制（2025-2026 熊市）
仍使 full-window 负收益。三档零命中，无 production-ready candidate。
