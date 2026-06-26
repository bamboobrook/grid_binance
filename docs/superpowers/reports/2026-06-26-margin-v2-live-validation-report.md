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

