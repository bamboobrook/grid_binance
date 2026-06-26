# Margin-v2 Live Validation Report

Date: 2026-06-26
Plan: `docs/superpowers/plans/2026-06-26-glm-live-validation-and-1000u-launch-plan.md`
Handoff: `docs/superpowers/reports/2026-06-26-margin-v2-final-backtest-handoff.md`
Executor: Claude Code (glm-5.2) continuing GLM's plan
Owner account: `flyingkid2022@outlook.com`

> Status legend: ✅ verified · ⏳ deferred to a later step · ⛔ blocked · ⏸ paused for user decision

This report is appended to cumulatively as each plan step completes. It records evidence (command output), code changes, smoke results, and the final user-confirmation gate.

---

## Pre-flight Critical Review (raised before execution)

Before executing, these risks were identified. They do not block Steps 1–7 (read-only verify / audit / test / dry-preflight), but they gate Step 6 (live 50U smoke) and Step 9 (1000U live):

1. **Drawdown sits exactly on the cap.** conservative 10.0000% / balanced 20.0000% / aggressive 30.0000%. Expected for an LP optimizer maximizing return subject to a DD constraint, but it means **zero safety margin** — live cannot reproduce the backtested DD exactly.
2. **Quarter returns are heavily front-loaded.** conservative quarter raw returns 332.98% / 33.76% / 31.91% / 18.58% — the first quarter carries the bulk of profit. Classic regime-overfit smell; the gate only enforces non-negativity, not stability.
3. **Member-level drawdowns are very large.** conservative members include FILUSDT 72.24%, UNIUSDT 80.82%, ICPUSDT 66.25% candidate DD. The portfolio relies on low cross-member correlation within the backtest window to compress portfolio DD to 10%.
4. **50U smoke min-notional tension.** With 8 members and 50U total, the smallest-weighted members (e.g. UNIUSDT 2.244564% → ~1.1U, BTCUSDT 2.21567% → ~1.1U notional) fall far below Binance USDT-M's ~5U min notional. The full conservative portfolio likely cannot smoke at 50U without min-notional rejection; the smoke design (Step 5) must resolve this (reduced member set or scaled budget).

Per the plan's non-negotiable gates, Steps 6 and 9 require explicit user authorization and are not started from inferred consent.

---

## Step 1 — Freeze And Verify Current Runtime State

### Safe baseline (prevents live risk right now) — ✅ all verified

**trading-engine is not running.** `docker ps` of the `grid-binance` stack lists: api-server, market-data-gateway, web, scheduler, billing-chain-listener, postgres, redis, prometheus, nginx, plus shared-db containers. **`trading-engine` is absent.**

**`BINANCE_LIVE_MODE=0`.** Confirmed in `.worktrees/full-v1/.env`:
```
BINANCE_LIVE_MODE=0
BINANCE_USDM_REST_BASE_URL=https://fapi.binance.com
```
Working code runs from worktree `.worktrees/full-v1` (branch `feature/full-v1`, HEAD `07d4722`); main repo is on `main` (HEAD `1b0ad35`).

### DB work from the handoff — ✅ reconciles exactly

Verification SQL on `grid-binance-postgres-1` / db `grid_binance`:

Visible flyingkid backtest tasks (expected exactly 3, all succeeded, all pass):
```
task_id                                          |status   |ann      |dd       |ann_pass|dd_pass|members|symbols
lp-martingale-aggressive-20260626-margin-v2      |succeeded|128.9630 |30.0000  |true    |true   |8      |8
lp-martingale-balanced-20260626-margin-v2        |succeeded|108.0591 |20.0000  |true    |true   |8      |8
lp-martingale-conservative-20260626-margin-v2    |succeeded|79.3593  |10.0000  |true    |true   |8      |8
```

Visible flyingkid martingale portfolios (expected exactly 3, all `pending_confirmation`):
```
portfolio_id                              |status                |risk_profile|ann      |dd       |candidates|symbols
mp_margin_v2_lp_aggressive_20260626       |pending_confirmation  |aggressive  |128.9630 |30.0000  |8         |8
mp_margin_v2_lp_balanced_20260626         |pending_confirmation  |balanced    |108.0591 |20.0000  |8         |8
mp_margin_v2_lp_conservative_20260626     |pending_confirmation  |conservative|79.3593  |10.0000  |8         |8
```

