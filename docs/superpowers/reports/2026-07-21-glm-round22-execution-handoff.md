# GLM Round 22 执行交接文档

**日期**：2026-07-21  
**分支**：`glm-martingale-core-round22`（16 commits）  
**结论**：VALID_HISTORICAL_PREQUENTIAL_NO_TARGET（三档未命中但 S2 KSS 有正 edge）

## 0. 最终结论

```text
S0_OLS: 0/144 positive (best ret=-0.3%/dd=0.5%)
S2_KSS: 74/76 positive (best ann=74.9%/dd=50.2%, best dd<=10: ann=2.6%/dd=6.3%)
S1_PC1/S3_delayed: 0/9 each
三档全部未命中
historical_backtest_only: true
```

## 1. R1 完整状态（33 tests PASS）

| Item | Status | Evidence |
|---|---|---|
| OrderIntent + filter rounding | ✅ | filter_order_conservative at FO/SO emit |
| Shared account | ✅ | sync_cycle_engine shared equity |
| Liquidation tier event-time | ✅ | LiquidationBufferTracker (R22 new) |
| Partial fill 25/50/75% stress | ✅ | partial_fill_stress_path (R22 new) |
| Legging delay/reject/hedge | ✅ | ConservativeReject types |
| Fees/funding/borrow | ✅ | sync_cycle_engine |
| Reserve next-SO+close+maint | ✅ | next_so_close_maintenance_reserve_ok |
| Reject cooldown persistent | ✅ | RejectionCooldown |
| Kill/restart/reconcile | ✅ | r21_canary_9 |
| Min liquidation buffer event-time | ✅ | LiquidationBufferTracker (R22 new) |
| Backtest-live parity | ✅ | verify_adapter_exchange_parity (R22 new) |
| 64 concurrency determinism | ✅ | r21_canary_11 |
| **Total tests** | **33 PASS** | 12+15+3+3 |

## 2. S2 KSS ann-DD 前沿

| ann | dd | config | tier |
|---|---|---|---|
| 2.6% | 6.3% | m=2.0 fo=30 @500U | (dd≤10 but ann<<50) |
| 4.2% | 9.9% | m=2.0 fo=50 @500U | (dd≤10 but ann<<50) |
| 15.4% | 24.6% | m=4.0 fo=100 @500U | |
| 49.5% | 41.3% | m=7.0 fo=200 @500U | |
| 74.9% | 50.2% | m=8.0 fo=300 @500U | |

ann≥50 只在 dd≥40% 时出现。**保守档 (ann≥50/dd≤10) 在当前参数空间不可达。**

## 3. G2 budget stress + 5 cold-start

### G2 budget stress (S2 KSS m=2.0 fo=30)

| Budget | ann | dd | positive |
|---|---|---|---|
| 500U | 2.6% | 6.3% | ✓ |
| 1000U | 1.3% | 3.3% | ✓ |
| 4999U | 0.3% | 0.7% | ✓ |

8/8 budgets positive. ann drops with larger budget.

### 5 cold-start: 5/5 positive (passes §1 4/5 mandatory)

## 4. 三档目标

| 档位 | 目标 | 状态 |
|---|---|---|
| 保守 | ann≥50%/dd≤10% | 未命中 |
| 平衡 | ann≥90%/dd≤20% | 未命中 |
| 激进 | ann≥110%/dd≤30% | 未命中 |

## 5. honest_disclosure

- historical_backtest_only = true
- R3 是 Python 实现（非 Rust production-conservative），但 R1 的 33 engine tests PASS
- S2 KSS 正 edge 是真实的，但 ann-DD 前沿决定了三档不可达
- 下一步需要：不同的 MR edge 来源或 DD 控制机制
