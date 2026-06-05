# Martingale Live DeepSeek Followup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining gaps before martingale portfolio strategies can be considered safe and observable in Binance live trading.

**Architecture:** Keep this work inside the live-trading path: API publish/preconfigure gates, Binance client contracts, trading-engine live executor, user-data stream reconciliation, and live statistics. Do not modify backtest search, backtest workers, FlyingKid cleanup, or Claude's current backtest optimization work.

**Tech Stack:** Rust workspace (`shared-binance`, `api-server`, `trading-engine`, `shared-db`), Node contract tests under `tests/verification`, Binance USD-M Futures REST and user-data stream APIs.

---

## Coordination Rules

- DeepSeek must not touch `apps/backtest-engine/**`, backtest worker code, FlyingKid task cleanup scripts, or `docs/superpowers/plans/2026-06-05-flyingkid-claude-followup-plan.md`.
- DeepSeek must not start, stop, clean, delete, or rerun Claude's backtest jobs.
- If a file already has unrelated modifications, inspect it and only edit the live-trading section needed for this plan.
- If committing, the commit message must include at least one of: `问题描述`, `复现路径`, or `修复思路`.
- Do not place real Binance orders during tests. All automated tests must use fakes, local mocks, or test-only request builders.

## Current Audit Findings

DeepSeek's recent live changes are directionally useful but not complete enough for production live trading:

- Good progress: `crates/shared-binance/src/client.rs` now covers live order placement, open-order readback, user-data stream listenKey start/keepalive, user trades, funding income, and exchange setting writes.
- Good progress: `apps/trading-engine/src/main.rs` added Redis live tick ingestion, per-strategy locking, user stream tasks, keepalive, execution update persistence, and martingale runtime start scaffolding.
- Good progress: `apps/api-server/src/services/martingale_exchange_preconfigure_service.rs` added explicit confirmations and exchange readback for Hedge Mode, margin type, and leverage.
- High-risk gap: `MartingalePublishService::confirm_start_portfolio` still only flips portfolio status to `running`; it does not require a successful exchange preconfigure summary, open-order/position safety checks, or proof that the trading engine can execute this portfolio.
- High-risk gap: running `martingale_live_portfolios` are converted to a readiness snapshot, but the visible live order submission path is still mainly `StrategyType::MartingaleGrid` inside `sync_strategy_orders`. A portfolio can look `running` while no portfolio orders are submitted.
- High-risk gap: trading-engine `ensure_futures_exchange_settings` can call position mode, margin type, and leverage setters from the runtime loop. Those setting changes should be API-confirmed pre-start operations, not repeated from the executor after the user has started live trading.
- High-risk gap: preconfigure applies margin/leverage without first blocking on existing open orders or nonzero positions. Binance position mode is account-level, and symbol margin/leverage changes can conflict with live state.
- Order correctness gap: martingale orders correctly avoid `reduceOnly` and set `positionSide`, but broader futures close paths still send `reduceOnly`; confirm no martingale close/TP path can hit that rule in Hedge Mode.
- Precision gap: current quantization uses tick/step when available, but live martingale uses `exchange_min_notional = 0` and does not visibly enforce minQty/minNotional after quantization.
- Statistics gap: user stream parsing handles `ORDER_TRADE_UPDATE`, but `ACCOUNT_UPDATE` is not parsed into account/position state. `snapshot_bundle` still reports realized PnL and fees as `"0"` in places, so live statistics can be incomplete.
- Recovery gap: user stream reconnect exists, but backfill after disconnect must reconcile REST `openOrders`, `userTrades`, `income`, and account/position state before claiming statistics are current.

## Binance Official API Baseline

Use these official USD-M Futures endpoints as the implementation contract:

