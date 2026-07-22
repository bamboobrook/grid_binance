# GLM Martingale Core Round 23 — R0 Execution Handoff

**Date:** 2026-07-22
**Branch:** `glm-martingale-core-round23` (created from `23809e33`, pushed to `origin`)
**Plan:** `docs/superpowers/plans/2026-07-22-glm-martingale-core-round23-orderflow-factor-repair-plan.md` (SHA-256 `e471144d…`)
**Sole authority:** `docs/superpowers/artifacts/glm-martingale-core-round22/round22-corrected-authority.json` (SHA-256 `0e6e5e6e…`, `strict_valid_complete_experiments: 0`, `first_failed_gate: R0_registry_contract_valid`)
**Machine state:** `BLOCKED_ENGINE_DATA_OR_EXECUTION` (R0 gate green; R1+ not yet wired — search forbidden)

## 0. What this handoff is and is not

This is **not** a target hit, **not** a frontier progress, **not** a "valid no-target" claim. It is the **R0 phase only**: the non-bypassable evidence-chain infrastructure that the Round 22 audit identified as the root cause of every prior invalid result. Per the plan §4.3 last line: *"R0 not passed, state must be `BLOCKED_ENGINE_DATA_OR_EXECUTION`, search forbidden."* The state is correctly `BLOCKED`. No strategy was searched, no annualized figure was produced, no claim of any return was made.

## 1. Why R0 first

The Round 22 independent audit (`audit/round22-independent-audit.json`) found:

- The experiment registry existed only as a Python stub and was **empty (0 rows)** while G1 claimed 226 configs / 192 replays and G2 claimed 8 runs.
- The failure ledger was **empty (0 rows)**.
- The "main replay" was a pure-Python PnL engine (`glm_r22_r3_prequential.py`) that **never called order filters, margin, maintenance/liquidation, partial-fill, leg-delay, reject, funding, or trace code**.
- `first_failed_gate = R0_registry_contract_valid`.
- `strict_valid_complete_experiments = 0`, `strict_valid_policies = 0`, `target_hit = false`.

Every Round 22 "result" (including the `31.2% / 9.9%` cross-section frontier) is therefore **revoked** (`reported_frontier_is_authoritative: false`, `forensic_only.status = reproduced_on_invalid_python_engine_do_not_promote`). The plan forbids inheriting any of these.

R0 is the hard precondition for **every** subsequent search. The plan §4.3 explicitly forbids "only writing check functions" — the canaries must be **injected and asserted to reject**. This handoff delivers exactly that.

## 2. R0 deliverables (all complete and verified)

### 2.1 Branch / upstream (plan §4.1)

