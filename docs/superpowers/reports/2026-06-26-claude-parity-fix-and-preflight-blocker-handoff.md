# Handoff to GLM/ChatGPT: Margin-v2 Parity Fix + confirm-start Preflight Blocker

Date: 2026-06-26
From: Claude Code (executor of GLM's live-validation-and-1000u-launch plan)
To: GLM / ChatGPT (for review + next-step plan)
Repo: `/home/bumblebee/Project/grid_binance`, branch `main`, HEAD `a022963` (pushed to `origin/main`)
Owner account: `flyingkid2022@outlook.com`

> Read this whole document before planning. The headline: the live/backtest parity gaps from the prior audit are **fixed and verified**, but a **new, separate blocker** was discovered when attempting the 50U live smoke — the `confirm_start_portfolio` capital preflight makes the 1000U launch **non-viable as currently designed**. A decision is needed.

---

## 1. Executive summary

1. **Prior handoff verified.** The 3 staged portfolios (`mp_margin_v2_lp_{conservative,balanced,aggressive}_20260626`) are in the DB as claimed (Step 1 of the launch plan). Their backtest headline numbers (cons 79.36% ann / 10% DD, bal 108.06% / 20%, agg 128.96% / 30%) reconcile.
2. **Prior parity audit CONFIRMED and the gaps FIXED.** The Step-2 audit found the live `trading-engine` could not reproduce the backtest (3 blocker-level divergences + 3 missing guards). Per the user's decision, live was fixed to match backtest (8 TDD tasks, all reviewed clean). The 3 staged portfolios were regenerated with corrected weights (sum = 100, was 200). All cargo suites green; docker images rebuilt.
3. **NEW blocker discovered at live smoke (Step 6).** `confirm_start_portfolio` runs `portfolio_projected_capital`, which sums the **full uncapped geometric series** of every strategy's `Multiplier` sizing. For these multiplier-heavy strategies that projection is tens of thousands of USDT, so the preflight rejects any normal budget. The 50U smoke was rejected (`projected margin 64585.1964 exceeds max_global_budget_quote 50`); the **1000U conservative launch would be rejected the same way** (full series ≫ 1000U). The runtime, by contrast, enforces the budget cap at trade time, so the preflight is far more conservative than the runtime.
4. **No live orders were ever placed.** `trading-engine` was never started. State was returned to safe (`BINANCE_LIVE_MODE=0`, zero orders, zero positions).

The user has asked for this handoff so GLM can review and plan the next step. The open decision is in §6.

---

## 2. What was fixed (parity) — base `fd39094` → HEAD `a022963` (10 commits, all on `main`, pushed)

Working tree note: the repo's `feature/full-v1` worktree is STALE and is NOT what Docker builds. Docker's build context is `../..` = the main repo root. The pre-existing 119-file WIP was committed at `fd39094` as a baseline before this work.

| Commit | Task | What |
|---|---|---|
| `f398c85` | 1 | Optimizer divides LP member weight by internal_count (BLOCKER 1) |
| `d136246` | 2 | `DEFAULT_FEE_BPS`/`DEFAULT_SLIPPAGE_BPS` made `pub` in `kline_engine.rs` |
| `69a1ede` | 3 | Live stop-loss → margin-based net-PnL drawdown (BLOCKER 2) |
| `4f3650a` | 3-fix | SL entry cost includes slippage; respect spot leverage (review findings I-1/I-3) |
| `c2a6b94` | 4 | Live candle aggregation 1h → 1m via lib `martingale_candle` (BLOCKER 3) |
| `534946f` | 5 | ATR>2% new-cycle pause guard |
| `6fe0f07` | 6 | ADX>45 safety-leg skip guard (reorder so it retries) |
| `2ff45fd` | 5-fix | Lower ATR-regression test fixture volatility below 2% guard |
| `3c3e50d` | 7 | Portfolio drawdown>6% new-cycle pause guard |
| `a022963` | 7-fix | ATR/portfolio-DD guards new-cycle-only; portfolio-DD equity nets costs (final-review I-1/I-2) |

### The 3 original blockers — now closed

1. **LP weights 200%→100%.** `scripts/optimize_margin_v2_lp_portfolios.py:~472` divided each candidate's weight by `internal_count` (mirrors `martingale_publish_service.rs:597`). Portfolios regenerated; SQL confirms each portfolio's 16 strategies sum to `100.0000`.
2. **Stop-loss was price-distance, ~10x looser than backtest.** New `apps/trading-engine/src/martingale_exit.rs::martingale_strategy_drawdown_pct`: `invested = qty*avg/leverage` (== backtest `capital_used_quote`); `net = realized + unrealized − entry_fees − exit_cost`; trigger when `(-net)/invested*100 >= pct_bps/100` (backtest divides by 100, old live divided by 10000). Opus review verified the math reproduces backtest exactly. Cost basis and spot leverage parity added in `4f3650a`.
3. **1h → 1m candles.** `apps/trading-engine/src/martingale_candle.rs` (`MINUTE_MS = 60_000`); `main.rs::completed_martingale_indicator_bars` is now a thin wrapper.

### 3 guards ported from backtest (`kline_engine.rs`)
- ATR>2% of close → pause new cycle (`enforce_new_entry_controls`).
- ADX>45 → skip safety leg, no index advance so it retries (`mark_leg_filled_with_context`).
- Portfolio drawdown >6% → pause new cycle (new `MartingaleRuntimeContext.portfolio_drawdown_pct`, fed by a per-portfolio equity peak tracker in `main.rs`'s reconcile loop; equity nets entry+exit costs).
- Final-review fix (`a022963`): ATR and portfolio-DD guards are **new-cycle-only** (gated behind `enforce_new_entry_controls(.., new_cycle: bool)`), matching backtest — they were leaking to safety-leg placement via the shared function.

### Re-validation
- 6 cargo suites, 0 failed (backtest-engine martingale 72 + portfolio 30; backtest-worker 57; trading-engine martingale/order_sync + full trading-engine 172; api-server martingale 24).
- 3 portfolios regenerated, weights sum 100, targets still met (cons 79.36%>50, bal 108.06%>90, agg 128.96%>110; DD 10/20/30).
- `api-server` + `trading-engine` docker images rebuilt from `a022963` (both built, exit 0).

### Deferred follow-ups (tracked, accuracy/robustness — not risk-model parity)
- SL funding-fee gap: live `realized≈0` mid-cycle drops funding accrual; biases SL slightly looser; needs per-fill cycle marker (schema change).
- Portfolio-DD peak is in-memory only (resets on `trading-engine` restart).
- `IndicatorRuntimeContext.bars_by_symbol` unbounded at 1m (cap must live in the live caller, NOT the shared struct — the shared struct holds the backtest's ~1.8M bars).
- TP fill is market-close (slippage) vs backtest exact-price; conservative uses `percent.bps` (price formula matches).
- Funding income backfill sliding window (`read_usdm_income(.., 100)`) non-monotonic >100 records.
- TP/SL `strategy.notes` config-source staleness (refreshed only on new-cycle order emission).

---

## 3. NEW BLOCKER — confirm-start capital preflight (the reason for this handoff)

### What happened (live, on the flyingkid account)
Executed under `BINANCE_LIVE_MODE=1` (api-server only; `trading-engine` never started):
- Login ✓ (session token works).
- `GET /martingale-portfolios/{id}/exchange-preflight` ✓: LTCUSDT+BTCUSDT `open_order_count=0`, `nonzero_position_count=0`, hedge ready, multi-asset ready (account clean).
- `POST .../exchange-preconfigure` ✓: set LTC isolated/6x, BTC isolated/10x (still clean, no orders).
- `POST /backtest/portfolios/{id}/confirm-start` `{max_global_budget_quote:"50"}` → **HTTP 409**: `projected margin 64585.1964 exceeds max_global_budget_quote 50.0000`.
- Restored `BINANCE_LIVE_MODE=0`. Zero orders/positions throughout.

### Root cause (file:line evidence)
- `apps/api-server/src/services/martingale_publish_service.rs:641-669` `portfolio_projected_capital` = Σ over strategies of `planned_margin_quote(...)`, **no budget capping**.
- `apps/backtest-engine/src/martingale/capital.rs:76` `planned_margin_quote` = Σ `leg_margin_series`.
- `capital.rs` `leg_margin_series` → `leg_notional_series(sizing, f64::MAX, min_notional)` — the budget param is `f64::MAX` (uncapped).
- `apps/backtest-engine/src/martingale/rules.rs` `compute_leg_notionals`:
  - `Multiplier { first_order_quote, multiplier, max_legs }` → `geometric_series(...)` (full series), `enforce_budget=true` but only errors if total > the passed budget (which is `f64::MAX` here, so no error).
  - `BudgetScaled { ..., max_budget_quote }` → scales the whole series down so Σ ≤ `min(max_budget_quote, budget)` (proportional shrink of every leg).

### Why this blocks everything
The staged portfolios use **`Multiplier` sizing** with high multipliers and many legs (e.g. LTC long foq30/mult2.2/10legs, LTC short foq70/mult2.4/10legs). The full geometric-series margin is enormous:
- Smoke (2 symbols LTC+BTC, 4 strategies): **64 585 USDT**.
- Conservative (8 candidates): far larger (each candidate's full series is tens of thousands).

`confirm_start_portfolio` (`martingale_publish_service.rs:342-352`) rejects when `projected_margin > max_global_budget_quote`. Therefore:
- **50U smoke: rejected** (64 585 > 50).
- **1000U conservative launch: would be rejected** (full series ≫ 1000U). The Step-7 1000U preflight and Step-9 1000U start are **not viable via the normal `confirm-start` flow as-is.**

### Runtime vs preflight inconsistency (the crux)
The live runtime **does** enforce the budget cap at trade time: `martingale_runtime` margin exposure ≤ `portfolio_budget_quote` (the global margin cap), and `cap_strategy_budget` / `apply_global_budget_allocations` set per-strategy caps. So at 50U the runtime would place full-size leg 0 (~26.67U margin) and stop adding legs when the budget is hit. The preflight's 64 585U figure is a "every leg of every strategy fills at full size" worst case that the runtime never permits. **The preflight is far more conservative than the runtime.**

### Why BudgetScaled doesn't rescue it
Converting to `BudgetScaled` would make `planned_margin_quote` cap at `max_budget_quote`, but it scales **every** leg by `budget/total` (e.g. 12.5/64585 ≈ 0.0002), shrinking leg 0 to ~0.006 USDT notional — far below Binance USDT-M's ~5 USDT min-notional → rejected by `validate_min_notional`. So BudgetScaled-at-small-budget breaks min-notional. This is exactly the "budget scaling keeps Binance min-notional constraints" caveat in the prior handoff — it is not satisfiable for these high-multiplier strategies at a small budget.

This is consistent with the prior handoff's own words: *"These portfolios are final backtest display candidates, not permission to trade."*

---

## 4. Current safe state

- `BINANCE_LIVE_MODE=0` (`.worktrees/full-v1/.env`); `api-server` restarted on it.
- `trading-engine`: NOT running.
- Binance USDT-M (flyingkid): `open_order_count=0`, `nonzero_position_count=0`. LTCUSDT/BTCUSDT left set to isolated 6x/10x and hedge mode on (harmless — no positions).
- DB:
  - The 3 staged portfolios regenerated with weights summing to 100, `pending_confirmation`.
  - A smoke portfolio `mp_smoke_50u_ltc_btc_20260626` exists, `pending_confirmation`, LTC+BTC (4 strategies, weight 25 each), with `martingale_portfolio_items` rows (strategy_instance_ids `msi_smoke_50u_ltc`/`msi_smoke_50u_btc`). **It cannot be confirm-started at 50U (preflight).** It can be deleted or repurposed.

---

## 5. Smoke portfolio creation notes (for whoever continues)

The smoke portfolio had to be created by **DB insert** (not the publish API), because `publish_portfolio` (`martingale_publish_service.rs:185`) requires all candidates to share one `task_id`, but LTC (robust-v1) and BTC (balanced-v3) are cross-task. The optimizer-created portfolios already bypass publish for the same reason. The smoke row + `martingale_portfolio_items` rows were inserted directly. `martingale_portfolio_items` PK is `strategy_instance_id` (globally unique), so the smoke uses new IDs that also match `config.portfolio_config.strategies[].strategy_instance_id`.

API driving was done via `docker exec grid-binance-web-1 node -e 'fetch("http://api-server:8080/...")'` (the api-server container has no curl; the web container's node can reach `api-server:8080` on the docker net; api-server routes have no `/api` prefix — that's the web BFF). Auth is `Authorization: Bearer <session_token>` from `POST /auth/login`.

---

## 6. Open decision for GLM (the next-step plan needs one of these)

The preflight blocker has to be resolved before any live smoke or 1000U start. Options:

1. **Smoke with `max_legs=1` (or otherwise budget-fitted) to validate the parity fixes live.** Full series = leg-0 = 26.67U < 50U → preflight passes. Validates leg-0 placement, TP (`percent.bps`), the **margin-based SL fix**, 1m candles, and the ATR/portfolio-DD entry guards on live Binance. Does NOT validate safety legs or restart-reconciliation-with-fills. Gets live confirmation of the parity work; does not address the 1000U blocker.
2. **Change the preflight to project the budget-capped margin** (what the runtime actually allows) instead of the full uncapped series — e.g. apply `apply_global_budget_allocations` before `portfolio_projected_capital`, or cap each strategy's projected margin at its per-strategy budget, or change `leg_notional_series` to take the real budget. This is a **risk-critical code change** to `martingale_publish_service.rs` / `capital.rs`; it changes what "preflight passes" means and must be designed carefully (what is the right projection? leg-0 + full-size legs until the budget is hit?). After it, both the full smoke and the 1000U launch could pass.
3. **Re-design the strategies** (lower multiplier / fewer legs / smaller first_order_quote) so the full geometric series fits the budget, then **re-run the backtest** (current results are for the high-multiplier configs and would no longer apply). Largest effort.
4. **Accept these portfolios are display-only** at normal budgets and stop the launch.

My (Claude's) assessment, FWIW: the preflight's full-uncapped-series projection is inconsistent with the runtime's budget-capped behavior and is the most likely thing to fix (option 2), but it is risk-critical and is GLM's call. Option 1 is a cheap way to get live validation of the parity fixes regardless of how 2/3/4 are decided.

---

## 7. Key references for planning

- Plan executed: `docs/superpowers/plans/2026-06-26-glm-live-validation-and-1000u-launch-plan.md`
- Prior handoff: `docs/superpowers/reports/2026-06-26-margin-v2-final-backtest-handoff.md`
- Full validation report (Step 1, Step 2 audit, parity fix, re-validation, smoke attempt, this blocker): `docs/superpowers/reports/2026-06-26-margin-v2-live-validation-report.md`
- Parity fix plan (8 TDD tasks): `~/.claude/plans/quiet-dreaming-wreath.md`
- Preflight code: `apps/api-server/src/services/martingale_publish_service.rs:234-421` (`confirm_start_portfolio`), `:641-669` (`portfolio_projected_capital`); `apps/backtest-engine/src/martingale/capital.rs:65-90`; `apps/backtest-engine/src/martingale/rules.rs` (`compute_leg_notionals`).
- Runtime budget cap (works correctly): `apps/trading-engine/src/martingale_runtime.rs:548-587` (margin budget gates), `apps/trading-engine/src/main.rs:1571-1629` (`apply_global_budget_allocations`, `cap_strategy_budget`).
- Docker build context = main repo root (`deploy/docker/docker-compose.yml` `context: ../..`); env file `.worktrees/full-v1/.env`.
