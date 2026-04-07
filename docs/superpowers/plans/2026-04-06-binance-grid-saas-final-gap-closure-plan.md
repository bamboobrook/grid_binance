# Binance Grid SaaS Final Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining spec gaps so `full-v1` matches the frozen March 31 Binance Grid SaaS design across user-facing strategy flows, real-time runtime execution, billing enforcement, notifications, analytics, and membership reminders.

**Architecture:** Keep the current Rust service split and Next.js web app, but remove the last mock/business-placeholder paths and connect the runtime into a real market-tick pipeline. Trading-critical truth must come from persisted exchange state, scheduler snapshots, chain listener enforcement, and worker-produced runtime events instead of form booleans or page-local placeholders.

**Tech Stack:** Rust, Axum, Tokio, SQLx/shared-db, PostgreSQL, Redis, Next.js App Router, TypeScript, Playwright, Binance REST/WebSocket, EVM/Solana RPC polling.

---

## File Structure

### Public auth email delivery closure
- Modify: `apps/api-server/src/services/auth_service.rs`
- Modify: `apps/api-server/tests/auth_flow.rs`
- Modify: `apps/web/src/app/api/auth/register/route.ts`
- Modify: `apps/web/src/app/api/auth/password-reset/route.ts`
- Modify: `.env.example`
- Modify: `docs/deployment/env-and-secrets.md`
- Modify: `docs/user-guide/getting-started.md`

### User product truth and strategy composer
- Modify: `apps/web/src/lib/api/user-product-state.ts`
- Modify: `apps/web/src/app/app/dashboard/page.tsx`
- Modify: `apps/web/src/app/app/orders/page.tsx`
- Modify: `apps/web/src/app/app/security/page.tsx`
- Modify: `apps/web/src/app/app/strategies/page.tsx`
- Modify: `apps/web/src/app/app/strategies/new/page.tsx`
- Modify: `apps/web/src/app/app/strategies/[id]/page.tsx`
- Modify: `apps/web/src/app/app/billing/page.tsx`
- Modify: `apps/web/src/app/app/telegram/page.tsx`
- Modify: `apps/web/src/app/api/user/strategies/create/route.ts`
- Modify: `apps/web/src/app/api/user/strategies/[id]/route.ts`
- Modify: `apps/web/src/app/api/user/billing/route.ts`
- Modify: `apps/web/src/app/api/user/security/route.ts`
- Test: `tests/e2e/user_app.spec.ts`
- Test: `tests/e2e/user_commercial_flows.spec.ts`

### Server pre-flight and strategy payload closure
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `apps/api-server/src/routes/strategies.rs`
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Modify: `crates/shared-domain/src/strategy.rs`

### Market tick bus and runtime execution closure
- Modify: `crates/shared-events/src/market.rs`
- Modify: `crates/shared-db/src/redis/client.rs`
- Modify: `apps/market-data-gateway/src/binance_ws.rs`
- Modify: `apps/market-data-gateway/src/main.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/strategy_runtime.rs`
- Modify: `apps/trading-engine/src/order_sync.rs`
- Modify: `apps/trading-engine/src/trade_sync.rs`
- Modify: `apps/trading-engine/src/execution_effects.rs`
- Modify: `apps/trading-engine/src/execution_sync.rs`
- Test: `apps/market-data-gateway/tests/runtime_wiring.rs`
- Test: `apps/trading-engine/tests/grid_runtime.rs`
- Test: `tests/simulation/trailing_tp.rs`
- Test: `tests/simulation/strategy_rebuild.rs`

### Billing enforcement and user reminder closure
- Modify: `apps/api-server/src/services/membership_service.rs`
- Modify: `apps/api-server/src/routes/billing.rs`
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `apps/billing-chain-listener/src/processor.rs`
- Modify: `apps/billing-chain-listener/src/rpc.rs`
- Modify: `apps/scheduler/src/jobs/reminders.rs`
- Modify: `apps/scheduler/src/jobs/membership_grace.rs`
- Modify: `apps/web/src/app/app/layout.tsx`
- Modify: `apps/web/src/app/app/billing/page.tsx`
- Test: `apps/api-server/tests/membership_flow.rs`
- Test: `apps/api-server/tests/admin_deposit_flow.rs`
- Test: `apps/api-server/tests/admin_address_pools_flow.rs`

### Analytics, statistics, and business notification closure
- Modify: `crates/shared-binance/src/client.rs`
- Modify: `apps/scheduler/src/main.rs`
- Modify: `apps/api-server/src/services/analytics_service.rs`
- Modify: `apps/api-server/src/services/telegram_service.rs`
- Modify: `apps/api-server/tests/analytics_flow.rs`
- Modify: `apps/api-server/tests/notification_flow.rs`

