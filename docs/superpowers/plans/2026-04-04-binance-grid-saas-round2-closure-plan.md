# Binance Grid SaaS Round 2 Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the April 4 audit gaps so the repository matches the frozen March 31 Binance Grid SaaS design across user flows, runtime services, billing runtime, notifications, analytics, and observability.

**Architecture:** Keep the current Rust modular-monolith-plus-workers layout, but remove mock acceptance paths from the user app and wire the real backend and worker services together. Trading, billing, notifications, analytics, and scheduler flows must persist business truth in PostgreSQL and Redis and expose that truth to the web app.

**Tech Stack:** Rust, Axum, Tokio, SQLx/shared-db, PostgreSQL, Redis, Next.js App Router, TypeScript, Playwright, Node test runner, Docker Compose, Prometheus, Binance WebSocket/metadata adapters, EVM/Solana RPC polling.

---

## File Structure

### User flow closure
- Modify: `apps/web/src/app/(public)/login/page.tsx`
- Modify: `apps/web/src/app/(public)/register/page.tsx`
- Create: `apps/web/src/app/(public)/verify-email/page.tsx`
- Create: `apps/web/src/app/(public)/password-reset/page.tsx`
- Modify: `apps/web/src/app/api/auth/login/route.ts`
- Modify: `apps/web/src/app/api/auth/register/route.ts`
- Create: `apps/web/src/app/api/auth/verify-email/route.ts`
- Create: `apps/web/src/app/api/auth/password-reset/route.ts`
- Modify: `apps/web/src/app/app/security/page.tsx`
- Modify: `apps/web/src/app/api/user/security/route.ts`
- Modify: `tests/e2e/user_app.spec.ts`

### Backend truth in user app
- Modify: `apps/web/src/lib/api/user-product-state.ts`
- Modify: `apps/web/src/app/app/dashboard/page.tsx`
- Modify: `apps/web/src/app/app/orders/page.tsx`
- Modify: `apps/web/src/app/app/analytics/page.tsx`
- Modify: `apps/web/src/app/app/exchange/page.tsx`
- Modify: `apps/web/src/app/app/telegram/page.tsx`
- Modify: `apps/web/src/app/app/billing/page.tsx`
- Modify: `apps/web/src/app/app/strategies/page.tsx`
- Modify: `apps/web/src/app/app/strategies/new/page.tsx`
- Modify: `apps/web/src/app/app/strategies/[id]/page.tsx`
- Modify: `apps/web/src/app/api/user/exchange/route.ts`
- Modify: `apps/web/src/app/api/user/telegram/route.ts`
- Modify: `apps/web/src/app/api/user/strategies/create/route.ts`
- Modify: `apps/web/src/app/api/user/strategies/[id]/route.ts`
- Create: `apps/web/src/app/api/user/exports/[kind]/route.ts`
- Modify: `tests/e2e/user_commercial_flows.spec.ts`

### Strategy/runtime closure
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `apps/api-server/src/routes/strategies.rs`
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Modify: `crates/shared-domain/src/strategy.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/strategy_runtime.rs`
- Modify: `apps/trading-engine/src/runtime.rs`
- Modify: `apps/trading-engine/tests/grid_runtime.rs`
- Modify: `tests/simulation/trailing_tp.rs`
- Modify: `tests/simulation/strategy_rebuild.rs`
- Modify: `apps/market-data-gateway/src/main.rs`
- Modify: `apps/market-data-gateway/src/binance_ws.rs`
- Modify: `apps/market-data-gateway/tests/runtime_wiring.rs`

### Billing/runtime closure
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `apps/billing-chain-listener/src/processor.rs`
- Modify: `apps/scheduler/src/main.rs`
- Modify: `apps/scheduler/src/jobs/membership_grace.rs`
- Modify: `apps/scheduler/src/jobs/reminders.rs`
- Modify: `apps/api-server/src/services/membership_service.rs`
- Modify: `apps/api-server/src/routes/admin_sweeps.rs`
- Modify: `apps/api-server/tests/membership_flow.rs`
- Modify: `apps/api-server/tests/admin_deposit_flow.rs`
- Modify: `apps/api-server/tests/admin_address_pools_flow.rs`
- Modify: `tests/e2e/admin_commercial_flows.spec.ts`

### Notifications, analytics, exports
- Modify: `apps/api-server/src/services/telegram_service.rs`
- Modify: `apps/api-server/src/services/analytics_service.rs`
- Modify: `apps/api-server/src/routes/telegram.rs`
- Modify: `apps/api-server/src/routes/exports.rs`
- Modify: `apps/api-server/tests/notification_flow.rs`
- Modify: `apps/api-server/tests/analytics_flow.rs`

