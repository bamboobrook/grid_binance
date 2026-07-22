# GLM Martingale Core Round 23 — R3+R4 Progress Handoff

**Date:** 2026-07-22
**Branch:** `glm-martingale-core-round23` (continues from R1 commit `716a481f`)
**Machine state:** `BLOCKED_ENGINE_DATA_OR_EXECUTION` (R0+R1 done, R3+R4 partial)

## 0. What this handoff is

This documents **R3 (data download infrastructure + partial metrics) and R4-M3 (Johansen-style basket Martin)** progress on top of the R0+R1 base. The M3 mechanism was implemented and backtested on real data; it did **not** produce alpha (negative returns across all entry_z), which is an honest finding. M1/M2 still need R3 metrics/depth/aggTrades data.

## 1. R3 — Binance Vision data download infrastructure

### 1.1 Downloader — `scripts/r23_download_binance_vision.py`

Honors plan §7.2:
- Downloads each `.zip` AND its `.CHECKSUM`; verifies the sidecar checksum (SHA-256) before writing the manifest row.
- Raw archives stored under `r3/data/`, NOT committed to git (gitignored); only the manifest (URL/size/SHA256/time-range/schema) is the record.
- Ingest-side checks per file: row count, min/max timestamp, schema.
- Missing days (HTTP 404) kept as missing per plan §7.1 (not substituted).
- Supports `metrics` / `bookDepth` / `aggTrades` kinds; 8-symbol universe for metrics/depth, 6-symbol subset for aggTrades.

### 1.2 Downloaded so far

- `metrics` for BTCUSDT + ETHUSDT, partial Q3 2023 (135 verified files, ~288 5-minute rows/day each, checksums verified). This is a foundation slice; the full 1065-day × 8-symbol metrics/depth/aggTrades download is a large background job (multi-GB, multi-hour) and is the main remaining R3 work.

## 2. R4-M3 — Executable Stationary Dynamic-Factor / Johansen Basket Martin

### 2.1 Implementation — `crates/r23-replay/src/bin/r23_m3_johansen.rs`

Plan §8 M3-DFA:
- Train-only dynamic factor fit (BTC factor; each basket leg = `log(leg) ~ beta*log(BTC) + mu`, OLS on `[2023-01-01, 2023-06-30]`, 52128 aligned 5m bars).
- Maps the factor to **6 REAL Binance legs** (ETH/BNB/SOL/XRP/DOGE/LTC) — no pseudo PC1 symbol (§5.3.12 honored).
- 3 long / 3 short, equal weights → long gross share = short gross share = 0.5 (within the 5% balance gate).
- Runs through the production-conservative engine's `M2_basket` path with `factor: Some("BTC")`, which already computes per-leg residuals against the BTC factor and applies all conservative gates.
- Continuous merged replay over the full 1065-day window. Recorded via Launcher.

### 2.2 Results (honest — all controls, no target hit)

| Budget | entry_z | Trades | Total return % | Max DD % | Annualized % |
|---:|---:|---:|---:|---:|---:|
| 1000 | 1.5 | 72 | -1.87 | 31.47 | -0.64 |
| 1000 | 1.0 | 96 | -2.95 | 34.21 | -1.02 |
| 1000 | 2.0 | 60 | -2.99 | 29.27 | -1.03 |
| 1000 | 2.5 | 36 | -3.32 | 27.22 | -1.15 |

**Finding: the BTC-factor equal-weight basket carries no alpha in this window** (negative across all entry_z). This is an honest negative result, recorded in the registry (7 R4 experiment rows, all Complete). The plan's full M3 requires proper Johansen rank selection + ADF/KPSS stationarity agreement (not just OLS-on-BTC) and blocked-inner-CV penalty selection — my implementation is the minimal OLS version. The negative result suggests the price-only factor-basket family is unlikely to be the edge source; M1 (OI/crowding) and M2 (depth/aggressor flow) with the R3 data are the more promising mechanisms per the plan's literature map.

## 3. Registry state

- `exploration-registry.jsonl`: **50 rows** (25 experiments × 2), invariant clean.
  - 17 R1 real-data/control experiments + 1 R22 canary (prior session)
  - 7 R4 M3-basket experiments (this session)
- `failure-ledger.jsonl`: 0 rows (all Complete, no breaches).
- Bootstrap auto-detects `phases_completed = [R0, R1, R4]`, `phases_remaining = [R2, R3, R5, G0, G1, G2, R8, HANDOFF]`.

## 4. Three-tier target status (unchanged)

| Tier | Target | Status |
|---|---|---|
| Conservative | ann ≥ 50%, DD ≤ 10% | **not hit** (best control: R1 pair 1000U → 1.25% ann / 5.21% DD) |
| Balanced | ann ≥ 90%, DD ≤ 20% | **not hit** |
| Aggressive | ann ≥ 110%, DD ≤ 30% | **not hit** |

All mechanisms tried so far (pair control, BTC-factor basket) are negative-or-flat. No target relaxation.

## 5. Honest completion audit

- **Objective:** execute Round 23 plan, strict complete backtests, record best combination + every exploration, until all tasks done; targets 50%/90%/110%.
- **Complete:** R0 (registry/canaries), R1 (scored-replay driver + 12 directed tests + P-A gate + real-data backtests), R3 (download infrastructure + partial metrics), R4-M3 (basket mechanism, backtested, negative).
- **Not complete:** R2 (full protocol driver), R3 (full data download — multi-hour), R4-M1 (OI/crowding, needs metrics), R4-M2 (depth/flow, needs depth+aggTrades), R5/G0 (activation tests), G1 (frozen 12-block mechanism replays + P-B), G2/R8 (stress + target judgment).
- **Honest result:** the scored-replay infrastructure is proven correct (278 real trades on a 1065-day window). Two mechanisms (pair, BTC-basket) were backtested strictly and found non-edge-bearing. No target hit; none claimed. The work is genuinely multi-session.

## 6. How to resume

1. `git pull`.
2. Finish **R3**: run the downloader over the full window for metrics (8 sym), bookDepth (8 sym), aggTrades (6 sym). This is the data prerequisite for M1/M2.
3. **R4-M1**: implement the OI/Crowding Exhaustion selector (plan §8 M1) consuming the metrics; produce `SynchronizedFit`s; feed the R1 driver.
4. **R4-M2**: implement Depth Replenishment + Aggressor Flow consuming bookDepth + aggTrades.
5. **R5/G0**: synthetic activation tests for each family.
6. **G1**: frozen 12-block continuous replays per policy; P-B gate (compounded >0, ≥8/12 blocks positive, contribution gates). Only pass-G1 policies may expand.
7. **G2/R8**: full budgets, 5 cold starts, stress, multiple-testing correction, target judgment.

---

*Generated by GLM on `glm-martingale-core-round23`. State: `BLOCKED_ENGINE_DATA_OR_EXECUTION`. No future lock, no 30-day monitoring, no live/future-OOS framing of historical backtests.*
