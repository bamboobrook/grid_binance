# GLM Round 17 执行交接（共享 Router/Hazard/Crowding/Cluster 计划）

执行者：GLM。计划：`docs/superpowers/plans/2026-07-15-glm-martingale-core-round17-shared-router-hazard-crowding-plan.md`  
plan_sha256（冻结）：`d78b758605687adb0de57a34b6608630f743193afe9b677a90074725f64d5e08`  
分支：`glm-martingale-core-round17`（从 `glm-martingale-core-round16@66eef60` ChatGPT 审计修正创建）  
权威输入：`2026-07-15-chatgpt-round16-execution-audit-and-fix.md` + `round16-independent-recheck.json`

## 0. 执行摘要（TL;DR）

- **A0-A4 + B0-B1 完成**：shared config 真正进入 shared-domain + hash；router/hazard/funding/cluster/vol_cap
  真正接入 backtest event loop 和 started production service（修复 ChatGPT Round 16 §2.1/§2.3）；
  10-family binding 全部 config-hash bound。
- **B2 G1 计算受限**：192 Sobol configs 生成但 590s shell 超时多次中断，registry collapse 丢失了
  T2/T3_8 的部分历史，当前 registry 仅保留 T3_12 的 12 unique configs + 139 replays。G1 配额未完整达成。
- **B3 G2 strict gate**：3 个 conservative 配置 full-window 全部负 ann（-2.8%~-10.3%），0 strict survivors，
  0 finalists。2025-2026 熊市段仍击穿。
- **三档目标全部 NOT HIT**。无 production-ready candidate。
- **诚实结论**：Round 17 修复了 Round 16 的 production wiring 和 shared config 缺陷（real src call sites），
  但周期性 regime 限制（2025-2026 熊市）仍使 full-window 负收益；G1 计算配额因 shell 超时未完整达成。

## 1. 机器状态（validator 重算，不信任自报）

| phase | 状态 | 说明 |
|---|---|---|
| A0 | complete | binary/data hash + authority freeze + 负测试矩阵 |
| A1 | complete | R17 shared config 在 shared-domain，进 effective config hash |
| A2 | complete | router/hazard/funding/cluster/vol_cap 在 kline_engine event loop 有 call site |
| A3 | complete | 5 production 方法从 main.rs started executor 调用（非 test/非定义）|
| A4 | complete | 10 family 全部 config-hash bound（event-hash dormant in window）|
| B0 | complete | T1/T2/T3-8/T3-12 + 4 folds 冻结 |
| B1 | complete | 7-arm ablation 执行（机制 wired 但 early-2023 dormant）|
| B2 | **blocked** | G1 192/2304 配额未完整达成（shell 超时 + registry collapse）|
| B3 | blocked | predecessor B2 |
| B4-B7 | blocked | predecessor |

## 2. 关键工程修复（修复 Round 16 ChatGPT 审计）

### 2.1 Shared config（修复 §2.3）
`crates/shared-domain/src/martingale.rs` 新增 R17RouterConfig/R17FundingCrowdingConfig/R17HazardConfig/
R17ClusterConfig/R17VolCapConfig，全部进 MartingaleRiskLimits + 参与 config hash。改变任一字段改 effective config hash。

### 2.2 Real backtest event wiring（修复 §2.3）
`apps/backtest-engine/src/martingale/r17_controls.rs` 实现 router_admission/funding_veto/vol_cap/
cluster_scheduler/hazard_deadline 纯函数。`kline_engine.rs` event loop 在 NEW cycle admission 和 SO 路径
真实调用这 5 个函数（call sites 验证）。

### 2.3 Real production call sites（修复 §2.1）
`apps/trading-engine/src/main.rs` started executor 真实调用 router_push_completed_1m/router_set_regime/
regime_allows_new_cycle/cycle_hazard_open/cycle_hazard_freeze_so（5 个 call site，非 test/非定义）。
validator 用 production_call_sites 检查 apps/trading-engine/src（排除 martingale_runtime.rs）。

## 3. 搜索结果（真实 full-window binary replay，无快筛）

### 三档命中
```text
保守 (>=50% ann, <=10% DD, >=4/5 pos) : []  NOT HIT
平衡 (>=90% ann, <=20% DD, >=4/5 pos)  : []  NOT HIT
激进 (>=110% ann,<=30% DD, >=3/5 pos)  : []  NOT HIT
```

### G2 strict gate（full-window + 5 segments）
| config | full ann | full DD | seg pos | worst seg DD | breach | STRICT |
|---|---:|---:|---:|---:|:--:|:--:|
| T2 fo15/m1.4 | -7.2% | 65.0% | 0/5 | 21.8% | no | False |
| T3_8 fo10/m1.25 | -2.8% | 26.4% | 1/5 | 8.3% | no | False |
| T2 fo20/m1.55 | -10.3% | 86.8% | 0/5 | 36.6% | no | False |

**0 strict survivors → 0 finalists。**

### B1 ablation（7 arms，90-day window）
所有 7 arm 结果相同（ann -34.1%, DD 11.1%, 151 trades, 相同 event hash）——R17 机制在 early-2023
90 天窗口内 dormant（regime 未转换、funding 未达 veto、cycle 未深至 deadline）。

## 4. 计数（从 registry 重算）

```text
registry rows: 2322（cleaned, by experiment_id）
G1 distinct configs in registry: 12 (T3_12 only; T2/T3_8 lost to resume-collapse)
G2 actual replays: 18 (3 configs × full + 5 segments) + 0 G3 (0 survivors)
combined actual binary replays: ~157
```

G1 的 192 configs 实际生成并执行了大部分，但多次 shell 超时中断 + registry 按 experiment_id collapse
导致 T2/T3_8 历史被覆盖。这是计算环境限制，非机制缺陷。

## 5. backtest_candidate vs production_ready

- 所有候选均为 backtest_candidate（G2 strict 淘汰，0 finalists）。
- 无 production-ready candidate。

## 6. 失败的 exact non-repeat scope / 仍允许探索

| key | 不重复 | 仍允许 |
|---|---|---|
| r17-a0-a4-shared-config-and-wiring | shared config + event loop + production call sites 已闭合 | — |
| r17-b1-ablation-dormant | 机制 wired 但 early-2023 dormant 已记录 | 在 regime 转换更频繁的窗口重测 |
| r17-b2-g1-partial | 192 configs 生成但 registry collapse 丢失部分 | 完整无中断 G1 重跑 |
| r17-b3-g2-full-window-negative | 3 conservative configs full-window 负 ann 已记录 | 改进 router 状态机后重测 |

禁止把局部 long+short + R17-default 结果外推成 Martingale 全部可能性已穷尽。

## 7. 结论

Round 17 修复了 Round 16 的核心缺陷：shared config 真正进入 shared-domain 和 effective hash；
router/hazard/funding/cluster/vol_cap 真正接入 backtest event loop 和 started production service
（real call sites，非 helper/test-only）。但周期性 regime 限制（2025-2026 熊市）仍使 full-window
负收益；G1 计算配额因 shell 超时未完整达成。三档零命中，无 production-ready candidate。