- New Order: `POST /fapi/v1/order`; in Hedge Mode `positionSide` must be sent and `reduceOnly` cannot be sent. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Order>
- Change Position Mode: `POST /fapi/v1/positionSide/dual`; this changes Hedge/One-way mode on every symbol. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/Change-Position-Mode>
- Change Margin Type: `POST /fapi/v1/marginType`. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/Change-Margin-Type>
- Change Initial Leverage: `POST /fapi/v1/leverage`; valid leverage range is 1 to 125. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/Change-Initial-Leverage>
- Current All Open Orders: `GET /fapi/v1/openOrders`. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/Current-All-Open-Orders>
- Exchange Information: `GET /fapi/v1/exchangeInfo`; do not use `pricePrecision` as tick size or `quantityPrecision` as step size. Use filters such as `PRICE_FILTER.tickSize`, `LOT_SIZE.stepSize`, and `MIN_NOTIONAL.notional`. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information>
- Symbol Configuration: `GET /fapi/v1/symbolConfig`. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/account/rest-api/Symbol-Config>
- Start User Data Stream: `POST /fapi/v1/listenKey`; the stream closes after 60 minutes unless keepalive is sent. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/user-data-streams/Start-User-Data-Stream>
- Keepalive User Data Stream: `PUT /fapi/v1/listenKey`. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/user-data-streams/Keepalive-User-Data-Stream>
- Order stream event: `ORDER_TRADE_UPDATE`. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/user-data-streams/Event-Order-Update>
- Balance and position stream event: `ACCOUNT_UPDATE`; only changed balances/positions are pushed. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/user-data-streams/Event-Balance-and-Position-Update>
- Account Information V3: `GET /fapi/v3/account`; includes assets and positions. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/account/rest-api/Account-Information-V3>
- Futures Account Balance V3: `GET /fapi/v3/balance`. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/account/rest-api/Futures-Account-Balance-V3>
- Account Trade List: `GET /fapi/v1/userTrades`. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/Account-Trade-List>
- Income History: `GET /fapi/v1/income`. Source: <https://developers.binance.com/docs/derivatives/usds-margined-futures/account/rest-api/Get-Income-History>

## Files To Modify

- Modify: `crates/shared-binance/src/client.rs`
  Add request/response contract support for USD-M symbol config, account V3 positions, ACCOUNT_UPDATE parsing, min-notional metadata, and test-only request assertions.
- Modify: `apps/api-server/src/services/martingale_exchange_preconfigure_service.rs`
  Add open-order/position preflight blocking and persist a stronger readiness summary.
- Modify: `apps/api-server/src/services/martingale_publish_service.rs`
  Make `confirm_start_portfolio` require exchange readiness and live-executor eligibility before status becomes `running`.
- Modify: `apps/trading-engine/src/main.rs`
  Remove repeated exchange setting mutation from the runtime loop, connect running portfolios to actual live order execution, and add REST reconciliation after stream restart.
- Modify: `apps/trading-engine/src/order_sync.rs`
  Enforce Hedge Mode order payload rules and minQty/minNotional after quantization.
- Modify: `apps/trading-engine/src/execution_sync.rs`
  Apply futures `positionSide`, realized PnL, fee, and martingale fill state consistently.
- Modify: `apps/trading-engine/src/trade_sync.rs`
  Backfill fills from `userTrades` by symbol and order/client order id, and keep idempotency.
- Modify: `apps/trading-engine/src/statistics.rs`
  Ensure live stats include realized PnL, unrealized PnL, fees, funding, open orders, positions, and sync freshness.
- Modify or add tests: `apps/trading-engine/tests/order_sync.rs`, `apps/trading-engine/tests/execution_sync.rs`, `apps/trading-engine/tests/martingale_runtime.rs`, `tests/verification/martingale_portfolio_contract.test.mjs`, `tests/verification/martingale_publish_basket_contract.test.mjs`, `tests/verification/exchange_testing_contract.test.mjs`.

## Task 1: Protect Work Isolation

**Files:**
- Read: `git status --short`
- Read: `docs/superpowers/plans/2026-06-05-flyingkid-claude-followup-plan.md`

- [ ] **Step 1: Capture current worktree state**

Run:

```bash
git status --short
```

Expected: Existing backtest-related changes may appear. Do not revert them.

- [ ] **Step 2: Confirm live-only scope**

Run:

```bash
rg -n "confirm_start_portfolio|exchange_preconfigure|ORDER_TRADE_UPDATE|ACCOUNT_UPDATE|positionSide|reduceOnly|openOrders|symbolConfig|fapi/v3/account|fapi/v3/balance" apps crates tests
```

Expected: results are in API live services, Binance client, trading-engine, and tests. Do not edit `apps/backtest-engine/**`.

- [ ] **Step 3: Commit only isolation notes if needed**

If a commit is made for documentation or test scaffolding:

```bash
git add docs/superpowers/plans/2026-06-05-martingale-live-deepseek-followup-plan.md
git commit -m "docs: 问题描述 document martingale live followup scope"
```

Expected: no backtest files are staged.

## Task 2: Add Official Binance Live API Contracts

**Files:**
- Modify: `crates/shared-binance/src/client.rs`
- Test: `crates/shared-binance/src/client.rs` unit tests

