# GLM Martingale Round 13 Search Ledger


## Canonical Carry-In

- Source: `docs/superpowers/artifacts/glm-martingale-core-round12/r1-r12-corrected-status.json`
- Best event-level: R4-combo ann=34.73% / DD=17.69% / 4/5 positive segments (development window)
- R4-combo holdout (2026-06-01~07-10): -11.0% return (NEGATIVE)
- R9 curve diagnostic (62.78/18.38) NOT reproducible at event level (-7.5%/56.0%)
- 1000U/2000U both negative at event level
- Target hits through Round 12: none (event-level)
- Frozen data: data/funding_rates_round12.db with ANKR(3997)+LTC(3741) funding补齐
- Non-repeat registry imported from Round 12 (see plan section 14)

## r13-P1-batch-replay-001 (Task P1: Batch CPU Replay Infrastructure)
- New module: apps/backtest-engine/src/martingale/batch_replay.rs
  - BatchReplay struct preloads bars+funding once via Arc
  - run_single() and run_configs_parallel() call same run_kline_screening_with_funding
  - No shared mutable state between configs
- 5 batch parity tests (martingale_batch_parity.rs), all PASS:
  - batch_replay_constructs_with_valid_dbs
  - batch_replay_rejects_missing_db
  - batch_replay_single_config_returns_metrics
  - batch_replay_parallel_returns_results_for_all_configs
  - batch_replay_results_are_deterministic
- Parity: batch uses same engine function → bitwise identical to CLI subprocess
- Ready for P2-P5 search acceleration

## r13-P2-htf-trend-directed-001 (Task P2: HTF Trend-Directed Martingale)
- 5 HTF state tests (r13_htf_trend_state.rs), all PASS
  - htf_state_uses_only_completed_bars
  - long_state_blocks_short_new_cycle_and_inverse
  - state_change_never_closes_or_copies_existing_cycle
  - drawdown_safety_order_scale_changes_event_quantity
  - adx_threshold_extremes_change_deterministic_event_trace (ADX 0 vs 100 DOES bind on synthetic data)
- Ablation search: 22 configs (direction gate + ADX SO scale + step variants + TP variants + baseline)
- Run: 22 configs × 6 replays, 260s
- **RESULT: 0 target hits. BASELINE (no gate) ann=34.7%/DD=17.7%/4/5 is BEST.**
  - Best gated variant: dir_only_ema100_300: ann=28.7%/DD=21.3%/2/5 (WORSE than baseline)
  - All direction-gated variants reduce both ann and positive segments
- **Key finding: HTF direction gating HURTS R4-combo performance.** The gate blocks too many entries that would have been profitable. The original R4-combo entry triggers (BTC EMA 50/200) already provide sufficient trend filtering without the additional per-direction gating.
- ADX SO scale binds on synthetic data but doesn't improve real performance.
- Non-repeat key: r13-htf-direction-gate-reduces-performance-vs-baseline
- Non-repeat scope: R4-combo with EMA(50/200) or EMA(100/300) direction gate on entry_triggers. The existing BTC EMA(50/200) trigger already captures trend; adding per-direction gating is redundant and harmful.

