# Binance Grid SaaS Commercial Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the current `feature/full-v1` baseline into a commercially usable Binance grid SaaS release that matches the frozen March 31 design document, with PostgreSQL and Redis as the only runtime persistence path.

**Architecture:** Keep the current Rust service boundaries and Next.js route partitioning, but treat the current branch only as a reusable scaffold. Reuse valid domain logic and verified route behavior where possible; replace SQLite, in-memory services, sample analytics, placeholder pages, and weak admin surfaces with production-grade implementations driven by PostgreSQL, Redis, and real read models.

**Tech Stack:** Rust, Axum, Tokio, PostgreSQL, Redis, SQLx, Next.js App Router, TypeScript, Playwright, Docker Compose, Prometheus, Binance REST/WebSocket APIs, EVM/Solana chain listeners, Telegram Bot API.

---

## Source Of Truth

The sole product source of truth is:

- `docs/superpowers/specs/2026-03-31-binance-grid-saas-design.md`

This plan supersedes:

- `docs/superpowers/plans/2026-03-31-binance-grid-saas-implementation-plan.md`

If current code or older plans conflict with the March 31 design document, implementation must follow the March 31 design document.

## Baseline Reality

The current branch contains reusable work, but it is not release-ready.

### Reuse

- Rust workspace, service split, and build chain
- Auth/session/token foundations
- Part of membership, strategy, and route topology
- Compose, Nginx, Prometheus, and verification scaffolding
- Existing tests that encode valid business constraints

### Redo

- All runtime database work built around SQLite
- All in-memory runtime services
- All pages that are still headings plus descriptive text
- All analytics sourced from sample fills
- All admin surfaces that are navigational only

### Delete

- SQLite as a runtime or test database path
- SQLite-specific migration assumptions
- Any documentation that treats the current minimal UI as acceptable delivery

## Required Repository Layout Changes

### Backend

- `crates/shared-db/`
  Rebuilt around PostgreSQL repositories, migrations, transactions, and read models.
- `crates/shared-domain/`
  Expanded to match the frozen data model.
- `apps/api-server/src/routes/`
  Completed for all public/user/admin domains from the design doc.
- `apps/api-server/src/services/`
  Completed for auth, profile, exchange, strategies, orders, analytics, billing, telegram, and admin operations.

### Frontend

- `apps/web/src/components/`
  Add reusable application shells, cards, banners, tables, drawers, dialogs, forms, action bars, and status chips.
- `apps/web/src/app/(public)/`
  Public landing, login, register.
- `apps/web/src/app/app/`
  Full user app routes from the page map.
- `apps/web/src/app/admin/`
  Full admin routes from the page map.
- `apps/web/src/app/help/`
  Help center backed by repository docs.

### Deployment And Verification

- `db/migrations/`
  Fresh PostgreSQL migrations for all required logical tables.
- `deploy/docker/docker-compose.yml`
  Add PostgreSQL and Redis, remove SQLite runtime assumptions.
- `tests/verification/`
  Add runtime-shape, docs, and acceptance assertions.
- `tests/e2e/`
  Expand to commercial user and admin flows.

## Delivery Sequence

1. PostgreSQL and Redis foundation
2. Identity, security, and profile completion
3. Membership, billing, address pools, abnormal deposit handling, sweeps
4. Exchange account persistence and symbol sync
5. Strategy persistence, revisions, grids, preflight, and orders domain
6. Runtime events, notifications, and analytics persistence
7. Shared web UI system and application shells
8. User web application completion
9. Admin web application completion
10. Documentation, deployment, and full acceptance hardening

## Global Verification Gates

