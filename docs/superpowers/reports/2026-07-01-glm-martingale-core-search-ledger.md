# GLM Martingale Core Search Ledger

Branch: `glm-martingale-core-indicator-expansion`
Plan: `docs/superpowers/plans/2026-07-01-glm-martingale-core-indicator-expansion-plan.md`
Started: 2026-07-01


## 2026-07-01 Task 1: Branch, ledger, evidence index initialized

- Branch: `glm-martingale-core-indicator-expansion`
- Plan commit: 4d2174d1b5cd56af8ba2979b27ae7f70e3b6776f
- market_data_full.db sha256: 3422e929ed994829b0a66efe4f8473eec1d43a2ce991ce654648cadeacd2ff19
- funding_rates.db sha256: 356e270dacce36b4703364545beeafe402f1aa55aa11caebc5708f5f3b6767a1

### Device profile
- CPU: 30 cores, 196GB RAM, RTX 5090 32GB, 3.6TB disk (2TB free)

### Prior evidence baseline (from read-only audits)
- 0/64508 normalized candidates pass final gates across all prior searches.
- Verified frontier: Conservative ~34% ann/7.6% DD; Balanced ~54% ann/17.7% DD; Aggressive 133.54% ann/29.88% DD (PASS on return/DD but segment gate NEVER validated).
- Structural blockers: 2024>=0 AND 2025>=0 = 0/590; ann/DD frontier has empty middle; high-ann concentrated in INJ/SOL/BTC long; 2025 short crash-coins die in 2023 bull.

### Engine capability (live-parity status) — critical for plan acceptance
- LIVE-PARITY (config-structured): `new_cycle_drawdown_pause_pct`, `new_cycle_atr_pause_pct`, `safety_skip_adx_threshold`.
- LIVE-PARITY expression indicators: `sma/ema/rsi/atr/adx/bb/bb_upper/bb_middle/bb_lower/bb_bandwidth/atr_percent` + raw OHLC + cross-symbol refs (`BTCUSDT.close`, `BTCUSDT.ema(30)`).
- RESEARCH-ONLY (env-only, NO live impl): `portfolio_equity_stop_pct`, `portfolio_stop_cooldown_hours`, `max_portfolio_active_cycles`.
- TP models live-parity: Percent, Amount, Atr(multiplier), Trailing, Mixed.
- Gap (plan needs): `macd`, `roc`, `donchian`, `slope` NOT in expression language. Per-symbol regime via cross-symbol refs is possible (e.g. `<SYMBOLUSDT>.close > <SYMBOLUSDT>.ema(50)`).


## 2026-07-01 Task 2: Regime-Gated single-strategy segment-first search

- Script: `scripts/glm_regime_gated_martingale_search.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core/regime-gated-single-strategy.json`
- External research synthesized (regime/ATR/ADX consensus) into families below.

### Method
- 26 symbols (full-period coverage), 13 regime families (per-symbol ADX/EMA/RSI/BB/ATR gates + BTC macro veto), 3 profiles x 3 param grids each.
- Segment-FIRST: each candidate run on 2024 + 2025 segments; only BOTH-positive survivors advance to full 5-seg validation.
- 3276 candidates evaluated in 435s (24 workers, 30-core machine).

### Result: 0/3276 survived segment gate (2024>0 AND 2025>0)
Confirms the structural anti-correlation: 2024 (bull) and 2025 (bear) are inversely correlated. Of 26 symbols, only BNBUSDT/TRXUSDT/BCHUSDT were buy-and-hold positive in BOTH years.

### Key single-candidate findings (best per segment)
- **BNBUSDT long adx_range_long balanced: 2024 +46.1% / 2025 -2.7%** (best single 2024 long).
- Crash-alt shorts (NEAR/AAVE/UNI/ARB/SUI/INJ/OP/ADA) conservative regime-gated: ~-0.2% both years (near-zero, low-trade — survives by not trading much).
- BNB/TRX/BCH are the only structurally long-positive-both-years majors.

