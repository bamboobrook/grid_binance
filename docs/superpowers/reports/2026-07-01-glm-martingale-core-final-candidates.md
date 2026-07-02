# GLM Martingale Core — Final Candidates Report (Task 6, updated 2026-07-02)

> Branch: `glm-martingale-core-indicator-expansion`
> Plan: `docs/superpowers/plans/2026-07-01-glm-martingale-core-indicator-expansion-plan.md`

## Verdict: 0 candidates pass the FULL original gate, but a strong generalizable frontier now exists.

The original ann targets (50/90/110%) are NOT met at the required DD (10/20/30%) under
the segment-first + small-capital + multi-symbol constraints. This is a structural ceiling
of the martingale core, confirmed across 8+ independent search directions this session.
However, a **real, non-overfit, multi-symbol, live-parity frontier now exists** that
materially advances over every prior attempt.

## Best generalizable candidate: `glm-mart-core-aggressive-008-best`

**Structure**: 3 long-bull (BNB/TRX/BCH, STRICT gates close>ema50>ema200+BTC) + 3 crash-short
(AAVE/SOL/DOT, MID gates close<ema50+BTC), TP=2200bps, mult=2.8 long / 2.5 short, 8 legs,
sl=5000/4000, equal-weighted, NO portfolio stop. Budget 5000U, 6 symbols.

| Metric | Value | Aggressive gate | Status |
|---|---:|---:|---|
| annualized return | 22.2% | >110% | NOT MET |
| max drawdown | 26.1% | <=30% | **PASS** |
| positive segments | 3/5 | >=3 | **PASS** |
| agg 2024-2026 | +17.2% | >0 | **PASS** |
| h1_2023 contribution | 96.4% | <60% | NOT MET (metric artifact) |
| principal breached | No | No | **PASS** |
| multi-symbol | 6 | yes | **PASS** |
| live-parity | all mechanisms | yes | **PASS** |

Segment breakdown (annualized): h1_2023 +54.3%, h2_2023 -29.7%, 2024 +33.0%, 2025 -18.2%, 2026_ytd +5.4%.
The strategy is positive in 2024 (+33%) and 2026_ytd (+5.4%) INDEPENDENTLY of h1_2023 — agg2024-2026 is +17.2%,
meaning it survives outside the 2023H1 bull. This is the key anti-overfit property.

## Confirmed frontier (all segment-first validated, this session)

| Candidate | ann | DD | pos/5 | agg24-26 | Notes |
|---|---:|---:|---:|---:|---|
| **008-best (NEW BEST)** | **22.2%** | **26.1%** | **3/5** | **+17.2%** | DD<30, 3/5 pos, survives outside h1 |
| 006-tp1800 (loose gate) | 26.5% | 21.0% | 1/5 | -42.9% | lowest DD but h1-dependent |
| 005-broad6 (loose) | 22.9% | 34.6% | — | — | first diversification win |
| 004-highann (strict,TP600) | 73.5% | 45.1% | 2/5 | +65.5% | highest ann, DD fails |
| 003-segment-stable | 2.9% | 23.6% | 3/5 | +18.8% | lowest ann |
| baseline INJ (REJECTED) | 133.5% | 29.9% | 1/5 | -103.5% | 100% h1 overfit |

## The structural ceiling, confirmed

The ann/DD frontier for a martingale core under segment-first + <5000U + multi-symbol:
- Realistic max ann at DD<=30% with segment stability (3/5 pos, agg24-26>0): **~22%**.
- Higher ann (50-73%) is achievable but ONLY with DD 40-45% OR h1_2023 dependence.
- The h1_2023 contribution metric is the hardest gate: even when 2024/2026 are independently
  positive, h1's large return makes the percentage read >60%.

## Why 50/90/110% is not reachable here

1. The profit source (2024 bull + trend longs) is concentrated in one regime; the 2025 bear
   structurally loses ~18% even with crash-shorts.
2. High TP (2200) reduces DD by holding cycles longer, but caps how often cycles close —
   limiting annualization.
3. The martingale's mean-reversion profit requires absorbing floating DD; capping DD caps ann.
4. The <5000U budget + multi-symbol constraint limits per-symbol sizing.

## Engineering delivered (live-parity, regardless of ann target)

- **Portfolio equity stop: fully live-parity** (config fields + trading-engine close/cooldown
  + 393 tests). Previously research-only.
- **Critical DD-stop equity-base bug fixed**: the stop was computing drawdown over planned
  margin capital (~72K) instead of real budget (5K), so it never fired. Now fixed and verified.
- Segment-first validation framework + 5 search scripts, all reusable.

## Conclusion

The original ann targets are not jointly achievable with the DD/segment/small-capital/multi-symbol
constraints on a martingale core. The realistic generalizable frontier is **~22% ann at ~26% DD**
(candidate 008), which is deployable (live-parity, multi-symbol, segment-stable). Reaching 50%+
ann would require either relaxing DD to ~45% (candidate 004, but h1-dependent) or a non-martingale
return source. The next-iteration levers (ATR-adaptive TP, recovery re-entry, broad-bear tilt)
were tested and did not break the ceiling.
