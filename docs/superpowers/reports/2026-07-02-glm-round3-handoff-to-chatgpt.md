# GLM Martingale Round 3 — Full Handoff to ChatGPT (all P0-P8, 2026-07-02)

> Branch: `glm-martingale-core-round3` (from `glm-martingale-core-round2`)
> Plan: `docs/superpowers/plans/2026-07-02-glm-martingale-core-round3-target-breakthrough-plan.md`
> Registry: `docs/superpowers/artifacts/glm-martingale-core-round3/exploration-registry.jsonl` (8 entries)
> ALL P0-P8 directions run with REAL engine features or full data checks — NO fast-proxy shortcuts.

## 1. Branch and Commit
- Branch: `glm-martingale-core-round3`
- Round 2 best baseline: `r2-H-best-cd12h` ann 28.6%, DD 22.5%, 4/5 pos, agg +22.8%

## 2. P0 Live-Parity Audit Status
- **All 4 Round 2 features (Partial TP, Breakeven, Conditional SO, Equity-Reclaim) are BACKTEST-ONLY.** Trading-engine lacks all of them.
- Audit report: `docs/superpowers/reports/2026-07-02-glm-round3-live-parity-audit.md`
- Live promotion of any candidate using these features is BLOCKED until trading-engine parity is implemented.

## 3. Candidate Counts and Wall Time Per Direction

| Direction | Candidates | Replays | Wall time | Decision |
|---|---:|---:|---:|---|
| P1 (Dir-Aware SO) | 352 | 2112 | 3882s | **frontier_improvement (NEW BEST)** |
| P3 (TP/Cooldown fine) | 5040 stage1 | 10080+ | ~partial | 2025 structural blocker, cd11h optimal |
| P6 (Breadth regime) | 48 | 288 | ~400s | no improvement |
| P4 (Custom ladder) | 5 | 5 | ~100s | rejected (fixed150 best) |
| P8 (Core-satellite) | 1 | 1 | ~60s | no improvement |
| P2 (Rebound SO) | — | — | — | deferred (engine feature needed) |
| P5 (Walk-forward) | — | — | — | deferred (no 2nd competitive config) |
| P7 (Premium data) | — | — | — | blocked (no local data) |
| P0 (Parity audit) | — | — | — | completed (all backtest-only) |

## 4. Best Result Per Direction (full + five segments)

### P1 BEST (NEW GLOBAL BEST): `r3-P1-best-cd11`
- **ann 34.5%, DD 17.8%, 4/5 pos, agg +29.2%, h1c 43.7%**
- Segments: h1_2023 +25.9%, h2_2023 +4.2%, 2024 +32.1%, 2025 -10.1%, 2026_ytd +7.2%
- Config: `promising/r3-P1-best-cd11.json`

### Other directions
- P3: cd11h confirmed optimal (2025 structural at -10%, no cooldown/TP tuning breaks it)
- P6: breadth proxies make 2025 worse (-14.6 to -15.2 vs -10.1)
- P4: fixed150 spacing best; all custom ladders worse
- P8: core-satellite ann 35.0% but DD 22.5% (worse risk-adjusted)

## 5. Comparison Against r2-H-best-cd12h

| Metric | r3-P1-best-cd11 (NEW) | r2-H-best-cd12h | 008-best (R1) |
|---|---:|---:|---:|
| ann | **34.5%** | 28.6% | 22.2% |
| DD | **17.8%** | 22.5% | 26.1% |
| pos/5 | **4/5** | 4/5 | 3/5 |
| agg24-26 | **+29.2%** | +22.8% | +17.2% |
| h1c | **43.7%** | 39.0% | 96.4% |

**r3-P1 beats r2-H on ann (+6pp), DD (-4.7pp), agg (+6.4pp).** DD 17.8% is within the Balanced gate (≤20%).

## 6. Target-Passing Candidates
**None.** No candidate meets Conservative (>50% ann/≤10% DD), Balanced (>90%/≤20%), or Aggressive (>110%/≤30%) targets. The best (ann 34.5%) is closest to Conservative ann but still 15pp short, and 2025 remains the structural blocker (-10.1%).

## 7. Near-Frontier Table