- `source "$HOME/.cargo/env" && cargo fmt --all --check`
- `source "$HOME/.cargo/env" && cargo test --workspace`
- `pnpm build`
- `pnpm test:e2e`
- `node --test tests/verification/*.test.mjs`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d --build`
- `./scripts/smoke.sh`

## Runtime Data Model Target

The PostgreSQL migration set must explicitly cover all logical tables from the design doc:

- `users`
- `admin_users`
- `user_sessions`
- `email_verification_tokens`
- `password_reset_tokens`
- `user_totp_factors`
- `user_exchange_accounts`
- `user_exchange_credentials`
- `telegram_bindings`
- `membership_plans`
- `membership_plan_prices`
- `membership_orders`
- `membership_entitlements`
- `deposit_address_pool`
- `deposit_address_allocations`
- `deposit_transactions`
- `deposit_order_queue`
- `fund_sweep_jobs`
- `fund_sweep_transfers`
- `strategies`
- `strategy_revisions`
- `strategy_grid_levels`
- `strategy_runtime_positions`
- `strategy_orders`
- `strategy_fills`
- `strategy_events`
- `strategy_profit_snapshots`
- `account_profit_snapshots`
- `exchange_wallet_snapshots`
- `exchange_account_trade_history`
- `strategy_templates`
- `system_configs`
- `audit_logs`
- `notification_logs`

## Task 1: Remove SQLite And Build PostgreSQL/Redis Foundation

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/shared-db/Cargo.toml`
- Replace: `crates/shared-db/src/lib.rs`
- Create: `crates/shared-db/src/postgres/*.rs`
- Create: `crates/shared-db/src/redis/*.rs`
- Replace: `db/migrations/0001_initial_core.sql`
- Create: `db/migrations/0002_identity_security.sql`
- Create: `db/migrations/0003_membership_billing.sql`
- Create: `db/migrations/0004_trading.sql`
- Create: `db/migrations/0005_admin_and_notifications.sql`
- Modify: `deploy/docker/docker-compose.yml`
- Modify: `docs/deployment/docker-compose.md`
- Create: `tests/verification/runtime_storage_shape.test.mjs`

- [ ] **Step 1: Write failing verification**

Add assertions that:

- compose includes `postgres` and `redis`
- compose no longer documents SQLite as runtime storage
- shared DB code no longer exposes SQLite runtime helpers

- [ ] **Step 2: Run failing verification**

Run: `node --test tests/verification/runtime_storage_shape.test.mjs`
Expected: FAIL because the current stack is still SQLite-based.

- [ ] **Step 3: Rebuild shared DB around PostgreSQL and Redis**

Implement:

- PostgreSQL connection pool bootstrap
- Redis client bootstrap
- migration runner
- transaction helpers
- repository modules for identity, billing, exchange, strategy, notification, admin

- [ ] **Step 4: Update application boot**

Modify runtime services to require PostgreSQL and Redis env vars at startup.

- [ ] **Step 5: Verify foundation**

Run:

- `source "$HOME/.cargo/env" && cargo test -p shared-db`
- `node --test tests/verification/runtime_storage_shape.test.mjs`

Expected: PASS.

## Task 2: Identity, Security, And Profile Completion

**Files:**
- Modify: `apps/api-server/src/services/auth_service.rs`
- Modify: `apps/api-server/src/routes/auth.rs`
- Modify: `apps/api-server/src/routes/security.rs`
- Create: `apps/api-server/src/routes/profile.rs`
- Modify: `apps/api-server/src/lib.rs`
- Modify: `apps/api-server/tests/auth_flow.rs`
- Create: `apps/api-server/tests/profile_flow.rs`

- [ ] **Step 1: Extend failing tests**

Require:

- durable email verification tokens
- durable password reset tokens
- durable TOTP factors
- profile read endpoint
- profile mutation endpoints required by the design and user docs

- [ ] **Step 2: Run focused tests**

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test auth_flow --test profile_flow`
Expected: FAIL because profile routes and durable factor/token storage are incomplete.

- [ ] **Step 3: Implement auth/profile completion**

Add:

- profile domain endpoints
- password change flow
- email status read model
- admin TOTP enforcement checks
- audit emission for security-sensitive actions

- [ ] **Step 4: Re-run focused tests**

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test auth_flow --test profile_flow`
Expected: PASS.

## Task 3: Membership, Billing, Address Pools, Abnormal Deposits, And Sweeps

**Files:**
- Modify: `apps/api-server/src/services/membership_service.rs`
- Modify: `apps/api-server/src/routes/billing.rs`
- Modify: `apps/api-server/src/routes/membership.rs`
- Create: `apps/api-server/src/routes/admin_memberships.rs`
- Create: `apps/api-server/src/routes/admin_deposits.rs`
- Create: `apps/api-server/src/routes/admin_sweeps.rs`
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `apps/scheduler/src/jobs/*.rs`
- Modify: `apps/api-server/tests/membership_flow.rs`
- Create: `apps/api-server/tests/admin_deposit_flow.rs`

- [ ] **Step 1: Extend failing tests**

Cover:

- all three chains plus stablecoin pricing rules
- pool exhaustion and queue behavior
- exact-amount enforcement
- abnormal transfer manual handling
- grace window behavior
- sweep job creation and audit logging

- [ ] **Step 2: Run focused billing tests**

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test membership_flow --test admin_deposit_flow`
Expected: FAIL because current implementation only partially covers these flows.

- [ ] **Step 3: Implement billing and entitlement completion**

Add:

- plan and price config
- order queue logic
- abnormal deposit state machine
- admin manual processing routes
- sweep job persistence
- audit logs for membership and treasury actions

- [ ] **Step 4: Re-run focused tests**

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test membership_flow --test admin_deposit_flow`
Expected: PASS.