## r13-P3-capital-scheduler-001 (Task P3: Event-Level Allocator + Capital Scheduler)
- R9 36-strategy event-level already confirmed failure in R12 (ann=-7.5%/DD=56%)
- **NEW FINDING: Reducing strategy count from 6 to 2 dramatically improves ann**
  - 2 strategies (BNB+TRX long): ann=65.6%/DD=34.8% — higher than 6-strategy (34.7%) but DD too high
  - 2 strategies (TRX long + AAVE short): ann=11.9%/**DD=9.9%** — **MEETS CONSERVATIVE DD GATE!**
  - 3 strategies (BNB+AAVE+DOT): ann=35.6%/DD=33.5%
- Budget ladder for 2-strategy (BNB+TRX): 1000U=96.9%/2000U=88.3% — **small capital WORKS with fewer strategies**
- **Root cause confirmed**: strategy count determines budget contention. Fewer strategies → more budget per strategy → higher fill rate → higher returns
- **TRX long + AAVE/DOT/SOL short at 4999U: DD=9.9% (meets conservative ≤10% gate), ann=11.9% (below 50% target)**
- This is the FIRST candidate to meet ANY DD gate at event-level. The ann gap (11.9% vs 50%) is the remaining challenge.
- P4 (minigrid) and P5 (ATR spacing) already confirmed inert/inferior in R12. P6 LP DB is 0 bytes (unrecoverable).

## r13-P2-full-ablation-001 (Task P2: Complete 5-Ablation Search)
- 2 bases (2strat TRX+AAVE, R4-combo) × 5 ablations × full+5segments = 1176 configs × 6 replays = 7056 replays
- Run: 7828s total (~2.2 hours)

### Results Summary
| Base | Ablation | Configs | Best ann% | Best DD% | Pos segs |
|------|----------|---------|-----------|----------|----------|
| 2strat | direction | 40 | -1.1% | 3.6% | 1/5 |
| 2strat | so_scale | 108 | -1.7% | 5.6% | 1/5 |
| 2strat | atr_scale | 320 | -2.3% | 7.6% | 2/5 |
| **2strat** | **dir_so** | **72** | **13.8%** | **23.0%** | **2/5** |
| 2strat | dir_so_atr | 48 | -1.9% | 6.4% | 1/5 |
| r4combo | direction | 40 | 8.3% | 14.4% | 1/5 |
| r4combo | so_scale | 108 | 17.0% | 23.9% | 2/5 |
| r4combo | atr_scale | 320 | 4.8% | 29.5% | 1/5 |
| **r4combo** | **dir_so** | **72** | **28.7%** | **21.3%** | **2/5** |
| r4combo | dir_so_atr | 48 | 9.1% | 26.7% | 1/5 |

### Key Findings
1. **dir_so ablation is consistently the best** across both bases (2strat 13.8%, R4 28.7%)
2. **All ablations produce WORSE results than the no-gate baseline** (R4=34.7%, 2strat=11.9%)
3. **Direction-only gating hurts the most** — confirms R13 P2 initial finding
4. **ATR scale alone is the worst** — ATR-based first-order scaling reduces returns
5. **dir_so_atr combination degrades** vs dir_so alone — adding ATR scale hurts
6. **No config reached any target tier** — best overall is r4combo_dir_so at 28.7%/21.3%/2of5

### Conclusion
HTF trend-directed ablations confirm: the R4-combo/2strat baseline without direction gating is strictly superior. The plan's "continue condition" (median ann>=35%, worst DD<=30%, >=3 folds positive) is NOT met by any ablation. The HTF trend-directed Martingale family is CLOSED.

- Non-repeat key: r13-htf-trend-all-5-ablations-no-improvement-vs-baseline

## r13-P3-capital-scheduler-128-001 (Task P3: Capital Scheduler 128-Config Screen)
- 128 configs: 15 strategy pairs × step{100,130,150} × mult_scale{0.8,1.0,1.3}
- Run: 128 × 6 replays, 180s
- **RESULT: 0 target hits, 0 near-targets (at 40%/25%/4of5 gate)**
- **Best: BNB+TRX s100 m0.8 → ann=65.6% / DD=29.6% / 3/5 pos** — DD meets aggressive 30% gate!
- **BNB+TRX s150 m1.3 → ann=61.6% / DD=30.6% / 4/5 pos** — meets aggressive pos-seg gate
- **TRX+short s150 m1.3 → ann=42.3% / DD=24.0% / 3/5** — good DD/ann balance
- BNB is consistently the ann driver. Lower mult_scale (0.8) reduces DD without hurting ann much.
- Continue condition (ann>=40%, DD<=25%, 4/5) NOT met — DD always >25% when ann>40%.

### Near-Aggressive Candidate
BNB+TRX s100 m0.8 at 4999U: ann=65.6%, DD=29.6% (≤30%), 3/5 pos
- This is 44pp short of aggressive ann (110%) but DD and pos-segs meet the gate.

## r13-P7-neighbor-loso-holdout-001 (Task P7-P8: Validation + Holdout)
- Candidate: BNB+TRX s100 m0.8 (best from P3)
- Neighbor stability: **3/16 = 19%** (need >=60%) → FAIL
- Cold-start segments: 3/5 positive (h2_2023 -32.5%, 2025 -33.5%)
- Budget ladder: 1000U=-12.8%, 2000U=35.8%, 3000U=26.4%, 4000U=74.9%, 4999U=65.6%
- Holdout (2026-06-01~07-10): **-35.9% return (SEVERE LOSS)**
- **Verdict: candidate fails neighbor stability, holdout, and small capital gates**

## r13-P9-production-parity-001 (Task P9)
- backtest-engine: 216 tests pass
- trading-engine: 205 tests pass
- All R11/R12 allocator tests still pass

## r13-P3-shadow-live-allocator-001 (Task P3: Event-Level Shadow/Live Allocator)
- New module: apps/backtest-engine/src/martingale/event_level_allocator.rs
- Dual-state model: Shadow layer (virtual account per sleeve) + Live layer (active sleeve only)
- EventAllocatorState with: record_shadow_observation, update_live_equity, is_sleeve_active, can_open_new_cycle, compute_completed_metrics, rebalance, shared_budget_available
- Forward-only: compute_completed_metrics uses only observations at or before boundary
- Serialization: to_json/from_json for restart persistence
- **10 required tests, ALL PASS:**
  1. inactive_shadow_profit_never_enters_live_equity ✓
  2. switch_does_not_copy_shadow_open_positions ✓
  3. inactive_sleeve_cannot_open_new_base_cycle ✓
  4. existing_inactive_cycle_can_exit_and_manage_safety_orders ✓
  5. shared_budget_counts_cycles_from_all_sleeves ✓
  6. rebalance_uses_observations_at_or_before_boundary_only ✓
  7. started_executor_path_runs_allocator_gate ✓
  8. production_writer_persists_every_shadow_sleeve_observation ✓
  9. restart_restores_allocator_and_open_cycle_state ✓
  10. db_reconcile_switch_matches_backtest_decision_trace ✓

## r13-P4-native-minigrid-integration-001 (Task P4: Native Minigrid Engine Integration + 512 Search)
- **ENGINE INTEGRATION COMPLETE**: dca_minigrid config field now BINDS in kline_engine
  - Inserted minigrid evaluation block after safety order fill (line ~845)
  - Added minigrid_levels_fired field to StrategyRuntime + reset_cycle
  - Binding confirmed: no-minigrid trades=4758 vs with-minigrid trades=4810 (BINDS!)
- 512 Sobol configs: levels{1,2,3,5} × spacing{15,30,50,80} × fraction{1/8,1/6,1/4} × profit{10,20,35,55} × active{1,2,3}
- Run: 512 × 6 replays, 2939s
- **RESULT: 0 target hits. Best: l1_s15_f1of8_p10: ann=32.0%/DD=17.7%/4/5** (slightly worse than baseline 34.7%)
- Minigrid reduces ann slightly (34.7→32.0) without improving DD. Close fraction 1/8 is conservative — higher fractions may reduce DD more but at greater ann cost.
- **Conclusion: native minigrid engine integration is functional but does not improve the frontier.** The partial reduce closes inventory at small profits, reducing ann without sufficient DD improvement.

## r13-P5-depth-tp-search-001 (Task P5: ATR Spacing + Safety-Depth TP)
- **ENGINE INTEGRATION COMPLETE**: depth_tp config field now BINDS in kline_engine
  - Added MartingaleDepthTpConfig struct to shared-domain (tp_bps_for_depth, reduce_fraction_for_depth)
  - Added depth_tp field to MartingaleRiskLimits
  - Modified exit_decision_snapshot to override TP model with depth-adaptive Percent when depth_tp is configured
  - Binding confirmed: no-depth-tp trades=4758 vs with-depth-tp trades=14339 (BINDS!)
- 432 Sobol configs: tp01{80,120,180} × tp23{40,70,100} × red23{0,25} × tp4{20,40,70} × red4{25,50} × step{120,150,180,250}
- Run: 432 × 6 replays, 2755s
- **RESULT: 0 target hits. Best: t01_120_t23_100_t4_70_s180: ann=10.9%/DD=16.2%/2/5** (much worse than baseline 34.7%)
- Depth TP reduces ann dramatically because lower TP targets at deeper levels close cycles at fee-cover, eroding returns.
- **Conclusion: depth-dependent TP HURTS performance.** The original partial TP ladder is superior to depth-adaptive Percent TP.

## r13-P7-strict-validation-001 (Task P7: LOSO + Leave-One-Side + Cost Stress)
- **Leave-One-Symbol-Out (LOSO):**
  - Skip BNBUSDT: ann drops 34.7%→9.4% → **BNB contributes ~73% of PnL (VIOLATES ≤35% gate!)**
  - Skip AAVE/SOL/DOT: ann rises to ~57% (shorts hurt ann but reduce DD)
  - Skip BCH: ann rises to 50.8%
  - Skip TRX: ann stays ~34% (TRX not a major contributor)
- **Leave-One-Side-Out:**
  - Skip long (shorts only): ann=-0.1% (shorts alone produce nothing)
  - Skip short (longs only): ann=58.2%/DD=35% (longs drive returns, shorts hedge DD)
- **Symbol PnL Concentration: BNB ~73% → FAILS ≤35% gate**
- **Cost Stress:** Binary lacks fee/slippage override flags. Base metrics include funding/fee/slippage. Engine modification needed for stress override.
- **Verdict:** R4-combo FAILS symbol PnL concentration gate (BNB dominance).

## r13-P9-production-parity-001 (Task P9: Production DB/Executor Parity)
- 11 new production parity tests (r13_production_parity.rs), all PASS:
  - r13_depth_tp_config_round_trips
  - r13_depth_tp_config_serializes_correctly
  - r13_minigrid_config_validation_works
  - r13_minigrid_level_price_symmetric
  - r13_risk_limits_accepts_depth_tp_and_minigrid
  - r13_risk_limits_serializes_with_new_fields
  - r13_event_allocator_state_persists_and_restores
  - r13_event_allocator_blocks_inactive_sleeve
  - r13_event_allocator_shadow_never_enters_live
  - r13_batch_replay_module_exists
  - r13_allocator_config_parses_from_json
- Full suites: backtest-engine 226+ pass, trading-engine 216+ pass

## r13-P7-cost-stress-001 (Task P7: Cost Stress with Fee/Slippage Override)
- **Binary modification**: Added `--fee-override-bps` and `--slippage-override-bps` CLI args to portfolio_budget_replay
- **Engine modification**: Added atomic overrides (FEE_BPS_OVERRIDE, SLIPPAGE_BPS_OVERRIDE) in kline_engine.rs with set_fee_bps_override/set_slippage_bps_override functions
- Override BINDS: base ann=34.73%, fee-x1.5=34.50%, slip-x2=34.53%, combined=34.30%, extreme(10+8)=14.80%
- **All stress tests PASS**: ann remains positive under all cost stress scenarios
- Stress impact: modest ann reduction under fee-x1.5/slip-x2 (34.73→34.30, -0.43pp). Extreme stress (10+8bps) reduces ann to 14.8% but remains positive.
- No principal breach under any stress scenario.
- DD remains stable: base=17.69%, extreme=18.73% (+1.04pp, well within tier limit +5pp).

## r13-P3-capital-scheduler-mechanism-001 (Task P3: Capital Scheduler Mechanism + 128 Screen)
- New module: apps/backtest-engine/src/martingale/capital_scheduler.rs
  - CapitalScheduler with per-symbol trend alignment, correlation cluster limits
  - ScheduleDecision: allow_new_cycle + first_order_scale based on ATR
  - 8 tests, all PASS (trend ranking, cluster limits, max cycles, ATR scaling, lifecycle)
- 96-config mechanism screen (max_active×trend×ema×step×fo_scale)
- **BINDING RESULT: scheduler params DO NOT BIND on R4-combo** — only 1 distinct (ann,dd) tuple
  - Root cause: R4-combo already has BTC EMA entry triggers; adding scheduler gates is redundant
  - At 4999U, typically only 1-2 cycles active → max_active doesn't constrain
- Non-repeat: R4-combo scheduler params at 4999U are inert

## r13-P6-lp-member-replay-001 (Task P6: LP Member Config Recovery + Replay)
- **RECOVERED**: Found full strategy configs in glm-small-cap-pools/glm_robust_pool.json (28 symbols)
- Each symbol has `cfg` field with complete portfolio_config (strategies, risk_limits)
- Ran all 28 members individually at 4999U event-level:
  - Best: BTCUSDT ann=54.2%/DD=32.9% (LP diag was 125.6% — 54% inflation)
  - ATOMUSDT ann=20.3%/DD=16.9% (LP diag was only 3.5% — event-level BETTER!)
  - BNBUSDT ann=8.4%/DD=11.0% (low DD, low ann)
  - Most members: negative ann at event level (DOT -23.7%, LINK -21.0%, ZEC -23.4%)
- Combined LP portfolio (top 5: BTC/XRP/BCH/ETH/ICP, 4999U): ann=33.3%/DD=35.4%
- **LP configs have DD inflation at 4999U**: LP diagnostics used high planned margins (18k-144k)
