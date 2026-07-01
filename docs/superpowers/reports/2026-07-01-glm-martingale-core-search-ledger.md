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