Cleanup artifacts verified:
- backtest_tasks: 3 visible + **44 archived** (`owner=archive+flyingkid2022@outlook.com`).
- martingale_portfolios: 3 visible + **5 archived**.
- Backup tables exist: `backtest_tasks_backup_margin_v2_lp_display_20260626`, `martingale_portfolios_backup_margin_v2_lp_display_20260626`.
- Superseded task `martingale-conservative-20260625-margin-v3` → status `cancelled`, archived.

Conservative portfolio `risk_summary` capital model matches the handoff JSON exactly:
```
source: margin_v2_lp_recombine
strategy_count: 16, enabled_strategy_count: 16, total_weight_pct: 100, max_leverage: 10
capital_model: {first_order_quote: "order notional", futures_margin: "notional / leverage",
                returns_drawdown_denominator: "planned margin capital"}
```
Note: `strategy_count=16` vs `candidate_count=8` → each candidate is split into a long + short strategy. This is the exact point Step 2 must verify (long/short weight splitting must not double-count capital).

### flyingkid Binance credentials — ✅ present (live path viable)

`user_exchange_credentials`: row for `flyingkid2022@outlook.com`, exchange `binance`, `api_key_masked = Y6xf****eN4f`, `encrypted_secret` present, created 2026-04-09, updated 2026-04-13.
`user_exchange_accounts`: market_scope `spot,usdm,coinm`, `is_active=true`, `checked_at = 2026-06-26 05:10:00 UTC`.

### Live Binance readback (open orders / positions / hedge mode / multi-assets mode) — ⏳ deferred to pre-smoke

Not performed in Step 1. Rationale: the engine is OFF and `BINANCE_LIVE_MODE=0`, so there is **zero live risk** regardless of current account state; this readback is safety-critical only immediately before placing orders. The plan repeats it in Step 6 ("Before start: Probe Binance and prove zero open orders and zero non-zero positions"), where it will be executed through the api-server's `/exchange/binance/account` / preconfigure flow (which loads the stored credentials and performs the signed readback). Extracting live API secrets and signing ad-hoc Binance calls now would add sensitive-credential handling with no safety benefit.

**Step 1 conclusion:** safe baseline established. No live risk present. Proceeding to Step 2 (parity audit), which is read-only.

---

## Step 2 — Live/Backtest Parity Audit

Audited via 4 parallel read-only subagents across capital model, signal/entry, exit/risk, and accounting/sync. **The critical findings below were each independently re-verified by direct DB query and source read.** The audit结论 is that the live runtime **cannot reproduce** the backtest that produced these portfolios. Three blocker-level gaps are verified.

### 🔴 BLOCKER 1 — LP member weights are doubled (sum = 200%, not 100%)

**Verified directly.** The conservative portfolio's 16 live strategies each carry the FULL member weight on both the long and short leg:

```
symbol   | dir   | portfolio_weight_pct
LTCUSDT  | long  | 36.082784
LTCUSDT  | short | 36.082784      <- pair sum 72.165568, intended 36.082784
DYDXUSDT | long  | 36.009540
DYDXUSDT | short | 36.009540      <- pair sum 72.019080
... (all 8 pairs doubled)
SUM over 16 strategies = 200.000000   (must be 100.0)
```

Root cause — `scripts/optimize_margin_v2_lp_portfolios.py:472`:
```python
live_strategy["portfolio_weight_pct"] = format(weight_pct, "f")   # full weight on EVERY leg, no /internal_count
```
Correct reference impl — `apps/api-server/src/services/martingale_publish_service.rs:597`, with explicit comment:
```rust
// long + short). Divide the item's portfolio weight equally among
// them so a single 100%-weighted long/short item does not reserve 200% of the capital.
let per_strategy_weight = item.weight_pct / Decimal::from(internal_count as u64);
```
The LP-recombine script bypasses the publish service (writes DB rows directly), so the protective division never runs. The backtest itself is correct — `portfolio_search.rs:1787` applies each candidate's weight once to its combined long_short curve. **The live config does not match the backtest's capital allocation.** At runtime (`main.rs:1550` `budget_cap = global_budget * weight_factor`) each pair reserves 2× its intended capital.

### 🔴 BLOCKER 2 — Stop-loss is a different quantity in live (~10x looser at 10x leverage)

Conservative candidates DO use `strategy_drawdown_pct` with `pct_bps` (BTCUSDT-long 9000, BTCUSDT-short 1800, DYDXUSDT-long 1200, DYDXUSDT-short 4000, …).