### Observability and acceptance
- Modify: `apps/api-server/src/main.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/market-data-gateway/src/main.rs`
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `apps/scheduler/src/main.rs`
- Modify: `deploy/monitoring/prometheus.yml`
- Modify: `deploy/monitoring/alert-rules.yml`
- Modify: `deploy/nginx/default.conf`
- Modify: `deploy/docker/docker-compose.yml`
- Modify: `tests/verification/compose.test.mjs`
- Modify: `docs/deployment/env-and-secrets.md`
- Modify: `docs/deployment/docker-compose.md`
- Modify: `docs/user-guide/*.md`
- Modify: `docs/admin-guide/*.md`

### Task 1: Public Auth And Security Closure

**Files:**
- Modify: `apps/web/src/app/(public)/login/page.tsx`
- Modify: `apps/web/src/app/(public)/register/page.tsx`
- Create: `apps/web/src/app/(public)/verify-email/page.tsx`
- Create: `apps/web/src/app/(public)/password-reset/page.tsx`
- Modify: `apps/web/src/app/api/auth/login/route.ts`
- Modify: `apps/web/src/app/api/auth/register/route.ts`
- Create: `apps/web/src/app/api/auth/verify-email/route.ts`
- Create: `apps/web/src/app/api/auth/password-reset/route.ts`
- Modify: `apps/web/src/app/app/security/page.tsx`
- Modify: `apps/web/src/app/api/user/security/route.ts`
- Test: `tests/e2e/user_app.spec.ts`

- [ ] Step 1: Add failing browser coverage for explicit email verification, password reset, and TOTP login.
- [ ] Step 2: Implement public pages and route handlers so registration no longer auto-verifies and login can submit `totp_code` when present.
- [ ] Step 3: Extend security center to display backend-issued TOTP bootstrap data and honest redirects after password/TOTP changes.
- [ ] Step 4: Run `pnpm test:e2e -- --grep "user"` and confirm the new auth/security browser flows pass.

### Task 2: User App Backend Truth Closure

**Files:**
- Modify: `apps/web/src/lib/api/user-product-state.ts`
- Modify: `apps/web/src/app/app/dashboard/page.tsx`
- Modify: `apps/web/src/app/app/orders/page.tsx`
- Modify: `apps/web/src/app/app/analytics/page.tsx`
- Modify: `apps/web/src/app/app/exchange/page.tsx`
- Modify: `apps/web/src/app/app/telegram/page.tsx`
- Modify: `apps/web/src/app/app/billing/page.tsx`
- Modify: `apps/web/src/app/app/strategies/page.tsx`
- Modify: `apps/web/src/app/app/strategies/new/page.tsx`
- Modify: `apps/web/src/app/app/strategies/[id]/page.tsx`
- Modify: `apps/web/src/app/api/user/exchange/route.ts`
- Modify: `apps/web/src/app/api/user/telegram/route.ts`
- Modify: `apps/web/src/app/api/user/strategies/create/route.ts`
- Modify: `apps/web/src/app/api/user/strategies/[id]/route.ts`
- Create: `apps/web/src/app/api/user/exports/[kind]/route.ts`
- Test: `tests/e2e/user_commercial_flows.spec.ts`

- [ ] Step 1: Add failing tests that require dashboard/order/strategy/telegram/billing surfaces to reflect backend truth instead of mock state.
- [ ] Step 2: Replace in-memory critical business state with server-side fetches to API endpoints and narrow local state to flash-only concerns.
- [ ] Step 3: Wire exchange, Telegram, strategy, and export actions to the real backend routes.
- [ ] Step 4: Run `pnpm test:e2e -- --grep "user commercial"` and confirm the user product flow passes with backend-backed data.

### Task 3: Strategy API And Runtime Closure

**Files:**
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `apps/api-server/src/routes/strategies.rs`
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Modify: `crates/shared-domain/src/strategy.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/strategy_runtime.rs`
- Modify: `apps/trading-engine/src/runtime.rs`
- Modify: `apps/trading-engine/tests/grid_runtime.rs`
- Modify: `tests/simulation/trailing_tp.rs`
- Modify: `tests/simulation/strategy_rebuild.rs`
- Modify: `apps/market-data-gateway/src/main.rs`
- Modify: `apps/market-data-gateway/src/binance_ws.rs`
- Modify: `apps/market-data-gateway/tests/runtime_wiring.rs`