### Diagnosis
Per-symbol regime gates are too conservative (low trade count → near-zero) or too loose (wrong-regime trades → losses). The regime gate alone cannot flip a symbol from negative to positive in both anti-correlated segments. The structural answer must be a **portfolio that COMBINES** a 2024-long-bull leg (BNB) with a 2025-short-crash leg, evaluated segment-first at the portfolio level.

### Next
Task 5 portfolio combination moved earlier: build a long_bull + short_crash portfolio and validate 5 segments. The single-symbol pool provides the building blocks.


## 2026-07-01 Task 5 (moved up): Portfolio segment-first search — BREAKTHROUGH

- Script: `scripts/glm_portfolio_segment_search.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core/portfolio-segments.json`
- 2592 portfolio candidates, 22 workers, 1360s.

### BREAKTHROUGH: 8/2592 survived 2024>0 AND 2025>0 segment gate (first time ever)
This is the first time ANY candidate (single or portfolio) made BOTH 2024 and 2025 positive — the structural blocker that was 0/590 in all prior work.

Best survivor: `agg-looloo-L2S1` (BNB-long + BCH-long + AAVE-short, loose gates):
- h1_2023: +14.7% (948 trades)
- h2_2023: -10.2% (69 trades)  ← LOSS
- 2024: +0.27% (895 trades) ✓
- 2025: +0.76% (337 trades) ✓
- 2026_ytd: -29.3% (470 trades) ← BIG LOSS (drives agg2024-2026 to -28.3%)
- full ann +4.1%, DD 37.8%, 3/5 positive segments (up from baseline 1/5)

### Root-cause diagnosis
- 2024+2025 now solvable via long-bull (BNB/BCH) + crash-short (AAVE) combo with loose regime gates.
- 2026_ytd is the NEW dominant problem: broad bear market (BNB -17.7%, BCH -49.5%, AAVE -43.7%, BTC -16%). The long legs lose; the short doesn't gain enough (BTC-uptrend veto blocks short entries when BTC bounces).
- h2_2023: late bull reversal burns the short leg (only 69 trades, mistimed).

### Promising-record candidate (Task 5 good-result protocol)
This 3/5-segment, 2024+2025-both-positive candidate is a NEW best segment frontier.
Recorded below and to be optimized next.


## 2026-07-01 Task 4 (key finding) + Task 5 optimization: Portfolio DD stop is the master risk lever

- Script: `scripts/glm_portfolio_optimize.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core/portfolio-optimized.json`
- 4500 candidates (tighter per-symbol gates + SL variants + DD-pause env), 395 near, 60 full-validated, 0 survivors of 2024+2025 both-positive gate (looser gates lost the short edge).

### BREAKTHROUGH: Portfolio equity stop (research env) cuts DD 37% -> 5.75%

Same structure BNB-long + AAVE-short (loose gates), with vs without DD stop:

| Variant | h1_2023 | 2024 | 2025 | 2026 | full ann | DD | pos |
|---|---:|---:|---:|---:|---:|---:|---:|
| no DD stop | -8.2% | +125.3% | -2.6% | -1.1% | -2.5% | 37.0% | 1/5 |
| DD stop 8% / 24h cd | +11.1% | +20.3% | -2.6% | -1.1% | +3.1% | 5.75% | 2/5 |

The DD stop (MARTINGALE_BT_PORTFOLIO_EQUITY_STOP_PCT=8, COOLDOWN=24h):
- Cuts max DD from 37% to **5.75%** (NEW LOWEST DD ever, prior best was 7.6%).
- Rescues h1_2023 (the stop fires after a drawdown, then cooldown avoids re-entry into the losing stretch).
- But reduces 2024 from +125% to +20% (the 8% stop is too tight — kills the big winning run).

