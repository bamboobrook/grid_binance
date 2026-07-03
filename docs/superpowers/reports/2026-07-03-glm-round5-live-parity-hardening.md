# Round 5 Task H: Live-Parity Hardening Report

## Current Status (from Round 4)
All 4 Round 2/3/4 features have trading-engine implementations:
1. Conditional SO: full parity (187 tests)
2. Multi-Stage Partial TP: stage tracking parity (conservative approximation)
3. Breakeven Stop: implemented
4. Equity-Reclaim: implemented

## Round 5 New Features (engine-only, no trading-engine parity yet)
5. Last-Executed SO Basis: backtest-only (needs trading-engine implementation)
6. Loss-Streak Risk Reduction: backtest-only
7. Volatility-Targeted Exposure: backtest-only

## Hardening Requirements per Plan

### 1. Partial TP fractional reduce-only close
**Status: PARTIAL.** Trading-engine currently closes FULL position on each TP trigger. True partial close (fractional quantity) requires modifying `request_martingale_close` to accept a quantity parameter. This is a conservative approximation (exits earlier than backtest).

### 2. Algo-order behavior
**Status: DOCUMENTED.** Binance USD-M conditional orders migrated to Algo Service (2025-12-09). The trading-engine's TP/SL/trailing orders must use `/fapi/v1/algoOrder`. Current implementation uses standard conditional orders which may need migration. This is a live-deployment concern, not a backtest validation issue.

### 3. Order count filters
**Status: POTENTIAL BLOCKER.** Binance has MAX_NUM_ORDERS (200) and MAX_NUM_ALGO_ORDERS per symbol. With 6 symbols × multiple TP stages × safety orders, the order count may approach limits. This needs live exchange testing to verify.

### 4. exchangeInfo filters
**Status: APPROXIMATE.** The backtest uses `exchange_min_notional = 5` as a fixed approximation. True tick size, step size, and min quantity must come from exchangeInfo snapshots. This is a live-deployment concern.

### 5. Stale conditional exit cleanup
**Status: NOT IMPLEMENTED.** On final TP or SL, stale opposite conditional exits should be canceled. The trading-engine's Stopping status handles this indirectly (stops all new orders) but doesn't explicitly cancel pending conditional orders.

## Conclusion
The fine-combo best (ann 49.93%) uses last-executed SO basis + vol-target + risk reduction, which are all backtest-only features. Trading-engine parity for these 3 new features is needed before live deployment. The 4 Round 4 features have approximate parity. Full exact parity (fractional close, algo-order, exchangeInfo) requires live exchange integration testing which is out of scope for backtest validation.

## Recommendation
For live deployment:
1. Implement last-executed SO basis in trading-engine (highest priority - it's the key ann driver)
2. Implement vol-targeting in trading-engine
3. Migrate to algo-order API for conditional orders
4. Test order count limits with real exchangeInfo
