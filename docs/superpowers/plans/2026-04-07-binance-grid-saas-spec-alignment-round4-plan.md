# Binance Grid SaaS Spec Alignment Round 4 Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining spec mismatches found in the April 7 re-audit so the `full-v1` worktree aligns with the frozen March 31 Binance Grid SaaS design across futures strategy configuration, admin templates, user visibility, analytics history, lifecycle semantics, and deployment acceptance.

**Architecture:** Keep the current Rust services plus Next.js app. Do not redesign the product. Extend the existing strategy/domain models and pages only where the frozen spec still has clear gaps: futures margin/leverage must become first-class saved inputs, templates must support full ladders, user pages must expose real history/runtime/error data instead of partial summaries, strategy lifecycle semantics must stop claiming exchange actions that have not actually happened, and deployment/testing docs must stop pretending incomplete local defaults are production-ready.

**Tech Stack:** Rust, Axum, SQLx/shared-db, PostgreSQL, Redis, Next.js App Router, TypeScript, Binance REST/WebSocket, EVM/Solana RPC.

---

### Task 1: Add futures margin mode, leverage, and durable amount-mode metadata

**Files:**
- Modify: `crates/shared-domain/src/strategy.rs`
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `apps/web/src/app/app/strategies/new/page.tsx`
- Modify: `apps/web/src/app/app/strategies/[id]/page.tsx`
- Modify: `apps/web/src/app/api/user/strategies/create/route.ts`
- Modify: `apps/web/src/app/api/user/strategies/[id]/route.ts`
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Modify: `docs/user-guide/create-grid-strategy.md`
- Modify: `docs/user-guide/manage-strategy.md`

- [ ] Add failing strategy tests proving futures strategies can save margin mode and leverage, and strategy amount mode survives edit round-trips.
- [ ] Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow`
- [ ] Add revision/domain fields for futures margin mode, leverage, and amount mode; thread them through create/update/template/apply flows, and keep them optional for spot.
- [ ] Update user create/edit pages so futures forms expose margin mode and leverage with saved round-trip behavior.
- [ ] Update docs so users can configure futures leverage and isolated/cross margin from the product flow.
- [ ] Re-run: `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow && pnpm build:web`

### Task 2: Make admin templates support full ladders, not fixed two-level forms

**Files:**
- Modify: `apps/web/src/app/admin/templates/page.tsx`
- Modify: `apps/web/src/app/api/admin/templates/route.ts`
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Modify: `docs/admin-guide/template-management.md`

- [ ] Add failing coverage proving admin template create/update can persist 3+ levels and custom ladders.
- [ ] Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow`
- [ ] Replace the fixed two-level template form with batch/custom JSON ladder editing that can express the full template strategy config.
- [ ] Keep template application copying the saved ladder exactly into user-owned drafts.
- [ ] Update admin docs to explain full-ladder template editing.
- [ ] Re-run: `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow && pnpm build:web`

### Task 3: Close user workflow visibility gaps in strategies, exchange, analytics, and reminders

**Files:**
- Modify: `apps/web/src/lib/api/mock-data.ts`
- Modify: `apps/web/src/lib/api/server.ts`
- Modify: `apps/web/src/components/ui/dialog.tsx`
- Modify: `apps/web/src/app/app/layout.tsx`
- Modify: `apps/web/src/app/app/strategies/page.tsx`
- Modify: `apps/web/src/app/app/strategies/[id]/page.tsx`
- Modify: `apps/web/src/app/app/exchange/page.tsx`
- Modify: `apps/web/src/app/app/orders/page.tsx`
- Modify: `apps/web/src/app/app/analytics/page.tsx`
- Modify: `apps/web/src/app/app/help/page.tsx`
- Modify: `apps/web/src/lib/api/help-articles.ts`
- Modify: `tests/verification/web_app_shell.test.mjs`
- Modify: `tests/e2e/user_app.spec.ts`
- Modify: `tests/e2e/user_commercial_flows.spec.ts`
- Modify: `docs/user-guide/getting-started.md`
- Modify: `docs/user-guide/manage-strategy.md`
- Modify: `docs/user-guide/troubleshooting.md`

