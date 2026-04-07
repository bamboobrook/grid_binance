# Binance Grid SaaS Remaining Spec Gap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining hard spec gaps found in the April 7 audit so the `full-v1` worktree aligns with the frozen March 31 Binance Grid SaaS design without leaving mock runtime, fake sweep, or placeholder analytics behavior in production paths.

**Architecture:** Keep the current Rust service split and Next.js route partitioning, but finish the last truth-boundary fixes: strategy readiness must be server-derived, sweeps must have a real submission lifecycle, analytics must stop writing zero placeholders, and runtime notifications must cover every failure path. User and admin pages should reflect those backend truths directly.

**Tech Stack:** Rust, Axum, SQLx/shared-db, PostgreSQL, Redis, Next.js App Router, TypeScript, Binance REST/WebSocket, EVM RPC, Solana RPC.

---

## File Structure

### Strategy payload truth and pre-flight closure
- Modify: `apps/web/src/app/api/user/strategies/create/route.ts`
- Modify: `apps/web/src/app/api/user/strategies/[id]/route.ts`
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `crates/shared-domain/src/strategy.rs`
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Modify: `tests/e2e/user_commercial_flows.spec.ts`

### Sweep execution lifecycle closure
- Modify: `apps/api-server/src/services/membership_service.rs`
- Modify: `apps/api-server/src/routes/admin_sweeps.rs`
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `apps/billing-chain-listener/src/rpc.rs`
- Modify: `apps/api-server/tests/admin_deposit_flow.rs`
- Modify: `apps/api-server/tests/admin_address_pools_flow.rs`

### Analytics precision closure
- Modify: `crates/shared-binance/src/client.rs`
- Modify: `apps/scheduler/src/main.rs`
- Modify: `apps/api-server/src/services/analytics_service.rs`
- Modify: `apps/trading-engine/src/statistics.rs`
- Modify: `apps/api-server/tests/analytics_flow.rs`

### Runtime notification and live failure closure
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/execution_effects.rs`
- Modify: `apps/trading-engine/src/trade_sync.rs`
- Modify: `apps/api-server/tests/notification_flow.rs`
- Modify: `apps/trading-engine/tests/execution_effects.rs`
- Modify: `apps/trading-engine/tests/trade_sync.rs`

### Docs and acceptance closure
- Modify: `.env.example`
- Modify: `docs/deployment/env-and-secrets.md`
- Modify: `docs/deployment/docker-compose.md`
- Modify: `docs/user-guide/create-grid-strategy.md`
- Modify: `docs/user-guide/manage-strategy.md`
- Modify: `docs/user-guide/membership-and-payment.md`

## Task 1: Remove Client Authored Readiness State From Strategy Save Paths

**Files:**
- Modify: `apps/web/src/app/api/user/strategies/create/route.ts`
- Modify: `apps/web/src/app/api/user/strategies/[id]/route.ts`
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `crates/shared-domain/src/strategy.rs`
- Test: `apps/api-server/tests/strategy_flow.rs`
- Test: `tests/e2e/user_commercial_flows.spec.ts`

- [ ] Step 1: Add or update a failing API test proving strategy create/update requests can omit readiness booleans and that pre-flight still derives status from membership, exchange account, symbol metadata, and wallet snapshots.
- [ ] Step 2: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow` and confirm the new assertion fails on the current request shape or fallback behavior.
- [ ] Step 3: Change the web create/update routes so they only submit user-owned trading parameters: name, symbol, market, mode, generation, levels, overall TP/SL, and post-trigger action.
- [ ] Step 4: Change `SaveStrategyRequest` and related server save/build logic so readiness flags are no longer trusted from the client, and make pre-flight use server-derived values without falling back to stored client booleans.
- [ ] Step 5: Re-run `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow`.
- [ ] Step 6: Re-run `pnpm test:e2e -- --grep "user commercial"` after extending the strategy journey assertions.

## Task 2: Replace Fake Sweep Completion With A Real Sweep Lifecycle

**Files:**
- Modify: `apps/api-server/src/services/membership_service.rs`
- Modify: `apps/api-server/src/routes/admin_sweeps.rs`
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `apps/billing-chain-listener/src/rpc.rs`
- Test: `apps/api-server/tests/admin_deposit_flow.rs`
- Test: `apps/api-server/tests/admin_address_pools_flow.rs`