- [ ] **Step 1: Add failing tests for Hedge Mode order payload**

Add or update tests proving USD-M Hedge Mode orders include `positionSide=LONG|SHORT`, omit `reduceOnly`, and keep `newClientOrderId` within Binance's 36-character rule.

Run:

```bash
cargo test -p shared-binance hedge_mode_order_payload -- --nocapture
```

Expected before implementation: test fails because the request builder or assertion helper does not exist.

- [ ] **Step 2: Add request builder/assertion helpers**

Implement test-only helpers that build signed request parameter vectors without sending to Binance. The tests must assert exact parameter names: `symbol`, `side`, `type`, `quantity`, `price`, `timeInForce`, `positionSide`, `newClientOrderId`; `reduceOnly` must be absent for Hedge Mode martingale orders.

- [ ] **Step 3: Add USD-M symbol config support**

Add a public method that uses:

```text
GET /fapi/v1/symbolConfig
```

Return normalized `symbol`, `margin_type`, `leverage`, and `max_notional_value`. Keep the existing `read_usdm_symbol_settings` behavior compatible, but prefer `symbolConfig` for readback when available.

- [ ] **Step 4: Add account V3 and ACCOUNT_UPDATE models**

Add structures for `GET /fapi/v3/account` positions and parse `ACCOUNT_UPDATE` user-data events into a typed result. Preserve existing `parse_user_data_message` behavior for `ORDER_TRADE_UPDATE`.

- [ ] **Step 5: Run Binance client tests**

Run:

```bash
cargo test -p shared-binance --lib -- --nocapture
```

Expected: all shared-binance unit tests pass without network calls.

## Task 3: Make Preconfigure Block Unsafe Live State

**Files:**
- Modify: `apps/api-server/src/services/martingale_exchange_preconfigure_service.rs`
- Modify: `crates/shared-binance/src/client.rs`
- Test: API service unit tests in `martingale_exchange_preconfigure_service.rs`

- [ ] **Step 1: Add failing preconfigure tests**

Add tests for these cases:

- Existing open order on any target symbol blocks margin/leverage changes.
- Nonzero position on any target symbol blocks margin type changes.
- Hedge Mode change is blocked when target symbols have open orders or positions.
- Read-only preflight returns `blocked` with exact symbols and reason; it does not call setters.

Run:

```bash
cargo test -p api-server martingale_exchange_preconfigure -- --nocapture
```

Expected before implementation: new tests fail.

- [ ] **Step 2: Implement open order and position checks**

Use Binance client methods backed by:

```text
GET /fapi/v1/openOrders
GET /fapi/v3/account
GET /fapi/v1/symbolConfig
```

Normalize symbols uppercase. Treat any open order or nonzero position amount on target symbols as a blocker before `set_usdm_position_mode`, `set_usdm_margin_type`, or `set_usdm_leverage`.

- [ ] **Step 3: Persist stronger readiness summary**

Persist `risk_summary.exchange_preconfigure` with:

```json
{
  "status": "ready|mismatch|blocked",
  "applied": true,
  "checked_at": "RFC3339",
  "blocked_symbols": [],
  "open_order_count": 0,
  "nonzero_position_count": 0,
  "hedge_mode": {},
  "symbols": []
}
```

Do not report `ready` when exchange readback is unavailable.

- [ ] **Step 4: Run preconfigure tests**

Run:

```bash
cargo test -p api-server martingale_exchange_preconfigure -- --nocapture
```

Expected: tests pass and prove setters are not called when blockers exist.

## Task 4: Gate Portfolio Start Before `running`

**Files:**
- Modify: `apps/api-server/src/services/martingale_publish_service.rs`
- Test: service unit tests and `tests/verification/martingale_portfolio_contract.test.mjs`

- [ ] **Step 1: Add failing start-gate tests**

Add tests proving `confirm_start_portfolio` rejects start when:

- `risk_summary.exchange_preconfigure.status` is missing.
- status is `mismatch` or `blocked`.
- status is `ready` but the snapshot is stale beyond a fixed TTL such as 10 minutes.
- the portfolio has no enabled strategy instances.
- the config cannot be converted to `MartingaleRuntimeConfig`.

Run:

```bash
cargo test -p api-server martingale_publish -- --nocapture
```

Expected before implementation: new tests fail.

- [ ] **Step 2: Implement readiness gate**

Before setting status to `running`, require:

