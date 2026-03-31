# Binance Grid SaaS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the approved Binance grid SaaS V1 from the frozen design spec in a clean, reviewable sequence, without skipping planning, verification, or delivery documentation.

**Architecture:** The implementation will follow the approved Rust-first modular monolith plus specialized workers model: one Next.js frontend, one Rust API service, and separate Rust services for trading, market data, chain listening, and scheduling. Shared crates will own configuration, auth, persistence, exchange integration, chain integration, domain models, events, and telemetry so that later batches extend stable boundaries instead of reworking foundations.

**Tech Stack:** Rust workspace, Axum, Tokio, SQLx, PostgreSQL, Redis, Next.js App Router, TypeScript, Tailwind CSS, Binance REST/WebSocket APIs, EVM RPC, Solana RPC, Telegram Bot API, Docker Compose.

---

## Planning Scope Note

The approved spec covers multiple independent subsystems, so implementation is intentionally decomposed into delivery batches. This document is the full master plan for review. After this master plan is approved, each batch will be executed in order with TDD and verification gates.

## Target Repository Layout

### Root

- `Cargo.toml`
  Workspace members and shared dependency versions.
- `package.json`
  Root frontend scripts and verification entrypoints.
- `pnpm-workspace.yaml`
  Web workspace package discovery.
- `turbo.json`
  Cross-project task runner.
- `.env.example`
  Full environment variable template.
- `Makefile`
  Operator shortcuts for setup, verify, seed, smoke, and compose.

### Apps

- `apps/api-server/`
  Public, user, and admin HTTP APIs.
- `apps/trading-engine/`
  Strategy pre-flight and runtime execution.
- `apps/market-data-gateway/`
  Active market data WebSocket multiplexing.
- `apps/billing-chain-listener/`
  Address pool rotation, chain monitoring, abnormal deposit handling.
- `apps/scheduler/`
  Hourly symbol sync, grace checks, reminder jobs, reconciliation jobs.
- `apps/web/`
  Marketing site, `/app/*`, `/admin/*`, help center.

### Shared crates

- `crates/shared-config/`
- `crates/shared-db/`
- `crates/shared-auth/`
- `crates/shared-domain/`
- `crates/shared-binance/`
- `crates/shared-chain/`
- `crates/shared-events/`
- `crates/shared-telemetry/`

### Data, docs, deployment, tests

- `db/migrations/`
- `db/seeds/`
- `deploy/docker/`
- `deploy/nginx/`
- `deploy/monitoring/`
- `docs/user-guide/`
- `docs/admin-guide/`
- `docs/deployment/`
- `tests/integration/`
- `tests/simulation/`
- `tests/e2e/`
- `tests/verification/`

## Batch Order

1. Foundation workspace and tooling
2. Web shell and workspace hygiene
3. Auth, email verification, password reset, TOTP, security center
4. Membership catalog, billing orders, address pools, grace lifecycle
5. Binance credentials, connection testing, symbol metadata sync, fuzzy search
6. Strategy draft/save/edit/template/pre-flight APIs
7. Market data gateway, scheduler, event wiring
8. Grid engine core, TP/SL, trailing TP, stop/rebuild behavior
9. Wallet/trade/funding/fee analytics, per-strategy statistics, exports
10. Telegram binding and notification flows
11. User web app delivery
12. Admin web app delivery
13. Deployment, observability, docs, release hardening

## Global Verification Gates

- Backend format: `source "$HOME/.cargo/env" && cargo fmt --all --check`
- Backend tests: `source "$HOME/.cargo/env" && cargo test --workspace`
- Frontend install: `corepack enable && pnpm install`
- Frontend lint/build: `pnpm lint && pnpm build`
- Verification scripts: `node --test tests/verification/*.test.mjs`
- E2E: `pnpm test:e2e`
- Compose smoke: `docker compose up -d --build && ./scripts/smoke.sh`

### Task 1: Foundation Workspace