Backtest — `apps/backtest-engine/src/martingale/kline_engine.rs:1218-1232`:
```rust
let invested = state.capital_used_quote();        // = sum of leg MARGIN (notional/leverage)
let pnl = strategy_net_pnl(state, current_price)?; // net PnL incl. fees
let drawdown_pct = (-pnl).max(0.0) / invested * 100.0;
strategy_stop: drawdown_pct >= *pct_bps as f64 / 100.0   // pct_bps/100 => percent of MARGIN
```
Live — `apps/trading-engine/src/main.rs:1966-1972`:
```rust
let offset = average_entry * Decimal::from(pct_bps) / Decimal::from(10_000_u32);  // pct_bps/10000 => fraction of PRICE
Some(match direction { Long => average_entry - offset, Short => average_entry + offset })
```
Divergence for `pct_bps = 1200`, leverage 10x:
- Backtest stops at **1.2% adverse price move** (12% of margin).
- Live stops at **12% adverse price move** (~120% of margin).
For `pct_bps = 9000` (BTCUSDT-long): backtest stops ~9% price move; live stops at **90% price move** (effectively no stop).

The headline "max DD ≤ 10%" was produced by the margin-based stop. Live's price-based stop is ~10–75x looser, ignores leverage, and ignores costs. **The backtested drawdown control does not exist in live; live DD can be far worse than 10%.**

### 🔴 BLOCKER 3 — Backtest interval is 1m, live hardcodes 1h

All three source tasks were run at **1m**:
```
martingale-aggressive-20260625-margin-v2   interval=1m
martingale-aggressive-20260625-robust-v1   interval=1m
martingale-balanced-20260625-margin-v3     interval=1m
```
Live aggregates trade ticks into **1h** candles, hardcoded — `apps/trading-engine/src/main.rs:127-128`:
```rust
const HOUR_MS: i64 = 3_600_000;
let bucket_open_ms = tick.event_time_ms.div_euclid(HOUR_MS) * HOUR_MS;
```
ATR (period 21), ADX (14), entry triggers, and spacing are all evaluated on different bars. ATR(1m) ≠ ATR(1h). **Live signal timing and spacing will not match the 1m-screened candidates.**

### 🟠 GAP 4 — Three backtest entry guards have no live equivalent

Hardcoded in backtest `kline_engine.rs`, absent in `martingale_runtime.rs`:
- ATR > 2% of price → pause new cycle (`kline_engine.rs:154-157`).
- Portfolio drawdown > 6% → pause new cycle (`kline_engine.rs:143`).
- ADX > 45 → skip the safety (averaging) order (`kline_engine.rs:244-248`).

Live will enter / average-down in conditions the backtest refused. The reported DD partly relied on these filters.

### 🟠 GAP 5 — Take-profit via market close, Percent-only

Live does not place a resting TP limit order; it polls price and issues a **market close** on trigger (`main.rs:2030-2039`) → slippage vs backtest's exact-price fill. Also `martingale_percent_take_profit_price` returns `None` for ATR/Trailing/Mixed/Amount TP (`main.rs:1948`) — silently dropped live (conservative uses `percent.bps`, so this is latent, not active, for this portfolio).

### 🟠 GAP 6 — Funding-fee income backfill is non-monotonic >100 records

`read_usdm_income(symbol, None, 100)` (`crates/shared-binance/src/client.rs:1310-1338`) fetches the **last 100** income records with no `startTime`; `total_funding`/`total_commission`/`total_realized` shrink once >100 records accrue (`main.rs:2565-2625`). `tranId` absent/non-int → `income_id=0` collision silently dedupes distinct records. No test coverage. Fees themselves are fine (live uses real Binance commission, not double-counted — `statistics.rs:120-125` is mutually exclusive).

### 🟠 GAP 7 — TP/SL config-source staleness (the max_legs anomaly class)

The reported "safety leg despite max_legs=1 from a stale snapshot" is **not reproducible in the leg-count path** — `max_legs` is read fresh from `portfolio.config` every tick (`main.rs:419→1455`, no static cache). **But the same defect class persists in TP/SL:** leg generation reads `portfolio.config["portfolio_config"]` (refreshed each tick) while TP/SL reads `strategy.notes` JSON (`main.rs:936-940`), which is **only refreshed when a new cycle emits orders** (`main.rs:562-591`). A mid-cycle config edit (e.g. raising `pct_bps`) makes leg generation and TP/SL disagree until cycle reset. There is also a hardcoded fallback (`max_legs:3, multiplier:1, TP:Percent{100}, SL:None`, `main.rs:1428-1434`) if `notes` is malformed.