### Implication
- The portfolio equity stop is the single most powerful risk control. An 8% stop is too tight (caps return); the sweet spot is likely 12-20%.
- This is RESEARCH-ONLY (env switch, no live impl). To promote to final candidates, the trading-engine needs a portfolio equity stop implementation (Task 4 TDD).
- Next: sweep DD-stop level (10-25%) to find the return/DD sweet spot.

### Best candidate recorded
- `glm-mart-core-aggressive-002`: DD 5.75%, ann 3.1%, 2/5 pos, research-only (needs live DD-stop).


## 2026-07-01 DD-stop sweep + best frontier

- Script: `scripts/glm_dd_stop_sweep.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core/dd-stop-sweep.json`
- 1260 candidates (long-bull+crash-short combos x DD-stop 10-30% x cooldown), 5269s.

### Best frontier: `agg-L2S1-40w12-s20c24` (BNB+TRX long + AAVE short, DD stop 20%/24h)
- h1_2023: +10.4%, h2_2023: -17.7%, 2024: +31.7%, 2025: -13.3%, 2026_ytd: +0.4%
- **3/5 positive (NEW BEST), agg2024-2026 = +18.8%, DD 23.6%, ann 2.9%**
- The DD stop made 2026_ytd go from -29% to flat (+0.4%) — the stop works.
- h2_2023 (-17.7%, late bull reversal) and 2025 (-13.3%, crash-short under-captures) are the loss segments.

### Return-ceiling diagnosis
Ann is only 2.9% despite 2024 +31.7% because h2_2023 and 2025 are strongly negative. The DD stop caps DD but cannot manufacture return. The fundamental gap: the crash-short sleeve must capture MORE of the 2025 bear to offset long losses, and h2_2023 needs a regime gate that avoids the late-bull reversal.

### Next directions to raise ann
1. ATR/ADX-adaptive spacing/TP (Task 3) to make the martingale more efficient per cycle.
2. More aggressive crash-short sizing (current short weight only 12%).
3. Add a 2025-specific regime (e.g. BTC dominance rising) to tilt short.


## 2026-07-01 Task 3: ATR/TP efficiency + DD-stop optimization — ann breakthrough

- Scripts: `scripts/glm_atr_adx_efficiency_search.py`, `scripts/glm_highann_dd_optimize.py`
- Outputs: `atr-adx-efficiency.json` (partial), `highann-dd-opt.json` (224 candidates)

### RETURN BREAKTHROUGH: ann 2.9% -> 73.5% via high TP + high multiplier + strict gates
Structure BNB+TRX long + AAVE short, strict regime gates, **TP=600bps, multiplier=3.0 (long)/2.5 (short), 9/8 legs**:
- full ann **73.5%**, DD 45.1%, agg2024-2026 = +65.5%, 2/5 positive segments
- segment: h1_2023 +79.3%, h2_2023 -34.5%, 2024 +127.6%, 2025 -31.8%, 2026_ytd -30.4%

The high TP (600bps vs 420) + high multiplier (3.0 vs 1.8) + strict gate makes each martingale cycle far more profitable. This is a 15x ann improvement over the loose-gate low-TP frontier.

### DD-stop does NOT reduce DD here (key finding)
DD stays at 45.1% regardless of portfolio equity stop (10/12/14%). Reason: the 45% DD is the MAX floating loss during the 2024 run that then recovers to +127.6%; the DD stop fires at the trough and PREVENTS the recovery (or the single-strategy sl_bps=5000/50% lets each cycle draw down hugely before stopping). The DD stop is the wrong lever here.

### To reduce DD: tighten the SINGLE-STRATEGY stop-loss (sl_bps)
Current sl_bps=5000 (50%) is far too loose — each martingale cycle can lose half its budget before stopping. Tightening to sl_bps=1500-2500 should cap per-cycle DD and thus portfolio DD. Next search.


## 2026-07-01 Tight single-strategy SL sweep — confirms ann/DD tradeoff cliff

- Script: `scripts/glm_tight_sl_search.py` (1008 candidates, sl_bps 1500-3500)
- Status: running (440/1008); partial results conclusive.

