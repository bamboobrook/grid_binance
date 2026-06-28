# 2026-06-28 GLM Core+Satellite + BTC Regime Filter - Final Status

## Major Advances This Session

### 1. Cross-Symbol Indicator Reference (New Code Feature)
Implemented in `apps/backtest-engine/src/martingale/indicator_runtime.rs`:
- `BTCUSDT.ema(50)` syntax for cross-symbol indicator references
- `atr_percent(14)` operand for volatility-regime filters
- Live-parity: trading-engine reuses the same module (zero duplication)
- All 168 existing tests + 4 new tests pass

### 2. BTC Regime Filter (New Strategy Mechanism)
- `BTCUSDT.close > BTCUSDT.ema(30)` filter on long entries
- `BTCUSDT.close < BTCUSDT.ema(30)` filter on short entries (counter-cyclical hedge)
- Reduced balanced DD from 33% → 23.77%

### 3. Core+Satellite Architecture (User's Idea Realized)
Based on the user's suggestion: a pool of multiple symbols where satellites provide diversification.
- **Core**: INJ long (dynsafe family, the proven 2023 return engine)
- **Satellites**: TRX/GALA/ADA/DYDX/NEAR longs with BTC+RSI filters (symbols that grew during INJ's 2023 correction)
- **Hedge**: FIL short gated on BTC downtrend (only hedges when BTC dumps)
- Result: DD dropped from ~24% to ~10.7% (conservative territory!)

## Best Candidates (All Verified Full-Period)

### Balanced (ann>90%, DD<=20%)
- **Config**: `docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_btc_shortdown_b5000.json`
- **Result**: **99.73% / 23.77%** (env: `MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT=0.5`)
- Ann far exceeds 90%. DD needs -3.77 to pass.
- DD source: 2023 mid-year correction (2023-04-18 peak to 2023-06-15 trough)

### Conservative (ann>50%, DD<=10%)
- **Config**: `docs/superpowers/artifacts/glm-conservative-candidate/best_conservative_core_sat_b5000.json`
- **Result**: **32.94% / 10.72%** (env: `MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT=0.5`)
- DD nearly passes (needs -0.72). Ann needs +17.06.
- Structure: INJ long (w~60, m3.5, tp320) + 5 satellites (TRX/GALA/ADA/DYDX/NEAR) + FIL short with BTC filter

### Aggressive (ann>110%, DD<=30%) — STILL VALID
- 133.54% / 29.88%, budget 3250U

## The Persistent Frontier

The core finding from ~500+ portfolio replays: there's a **phase transition** in the ann/DD frontier controlled by whether the INJ martingale ladder self-truncates at the budget cap:

| Regime | Ann range | DD range | blocked_legs |
|---|---:|---:|---:|
| Self-truncated (low DD) | ~30-35% | ~10-13% | 40-75 |
| Full-ladder (high ann) | ~55-100% | ~24-30% | 120-160 |

The middle ground (ann 35-55% with DD 13-24%) is empty. Every lever tried (TP, SL, multiplier, legs, cooldown, RSI/EMA/ADX filters, BTC regime, satellite diversification, BudgetScaled, leverage, time windows) cannot break this cliff.

## What Was Tried (Comprehensive)
- Parameter sweeps: multiplier, weight, legs, TP, SL, spacing, cooldown, leverage
- Entry filters: RSI, EMA, SMA, ADX, BB bandwidth, atr_percent, time windows
- Cross-symbol regime: BTC trend (up/down), BTC volatility, ETH trend, BTC+ETH combo
- Portfolio structures: core+satellite, dynamic pool with max_active, BudgetScaled, mixed leverage
- Env guards: DD pause, ATR pause, portfolio equity stop, max active cycles
- Alternative engines: LTC, LINK, AAVE, GALA, DYDX, NEAR, TRX as primary
- Multi-engine combos: INJ+LTC, INJ+LINK, 3-engine, 5-engine

## Not Merged
All code changes (indicator_runtime.rs) and config artifacts remain in the working tree for ChatGPT review. Nothing merged to main.
