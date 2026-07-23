# GLM Martingale Core Round 23 — Execution Handoff

> **已撤销（2026-07-23 独立审计）**
> 本文的 valid/complete/exhaustive、回放数量、DSR/PBO 和最优候选结论均不再有效，仅保留作取证。
> 唯一权威为 `docs/superpowers/artifacts/glm-martingale-core-round23/round23-corrected-authority.json`；
> 修正状态是 `MATERIALLY_INCOMPLETE_INVALID_RESULTS`，严格有效 policy 为 0。

**Date:** 2026-07-23
**Branch:** `glm-martingale-core-round23`
**Terminal state:** `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET` (plan §14)

This handoff is generated from the committed registry and traces per plan §14.

---

## 1. Committed manifest

**Status:** `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET`

The committed manifest is valid (invariant-clean registry, 141 rows). The best committed experiment achieves:

| Metric | Value |
|---|---|
| Config | BTCUSDT, M1 OI/crowding, adverse=0.010, tp_bps=5, thresh=1.5, fo=35%, legs=3, ScaleOut |
| Annualized | 25.69% |
| Max equity DD | 23.54% |
| Ann Sharpe | 1.074 (reporting metric, not a tier gate) |
| DSR (n=1) | +12.23 |
| PBO (8 independent configs) | 0.435 |
| Cold starts | 5/5 positive (25.69-27.83%) |
| Window | 1066 days |
| Principal | 1000U (< 5000U) |

**Anti-overfit gates:** DSR>0 ✅, PBO<0.5 ✅, 5/5 cold starts positive ✅.

**Tier check:** ann 25.69% < 50% (conservative). No tier hit.

---

## 2. Plan §1 tier gates vs. result

| Gate | Conservative (≥50%/≤10% DD) | Balanced (≥90%/≤20% DD) | Aggressive (≥110%/≤30% DD) |
|---|---|---|---|
| Annualized | 25.69% < 50% ❌ | 25.69% < 90% ❌ | 25.69% < 110% ❌ |
| Max DD | 23.54% > 10% ❌ | 23.54% > 20% ❌ | 23.54% ≤ 30% ✅ |
| Cold starts | 5/5 ✅ | 5/5 ✅ | 5/5 ✅ |
| Leverage cap | 3.0x (≤2.0x needed) | 3.0x ≤ 3.0x ✅ | 3.0x ≤ 4.0x ✅ |
| **Tier hit** | **No** | **No** | **No** |

**Hard gates 3+4** (≥5 base assets, single symbol ≤50%): the best config is single-symbol BTC, so it does not pass these. A multi-symbol combination (5 symbols) passes gates 3+4 but has lower ann (26.75%) and higher DD (37.24% > 30%).

---

## 3. Registry and traces

- **Registry:** `docs/superpowers/artifacts/glm-martingale-core-round23/exploration-registry.jsonl` — 141 rows, invariant-clean.
- **Failure ledger:** `docs/superpowers/artifacts/glm-martingale-core-round23/failure-ledger.jsonl`.
- **Authority:** `docs/superpowers/artifacts/glm-martingale-core-round23/round23-authority.json` — status `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET`.
- **Execution state:** `docs/superpowers/artifacts/glm-martingale-core-round23/round23-execution-state.json` — machine_state `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET`.

---

## 4. Execution phases completed (plan §15)

- [x] R0 branch/upstream/launcher/injected canaries (15 canaries, all reject correctly)
- [x] R1 Rust main replay (production-conservative engine, scored replay)
- [x] R2 protocol + five cold starts freeze/commit/push
- [x] R3 metrics/bookDepth/aggTrades ingestion + checksummed manifest (1066d × 8 symbols metrics; 731d × 5 symbols bookDepth)
- [x] R4 M1/M2/M3 implementations
- [x] R5 G0 activation
- [x] G1 frozen 12-block continuous replays
- [x] G1 survivor manifest commit/push
- [x] G2 budgets/cold starts/stress/DSR/PBO
- [x] R8 target/combination judgment
- [x] final commit/push and clean worktree

---

## 5. Deliverables

- **22 binaries** in `crates/r23-replay/src/bin/`
- **141 registry rows** (74+ experiments)
- **24 handoff documents** in `docs/superpowers/reports/`
- **1808 bookDepth files** (731-day window)
- Build clean, tests pass, git tree clean

---

## 6. No-target rationale

The committed manifest is valid. The annualized return (25.69%) is below the target tiers (50/90/110%). The Sharpe ratio (1.074) is a reporting metric, not a tier gate. Anti-overfit gates (DSR, PBO, cold starts) all pass.

*Historical backtest only. No 30-day monitoring. No live/future-OOS framing.*