## Task 4: Exchange Accounts, Credentials, And Symbol Sync

**Files:**
- Modify: `apps/api-server/src/services/exchange_service.rs`
- Modify: `apps/api-server/src/routes/exchange.rs`
- Modify: `apps/market-data-gateway/src/main.rs`
- Modify: `apps/scheduler/src/main.rs`
- Modify: `crates/shared-binance/src/*.rs`
- Modify: `apps/api-server/tests/exchange_flow.rs`

- [ ] **Step 1: Extend failing tests**

Require:

- one user one Binance account
- masked credential read model
- durable connection health status
- spot plus USDⓈ-M plus COIN-M symbol metadata sync
- fuzzy symbol search backed by persisted metadata

- [ ] **Step 2: Run focused exchange tests**

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test exchange_flow`
Expected: FAIL because current symbol and credential state are not durably modeled.

- [ ] **Step 3: Implement exchange persistence and sync**

Add:

- user exchange account tables
- encrypted credential storage
- periodic symbol metadata refresh
- sync status and last-sync metadata
- hedge-mode and permissions validation snapshots

- [ ] **Step 4: Re-run focused tests**

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test exchange_flow`
Expected: PASS.

## Task 5: Strategy Domain Rebuild To Match The Frozen Model

**Files:**
- Modify: `crates/shared-domain/src/strategy.rs`
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `apps/api-server/src/routes/strategies.rs`
- Create: `apps/api-server/src/routes/orders.rs`
- Modify: `apps/trading-engine/src/*.rs`
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Create: `tests/simulation/trailing_tp.rs`
- Create: `tests/simulation/strategy_rebuild.rs`

- [ ] **Step 1: Extend failing strategy tests**

Require:

- strategy revisions
- grid levels
- arithmetic, geometric, and custom grids
- per-grid TP settings
- trailing TP restrictions
- optional overall TP/SL
- start/pause/resume/stop/delete rules from the design doc
- soft archive delete semantics

- [ ] **Step 2: Run focused strategy and simulation tests**

Run:

- `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow`
- `source "$HOME/.cargo/env" && cargo test --test trailing_tp --test strategy_rebuild`

Expected: FAIL because the current strategy model is under-specified.

- [ ] **Step 3: Rebuild strategy persistence and runtime contracts**

Add:

- strategy revisions
- grid level persistence
- runtime positions/orders/fills/events
- explicit preflight step reporting
- stop/rebuild post-trigger behavior

- [ ] **Step 4: Re-run focused tests**

Run the same commands as Step 2.
Expected: PASS.

## Task 6: Durable Notifications And Analytics

**Files:**
- Modify: `apps/api-server/src/services/telegram_service.rs`
- Modify: `apps/api-server/src/services/analytics_service.rs`
- Modify: `apps/api-server/src/routes/telegram.rs`
- Modify: `apps/api-server/src/routes/analytics.rs`
- Modify: `apps/api-server/src/routes/exports.rs`
- Modify: `apps/api-server/tests/notification_flow.rs`
- Modify: `apps/api-server/tests/analytics_flow.rs`

- [ ] **Step 1: Extend failing tests**

Require:

- durable Telegram binding records
- durable in-app notification log
- fill-based analytics from persisted strategy/exchange data
- account-level and strategy-level snapshots
- CSV export for orders, fills, strategy stats, payment records

- [ ] **Step 2: Run focused tests**

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test notification_flow --test analytics_flow`
Expected: FAIL because current notification and analytics services are still demo-grade.

- [ ] **Step 3: Implement durable notifications and analytics**

Replace:

- in-memory Telegram state
- sample analytics data

With:

- persisted binding and notification logs
- analytics projection readers built from trading and exchange tables

- [ ] **Step 4: Re-run focused tests**

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test notification_flow --test analytics_flow`
Expected: PASS.

## Task 7: Admin API Completion

**Files:**
- Create: `apps/api-server/src/routes/admin_users.rs`
- Create: `apps/api-server/src/routes/admin_memberships.rs`
- Create: `apps/api-server/src/routes/admin_deposits.rs`
- Create: `apps/api-server/src/routes/admin_strategies.rs`
- Create: `apps/api-server/src/routes/admin_sweeps.rs`
- Create: `apps/api-server/src/routes/admin_system.rs`
- Modify: `apps/api-server/src/routes/admin_templates.rs`
- Modify: `apps/api-server/src/lib.rs`
- Create: `apps/api-server/tests/admin_api_flow.rs`