### What IS at parity (verified)

- `first_order_quote` = order notional; margin = notional/leverage; budget checks use margin; qty = notional/price — `martingale_runtime.rs:601-608`, matches `capital.rs`. ✅
- Safety-leg math (multiplier, max_legs gate) — live reuses backtest `compute_leg_notionals`. ✅
- `indicator_expression` evaluator and indicator set — shared compiled code (`martingale_runtime.rs:3-4,200`). ✅
- Cooldown — same `seconds*1000` ms window. ✅
- Fees — actual Binance commission, no double-count. ✅
- Hedge-mode position side, order status, realized/unrealized separation. ✅
- Ordinary grid order sync tests. ✅

### Step 2 conclusion — ⛔ PARITY FAILED. Live smoke and formal live start are blocked.

Per the plan's non-negotiable gate ("Do not start any live smoke or formal live executor until the live/backtest parity checks in this plan pass"), **Steps 6, 7, and 9 cannot proceed.** The backtest headline (conservative 79.36% ann / 10.00% DD) is not reproducible in the live runtime: capital is allocated 2×, the stop is 10–75x looser, and signals run on 1h instead of 1m bars. Executing the 50U smoke now would mechanically "pass" while validating the wrong risk model.


## Parity Fix (Steps 2→redux) — live aligned to backtest

User decision (2026-06-26): fix live to match backtest, re-validate (targets cons>50% / bal>90% / agg>110%). Executed via a superpowers TDD plan (`~/.claude/plans/quiet-dreaming-wreath.md`, 8 tasks, subagent-driven-development). All work committed on `main` (WIP baseline `fd39094`; docker build context is the main repo root). `BINANCE_LIVE_MODE=0` throughout; `trading-engine` never started.

### Commits (base `fd39094` → HEAD)
- `f398c85` Task 1: optimizer divides LP member weight by internal_count (BLOCKER 1)
- `d136246` Task 2: expose DEFAULT_FEE_BPS/DEFAULT_SLIPPAGE_BPS (pub)
- `69a1ede` Task 3: margin-based net-PnL drawdown stop-loss (BLOCKER 2)
- `4f3650a` Task 3 fix: SL entry cost incl. slippage; respect spot leverage
- `c2a6b94` Task 4: 1m candle aggregation via lib `martingale_candle` (BLOCKER 3)
- `534946f` Task 5: ATR>2% new-cycle pause guard
- `6fe0f07` Task 6: ADX>45 safety-leg skip guard (reorder for retry)
- `2ff45fd` Task 5 regression fix: lower ATR-regression test fixture volatility
- `3c3e50d` Task 7: portfolio drawdown>6% new-cycle pause guard (real equity wiring, no None stub)

### 🔴 Blockers — all CLOSED (re-verified)

**Blocker 1 (LP weights 200%→100%):** Regenerated the three portfolios with `--save`; SQL confirms each portfolio's 16 strategies now sum to **100.0000** (was 200.0000). Targets unchanged and met:
```
mp_margin_v2_lp_aggressive_20260626   weight_sum=100.0000  ann=128.9630 (>110) dd=30.0000
mp_margin_v2_lp_balanced_20260626     weight_sum=100.0000  ann=108.0591 (>90)  dd=20.0000
mp_margin_v2_lp_conservative_20260626 weight_sum=100.0000  ann=79.3593  (>50)  dd=10.0000
```
**Blocker 2 (SL was price-distance, ~10x looser):** `martingale_exit::martingale_strategy_drawdown_pct` now uses `invested = qty*avg/leverage` (== backtest `capital_used_quote`), `net = realized + unrealized − entry_fees − exit_cost`, triggers when `(-net)/invested*100 >= pct_bps/100` — matches backtest `strategy_net_pnl`/`capital_used_quote` (verified algebraically + numerically by opus review; at 10x/1200bps the trigger moved from 12% adverse to ~1.07% adverse). Cost basis shared with backtest (DEFAULT_FEE_BPS+DEFAULT_SLIPPAGE_BPS); spot leverage forced to 1.0.
**Blocker 3 (1h→1m):** Live aggregates trade ticks into **1m** candles (`martingale_candle::MINUTE_MS = 60_000`), matching the 1m backtest interval.

