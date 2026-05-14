# Backtest Result Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add safe archive/delete management for martingale backtest tasks so the UI stays clean without losing published portfolio integrity.

**Architecture:** Extend the existing shared-db repository with owner-scoped archive/delete operations, expose them through BacktestService and Axum routes, proxy them in Next.js, then wire BacktestTaskList actions and filters. Deletion is hard delete for task/candidates/artifacts/events only when terminal and unreferenced; archive is a reversible hidden marker in `summary`.

**Tech Stack:** Rust, shared-db, Axum api-server, Next.js App Router, TypeScript, node contract tests.

---

### Task 1: Repository Archive/Delete

**Files:**
- Modify: `crates/shared-db/src/backtest.rs`

- [ ] Add failing tests for archive marker and delete cascade.
- [ ] Implement `archive_task(task_id)` by merging `summary.archived=true`, `archived_at`, and appending event.
- [ ] Implement `delete_task(owner, task_id)` with owner/status/reference guards and cascade delete.
- [ ] Run `cargo test -p shared-db backtest -- --nocapture`.

### Task 2: API Service and Routes

**Files:**
- Modify: `apps/api-server/src/services/backtest_service.rs`
- Modify: `apps/api-server/src/routes/backtest.rs`
- Modify: `apps/api-server/tests/martingale_backtest_flow.rs` or adjacent API tests

- [ ] Add API/service tests for archive/delete ownership and state guards.
- [ ] Add `Archive/Delete` service methods.
- [ ] Add `POST /backtest/tasks/{id}/archive` and `DELETE /backtest/tasks/{id}`.
- [ ] Run `cargo test -p api-server martingale -- --nocapture`.

### Task 3: Next Proxy and UI

**Files:**
- Modify: `apps/web/app/api/user/backtest/tasks/[id]/route.ts`
- Create: `apps/web/app/api/user/backtest/tasks/[id]/archive/route.ts`
- Modify: `apps/web/components/backtest/backtest-task-list.tsx`
- Modify: `apps/web/components/backtest/backtest-console.tsx`
- Modify: `apps/web/lib/api-types.ts` if needed

- [ ] Add route contract test expectations for archive/delete proxy.
- [ ] Add filter tabs and per-task archive/delete buttons.
- [ ] Wire actions in BacktestConsole with refresh and selected-task cleanup.
- [ ] Run `pnpm --filter web exec tsc --noEmit`.

### Task 4: Verification and Restart

**Files:**
- Modify: none expected beyond code above

- [ ] Run repository/API/frontend contract tests.
- [ ] Rebuild/restart `web`, `api-server`, and `backtest-worker` if backend changed.
- [ ] Verify `http://127.0.0.1:8080/zh/app/backtest` is reachable.