- [ ] **Step 1: Write failing admin API tests**

Require all admin domains from the design doc.

- [ ] **Step 2: Run focused admin API tests**

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test admin_api_flow`
Expected: FAIL because several admin domains do not exist yet.

- [ ] **Step 3: Implement admin APIs**

Add:

- user overview
- membership overrides and timeline
- deposit exception queues
- strategy runtime overview
- sweep operations
- system config
- audit list/read models

- [ ] **Step 4: Re-run focused admin API tests**

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test admin_api_flow`
Expected: PASS.

## Task 8: Shared Web UI System And Application Shells

**Files:**
- Create: `apps/web/src/components/layout/*.tsx`
- Create: `apps/web/src/components/ui/*.tsx`
- Create: `apps/web/src/components/forms/*.tsx`
- Create: `apps/web/src/components/tables/*.tsx`
- Create: `apps/web/src/lib/api/*.ts`
- Modify: `apps/web/src/styles/globals.css`
- Modify: `apps/web/src/app/layout.tsx`
- Create: `tests/verification/web_app_shell.test.mjs`

- [ ] **Step 1: Write failing shell verification**

Assert the app uses reusable user/admin shells, not route-local bare markup.

- [ ] **Step 2: Run verification**

Run: `node --test tests/verification/web_app_shell.test.mjs`
Expected: FAIL because the current web layer lacks a shared app shell system.

- [ ] **Step 3: Build shared UI system**

Add:

- public shell
- user shell
- admin shell
- status banners
- cards, tables, forms, tabs, chips, dialogs
- server-side API client helpers

- [ ] **Step 4: Verify shell**

Run: `node --test tests/verification/web_app_shell.test.mjs && pnpm build`
Expected: PASS.

## Task 9: Public And User App Completion

**Files:**
- Modify: `apps/web/src/app/page.tsx`
- Modify: `apps/web/src/app/(public)/login/page.tsx`
- Modify: `apps/web/src/app/(public)/register/page.tsx`
- Modify: `apps/web/src/app/app/dashboard/page.tsx`
- Modify: `apps/web/src/app/app/exchange/page.tsx`
- Modify: `apps/web/src/app/app/strategies/page.tsx`
- Create: `apps/web/src/app/app/strategies/new/page.tsx`
- Modify: `apps/web/src/app/app/strategies/[id]/page.tsx`
- Create: `apps/web/src/app/app/orders/page.tsx`
- Modify: `apps/web/src/app/app/billing/page.tsx`
- Create: `apps/web/src/app/app/telegram/page.tsx`
- Modify: `apps/web/src/app/app/security/page.tsx`
- Create: `apps/web/src/app/app/help/page.tsx`
- Modify: `apps/web/src/app/help/[slug]/page.tsx`
- Modify: `tests/e2e/user_app.spec.ts`
- Create: `tests/e2e/user_commercial_flows.spec.ts`

- [ ] **Step 1: Write failing user E2E**

Require:

- landing page with pricing and risk copy
- exchange credential save/test
- billing order creation with exact-amount warnings
- strategy create/edit/preflight/start flows
- orders/history visibility
- Telegram bind flow
- security center operations
- actionable dashboard states

- [ ] **Step 2: Run user E2E**

Run: `pnpm test:e2e --grep "user commercial"`
Expected: FAIL because current pages are still mostly placeholders.

- [ ] **Step 3: Implement user pages**

Build real forms, tables, summaries, warnings, and task-oriented flows for every user route in the page map.

- [ ] **Step 4: Re-run user E2E**

Run: `pnpm test:e2e --grep "user commercial"`
Expected: PASS.

## Task 10: Admin App Completion

**Files:**
- Modify: `apps/web/src/app/admin/dashboard/page.tsx`
- Modify: `apps/web/src/app/admin/users/page.tsx`
- Create: `apps/web/src/app/admin/memberships/page.tsx`
- Create: `apps/web/src/app/admin/deposits/page.tsx`
- Modify: `apps/web/src/app/admin/address-pools/page.tsx`
- Modify: `apps/web/src/app/admin/templates/page.tsx`
- Create: `apps/web/src/app/admin/strategies/page.tsx`
- Create: `apps/web/src/app/admin/sweeps/page.tsx`
- Modify: `apps/web/src/app/admin/audit/page.tsx`
- Create: `apps/web/src/app/admin/system/page.tsx`
- Modify: `tests/e2e/admin_app.spec.ts`
- Create: `tests/e2e/admin_commercial_flows.spec.ts`

- [ ] **Step 1: Write failing admin E2E**

Require:

