# 2026-06-28 GLM Cross-Symbol Regime Filter Breakthrough

## Major Implementation: Cross-Symbol Indicator Reference

GLM implemented a new feature in `apps/backtest-engine/src/martingale/indicator_runtime.rs` that allows an `indicator_expression` entry trigger to reference a DIFFERENT symbol's indicators. This enables true market-regime filtering (e.g., an INJ strategy can gate entries on BTC's trend state).

### Syntax
- `ema(50)` — strategy's own symbol (unchanged, backward-compatible)
- `BTCUSDT.ema(50)` — reference BTC's EMA from any strategy
- `btcusdt.close` — case-insensitive symbol prefix, works for OHLC too
- `BTCUSDT.bb_bandwidth(20, 2)` — works for all indicator types

### Live-Parity
The trading-engine (`apps/trading-engine/src/martingale_runtime.rs:205-208`) imports and reuses `backtest_engine::martingale::indicator_runtime` directly (via `apps/trading-engine/Cargo.toml:14` path dependency). So the change automatically applies to the live path. The only live-parity gap is data subscription: a referenced foreign symbol (BTCUSDT) needs its bars pushed into the indicator context, which is solved by adding a tiny "observer" strategy (weight 0.5%, wide stop) for that symbol in the portfolio config.

### Tests
- All 168 existing backtest-engine tests pass
- 3 new unit tests added for cross-symbol references (case handling, OHLC lookup, missing-symbol graceful degradation)

## Breakthrough Results: BTC Regime Filter

### Key Discovery: DD is Temporal
The aggressive 0105 made +1996% in 2023 but lost ground in 2024-2026 (-14% in 2025). The DD comes from continued trading in the post-2023 choppy/bearish period. A BTC downtrend filter on the hedge (short) legs eliminates most of this 2024-2026 DD.

### The "BTC Shortdown" Filter
The most effective configuration gates SHORT entries on BTC being in a DOWNTrend (`BTCUSDT.close < BTCUSDT.ema(30)`). This means the FIL short hedge only activates when BTC is dumping — exactly when it's needed. Combined with the ATR_PAUSE 0.5 env tweak:

| Config | Ann | DD | Budget | Notes |
|---|---:|---:|---:|---|
| `v11_injl48_m35_l6_shortema30_atrpause0.5` | **99.73%** | **23.77%** | 5000 | **BEST BALANCED** — ann far past 90, DD +3.77 over |
| `v13_injl48_m35_shortema30_ethdown_atrpause05` | 100.37% | 23.79% | 5000 | ETH downtrend added (no real gain) |
| `v11_injl49_m35_l6_shortema30` | 99.98% | 24.10% | 5000 | No atrpause |
| `v10_injl48_m3.5_l6_shortdown` | 99.42% | 24.27% | 5000 | No atrpause, original shortdown |

### DD-Passing Configurations (dd<=20)
| Config | Ann | DD | Notes |
|---|---:|---:|---|
| `v14_injl65_m3.0_shortema25_rsi60_atrpause05` | 57.61% | 17.43% | DD passes, ann needs +32 |
| `v12_injl60_m2.8_shortema25_rsi60_atrpause05` | 57.39% | 17.47% | DD passes |
| `v5_injl40_ltcl25_btcema30` | 52.57% | 18.20% | LTC as 2nd engine |

### Remaining Gap
The DD floor at ann~99% is ~23.77%, which comes from a **2023 mid-year correction** (April-June 2023, when INJ corrected ~22% even though BTC was still in uptrend). The BTC downtrend filter fixed the 2024-2026 DD but cannot catch same-trend corrections. Additional filters tried (RSI, EMA, tighter stops, portfolio equity stop) did not break the 23-24% DD floor without destroying ann.

## Status

| Profile | Gate | Result |
|---|---:|---|
| Conservative | ann>50%, DD<=10% | NOT FOUND. Best: 52.57%/18.20% (DD +8.20) |
| Balanced | ann>90%, DD<=20% | NOT FOUND (closest ever: 99.73%/23.77%, DD +3.77) |
| Aggressive | ann>110%, DD<=30% | FOUND (133.54%/29.88%) |

The cross-symbol regime filter is a genuine advance — it moved the balanced frontier from 99.51%/26.68% (pre-filter) to 99.73%/23.77% (with BTC shortdown filter + ATR pause). The DD gap shrank from +6.68 to +3.77.

## Artifacts
- Code change: `apps/backtest-engine/src/martingale/indicator_runtime.rs` (resolve_operand + parse_indicator_call + split_symbol_prefix + 3 tests)
- Best balanced config: `/tmp/codex_small_search/glm_btc_regime_v11_configs/v11_injl48_m35_l6_shortema30_atrpause0.5.json` (requires env `MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT=0.5`)
- All BTC-regime results: `/tmp/codex_small_search/glm_btc_regime_v*/results.json`
- Not merged to main — all changes in working tree for ChatGPT review.