- [ ] Step 1: Add or update failing admin tests proving new sweep jobs are not written as immediate `completed` jobs with fake `sweep-*` hashes.
- [ ] Step 2: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test admin_deposit_flow --test admin_address_pools_flow` and confirm the sweep assertions fail before implementation.
- [ ] Step 3: Change sweep creation so the API records a pending/submitted lifecycle, leaves tx hash empty until a real submission exists, and writes audit details for requested transfers.
- [ ] Step 4: Add chain-specific sweep submission helpers that can emit real submission metadata for ETH, BSC, and SOL when signer configuration is present, and persist terminal success/failure plus real tx hash or signature.
- [ ] Step 5: Update admin sweep listing and tests so UI/API expose lifecycle state truthfully instead of treating every request as completed.
- [ ] Step 6: Re-run `source "$HOME/.cargo/env" && cargo test -p api-server --test admin_deposit_flow --test admin_address_pools_flow`.

## Task 3: Remove Remaining Fee And Funding Placeholders From Analytics

**Files:**
- Modify: `crates/shared-binance/src/client.rs`
- Modify: `apps/scheduler/src/main.rs`
- Modify: `apps/api-server/src/services/analytics_service.rs`
- Modify: `apps/trading-engine/src/statistics.rs`
- Test: `apps/api-server/tests/analytics_flow.rs`

- [ ] Step 1: Add or update a failing analytics test proving strategy and account reports no longer flatten funding totals to zero when snapshot data exists, and that snapshot sync does not persist fake fee values.
- [ ] Step 2: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test analytics_flow && cargo test -p scheduler` and confirm the new assertions fail.
- [ ] Step 3: Replace hardcoded `"0"` fee fields and `Decimal::ZERO` funding placeholders with best-available exchange/snapshot values, keeping unknown values explicit instead of silently faking correctness.
- [ ] Step 4: Make strategy snapshot sync derive unrealized PnL, fees, and funding totals from runtime fills and the latest account data rather than writing zero placeholders.
- [ ] Step 5: Re-run `source "$HOME/.cargo/env" && cargo test -p api-server --test analytics_flow && cargo test -p scheduler && cargo test -p trading-engine`.

## Task 4: Close Runtime Error And Notification Gaps

**Files:**
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/execution_effects.rs`
- Modify: `apps/trading-engine/src/trade_sync.rs`
- Test: `apps/api-server/tests/notification_flow.rs`
- Test: `apps/trading-engine/tests/execution_effects.rs`
- Test: `apps/trading-engine/tests/trade_sync.rs`

- [ ] Step 1: Add failing tests proving `live_order_sync_failed` emits the same runtime-error notification path as other runtime exceptions, and that fill/business notifications still write a telegram log row with `failed` status when a binding exists but delivery cannot happen.
- [ ] Step 2: Run `source "$HOME/.cargo/env" && cargo test -p trading-engine && cargo test -p api-server --test notification_flow` and confirm the new assertions fail.
- [ ] Step 3: Route live order sync failures through the same user-readable remediation notification path used by runtime exception auto-pause.
- [ ] Step 4: Make fill and fill-profit notification writers record telegram delivery outcome rows whenever a Telegram binding exists, including explicit failed rows when bot token or delivery is unavailable.
- [ ] Step 5: Re-run `source "$HOME/.cargo/env" && cargo test -p trading-engine && cargo test -p api-server --test notification_flow`.

## Task 5: Close Docs And Acceptance Evidence

**Files:**
- Modify: `.env.example`
- Modify: `docs/deployment/env-and-secrets.md`
- Modify: `docs/deployment/docker-compose.md`
- Modify: `docs/user-guide/create-grid-strategy.md`
- Modify: `docs/user-guide/manage-strategy.md`
- Modify: `docs/user-guide/membership-and-payment.md`

- [ ] Step 1: Update env and deployment docs to describe the real sweep signer / RPC requirements, market tick runtime requirements, and the exact behavior of membership grace popups.
- [ ] Step 2: Update user guides so strategy editing, pre-flight, trailing TP fee warning, payment exact-match rules, and membership grace handling match the actual implementation after Tasks 1-4.
- [ ] Step 3: Run `pnpm build:web && node --test tests/verification/web_app_shell.test.mjs`.
- [ ] Step 4: Run the final focused acceptance set:
  - `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow --test admin_deposit_flow --test admin_address_pools_flow --test analytics_flow --test notification_flow`
  - `source "$HOME/.cargo/env" && cargo test -p trading-engine`
  - `source "$HOME/.cargo/env" && cargo test -p billing-chain-listener`
  - `source "$HOME/.cargo/env" && cargo test -p market-data-gateway`

## Self-Review

- Spec coverage: this plan targets the remaining non-aligned areas from the frozen March 31 spec audit: strategy server truth, real sweep execution, analytics precision, runtime notification completeness, and operator/user docs.
- Placeholder scan: no `TODO` / `TBD` markers are intentionally left in the plan.
- Type consistency: task names and file targets match current code locations in the `full-v1` worktree.