```text
portfolio.status in pending_confirmation|paused
exchange_preconfigure.status == "ready"
exchange_preconfigure.checked_at within TTL
enabled strategy count > 0
no running/paused symbol conflict
config validates through MartingalePortfolioConfig::validate()
```

Return a conflict error that tells the UI to run exchange preconfigure first.

- [ ] **Step 3: Record start intent**

When start passes, persist a `risk_summary.live_start` object with `confirmed_at`, `executor_state: "pending_pickup"`, and `strategy_count`. The trading engine should update this to `executor_state: "started"` only after orders are created/submitted or explicitly staged.

- [ ] **Step 4: Run publish tests**

Run:

```bash
cargo test -p api-server martingale_publish -- --nocapture
node --test tests/verification/martingale_portfolio_contract.test.mjs tests/verification/martingale_publish_basket_contract.test.mjs
```

Expected: portfolio start cannot falsely show `running` without live prerequisites.

## Task 5: Connect Running Portfolios To Actual Live Execution

**Files:**
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Modify: `apps/trading-engine/src/order_sync.rs`
- Test: `apps/trading-engine/tests/martingale_runtime.rs`, `apps/trading-engine/tests/order_sync.rs`

- [ ] **Step 1: Add failing executor pickup tests**

Add tests proving a `running` `MartingalePortfolioRecord` is transformed into deterministic martingale live orders with:

- stable `client_order_id`
- correct `symbol`
- correct `side`
- correct `positionSide`
- no `reduceOnly` in Hedge Mode
- no duplicate submission after restart

Run:

```bash
cargo test -p trading-engine martingale -- --nocapture
```

Expected before implementation: at least one new test fails because portfolio orders are not submitted.

- [ ] **Step 2: Remove runtime setting mutation**

Stop calling `set_usdm_position_mode`, `set_usdm_margin_type`, and `set_usdm_leverage` from the trading-engine reconcile loop for already-started portfolios. The executor may read settings and block if they drift, but setting changes belong in the API preconfigure flow.

- [ ] **Step 3: Implement portfolio executor state**

For each running portfolio, trading-engine must:

1. Rebuild `MartingaleRuntimeConfig` from `portfolio.config.portfolio_config`.
2. Read exchange settings and verify they match `exchange_preconfigure`.
3. Recover existing open orders by `client_order_id`.
4. Create missing initial leg orders.
5. Submit orders through `BinanceClient::place_order` only after quantization/min-notional checks pass.
6. Persist `risk_summary.live_start.executor_state = "started"` with order ids.

- [ ] **Step 4: Run executor tests**

Run:

```bash
cargo test -p trading-engine martingale -- --nocapture
cargo test -p trading-engine order_sync -- --nocapture
```

Expected: no duplicate orders after restart; no exchange setting mutation during normal executor loop.

## Task 6: Enforce Order Quantization And Notional Rules

**Files:**
- Modify: `apps/trading-engine/src/order_sync.rs`
- Modify: `crates/shared-binance/src/client.rs`
- Test: `apps/trading-engine/tests/order_sync.rs`

- [ ] **Step 1: Add failing quantization tests**

Add tests covering:

- price floors to `PRICE_FILTER.tickSize`
- quantity floors to `LOT_SIZE.stepSize`
- quantity below `LOT_SIZE.minQty` is rejected before placement
- notional below `MIN_NOTIONAL.notional` is rejected before placement
- `newClientOrderId` longer than 36 chars is rejected before placement

Run:

```bash
cargo test -p trading-engine order_sync -- --nocapture
```

Expected before implementation: minQty/minNotional/client order id tests fail.

- [ ] **Step 2: Implement rule object**

Extend `OrderQuantizationRules` to include:

```rust
pub min_quantity: Option<Decimal>,
pub min_notional: Option<Decimal>,
pub client_order_id_max_len: usize,
```

Populate it from exchange metadata filters, not precision fields.

- [ ] **Step 3: Reject invalid orders before Binance call**

If validation fails, record a fatal order sync error and move the strategy/portfolio to a blocked or error-paused state with a user-readable event. Do not send the bad request to Binance.

- [ ] **Step 4: Run order tests**

Run:

```bash
cargo test -p trading-engine order_sync -- --nocapture
```

Expected: all order payload and validation tests pass.

## Task 7: Complete User Stream And REST Reconciliation

**Files:**
- Modify: `crates/shared-binance/src/client.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/execution_sync.rs`
- Modify: `apps/trading-engine/src/trade_sync.rs`
- Test: `apps/trading-engine/tests/execution_sync.rs`, `apps/trading-engine/tests/trade_sync.rs`