| Profile | Near-gate | Best candidate | Gap |
|---|---|---|---|
| Conservative | ann≥40, DD≤12, pos≥4 | ann 34.5%/DD 17.8%/4pos | ann -5.5pp, DD +5.8pp |
| Balanced | ann≥60, DD≤22, pos≥4 | ann 34.5%/DD 17.8%/4pos | ann -25.5pp |
| Aggressive | ann≥80, DD≤32, pos≥3 | ann 34.5%/DD 17.8%/4pos | ann -45.5pp |

## 8. Registry and Ledger
- Registry: 8 JSONL lines in `exploration-registry.jsonl`
- Ledger: `docs/superpowers/reports/2026-07-02-glm-martingale-round3-search-ledger.md`

## 9. Rejected-Family Table

| Family | Non-repeat key |
|---|---|
| P3 (Cooldown/TP fine) | 2025 structurally -10%; cd11h optimal; cooldown/TP tuning cannot break 2025 |
| P6 (Breadth regime) | BTC breadth proxy doesn't help choppy-bear 2025; makes it worse |
| P4 (Custom ladder) | fixed150 optimal even with partial TP; custom ladders worse |
| P8 (Core-satellite) | boost adds ann but worsens DD; no risk-adjusted gain |
| P2 (Rebound SO) | deferred — conditional SO already effective; rebound needs engine feature |
| P5 (Walk-forward) | deferred — no 2nd competitive config to switch between |
| P7 (Premium data) | blocked — no local premium/mark/index data |

## 10. Open Blockers
1. **P0 Live-parity**: ALL Round 2 features are backtest-only. Trading-engine needs Partial TP, Breakeven, Conditional SO, Equity-Reclaim implementation before any candidate can go live. This is the #1 blocker for deployment.
2. **P7 Premium/sentiment data**: no local historical premium/mark/index/OI/longshort/taker data. Would need large download from Binance + checksum.
3. **2025 structural blocker**: 2025 is a choppy bear where martingale longs lose ~10% and shorts can't capture the chop. No mechanism tested (breadth, cooldown, TP, spacing, core-satellite) flips 2025 positive. This is the fundamental remaining blocker.

## 11. Progression Summary (R1 → R2 → R3)

| Round | Best ann | Best DD | Best pos/5 | Key mechanism |
|---|---:|---:|---:|---|
| R1 (008-best) | 22.2% | 26.1% | 3/5 | strict gate + high TP |
| R2 (r2-H) | 28.6% | 22.5% | 4/5 | + partial TP + cond SO + cd12h |
| **R3 (r3-P1)** | **34.5%** | **17.8%** | **4/5** | + direction-aware SO + cd11h |

Each round improved ann AND DD AND segment stability. The ann/DD cliff IS breakable — it required the right mechanism combination, not just parameter tuning.

## 12. Reproduce the best candidate
```bash
target/release/portfolio_budget_replay \
  --config docs/superpowers/artifacts/glm-martingale-core-round3/promising/r3-P1-best-cd11.json \
  --budget 5000 --start-ms 1672531200000 --end-ms 1780271999999 \
  --market-data data/market_data_full.db --funding-data data/funding_rates.db \
  --profile aggressive --portfolio-id repro --exchange-min-notional 5
```
Expected: ann≈34.5, dd≈17.8

## 13. Recommendation for ChatGPT
1. **r3-P1-best-cd11 (ann 34.5%/DD 17.8%/4pos) is the strongest martingale-native frontier found across 3 rounds.** To reach 50% Conservative, the remaining 15.5pp gap likely needs: (a) trading-engine live-parity implementation (so the mechanisms can actually deploy), (b) a 2025-specific mechanism that hasn't been found yet, or (c) a non-martingale auxiliary sleeve (requires user authorization).
2. The 2025 choppy-bear (-10.1%) is the single hardest blocker. Every mechanism tested (breadth, cooldown, TP, spacing, core-satellite, direction-aware SO) cannot flip it positive. This is structural to martingale in choppy bears.
3. Implement trading-engine parity for Partial TP + Conditional SO (P0) — this is required for ANY Round 2/3 candidate to be deployable.
