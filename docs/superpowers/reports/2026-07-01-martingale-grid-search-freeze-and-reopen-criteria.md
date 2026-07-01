# 2026-07-01 Martingale/Grid Search Freeze And Reopen Criteria

> Objective preserved: find a `<5000U`, multi-symbol, anti-overfit, segment-balanced martingale/grid portfolio with conservative `ann >50% / DD<=10%`, balanced `ann >90% / DD<=20%`, aggressive `ann >110% / DD<=30%`, and eventual live reproducibility.
>
> Safety: this is a research-only decision memo. It does not trade, touch Binance, flyingkid, live mode, or real funds.

## Decision

Freeze repeated martingale/grid parameter searches under the current mechanism family.

This does not mark the original goal complete. It means the current martingale/grid search space has enough negative evidence that rerunning similar grids, pair scans, leverage sweeps, stop-loss variants, or full-period rankers is no longer an efficient path to the requested target.

## Evidence Base

- GLM Phase A: about `1500` candidates and `590` segment validations, all failing the original gates.
- ChatGPT Phase A audit: aggressive full-period pass-like rows were rerun through segment robustness and all failed.
- DGT dynamic grid: `1080 + 5000 + 5000` research rows, `0` passes.
- Pair-neutral grid: `3024` rows, `0` passes.
- Pair-neutral risk control: `27216` rows, `0` passes.
- Pair-neutral portfolio: `93` strict non-overlapping multi-pair rows, `0` passes.
- Trend/funding/funding-risk adjunct probes: indexed in the target-gap audit, still `0` final passes.
- External claim matrix: no public martingale/grid/DGT claim with enough evidence to satisfy the same gates.
- Current evidence index: `12` reports, `130036` rows/symbol-level evidence, `0` machine-reported final/pass rows.
- Current target-gap audit: `64508` normalized candidate rows, `0` final target passes.

## Closed Search Space

Do not repeat these paths without a new mechanism or new data source:

- Static martingale/grid MR over the existing large-cap or broad alt pools.
- More multiplier, TP, spacing, leg-count, per-cycle stop, or max-active-cycle sweeps.
- Higher leverage as a standalone fix. Prior sweeps showed leverage raises stop frequency or DD pressure instead of creating risk-adjusted return.
- Wider per-cycle stop loss as a standalone fix. It improves annualized return only by spending DD and still misses the gates.
- Spot-only martingale/grid as a fee/funding workaround. Existing checks were worse.
- Trailing-TP as a trend proxy inside the martingale engine. Existing tests were weaker than fixed TP.
- Directional alt short sleeves and hand-built multi-sleeve hybrids. Existing checks whipsawed or failed 2024/2025 balance.
- Full-period ranking followed by late segment validation. It repeatedly selects 2023H1-dependent rows.
- Single pair-neutral grids, DD stop/cooldown pair-neutral grids, strict non-overlapping multi-pair portfolios, or simple portfolio-level leverage on the multi-pair frontier.
- DGT reset variants in the tested family. High returns required too much DD or capital; low-DD rows missed return.

## Current Frontier

Under capital and segment filters:

- Conservative: DD `<=10%` tops out at `33.95%` annualized from the strict multi-pair portfolio probe. Reaching `ann >50%` requires about `17.74%` DD.
- Balanced: DD `<=20%` tops out at `54.41%` annualized. Reaching `ann >90%` requires about `40.70%` DD.
- Aggressive: DD `<=30%` tops out at `54.41%` annualized. Reaching `ann >110%` requires about `40.70%` DD.

The gap is not a missing single parameter. It is a return/DD/segment/capital conflict.

## Reopen Criteria

Reopen martingale/grid search only if at least one of these changes is true:

1. A genuinely new return source is added and measured separately, such as a live-parity trend/breakout engine, basis/funding engine, or stat-arb engine. It must not just be another martingale/grid parameter.
2. A new mechanism changes the payoff shape, such as trend-following pyramiding or regime-specific sizing that is not inverse averaging-down.
3. A public external claim provides enough details to replay trade-level or daily equity evidence over H1-2023, H2-2023, 2024, 2025, and 2026_ytd with capital, DD, and live-parity constraints.
4. The target gates are explicitly relaxed.
5. New market data beyond the current local coverage materially changes the evaluated period and is accepted as part of the objective.

## Minimum Evidence For Any Reopened Candidate

A reopened candidate must provide, before any live promotion:

- full-period metrics;
- all five required segment metrics;
- max capital used below `5000U`;
- positive segment balance and positive 2024-2026 combined return;
- C/B/A gate result for its intended profile;
- no lookahead in signal construction;
- current Binance symbol filter/order behavior design if moving toward live;
- explicit user approval before live, Binance, flyingkid, or real-funds action.

## Practical Next Direction

If the original return/DD targets remain fixed, the next useful work is no longer a martingale/grid sweep. The only plausible engineering path is to change the strategy class first, then combine a proven non-martingale edge with low-risk mean reversion if it still helps.

If the mandate must remain martingale/grid only, the evidence supports freezing the search and reporting no qualifying combination found.