- [ ] **Step 1: Add failing ACCOUNT_UPDATE tests**

Add tests proving `ACCOUNT_UPDATE` updates balances, positions, margin type, unrealized PnL, and funding reason without relying on `ORDER_TRADE_UPDATE`.

Run:

```bash
cargo test -p trading-engine execution_sync -- --nocapture
```

Expected before implementation: account update tests fail.

- [ ] **Step 2: Implement ACCOUNT_UPDATE ingestion**

Parse and apply the event fields:

```text
B[].a, B[].wb, B[].cw, B[].bc
P[].s, P[].pa, P[].ep, P[].bep, P[].cr, P[].up, P[].mt, P[].iw, P[].ps
```

Only changed positions are pushed by Binance, so do not delete positions that are absent from one event.

- [ ] **Step 3: Add REST backfill after stream reconnect**

When a user stream starts or restarts, run a bounded reconciliation:

```text
GET /fapi/v1/openOrders
GET /fapi/v1/userTrades
GET /fapi/v1/income
GET /fapi/v3/account
GET /fapi/v3/balance
```

Use idempotency keys: exchange order id, client order id, trade id, income transaction id, and event time.

- [ ] **Step 4: Prevent keepalive leaks**

Ensure the keepalive task is tied to one stream lifecycle and is aborted when that stream disconnects. A reconnect must create exactly one new keepalive loop for that listenKey.

- [ ] **Step 5: Run reconciliation tests**

Run:

```bash
cargo test -p trading-engine execution_sync -- --nocapture
cargo test -p trading-engine trade_sync -- --nocapture
```

Expected: execution updates and REST backfill remain idempotent.

## Task 8: Fix Live Statistics Completeness

**Files:**
- Modify: `apps/trading-engine/src/statistics.rs`
- Modify: `crates/shared-binance/src/client.rs`
- Modify: API routes/services that expose analytics if needed
- Test: existing analytics/statistics tests or new focused tests

- [ ] **Step 1: Add failing statistics tests**

Add tests proving live stats include:

- realized PnL from fills or `REALIZED_PNL` income
- unrealized PnL from `ACCOUNT_UPDATE` or `/fapi/v3/account`
- fees from trades and `COMMISSION` income
- funding from `FUNDING_FEE` income
- open order count
- position count and notional
- last successful sync timestamp

Run:

```bash
cargo test -p trading-engine statistics -- --nocapture
```

Expected before implementation: tests fail where values are still zero placeholders.

- [ ] **Step 2: Replace zero placeholders**

Do not report `"0"` for unknown live values. Use real values when available and explicit `unknown`/missing fields when unavailable. The UI should distinguish zero from not yet synced.

- [ ] **Step 3: Add stale-data indicator**

Expose `last_user_stream_event_at`, `last_rest_reconcile_at`, and `stats_stale` so the front end can warn when data is older than the configured threshold.

- [ ] **Step 4: Run statistics tests**

Run:

```bash
cargo test -p trading-engine statistics -- --nocapture
```

Expected: stats are non-placeholder and idempotent.

## Task 9: End-To-End Verification Without Real Orders

**Files:**
- Test: `tests/verification/martingale_portfolio_contract.test.mjs`
- Test: `tests/verification/exchange_testing_contract.test.mjs`
- Test: Rust tests touched above

- [ ] **Step 1: Run focused Rust test suite**

Run:

```bash
cargo test -p shared-binance --lib -- --nocapture
cargo test -p api-server martingale_exchange_preconfigure -- --nocapture
cargo test -p api-server martingale_publish -- --nocapture
cargo test -p trading-engine order_sync -- --nocapture
cargo test -p trading-engine execution_sync -- --nocapture
cargo test -p trading-engine trade_sync -- --nocapture
cargo test -p trading-engine martingale -- --nocapture
```

Expected: all focused tests pass.

- [ ] **Step 2: Run verification contracts**

Run:

```bash
node --test tests/verification/martingale_portfolio_contract.test.mjs tests/verification/martingale_publish_basket_contract.test.mjs tests/verification/exchange_testing_contract.test.mjs
```

Expected: contracts pass. If a contract reads stale absolute worktree paths, fix the contract path to current repo files without changing production behavior.

- [ ] **Step 3: Produce final report**

Write a short report with:

- files changed
- tests run
- remaining live-trading risks
- confirmation that no backtest/FlyingKid/Claude files were modified
- confirmation that no real Binance order was sent

Expected: report is suitable for user review before any live deployment.