**Files:**
- Create: `Cargo.toml`
- Create: `package.json`
- Create: `pnpm-workspace.yaml`
- Create: `turbo.json`
- Create: `.env.example`
- Create: `Makefile`
- Create: `apps/*/Cargo.toml`
- Create: `apps/*/src/main.rs`
- Create: `crates/*/Cargo.toml`
- Create: `crates/*/src/lib.rs`
- Create: `db/migrations/0001_initial_core.sql`
- Create: `tests/verification/workspace_foundation.test.mjs`

- [ ] **Step 1: Write the failing root verification test**

```js
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("foundation workspace files exist", () => {
  for (const path of ["Cargo.toml", "package.json", "apps/api-server/src/main.rs"]) {
    assert.ok(fs.existsSync(path), `${path} should exist`);
  }
});
```

- [ ] **Step 2: Run the failing verification**

Run: `node --test tests/verification/workspace_foundation.test.mjs`
Expected: FAIL because the workspace files are missing.

- [ ] **Step 3: Add the minimal workspace scaffold**

```toml
[workspace]
members = ["apps/api-server", "apps/trading-engine", "apps/market-data-gateway", "apps/billing-chain-listener", "apps/scheduler", "crates/shared-config", "crates/shared-db", "crates/shared-auth", "crates/shared-domain", "crates/shared-binance", "crates/shared-chain", "crates/shared-events", "crates/shared-telemetry"]
resolver = "2"
```

```rust
fn main() {
    println!("bootstrap");
}
```

- [ ] **Step 4: Run foundation verification**