### Guards ported (GAP 4 closed)
- ATR > 2% of close → pause new cycle (`enforce_new_entry_controls`).
- ADX > 45 → skip safety leg, no index advance (retries) (`mark_leg_filled_with_context`).
- Portfolio drawdown > 6% → pause new cycle (new `MartingaleRuntimeContext.portfolio_drawdown_pct`, fed by a per-portfolio equity peak tracker in the reconcile loop: `budget + realized + unrealized`).

### Tests — all green
`cargo test` (6 suites): backtest-engine martingale 72, portfolio 30, backtest-worker 57, trading-engine martingale 15 + order_sync 7 + (full trading-engine suite 170), api-server martingale 24. **0 failed.** New TDD tests: weight halving, margin-SL parity (long+short), 1m bucket boundary, ATR pause (high+low), ADX skip (+no-ADX inert), portfolio-DD pause.

### Deferred follow-ups (accuracy/robustness; do NOT block re-validation, address before Step 8)
- **SL funding gap (I-2):** live `realized≈0` mid-cycle drops funding-fee accrual; live SL fires marginally later than backtest for positions held across funding. Needs a per-fill cycle marker (schema change). Biased safely looser; dominant term correct.
- **Portfolio-DD peak in-memory:** resets on engine restart (consistent with INDICATOR_FEEDS pattern). Persist peak across restart before long unattended runs.
- **Bar retention:** `IndicatorRuntimeContext.bars_by_symbol` is unbounded at 1m (shared by backtest+live; cap must live in the live caller, not the shared struct). Acceptable for short smoke/1000U.
- **TP fill mechanism:** live still issues a market close on TP (slippage) vs backtest exact-price fill; conservative uses `percent.bps` (price formula matches). Affects return accuracy, not the DD cap.
- **Funding income windowing:** `read_usdm_income(..., 100)` sliding window → non-monotonic >100 records; switch to `startTime`-anchored cumulative.
- **TP/SL config-source staleness:** `strategy.notes` refreshed only on new-cycle order emission; refresh each tick.

### Task 8 status
- Portfolio regeneration + weight/target verification: ✅ done.
- Re-audit parity (3 blockers + guards): ✅ closed.
- Full cargo suites: ✅ green.
- `api-server` + `trading-engine` docker build: in progress (verification only; `trading-engine` NOT started).

**Conclusion:** live/backtest parity restored. Resume the launch plan at **Step 5 (50U smoke design)**. Per the plan's non-negotiable gates, **Step 6 (live smoke) and Step 9 (1000U start) require explicit user authorization** — do not infer consent.

### Final whole-branch review fixes
Final opus review verdict: **Ship-with-follow-ups** (no Critical). Two Important cross-task findings fixed in `a022963`:
- **I-2:** ATR>2% and portfolio-DD>6% guards were leaking to safety-leg placement via shared `enforce_new_entry_controls`; backtest applies them new-cycle-only. Fixed: guards gated behind a `new_cycle: bool` param (`start_cycle`→true, `mark_leg_filled`→false). +test `high_atr_does_not_block_safety_leg_after_fill`.
- **I-1:** portfolio-DD equity now nets entry+exit costs (matches backtest `unrealized_pnl`), so the 6% pause fires at the right point.
Full `cargo test -p trading-engine`: **172 passed / 0 failed**. (Docker images last built at `3c3e50d`; rebuild `api-server`+`trading-engine` before the smoke since `a022963` landed after.)

**Deferred follow-ups (tracked, pre-1000U):** SL funding-fee gap (I-2 from Task 3 review); portfolio-DD peak in-memory (persist across restart); 1m bar retention (live-caller trim); TP market-close slippage; funding income windowing; TP/SL notes staleness.

## Step 5 — Clean 50U Smoke Design

**Objective:** validate every conservative-portfolio mechanic on Binance USDT-M with ≤50U at risk, from a clean state, as the first live test of the parity fix (margin SL, 1m candles, ATR/ADX/DD guards, weight=100). No stale state.

