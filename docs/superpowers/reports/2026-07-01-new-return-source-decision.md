# 2026-07-01 New Return Source Decision

This is a research-only decision report for the original martingale/grid objective. It does not trade, touch Binance, flyingkid, live mode, or real funds.

## Why This Exists

The repeated pure martingale/grid search is frozen under `docs/superpowers/reports/2026-07-01-martingale-grid-search-freeze-and-reopen-criteria.md`. Current evidence shows no qualifying C/B/A martingale/grid candidate. The only remaining engineering question is whether a genuinely different return source can be proven before any live-parity work is attempted.

## Current New-Source Evidence

| Source | Current best evidence | Decision |
|---|---|---|
| Funding/carry sleeve | `docs/superpowers/reports/2026-07-01-funding-sleeve-probe.md`: top idealized standalone short funding stream is `DYDXUSDT` at `9.31% ann / 0.20% DD`, with `0` C/B/A passes. | Useful low-DD smoothing source, not enough to bridge 50/90/110% annualized targets. |
| Research-only trend sleeve | `docs/superpowers/reports/2026-07-01-trend-sleeve-frontier-probe.md`: `1200` rows, `0` passes; best observed trend row `43.24% ann / 63.21% DD`; best segment/capital aggressive-style row `41.92% ann / 36.25% DD`. | Real trend return exists, but DD remains far above C/B/A gates and this is not martingale/live-parity. |
| Trend risk-control probe | `docs/superpowers/reports/2026-07-01-trend-risk-control-probe.md`: risk controls reduce DD but still produce `0` passes. | Worth revisiting only if a stronger trend/breakout edge is found first. |
| DGT dynamic grid | `docs/superpowers/reports/2026-07-01-dgt-dynamic-grid-probe.md`: high annualized rows fail DD, capital, or segment gates; `0` passes. | Not a live-promotion candidate. |
| Pair-neutral grids | Pair-neutral and risk-control reports found balanced segment behavior, but the best robust frontier remains below required annualized return; `0` passes. | Current best martingale-adjacent diversification path still misses targets. |
| External public claims | `docs/superpowers/reports/2026-07-01-external-martingale-grid-claim-gate-matrix.md`: no public replayable candidate meets the gates. | No external candidate to import or replay. |

## Next Work Gate

Do not start live-parity implementation for a new return source until a backtest-only probe produces all of the following:

- multi-symbol portfolio;
- capital below `5000U`;
- `5/5` reported segments or at least `4/5` positive with positive `2024-2026` aggregate;
- conservative-style row near or above `50% ann / DD<=10%`, not merely high annualized with high DD;
- no single-symbol concentration path;
- no future-looking symbol selection or indicator input;
- exact rule description, costs, and data window suitable for replay.

For balanced/aggressive promotion work, the same probe must first show rows close to `90% ann / DD<=20%` and `110% ann / DD<=30%`. Without that offline proof, live-parity work would be polishing an unqualified mechanism.

## Practical Next Candidate

If continuing beyond martingale/grid, the only rational next probe is a **backtest-only breakout/trend sleeve** with stricter DD controls and multi-symbol portfolio combination. It should be treated as a separate strategy class, not as evidence that a martingale strategy met the original objective. Funding can be included only as a small carry/smoothing term.

## Conclusion

No new return source currently rescues the original martingale/grid objective. The search should remain closed to repeated martingale/grid sweeps and open only to a separately proven trend/breakout or stat-arb sleeve that first passes offline gates before any live-parity work.