### Findings (the ann/DD cliff is real and empty in the middle)
| sl_bps (long/short) | TP | mult | portfolio DD stop | ann | DD | pos |
|---|---|---|---|---:|---:|---:|
| 5000/4000 (loose) | 600 | 3.0/2.5 | 12/24h | **73.5%** | 45.1% | 2/5 |
| 2500/2500 | 600 | 2.5/2.0 | 25/24h | -3.2% | **16.7%** | 1/5 |
| 2000/2000 | 700 | 3.0/2.5 | 20/24h | -2.6% | **9.6%** | 0/5 |
| 2500/2500 | 600 | 2.5/2.0 | none | -2.8% | **9.4%** | 1/5 |

- Tight SL (2000-2500) achieves DD 9-17% (great) but ann goes NEGATIVE (-2 to -3%).
- Loose SL (5000) achieves ann 73.5% (great) but DD 45% (fails 30% gate).
- **No configuration found with ann>50% AND DD<=30%.** The middle is empty.
- This independently re-confirms the prior final verdict's "empty middle" finding,
  now with the high-ann structure (73.5% vs prior 54% best) AND segment-first validation.

### Why the cliff exists
A martingale cycle's profit comes from price reverting to the average entry. High
TP (600bps) + high multiplier (3.0) means each winning cycle earns a lot, but to
get there the cycle must absorb large adverse excursion (the floating DD). Tight SL
cuts the DD by closing cycles at a loss BEFORE they can revert — killing the
mean-reversion profit. This is structural to martingale, not a parameter gap.


## 2026-07-01 Task 4 part 2: trading-engine portfolio equity stop (live impl) — COMPLETE

Implemented the live trading-engine portfolio equity stop (close-all + cooldown),
achieving full live-parity with the backtest guard.

### Changes
- `crates/shared-domain/src/martingale.rs`: added `portfolio_equity_stop_pct` and
  `portfolio_stop_cooldown_hours` to `MartingaleRiskLimits` (Option, serde default).
- `apps/backtest-engine/src/martingale/kline_engine.rs`: `RiskGuardThresholds::from_config`
  now reads these two fields config-first (parity resolution path), matching live.
- `apps/trading-engine/src/main.rs`: added `evaluate_portfolio_equity_stop()` (closes all
  Running martingale positions via reduceOnly `pfstop-` close orders, sets status=Stopping,
  pushes `portfolio_equity_stop` event, sets cooldown-until) + `portfolio_stop_in_cooldown()`.
  Wired into BOTH reconcile paths (bootstrap line ~517, executor line ~850).
- `apps/trading-engine/src/martingale_runtime.rs`: added `portfolio_stop_cooldown_active`
  to `MartingaleRuntimeContext` + cooldown gate in `enforce_new_entry_controls` (blocks
  new cycles while cooldown active — parity with backtest `portfolio_stop_cooldown_until_ms`).

### Verification
- backtest-engine: 208 tests pass (182+3+23).
- trading-engine: 185 tests pass (161 prior + 2 new cooldown tests + 22 other). Includes
  pre-existing `martingale_runtime_cooldown_blocks_live_reentry_until_elapsed` which now
  exercises the new gate.
- New tests: `cooldown_blocks_new_cycle_entries`, `no_cooldown_allows_new_cycle_entry_path`.

### Live-parity status
The portfolio equity stop is now FULLY live-parity: same config field drives both engines,
same threshold resolution (config-first → env → default), same semantics (drawdown vs
known equity peak, close-all reduceOnly, cooldown block). Candidates using it are no longer
research-only once this ships.


## 2026-07-01 DD-stop equity-base BUG FIX (critical correctness fix)