### Why a reduced smoke (not the full 8-symbol portfolio)
The conservative portfolio's 16 strategies sum to **99.17U of leg-0 margin** (Σ first_order_quote/leverage), which exceeds the 50U smoke budget. Per-symbol long+short leg-0 margins: DYDXUSDT 20.0, LTCUSDT 16.67, UNIUSDT 12.5, ICPUSDT/BTCUSDT/XRPUSDT/INJUSDT/FILUSDT 10.0 each. So a 2-symbol subset (~27U leg-0 margin) fits 50U with buffer and still exercises: multi-symbol coexistence, hedge mode (long+short legs open simultaneously), portfolio-weight splitting, and the full per-strategy path (which is identical across members). The 8-symbol diversification and exact weight split are already covered by the unit tests (16-strategy sum=100) and the 1000U preflight (Step 7).

### Smoke portfolio
- **Fresh portfolio** `mp_smoke_50u_<YYYYMMDD>` (NOT the conservative portfolio; created at Step 6 setup, status `pending_confirmation` → preflight only until authorized).
- **Members:** 2 symbols, long+short each (4 strategies). Recommended **LTCUSDT (lev 6)** + **ICPUSDT (lev 10)** — leg-0 margin 16.67 + 10.0 = **26.67U**, leaving ~23U buffer for legs/fees/slippage. (Alternate: LTCUSDT + BTCUSDT = 26.67U.)
- **Configs copied verbatim** from the corresponding conservative strategies: same `sizing.multiplier` (first_order_quote/multiplier/max_legs), same `stop_loss.strategy_drawdown_pct.pct_bps`, same `take_profit.percent.bps`, same `indicators` (atr/adx periods), same spacing. This guarantees the smoke exercises the parity-fixed code paths.
- **Budget:** `max_global_budget_quote = 50` USDT (margin). First orders: LTCUSDT long 30U / short 70U, ICPUSDT long 90U / short 10U notional — all ≥ Binance 5U min-notional; LTC short & ICP long ≥25U (the plan's "preferably ≥25U").

### Coverage (must validate before cleanup)
1. Exchange preconfigure: hedge mode ON, multi-assets mode OFF, per-symbol margin/leverage set (LTC 6x, ICP 10x).
2. Confirm-start preflight (`portfolio_projected_capital`): projected margin + fee/slippage buffer ≤ 50U; per-member first-order notional ≥ 5U min; exact executor strategy instances created.
3. First order placement (long+short): actual Binance notional == configured first_order_quote after qty rounding; DB order status matches Binance.
4. TP path: `percent.bps` trigger → market close (note: live uses market close, slippage vs backtest exact price — record the fill price).
5. **SL path (the parity fix):** drive a controlled adverse move and confirm the margin-based drawdown stop fires at ~`pct_bps/100` of margin (≈`pct_bps/(100×leverage)` adverse price), NOT the old `pct_bps/10000` price distance. This is the key live confirmation of BLOCKER 2's fix.
6. Safety-leg generation + budget blocking: a fill triggers the next leg; global/strategy margin cap blocks further legs before 50U breach.
7. Guards: observe ATR>2% new-cycle pause / ADX>45 safety-skip / portfolio-DD>6% pause fire or skip as conditions warrant (1m candles must warm up ~21min first).
8. Restart reconciliation: restart `trading-engine` mid-position; confirm no duplicate orders, no resurrected stale orders, positions/legs reconciled exactly.
9. Trade-fill sync, fee sync (actual Binance commission), funding-sync path (at least one funding event recorded, not double-counted).
10. Position flatten + cleanup: cancel all open orders, flatten all positions, restore `BINANCE_LIVE_MODE=0`.

### Hard stops (abort + cleanup immediately)
Any: duplicate order; stale order resurrected after restart; min-notional rejection (-1121, -8802); `-2019 Margin is insufficient`; TP/SL mismatch (especially SL firing at the wrong distance); fee/funding/statistics mismatch; any open order or non-zero position remaining after cleanup.

### Authorization boundary
This is DESIGN only — no orders placed. **Step 6 execution (engine start + live orders) requires explicit user authorization.** Before Step 6: rebuild `api-server`+`trading-engine` (code `a022963` is newer than the last image build at `3c3e50d`), probe Binance for zero orders/positions, create+preflight the smoke portfolio, then await the go-ahead.

## Step 6 — prep done; execution is session-bound (paused for final authorization)

**Prep completed (autonomous, no live orders):**
- Rebuilt `api-server` + `trading-engine` images from `a022963` (parity-fixed code; both built, exit 0).
- Restarted `api-server` with the new image (`trading-engine` NOT started; `BINANCE_LIVE_MODE=0`).
- Refined smoke members to **LTCUSDT (foq 30/70, lev 6) + BTCUSDT (foq 60/40, lev 10)** — both sides' first order ≥25U (the plan's preference), pair leg-0 margin 16.67+10 = **26.67U**, ~23U buffer under the 50U budget. Configs extract verbatim from the conservative portfolio (same SL `strategy_drawdown_pct`/TP `percent`/atr/cooldown/spacing).