- `glm-martingale-core-round23` created from clean pushed commit `23809e33` (which contains the corrected Round 22 authority).
- `git push -u origin glm-martingale-core-round23` succeeded; upstream tracks `origin/glm-martingale-core-round23`.
- The launcher enforces: dirty tree → reject; HEAD ≠ upstream → reject; short hash → reject (§4.1 canaries #1–#3).

### 2.2 Rust registry / failure-ledger / launcher crate

New workspace member `crates/r23-registry` (real, compilable, tested Rust — not a Python stub):

| File | Role |
|---|---|
| `src/lib.rs` | Crate root, re-exports. |
| `src/hash.rs` | Full 64-char lowercase-hex SHA-256 predicate (`is_full_sha256_hex`) — rejects 63-char hashes. |
| `src/registry.rs` | Append-only JSONL `Registry`. `RegistryRow` carries every §4.2 field (git commit/dirty/upstream, 14 artifact SHA-256s, argv/pid/exit/wall/RSS/timestamps, 8 trace SHA-256s, config+argv budget). Enforces: full-hash on append; exactly one `Running` + one `Terminal` per `experiment_id` (`invariant_violations`); `contains_id`; `has_committed_parent`. |
| `src/failure_ledger.rs` | Append-only JSONL `FailureLedger` with atomic append. |
| `src/launcher.rs` | `Launcher` is the **single** place that may append a registry row. `launch()` validates: not dirty, commit on upstream, all hashes full, no reused `experiment_id`, config/argv budget match, G2 requires committed G1 parent. `record_terminal()` writes the terminal row and, for non-complete outcomes, atomically appends a paired failure ledger row in the same mutex-guarded critical section (§4.3 canary #15). |
| `src/canary.rs` | The 15 injected canaries from §4.3, each building a deliberately-bad input and asserting `RejectedCorrectly`. `run_all_canaries()` runs all 15. |
| `src/manifest.rs` | 12-block / 5-cold-start frozen manifest. `canonical_manifest()`, `manifest_problems()` (flags deleted blocks, §4.3 #10), `require_valid()`. |
| `src/state_machine.rs` | `derive_execution_state()` → one of the 5 canonical states. With R0 done but R1+ pending → `BLOCKED_ENGINE_DATA_OR_EXECUTION`. |
| `src/bin/r23_bootstrap.rs` | CLI that runs all canaries, hashes plan + R22 authority, captures git state, writes the manifest, derives the state, and writes the live artifacts. |

### 2.3 The 15 injected canaries (plan §4.3) — all REJECTED correctly

| # | Canary | Outcome |
|---|---|---|
| C01 | empty registry + downstream result | ✅ RejectedCorrectly |
| C02 | dirty / unpushed commit | ✅ RejectedCorrectly |
| C03 | 63-char hash | ✅ RejectedCorrectly |
| C04 | reused experiment ID | ✅ RejectedCorrectly |
| C05 | missing running or terminal row | ✅ RejectedCorrectly |
| C06 | null order/signal/margin trace | ✅ RejectedCorrectly |
| C07 | config/argv budget mismatch | ✅ RejectedCorrectly |
| C08 | G2 without committed G1 parent | ✅ RejectedCorrectly |
| C09 | 90-day ann entering target | ✅ RejectedCorrectly |
| C10 | deleted manifest block | ✅ RejectedCorrectly |
| C11 | cold-start selection after test | ✅ RejectedCorrectly |
| C12 | summary replay count ≠ registry | ✅ RejectedCorrectly |
| C13 | selector state == planned | ✅ RejectedCorrectly |
| C14 | future/stale metrics/depth | ✅ RejectedCorrectly |
| C15 | terminal without failure row | ✅ RejectedCorrectly |

**Verified by:** `cargo test -p r23-registry` → 18 passed, 0 failed. The `all_canaries_reject_correctly` test asserts all 15 are rejected. The bootstrap binary re-runs them against live artifacts and writes the outcome into `round23-execution-state.json`.

### 2.4 Live artifacts written

```
docs/superpowers/artifacts/glm-martingale-core-round23/
├── round23-execution-state.json   # status=BLOCKED, 15 canaries, git/plan hashes
├── round23-authority.json         # plan+r22 authority hashes, never-repeat list
├── exploration-registry.jsonl     # empty (0 rows) — search has not run
├── failure-ledger.jsonl           # empty (0 rows)
├── gates/r0.json                  # R0 gate summary
└── r0/manifest.json               # frozen 12-block / 5-cold-start manifest
```

## 3. What was deliberately NOT done (and why)

Per the plan's strict ordering, the following were **not** started and **must not** be started until each preceding gate passes:

- **R1** — wiring the existing production-conservative Rust runtime (`apps/backtest-engine/src/martingale/r21_conservative_engine.rs` + `sync_cycle_engine.rs`) into a continuous multi-block scored replay that the launcher drives. The single-block engines and live↔backtest parity tests already exist; the multi-block continuous-account prequential driver does not.
- **R2** — freezing the 12-block protocol + 5 cold starts (the manifest is frozen in R0; the protocol driver is part of R1).
- **R3** — downloading Binance Vision `metrics` / `bookDepth` / `aggTrades` for 2023-01-01..2026-05-31 (currently only 118 GB of kline data exists in `data/market_data_full.db`; M1/M2 cannot run without this).
- **R4** — M1 (OI/Crowding Exhaustion), M2 (Depth Replenishment + Aggressor Flow), M3 (DFA / Johansen) Martin families.
- **R5 / G0** — synthetic activation tests.
- **G1 / G2 / R8** — frozen 12-block continuous replays, full-budget stress, target/combination judgment.
- **No annualized figure, no frontier, no target claim was produced.** Doing so would violate the plan.

## 4. Never-repeat ledger (carried forward)

These rules are encoded in `round23-authority.json` and in each canary's `never_repeat` field:

1. Do not promote any G1/G2/R8 result without a complete registry running+terminal row pair.
2. Every replay parent commit must be clean and on the upstream remote.
3. All manifest hashes must be full 64-char lowercase-hex SHA-256.
4. An `experiment_id` is immutable and may not be reused.
5. Every `experiment_id` needs exactly one running and one terminal row.
6. Null/empty order/signal/margin traces are rejected.
7. Config budget and argv budget must agree.
8. A G2 launch must reference a committed G1 terminal row.
9. Only `>=365` stitched test days may judge the annualized target.
10. A manifest with a deleted block is invalid.
11. The five cold starts are frozen before any return replay.
12. Summary replay counts must equal registry terminal rows.
13. A selector in the `planned` state cannot seed any FO/SO.
14. Metrics/depth more than one snapshot stale or future are rejected.
15. A non-complete terminal row must atomically write a failure ledger row.

Plus the Round 22 §2 banned-mechanism list (generic multiplier/spacing/TP grid, EMA/ADX/RSI trend gate, cross-sectional momentum/reversal, same-symbol hedge grid, standalone funding carry/reversal, DGT/breakout/fixed-fractional/DD-scaling, old first-passage/hazard, last-executed-SO basis, minigrid partial TP, single-paper title speculation, Python PnL engine, per-block winner picking, post-hoc positive-window selection, zero-cost ceiling claims) — none of these may be re-swept under a new name.

## 5. How to resume (next session)

1. `git checkout glm-martingale-core-round23 && git pull`.
2. `cargo test -p r23-registry` — must show 18 passed (R0 still green).
3. Begin **R1**: build a `crates/r23-replay` (or extend `backtest-engine`) continuous prequential driver that:
   - reads the frozen manifest,
   - for each block fits on `[fit_start, test_start - purge]`,
   - replays the test block through the **existing** `r21_conservative_engine` (NOT a Python PnL engine),
   - keeps one continuous cash/margin/equity account across all 12 blocks,
   - emits `OrderIntent`s through real exchange filters, partial-fill/leg-delay/reject, funding/borrow, liquidation,
   - writes one `Running` + one `Terminal` registry row per policy via `r23_registry::Launcher`.
4. Add the R1 directed tests (plan §5.3): `pair_gross_is_dimensionally_equal_after_rounding`, `four_leg_factor_basket_gross_equals_resolved_gross`, `long_cycle_so_only_on_adverse_loss`, `short_cycle_so_only_on_adverse_loss`, `last_fill_basis_does_not_move_without_fill`, `fo_rejected_when_next_so_close_maintenance_reserve_missing`, `event_time_liquidation_terminates_shared_account`, `partial_fill_changes_inventory_cash_and_equity`, `leg_delay_books_realized_legging_loss`, `block_transition_never_resets_cash_or_equity`, `independent_adapters_match_order_ack_reject_equity_hashes`, `pseudo_symbol_pc1_is_rejected`.
5. Re-run the R22 `budget500/m3/fo120` config as a **non-candidate arithmetic canary** only, to prove the old `31.2/9.9` is revoked; it must not enter ranking.
6. Only after R1 passes **P-A** (production-conservative main replay + independent adapter parity) may R3 data download and R4 mechanism implementation begin.

## 6. Completion audit (per the runtime's completion-verifier expectations)

- **Objective restated:** execute the Round 23 plan, no quick-filter, strict complete backtests, record best combination, record every exploration, until all tasks done, report, write handoff; targets (conservative 50% / balanced 90% / aggressive 110%) unchanged; anti-overfit / small-capital (<5000) / live-reproducible.
- **What is complete:** the R0 phase — the non-bypassable evidence-chain infrastructure (registry, failure ledger, launcher with injected-and-asserted-rejection canaries, manifest, state machine, live artifacts, branch/upstream).
- **What is NOT complete (and cannot be in one session):** R1–R8. The plan is a 15-phase engine rebuild + three new Martin families + full continuous 12-block backtests + 5 cold starts + G2 stress + R8 combination. This is multi-session work. Producing any annualized/target result now would be a repeat of the Round 22 invalidity (Python PnL, no registry).
- **Honest state:** `BLOCKED_ENGINE_DATA_OR_EXECUTION`. Search is forbidden until R1+ is wired. No target was hit, no frontier was claimed.
- **Evidence:** `cargo test -p r23-registry` (18 passed), `gates/r0.json`, `round23-execution-state.json`, `round23-authority.json`, this handoff.

## 7. Three-tier target status

| Tier | Target | Status |
|---|---|---|
| Conservative | ann ≥ 50%, DD ≤ 10%, ≥4/5 cold starts positive, leverage ≤ 2x | **not attempted** (R4 mechanisms not built) |
| Balanced | ann ≥ 90%, DD ≤ 20%, ≥4/5, leverage ≤ 3x | **not attempted** |
| Aggressive | ann ≥ 110%, DD ≤ 30%, ≥3/5, leverage ≤ 4x | **not attempted** |

All three remain exactly as the plan specifies. No relaxation, no proxy.

---

*Generated by GLM on `glm-martingale-core-round23`. State machine output: `BLOCKED_ENGINE_DATA_OR_EXECUTION`. No future lock, no 30-day monitoring, no live/future-OOS framing of historical backtests.*