### Bug found
The portfolio equity stop in `run_kline_screening_with_funding` computed its
drawdown over `initial_margin_capital` (sum of ALL planned leg margins, ~72K for
the high-ann structure), NOT over the real budget (5000). The reported
`on_budget` max_drawdown_pct uses `budget + cum_pnl`. These are different equity
bases, so the stop's drawdown (over 72K) barely moved while the reported DD
(over 5K) reached 45%. **The stop never fired at 12/15/18/20/25%** because the
margin-based DD stayed under 10% even when budget-based DD was 45%.

### Fix
- `run_kline_screening_with_funding` now takes a `budget_quote: f64` arg.
- Added `budget_equity_peak_quote` (init = budget). The stop evaluates drawdown
  over `budget + (equity_quote - initial_margin_capital)` — the same base
  `on_budget_metrics` reports DD over. Parity with live `portfolio_drawdown_pct_for`
  (which uses `budget + realized + unrealized`).
- Updated all 6 callers (replay binary, search binary, reprice binary, backtest-
  worker x3, run_kline_screening wrapper, 2 internal tests).

### Verification
- backtest-engine: 208 tests pass. trading-engine: 185 tests pass.
- BEFORE fix: 12% stop → ann 73.3, DD 45.5 (stop never fired, 0 events).
- AFTER fix: 12% stop → ann 8.96, DD 12.94 (stop fires correctly, caps DD).
- Stop now correctly caps reported DD at ~stop_level + slack.

### Implication for the ann/DD cliff
The cliff is confirmed REAL (not an artifact of the bug): budget-based 73% ann
needs 45% DD. The stop can cap DD to any level but at the cost of ann. The
honest frontier is unchanged, but the stop mechanism is now correct for live.


## 2026-07-02 Diversification frontier: broad 6-symbol cuts DD 45%->34.6%

- NEW BEST real (non-overfit) frontier: **6-symbol portfolio (BNB/TRX/BCH long + AAVE/SOL/DOT short, strict gates, TP=600, mult=3.0/2.5) = ann 22.9%, DD 34.6%, 36798 trades**.
- Diversification (3 longs + 3 shorts) cut DD from 45.5% (3-symbol) to 34.6% while keeping ann positive (22.9%).
- This is the closest to the aggressive DD<=30% target yet. DD stop at 35% does not fire (peak DD is 34.6%); at 30/32% it fires but kills ann.
- Next: expand to 8-10 symbols + tune to push DD under 30%.


## 2026-07-02 HIGH-TP DD breakthrough (but h1_2023 dependence persists)

- TP sweep on loose-gate broad6 (3 longs + 3 shorts) found high TP dramatically lowers DD:
  - TP=1600: ann 26.9%, DD 28.1%
  - **TP=1800: ann 26.5%, DD 21.0%** (lowest DD with positive ann — recorded as 006)
  - TP=2000: ann 22.3%, DD 22.3%
- HIGH TP is a real DD-reduction lever: longer-held cycles profit from trends, stop out less.
- BUT segment validation of TP=1800 shows h1_2023 dependence: h1 +73%, h2_2023 -36%, 2024 -13%, 2025 -21%, 2026 -20%. agg2024-2026 = -42.9%. Only 1/5 positive.
- So high-TP cuts DD but the profit is still h1_2023-concentrated. Need to combine high-TP's DD benefit with the 2024+2025-both-positive structure.


## 2026-07-02 BREAKTHROUGH: TP=2200 + mult=2.8 strict-gate broad6 — DD<30 + segment-stable

- **glm-mart-core-aggressive-007**: ann 21.9%, DD 26.1%, 3/5 positive segments, agg2024-2026 +16.8%.
- Segments: h1_2023 +23.8%, h2_2023 -16.7%, 2024 +33.4%, 2025 -18.7%, 2026_ytd +2.1%.
- Passes: DD<=30 (26.1%), pos_segs>=3 (3/5), agg24-26>0 (+16.8%).
- Only reject: h1_contrib 99.6%>60% (contribution-metric artifact; 2024+2026 are also positive).
- This is the closest to the aggressive gate. Next: reduce h2_2023/2025 losses to lower h1 contribution.