- [ ] Step 1: Add failing backend and simulation tests for server-derived pre-flight, futures conflict rules, batch start, runtime error pause, and required spot/futures modes.
- [ ] Step 2: Remove client-authored readiness booleans from the accepted pre-flight path and derive checks from persisted membership, exchange, symbol, and balance data.
- [ ] Step 3: Turn `trading-engine` and `market-data-gateway` mains into long-running workers that load active strategies, manage subscriptions, process ticks, and persist runtime updates.
- [ ] Step 4: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow && cargo test -p trading-engine --test grid_runtime && cargo test --test trailing_tp && cargo test --test strategy_rebuild`.

### Task 4: Billing Runtime Closure

**Files:**
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `apps/billing-chain-listener/src/processor.rs`
- Modify: `apps/scheduler/src/main.rs`
- Modify: `apps/scheduler/src/jobs/membership_grace.rs`
- Modify: `apps/scheduler/src/jobs/reminders.rs`
- Modify: `apps/api-server/src/services/membership_service.rs`
- Modify: `apps/api-server/src/routes/admin_sweeps.rs`
- Modify: `apps/api-server/tests/membership_flow.rs`
- Modify: `apps/api-server/tests/admin_deposit_flow.rs`
- Modify: `apps/api-server/tests/admin_address_pools_flow.rs`
- Modify: `tests/e2e/admin_commercial_flows.spec.ts`

- [ ] Step 1: Add failing tests for chain confirmation enforcement, grace-end auto-pause, address-pool scoped sweep execution, and queue promotion after real state transitions.
- [ ] Step 2: Implement confirmation-aware transfer processing and consume per-chain confirmation policy from system config.
- [ ] Step 3: Add active RPC polling loops, grace-expiry scheduler execution, and executable sweep job state transitions with persisted tx hashes.
- [ ] Step 4: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test membership_flow --test admin_deposit_flow --test admin_address_pools_flow` and `pnpm test:e2e -- --grep "admin commercial"`.

### Task 5: Notifications, Analytics, Exports, And Observability Closure

**Files:**
- Modify: `apps/api-server/src/services/telegram_service.rs`
- Modify: `apps/api-server/src/services/analytics_service.rs`
- Modify: `apps/api-server/src/routes/telegram.rs`
- Modify: `apps/api-server/src/routes/exports.rs`
- Modify: `apps/api-server/tests/notification_flow.rs`
- Modify: `apps/api-server/tests/analytics_flow.rs`
- Modify: `apps/api-server/src/main.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/market-data-gateway/src/main.rs`
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `apps/scheduler/src/main.rs`
- Modify: `deploy/monitoring/prometheus.yml`
- Modify: `deploy/monitoring/alert-rules.yml`
- Modify: `deploy/nginx/default.conf`
- Modify: `deploy/docker/docker-compose.yml`
- Modify: `tests/verification/compose.test.mjs`
- Modify: `docs/deployment/env-and-secrets.md`
- Modify: `docs/deployment/docker-compose.md`
- Modify: `docs/user-guide/*.md`
- Modify: `docs/admin-guide/*.md`

- [ ] Step 1: Add failing tests for business-event notification production, analytics-backed user pages, export downloads, metrics exposure, and alert coverage.
- [ ] Step 2: Wire business services to emit notification records automatically and expose analytics/export actions in the web app.
- [ ] Step 3: Add structured logs, business metrics, and spec-aligned alert rules to the deployed stack.
- [ ] Step 4: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test notification_flow --test analytics_flow`, `node --test tests/verification/*.test.mjs`, `pnpm build`, and `./scripts/smoke.sh`.

## Spec Coverage Map

- 4.3 user auth, email verification, password reset, TOTP: Task 1
- 4.2 exchange connectivity and symbol support in user product flow: Tasks 2 and 3
- 4.4, 4.5, 7.* membership, chain payment, address pools, confirmations, grace, sweeps: Task 4
- 4.6, 4.7, 4.8, 4.9, 6.*, 8.* strategy lifecycle, runtime, market data, pre-flight: Task 3
- 4.10, 9.*, 10.*, 21 analytics, Telegram, notifications, exports: Tasks 2 and 5
- 17.* observability and release acceptance: Task 5
- 16.4 browser coverage: Tasks 1, 2, and 4
- 18.* docs and help alignment: Task 5

## Self-Review

- Spec coverage: all high-severity audit gaps are mapped into Tasks 1 through 5.
- Placeholder scan: no `TODO`/`TBD` markers are intentionally left in the plan.
- Type consistency: the plan keeps the existing service and route boundaries and focuses on replacing mock acceptance paths with backend truth.