- membership management
- abnormal deposit handling
- address pool expansion
- template management
- strategy oversight
- sweep job visibility
- audit log review
- system configuration

- [ ] **Step 2: Run admin E2E**

Run: `pnpm test:e2e --grep "admin commercial"`
Expected: FAIL because these operator flows do not exist yet.

- [ ] **Step 3: Implement admin pages**

Build real admin tables, forms, filters, action panels, and audit-driven operator workflows.

- [ ] **Step 4: Re-run admin E2E**

Run: `pnpm test:e2e --grep "admin commercial"`
Expected: PASS.

## Task 11: Documentation, Help Center, And Deployment Hardening

**Files:**
- Modify: `docs/user-guide/getting-started.md`
- Create: `docs/user-guide/binance-api-setup.md`
- Create: `docs/user-guide/membership-and-payment.md`
- Create: `docs/user-guide/create-grid-strategy.md`
- Create: `docs/user-guide/manage-strategy.md`
- Modify: `docs/user-guide/security-center.md`
- Create: `docs/user-guide/telegram-notifications.md`
- Create: `docs/user-guide/troubleshooting.md`
- Create: `docs/admin-guide/address-pool-management.md`
- Create: `docs/admin-guide/membership-operations.md`
- Create: `docs/admin-guide/template-management.md`
- Create: `docs/admin-guide/abnormal-order-handling.md`
- Create: `docs/admin-guide/system-config-and-audit.md`
- Modify: `docs/deployment/docker-compose.md`
- Create: `docs/deployment/env-and-secrets.md`
- Create: `docs/deployment/backup-and-restore.md`
- Modify: `scripts/smoke.sh`
- Create: `tests/verification/commercial_docs_and_acceptance.test.mjs`

- [ ] **Step 1: Write failing docs verification**

Require all guide files from the design doc and assert smoke checks mention the commercial runtime path.

- [ ] **Step 2: Run docs verification**

Run: `node --test tests/verification/commercial_docs_and_acceptance.test.mjs`
Expected: FAIL because the required guide set is incomplete.

- [ ] **Step 3: Complete docs and help content**

Make repository docs and in-app help center reflect the actual product and deployment path.

- [ ] **Step 4: Re-run docs verification**

Run: `node --test tests/verification/commercial_docs_and_acceptance.test.mjs`
Expected: PASS.

## Task 12: Full Acceptance Gate

**Files:**
- Modify: `tests/verification/*.test.mjs`
- Modify: `tests/e2e/*.spec.ts`
- Modify: `deploy/docker/docker-compose.yml`

- [ ] **Step 1: Run the full non-compose gate**

Run:

- `source "$HOME/.cargo/env" && cargo fmt --all --check`
- `source "$HOME/.cargo/env" && cargo test --workspace`
- `pnpm build`
- `pnpm test:e2e`
- `node --test tests/verification/*.test.mjs`

Expected: PASS.

- [ ] **Step 2: Run full compose acceptance**

Run:

- `docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d --build`
- `./scripts/smoke.sh`

Expected: PASS with PostgreSQL and Redis in the stack and real web flows reachable.

- [ ] **Step 3: Review against frozen acceptance baseline**

Re-check implementation against Section 21 of the March 31 design document and confirm:

- complete auth and security flow
- complete membership and billing flow
- complete Binance credential flow
- complete strategy lifecycle flow
- Telegram binding and notifications
- strategy/account statistics
- exports
- admin management and audit logging
- docs
- deployment

## Reuse / Redo Matrix

### Reuse first

- session token model
- route partitioning
- compose/nginx/prometheus baseline
- strategy and membership test assets where rules are still correct

### Redo first

- shared DB internals
- exchange persistence
- telegram persistence
- analytics persistence
- all placeholder pages
- all missing admin domains

## Self-Review

Spec coverage:

- page map: Tasks 8, 9, 10
- domain boundaries: Tasks 2, 3, 4, 5, 6, 7
- data model: Tasks 1, 3, 4, 5, 6, 7
- testing strategy: Tasks 2 through 12
- docs and operations: Task 11 and Task 12

Placeholder scan:

- No task accepts placeholder pages as delivered UI
- No task accepts sample analytics or in-memory runtime state
- No task preserves SQLite

Type consistency:

- PostgreSQL/Redis foundation precedes all runtime-domain rewrites
- admin/user APIs precede final page delivery
- shared UI system precedes user/admin route completion

## Execution Handoff

This plan is now the active implementation plan for commercial recovery.

Execution rule:

- Follow this file instead of the March 31 implementation plan wherever they conflict.
- Follow the March 31 design spec instead of this plan if a requirement gap appears.
