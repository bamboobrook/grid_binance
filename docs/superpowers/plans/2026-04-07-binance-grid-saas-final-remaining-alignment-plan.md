# Binance Grid SaaS Final Remaining Alignment Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the last remaining spec gaps after the April 7 audit so the `full-v1` worktree aligns with the frozen March 31 Binance Grid SaaS design across analytics truth, sweep execution, listener truth boundaries, and residual mock surfaces.

**Architecture:** Keep the current multi-service Rust + Next.js architecture, but finish the last truth-boundary repairs. Runtime/business truth must come from persisted exchange snapshots, listener-owned confirmation flow, and chain-specific sweep execution records instead of placeholders, fake completions, or UI-only mock data.

**Tech Stack:** Rust, Axum, SQLx/shared-db, PostgreSQL, Redis, Next.js App Router, TypeScript, Binance REST/WebSocket, EVM RPC, Solana RPC.

---

## Remaining Scope

### Analytics truth closure
- Modify: `crates/shared-binance/src/client.rs`
- Modify: `apps/scheduler/src/main.rs`
- Modify: `apps/api-server/src/services/analytics_service.rs`
- Modify: `apps/trading-engine/src/statistics.rs`
- Modify: `apps/api-server/tests/analytics_flow.rs`

### Real sweep lifecycle closure
- Modify: `apps/api-server/src/services/membership_service.rs`
- Modify: `apps/api-server/src/routes/admin_sweeps.rs`
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `apps/billing-chain-listener/src/rpc.rs`
- Modify: `apps/api-server/tests/admin_deposit_flow.rs`
- Modify: `apps/api-server/tests/admin_address_pools_flow.rs`

### Listener truth boundary closure
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `apps/billing-chain-listener/src/processor.rs`
- Modify: `apps/api-server/tests/membership_flow.rs`
- Modify: `tests/e2e/admin_commercial_flows.spec.ts`

### Residual mock/docs closure
- Modify: `apps/web/src/lib/api/mock-data.ts`
- Modify: `docs/deployment/env-and-secrets.md`
- Modify: `docs/deployment/docker-compose.md`
- Modify: `docs/user-guide/membership-and-payment.md`

## Task 1: Remove analytics placeholders and derive funding/fees from stored truth
- [ ] Write failing analytics tests for strategy snapshot funding/unrealized and account snapshot fee placeholders.
- [ ] Run `source "$HOME/.cargo/env" && cargo test -p api-server --test analytics_flow` and confirm failure.
- [ ] Replace snapshot placeholder fees/funding in `shared-binance` and `scheduler` with explicit best-available values.
- [ ] Update analytics aggregation to stop flattening strategy funding to zero when snapshot data exists.
- [ ] Re-run `source "$HOME/.cargo/env" && cargo test -p api-server --test analytics_flow && cargo test -p scheduler`.

## Task 2: Turn sweep jobs from pending records into real execution lifecycle
- [ ] Write failing admin sweep tests for `requested -> submitted/failed` lifecycle, real tx hash persistence, and no fake completion state.
- [ ] Run targeted admin deposit/address-pool tests and confirm failure.
- [ ] Add sweep submission metadata + terminal status handling for ETH/BSC/SOL.
- [ ] Update admin sweep listing/docs to expose lifecycle truthfully.
- [ ] Re-run `source "$HOME/.cargo/env" && cargo test -p api-server --test admin_deposit_flow --test admin_address_pools_flow`.

## Task 3: Remove listener-side confirmation/source bypasses
- [ ] Write failing tests proving internal ingest cannot trust external `confirmations/observed_at` as final truth in production paths.
- [ ] Tighten listener internal ingest so it no longer acts as a silent confirmation bypass, or clearly gates it to non-production/testing flows.
- [ ] Re-run `source "$HOME/.cargo/env" && cargo test -p billing-chain-listener && cargo test -p api-server --test membership_flow`.

## Task 4: Remove residual mock surfaces and align docs
- [ ] Remove stale sweep/mock data from web mock helpers that no longer match product truth.
- [ ] Update deployment and user docs for the hardened billing flow and sweep/runtime requirements.
- [ ] Re-run `pnpm build:web && node --test tests/verification/web_app_shell.test.mjs`.

## Final Acceptance
- [ ] `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow --test membership_flow --test admin_deposit_flow --test analytics_flow`
- [ ] `source "$HOME/.cargo/env" && cargo test -p trading-engine`
- [ ] `source "$HOME/.cargo/env" && cargo test -p billing-chain-listener`
- [ ] `pnpm build:web`
