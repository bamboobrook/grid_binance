# Binance Grid SaaS Spec Alignment Round 3 Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the final spec mismatches still present after the April 7 re-audit so the `full-v1` worktree matches the frozen March 31 Binance Grid SaaS design across sweep execution, analytics truth, user strategy workflow, billing UX, and operator/user documentation.

**Architecture:** Keep the current Rust multi-service + Next.js structure. Do not redesign the product. Fix only the remaining requirement mismatches: turn sweep jobs into executable lifecycle records, stop analytics from publishing placeholder values as truth, expose the missing strategy and billing controls in the user app, and align docs/tests with the now-real runtime paths.

**Tech Stack:** Rust, Axum, SQLx/shared-db, PostgreSQL, Redis, Next.js App Router, TypeScript, Playwright, Binance REST/WebSocket, EVM/Solana RPC.

---

### Task 1: Close sweep execution lifecycle

**Files:**
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `apps/billing-chain-listener/src/rpc.rs`
- Modify: `apps/api-server/src/services/membership_service.rs`
- Modify: `apps/api-server/src/routes/admin_sweeps.rs`
- Modify: `crates/shared-db/src/postgres/billing.rs`
- Modify: `apps/api-server/tests/admin_deposit_flow.rs`
- Modify: `apps/api-server/tests/admin_address_pools_flow.rs`
- Modify: `docs/deployment/env-and-secrets.md`
- Modify: `docs/deployment/docker-compose.md`

- [ ] Add failing sweep tests for `pending -> submitted/failed/confirmed` lifecycle visibility and persisted tx references.
- [ ] Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test admin_deposit_flow --test admin_address_pools_flow`
- [ ] Implement DB claim/update helpers for sweep jobs and transfers.
- [ ] Implement listener submission and confirmation loops with a real executor boundary and persisted lifecycle timestamps/errors.
- [ ] Expose full lifecycle fields through `/admin/sweeps` and document required signer/executor env.
- [ ] Re-run: `source "$HOME/.cargo/env" && cargo test -p api-server --test admin_deposit_flow --test admin_address_pools_flow && cargo test -p billing-chain-listener`

### Task 2: Remove analytics placeholder truth

**Files:**
- Modify: `crates/shared-binance/src/client.rs`
- Modify: `apps/scheduler/src/main.rs`
- Modify: `apps/api-server/src/services/analytics_service.rs`
- Modify: `apps/api-server/tests/analytics_flow.rs`
- Modify: `apps/trading-engine/src/statistics.rs`

- [ ] Add failing analytics tests proving spot/futures snapshot placeholders no longer overwrite user-visible fees/funding/realized values and fills no longer fake funding as zero.
- [ ] Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test analytics_flow`
- [ ] Replace snapshot placeholder values with best-available truth or explicit absence.
- [ ] Keep strategy/user aggregation from flattening funding, fees, and unrealized values to fake zero.
- [ ] Re-run: `source "$HOME/.cargo/env" && cargo test -p api-server --test analytics_flow && cargo test -p scheduler && cargo test -p trading-engine`

### Task 3: Close user workflow gaps in strategies, billing, Telegram, and docs

**Files:**
- Modify: `apps/web/src/app/app/strategies/page.tsx`
- Modify: `apps/web/src/app/app/strategies/new/page.tsx`
- Modify: `apps/web/src/app/app/strategies/[id]/page.tsx`
- Modify: `apps/web/src/app/api/user/strategies/create/route.ts`
- Modify: `apps/web/src/app/api/user/strategies/[id]/route.ts`
- Modify: `apps/web/src/app/api/user/strategies/batch/route.ts`
- Modify: `apps/web/src/app/app/billing/page.tsx`
- Modify: `apps/web/src/app/api/user/billing/route.ts`
- Modify: `apps/web/src/app/app/telegram/page.tsx`
- Modify: `apps/api-server/src/services/membership_service.rs`
- Modify: `tests/e2e/user_commercial_flows.spec.ts`
- Modify: `tests/verification/web_app_shell.test.mjs`
- Modify: `docs/user-guide/create-grid-strategy.md`
- Modify: `docs/user-guide/manage-strategy.md`
- Modify: `docs/user-guide/membership-and-payment.md`

- [ ] Add failing web verification/E2E assertions for batch start, strategy amount-mode controls, billing address/queue visibility, and Telegram delivery wording.
- [ ] Run: `pnpm build:web && node --test tests/verification/web_app_shell.test.mjs`
- [ ] Expose batch start for selected strategies, plus stop-all only as the global action.
- [ ] Replace JSON-only strategy editing with real amount/quote-mode, batch spacing/TP controls, while keeping advanced custom JSON editing available.
- [ ] Surface billing address, queue position, and lock-expiry semantics in the page and redirect notice.
- [ ] Fix Telegram delivery wording so “not bound / failed / web only / delivered” are distinguishable.
- [ ] Expand user docs to cover API permission setup, strategy creation, batch/manual grid editing, restart rules, membership payment warnings, and remediation guidance.
- [ ] Re-run: `pnpm build:web && node --test tests/verification/web_app_shell.test.mjs`

### Final Acceptance

- [ ] `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow --test membership_flow --test admin_deposit_flow --test analytics_flow --test notification_flow --test auth_flow`
- [ ] `source "$HOME/.cargo/env" && cargo test -p billing-chain-listener`
- [ ] `source "$HOME/.cargo/env" && cargo test -p scheduler`
- [ ] `source "$HOME/.cargo/env" && cargo test -p trading-engine`
- [ ] `pnpm build:web`
