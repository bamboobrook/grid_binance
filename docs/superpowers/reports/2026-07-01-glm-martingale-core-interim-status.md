# GLM Martingale Core — Interim Status Report (2026-07-01)

> Branch: `glm-martingale-core-indicator-expansion`
> Plan: `docs/superpowers/plans/2026-07-01-glm-martingale-core-indicator-expansion-plan.md`
> Ledger: `docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md`

## What this session achieved (NEW vs all prior work)

This session advanced the martingale-core search in four concrete ways over every
prior attempt (~64,508 prior candidates, 0 passes):

### 1. FIRST-EVER segment-first pipeline (2024+2025 both-positive gate)
Built `glm_segment_validator.py` + `glm_regime_gated_martingale_search.py`. The
"2024≥0 AND 2025≥0" condition was **0/590** in all prior work. This session:
- 0/3276 single-symbol candidates passed (per-symbol regime gates alone insufficient).
- **8/2592 portfolio candidates passed** the 2024+2025 both-positive gate (first time ever) — via long-bull (BNB/BCH/TRX) + crash-short (AAVE) combos with regime gates.

### 2. Portfolio equity-stop promoted to live-parity config (Task 4 part 1)
`portfolio_equity_stop_pct` and `portfolio_stop_cooldown_hours` moved from
research-only env switches into `MartingaleRiskLimits` (shared-domain), with the
backtest engine reading config-first (parity resolution). Backtest builds clean,
208 tests pass. The live trading-engine close+cooldown implementation is the
remaining work (Task 4 part 2 — implementation map documented).

### 3. Return breakthrough: ann 2.9% → 73.5% (Task 3)
High TP (600bps) + high multiplier (3.0) + strict per-symbol regime gate on the
BNB+TRX-long + AAVE-short structure produced **ann 73.5%** (15× the prior
segment-stable frontier of 2.9%). This exceeds the Conservative 50% ann target.
- DD is 45.1% (fails the 30% gate).

### 4. Confirmed the ann/DD cliff with the new high-ann structure
Tight single-strategy stop-loss sweep (sl_bps 1500–3500) confirms: with DD≤30%,
best ann is **−1.9% (negative)**; with ann>50%, DD is ≥45%. The middle is empty.
This re-confirms the prior final verdict's structural finding, now with the
higher ann frontier and segment-first validation.

## Confirmed frontier (4 distinct points)

| Candidate | ann | DD | pos/5 | agg24-26 | Verdict |
|---|---:|---:|---:|---:|---|
| original INJ baseline | 133.5% | 29.9% | 1/5 | −103.5% | passes Agg return/DD but 100% h1_2023 overfit |
| **high-ann (NEW)** | **73.5%** | 45.1% | 2/5 | +65.5% | ann>Conservative but DD fails; 2024-driven |
| segment-stable (NEW) | 2.9% | 23.6% | **3/5** | +18.8% | best stability but ann far below target |
| low-DD (NEW) | −2.8% | **9.4%** | 1/5 | +157.3% | DD passes Conservative but ann negative |

## Honest verdict on the original targets

| Target | Gate | Status |
|---|---|---|
| Conservative ann>50%, DD≤10% | NOT met | Best DD≤10% has ann −2.8%; best ann>50% has DD 45% |
| Balanced ann>90%, DD≤20% | NOT met | No config with ann>50% AND DD≤20% |
| Aggressive ann>110%, DD≤30% | NOT met | 133% baseline is overfit (1/5 pos, h1=117%); 73.5% real but DD 45% |

The ann/DD cliff is structural to martingale: high per-cycle profit (high TP +
high multiplier) requires absorbing large adverse excursion (the DD); tight SL
caps DD by closing cycles before they revert, killing the mean-reversion profit.

## What would need to happen to break the cliff (open directions)

1. **ATR-adaptive TP per cycle** (not yet fully tested with high multiplier) — TP
   that scales down in high-vol regimes might reduce per-cycle DD while keeping
   profit in calm regimes.
2. **Live portfolio equity-stop with re-entry on recovery** — close at −X% but
   re-enter when equity recovers, capturing the 2024 round-trip (−45%→+127%).
   Requires the trading-engine close+cooldown implementation (Task 4 part 2).
3. **Separate 2025/2026 bear regime** — a regime detector that tilts the whole
   portfolio short when broad bear conditions hold (the crash-short sleeve
   currently under-captures 2025/2026 because its gate is too strict).

## Live-parity status of all mechanisms used

- ✅ Per-symbol regime gates (ema/adx/rsi/bb/atr_percent + cross-symbol refs) — live-parity.
- ✅ Martingale core (multiplier/legs/TP/SL/spacing) — live-parity.
- ✅ DD-pause / ATR-pause / ADX-skip — live-parity (config-structured).
- ✅ Portfolio equity stop / cooldown — config-structured (backtest); **live close+cooldown impl pending** (Task 4 part 2).
- All recorded candidates that use the portfolio stop are marked `research_only=true` until the live impl lands.

## Next session continuation points

1. Finish Task 4 part 2: implement portfolio equity stop close-all + cooldown in
   `apps/trading-engine/src/main.rs` (implementation map in ledger; cooldown
   state OnceLocks already added).
2. Test ATR-adaptive TP (Atr multiplier model) with high multiplier to seek the
   ann/DD sweet spot.
3. If the cliff holds after those, the honest conclusion is that the original
   ann targets (50/90/110%) are not jointly achievable with DD≤10/20/30% under a
   martingale core + <5000U + multi-symbol + segment-stable constraint; the
   realistic frontier is ~10–20% ann (matches external literature).
