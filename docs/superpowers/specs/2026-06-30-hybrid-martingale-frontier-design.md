# 2026-06-30 Hybrid Martingale Frontier Design

## 1. Goal

Find, or falsify with stronger evidence, portfolios that satisfy the original user target:

- capital or margin used below 5000 USDT;
- multi-symbol, not a single-symbol escape hatch;
- robust across H1-2023, H2-2023, 2024, 2025, and 2026_ytd;
- conservative: annualized return above 50% with max drawdown at or below 10%;
- balanced: annualized return above 90% with max drawdown at or below 20%;
- aggressive: annualized return above 110% with max drawdown at or below 30%;
- final candidates must be reproducible in live trading before any live or flyingkid step.

The target is not lowered. The approved design changes only the search boundary: a deliverable portfolio may include non-martingale sleeves if the portfolio still contains a martingale/grid sleeve and passes the original gates.

## 2. Current Evidence

Current reports and replays show no deliverable pure martingale/grid portfolio under the original gates.

- Conservative and balanced profiles have no full-period passing candidate in the verified search pools.
- Aggressive has a small number of full-period passing candidates, but all fail segment robustness and are dominated by H1-2023.
- Segment-first and regime allocator searches improve robustness but cap annualized return near the low double digits.
- Funding-only estimates from local funding history are too small to bridge the 50%/90%/110% annualized targets alone.

This does not prove that every possible trading strategy is impossible. It does prove that another pure martingale parameter sweep is unlikely to move the requested end state.

## 3. Approved Approach

Use a two-phase hybrid frontier search.

### Phase 1: Backtest-Only Frontier Probe

Build a research-only portfolio evaluator that can combine independent sleeves:

- martingale/grid sleeve: existing live-parity candidates and best robust baselines;
- trend or breakout sleeve: high-timeframe momentum, EMA, Donchian/high-low breakout, or equivalent no-lookahead trend rules;
- funding/carry sleeve: funding-rate-driven short-perp or delta-neutral approximations, using local funding history and explicit cost assumptions.

Phase 1 must not touch Binance, live mode, flyingkid, or real funds. It exists only to answer whether a hybrid frontier can reach the original C/B/A gates under offline replay.

### Phase 2: Live-Parity Promotion

Only mechanisms that pass Phase 1 may be promoted. Promotion requires:

- shared config/schema representation;
- backtest implementation;
- trading-engine implementation or an explicit live execution design;
- budget replay and live-parity gate coverage;
- exchange filter, order type, reduce-only, funding, fee, slippage, and min-notional alignment.

No Phase 1-only candidate may be described as live-tradable.

## 4. Acceptance Gates

Every candidate must produce a machine-readable report with:

- full-period annualized return, max drawdown, capital used, blocked budget events, trade count, fee estimate, funding estimate, and symbols;
- segment metrics for H1-2023, H2-2023, 2024, 2025, and 2026_ytd;
- positive segment count and 2024-2026 aggregate return;
- sleeve-level PnL attribution;
- trial count and overfit flags;
- no-lookahead assertion for every indicator and allocation decision;
- live-parity status: `research_only`, `promotion_required`, or `live_parity_passed`.

The final C/B/A gates remain:

| Profile | Annualized return | Max drawdown | Budget |
|---|---:|---:|---:|
| Conservative | > 50% | <= 10% | < 5000 USDT |
| Balanced | > 90% | <= 20% | < 5000 USDT |
| Aggressive | > 110% | <= 30% | < 5000 USDT |

A portfolio fails if it is single-symbol, relies only on H1-2023, fails any required segment gate, exceeds budget, or lacks a credible path to live reproduction.

## 5. Architecture

Phase 1 should stay isolated from production runtime code unless a small reusable parser or metrics helper already exists.

Recommended components:

- `hybrid_frontier_probe`: offline runner that loads existing market data and funding data, generates sleeve return streams, combines weights, and writes reports;
- `sleeve_stream`: normalized time series for each sleeve with timestamped equity deltas and capital requirement;
- `portfolio_allocator`: no-lookahead combiner that applies fixed or walk-forward weights with budget caps;
- `robustness_report`: JSON and markdown output with the gates in section 4.

The runner should prefer existing local market/funding databases and existing martingale replay outputs. If it needs a new indicator such as Donchian high/low, add it first as research-only logic inside the probe. Promotion to engine code is Phase 2.

## 6. Data Flow

1. Load market data for the selected symbols and time range.
2. Load funding data from the local funding database.
3. Generate sleeve streams using only information available at or before each timestamp.
4. Combine sleeve streams under the selected profile budget and drawdown constraints.
5. Replay full period and five fixed segments.
6. Emit candidate JSON, summary markdown, and a rejection table for near misses.

The probe must preserve the exact segment boundaries already used by the martingale robustness validator so that new evidence is comparable to existing reports.

## 7. Error Handling

The probe must fail closed:

- missing data for a symbol or sleeve rejects that sleeve for the affected run;
- lookahead-sensitive indicators require explicit warmup and cannot trade before warmup completes;
- funding gaps must be reported and either forward-filled under a documented rule or treated as no-position intervals;
- any budget overflow, negative timestamp ordering, or unknown fee model marks the candidate invalid.

## 8. Testing

Before implementation, write an implementation plan with tests. Expected test coverage:

- no-lookahead stream construction;
- segment splitter uses the required five segments;
- budget cap rejects over-budget allocations;
- final gate evaluator preserves the original C/B/A thresholds;
- reports distinguish `research_only` from `live_parity_passed`;
- deterministic replay from the same seed/config.

Phase 1 verification is enough to decide whether the hybrid path is worth promoting, but it is not enough to claim live readiness.

## 9. Non-Goals

- Do not start live trading.
- Do not touch Binance credentials or real orders.
- Do not publish to flyingkid.
- Do not weaken the original C/B/A gates.
- Do not call a research-only trend or funding sleeve live-parity just because it backtests.
- Do not re-run broad pure martingale parameter sweeps that match already falsified paths unless a new mechanism changes the payoff source.

## 10. Decision Rule

After Phase 1:

- If at least one C/B/A candidate passes all offline gates, write a promotion plan for only the mechanisms used by those candidates.
- If no candidate passes, report the Pareto frontier and state which original requirements remain impossible under the tested hybrid scope.
- If a candidate passes only by concentrating return in one segment or one symbol, reject it as overfit even if full-period numbers pass.