**Execution boundary (needs the running api-server as flyingkid — session-bound):**
The remaining live steps — Binance zero-orders/positions probe (`/exchange/binance/account`), smoke portfolio create, `confirm-start` preflight (`portfolio_projected_capital` ≤ 50U), exchange preconfigure (hedge/leverage) — are driven through the api-server, which authenticates by user session. Driving them safely (with the preflight safety gate intact) requires the flyingkid session; bypassing the api-server to insert a running portfolio directly would skip the preflight and is rejected. The final live action — `BINANCE_LIVE_MODE=1` + start `trading-engine` + monitor — is the authorization gate.

**Awaiting:** how to drive the live execution (user via web UI; or provide a session; or pause).

## Step 6 — smoke BLOCKED by capital preflight (critical launch finding)

**What happened (live, flyingkid account, BINANCE_LIVE_MODE=1 window):**
- Login ✓; Binance account healthy, hedge_mode_ok, withdrawals_disabled.
- exchange-preflight ✓: LTCUSDT+BTCUSDT `open_order_count=0`, `nonzero_position_count=0`, hedge ready, multi-asset ready (account clean).
- exchange-preconfigure ✓: set LTC isolated/6x, BTC isolated/10x (account still clean, no orders).
- **confirm-start (50U) REJECTED (409): `projected margin 64585.1964 exceeds max_global_budget_quote 50.0000`.**
- No orders ever placed (trading-engine never started). Restored `BINANCE_LIVE_MODE=0`.

**Root cause:** `confirm_start_portfolio` runs `portfolio_projected_capital` → Σ per strategy of `planned_margin_quote` = Σ `leg_margin_series` (the FULL uncapped geometric series). For the smoke's Multiplier strategies (LTC long foq30/mult2.2/10legs, LTC short foq70/mult2.4/10legs, BTC long foq60/mult2.2/6legs, BTC short foq40/mult1.25/10legs) the full-series margin = 64585 U. The preflight requires `budget ≥ full series`, so 50U is rejected.

**This blocks the entire live chain, not just the smoke:**
- 50U smoke: rejected (64585 > 50).
- **1000U conservative launch (Step 7):** the conservative portfolio's full-series margin (8 Multiplier candidates, mult 1.25–2.4, 5–10 legs) is far larger than 1000U → confirm-start would reject there too. The 1000U launch is NOT viable via the normal confirm-start flow as-is.
- The runtime DOES enforce the budget cap at trade time (`martingale_runtime` margin exposure ≤ `portfolio_budget_quote`), so the 64585U figure is a worst-case-if-every-leg-fills projection, not what the runtime would actually use. The preflight is far more conservative than the runtime.

**Why:** the strategies use `Multiplier` sizing (uncapped geometric series). Deployment at a budget would need `BudgetScaled` sizing (`max_budget_quote` caps the series) — the "budget scaling" step the handoff explicitly flagged as unverified. That conversion is not applied; the portfolios are staged as Multiplier. This is consistent with the handoff's "display candidates, not permission to trade."

**Decision needed (blocks Step 6/7/9):**
1. Re-size to fit budget — e.g. smoke with `max_legs=1` (series = leg-0 = 26.67U < 50U) → partial smoke (validates leg-0 + TP + margin-SL, no safety legs); or convert strategies to `BudgetScaled` with per-strategy `max_budget_quote` so the series caps at budget. Changes mechanics vs the backtested Multiplier configs.
2. Change the preflight to project the **budget-capped** margin (apply `apply_global_budget_allocations` before `portfolio_projected_capital`, or cap `planned_margin_quote` at the per-strategy budget) — a code change to risk-critical preflight logic; would let 1000U pass but changes what "preflight passes" means.
3. Accept these portfolios are not deployable via confirm-start at 1000U (display-only) and stop the launch.

**Current safe state:** `BINANCE_LIVE_MODE=0`; api-server restarted; trading-engine not running; smoke portfolio `mp_smoke_50u_ltc_btc_20260626` left as `pending_confirmation` (harmless); Binance LTC/BTC set to isolated 6x/10x (harmless, no positions); zero orders/positions.
