# Binance Grid SaaS Round 2 Closure Design

## 1. Purpose

This document closes the gaps found during the April 4 requirements audit against the frozen March 31 product design.

The March 31 design remains the source of truth. This document does not redefine product scope. It defines the second-round implementation closure required to make the repository align with the frozen requirements instead of shipping a mixed state of real backend services plus mock-driven user flows.

## 2. Closure Goal

Bring the project from "admin and backend heavy, user shell partially mocked, runtime services partially skeletal" to "spec-aligned end-to-end V1" across these missing areas:

- user-facing real backend flows
- trading runtime execution wiring
- billing runtime closure
- notification and analytics closure
- production-grade observability baseline

## 3. Frozen Closure Decisions

### 3.1 User Web Must Stop Using Mock Product State For Critical Flows

The following user flows must use backend truth and must no longer mutate local in-memory state as the source of record:

- exchange credential save and connection test
- Telegram bind code generation and binding status
- strategy create, edit, pre-flight, start, resume, pause, stop, delete
- dashboard, orders, analytics, and export surfaces

The remaining lightweight local flash state may be kept only for transient UI messaging, never for business truth.

### 3.2 Public Auth Must Become Honest

The public authentication surface must stop auto-verifying email inside the web proxy layer.

Required browser-visible flows:

- register -> verification code step -> verify email -> login
- login with optional TOTP input when challenged
- password reset request -> reset confirm

The web app may keep using the Rust API as the system of record, but the browser flow must reflect the real lifecycle required by the frozen spec.

### 3.3 Trading Runtime Must Use A Single Real Runtime Path

The repository currently contains domain/runtime helpers and tests, but the deployed `trading-engine` process is still a health-only shell.

Round 2 will make `apps/trading-engine` the canonical runtime executor for:

- loading active strategies from storage
- rebuilding runtime state from stored revisions and positions
- reacting to market ticks
- emitting order, fill, and runtime events into storage
- pushing auto-pause signals when runtime integrity fails

The simpler library-only runtime path that still rejects spec-required modes must be aligned with the strategy runtime engine or removed from the acceptance path.

### 3.4 Market Data Gateway Must Perform Real Subscription Work

The deployed `market-data-gateway` process must:

- maintain Binance stream subscriptions for active symbols only
- refresh subscriptions as active strategy symbols change
- fan out normalized market ticks to the trading engine
- expose health that reflects connection freshness, not only process liveness

This still follows the frozen decision that global symbol support comes from hourly metadata sync, not full-market WebSocket subscriptions.

### 3.5 Pre-Flight Must Compute Truth On The Server

Strategy pre-flight must stop trusting client-supplied readiness booleans.

The API service must derive readiness from persisted system state:

- membership entitlement and grace window
- exchange credential validation snapshot
- market-specific access and hedge mode
- symbol metadata and filters
- strategy conflict rules
- available balance and collateral snapshots
- trailing TP validity

The user UI may render these results, but must not author them.

### 3.6 Billing Runtime Must Enforce Confirmation Policy

The chain listener must no longer match and credit orders from a bare observed transfer without confirmation semantics.

Round 2 will add:

- confirmation-aware transfer observation records
- per-chain required confirmation checks sourced from system config
- pending -> confirmed -> matched classification flow
- listener-side consumption of ETH/BSC/SOL confirmation policy

### 3.7 Billing Runtime Must Become Active Instead Of Passive

The product requires automatic chain monitoring and operational sweeps.

Round 2 will implement:

- chain pollers for ETH, BSC, and SOL using configured RPC endpoints
- address-pool scoped transfer scanning
- abnormal transfer classification
- sweep job execution from pool addresses to treasury addresses
- persisted sweep transfer hashes and completion state

### 3.8 Grace Expiry Must Auto-Pause Strategies

The scheduler must execute the membership grace job on a recurring schedule.

When grace ends:

- all running strategies for the affected user are auto-paused
- runtime orders are canceled
- pause reason is persisted
- in-app and Telegram notifications are generated

### 3.9 Notifications Must Be Produced By Business Events

The notification service must stop acting only as a manual dispatch sink.

Business flows must emit notifications automatically for:

- strategy started
- strategy paused
- strategy auto-paused on runtime error
- grid fill executed
- fill profit reported
- overall take profit triggered
- overall stop loss triggered
- membership reminders and grace expiry
- deposit confirmed
- API credential issues

### 3.10 Analytics And Exports Must Reach The User UI

The backend analytics and CSV export APIs already exist. Round 2 will expose them in the user web app:

- dashboard metrics from `/analytics`
- strategy-level analytics in strategy detail
- account activity and fill history from backend reports
- CSV export actions for orders, fills, strategy stats, and payments

### 3.11 Observability Must Become Business-Aware

Process health alone is insufficient.

Round 2 will add:

- structured JSON logging across Rust services
- shared request or correlation identifiers where applicable
- Prometheus counters and gauges for runtime, billing, gateway, and scheduler jobs
- alert rules for market stream instability, order failures, chain listener failure, address pool exhaustion, membership expiry job failure, and database connectivity

## 4. Change Boundaries

### 4.1 Files And Subsystems To Keep

Keep and extend:

- PostgreSQL and Redis runtime architecture
- Rust workspace split
- current admin commercial surface
- current docs set and help center ingestion
- existing analytics/export service contracts where possible

### 4.2 Files And Patterns To Remove From Acceptance Path

Remove mock-driven acceptance behavior from:

- `apps/web/src/lib/api/user-product-state.ts`
- `apps/web/src/app/api/user/exchange/route.ts`
- `apps/web/src/app/api/user/telegram/route.ts`
- `apps/web/src/app/api/user/strategies/create/route.ts`
- `apps/web/src/app/api/user/strategies/[id]/route.ts`
- any page that uses fixed KPI text instead of backend results for required product truth

They may remain as transient compatibility helpers during migration, but the final accepted product path must not rely on them.

## 5. Delivery Decomposition

Round 2 is split into five closure tracks:

1. User flow closure
2. Strategy and runtime closure
3. Billing and membership runtime closure
4. Notification, analytics, and export closure
5. Observability and acceptance closure

Each track must end with tests that fail before implementation and pass after implementation.

## 6. Acceptance Standard

Round 2 is complete only when all of the following are true:

- the user browser flows use backend truth for auth, exchange, strategies, billing, Telegram, analytics, and exports
- the deployed runtime services do real work beyond health probes
- membership grace expiry auto-pauses running strategies
- chain confirmation policy is enforced before auto-credit
- sweep jobs produce executable transfer records and terminal states
- notifications are emitted from real business flows, not only manual test routes
- all required spot and futures modes in the frozen spec are supported by the accepted runtime path
- the verification suite proves the above with backend tests, simulation tests, E2E tests, and compose acceptance
