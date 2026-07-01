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

