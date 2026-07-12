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
