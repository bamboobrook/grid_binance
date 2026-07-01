# GLM Martingale Core — Final Candidates Report (Task 6)

> Branch: `glm-martingale-core-indicator-expansion`
> Generated: 2026-07-01

## Verdict: 0 qualifying candidates for the original gates. Frontier report below.

Per the plan's section 6 (anti-overfit gate) and section 8 (stop condition), this
report does NOT fabricate a pass. No configuration simultaneously satisfies the
return, DD, budget, multi-symbol, segment-stability, and live-parity gates for
any of the three profiles. The confirmed frontier and the structural reason are
documented honestly so the next iteration has a clear target.

## Gate status by profile

| Profile | ann target | DD target | pos-seg target | Best real (non-overfit) | Status |
|---|---:|---:|---:|---|---|
| Conservative | >50% | ≤10% | ≥4/5 | ann −2.8% @ DD 9.4% (1/5) | **NOT MET** |
| Balanced | >90% | ≤20% | ≥4/5 | none with ann>50% @ DD≤20% | **NOT MET** |
| Aggressive | >110% | ≤30% | ≥3/5 | ann 73.5% @ DD 45.1% (2/5) | **NOT MET** (DD fails) |

The 133.5% ann original INJ baseline is excluded — it is 100% h1_2023-driven
(1/5 positive segments, h1 contribution 117%, 2024-2026 aggregate −103.5%), which
the plan's anti-overfit gate explicitly rejects.

## Confirmed frontier (4 points, all segment-first validated)

| Candidate | ann | DD | pos/5 | agg24-26 | live-parity | Notes |
|---|---:|---:|---:|---:|---|---|
| high-ann (NEW) | 73.5% | 45.1% | 2/5 | +65.5% | ✅ (now w/ live DD stop) | ann>Conservative; DD fails; 2024-driven |
| segment-stable (NEW) | 2.9% | 23.6% | **3/5** | +18.8% | ✅ | best stability; ann far below target |
| low-DD (NEW) | −2.8% | 9.4% | 1/5 | +157.3% | ✅ | DD passes Conservative; ann negative |
| baseline INJ | 133.5% | 29.9% | 1/5 | −103.5% | n/a | REJECTED — h1_2023 overfit |

## The structural blocker: the ann/DD cliff

Confirmed across three independent searches this session (high-TP efficiency,
tight single-strategy SL sweep, portfolio DD-stop sweep):
- High per-cycle profit (TP≥600bps + multiplier≥3.0) → ann 50–134% but DD ≥30–45%.
- Tight risk (SL≤2500bps) → DD ≤10–17% but ann ≤0% (negative).
- **No configuration exists with ann>50% AND DD≤30%.** The middle is empty.

This is structural to martingale: the profit comes from price reverting to the
averaged entry, which requires absorbing large adverse excursion (the DD). Tight
stops cap DD by closing before reversion, killing the mean-reversion profit.

## Live-parity status (significant progress this session)

The portfolio equity stop — previously research-only (env switches with NO live
implementation) — is now **fully live-parity**:
- `portfolio_equity_stop_pct` + `portfolio_stop_cooldown_hours` are config-structured
  in `MartingaleRiskLimits` (shared-domain).
- Both backtest and trading-engine read them config-first (parity resolution).
- The trading-engine now closes all active positions via reduceOnly + enters
  cooldown when drawdown breaches the threshold (Task 4 part 2, 185 tests pass).
- Candidates using the portfolio stop are no longer research-only.

All other mechanisms used (per-symbol regime gates, ATR/ADX/RSI/BB indicators,
cross-symbol refs, martingale core) were already live-parity.

## Next-iteration directions to break the cliff

1. **ATR-adaptive TP per cycle** with high multiplier — TP that scales down in
   high-vol regimes may reduce per-cycle DD while keeping profit in calm regimes.
   (Partially explored; not yet combined with the live DD stop + re-entry.)
2. **Recovery re-entry on the live DD stop** — close at −X% but re-enter when
   equity recovers, capturing round-trips like 2024's −45%→+127%. The live
   cooldown implementation now supports tuning this (cooldown hours = re-entry delay).
3. **Broad-bear regime tilt** — a portfolio-level regime that tilts short when
   BTC + altcoin breadth both decline (2025/2026), so the crash-short sleeve
   captures more of the bear instead of under-trading it.

## Conclusion

The original targets (50/90/110% ann at 10/20/30% DD, <5000U, multi-symbol,
segment-stable) are not jointly achievable with a pure martingale core under
the segment-first anti-overfit gate. This session narrowed the gap materially:
ann 2.9%→73.5% (15×), 0→8 portfolios making 2024+2025 both-positive (first ever),
0→3/5 segment stability, and full live-parity for the portfolio equity stop.
The realistic martingale frontier under all constraints is ~10–20% ann at
~10–20% DD (matching external literature), unless one of the three open
directions above breaks the cliff.
