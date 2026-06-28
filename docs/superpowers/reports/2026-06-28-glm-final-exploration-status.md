# 2026-06-28 GLM Final Exploration Status (Most Comprehensive)

## Implementation Summary (3 New Code Features)

### 1. Cross-Symbol Indicator Reference (`indicator_runtime.rs`)
- Syntax: `BTCUSDT.ema(50)`, `btcusdt.close`, `BTCUSDT.bb_bandwidth(20, 2)`
- Lets any strategy reference any symbol's indicators for regime filtering
- Live-parity: trading-engine reuses the same module (zero duplication)
- 4 new unit tests, all 168 existing tests pass

### 2. `atr_percent` Indicator (`indicator_runtime.rs`)
- New operand: `atr_percent(14)` = ATR/close*100 (volatility as % of price)
- Enables volatility-regime filters without arithmetic in expressions

### 3. ATR Spacing Warmup Fix (`rules.rs`)
- ATR spacing no longer errors when ATR is unavailable (warmup period)
- Falls back to `min_step_bps` until ATR becomes available
- Safe, conservative behavior; all 11 rules tests pass

## Strategy Exploration Summary (~600 Portfolio Replays)

### Approaches Tried
1. **Parameter sweeps**: multiplier, weight, legs, TP, SL, spacing, cooldown, leverage (v2-v16)
2. **Entry filters**: RSI, EMA, SMA, ADX, BB bandwidth, atr_percent, time windows
3. **Cross-symbol regime**: BTC trend (up/down), BTC volatility, ETH trend, BTC+ETH combo
4. **Core+satellite**: INJ core + TRX/GALA/ADA/DYDX/NEAR satellites with BTC+RSI filters
5. **Dynamic pool + max_active**: user's idea, tested with native params, uniform params, fast rotation
6. **BudgetScaled sizing**, partial investment, compound portfolios
7. **Mixed leverage**, booster strategies, alternative engines
8. **ATR-based spacing** with warmup fix

### Key Discoveries
1. **DD is temporal**: aggressive 0105 made +1996% in 2023, lost ground in 2024-2026
2. **BTC shortdown filter** reduces DD by ~8 points (33% → 25%)
3. **Core+satellite** reduces DD by ~14 points (18% → 10.7% for conservative)
4. **Phase transition**: ann/DD frontier has a sharp cliff controlled by ladder truncation
   - Self-truncated: ann ~30-35%, DD ~10-13%
   - Full-ladder: ann ~55-100%, DD ~24-30%
   - No middle ground (ann 35-55% at DD 13-24%)

## Best Candidates (All Verified)

| Profile | Gate | Best Result | Gap |
|---|---|---|---|
| Conservative | ann>50%, DD<=10% | 32.94%/10.72% | DD -0.72, ann +17.06 |
| Balanced | ann>90%, DD<=20% | 99.73%/23.77% | DD +3.77 (ann far exceeds) |
| Aggressive | ann>110%, DD<=30% | 133.54%/29.88% | ✅ PASS |

### Conservative Config
`docs/superpowers/artifacts/glm-conservative-candidate/best_conservative_core_sat_b5000.json`
- INJ long (w~60, m3.5, tp320) + 5 satellites (TRX/GALA/ADA/DYDX/NEAR) + FIL short with BTC filter
- Env: `MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT=0.5`
- 2466 trades, 630 stops, maxcap 1751/5000

### Balanced Config
`docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_btc_shortdown_b5000.json`
- INJ long (w48, m3.5, l6) + FIL/AAVE dynsafe + FIL short with BTC filter
- Env: `MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT=0.5`
- 13518 trades, 5408 stops, maxcap 3474/5000

## Not Merged
All code changes (`indicator_runtime.rs`, `rules.rs`) and artifacts remain in the working tree for ChatGPT review.