Run: `source "$HOME/.cargo/env" && cargo test --workspace && node --test tests/verification/*.test.mjs`
Expected: PASS for the empty foundation scaffold.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml package.json pnpm-workspace.yaml turbo.json .env.example Makefile apps crates db tests/verification
git commit -m "chore: 问题描述 bootstrap repository foundation"
```

### Task 2: Web Shell and Workspace Hygiene

**Files:**
- Modify: `.gitignore`
- Modify: `package.json`
- Modify: `turbo.json`
- Create: `Cargo.lock`
- Create: `apps/web/package.json`
- Create: `apps/web/next.config.ts`
- Create: `apps/web/tsconfig.json`
- Create: `apps/web/next-env.d.ts`
- Create: `apps/web/postcss.config.js`
- Create: `apps/web/tailwind.config.ts`
- Create: `apps/web/src/app/layout.tsx`
- Create: `apps/web/src/app/page.tsx`
- Create: `apps/web/src/styles/globals.css`
- Create: `tests/verification/web_shell.test.mjs`

- [ ] **Step 1: Write the failing web shell verification**

```js
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("web shell and workspace hygiene are present", () => {
  for (const path of [
    "apps/web/package.json",
    "apps/web/src/app/layout.tsx",
    "apps/web/src/app/page.tsx",
    "Cargo.lock",
  ]) {
    assert.ok(fs.existsSync(path), `${path} should exist`);
  }

  const gitignore = fs.readFileSync(".gitignore", "utf8");
  assert.match(gitignore, /^target\/$/m);

  const rootPackage = JSON.parse(fs.readFileSync("package.json", "utf8"));
  assert.ok(rootPackage.scripts["build:web"]);
  assert.ok(rootPackage.scripts.build);
});
```

- [ ] **Step 2: Run the failing verification**

Run: `node --test tests/verification/web_shell.test.mjs`
Expected: FAIL because the web shell files, lockfile, and build scripts do not exist yet.

- [ ] **Step 3: Add the minimal web shell and hygiene fixes**

```json
{
  "name": "grid-binance",
  "private": true,
  "packageManager": "pnpm@10.17.1",
  "scripts": {
    "build:web": "pnpm --filter web build",
    "build": "pnpm build:web",
    "lint": "pnpm --filter web lint",
    "test": ". \"$HOME/.cargo/env\" && cargo test --workspace && node --test tests/verification/*.test.mjs"
  }
}
```

```tsx
import "../styles/globals.css";
import type { ReactNode } from "react";

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
```

```tsx
export default function HomePage() {
  return <main>Grid Binance</main>;
}
```

- [ ] **Step 4: Run web shell verification**

Run: `corepack enable && pnpm install && pnpm build:web && source "$HOME/.cargo/env" && cargo test --workspace && node --test tests/verification/*.test.mjs`
Expected: PASS for the minimal `apps/web` shell, committed lockfile, and clean workspace hygiene.

- [ ] **Step 5: Commit**

```bash
git add .gitignore Cargo.lock package.json turbo.json apps/web tests/verification
git commit -m "chore: 修复思路 add web shell and workspace hygiene baseline"
```

### Task 3: Auth and Security Center

**Files:**
- Create: `apps/api-server/src/routes/auth.rs`
- Create: `apps/api-server/src/routes/security.rs`
- Create: `apps/api-server/src/services/auth_service.rs`
- Create: `crates/shared-auth/src/password.rs`
- Create: `crates/shared-auth/src/totp.rs`
- Create: `crates/shared-auth/src/email_code.rs`
- Create: `tests/integration/auth_flow.rs`
- Create: `apps/web/src/app/(public)/login/page.tsx`
- Create: `apps/web/src/app/(public)/register/page.tsx`
- Create: `apps/web/src/app/app/security/page.tsx`

- [ ] **Step 1: Write failing auth flow coverage**

```rust
#[tokio::test]
async fn register_verify_login_and_enable_totp() {
    assert!(false, "implement register -> verify -> login -> totp");
}
```

- [ ] **Step 2: Run the auth integration test**

Run: `source "$HOME/.cargo/env" && cargo test --test auth_flow`
Expected: FAIL because auth routes and services do not exist.

- [ ] **Step 3: Implement the minimal auth surface**

```rust
pub struct RegisterUserRequest {
    pub email: String,
    pub password: String,
}
```

```tsx
export default function SecurityPage() {
  return <main>Security Center</main>;
}
```

- [ ] **Step 4: Run auth verification**

Run: `source "$HOME/.cargo/env" && cargo test --test auth_flow && pnpm build`
Expected: PASS for register, verify, login, password reset, and TOTP happy paths.

- [ ] **Step 5: Commit**

```bash
git add apps/api-server crates/shared-auth tests/integration apps/web
git commit -m "feat: 修复思路 add auth and security center baseline"
```

### Task 4: Membership, Billing Orders, and Address Pools

**Files:**
- Create: `apps/api-server/src/routes/billing.rs`
- Create: `apps/api-server/src/routes/membership.rs`
- Create: `apps/billing-chain-listener/src/address_pool.rs`
- Create: `apps/billing-chain-listener/src/order_matcher.rs`
- Create: `crates/shared-chain/src/assignment.rs`
- Create: `crates/shared-domain/src/membership.rs`
- Create: `tests/integration/membership_flow.rs`
- Create: `apps/web/src/app/app/membership/page.tsx`
- Create: `apps/web/src/app/admin/billing/page.tsx`

- [ ] **Step 1: Write failing membership flow coverage**

```rust
#[tokio::test]
async fn assign_exact_amount_order_and_activate_membership() {
    assert!(false, "implement chain order assignment and membership activation");
}
```

- [ ] **Step 2: Run the membership integration test**

Run: `source "$HOME/.cargo/env" && cargo test --test membership_flow`
Expected: FAIL because billing and address-pool flows do not exist.

- [ ] **Step 3: Implement the minimal billing domain**

```rust
pub enum MembershipStatus {
    Pending,
    Active,
    Grace,
    Expired,
    Frozen,
    Revoked,
}
```

```rust
pub struct AddressAssignment {
    pub chain: String,
    pub address: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}
```

- [ ] **Step 4: Run billing verification**

Run: `source "$HOME/.cargo/env" && cargo test --test membership_flow`
Expected: PASS for address rotation, exact-amount match, grace window, and admin override behavior.

- [ ] **Step 5: Commit**

```bash
git add apps/api-server apps/billing-chain-listener crates/shared-chain crates/shared-domain tests/integration apps/web
git commit -m "feat: 问题描述 add membership billing address pools"
```

### Task 5: Binance Credentials and Symbol Metadata

**Files:**
- Create: `apps/api-server/src/routes/exchange.rs`
- Create: `apps/scheduler/src/jobs/symbol_sync.rs`
- Create: `crates/shared-binance/src/client.rs`
- Create: `crates/shared-binance/src/metadata.rs`
- Create: `tests/integration/exchange_flow.rs`
- Create: `apps/web/src/app/app/exchange/page.tsx`

- [ ] **Step 1: Write failing exchange coverage**

```rust
#[tokio::test]
async fn save_credentials_test_connection_and_sync_symbols() {
    assert!(false, "implement credential validation and symbol sync");
}
```

- [ ] **Step 2: Run the exchange integration test**

Run: `source "$HOME/.cargo/env" && cargo test --test exchange_flow`
Expected: FAIL because exchange clients and sync job do not exist.

- [ ] **Step 3: Implement the minimal exchange layer**

```rust
pub struct ExchangeCredentialCheck {
    pub can_read_spot: bool,
    pub can_read_futures: bool,
    pub hedge_mode_ok: bool,
}
```

```rust
pub struct SymbolMetadata {
    pub symbol: String,
    pub market: String,
    pub status: String,
}
```

- [ ] **Step 4: Run exchange verification**

Run: `source "$HOME/.cargo/env" && cargo test --test exchange_flow`
Expected: PASS for credential test, symbol sync, fuzzy search, and hedge-mode validation.

- [ ] **Step 5: Commit**

```bash
git add apps/api-server apps/scheduler crates/shared-binance tests/integration apps/web
git commit -m "feat: 修复思路 add binance credential and metadata sync"
```

### Task 6: Strategy Drafting, Templates, and Pre-Flight

**Files:**
- Create: `apps/api-server/src/routes/strategies.rs`
- Create: `apps/api-server/src/services/strategy_service.rs`
- Create: `apps/api-server/src/routes/admin_templates.rs`
- Create: `crates/shared-domain/src/strategy.rs`
- Create: `tests/integration/strategy_flow.rs`
- Create: `apps/web/src/app/app/strategies/page.tsx`
- Create: `apps/web/src/app/admin/templates/page.tsx`

- [ ] **Step 1: Write failing strategy coverage**

```rust
#[tokio::test]
async fn create_save_pause_edit_and_start_strategy() {
    assert!(false, "implement strategy draft lifecycle and preflight");
}
```

- [ ] **Step 2: Run the strategy integration test**

Run: `source "$HOME/.cargo/env" && cargo test --test strategy_flow`
Expected: FAIL because strategy CRUD and template flows do not exist.

- [ ] **Step 3: Implement the minimal strategy domain**

```rust
pub enum StrategyStatus {
    Draft,
    Running,
    Paused,
    Stopped,
    Error,
}
```

```rust
pub struct PreflightFailure {
    pub step: String,
    pub reason: String,
}
```

- [ ] **Step 4: Run strategy verification**

Run: `source "$HOME/.cargo/env" && cargo test --test strategy_flow`
Expected: PASS for draft/save/edit restrictions, template copy, pre-flight failure reporting, batch pause/delete, and stop-all.

- [ ] **Step 5: Commit**

```bash
git add apps/api-server crates/shared-domain tests/integration apps/web
git commit -m "feat: 问题描述 add strategy drafting templates and preflight"
```

### Task 7: Market Data Gateway and Scheduler Wiring

**Files:**
- Create: `apps/market-data-gateway/src/subscriptions.rs`
- Create: `apps/market-data-gateway/src/binance_ws.rs`
- Create: `apps/scheduler/src/jobs/membership_grace.rs`
- Create: `apps/scheduler/src/jobs/reminders.rs`
- Create: `crates/shared-events/src/market.rs`
- Create: `tests/integration/runtime_wiring.rs`

- [ ] **Step 1: Write failing runtime wiring coverage**

```rust
#[tokio::test]
async fn subscribe_only_active_symbols_and_emit_ticks() {
    assert!(false, "implement gateway subscription lifecycle");
}
```

- [ ] **Step 2: Run the runtime wiring test**

Run: `source "$HOME/.cargo/env" && cargo test --test runtime_wiring`
Expected: FAIL because gateway and scheduler jobs do not exist.

- [ ] **Step 3: Implement the minimal event wiring**

```rust
pub struct MarketTick {
    pub symbol: String,
    pub price: rust_decimal::Decimal,
    pub event_time_ms: i64,
}
```

- [ ] **Step 4: Run runtime verification**

Run: `source "$HOME/.cargo/env" && cargo test --test runtime_wiring`
Expected: PASS for active subscription, reconnect, health, grace pause job, and reminder job behavior.

- [ ] **Step 5: Commit**

```bash
git add apps/market-data-gateway apps/scheduler crates/shared-events tests/integration
git commit -m "feat: 修复思路 add market data gateway and scheduler wiring"
```

### Task 8: Grid Engine Core and Trade Lifecycle

**Files:**
- Create: `apps/trading-engine/src/grid_builder.rs`
- Create: `apps/trading-engine/src/runtime.rs`
- Create: `apps/trading-engine/src/take_profit.rs`
- Create: `apps/trading-engine/src/stop_loss.rs`
- Create: `tests/simulation/grid_runtime.rs`

- [ ] **Step 1: Write failing grid simulation coverage**

```rust
#[test]
fn trailing_take_profit_uses_post_activation_high() {
    assert!(false, "implement trailing take profit simulation");
}
```

- [ ] **Step 2: Run the grid simulation suite**

Run: `source "$HOME/.cargo/env" && cargo test --test grid_runtime`
Expected: FAIL because the engine core does not exist.

- [ ] **Step 3: Implement the minimal grid runtime contracts**

```rust
pub enum GridMode {
    SpotClassic,
    SpotBuyOnly,
    SpotSellOnly,
    FuturesLong,
    FuturesShort,
    FuturesNeutral,
}
```

```rust
pub struct TrailingTakeProfit {
    pub trigger_price: rust_decimal::Decimal,
    pub trailing_percent: rust_decimal::Decimal,
}
```

- [ ] **Step 4: Run engine verification**

Run: `source "$HOME/.cargo/env" && cargo test --workspace`
Expected: PASS for arithmetic, geometric, custom grids, maker TP, taker trailing TP, overall TP/SL, stop, pause, resume, and rebuild behavior.

- [ ] **Step 5: Commit**

```bash
git add apps/trading-engine tests/simulation
git commit -m "feat: 问题描述 add grid engine runtime core"
```

### Task 9: Analytics, Statistics, and Exports

**Files:**
- Create: `apps/api-server/src/routes/analytics.rs`
- Create: `apps/api-server/src/routes/exports.rs`
- Create: `apps/trading-engine/src/statistics.rs`
- Create: `crates/shared-domain/src/analytics.rs`
- Create: `tests/integration/analytics_flow.rs`
- Create: `apps/web/src/app/app/analytics/page.tsx`

- [ ] **Step 1: Write failing analytics coverage**

```rust
#[tokio::test]
async fn compute_strategy_and_account_profit_fee_and_cost_views() {
    assert!(false, "implement analytics projections and exports");
}
```

- [ ] **Step 2: Run the analytics integration test**

Run: `source "$HOME/.cargo/env" && cargo test --test analytics_flow`
Expected: FAIL because statistics projections and export endpoints do not exist.

- [ ] **Step 3: Implement the minimal analytics contracts**

```rust
pub struct StrategyProfitSummary {
    pub realized_pnl: rust_decimal::Decimal,
    pub unrealized_pnl: rust_decimal::Decimal,
    pub fees_paid: rust_decimal::Decimal,
}
```

- [ ] **Step 4: Run analytics verification**

Run: `source "$HOME/.cargo/env" && cargo test --test analytics_flow`
Expected: PASS for per-fill PnL, per-strategy summaries, user aggregates, fee/funding aggregation, and CSV export behavior.

- [ ] **Step 5: Commit**

```bash
git add apps/api-server apps/trading-engine crates/shared-domain tests/integration apps/web
git commit -m "feat: 修复思路 add analytics statistics and exports"
```

### Task 10: Telegram and In-App Notifications

**Files:**
- Create: `apps/api-server/src/routes/telegram.rs`
- Create: `apps/api-server/src/services/telegram_service.rs`
- Create: `crates/shared-events/src/notifications.rs`
- Create: `tests/integration/notification_flow.rs`
- Create: `apps/web/src/app/app/notifications/page.tsx`

- [ ] **Step 1: Write failing notification coverage**

```rust
#[tokio::test]
async fn bind_telegram_and_dispatch_runtime_membership_alerts() {
    assert!(false, "implement telegram bind and notification dispatch");
}
```

- [ ] **Step 2: Run the notification integration test**

Run: `source "$HOME/.cargo/env" && cargo test --test notification_flow`
Expected: FAIL because bind codes and notification dispatch do not exist.

- [ ] **Step 3: Implement the minimal notification contracts**

```rust
pub enum NotificationKind {
    StrategyStarted,
    StrategyPaused,
    MembershipExpiring,
    DepositConfirmed,
    RuntimeError,
}
```

- [ ] **Step 4: Run notification verification**

Run: `source "$HOME/.cargo/env" && cargo test --test notification_flow`
Expected: PASS for Telegram bind, deposit success, membership reminder, runtime failure, and expiry popup signal behavior.

- [ ] **Step 5: Commit**

```bash
git add apps/api-server crates/shared-events tests/integration apps/web
git commit -m "feat: 问题描述 add telegram and notification flows"
```

### Task 11: User Web App Delivery

**Files:**
- Create: `apps/web/src/app/page.tsx`
- Create: `apps/web/src/app/app/dashboard/page.tsx`
- Create: `apps/web/src/app/app/billing/page.tsx`
- Create: `apps/web/src/app/app/strategies/[id]/page.tsx`
- Create: `apps/web/src/app/help/[slug]/page.tsx`
- Create: `tests/e2e/user_app.spec.ts`

- [ ] **Step 1: Write failing user E2E coverage**

```ts
import { test, expect } from "@playwright/test";

test("user can review billing, security, and strategies", async ({ page }) => {
  await page.goto("/app/dashboard");
  await expect(page.locator("main")).toBeVisible();
});
```

- [ ] **Step 2: Run the user E2E test**

Run: `pnpm test:e2e --grep "user can review billing, security, and strategies"`
Expected: FAIL because the user app pages do not exist.

- [ ] **Step 3: Implement the minimal user app routes**

```tsx
export default function DashboardPage() {
  return <main>User Dashboard</main>;
}
```

- [ ] **Step 4: Run user app verification**

Run: `pnpm build && pnpm test:e2e`
Expected: PASS for registration entry, billing center, strategy workspace, analytics pages, help center, and expiry reminder flows.

- [ ] **Step 5: Commit**

```bash
git add apps/web tests/e2e
git commit -m "feat: 修复思路 deliver user web application"
```

### Task 12: Admin Web App Delivery

**Files:**
- Create: `apps/web/src/app/admin/dashboard/page.tsx`
- Create: `apps/web/src/app/admin/users/page.tsx`
- Create: `apps/web/src/app/admin/address-pools/page.tsx`
- Create: `apps/web/src/app/admin/audit/page.tsx`
- Create: `tests/e2e/admin_app.spec.ts`

- [ ] **Step 1: Write failing admin E2E coverage**

```ts
import { test, expect } from "@playwright/test";

test("admin can manage members and address pools", async ({ page }) => {
  await page.goto("/admin/dashboard");
  await expect(page.locator("main")).toBeVisible();
});
```

- [ ] **Step 2: Run the admin E2E test**

Run: `pnpm test:e2e --grep "admin can manage members and address pools"`
Expected: FAIL because the admin pages do not exist.

- [ ] **Step 3: Implement the minimal admin app routes**

```tsx
export default function AdminDashboardPage() {
  return <main>Admin Dashboard</main>;
}
```

- [ ] **Step 4: Run admin verification**

Run: `pnpm build && pnpm test:e2e`
Expected: PASS for member control, price config, address pool expansion, templates, audit logs, and treasury sweep views.

- [ ] **Step 5: Commit**

```bash
git add apps/web tests/e2e
git commit -m "feat: 问题描述 deliver admin web application"
```

### Task 13: Deployment, Docs, and Release Hardening

**Files:**
- Create: `deploy/docker/docker-compose.yml`
- Create: `deploy/docker/*.Dockerfile`
- Create: `deploy/nginx/default.conf`
- Create: `deploy/monitoring/*.yml`
- Create: `docs/user-guide/*.md`
- Create: `docs/admin-guide/*.md`
- Create: `docs/deployment/*.md`
- Create: `scripts/smoke.sh`
- Create: `tests/verification/compose.test.mjs`

- [ ] **Step 1: Write failing deployment verification**

```js
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("compose and docs assets exist", () => {
  assert.ok(fs.existsSync("deploy/docker/docker-compose.yml"));
  assert.ok(fs.existsSync("docs/user-guide"));
  assert.ok(fs.existsSync("docs/admin-guide"));
});
```

- [ ] **Step 2: Run the deployment verification**

Run: `node --test tests/verification/*.test.mjs`
Expected: FAIL because deployment and docs assets do not exist.

- [ ] **Step 3: Implement the release assets**

```yaml
services:
  api-server:
    build:
      context: ../..
      dockerfile: deploy/docker/api-server.Dockerfile
```

```bash
#!/usr/bin/env bash
set -euo pipefail
docker compose up -d --build
```

- [ ] **Step 4: Run final release verification**

Run: `source "$HOME/.cargo/env" && cargo fmt --all --check && cargo test --workspace && pnpm build && pnpm test:e2e && node --test tests/verification/*.test.mjs && docker compose up -d --build && ./scripts/smoke.sh`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add deploy docs scripts tests/verification
git commit -m "chore: 修复思路 add deployment docs and release hardening"
```

## Requirement Coverage Map

- Foundation web shell, root build chain, lockfile, and build artifact hygiene: Task 2
- Auth, registration, password reset, 2FA, security center: Task 3
- Membership pricing, chain payment, grace period, address pools, admin override: Task 4
- Binance API test, symbol sync, symbol search, futures checks: Task 5
- Strategy save/edit/pause/start/pre-flight/template flow: Task 6
- WebSocket market data, active subscription, hourly scheduler sync: Task 7
- Grid modes, custom grids, TP/SL, trailing TP, stop/rebuild: Task 8
- Wallet/trade/funding/fee stats, strategy-level stats, exports: Task 9
- Telegram binding and runtime/member notifications: Task 10
- User app pages and help center: Task 11
- Admin pages, address pool expansion, audit logs, pricing config: Task 12
- Docker Compose, monitoring, user/admin/deployment docs: Task 13

## Self-Review

- Spec coverage: all sections from the approved design are mapped into the thirteen tasks above.
- Placeholder scan: no `TODO`, `TBD`, or deferred unnamed steps are left in the plan.
- Scope check: the plan is intentionally decomposed into thirteen implementation batches to keep each execution step reviewable and testable.