- [ ] Add failing web verification/E2E assertions for analytics nav visibility, filtered bulk actions, runtime event visibility, exchange validation details, and expiry popup rendering.
- [ ] Run: `pnpm build:web && node --test tests/verification/web_app_shell.test.mjs`
- [ ] Add analytics to user navigation and expose filter-driven bulk strategy actions from the visible result set.
- [ ] Show strategy runtime events, failed steps, and remediation guidance directly in the strategy workspace.
- [ ] Make exchange connection failures show exact validation failures instead of generic pass/fail copy.
- [ ] Render membership expiry/grace reminders as an actual modal popup rather than only inline banners.
- [ ] Expand user help coverage for analytics, orders/history, exports, and runtime recovery.
- [ ] Re-run: `pnpm build:web && node --test tests/verification/web_app_shell.test.mjs`

### Task 4: Expose real exchange trade history and stop publishing fake strategy/account truth

**Files:**
- Modify: `crates/shared-domain/src/analytics.rs`
- Modify: `crates/shared-binance/src/client.rs`
- Modify: `apps/api-server/src/services/analytics_service.rs`
- Modify: `apps/scheduler/src/main.rs`
- Modify: `apps/trading-engine/src/trade_sync.rs`
- Modify: `apps/api-server/tests/analytics_flow.rs`
- Modify: `apps/web/src/app/app/orders/page.tsx`
- Modify: `apps/web/src/app/app/analytics/page.tsx`

- [ ] Add failing analytics tests proving the report carries exchange trade history rows and that strategy/account aggregates do not flatten realized/funding/fees to fake zeros.
- [ ] Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test analytics_flow`
- [ ] Extend the analytics report with recent exchange trade history records and show them in user orders/analytics pages.
- [ ] Stop treating `ExchangeFill.realized_pnl = None` and zeroed account snapshots as production truth when user-visible statistics are computed.
- [ ] Re-run: `source "$HOME/.cargo/env" && cargo test -p api-server --test analytics_flow && cargo test -p trading-engine && pnpm build:web`

### Task 5: Correct lifecycle semantics for stop, delete, futures balance checks, and resume

**Files:**
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `apps/trading-engine/src/order_sync.rs`
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Modify: `docs/user-guide/manage-strategy.md`

- [ ] Add failing strategy tests proving stop/delete do not claim exchange completion prematurely, futures pre-flight checks the correct wallet scope, and resume/rebuild is explicit about current-market limitations.
- [ ] Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow`
- [ ] Tighten stop/delete guards so archive is blocked while exchange-linked orders still need cancellation reconciliation.
- [ ] Fix futures balance/pre-flight account selection to respect market scope instead of always reading the generic wallet snapshot bucket.
- [ ] Make resume/rebuild user-visible messaging honest about what current-market data is used.
- [ ] Re-run: `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow`

### Task 6: Remove deployment acceptance gaps around mail and sweep execution

**Files:**
- Modify: `.env.example`
- Modify: `deploy/docker/docker-compose.yml`
- Modify: `docs/deployment/env-and-secrets.md`
- Modify: `docs/deployment/docker-compose.md`
- Modify: `tests/verification/compose.test.mjs`
- Modify: `tests/verification/commercial_docs_and_acceptance.test.mjs`
- Create: `deploy/docker/mailpit-compose.override.example.yml`

- [ ] Add failing deployment verification proving operators get a documented mail path for registration/password-reset and explicit acceptance caveats for sweep execution instead of dead defaults.
- [ ] Run: `node --test tests/verification/compose.test.mjs tests/verification/commercial_docs_and_acceptance.test.mjs`
- [ ] Replace dead default mail placeholders with runnable local acceptance guidance and compose-local mail capture instructions.
- [ ] Document the sweep executor requirement as a hard deployment dependency until an in-repo signer service exists, so deployment docs stop presenting the stack as fully closed when it is not.
- [ ] Re-run: `node --test tests/verification/compose.test.mjs tests/verification/commercial_docs_and_acceptance.test.mjs`

### Final Acceptance

- [ ] `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow --test membership_flow --test admin_deposit_flow --test analytics_flow --test notification_flow --test auth_flow`
- [ ] `source "$HOME/.cargo/env" && cargo test -p billing-chain-listener`
- [ ] `source "$HOME/.cargo/env" && cargo test -p trading-engine`
- [ ] `pnpm build:web`
- [ ] `node --test tests/verification/web_app_shell.test.mjs tests/verification/compose.test.mjs tests/verification/commercial_docs_and_acceptance.test.mjs`