### Deployment and docs closure
- Modify: `.env.example`
- Modify: `deploy/docker/docker-compose.yml`
- Modify: `docs/deployment/env-and-secrets.md`
- Modify: `docs/deployment/docker-compose.md`
- Modify: `docs/user-guide/create-grid-strategy.md`
- Modify: `docs/user-guide/manage-strategy.md`
- Modify: `docs/user-guide/membership-and-payment.md`
- Modify: `docs/user-guide/security-center.md`

## Task 0: Deliver Verification And Reset Codes Through Real Email Paths

**Files:**
- Modify: `apps/api-server/src/services/auth_service.rs`
- Modify: `apps/api-server/tests/auth_flow.rs`
- Modify: `apps/web/src/app/api/auth/register/route.ts`
- Modify: `apps/web/src/app/api/auth/password-reset/route.ts`
- Modify: `.env.example`
- Modify: `docs/deployment/env-and-secrets.md`
- Modify: `docs/user-guide/getting-started.md`

- [ ] Step 1: Add a real outbound email delivery path for verification codes and password-reset codes, driven by runtime configuration instead of returning the codes to the browser in production flows.
- [ ] Step 2: Keep deterministic testing support, but stop storing issued codes in browser cookies on accepted runtime paths.
- [ ] Step 3: Update the public auth routes and user-facing docs so registration and password reset clearly tell the user to check email for the code.
- [ ] Step 4: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test auth_flow`.

## Task 1: Remove Remaining Mock Business State From The User App

**Files:**
- Modify: `apps/web/src/lib/api/user-product-state.ts`
- Modify: `apps/web/src/app/app/dashboard/page.tsx`
- Modify: `apps/web/src/app/app/orders/page.tsx`
- Modify: `apps/web/src/app/app/security/page.tsx`
- Modify: `apps/web/src/app/app/telegram/page.tsx`
- Test: `tests/e2e/user_app.spec.ts`

- [ ] Step 1: Delete the business-truth helpers in `user-product-state.ts` that still fabricate strategies, fills, billing state, and trade history, leaving only transient flash helpers if still needed.
- [ ] Step 2: Change dashboard, orders, security, and Telegram pages to fetch backend truth directly from `/analytics`, `/profile`, `/notifications`, `/telegram/binding`, `/billing/overview`, and the real orders endpoints instead of deriving values from the in-memory store or fake order IDs.
- [ ] Step 3: Add a shared app-level popup surface in `apps/web/src/app/app/layout.tsx` that reads notification records flagged with `show_expiry_popup` and renders the expiry/grace reminder banner or modal on page load.
- [ ] Step 4: Make dashboard show real wallet/account activity summaries and keep the expiry popup wired to backend notifications, then run `pnpm test:e2e -- --grep "user"` and update `tests/e2e/user_app.spec.ts` until the auth/security/dashboard flows pass without relying on mock state.

## Task 2: Replace The Strategy UI Hardcodes With Full Draft Payloads

**Files:**
- Modify: `apps/web/src/app/app/strategies/page.tsx`
- Modify: `apps/web/src/app/app/strategies/new/page.tsx`
- Modify: `apps/web/src/app/app/strategies/[id]/page.tsx`
- Modify: `apps/web/src/app/api/user/strategies/create/route.ts`
- Modify: `apps/web/src/app/api/user/strategies/[id]/route.ts`
- Test: `tests/e2e/user_commercial_flows.spec.ts`

- [ ] Step 1: Expand the new/edit strategy forms so the user can edit all required draft inputs from the spec: symbol search term with fuzzy results, market, mode, generation, per-level entry price, per-level quantity, per-level take profit, optional per-level trailing, overall TP, optional overall SL, and post-trigger action.
- [ ] Step 2: Remove the hardcoded three-level payloads and client-authored readiness booleans from the strategy create and update routes; instead serialize the actual form levels and let the backend compute readiness, and never auto-pause a running strategy implicitly from the web route.
- [ ] Step 3: Add list-page controls for batch pause, batch delete, and stop-all, plus a user-facing template-apply entry that clones admin templates into user-owned drafts without routing through `/admin/*`.
- [ ] Step 4: Run `pnpm test:e2e -- --grep "user commercial"` and extend coverage to verify draft save, pre-flight failure visibility, template-derived draft editing, batch pause/delete, and stop-all.

## Task 3: Move Pre-Flight To Full Server Truth

**Files:**
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Modify: `crates/shared-domain/src/strategy.rs`

- [ ] Step 1: Remove the request-time readiness booleans from the accepted strategy payload path, keeping only user-supplied trading parameters and deriving readiness strictly from membership, exchange validation snapshots, symbol metadata, conflict checks, and exchange snapshots.
- [ ] Step 2: Extend pre-flight to compute `filters_and_notional`, `margin_or_leverage`, and `balance_or_collateral` from stored symbol metadata and latest wallet/account snapshots instead of trusting form values.
- [ ] Step 3: Add a non-blocking warning field for trailing TP explaining taker execution and higher fees, while still failing when `trailing_bps > take_profit_bps`.
- [ ] Step 4: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow` and keep only server-derived pre-flight assertions.

## Task 4: Publish Market Ticks Beyond The Gateway Process

**Files:**
- Modify: `crates/shared-events/src/market.rs`
- Modify: `crates/shared-db/src/redis/client.rs`
- Modify: `apps/market-data-gateway/src/binance_ws.rs`
- Modify: `apps/market-data-gateway/src/main.rs`
- Test: `apps/market-data-gateway/tests/runtime_wiring.rs`

- [ ] Step 1: Add Redis-backed publish and subscribe helpers for normalized `MarketTick` payloads in `shared-db`, using one channel namespace for Binance market ticks.
- [ ] Step 2: Make `market-data-gateway` publish every normalized subscribed tick to Redis after updating local gateway health, instead of dropping the event inside process memory.
- [ ] Step 3: Keep the current active-symbol-only subscription refresh loop, but also expose stale-stream metrics based on published tick freshness.
- [ ] Step 4: Run `source "$HOME/.cargo/env" && cargo test -p market-data-gateway --test runtime_wiring` and add assertions that live ticks are published to Redis channels.

## Task 5: Connect Trading Engine Runtime To Market Ticks

**Files:**
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/strategy_runtime.rs`
- Modify: `apps/trading-engine/src/order_sync.rs`
- Modify: `apps/trading-engine/src/trade_sync.rs`
- Modify: `apps/trading-engine/src/execution_sync.rs`
- Modify: `apps/trading-engine/src/execution_effects.rs`
- Test: `apps/trading-engine/tests/grid_runtime.rs`
- Test: `tests/simulation/trailing_tp.rs`
- Test: `tests/simulation/strategy_rebuild.rs`

- [ ] Step 1: Subscribe `trading-engine` to the Redis market-tick channel and route each tick to matching running strategies by `symbol + market`.
- [ ] Step 2: Use `StrategyRuntimeEngine::on_price()` as the accepted runtime path so maker TP, trailing TP, overall TP, and overall SL can emit live exit intents.
- [ ] Step 3: When exit intents are produced in live mode, place/cancel the corresponding Binance orders or market closes and persist resulting runtime events, fills, and state transitions.
- [ ] Step 4: Rework resume/stop so resume rebuilds from current exchange positions and latest market price, and stop performs real market close requests before marking the strategy stopped.
- [ ] Step 5: On runtime failure, auto-pause the strategy, persist a user-readable remediation event, and emit in-app plus Telegram runtime-error notifications.
- [ ] Step 6: Run `source "$HOME/.cargo/env" && cargo test -p trading-engine --test grid_runtime && cargo test --test trailing_tp && cargo test --test strategy_rebuild`.

## Task 6: Complete User Data Stream And Exchange Snapshot Fidelity

**Files:**
- Modify: `crates/shared-binance/src/client.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/scheduler/src/main.rs`
- Modify: `apps/api-server/src/services/analytics_service.rs`
- Test: `apps/api-server/tests/analytics_flow.rs`

- [ ] Step 1: Extend Binance user-data parsing to cover account and position update events needed for wallet, balance, and futures exposure refresh.
- [ ] Step 2: Stop writing placeholder fee and funding values in snapshot collection; persist the best available exchange-side fees/funding fields and keep missing values explicit rather than silently using incorrect substitutes.
- [ ] Step 3: Update scheduler strategy snapshot generation to calculate per-strategy unrealized PnL and funding totals from runtime positions plus latest account data instead of hardcoding zero.
- [ ] Step 4: Update analytics aggregation so account-level and strategy-level fees/funding/unrealized values come from stored snapshots first and no longer default strategy funding to zero when data exists.
- [ ] Step 5: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test analytics_flow && cargo test -p scheduler`.

## Task 7: Enforce Billing Confirmation Policy On Every Path

**Files:**
- Modify: `apps/api-server/src/services/membership_service.rs`
- Modify: `apps/api-server/src/routes/billing.rs`
- Modify: `apps/billing-chain-listener/src/processor.rs`
- Test: `apps/api-server/tests/membership_flow.rs`
- Test: `apps/api-server/tests/admin_deposit_flow.rs`

- [ ] Step 1: Remove or harden the manual `/billing/orders/match` path so it cannot bypass per-chain confirmation thresholds defined in system config.
- [ ] Step 2: Keep chain-listener processing as the only automatic credit path, and require abnormal/manual handling for anything that does not arrive through the confirmation-aware listener flow.
- [ ] Step 3: Preserve the exact-amount, wrong-token, expired-order, and ambiguous-match handling already present, but add tests that prove confirmation bypass is impossible.
- [ ] Step 4: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test membership_flow --test admin_deposit_flow`.

## Task 8: Turn Sweep Jobs Into Real Executable Chain Work

**Files:**
- Modify: `apps/api-server/src/services/membership_service.rs`
- Modify: `apps/billing-chain-listener/src/rpc.rs`
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `.env.example`
- Modify: `docs/deployment/env-and-secrets.md`
- Test: `apps/api-server/tests/admin_address_pools_flow.rs`

- [ ] Step 1: Replace the placeholder `completed` sweep transfers and fake tx hashes with queued or submitted sweep execution records that carry real chain-specific submission metadata.
- [ ] Step 2: Add chain-specific sweep execution hooks driven by configured private keys or signer material for ETH, BSC, and SOL pool addresses, and persist terminal success or failure state plus real tx hash.
- [ ] Step 3: Keep audit logging for every requested transfer, submitted sweep, and final sweep status update.
- [ ] Step 4: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test admin_address_pools_flow`.

## Task 9: Finish Business-Event Notification Coverage

**Files:**
- Modify: `apps/api-server/src/services/telegram_service.rs`
- Modify: `apps/trading-engine/src/execution_effects.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/billing-chain-listener/src/processor.rs`
- Modify: `apps/scheduler/src/jobs/reminders.rs`
- Modify: `apps/scheduler/src/jobs/membership_grace.rs`
- Test: `apps/api-server/tests/notification_flow.rs`

- [ ] Step 1: Add automatic notification emission for overall TP, overall SL, runtime error auto-pause, API credential invalidation, deposit confirmed Telegram delivery, and grace-expiry popup events.
- [ ] Step 2: Ensure every emitted business event writes both notification log rows and Telegram delivery status rows when a binding exists.
- [ ] Step 3: Keep notification copy user-readable and aligned with the step-based failure guidance returned by pre-flight and runtime error flows.
- [ ] Step 4: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test notification_flow`.

## Task 10: Close Docs, Compose, And Acceptance

**Files:**
- Modify: `deploy/docker/docker-compose.yml`
- Modify: `docs/deployment/docker-compose.md`
- Modify: `docs/deployment/env-and-secrets.md`
- Modify: `docs/user-guide/create-grid-strategy.md`
- Modify: `docs/user-guide/manage-strategy.md`
- Modify: `docs/user-guide/membership-and-payment.md`
- Modify: `docs/user-guide/security-center.md`
- Test: `tests/verification/compose.test.mjs`

- [ ] Step 1: Document the new runtime requirements for Redis market-tick channels, real sweep signer configuration, and the popup reminder behavior.
- [ ] Step 2: Update user guides so they match the actual strategy composer, pre-flight warning flow, stop/pause semantics, billing exact-amount rules, and 2FA/security behavior.
- [ ] Step 3: Verify compose wiring exposes the required env vars for Binance live mode, Telegram, chain RPC, sweep signing, and Redis-backed tick transport.
- [ ] Step 4: Run `node --test tests/verification/compose.test.mjs`.

## Spec Coverage Map

- User auth, email delivery, security center, backend-truth web flows, and popup reminders: Tasks 0, 1, and 10
- User strategy creation, editing, batch actions, save-before-start, template-derived draft editing, and pre-flight visibility: Tasks 2 and 3
- Full server-derived pre-flight: Task 3
- WebSocket active-symbol trading path, TP/SL, stop/resume, runtime error handling: Tasks 4 and 5
- Account/funding/fee/statistics accuracy and exports: Task 6
- Chain confirmation enforcement, exact amount rules, abnormal handling, address pools, grace expiry: Task 7
- Sweep execution and audit: Task 8
- Telegram and in-app business event coverage: Task 9
- Docker deployment and repo-backed guides: Tasks 0 and 10

## Self-Review

- Spec coverage: the current known gaps from the local audit and subagent audits are all mapped to Tasks 1 through 10.
- Placeholder scan: the plan avoids `TODO` and vague “handle later” language; each task names the concrete subsystem and verification target.
- Type consistency: the plan keeps the existing service boundaries and extends the current `shared-binance`, `shared-db`, `market-data-gateway`, `trading-engine`, `api-server`, and `web` paths instead of inventing new services.
